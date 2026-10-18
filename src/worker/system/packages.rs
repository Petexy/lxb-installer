//! pacman inside the target: the one way the desktop, software, drivers and
//! snapshot steps install and remove packages, so that the package databases
//! are synchronised once per run and pacman's output reaches the log as it
//! runs.
//!
//! An installation from the internet is a full upgrade the first time
//! (`pacman -Syu <names>`): the image is weeks or months older than the
//! repositories, and a package built against today's libraries installed
//! next to the image's older ones is the partial upgrade Arch does not
//! support. Once the system has been brought up to date in this run
//! (`Install::synced`), later installations only add what they name.
use super::check::valid_package;
use super::Install;
use crate::model::config::Desktop;
use crate::model::protocol::{Failure, Step};
use crate::worker::layout::Stop;
use crate::worker::run;
use std::collections::BTreeSet;

/// Packages a desktop's dependencies would bring that the installed system
/// must not get, each handed to pacman as already installed
/// (`--assume-installed <name>`, no version, which satisfies an unversioned
/// dependency) so that no transaction of the run installs it.
///
/// Kinexin: `kinexin-desktop` depends on `plasma-meta`, which depends on
/// `plasma-login-manager`, a second login screen beside CEDM, which is
/// Kinexin's (by design; sddm, still one of kinexin-desktop's own
/// dependencies, is installed and switched off). This holds only for the
/// installation: a later system upgrade that upgrades `plasma-meta` resolves
/// its dependencies again and brings plasma-login-manager in after all,
/// installed but switched off, since CEDM keeps the display-manager link. The
/// lasting fix is in kinexin-desktop's dependencies in linexin-repo, not
/// here.
pub fn assumed_installed(desktop: Desktop) -> &'static [&'static str] {
    match desktop {
        Desktop::Kinexin => &["plasma-login-manager"],
        Desktop::Linexin | Desktop::LineXinBar => &[],
    }
}

/// `args` with [`assumed_installed`] added for `desktop` when they are a
/// transaction that can bring packages in (`-S…` or `-U…`, the operation
/// first, as every call here passes it); removals and database changes
/// cannot, and `-D` does not take the option at all.
pub fn transaction_args(desktop: Desktop, args: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(args.len() + 2);
    let Some((operation, rest)) = args.split_first() else {
        return out;
    };
    out.push(operation.to_string());
    if operation.starts_with("-S") || operation.starts_with("-U") {
        for name in assumed_installed(desktop) {
            out.push("--assume-installed".to_string());
            out.push(name.to_string());
        }
    }
    out.extend(rest.iter().map(|a| a.to_string()));
    out
}

/// What pacman is doing to a package, as it says it on a line of its own.
const OPERATIONS: &[&str] = &[
    "installing",
    "upgrading",
    "reinstalling",
    "downgrading",
    "removing",
];

/// Brings the package databases and Arch's keyring up to date, when this
/// run has not done so already: a newer keyring first, so that packages
/// signed by keys added since the image was made can be checked.
pub fn sync(install: &mut Install) -> Result<(), Stop> {
    if install.synced {
        return Ok(());
    }
    let result = pacman(
        install,
        None,
        &["-Sy", "--needed", "--noconfirm", "archlinux-keyring"],
    );
    stop_gpg_helpers(install);
    result
}

/// Installs `names` from the repositories into the target.
pub fn install(install: &mut Install, names: &[&str]) -> Result<(), Stop> {
    install_as(install, None, names, &[])
}

/// [`install`] with pacman's counts reported as progress of `step`, and
/// `extra` options added to the transaction (`--overwrite *`).
pub fn install_as(
    install: &mut Install,
    step: Option<Step>,
    names: &[&str],
    extra: &[&str],
) -> Result<(), Stop> {
    check_names(names)?;
    sync(install)?;
    let args = install_args(install.synced, names, extra);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = pacman(install, step, &args);
    stop_gpg_helpers(install);
    result?;
    install.synced = true;
    Ok(())
}

/// Brings the whole system up to date, `step` taking pacman's counts.
pub fn upgrade(install: &mut Install, step: Step) -> Result<(), Stop> {
    sync(install)?;
    let result = pacman(install, Some(step), &["-Syu", "--noconfirm"]);
    stop_gpg_helpers(install);
    result?;
    install.synced = true;
    Ok(())
}

/// Removes the one package `name` from the target in a transaction of its
/// own, with what only it needed (`-Rns`).
pub fn remove(install: &mut Install, name: &str) -> Result<(), Stop> {
    check_names(&[name])?;
    pacman(install, None, &["-Rns", "--noconfirm", name])
}

/// Removes `names` in one transaction together with everything that depends
/// on them and everything only they needed (`-Rsc`), as the legacy
/// installer removed GNOME for Kinexin.
pub fn remove_with_dependents(install: &mut Install, names: &[&str]) -> Result<(), Stop> {
    check_names(names)?;
    let mut args = vec!["-Rsc", "--noconfirm"];
    args.extend_from_slice(names);
    pacman(install, None, &args)
}

