//! The System step: what turns a copy of the live image into a system of
//! its own.
//!
//! The fstab storage worked out is written; everything only the live session
//! needs is removed — its account (with no `pkill`: arch-chroot shares the
//! live session's processes, and killing the live user's would end the
//! session the installer runs in; `userdel` itself ignores processes outside
//! the chroot), its autologins, the password-free polkit rule, the units
//! that set up the live medium, its own networking beside NetworkManager,
//! the installers' autostart entries, the live initramfs settings, the live
//! session's GNOME settings and its NVIDIA overrides; the installed preset
//! takes the live one's place; the image's files kept for the installed
//! system (`*-postinstall`) take their real names and the legacy installer's
//! other finishing touches are made (§4.6); the machine gets an
//! identity of its own at first start; and pacman gets a keyring of its own
//! (the live session's was made on the medium's tmpfs and never reached the
//! image).
use super::accounts::has_entry;
use super::{safe_token, Chroot, Install};
use crate::model::protocol::Step;
use crate::worker::layout::Stop;
use crate::worker::{run, target};
use std::path::Path;

/// The live session's account.
pub const LIVE_USER: &str = "liveuser";

/// Files and directories only the live session uses, relative to the root:
/// root's autologin on tty1, polkit without passwords for wheel, the
/// medium's welcome text, the live initramfs settings, archiso's script for
/// root's login shell, the random seed the image was built with, and the
/// profile's two NVIDIA overrides.
///
/// Those last two (`post-install.sh:195-196`) mask nvidia-utils' own files
/// of the same names, which blacklist nouveau and load nvidia-uvm at start:
/// the live medium chooses between nouveau and NVIDIA's driver on the
/// kernel line of each boot entry instead. The installed system's entries
/// carry no such choice, so with the overrides left in place nouveau would
/// be free to take an NVIDIA card before NVIDIA's own driver.
pub const LIVE_FILES: &[&str] = &[
    "etc/systemd/system/getty@tty1.service.d",
    "etc/polkit-1/rules.d/49-nopasswd_global.rules",
    "etc/motd",
    "etc/mkinitcpio.conf.d/archiso.conf",
    "root/.zlogin",
    "root/.automated_script.sh",
    "var/lib/systemd/random-seed",
    "etc/modprobe.d/nvidia-utils.conf",
    "etc/modules-load.d/nvidia-utils.conf",
];

/// The live session's own GNOME settings among the image's system-wide
/// dconf keyfiles (`post-install.sh:135-136`): the one that keeps
/// dash-to-dock from opening the overview at start, so that the installer
/// is what the live session shows, and an older one that changed logging
/// out. `dconf update` compiles the keyfiles into the database GNOME reads,
/// so it is run again once one of them is gone.
pub const LIVE_DCONF: &[&str] = &[
    "etc/dconf/db/local.d/00-logout",
    "etc/dconf/db/local.d/06-disableoverviewinstallation",
];

/// Files the image carries twice, the live session's under the real name
/// and the installed system's beside it as `*-postinstall`: the skeleton's
/// zsh settings (grml's live, Linexin's installed), the Arch logos the
/// About pages show (the same bytes on 3.0, so only the spare copies go)
/// and the spinner theme's watermark (the system boots the profile's own
/// `install` theme, whose watermark is Linexin's already). Renamed in the
/// System step, before Accounts, so that the new account's home is made
/// from the installed skeleton; the legacy installer renamed `.zshrc` only
/// after it had made the account (`post-install.sh:155-165`).
pub const POSTINSTALL_FILES: &[(&str, &str)] = &[
    ("etc/skel/.zshrc_postinstall", "etc/skel/.zshrc"),
    (
        "usr/share/pixmaps/archlinux-logo-text-dark-postinstall.svg",
        "usr/share/pixmaps/archlinux-logo-text-dark.svg",
    ),
    (
        "usr/share/pixmaps/archlinux-logo-text-postinstall.svg",
        "usr/share/pixmaps/archlinux-logo-text.svg",
    ),
    (
        "usr/share/plymouth/themes/spinner/watermark-postinstall.png",
        "usr/share/plymouth/themes/spinner/watermark.png",
    ),
];

