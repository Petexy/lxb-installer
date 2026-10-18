//! Everything the person chooses, as one value the worker can check again.
//!
//! CONTRACT (phase 0): these types are what the interface builds and the
//! worker consumes. Add fields only with both sides in mind; every struct
//! denies unknown fields, so the two halves must be built from one tree.
use super::disks::Disk;
use super::layout::{self, DiskLayout};
use super::secret::Secret;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where on the chosen disk Linexin goes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Target {
    /// Erase the whole disk and make a fresh table.
    WholeDisk,
    /// Use one gap of free space from the inventory (start/size in bytes,
    /// exactly as `Disk::free` listed it).
    FreeSpace { start: u64, size: u64 },
    /// Replace one existing partition.
    Partition { path: String },
    /// Disks laid out by hand on Partitions (Where to install's Advanced):
    /// the end state of every disk the layout changes or mounts from, in
    /// the order the machine lists them, judged by the one rule set both
    /// halves share (`model::layout`). A struct variant, as the tag is
    /// carried inside the value (`{"kind": "layout", "disks": […]}`).
    ///
    /// The request keeps its shape around it: `disk` and `storage` are
    /// pinned to what the layout says (`layout::pinned`) — the disk that
    /// holds `/`, `/`'s filesystem, no separate `/home`, the swap file or
    /// none beside a swap partition — so that everything reading them reads
    /// the layout's answer without knowing layouts. The consent list is
    /// what it destroys (`layout::destroys`). **Carried out only where
    /// QEMU proved it**: the worker refuses a request holding one unless
    /// all it needs lies within one set of `layout::PROVEN`
    /// (`check::validate`), as the interface puts a plan in use only there
    /// (§7.0, Unit Q, Q2).
    Layout { disks: Vec<DiskLayout> },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Filesystem {
    #[default]
    Ext4,
    /// Btrfs with `@` and `@home` (and `@swap` when there is a swap file).
    Btrfs,
}

/// A separate partition for `/home`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Home {
    pub path: String,
    /// Erase it as ext4 (true) or keep what is on it (false).
    pub format: bool,
    /// The disk the partition is on, as the person reviewed it, when that is
    /// not the disk Linexin goes on (`InstallConfig::disk`); `None` when it
    /// is that disk. Carried whole so that the partition is judged from the
    /// disk as it was shown, and so that the worker can find that disk again
    /// and refuse if it changed, as it does the target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<Disk>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Storage {
    pub filesystem: Filesystem,
    /// Size of the swap file in MiB; 0 means no swap file. Default 4096.
    pub swap_mib: u32,
    pub home: Option<Home>,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            filesystem: Filesystem::Ext4,
            swap_mib: 4096,
            home: None,
        }
    }
}

/// Which boot loader to install. `Automatic` is the legacy installer's
/// policy: GRUB on BIOS or a non-GPT EFI partition, systemd-boot when Linexin
/// is alone, rEFInd when another system is found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Bootloader {
    #[default]
    Automatic,
    SystemdBoot,
    Grub,
    Refind,
}

/// The desktop the installed system starts into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Desktop {
    /// The GNOME-based desktop the Linexin ISO carries.
    #[default]
    Linexin,
    /// The Plasma-based desktop (`kinexin-desktop`, from the internet).
    Kinexin,
    /// LineXinBar with the CEDM login screen (`lxb`, `cedm`, from the
    /// internet unless the image already has them).
    LineXinBar,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Software {
    /// Update the installed system from the internet before first start.
    pub updates: bool,
    /// Flatpak application IDs to install (from flathub).
    pub flatpaks: Vec<String>,
    /// Packages of the image to remove from the installed system.
    pub remove: Vec<String>,
    /// Remove the other vendor's CPU microcode and unused GPU drivers.
    pub prune_drivers: bool,
    /// Set up pre-update snapshots (only takes effect on Btrfs).
    pub snapshots: bool,
}