/// Removes `name` without looking at what depends on it (`-Rdd`), as the
/// legacy installer removed drivers: what needs a graphics driver needs one
/// of several, and the one that stays is enough.
pub fn remove_ignoring_dependents(install: &mut Install, name: &str) -> Result<(), Stop> {
    check_names(&[name])?;
    pacman(install, None, &["-Rdd", "--noconfirm", name])
}

/// Marks `names` as explicitly installed (`-D --asexplicit`), so that a
/// recursive removal of what pulled them in as dependencies (`-Rs`) keeps
/// them. Every name must be installed: pacman refuses the whole list
/// otherwise.
pub fn mark_explicit(install: &mut Install, names: &[&str]) -> Result<(), Stop> {
    check_names(names)?;
    let mut args = vec!["-D", "--asexplicit"];
    args.extend_from_slice(names);
    pacman(install, None, &args)
}

/// Whether the target has the package `name` installed.
pub fn installed(install: &Install, name: &str) -> bool {
    valid_package(name) && run::chroot(install.root(), "pacman", &["-Q", name]).is_ok()
}

/// Every package the target has installed, by name (`pacman -Qq`).
pub fn installed_names(install: &Install) -> Result<BTreeSet<String>, Stop> {
    run::chroot(install.root(), "pacman", &["-Qq"]).map(|out| names_in(&out))
}

/// The package names in `pacman -Qq` output, one per line.
pub fn names_in(output: &str) -> BTreeSet<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|name| valid_package(name))
        .map(str::to_string)
        .collect()
}

/// pacman's arguments for installing `names`: with a full upgrade unless the
/// system is already up to date, and never reinstalling what is there.
pub fn install_args(synced: bool, names: &[&str], extra: &[&str]) -> Vec<String> {
    let mut args = vec![
        if synced { "-S" } else { "-Syu" }.to_string(),
        "--needed".to_string(),
        "--noconfirm".to_string(),
    ];
    args.extend(extra.iter().map(|a| a.to_string()));
    args.extend(names.iter().map(|n| n.to_string()));
    args
}

fn check_names(names: &[&str]) -> Result<(), Stop> {
    match names.iter().find(|name| !valid_package(name)) {
        Some(bad) => Err(Stop::new(
            Failure::Internal,
            format!("not a package name: {bad:?}"),
        )),
        None => Ok(()),
    }
}

/// Runs pacman inside the target, every line of its output into the log as
/// it comes and its counts into `step`'s progress. Every pacman transaction
/// of the run goes through here, so each one that can bring packages in
/// carries what the chosen desktop must not get ([`transaction_args`]).
fn pacman(install: &mut Install, step: Option<Step>, args: &[&str]) -> Result<(), Stop> {
    let root = install.root().to_path_buf();
    let args = transaction_args(install.config.desktop, args);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut counts = Counts::default();
    run::chroot_streaming(&root, "pacman", &args, &mut |line| {
        run::log(line);
        if let (Some(step), Some((done, total))) = (step, counts.line(line)) {
            install.progress(step, done, total);
        }
    })
}

/// pacman-key, run by the keyring package's install script, leaves gpg's
/// helpers running inside the target, where they would keep its
/// filesystems busy (as the System step's own keyring does; see `tidy`).
fn stop_gpg_helpers(install: &Install) {
    let stopped = run::chroot(
        install.root(),
        "gpgconf",
        &["--homedir", "/etc/pacman.d/gnupg", "--kill", "all"],
    );
    if let Err(stop) = stopped {
        run::log(&format!(
            "packages: gpg's helpers were not stopped: {}",
            stop.detail
        ));
    }
}

/// pacman's progress, worked out from its output one line at a time.
///
/// On a terminal pacman numbers each package (`( 3/12) installing foo`).
/// Writing to a pipe, as it does here, it does not: it lists the
/// transaction first (`Packages (12) …`) and then says `installing foo...`
/// for each package, so the count is kept here. Its hooks are numbered too
/// (`(1/7) Arming ConditionNeedsUpdate...`) but are not packages, so only
/// lines naming an operation on a package count.
#[derive(Debug, Default)]
pub struct Counts {
    done: u64,
    total: u64,
}

