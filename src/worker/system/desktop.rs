//! The desktop step: the chosen desktop, its login screen, and no other.
//!
//! One desktop only, by design: the person picks one of the three and no
//! other desktop is left offered. What persists of the family in every
//! variant is CEDM, the compositor it runs on and the toolkit
//! ([`ALWAYS_KEPT`]), and of the system Bluetooth and the power profiles
//! ([`SYSTEM_KEPT`]); they are marked explicitly installed before anything is
//! removed, so that a recursive removal of what pulled them in keeps them.
//!
//! Linexin is the image's own GNOME; LineXinBar's packages and the family's
//! applications are removed ([`LINEXINBAR_REMOVED`]). Kinexin is downloaded
//! (`kinexin-desktop`, which overwrites some of GNOME's files, hence
//! `--overwrite *`), then GNOME and LineXinBar are removed, and the Plasma
//! panel's launchers are fitted to what the person chose. LineXinBar adds `lxb`
//! and `cedm` (downloaded unless the image carries them; the cursor theme both
//! it and GNOME's `linexin-desktop` ship is the only thing it may overwrite),
//! then GNOME is removed. Each removal is followed by a check that the chosen
//! desktop's own packages ([`own_packages`]) and the kept ones are still there,
//! and the installation stops if one is not.
//!
//! Kinexin and LineXinBar sign in through CEDM, offering their own session
//! first (`plasma` or `lxb`): CEDM is downloaded with Kinexin when the image
//! does not carry it, and a Plasma display manager kinexin-desktop still
//! brings (sddm) is installed but switched off. Linexin (GNOME) always signs
//! in through gdm: GNOME Shell's lock screen unlocks through gdm and cannot
//! lock at all under another display manager. Every desktop starts into
//! `graphical.target`.
//!
//! The login screens list the sessions in `wayland-sessions` and `xsessions`
//! under /usr/local/share and /usr/share. On the 3.0 image only gnome-session
//! (`gnome`, a member of the gnome group) and lxb-desktop (`lxb`) put one
//! there, and Kinexin brings plasma-workspace's (`plasma`), so the removals
//! above leave the chosen desktop's alone; what is left is logged.
//!
//! Whatever the desktop, the menus are tidied as the legacy installer tidied
//! them (the helper programs it hid stay out of every menu) and Flatpak
//! applications may read the GTK settings the desktop's look is made of.
//!
//! Units are enabled with the live system's `systemctl --root`, which only
//! writes links inside the target and needs nothing running there.
use super::{packages, Install};
use crate::model::config::{Desktop, InstallConfig};
use crate::model::protocol::{Failure, Step};
use crate::worker::layout::Stop;
use crate::worker::{run, target};
use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub const GDM: &str = "gdm.service";
pub const CEDM: &str = "cedm.service";
pub const GRAPHICAL: &str = "graphical.target";

pub const KINEXIN: &str = "kinexin-desktop";
pub const CEDM_PACKAGE: &str = "cedm";
pub const LINEXINBAR_PACKAGES: &[&str] = &["lxb", CEDM_PACKAGE];
/// The one place lxb-compositor up to 0.9.3 and the image's `linexin-desktop`
/// both put files: the Bibata cursor theme. Without leave to replace them
/// pacman stops on the conflict; leave for anything wider could hide a real
/// one. From 0.9.4 LineXinBar's copy has a directory of its own
/// (`lxb-Bibata-Modern-Classic`), so this matters only while linexin-repo
/// still serves the older package, and asks nothing of the newer one.
pub const LINEXINBAR_SHARED_FILES: &str = "/usr/share/icons/Bibata-Modern-Classic/*";

/// What every desktop keeps of the family, whichever is chosen: CEDM, the
/// compositor it runs on (one of its dependencies), and the toolkit with its
/// Python binding, which the family's other programs are built on.
pub const ALWAYS_KEPT: &[&str] = &[
    CEDM_PACKAGE,
    "lxb-compositor",
    "lxb-toolkit",
    "python-lxb-toolkit",
];

/// The system's own services, which outlive whichever desktop brought them:
/// BlueZ, whose `bluetooth.service` the System step enables and which
/// LineXinBar's Settings > Bluetooth talks to, and power-profiles-daemon,
/// whose profiles LineXinBar's Settings switch. On the 3.0 image both come
/// in only as GNOME's dependencies, so removing GNOME recursively for
/// LineXinBar would take them, and leave `bluetooth.service` enabled with
/// nothing behind it (seen in QEMU, iso300-linexinbar-offline). Marked
/// explicitly installed with [`ALWAYS_KEPT`], whichever the target has.
pub const SYSTEM_KEPT: &[&str] = &["bluez", "power-profiles-daemon"];

/// Everything marked explicitly installed before another desktop is removed:
/// the family's [`ALWAYS_KEPT`], then the system's [`SYSTEM_KEPT`].
pub fn kept_whatever_is_chosen() -> Vec<&'static str> {
    ALWAYS_KEPT.iter().chain(SYSTEM_KEPT).copied().collect()
}

/// LineXinBar, removed when another desktop is chosen: its meta package, the
/// shell (whose `lxb.desktop` is the session a login screen would offer),
/// its integrations, and the family's applications the meta package brings.
pub const LINEXINBAR_REMOVED: &[&str] = &[
    "lxb",
    "lxb-desktop",
    "lxb-heroic",
    "lxb-retroarch",
    "lxb-rpcs3",
    "lxb-proton",
    "distribumpy",
    "imagonsole",
    "songonsole",
    "videonsole",
];

