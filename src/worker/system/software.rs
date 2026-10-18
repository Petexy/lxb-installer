//! The software steps the person chose: updates (with the account's GNOME
//! Shell extensions brought up to date after them, as legacy did), removing
//! deselected packages, and Flatpak applications.
//!
//! None of them can make the installed system unusable by failing, so none
//! of them stops the installation: whatever does not work becomes a warning
//! the interface words, and the system is left as the image had it.
use super::check::{valid_flatpak, valid_package};
use super::{accounts, packages, plan, Install};
use crate::model::protocol::{Step, Warning};
use crate::worker::layout::Stop;
use crate::worker::{run, target};

/// Where Flatpak applications come from.
pub const FLATHUB: &str = "flathub";
pub const FLATHUB_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

/// What the Updates step does, given whether the internet answered and
/// whether the system was already brought up to date in this run (by a
/// desktop downloaded before it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdatesPlan {
    Skip(Warning),
    AlreadyDone,
    Run,
}

/// The tool the legacy installer updated the account's GNOME Shell
/// extensions with once the system was up to date (`post-install.sh:351-362`),
/// from linexin-repo. It is downloaded for that and removed again, as legacy
/// did, so it is not one of the live system's programs nor a dependency of
/// this package (the snapshots step downloads Timeshift the same way).
pub const EXTENSIONS_TOOL: &str = "gnome-extensions-cli";

/// Where an account's own GNOME Shell extensions are, inside its home:
/// Linexin's come there from the skeleton.
pub const EXTENSIONS_DIR: &str = ".local/share/gnome-shell/extensions";

pub fn updates_plan(online: bool, synced: bool) -> UpdatesPlan {
    match (online, synced) {
        (false, _) => UpdatesPlan::Skip(Warning::UpdatesSkippedOffline),
        (true, true) => UpdatesPlan::AlreadyDone,
        (true, false) => UpdatesPlan::Run,
    }
}

/// `Step::Updates`: the whole system brought up to date, Arch's keyring
/// first.
pub fn updates(install: &mut Install) -> Result<(), Stop> {
    match updates_plan(install.online, install.synced) {
        UpdatesPlan::Skip(warning) => {
            run::log("updates: offline, skipped");
            install.warn(warning);
        }
        UpdatesPlan::AlreadyDone => {
            run::log("updates: the system was brought up to date earlier in this run");
            refresh_extensions(install);
        }
        UpdatesPlan::Run => match packages::upgrade(install, Step::Updates) {
            Ok(()) => refresh_extensions(install),
            Err(stop) => {
                run::log(&format!("updates: {}", stop.detail));
                install.warn(Warning::UpdatesFailed);
            }
        },
    }
    Ok(())
}

/// Whether the account's extensions are brought up to date with the GNOME
/// Shell the upgrade installed: when GNOME stays (Linexin; Kinexin and
/// LineXinBar removed it in the Desktop step, before), the account has
/// extensions of its own, and its home is one the installer made — a kept
/// /home is the person's and is left as it is.
///
/// GNOME Shell 51 looks for extension updates itself only when an
/// extensions app is installed (its `updatesSupported`), and the 3.0 image
/// has none, so without this the extensions stay as old as the image while
/// GNOME moves on.
pub fn refreshes_extensions(gnome_shell: bool, has_extensions: bool, home_is_new: bool) -> bool {
    gnome_shell && has_extensions && home_is_new
}

/// How long the extensions may take, in seconds. gext downloads without a
/// time limit of its own, and an installation must not wait for ever on a
/// server that stopped answering.
pub const EXTENSIONS_TIME_LIMIT: &str = "600";

