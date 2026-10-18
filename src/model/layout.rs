//! Hand-made layouts: the rules a disk laid out by hand on Partitions is
//! judged by, kept in one place so the interface and the worker cannot
//! disagree about them (`INSTALLER-HANDOFF.md` §7.0, Unit Q, "One rule
//! set").
//!
//! The numbers both halves build partitions by live here too: how a new
//! partition is aligned, how large the partitions the installer makes are,
//! where on a disk a partition may stand at all ([`usable`]), and how much
//! room an EFI system partition must have free. The worker's planner of the
//! easy path (`worker::partition::plan`) and its EFI check
//! (`worker::storage::esp`) read them from here, unchanged.
//!
//! Pure: nothing here reads or writes the machine.
use super::config::{Bootloader, Filesystem, InstallConfig, Software, Target};
use super::disks::{
    self, device_path_ok, part_type, Blocked, Content, Disk, Partition, Table, GIB, MAX_SWAP,
    MBR_PRIMARIES, MIB, MIN_HOME, MIN_INSTALL,
};
use super::firmware::Firmware;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Every new partition starts and ends on a multiple of this.
pub const ALIGN: u64 = MIB;
/// The EFI system partition the installer makes: room for several kernels
/// and their start-up images, and for snapshot boot entries, which is what
/// systemd-boot needs when the kernels live on it.
pub const ESP_SIZE: u64 = 2 * GIB;
/// GRUB's core image on a GPT disk started by a BIOS needs a partition of
/// its own; 1 MiB is many times what it uses.
pub const BIOS_BOOT_SIZE: u64 = MIB;
/// The Linux boot partition made beside a shared EFI system partition too
/// small for the kernels: the same room the installer's own EFI partition
/// gives them.
pub const LINUX_BOOT_SIZE: u64 = 2 * GIB;
/// An MBR stores starts and sizes in 32 bits, so nothing on an msdos table
/// can end past this sector.
pub const MBR_LIMIT: u64 = 1 << 32;
/// The size of one GPT partition entry array (128 entries of 128 bytes),
/// which a GPT keeps a backup of at the end of the disk.
const GPT_ENTRIES_BYTES: u64 = 128 * 128;

/// A shared EFI system partition holds the kernels (mounted at `/boot`) only
/// with at least this much free: a kernel, its start-up image and the
/// fallback one, and room for the snapshot boot entries. With less it is
/// mounted at `/efi` for the boot loader alone, and the kernels go on a new
/// Linux boot partition.
pub const ESP_BOOT_FREE: u64 = 600 * MIB;
/// With less than this free, even a boot loader's own files do not fit.
pub const ESP_MIN_FREE: u64 = 32 * MIB;

/// A run of sectors, `start` included, `end` not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u64,
    pub end: u64,
}

impl Span {
    pub fn len(self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.end <= self.start
    }

    pub fn overlaps(self, other: Span) -> bool {
        self.start < other.end && other.start < self.end
    }

    pub fn contains(self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

/// The sectors of a disk that a partition can use, trimmed to whole MiB: from
/// the first MiB (past the table) to the GPT's backup at the end, or to the
/// last sector an MBR can address.
pub fn usable(table: Table, total: u64, sector: u64) -> Span {
    let align = ALIGN / sector;
    let end = match table {
        Table::Gpt => total.saturating_sub(1 + GPT_ENTRIES_BYTES.div_ceil(sector)),
        Table::Msdos => total.min(MBR_LIMIT),
    };
    Span {
        start: align,
        end: end / align * align,
    }
}

// -------------------------------------------------------------------------
// What a partition is made with and where it is used.

/// A filesystem a partition can be formatted as, by the name GParted gives
/// it — the names Format as offers, in its order — and the `bios_grub`
/// partition a new partition can be on a BIOS machine's gpt disk, which
/// holds no filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fs {
    Ext4,
    Btrfs,
    Xfs,
    Fat32,
    LinuxSwap,
    /// GRUB's core on a BIOS machine's gpt disk: 1 MiB, unformatted.
    BiosGrub,
}

/// The least swap the plan makes or accepts: less is not worth a partition.
pub const SWAP_LEAST: u64 = 512 * MIB;

impl Fs {
    /// Every filesystem Format as offers, in the order it offers them.
    pub const ALL: [Fs; 5] = [Fs::Ext4, Fs::Btrfs, Fs::Xfs, Fs::Fat32, Fs::LinuxSwap];

    /// GParted's name for it, never translated: `ext4`, `fat32`,
    /// `linux-swap`, and `bios_grub` for GRUB's own (its flag's name).
    pub fn name(self) -> &'static str {
        match self {
            Fs::Ext4 => "ext4",
            Fs::Btrfs => "btrfs",
            Fs::Xfs => "xfs",
            Fs::Fat32 => "fat32",
            Fs::LinuxSwap => "linux-swap",
            Fs::BiosGrub => "bios_grub",
        }
    }

    /// lsblk's name for it, which a partition's `fs_type` holds — so a copy
    /// of a disk as a plan leaves it is read as any disk is.
    pub fn lsblk(self) -> &'static str {
        match self {
            Fs::Ext4 => "ext4",
            Fs::Btrfs => "btrfs",
            Fs::Xfs => "xfs",
            Fs::Fat32 => "vfat",
            Fs::LinuxSwap => "swap",
            Fs::BiosGrub => "",
        }
    }

    /// What a partition formatted so holds, as discovery would classify it.
    pub fn content(self) -> Content {
        match self {
            Fs::Ext4 | Fs::Btrfs | Fs::Xfs => Content::Linux,
            Fs::Fat32 => Content::Data,
            Fs::LinuxSwap => Content::Swap,
            Fs::BiosGrub => Content::BiosBoot,
        }
    }

    /// The least a partition of it is made at: what its tools make at the
    /// least (GParted's own floors for btrfs and FAT32, xfsprogs' 300 MiB),
    /// swap worth having, and GRUB's 1 MiB exactly.
    pub fn least(self) -> u64 {
        match self {
            Fs::Ext4 => 32 * MIB,
            Fs::Btrfs => 256 * MIB,
            Fs::Xfs => 300 * MIB,
            Fs::Fat32 => 33 * MIB,
            Fs::LinuxSwap => SWAP_LEAST,
            Fs::BiosGrub => BIOS_BOOT_SIZE,
        }
    }

    /// The filesystem of this name (`map::filesystem`'s), if it is one a
    /// plan makes.
    pub fn named(name: &str) -> Option<Fs> {
        [
            Fs::Ext4,
            Fs::Btrfs,
            Fs::Xfs,
            Fs::Fat32,
            Fs::LinuxSwap,
            Fs::BiosGrub,
        ]
        .into_iter()
        .find(|fs| fs.name() == name)
    }
}

/// Where a partition is used in the installed system: the mount points
/// Mount point offers, in its order, swap, and a path of the person's own
/// (Other…). It travels as its path (`/`, `/home`, `swap`, `/data`), the one
/// spelling a person reads too.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum MountPoint {
    Root,
    Home,
    Efi,
    Boot,
    Swap,
    /// A path typed in, held to [`mount_path`]'s rules.
    Other(String),
}

impl MountPoint {
    /// Every mount point offered by name, in the order Mount point offers
    /// them; Other… comes after them.
    pub const ALL: [MountPoint; 5] = [
        MountPoint::Root,
        MountPoint::Home,
        MountPoint::Efi,
        MountPoint::Boot,
        MountPoint::Swap,
    ];

    /// Its path, or `swap`, never translated.
    pub fn path(&self) -> &str {
        match self {
            MountPoint::Root => "/",
            MountPoint::Home => "/home",
            MountPoint::Efi => "/efi",
            MountPoint::Boot => "/boot",
            MountPoint::Swap => "swap",
            MountPoint::Other(path) => path,
        }
    }

    /// The mount point `path` names: one offered by name where it is that
    /// one's path, else a path of one's own, as it is written (whether it
    /// may be used is [`mount_path`]'s question).
    pub fn at(path: &str) -> MountPoint {
        MountPoint::ALL
            .iter()
            .find(|named| named.path() == path)
            .cloned()
            .unwrap_or_else(|| MountPoint::Other(path.to_string()))
    }

    /// Whether a partition holding `fs` (by `map::filesystem`'s name) suits
    /// it: `/` ext4 or Btrfs, `/home` those or xfs, `/efi` and `/boot` FAT32 (the
    /// firmware reads them, and the worker's Linux boot partition is FAT),
    /// swap `linux-swap`, a path of one's own any filesystem that mounts.
    pub fn suits(&self, fs: Option<&str>) -> bool {
        match self {
            MountPoint::Root => matches!(fs, Some("ext4" | "btrfs")),
            MountPoint::Home => matches!(fs, Some("ext4" | "btrfs" | "xfs")),
            MountPoint::Efi | MountPoint::Boot => fs == Some("fat32"),
            MountPoint::Swap => fs == Some("linux-swap"),
            MountPoint::Other(_) => fs.is_some_and(|fs| !matches!(fs, "linux-swap" | "extended")),
        }
    }

    /// The least a partition used here may be, by the worker's numbers: `/`
    /// the smallest place Where to install offers (`MIN_INSTALL`, to a whole
    /// MiB), `/home` the worker's least home, `/efi` room for a boot loader,
    /// `/boot` room for the kernels, swap [`SWAP_LEAST`].
    pub fn least(&self) -> u64 {
        match self {
            MountPoint::Root => MIN_INSTALL.div_ceil(MIB) * MIB,
            MountPoint::Home => MIN_HOME,
            MountPoint::Efi => ESP_MIN_FREE,
            MountPoint::Boot => ESP_BOOT_FREE,
            MountPoint::Swap => SWAP_LEAST,
            MountPoint::Other(_) => 0,
        }
    }

    /// The filesystem a partition used here is made with where the one in
    /// hand does not suit it: what choosing a mount point on New partition
    /// puts in its File system row, so the two agree.
    pub fn suited(&self, fs: Fs) -> Fs {
        if self.suits(Some(fs.name())) {
            return fs;
        }
        match self {
            MountPoint::Efi | MountPoint::Boot => Fs::Fat32,
            MountPoint::Swap => Fs::LinuxSwap,
            MountPoint::Root | MountPoint::Home | MountPoint::Other(_) => Fs::Ext4,
        }
    }
}

impl From<String> for MountPoint {
    fn from(path: String) -> Self {
        if path == "swap" {
            return MountPoint::Swap;
        }
        MountPoint::at(&path)
    }
}

impl From<MountPoint> for String {
    fn from(at: MountPoint) -> Self {
        at.path().to_string()
    }
}

// -------------------------------------------------------------------------
// A mount point of one's own, and a label.

/// Why a path typed for a mount point of one's own cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathRefused {
    /// It is not a whole path from `/` (or has `..` in it, or nothing).
    NotWhole,
    /// Longer than [`PATH_MOST`] bytes.
    Long,
    /// It holds whitespace, a control character or a backslash, which an
    /// fstab line would have to escape: refused rather than escaped.
    Spaces,
    /// Another partition is used there already.
    Twice(String),
    /// The system's own folders, which are on `/` and nowhere else, and the
    /// swap file's (`/swap`, `/swapfile`).
    System(String),
    /// Inside `/home`, `/boot` or `/efi` (the one named): a partition
    /// mounted inside `/home` would be crossed by the new account's
    /// `chown -R` and by the live account's `userdel -r`, and the boot
    /// loaders own `/boot` and `/efi` whole.
    Under(String),
}

/// The system's own folders, which live on `/` itself and are never a
/// partition of their own (`/proc`, `/sys`, `/dev` and `/run` are not on a
/// disk at all) — nor is anything under them.
pub const SYSTEM_FOLDERS: [&str; 10] = [
    "/proc", "/sys", "/dev", "/run", "/usr", "/etc", "/bin", "/sbin", "/lib", "/lib64",
];

/// Where the installer puts a swap file (a Btrfs `@swap` subvolume, or a
/// file on `/`): never a mount point of one's own, nor anything under it.
pub const SWAP_FILE_PATHS: [&str; 2] = ["/swap", "/swapfile"];

/// The mount points offered by name that nothing of one's own is mounted
/// inside ([`PathRefused::Under`]).
pub const NOTHING_INSIDE: [&str; 3] = ["/home", "/boot", "/efi"];

/// The longest a mount point of one's own may be, in bytes.
pub const PATH_MOST: usize = 255;

/// `typed` as a path: `//` made `/`, a `/.` in the middle or at the end
/// taken out, the `/` at its end taken off (but for `/` itself). `None` for
/// what is not a whole path from `/`, or goes up with `..`.
pub fn normal_path(typed: &str) -> Option<String> {
    let typed = typed.trim();
    if !typed.starts_with('/') {
        return None;
    }
    let mut parts = Vec::new();
    for part in typed.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            part => parts.push(part),
        }
    }
    Some(format!("/{}", parts.join("/")))
}

/// The mount point `typed` is: a whole path, normalised ([`normal_path`]);
/// one of the mount points offered by name is that one (`/home` typed is
/// `/home`); any other held to [`own_path`]'s rules. Whether another
/// partition is used there already is the plan's to say.
pub fn mount_path(typed: &str) -> Result<MountPoint, PathRefused> {
    let path = normal_path(typed).ok_or(PathRefused::NotWhole)?;
    let at = MountPoint::at(&path);
    if let MountPoint::Other(path) = &at {
        own_path(path)?;
    }
    Ok(at)
}

/// Whether `path`, exactly as it is written, may be a mount point of one's
/// own: a whole path in [`normal_path`]'s form and not `/` itself; at most
/// [`PATH_MOST`] bytes; no whitespace, control character or backslash
/// (refused rather than escaped in the fstab); never the system's own
/// folders, the swap file's, or anything under them; never inside
/// [`NOTHING_INSIDE`]. What travels in a layout is held to it again, so a
/// path the interface never offered is refused all the same.
pub fn own_path(path: &str) -> Result<(), PathRefused> {
    if normal_path(path).as_deref() != Some(path) || path == "/" {
        return Err(PathRefused::NotWhole);
    }
    if path.len() > PATH_MOST {
        return Err(PathRefused::Long);
    }
    if path
        .chars()
        .any(|letter| letter.is_whitespace() || letter.is_control() || letter == '\\')
    {
        return Err(PathRefused::Spaces);
    }
    let system = SYSTEM_FOLDERS
        .iter()
        .chain(SWAP_FILE_PATHS.iter())
        .find(|folder| at_or_under(path, folder));
    if let Some(folder) = system {
        return Err(PathRefused::System((*folder).to_string()));
    }
    let inside = NOTHING_INSIDE
        .iter()
        .find(|folder| path != **folder && at_or_under(path, folder));
    if let Some(folder) = inside {
        return Err(PathRefused::Under((*folder).to_string()));
    }
    Ok(())
}

/// Whether `path` is `folder` or anything under it.
pub fn at_or_under(path: &str, folder: &str) -> bool {
    path == folder
        || path
            .strip_prefix(folder)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// How long a label a filesystem holds: so many bytes of UTF-8, or so many
/// characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    Bytes(usize),
    Chars(usize),
}

impl Limit {
    /// Whether `label` fits.
    pub fn fits(self, label: &str) -> bool {
        match self {
            Limit::Bytes(most) => label.len() <= most,
            Limit::Chars(most) => label.chars().count() <= most,
        }
    }
}

/// The label a filesystem (by `map::filesystem`'s name) holds, where it
/// holds one at all: ext2/3/4 16 bytes, FAT32 11, xfs 12, btrfs 255, swap
/// 16, NTFS 128 characters, exFAT 11 characters. `None` for what holds no
/// label Partitions sets — no filesystem, `bios_grub`, an extended one.
pub fn label_limit(filesystem: Option<&str>) -> Option<Limit> {
    Some(match filesystem? {
        "ext2" | "ext3" | "ext4" => Limit::Bytes(16),
        "fat32" | "fat16" => Limit::Bytes(11),
        "xfs" => Limit::Bytes(12),
        "btrfs" => Limit::Bytes(255),
        "linux-swap" => Limit::Bytes(16),
        "ntfs" => Limit::Chars(128),
        "exfat" => Limit::Chars(11),
        _ => return None,
    })
}

// -------------------------------------------------------------------------
// Where a new partition can be made, and how small one can be shrunk.

/// Whether `part` is a logical partition of a DOS table.
pub fn logical(table: Option<Table>, part: &Partition) -> bool {
    table == Some(Table::Msdos) && part.number > 4
}

/// Whether something is written over the whole of `disk` rather than into a
/// partition — a filesystem, LUKS, LVM or RAID made on the disk itself
/// (`Disk::content`), even beside a partition table — which is never written
/// over: nothing is made beside it, since a partition made there would be
/// written into it, and no new table goes over it, since the consent's
/// `table` line names only the partitions it erases. [`shape`] refuses both
/// ([`Refusal::Mismatch`]); Partitions offers neither ([`can_create`], and
/// Options' new table), and says why. Discovery lists no free space on such
/// a disk, so a formatted partition grows on it only over the partitions
/// the layout deletes after it ([`Refusal::Overlap`]), and Partitions'
/// Resize offers it no more.
pub fn written_whole(disk: &Disk) -> bool {
    disk.content.is_some()
}

/// Whether the rule set lays out partitions on `disk` at all by the size of
/// its logical sector: a power of two from 512 to 4096 bytes, the sizes the
/// worker's tools are proven at (its image tests run at 512 and 4096). Any
/// other — a disk whose logical sector is larger — [`shape`] refuses whole
/// ([`Refusal::Mismatch`]), whatever is asked of it, so Partitions offers
/// nothing on it, no new table among it, and says so.
pub fn sector_fits(disk: &Disk) -> bool {
    let sector = disk.logical_sector;
    (512..=4096).contains(&sector) && sector.is_power_of_two()
}

/// Why no new partition can be made in a run of unallocated space: it needs
/// nothing written over the whole disk ([`written_whole`]), a partition
/// table, a place in it on an msdos disk (four primary partitions at most),
/// to stand outside an extended partition (no logical partitions are made),
/// and on an msdos disk no extended partition kept at all (by design, as
/// on Where to install: a new partition's place beside one would be
/// sfdisk's to choose).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoNew {
    Whole,
    Table,
    Primaries,
    Logical,
    BesideExtended,
}

/// How many primary partitions an msdos `disk` — as a plan leaves it — has:
/// its partitions numbered one to four, its extended one among them, and
/// every partition the plan makes (counted as primary: none is logical).
pub fn primaries(disk: &Disk) -> usize {
    disk.partitions
        .iter()
        .filter(|part| !logical(disk.table, part))
        .count()
}

/// Whether a new partition can be made in the unallocated space from
/// `start` of `disk` — as a plan leaves it — and if not, why.
pub fn can_create(disk: &Disk, start: u64) -> Result<(), NoNew> {
    if written_whole(disk) {
        return Err(NoNew::Whole);
    }
    let Some(table) = disk.table else {
        return Err(NoNew::Table);
    };
    if table == Table::Msdos {
        let inside = disk.partitions.iter().any(|part| {
            part.is_extended() && start >= part.start && start < part.start + part.size
        });
        if inside {
            return Err(NoNew::Logical);
        }
        if disk.partitions.iter().any(Partition::is_extended) {
            return Err(NoNew::BesideExtended);
        }
        if primaries(disk) >= MBR_PRIMARIES {
            return Err(NoNew::Primaries);
        }
    }
    Ok(())
}

/// What is left on a filesystem over what it uses before it is shrunk:
/// a tenth of what is used, and never less than a GiB — room for the
/// filesystem's own bookkeeping, for the resize tools, which refuse to
/// shrink a filesystem to within a few per cent of its data (ntfsresize
/// among them), and for what is written on it before the person's first
/// boot.
pub fn margin(used: u64) -> u64 {
    (used / 10).max(GIB)
}

// -------------------------------------------------------------------------
// The request: disks laid out by hand.

/// One disk of a hand-made layout: the disk as the person reviewed it, and
/// what becomes of it — the end state of each partition, never the list of
/// operations that led there. Partitions moves an operation asked for again
/// to the end of its list ("resize sda3, create, resize sda3 again" lists
/// the create first), so the list's order is no order to carry it out in;
/// the end state has none to get wrong.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct DiskLayout {
    /// The whole disk as reviewed, carried as `Home::disk` carries one: the
    /// worker finds it again by every field and refuses if any changed.
    pub disk: Disk,
    /// A new partition table written over the disk, if one is: every
    /// partition on it is then deleted.
    pub table: Option<Table>,
    /// One for each partition of `disk`, in its order: what becomes of it.
    pub parts: Vec<Part>,
    /// The partitions made, in start order.
    pub create: Vec<Create>,
}

/// One partition of the disk as reviewed — named by everything that says
/// which one it is — and what becomes of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Part {
    pub path: String,
    pub number: u32,
    pub start: u64,
    pub size: u64,
    pub fate: Fate,
}

/// What becomes of a partition. A kept partition never grows and is never
/// retyped; a label is set only on a filesystem the layout makes, as part
/// of making it (only when formatting, by design).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Fate {
    /// Kept as it is, mounted at `mount` or nowhere.
    Keep { mount: Option<MountPoint> },
    /// Kept, its filesystem and its entry shrunk to `to` bytes from its
    /// start, which never moves: an ext4 or NTFS one only.
    Shrink { to: u64, mount: Option<MountPoint> },
    /// A new filesystem made on it, labelled `label` (empty for none), its
    /// entry `size` bytes long: the size it was reviewed at, or another
    /// where the entry alone is resized (its start never moves).
    Format {
        fs: Fs,
        label: String,
        size: u64,
        mount: Option<MountPoint>,
    },
    /// Deleted. Under a new table, every partition is.
    Delete,
}

impl Fate {
    /// Where the partition is used, if anywhere.
    pub fn mount(&self) -> Option<&MountPoint> {
        match self {
            Fate::Keep { mount } | Fate::Shrink { mount, .. } | Fate::Format { mount, .. } => {
                mount.as_ref()
            }
            Fate::Delete => None,
        }
    }

    /// Whether what is on the partition is kept: kept as it is, or shrunk.
    pub fn keeps(&self) -> bool {
        matches!(self, Fate::Keep { .. } | Fate::Shrink { .. })
    }
}

/// A partition made: where it starts and how long it is, in bytes and on a
/// MiB both, what it is made with, its label (empty for none) and where it
/// is used.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Create {
    pub start: u64,
    pub size: u64,
    pub fs: Fs,
    pub label: String,
    pub mount: Option<MountPoint>,
}

// -------------------------------------------------------------------------
// What can be kept, mounted and shrunk.

/// The filesystems a kept partition may be mounted from: those the
/// installed system mounts with what it has (ntfs3, vfat and exfat are the
/// kernel's own). Never `crypto_LUKS`, `BitLocker`, `LVM2_member`,
/// `linux_raid_member`, `iso9660`, `squashfs`, `zfs_member`, swap, or what
/// discovery could not name.
pub const MOUNTABLE: [&str; 9] = [
    "ext2", "ext3", "ext4", "btrfs", "xfs", "f2fs", "vfat", "exfat", "ntfs",
];

/// The filesystems a kept `/home` may be: Linux's own, which hold owners
/// and modes.
pub const KEPT_HOME_FILESYSTEMS: [&str; 6] = ["ext2", "ext3", "ext4", "btrfs", "xfs", "f2fs"];

/// Where the copied image puts files of its own, and everything below: a
/// partition used there must be formatted (a kept one would have the
/// image's files copied over what is on it) and hold what the image puts
/// there ([`measured`]).
pub const IMAGE_PATHS: [&str; 5] = ["/var", "/opt", "/srv", "/root", "/tmp"];

/// The filesystems a kept partition can be shrunk: ext4 and NTFS, the two
/// this version starts with.
pub const SHRINKABLE: [&str; 2] = ["ext4", "ntfs"];

/// The most entries a GPT made by the usual tools holds.
const GPT_MOST: usize = super::disks::GPT_ENTRIES;

/// Whether a kept partition holding `fs_type` (lsblk's name) can be
/// mounted ([`MOUNTABLE`]).
pub fn mountable(fs_type: &str) -> bool {
    MOUNTABLE.contains(&fs_type)
}

/// Whether the copied image puts files at `path` ([`IMAGE_PATHS`]).
pub fn image_fills(path: &str) -> bool {
    IMAGE_PATHS.iter().any(|folder| at_or_under(path, folder))
}

/// Whether `part`, kept, can be shrunk: ext4 or NTFS, and never an EFI
/// system partition, Microsoft's reserved space, a `bios_grub` partition,
/// Windows' recovery tools or an extended partition.
pub fn shrinkable(part: &Partition) -> bool {
    SHRINKABLE.contains(&part.fs_type.as_str())
        && !part.is_extended()
        && !matches!(
            part.content,
            Content::Efi
                | Content::MicrosoftReserved
                | Content::BiosBoot
                | Content::WindowsRecovery
        )
}

/// Whether `part` is an EFI system partition that can be kept as one: its
/// type says so and it holds FAT.
pub fn kept_esp(part: &Partition) -> bool {
    part.content == Content::Efi && part.fs_type == "vfat"
}

/// The least a filesystem `used` bytes full can be shrunk to: what it uses
/// and a [`margin`], and never below the least its own resize tool names
/// (`resize2fs -P`, `ntfsresize --info`), up to a whole MiB.
pub fn shrink_floor(used: u64, tool_least: Option<u64>) -> u64 {
    (used + margin(used))
        .max(tool_least.unwrap_or(0))
        .div_ceil(MIB)
        * MIB
}

/// The least a partition made with `fs` can be on a disk of `sector`-byte
/// sectors: [`Fs::least`], and for FAT32 the 65,525 clusters it needs at
/// least, a cluster being a sector at the least — 33 MiB on 512-byte
/// sectors, 257 MiB on 4096-byte ones, where `mkfs.fat` refuses a smaller
/// FAT32.
pub fn fs_least(fs: Fs, sector: u32) -> u64 {
    match fs {
        Fs::Fat32 => {
            let sector = u64::from(sector.max(512));
            let clusters = 65_525 * sector;
            let tables = 2 * 4 * 65_525;
            let reserved = 32 * sector;
            fs.least()
                .max((clusters + tables + reserved).div_ceil(MIB) * MIB)
        }
        fs => fs.least(),
    }
}

/// Whether a filesystem made as `fs` suits being used at `at`: `/` ext4 or
/// Btrfs, `/home` ext4, Btrfs or xfs, `/efi` and `/boot` FAT32, swap
/// `linux-swap`, a path of one's own any filesystem but swap. `bios_grub`
/// holds none, and is used nowhere.
pub fn made_suits(at: &MountPoint, fs: Fs) -> bool {
    match at {
        MountPoint::Root => matches!(fs, Fs::Ext4 | Fs::Btrfs),
        MountPoint::Home => matches!(fs, Fs::Ext4 | Fs::Btrfs | Fs::Xfs),
        MountPoint::Efi | MountPoint::Boot => fs == Fs::Fat32,
        MountPoint::Swap => fs == Fs::LinuxSwap,
        MountPoint::Other(_) => !matches!(fs, Fs::LinuxSwap | Fs::BiosGrub),
    }
}

/// The letters a FAT label may hold: capital letters, digits, the space
/// and the signs FAT's directory entries allow.
pub fn fat_letter(letter: char) -> bool {
    letter.is_ascii_uppercase() || letter.is_ascii_digit() || " !#$%&'()-@^_`{}~".contains(letter)
}

/// What a label of `filesystem` (by `map::filesystem`'s name) takes for
/// `letter`, typed `first` or after others: FAT's own letters, made
/// capitals as GParted writes them, never a space first; no control
/// character; nothing starting with `-`, which a tool would read as one of
/// its options; no `/` anywhere ([`LABEL_REFUSES`]). `None` for a letter it
/// cannot take.
pub fn label_letter(filesystem: Option<&str>, letter: char, first: bool) -> Option<char> {
    if letter.is_control() || (first && letter == '-') || letter == LABEL_REFUSES {
        return None;
    }
    if matches!(filesystem, Some("fat32" | "fat16")) {
        let letter = letter.to_ascii_uppercase();
        return (fat_letter(letter) && !(first && letter == ' ')).then_some(letter);
    }
    Some(letter)
}

/// The letter no label holds: a label is handed to the tool that makes its
/// filesystem as an argument of its own, beside the device, and the worker's
/// fence (`worker::manual::fence`) takes every argument naming `/dev` for a
/// device to judge — so a label holding `/` ("a/devops", "/dev/sdb") could
/// stop a run after its tables were written, or be read as a disk. Without
/// it no label can name a device or a path at all. (FAT never held one.)
pub const LABEL_REFUSES: char = '/';

/// Whether `label` can be given a filesystem made as `fs`: empty, or within
/// its byte limit — ext4 16, FAT32 11 (in FAT's own letters, never
/// starting with a space), xfs 12, Btrfs 255, swap 16 — with no control
/// character, no `/` ([`LABEL_REFUSES`]) and not starting with `-` (which a
/// tool would read as one of its options). `bios_grub` holds none.
pub fn label_fits(fs: Fs, label: &str) -> bool {
    if label.is_empty() {
        return true;
    }
    if label.starts_with('-')
        || label.contains(LABEL_REFUSES)
        || label.chars().any(char::is_control)
    {
        return false;
    }
    match fs {
        Fs::Ext4 | Fs::LinuxSwap => label.len() <= 16,
        Fs::Fat32 => label.len() <= 11 && !label.starts_with(' ') && label.chars().all(fat_letter),
        Fs::Xfs => label.len() <= 12,
        Fs::Btrfs => label.len() <= 255,
        Fs::BiosGrub => false,
    }
}

/// The table `layout` leaves its disk with: the new one, or the one it has.
pub fn table_after(layout: &DiskLayout) -> Option<Table> {
    layout.table.or(layout.disk.table)
}

/// Where the extended partition `extended` of `disk` reaches at the least:
/// the kernel reports one as a 1 KiB stub, so as far as the logical
/// partitions inside it go, or its own entry where that is longer. Nothing
/// is ever made beside a kept one, so only what lies inside it is judged
/// by this.
fn extended_reach(disk: &Disk, extended: &Partition) -> u64 {
    disk.partitions
        .iter()
        .filter(|part| logical(disk.table, part))
        .map(|part| part.start + part.size)
        .fold(extended.start + extended.size, u64::max)
}

// -------------------------------------------------------------------------
// What reading the disks found.

/// What reading a kept partition with privilege found: its numbers, or why
/// they are none. Read by the measure entry on a live system and by the
/// worker again before it writes; the demo has its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Measure {
    /// `used` bytes in use and `free` bytes free, as the filesystem says,
    /// and the least its own resize tool would shrink it to, where it says
    /// one. For an NTFS, only once a `hiberfil.sys` of its own says Windows
    /// is not hibernated there.
    Read {
        used: u64,
        free: u64,
        least: Option<u64>,
    },
    /// An NTFS its tool read cleanly — `used` bytes in use, `free` free —
    /// that holds no `hiberfil.sys` of its own: a Windows' data volume (a
    /// "D:"; Windows keeps that file on the partition it starts from
    /// alone), or the partition of a Windows whose hibernation is switched
    /// off. Whether a Windows that is hibernated, or was shut down by Fast
    /// Startup, still holds what is on it only its journal says, which is
    /// not read, so it is never shrunk ([`Measures::shrink_of`]) — but it
    /// holds no other NTFS back.
    NoHiberfile { used: u64, free: u64 },
    /// Windows is hibernated on it, or started fast: its filesystem is
    /// held for Windows to come back to.
    Hibernated,
    /// Its filesystem asks to be checked first.
    NeedsCheck,
    /// BitLocker or LUKS: never touched.
    Encrypted,
    /// Mounted, or swap, or something sits on it.
    InUse,
    /// Its tool could not read it — or, on a disk the measure entry did not
    /// read (not as listed any more, or one nothing may be made on), it was
    /// not read at all.
    Unreadable,
}

impl Measure {
    /// What the filesystem uses, where reading said: what Partitions'
    /// details show as Used.
    pub fn used(&self) -> Option<u64> {
        match self {
            Measure::Read { used, .. } | Measure::NoHiberfile { used, .. } => Some(*used),
            _ => None,
        }
    }
}

/// One partition's measure, named by the partition as it was read: its
/// path, its filesystem (lsblk's name for it, `ntfs`, `BitLocker`…), its
/// filesystem's UUID and its size — so a measure of another partition at
/// the same path is never taken for this one's, and every NTFS and
/// BitLocker volume of the measurement is known as one
/// ([`Measures::shrink_of`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Measured {
    pub path: String,
    pub fs: String,
    pub uuid: String,
    pub size: u64,
    pub measure: Measure,
}

impl Measured {
    /// Whether this is the measure of `part`, as it was reviewed.
    fn of(&self, part: &Partition) -> bool {
        self.path == part.path
            && self.fs == part.fs_type
            && self.uuid == part.uuid
            && self.size == part.size
    }
}

/// Every measure there is: of every disk the measurement was asked about,
/// read or not.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Measures {
    pub parts: Vec<Measured>,
}

/// What a kept ext4 or NTFS that may be shrunk was read as
/// ([`Measures::shrink_of`]): the bytes its filesystem uses, the least its
/// own resize tool would shrink it to, where it says one, and the
/// filesystem's own size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shrinkable {
    pub used: u64,
    pub least: Option<u64>,
    /// The filesystem's own size, as reading said it: what it uses and what
    /// is free on it — an ext4's blocks (`dumpe2fs -h`), an NTFS volume's
    /// clusters and the sector of its backup boot sector, as `ntfsresize
    /// --info` counts it. Its partition's size, or nearly, where it fills
    /// it; less where it was left smaller than its partition — a partition
    /// grown without its filesystem, or a filesystem shrunk by a run that
    /// stopped before its entry was.
    pub filesystem: u64,
}

impl Shrinkable {
    /// **Whether an entry `to` bytes long holds the filesystem as it is**:
    /// then only the entry is made smaller (`sfdisk -N`), and the filesystem
    /// is left as it is — never handed to its tool, which given a size at or
    /// above its own would grow it. Below it, the filesystem is shrunk
    /// first, to `to`. The rule set's, the interface's and the worker's
    /// alike (`measured`, `ui::partitions::resize_room`,
    /// `manual::shrinking::ready`).
    pub fn entry_only(&self, to: u64) -> bool {
        self.filesystem <= to
    }

    /// **The least its entry may be made**: what the filesystem uses and a
    /// margin, never below its tool's own least ([`shrink_floor`]), where
    /// the filesystem is shrunk; but where it is already smaller than that,
    /// its own size, up to a whole MiB — an entry that holds it is made
    /// smaller alone ([`entry_only`](Self::entry_only)), the filesystem left
    /// as it was.
    pub fn least_entry(&self) -> u64 {
        shrink_floor(self.used, self.least).min(self.filesystem.div_ceil(MIB) * MIB)
    }
}