/// The system's identity file, and where the family keeps it: the image
/// writes `/etc/os-release` as a file of its own, while linexin-upgrade-tool's
/// pacman hook keeps `/usr/lib/os-release` Linexin's whenever an upgrade of
/// `filesystem` replaces it. systemd reads `/etc` first, so the image's file
/// moves to `/usr/lib` and `/etc` links to it (`post-install.sh:157-158`),
/// as Arch has it. The link is relative, so it resolves inside this system
/// even when another one mounts it (os-prober, a boot menu reading it).
pub const OS_RELEASE: &str = "etc/os-release";
pub const OS_RELEASE_SHARED: &str = "usr/lib/os-release";
pub const OS_RELEASE_LINK: &str = "../usr/lib/os-release";

/// Scripts the profile puts into the image without their exec bit, which
/// the legacy installer gave back (`post-install.sh:170,175`). This mends a
/// mistake in the profile rather than doing anything an installation needs:
/// `profiledef.sh`'s `file_permissions` names neither, so archiso leaves
/// them 0644 and they cannot be run, live or installed. The four other
/// scripts the legacy installer made executable (rum, photo, designer,
/// publisher) are no longer on the image.
pub const UNEXECUTABLE_SCRIPTS: &[&str] = &["usr/bin/bsod", "usr/bin/linexin-sound-fix-usb-dac"];

/// Bluetooth's service, which the legacy installer enabled
/// (`post-install.sh:152`). The 3.0 image enables it already; enabling it
/// again changes nothing there and keeps Bluetooth on with an image that
/// does not.
pub const BLUETOOTH: &str = "bluetooth.service";

/// Units only the live session runs: the keyring on tmpfs, the mirror
/// chooser, the live medium's sound and speech helpers, and the wait for
/// the clock to be synchronised.
///
/// archiso enables `systemd-time-wait-sync.service` (in
/// `sysinit.target.wants`) so that pacman-init makes its keys with a true
/// clock. In an installed system nothing needs it, and without an internet
/// connection it waits for ever: time-sync.target is never reached, and
/// with it neither are the timers nor multi-user.target and
/// graphical.target, so start-up never finishes (seen on the first QEMU
/// installation, offline: `is-system-running` stayed `starting`).
/// Only the link is removed; the unit itself stays in /usr/lib.
pub const LIVE_UNITS: &[&str] = &[
    "pacman-init.service",
    "etc-pacman.d-gnupg.mount",
    "choose-mirror.service",
    "livecd-alsa-unmuter.service",
    "livecd-talk.service",
    "systemd-time-wait-sync.service",
];

/// The live medium's own networking, which archiso enables beside
/// NetworkManager: systemd-networkd with its socket, its wait for the
/// network and its D-Bus name, and iwd. In an installed system both would
/// take the same Ethernet and Wi-Fi devices NetworkManager manages (the
/// image sets it up with wpa_supplicant, not iwd), so NetworkManager alone
/// is kept. archiso's other services (sshd, cloud-init, reflector) stay
/// enabled, as the legacy installer left them.
pub const LIVE_NETWORK_UNITS: &[&str] = &[
    "systemd-networkd.service",
    "systemd-networkd.socket",
    "systemd-networkd-wait-online.service",
    "dbus-org.freedesktop.network1.service",
    "iwd.service",
];

/// archiso's systemd-networkd configuration, which would bring every wired,
/// wireless and mobile link up by DHCP whatever NetworkManager is doing.
pub const LIVE_NETWORK_FILES: &[&str] = &[
    "etc/systemd/network/20-ethernet.network",
    "etc/systemd/network/20-wlan.network",
    "etc/systemd/network/20-wwan.network",
];

/// The installers' autostart entries: the GTK installer's, and this one's
/// should a live image ever start it the same way.
pub const INSTALLER_AUTOSTARTS: &[&str] = &[
    "github.petexy.linexininstaller.desktop",
    "lxb-installer.desktop",
];

/// gdm's configuration, where the live session logs in by itself.
pub const GDM_CONF: &str = "etc/gdm/custom.conf";
/// greetd's, where a live session that logs in through CEDM does the same:
/// CEDM's own (its unit runs greetd with it) and greetd's default.
pub const GREETD_CONFS: &[&str] = &["etc/cedm/greetd.toml", "etc/greetd/config.toml"];
/// SDDM's, where the image logs the live user in by itself
/// (`sddm.conf.d/autologin.conf`, `[Autologin] User=liveuser`): its main file
/// and the directory of its drop-ins. With GNOME nothing reads them; once
/// Kinexin brings SDDM, it tries that login at every start, fails it ("User
/// not known") and only then shows its greeter.
pub const SDDM_CONF: &str = "etc/sddm.conf";
pub const SDDM_CONF_DIR: &str = "etc/sddm.conf.d";

