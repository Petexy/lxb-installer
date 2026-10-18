//! The system image the installation copies, looked at before anything is
//! written.
//!
//! The image is the live session's read-only squashfs (or erofs) under
//! `/run/archiso/airootfs`. Everything the later steps take for granted about
//! it is made sure of here, so that a broken or unexpected medium stops the
//! installation before the disk is touched rather than half way through:
//! that it is the read-only filesystem archiso mounted from a loop device,
//! reached through directories only root can change, that it has one kernel
//! to copy, the account and locale files the steps edit, the programs they
//! run inside the target, and the language and time zone the person chose.
use super::safe_token;
use crate::model::catalogs::{self, DesktopOffer};
use crate::model::config::{Desktop, InstallConfig};
use crate::model::live;
use crate::model::protocol::Failure;
use crate::worker::layout::Stop;
use crate::worker::target;
use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// The programs the system steps run inside the target through
/// `arch-chroot`, which must therefore be in the image.
pub const TARGET_PROGRAMS: &[&str] = &[
    "useradd",
    "userdel",
    "groupdel",
    "chpasswd",
    "passwd",
    "visudo",
    "locale-gen",
    "pacman",
    "pacman-key",
    "mkinitcpio",
];

/// The files the system steps edit in place, which must be in the image.
pub const TARGET_FILES: &[&str] = &["etc/passwd", "etc/group", "etc/locale.gen"];

/// The console font the legacy installer set in `vconsole.conf`.
pub const CONSOLE_FONT: &str = "ter-v16n";

/// The kernel the image boots with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kernel {
    /// The directory name in `usr/lib/modules` (`7.0.5-arch1-1`).
    pub version: String,
    /// The package it came from (`linux`), which names its preset and the
    /// file in `/boot` (`vmlinuz-linux`).
    pub pkgbase: String,
}

/// What the image was found to be.
#[derive(Clone, Debug)]
pub struct Image {
    /// The image's resolved path.
    pub path: PathBuf,
    pub kernel: Kernel,
    /// `etc/mkinitcpio.d/<pkgbase>-postinstall.preset` is there to take the
    /// live preset's place (the System step swaps it in).
    pub postinstall_preset: bool,
    /// The three desktops, each marked by whether the image carries it.
    pub desktops: Vec<DesktopOffer>,
    /// The packages installed in the image.
    pub packages: BTreeSet<String>,
    /// The legacy console font is in the image.
    pub console_font: bool,
}

impl Image {
    pub fn carries(&self, desktop: Desktop) -> bool {
        self.desktops
            .iter()
            .any(|offer| offer.desktop == desktop && offer.in_image)
    }

    /// The kernel file inside the image.
    pub fn kernel_file(&self) -> PathBuf {
        self.path
            .join("usr/lib/modules")
            .join(&self.kernel.version)
            .join("vmlinuz")
    }

    /// Where the kernel goes in the target, relative to its root
    /// (`boot/vmlinuz-linux`), which is where the preset looks for it.
    pub fn boot_kernel(&self) -> String {
        format!("boot/vmlinuz-{}", self.kernel.pkgbase)
    }
}

fn invalid(detail: impl Into<String>) -> Stop {
    Stop::new(Failure::ImageInvalid, detail)
}