/// Why reading says a kept ext4 or NTFS may not be shrunk
/// ([`Measures::shrink_of`]): what was read of it, or — for an NTFS — what
/// was read of another Windows volume of the measurement (`other`, by its
/// device).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unshrinkable {
    /// Its own measure: `Hibernated`, `NeedsCheck`, `Encrypted`, `InUse` or
    /// `Unreadable`, never `Read` nor `NoHiberfile`.
    Itself(Measure),
    /// An NTFS that holds no `hiberfil.sys` of its own
    /// ([`Measure::NoHiberfile`]): a Windows' data volume, or the partition
    /// of a Windows whose hibernation is switched off — never told apart.
    /// Only an NTFS whose own file says Windows is not hibernated is
    /// shrunk; Windows itself can shrink this one.
    NoHiberfile,
    /// Another NTFS, `other`, is signed hibernated: a Windows on this
    /// computer is hibernated, or was shut down by Fast Startup, and may
    /// hold this one too.
    Hibernated { other: String },
    /// Whether a Windows keeps `other`, another NTFS, could not be read: its
    /// tool or its sign of hibernation failed, it asks for Windows' check
    /// first, it is in use, or its disk was not read.
    Unread { other: String },
    /// `other` is BitLocker: whatever Windows' it is, whether that Windows
    /// is hibernated cannot be read.
    BitLocker { other: String },
}

impl Measures {
    /// The measure of `part`, as it was reviewed, if there is one.
    pub fn of(&self, part: &Partition) -> Option<&Measure> {
        self.parts
            .iter()
            .find(|one| one.of(part))
            .map(|one| &one.measure)
    }

    /// **Whether `part`, kept, may be shrunk — the one place the rule set
    /// reads a measurement for a shrink**: the interface's from the measure
    /// entry's answer, and the worker's own in phase A (step Q9, which
    /// measures every disk again first). `None` when nothing was read of
    /// it; its numbers when it may be; why not when it may not.
    ///
    /// An ext4 may be shrunk when it was read (`Measure::Read`). **An NTFS
    /// only when all of these hold** (§6; Unit Q, step Q8):
    ///
    /// 1. `ntfsresize --info` read it cleanly, and
    /// 2. a `hiberfil.sys` of its own exists and says Windows is not
    ///    hibernated there (`Read`: without one it is `NoHiberfile`, never
    ///    shrunk, though it holds no other back); and
    /// 3. across every disk of the measurement, no other NTFS is signed
    ///    hibernated, none could not be read whole — its tool or its sign
    ///    failed (an error, not a missing file), it asks for Windows'
    ///    check, it is in use, its disk was not read — and no volume is
    ///    BitLocker.
    ///
    /// For a Windows that is hibernated, or was shut down by Fast Startup,
    /// keeps every NTFS it had mounted as it left it, and it may be another
    /// Windows than the one whose partition this is: the partition one
    /// Windows starts from can be another's data volume. Only a
    /// `hiberfil.sys` says a Windows is hibernated, and it is read only on
    /// the partition that Windows starts from — so one that cannot be read,
    /// anywhere, holds every NTFS back.
    pub fn shrink_of(&self, part: &Partition) -> Option<Result<Shrinkable, Unshrinkable>> {
        let shrinkable = match self.of(part)? {
            Measure::Read { used, free, least } => Shrinkable {
                used: *used,
                least: *least,
                filesystem: used.saturating_add(*free),
            },
            Measure::NoHiberfile { .. } => return Some(Err(Unshrinkable::NoHiberfile)),
            why => return Some(Err(Unshrinkable::Itself(*why))),
        };
        if part.fs_type == "ntfs" {
            if let Some(why) = self.windows_held(part) {
                return Some(Err(why));
            }
        }
        Some(Ok(shrinkable))
    }

    /// What holds every NTFS back but `part` itself, across every disk of
    /// the measurement ([`shrink_of`](Self::shrink_of), 3.): the first NTFS
    /// signed hibernated, then the first that could not be read whole, then
    /// the first BitLocker volume — each in the measurement's order. An NTFS
    /// read, or read with no `hiberfil.sys` of its own, holds nothing back.
    fn windows_held(&self, part: &Partition) -> Option<Unshrinkable> {
        let others: Vec<&Measured> = self.parts.iter().filter(|one| !one.of(part)).collect();
        let ntfs = |measure: fn(&Measure) -> bool| {
            others
                .iter()
                .find(|one| one.fs == "ntfs" && measure(&one.measure))
                .map(|one| one.path.clone())
        };
        if let Some(other) = ntfs(|measure| *measure == Measure::Hibernated) {
            return Some(Unshrinkable::Hibernated { other });
        }
        let unread = |measure: &Measure| {
            !matches!(
                measure,
                Measure::Read { .. } | Measure::NoHiberfile { .. } | Measure::Hibernated
            )
        };
        if let Some(other) = ntfs(unread) {
            return Some(Unshrinkable::Unread { other });
        }
        others
            .iter()
            .find(|one| one.fs == "BitLocker")
            .map(|one| Unshrinkable::BitLocker {
                other: one.path.clone(),
            })
    }
}

/// How much the image puts in each folder it fills (`du -sb
/// --one-file-system` of the image's own), by the folder's path.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ImageSizes {
    pub paths: BTreeMap<String, u64>,
}

// -------------------------------------------------------------------------
// What a layout is refused for.

/// A partition a refusal is about: one the disk has, by its device, or one
/// the layout makes, by its disk and where it starts (Partitions names it
/// "New Partition #N" from its own plan). The worker's executor
/// (`worker::manual`) keys what it finds again on the disks by it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Which {
    Part(String),
    Made { disk: String, start: u64 },
}

/// Why a hand-made layout cannot be carried out, as an identifier: the
/// interface says it in the catalogs' words (Partitions keeps GParted's),
/// the worker answers `InvalidRequest`. [`shape`] gives the first in the
/// order its sections are written in, [`measured`] the first of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    // The disks.
    /// A disk that cannot be used — the installer's own medium, one in use,
    /// one read-only — is in it (one too small for Linexin may hold
    /// anything but `/`).
    Blocked { disk: String },
    /// What it says of a disk is not the disk as it was read: a disk named
    /// twice, a partition that is not the one reviewed, a start that moved,
    /// a partition kept under a new table, an extended partition formatted,
    /// something made on a disk with no table, something made beside or a
    /// new table written over what is written on the whole disk. Partitions
    /// never makes one.
    Mismatch { disk: String },
    // Where partitions stand.
    /// Made, or its entry changed, off a whole MiB.
    Unaligned { part: Which },
    /// Past what its disk's table reaches (the backup GPT, an MBR's 2 TiB),
    /// or a logical partition past its extended one.
    Outside { part: Which },
    /// It lies over another partition — or, a logical partition's entry
    /// grown, over the table (EBR) of a logical partition kept after it on
    /// the disk, which stands in the space before that one; or, on a disk
    /// something is written over as a whole, a formatted entry grown past
    /// the partitions the layout deletes after it, into what may be that
    /// whole-disk content.
    Overlap { part: Which },
    /// An msdos disk would hold more than four primary partitions.
    Primaries { disk: String },
    /// Something is made on an msdos disk that keeps an extended partition
    /// (refused by design, as on Where to install): its place would be
    /// sfdisk's to choose, inside or beside the extended one.
    BesideExtended { disk: String },
    /// A logical partition is deleted while one numbered after it is kept:
    /// the kernel would number the kept one anew halfway through. (Numbered
    /// after, not standing after: logical partitions are numbered in the
    /// order their tables are chained, which need not be the disk's.)
    LogicalOrder { part: Which },
    /// A GPT would hold more than 128 entries.
    Entries { disk: String },
    // What is done to them.
    /// A shrink of what cannot be shrunk ([`shrinkable`]).
    CantShrink { part: Which },
    /// A kept partition made larger: kept partitions only shrink.
    Grows { part: Which },
    /// A `bios_grub` partition anywhere but a gpt disk a BIOS starts, of
    /// anything but 1 MiB, or used at a mount point or labelled.
    BiosGrub { part: Which },
    /// A label its filesystem cannot hold ([`label_fits`]).
    Label { part: Which },
    // Where they are used.
    /// A mount point used by more than one partition (swap may be several).
    Twice { at: MountPoint },
    /// Nothing is mounted at `/`.
    Root,
    /// The partition at a mount point is smaller than it must be: `/`
    /// `MIN_INSTALL` (and never on a disk too small for Linexin), `/home`
    /// `MIN_HOME`, swap [`SWAP_LEAST`], a formatted `/boot` and `/efi` room
    /// for the kernels and for a boot loader, a folder the image fills what
    /// it puts there ([`measured`]).
    TooSmall { at: MountPoint, least: u64 },
    /// Kept as it is where only a fresh filesystem will do: anywhere but
    /// `/home`, an EFI system partition at `/efi` or (alone) at `/boot`, or
    /// a path of one's own the image does not fill.
    MustFormat { part: Which, at: MountPoint },
    /// The filesystem at a mount point does not suit it.
    Needs { at: MountPoint },
    /// A kept filesystem the installer cannot mount ([`MOUNTABLE`]); `fs`
    /// is lsblk's name for it, empty for none.
    Unmountable { part: Which, fs: String },
    /// A path of one's own that breaks [`own_path`]'s rules.
    Path { why: PathRefused },
    /// Something mounted inside `at`, where the kept partition `part` is
    /// mounted. A kept partition is not mounted while Linexin is installed,
    /// so the folder for a mount inside it would be made in it at the first
    /// start — a write to what is kept, which is written to only to be
    /// shrunk — or, where it did not mount, on `/`, and hidden once it did.
    InsideKept { part: Which, at: MountPoint },
    /// A filesystem made on a partition smaller than its tools make one
    /// ([`fs_least`]).
    FsTooSmall { part: Which, fs: Fs, least: u64 },
    /// Restore points asked for on a Btrfs `/` beside a separate `/var`,
    /// whose snapshots would not hold the package database they restore.
    RestorePoints,
    // What starts the computer.
    /// On UEFI, nothing at `/efi` or `/boot` to be the EFI system partition.
    Efi,
    /// On UEFI, an EFI system partition at `/efi` with no `/boot` beside it
    /// for the kernels: mount it at `/boot`, or add a `/boot`.
    Boot,
    /// `/boot` on another disk than the one it must stand on: the EFI
    /// system partition's on UEFI, `/`'s on BIOS (named by `beside`).
    BootElsewhere { beside: MountPoint },
    /// Something mounted at `/efi` on a machine a BIOS starts.
    EfiOnBios,
    /// On BIOS, the gpt disk `/` is on has no `bios_grub` partition for
    /// GRUB's core.
    BiosGrubMissing { disk: String },
    /// On BIOS, the msdos disk `/` is on has a partition in its first MiB,
    /// where GRUB's core goes.
    GrubGap { disk: String },
    /// The boot loader chosen cannot start this layout: systemd-boot needs
    /// UEFI and a GPT on the EFI system partition's disk, rEFInd UEFI.
    Bootloader { loader: Bootloader },
    // What reading the disks found ([`measured`]).
    /// How much of a partition to be shrunk, or of a kept EFI system
    /// partition at `/boot`, is used is not known.
    NotRead { part: Which },
    /// Shrunk below what it uses and a margin ([`shrink_floor`]).
    ShrinkBelow { part: Which, least: u64 },
    /// Reading said it cannot be shrunk: what was read of it, or of
    /// another Windows volume ([`Measures::shrink_of`]).
    Unshrinkable { part: Which, why: Unshrinkable },
    /// A kept EFI system partition with less free than it must have:
    /// `ESP_BOOT_FREE` at `/boot`, `ESP_MIN_FREE` at `/efi`.
    EspFull { part: Which, least: u64 },
    /// How much the image puts in a folder it fills is not known.
    ImageUnknown { at: MountPoint },
}

// -------------------------------------------------------------------------
// The rules.

/// Whether `layout` can be carried out on a machine started by `firmware`,
/// with `bootloader` and the restore points `software` asks for — judged
/// from the disks as they were reviewed, with no measurements (those are
/// [`measured`]'s). The first thing refused, section by section:
///
/// 1. *the disks:* each named once, by a path discovery makes, none that
///    cannot be used ([`Refusal::Blocked`]: one too small for Linexin may
///    hold anything but `/`); a part for every partition reviewed, in its
///    order, with its path, number, start and size; every one deleted under
///    a new table, and no new table over what is written on the whole disk;
///    creates in start order, on a disk with a table and nothing written
///    over the whole of it;
/// 2. *where partitions stand:* creates and changed entries on a MiB and
///    inside what the table reaches ([`usable`]); nothing overlapping, a
///    kept partition taken at its new end and an extended one as far as its
///    logical partitions reach; on a disk something is written over as a
///    whole, a formatted entry grown only over the partitions the layout
///    deletes after it, never into what discovery lists as no free space;
///    on msdos four primaries at most, nothing
///    made beside a kept extended partition, no logical partition deleted
///    before one numbered after it, nor its entry grown while one after it
///    on the disk stays (that one's own table, its EBR, stands before it);
///    on gpt 128 entries at most;
/// 3. *what is done to them:* shrinks only of what [`shrinkable`] allows,
///    and only smaller; `bios_grub` only on a BIOS machine's gpt disk, 1 MiB
///    exactly, unmounted and unlabelled; every label one its filesystem
///    holds;
/// 4. *where they are used:* each mount point once (swap aside); a `/`
///    large enough; each path of one's own by [`own_path`]; a kept
///    partition only where one may be kept, in a filesystem that suits it
///    and that the installer can mount; a made one in a filesystem that
///    suits where it is used ([`made_suits`]); each at least what its mount
///    point needs; nothing mounted inside a kept partition's path; every
///    filesystem made at least what its tools make; no restore points
///    beside a separate `/var`;
/// 5. *what starts the computer:* on UEFI an EFI system partition alone at
///    `/boot`, or at `/efi` with a `/boot` on its disk; on BIOS nothing at
///    `/efi`, a `/boot` only on `/`'s disk, and a `bios_grub` partition (gpt)
///    or a free first MiB (msdos) on `/`'s disk; the boot loader one that
///    can start it.
pub fn shape(
    layout: &[DiskLayout],
    firmware: Firmware,
    bootloader: Bootloader,
    software: &Software,
) -> Result<(), Refusal> {
    disks(layout)?;
    for disk in layout {
        geometry(disk)?;
    }
    for disk in layout {
        operations(disk, firmware)?;
    }
    mounts(layout, software)?;
    boot(layout, firmware, bootloader)
}

/// [`shape`]'s first section: the disks are the ones reviewed.
fn disks(layout: &[DiskLayout]) -> Result<(), Refusal> {
    let mut seen = BTreeSet::new();
    for one in layout {
        let disk = &one.disk;
        let mismatch = || Refusal::Mismatch {
            disk: disk.path.clone(),
        };
        if !seen.insert(disk.path.as_str()) || !device_path_ok(&disk.path) {
            return Err(mismatch());
        }
        let small = disk.blocked == Some(Blocked::TooSmall);
        if (disk.blocked.is_some() && !small) || disk.read_only {
            return Err(Refusal::Blocked {
                disk: disk.path.clone(),
            });
        }
        if !sector_fits(disk) {
            return Err(mismatch());
        }
        let reviewed = one.parts.len() == disk.partitions.len()
            && one.parts.iter().zip(&disk.partitions).all(|(part, was)| {
                part.path == was.path
                    && part.number == was.number
                    && part.start == was.start
                    && part.size == was.size
            });
        if !reviewed {
            return Err(mismatch());
        }
        if one.table.is_some() && one.parts.iter().any(|part| part.fate != Fate::Delete) {
            return Err(mismatch());
        }
        // An extended partition is a frame: kept unused, or deleted — and
        // deleted only with every logical partition in it.
        let kept_inside = one
            .parts
            .iter()
            .zip(&disk.partitions)
            .any(|(part, was)| logical(disk.table, was) && part.fate != Fate::Delete);
        for (part, was) in one.parts.iter().zip(&disk.partitions) {
            let unused = matches!(part.fate, Fate::Keep { mount: None });
            let gone = part.fate == Fate::Delete;
            if was.is_extended() && (!(unused || gone) || (gone && kept_inside)) {
                return Err(mismatch());
            }
        }
        let ordered = one
            .create
            .windows(2)
            .all(|pair| pair[0].start < pair[1].start);
        // Something written over the whole disk (a filesystem, LUKS, LVM,
        // RAID) is never written over: nothing is made beside it, and no new
        // table goes over it, which the consent's `table` line could not
        // name ([`written_whole`]). Partitions offers neither — its
        // unallocated space no New…, Options no new table — and the details
        // say why.
        let whole = written_whole(disk);
        let room_to_make = table_after(one).is_some() && !whole;
        if !ordered || (!one.create.is_empty() && !room_to_make) || (one.table.is_some() && whole) {
            return Err(mismatch());
        }
    }
    Ok(())
}

/// A partition as a layout leaves it on its disk: what [`geometry`] judges.
struct Stands {
    which: Which,
    start: u64,
    end: u64,
    logical: bool,
    extended: bool,
}

/// [`shape`]'s second section, for one disk: where its partitions stand.
fn geometry(one: &DiskLayout) -> Result<(), Refusal> {
    let disk = &one.disk;
    let Some(table) = table_after(one) else {
        return Ok(());
    };
    let sector = u64::from(disk.logical_sector);
    let room = usable(table, disk.size / sector, sector);
    let (first, last) = (room.start * sector, room.end * sector);
    let inside = |start: u64, end: u64| start >= first && end <= last;
    let on_mib = |bytes: u64| bytes.is_multiple_of(MIB);
    // Under a new table nothing that was there stands any more.
    let table_old = if one.table.is_some() {
        None
    } else {
        disk.table
    };
    let mut stands = Vec::new();
    for (part, was) in one.parts.iter().zip(&disk.partitions) {
        let which = Which::Part(part.path.clone());
        let end = match &part.fate {
            Fate::Delete => continue,
            Fate::Keep { .. } if was.is_extended() => extended_reach(disk, was),
            Fate::Keep { .. } => part.start + part.size,
            Fate::Shrink { to, .. } => {
                if !on_mib(part.start + to) {
                    return Err(Refusal::Unaligned { part: which });
                }
                part.start + to
            }
            Fate::Format { size, .. } => {
                let end = part.start + size;
                if *size != part.size {
                    if !on_mib(end) || *size == 0 {
                        return Err(Refusal::Unaligned { part: which });
                    }
                    if !inside(part.start, end) {
                        return Err(Refusal::Outside { part: which });
                    }
                }
                end
            }
        };
        stands.push(Stands {
            which,
            start: part.start,
            end,
            logical: logical(table_old, was),
            extended: was.is_extended(),
        });
    }
    for made in &one.create {
        let which = Which::Made {
            disk: disk.path.clone(),
            start: made.start,
        };
        if !on_mib(made.start) || !on_mib(made.size) || made.size == 0 {
            return Err(Refusal::Unaligned { part: which });
        }
        if !inside(made.start, made.start + made.size) {
            return Err(Refusal::Outside { part: which });
        }
        stands.push(Stands {
            which,
            start: made.start,
            end: made.start + made.size,
            logical: false,
            extended: false,
        });
    }
    // The top level: primary partitions, extended frames, what is made.
    let mut top: Vec<&Stands> = stands.iter().filter(|one| !one.logical).collect();
    top.sort_by_key(|one| one.start);
    for pair in top.windows(2) {
        if pair[1].start < pair[0].end {
            return Err(Refusal::Overlap {
                part: pair[1].which.clone(),
            });
        }
    }
    // Inside the extended frame: the logical partitions, each inside it.
    let frame = stands.iter().find(|one| one.extended);
    let mut inner: Vec<&Stands> = stands.iter().filter(|one| one.logical).collect();
    inner.sort_by_key(|one| one.start);
    for one in &inner {
        if frame.is_none_or(|frame| one.start < frame.start || one.end > frame.end) {
            return Err(Refusal::Outside {
                part: one.which.clone(),
            });
        }
    }
    for pair in inner.windows(2) {
        if pair[1].start < pair[0].end {
            return Err(Refusal::Overlap {
                part: pair[1].which.clone(),
            });
        }
    }
    // On a disk something is written over as a whole, discovery lists no
    // free space ([`written_whole`]): what lies between its partitions and
    // after them may be that whole-disk content's. A formatted partition
    // grows there only over what was a partition the layout deletes — a
    // neighbour's own span, one after another with no gap between — never
    // into the rest. (Nothing is made on such a disk at all.)
    if written_whole(disk) {
        let deleted: Vec<(u64, u64)> = one
            .parts
            .iter()
            .zip(&disk.partitions)
            .filter(|(part, _)| part.fate == Fate::Delete)
            .map(|(_, was)| {
                let end = if was.is_extended() {
                    extended_reach(disk, was)
                } else {
                    was.start + was.size
                };
                (was.start, end)
            })
            .collect();
        for part in &one.parts {
            let Fate::Format { size, .. } = part.fate else {
                continue;
            };
            let (mut reach, end) = (part.start + part.size, part.start + size);
            while reach < end {
                match deleted
                    .iter()
                    .find(|(start, stop)| *start <= reach && reach < *stop)
                {
                    Some((_, stop)) => reach = *stop,
                    None => {
                        return Err(Refusal::Overlap {
                            part: Which::Part(part.path.clone()),
                        })
                    }
                }
            }
        }
    }
    match table {
        Table::Msdos => {
            if top.len() > MBR_PRIMARIES {
                return Err(Refusal::Primaries {
                    disk: disk.path.clone(),
                });
            }
            if frame.is_some() && !one.create.is_empty() {
                return Err(Refusal::BesideExtended {
                    disk: disk.path.clone(),
                });
            }
            let logicals: Vec<(&Part, bool)> = one
                .parts
                .iter()
                .zip(&disk.partitions)
                .filter(|(_, was)| logical(table_old, was))
                .map(|(part, _)| (part, part.fate == Fate::Delete))
                .collect();
            for (part, deleted) in &logicals {
                // The kernel numbers logical partitions in the order their
                // tables are chained, which need not be the order they stand
                // in on the disk: deleting one renumbers those numbered after
                // it, wherever they stand.
                let numbered_after = logicals
                    .iter()
                    .any(|(other, gone)| !gone && other.number > part.number);
                if *deleted && numbered_after {
                    return Err(Refusal::LogicalOrder {
                        part: Which::Part(part.path.clone()),
                    });
                }
                // Before each logical partition stands its own table, its
                // EBR, which discovery never shows and the overlap above
                // never sees: a logical entry grows past where it ended only
                // once every logical partition after it on the disk is
                // deleted — after it by where it starts, not by its number,
                // since a table chained out of the disk's order numbers a
                // partition standing after it lower. (A formatted entry is
                // the only one that may grow at all; a kept one growing is
                // `Grows`.)
                let kept_after = logicals
                    .iter()
                    .any(|(other, gone)| !gone && other.start > part.start);
                let grown = matches!(part.fate, Fate::Format { size, .. } if size > part.size);
                if grown && kept_after {
                    return Err(Refusal::Overlap {
                        part: Which::Part(part.path.clone()),
                    });
                }
            }
        }
        Table::Gpt => {
            if stands.len() > GPT_MOST {
                return Err(Refusal::Entries {
                    disk: disk.path.clone(),
                });
            }
        }
    }
    Ok(())
}

/// [`shape`]'s third section, for one disk: what is done to its partitions.
fn operations(one: &DiskLayout, firmware: Firmware) -> Result<(), Refusal> {
    let table = table_after(one);
    let grub_here = firmware == Firmware::Bios && table == Some(Table::Gpt);
    let made = |which: Which, fs: Fs, label: &str, size: u64, mount: Option<&MountPoint>| {
        if fs == Fs::BiosGrub && (!grub_here || size != BIOS_BOOT_SIZE || mount.is_some()) {
            return Err(Refusal::BiosGrub { part: which });
        }
        if !label_fits(fs, label) {
            return Err(Refusal::Label { part: which });
        }
        Ok(())
    };
    for (part, was) in one.parts.iter().zip(&one.disk.partitions) {
        let which = Which::Part(part.path.clone());
        match &part.fate {
            Fate::Shrink { to, .. } => {
                if !shrinkable(was) || *to == 0 {
                    return Err(Refusal::CantShrink { part: which });
                }
                if *to >= part.size {
                    return Err(Refusal::Grows { part: which });
                }
            }
            Fate::Format {
                fs,
                label,
                size,
                mount,
            } => made(which, *fs, label, *size, mount.as_ref())?,
            Fate::Keep { .. } | Fate::Delete => {}
        }
    }
    for create in &one.create {
        let which = Which::Made {
            disk: one.disk.path.clone(),
            start: create.start,
        };
        made(
            which,
            create.fs,
            &create.label,
            create.size,
            create.mount.as_ref(),
        )?;
    }
    Ok(())
}

/// One partition a layout uses at a mount point, as it will be.
struct Use<'a> {
    /// Its disk, by its place in the layout.
    disk: usize,
    which: Which,
    at: &'a MountPoint,
    /// The filesystem made on it; `None` where what is on it is kept.
    made: Option<Fs>,
    /// The partition as reviewed, for one that is there now.
    was: Option<&'a Partition>,
    /// Its size once the layout is carried out.
    size: u64,
}

/// Every partition `layout` uses at a mount point, in the order the mount
/// points are offered (`/`, `/home`, `/efi`, `/boot`, swap, paths of one's
/// own by their paths).
fn uses(layout: &[DiskLayout]) -> Vec<Use<'_>> {
    let mut used = Vec::new();
    for (index, one) in layout.iter().enumerate() {
        for (part, was) in one.parts.iter().zip(&one.disk.partitions) {
            let Some(at) = part.fate.mount() else {
                continue;
            };
            let (made, size) = match &part.fate {
                Fate::Shrink { to, .. } => (None, *to),
                Fate::Format { fs, size, .. } => (Some(*fs), *size),
                _ => (None, part.size),
            };
            used.push(Use {
                disk: index,
                which: Which::Part(part.path.clone()),
                at,
                made,
                was: Some(was),
                size,
            });
        }
        for create in &one.create {
            let Some(at) = &create.mount else {
                continue;
            };
            used.push(Use {
                disk: index,
                which: Which::Made {
                    disk: one.disk.path.clone(),
                    start: create.start,
                },
                at,
                made: Some(create.fs),
                was: None,
                size: create.size,
            });
        }
    }
    used.sort_by(|one, other| one.at.cmp(other.at));
    used
}

/// [`shape`]'s fourth section: where the partitions are used.
fn mounts(layout: &[DiskLayout], software: &Software) -> Result<(), Refusal> {
    let used = uses(layout);
    for pair in used.windows(2) {
        if pair[0].at == pair[1].at && *pair[0].at != MountPoint::Swap {
            return Err(Refusal::Twice {
                at: pair[0].at.clone(),
            });
        }
    }
    let Some(root) = used.iter().find(|one| *one.at == MountPoint::Root) else {
        return Err(Refusal::Root);
    };
    let small_disk = layout[root.disk].disk.blocked == Some(Blocked::TooSmall);
    if root.size < MIN_INSTALL || small_disk {
        return Err(Refusal::TooSmall {
            at: MountPoint::Root,
            least: MIN_INSTALL,
        });
    }
    let efi_used = used.iter().any(|one| *one.at == MountPoint::Efi);
    for one in &used {
        if let MountPoint::Other(path) = one.at {
            own_path(path).map_err(|why| Refusal::Path { why })?;
        }
        let sector = layout[one.disk].disk.logical_sector;
        match (one.made, one.was) {
            (None, Some(was)) => {
                let may_keep = match one.at {
                    MountPoint::Home => true,
                    MountPoint::Efi => kept_esp(was),
                    MountPoint::Boot => kept_esp(was) && !efi_used,
                    MountPoint::Other(path) => !image_fills(path),
                    MountPoint::Root | MountPoint::Swap => false,
                };
                if !may_keep {
                    return Err(Refusal::MustFormat {
                        part: one.which.clone(),
                        at: one.at.clone(),
                    });
                }
                let fs = was.fs_type.as_str();
                if *one.at == MountPoint::Home && !KEPT_HOME_FILESYSTEMS.contains(&fs) {
                    return Err(Refusal::Needs { at: one.at.clone() });
                }
                if !mountable(fs) {
                    return Err(Refusal::Unmountable {
                        part: one.which.clone(),
                        fs: fs.to_string(),
                    });
                }
            }
            (Some(fs), _) if !made_suits(one.at, fs) => {
                return Err(Refusal::Needs { at: one.at.clone() })
            }
            _ => {}
        }
        // A kept EFI system partition's room is a measurement's to judge.
        let least = match (one.at, one.made) {
            (MountPoint::Home, _) => MIN_HOME,
            (MountPoint::Swap, Some(_)) => SWAP_LEAST,
            (MountPoint::Boot, Some(_)) => ESP_BOOT_FREE,
            (MountPoint::Efi, Some(fs)) => ESP_MIN_FREE.max(fs_least(fs, sector)),
            _ => 0,
        };
        if one.size < least {
            return Err(Refusal::TooSmall {
                at: one.at.clone(),
                least,
            });
        }
    }
    // Nothing is mounted inside a kept partition's path: it is not mounted
    // while Linexin is installed, so the folder a mount inside it needs
    // would be made in it at the first start — a write to a partition that
    // is kept — or, where it did not mount (`nofail`), on `/`, and hidden
    // under it once it did.
    for kept in used.iter().filter(|one| one.made.is_none()) {
        if matches!(kept.at, MountPoint::Root | MountPoint::Swap) {
            continue;
        }
        let path = kept.at.path();
        let inside = used.iter().any(|other| {
            let under = other.at.path();
            under != path && at_or_under(under, path)
        });
        if inside {
            return Err(Refusal::InsideKept {
                part: kept.which.clone(),
                at: kept.at.clone(),
            });
        }
    }
    for one in layout {
        let sector = one.disk.logical_sector;
        let formatted = one.parts.iter().filter_map(|part| match &part.fate {
            Fate::Format { fs, size, .. } => Some((Which::Part(part.path.clone()), *fs, *size)),
            _ => None,
        });
        let created = one.create.iter().map(|made| {
            let which = Which::Made {
                disk: one.disk.path.clone(),
                start: made.start,
            };
            (which, made.fs, made.size)
        });
        for (which, fs, size) in formatted.chain(created) {
            let least = fs_least(fs, sector);
            if size < least {
                return Err(Refusal::FsTooSmall {
                    part: which,
                    fs,
                    least,
                });
            }
        }
    }
    if software.snapshots && root.made == Some(Fs::Btrfs) && separate_var(layout) {
        return Err(Refusal::RestorePoints);
    }
    Ok(())
}

/// Whether `layout` mounts a partition of its own at `/var`: restore points
/// are switched off beside one (§7.0, Unit Q, decision 8).
pub fn separate_var(layout: &[DiskLayout]) -> bool {
    uses(layout)
        .iter()
        .any(|one| matches!(one.at, MountPoint::Other(path) if path == "/var"))
}

/// [`shape`]'s fifth section: what starts the computer.
fn boot(layout: &[DiskLayout], firmware: Firmware, bootloader: Bootloader) -> Result<(), Refusal> {
    let used = uses(layout);
    let at = |mount: MountPoint| used.iter().find(|one| *one.at == mount);
    let (efi, boot) = (at(MountPoint::Efi), at(MountPoint::Boot));
    let Some(root) = at(MountPoint::Root) else {
        return Err(Refusal::Root);
    };
    match firmware {
        Firmware::Uefi => match (efi, boot) {
            (None, None) => return Err(Refusal::Efi),
            (Some(_), None) => return Err(Refusal::Boot),
            (Some(efi), Some(boot)) if efi.disk != boot.disk => {
                return Err(Refusal::BootElsewhere {
                    beside: MountPoint::Efi,
                })
            }
            _ => {}
        },
        Firmware::Bios => {
            if efi.is_some() {
                return Err(Refusal::EfiOnBios);
            }
            if boot.is_some_and(|boot| boot.disk != root.disk) {
                return Err(Refusal::BootElsewhere {
                    beside: MountPoint::Root,
                });
            }
            let disk = &layout[root.disk];
            let path = disk.disk.path.clone();
            match table_after(disk) {
                Some(Table::Gpt) => {
                    let kept = disk
                        .parts
                        .iter()
                        .zip(&disk.disk.partitions)
                        .any(|(part, was)| was.content == Content::BiosBoot && part.fate.keeps());
                    let formatted = disk.parts.iter().any(|part| {
                        matches!(
                            part.fate,
                            Fate::Format {
                                fs: Fs::BiosGrub,
                                ..
                            }
                        )
                    });
                    let made = disk.create.iter().any(|made| made.fs == Fs::BiosGrub);
                    if !(kept || formatted || made) {
                        return Err(Refusal::BiosGrubMissing { disk: path });
                    }
                }
                Some(Table::Msdos) => {
                    let first = disk
                        .parts
                        .iter()
                        .filter(|part| part.fate != Fate::Delete)
                        .map(|part| part.start)
                        .chain(disk.create.iter().map(|made| made.start))
                        .min();
                    if first.is_some_and(|first| first < MIB) {
                        return Err(Refusal::GrubGap { disk: path });
                    }
                }
                None => {}
            }
        }
    }
    if !bootloader_possible(bootloader, firmware, layout) {
        return Err(Refusal::Bootloader { loader: bootloader });
    }
    Ok(())
}

/// The rules that need numbers read with privilege, judged from
/// `measures` and `images` (the interface's, read when Partitions opened;
/// the worker's own, read again before it writes). An unknown number is
/// refused, never guessed — but for a kept EFI system partition at `/efi`,
/// which the worker measures again before anything is written and refuses
/// then (`EspTooFull`) as it always has. In order:
///
/// 1. a shrink: of what reading says may be shrunk
///    ([`Measures::shrink_of`] — an NTFS judged across every disk of the
///    measurement), and not below what the filesystem uses and a margin
///    ([`shrink_floor`]) — but for an entry that holds the filesystem as it
///    is, whose filesystem is left as it is: not below the filesystem's own
///    size ([`Shrinkable::least_entry`], [`Shrinkable::entry_only`]);
/// 2. a kept EFI system partition: `ESP_BOOT_FREE` free at `/boot`,
///    `ESP_MIN_FREE` at `/efi`;
/// 3. a partition made or formatted where the image puts files: room for
///    what it puts there, less what mounts inside it take, and a quarter
///    more and a GiB — and 4 GiB more at `/var`, for what is installed and
///    logged after.
pub fn measured(
    layout: &[DiskLayout],
    measures: &Measures,
    images: &ImageSizes,
) -> Result<(), Refusal> {
    for one in layout {
        for (part, was) in one.parts.iter().zip(&one.disk.partitions) {
            let Fate::Shrink { to, .. } = part.fate else {
                continue;
            };
            let which = Which::Part(part.path.clone());
            match measures.shrink_of(was) {
                None => return Err(Refusal::NotRead { part: which }),
                Some(Ok(shrinkable)) => {
                    let least = shrinkable.least_entry();
                    if to < least {
                        return Err(Refusal::ShrinkBelow { part: which, least });
                    }
                }
                Some(Err(why)) => return Err(Refusal::Unshrinkable { part: which, why }),
            }
        }
    }
    let used = uses(layout);
    for one in &used {
        let (None, Some(was)) = (one.made, one.was) else {
            continue;
        };
        let least = match one.at {
            MountPoint::Boot => ESP_BOOT_FREE,
            MountPoint::Efi => ESP_MIN_FREE,
            _ => continue,
        };
        let free = match measures.of(was) {
            Some(Measure::Read { free, .. }) => Some(*free),
            _ => None,
        };
        match free {
            Some(free) if free < least => {
                return Err(Refusal::EspFull {
                    part: one.which.clone(),
                    least,
                })
            }
            None if *one.at == MountPoint::Boot => {
                return Err(Refusal::NotRead {
                    part: one.which.clone(),
                })
            }
            _ => {}
        }
    }
    let filled: Vec<&str> = used
        .iter()
        .filter(|one| one.made.is_some())
        .filter_map(|one| match one.at {
            MountPoint::Other(path) if image_fills(path) => Some(path.as_str()),
            _ => None,
        })
        .collect();
    for one in &used {
        let MountPoint::Other(path) = one.at else {
            continue;
        };
        if one.made.is_none() || !image_fills(path) {
            continue;
        }
        let Some(least) = image_floor(path, &filled, images) else {
            return Err(Refusal::ImageUnknown { at: one.at.clone() });
        };
        if one.size < least {
            return Err(Refusal::TooSmall {
                at: one.at.clone(),
                least,
            });
        }
    }
    Ok(())
}