/// How many parts the step reports progress in.
const PARTS: u64 = 8;

pub fn run(install: &mut Install) -> Result<(), Stop> {
    let root = install.root().to_path_buf();
    // A /home kept from before — the easy path's, or one a hand-made layout
    // keeps — holds the person's files: the live account's home there is
    // left alone.
    let kept_home = install.config.home_kept();

    target::write(&root, "etc/fstab", &install.mounted.fstab, 0o644)?;
    install.progress(Step::System, 1, PARTS);

    remove_live_user(&root, kept_home, &mut |program, args| {
        run::chroot(&root, program, args).map(|_| ())
    })?;
    install.progress(Step::System, 2, PARTS);

    for file in LIVE_FILES.iter().chain(LIVE_NETWORK_FILES) {
        target::remove(&root, file)?;
    }
    let units = target::list(&root, "etc/systemd/system")?;
    for path in unit_paths(&units, LIVE_UNITS)
        .into_iter()
        .chain(unit_paths(&units, LIVE_NETWORK_UNITS))
    {
        target::remove(&root, &path)?;
    }
    remove_autostarts(&root, kept_home)?;
    if let Some(text) = target::read(&root, GDM_CONF)? {
        if let Some(rewritten) = without_autologin(&text) {
            target::write(&root, GDM_CONF, &rewritten, 0o644)?;
            run::log("system: gdm no longer logs in by itself");
        }
    }
    for conf in GREETD_CONFS {
        if let Some(text) = target::read(&root, conf)? {
            if let Some(rewritten) = without_initial_session(&text) {
                target::write(&root, conf, &rewritten, 0o644)?;
                run::log(&format!("system: /{conf} no longer logs in by itself"));
            }
        }
    }
    let dropins = target::list(&root, SDDM_CONF_DIR)?;
    for conf in sddm_confs(&dropins) {
        if let Some(text) = target::read(&root, &conf)? {
            if let Some(rewritten) = sddm_without_autologin(&text) {
                if has_settings(&rewritten) {
                    target::write(&root, &conf, &rewritten, 0o644)?;
                } else {
                    target::remove(&root, &conf)?;
                }
                run::log(&format!(
                    "system: SDDM no longer logs in by itself (/{conf})"
                ));
            }
        }
    }
    forget_live_settings(&root)?;
    install.progress(Step::System, 3, PARTS);

    let pkgbase = &install.image.kernel.pkgbase;
    let installed = format!("etc/mkinitcpio.d/{pkgbase}-postinstall.preset");
    let preset = format!("etc/mkinitcpio.d/{pkgbase}.preset");
    if target::rename(&root, &installed, &preset)? {
        run::log(&format!("system: /{installed} is now /{preset}"));
    } else {
        run::log(&format!("system: the image's own /{preset} is kept"));
    }
    install.progress(Step::System, 4, PARTS);

    finishing_touches(&root)?;
    install.progress(Step::System, 5, PARTS);

    // Empty rather than "uninitialized": systemd then makes an identity at
    // first start and commits it, without treating the boot as a first boot,
    // which would apply the unit presets over what the desktop step enabled.
    target::write(&root, "etc/machine-id", "", 0o444)?;
    install.progress(Step::System, 6, PARTS);

    fix_local_database(&root)?;
    install.progress(Step::System, 7, PARTS);

    keyring(&root)?;
    install.progress(Step::System, 8, PARTS);
    Ok(())
}

/// Removes the live session's GNOME settings and, when one was there,
/// compiles the system's dconf databases again without it: the database
/// the image was built with still holds the setting otherwise.
fn forget_live_settings(root: &Path) -> Result<(), Stop> {
    let mut removed = false;
    for file in LIVE_DCONF {
        if target::exists(root, file) {
            target::remove(root, file)?;
            run::log(&format!("system: /{file} removed"));
            removed = true;
        }
    }
    if dconf_needs_update(removed, target::exists(root, "usr/bin/dconf")) {
        // GNOME starts either way; only the live setting would stay.
        match run::chroot(root, "dconf", &["update"]) {
            Ok(_) => run::log("system: the dconf databases were compiled again"),
            Err(stop) => run::log(&format!(
                "system: the dconf databases keep the live settings: {}",
                stop.detail
            )),
        }
    }
    Ok(())
}