/// Looks at the image `config.source` names and returns what it is, or
/// `ImageInvalid` saying what is wrong.
pub fn inspect(config: &InstallConfig) -> Result<Image, Stop> {
    let path = std::fs::canonicalize(&config.source)
        .map_err(|e| invalid(format!("{}: {e}", config.source.display())))?;
    let expected =
        std::fs::canonicalize(live::IMAGE).map_err(|e| invalid(format!("{}: {e}", live::IMAGE)))?;
    if path != expected {
        return Err(invalid(format!(
            "{} is not the live session's image {}",
            path.display(),
            live::IMAGE
        )));
    }
    for directory in path.ancestors() {
        let meta = std::fs::symlink_metadata(directory)
            .map_err(|e| invalid(format!("{}: {e}", directory.display())))?;
        if !meta.is_dir() || !safe_owner(meta.uid(), meta.mode()) {
            return Err(invalid(format!(
                "{} could be changed by someone other than root",
                directory.display()
            )));
        }
    }
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")
        .map_err(|e| invalid(format!("cannot read the mount table: {e}")))?;
    mount_problem(&mountinfo, &path).map_or(Ok(()), |detail| Err(invalid(detail)))?;

    for file in TARGET_FILES {
        if !target::is_file(&path, file) {
            return Err(invalid(format!("the image has no {file}")));
        }
    }
    for program in TARGET_PROGRAMS {
        if !target::exists(&path, &format!("usr/bin/{program}")) {
            return Err(invalid(format!("the image has no {program}")));
        }
    }

    let kernel = choose_kernel(kernels(&path)?).map_err(invalid)?;
    let postinstall_preset = target::is_file(
        &path,
        &format!("etc/mkinitcpio.d/{}-postinstall.preset", kernel.pkgbase),
    );
    let preset = target::read(
        &path,
        &format!("etc/mkinitcpio.d/{}.preset", kernel.pkgbase),
    )?;
    preset_usable(postinstall_preset, preset.as_deref()).map_err(invalid)?;

    let region = &config.region;
    if !target::is_file(&path, &format!("usr/share/zoneinfo/{}", region.timezone)) {
        return Err(invalid(format!(
            "the image has no time zone {}",
            region.timezone
        )));
    }
    let locale = locale_source(&region.locale);
    if !target::is_file(&path, &format!("usr/share/i18n/locales/{locale}")) {
        return Err(invalid(format!("the image has no locale {locale}")));
    }

    let desktops = catalogs::desktops(&path);
    let packages = installed(&path);
    let image = Image {
        console_font: target::is_file(
            &path,
            &format!("usr/share/kbd/consolefonts/{CONSOLE_FONT}.psf.gz"),
        ),
        path,
        kernel,
        postinstall_preset,
        desktops,
        packages,
    };
    // Linexin is the image's own desktop and is never downloaded: an image
    // without it cannot install it.
    if config.desktop == Desktop::Linexin && !image.carries(Desktop::Linexin) {
        return Err(invalid("the image does not carry the Linexin desktop"));
    }
    Ok(image)
}

/// A directory on the way to the image must be root's and writable by
/// nobody else, or another user could put a different tree in its place
/// between this check and the copy.
pub fn safe_owner(uid: u32, mode: u32) -> bool {
    uid == 0 && mode & 0o022 == 0
}

/// What is wrong with the mount at `path` for an image to copy, or `None`:
/// the mount on top there must be a squashfs or erofs from a loop device,
/// mounted read-only.
pub fn mount_problem(mountinfo: &str, path: &Path) -> Option<String> {
    let mounts = live::mounts(mountinfo);
    let Some(mount) = mounts
        .iter()
        .rev()
        .find(|m| Path::new(&m.mount_point) == path)
    else {
        return Some(format!("nothing is mounted at {}", path.display()));
    };
    if !matches!(mount.fs_type.as_str(), "squashfs" | "erofs") {
        return Some(format!(
            "{} is {}, not a squashfs or erofs image",
            path.display(),
            mount.fs_type
        ));
    }
    if !mount.source.starts_with("/dev/loop") {
        return Some(format!(
            "{} is mounted from {}, not a loop device",
            path.display(),
            mount.source
        ));
    }
    if !mount.options.iter().any(|o| o == "ro") {
        return Some(format!("{} is not mounted read-only", path.display()));
    }
    None
}

/// Each directory in the image's `usr/lib/modules`: its name, the package it
/// says it belongs to, and whether it holds a kernel.
fn kernels(image: &Path) -> Result<Vec<(String, Option<String>, bool)>, Stop> {
    let mut found = Vec::new();
    for name in target::list(image, "usr/lib/modules")? {
        let directory = format!("usr/lib/modules/{name}");
        if !target::is_dir(image, &directory) {
            continue;
        }
        let pkgbase = target::read(image, &format!("{directory}/pkgbase"))?
            .map(|text| text.trim().to_string());
        let has_kernel = target::is_file(image, &format!("{directory}/vmlinuz"));
        found.push((name, pkgbase, has_kernel));
    }
    Ok(found)
}