/// The least a partition mounted at `path`, which the image fills, may be:
/// what the image puts there less what the mounts among `mounted` inside it
/// take (the outermost of them; one whose size is unknown is taken as
/// nothing, which only asks for more), a quarter more and a GiB, and 4 GiB
/// more at `/var`. `None` when what the image puts there is not known.
pub fn image_floor(path: &str, mounted: &[&str], images: &ImageSizes) -> Option<u64> {
    let whole = *images.paths.get(path)?;
    let inside: Vec<&str> = mounted
        .iter()
        .copied()
        .filter(|other| *other != path && at_or_under(other, path))
        .collect();
    let outermost = inside.iter().filter(|other| {
        !inside
            .iter()
            .any(|outer| outer != *other && at_or_under(other, outer))
    });
    let taken: u64 = outermost.filter_map(|other| images.paths.get(*other)).sum();
    let base = whole.saturating_sub(taken);
    let mut floor = (base * 5).div_ceil(4) + GIB;
    if path == "/var" {
        floor += 4 * GIB;
    }
    Some(floor.div_ceil(MIB) * MIB)
}

/// Whether `choice` can start Linexin from `layout` on a machine started by
/// `firmware`: judged by the table the EFI system partition's disk is left
/// with (on BIOS, `/`'s), as Where to install judges by its disk's. With
/// neither in it yet, by the firmware alone.
pub fn bootloader_possible(choice: Bootloader, firmware: Firmware, layout: &[DiskLayout]) -> bool {
    let used = uses(layout);
    let at = |mount: MountPoint| used.iter().find(|one| *one.at == mount);
    let starts = match firmware {
        Firmware::Uefi => at(MountPoint::Efi).or_else(|| at(MountPoint::Boot)),
        Firmware::Bios => at(MountPoint::Root),
    };
    match starts.and_then(|one| table_after(&layout[one.disk])) {
        Some(table) => disks::bootloader_possible(choice, firmware, table),
        None => match firmware {
            Firmware::Uefi => true,
            Firmware::Bios => matches!(choice, Bootloader::Automatic | Bootloader::Grub),
        },
    }
}

// -------------------------------------------------------------------------
// Partition types.

/// What a partition used at `/efi` or `/boot` is for, which decides its
/// type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The EFI system partition: at `/efi`, or alone at `/boot`, on UEFI.
    Esp,
    /// The Linux extended boot partition (XBOOTLDR): a `/boot` beside an
    /// EFI system partition at `/efi`, on UEFI.
    LinuxBoot,
    /// Anything else.
    Plain,
}

/// The role of a partition used at `at`, on a machine started by
/// `firmware`, in a layout that does (`efi_used`) or does not use `/efi`.
pub fn role(firmware: Firmware, at: Option<&MountPoint>, efi_used: bool) -> Role {
    match (firmware, at) {
        (Firmware::Uefi, Some(MountPoint::Efi)) => Role::Esp,
        (Firmware::Uefi, Some(MountPoint::Boot)) if efi_used => Role::LinuxBoot,
        (Firmware::Uefi, Some(MountPoint::Boot)) => Role::Esp,
        _ => Role::Plain,
    }
}

/// The type a partition made or formatted as `fs`, used at `at` in `role`,
/// is given on a `table` — as lsblk reports types (`part_type`'s
/// spelling). Partition types never travel in a layout: both halves compute
/// them here. A kept partition is never retyped.
///
/// `bios_grub` its own; the EFI system partition and XBOOTLDR theirs; swap
/// (wherever it is used, or nowhere) Linux swap; `/` Linux root (x86-64)
/// and `/home` Linux home on gpt, Linux on msdos; FAT32 elsewhere on msdos
/// the FAT32 LBA type; everything else Linux.
pub fn part_type(table: Table, fs: Fs, at: Option<&MountPoint>, role: Role) -> &'static str {
    let gpt = table == Table::Gpt;
    let pick = |gpt_type, mbr_type| if gpt { gpt_type } else { mbr_type };
    match (fs, role, at) {
        (Fs::BiosGrub, _, _) => pick(part_type::BIOS_BOOT, part_type::MBR_LINUX),
        (_, Role::Esp, _) => pick(part_type::ESP, part_type::MBR_ESP),
        (_, Role::LinuxBoot, _) => pick(part_type::XBOOTLDR, part_type::MBR_XBOOTLDR),
        (Fs::LinuxSwap, _, _) => pick(part_type::LINUX_SWAP, part_type::MBR_SWAP),
        (_, _, Some(MountPoint::Root)) => pick(part_type::LINUX_ROOT_X86_64, part_type::MBR_LINUX),
        (_, _, Some(MountPoint::Home)) => pick(part_type::LINUX_HOME, part_type::MBR_LINUX),
        (Fs::Fat32, _, _) => pick(part_type::LINUX, part_type::MBR_FAT32_LBA),
        _ => pick(part_type::LINUX, part_type::MBR_LINUX),
    }
}

/// Whether `layout` uses a partition at `/efi`.
pub fn efi_used(layout: &[DiskLayout]) -> bool {
    uses(layout).iter().any(|one| *one.at == MountPoint::Efi)
}

// -------------------------------------------------------------------------
// What a layout destroys: the consent.

/// One thing a layout destroys, named by everything that says which it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destroy {
    /// A new table written over `disk`, erasing every partition on it
    /// (`erases`, by number).
    Table {
        disk: String,
        table: Table,
        erases: Vec<String>,
    },
    /// A partition deleted; `fs` what it holds (lsblk's name, `-` for
    /// none).
    Delete {
        part: String,
        number: u32,
        start: u64,
        size: u64,
        fs: String,
    },
    /// A filesystem shrunk from `from` bytes to `to`.
    Shrink {
        part: String,
        number: u32,
        start: u64,
        from: u64,
        to: u64,
        fs: String,
    },
    /// A partition formatted as `fs`, its entry `to` bytes long (from
    /// `size`).
    Format {
        part: String,
        number: u32,
        start: u64,
        size: u64,
        to: u64,
        fs: Fs,
    },
}

impl Destroy {
    /// The line the consent carries for it, compared whole by the worker:
    /// `table /dev/sdb gpt erases /dev/sdb1 /dev/sdb2`, `delete /dev/sda4 4
    /// start=… size=… ntfs`, `shrink /dev/sda3 3 start=… from=… to=…
    /// ntfs`, `format /dev/sda5 5 start=… size=… to=… ext4`, sizes in bytes.
    pub fn line(&self) -> String {
        match self {
            Destroy::Table {
                disk,
                table,
                erases,
            } => {
                let name = match table {
                    Table::Gpt => "gpt",
                    Table::Msdos => "msdos",
                };
                if erases.is_empty() {
                    format!("table {disk} {name}")
                } else {
                    format!("table {disk} {name} erases {}", erases.join(" "))
                }
            }
            Destroy::Delete {
                part,
                number,
                start,
                size,
                fs,
            } => format!("delete {part} {number} start={start} size={size} {fs}"),
            Destroy::Shrink {
                part,
                number,
                start,
                from,
                to,
                fs,
            } => format!("shrink {part} {number} start={start} from={from} to={to} {fs}"),
            Destroy::Format {
                part,
                number,
                start,
                size,
                to,
                fs,
            } => format!(
                "format {part} {number} start={start} size={size} to={to} {}",
                fs.name()
            ),
        }
    }
}

/// Everything `layout` destroys, in a fixed order: its disks by path; on
/// each, a new table first (which names every partition it erases, and is
/// then all there is), then the partitions reviewed, by number. What it
/// makes in unallocated space, keeps, mounts or labels destroys nothing; a
/// kept partition never grows, and a label lives only inside a format.
pub fn destroys(layout: &[DiskLayout]) -> Vec<Destroy> {
    let mut disks: Vec<&DiskLayout> = layout.iter().collect();
    disks.sort_by(|one, other| one.disk.path.cmp(&other.disk.path));
    let mut destroyed = Vec::new();
    for one in disks {
        let mut parts: Vec<(&Part, &Partition)> =
            one.parts.iter().zip(&one.disk.partitions).collect();
        parts.sort_by_key(|(part, _)| part.number);
        if let Some(table) = one.table {
            destroyed.push(Destroy::Table {
                disk: one.disk.path.clone(),
                table,
                erases: parts.iter().map(|(part, _)| part.path.clone()).collect(),
            });
            continue;
        }
        for (part, was) in parts {
            let fs = if was.fs_type.is_empty() {
                "-".to_string()
            } else {
                was.fs_type.clone()
            };
            let (path, number, start, size) =
                (part.path.clone(), part.number, part.start, part.size);
            destroyed.push(match &part.fate {
                Fate::Keep { .. } => continue,
                Fate::Delete => Destroy::Delete {
                    part: path,
                    number,
                    start,
                    size,
                    fs,
                },
                Fate::Shrink { to, .. } => Destroy::Shrink {
                    part: path,
                    number,
                    start,
                    from: size,
                    to: *to,
                    fs,
                },
                Fate::Format {
                    fs: made, size: to, ..
                } => Destroy::Format {
                    part: path,
                    number,
                    start,
                    size,
                    to: *to,
                    fs: *made,
                },
            });
        }
    }
    destroyed
}

// -------------------------------------------------------------------------
// What a layout needs proven.

/// What a hand-made layout asks of the worker: a kind of work, and the
/// kind of machine and of partition table it asks it on. Every layout
/// needs its firmware and the kind of every table it writes or keeps
/// ([`needs`]), and both halves carry it out only where everything it
/// needs lies within one set of [`PROVEN`] — a mix one QEMU scenario that
/// passed ran whole ([`unproven`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    /// A new partition table written over a disk (`wipefs`, then one
    /// script with every partition made in it).
    FreshTable,
    /// Partitions made on a disk that keeps its table (`sfdisk --append`).
    KeepTable,
    /// Partitions deleted from a disk that keeps its table (`sfdisk
    /// --delete`): another step than making them, so a scenario that only
    /// made partitions on a kept table opens no deletion there, nor one
    /// that only deleted any making — nor both together, unless one ran
    /// both. A primary partition, a GPT one or an extended frame: a
    /// logical partition deleted is [`DeleteLogical`].
    ///
    /// [`DeleteLogical`]: Capability::DeleteLogical
    DeleteEntries,
    /// A logical partition of a kept msdos table deleted ([`Logical`]
    /// too): libfdisk unlinks it from the table chained before it, where a
    /// primary partition's entry is cleared in the disk's own table — so a
    /// scenario that deleted only a logical partition opens no primary
    /// deleted, nor one that deleted only a primary any logical one.
    ///
    /// [`Logical`]: Capability::Logical
    DeleteLogical,
    /// An existing partition formatted where it stands: retyped (and
    /// renamed, on GPT) and made anew.
    ChangeEntries,
    /// A partition formatted with its entry made smaller (`sfdisk -N`,
    /// before anything is deleted or made) — [`ChangeEntries`] too.
    ///
    /// [`ChangeEntries`]: Capability::ChangeEntries
    SmallerEntry,
    /// A partition formatted with its entry grown into what is free after
    /// it (`sfdisk -N`, once everything is deleted and made) —
    /// [`ChangeEntries`] too. A primary partition or a GPT one: a logical
    /// partition grown is [`LargerLogical`].
    ///
    /// [`ChangeEntries`]: Capability::ChangeEntries
    /// [`LargerLogical`]: Capability::LargerLogical
    LargerEntry,
    /// A logical partition of a kept msdos table formatted with its entry
    /// grown ([`ChangeEntries`] and [`Logical`] too): its size written in
    /// the table chained before it, inside the extended frame, where a
    /// primary partition's is written in the disk's own table.
    ///
    /// [`ChangeEntries`]: Capability::ChangeEntries
    /// [`Logical`]: Capability::Logical
    LargerLogical,
    /// A kept partition used as `/home`.
    KeptHome,
    /// More than one disk in it.
    SeveralDisks,
    /// A machine UEFI starts.
    Uefi,
    /// A machine a BIOS starts.
    Bios,
    /// A GPT partition table on a disk of the layout: a new one, one kept
    /// whose entries change, or one only mounted from.
    Gpt,
    /// An msdos partition table on a disk of the layout, as [`Gpt`] is
    /// one: what is proven on GPT does not open an msdos disk, nor what is
    /// proven of msdos under a BIOS a machine UEFI starts.
    ///
    /// [`Gpt`]: Capability::Gpt
    Msdos,
    /// A logical partition of a kept msdos table written to — deleted,
    /// formatted or its filesystem shrunk — or its extended frame deleted:
    /// entries chained in the extended partition's own tables, which the
    /// kernel numbers anew as one goes, never a primary partition's. A
    /// logical partition only mounted from asks nothing of it. A deletion
    /// and an entry grown have a logical capability of their own
    /// ([`DeleteLogical`], [`LargerLogical`]); a format in place, an entry
    /// made smaller and a shrink ask their own capability beside this one,
    /// which pairs only what the one set holding it ran on both kinds of
    /// partition (the logical scenario formats a primary partition and a
    /// logical one).
    ///
    /// [`DeleteLogical`]: Capability::DeleteLogical
    /// [`LargerLogical`]: Capability::LargerLogical
    Logical,
    /// The first logical partition of a kept msdos table deleted, and so
    /// every one, the logical partitions going from the last: the frame
    /// left empty ([`DeleteLogical`] and [`Logical`] too). libfdisk deletes
    /// it by another branch than a later one's ("delete logical [move
    /// down]", where every later one is "[last in the chain]"), clearing
    /// the table that stands where the frame begins — which deleting the
    /// last logical partitions of several never runs.
    ///
    /// [`DeleteLogical`]: Capability::DeleteLogical
    /// [`Logical`]: Capability::Logical
    EmptyFrame,
    /// The extended frame of a kept msdos table deleted, with every logical
    /// partition in it ([`Logical`] and [`DeleteEntries`] too): the one
    /// write the rule set lets an extended partition take, which deleting a
    /// logical partition does not run.
    ///
    /// [`Logical`]: Capability::Logical
    /// [`DeleteEntries`]: Capability::DeleteEntries
    Extended,
    /// `/` or `/boot` on a logical partition of a kept msdos table, made
    /// anew there or a `/boot` kept there: every scenario's `/` is a
    /// partition made or a primary one, and none keeps a `/boot` on msdos.
    /// The boot flag goes only on a primary partition
    /// (`manual::plan::activation`), so it then stands on `/` alone, or on
    /// nothing that starts Linexin.
    RootOnLogical,
    /// The boot flag set on a kept msdos table (phase B's step 8, `sfdisk
    /// --activate`), where no partition left standing is marked active as
    /// the disk was reviewed ([`activates`]): a write to the disk's own
    /// table no other step makes, which a scenario that found Windows
    /// marked — and so set nothing — never ran.
    ///
    /// [`activates`]: activates
    Activate,
    /// A `/boot` of its own on a machine a BIOS starts — made, formatted or
    /// a kept EFI system partition there — beside `/` on its disk: GRUB's
    /// core then reads its modules, `grub.cfg` and the kernels from a FAT32
    /// partition, and on msdos the boot flag goes on it, not on `/`
    /// ([`activates`]). Every BIOS scenario keeps them on `/`.
    ///
    /// [`activates`]: activates
    BootOnBios,
    /// A disk of the layout whose logical sector is not 512 bytes — 4096,
    /// or another size [`sector_fits`] takes — written to or only mounted
    /// from, its table kept: its table, the `-N` scripts, every partition's
    /// place and FAT32's least are reckoned in its sectors. QEMU gave every
    /// scenario's disks 512-byte sectors, so none ran one.
    LargeSectors,
    /// A kept ext4 filesystem shrunk (`e2fsck`, `resize2fs`), then its
    /// entry.
    ShrinkExt4,
    /// A kept NTFS shrunk (Windows' sign of hibernation read again,
    /// `ntfsresize`), then its entry: another tool and other checks than
    /// an ext4's, so neither shrink proves the other.
    ShrinkNtfs,
}

/// Every capability, in [`Capability`]'s order. Test builds only.
#[cfg(test)]
pub(crate) const EVERY_CAPABILITY: &[Capability] = &[
    Capability::FreshTable,
    Capability::KeepTable,
    Capability::DeleteEntries,
    Capability::DeleteLogical,
    Capability::ChangeEntries,
    Capability::SmallerEntry,
    Capability::LargerEntry,
    Capability::LargerLogical,
    Capability::KeptHome,
    Capability::SeveralDisks,
    Capability::Uefi,
    Capability::Bios,
    Capability::Gpt,
    Capability::Msdos,
    Capability::Logical,
    Capability::EmptyFrame,
    Capability::Extended,
    Capability::RootOnLogical,
    Capability::Activate,
    Capability::BootOnBios,
    Capability::LargeSectors,
    Capability::ShrinkExt4,
    Capability::ShrinkNtfs,
];

/// What QEMU has proven: for each scenario of the QEMU campaign (Unit Q)
/// that passed, the exact set of capabilities its plan needed ([`needs`]),
/// in [`Capability`]'s order. A layout is carried out only where everything
/// it needs lies within one of them ([`unproven`]): only a mix a scenario
/// ran whole, the firmware and the kinds of table counted too — never
/// capabilities each proven, but apart (decision 20). Filled with what
/// passed: the set of each scenario that passed, as the campaign's table
/// names it for that scenario (`CAMPAIGN`, test builds), and nothing for
/// one skipped or failed.
///
/// Every one of the campaign's eleven scenarios that carry a plan out
/// passed in QEMU, and every refusal its step names, so it holds the ten
/// sets of the campaign's table (the two scenarios writing a new gpt under
/// UEFI need the same one), in the table's order.
pub const PROVEN: &[&[Capability]] = {
    use Capability::*;
    &[
        // manual-uefi-empty and manual-uefi-new-table-over-used.
        &[FreshTable, Uefi, Gpt],
        // manual-uefi-beside-windows.
        &[
            KeepTable,
            DeleteEntries,
            ChangeEntries,
            SmallerEntry,
            Uefi,
            Gpt,
        ],
        // manual-uefi-three-disks.
        &[FreshTable, KeptHome, SeveralDisks, Uefi, Gpt],
        // manual-bios-gpt.
        &[FreshTable, Bios, Gpt],
        // manual-bios-msdos-keep-fat.
        &[KeepTable, Bios, Msdos, Activate],
        // manual-bios-msdos-logical.
        &[
            DeleteLogical,
            ChangeEntries,
            LargerLogical,
            Bios,
            Msdos,
            Logical,
            Activate,
        ],
        // manual-uefi-shrink-ntfs.
        &[KeepTable, Uefi, Gpt, ShrinkNtfs],
        // manual-uefi-shrink-ext4.
        &[FreshTable, KeepTable, SeveralDisks, Uefi, Gpt, ShrinkExt4],
        // manual-uefi-reinstall-keep-home.
        &[ChangeEntries, KeptHome, Uefi, Gpt],
        // manual-bios-msdos-windows-keep-home.
        &[KeepTable, ChangeEntries, KeptHome, Bios, Msdos, ShrinkNtfs],
    ]
};

/// Each scenario of the QEMU campaign (Unit Q) that carries a plan out, with
/// the exact set its plan needs — `needs` of the layout its walk leaves, as
/// `tests/e2e/scenarios.py` says it at the scenario's head and in its `needs`
/// (`test_unit_q.py` holds the two to this table, and the campaign's replay
/// tests hold the walk's layout to it). [`PROVEN`] holds the set of each that
/// passed, which is every one of them. Test builds only.
#[cfg(test)]
pub(crate) const CAMPAIGN: &[(&str, &[Capability])] = {
    use Capability::*;
    &[
        ("manual-uefi-empty", &[FreshTable, Uefi, Gpt]),
        ("manual-uefi-new-table-over-used", &[FreshTable, Uefi, Gpt]),
        (
            "manual-uefi-beside-windows",
            &[
                KeepTable,
                DeleteEntries,
                ChangeEntries,
                SmallerEntry,
                Uefi,
                Gpt,
            ],
        ),
        (
            "manual-uefi-three-disks",
            &[FreshTable, KeptHome, SeveralDisks, Uefi, Gpt],
        ),
        ("manual-bios-gpt", &[FreshTable, Bios, Gpt]),
        (
            "manual-bios-msdos-keep-fat",
            &[KeepTable, Bios, Msdos, Activate],
        ),
        (
            "manual-bios-msdos-logical",
            &[
                DeleteLogical,
                ChangeEntries,
                LargerLogical,
                Bios,
                Msdos,
                Logical,
                Activate,
            ],
        ),
        (
            "manual-uefi-shrink-ntfs",
            &[KeepTable, Uefi, Gpt, ShrinkNtfs],
        ),
        (
            "manual-uefi-shrink-ext4",
            &[FreshTable, KeepTable, SeveralDisks, Uefi, Gpt, ShrinkExt4],
        ),
        // The two scenarios decision 20 added.
        (
            "manual-uefi-reinstall-keep-home",
            &[ChangeEntries, KeptHome, Uefi, Gpt],
        ),
        (
            "manual-bios-msdos-windows-keep-home",
            &[KeepTable, ChangeEntries, KeptHome, Bios, Msdos, ShrinkNtfs],
        ),
    ]
};

/// The sets a layout is judged by: [`PROVEN`] — and, in a test build only,
/// the sets a test pretends are proven on its own thread instead
/// ([`PretendProven`]), so that a test can judge a layout by fewer sets
/// than passed (none, as were none written) or by a set no scenario ran.
/// There is no switch, variable, argument, file or feature that changes
/// what a build that runs judges by.
fn proven_sets() -> &'static [&'static [Capability]] {
    #[cfg(test)]
    if let Some(pretended) = PRETENDED_PROVEN.get() {
        return pretended;
    }
    PROVEN
}

#[cfg(test)]
thread_local! {
    /// What this thread's test pretends is proven (`PretendProven`).
    static PRETENDED_PROVEN: std::cell::Cell<Option<&'static [&'static [Capability]]>> =
        const { std::cell::Cell::new(None) };
}

/// While it lives, [`unproven`] judges on this thread by the sets given
/// rather than by [`PROVEN`]: how a test judges a layout by other sets
/// than those that passed — none, or one scenario's alone, or one no
/// scenario ran. Dropped, it gives back whatever the thread pretended
/// before. Test builds only.
#[cfg(test)]
pub(crate) struct PretendProven(Option<&'static [&'static [Capability]]>);

#[cfg(test)]
impl PretendProven {
    /// `sets` pretended proven: each the needs of a scenario that passed.
    pub(crate) fn these(sets: &'static [&'static [Capability]]) -> PretendProven {
        PretendProven(PRETENDED_PROVEN.replace(Some(sets)))
    }
}

#[cfg(test)]
impl Drop for PretendProven {
    fn drop(&mut self) {
        PRETENDED_PROVEN.set(self.0);
    }
}

/// The kept partitions `layout` shrinks the filesystem of, as they were
/// reviewed, disk by disk as the layout lists them: what the worker's
/// Resize step shrinks first (step Q9). An NTFS among them is Windows',
/// which Windows checks the next time it starts — what Done then says
/// (`Warning::WindowsWillCheck`).
pub fn shrunk(layout: &[DiskLayout]) -> Vec<&Partition> {
    layout
        .iter()
        .flat_map(|one| one.parts.iter().zip(&one.disk.partitions))
        .filter(|(part, _)| matches!(part.fate, Fate::Shrink { .. }))
        .map(|(_, was)| was)
        .collect()
}

/// Whether `layout` writes to its disk at all: a new table, a partition
/// made, or any reviewed partition not simply kept. A disk only mounted
/// from is never written, and no destructive command is ever given it.
pub fn writes_to(layout: &DiskLayout) -> bool {
    layout.table.is_some()
        || !layout.create.is_empty()
        || layout
            .parts
            .iter()
            .any(|part| !matches!(part.fate, Fate::Keep { .. }))
}

/// Whether the plan's last step on `one`'s disk sets the boot flag (phase
/// B, step 8, `sfdisk --activate`; `manual::plan::activation`, which decides
/// by the table read once steps 1 to 7 are done): on a kept msdos table
/// where no partition left standing is marked active — judged from the
/// flags the disk was reviewed with (`Partition::bootable`), which phase A
/// holds the disk to, a partition deleted taking its mark with it and no
/// other step changing one — on the first of a `/boot` there and `/` that
/// is a primary partition: one made, which on msdos is always a primary
/// one ([`can_create`]), or one numbered 1 to 4. Never where `/` is not on
/// that disk, nor under a new table, whose own script marks it.
pub fn activates(one: &DiskLayout) -> bool {
    if one.table.is_some() || one.disk.table != Some(Table::Msdos) {
        return false;
    }
    let reviewed = || one.parts.iter().zip(&one.disk.partitions);
    if reviewed().any(|(part, was)| was.bootable && part.fate != Fate::Delete) {
        return false;
    }
    // Whether what stands at `at` on this disk, if anything, is a primary
    // partition: a kept one first, as the flag's candidates are offered.
    let primary_at = |at: &MountPoint| {
        let kept = reviewed()
            .find(|(part, _)| part.fate.mount() == Some(at))
            .map(|(_, was)| !logical(one.disk.table, was));
        let made = one
            .create
            .iter()
            .any(|made| made.mount.as_ref() == Some(at));
        kept.or(made.then_some(true))
    };
    let Some(root) = primary_at(&MountPoint::Root) else {
        return false;
    };
    primary_at(&MountPoint::Boot) == Some(true) || root
}

/// Every capability `layout` needs on a machine started by `firmware`, in
/// [`Capability`]'s order, each once: always its firmware, and the kind of
/// every table its disks have once it is carried out — a new one, one kept
/// and written to, and one only mounted from alike — and of every table a
/// new one is written over; on a kept table, partitions made and deleted
/// apart, and a formatted partition's entry made smaller or grown apart
/// from its format; on a kept msdos table, each deletion and each entry
/// grown by the kind of partition it is done to — a logical one's
/// ([`DeleteLogical`](Capability::DeleteLogical),
/// [`LargerLogical`](Capability::LargerLogical)) apart from a primary
/// one's — the first logical partition deleted
/// ([`EmptyFrame`](Capability::EmptyFrame)) apart from a later one,
/// [`Logical`](Capability::Logical) for a logical partition written to or
/// an extended frame deleted, [`Extended`](Capability::Extended) for the
/// second, and [`RootOnLogical`](Capability::RootOnLogical) for `/` or
/// `/boot` on a logical partition; [`Activate`](Capability::Activate)
/// where it sets the boot flag ([`activates`]);
/// [`BootOnBios`](Capability::BootOnBios) for a `/boot` on a machine a
/// BIOS starts; [`LargeSectors`](Capability::LargeSectors) for a disk
/// whose logical sector is not 512 bytes, written to or only mounted from;
/// and a shrink by the filesystem it shrinks — one neither tool shrinks,
/// which the rule set refuses before this is asked, both, as no scenario
/// ran.
pub fn needs(layout: &[DiskLayout], firmware: Firmware) -> Vec<Capability> {
    let mut needed = BTreeSet::new();
    if layout.len() > 1 {
        needed.insert(Capability::SeveralDisks);
    }
    // A disk whose logical sector is not 512 bytes, whatever is done on it
    // — a disk only mounted from keeps its table, as the kinds of table
    // count it: every scenario's disks had 512-byte sectors.
    if layout.iter().any(|one| one.disk.logical_sector != 512) {
        needed.insert(Capability::LargeSectors);
    }
    needed.insert(match firmware {
        Firmware::Uefi => Capability::Uefi,
        Firmware::Bios => Capability::Bios,
    });
    // A `/boot` under BIOS, whatever it is made of or kept as, which no
    // scenario ran: under UEFI every scenario has one, and under BIOS none.
    if firmware == Firmware::Bios && uses(layout).iter().any(|one| *one.at == MountPoint::Boot) {
        needed.insert(Capability::BootOnBios);
    }
    for one in layout {
        // The table it has once the plan is carried out and, under a new
        // one, the table written over: a new GPT over an msdos disk reads,
        // checks and erases an msdos table.
        let over = one.table.and(one.disk.table);
        for table in [table_after(one), over].into_iter().flatten() {
            needed.insert(match table {
                Table::Gpt => Capability::Gpt,
                Table::Msdos => Capability::Msdos,
            });
        }
        let kept_table = one.table.is_none();
        if !kept_table {
            needed.insert(Capability::FreshTable);
        } else if !one.create.is_empty() {
            needed.insert(Capability::KeepTable);
        }
        if activates(one) {
            needed.insert(Capability::Activate);
        }
        for (part, was) in one.parts.iter().zip(&one.disk.partitions) {
            // A logical partition of a kept msdos table: a deletion and an
            // entry grown there are other steps of libfdisk's than on a
            // primary partition — the entry is in the table chained before
            // it, inside the frame — and counted apart. Under a new table
            // everything goes at once, and nothing is a logical one's.
            let in_chain = kept_table && logical(one.disk.table, was);
            match &part.fate {
                Fate::Delete if in_chain => {
                    needed.insert(Capability::DeleteLogical);
                    // The first of the chain, numbered 5 (the kernel numbers
                    // logical partitions from 5 in the order their tables
                    // are chained), which goes only once every later one
                    // has: the frame left empty.
                    if was.number == 5 {
                        needed.insert(Capability::EmptyFrame);
                    }
                }
                Fate::Delete if kept_table => {
                    needed.insert(Capability::DeleteEntries);
                }
                Fate::Format { size, .. } => {
                    needed.insert(Capability::ChangeEntries);
                    if *size < part.size {
                        needed.insert(Capability::SmallerEntry);
                    } else if *size > part.size {
                        needed.insert(if in_chain {
                            Capability::LargerLogical
                        } else {
                            Capability::LargerEntry
                        });
                    }
                }
                Fate::Shrink { .. } => match was.fs_type.as_str() {
                    "ext4" => {
                        needed.insert(Capability::ShrinkExt4);
                    }
                    "ntfs" => {
                        needed.insert(Capability::ShrinkNtfs);
                    }
                    _ => {
                        needed.insert(Capability::ShrinkExt4);
                        needed.insert(Capability::ShrinkNtfs);
                    }
                },
                Fate::Keep { .. } | Fate::Delete => {}
            }
            if part.fate.keeps() && part.fate.mount() == Some(&MountPoint::Home) {
                needed.insert(Capability::KeptHome);
            }
            // `/` or `/boot` on a logical partition of a kept table, which
            // no scenario ran.
            if in_chain && matches!(part.fate.mount(), Some(MountPoint::Root | MountPoint::Boot)) {
                needed.insert(Capability::RootOnLogical);
            }
            // A logical partition, or its extended frame, written to on a
            // kept table: under a new one everything is gone at once.
            let written = !matches!(part.fate, Fate::Keep { .. });
            let chained = logical(one.disk.table, was) || was.is_extended();
            if kept_table && written && chained {
                needed.insert(Capability::Logical);
                if was.is_extended() {
                    needed.insert(Capability::Extended);
                }
            }
        }
    }
    needed.into_iter().collect()
}

/// Why a layout is not carried out by this build ([`unproven`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unproven {
    /// A capability it needs that no set of [`PROVEN`] holds at all: the
    /// first, in [`Capability`]'s order.
    Never(Capability),
    /// Everything it needs, in [`Capability`]'s order: each held by some
    /// set of [`PROVEN`], but no one set holds them all — a mix no scenario
    /// ran whole.
    Together(Vec<Capability>),
}

impl Unproven {
    /// What the worker says of it where it refuses the layout.
    pub fn line(&self) -> String {
        match self {
            Unproven::Never(capability) => {
                format!("the layout needs {capability:?}, which this build has not proven")
            }
            Unproven::Together(needed) => format!(
                "the layout needs {needed:?} together, which no one scenario this build has proven needed"
            ),
        }
    }
}

/// `None` where everything `layout` needs on a machine started by
/// `firmware` lies within one set of [`PROVEN`]; otherwise why not — the
/// first capability no set holds, or, each held by one, the mix none holds
/// whole. Were no set proven, every layout would be refused.
pub fn unproven(layout: &[DiskLayout], firmware: Firmware) -> Option<Unproven> {
    let needed = needs(layout, firmware);
    let sets = proven_sets();
    if sets
        .iter()
        .any(|set| needed.iter().all(|one| set.contains(one)))
    {
        return None;
    }
    let never = needed
        .iter()
        .copied()
        .find(|one| !sets.iter().any(|set| set.contains(one)));
    Some(match never {
        Some(capability) => Unproven::Never(capability),
        None => Unproven::Together(needed),
    })
}

// -------------------------------------------------------------------------
// The easy fields of a request that carries a layout.

/// Whether `layout` keeps a partition as it is — or only shrinks it — and
/// mounts it at `/home`: the person's files from before, which
/// `InstallConfig::home_kept` keeps the live account's removal and the new
/// account's `chown -R` out of.
pub fn keeps_home(layout: &[DiskLayout]) -> bool {
    layout
        .iter()
        .flat_map(|one| &one.parts)
        .any(|part| part.fate.keeps() && part.fate.mount() == Some(&MountPoint::Home))
}

/// The fields of a request's easy path that a layout decides (§7.0, Unit
/// Q, "The request" 2), so that everything that reads them — the restore
/// points, the network check, the tools an installation needs, the swap
/// file's step, Desktop's restore points and start-up managers — reads the
/// layout's answer without knowing layouts. `storage.home` is always
/// `None`: a `/home` is the layout's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pins {
    /// `InstallConfig::disk`: the disk that holds `/`, as it was reviewed.
    pub disk: Disk,
    /// `Storage::filesystem`: what `/` is made with, ext4 or Btrfs.
    pub filesystem: Filesystem,
    /// `Storage::swap_mib`: none where the layout uses a swap partition,
    /// else the swap file asked for.
    pub swap_mib: u32,
}

/// Why a request's easy fields are not its layout's pins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unpinned {
    /// It carries no layout, or nothing made as ext4 or Btrfs at `/`.
    Root,
    /// `disk` is not the disk that holds `/`, exactly as it was reviewed.
    Disk,
    /// A separate `/home` of the easy path's beside the layout.
    Home,
    /// `storage.filesystem` is not what `/` is made with.
    Filesystem,
    /// A swap file beside a swap partition.
    Swap,
    /// A swap file larger than a quarter of `/`, or than `MAX_SWAP`: the
    /// easy path's bound (`disks::validate_storage`), with `/` the room.
    SwapTooLarge,
}

/// What `/` is used as in `layout`: where it is (by its disk's place in
/// the layout), its size once the layout is carried out, and the
/// filesystem it is made with, as the request's easy field says it.
fn root_of(layout: &[DiskLayout]) -> Result<(usize, u64, Filesystem), Refusal> {
    let used = uses(layout);
    let root = used
        .iter()
        .find(|one| *one.at == MountPoint::Root)
        .ok_or(Refusal::Root)?;
    let filesystem = match root.made {
        Some(Fs::Ext4) => Filesystem::Ext4,
        Some(Fs::Btrfs) => Filesystem::Btrfs,
        _ => {
            return Err(Refusal::Needs {
                at: MountPoint::Root,
            })
        }
    };
    Ok((root.disk, root.size, filesystem))
}