/// CEDM's administrator policy, and the session it offers first.
pub const CEDM_CONFIG: &str = "etc/cedm/config.toml";
pub const LXB_SESSION: &str = "lxb";

/// Where the login screens find sessions, inside the target: CEDM reads
/// these four (its `sessions::standard_directories`), and gdm the last two.
pub const SESSION_DIRS: &[&str] = &[
    "usr/local/share/wayland-sessions",
    "usr/local/share/xsessions",
    "usr/share/wayland-sessions",
    "usr/share/xsessions",
];

/// The Plasma panel layout, inside a home or the skeleton.
pub const PLASMA_APPLETS: &str = ".config/plasma-org.kde.plasma.desktop-appletsrc";
/// The section of it that holds Kinexin's task manager launchers.
pub const LAUNCHER_SECTION: &str = "[Containments][73][Applets][74][Configuration][General]";

/// Display managers a Plasma desktop may bring, by unit: kinexin-desktop
/// depends on sddm, plasma-meta on plasma-login-manager (which the run never
/// installs; see `packages::assumed_installed`). GNOME's gdm is not among
/// them: it is the image's own.
pub const DISPLAY_MANAGERS: &[&str] = &[
    "sddm.service",
    "plasmalogin.service",
    "lightdm.service",
    "ly.service",
    "greetd.service",
];

/// The helper programs the legacy installer took out of the menus
/// (`post-install.sh:139-148`): Avahi's SSH, VNC and Zeroconf browsers, the
/// V4L2 test and capture tools, the software token, vim and lftp (both
/// terminal programs) and GNOME's Extensions app, by their desktop-file ids.
/// Extensions is not on the 3.0 image; an entry that is not there is left
/// alone.
pub const HIDDEN_APPS: &[&str] = &[
    "bssh",
    "avahi-discover",
    "qv4l2",
    "qvidcap",
    "stoken-gui",
    "stoken-gui-small",
    "vim",
    "lftp",
    "bvnc",
    "org.gnome.Extensions",
];

/// Where packages put desktop entries, and where an administrator's own
/// copy of one goes. `/usr/local/share` comes before `/usr/share` in
/// `XDG_DATA_DIRS` (the default when it is unset, and the order Flatpak's
/// environment generator keeps behind its own exports), and every menu
/// takes an id from the first directory that has it: GLib's app info
/// (GNOME Shell's grid), KDE's service cache (Plasma's launcher) and
/// LineXinBar's start screen (`apps::scan` counts an id as seen before it
/// reads the entry, and drops one marked `NoDisplay`).
pub const PACKAGED_ENTRIES: &str = "usr/share/applications";
pub const LOCAL_ENTRIES: &str = "usr/local/share/applications";

/// What Flatpak applications may read of the person's own settings: the
/// GTK 3 and 4 configuration the desktop's theme and colours are written
/// into, read-only (`post-install.sh:385-386`). For every Flatpak, system
/// wide, including those installed later.
pub const FLATPAK_GTK_FILESYSTEMS: &[&str] = &["xdg-config/gtk-4.0:ro", "xdg-config/gtk-3.0:ro"];

pub fn run(install: &mut Install) -> Result<(), Stop> {
    let desktop = install.config.desktop;
    match desktop {
        Desktop::Linexin => linexin(install),
        Desktop::Kinexin => kinexin(install),
        Desktop::LineXinBar => linexinbar(install),
    }?;
    // After the desktop's own packages, so that an entry Kinexin brings is
    // hidden as well.
    let root = install.root().to_path_buf();
    hide_helpers(&root);
    flatpak_gtk_override(&root);
    log_sessions(&root, desktop);
    Ok(())
}

/// Takes [`HIDDEN_APPS`] out of the menus with a copy of each entry in
/// [`LOCAL_ENTRIES`] marked `NoDisplay`. The legacy installer appended
/// `Hidden=true` to the packaged file itself, which the next upgrade of
/// that package put back as it was; a copy in /usr/local belongs to no
/// package. `NoDisplay` ("do not show in menus") rather than `Hidden`
/// ("deleted"): the program still opens what it is for, vim the text files
/// a file manager offers it. A copy already there is the image's own and
/// is kept. Cosmetic, so a failure is only logged.
fn hide_helpers(root: &Path) {
    for id in HIDDEN_APPS {
        let packaged = format!("{PACKAGED_ENTRIES}/{id}.desktop");
        let local = format!("{LOCAL_ENTRIES}/{id}.desktop");
        if target::exists(root, &local) || !target::is_file(root, &packaged) {
            continue;
        }
        let hidden = target::read(root, &packaged).and_then(|text| {
            match text.as_deref().and_then(without_display) {
                Some(entry) => {
                    target::create_dir(root, LOCAL_ENTRIES)?;
                    target::write(root, &local, &entry, 0o644).map(|()| true)
                }
                None => Ok(false),
            }
        });
        match hidden {
            Ok(true) => run::log(&format!("desktop: {id} is out of the menus")),
            Ok(false) => {}
            Err(stop) => run::log(&format!(
                "desktop: {id} stays in the menus: {}",
                stop.detail
            )),
        }
    }
}