/// Whether `dconf update` is run: when a keyfile went and the system has
/// dconf to compile the rest with.
pub fn dconf_needs_update(removed: bool, has_dconf: bool) -> bool {
    removed && has_dconf
}

/// The legacy installer's finishing touches that are not about the live
/// session: the installed files take their real names, the identity file
/// moves where the family keeps it, the profile's scripts can be run, and
/// Bluetooth starts with the system.
fn finishing_touches(root: &Path) -> Result<(), Stop> {
    for (spare, real) in POSTINSTALL_FILES {
        if target::rename(root, spare, real)? {
            run::log(&format!("system: /{spare} is now /{real}"));
        }
    }
    // Only a file of its own moves: an /etc/os-release that is a link
    // already is how it should be.
    if target::is_file(root, OS_RELEASE) {
        target::rename(root, OS_RELEASE, OS_RELEASE_SHARED)?;
        target::symlink(root, OS_RELEASE, OS_RELEASE_LINK)?;
        run::log(&format!(
            "system: /{OS_RELEASE} is now /{OS_RELEASE_SHARED}, linked back"
        ));
    }
    for script in UNEXECUTABLE_SCRIPTS {
        if target::is_file(root, script) {
            target::set_mode(root, script, 0o755)?;
        }
    }
    if target::exists(root, &format!("usr/lib/systemd/system/{BLUETOOTH}")) {
        match super::desktop::systemctl(root, &["enable", BLUETOOTH]) {
            Ok(()) => run::log("system: Bluetooth starts with the system"),
            Err(stop) => run::log(&format!(
                "system: Bluetooth was not enabled: {}",
                stop.detail
            )),
        }
    }
    Ok(())
}

/// Removes the live account from the target at `root`, and its group if
/// `userdel` left it, each command run inside the target by `chroot`
/// (`run::chroot`, which a test replaces with a recorder that runs
/// nothing) — and its home with it (`userdel -r`, then whatever is left of
/// `/home/liveuser`) unless the homes are on a `/home` kept from before
/// (`kept_home`, `home_kept()`): then nothing under `/home` is touched, the
/// person's partition left as it was.
pub(crate) fn remove_live_user(
    root: &Path,
    kept_home: bool,
    chroot: &mut Chroot,
) -> Result<(), Stop> {
    let passwd = target::read(root, "etc/passwd")?.unwrap_or_default();
    if has_entry(&passwd, LIVE_USER) {
        let remove_home = !kept_home && target::exists(root, &format!("home/{LIVE_USER}"));
        chroot("userdel", &userdel_args(remove_home))?;
    }
    let group = target::read(root, "etc/group")?.unwrap_or_default();
    if has_entry(&group, LIVE_USER) {
        chroot("groupdel", &[LIVE_USER])?;
    }
    if !kept_home {
        target::remove(root, &format!("home/{LIVE_USER}"))?;
    }
    Ok(())
}

/// The installers' autostart entries removed from the target at `root`
/// ([`autostart_paths`]): the system's, the skeleton's, root's and — unless
/// the homes are on a `/home` kept from before (`kept_home`), which is the
/// person's own and left as it is — every home's.
pub(crate) fn remove_autostarts(root: &Path, kept_home: bool) -> Result<(), Stop> {
    let homes = if kept_home {
        Vec::new()
    } else {
        target::list(root, "home")?
    };
    for path in autostart_paths(&homes) {
        target::remove(root, &path)?;
    }
    Ok(())
}

/// `userdel`'s arguments: with `-r` only when there is a home to remove,
/// since `userdel -r` on a home that is not there is not an error everywhere
/// alike.
pub fn userdel_args(remove_home: bool) -> Vec<&'static str> {
    if remove_home {
        vec!["-r", LIVE_USER]
    } else {
        vec![LIVE_USER]
    }
}

/// Every path a live unit has in `etc/systemd/system`: the unit file itself
/// and its link in each `.wants` and `.requires` directory among `entries`
/// (that directory's listing). Paths that are not there are harmless to
/// remove.
pub fn unit_paths(entries: &[String], units: &[&str]) -> Vec<String> {
    let mut paths = Vec::new();
    for unit in units {
        paths.push(format!("etc/systemd/system/{unit}"));
        for directory in entries
            .iter()
            .filter(|e| e.ends_with(".wants") || e.ends_with(".requires"))
        {
            paths.push(format!("etc/systemd/system/{directory}/{unit}"));
        }
    }
    paths
}