impl Default for Software {
    fn default() -> Self {
        Self {
            updates: false,
            flatpaks: Vec::new(),
            remove: Vec::new(),
            prune_drivers: true,
            snapshots: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keyboard {
    /// The XKB layout (`pl`) and variant (`""`, `dvorak`…).
    pub layout: String,
    pub variant: String,
    /// The console keymap derived from them (`pl2`, `us`).
    pub console: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// `pl_PL.UTF-8`.
    pub locale: String,
    /// `Europe/Warsaw`.
    pub timezone: String,
    pub keyboard: Keyboard,
}

impl Default for Region {
    fn default() -> Self {
        Self {
            locale: "en_US.UTF-8".into(),
            timezone: "UTC".into(),
            keyboard: Keyboard {
                layout: "us".into(),
                variant: String::new(),
                console: "us".into(),
            },
        }
    }
}

// Deliberately no Debug: it holds passwords.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub full_name: String,
    pub username: String,
    pub password: Secret,
    pub hostname: String,
    /// A separate root password; `None` keeps root locked (the default).
    pub root_password: Option<Secret>,
}

// Deliberately no Debug: it holds passwords.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallConfig {
    /// The disk as the person reviewed it.
    pub disk: Disk,
    pub target: Target,
    pub storage: Storage,
    pub bootloader: Bootloader,
    pub desktop: Desktop,
    pub software: Software,
    pub region: Region,
    pub account: Account,
    /// The read-only system image to copy (`/run/archiso/airootfs`).
    pub source: PathBuf,
}

impl InstallConfig {
    /// Whether the person's homes are on a partition kept as it is, whose
    /// files are theirs from before: a separate `/home` kept unformatted
    /// (the easy path's), or a layout that keeps — or only shrinks — the
    /// partition it mounts at `/home` (`layout::keeps_home`). Then the live
    /// account's `/home/liveuser` is not removed with `userdel -r` and the
    /// person's home is not given to them with `chown -R`: both would
    /// reach into what was kept. On the easy path exactly today's
    /// expression, held so by a test over every fixture request.
    pub fn home_kept(&self) -> bool {
        let easy = self.storage.home.as_ref().is_some_and(|home| !home.format);
        let laid = match &self.target {
            Target::Layout { disks } => layout::keeps_home(disks),
            Target::WholeDisk | Target::FreeSpace { .. } | Target::Partition { .. } => false,
        };
        easy || laid
    }
}

/// The system image of an archiso live session: the squashfs underneath the
/// writable overlay, exactly as it was on the medium.
pub const DEFAULT_SOURCE: &str = "/run/archiso/airootfs";

/// The legacy installer's default computer name.
pub const DEFAULT_HOSTNAME: &str = "Linexin-PC";

/// A choice the person made that cannot be installed, said as an
/// identifier the interface words from its catalogs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Invalid {
    UsernameEmpty,
    /// Must start with a lowercase letter or `_`, then lowercase letters,
    /// digits, `_` or `-`; at most 32 characters.
    UsernameShape,
    /// A system account name such as `root` or `liveuser`.
    UsernameReserved,
    FullNameShape,
    HostnameShape,
    PasswordEmpty,
    PasswordMismatch,
    /// Contains a control character or is longer than 128 characters.
    PasswordShape,
    RootPasswordEmpty,
    LocaleUnknown,
    TimezoneUnknown,
    KeyboardUnknown,
    /// Storage choices: owned by the storage work (`disks::validate_storage`).
    DiskBlocked,
    TargetMissing,
    TargetTooSmall,
    /// The disk's partition table has no entry left for the partitions the
    /// installation must add: a DOS table's four primary entries are taken,
    /// or a logical partition would have to give up room for an EFI one.
    TableFull,
    /// The EFI system partition or a BIOS boot partition: what starts the
    /// machine, never a place to install.
    TargetIsEsp,
    /// A partition Windows keeps for itself (Microsoft reserved, Windows
    /// recovery): replacing it would break that Windows's updates or repair.
    TargetProtected,
    HomeInvalid,
    SwapTooLarge,
    BootloaderUnavailable,
    SourceInvalid,
    /// A layout made by hand that the rule set refuses (`layout::shape`),
    /// or whose request's easy fields are not the layout's own
    /// (`layout::pinned`). The interface judges a plan with Partitions'
    /// own words before it ever builds one; this is the storage checks'
    /// answer for one that reached them anyway.
    LayoutRefused,
}

/// Names the installed system must not give a person.
pub const RESERVED_USERNAMES: &[&str] = &[
    "root",
    "daemon",
    "bin",
    "sys",
    "sync",
    "games",
    "man",
    "lp",
    "mail",
    "news",
    "uucp",
    "proxy",
    "www-data",
    "backup",
    "nobody",
    "liveuser",
    "http",
    "ftp",
    "dbus",
    "polkitd",
    "cedm-greeter",
    "gdm",
    "alpm",
    "git",
];