/// A desktop entry with `NoDisplay=true` as the first setting of its
/// `[Desktop Entry]` group and no other `NoDisplay` there, everything else
/// as it was; `None` when it has no such group or is hidden already. First,
/// because a reader that meets a key twice may keep either: LineXinBar's
/// keeps the first, and only one is left.
pub fn without_display(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len() + 16);
    let mut inside = false;
    let mut found = false;
    let mut hidden_already = false;
    for line in text.split_inclusive('\n') {
        let bare = line.trim();
        if bare.starts_with('[') {
            inside = bare == "[Desktop Entry]" && !found;
            out.push_str(line);
            if inside {
                if !line.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("NoDisplay=true\n");
                found = true;
            }
            continue;
        }
        if inside {
            if let Some((key, value)) = bare.split_once('=') {
                if key.trim() == "NoDisplay" {
                    hidden_already |= value.trim() == "true";
                    continue;
                }
            }
        }
        out.push_str(line);
    }
    (found && !hidden_already).then_some(out)
}

/// `flatpak override`'s arguments for [`FLATPAK_GTK_FILESYSTEMS`]: the
/// system-wide overrides, which every account's Flatpaks read.
pub fn flatpak_override_args() -> Vec<String> {
    let mut args = vec!["override".to_string(), "--system".to_string()];
    args.extend(
        FLATPAK_GTK_FILESYSTEMS
            .iter()
            .map(|place| format!("--filesystem={place}")),
    );
    args
}

/// Lets every Flatpak read the GTK settings, when the system has Flatpak.
/// Nothing is downloaded. Cosmetic, so a failure is only logged.
fn flatpak_gtk_override(root: &Path) {
    if !target::exists(root, "usr/bin/flatpak") {
        return;
    }
    let args = flatpak_override_args();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match run::chroot(root, "flatpak", &args) {
        Ok(_) => run::log("desktop: Flatpak applications may read the GTK settings"),
        Err(stop) => run::log(&format!(
            "desktop: Flatpak applications keep their own GTK settings: {}",
            stop.detail
        )),
    }
}

fn linexin(install: &mut Install) -> Result<(), Stop> {
    remove_others(install)?;
    let root = install.root().to_path_buf();
    // An image whose live session signs in through CEDM has it as the
    // display manager; GNOME's gdm takes its place. `--force` because the
    // display-manager link may still name CEDM if disabling it left it.
    if target::exists(&root, &format!("usr/lib/systemd/system/{CEDM}")) {
        if let Err(stop) = systemctl(&root, &["disable", CEDM]) {
            run::log(&format!("desktop: CEDM was not disabled: {}", stop.detail));
        }
    }
    systemctl(&root, &["enable", "--force", GDM])?;
    systemctl(&root, &["set-default", GRAPHICAL])
}

fn kinexin(install: &mut Install) -> Result<(), Stop> {
    if !install.image.carries(Desktop::Kinexin) {
        need_online(install, KINEXIN)?;
        // CEDM in the same transaction when the image has none (2.1.4):
        // Kinexin signs in through it whatever the image.
        packages::install_as(
            install,
            Some(Step::Desktop),
            &[KINEXIN, CEDM_PACKAGE],
            &["--overwrite", "*"],
        )?;
    }
    remove_others(install)?;
    fit_launchers(install.root(), install.config);
    cedm_login(install, Desktop::Kinexin)?;
    let root = install.root().to_path_buf();
    systemctl(&root, &["set-default", GRAPHICAL])
}

fn linexinbar(install: &mut Install) -> Result<(), Stop> {
    if !install.image.carries(Desktop::LineXinBar) {
        need_online(install, "lxb and cedm")?;
        packages::install_as(
            install,
            Some(Step::Desktop),
            LINEXINBAR_PACKAGES,
            &["--overwrite", LINEXINBAR_SHARED_FILES],
        )?;
    }
    remove_others(install)?;
    cedm_login(install, Desktop::LineXinBar)?;
    let root = install.root().to_path_buf();
    systemctl(&root, &["set-default", GRAPHICAL])
}

/// One desktop only: after the chosen desktop's packages are in, what every
/// desktop keeps is marked explicitly installed and the other desktops
/// ([`removed_for`]) are removed, each checked for what it must not take.
fn remove_others(install: &mut Install) -> Result<(), Stop> {
    let kept = keep_family(install)?;
    for other in removed_for(install.config.desktop) {
        if *other == Other::Gnome {
            // Before GNOME goes, so that gdm's display-manager link goes
            // with it rather than being left pointing at nothing.
            let root = install.root().to_path_buf();
            if let Err(stop) = systemctl(&root, &["disable", GDM]) {
                run::log(&format!("desktop: gdm was not disabled: {}", stop.detail));
            }
        }
        remove_other(install, *other, &kept)?;
    }
    Ok(())
}

/// The packages without which the chosen desktop is not there: checked after
/// every removal of another desktop, which stops the installation if one of
/// them went with it.
pub fn own_packages(desktop: Desktop) -> &'static [&'static str] {
    match desktop {
        Desktop::Linexin => &["gdm", "gnome-shell"],
        Desktop::Kinexin => &[KINEXIN, CEDM_PACKAGE],
        Desktop::LineXinBar => LINEXINBAR_PACKAGES,
    }
}