/// What updates the account's extensions, `timeout`'s arguments: gext run
/// as the account through `runuser`, reading and writing the extensions
/// folder directly (`--filesystem`, since no session is running to ask),
/// asking nothing, in a cleared environment with the account's own home, so
/// neither root's home nor the worker's settings reach it. With no session
/// bus gext cannot change which extensions are switched on, so they stay as
/// the image's settings have them, as they did under legacy's `sudo -u`.
pub fn gext_args(user: &str) -> Vec<String> {
    vec![
        "--kill-after=30".into(),
        EXTENSIONS_TIME_LIMIT.into(),
        "/usr/bin/runuser".into(),
        "-u".into(),
        user.into(),
        "--".into(),
        "/usr/bin/env".into(),
        "-i".into(),
        format!("HOME=/home/{user}"),
        format!("XDG_DATA_HOME=/home/{user}/.local/share"),
        "PATH=/usr/bin".into(),
        "LANG=C.UTF-8".into(),
        "gext".into(),
        "--filesystem".into(),
        "update".into(),
        "-y".into(),
    ]
}

/// Brings the account's GNOME Shell extensions up to date from
/// extensions.gnome.org with [`EXTENSIONS_TOOL`], downloaded for it and
/// removed again unless the system had it. Like legacy's, it is a courtesy:
/// whatever does not work is logged and the extensions stay as the image
/// had them.
fn refresh_extensions(install: &mut Install) {
    let root = install.root().to_path_buf();
    let user = install.config.account.username.clone();
    let wanted = refreshes_extensions(
        target::exists(&root, "usr/bin/gnome-shell"),
        target::is_dir(&root, &format!("home/{user}/{EXTENSIONS_DIR}")),
        accounts::home_is_new(install.config),
    );
    if !wanted {
        run::log("updates: no GNOME extensions of the account's to bring up to date");
        return;
    }
    let had_tool = packages::installed(install, EXTENSIONS_TOOL);
    if !had_tool {
        if let Err(stop) = packages::install(install, &[EXTENSIONS_TOOL]) {
            run::log(&format!(
                "updates: the GNOME extensions stay as they were: {}",
                stop.detail
            ));
            return;
        }
    }
    let args = gext_args(&user);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match run::chroot_streaming(&root, "timeout", &args, &mut |line| run::log(line)) {
        Ok(()) => run::log("updates: the GNOME extensions are up to date"),
        Err(stop) => run::log(&format!(
            "updates: the GNOME extensions stay as they were: {}",
            stop.detail
        )),
    }
    if !had_tool {
        if let Err(stop) = packages::remove(install, EXTENSIONS_TOOL) {
            run::log(&format!(
                "updates: {EXTENSIONS_TOOL} was left installed: {}",
                stop.detail
            ));
        }
    }
}

/// `Step::Packages`: every package the person deselected, and the
/// installers, removed each in a transaction of its own, so that one that
/// something else still needs does not keep the others.
pub fn remove_packages(install: &mut Install) -> Result<(), Stop> {
    let list = plan::removals(install.config, &install.image.packages);
    let total = list.len() as u64;
    let mut kept = Vec::new();
    for (index, name) in list.iter().enumerate() {
        if !valid_package(name) {
            kept.push(name.clone());
        } else if !packages::installed(install, name) {
            // Taken already, with the desktop that needed it (Kinexin's
            // removal of GNOME) or by an earlier removal here.
            run::log(&format!("packages: {name} is no longer installed"));
        } else if let Err(stop) = packages::remove(install, name) {
            run::log(&format!("packages: {name} kept: {}", stop.detail));
            kept.push(name.clone());
        }
        install.progress(Step::Packages, index as u64 + 1, total);
    }
    if let Some(warning) = removal_warning(&kept) {
        install.warn(warning);
    }
    Ok(())
}

/// The warning for the packages that could not be removed, if any were.
pub fn removal_warning(kept: &[String]) -> Option<Warning> {
    (!kept.is_empty()).then_some(Warning::PackagesKept)
}