/// The installers' autostart entries in the system-wide directory, the
/// skeleton new accounts are made from, root's home and the given homes.
pub fn autostart_paths(homes: &[String]) -> Vec<String> {
    let mut directories = vec![
        "etc/xdg/autostart".to_string(),
        "etc/skel/.config/autostart".to_string(),
        "root/.config/autostart".to_string(),
    ];
    directories.extend(
        homes
            .iter()
            .map(|home| format!("home/{home}/.config/autostart")),
    );
    directories
        .iter()
        .flat_map(|directory| {
            INSTALLER_AUTOSTARTS
                .iter()
                .map(move |entry| format!("{directory}/{entry}"))
        })
        .collect()
}

/// gdm's configuration without its automatic login (`AutomaticLoginEnable`,
/// `AutomaticLogin`), everything else kept as it was; `None` when there was
/// none to remove.
pub fn without_autologin(text: &str) -> Option<String> {
    let mut changed = false;
    let mut kept = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let key = line.split('=').next().unwrap_or_default().trim();
        if key.starts_with("AutomaticLogin") {
            changed = true;
        } else {
            kept.push_str(line);
        }
    }
    changed.then_some(kept)
}

/// SDDM's configuration files in the target: the main one, then each drop-in
/// of `entries` (the names in its `.d` directory) that SDDM reads, which are
/// those ending in `.conf`.
pub fn sddm_confs(entries: &[String]) -> Vec<String> {
    let mut confs = vec![SDDM_CONF.to_string()];
    let mut dropins: Vec<&String> = entries
        .iter()
        .filter(|name| name.ends_with(".conf") && safe_token(name))
        .collect();
    dropins.sort();
    confs.extend(
        dropins
            .into_iter()
            .map(|name| format!("{SDDM_CONF_DIR}/{name}")),
    );
    confs
}

/// SDDM's configuration without its automatic login: every setting of its
/// `[Autologin]` section (`User`, `Session`, `Relogin`), which only ever
/// concern that login, dropped, and everything else kept as it was; `None`
/// when there was none to drop.
pub fn sddm_without_autologin(text: &str) -> Option<String> {
    let mut changed = false;
    let mut inside = false;
    let mut kept = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let bare = line.trim();
        if bare.starts_with('[') {
            inside = bare == "[Autologin]";
        } else if inside && bare.contains('=') && !bare.starts_with(['#', ';']) {
            changed = true;
            continue;
        }
        kept.push_str(line);
    }
    changed.then_some(kept)
}

/// Whether an INI file still sets anything: a line that is neither blank, a
/// comment nor a section heading.
pub fn has_settings(text: &str) -> bool {
    text.lines()
        .map(str::trim)
        .any(|line| !line.is_empty() && !line.starts_with(['#', ';']) && !line.starts_with('['))
}

/// greetd's configuration without its `[initial_session]` table, the session
/// greetd starts once at boot without asking anybody (how a live image logs
/// in through CEDM), everything else kept as it was; `None` when there was
/// none to remove.
pub fn without_initial_session(text: &str) -> Option<String> {
    let mut changed = false;
    let mut skipping = false;
    let mut kept = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let bare = line.trim();
        if bare.starts_with('[') {
            let name = bare
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or_default()
                .trim();
            skipping = name == "initial_session";
            changed |= skipping;
        }
        if !skipping {
            kept.push_str(line);
        }
    }
    changed.then_some(kept)
}

/// A local package database entry without its `%INSTALLED_DB%` block (the
/// header, its values and the blank line that ends it), which the image's
/// build leaves in every entry and which the installed pacman does not
/// know; `None` when there was none. This is the legacy installer's fix
/// (`post-install.sh:190`).
pub fn strip_installed_db(desc: &str) -> Option<String> {
    let mut changed = false;
    let mut skipping = false;
    let mut kept = String::with_capacity(desc.len());
    for line in desc.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if skipping {
            skipping = !bare.is_empty();
        } else if bare == "%INSTALLED_DB%" {
            changed = true;
            skipping = true;
        } else {
            kept.push_str(line);
        }
    }
    changed.then_some(kept)
}