/// The kernel to install from the module directories found: the one with a
/// `vmlinuz` and a `pkgbase`, or, when the image has several, the one from
/// the `linux` package (the one the preset is written for). Names that could
/// be read as options or paths are refused.
pub fn choose_kernel(found: Vec<(String, Option<String>, bool)>) -> Result<Kernel, String> {
    let kernels: Vec<Kernel> = found
        .into_iter()
        .filter_map(|(version, pkgbase, has_kernel)| match pkgbase {
            Some(pkgbase) if has_kernel => Some(Kernel { version, pkgbase }),
            _ => None,
        })
        .collect();
    let kernel = match kernels.len() {
        0 => return Err("the image has no kernel in usr/lib/modules".into()),
        1 => kernels.into_iter().next().expect("one kernel"),
        _ => {
            let mut linux: Vec<Kernel> = kernels
                .into_iter()
                .filter(|k| k.pkgbase == "linux")
                .collect();
            if linux.len() != 1 {
                return Err("the image has several kernels and none is the one from linux".into());
            }
            linux.remove(0)
        }
    };
    if !safe_token(&kernel.version) || !safe_token(&kernel.pkgbase) {
        return Err(format!(
            "the image's kernel has an unusable name: {:?} {:?}",
            kernel.version, kernel.pkgbase
        ));
    }
    Ok(kernel)
}

/// Whether the installed system will get a usable preset: either the image
/// has the `-postinstall` preset to swap in, or its own preset is not
/// archiso's (which builds a live-medium initramfs that cannot start an
/// installed system).
pub fn preset_usable(postinstall: bool, preset: Option<&str>) -> Result<(), String> {
    if postinstall {
        return Ok(());
    }
    match preset {
        Some(text) if !text.contains("archiso") => Ok(()),
        Some(_) => Err("the image has only the live medium's mkinitcpio preset".into()),
        None => Err("the image has no mkinitcpio preset for its kernel".into()),
    }
}

/// The glibc locale source a locale is generated from (`pl_PL` for
/// `pl_PL.UTF-8`).
pub fn locale_source(locale: &str) -> &str {
    locale.strip_suffix(".UTF-8").unwrap_or(locale)
}

/// The packages installed in the image, from the names of pacman's local
/// database directories (`name-version-release`).
fn installed(image: &Path) -> BTreeSet<String> {
    target::list(image, "var/lib/pacman/local")
        .unwrap_or_default()
        .iter()
        .filter_map(|entry| package_name(entry))
        .map(str::to_string)
        .collect()
}

/// The package name in a local database entry: names may hold hyphens,
/// versions and releases never do, so it is all before the second hyphen
/// from the end.
pub fn package_name(entry: &str) -> Option<&str> {
    let mut parts = entry.rsplitn(3, '-');
    let release = parts.next()?;
    let version = parts.next()?;
    let name = parts.next()?;
    (!name.is_empty() && !version.is_empty() && !release.is_empty()).then_some(name)
}