/// `Step::Flatpaks`: the chosen applications from Flathub, installed for
/// every account. Each is installed on its own, so one that Flathub no
/// longer has does not keep the others out.
pub fn flatpaks(install: &mut Install) -> Result<(), Stop> {
    if !install.online {
        run::log("flatpaks: offline, none installed");
        install.warn(Warning::FlatpaksFailed);
        return Ok(());
    }
    let root = install.root().to_path_buf();
    let ids = install.config.software.flatpaks.clone();
    let mut failed = ids.iter().any(|id| !valid_flatpak(id));
    if let Err(stop) = run::chroot(&root, "flatpak", &remote_add_args()) {
        run::log(&format!(
            "flatpaks: Flathub could not be added: {}",
            stop.detail
        ));
        install.warn(Warning::FlatpaksFailed);
        return Ok(());
    }
    let wanted: Vec<&String> = ids.iter().filter(|id| valid_flatpak(id)).collect();
    let total = wanted.len() as u64;
    for (index, id) in wanted.into_iter().enumerate() {
        let args = install_args(id);
        let installed = run::chroot_streaming(&root, "flatpak", &args, &mut |line| run::log(line));
        if let Err(stop) = installed {
            run::log(&format!("flatpaks: {id} not installed: {}", stop.detail));
            failed = true;
        }
        install.progress(Step::Flatpaks, index as u64 + 1, total);
    }
    if failed {
        install.warn(Warning::FlatpaksFailed);
    }
    Ok(())
}

/// Adds Flathub for the whole system, unless it is there already.
pub fn remote_add_args() -> [&'static str; 5] {
    [
        "remote-add",
        "--if-not-exists",
        "--system",
        FLATHUB,
        FLATHUB_REPO,
    ]
}

/// Installs one application for the whole system, asking nothing.
pub fn install_args(id: &str) -> [&str; 6] {
    ["install", "--system", "-y", "--noninteractive", FLATHUB, id]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_need_the_internet_and_run_once() {
        assert_eq!(
            updates_plan(false, false),
            UpdatesPlan::Skip(Warning::UpdatesSkippedOffline)
        );
        assert_eq!(
            updates_plan(false, true),
            UpdatesPlan::Skip(Warning::UpdatesSkippedOffline)
        );
        assert_eq!(updates_plan(true, true), UpdatesPlan::AlreadyDone);
        assert_eq!(updates_plan(true, false), UpdatesPlan::Run);
    }

    #[test]
    fn extensions_are_refreshed_only_where_gnome_and_a_new_home_have_them() {
        assert!(refreshes_extensions(true, true, true));
        // Kinexin removed GNOME; an account with none; a kept /home.
        assert!(!refreshes_extensions(false, true, true));
        assert!(!refreshes_extensions(true, false, true));
        assert!(!refreshes_extensions(true, true, false));
    }

    #[test]
    fn extensions_are_refreshed_as_the_person_in_their_own_home() {
        let args = gext_args("zoe");
        // Never waiting for ever on the network.
        assert_eq!(&args[..2], ["--kill-after=30", "600"]);
        assert_eq!(&args[2..6], ["/usr/bin/runuser", "-u", "zoe", "--"]);
        // A cleared environment: nothing of root's or the worker's.
        assert_eq!(&args[6..8], ["/usr/bin/env", "-i"]);
        assert!(args.contains(&"HOME=/home/zoe".to_string()));
        assert!(args.contains(&"XDG_DATA_HOME=/home/zoe/.local/share".to_string()));
        assert!(!args.iter().any(|a| a.contains("/root")));
        assert_eq!(
            &args[args.len() - 4..],
            ["gext", "--filesystem", "update", "-y"]
        );
        assert!(valid_package(EXTENSIONS_TOOL));
        assert!(EXTENSIONS_DIR.starts_with(".local/share/"));
    }

    #[test]
    fn a_kept_package_is_a_warning_not_a_stop() {
        assert_eq!(removal_warning(&[]), None);
        assert_eq!(
            removal_warning(&["gnome-maps".to_string()]),
            Some(Warning::PackagesKept)
        );
    }

    #[test]
    fn flatpaks_are_installed_system_wide_from_flathub() {
        assert_eq!(
            remote_add_args(),
            [
                "remote-add",
                "--if-not-exists",
                "--system",
                "flathub",
                "https://dl.flathub.org/repo/flathub.flatpakrepo"
            ]
        );
        assert_eq!(
            install_args("org.example.App"),
            [
                "install",
                "--system",
                "-y",
                "--noninteractive",
                "flathub",
                "org.example.App"
            ]
        );
    }
}