/// The desktops removed for each choice, in the order they are removed.
pub fn removed_for(desktop: Desktop) -> &'static [Other] {
    match desktop {
        Desktop::Linexin => &[Other::LineXinBar],
        Desktop::Kinexin => &[Other::Gnome, Other::LineXinBar],
        Desktop::LineXinBar => &[Other::Gnome],
    }
}

/// A desktop removed because another was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Other {
    /// The installed members of the `gnome` group, with what depends on
    /// them, as the legacy installer removed GNOME for Kinexin.
    Gnome,
    /// [`LINEXINBAR_REMOVED`], whichever are installed.
    LineXinBar,
}

impl Other {
    fn name(self) -> &'static str {
        match self {
            Other::Gnome => "GNOME",
            Other::LineXinBar => "LineXinBar",
        }
    }
}

/// The session CEDM offers first for each desktop: the name of its entry in
/// /usr/share/wayland-sessions.
pub fn cedm_session(desktop: Desktop) -> &'static str {
    match desktop {
        Desktop::Linexin => "gnome",
        Desktop::Kinexin => "plasma",
        Desktop::LineXinBar => LXB_SESSION,
    }
}

/// Whether CEDM is the login screen: with Kinexin and LineXinBar, whatever
/// the image (by design: CEDM is their default; an image without it has it
/// downloaded with them). Never with GNOME (Linexin): GNOME Shell's lock
/// screen unlocks through gdm, and signed in through CEDM it says "Screen
/// Locking requires the GNOME display manager" and cannot lock at all (seen
/// in QEMU, iso300-linexin; GNOME keeps gdm).
pub fn uses_cedm(desktop: Desktop) -> bool {
    match desktop {
        Desktop::Linexin => false,
        Desktop::Kinexin | Desktop::LineXinBar => true,
    }
}

/// The other display managers a target may have, by unit: gdm, and the ones
/// a Plasma desktop may bring. A desktop package can still depend on one of
/// them, so each is installed but switched off.
pub fn other_display_managers() -> impl Iterator<Item = &'static str> {
    std::iter::once(GDM).chain(DISPLAY_MANAGERS.iter().copied())
}

/// CEDM as the login screen, offering `desktop`'s session first: every other
/// display manager the target has is disabled, CEDM takes the
/// display-manager link (`--force`, since it may still name another), and its
/// policy names the session.
fn cedm_login(install: &mut Install, desktop: Desktop) -> Result<(), Stop> {
    debug_assert!(uses_cedm(desktop));
    let root = install.root().to_path_buf();
    for unit in other_display_managers() {
        if !target::exists(&root, &format!("usr/lib/systemd/system/{unit}")) {
            continue;
        }
        if let Err(stop) = systemctl(&root, &["disable", unit]) {
            run::log(&format!(
                "desktop: {unit} was not disabled: {}",
                stop.detail
            ));
        }
    }
    systemctl(&root, &["enable", "--force", CEDM])?;
    let session = cedm_session(desktop);
    let existing = target::read(&root, CEDM_CONFIG)?;
    target::create_dir(&root, "etc/cedm")?;
    target::write(
        &root,
        CEDM_CONFIG,
        &cedm_config(existing.as_deref(), session),
        0o644,
    )?;
    run::log(&format!(
        "desktop: CEDM is the login screen, offering the {session} session first"
    ));
    Ok(())
}

/// The worker refuses a desktop that must be downloaded before it writes
/// anything when the internet does not answer; this is the same rule kept
/// where the download happens, should the two ever disagree.
fn need_online(install: &Install, what: &str) -> Result<(), Stop> {
    if install.online {
        Ok(())
    } else {
        Err(Stop::new(
            Failure::NeedsInternet,
            format!("{what} must be downloaded and the internet did not answer"),
        ))
    }
}

/// `systemctl --root=<target> <args>`.
pub fn systemctl(root: &Path, args: &[&str]) -> Result<(), Stop> {
    let root = format!("--root={}", run::utf8(root)?);
    let mut all = vec![root.as_str()];
    all.extend_from_slice(args);
    run::run("systemctl", &all).map(|_| ())
}

/// Marks the packages kept whatever is chosen ([`kept_whatever_is_chosen`])
/// that the target has as explicitly installed, before anything is removed:
/// on the 3.0 image the family's came in as dependencies of the `lxb` meta
/// package and the system's as GNOME's, and removing either recursively
/// (`-Rs`) would take them with it. Returns the ones marked, which must
/// still be there after every removal.
fn keep_family(install: &mut Install) -> Result<Vec<&'static str>, Stop> {
    let installed = packages::installed_names(install)?;
    let kept = present(&kept_whatever_is_chosen(), &installed);
    if !kept.is_empty() {
        packages::mark_explicit(install, &kept)?;
        run::log(&format!(
            "desktop: kept whatever is chosen: {}",
            kept.join(" ")
        ));
    }
    Ok(kept)
}