fn fix_local_database(root: &Path) -> Result<(), Stop> {
    let mut fixed = 0;
    for entry in target::list(root, "var/lib/pacman/local")? {
        let desc = format!("var/lib/pacman/local/{entry}/desc");
        if !target::is_file(root, &desc) {
            continue;
        }
        let text = match target::read(root, &desc) {
            Ok(Some(text)) => text,
            Ok(None) => continue,
            Err(stop) => {
                run::log(&format!("system: /{desc} left as it is: {}", stop.detail));
                continue;
            }
        };
        if let Some(stripped) = strip_installed_db(&text) {
            target::write(root, &desc, &stripped, 0o644)?;
            fixed += 1;
        }
    }
    run::log(&format!(
        "system: {fixed} package database entries had an %INSTALLED_DB% block"
    ));
    Ok(())
}

/// The keyrings to populate from the names in `usr/share/pacman/keyrings`:
/// each `<name>.gpg`, Arch's own first.
pub fn keyrings(names: &[String]) -> Vec<String> {
    let mut rings: Vec<String> = names
        .iter()
        .filter_map(|name| name.strip_suffix(".gpg"))
        .filter(|ring| safe_token(ring))
        .map(str::to_string)
        .collect();
    rings.sort_by_key(|ring| (ring != "archlinux", ring.clone()));
    rings
}