impl Counts {
    /// Takes one line; returns `(done, total)` when it moved the count.
    pub fn line(&mut self, line: &str) -> Option<(u64, u64)> {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Packages (") {
            let total = rest.split(')').next()?.trim().parse().ok()?;
            self.total = total;
            self.done = 0;
            return Some((0, total));
        }
        if let Some(rest) = line.strip_prefix('(') {
            let (count, after) = rest.split_once(')')?;
            let (done, total) = count.split_once('/')?;
            let operation = after.split_whitespace().next()?;
            if !OPERATIONS.contains(&operation) {
                return None;
            }
            self.done = done.trim().parse().ok()?;
            self.total = total.trim().parse().ok()?;
            return Some((self.done.min(self.total), self.total));
        }
        let operation = line.split_whitespace().next()?;
        if OPERATIONS.contains(&operation) && line.ends_with("...") && self.total > 0 {
            self.done += 1;
            return Some((self.done.min(self.total), self.total));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_installation_is_a_full_upgrade() {
        assert_eq!(
            install_args(false, &["lxb", "cedm"], &[]),
            ["-Syu", "--needed", "--noconfirm", "lxb", "cedm"]
        );
        assert_eq!(
            install_args(true, &["kinexin-desktop"], &["--overwrite", "*"]),
            [
                "-S",
                "--needed",
                "--noconfirm",
                "--overwrite",
                "*",
                "kinexin-desktop"
            ]
        );
    }

    #[test]
    fn kinexin_never_installs_plasmas_own_login_screen() {
        let install = install_args(false, &["kinexin-desktop"], &["--overwrite", "*"]);
        let install: Vec<&str> = install.iter().map(String::as_str).collect();
        assert_eq!(
            transaction_args(Desktop::Kinexin, &install),
            [
                "-Syu",
                "--assume-installed",
                "plasma-login-manager",
                "--needed",
                "--noconfirm",
                "--overwrite",
                "*",
                "kinexin-desktop"
            ]
        );
        // Every later transaction that can bring packages in carries it too:
        // the keyring, the Updates step's full upgrade, Timeshift, a file.
        for args in [
            &["-Sy", "--needed", "--noconfirm", "archlinux-keyring"][..],
            &["-Syu", "--noconfirm"],
            &["-S", "--needed", "--noconfirm", "timeshift"],
            &["-U", "--noconfirm", "/var/cache/example-1-1.pkg.tar.zst"],
        ] {
            let out = transaction_args(Desktop::Kinexin, args);
            assert_eq!(out[1..3], ["--assume-installed", "plasma-login-manager"]);
            assert_eq!(out.len(), args.len() + 2);
        }
        // Removals and database changes bring nothing, and -D refuses it.
        for args in [
            &["-Rsc", "--noconfirm", "gnome-shell"][..],
            &["-Rns", "--noconfirm", "gparted"],
            &["-D", "--asexplicit", "cedm"],
        ] {
            assert_eq!(transaction_args(Desktop::Kinexin, args), args);
        }
    }

    #[test]
    fn the_other_desktops_assume_nothing() {
        for desktop in [Desktop::Linexin, Desktop::LineXinBar] {
            assert!(assumed_installed(desktop).is_empty());
            let args = ["-Syu", "--needed", "--noconfirm", "lxb", "cedm"];
            assert_eq!(transaction_args(desktop, &args), args);
        }
        assert!(transaction_args(Desktop::Kinexin, &[]).is_empty());
    }

    #[test]
    fn installed_names_are_read_one_per_line() {
        let names = names_in("cedm\nlxb-toolkit\n\n  gnome-shell \n-Rdd\n");
        assert_eq!(
            names.into_iter().collect::<Vec<_>>(),
            ["cedm", "gnome-shell", "lxb-toolkit"]
        );
        assert!(names_in("").is_empty());
    }

    #[test]
    fn names_that_could_be_options_are_refused() {
        assert!(check_names(&["lxb", "cedm"]).is_ok());
        assert!(check_names(&["-Rdd"]).is_err());
        assert!(check_names(&["Gnome"]).is_err());
    }

    #[test]
    fn piped_output_is_counted_from_the_transaction_list() {
        let mut counts = Counts::default();
        let lines = [
            ":: Synchronizing package databases...",
            "resolving dependencies...",
            "Packages (3) cedm-0.9.4-1  lxb-0.9.4-1  lxb-toolkit-0.9.4-1",
            "Total Installed Size:  42.00 MiB",
            ":: Processing package changes...",
            "installing lxb-toolkit...",
            "installing lxb...",
            "upgrading cedm...",
            ":: Running post-transaction hooks...",
            "(1/2) Arming ConditionNeedsUpdate...",
            "(2/2) Reloading system manager configuration...",
        ];
        let moves: Vec<(u64, u64)> = lines.iter().filter_map(|l| counts.line(l)).collect();
        assert_eq!(moves, [(0, 3), (1, 3), (2, 3), (3, 3)]);
    }

    #[test]
    fn terminal_output_carries_its_own_counts() {
        let mut counts = Counts::default();
        assert_eq!(counts.line("( 3/12) installing gnome-maps"), Some((3, 12)));
        assert_eq!(counts.line("(12/12) removing totem"), Some((12, 12)));
        assert_eq!(counts.line("( 1/12) checking keys in keyring"), None);
        assert_eq!(counts.line("(1/7) Arming ConditionNeedsUpdate..."), None);
    }

    #[test]
    fn nothing_counts_before_the_list_and_nothing_runs_past_it() {
        let mut counts = Counts::default();
        assert_eq!(counts.line("installing foo..."), None);
        assert_eq!(counts.line("Packages (1) foo-1-1"), Some((0, 1)));
        assert_eq!(counts.line("installing foo..."), Some((1, 1)));
        assert_eq!(counts.line("installing bar..."), Some((1, 1)));
        assert_eq!(counts.line("warning: installing is fun"), None);
        assert_eq!(counts.line("Packages (x) foo"), None);
    }
}