pub fn validate_username(name: &str) -> Result<(), Invalid> {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return Err(Invalid::UsernameEmpty);
    }
    let first_ok = bytes[0].is_ascii_lowercase() || bytes[0] == b'_';
    let rest_ok = bytes
        .iter()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_' || *c == b'-');
    if bytes.len() > 32 || !first_ok || !rest_ok {
        return Err(Invalid::UsernameShape);
    }
    if RESERVED_USERNAMES.contains(&name) {
        return Err(Invalid::UsernameReserved);
    }
    Ok(())
}

/// A full name may be any text a person writes, but not one that would
/// break `/etc/passwd`: no colon, comma, newline or other control character.
pub fn validate_full_name(name: &str) -> Result<(), Invalid> {
    if name.chars().count() > 128 || name.chars().any(|c| c.is_control() || c == ':' || c == ',') {
        return Err(Invalid::FullNameShape);
    }
    Ok(())
}

pub fn validate_hostname(name: &str) -> Result<(), Invalid> {
    let bytes = name.as_bytes();
    let ok = !bytes.is_empty()
        && bytes.len() <= 63
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'-');
    if ok {
        Ok(())
    } else {
        Err(Invalid::HostnameShape)
    }
}

/// Any printable text up to 128 characters, including letters outside
/// ASCII: chpasswd takes the line after the first colon whole.
pub fn validate_password(password: &Secret, repeat: &Secret) -> Result<(), Invalid> {
    if password.is_empty() {
        return Err(Invalid::PasswordEmpty);
    }
    if password.chars() > 128 || password.expose().chars().any(char::is_control) {
        return Err(Invalid::PasswordShape);
    }
    if password != repeat {
        return Err(Invalid::PasswordMismatch);
    }
    Ok(())
}

/// The account half of the checks; the storage half is
/// `disks::validate_storage`, and both run in the interface before the
/// review and in the worker before anything is written.
pub fn validate_account(account: &Account) -> Result<(), Invalid> {
    validate_full_name(&account.full_name)?;
    validate_username(&account.username)?;
    validate_hostname(&account.hostname)?;
    let password = &account.password;
    validate_password(password, password)?;
    if let Some(root) = &account.root_password {
        if root.is_empty() {
            return Err(Invalid::RootPasswordEmpty);
        }
        validate_password(root, root)?;
    }
    Ok(())
}

/// Shape checks only: the values must also exist in the image, which the
/// worker checks against the image itself.
pub fn validate_region(region: &Region) -> Result<(), Invalid> {
    let locale_ok = region.locale.len() < 64
        && region.locale.ends_with(".UTF-8")
        && region
            .locale
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.@-".contains(&c));
    if !locale_ok {
        return Err(Invalid::LocaleUnknown);
    }
    let zone = &region.timezone;
    let zone_ok = !zone.is_empty()
        && zone.len() < 100
        && zone.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_+-".contains(&c))
        });
    if !zone_ok {
        return Err(Invalid::TimezoneUnknown);
    }
    let token = |s: &str, empty_ok: bool| {
        (empty_ok || !s.is_empty())
            && s.len() < 64
            && s.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-+.".contains(&c))
    };
    let k = &region.keyboard;
    if !token(&k.layout, false) || !token(&k.variant, true) || !token(&k.console, false) {
        return Err(Invalid::KeyboardUnknown);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_follow_the_legacy_rules() {
        assert!(validate_username("piotr").is_ok());
        assert!(validate_username("_build-2").is_ok());
        for bad in [
            "",
            "Piotr",
            "1abc",
            "a b",
            "x;reboot",
            "liveuser",
            "root",
            &"a".repeat(33),
        ] {
            assert!(validate_username(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn passwords_take_any_printable_text() {
        let ok = Secret::new("zażółć: gęślą jaźń");
        assert!(validate_password(&ok, &ok).is_ok());
        let other = Secret::new("different");
        assert_eq!(
            validate_password(&ok, &other),
            Err(Invalid::PasswordMismatch)
        );
        let control = Secret::new("line\nbreak");
        assert_eq!(
            validate_password(&control, &control),
            Err(Invalid::PasswordShape)
        );
    }

    #[test]
    fn region_values_cannot_escape_their_directories() {
        let mut region = Region::default();
        assert!(validate_region(&region).is_ok());
        region.timezone = "../../etc/shadow".into();
        assert!(validate_region(&region).is_err());
        region = Region::default();
        region.keyboard.layout = "us;reboot".into();
        assert!(validate_region(&region).is_err());
    }

    #[test]
    fn full_names_cannot_break_passwd() {
        assert!(validate_full_name("Zoë O'Brien").is_ok());
        assert!(validate_full_name("a:b").is_err());
        assert!(validate_full_name("a,b").is_err());
    }
}