/// Removes `other` with what depends on it and what only it needed
/// (`-Rsc`), then checks that the chosen desktop's own packages and `kept`
/// are all still installed. An `other` that cannot be removed is left beside
/// the chosen desktop, which still works, and logged; a removal that took
/// the chosen desktop or a kept package with it is not, and stops.
fn remove_other(install: &mut Install, other: Other, kept: &[&str]) -> Result<(), Stop> {
    let names = match other {
        Other::Gnome => run::chroot(install.root(), "pacman", &["-Qgq", "gnome"])
            .map(|out| group_members(&out))
            .unwrap_or_default(),
        Other::LineXinBar => {
            let installed = packages::installed_names(install)?;
            present(LINEXINBAR_REMOVED, &installed)
                .into_iter()
                .map(str::to_string)
                .collect()
        }
    };
    let what = other.name();
    if names.is_empty() {
        run::log(&format!("desktop: no package of {what} is installed"));
        return Ok(());
    }
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    match packages::remove_with_dependents(install, &refs) {
        Ok(()) => run::log(&format!("desktop: {what} was removed")),
        Err(stop) => run::log(&format!("desktop: {what} was kept: {}", stop.detail)),
    }
    let mut survivors: Vec<&str> = own_packages(install.config.desktop).to_vec();
    survivors.extend_from_slice(kept);
    let installed = packages::installed_names(install)?;
    let lost = missing(&survivors, &installed);
    if lost.is_empty() {
        Ok(())
    } else {
        Err(Stop::new(
            Failure::CommandFailed,
            format!("removing {what} removed {} too", lost.join(" ")),
        ))
    }
}

/// The names in `wanted` that are installed, in `wanted`'s order.
pub fn present(wanted: &[&'static str], installed: &BTreeSet<String>) -> Vec<&'static str> {
    wanted
        .iter()
        .copied()
        .filter(|name| installed.contains(*name))
        .collect()
}

/// The names in `wanted` that are not installed, each once.
pub fn missing<'a>(wanted: &[&'a str], installed: &BTreeSet<String>) -> Vec<&'a str> {
    let mut lost: Vec<&str> = Vec::new();
    for name in wanted {
        if !installed.contains(*name) && !lost.contains(name) {
            lost.push(name);
        }
    }
    lost
}

/// The package names in `pacman -Qg` output, with or without the group
/// name in front of each.
pub fn group_members(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .filter(|name| super::check::valid_package(name))
        .map(str::to_string)
        .collect()
}

/// The desktop a session entry (`gnome`, `plasmax11`, `lxb`) belongs to, by
/// the name each desktop's packages give theirs; `None` for any other.
pub fn session_desktop(session: &str) -> Option<Desktop> {
    if session.starts_with("gnome") {
        Some(Desktop::Linexin)
    } else if session.starts_with("plasma") {
        Some(Desktop::Kinexin)
    } else if session == LXB_SESSION {
        Some(Desktop::LineXinBar)
    } else {
        None
    }
}

/// The sessions in `found` (entry names without `.desktop`) that belong to
/// another of the three desktops than `desktop`.
pub fn other_desktops_sessions(desktop: Desktop, found: &[String]) -> Vec<String> {
    found
        .iter()
        .filter(|session| session_desktop(session).is_some_and(|owner| owner != desktop))
        .cloned()
        .collect()
}

/// Logs the sessions the login screen will offer, and any of another of the
/// three desktops a removal kept. Nothing is changed: an entry hidden with a
/// copy in /usr/local would not hide the packaged one from CEDM, which takes
/// the next directory's entry when the first one's is hidden.
fn log_sessions(root: &Path, desktop: Desktop) {
    let mut found: Vec<String> = Vec::new();
    for dir in SESSION_DIRS {
        for name in target::list(root, dir).unwrap_or_default() {
            if let Some(session) = name.strip_suffix(".desktop") {
                if !found.iter().any(|f| f == session) {
                    found.push(session.to_string());
                }
            }
        }
    }
    run::log(&format!(
        "desktop: the login screen offers: {}",
        if found.is_empty() {
            "nothing".to_string()
        } else {
            found.join(" ")
        }
    ));
    let others = other_desktops_sessions(desktop, &found);
    if !others.is_empty() {
        run::log(&format!(
            "desktop: another desktop's session is still offered: {}",
            others.join(" ")
        ));
    }
}

/// The legacy launcher edit (`post-install.sh`, update_kinexin_plasma_launchers):
/// the task manager's launchers in the skeleton of the target at `root`,
/// and in the new account's home unless that home was kept, match what the
/// person chose. Cosmetic, so a failure is only logged.
pub(crate) fn fit_launchers(root: &Path, config: &InstallConfig) {
    let software = &config.software;
    let launchers = kinexin_launchers(&software.flatpaks, &software.remove);
    run::log(&format!("desktop: Plasma launchers: {launchers}"));
    let kept_home = config.home_kept();
    let mut files = vec![format!("etc/skel/{PLASMA_APPLETS}")];
    if !kept_home {
        files.push(format!("home/{}/{PLASMA_APPLETS}", config.account.username));
    }
    for file in files {
        let result = target::read(root, &file).and_then(|text| match text {
            Some(text) => match with_launchers(&text, &launchers) {
                Some(edited) => rewrite_keeping_owner(root, &file, &edited),
                None => Ok(()),
            },
            None => Ok(()),
        });
        if let Err(stop) = result {
            run::log(&format!("desktop: /{file} left as it was: {}", stop.detail));
        }
    }
}