/// A keyring of the installed system's own, from the keys its packages ship.
fn keyring(root: &Path) -> Result<(), Stop> {
    let rings = keyrings(&target::list(root, "usr/share/pacman/keyrings")?);
    run::chroot(root, "pacman-key", &["--init"])?;
    if !rings.is_empty() {
        let mut args = vec!["--populate"];
        args.extend(rings.iter().map(String::as_str));
        run::chroot(root, "pacman-key", &args)?;
    }
    // pacman-key leaves gpg's helpers running inside the target, where they
    // would keep its filesystems busy and stop them from being released.
    let stop_helpers = run::chroot(
        root,
        "gpgconf",
        &["--homedir", "/etc/pacman.d/gnupg", "--kill", "all"],
    );
    if let Err(stop) = stop_helpers {
        run::log(&format!(
            "system: gpg's helpers were not stopped: {}",
            stop.detail
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greetd_loses_only_its_initial_session() {
        let conf = "\
[terminal]
vt = 1

[initial_session]
command = \"lxb-session\"
user = \"liveuser\"

[default_session]
command = \"cedm-greeter-session\"
user = \"greeter\"
";
        let rewritten = without_initial_session(conf).unwrap();
        assert_eq!(
            rewritten,
            "\
[terminal]
vt = 1

[default_session]
command = \"cedm-greeter-session\"
user = \"greeter\"
"
        );
        assert_eq!(without_initial_session(&rewritten), None);
        assert_eq!(
            without_initial_session(
                "[default_session]\nuser = \"greeter\"\n[ initial_session ]\nuser = \"x\"\n"
            ),
            Some("[default_session]\nuser = \"greeter\"\n".into())
        );
    }

    #[test]
    fn sddm_loses_only_its_automatic_login() {
        // The live image's own drop-in: nothing is left, so it goes.
        let live =
            "[Autologin]\nUser=liveuser\nSession=/usr/share/wayland-sessions/plasma.desktop\n";
        let rewritten = sddm_without_autologin(live).unwrap();
        assert_eq!(rewritten, "[Autologin]\n");
        assert!(!has_settings(&rewritten));
        // A file with more in it keeps the rest, comments included.
        let conf = "\
[General]
HaltCommand=/usr/bin/systemctl poweroff

[Autologin]
# set by the live image
User=liveuser
Relogin=false

[Theme]
Current=sugar-candy
";
        let rewritten = sddm_without_autologin(conf).unwrap();
        assert_eq!(
            rewritten,
            conf.replace("User=liveuser\n", "")
                .replace("Relogin=false\n", "")
        );
        assert!(has_settings(&rewritten));
        assert_eq!(sddm_without_autologin(&rewritten), None);
        // Kinexin's own settings have no automatic login to drop.
        assert_eq!(
            sddm_without_autologin("[Theme]\nCurrent=sugar-candy\n[Users]\nMinimumUid=1000\n"),
            None
        );
    }

    #[test]
    fn sddm_is_read_from_its_file_and_its_conf_drop_ins() {
        let entries = [
            "kde_settings.conf".to_string(),
            "autologin.conf".to_string(),
            "notes.txt".to_string(),
            "../x.conf".to_string(),
        ];
        assert_eq!(
            sddm_confs(&entries),
            [
                "etc/sddm.conf",
                "etc/sddm.conf.d/autologin.conf",
                "etc/sddm.conf.d/kde_settings.conf"
            ]
        );
        assert_eq!(sddm_confs(&[]), ["etc/sddm.conf"]);
    }

    #[test]
    fn gdm_loses_only_its_automatic_login() {
        let conf = "\
# GDM configuration storage
[daemon]
AutomaticLoginEnable=True
AutomaticLogin=liveuser
WaylandEnable=true
#AutomaticLogin=someone

[security]
";
        let rewritten = without_autologin(conf).unwrap();
        assert_eq!(
            rewritten,
            "\
# GDM configuration storage
[daemon]
WaylandEnable=true
#AutomaticLogin=someone

[security]
"
        );
        assert_eq!(without_autologin(&rewritten), None);
        assert_eq!(
            without_autologin("[daemon]\n  AutomaticLogin = x"),
            Some("[daemon]\n".into())
        );
    }

    #[test]
    fn the_installed_db_block_is_cut_out_whole() {
        let desc = "\
%NAME%
example

%VERSION%
1.0-1

%INSTALLED_DB%
/run/archiso/x86_64/pacman

%VALIDATION%
pgp
";
        let stripped = strip_installed_db(desc).unwrap();
        assert_eq!(
            stripped,
            "%NAME%\nexample\n\n%VERSION%\n1.0-1\n\n%VALIDATION%\npgp\n"
        );
        assert_eq!(strip_installed_db(&stripped), None);
        // As the last block, with nothing after it.
        assert_eq!(
            strip_installed_db("%NAME%\nx\n\n%INSTALLED_DB%\nlocal\n").unwrap(),
            "%NAME%\nx\n\n"
        );
    }

    #[test]
    fn live_units_are_removed_with_every_link_to_them() {
        let entries: Vec<String> = [
            "choose-mirror.service",
            "getty.target.wants",
            "multi-user.target.wants",
            "sound.target.wants",
            "display-manager.service",
        ]
        .iter()
        .map(|e| e.to_string())
        .collect();
        let paths = unit_paths(&entries, &["livecd-talk.service"]);
        assert_eq!(
            paths,
            [
                "etc/systemd/system/livecd-talk.service",
                "etc/systemd/system/getty.target.wants/livecd-talk.service",
                "etc/systemd/system/multi-user.target.wants/livecd-talk.service",
                "etc/systemd/system/sound.target.wants/livecd-talk.service",
            ]
        );
    }

    #[test]
    fn networkmanager_is_left_alone_with_the_network() {
        // As enabled on the 2.1.4 live medium (seen in the first QEMU
        // installation's /etc/systemd/system).
        let entries: Vec<String> = [
            "multi-user.target.wants",
            "network-online.target.wants",
            "sockets.target.wants",
        ]
        .iter()
        .map(|e| e.to_string())
        .collect();
        let paths = unit_paths(&entries, LIVE_NETWORK_UNITS);
        for link in [
            "multi-user.target.wants/systemd-networkd.service",
            "multi-user.target.wants/iwd.service",
            "network-online.target.wants/systemd-networkd-wait-online.service",
            "sockets.target.wants/systemd-networkd.socket",
            "dbus-org.freedesktop.network1.service",
        ] {
            assert!(
                paths.contains(&format!("etc/systemd/system/{link}")),
                "{link}"
            );
        }
        for kept in [
            "NetworkManager",
            "systemd-resolved",
            "sshd",
            "cloud-init",
            "reflector",
        ] {
            assert!(paths.iter().all(|path| !path.contains(kept)), "{kept}");
        }
        assert!(LIVE_NETWORK_FILES
            .iter()
            .all(|file| file.starts_with("etc/systemd/network/") && file.ends_with(".network")));
    }

    #[test]
    fn the_wait_for_the_clock_goes_with_the_live_units() {
        // archiso links it into sysinit.target.wants; an installed system
        // that is offline at start would otherwise never finish starting.
        assert!(LIVE_UNITS.contains(&"systemd-time-wait-sync.service"));
        let entries: Vec<String> = ["sysinit.target.wants", "multi-user.target.wants"]
            .iter()
            .map(|e| e.to_string())
            .collect();
        let paths = unit_paths(&entries, LIVE_UNITS);
        assert!(paths.contains(
            &"etc/systemd/system/sysinit.target.wants/systemd-time-wait-sync.service".to_string()
        ));
        // The vendor unit is never named: only links under /etc go.
        assert!(paths
            .iter()
            .all(|path| path.starts_with("etc/systemd/system/")));
    }

    #[test]
    fn autostarts_are_looked_for_everywhere_a_session_reads_them() {
        let paths = autostart_paths(&["someone".into()]);
        assert!(paths.contains(
            &"etc/skel/.config/autostart/github.petexy.linexininstaller.desktop".to_string()
        ));
        assert!(paths.contains(&"etc/xdg/autostart/lxb-installer.desktop".to_string()));
        assert!(paths.contains(
            &"home/someone/.config/autostart/github.petexy.linexininstaller.desktop".to_string()
        ));
        assert_eq!(autostart_paths(&[]).len(), 3 * INSTALLER_AUTOSTARTS.len());
    }

    #[test]
    fn the_live_nvidia_overrides_go_but_never_the_drivers_own_files() {
        for file in [
            "etc/modprobe.d/nvidia-utils.conf",
            "etc/modules-load.d/nvidia-utils.conf",
        ] {
            assert!(LIVE_FILES.contains(&file), "{file}");
        }
        // nvidia-utils' own blacklist and module list live in /usr/lib and
        // are what the installed system goes by.
        assert!(LIVE_FILES.iter().all(|file| !file.starts_with("usr/")));
        // The image's power-management setting for NVIDIA is not the live
        // medium's and stays.
        assert!(!LIVE_FILES.contains(&"etc/modprobe.d/nvidia-power-management.conf"));
    }

    #[test]
    fn dconf_is_compiled_again_only_when_a_live_setting_went() {
        assert!(dconf_needs_update(true, true));
        assert!(!dconf_needs_update(false, true));
        assert!(!dconf_needs_update(true, false));
        assert!(LIVE_DCONF
            .iter()
            .all(|file| file.starts_with("etc/dconf/db/local.d/")));
        // The image's own look (dark style, dock, extensions…) is kept.
        assert!(!LIVE_DCONF
            .iter()
            .any(|file| file.contains("05-gnomeextensions")));
    }

    #[test]
    fn each_postinstall_file_takes_the_name_beside_it() {
        for (spare, real) in POSTINSTALL_FILES {
            let (spare_dir, spare_name) = spare.rsplit_once('/').unwrap();
            let (real_dir, real_name) = real.rsplit_once('/').unwrap();
            assert_eq!(spare_dir, real_dir, "{spare}");
            let unmarked = spare_name
                .replace("-postinstall", "")
                .replace("_postinstall", "");
            assert_eq!(unmarked, real_name, "{spare}");
        }
        assert!(POSTINSTALL_FILES.contains(&("etc/skel/.zshrc_postinstall", "etc/skel/.zshrc")));
    }

    #[test]
    fn os_release_links_back_to_where_the_family_keeps_it() {
        // The link is read from etc/, so it must lead from there to the
        // shared file and never out of the system it is in.
        assert!(!OS_RELEASE_LINK.starts_with('/'));
        let mut resolved: Vec<&str> = OS_RELEASE.split('/').collect();
        resolved.pop();
        for part in OS_RELEASE_LINK.split('/') {
            match part {
                ".." => {
                    resolved.pop();
                }
                part => resolved.push(part),
            }
        }
        assert_eq!(resolved.join("/"), OS_RELEASE_SHARED);
    }

    #[test]
    fn only_the_profiles_scripts_that_are_still_shipped_get_their_exec_bit() {
        assert_eq!(
            UNEXECUTABLE_SCRIPTS,
            ["usr/bin/bsod", "usr/bin/linexin-sound-fix-usb-dac"]
        );
        for gone in ["usr/local/bin/rum", "usr/bin/photo", "usr/bin/designer"] {
            assert!(!UNEXECUTABLE_SCRIPTS.contains(&gone), "{gone}");
        }
    }

    #[test]
    fn userdel_removes_a_home_only_when_there_is_one() {
        assert_eq!(userdel_args(true), ["-r", "liveuser"]);
        assert_eq!(userdel_args(false), ["liveuser"]);
    }

    #[test]
    fn keyrings_put_arch_first_and_skip_other_files() {
        let names: Vec<String> = [
            "archlinux-revoked",
            "archlinux-trusted",
            "archlinux.gpg",
            "linexin-trusted",
            "linexin.gpg",
            "-evil.gpg",
            "aaa.gpg",
        ]
        .iter()
        .map(|n| n.to_string())
        .collect();
        assert_eq!(keyrings(&names), ["archlinux", "aaa", "linexin"]);
    }
}