/// Whether `layout` uses a partition as swap: the swap file is then none.
fn swap_used(layout: &[DiskLayout]) -> bool {
    uses(layout).iter().any(|one| *one.at == MountPoint::Swap)
}

/// The pins of `layout`, the swap file asked for being `swap_file_mib`
/// (Options' on Partitions, which is Where to install's): what the
/// interface puts in a request's easy fields. Refused as [`shape`] refuses
/// it where nothing is used at `/` (`Root`), or what is there is not made
/// as ext4 or Btrfs (`Needs`). The swap file's bound is [`pinned`]'s to
/// judge, as it is the worker's.
pub fn pins(layout: &[DiskLayout], swap_file_mib: u32) -> Result<Pins, Refusal> {
    let (disk, _, filesystem) = root_of(layout)?;
    Ok(Pins {
        disk: layout[disk].disk.clone(),
        filesystem,
        swap_mib: if swap_used(layout) { 0 } else { swap_file_mib },
    })
}

/// **Whether a request's easy fields are its layout's pins**, judged by
/// both halves — the interface before it asks the last question, the
/// worker before it reads anything else of the layout: `disk` the disk
/// that holds `/`, exactly as reviewed; no separate `/home`; `filesystem`
/// `/`'s, ext4 or Btrfs; no swap file beside a swap partition, and
/// otherwise one no larger than a quarter of `/` and `MAX_SWAP`. The first
/// that does not hold, in that order; `Root` for a request that carries no
/// layout at all.
pub fn pinned(config: &InstallConfig) -> Result<(), Unpinned> {
    let Target::Layout { disks: layout } = &config.target else {
        return Err(Unpinned::Root);
    };
    let (disk, root, filesystem) = root_of(layout).map_err(|_| Unpinned::Root)?;
    if config.disk != layout[disk].disk {
        return Err(Unpinned::Disk);
    }
    if config.storage.home.is_some() {
        return Err(Unpinned::Home);
    }
    if config.storage.filesystem != filesystem {
        return Err(Unpinned::Filesystem);
    }
    let swap = u64::from(config.storage.swap_mib) * MIB;
    if swap > 0 && swap_used(layout) {
        return Err(Unpinned::Swap);
    }
    if swap > (root / 4).min(MAX_SWAP) {
        return Err(Unpinned::SwapTooLarge);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::{classify, Transport};

    // ---------------------------------------------------------------------
    // Disks written by hand, in the shape discovery reports them. Nothing
    // here is read from the machine the tests run on.

    fn part(
        disk: &str,
        number: u32,
        start_mib: u64,
        size_mib: u64,
        fs: &str,
        ty: &str,
    ) -> Partition {
        Partition {
            path: format!("{disk}{number}"),
            number,
            start: start_mib * MIB,
            size: size_mib * MIB,
            fs_type: fs.into(),
            label: String::new(),
            part_type: ty.into(),
            uuid: format!("UUID-{number}"),
            mountpoints: Vec::new(),
            content: classify(ty, fs),
            bootable: false,
        }
    }

    fn disk(path: &str, table: Option<Table>, size_gib: u64, partitions: Vec<Partition>) -> Disk {
        Disk {
            path: path.into(),
            diskseq: Some(7),
            model: "Example Disk".into(),
            serial: "TEST-0001".into(),
            size: size_gib * GIB,
            major_minor: "8:0".into(),
            logical_sector: 512,
            transport: Transport::Sata,
            removable: false,
            rotational: false,
            table,
            content: None,
            read_only: false,
            blocked: None,
            partitions,
            free: Vec::new(),
        }
    }

    /// A 512 GiB Windows disk: its EFI partition, Microsoft's reserved
    /// space, Windows, room from 204917 MiB, the recovery tools at the end.
    fn windows() -> Disk {
        disk(
            "/dev/sda",
            Some(Table::Gpt),
            512,
            vec![
                part("/dev/sda", 1, 1, 100, "vfat", part_type::ESP),
                part("/dev/sda", 2, 101, 16, "", part_type::MICROSOFT_RESERVED),
                part(
                    "/dev/sda",
                    3,
                    117,
                    200 * 1024,
                    "ntfs",
                    part_type::BASIC_DATA,
                ),
                part(
                    "/dev/sda",
                    4,
                    523_537,
                    750,
                    "ntfs",
                    part_type::WINDOWS_RECOVERY,
                ),
            ],
        )
    }

    /// A 256 GiB disk with nothing on it.
    fn blank() -> Disk {
        disk("/dev/sdb", None, 256, Vec::new())
    }

    /// A 128 GiB Linux disk a BIOS started: bios_grub, root, swap, home.
    fn linux() -> Disk {
        disk(
            "/dev/sdc",
            Some(Table::Gpt),
            128,
            vec![
                part("/dev/sdc", 1, 1, 1, "", part_type::BIOS_BOOT),
                part(
                    "/dev/sdc",
                    2,
                    2,
                    40 * 1024,
                    "ext4",
                    part_type::LINUX_ROOT_X86_64,
                ),
                part(
                    "/dev/sdc",
                    3,
                    40_962,
                    4 * 1024,
                    "swap",
                    part_type::LINUX_SWAP,
                ),
                part(
                    "/dev/sdc",
                    4,
                    45_058,
                    80 * 1024,
                    "ext4",
                    part_type::LINUX_HOME,
                ),
            ],
        )
    }

    /// A 160 GiB msdos disk: Windows, then an extended partition (the
    /// kernel's 1 KiB stub) holding Linux, its swap and a FAT32 one; nothing
    /// marked active, as on the e2e fixtures `msdos-extended-base` and
    /// `bios-base`.
    fn old() -> Disk {
        let mut extended = part("/dev/sdd", 2, 40_961, 1, "", part_type::MBR_EXTENDED);
        extended.size = 1024;
        disk(
            "/dev/sdd",
            Some(Table::Msdos),
            160,
            vec![
                part("/dev/sdd", 1, 1, 40_960, "ntfs", part_type::MBR_NTFS),
                extended,
                part(
                    "/dev/sdd",
                    5,
                    40_962,
                    30 * 1024,
                    "ext4",
                    part_type::MBR_LINUX,
                ),
                part("/dev/sdd", 6, 71_683, 2 * 1024, "swap", part_type::MBR_SWAP),
                part(
                    "/dev/sdd",
                    7,
                    73_732,
                    28 * 1024,
                    "vfat",
                    part_type::MBR_FAT32_LBA,
                ),
            ],
        )
    }

    /// A 64 GiB msdos disk an older BIOS computer starts: Windows from 1 MiB,
    /// then an old Linux's root and its home to the end, three primaries —
    /// the e2e fixture `windows-msdos-base` to the MiB, Windows marked
    /// active as it is there.
    fn windows_mbr() -> Disk {
        let mut windows = part("/dev/sde", 1, 1, 32 * 1024, "ntfs", part_type::MBR_NTFS);
        windows.bootable = true;
        disk(
            "/dev/sde",
            Some(Table::Msdos),
            64,
            vec![
                windows,
                part(
                    "/dev/sde",
                    2,
                    32_769,
                    28 * 1024,
                    "ext4",
                    part_type::MBR_LINUX,
                ),
                part("/dev/sde", 3, 61_441, 4095, "ext4", part_type::MBR_LINUX),
            ],
        )
    }

    /// [`windows_mbr`] with nothing marked active: a disk whose mark was
    /// never set, or was cleared.
    fn bare_mbr() -> Disk {
        let mut disk = windows_mbr();
        disk.partitions[0].bootable = false;
        disk
    }

    /// [`old`] with its logical partitions numbered out of the disk's
    /// order, as a table numbers them whose logical partitions were made in
    /// another order (the kernel numbers them as their tables are chained;
    /// fdisk's "fix partitions order" is for this): on the disk sdd6 (the
    /// Linux), sdd7 (its swap), then sdd5 (the FAT32), listed by number as
    /// discovery lists them.
    fn jumbled() -> Disk {
        let mut disk = old();
        disk.partitions.truncate(2);
        disk.partitions.extend([
            part(
                "/dev/sdd",
                5,
                73_732,
                28 * 1024,
                "vfat",
                part_type::MBR_FAT32_LBA,
            ),
            part(
                "/dev/sdd",
                6,
                40_962,
                30 * 1024,
                "ext4",
                part_type::MBR_LINUX,
            ),
            part("/dev/sdd", 7, 71_683, 2 * 1024, "swap", part_type::MBR_SWAP),
        ]);
        disk
    }

    /// `disk` with every partition kept and unused, and nothing made.
    fn kept(disk: Disk) -> DiskLayout {
        let parts = disk
            .partitions
            .iter()
            .map(|was| Part {
                path: was.path.clone(),
                number: was.number,
                start: was.start,
                size: was.size,
                fate: Fate::Keep { mount: None },
            })
            .collect();
        DiskLayout {
            disk,
            table: None,
            parts,
            create: Vec::new(),
        }
    }

    /// `disk` under a new `table`: every partition deleted.
    fn fresh(disk: Disk, table: Table) -> DiskLayout {
        let mut one = kept(disk);
        one.table = Some(table);
        for part in &mut one.parts {
            part.fate = Fate::Delete;
        }
        one
    }

    /// `one` with partition `number` given `fate`.
    fn with(mut one: DiskLayout, number: u32, fate: Fate) -> DiskLayout {
        let part = one.parts.iter_mut().find(|part| part.number == number);
        part.expect("no such partition").fate = fate;
        one
    }

    /// `one` with a partition made from `start` MiB, `size` MiB long.
    fn made(
        mut one: DiskLayout,
        start: u64,
        size: u64,
        fs: Fs,
        mount: Option<MountPoint>,
    ) -> DiskLayout {
        one.create.push(Create {
            start: start * MIB,
            size: size * MIB,
            fs,
            label: String::new(),
            mount,
        });
        one.create.sort_by_key(|made| made.start);
        one
    }

    fn other(path: &str) -> Option<MountPoint> {
        Some(MountPoint::Other(path.into()))
    }

    fn format(fs: Fs, size: u64, mount: Option<MountPoint>) -> Fate {
        Fate::Format {
            fs,
            label: String::new(),
            size,
            mount,
        }
    }

    /// The blank disk under a new gpt: a 1 GiB EFI partition alone at
    /// `/boot`, and 100 GiB of ext4 at `/`.
    fn good_fresh() -> DiskLayout {
        let one = fresh(blank(), Table::Gpt);
        let one = made(one, 1, 1024, Fs::Fat32, Some(MountPoint::Boot));
        made(one, 1025, 100 * 1024, Fs::Ext4, Some(MountPoint::Root))
    }

    /// Beside Windows: its EFI partition kept at `/efi`, a new FAT32 `/boot`
    /// and 100 GiB of ext4 at `/` made in the room after Windows.
    fn good_beside_windows() -> DiskLayout {
        let one = with(
            kept(windows()),
            1,
            Fate::Keep {
                mount: Some(MountPoint::Efi),
            },
        );
        let one = made(one, 204_917, 1024, Fs::Fat32, Some(MountPoint::Boot));
        made(one, 205_941, 100 * 1024, Fs::Ext4, Some(MountPoint::Root))
    }

    /// The blank disk under a new gpt for a machine a BIOS starts: GRUB's
    /// 1 MiB `bios_grub` partition, and 100 GiB of ext4 at `/` — no
    /// `/boot`, as `manual-bios-gpt`'s plan has none.
    fn bios_fresh() -> DiskLayout {
        let one = made(fresh(blank(), Table::Gpt), 1, 1, Fs::BiosGrub, None);
        made(one, 2, 100 * 1024, Fs::Ext4, Some(MountPoint::Root))
    }

    /// Beside Windows on a machine a BIOS starts: GRUB's 1 MiB `bios_grub`
    /// partition and 100 GiB of ext4 at `/` made in the room after Windows,
    /// its EFI partition left unused — no `/efi` and no `/boot`.
    fn bios_beside_windows() -> DiskLayout {
        let one = made(kept(windows()), 204_917, 1, Fs::BiosGrub, None);
        made(one, 204_918, 100 * 1024, Fs::Ext4, Some(MountPoint::Root))
    }

    /// A 40 GiB msdos disk an older BIOS computer starts: a FAT32 DATA
    /// partition from 1 MiB, 4 GiB, and room after it; nothing marked
    /// active — the e2e fixture `bios-base`, which the keep-FAT scenario
    /// walks.
    fn bios_base() -> Disk {
        disk(
            "/dev/sdf",
            Some(Table::Msdos),
            40,
            vec![part(
                "/dev/sdf",
                1,
                1,
                4096,
                "vfat",
                part_type::MBR_FAT32_LBA,
            )],
        )
    }

    /// [`bios_base`] as the keep-FAT scenario plans it, DATA kept at a path
    /// of its own, and a 1 GiB FAT32 `/boot` made before its 32 GiB of
    /// ext4 at `/`: a `/boot` under BIOS, where the boot flag goes on it.
    fn boot_before_root_on_bios() -> DiskLayout {
        let data = with(
            kept(bios_base()),
            1,
            Fate::Keep {
                mount: other("/mnt/data"),
            },
        );
        let one = made(data, 4097, 1024, Fs::Fat32, Some(MountPoint::Boot));
        made(one, 5121, 32 * 1024, Fs::Ext4, Some(MountPoint::Root))
    }

    /// `one` on a disk of 4096-byte logical sectors, which [`sector_fits`]
    /// takes: every place and size in it is whole MiB, so the plan is the
    /// same plan, judged by the same rules.
    fn on_large_sectors(mut one: DiskLayout) -> DiskLayout {
        one.disk.logical_sector = 4096;
        one
    }

    fn judge(layout: &[DiskLayout], firmware: Firmware) -> Result<(), Refusal> {
        shape(
            layout,
            firmware,
            Bootloader::Automatic,
            &Software::default(),
        )
    }

    fn uefi(layout: DiskLayout) -> Result<(), Refusal> {
        judge(&[layout], Firmware::Uefi)
    }

    fn bios(layout: DiskLayout) -> Result<(), Refusal> {
        judge(&[layout], Firmware::Bios)
    }

    fn sda(number: u32) -> Which {
        Which::Part(format!("/dev/sda{number}"))
    }

    fn new_on(disk: &str, start_mib: u64) -> Which {
        Which::Made {
            disk: disk.into(),
            start: start_mib * MIB,
        }
    }

    /// Each refusal's name, so the test of them all can say which it has
    /// not seen; a refusal added without a test of its own stops here.
    fn variant(refusal: &Refusal) -> &'static str {
        match refusal {
            Refusal::Blocked { .. } => "Blocked",
            Refusal::Mismatch { .. } => "Mismatch",
            Refusal::Unaligned { .. } => "Unaligned",
            Refusal::Outside { .. } => "Outside",
            Refusal::Overlap { .. } => "Overlap",
            Refusal::Primaries { .. } => "Primaries",
            Refusal::BesideExtended { .. } => "BesideExtended",
            Refusal::LogicalOrder { .. } => "LogicalOrder",
            Refusal::Entries { .. } => "Entries",
            Refusal::CantShrink { .. } => "CantShrink",
            Refusal::Grows { .. } => "Grows",
            Refusal::BiosGrub { .. } => "BiosGrub",
            Refusal::Label { .. } => "Label",
            Refusal::Twice { .. } => "Twice",
            Refusal::Root => "Root",
            Refusal::TooSmall { .. } => "TooSmall",
            Refusal::MustFormat { .. } => "MustFormat",
            Refusal::Needs { .. } => "Needs",
            Refusal::Unmountable { .. } => "Unmountable",
            Refusal::Path { .. } => "Path",
            Refusal::InsideKept { .. } => "InsideKept",
            Refusal::FsTooSmall { .. } => "FsTooSmall",
            Refusal::RestorePoints => "RestorePoints",
            Refusal::Efi => "Efi",
            Refusal::Boot => "Boot",
            Refusal::BootElsewhere { .. } => "BootElsewhere",
            Refusal::EfiOnBios => "EfiOnBios",
            Refusal::BiosGrubMissing { .. } => "BiosGrubMissing",
            Refusal::GrubGap { .. } => "GrubGap",
            Refusal::Bootloader { .. } => "Bootloader",
            Refusal::NotRead { .. } => "NotRead",
            Refusal::ShrinkBelow { .. } => "ShrinkBelow",
            Refusal::Unshrinkable { .. } => "Unshrinkable",
            Refusal::EspFull { .. } => "EspFull",
            Refusal::ImageUnknown { .. } => "ImageUnknown",
        }
    }

    /// Every refusal there is, by [`variant`]'s names.
    const EVERY: [&str; 35] = [
        "Blocked",
        "Mismatch",
        "Unaligned",
        "Outside",
        "Overlap",
        "Primaries",
        "BesideExtended",
        "LogicalOrder",
        "Entries",
        "CantShrink",
        "Grows",
        "BiosGrub",
        "Label",
        "Twice",
        "Root",
        "TooSmall",
        "MustFormat",
        "Needs",
        "Unmountable",
        "Path",
        "InsideKept",
        "FsTooSmall",
        "RestorePoints",
        "Efi",
        "Boot",
        "BootElsewhere",
        "EfiOnBios",
        "BiosGrubMissing",
        "GrubGap",
        "Bootloader",
        "NotRead",
        "ShrinkBelow",
        "Unshrinkable",
        "EspFull",
        "ImageUnknown",
    ];

    #[test]
    fn the_good_layouts_pass_both_halves_of_the_rules() {
        let none = (Measures::default(), ImageSizes::default());
        for (name, layout, firmware) in [
            ("a fresh gpt on UEFI", good_fresh(), Firmware::Uefi),
            ("beside Windows", good_beside_windows(), Firmware::Uefi),
        ] {
            assert_eq!(
                judge(std::slice::from_ref(&layout), firmware),
                Ok(()),
                "{name}"
            );
            assert_eq!(measured(&[layout], &none.0, &none.1), Ok(()), "{name}");
        }
        // On BIOS: gpt with its bios_grub, and msdos with its first MiB free.
        let grub = made(fresh(blank(), Table::Gpt), 1, 1, Fs::BiosGrub, None);
        let grub = made(grub, 2, 100 * 1024, Fs::Ext4, Some(MountPoint::Root));
        assert_eq!(bios(grub), Ok(()));
        let dos = made(
            fresh(blank(), Table::Msdos),
            1,
            100 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        );
        assert_eq!(bios(dos), Ok(()));
        // The old msdos disk: Linux formatted afresh as /, the swap formatted
        // as swap, the FAT32 one kept at a path of one's own.
        let mut dos = with(
            kept(old()),
            5,
            format(Fs::Ext4, 30 * 1024 * MIB, Some(MountPoint::Root)),
        );
        dos = with(
            dos,
            6,
            format(Fs::LinuxSwap, 2 * 1024 * MIB, Some(MountPoint::Swap)),
        );
        dos = with(
            dos,
            7,
            Fate::Keep {
                mount: other("/data"),
            },
        );
        assert_eq!(bios(dos), Ok(()));
    }

    #[test]
    fn every_refusal_is_given_for_what_it_is_for() {
        let read = |part: &Partition, used: u64, free: u64| Measured {
            path: part.path.clone(),
            fs: part.fs_type.clone(),
            uuid: part.uuid.clone(),
            size: part.size,
            measure: Measure::Read {
                used,
                free,
                least: None,
            },
        };
        let measured_with =
            |layout: DiskLayout, measures: Vec<Measured>, images: &[(&str, u64)]| {
                assert_eq!(uefi(layout.clone()), Ok(()), "shape refused it first");
                let images = ImageSizes {
                    paths: images
                        .iter()
                        .map(|(path, size)| (path.to_string(), *size))
                        .collect(),
                };
                measured(&[layout], &Measures { parts: measures }, &images)
            };
        let windows_disk = windows();
        let windows_part = |number: usize| windows_disk.partitions[number - 1].clone();
        let mut luks = windows();
        luks.partitions.push(part(
            "/dev/sda",
            5,
            400_000,
            10 * 1024,
            "crypto_LUKS",
            part_type::LINUX,
        ));
        let mut in_use = windows();
        in_use.blocked = Some(Blocked::InUse {
            mountpoints: vec!["/mnt".into()],
        });
        let mut small = disk("/dev/sde", Some(Table::Gpt), 16, Vec::new());
        small.blocked = Some(Blocked::TooSmall);
        let mut moved = kept(windows());
        moved.parts[2].start += MIB;
        let mut twice = good_fresh();
        twice.disk.path = "/dev/sda".into();
        let mut unsorted = good_fresh();
        unsorted.create.reverse();
        let mut wide = good_fresh();
        wide.disk.logical_sector = 8192;
        let mut odd = good_fresh();
        odd.create[1].start += 512;
        let dos_full = (0..5).fold(fresh(blank(), Table::Msdos), |one, at| {
            made(one, 1 + at * 1024, 1024, Fs::Ext4, None)
        });
        let entries = (0..129).fold(fresh(blank(), Table::Gpt), |one, at| {
            made(one, 1 + at, 1, Fs::Ext4, None)
        });
        let mut gap_63 = disk(
            "/dev/sdb",
            Some(Table::Msdos),
            256,
            vec![part("/dev/sdb", 1, 0, 1024, "ntfs", part_type::MBR_NTFS)],
        );
        gap_63.partitions[0].start = 63 * 512;
        let gap_63 = made(
            kept(gap_63),
            2048,
            100 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        );
        let mut labelled = good_fresh();
        labelled.create[1].label = "-root".into();
        let mut dos_esp = disk(
            "/dev/sdb",
            Some(Table::Msdos),
            256,
            vec![part("/dev/sdb", 1, 1, 300, "vfat", part_type::MBR_ESP)],
        );
        dos_esp.partitions[0].content = Content::Efi;
        let dos_esp = with(
            kept(dos_esp),
            1,
            Fate::Keep {
                mount: Some(MountPoint::Boot),
            },
        );
        let dos_esp = made(dos_esp, 301, 100 * 1024, Fs::Ext4, Some(MountPoint::Root));
        let shrunk = |to: u64| with(good_beside_windows(), 3, Fate::Shrink { to, mount: None });
        let mut hibernated = read(&windows_part(3), 0, 0);
        hibernated.measure = Measure::Hibernated;
        let var = |size: u64| made(good_fresh(), 200 * 1024, size, Fs::Ext4, other("/var"));
        let mut btrfs_beside_var = var(20 * 1024);
        btrfs_beside_var.create[1].fs = Fs::Btrfs;

        let cases: Vec<(&str, Result<(), Refusal>, Refusal)> = vec![
            // The disks.
            (
                "a disk in use",
                uefi(kept(in_use)),
                Refusal::Blocked {
                    disk: "/dev/sda".into(),
                },
            ),
            (
                "/ on a disk too small for Linexin",
                uefi(made(
                    made(
                        fresh(small, Table::Gpt),
                        1,
                        1024,
                        Fs::Fat32,
                        Some(MountPoint::Boot),
                    ),
                    1025,
                    15 * 1024 - 2,
                    Fs::Ext4,
                    Some(MountPoint::Root),
                )),
                Refusal::TooSmall {
                    at: MountPoint::Root,
                    least: MIN_INSTALL,
                },
            ),
            (
                "a start that moved",
                uefi(moved),
                Refusal::Mismatch {
                    disk: "/dev/sda".into(),
                },
            ),
            (
                "a disk named twice",
                judge(&[good_beside_windows(), twice], Firmware::Uefi),
                Refusal::Mismatch {
                    disk: "/dev/sda".into(),
                },
            ),
            (
                "creates out of order",
                uefi(unsorted),
                Refusal::Mismatch {
                    disk: "/dev/sdb".into(),
                },
            ),
            (
                "8 KiB sectors",
                uefi(wide),
                Refusal::Mismatch {
                    disk: "/dev/sdb".into(),
                },
            ),
            (
                "something made with no table",
                uefi(made(kept(blank()), 1, 1024, Fs::Ext4, None)),
                Refusal::Mismatch {
                    disk: "/dev/sdb".into(),
                },
            ),
            (
                "a partition kept under a new table",
                uefi(with(
                    fresh(windows(), Table::Gpt),
                    3,
                    Fate::Keep { mount: None },
                )),
                Refusal::Mismatch {
                    disk: "/dev/sda".into(),
                },
            ),
            (
                "the extended frame formatted",
                bios(with(kept(old()), 2, format(Fs::Ext4, 1024, None))),
                Refusal::Mismatch {
                    disk: "/dev/sdd".into(),
                },
            ),
            // Where partitions stand.
            (
                "a start off a MiB",
                uefi(odd),
                Refusal::Unaligned {
                    part: Which::Made {
                        disk: "/dev/sdb".into(),
                        start: 1025 * MIB + 512,
                    },
                },
            ),
            (
                "past the backup gpt",
                uefi(made(good_fresh(), 256 * 1024 - 1, 1, Fs::Ext4, None)),
                Refusal::Outside {
                    part: new_on("/dev/sdb", 256 * 1024 - 1),
                },
            ),
            (
                "over Windows",
                uefi(made(good_beside_windows(), 100_000, 1024, Fs::Ext4, None)),
                Refusal::Overlap {
                    part: new_on("/dev/sda", 100_000),
                },
            ),
            (
                "five primaries",
                bios(dos_full),
                Refusal::Primaries {
                    disk: "/dev/sdb".into(),
                },
            ),
            (
                "beside a kept extended partition",
                bios(made(kept(old()), 102_404, 10 * 1024, Fs::Ext4, None)),
                Refusal::BesideExtended {
                    disk: "/dev/sdd".into(),
                },
            ),
            (
                "a logical deleted before a kept one",
                bios(with(kept(old()), 5, Fate::Delete)),
                Refusal::LogicalOrder {
                    part: Which::Part("/dev/sdd5".into()),
                },
            ),
            (
                "129 gpt entries",
                uefi(entries),
                Refusal::Entries {
                    disk: "/dev/sdb".into(),
                },
            ),
            // What is done to them.
            (
                "the EFI partition shrunk",
                uefi(with(
                    good_beside_windows(),
                    1,
                    Fate::Shrink {
                        to: 50 * MIB,
                        mount: Some(MountPoint::Efi),
                    },
                )),
                Refusal::CantShrink { part: sda(1) },
            ),
            (
                "Windows grown",
                uefi(with(
                    kept(windows()),
                    3,
                    Fate::Shrink {
                        to: 201 * GIB,
                        mount: None,
                    },
                )),
                Refusal::Grows { part: sda(3) },
            ),
            (
                "bios_grub on UEFI",
                uefi(made(good_fresh(), 200 * 1024, 1, Fs::BiosGrub, None)),
                Refusal::BiosGrub {
                    part: new_on("/dev/sdb", 200 * 1024),
                },
            ),
            (
                "a label starting with -",
                uefi(labelled),
                Refusal::Label {
                    part: new_on("/dev/sdb", 1025),
                },
            ),
            // Where they are used.
            (
                "/ twice",
                uefi(made(
                    good_fresh(),
                    200 * 1024,
                    30 * 1024,
                    Fs::Ext4,
                    Some(MountPoint::Root),
                )),
                Refusal::Twice {
                    at: MountPoint::Root,
                },
            ),
            (
                "nothing at /",
                uefi(made(
                    fresh(blank(), Table::Gpt),
                    1,
                    1024,
                    Fs::Fat32,
                    Some(MountPoint::Boot),
                )),
                Refusal::Root,
            ),
            (
                "Windows kept at /",
                uefi(with(
                    kept(windows()),
                    3,
                    Fate::Keep {
                        mount: Some(MountPoint::Root),
                    },
                )),
                Refusal::MustFormat {
                    part: sda(3),
                    at: MountPoint::Root,
                },
            ),
            (
                "xfs at /",
                uefi({
                    let mut one = good_fresh();
                    one.create[1].fs = Fs::Xfs;
                    one
                }),
                Refusal::Needs {
                    at: MountPoint::Root,
                },
            ),
            (
                "LUKS kept at /data",
                uefi({
                    let one = with(
                        kept(luks),
                        1,
                        Fate::Keep {
                            mount: Some(MountPoint::Efi),
                        },
                    );
                    let one = with(
                        one,
                        5,
                        Fate::Keep {
                            mount: other("/data"),
                        },
                    );
                    let one = made(one, 204_917, 1024, Fs::Fat32, Some(MountPoint::Boot));
                    made(one, 205_941, 100 * 1024, Fs::Ext4, Some(MountPoint::Root))
                }),
                Refusal::Unmountable {
                    part: sda(5),
                    fs: "crypto_LUKS".into(),
                },
            ),
            (
                "inside /home",
                uefi(made(
                    good_fresh(),
                    200 * 1024,
                    1024,
                    Fs::Ext4,
                    other("/home/games"),
                )),
                Refusal::Path {
                    why: PathRefused::Under("/home".into()),
                },
            ),
            (
                "a new partition inside Windows' path",
                uefi(made(
                    with(
                        good_beside_windows(),
                        3,
                        Fate::Keep {
                            mount: other("/data"),
                        },
                    ),
                    308_341,
                    1024,
                    Fs::Ext4,
                    other("/data/games"),
                )),
                Refusal::InsideKept {
                    part: sda(3),
                    at: MountPoint::Other("/data".into()),
                },
            ),
            (
                "btrfs on 128 MiB",
                uefi(made(good_fresh(), 200 * 1024, 128, Fs::Btrfs, None)),
                Refusal::FsTooSmall {
                    part: new_on("/dev/sdb", 200 * 1024),
                    fs: Fs::Btrfs,
                    least: 256 * MIB,
                },
            ),
            (
                "restore points beside /var",
                uefi(btrfs_beside_var.clone()),
                Refusal::RestorePoints,
            ),
            // What starts the computer.
            (
                "no EFI partition",
                uefi({
                    let mut one = good_fresh();
                    one.create[0].mount = None;
                    one
                }),
                Refusal::Efi,
            ),
            (
                "an EFI partition at /efi alone",
                uefi(with(
                    made(
                        kept(windows()),
                        204_917,
                        100 * 1024,
                        Fs::Ext4,
                        Some(MountPoint::Root),
                    ),
                    1,
                    Fate::Keep {
                        mount: Some(MountPoint::Efi),
                    },
                )),
                Refusal::Boot,
            ),
            (
                "/boot on another disk than /efi",
                judge(
                    &[
                        made(
                            made(
                                fresh(blank(), Table::Gpt),
                                1,
                                1024,
                                Fs::Fat32,
                                Some(MountPoint::Efi),
                            ),
                            1025,
                            100 * 1024,
                            Fs::Ext4,
                            Some(MountPoint::Root),
                        ),
                        made(
                            kept(windows()),
                            204_917,
                            1024,
                            Fs::Fat32,
                            Some(MountPoint::Boot),
                        ),
                    ],
                    Firmware::Uefi,
                ),
                Refusal::BootElsewhere {
                    beside: MountPoint::Efi,
                },
            ),
            (
                "/efi on BIOS",
                bios(made(
                    made(
                        made(fresh(blank(), Table::Gpt), 1, 1, Fs::BiosGrub, None),
                        2,
                        1024,
                        Fs::Fat32,
                        Some(MountPoint::Efi),
                    ),
                    1026,
                    100 * 1024,
                    Fs::Ext4,
                    Some(MountPoint::Root),
                )),
                Refusal::EfiOnBios,
            ),
            (
                "no bios_grub",
                bios(made(
                    fresh(blank(), Table::Gpt),
                    1,
                    100 * 1024,
                    Fs::Ext4,
                    Some(MountPoint::Root),
                )),
                Refusal::BiosGrubMissing {
                    disk: "/dev/sdb".into(),
                },
            ),
            (
                "a partition at sector 63",
                bios(gap_63),
                Refusal::GrubGap {
                    disk: "/dev/sdb".into(),
                },
            ),
            (
                "systemd-boot on an msdos EFI disk",
                shape(
                    &[dos_esp],
                    Firmware::Uefi,
                    Bootloader::SystemdBoot,
                    &Software::default(),
                ),
                Refusal::Bootloader {
                    loader: Bootloader::SystemdBoot,
                },
            ),
            // What reading the disks found.
            (
                "a shrink nobody read",
                measured_with(shrunk(150 * 1024 * MIB), Vec::new(), &[]),
                Refusal::NotRead { part: sda(3) },
            ),
            (
                "a shrink below what is used",
                measured_with(
                    shrunk(60 * 1024 * MIB),
                    vec![read(
                        &windows_part(3),
                        100 * GIB,
                        windows_part(3).size - 100 * GIB,
                    )],
                    &[],
                ),
                Refusal::ShrinkBelow {
                    part: sda(3),
                    least: shrink_floor(100 * GIB, None),
                },
            ),
            (
                "a hibernated Windows",
                measured_with(shrunk(150 * 1024 * MIB), vec![hibernated], &[]),
                Refusal::Unshrinkable {
                    part: sda(3),
                    why: Unshrinkable::Itself(Measure::Hibernated),
                },
            ),
            (
                "an EFI partition too full",
                measured_with(
                    good_beside_windows(),
                    vec![read(&windows_part(1), 84 * MIB, 16 * MIB)],
                    &[],
                ),
                Refusal::EspFull {
                    part: sda(1),
                    least: ESP_MIN_FREE,
                },
            ),
            (
                "/var with nothing known",
                measured_with(var(20 * 1024), Vec::new(), &[]),
                Refusal::ImageUnknown {
                    at: MountPoint::Other("/var".into()),
                },
            ),
        ];
        let mut seen = BTreeSet::new();
        for (name, judged, wanted) in cases {
            assert_eq!(judged, Err(wanted.clone()), "{name}");
            seen.insert(variant(&wanted));
        }
        let missing: Vec<&str> = EVERY
            .iter()
            .copied()
            .filter(|name| !seen.contains(name))
            .collect();
        assert!(missing.is_empty(), "no case for {missing:?}");
        // Switched off, restore points are no refusal beside /var.
        let software = Software {
            snapshots: false,
            ..Software::default()
        };
        assert_eq!(
            shape(
                &[btrfs_beside_var],
                Firmware::Uefi,
                Bootloader::Automatic,
                &software
            ),
            Ok(())
        );
    }

    #[test]
    fn the_least_sizes_are_the_mount_points_and_the_filesystems() {
        let mut efi = good_fresh();
        efi.create[0].mount = Some(MountPoint::Efi);
        efi.create[0].size = 32 * MIB;
        let efi = made(efi, 200 * 1024, 1024, Fs::Fat32, Some(MountPoint::Boot));
        assert_eq!(
            uefi(efi),
            Err(Refusal::TooSmall {
                at: MountPoint::Efi,
                least: 33 * MIB
            })
        );
        let mut boot = good_fresh();
        boot.create[0].size = 300 * MIB;
        assert_eq!(
            uefi(boot),
            Err(Refusal::TooSmall {
                at: MountPoint::Boot,
                least: ESP_BOOT_FREE
            })
        );
        let home = made(
            good_fresh(),
            200 * 1024,
            512,
            Fs::Ext4,
            Some(MountPoint::Home),
        );
        assert_eq!(
            uefi(home),
            Err(Refusal::TooSmall {
                at: MountPoint::Home,
                least: MIN_HOME
            })
        );
        let swap = made(
            good_fresh(),
            200 * 1024,
            256,
            Fs::LinuxSwap,
            Some(MountPoint::Swap),
        );
        assert_eq!(
            uefi(swap),
            Err(Refusal::TooSmall {
                at: MountPoint::Swap,
                least: SWAP_LEAST
            })
        );
        // FAT32 on 4096-byte sectors needs room for its 65,525 clusters.
        assert_eq!(fs_least(Fs::Fat32, 512), 33 * MIB);
        assert_eq!(fs_least(Fs::Fat32, 4096), 257 * MIB);
        assert_eq!(fs_least(Fs::Btrfs, 4096), 256 * MIB);
        // Swap may be on more than one partition; a mount point of one's own
        // once.
        let swaps = made(
            good_fresh(),
            200 * 1024,
            1024,
            Fs::LinuxSwap,
            Some(MountPoint::Swap),
        );
        let swaps = made(
            swaps,
            201 * 1024,
            1024,
            Fs::LinuxSwap,
            Some(MountPoint::Swap),
        );
        assert_eq!(uefi(swaps), Ok(()));
        let data = made(good_fresh(), 200 * 1024, 1024, Fs::Ext4, other("/data"));
        let data = made(data, 201 * 1024, 1024, Fs::Xfs, other("/data"));
        assert_eq!(
            uefi(data),
            Err(Refusal::Twice {
                at: MountPoint::Other("/data".into())
            })
        );
    }

    #[test]
    fn a_kept_partition_serves_only_where_it_may() {
        // /home kept, of Linux's own filesystems only.
        let linux_home = with(
            kept(linux()),
            4,
            Fate::Keep {
                mount: Some(MountPoint::Home),
            },
        );
        assert_eq!(judge(&[good_fresh(), linux_home], Firmware::Uefi), Ok(()));
        let windows_home = with(
            good_beside_windows(),
            3,
            Fate::Keep {
                mount: Some(MountPoint::Home),
            },
        );
        assert_eq!(
            uefi(windows_home),
            Err(Refusal::Needs {
                at: MountPoint::Home
            })
        );
        // Windows kept at a path of one's own; never where the image puts
        // files, which must be formatted.
        let at_data = with(
            good_beside_windows(),
            3,
            Fate::Keep {
                mount: other("/mnt/windows"),
            },
        );
        assert_eq!(uefi(at_data), Ok(()));
        for path in ["/var", "/opt/games", "/tmp"] {
            let at = with(good_beside_windows(), 3, Fate::Keep { mount: other(path) });
            assert_eq!(
                uefi(at),
                Err(Refusal::MustFormat {
                    part: sda(3),
                    at: MountPoint::Other(path.into())
                }),
                "{path}"
            );
        }
        // The EFI partition kept at /boot alone holds the kernels; beside a
        // /efi, a kept one at /boot must be formatted (it becomes XBOOTLDR).
        let esp_boot = with(
            kept(windows()),
            1,
            Fate::Keep {
                mount: Some(MountPoint::Boot),
            },
        );
        let esp_boot = made(
            esp_boot,
            204_917,
            100 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        );
        assert_eq!(uefi(esp_boot), Ok(()));
        // Windows' recovery tools are no EFI partition.
        let recovery = with(
            kept(windows()),
            4,
            Fate::Keep {
                mount: Some(MountPoint::Efi),
            },
        );
        let recovery = made(recovery, 204_917, 1024, Fs::Fat32, Some(MountPoint::Boot));
        let recovery = made(
            recovery,
            205_941,
            100 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        );
        assert_eq!(
            uefi(recovery),
            Err(Refusal::MustFormat {
                part: sda(4),
                at: MountPoint::Efi
            })
        );
        // Swap kept: a fresh one is needed.
        let swap = with(
            kept(linux()),
            3,
            Fate::Keep {
                mount: Some(MountPoint::Swap),
            },
        );
        assert_eq!(
            judge(&[good_fresh(), swap], Firmware::Uefi),
            Err(Refusal::MustFormat {
                part: Which::Part("/dev/sdc3".into()),
                at: MountPoint::Swap
            })
        );
    }

    #[test]
    fn nothing_is_mounted_inside_a_kept_partitions_path() {
        // Windows kept at /data: nothing made, formatted or kept inside it,
        // however deep, since the folder for it would be made in Windows'
        // NTFS at the first start.
        let windows_at =
            |path: &str| with(good_beside_windows(), 3, Fate::Keep { mount: other(path) });
        let refused = Err(Refusal::InsideKept {
            part: sda(3),
            at: MountPoint::Other("/data".into()),
        });
        for (fs, path) in [
            (Fs::Ext4, "/data/games"),
            (Fs::Xfs, "/data/a/b"),
            (Fs::Fat32, "/data/x"),
        ] {
            let inside = made(windows_at("/data"), 308_341, 1024, fs, other(path));
            assert_eq!(uefi(inside), refused, "{path}");
        }
        let old_home = with(
            kept(linux()),
            4,
            Fate::Keep {
                mount: other("/data/old"),
            },
        );
        assert_eq!(
            judge(&[windows_at("/data"), old_home], Firmware::Uefi),
            refused,
            "another partition kept inside it"
        );
        // Shrunk, it is kept all the same.
        let shrunk = with(
            good_beside_windows(),
            3,
            Fate::Shrink {
                to: 150 * GIB,
                mount: other("/data"),
            },
        );
        let shrunk = made(shrunk, 308_341, 1024, Fs::Ext4, other("/data/games"));
        assert_eq!(uefi(shrunk), refused);
        // A path that only begins with its letters is beside it, not in it;
        // and a kept partition inside one made afresh is mounted in that one
        // (its folder made there while Linexin is installed).
        let beside = made(
            windows_at("/data"),
            308_341,
            1024,
            Fs::Ext4,
            other("/database"),
        );
        assert_eq!(uefi(beside), Ok(()));
        let in_new = made(
            windows_at("/data/windows"),
            308_341,
            1024,
            Fs::Ext4,
            other("/data"),
        );
        assert_eq!(uefi(in_new), Ok(()));
    }

    #[test]
    fn nothing_is_written_over_what_is_on_the_whole_disk() {
        // A filesystem (or LUKS, LVM, RAID) written straight onto the disk:
        // a new table over it would erase it under a consent line naming
        // nothing ("table /dev/sdb gpt"), so it is refused, with partitions
        // made in the new table or with none.
        let mut whole = blank();
        whole.content = Some(Content::Other);
        let refused = Err(Refusal::Mismatch {
            disk: "/dev/sdb".into(),
        });
        let boot_and_root = |one: DiskLayout| {
            let one = made(one, 1, 1024, Fs::Fat32, Some(MountPoint::Boot));
            made(one, 1025, 100 * 1024, Fs::Ext4, Some(MountPoint::Root))
        };
        assert_eq!(
            uefi(boot_and_root(fresh(whole.clone(), Table::Gpt))),
            refused
        );
        assert_eq!(
            judge(
                &[good_beside_windows(), fresh(whole.clone(), Table::Msdos)],
                Firmware::Uefi
            ),
            refused,
            "a new table alone"
        );
        // Nor is anything made beside it where the disk has a table too.
        let mut tabled = whole;
        tabled.table = Some(Table::Gpt);
        assert_eq!(uefi(boot_and_root(kept(tabled))), refused);
        // With nothing on it, the same layout passes.
        assert_eq!(uefi(boot_and_root(fresh(blank(), Table::Gpt))), Ok(()));
    }

    /// On a disk something is written over as a whole, discovery lists no
    /// free space: a formatted partition grows only over the partitions the
    /// layout deletes after it, one after another, never into a gap between
    /// them or after the last — which on any other disk it may.
    #[test]
    fn a_formatted_partition_grows_on_a_disk_written_over_as_a_whole_only_over_what_goes() {
        let mut data = disk(
            "/dev/sdd",
            Some(Table::Gpt),
            16,
            vec![
                part("/dev/sdd", 1, 1, 1000, "ext4", part_type::LINUX),
                part("/dev/sdd", 2, 1001, 1000, "ext4", part_type::LINUX),
                part("/dev/sdd", 3, 3001, 1000, "ext4", part_type::LINUX),
            ],
        );
        let at = |path: &str| Some(MountPoint::from(path.to_string()));
        let grown = |disk: &Disk, number: u32, size: u64, deleted: &[u32]| {
            let mut one = with(
                kept(disk.clone()),
                number,
                format(Fs::Ext4, size * MIB, at("/data")),
            );
            for gone in deleted {
                one = with(one, *gone, Fate::Delete);
            }
            judge(&[good_fresh(), one], Firmware::Uefi)
        };
        let sdd = |number: u32| Which::Part(format!("/dev/sdd{number}"));
        for content in [None, Some(Content::Other)] {
            data.content = content;
            let whole = content.is_some();
            let over_the_rest = |number: u32| {
                if whole {
                    Err(Refusal::Overlap { part: sdd(number) })
                } else {
                    Ok(())
                }
            };
            // Over its deleted neighbour's own span, to its end: allowed.
            assert_eq!(grown(&data, 1, 2000, &[2]), Ok(()), "{content:?}");
            // Shrunk, or kept at its size: nothing grows.
            assert_eq!(grown(&data, 2, 600, &[]), Ok(()), "{content:?}");
            // Past it into the gap, into the gap with nothing deleted, over
            // two deleted ones with the gap between, past the last.
            assert_eq!(grown(&data, 1, 2500, &[2]), over_the_rest(1), "{content:?}");
            assert_eq!(grown(&data, 2, 1500, &[]), over_the_rest(2), "{content:?}");
            assert_eq!(
                grown(&data, 1, 4000, &[2, 3]),
                over_the_rest(1),
                "{content:?}"
            );
            assert_eq!(grown(&data, 3, 2000, &[]), over_the_rest(3), "{content:?}");
            // Over both deleted ones where they meet: allowed.
            let mut meeting = data.clone();
            meeting.partitions[2].start = 2001 * MIB;
            assert_eq!(grown(&meeting, 1, 3000, &[2, 3]), Ok(()), "{content:?}");
        }
    }

    #[test]
    fn a_disk_too_small_for_linexin_holds_anything_but_root() {
        let mut small = disk(
            "/dev/sde",
            Some(Table::Gpt),
            16,
            vec![part("/dev/sde", 1, 1, 15 * 1024, "ext4", part_type::LINUX)],
        );
        small.blocked = Some(Blocked::TooSmall);
        let home = with(
            kept(small.clone()),
            1,
            Fate::Keep {
                mount: Some(MountPoint::Home),
            },
        );
        assert_eq!(judge(&[good_fresh(), home], Firmware::Uefi), Ok(()));
        // Read-only, it holds nothing.
        small.read_only = true;
        let home = with(
            kept(small),
            1,
            Fate::Keep {
                mount: Some(MountPoint::Home),
            },
        );
        assert_eq!(
            judge(&[good_fresh(), home], Firmware::Uefi),
            Err(Refusal::Blocked {
                disk: "/dev/sde".into()
            })
        );
    }

    #[test]
    fn logical_partitions_go_from_the_last_and_their_frame_with_them() {
        // The last logical partition may go; then the one before it.
        let last = with(kept(old()), 7, Fate::Delete);
        let last = with(
            last,
            5,
            format(Fs::Ext4, 30 * 1024 * MIB, Some(MountPoint::Root)),
        );
        assert_eq!(bios(last.clone()), Ok(()));
        assert_eq!(bios(with(last, 6, Fate::Delete)), Ok(()));
        // The frame only with all of them.
        let frame = with(kept(old()), 2, Fate::Delete);
        assert_eq!(
            bios(frame),
            Err(Refusal::Mismatch {
                disk: "/dev/sdd".into()
            })
        );
        let mut gone = kept(old());
        for number in [2, 5, 6, 7] {
            gone = with(gone, number, Fate::Delete);
        }
        // With it gone, a primary partition may be made where it stood.
        let gone = made(gone, 40_961, 100 * 1024, Fs::Ext4, Some(MountPoint::Root));
        assert_eq!(bios(gone), Ok(()));
        // A logical partition's entry stays inside its frame.
        let grown = with(
            kept(old()),
            7,
            format(Fs::Ext4, 40 * 1024 * MIB, Some(MountPoint::Root)),
        );
        assert_eq!(
            bios(grown),
            Err(Refusal::Outside {
                part: Which::Part("/dev/sdd7".into())
            })
        );
        // Nor over the table of a logical partition kept after it: up to
        // sdd6's start it overlaps no partition, but sdd6's own table (its
        // EBR) stands in the MiB before sdd6, which only sdd6's deletion
        // takes away — and sdd6 goes only once sdd7 has.
        let sdd5 = Which::Part("/dev/sdd5".into());
        let to_sdd6 = (71_683 - 40_962) * MIB;
        let root = |size: u64| format(Fs::Ext4, size, Some(MountPoint::Root));
        let grown = with(kept(old()), 5, root(to_sdd6));
        assert_eq!(bios(grown), Err(Refusal::Overlap { part: sdd5.clone() }));
        let last_gone = with(with(kept(old()), 7, Fate::Delete), 5, root(to_sdd6));
        assert_eq!(bios(last_gone), Err(Refusal::Overlap { part: sdd5 }));
        let alone = with(with(kept(old()), 7, Fate::Delete), 6, Fate::Delete);
        let to_the_frame = (102_404 - 40_962) * MIB;
        assert_eq!(bios(with(alone, 5, root(to_the_frame))), Ok(()));
        // Shrunk, or as long as it was, it stands where it stood.
        for size in [24 * 1024 * MIB, 30 * 1024 * MIB] {
            assert_eq!(bios(with(kept(old()), 5, root(size))), Ok(()));
        }
        // The last logical partition has no table after it to cover: it
        // grows as far as its frame reaches.
        let mut roomy = old();
        roomy.partitions[1].size = (122_880 - 40_961) * MIB;
        let sdd7 = format(Fs::Ext4, (122_880 - 73_732) * MIB, Some(MountPoint::Root));
        assert_eq!(bios(with(kept(roomy), 7, sdd7)), Ok(()));
    }

    #[test]
    fn a_logical_partition_grows_over_no_table_of_one_standing_after_it_on_the_disk() {
        // Logical partitions numbered out of the disk's order (step Q1). On
        // the jumbled disk sdd6 stands first, sdd7 next and sdd5 last. sdd7,
        // numbered last, may go; then sdd6 grown up to sdd5's start overlaps
        // no partition and has none numbered after it kept — yet sdd5 stands
        // after it on the disk, and sdd5's own table (its EBR) in the MiB
        // before it.
        let root = |size: u64| format(Fs::Ext4, size, Some(MountPoint::Root));
        let sdd6 = Which::Part("/dev/sdd6".into());
        let swap_gone = with(kept(jumbled()), 7, Fate::Delete);
        let to_sdd5 = (73_732 - 40_962) * MIB;
        assert_eq!(
            bios(with(swap_gone.clone(), 6, root(to_sdd5))),
            Err(Refusal::Overlap { part: sdd6 })
        );
        // Shrunk, or as long as it was, it stands where it stood (and sdd7,
        // numbered last, goes).
        for size in [24 * 1024 * MIB, 30 * 1024 * MIB] {
            assert_eq!(bios(with(swap_gone.clone(), 6, root(size))), Ok(()));
        }
        // Numbered after is not standing after: sdd5, numbered before the
        // two others but last on the disk, has no table after it to cover,
        // and grows as far as its frame reaches with both of them kept.
        let mut roomy = jumbled();
        roomy.partitions[1].size = (122_880 - 40_961) * MIB;
        let sdd5 = root((122_880 - 73_732) * MIB);
        assert_eq!(bios(with(kept(roomy), 5, sdd5)), Ok(()));
        // Deleting still goes by number, as the kernel renumbers: sdd5,
        // though it stands last, goes only once sdd6 and sdd7 have.
        assert_eq!(
            bios(with(kept(jumbled()), 5, Fate::Delete)),
            Err(Refusal::LogicalOrder {
                part: Which::Part("/dev/sdd5".into())
            })
        );
    }

    /// **A first partition at sector 63 leaves GRUB no room** (Unit Q, Q7):
    /// on a machine a BIOS starts, GRUB's core goes in the first MiB of the
    /// msdos disk that holds `/`, behind its MBR — and a disk partitioned
    /// by tools of DOS's time starts its first partition at sector 63,
    /// leaving 31 KiB. Refused whatever the layout does with that partition
    /// but delete it — kept unused, kept at a path of one's own, formatted
    /// at its own size — at 4096 bytes a sector as at 512; up to the last
    /// sector before 1 MiB, and not from it. Only `/`'s disk is held to it,
    /// and only on BIOS: on UEFI GRUB goes on the EFI system partition.
    #[test]
    fn grub_gap_refuses_a_first_partition_at_sector_63() {
        let gap = Err(Refusal::GrubGap {
            disk: "/dev/sde".into(),
        });
        for sector in [512, 4096] {
            // An old msdos disk: Windows' NTFS from `start`, 100 GiB long.
            let old = |start: u64| {
                let windows = part("/dev/sde", 1, 0, 100 * 1024, "ntfs", part_type::MBR_NTFS);
                let mut disk = disk("/dev/sde", Some(Table::Msdos), 256, vec![windows]);
                disk.logical_sector = sector;
                disk.partitions[0].start = start;
                kept(disk)
            };
            let size = old(0).parts[0].size;
            let root = |one: DiskLayout| {
                made(
                    one,
                    101 * 1024,
                    100 * 1024,
                    Fs::Ext4,
                    Some(MountPoint::Root),
                )
            };
            let at_63 = || old(63 * u64::from(sector));
            for (what, fate) in [
                ("kept", Fate::Keep { mount: None }),
                (
                    "kept at a path of its own",
                    Fate::Keep {
                        mount: other("/windows"),
                    },
                ),
                ("formatted", format(Fs::Ext4, size, other("/data"))),
            ] {
                let judged = bios(root(with(at_63(), 1, fate)));
                assert_eq!(judged, gap, "{what} at sector 63 of {sector}");
            }
            // Deleted, it leaves the gap free.
            assert_eq!(bios(root(with(at_63(), 1, Fate::Delete))), Ok(()));
            // The last sector before 1 MiB is in the gap; 1 MiB is past it.
            let last = MIB - u64::from(sector);
            assert_eq!(bios(root(old(last))), gap, "at {last} of {sector}");
            assert_eq!(bios(root(old(MIB))), Ok(()), "at 1 MiB of {sector}");
            // On UEFI the gap is nobody's: an EFI system partition made
            // beside it at /boot holds what starts Linexin.
            let efi = made(at_63(), 101 * 1024, 1024, Fs::Fat32, Some(MountPoint::Boot));
            let efi = made(
                efi,
                102 * 1024,
                100 * 1024,
                Fs::Ext4,
                Some(MountPoint::Root),
            );
            assert_eq!(uefi(efi), Ok(()), "UEFI at {sector}");
            // Only / 's disk is held to it: Windows kept at a path of its own
            // on the old disk, / on a blank gpt disk with its bios_grub.
            let windows = with(
                at_63(),
                1,
                Fate::Keep {
                    mount: other("/windows"),
                },
            );
            let elsewhere = made(fresh(blank(), Table::Gpt), 1, 1, Fs::BiosGrub, None);
            let elsewhere = made(elsewhere, 2, 100 * 1024, Fs::Ext4, Some(MountPoint::Root));
            assert_eq!(
                judge(&[windows, elsewhere], Firmware::Bios),
                Ok(()),
                "/ on another disk at {sector}"
            );
        }
    }

    #[test]
    fn labels_are_held_to_what_their_filesystems_take() {
        for (fs, label, fits) in [
            (Fs::Ext4, "", true),
            (Fs::Ext4, "Linexin root 123", true),
            (Fs::Ext4, "Linexin root 1234", false),
            (Fs::Ext4, "Zażółć gęś", true),
            (Fs::Ext4, "Zażółć gęśl", false),
            (Fs::Fat32, "EFI", true),
            (Fs::Fat32, "BOOT_2026", true),
            (Fs::Fat32, "boot", false),
            (Fs::Fat32, "ZAŻÓŁĆ", false),
            (Fs::Fat32, " BOOT", false),
            (Fs::Fat32, "LINEXINBOOT", true),
            (Fs::Fat32, "LINEXINBOOT1", false),
            (Fs::Xfs, "twelve chars", true),
            (Fs::Xfs, "thirteen char", false),
            (Fs::Btrfs, &"b".repeat(255), true),
            (Fs::Btrfs, &"b".repeat(256), false),
            (Fs::LinuxSwap, "swap", true),
            (Fs::Ext4, "-L", false),
            (Fs::Ext4, "tab\there", false),
            (Fs::Ext4, "a-b", true),
            (Fs::Ext4, "a/devops", false),
            (Fs::Ext4, "/dev/vdz", false),
            (Fs::Btrfs, "x/dev", false),
            (Fs::Xfs, "/", false),
            (Fs::LinuxSwap, "swap/1", false),
            (Fs::BiosGrub, "grub", false),
            (Fs::BiosGrub, "", true),
        ] {
            assert_eq!(label_fits(fs, label), fits, "{fs:?} {label:?}");
        }
    }

    /// **A filesystem smaller than its partition has only its entry made
    /// smaller** (Partitions and the worker agree on it): reading carries the
    /// filesystem's own size into `Shrinkable` — what it uses and what is
    /// free on it — and an entry at or above it is made smaller alone
    /// (`entry_only`), down to that size even below what it uses and a margin
    /// (`least_entry`); below it, the filesystem is shrunk first, and not
    /// below what it uses and a margin. Windows' 200 GiB partition holding a
    /// 105 GiB volume, 100 GiB of it used — the floor 110 GiB — as a
    /// partition grown without its filesystem, or a run stopped between a
    /// filesystem's shrink and its entry, leaves one; and the same 105 GiB
    /// asked of a volume filling its partition.
    #[test]
    fn a_filesystem_smaller_than_its_partition_has_only_its_entry_made_smaller() {
        let windows_part = windows().partitions[2].clone();
        let reading = |used: u64, free: u64| Measures {
            parts: vec![Measured {
                path: windows_part.path.clone(),
                fs: windows_part.fs_type.clone(),
                uuid: windows_part.uuid.clone(),
                size: windows_part.size,
                measure: Measure::Read {
                    used,
                    free,
                    least: None,
                },
            }],
        };
        let small = reading(100 * GIB, 5 * GIB);
        let Some(Ok(shrinkable)) = small.shrink_of(&windows_part) else {
            panic!("{:?}", small.shrink_of(&windows_part));
        };
        assert_eq!(shrinkable.filesystem, 105 * GIB);
        assert_eq!(shrinkable.least_entry(), 105 * GIB);
        assert!(shrinkable.entry_only(105 * GIB) && shrinkable.entry_only(150 * GIB));
        assert!(!shrinkable.entry_only(105 * GIB - MIB));
        let shrunk = |to: u64| {
            vec![with(
                good_beside_windows(),
                3,
                Fate::Shrink { to, mount: None },
            )]
        };
        let none = ImageSizes::default();
        for to in [105 * GIB, 110 * GIB, 150 * GIB] {
            assert_eq!(uefi(shrunk(to)[0].clone()), Ok(()), "{to}");
            assert_eq!(measured(&shrunk(to), &small, &none), Ok(()), "{to}");
        }
        assert_eq!(
            measured(&shrunk(105 * GIB - MIB), &small, &none),
            Err(Refusal::ShrinkBelow {
                part: sda(3),
                least: 105 * GIB,
            })
        );
        // Filling its partition, its filesystem is shrunk: not below what
        // it uses and a margin.
        let full = reading(100 * GIB, windows_part.size - 100 * GIB);
        let Some(Ok(filling)) = full.shrink_of(&windows_part) else {
            panic!("{:?}", full.shrink_of(&windows_part));
        };
        assert_eq!(filling.filesystem, windows_part.size);
        assert!(!filling.entry_only(150 * GIB));
        assert_eq!(filling.least_entry(), shrink_floor(100 * GIB, None));
        assert_eq!(measured(&shrunk(110 * GIB), &full, &none), Ok(()));
        assert_eq!(
            measured(&shrunk(105 * GIB), &full, &none),
            Err(Refusal::ShrinkBelow {
                part: sda(3),
                least: 110 * GIB,
            })
        );
    }

    /// **An NTFS is shrunk only while no Windows of the measurement holds it
    /// back** (`Measures::shrink_of`, the one place the rule set reads a
    /// measurement for a shrink; step Q8): (a) its tool read it cleanly, (b)
    /// a `hiberfil.sys` of its own says Windows is not hibernated there, and
    /// (c) across every disk of the measurement no other NTFS is signed
    /// hibernated, none could not be read whole (its tool or its sign failed,
    /// a check due, in use, its disk not read) and no volume is BitLocker.
    /// One with no `hiberfil.sys` of its own is never shrunk, but holds no
    /// other back. An ext4 is judged by its own reading alone.
    #[test]
    fn an_ntfs_is_shrunk_only_while_no_windows_of_the_measurement_holds_it_back() {
        use Measure::{Encrypted, Hibernated, InUse, NeedsCheck, Unreadable};
        let system = windows().partitions[2].clone();
        let elsewhere = old().partitions[0].clone();
        let mut data = elsewhere.clone();
        data.path = "/dev/sdd5".into();
        let home = linux().partitions[3].clone();
        let esp = windows().partitions[0].clone();
        let bitlocker = part("/dev/sdb", 1, 1, 1024, "BitLocker", part_type::BASIC_DATA);
        let luks = part("/dev/sdb", 2, 1025, 1024, "crypto_LUKS", part_type::LINUX);
        let measured = |part: &Partition, measure: Measure| Measured {
            path: part.path.clone(),
            fs: part.fs_type.clone(),
            uuid: part.uuid.clone(),
            size: part.size,
            measure,
        };
        let read = Measure::Read {
            used: 50 * GIB,
            free: 150 * GIB,
            least: Some(60 * GIB),
        };
        let no_file = Measure::NoHiberfile {
            used: 50 * GIB,
            free: 150 * GIB,
        };
        let may = Some(Ok(Shrinkable {
            used: 50 * GIB,
            least: Some(60 * GIB),
            filesystem: 200 * GIB,
        }));
        let judged = |of: &Partition, parts: Vec<Measured>| Measures { parts }.shrink_of(of);
        // Windows' own partition beside every other there is.
        let held = |others: Vec<Measured>| {
            let mut parts = vec![measured(&system, read)];
            parts.extend(others);
            judged(&system, parts)
        };
        let named = |part: &Partition| part.path.clone();

        // Read and signed not hibernated, alone: shrunk. Not read: not known.
        assert_eq!(judged(&system, vec![measured(&system, read)]), may);
        assert_eq!(judged(&system, vec![measured(&elsewhere, read)]), None);
        // (a) and (b): its own reading — its tool refusing it, its sign
        // hibernated or unread, and no hiberfil.sys of its own.
        for own in [NeedsCheck, Unreadable, InUse, Hibernated, Encrypted] {
            assert_eq!(
                judged(&system, vec![measured(&system, own)]),
                Some(Err(Unshrinkable::Itself(own))),
                "{own:?}"
            );
        }
        assert_eq!(
            judged(&system, vec![measured(&system, no_file)]),
            Some(Err(Unshrinkable::NoHiberfile))
        );
        // (c): another NTFS signed hibernated…
        assert_eq!(
            held(vec![measured(&elsewhere, Hibernated)]),
            Some(Err(Unshrinkable::Hibernated {
                other: named(&elsewhere)
            }))
        );
        // …one whose state could not be read: its tool or its sign failed
        // (an error, not a missing file), a check due, in use, on a disk
        // not read…
        for unread in [Unreadable, NeedsCheck, InUse, Encrypted] {
            assert_eq!(
                held(vec![measured(&elsewhere, unread)]),
                Some(Err(Unshrinkable::Unread {
                    other: named(&elsewhere)
                })),
                "{unread:?}"
            );
        }
        // …and a BitLocker volume, however it was answered.
        for locked in [Encrypted, InUse, Unreadable] {
            assert_eq!(
                held(vec![measured(&bitlocker, locked)]),
                Some(Err(Unshrinkable::BitLocker {
                    other: named(&bitlocker)
                })),
                "{locked:?}"
            );
        }
        // But another NTFS read, or read with no hiberfil.sys of its own (a
        // data volume), holds nothing back; nor a LUKS volume, nor an ext4
        // or an EFI partition whatever was read of them.
        assert_eq!(held(vec![measured(&elsewhere, read)]), may);
        assert_eq!(held(vec![measured(&elsewhere, no_file)]), may);
        let not_windows = vec![
            measured(&luks, Encrypted),
            measured(&home, Unreadable),
            measured(&esp, InUse),
        ];
        assert_eq!(held(not_windows), may);
        // A Windows hibernated is said first, then one unread, then
        // BitLocker, whatever the measurement's order.
        let all = vec![
            measured(&bitlocker, Encrypted),
            measured(&data, Unreadable),
            measured(&elsewhere, Hibernated),
        ];
        assert_eq!(
            held(all.clone()),
            Some(Err(Unshrinkable::Hibernated {
                other: named(&elsewhere)
            }))
        );
        assert_eq!(
            held(all[..2].to_vec()),
            Some(Err(Unshrinkable::Unread {
                other: named(&data)
            }))
        );
        // An ext4 is its own reading alone, whatever the Windows.
        assert_eq!(
            judged(&home, [vec![measured(&home, read)], all.clone()].concat()),
            may
        );
        // And the rule set's own: Windows' shrink refused for the other.
        let to = shrink_floor(50 * GIB, Some(60 * GIB));
        let layout = with(good_beside_windows(), 3, Fate::Shrink { to, mount: None });
        let images = ImageSizes::default();
        let refused = |others: Vec<Measured>| {
            let mut parts = vec![measured(&system, read)];
            parts.extend(others);
            measured_judged(&layout, Measures { parts }, &images)
        };
        assert_eq!(refused(vec![measured(&elsewhere, read)]), Ok(()));
        assert_eq!(
            refused(vec![measured(&elsewhere, Hibernated)]),
            Err(Refusal::Unshrinkable {
                part: sda(3),
                why: Unshrinkable::Hibernated {
                    other: named(&elsewhere)
                },
            })
        );
        assert_eq!(
            refused(vec![measured(&bitlocker, Encrypted)]),
            Err(Refusal::Unshrinkable {
                part: sda(3),
                why: Unshrinkable::BitLocker {
                    other: named(&bitlocker)
                },
            })
        );
    }

    fn measured_judged(
        layout: &DiskLayout,
        measures: Measures,
        images: &ImageSizes,
    ) -> Result<(), Refusal> {
        measured(std::slice::from_ref(layout), &measures, images)
    }

    #[test]
    fn a_shrink_keeps_what_is_used_and_a_margin_and_the_tools_least() {
        assert_eq!(shrink_floor(10 * GIB, None), 11 * GIB);
        assert_eq!(shrink_floor(100 * GIB, None), 110 * GIB);
        assert_eq!(shrink_floor(10 * GIB, Some(12 * GIB + 1)), 12 * GIB + MIB);
        // ext4 and NTFS kept, nothing else; never Windows' own partitions.
        let disk = windows();
        assert!(shrinkable(&disk.partitions[2]));
        assert!(!shrinkable(&disk.partitions[0]), "the EFI partition");
        assert!(!shrinkable(&disk.partitions[3]), "the recovery tools");
        assert!(shrinkable(&linux().partitions[1]));
        assert!(!shrinkable(&old().partitions[1]), "an extended frame");
        assert!(!shrinkable(&old().partitions[4]), "FAT32");
        // Read, a shrink to its floor is fine.
        let to = shrink_floor(100 * GIB, None);
        let layout = with(good_beside_windows(), 3, Fate::Shrink { to, mount: None });
        let windows = &layout.disk.partitions[2];
        let measures = Measures {
            parts: vec![Measured {
                path: windows.path.clone(),
                fs: windows.fs_type.clone(),
                uuid: windows.uuid.clone(),
                size: windows.size,
                measure: Measure::Read {
                    used: 100 * GIB,
                    free: 0,
                    least: None,
                },
            }],
        };
        assert_eq!(uefi(layout.clone()), Ok(()));
        assert_eq!(
            measured(
                std::slice::from_ref(&layout),
                &measures,
                &ImageSizes::default()
            ),
            Ok(())
        );
        // A measure of another partition at the same path is not this one's.
        let mut other_one = measures.clone();
        other_one.parts[0].uuid = "ELSEWHERE".into();
        assert_eq!(
            measured(&[layout], &other_one, &ImageSizes::default()),
            Err(Refusal::NotRead { part: sda(3) })
        );
    }

    #[test]
    fn a_kept_efi_partition_has_the_room_its_mount_point_needs() {
        let esp = windows().partitions[0].clone();
        let with_free = |free: u64| Measures {
            parts: vec![Measured {
                path: esp.path.clone(),
                fs: esp.fs_type.clone(),
                uuid: esp.uuid.clone(),
                size: esp.size,
                measure: Measure::Read {
                    used: esp.size - free,
                    free,
                    least: None,
                },
            }],
        };
        let images = ImageSizes::default();
        // At /efi, the boot loader's room; unknown, the worker reads it again
        // before it writes.
        let at_efi = [good_beside_windows()];
        assert_eq!(measured(&at_efi, &Measures::default(), &images), Ok(()));
        assert_eq!(measured(&at_efi, &with_free(ESP_MIN_FREE), &images), Ok(()));
        // At /boot alone, the kernels' room, never guessed.
        let alone = with(
            kept(windows()),
            1,
            Fate::Keep {
                mount: Some(MountPoint::Boot),
            },
        );
        let alone = [made(
            alone,
            204_917,
            100 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        )];
        assert_eq!(
            measured(&alone, &Measures::default(), &images),
            Err(Refusal::NotRead { part: sda(1) })
        );
        assert_eq!(
            measured(&alone, &with_free(69 * MIB), &images),
            Err(Refusal::EspFull {
                part: sda(1),
                least: ESP_BOOT_FREE
            })
        );
    }

    #[test]
    fn a_folder_the_image_fills_holds_what_it_puts_there() {
        let images = ImageSizes {
            paths: [("/var", 2 * GIB), ("/opt", 400 * MIB)]
                .into_iter()
                .map(|(path, size)| (path.to_string(), size))
                .collect(),
        };
        // /var: 2 GiB × 1.25 + 1 GiB, and 4 GiB more.
        assert_eq!(
            image_floor("/var", &["/var"], &images),
            Some(7 * GIB + 512 * MIB)
        );
        assert_eq!(image_floor("/opt", &["/opt"], &images), Some(1524 * MIB));
        assert_eq!(image_floor("/srv", &["/srv"], &images), None);
        // What a mount inside it takes is not counted twice.
        let nested = ImageSizes {
            paths: [
                ("/var", 4 * GIB),
                ("/var/lib", 3 * GIB),
                ("/var/lib/x", GIB),
            ]
            .into_iter()
            .map(|(path, size)| (path.to_string(), size))
            .collect(),
        };
        assert_eq!(
            image_floor("/var", &["/var", "/var/lib", "/var/lib/x"], &nested),
            Some(GIB + 256 * MIB + 5 * GIB)
        );
        let var = |size: u64| {
            [made(
                good_fresh(),
                200 * 1024,
                size,
                Fs::Ext4,
                other("/var"),
            )]
        };
        assert_eq!(
            measured(&var(7 * 1024), &Measures::default(), &images),
            Err(Refusal::TooSmall {
                at: MountPoint::Other("/var".into()),
                least: 7 * GIB + 512 * MIB
            })
        );
        assert_eq!(
            measured(&var(8 * 1024), &Measures::default(), &images),
            Ok(())
        );
    }

    #[test]
    fn each_partition_made_is_typed_by_where_it_is_used() {
        use Fs::*;
        use MountPoint::*;
        let gpt = |fs, at: Option<MountPoint>, role| part_type(Table::Gpt, fs, at.as_ref(), role);
        let dos = |fs, at: Option<MountPoint>, role| part_type(Table::Msdos, fs, at.as_ref(), role);
        for (fs, at, role, on_gpt, on_msdos) in [
            (
                Ext4,
                Some(Root),
                Role::Plain,
                part_type::LINUX_ROOT_X86_64,
                part_type::MBR_LINUX,
            ),
            (
                Btrfs,
                Some(Root),
                Role::Plain,
                part_type::LINUX_ROOT_X86_64,
                part_type::MBR_LINUX,
            ),
            (
                Xfs,
                Some(Home),
                Role::Plain,
                part_type::LINUX_HOME,
                part_type::MBR_LINUX,
            ),
            (
                LinuxSwap,
                Some(Swap),
                Role::Plain,
                part_type::LINUX_SWAP,
                part_type::MBR_SWAP,
            ),
            (
                LinuxSwap,
                None,
                Role::Plain,
                part_type::LINUX_SWAP,
                part_type::MBR_SWAP,
            ),
            (
                Fat32,
                Some(Efi),
                Role::Esp,
                part_type::ESP,
                part_type::MBR_ESP,
            ),
            (
                Fat32,
                Some(Boot),
                Role::Esp,
                part_type::ESP,
                part_type::MBR_ESP,
            ),
            (
                Fat32,
                Some(Boot),
                Role::LinuxBoot,
                part_type::XBOOTLDR,
                part_type::MBR_XBOOTLDR,
            ),
            (
                Fat32,
                Some(Boot),
                Role::Plain,
                part_type::LINUX,
                part_type::MBR_FAT32_LBA,
            ),
            (
                BiosGrub,
                None,
                Role::Plain,
                part_type::BIOS_BOOT,
                part_type::MBR_LINUX,
            ),
            (
                Fat32,
                Some(Other("/data".into())),
                Role::Plain,
                part_type::LINUX,
                part_type::MBR_FAT32_LBA,
            ),
            (
                Xfs,
                Some(Other("/srv".into())),
                Role::Plain,
                part_type::LINUX,
                part_type::MBR_LINUX,
            ),
            (
                Ext4,
                None,
                Role::Plain,
                part_type::LINUX,
                part_type::MBR_LINUX,
            ),
        ] {
            assert_eq!(gpt(fs, at.clone(), role), on_gpt, "{fs:?} {at:?} {role:?}");
            assert_eq!(
                dos(fs, at.clone(), role),
                on_msdos,
                "{fs:?} {at:?} {role:?}"
            );
        }
        // The roles: on UEFI the EFI partition at /efi, or alone at /boot;
        // a /boot beside /efi XBOOTLDR; on BIOS nothing is either.
        assert_eq!(role(Firmware::Uefi, Some(&Efi), true), Role::Esp);
        assert_eq!(role(Firmware::Uefi, Some(&Boot), false), Role::Esp);
        assert_eq!(role(Firmware::Uefi, Some(&Boot), true), Role::LinuxBoot);
        assert_eq!(role(Firmware::Uefi, Some(&Root), true), Role::Plain);
        assert_eq!(role(Firmware::Bios, Some(&Boot), false), Role::Plain);
        assert!(efi_used(&[good_beside_windows()]));
        assert!(!efi_used(&[good_fresh()]));
    }

    #[test]
    fn what_a_layout_destroys_is_listed_disk_by_disk_table_first() {
        let windows = with(kept(windows()), 4, Fate::Delete);
        let windows = with(
            windows,
            3,
            Fate::Shrink {
                to: 150 * GIB,
                mount: None,
            },
        );
        let windows = with(
            windows,
            1,
            format(Fs::Fat32, 100 * MIB, Some(MountPoint::Boot)),
        );
        let windows = made(
            windows,
            204_917,
            100 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        );
        let linux = with(
            kept(linux()),
            4,
            Fate::Keep {
                mount: Some(MountPoint::Home),
            },
        );
        let lines: Vec<String> = destroys(&[
            fresh(self::linux(), Table::Gpt),
            good_fresh(),
            windows.clone(),
        ])
        .iter()
        .map(Destroy::line)
        .collect();
        assert_eq!(
            lines,
            [
                format!(
                    "format /dev/sda1 1 start={} size={} to={} fat32",
                    MIB,
                    100 * MIB,
                    100 * MIB
                ),
                format!(
                    "shrink /dev/sda3 3 start={} from={} to={} ntfs",
                    117 * MIB,
                    200 * GIB,
                    150 * GIB
                ),
                format!(
                    "delete /dev/sda4 4 start={} size={} ntfs",
                    523_537 * MIB,
                    750 * MIB
                ),
                "table /dev/sdb gpt".to_string(),
                "table /dev/sdc gpt erases /dev/sdc1 /dev/sdc2 /dev/sdc3 /dev/sdc4".to_string(),
            ]
        );
        // Kept, mounted or made, nothing is destroyed.
        assert!(destroys(&[linux, good_beside_windows()]).is_empty());
        // The order does not hang on the order the layout lists its disks in.
        let again: Vec<String> =
            destroys(&[windows, good_fresh(), fresh(self::linux(), Table::Gpt)])
                .iter()
                .map(Destroy::line)
                .collect();
        assert_eq!(again, lines);
    }

    /// A layout needs what it asks of the worker, and goes on only where
    /// all of that lies within one set of [`PROVEN`] — what passed in QEMU:
    /// a new gpt on a blank disk under UEFI, Windows formatted or shrunk
    /// beside partitions made, an ext4 shrunk, each as one scenario ran it;
    /// not a new gpt under BIOS beside a `/home` kept on another disk, each
    /// of whose needs some scenario ran but none all together; and not a
    /// primary partition's entry grown, which no scenario ran at all.
    #[test]
    fn a_layout_needs_what_it_asks_of_the_worker_and_goes_on_only_where_it_passed() {
        use Capability::*;
        assert_eq!(
            needs(&[good_fresh()], Firmware::Uefi),
            [FreshTable, Uefi, Gpt]
        );
        assert_eq!(
            needs(&[good_beside_windows()], Firmware::Uefi),
            [KeepTable, Uefi, Gpt]
        );
        assert_eq!(
            needs(&[windows_formatted()], Firmware::Uefi),
            [KeepTable, ChangeEntries, Uefi, Gpt]
        );
        assert_eq!(
            needs(&[windows_shrunk()], Firmware::Uefi),
            [KeepTable, Uefi, Gpt, ShrinkNtfs]
        );
        assert_eq!(
            needs(&[linux_shrunk()], Firmware::Uefi),
            [KeepTable, Uefi, Gpt, ShrinkExt4]
        );
        assert_eq!(
            needs(&[bios_fresh(), linux_home()], Firmware::Bios),
            [FreshTable, KeptHome, SeveralDisks, Bios, Gpt]
        );
        for (layout, firmware) in [
            (good_fresh(), Firmware::Uefi),
            (good_beside_windows(), Firmware::Uefi),
            (windows_formatted(), Firmware::Uefi),
            (windows_shrunk(), Firmware::Uefi),
            (linux_shrunk(), Firmware::Uefi),
        ] {
            assert_eq!(unproven(&[layout], firmware), None, "{firmware:?}");
        }
        assert_eq!(
            unproven(&[bios_fresh(), linux_home()], Firmware::Bios),
            Some(Unproven::Together(vec![
                FreshTable,
                KeptHome,
                SeveralDisks,
                Bios,
                Gpt
            ]))
        );
        assert_eq!(
            unproven(&[primary_grown_on_msdos()], Firmware::Bios),
            Some(Unproven::Never(LargerEntry))
        );
    }

    /// Every capability is in [`EVERY_CAPABILITY`] once, in its order: the
    /// list the tests pretend everything proven by and reckon mixes over.
    #[test]
    fn every_capability_is_listed_once_in_its_order() {
        let place = |one: Capability| -> usize {
            // A capability added to the enum and not here fails to build.
            match one {
                Capability::FreshTable => 0,
                Capability::KeepTable => 1,
                Capability::DeleteEntries => 2,
                Capability::DeleteLogical => 3,
                Capability::ChangeEntries => 4,
                Capability::SmallerEntry => 5,
                Capability::LargerEntry => 6,
                Capability::LargerLogical => 7,
                Capability::KeptHome => 8,
                Capability::SeveralDisks => 9,
                Capability::Uefi => 10,
                Capability::Bios => 11,
                Capability::Gpt => 12,
                Capability::Msdos => 13,
                Capability::Logical => 14,
                Capability::EmptyFrame => 15,
                Capability::Extended => 16,
                Capability::RootOnLogical => 17,
                Capability::Activate => 18,
                Capability::BootOnBios => 19,
                Capability::LargeSectors => 20,
                Capability::ShrinkExt4 => 21,
                Capability::ShrinkNtfs => 22,
            }
        };
        assert_eq!(EVERY_CAPABILITY.len(), 23);
        for (at, one) in EVERY_CAPABILITY.iter().enumerate() {
            assert_eq!(place(*one), at, "{one:?}");
        }
        assert!(EVERY_CAPABILITY.windows(2).all(|two| two[0] < two[1]));
    }

    /// **A shrink is proven by its filesystem's scenario, and a logical
    /// partition only by a scenario that wrote one**: were one `Shrink` to
    /// hold ext4 and NTFS alike, and nothing to mark a logical partition, the
    /// BIOS scenario that passed — an NTFS shrunk on an MBR disk — would let
    /// an ext4 shrunk there go on, and with it a logical partition shrunk,
    /// which no scenario runs. With the campaign's sets pretended proven, an
    /// ext4 shrunk on an msdos disk, primary or logical, an NTFS shrunk
    /// beside a logical partition deleted, and a logical partition written
    /// beside a kept `/home` are each refused as a mix no scenario ran, and
    /// the extended frame deleted as what none ran at all; each shrink still
    /// goes on on GPT under UEFI, where its scenario shrank it. With the sets
    /// of the scenarios that shrink each filesystem left out, that shrink is
    /// refused alone, the other still going on; and with the logical
    /// scenario's set left out, every write to a logical partition is
    /// refused, the same work on a primary one going on.
    #[test]
    fn a_shrink_is_proven_by_its_filesystem_and_a_logical_partition_by_a_scenario_that_wrote_one() {
        use Capability::*;
        let sets = the_campaigns_sets();
        let ext4_on_msdos = vec![with(
            kept(windows_mbr()),
            2,
            Fate::Shrink {
                to: 20 * GIB,
                mount: None,
            },
        )];
        let ext4_logical = vec![with(
            kept(old()),
            5,
            Fate::Shrink {
                to: 20 * GIB,
                mount: other("/data"),
            },
        )];
        let ntfs_beside_a_logical = vec![with(
            with(
                kept(old()),
                1,
                Fate::Shrink {
                    to: 30 * GIB,
                    mount: None,
                },
            ),
            7,
            Fate::Delete,
        )];
        let home_beside_a_logical = vec![with(
            with(
                kept(old()),
                5,
                Fate::Keep {
                    mount: Some(MountPoint::Home),
                },
            ),
            7,
            Fate::Delete,
        )];
        // A logical partition deleted, one formatted, and the same work on
        // the disk's primary partition.
        let logical_deleted = vec![old_deleted()];
        let logical_formatted = vec![with(
            kept(old()),
            5,
            format(Fs::Ext4, 30 * GIB, other("/data")),
        )];
        let primary_formatted = vec![with(
            kept(old()),
            1,
            format(Fs::Ext4, 40 * GIB, other("/data")),
        )];
        assert_eq!(
            needs(&ext4_on_msdos, Firmware::Bios),
            [Bios, Msdos, ShrinkExt4]
        );
        assert_eq!(
            needs(&ext4_logical, Firmware::Bios),
            [Bios, Msdos, Logical, ShrinkExt4]
        );
        assert_eq!(
            needs(&ntfs_beside_a_logical, Firmware::Bios),
            [DeleteLogical, Bios, Msdos, Logical, ShrinkNtfs]
        );
        assert_eq!(
            needs(&home_beside_a_logical, Firmware::Bios),
            [DeleteLogical, KeptHome, Bios, Msdos, Logical]
        );
        assert_eq!(
            needs(&logical_formatted, Firmware::Bios),
            [ChangeEntries, Bios, Msdos, Logical]
        );
        assert_eq!(
            needs(&primary_formatted, Firmware::Bios),
            [ChangeEntries, Bios, Msdos]
        );
        assert_eq!(
            needs(&[old_extended_deleted()], Firmware::Bios),
            [
                DeleteEntries,
                DeleteLogical,
                Bios,
                Msdos,
                Logical,
                EmptyFrame,
                Extended
            ]
        );
        // A logical partition only mounted from is no write.
        assert_eq!(needs(&[old_mounted()], Firmware::Bios), [Bios, Msdos]);
        // A filesystem neither tool shrinks (the rule set refuses it before
        // this is asked): both shrinks, which no scenario ran together.
        assert_eq!(
            needs(
                &[with(
                    kept(windows()),
                    1,
                    Fate::Shrink {
                        to: 50 * MIB,
                        mount: None,
                    },
                )],
                Firmware::Uefi,
            ),
            [Uefi, Gpt, ShrinkExt4, ShrinkNtfs]
        );
        // Under a new table nothing logical is written: everything goes,
        // the msdos table written over counted.
        assert_eq!(
            needs(&[fresh(old(), Table::Gpt)], Firmware::Uefi),
            [FreshTable, Uefi, Gpt, Msdos]
        );
        {
            let _campaign = PretendProven::these(sets);
            for (layout, firmware) in [
                (&ext4_on_msdos, Firmware::Bios),
                (&ext4_logical, Firmware::Bios),
                (&ntfs_beside_a_logical, Firmware::Bios),
                (&home_beside_a_logical, Firmware::Bios),
                // Msdos under UEFI, whatever is shrunk.
                (&ext4_on_msdos, Firmware::Uefi),
            ] {
                assert_eq!(
                    unproven(layout, firmware),
                    Some(Unproven::Together(needs(layout, firmware))),
                    "{layout:?}"
                );
            }
            // Named by the frame left empty, the first of the two no set
            // holds.
            assert_eq!(
                unproven(&[old_extended_deleted()], Firmware::Bios),
                Some(Unproven::Never(EmptyFrame))
            );
            for (layout, firmware) in [
                (vec![linux_shrunk()], Firmware::Uefi),
                (vec![windows_shrunk()], Firmware::Uefi),
                (logical_deleted.clone(), Firmware::Bios),
                (logical_formatted.clone(), Firmware::Bios),
                (primary_formatted.clone(), Firmware::Bios),
            ] {
                assert_eq!(unproven(&layout, firmware), None, "{layout:?}");
            }
        }
        let leaving_out = |out| sets_without(sets, out);
        {
            let _no_ext4 = PretendProven::these(leaving_out(ShrinkExt4));
            assert_eq!(
                unproven(&[linux_shrunk()], Firmware::Uefi),
                Some(Unproven::Never(ShrinkExt4))
            );
            assert_eq!(unproven(&[windows_shrunk()], Firmware::Uefi), None);
        }
        {
            let _no_ntfs = PretendProven::these(leaving_out(ShrinkNtfs));
            assert_eq!(
                unproven(&[windows_shrunk()], Firmware::Uefi),
                Some(Unproven::Never(ShrinkNtfs))
            );
            assert_eq!(unproven(&[linux_shrunk()], Firmware::Uefi), None);
        }
        {
            let _no_logical = PretendProven::these(leaving_out(Logical));
            // A logical partition deleted is named by its deletion, which
            // only the logical scenario ran; one formatted by Logical.
            assert_eq!(
                unproven(&logical_deleted, Firmware::Bios),
                Some(Unproven::Never(DeleteLogical))
            );
            assert_eq!(
                unproven(&logical_formatted, Firmware::Bios),
                Some(Unproven::Never(Logical))
            );
            assert_eq!(unproven(&primary_formatted, Firmware::Bios), None);
        }
    }

    /// **Every layout needs its firmware and the kind of every table it
    /// writes or keeps** (decision 20: the computer's type and the disk's
    /// table type counted too): a machine UEFI starts needs `Uefi` as one a
    /// BIOS starts needs `Bios`; a disk under a new table, a kept one written
    /// to and one only mounted from each need their table's kind — `Gpt` or
    /// `Msdos` — so what is proven on GPT never opens an msdos disk
    /// (step Q5), what is proven of msdos under BIOS never opens a machine
    /// UEFI starts, and what is proven of a new GPT under BIOS never opens a
    /// kept one there. With Q5's three and several disks on GPT under UEFI
    /// and a new GPT under BIOS pretended proven, each a set of its own: GPT
    /// under UEFI goes on, every msdos layout is refused naming `Msdos`, and
    /// the kept GPT disk under BIOS is refused as a mix no set holds whole.
    #[test]
    fn every_layout_needs_its_firmware_and_the_kind_of_every_table_it_writes_or_keeps() {
        use Capability::*;
        assert_eq!(
            needs(std::slice::from_ref(&msdos_fresh()), Firmware::Uefi),
            [FreshTable, Uefi, Msdos]
        );
        assert_eq!(
            needs(std::slice::from_ref(&msdos_fresh()), Firmware::Bios),
            [FreshTable, Bios, Msdos]
        );
        let formatted = with(kept(old()), 5, format(Fs::Ext4, 30 * GIB, other("/data")));
        // A logical partition formatted: its table's kind, and Logical.
        assert_eq!(
            needs(std::slice::from_ref(&formatted), Firmware::Bios),
            [ChangeEntries, Bios, Msdos, Logical]
        );
        assert_eq!(
            needs(&[old_deleted()], Firmware::Uefi),
            [DeleteLogical, Uefi, Msdos, Logical]
        );
        // Only mounted from, its table kept as it is and nothing written
        // there: its kind counted all the same.
        assert_eq!(
            needs(&[good_fresh(), old_mounted()], Firmware::Uefi),
            [FreshTable, SeveralDisks, Uefi, Gpt, Msdos]
        );
        assert_eq!(needs(&[linux_data()], Firmware::Uefi), [Uefi, Gpt]);
        assert_eq!(needs(&[old_mounted()], Firmware::Bios), [Bios, Msdos]);
        // A kept GPT disk under BIOS: its firmware and its table.
        assert_eq!(
            needs(&[bios_beside_windows()], Firmware::Bios),
            [KeepTable, Bios, Gpt]
        );
        let gpt = [good_fresh(), good_beside_windows(), windows_formatted()];
        assert!(gpt.iter().all(|one| {
            let needed = needs(std::slice::from_ref(one), Firmware::Uefi);
            needed.contains(&Gpt) && !needed.contains(&Msdos) && !needed.contains(&Bios)
        }));
        // (A logical partition's deletion among them, so that the logical
        // partition deleted under UEFI is refused for its table alone.)
        let _proven = PretendProven::these(&[
            &[
                FreshTable,
                KeepTable,
                DeleteEntries,
                DeleteLogical,
                ChangeEntries,
                SeveralDisks,
                Uefi,
                Gpt,
            ],
            &[FreshTable, Bios, Gpt],
        ]);
        for one in &gpt {
            assert_eq!(unproven(std::slice::from_ref(one), Firmware::Uefi), None);
        }
        assert_eq!(unproven(&[bios_fresh()], Firmware::Bios), None);
        for (layout, firmware) in [
            (vec![msdos_fresh()], Firmware::Uefi),
            (vec![msdos_fresh()], Firmware::Bios),
            (vec![formatted], Firmware::Bios),
            (vec![old_deleted()], Firmware::Uefi),
            (vec![good_fresh(), old_mounted()], Firmware::Uefi),
        ] {
            assert_eq!(
                unproven(&layout, firmware),
                Some(Unproven::Never(Msdos)),
                "{firmware:?} {layout:?}"
            );
        }
        assert_eq!(
            unproven(&[bios_beside_windows()], Firmware::Bios),
            Some(Unproven::Together(vec![KeepTable, Bios, Gpt]))
        );
    }

    /// What a test pretends is proven holds on its own thread, for as long
    /// as the pretence lives, nested pretences giving back the one before;
    /// and with none, what passed is: `PROVEN`, which refuses a new gpt
    /// under BIOS beside a `/home` kept on another disk (no one scenario
    /// ran the two together). Pretended proven by nothing, as were no set
    /// written, every layout is refused.
    #[test]
    fn a_pretended_proven_set_holds_on_its_own_thread_and_while_it_lives() {
        use Capability::*;
        let two = || [bios_fresh(), linux_home()];
        let together = Some(Unproven::Together(vec![
            FreshTable,
            KeptHome,
            SeveralDisks,
            Bios,
            Gpt,
        ]));
        assert_eq!(unproven(&two(), Firmware::Bios), together);
        {
            let _both = PretendProven::these(&[&[FreshTable, KeptHome, SeveralDisks, Bios, Gpt]]);
            assert_eq!(unproven(&two(), Firmware::Bios), None);
            assert_eq!(
                unproven(&two(), Firmware::Uefi),
                Some(Unproven::Never(Uefi))
            );
            {
                let _none = PretendProven::these(&[]);
                assert_eq!(
                    unproven(&two(), Firmware::Bios),
                    Some(Unproven::Never(FreshTable))
                );
                assert_eq!(
                    unproven(&[good_fresh()], Firmware::Uefi),
                    Some(Unproven::Never(FreshTable))
                );
            }
            assert_eq!(unproven(&two(), Firmware::Bios), None);
            let elsewhere = std::thread::spawn(move || unproven(&two(), Firmware::Bios));
            assert_eq!(elsewhere.join().unwrap(), together);
        }
        assert_eq!(unproven(&two(), Firmware::Bios), together);
        assert_eq!(unproven(&[good_fresh()], Firmware::Uefi), None);
    }

    /// Windows' disk beside it ([`good_beside_windows`]) with Windows
    /// formatted for data: partitions made and one formatted on a GPT kept.
    fn windows_formatted() -> DiskLayout {
        with(
            good_beside_windows(),
            3,
            format(Fs::Ext4, 200 * GIB, other("/data")),
        )
    }

    /// Windows' disk beside it ([`good_beside_windows`]) with its recovery
    /// partition deleted and Windows formatted for `/home` at half its
    /// size: partitions made and deleted on a GPT kept, and one formatted
    /// with its entry made smaller.
    fn windows_replaced() -> DiskLayout {
        with(
            with(good_beside_windows(), 4, Fate::Delete),
            3,
            format(Fs::Ext4, 100 * GIB, Some(MountPoint::Home)),
        )
    }

    /// Windows' disk beside it with Windows shrunk to 150 GiB.
    fn windows_shrunk() -> DiskLayout {
        with(
            good_beside_windows(),
            3,
            Fate::Shrink {
                to: 150 * GIB,
                mount: None,
            },
        )
    }

    /// The Linux disk only mounted from: its home kept at `/home`.
    fn linux_home() -> DiskLayout {
        with(
            kept(linux()),
            4,
            Fate::Keep {
                mount: Some(MountPoint::Home),
            },
        )
    }

    /// The Linux disk only mounted from: its home kept at a path of one's
    /// own, nothing written to it.
    fn linux_data() -> DiskLayout {
        with(
            kept(linux()),
            4,
            Fate::Keep {
                mount: other("/data"),
            },
        )
    }

    /// The blank disk under a new msdos table, `/` made on it.
    fn msdos_fresh() -> DiskLayout {
        made(
            fresh(blank(), Table::Msdos),
            1,
            100 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        )
    }

    /// The old msdos disk with its last logical partition deleted.
    fn old_deleted() -> DiskLayout {
        with(kept(old()), 7, Fate::Delete)
    }

    /// The old msdos disk only mounted from: its FAT32 kept at a path of
    /// one's own, nothing written to it.
    fn old_mounted() -> DiskLayout {
        with(
            kept(old()),
            7,
            Fate::Keep {
                mount: other("/camera"),
            },
        )
    }

    /// The old msdos disk with its extended frame deleted, and every
    /// logical partition in it.
    fn old_extended_deleted() -> DiskLayout {
        let mut one = kept(old());
        for number in [2, 5, 6, 7] {
            one = with(one, number, Fate::Delete);
        }
        one
    }

    /// The old msdos disk with every logical partition deleted, the first
    /// among them, its frame kept, and Windows' primary partition formatted
    /// as `/`: the frame left empty.
    fn every_logical_deleted() -> DiskLayout {
        let mut one = with(
            kept(old()),
            1,
            format(Fs::Ext4, 40 * GIB, Some(MountPoint::Root)),
        );
        for number in [5, 6, 7] {
            one = with(one, number, Fate::Delete);
        }
        one
    }

    /// The old msdos disk with Windows, its primary partition, deleted, and
    /// the first logical partition formatted as `/` — which would leave no
    /// partition marked active, the boot flag going only on a primary one
    /// (`manual::plan::activation`): `/` on a logical partition.
    fn windows_deleted_beside_a_logical_root() -> DiskLayout {
        with(
            with(kept(old()), 1, Fate::Delete),
            5,
            format(Fs::Ext4, 30 * GIB, Some(MountPoint::Root)),
        )
    }

    /// The old msdos disk with Windows, its primary partition, deleted, and
    /// the first logical partition formatted for data: a primary partition
    /// deleted beside a logical one written.
    fn windows_deleted_beside_a_logical_one() -> DiskLayout {
        with(
            with(kept(old()), 1, Fate::Delete),
            5,
            format(Fs::Ext4, 30 * GIB, other("/data")),
        )
    }

    /// The old msdos disk with its frame reaching to 120 GiB and its last
    /// logical partition formatted for data, its entry grown to the frame's
    /// end: a logical partition grown, nothing deleted.
    fn logical_grown() -> DiskLayout {
        let mut roomy = old();
        roomy.partitions[1].size = (122_880 - 40_961) * MIB;
        with(
            kept(roomy),
            7,
            format(Fs::Ext4, (122_880 - 73_732) * MIB, other("/data")),
        )
    }

    /// The MBR disk ([`windows_mbr`]) with the old home, its last primary
    /// partition, deleted, and the old root formatted as `/` at its own
    /// size: a primary partition deleted on msdos.
    fn primary_deleted_on_msdos() -> DiskLayout {
        with(
            with(kept(windows_mbr()), 3, Fate::Delete),
            2,
            format(Fs::Ext4, 28 * GIB, Some(MountPoint::Root)),
        )
    }

    /// The MBR disk with the old home deleted and the old root formatted as
    /// `/`, its entry grown to the disk's end over where the home stood: a
    /// primary partition's entry grown on msdos.
    fn primary_grown_on_msdos() -> DiskLayout {
        with(
            with(kept(windows_mbr()), 3, Fate::Delete),
            2,
            format(Fs::Ext4, 32_767 * MIB, Some(MountPoint::Root)),
        )
    }

    /// The Linux disk with its ext4 home shrunk to 40 GiB and kept at
    /// `/data`, an ext4 `/var` made in what it gives up.
    fn linux_shrunk() -> DiskLayout {
        let one = with(
            kept(linux()),
            4,
            Fate::Shrink {
                to: 40 * GIB,
                mount: other("/data"),
            },
        );
        made(one, 45_058 + 40 * 1024, 8 * 1024, Fs::Ext4, other("/var"))
    }

    /// An ext4 shrunk on its own: the Linux disk's root, nothing else
    /// written.
    fn ext4_shrunk() -> DiskLayout {
        with(
            kept(linux()),
            2,
            Fate::Shrink {
                to: 30 * GIB,
                mount: None,
            },
        )
    }

    /// An NTFS shrunk on its own: Windows' disk, nothing else written.
    fn ntfs_shrunk() -> DiskLayout {
        with(
            kept(windows()),
            3,
            Fate::Shrink {
                to: 150 * GIB,
                mount: None,
            },
        )
    }

    /// The plan of each of [`CAMPAIGN`]'s scenarios as layouts written here
    /// can stand for it — the same kinds of work on the same kinds of
    /// machine and table, which is all [`needs`] reads — by the scenario's
    /// name, on the firmware it runs under.
    fn the_campaigns_plans() -> Vec<(&'static str, Vec<DiskLayout>, Firmware)> {
        // As `plan_bios_logical` walks it: the primary partition before the
        // frame formatted as `/` at its own size; the last logical partition
        // deleted; the swap partition before it formatted and its entry
        // grown over where that stood; the first logical one kept at a path
        // of its own.
        let logical = with(
            with(
                with(
                    old_deleted(),
                    1,
                    format(Fs::Ext4, 40 * GIB, Some(MountPoint::Root)),
                ),
                6,
                format(Fs::LinuxSwap, 30 * GIB, Some(MountPoint::Swap)),
            ),
            5,
            Fate::Keep {
                mount: other("/data"),
            },
        );
        // The old Linux reinstalled: its root and swap formatted, its home
        // kept, nothing made or deleted.
        let reinstalled = with(
            with(
                linux_home(),
                2,
                format(Fs::Ext4, 40 * GIB, Some(MountPoint::Root)),
            ),
            3,
            format(Fs::LinuxSwap, 4 * GIB, Some(MountPoint::Swap)),
        );
        // Windows shrunk on the computer's one MBR disk and a swap partition
        // made in what it gives up, the old root formatted, the home kept.
        let beside_windows_mbr = made(
            with(
                with(
                    with(
                        kept(windows_mbr()),
                        1,
                        Fate::Shrink {
                            to: 28 * GIB,
                            mount: None,
                        },
                    ),
                    2,
                    format(Fs::Ext4, 28 * GIB, Some(MountPoint::Root)),
                ),
                3,
                Fate::Keep {
                    mount: Some(MountPoint::Home),
                },
            ),
            28_673,
            4 * 1024,
            Fs::LinuxSwap,
            Some(MountPoint::Swap),
        );
        vec![
            ("manual-uefi-empty", vec![good_fresh()], Firmware::Uefi),
            (
                "manual-uefi-new-table-over-used",
                vec![made(
                    fresh(windows(), Table::Gpt),
                    1,
                    100 * 1024,
                    Fs::Btrfs,
                    Some(MountPoint::Root),
                )],
                Firmware::Uefi,
            ),
            (
                "manual-uefi-beside-windows",
                vec![windows_replaced()],
                Firmware::Uefi,
            ),
            (
                "manual-uefi-three-disks",
                vec![good_fresh(), linux_home()],
                Firmware::Uefi,
            ),
            ("manual-bios-gpt", vec![bios_fresh()], Firmware::Bios),
            (
                "manual-bios-msdos-keep-fat",
                vec![made(
                    kept(old()),
                    102_404,
                    32 * 1024,
                    Fs::Ext4,
                    Some(MountPoint::Root),
                )],
                Firmware::Bios,
            ),
            ("manual-bios-msdos-logical", vec![logical], Firmware::Bios),
            (
                "manual-uefi-shrink-ntfs",
                vec![windows_shrunk()],
                Firmware::Uefi,
            ),
            (
                "manual-uefi-shrink-ext4",
                vec![good_fresh(), linux_shrunk()],
                Firmware::Uefi,
            ),
            (
                "manual-uefi-reinstall-keep-home",
                vec![reinstalled],
                Firmware::Uefi,
            ),
            (
                "manual-bios-msdos-windows-keep-home",
                vec![beside_windows_mbr],
                Firmware::Bios,
            ),
        ]
    }

    /// Every set of [`CAMPAIGN`], as `PROVEN` holds them once every
    /// scenario has passed.
    fn the_campaigns_sets() -> &'static [&'static [Capability]] {
        CAMPAIGN
            .iter()
            .map(|(_, set)| *set)
            .collect::<Vec<_>>()
            .leak()
    }

    /// `sets` less every set holding `out`: `PROVEN` were each scenario
    /// that needs `out` to fail.
    fn sets_without(
        sets: &'static [&'static [Capability]],
        out: Capability,
    ) -> &'static [&'static [Capability]] {
        sets.iter()
            .copied()
            .filter(|set| !set.contains(&out))
            .collect::<Vec<_>>()
            .leak()
    }

    /// What every layout that needs `one` needs with it: a logical
    /// partition stands on an msdos table, and one deleted or grown is a
    /// logical partition written; the first logical partition deleted is a
    /// logical partition deleted; the extended frame is deleted with every
    /// logical partition in it, a deletion on a kept table; `/` or `/boot`
    /// on a logical partition, and the boot flag set, stand on an msdos
    /// table; a `/boot` under BIOS is under BIOS; and an entry made smaller
    /// or grown, primary or logical, is a partition's formatted.
    fn implied(one: Capability) -> &'static [Capability] {
        use Capability::*;
        match one {
            Logical => &[Msdos],
            DeleteLogical => &[Logical],
            LargerLogical => &[ChangeEntries, Logical],
            EmptyFrame => &[DeleteLogical],
            Extended => &[DeleteEntries, Logical],
            RootOnLogical | Activate => &[Msdos],
            BootOnBios => &[Bios],
            SmallerEntry | LargerEntry => &[ChangeEntries],
            _ => &[],
        }
    }

    /// The least mixes no one of `sets` holds whole: every set of
    /// capabilities a machine can need — never `Uefi` beside `Bios` — that
    /// lies within none of `sets` while everything one capability fewer
    /// does; less each of them that no layout can need without another of
    /// them, by what it brings with it ([`implied`]: a logical partition
    /// under UEFI is msdos under UEFI). A layout is refused by `sets`
    /// exactly when what it needs holds one of them. In [`Capability`]'s
    /// order, the fewer first.
    fn least_mixes_held_by_none(sets: &[&[Capability]]) -> Vec<Vec<Capability>> {
        use Capability::*;
        let all = EVERY_CAPABILITY;
        let bit = |one: &Capability| -> u32 {
            1 << all.iter().position(|each| each == one).expect("listed")
        };
        let masks: Vec<u32> = sets
            .iter()
            .map(|set| set.iter().map(bit).fold(0, |m, b| m | b))
            .collect();
        let held = |bits: u32| masks.iter().any(|mask| bits & !mask == 0);
        let brought = |mut bits: u32| loop {
            let more = (0..all.len())
                .filter(|at| bits & (1 << at) != 0)
                .flat_map(|at| implied(all[at]).iter().map(bit))
                .fold(bits, |m, b| m | b);
            if more == bits {
                break bits;
            }
            bits = more;
        };
        let both = bit(&Uefi) | bit(&Bios);
        let plain: Vec<u32> = (1u32..1 << all.len())
            .filter(|bits| bits & both != both && !held(*bits))
            .filter(|bits| {
                (0..all.len())
                    .filter(|at| bits & (1 << at) != 0)
                    .all(|at| held(bits & !(1 << at)))
            })
            .collect();
        let mut least: Vec<Vec<Capability>> = plain
            .iter()
            .filter(|bits| {
                let with = brought(**bits);
                !plain
                    .iter()
                    .any(|other| other != *bits && other & !with == 0)
            })
            .map(|bits| {
                (0..all.len())
                    .filter(|at| bits & (1 << at) != 0)
                    .map(|at| all[at])
                    .collect()
            })
            .collect();
        least.sort_by(|one, other| one.len().cmp(&other.len()).then_with(|| one.cmp(other)));
        least
    }

    /// **Only a mix one scenario that passed ran whole goes on** (decision
    /// 20: only mixes a QEMU test actually ran are allowed). With `PROVEN`
    /// itself — every one of the campaign's eleven scenarios that carry a
    /// plan out passed, the two decision 20 added among them — each
    /// scenario's plan goes on, by its own set alone too, and so does a plan
    /// needing less than one set; while the fifty least mixes no one set
    /// holds — a primary partition's entry grown, the first logical partition
    /// deleted, `/` or `/boot` on a logical partition, a `/boot` under BIOS
    /// and a disk whose logical sector is not 512 bytes, which no set holds
    /// at all, and the thirty pairs and fifteen triples §7.0's campaign
    /// names, a `/home` kept or a Windows shrunk on an MBR disk where the
    /// plan sets the boot flag, a new msdos table, several disks under BIOS,
    /// msdos under UEFI, an msdos disk beside a GPT one, the kept GPT disk
    /// under BIOS, an ext4 shrunk on msdos, a logical partition written
    /// beside a kept `/home`, a primary partition deleted under BIOS or on
    /// msdos and a formatted partition's entry made smaller on msdos among
    /// them — are each refused in a plan that needs one, though every other
    /// capability they need is in some set and judged alone would have let
    /// them through. Pretended proven by nothing, as were no set written,
    /// every one of these plans is refused, a plan that writes nothing too.
    #[test]
    fn only_a_mix_one_passing_scenario_ran_goes_on() {
        use Capability::*;
        let sets = the_campaigns_sets();
        let plans = the_campaigns_plans();
        let names: Vec<&str> = plans.iter().map(|(name, _, _)| *name).collect();
        let named: Vec<&str> = CAMPAIGN.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, named);
        // A partition formatted beside a kept Windows shrunk.
        let shrunk_and_formatted = || {
            vec![with(
                windows_shrunk(),
                4,
                format(Fs::Ext4, 750 * MIB, other("/data")),
            )]
        };
        // A kept /home on a GPT disk with a partition made beside it.
        let home_beside_a_making = || {
            vec![made(
                linux_home(),
                125 * 1024,
                2 * 1024,
                Fs::Ext4,
                other("/srv"),
            )]
        };
        let shrunk = |one: DiskLayout, number: u32, to: u64| {
            with(one, number, Fate::Shrink { to, mount: None })
        };
        // The Linux disk's swap deleted; its root formatted smaller.
        let linux_deleted = || with(kept(linux()), 3, Fate::Delete);
        let root_smaller =
            |one: DiskLayout| with(one, 2, format(Fs::Ext4, 30 * GIB, Some(MountPoint::Root)));
        let refused = [
            // What no set holds at all: a primary partition's entry grown
            // (the logical scenario grows a logical one's), and the first
            // logical partition deleted, the frame left empty (the logical
            // scenario deletes the last of three) — the extended frame's
            // deletion among it, which is a primary entry deleted on msdos
            // too.
            (
                vec![LargerEntry],
                vec![primary_grown_on_msdos()],
                Firmware::Bios,
            ),
            (
                vec![EmptyFrame],
                vec![every_logical_deleted()],
                Firmware::Bios,
            ),
            // `/` on a logical partition, Windows' primary deleted beside it.
            (
                vec![RootOnLogical],
                vec![windows_deleted_beside_a_logical_root()],
                Firmware::Bios,
            ),
            // A `/boot` under BIOS: keep-FAT's disk, a FAT32 `/boot` made
            // before `/` (every BIOS scenario keeps its kernels on `/`).
            (
                vec![BootOnBios],
                vec![boot_before_root_on_bios()],
                Firmware::Bios,
            ),
            // A disk of 4096-byte sectors: the beside-Windows scenario's
            // plan there (every scenario's disks had 512-byte ones).
            (
                vec![LargeSectors],
                vec![on_large_sectors(windows_replaced())],
                Firmware::Uefi,
            ),
            // The thirty pairs. A new table on one disk, a partition
            // deleted on another.
            (
                vec![FreshTable, DeleteEntries],
                vec![good_fresh(), linux_deleted()],
                Firmware::Uefi,
            ),
            // A new table on one disk and an existing partition formatted
            // on another.
            (
                vec![FreshTable, ChangeEntries],
                vec![good_fresh(), windows_formatted()],
                Firmware::Uefi,
            ),
            // A new msdos table holding `/`: its script marks `/` bootable
            // (`manual::plan::script`), which no scenario's new table does.
            (vec![FreshTable, Msdos], vec![msdos_fresh()], Firmware::Bios),
            // A new table on one disk, Windows shrunk on another.
            (
                vec![FreshTable, ShrinkNtfs],
                vec![good_fresh(), windows_shrunk()],
                Firmware::Uefi,
            ),
            // A partition made beside a logical partition written.
            (
                vec![KeepTable, Logical],
                vec![old_deleted(), bios_beside_windows()],
                Firmware::Bios,
            ),
            // A partition deleted beside a kept /home.
            (
                vec![DeleteEntries, KeptHome],
                vec![with(linux_home(), 3, Fate::Delete)],
                Firmware::Uefi,
            ),
            (
                vec![DeleteEntries, SeveralDisks],
                vec![with(kept(windows()), 4, Fate::Delete), linux_data()],
                Firmware::Uefi,
            ),
            // A primary partition deleted under BIOS, and on msdos: the old
            // home on the MBR disk deleted, its old root formatted as `/`;
            // Windows deleted beside a logical partition formatted.
            (
                vec![DeleteEntries, Bios],
                vec![primary_deleted_on_msdos()],
                Firmware::Bios,
            ),
            (
                vec![DeleteEntries, Msdos],
                vec![windows_deleted_beside_a_logical_one()],
                Firmware::Bios,
            ),
            // A partition deleted beside an ext4 shrunk, and beside an NTFS.
            (
                vec![DeleteEntries, ShrinkExt4],
                vec![with(ext4_shrunk(), 3, Fate::Delete)],
                Firmware::Uefi,
            ),
            (
                vec![DeleteEntries, ShrinkNtfs],
                vec![with(ntfs_shrunk(), 4, Fate::Delete)],
                Firmware::Uefi,
            ),
            (
                vec![ChangeEntries, SeveralDisks],
                vec![windows_formatted(), linux_data()],
                Firmware::Uefi,
            ),
            // An ext4 shrunk beside a partition formatted.
            (
                vec![ChangeEntries, ShrinkExt4],
                vec![with(
                    ext4_shrunk(),
                    3,
                    format(Fs::LinuxSwap, 4 * GIB, Some(MountPoint::Swap)),
                )],
                Firmware::Uefi,
            ),
            // An old root formatted smaller beside a kept /home.
            (
                vec![SmallerEntry, KeptHome],
                vec![root_smaller(linux_home())],
                Firmware::Uefi,
            ),
            // An entry made smaller under BIOS, and on msdos.
            (
                vec![SmallerEntry, Bios],
                vec![root_smaller(kept(linux()))],
                Firmware::Bios,
            ),
            (
                vec![SmallerEntry, Msdos],
                vec![with(
                    kept(windows_mbr()),
                    2,
                    format(Fs::Ext4, 20 * GIB, Some(MountPoint::Root)),
                )],
                Firmware::Bios,
            ),
            // An entry made smaller beside Windows shrunk.
            (
                vec![SmallerEntry, ShrinkNtfs],
                vec![with(
                    windows_shrunk(),
                    4,
                    format(Fs::Ext4, 500 * MIB, other("/data")),
                )],
                Firmware::Uefi,
            ),
            // A logical partition deleted beside a kept /home.
            (
                vec![KeptHome, Logical],
                vec![with(
                    with(
                        kept(old()),
                        5,
                        Fate::Keep {
                            mount: Some(MountPoint::Home),
                        },
                    ),
                    7,
                    Fate::Delete,
                )],
                Firmware::Bios,
            ),
            // The reinstall of the BIOS keep-home scenario on an MBR disk
            // where nothing is marked: the boot flag set beside a kept /home.
            (
                vec![KeptHome, Activate],
                vec![with(
                    with(
                        kept(bare_mbr()),
                        2,
                        format(Fs::Ext4, 28 * GIB, Some(MountPoint::Root)),
                    ),
                    3,
                    Fate::Keep {
                        mount: Some(MountPoint::Home),
                    },
                )],
                Firmware::Bios,
            ),
            // An ext4 shrunk beside a kept /home.
            (
                vec![KeptHome, ShrinkExt4],
                vec![shrunk(linux_home(), 2, 30 * GIB)],
                Firmware::Uefi,
            ),
            // Several disks under BIOS, and on msdos.
            (
                vec![SeveralDisks, Bios],
                vec![bios_fresh(), linux_data()],
                Firmware::Bios,
            ),
            (
                vec![SeveralDisks, Msdos],
                vec![good_fresh(), old_mounted()],
                Firmware::Uefi,
            ),
            // Windows shrunk beside another disk.
            (
                vec![SeveralDisks, ShrinkNtfs],
                vec![ntfs_shrunk(), linux_data()],
                Firmware::Uefi,
            ),
            // Msdos under UEFI, even a disk only mounted from.
            (vec![Uefi, Msdos], vec![old_mounted()], Firmware::Uefi),
            // An ext4 shrunk under BIOS.
            (vec![Bios, ShrinkExt4], vec![ext4_shrunk()], Firmware::Bios),
            // An msdos disk beside a GPT one.
            (
                vec![Gpt, Msdos],
                vec![bios_fresh(), old_deleted()],
                Firmware::Bios,
            ),
            // An ext4 shrunk on an msdos disk.
            (
                vec![Msdos, ShrinkExt4],
                vec![shrunk(kept(windows_mbr()), 2, 20 * GIB)],
                Firmware::Bios,
            ),
            // Windows shrunk beside a logical partition deleted.
            (
                vec![Logical, ShrinkNtfs],
                vec![with(shrunk(kept(old()), 1, 30 * GIB), 7, Fate::Delete)],
                Firmware::Bios,
            ),
            // Windows shrunk on an MBR disk where nothing is marked, `/` made
            // in what it gives up: the boot flag set beside the shrink.
            (
                vec![Activate, ShrinkNtfs],
                vec![made(
                    shrunk(kept(bare_mbr()), 1, 28 * GIB),
                    28_673,
                    4 * 1024,
                    Fs::Ext4,
                    Some(MountPoint::Root),
                )],
                Firmware::Bios,
            ),
            // An ext4 and an NTFS shrunk.
            (
                vec![ShrinkExt4, ShrinkNtfs],
                vec![ext4_shrunk(), ntfs_shrunk()],
                Firmware::Uefi,
            ),
            // The fifteen triples, each two of which some scenario runs
            // but none all three.
            (
                vec![FreshTable, KeepTable, KeptHome],
                [vec![good_fresh()], home_beside_a_making()].concat(),
                Firmware::Uefi,
            ),
            (
                vec![FreshTable, KeepTable, Bios],
                vec![bios_fresh(), bios_beside_windows()],
                Firmware::Bios,
            ),
            (
                vec![FreshTable, KeptHome, Bios],
                vec![bios_fresh(), linux_home()],
                Firmware::Bios,
            ),
            // A partition made beside the old root formatted as `/` on an
            // MBR disk where nothing is marked: the boot flag set on it.
            (
                vec![KeepTable, ChangeEntries, Activate],
                vec![made(
                    with(
                        kept(old()),
                        1,
                        format(Fs::Ext4, 40 * GIB, Some(MountPoint::Root)),
                    ),
                    102_404,
                    4 * 1024,
                    Fs::LinuxSwap,
                    Some(MountPoint::Swap),
                )],
                Firmware::Bios,
            ),
            (
                vec![KeepTable, KeptHome, SeveralDisks],
                vec![linux_home(), good_beside_windows()],
                Firmware::Uefi,
            ),
            (
                vec![KeepTable, KeptHome, Uefi],
                home_beside_a_making(),
                Firmware::Uefi,
            ),
            (
                vec![KeepTable, KeptHome, Gpt],
                home_beside_a_making(),
                Firmware::Uefi,
            ),
            // The kept GPT disk under BIOS.
            (
                vec![KeepTable, Bios, Gpt],
                vec![bios_beside_windows()],
                Firmware::Bios,
            ),
            (
                vec![ChangeEntries, Uefi, ShrinkNtfs],
                shrunk_and_formatted(),
                Firmware::Uefi,
            ),
            // An old Linux formatted on a GPT disk under BIOS.
            (
                vec![ChangeEntries, Bios, Gpt],
                vec![with(
                    kept(linux()),
                    2,
                    format(Fs::Ext4, 40 * GIB, Some(MountPoint::Root)),
                )],
                Firmware::Bios,
            ),
            (
                vec![ChangeEntries, Gpt, ShrinkNtfs],
                shrunk_and_formatted(),
                Firmware::Uefi,
            ),
            (
                vec![KeptHome, Uefi, ShrinkNtfs],
                vec![linux_home(), windows_shrunk()],
                Firmware::Uefi,
            ),
            (
                vec![KeptHome, Bios, Gpt],
                vec![bios_fresh(), linux_home()],
                Firmware::Bios,
            ),
            (
                vec![KeptHome, Gpt, ShrinkNtfs],
                vec![linux_home(), windows_shrunk()],
                Firmware::Uefi,
            ),
            // A Windows shrunk on a GPT disk under BIOS.
            (
                vec![Bios, Gpt, ShrinkNtfs],
                vec![ntfs_shrunk()],
                Firmware::Bios,
            ),
        ];
        let least: Vec<Vec<Capability>> = refused.iter().map(|(mix, _, _)| mix.clone()).collect();
        assert_eq!(least_mixes_held_by_none(sets), least);
        assert_eq!(least_mixes_held_by_none(PROVEN), least);
        let within = [
            (vec![good_beside_windows()], Firmware::Uefi),
            (vec![good_fresh(), good_fresh()], Firmware::Uefi),
            // A logical partition formatted, and one deleted, under BIOS.
            (
                vec![with(
                    kept(old()),
                    5,
                    format(Fs::Ext4, 30 * GIB, other("/data")),
                )],
                Firmware::Bios,
            ),
            (vec![old_deleted()], Firmware::Bios),
            (vec![linux_data()], Firmware::Uefi),
            // Each filesystem shrunk alone, on GPT under UEFI.
            (vec![ext4_shrunk()], Firmware::Uefi),
            (vec![ntfs_shrunk()], Firmware::Uefi),
            // Partitions made and deleted on GPT under UEFI; an entry made
            // smaller there; a logical partition's entry grown on msdos
            // under BIOS. (A primary partition deleted or grown on msdos
            // under BIOS is refused above: the logical scenario deletes and
            // grows only logical ones.)
            (
                vec![with(good_beside_windows(), 4, Fate::Delete)],
                Firmware::Uefi,
            ),
            (vec![root_smaller(kept(linux()))], Firmware::Uefi),
            (vec![logical_grown()], Firmware::Bios),
            // What decision 20's two open beyond their own plans: a home kept
            // alone under UEFI; a home kept on msdos and a Windows shrunk on
            // msdos, each under BIOS.
            (vec![linux_home()], Firmware::Uefi),
            (
                vec![with(
                    kept(old()),
                    5,
                    Fate::Keep {
                        mount: Some(MountPoint::Home),
                    },
                )],
                Firmware::Bios,
            ),
            (vec![shrunk(kept(old()), 1, 30 * GIB)], Firmware::Bios),
        ];
        // Judged by `PROVEN` itself, as a build that runs judges a plan.
        {
            for ((name, layout, firmware), (_, set)) in plans.iter().zip(CAMPAIGN) {
                assert_eq!(needs(layout, *firmware), *set, "{name}");
                assert_eq!(unproven(layout, *firmware), None, "{name}");
                let _own = PretendProven::these(std::slice::from_ref(set));
                assert_eq!(unproven(layout, *firmware), None, "{name} by its own set");
            }
            for (layout, firmware) in &within {
                let needed = needs(layout, *firmware);
                assert!(PROVEN
                    .iter()
                    .any(|set| needed.iter().all(|one| set.contains(one))
                        && set.len() > needed.len()));
                assert_eq!(unproven(layout, *firmware), None, "{needed:?}");
            }
            for (mix, layout, firmware) in &refused {
                let needed = needs(layout, *firmware);
                assert!(
                    mix.iter().all(|one| needed.contains(one)),
                    "{mix:?}: {needed:?}"
                );
                let held_by_none: Vec<Capability> = needed
                    .iter()
                    .copied()
                    .filter(|one| !PROVEN.iter().any(|set| set.contains(one)))
                    .collect();
                if mix.len() == 1 {
                    // No set holds it: refused for it alone.
                    assert_eq!(held_by_none, *mix, "{mix:?}");
                    assert_eq!(
                        unproven(layout, *firmware),
                        Some(Unproven::Never(mix[0])),
                        "{mix:?}"
                    );
                    continue;
                }
                // Each capability alone is in some set: judged one by one,
                // as before decision 20, it would have gone on.
                assert!(held_by_none.is_empty(), "{mix:?}: {held_by_none:?}");
                assert_eq!(
                    unproven(layout, *firmware),
                    Some(Unproven::Together(needed.clone())),
                    "{mix:?}"
                );
            }
        }
        // Pretended proven by nothing, as were no set written, every one of
        // them is refused, by the first capability it needs.
        let _none = PretendProven::these(&[]);
        let every = plans
            .into_iter()
            .map(|(_, layout, firmware)| (layout, firmware))
            .chain(
                refused
                    .into_iter()
                    .map(|(_, layout, firmware)| (layout, firmware)),
            )
            .chain(within);
        for (layout, firmware) in every {
            let first = needs(&layout, firmware)[0];
            assert_eq!(
                unproven(&layout, firmware),
                Some(Unproven::Never(first)),
                "{layout:?}"
            );
        }
    }

    /// Whether every one of `proven` is the exact set a scenario of the
    /// campaign needed: what `PROVEN` may hold, and nothing else.
    fn named_by_the_campaign(proven: &[&[Capability]]) -> bool {
        proven
            .iter()
            .all(|set| CAMPAIGN.iter().any(|(_, needed)| needed == set))
    }

    /// **`PROVEN` holds only sets the campaign's scenarios need, and the set
    /// of every one that passed** (decision 20): each set the campaign may
    /// write in is the exact need of one of its scenarios, in `Capability`'s
    /// order, each capability once, with exactly one firmware and at least
    /// one kind of table; a set no scenario needed, or one put together from
    /// two, is not one of them; and `PROVEN` holds, once each, the sets of
    /// the eleven scenarios that carry a plan out — every one passed in
    /// QEMU — which are ten, the two new gpts under UEFI needing one, in the
    /// campaign's order.
    #[test]
    fn every_set_proven_is_the_exact_need_of_a_scenario_of_the_campaign() {
        use Capability::*;
        let mut passed: Vec<&[Capability]> = Vec::new();
        for (_, set) in CAMPAIGN {
            if !passed.contains(set) {
                passed.push(set);
            }
        }
        assert_eq!(PROVEN, passed.as_slice());
        assert_eq!(PROVEN.len(), 10);
        assert_eq!(CAMPAIGN.len(), 11);
        assert!(named_by_the_campaign(PROVEN));
        assert!(named_by_the_campaign(the_campaigns_sets()));
        for (name, set) in CAMPAIGN {
            assert!(set.windows(2).all(|two| two[0] < two[1]), "{name}");
            let firmware = set.iter().filter(|one| matches!(one, Uefi | Bios)).count();
            assert_eq!(firmware, 1, "{name}");
            assert!(set.iter().any(|one| [Gpt, Msdos].contains(one)), "{name}");
        }
        assert!(!named_by_the_campaign(&[&[KeepTable, Bios, Gpt]]));
        assert!(!named_by_the_campaign(&[&[
            FreshTable,
            KeepTable,
            ChangeEntries,
            KeptHome,
            SeveralDisks,
            Uefi,
            Gpt,
            ShrinkNtfs
        ]]));
        assert!(!named_by_the_campaign(&[&[KeepTable, Uefi, Gpt]]));
        // A shrink scenario's set with the other filesystem: no scenario's.
        assert!(!named_by_the_campaign(&[&[
            KeepTable, Uefi, Gpt, ShrinkExt4
        ]]));
        // The logical scenario's set without Logical, with Extended or the
        // frame left empty, with partitions made where it only deleted, or
        // with a primary partition's deletion and entry grown for the
        // logical one's it ran (the set before the kind was counted).
        assert!(!named_by_the_campaign(&[&[
            DeleteLogical,
            ChangeEntries,
            LargerLogical,
            Bios,
            Msdos
        ]]));
        assert!(!named_by_the_campaign(&[&[
            DeleteLogical,
            ChangeEntries,
            LargerLogical,
            Bios,
            Msdos,
            Logical,
            Extended
        ]]));
        assert!(!named_by_the_campaign(&[&[
            DeleteLogical,
            ChangeEntries,
            LargerLogical,
            Bios,
            Msdos,
            Logical,
            EmptyFrame
        ]]));
        assert!(!named_by_the_campaign(&[&[
            KeepTable,
            DeleteLogical,
            ChangeEntries,
            LargerLogical,
            Bios,
            Msdos,
            Logical
        ]]));
        assert!(!named_by_the_campaign(&[&[
            DeleteEntries,
            ChangeEntries,
            LargerEntry,
            Bios,
            Msdos,
            Logical
        ]]));
        // The beside-Windows scenario's set with its entry grown rather
        // than made smaller.
        assert!(!named_by_the_campaign(&[&[
            KeepTable,
            DeleteEntries,
            ChangeEntries,
            LargerEntry,
            Uefi,
            Gpt
        ]]));
        // No scenario deletes an extended frame, nor leaves one empty.
        assert!(CAMPAIGN
            .iter()
            .all(|(_, set)| !set.contains(&Extended) && !set.contains(&EmptyFrame)));
        // An entry resized is always a partition formatted, and a logical
        // partition deleted or grown a logical partition written.
        for (name, set) in CAMPAIGN {
            if [SmallerEntry, LargerEntry, LargerLogical]
                .iter()
                .any(|one| set.contains(one))
            {
                assert!(set.contains(&ChangeEntries), "{name}");
            }
            if set.contains(&DeleteLogical) || set.contains(&LargerLogical) {
                assert!(set.contains(&Logical), "{name}");
            }
        }
    }

    /// **Partitions made and deleted, and a formatted partition's entry
    /// made smaller or grown, are each proven apart, and a new table counts
    /// the table it writes over.** Were one KeepTable to hold a deletion and
    /// a making, the BIOS scenario that only makes partitions on an MBR disk
    /// would open a primary deleted and another made in its place, which no
    /// scenario runs; were one ChangeEntries to hold an entry grown, which
    /// only the logical scenario grows (on msdos under BIOS), and one made
    /// smaller, which only the beside-Windows scenario shrinks (on GPT under
    /// UEFI), each would be opened where the other ran; and a new GPT over
    /// an msdos disk would need nothing of msdos. With the campaign's sets
    /// pretended proven each of those is refused as a mix no scenario ran,
    /// while each part goes on where its scenario ran it; with the sets of
    /// the scenarios that run a part left out, that part is refused alone,
    /// in its capability's words. *(Since the kind of partition is counted
    /// too, the logical scenario's deletion and entry grown are a logical
    /// partition's, `DeleteLogical` and `LargerLogical`: a primary partition
    /// deleted on msdos, or its entry grown there, is refused as well —
    /// `a_deletion_or_an_entry_grown_is_proven_by_the_kind_of_partition_it_ran_on`.)*
    #[test]
    fn a_deletion_and_a_resized_entry_are_proven_apart_and_a_new_table_counts_what_it_writes_over()
    {
        use Capability::*;
        let sets = the_campaigns_sets();
        let entry = |disk: Disk, number: u32, fs: Fs, size: u64, at: MountPoint| {
            vec![with(kept(disk), number, format(fs, size, Some(at)))]
        };
        // The old home on the MBR disk deleted, alone or with a swap
        // partition made in its place.
        let deleted_on_msdos = vec![with(kept(windows_mbr()), 3, Fate::Delete)];
        let replaced_on_msdos = vec![made(
            deleted_on_msdos[0].clone(),
            61_441,
            4095,
            Fs::LinuxSwap,
            Some(MountPoint::Swap),
        )];
        // A partition made on the old msdos disk, as the keep-FAT scenario
        // makes them; and Windows' recovery partition deleted beside the
        // partitions made on its GPT.
        let made_on_msdos = vec![made(
            kept(old()),
            102_404,
            32 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        )];
        let replaced_on_gpt = vec![with(good_beside_windows(), 4, Fate::Delete)];
        // Entries grown and made smaller, on GPT and on msdos.
        let grown_on_gpt = entry(linux(), 3, Fs::LinuxSwap, 8 * GIB, MountPoint::Swap);
        let smaller_on_gpt = entry(linux(), 2, Fs::Ext4, 30 * GIB, MountPoint::Root);
        let grown_on_msdos = entry(windows_mbr(), 2, Fs::Ext4, 30 * GIB, MountPoint::Root);
        let smaller_on_msdos = entry(windows_mbr(), 2, Fs::Ext4, 20 * GIB, MountPoint::Root);
        let smaller_beside_a_home = vec![with(
            linux_home(),
            2,
            format(Fs::Ext4, 30 * GIB, Some(MountPoint::Root)),
        )];
        // A new GPT over the old msdos disk, and over Windows' GPT.
        let root = |one: DiskLayout| made(one, 1, 100 * 1024, Fs::Ext4, Some(MountPoint::Root));
        let gpt_over_msdos = vec![root(fresh(old(), Table::Gpt))];
        let gpt_over_gpt = vec![root(fresh(windows(), Table::Gpt))];
        assert_eq!(
            needs(&deleted_on_msdos, Firmware::Bios),
            [DeleteEntries, Bios, Msdos]
        );
        assert_eq!(
            needs(&replaced_on_msdos, Firmware::Bios),
            [KeepTable, DeleteEntries, Bios, Msdos]
        );
        assert_eq!(
            needs(&made_on_msdos, Firmware::Bios),
            [KeepTable, Bios, Msdos, Activate]
        );
        assert_eq!(
            needs(&replaced_on_gpt, Firmware::Uefi),
            [KeepTable, DeleteEntries, Uefi, Gpt]
        );
        assert_eq!(
            needs(&grown_on_gpt, Firmware::Uefi),
            [ChangeEntries, LargerEntry, Uefi, Gpt]
        );
        assert_eq!(
            needs(&smaller_on_gpt, Firmware::Uefi),
            [ChangeEntries, SmallerEntry, Uefi, Gpt]
        );
        assert_eq!(
            needs(&grown_on_msdos, Firmware::Bios),
            [ChangeEntries, LargerEntry, Bios, Msdos]
        );
        assert_eq!(
            needs(&smaller_on_msdos, Firmware::Bios),
            [ChangeEntries, SmallerEntry, Bios, Msdos]
        );
        assert_eq!(
            needs(&smaller_beside_a_home, Firmware::Uefi),
            [ChangeEntries, SmallerEntry, KeptHome, Uefi, Gpt]
        );
        // Formatted at its own size: no entry resized.
        assert_eq!(
            needs(
                &entry(linux(), 2, Fs::Ext4, 40 * GIB, MountPoint::Root),
                Firmware::Uefi
            ),
            [ChangeEntries, Uefi, Gpt]
        );
        assert_eq!(
            needs(&gpt_over_msdos, Firmware::Uefi),
            [FreshTable, Uefi, Gpt, Msdos]
        );
        assert_eq!(
            needs(&gpt_over_gpt, Firmware::Uefi),
            [FreshTable, Uefi, Gpt]
        );
        assert_eq!(
            needs(&[root(fresh(blank(), Table::Gpt))], Firmware::Uefi),
            [FreshTable, Uefi, Gpt]
        );
        // A logical partition's entry grown, as the logical scenario grows
        // one.
        let grown_logical = vec![logical_grown()];
        assert_eq!(
            needs(&grown_logical, Firmware::Bios),
            [ChangeEntries, LargerLogical, Bios, Msdos, Logical]
        );
        {
            let _campaign = PretendProven::these(sets);
            for (layout, firmware) in [
                (&replaced_on_msdos, Firmware::Bios),
                (&deleted_on_msdos, Firmware::Bios),
                (&smaller_on_msdos, Firmware::Bios),
                (&smaller_beside_a_home, Firmware::Uefi),
                (&gpt_over_msdos, Firmware::Uefi),
                (&gpt_over_msdos, Firmware::Bios),
            ] {
                assert_eq!(
                    unproven(layout, firmware),
                    Some(Unproven::Together(needs(layout, firmware))),
                    "{firmware:?} {layout:?}"
                );
            }
            // A primary partition's entry, or a GPT one's, grown: no
            // scenario grows one.
            for (layout, firmware) in [
                (&grown_on_gpt, Firmware::Uefi),
                (&grown_on_msdos, Firmware::Bios),
            ] {
                assert_eq!(
                    unproven(layout, firmware),
                    Some(Unproven::Never(LargerEntry)),
                    "{firmware:?} {layout:?}"
                );
            }
            for (layout, firmware) in [
                (&made_on_msdos, Firmware::Bios),
                (&replaced_on_gpt, Firmware::Uefi),
                (&smaller_on_gpt, Firmware::Uefi),
                (&grown_logical, Firmware::Bios),
                (&gpt_over_gpt, Firmware::Uefi),
            ] {
                assert_eq!(unproven(layout, firmware), None, "{firmware:?} {layout:?}");
            }
        }
        {
            let _no_deletion = PretendProven::these(sets_without(sets, DeleteEntries));
            assert_eq!(
                unproven(&replaced_on_gpt, Firmware::Uefi),
                Some(Unproven::Never(DeleteEntries))
            );
            assert_eq!(
                unproven(&deleted_on_msdos, Firmware::Bios),
                Some(Unproven::Never(DeleteEntries))
            );
            assert_eq!(unproven(&made_on_msdos, Firmware::Bios), None);
        }
        {
            let _no_smaller = PretendProven::these(sets_without(sets, SmallerEntry));
            assert_eq!(
                unproven(&smaller_on_gpt, Firmware::Uefi),
                Some(Unproven::Never(SmallerEntry))
            );
            assert_eq!(unproven(&grown_logical, Firmware::Bios), None);
        }
        {
            let _no_larger = PretendProven::these(sets_without(sets, LargerLogical));
            assert_eq!(
                unproven(&grown_logical, Firmware::Bios),
                Some(Unproven::Never(LargerLogical))
            );
            assert_eq!(unproven(&smaller_on_gpt, Firmware::Uefi), None);
        }
    }

    /// **A deletion or an entry grown is proven by the kind of partition it
    /// ran on.** Were `Logical` counted once for the whole plan, beside a
    /// `DeleteEntries` and a `LargerEntry` that did not say which partition
    /// they were done to, the logical scenario's set — the last of three
    /// logical partitions deleted, one grown, a primary one formatted where
    /// it stands — would let through work on an MBR disk under BIOS that no
    /// scenario runs: a primary partition deleted, one grown, Windows'
    /// primary partition deleted beside a logical one written (and beside
    /// `/` there, which leaves nothing marked active and is refused for that
    /// alone since `RootOnLogical` is counted); and every logical partition
    /// deleted down to the first, the frame left empty, which libfdisk does
    /// by another branch than the last one's. Each is refused with the
    /// campaign's sets pretended proven — a primary partition deleted as a
    /// mix no scenario ran, a primary's entry grown and the frame left
    /// empty as what none ran at all — while what the logical scenario ran,
    /// and less, still goes on: logical partitions deleted from the last
    /// but not the first, one grown, one or a primary formatted where it
    /// stands. And `Logical`, still asked beside a format in place, an
    /// entry made smaller and a shrink, pairs in each set that holds it
    /// only what that scenario ran on a logical partition and on another.
    #[test]
    fn a_deletion_or_an_entry_grown_is_proven_by_the_kind_of_partition_it_ran_on() {
        use Capability::*;
        let sets = the_campaigns_sets();
        let primary_grown = vec![primary_grown_on_msdos()];
        let primary_deleted = vec![primary_deleted_on_msdos()];
        let windows_deleted = vec![windows_deleted_beside_a_logical_one()];
        let emptied = vec![every_logical_deleted()];
        assert_eq!(
            needs(&primary_grown, Firmware::Bios),
            [DeleteEntries, ChangeEntries, LargerEntry, Bios, Msdos]
        );
        assert_eq!(
            needs(&primary_deleted, Firmware::Bios),
            [DeleteEntries, ChangeEntries, Bios, Msdos]
        );
        assert_eq!(
            needs(&windows_deleted, Firmware::Bios),
            [DeleteEntries, ChangeEntries, Bios, Msdos, Logical]
        );
        assert_eq!(
            needs(&emptied, Firmware::Bios),
            [
                DeleteLogical,
                ChangeEntries,
                Bios,
                Msdos,
                Logical,
                EmptyFrame,
                Activate
            ]
        );
        // What the logical scenario runs, and its parts: the last logical
        // partition deleted, and the one before it too (still not the
        // first); a logical partition grown; one formatted at its own size;
        // the primary partition formatted.
        let last_deleted = vec![old_deleted()];
        let two_deleted = vec![with(old_deleted(), 6, Fate::Delete)];
        let grown = vec![logical_grown()];
        let logical_formatted = vec![with(
            kept(old()),
            5,
            format(Fs::Ext4, 30 * GIB, other("/data")),
        )];
        let primary_formatted = vec![with(
            kept(old()),
            1,
            format(Fs::Ext4, 40 * GIB, other("/data")),
        )];
        for deleted in [&last_deleted, &two_deleted] {
            assert_eq!(
                needs(deleted, Firmware::Bios),
                [DeleteLogical, Bios, Msdos, Logical]
            );
        }
        assert_eq!(
            needs(&grown, Firmware::Bios),
            [ChangeEntries, LargerLogical, Bios, Msdos, Logical]
        );
        // Under a new table nothing is a logical partition's: everything
        // goes at once.
        assert_eq!(
            needs(&[fresh(old(), Table::Msdos)], Firmware::Bios),
            [FreshTable, Bios, Msdos]
        );
        {
            let _campaign = PretendProven::these(sets);
            for layout in [&primary_deleted, &windows_deleted] {
                assert_eq!(
                    unproven(layout, Firmware::Bios),
                    Some(Unproven::Together(needs(layout, Firmware::Bios))),
                    "{layout:?}"
                );
            }
            assert_eq!(
                unproven(&primary_grown, Firmware::Bios),
                Some(Unproven::Never(LargerEntry))
            );
            assert_eq!(
                unproven(&emptied, Firmware::Bios),
                Some(Unproven::Never(EmptyFrame))
            );
            for layout in [
                &last_deleted,
                &two_deleted,
                &grown,
                &logical_formatted,
                &primary_formatted,
            ] {
                assert_eq!(unproven(layout, Firmware::Bios), None, "{layout:?}");
            }
        }
        // `Logical` beside what a logical partition shares with a primary
        // one: in each set holding it, each of these its scenario's plan
        // ran on a logical partition and on another — else the set would
        // open it on the kind it did not run on.
        let asks = |part: &Part, was: &Partition, one: Capability| match (&part.fate, one) {
            (Fate::Format { .. }, ChangeEntries) => true,
            (Fate::Format { size, .. }, SmallerEntry) => *size < part.size,
            (Fate::Shrink { .. }, ShrinkExt4) => was.fs_type == "ext4",
            (Fate::Shrink { .. }, ShrinkNtfs) => was.fs_type == "ntfs",
            _ => false,
        };
        let mut paired = 0;
        for ((name, layout, _), (_, set)) in the_campaigns_plans().iter().zip(CAMPAIGN) {
            if !set.contains(&Logical) {
                continue;
            }
            for one in [ChangeEntries, SmallerEntry, ShrinkExt4, ShrinkNtfs] {
                if !set.contains(&one) {
                    continue;
                }
                let kinds: BTreeSet<bool> = layout
                    .iter()
                    .filter(|disk| disk.table.is_none())
                    .flat_map(|disk| {
                        disk.parts
                            .iter()
                            .zip(&disk.disk.partitions)
                            .filter(|(part, was)| asks(part, was, one))
                            .map(|(_, was)| logical(disk.disk.table, was))
                    })
                    .collect();
                assert_eq!(kinds.len(), 2, "{name}: {one:?} on {kinds:?} alone");
                paired += 1;
            }
        }
        // The logical scenario's format in place, on the primary partition
        // and on the logical one it grows.
        assert_eq!(paired, 1);
    }

    /// **`/` or `/boot` on a logical partition is proven by no scenario**
    /// (decision 20): every scenario's `/` is a partition made or a primary
    /// one, and none has a `/boot` on msdos, so a plan putting either on a
    /// logical partition of a kept msdos table — where the boot flag, which
    /// goes only on partitions 1 to 4 (`manual::plan::activation`), then
    /// stands on `/` alone, or on nothing that starts Linexin — needs
    /// `RootOnLogical`, which no set holds. With the campaign's sets
    /// pretended proven it is refused for that alone, though the logical
    /// scenario's set holds all else such a plan needs; given to every set
    /// too, it would go on. A `/` on a primary partition, a GPT partition
    /// numbered past four and anything under a new table need nothing of it.
    #[test]
    fn root_or_boot_on_a_logical_partition_is_proven_by_no_scenario() {
        use Capability::*;
        let sets = the_campaigns_sets();
        let root = |size: u64, at: MountPoint| format(Fs::Ext4, size, Some(at));
        let root_logical = vec![with(kept(old()), 5, root(30 * GIB, MountPoint::Root))];
        let boot_logical = vec![with(
            with(kept(old()), 1, root(40 * GIB, MountPoint::Root)),
            5,
            root(30 * GIB, MountPoint::Boot),
        )];
        let boot_kept_logical = vec![with(
            kept(old()),
            7,
            Fate::Keep {
                mount: Some(MountPoint::Boot),
            },
        )];
        let root_primary = vec![with(kept(old()), 1, root(40 * GIB, MountPoint::Root))];
        let mut five = linux();
        five.partitions[1].number = 5;
        five.partitions[1].path = "/dev/sdc5".into();
        let gpt_five = vec![with(kept(five), 5, root(40 * GIB, MountPoint::Root))];
        let under_a_new_table = vec![made(
            fresh(old(), Table::Msdos),
            1,
            100 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        )];
        assert_eq!(
            needs(&root_logical, Firmware::Bios),
            [ChangeEntries, Bios, Msdos, Logical, RootOnLogical]
        );
        // `/boot` on a logical partition and `/` on the primary one: the flag
        // goes on `/`. (A `/boot` under BIOS asks `BootOnBios` too, which no
        // set holds either.)
        assert_eq!(
            needs(&boot_logical, Firmware::Bios),
            [
                ChangeEntries,
                Bios,
                Msdos,
                Logical,
                RootOnLogical,
                Activate,
                BootOnBios
            ]
        );
        assert_eq!(
            needs(&boot_kept_logical, Firmware::Bios),
            [Bios, Msdos, RootOnLogical, BootOnBios]
        );
        assert_eq!(
            needs(&root_primary, Firmware::Bios),
            [ChangeEntries, Bios, Msdos, Activate]
        );
        assert_eq!(needs(&gpt_five, Firmware::Uefi), [ChangeEntries, Uefi, Gpt]);
        assert_eq!(
            needs(&under_a_new_table, Firmware::Bios),
            [FreshTable, Bios, Msdos]
        );
        assert!(CAMPAIGN
            .iter()
            .all(|(_, set)| !set.contains(&RootOnLogical)));
        let logical_set = CAMPAIGN
            .iter()
            .find(|(name, _)| *name == "manual-bios-msdos-logical")
            .map(|(_, set)| *set)
            .unwrap();
        assert!(needs(&root_logical, Firmware::Bios)
            .iter()
            .all(|one| *one == RootOnLogical || logical_set.contains(one)));
        {
            let _campaign = PretendProven::these(sets);
            for layout in [&root_logical, &boot_logical, &boot_kept_logical] {
                assert_eq!(
                    unproven(layout, Firmware::Bios),
                    Some(Unproven::Never(RootOnLogical)),
                    "{layout:?}"
                );
            }
            assert_eq!(unproven(&root_primary, Firmware::Bios), None);
        }
        let with_it: Vec<&'static [Capability]> = sets
            .iter()
            .map(|set| {
                let mut more = set.to_vec();
                more.push(RootOnLogical);
                more.sort();
                &*more.leak()
            })
            .collect();
        let _given = PretendProven::these(with_it.leak());
        assert_eq!(unproven(&root_logical, Firmware::Bios), None);
    }

    /// **The boot flag is proven where the plan sets it** (decision 20).
    /// Phase B's step 8 (`sfdisk --activate`) writes the disk's own table
    /// where a kept msdos table holding `/` has nothing left standing marked
    /// ([`activates`], from the flags the disk was reviewed with): the
    /// keep-FAT and logical scenarios run it (their fixtures mark nothing),
    /// the BIOS Windows scenario does not (Windows keeps its mark). So a
    /// reinstall with `/home` kept on an MBR disk where nothing is marked
    /// needs `Activate` beside what the BIOS Windows scenario ran, and is
    /// refused as a mix no scenario ran; the same on a disk whose Windows
    /// keeps its mark goes on. A marked partition deleted takes its mark with
    /// it; `/` on a logical partition alone has no primary to take it; a disk
    /// only mounted from, a GPT disk and a new msdos table (whose own script
    /// marks it) set nothing by this step; and under UEFI the step is the
    /// same. With the two scenarios that set it left out, a plan that sets it
    /// is refused for it alone.
    #[test]
    fn the_boot_flag_is_proven_where_the_plan_sets_it() {
        use Capability::*;
        let sets = the_campaigns_sets();
        let root =
            |one: DiskLayout| with(one, 2, format(Fs::Ext4, 28 * GIB, Some(MountPoint::Root)));
        let home = |one: DiskLayout| {
            with(
                one,
                3,
                Fate::Keep {
                    mount: Some(MountPoint::Home),
                },
            )
        };
        let reinstall_bare = vec![home(root(kept(bare_mbr())))];
        let reinstall_marked = vec![home(root(kept(windows_mbr())))];
        let windows_deleted = vec![with(root(kept(windows_mbr())), 1, Fate::Delete)];
        let made_bare = vec![made(
            kept(old()),
            102_404,
            32 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        )];
        let logical_root = with(
            kept(old()),
            5,
            format(Fs::Ext4, 30 * GIB, Some(MountPoint::Root)),
        );
        // A `/boot` on the primary partition before `/` on a logical one:
        // the flag goes on `/boot`.
        let boot_before_logical_root = with(
            logical_root.clone(),
            1,
            format(Fs::Ext4, 40 * GIB, Some(MountPoint::Boot)),
        );
        let only_mounted = with(
            kept(bare_mbr()),
            3,
            Fate::Keep {
                mount: other("/data"),
            },
        );
        assert!(activates(&reinstall_bare[0]));
        assert!(!activates(&reinstall_marked[0]));
        assert!(activates(&windows_deleted[0]));
        assert!(activates(&made_bare[0]));
        assert!(!activates(&logical_root));
        assert!(activates(&boot_before_logical_root));
        assert!(!activates(&only_mounted));
        assert!(!activates(&msdos_fresh()));
        assert!(!activates(&good_beside_windows()));
        assert_eq!(
            needs(&reinstall_bare, Firmware::Bios),
            [ChangeEntries, KeptHome, Bios, Msdos, Activate]
        );
        assert_eq!(
            needs(&reinstall_marked, Firmware::Bios),
            [ChangeEntries, KeptHome, Bios, Msdos]
        );
        assert_eq!(
            needs(&reinstall_bare, Firmware::Uefi),
            [ChangeEntries, KeptHome, Uefi, Msdos, Activate]
        );
        assert_eq!(
            needs(&windows_deleted, Firmware::Bios),
            [DeleteEntries, ChangeEntries, Bios, Msdos, Activate]
        );
        assert_eq!(
            needs(&[good_fresh(), only_mounted], Firmware::Uefi),
            [FreshTable, SeveralDisks, Uefi, Gpt, Msdos]
        );
        // Exactly the keep-FAT and logical scenarios set it.
        let setting: Vec<&str> = CAMPAIGN
            .iter()
            .filter(|(_, set)| set.contains(&Activate))
            .map(|(name, _)| *name)
            .collect();
        assert_eq!(
            setting,
            ["manual-bios-msdos-keep-fat", "manual-bios-msdos-logical"]
        );
        {
            let _campaign = PretendProven::these(sets);
            assert_eq!(
                unproven(&reinstall_bare, Firmware::Bios),
                Some(Unproven::Together(needs(&reinstall_bare, Firmware::Bios)))
            );
            assert_eq!(unproven(&reinstall_marked, Firmware::Bios), None);
            assert_eq!(unproven(&made_bare, Firmware::Bios), None);
        }
        let _unset = PretendProven::these(sets_without(sets, Activate));
        assert_eq!(
            unproven(&made_bare, Firmware::Bios),
            Some(Unproven::Never(Activate))
        );
        assert_eq!(unproven(&reinstall_marked, Firmware::Bios), None);
    }

    /// **A `/boot` under BIOS is proven by no scenario** (decision 20): every
    /// BIOS scenario keeps its kernels on `/`, yet the rule set takes a
    /// `/boot` there on `/`'s disk — a FAT32 one made or formatted, or an EFI
    /// system partition kept — from which GRUB's core then reads its modules,
    /// `grub.cfg` and the kernels, and on msdos the boot flag goes on it
    /// rather than on `/`. So a plan with one needs `BootOnBios`, which no
    /// set holds: with the campaign's sets pretended proven, keep-FAT's disk
    /// with a FAT32 `/boot` made before `/`, a new GPT with one beside
    /// `bios_grub`, the BIOS Windows plan with one made in the room Windows
    /// gives up and an msdos EFI system partition kept at `/boot` — each
    /// passing the rule set — are refused for it alone, though keep-FAT's,
    /// bios-gpt's and the BIOS Windows scenario's sets hold all else each
    /// needs; given to every set too, each would go on. Under UEFI a `/boot`,
    /// which every scenario has, asks nothing of it.
    #[test]
    fn a_boot_partition_under_bios_is_proven_by_no_scenario() {
        use Capability::*;
        let sets = the_campaigns_sets();
        let set_of = |name: &str| {
            CAMPAIGN
                .iter()
                .find(|(named, _)| *named == name)
                .map(|(_, set)| *set)
                .unwrap()
        };
        let on_gpt = made(
            made(
                made(fresh(blank(), Table::Gpt), 1, 1, Fs::BiosGrub, None),
                2,
                1024,
                Fs::Fat32,
                Some(MountPoint::Boot),
            ),
            1026,
            100 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        );
        let beside_windows = made(
            with(
                with(
                    with(
                        kept(windows_mbr()),
                        1,
                        Fate::Shrink {
                            to: 28 * GIB,
                            mount: None,
                        },
                    ),
                    2,
                    format(Fs::Ext4, 28 * GIB, Some(MountPoint::Root)),
                ),
                3,
                Fate::Keep {
                    mount: Some(MountPoint::Home),
                },
            ),
            28_673,
            4 * 1024,
            Fs::Fat32,
            Some(MountPoint::Boot),
        );
        let esp = disk(
            "/dev/sdg",
            Some(Table::Msdos),
            40,
            vec![part("/dev/sdg", 1, 1, 1024, "vfat", part_type::MBR_ESP)],
        );
        let kept_esp = made(
            with(
                kept(esp),
                1,
                Fate::Keep {
                    mount: Some(MountPoint::Boot),
                },
            ),
            1025,
            32 * 1024,
            Fs::Ext4,
            Some(MountPoint::Root),
        );
        let plans = [
            (
                boot_before_root_on_bios(),
                "manual-bios-msdos-keep-fat",
                vec![KeepTable, Bios, Msdos, Activate, BootOnBios],
            ),
            (
                on_gpt,
                "manual-bios-gpt",
                vec![FreshTable, Bios, Gpt, BootOnBios],
            ),
            (
                beside_windows,
                "manual-bios-msdos-windows-keep-home",
                vec![
                    KeepTable,
                    ChangeEntries,
                    KeptHome,
                    Bios,
                    Msdos,
                    BootOnBios,
                    ShrinkNtfs,
                ],
            ),
            (
                kept_esp,
                "manual-bios-msdos-keep-fat",
                vec![KeepTable, Bios, Msdos, Activate, BootOnBios],
            ),
        ];
        assert!(CAMPAIGN.iter().all(|(_, set)| !set.contains(&BootOnBios)));
        for (one, name, needed) in &plans {
            let layout = std::slice::from_ref(one);
            assert_eq!(judge(layout, Firmware::Bios), Ok(()), "{name}");
            assert_eq!(needs(layout, Firmware::Bios), *needed, "{name}");
            assert!(needed
                .iter()
                .all(|each| *each == BootOnBios || set_of(name).contains(each)));
            assert!(!needs(layout, Firmware::Uefi).contains(&BootOnBios));
        }
        assert!(!needs(&[good_fresh()], Firmware::Uefi).contains(&BootOnBios));
        {
            let _campaign = PretendProven::these(sets);
            for (one, name, _) in &plans {
                assert_eq!(
                    unproven(std::slice::from_ref(one), Firmware::Bios),
                    Some(Unproven::Never(BootOnBios)),
                    "{name}"
                );
            }
        }
        let with_it: Vec<&'static [Capability]> = sets
            .iter()
            .map(|set| {
                let mut more = set.to_vec();
                more.push(BootOnBios);
                more.sort();
                &*more.leak()
            })
            .collect();
        let _given = PretendProven::these(with_it.leak());
        for (one, name, _) in &plans {
            assert_eq!(
                unproven(std::slice::from_ref(one), Firmware::Bios),
                None,
                "{name}"
            );
        }
    }

    /// **A disk whose logical sector is not 512 bytes is proven by no
    /// scenario** (decision 20): QEMU gave every scenario's
    /// disks 512-byte sectors, yet the rule set lays a plan out on a disk of
    /// 4096-byte ones ([`sector_fits`]), and the worker reckons that disk's
    /// table, its `-N` scripts and every place in them in its sectors. So a
    /// plan that writes to such a disk, or keeps its table there, needs
    /// `LargeSectors`, which no set holds. Each scenario's own plan — at 512
    /// bytes its own set exactly, going on by `PROVEN` and by that set
    /// alone — laid on 4096-byte sectors is judged by the rule set as it is
    /// at 512 (eight of the eleven pass it at both; the other three stand
    /// for their needs alone), needs its set and `LargeSectors`, and is
    /// refused for that alone: by `PROVEN`, by its own set alone and by one
    /// set holding every other capability; by each other set alone it is
    /// refused too. So is a plan over several disks with one of them alone
    /// on 4096-byte sectors, the one written to or the one only mounted
    /// from. Given to every set, each would go on: nothing else refuses it.
    #[test]
    fn a_disk_of_large_sectors_is_proven_by_no_scenario() {
        use Capability::*;
        assert!(CAMPAIGN.iter().all(|(_, set)| !set.contains(&LargeSectors)));
        assert!(PROVEN.iter().all(|set| !set.contains(&LargeSectors)));
        let every_other: &'static [Capability] = EVERY_CAPABILITY
            .iter()
            .copied()
            .filter(|one| *one != LargeSectors)
            .collect::<Vec<_>>()
            .leak();
        let every_other: &'static [&'static [Capability]] = vec![every_other].leak();
        let mut laid = Vec::new();
        for ((name, layout, firmware), (_, set)) in the_campaigns_plans().into_iter().zip(CAMPAIGN)
        {
            // At 512 bytes, as every scenario ran: its own set exactly.
            assert!(
                layout.iter().all(|one| one.disk.logical_sector == 512),
                "{name}"
            );
            assert_eq!(needs(&layout, firmware), *set, "{name}");
            assert_eq!(unproven(&layout, firmware), None, "{name}");
            {
                let _own = PretendProven::these(std::slice::from_ref(set));
                assert_eq!(unproven(&layout, firmware), None, "{name} by its own set");
            }
            // Every disk on 4096-byte sectors, and over several disks each
            // one alone.
            let mut large = vec![layout
                .iter()
                .cloned()
                .map(on_large_sectors)
                .collect::<Vec<_>>()];
            if layout.len() > 1 {
                for at in 0..layout.len() {
                    let mut one = layout.clone();
                    one[at] = on_large_sectors(one[at].clone());
                    large.push(one);
                }
            }
            let mut with_it = set.to_vec();
            with_it.push(LargeSectors);
            with_it.sort();
            for one in large {
                assert_eq!(judge(&one, firmware), judge(&layout, firmware), "{name}");
                assert_eq!(needs(&one, firmware), with_it, "{name}");
                laid.push((name, one, firmware, set));
            }
        }
        // Each of the eleven at 4096 bytes, and each disk of the two plans
        // over two disks alone at 4096; all but the three stand-ins of
        // needs alone (a new table over a used disk and the reinstall with
        // no EFI partition, keep-FAT's beside an extended one) a plan the
        // rule set takes.
        assert_eq!(laid.len(), 11 + 2 + 2);
        let taken = laid
            .iter()
            .filter(|(_, one, firmware, _)| judge(one, *firmware).is_ok())
            .count();
        assert_eq!(taken, 15 - 3);
        let refused = Some(Unproven::Never(LargeSectors));
        for (name, one, firmware, set) in &laid {
            assert_eq!(unproven(one, *firmware), refused, "{name}");
            {
                let _own = PretendProven::these(std::slice::from_ref(*set));
                assert_eq!(unproven(one, *firmware), refused, "{name} by its own set");
            }
            {
                let _every_other = PretendProven::these(every_other);
                assert_eq!(
                    unproven(one, *firmware),
                    refused,
                    "{name} by every other capability"
                );
            }
            for each in PROVEN {
                let _alone = PretendProven::these(std::slice::from_ref(each));
                assert!(unproven(one, *firmware).is_some(), "{name} by {each:?}");
            }
        }
        let given: Vec<&'static [Capability]> = PROVEN
            .iter()
            .map(|set| {
                let mut more = set.to_vec();
                more.push(LargeSectors);
                more.sort();
                &*more.leak()
            })
            .collect();
        let _given = PretendProven::these(given.leak());
        for (name, one, firmware, _) in &laid {
            assert_eq!(unproven(one, *firmware), None, "{name}");
        }
    }

    #[test]
    fn a_layout_travels_whole_and_refuses_what_it_does_not_know() {
        let layout = vec![
            good_beside_windows(),
            with(
                kept(linux()),
                4,
                Fate::Keep {
                    mount: Some(MountPoint::Home),
                },
            ),
        ];
        let text = serde_json::to_string(&layout).unwrap();
        let back: Vec<DiskLayout> = serde_json::from_str(&text).unwrap();
        assert_eq!(back, layout);
        assert!(
            text.contains(r#""fate":{"kind":"keep","mount":"/efi"}"#),
            "{text}"
        );
        let mut value: serde_json::Value = serde_json::to_value(&layout).unwrap();
        value[0]["extra"] = serde_json::json!(1);
        assert!(serde_json::from_value::<Vec<DiskLayout>>(value).is_err());
        let mut value: serde_json::Value = serde_json::to_value(&layout).unwrap();
        value[0]["parts"][0]["fate"]["label"] = serde_json::json!("x");
        assert!(serde_json::from_value::<Vec<DiskLayout>>(value).is_err());
    }

    #[test]
    fn a_mount_point_travels_as_its_path_and_a_filesystem_as_its_name() {
        for (at, said) in [
            (MountPoint::Root, "\"/\""),
            (MountPoint::Home, "\"/home\""),
            (MountPoint::Efi, "\"/efi\""),
            (MountPoint::Boot, "\"/boot\""),
            (MountPoint::Swap, "\"swap\""),
            (MountPoint::Other("/srv/media".into()), "\"/srv/media\""),
        ] {
            assert_eq!(serde_json::to_string(&at).unwrap(), said);
            assert_eq!(serde_json::from_str::<MountPoint>(said).unwrap(), at);
        }
        // A path offered by name is that one, however it came.
        assert_eq!(
            serde_json::from_str::<MountPoint>("\"/home\"").unwrap(),
            MountPoint::Home
        );
        for (fs, said) in [
            (Fs::Ext4, "\"ext4\""),
            (Fs::Fat32, "\"fat32\""),
            (Fs::LinuxSwap, "\"linux-swap\""),
            (Fs::BiosGrub, "\"bios-grub\""),
        ] {
            assert_eq!(serde_json::to_string(&fs).unwrap(), said);
            assert_eq!(serde_json::from_str::<Fs>(said).unwrap(), fs);
        }
        assert!(serde_json::from_str::<Fs>("\"ntfs\"").is_err());
    }

    #[test]
    fn a_path_of_ones_own_is_held_to_the_rules_as_written() {
        for path in [
            "/data",
            "/srv/media",
            "/var",
            "/var/log",
            "/mnt/windows",
            "/home2",
        ] {
            assert_eq!(own_path(path), Ok(()), "{path}");
        }
        // Not in its normal form, or not a path of one's own at all.
        for path in [
            "data",
            "/data/",
            "//data",
            "/srv/./www",
            "/srv/../etc",
            "/",
            "",
        ] {
            assert_eq!(own_path(path), Err(PathRefused::NotWhole), "{path}");
        }
        let long = format!("/{}", "a".repeat(PATH_MOST));
        assert_eq!(own_path(&long), Err(PathRefused::Long));
        assert_eq!(own_path(&long[..PATH_MOST]), Ok(()));
        for path in [
            "/my data",
            "/tab\there",
            "/new\nline",
            "/back\\slash",
            "/nbsp\u{a0}x",
        ] {
            assert_eq!(own_path(path), Err(PathRefused::Spaces), "{path:?}");
        }
        for (path, folder) in [
            ("/usr", "/usr"),
            ("/usr/local", "/usr"),
            ("/proc/1", "/proc"),
            ("/swap", "/swap"),
            ("/swap/x", "/swap"),
            ("/swapfile", "/swapfile"),
        ] {
            assert_eq!(
                own_path(path),
                Err(PathRefused::System(folder.into())),
                "{path}"
            );
        }
        for (path, folder) in [
            ("/home/piotr", "/home"),
            ("/home/piotr/games", "/home"),
            ("/boot/efi", "/boot"),
            ("/efi/EFI", "/efi"),
        ] {
            assert_eq!(
                own_path(path),
                Err(PathRefused::Under(folder.into())),
                "{path}"
            );
        }
        // Typed, a path is made whole first, and one offered by name is
        // that one: /home itself is fine, inside it is not.
        assert_eq!(
            mount_path(" /data/ "),
            Ok(MountPoint::Other("/data".into()))
        );
        assert_eq!(mount_path("/home/"), Ok(MountPoint::Home));
        assert_eq!(mount_path("//"), Ok(MountPoint::Root));
        assert_eq!(
            mount_path("/home//piotr"),
            Err(PathRefused::Under("/home".into()))
        );
        assert_eq!(
            mount_path("/swapfile"),
            Err(PathRefused::System("/swapfile".into()))
        );
        // Folders only named like them are not theirs.
        assert_eq!(own_path("/library"), Ok(()));
        assert_eq!(own_path("/homes"), Ok(()));
        assert_eq!(own_path("/swapped"), Ok(()));
    }

    // ---------------------------------------------------------------------
    // A request carrying a layout (Q2).

    /// A request's configuration around `layout`, its easy fields its
    /// pins (the swap file `swap_mib` asked for), as the interface builds
    /// it.
    fn pinned_config(layout: Vec<DiskLayout>, swap_mib: u32) -> InstallConfig {
        let pins = pins(&layout, swap_mib).expect("the layout pins");
        let mut config = crate::worker::partition::fixtures::config(pins.disk, Target::WholeDisk);
        config.storage = crate::model::config::Storage {
            filesystem: pins.filesystem,
            swap_mib: pins.swap_mib,
            home: None,
        };
        config.target = Target::Layout { disks: layout };
        config
    }

    /// The pins are `/`'s disk, `/`'s filesystem, no separate `/home` and
    /// the swap file — none beside a swap partition — and each easy field
    /// not its pin is refused for what it is, the swap file held to the
    /// easy path's bound with `/` the room.
    #[test]
    fn a_request_carrying_a_layout_has_its_pins_for_its_easy_fields() {
        // On the blank disk under a new gpt: /dev/sdb and ext4.
        let config = pinned_config(vec![good_fresh()], 4096);
        assert_eq!(config.disk, blank());
        assert_eq!(config.storage.filesystem, Filesystem::Ext4);
        assert_eq!(config.storage.swap_mib, 4096);
        assert_eq!(pinned(&config), Ok(()));
        // Beside Windows, a second disk holding a Btrfs /: that disk.
        let other = with(
            kept(linux()),
            2,
            format(Fs::Btrfs, 40 * GIB, Some(MountPoint::Root)),
        );
        let mut beside = good_beside_windows();
        beside.create.pop();
        let config = pinned_config(vec![beside, other], 0);
        assert_eq!(config.disk, linux());
        assert_eq!(config.storage.filesystem, Filesystem::Btrfs);
        assert_eq!(pinned(&config), Ok(()));
        // A swap partition used: no swap file, whatever was asked for.
        let swapping = with(
            kept(linux()),
            3,
            format(Fs::LinuxSwap, 4 * GIB, Some(MountPoint::Swap)),
        );
        let swapping = with(
            swapping,
            2,
            format(Fs::Ext4, 40 * GIB, Some(MountPoint::Root)),
        );
        assert_eq!(
            pins(std::slice::from_ref(&swapping), 8192)
                .unwrap()
                .swap_mib,
            0
        );
        // Nothing at /, or / not made as ext4 or Btrfs: refused as the rule
        // set refuses it.
        assert_eq!(pins(&[kept(windows())], 0), Err(Refusal::Root));
        let xfs = with(
            kept(linux()),
            2,
            format(Fs::Xfs, 40 * GIB, Some(MountPoint::Root)),
        );
        assert_eq!(
            pins(&[xfs], 0),
            Err(Refusal::Needs {
                at: MountPoint::Root
            })
        );
        // Each easy field that is not its pin.
        let good = pinned_config(vec![good_fresh()], 4096);
        let mut config = good.clone();
        config.disk = windows();
        assert_eq!(pinned(&config), Err(Unpinned::Disk));
        let mut config = good.clone();
        config.disk.serial = "ANOTHER".into();
        assert_eq!(pinned(&config), Err(Unpinned::Disk), "a field of the disk");
        let mut config = good.clone();
        config.storage.home = Some(crate::model::config::Home {
            path: "/dev/sdb1".into(),
            format: false,
            disk: None,
        });
        assert_eq!(pinned(&config), Err(Unpinned::Home));
        let mut config = good.clone();
        config.storage.filesystem = Filesystem::Btrfs;
        assert_eq!(pinned(&config), Err(Unpinned::Filesystem));
        let mut config = pinned_config(vec![swapping], 0);
        assert_eq!(pinned(&config), Ok(()));
        config.storage.swap_mib = 2048;
        assert_eq!(pinned(&config), Err(Unpinned::Swap));
        // 100 GiB of /: a quarter is 25 GiB, so 16 GiB of swap file is
        // taken and 32 GiB is not; on 2 TiB, never past MAX_SWAP.
        let mut config = good.clone();
        config.storage.swap_mib = 16 * 1024;
        assert_eq!(pinned(&config), Ok(()));
        config.storage.swap_mib = 32 * 1024;
        assert_eq!(pinned(&config), Err(Unpinned::SwapTooLarge));
        let mut huge = disk("/dev/sdb", None, 4096, Vec::new());
        huge.size = 4096 * GIB;
        let big = fresh(huge, Table::Gpt);
        let big = made(big, 1, 1024, Fs::Fat32, Some(MountPoint::Boot));
        let big = made(big, 1025, 2048 * 1024, Fs::Ext4, Some(MountPoint::Root));
        let mut config = pinned_config(vec![big], 0);
        config.storage.swap_mib = u32::try_from(MAX_SWAP / MIB).unwrap();
        assert_eq!(pinned(&config), Ok(()));
        config.storage.swap_mib += 1024;
        assert_eq!(pinned(&config), Err(Unpinned::SwapTooLarge));
        // A request carrying no layout has no pins to hold.
        let mut easy = good;
        easy.target = Target::WholeDisk;
        assert_eq!(pinned(&easy), Err(Unpinned::Root));
    }

    /// `home_kept()` is today's expression on every request of the easy
    /// path — a separate `/home` kept unformatted — and, for a layout, a
    /// partition kept or shrunk at `/home`; one formatted there, or kept
    /// elsewhere, is not.
    #[test]
    fn a_home_is_kept_by_a_home_kept_or_by_a_layout_that_keeps_one() {
        use crate::model::config::Home;
        use crate::worker::partition::fixtures::{all, config, targets};
        let today = |config: &InstallConfig| {
            config
                .storage
                .home
                .as_ref()
                .is_some_and(|home| !home.format)
        };
        for sector in [512, 4096] {
            for (name, disk) in all(sector) {
                for (place, target) in targets(&disk) {
                    let mut easy = config(disk.clone(), target);
                    for home in [None, Some(true), Some(false)] {
                        easy.storage.home = home.map(|format| Home {
                            path: "/dev/vdy1".into(),
                            format,
                            disk: None,
                        });
                        assert_eq!(
                            easy.home_kept(),
                            today(&easy),
                            "{name} {sector} {place} {home:?}"
                        );
                    }
                }
            }
        }
        let at_home = Some(MountPoint::Home);
        let kept_home = with(
            kept(linux()),
            4,
            Fate::Keep {
                mount: at_home.clone(),
            },
        );
        let root = with(
            kept_home.clone(),
            2,
            format(Fs::Ext4, 40 * GIB, Some(MountPoint::Root)),
        );
        let laid = |layout: DiskLayout| {
            let mut config = config(layout.disk.clone(), Target::WholeDisk);
            config.target = Target::Layout {
                disks: vec![layout],
            };
            config
        };
        assert!(keeps_home(std::slice::from_ref(&root)));
        assert!(laid(root.clone()).home_kept());
        let shrunk = with(
            root.clone(),
            4,
            Fate::Shrink {
                to: 60 * GIB,
                mount: at_home.clone(),
            },
        );
        assert!(laid(shrunk).home_kept(), "shrunk, it is still kept");
        let formatted = with(root.clone(), 4, format(Fs::Ext4, 80 * GIB, at_home));
        assert!(!laid(formatted).home_kept());
        let elsewhere = with(
            root,
            4,
            Fate::Keep {
                mount: other("/data"),
            },
        );
        assert!(!laid(elsewhere).home_kept());
    }

    /// A request carrying a layout travels whole and back, its tag inside
    /// the target as the other places' is, and an unknown field anywhere in
    /// it — beside the disks, in a disk's layout, in a fate — is refused.
    #[test]
    fn a_request_carrying_a_layout_travels_whole_and_refuses_what_it_does_not_know() {
        use crate::model::protocol::Request;
        let shrunk = with(
            good_beside_windows(),
            3,
            Fate::Shrink {
                to: 150 * GIB,
                mount: other("/windows"),
            },
        );
        let config = pinned_config(vec![shrunk], 2048);
        let request = Request {
            consent: crate::model::disks::consent(&config),
            config,
        };
        let json = serde_json::to_string(&request).unwrap();
        let back: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(back.config.target, request.config.target);
        assert_eq!(back.consent, request.consent);
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["config"]["target"]["kind"], "layout");
        for (pointer, what) in [
            ("/config/target", "beside the disks"),
            ("/config/target/disks/0", "in a disk's layout"),
            ("/config/target/disks/0/parts/2/fate", "in a fate"),
            ("/config/target/disks/0/create/0", "in a partition made"),
        ] {
            let mut value = value.clone();
            value
                .pointer_mut(pointer)
                .and_then(serde_json::Value::as_object_mut)
                .unwrap_or_else(|| panic!("{pointer}"))
                .insert("unknown".into(), serde_json::json!(1));
            assert!(
                serde_json::from_value::<Request>(value).is_err(),
                "an unknown field {what} was taken"
            );
        }
    }

    /// The storage checks hand a layout to the rule set (`room` has none to
    /// give, `validate_storage` judges its pins and its shape, `consent` is
    /// what it destroys), and say a boot loader that cannot start it, or a
    /// swap file too large for its `/`, as they say them on the easy path.
    #[test]
    fn the_storage_checks_hand_a_layout_to_the_rule_set() {
        use crate::model::config::Invalid;
        use crate::model::disks::{consent, room, validate_storage};
        let good = pinned_config(vec![good_beside_windows()], 4096);
        assert_eq!(room(&good, Firmware::Uefi), Err(Invalid::LayoutRefused));
        assert_eq!(validate_storage(&good, Firmware::Uefi), Ok(()));
        // An ESP shared beside Windows: every boot loader starts it on UEFI,
        // none of systemd-boot or rEFInd on BIOS (which also wants no /efi).
        assert_eq!(
            validate_storage(&good, Firmware::Bios),
            Err(Invalid::LayoutRefused)
        );
        let msdos = with(
            kept(old()),
            5,
            format(Fs::Ext4, 30 * GIB, Some(MountPoint::Root)),
        );
        let mut on_dos = pinned_config(vec![msdos], 0);
        assert_eq!(validate_storage(&on_dos, Firmware::Bios), Ok(()));
        on_dos.bootloader = Bootloader::SystemdBoot;
        assert_eq!(
            validate_storage(&on_dos, Firmware::Bios),
            Err(Invalid::BootloaderUnavailable)
        );
        let mut swap = good.clone();
        swap.storage.swap_mib = 64 * 1024;
        assert_eq!(
            validate_storage(&swap, Firmware::Uefi),
            Err(Invalid::SwapTooLarge)
        );
        let mut unpinned = good.clone();
        unpinned.storage.filesystem = Filesystem::Btrfs;
        assert_eq!(
            validate_storage(&unpinned, Firmware::Uefi),
            Err(Invalid::LayoutRefused)
        );
        let mut source = good.clone();
        source.source = "relative/path".into();
        assert_eq!(
            validate_storage(&source, Firmware::Uefi),
            Err(Invalid::SourceInvalid)
        );
        // Its consent is every line of what it destroys, in the rule set's
        // order; here nothing (Windows' EFI partition is kept), and with a
        // shrink and a delete, those.
        assert!(consent(&good).is_empty());
        let laid = with(
            with(
                good_beside_windows(),
                3,
                Fate::Shrink {
                    to: 150 * GIB,
                    mount: None,
                },
            ),
            4,
            Fate::Delete,
        );
        let config = pinned_config(vec![laid.clone()], 0);
        let lines: Vec<String> = destroys(&[laid]).iter().map(Destroy::line).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(consent(&config), lines);
    }
}