/// Kinexin's launchers, in the legacy order: Zen when it was chosen as a
/// Flatpak, the file manager always, then Discover, Linexin Center and Steam
/// unless the person deselected their package.
pub fn kinexin_launchers(flatpaks: &[String], removed: &[String]) -> String {
    let removed = |names: &[&str]| names.iter().any(|n| removed.iter().any(|r| r == n));
    let mut launchers = Vec::new();
    if flatpaks.iter().any(|id| id == "app.zen_browser.zen") {
        launchers.push("applications:app.zen_browser.zen.desktop");
    }
    launchers.push("preferred://filemanager");
    if !removed(&["discover", "plasma-discover"]) {
        launchers.push("applications:org.kde.discover.desktop");
    }
    if !removed(&[
        "linexincenter",
        "linexin-center",
        "github.petexy.linexincenter",
    ]) {
        launchers.push("applications:github.petexy.linexincenter.desktop");
    }
    if !removed(&["steam"]) {
        launchers.push("applications:steam.desktop");
    }
    launchers.join(",")
}

/// The Plasma layout with the `launchers=` line of [`LAUNCHER_SECTION`]
/// replaced, everything else as it was; `None` when there was nothing to
/// change.
pub fn with_launchers(text: &str, launchers: &str) -> Option<String> {
    let mut inside = false;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if bare.starts_with('[') {
            inside = bare == LAUNCHER_SECTION;
        } else if inside && bare.starts_with("launchers=") {
            let ending = &line[bare.len()..];
            out.push_str(&format!("launchers={launchers}{ending}"));
            continue;
        }
        out.push_str(line);
    }
    (out != text).then_some(out)
}

/// Replaces a file inside the target keeping its owner and mode: a file in
/// the person's home must stay theirs.
fn rewrite_keeping_owner(root: &Path, relative: &str, text: &str) -> Result<(), Stop> {
    let path = target::path(root, relative)?;
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => meta,
        _ => return Ok(()),
    };
    target::write(root, relative, text, meta.mode() & 0o7777)?;
    std::os::unix::fs::lchown(&path, Some(meta.uid()), Some(meta.gid())).map_err(|e| {
        Stop::new(
            Failure::Internal,
            format!("cannot give {} back its owner: {e}", path.display()),
        )
    })
}

/// The keys CEDM's policy file knows (`ConsoleExperienceDesktopManager`
/// `src/config.rs`): it refuses a file with any other and then fails closed,
/// so nothing else may be kept.
const CEDM_KEYS: &[&str] = &[
    "default_session",
    "remember_user",
    "remember_session",
    "language",
    "sound",
    "power",
];
const CEDM_POWER_KEYS: &[&str] = &["sleep", "restart", "shut_down"];