/// An image as the tests need one: the desktops it carries and its
/// packages, with an invented kernel.
#[cfg(test)]
pub fn fixture(carried: &[Desktop], packages: &[&str]) -> Image {
    Image {
        path: PathBuf::from(live::IMAGE),
        kernel: Kernel {
            version: "1.2.3-example1-1".into(),
            pkgbase: "linux".into(),
        },
        postinstall_preset: true,
        desktops: [Desktop::Linexin, Desktop::Kinexin, Desktop::LineXinBar]
            .into_iter()
            .map(|desktop| DesktopOffer {
                desktop,
                in_image: carried.contains(&desktop),
            })
            .collect(),
        packages: packages.iter().map(|p| p.to_string()).collect(),
        console_font: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        version: &str,
        pkgbase: Option<&str>,
        has_kernel: bool,
    ) -> (String, Option<String>, bool) {
        (version.into(), pkgbase.map(str::to_string), has_kernel)
    }

    #[test]
    fn the_one_kernel_is_chosen() {
        let kernel = choose_kernel(vec![
            entry("7.0.5-arch1-1", Some("linux"), true),
            // Modules a package left without a kernel (extramodules) do not count.
            entry("extramodules-7.0", None, false),
        ])
        .unwrap();
        assert_eq!(kernel.version, "7.0.5-arch1-1");
        assert_eq!(kernel.pkgbase, "linux");
    }

    #[test]
    fn of_several_kernels_the_linux_one_is_chosen() {
        let kernel = choose_kernel(vec![
            entry("6.12.9-1-lts", Some("linux-lts"), true),
            entry("7.0.5-arch1-1", Some("linux"), true),
        ])
        .unwrap();
        assert_eq!(kernel.pkgbase, "linux");
        assert!(choose_kernel(vec![
            entry("6.12.9-1-lts", Some("linux-lts"), true),
            entry("7.0.5-1-zen", Some("linux-zen"), true),
        ])
        .is_err());
        let lone = choose_kernel(vec![entry("7.0.5-1-zen", Some("linux-zen"), true)]).unwrap();
        assert_eq!(lone.pkgbase, "linux-zen");
    }

    #[test]
    fn no_kernel_or_an_unsafe_name_is_refused() {
        assert!(choose_kernel(vec![]).is_err());
        assert!(choose_kernel(vec![entry("7.0.5", Some("linux"), false)]).is_err());
        assert!(choose_kernel(vec![entry("7.0.5", Some("-linux"), true)]).is_err());
        assert!(choose_kernel(vec![entry("7.0 5", Some("linux"), true)]).is_err());
    }

    #[test]
    fn only_a_read_only_loop_image_is_accepted() {
        let good = "\
25 1 0:24 / /run rw shared:6 - tmpfs run rw
32 25 7:0 / /run/archiso/airootfs ro,relatime shared:11 - squashfs /dev/loop0 ro
";
        let path = Path::new("/run/archiso/airootfs");
        assert_eq!(mount_problem(good, path), None);
        assert!(mount_problem("", path).is_some());
        let writable = good.replace("airootfs ro,relatime", "airootfs rw,relatime");
        assert!(mount_problem(&writable, path).is_some());
        let disk = good.replace("/dev/loop0", "/dev/sdz2");
        assert!(mount_problem(&disk, path).is_some());
        let ext4 = good.replace("squashfs /dev/loop0", "ext4 /dev/loop0");
        assert!(mount_problem(&ext4, path).is_some());
        let covered = format!("{good}40 25 0:40 / /run/archiso/airootfs rw - tmpfs tmpfs rw\n");
        assert!(mount_problem(&covered, path).is_some());
    }

    #[test]
    fn only_root_may_change_the_way_to_the_image() {
        assert!(safe_owner(0, 0o40755));
        assert!(safe_owner(0, 0o40555));
        assert!(!safe_owner(1000, 0o40755));
        assert!(!safe_owner(0, 0o40775));
        assert!(!safe_owner(0, 0o41777));
    }

    #[test]
    fn the_live_preset_alone_is_not_enough() {
        assert!(preset_usable(true, None).is_ok());
        assert!(preset_usable(
            false,
            Some("ALL_kver='/boot/vmlinuz-linux'\nPRESETS=('default')\n")
        )
        .is_ok());
        assert!(preset_usable(
            false,
            Some("ALL_config='/etc/mkinitcpio.conf.d/archiso.conf'\n")
        )
        .is_err());
        assert!(preset_usable(false, None).is_err());
    }

    #[test]
    fn locale_sources_and_package_names() {
        assert_eq!(locale_source("pl_PL.UTF-8"), "pl_PL");
        assert_eq!(locale_source("C"), "C");
        assert_eq!(package_name("gnome-shell-1:48.1-1"), Some("gnome-shell"));
        assert_eq!(
            package_name("linexin-installer-260717-1"),
            Some("linexin-installer")
        );
        assert_eq!(package_name("broken"), None);
    }
}