/// CEDM's policy with `default_session` set to `session`: a file of its own
/// when the target has none, or the existing one with that one line set and
/// only the keys CEDM knows kept (comments and blank lines stay).
pub fn cedm_config(existing: Option<&str>, session: &str) -> String {
    let mut out = format!("default_session = \"{session}\"\n");
    let Some(existing) = existing else {
        return format!(
            "# Written by the Linexin installer: the desktop chosen is the session offered first.\n{out}"
        );
    };
    #[derive(PartialEq)]
    enum Table {
        Top,
        Power,
        Other,
    }
    let mut table = Table::Top;
    for line in existing.lines() {
        let bare = line.trim();
        if bare.is_empty() || bare.starts_with('#') {
            if table != Table::Other {
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        if bare.starts_with('[') {
            table = if bare.trim_end_matches(|c: char| c != ']') == "[power]" {
                Table::Power
            } else {
                Table::Other
            };
            if table == Table::Power {
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        let key = bare
            .split('=')
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"');
        let keep = match table {
            Table::Top => CEDM_KEYS.contains(&key) && key != "default_session",
            Table::Power => CEDM_POWER_KEYS.contains(&key),
            Table::Other => false,
        };
        if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_helper_is_hidden_by_a_copy_marked_first_in_its_main_group() {
        // An invented entry, shaped like Avahi's browsers: one group, no
        // NoDisplay of its own, a trailing action group.
        let entry = "\
# packaged
[Desktop Entry]
Version=1.0
Name=Example Browser
Exec=/usr/bin/example-browser
Type=Application

[Desktop Action Window]
Name=New Window
Exec=/usr/bin/example-browser --window
";
        let hidden = without_display(entry).unwrap();
        assert_eq!(
            hidden,
            entry.replace("[Desktop Entry]\n", "[Desktop Entry]\nNoDisplay=true\n")
        );
        // Not appended at the end, where it would belong to the action.
        assert!(!hidden.ends_with("NoDisplay=true\n"));
        // Hidden already: nothing to copy.
        assert_eq!(without_display(&hidden), None);
    }

    #[test]
    fn a_helpers_own_no_display_is_replaced_not_repeated() {
        let entry = "[Desktop Entry]\nName=Tool\nExec=tool\nNoDisplay=false\nType=Application\n";
        assert_eq!(
            without_display(entry).unwrap(),
            "[Desktop Entry]\nNoDisplay=true\nName=Tool\nExec=tool\nType=Application\n"
        );
        // A NoDisplay in another group is that group's and stays.
        let entry = "[Desktop Entry]\nName=Tool\n[Desktop Action X]\nNoDisplay=false\n";
        assert_eq!(
            without_display(entry).unwrap(),
            "[Desktop Entry]\nNoDisplay=true\nName=Tool\n[Desktop Action X]\nNoDisplay=false\n"
        );
        // A header with nothing after it, not even a line end.
        assert_eq!(
            without_display("[Desktop Entry]").unwrap(),
            "[Desktop Entry]\nNoDisplay=true\n"
        );
        // Not a desktop entry at all.
        assert_eq!(without_display("Name=Tool\n"), None);
        assert_eq!(without_display(""), None);
    }

    #[test]
    fn the_hidden_helpers_are_the_legacy_list_by_id() {
        assert_eq!(HIDDEN_APPS.len(), 10);
        for id in HIDDEN_APPS {
            assert!(!id.ends_with(".desktop") && !id.contains('/'), "{id}");
        }
        // Copies go where packages never write, in front of where they do.
        assert_eq!(
            LOCAL_ENTRIES,
            format!("usr/local/{}", &PACKAGED_ENTRIES[4..])
        );
    }

    #[test]
    fn flatpaks_may_read_only_the_gtk_settings_system_wide() {
        assert_eq!(
            flatpak_override_args(),
            [
                "override",
                "--system",
                "--filesystem=xdg-config/gtk-4.0:ro",
                "--filesystem=xdg-config/gtk-3.0:ro"
            ]
        );
    }

    #[test]
    fn launchers_follow_the_persons_choices() {
        assert_eq!(
            kinexin_launchers(&[], &[]),
            "preferred://filemanager,applications:org.kde.discover.desktop,\
             applications:github.petexy.linexincenter.desktop,applications:steam.desktop"
        );
        assert_eq!(
            kinexin_launchers(
                &strings(&["app.zen_browser.zen", "org.example.App"]),
                &strings(&["steam", "plasma-discover", "linexin-center"])
            ),
            "applications:app.zen_browser.zen.desktop,preferred://filemanager"
        );
    }

    #[test]
    fn only_the_task_managers_launchers_change() {
        let layout = "\
[Containments][73][Applets][74]
immutability=1
launchers=keep-me

[Containments][73][Applets][74][Configuration][General]
launchers=applications:old.desktop
showOnlyCurrentDesktop=false

[Containments][80][Configuration][General]
launchers=keep-me-too
";
        let edited = with_launchers(layout, "preferred://filemanager").unwrap();
        assert_eq!(
            edited,
            layout.replace(
                "launchers=applications:old.desktop",
                "launchers=preferred://filemanager"
            )
        );
        assert_eq!(with_launchers(&edited, "preferred://filemanager"), None);
        assert_eq!(with_launchers("[General]\nlaunchers=x\n", "y"), None);
    }

    #[test]
    fn gnome_group_members_are_read_with_or_without_the_group_name() {
        assert_eq!(
            group_members("baobab\ngdm\ngnome-shell\n"),
            ["baobab", "gdm", "gnome-shell"]
        );
        assert_eq!(
            group_members("gnome gdm\ngnome nautilus\n"),
            ["gdm", "nautilus"]
        );
        assert!(group_members("").is_empty());
    }

    #[test]
    fn linexinbar_may_overwrite_only_the_shared_cursor_theme() {
        let glob = LINEXINBAR_SHARED_FILES;
        assert!(glob.starts_with("/usr/share/icons/Bibata-Modern-Classic/"));
        assert_eq!(glob.matches('*').count(), 1);
        assert!(glob.ends_with("/*"));
    }

    #[test]
    fn cedm_gets_lxb_as_its_default_session() {
        let fresh = cedm_config(None, LXB_SESSION);
        assert!(fresh.lines().any(|l| l == "default_session = \"lxb\""));
        assert!(fresh
            .lines()
            .all(|l| l.starts_with('#') || l.starts_with("default_session")));
    }

    #[test]
    fn plasma_and_linexinbar_sign_in_through_cedm_and_gnome_through_gdm() {
        // Whatever the image: CEDM comes with Kinexin when it has none.
        assert!(uses_cedm(Desktop::Kinexin));
        assert!(uses_cedm(Desktop::LineXinBar));
        // GNOME cannot lock its screen without gdm.
        assert!(!uses_cedm(Desktop::Linexin));
        assert_eq!(cedm_session(Desktop::Linexin), "gnome");
        assert_eq!(cedm_session(Desktop::Kinexin), "plasma");
        assert_eq!(cedm_session(Desktop::LineXinBar), "lxb");
        let fresh = cedm_config(None, cedm_session(Desktop::Kinexin));
        assert!(fresh.lines().any(|l| l == "default_session = \"plasma\""));
        // A desktop that signs in through CEDM cannot lose it to a removal.
        for desktop in [Desktop::Kinexin, Desktop::LineXinBar] {
            assert!(own_packages(desktop).contains(&CEDM_PACKAGE), "{desktop:?}");
        }
    }

    #[test]
    fn cedm_switches_off_every_other_display_manager() {
        let others: Vec<&str> = other_display_managers().collect();
        for unit in [
            "gdm.service",
            "sddm.service",
            "plasmalogin.service",
            "greetd.service",
        ] {
            assert!(others.contains(&unit), "{unit}");
        }
        assert!(!others.contains(&CEDM));
    }

    #[test]
    fn one_desktop_only_each_choice_removes_the_others() {
        assert_eq!(removed_for(Desktop::Linexin), [Other::LineXinBar]);
        assert_eq!(
            removed_for(Desktop::Kinexin),
            [Other::Gnome, Other::LineXinBar]
        );
        assert_eq!(removed_for(Desktop::LineXinBar), [Other::Gnome]);
        assert_eq!(own_packages(Desktop::Linexin), ["gdm", "gnome-shell"]);
        assert_eq!(own_packages(Desktop::Kinexin), ["kinexin-desktop", "cedm"]);
        assert_eq!(own_packages(Desktop::LineXinBar), ["lxb", "cedm"]);
    }

    #[test]
    fn what_persists_is_never_what_is_removed() {
        for name in ALWAYS_KEPT {
            assert!(!LINEXINBAR_REMOVED.contains(name), "{name}");
        }
        // The shell's session goes with LineXinBar, and so do the family's
        // applications; CEDM, the compositor and the toolkit stay.
        for name in ["lxb", "lxb-desktop", "distribumpy", "videonsole"] {
            assert!(LINEXINBAR_REMOVED.contains(&name), "{name}");
        }
        assert_eq!(
            ALWAYS_KEPT,
            [
                "cedm",
                "lxb-compositor",
                "lxb-toolkit",
                "python-lxb-toolkit"
            ]
        );
    }

    #[test]
    fn bluetooth_and_the_power_profiles_outlive_any_desktop() {
        // The System step enables bluetooth.service, and LineXinBar's
        // Settings talk to BlueZ and to power-profiles-daemon: neither may go
        // with the desktop that brought it.
        assert_eq!(SYSTEM_KEPT, ["bluez", "power-profiles-daemon"]);
        for name in SYSTEM_KEPT {
            assert!(!LINEXINBAR_REMOVED.contains(name), "{name}");
            assert!(!ALWAYS_KEPT.contains(name), "{name}");
        }
        assert_eq!(
            kept_whatever_is_chosen(),
            [
                "cedm",
                "lxb-compositor",
                "lxb-toolkit",
                "python-lxb-toolkit",
                "bluez",
                "power-profiles-daemon"
            ]
        );
        // An invented system that has BlueZ but not the power profiles: only
        // what is there is marked, family first.
        let installed: BTreeSet<String> = ["bluez", "cedm", "example-editor", "lxb-toolkit"]
            .iter()
            .map(|p| p.to_string())
            .collect();
        assert_eq!(
            present(&kept_whatever_is_chosen(), &installed),
            ["cedm", "lxb-toolkit", "bluez"]
        );
    }

    #[test]
    fn only_what_is_installed_is_removed_or_kept() {
        // An invented system with the meta package and two of its parts.
        let installed: BTreeSet<String> = [
            "cedm",
            "example-editor",
            "lxb",
            "lxb-desktop",
            "lxb-toolkit",
            "songonsole",
        ]
        .iter()
        .map(|p| p.to_string())
        .collect();
        assert_eq!(
            present(LINEXINBAR_REMOVED, &installed),
            ["lxb", "lxb-desktop", "songonsole"]
        );
        assert_eq!(present(ALWAYS_KEPT, &installed), ["cedm", "lxb-toolkit"]);
        assert!(present(LINEXINBAR_REMOVED, &BTreeSet::new()).is_empty());
    }

    #[test]
    fn a_removal_that_took_the_chosen_desktop_is_named() {
        let installed: BTreeSet<String> = ["cedm", "lxb-toolkit", "plasma-workspace"]
            .iter()
            .map(|p| p.to_string())
            .collect();
        let survivors = ["kinexin-desktop", "cedm", "cedm", "lxb-toolkit"];
        assert_eq!(missing(&survivors, &installed), ["kinexin-desktop"]);
        assert!(missing(&["cedm", "lxb-toolkit"], &installed).is_empty());
        let gone = ["lxb-compositor", "lxb-compositor"];
        assert_eq!(missing(&gone, &installed), ["lxb-compositor"]);
    }

    #[test]
    fn sessions_are_told_apart_by_desktop() {
        assert_eq!(session_desktop("gnome"), Some(Desktop::Linexin));
        assert_eq!(session_desktop("gnome-xorg"), Some(Desktop::Linexin));
        assert_eq!(session_desktop("plasma"), Some(Desktop::Kinexin));
        assert_eq!(session_desktop("plasmax11"), Some(Desktop::Kinexin));
        assert_eq!(session_desktop("lxb"), Some(Desktop::LineXinBar));
        assert_eq!(session_desktop("lxb-example"), None);
        assert_eq!(session_desktop("example-session"), None);
        let found = strings(&["gnome", "lxb", "example-session"]);
        assert_eq!(
            other_desktops_sessions(Desktop::LineXinBar, &found),
            ["gnome"]
        );
        assert_eq!(
            other_desktops_sessions(Desktop::Kinexin, &found),
            ["gnome", "lxb"]
        );
        assert!(other_desktops_sessions(Desktop::Kinexin, &strings(&["plasma"])).is_empty());
        // The directories CEDM reads, in its order: /usr/local first.
        assert_eq!(SESSION_DIRS.len(), 4);
        assert!(SESSION_DIRS[0].starts_with("usr/local/share/"));
        assert!(SESSION_DIRS
            .iter()
            .all(|d| d.ends_with("/wayland-sessions") || d.ends_with("/xsessions")));
    }

    #[test]
    fn an_existing_cedm_policy_keeps_only_what_cedm_knows() {
        let existing = "\
# policy
default_session = \"gnome\"
remember_user = false
greeting = \"hello\"

[power]
sleep = false
hibernate = true

[theme]
# a table CEDM does not have
accent = \"blue\"
";
        assert_eq!(
            cedm_config(Some(existing), LXB_SESSION),
            "\
default_session = \"lxb\"
# policy
remember_user = false

[power]
sleep = false

"
        );
    }
}
