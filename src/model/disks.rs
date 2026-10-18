//! The disks of the machine, as both processes see them.
//!
//! CONTRACT (phase 0): the types below are fixed. Discovery is read-only: it
//! reads `lsblk --json`, `/proc/swaps`, `/sys/class/block/*/holders` and the
//! udev database, writes nothing, and needs no privilege, so the interface
//! can call it as the person and the worker can call it again as root and
//! expect the same answer for a disk nobody touched.
//!
//! Three questions are answered here and nowhere else, so that the interface
//! and the worker cannot disagree about them:
//!
//! - [`discover`]: what is on the machine, and which disks must not be
//!   touched at all ([`Blocked`]). Anything that cannot be read is a reason
//!   to refuse the disk, never a reason to guess.
//! - [`validate_storage`]: whether a choice of disk, place, /home, swap and
//!   boot loader can be installed, judged from the disk as the person
//!   reviewed it (the storage rules of `INSTALLER-HANDOFF.md` §4.3).
//! - [`consent`]: which existing devices that choice destroys, which is what
//!   the confirmation dialog lists and what the worker insists on seeing.
//!
//! Sizes are bytes throughout. lsblk reports a partition's `START` in
//! 512-byte units whatever the disk's logical sector size is (it reads the
//! kernel's `/sys/class/block/<part>/start`, which is always counted that
//! way), so a start is `START × 512` and never `START × LOG-SEC`; the logical
//! sector size only matters for what a partition table reserves at the end
//! of a disk.
use super::config::{Bootloader, Home, InstallConfig, Invalid, Target};
use super::firmware::Firmware;
use super::layout::{self, Destroy, DiskLayout, Refusal, Unpinned};
use super::live::{self, LiveSession};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

pub const MIB: u64 = 1024 * 1024;
pub const GIB: u64 = 1024 * MIB;

/// The smallest space the installer offers as a place to put Linexin: the
/// legacy installer's 25 GB, counted *after* any EFI partition it must add.
pub const MIN_INSTALL: u64 = 25 * 1000 * 1000 * 1000;
/// The smallest gap of free space worth listing at all.
pub const MIN_GAP: u64 = 256 * MIB;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Table {
    Gpt,
    Msdos,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    Nvme,
    Sata,
    Usb,
    Mmc,
    Virtio,
    Other,
}

/// Why a disk cannot be chosen at all. The interface outlines such a disk,
/// skips it with the light and says the reason in plain words under it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Blocked {
    /// It holds the medium this live system was started from.
    InstallationMedia,
    /// Something on it is mounted (the paths are for Details only).
    InUse { mountpoints: Vec<String> },
    /// Part of it is active swap.
    Swap,
    /// RAID, encryption or device-mapper sits on top of it.
    Holder,
    /// The disk is read-only.
    ReadOnly,
    /// It is smaller than any useful installation.
    TooSmall,
    /// Its state could not be read completely, so it is refused rather
    /// than guessed at.
    Unknown,
}

/// What a partition appears to hold, judged from its type and filesystem
/// only (the interface cannot look inside without privilege).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Content {
    /// An EFI system partition.
    Efi,
    /// A BIOS boot partition (GRUB's core on a GPT disk).
    BiosBoot,
    /// Microsoft reserved (no filesystem; never offered).
    MicrosoftReserved,
    /// Windows recovery tools.
    WindowsRecovery,
    /// An NTFS partition — usually Windows or Windows data.
    Windows,
    /// A Linux filesystem (ext4, Btrfs, XFS…).
    Linux,
    /// Linux swap.
    Swap,
    /// FAT/exFAT data.
    Data,
    /// No recognised filesystem.
    Empty,
    /// Something else.
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Partition {
    /// `/dev/sda2`, `/dev/nvme0n1p2` — read back from lsblk, never built.
    pub path: String,
    /// The partition number in the table.
    pub number: u32,
    /// Start and size in bytes.
    pub start: u64,
    pub size: u64,
    /// `ntfs`, `vfat`, `ext4`, `btrfs`… or empty.
    pub fs_type: String,
    /// The filesystem label, or empty.
    pub label: String,
    /// The GPT type GUID (lower case) or the MBR type (`0x83`), or empty.
    pub part_type: String,
    /// The filesystem UUID, or empty.
    pub uuid: String,
    pub mountpoints: Vec<String>,
    pub content: Content,
    /// Whether an msdos table marks it active: the boot flag, as udev read
    /// the table (lsblk's `PARTFLAGS`, libblkid giving an entry's boot
    /// indicator, `0x80`, as its flags). Never on a GPT disk, whose flags
    /// are attribute bits and no boot flag. A hand-made layout's last step
    /// on a kept msdos table sets it only where nothing standing is marked
    /// (`layout::activates`), and phase A holds the disk to it before
    /// anything is written. Left out of a request where it is false, so
    /// every request written before it is read as it was.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bootable: bool,
}

/// Whether two filesystem UUIDs, as tools write them, name the same
/// filesystem: compared without case, since one tool may write in capitals
/// what another writes in small letters (a FAT volume's `ABCD-1234`, a
/// GUID). The one way the worker compares a UUID read now — `blkid -p`, or
/// lsblk again — with the one a partition was reviewed with. Two empty
/// UUIDs are the same; whether an empty one may stand at all is the
/// caller's to judge.
pub fn same_uuid(one: &str, other: &str) -> bool {
    one.eq_ignore_ascii_case(other)
}

/// A run of unpartitioned space, aligned to 1 MiB at both ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gap {
    pub start: u64,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disk {
    /// `/dev/sda`. The worker re-finds the disk by every field below and
    /// refuses if any changed since the person reviewed it.
    pub path: String,
    /// The kernel's disk sequence number (`DISK-SEQ`), unique per boot and
    /// changed by a replug: the strongest identity a disk has.
    pub diskseq: Option<u64>,
    pub model: String,
    pub serial: String,
    pub size: u64,
    pub major_minor: String,
    pub logical_sector: u32,
    pub transport: Transport,
    pub removable: bool,
    /// Whether the disk says it spins (lsblk's `ROTA`): a hard disk rather
    /// than one made of flash. Only what kind of mark its row wears hangs on
    /// it; it is no part of which disk this is, so the worker's check that the
    /// disk is the one reviewed leaves it out.
    #[serde(default)]
    pub rotational: bool,
    /// The partition table, or `None` for a disk with none.
    pub table: Option<Table>,
    /// What is written straight onto the disk itself rather than into a
    /// partition: a filesystem, encryption, LVM or RAID made on the whole
    /// disk, which lsblk reports as the disk's own `FSTYPE`. Classified as a
    /// partition's content is; `None` where the disk itself carries nothing.
    /// A disk with this holds something even with no partitions at all, so
    /// it offers no free space and is never erased unless the person says so.
    #[serde(default)]
    pub content: Option<Content>,
    pub read_only: bool,
    pub blocked: Option<Blocked>,
    pub partitions: Vec<Partition>,
    /// Free gaps of at least [`MIN_GAP`], in disk order.
    pub free: Vec<Gap>,
}

impl Disk {
    /// Whether nothing at all is on the disk: no partitions, and nothing
    /// written over the whole of it.
    pub fn is_blank(&self) -> bool {
        self.partitions.is_empty() && self.content.is_none()
    }

    /// The disk's existing EFI system partition, if it has one.
    pub fn esp(&self) -> Option<&Partition> {
        self.partitions.iter().find(|p| p.content == Content::Efi)
    }

    pub fn partition(&self, path: &str) -> Option<&Partition> {
        self.partitions.iter().find(|p| p.path == path)
    }
}

impl Partition {
    /// An extended partition of a DOS table: a container for logical
    /// partitions, never a place to put anything itself.
    pub fn is_extended(&self) -> bool {
        matches!(
            self.part_type.as_str(),
            part_type::MBR_EXTENDED | part_type::MBR_EXTENDED_LBA | part_type::MBR_EXTENDED_LINUX
        )
    }

    /// Whether Linexin may be put in place of this partition. The EFI system
    /// partition, the areas firmware and Windows reserve (its reserved and
    /// recovery partitions), and an extended partition are never offered;
    /// [`validate_storage`] refuses them too.
    pub fn replaceable(&self) -> bool {
        !self.is_extended()
            && !matches!(
                self.content,
                Content::Efi
                    | Content::BiosBoot
                    | Content::MicrosoftReserved
                    | Content::WindowsRecovery
            )
    }
}

/// Partition type identifiers, as lsblk reports them and as
/// [`Partition::part_type`] holds them: GPT type GUIDs in lower case, and DOS
/// types as two hex digits after `0x` (lsblk writes `0x7`; discovery pads it
/// to `0x07` so one spelling is compared everywhere).
pub mod part_type {
    pub const ESP: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
    pub const BIOS_BOOT: &str = "21686148-6449-6e6f-744e-656564454649";
    pub const MICROSOFT_RESERVED: &str = "e3c9e316-0b5c-4db8-817d-f92df00215ae";
    pub const WINDOWS_RECOVERY: &str = "de94bba4-06d1-4d40-a16a-bfd50179d6ac";
    pub const BASIC_DATA: &str = "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7";
    pub const LINUX: &str = "0fc63daf-8483-4772-8e79-3d69d8477de4";
    pub const LINUX_ROOT_X86_64: &str = "4f68bce3-e8cd-4db1-96e7-fbcaf984b709";
    pub const LINUX_HOME: &str = "933ac7e1-2eb4-4f13-b844-0e14e2aef915";
    pub const LINUX_SWAP: &str = "0657fd6d-a4ab-43c4-84e5-0933c84b4f4f";
    pub const MBR_ESP: &str = "0xef";
    pub const MBR_LINUX: &str = "0x83";
    pub const MBR_SWAP: &str = "0x82";
    pub const MBR_NTFS: &str = "0x07";
    pub const MBR_FAT32: &str = "0x0b";
    pub const MBR_FAT32_LBA: &str = "0x0c";
    pub const MBR_FAT16_LBA: &str = "0x0e";
    pub const MBR_WINDOWS_RECOVERY: &str = "0x27";
    pub const MBR_EXTENDED: &str = "0x05";
    pub const MBR_EXTENDED_LBA: &str = "0x0f";
    pub const MBR_EXTENDED_LINUX: &str = "0x85";
    /// "Linux extended boot" (XBOOTLDR) from the Discoverable Partitions
    /// Specification: the FAT `/boot` beside an EFI system partition at
    /// `/efi`, holding the kernels (the worker's `GPT_LINUX_BOOT`).
    pub const XBOOTLDR: &str = "bc13c2ff-59e6-4262-a352-b275fd6f7172";
    /// XBOOTLDR's type on an MBR, from the Boot Loader Specification.
    pub const MBR_XBOOTLDR: &str = "0xea";
}

/// The EFI system partition the installer makes on a disk that has none.
pub const NEW_ESP: u64 = 2 * GIB;
/// The Linux boot partition the installer makes for the kernels beside a
/// reused EFI system partition that has no room for them.
pub const LINUX_BOOT: u64 = 2 * GIB;
/// The BIOS boot partition GRUB needs on a GPT disk started by BIOS.
pub const BIOS_BOOT: u64 = MIB;
/// The smallest partition offered as a separate /home: the legacy
/// installer's "1 GB", which it counted in GiB.
pub const MIN_HOME: u64 = GIB;
/// The largest swap file: the legacy installer's 128 GB slider, which it
/// counted in GiB.
pub const MAX_SWAP: u64 = 128 * GIB;
/// The partition entries a GPT made by the usual tools has room for.
pub const GPT_ENTRIES: usize = 128;
/// A DOS table has four primary entries (an extended partition takes one).
pub const MBR_PRIMARIES: usize = 4;

/// What discovery asks lsblk for (`lsblk --list-columns` on util-linux 2.42).
pub const LSBLK_COLUMNS: &str = "NAME,TYPE,SIZE,MODEL,SERIAL,RO,RM,ROTA,TRAN,MOUNTPOINTS,\
MAJ:MIN,FSTYPE,LABEL,UUID,PARTTYPE,PARTFLAGS,PARTN,START,PTTYPE,LOG-SEC,DISK-SEQ";

/// What lsblk itself does not say about the disks, gathered by [`discover`]
/// and handed to [`parse_lsblk`], so that every judgement made from them can
/// be tested on facts written by hand.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    /// The text of `/proc/swaps`; `None` when it could not be read, which
    /// leaves every disk [`Blocked::Unknown`].
    pub swaps: Option<String>,
    /// Device paths with something in `/sys/class/block/<name>/holders`
    /// (RAID, encryption, device-mapper on top of them).
    pub holders: BTreeSet<String>,
    /// Device paths whose holders could not be read at all.
    pub holders_unreadable: BTreeSet<String>,
    /// The live session, when this is one.
    pub live: Option<LiveSession>,
    /// The devices (`major:minor`) archiso has mounted below `/run/archiso/`.
    pub medium_devices: Vec<String>,
    /// For extended partitions only: their size in bytes as the partition
    /// table gives it. The kernel shows an extended partition as a 1 KiB
    /// stub, so without this its real extent is unknown (see [`free_gaps`]).
    pub extended_sizes: BTreeMap<String, u64>,
}

/// Every disk on the machine, including the ones that cannot be chosen.
/// Read-only.
pub fn discover() -> Result<Vec<Disk>, String> {
    let json = lsblk()?;
    let tree: Value =
        serde_json::from_str(&json).map_err(|e| format!("lsblk printed no JSON: {e}"))?;
    let mut facts = Facts {
        swaps: std::fs::read_to_string("/proc/swaps").ok(),
        live: live::detect(),
        ..Facts::default()
    };
    if let Ok(mountinfo) = std::fs::read_to_string("/proc/self/mountinfo") {
        facts.medium_devices = live::medium_devices(&mountinfo);
    }
    for node in every_node(&tree) {
        let path = string(node, "name");
        if !device_path_ok(&path) {
            continue;
        }
        match held(&path) {
            Ok(true) => {
                facts.holders.insert(path.clone());
            }
            Ok(false) => {}
            Err(_) => {
                facts.holders_unreadable.insert(path.clone());
            }
        }
        if string(node, "type") == "part" && mbr_extended(&normal_part_type(node)) {
            let major_minor = string(node, "maj:min");
            if let Some(size) = udev_entry_size(&major_minor) {
                facts.extended_sizes.insert(path, size);
            }
        }
    }
    parse_lsblk(&json, &facts)
}

/// Whether anything holds the block device at `path` (a `/dev/<name>` that
/// [`device_path_ok`] passes): RAID, encryption or device-mapper stacked on
/// it, as its `/sys/class/block/<name>/holders` lists them. Read-only. In a
/// test build it refuses before anything is read
/// (`run::fence::block_devices`), as [`lsblk`] refuses at the head of
/// [`discover`]: no test reads this computer's block devices, and the
/// judgements made of holders are tested on [`Facts`] written by hand.
fn held(path: &str) -> std::io::Result<bool> {
    #[cfg(test)]
    crate::worker::run::fence::block_devices();
    let name = &path["/dev/".len()..];
    let mut entries = std::fs::read_dir(Path::new("/sys/class/block").join(name).join("holders"))?;
    Ok(entries.next().is_some())
}

/// Runs lsblk as discovery needs it: from the system's own directories, with
/// a cleared environment and the C locale, so that neither the person's PATH
/// nor their language changes what is read.
fn lsblk() -> Result<String, String> {
    // Started here rather than through `run`, so in a test build it asks the
    // fence itself, before lsblk is even looked up; the fence refuses it, as
    // no test may list this computer's disks, nor the worker's checks that
    // find a disk again through `discover` (see `run::fence`).
    #[cfg(test)]
    crate::worker::run::fence::check(
        "lsblk",
        &["--json", "--bytes", "--paths", "--output", LSBLK_COLUMNS],
    );
    let program = crate::worker::run::command_path("lsblk").map_err(|stop| stop.detail)?;
    let output = std::process::Command::new(program)
        .args(["--json", "--bytes", "--paths", "--output", LSBLK_COLUMNS])
        .env_clear()
        .env("PATH", crate::worker::run::SAFE_PATH)
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("lsblk could not start: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "lsblk failed: {} {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    // A label in some other encoding must not cost the whole list; it is only
    // ever shown, and both processes read it back the same way.
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The size of a partition as its partition table gives it, from what udev
/// recorded when it probed the partition (world-readable, so no privilege is
/// needed). libblkid counts it in 512-byte units.
fn udev_entry_size(major_minor: &str) -> Option<u64> {
    if major_minor.is_empty() || !major_minor.bytes().all(|c| c.is_ascii_digit() || c == b':') {
        return None;
    }
    let text = std::fs::read_to_string(format!("/run/udev/data/b{major_minor}")).ok()?;
    udev_property(&text, "ID_PART_ENTRY_SIZE")?
        .parse::<u64>()
        .ok()?
        .checked_mul(512)
}

/// One `E:KEY=value` property from a udev database entry.
fn udev_property<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .filter_map(|line| line.strip_prefix("E:"))
        .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
}

/// The disks in lsblk's JSON (`--json --bytes --paths --output`
/// [`LSBLK_COLUMNS`]), judged with `facts`. Pure.
///
/// Only `type: disk` is a disk. zram, loop, RAM disks, optical and floppy
/// drives, and an eMMC's hardware boot areas are left out entirely: none of
/// them is somewhere to install, and listing them as blocked would only be
/// noise. Everything else is listed, blocked or not.
pub fn parse_lsblk(json: &str, facts: &Facts) -> Result<Vec<Disk>, String> {
    let tree: Value =
        serde_json::from_str(json).map_err(|e| format!("lsblk printed no JSON: {e}"))?;
    let nodes = tree
        .get("blockdevices")
        .and_then(Value::as_array)
        .ok_or("lsblk printed no list of block devices")?;
    Ok(nodes
        .iter()
        .filter_map(|node| disk_from(node, facts))
        .collect())
}

fn disk_from(node: &Value, facts: &Facts) -> Option<Disk> {
    if string(node, "type") != "disk" {
        return None;
    }
    let path = string(node, "name");
    if !device_path_ok(&path) || ignored(&path) {
        return None;
    }
    // Everything that would have to be guessed makes the disk Unknown.
    let mut complete = true;
    let size = number(node, "size").unwrap_or_else(|| {
        complete = false;
        0
    });
    let read_only = flag(node, "ro").unwrap_or_else(|| {
        complete = false;
        true
    });
    let major_minor = string(node, "maj:min");
    complete &= !major_minor.is_empty();
    let mountpoints = mountpoints(node).unwrap_or_else(|| {
        complete = false;
        Vec::new()
    });
    let logical_sector = number(node, "log-sec")
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n >= 512 && n.is_power_of_two())
        .unwrap_or_else(|| {
            complete = false;
            512
        });
    // A signature on the disk node itself: something made on the whole disk
    // with no partition table (a "superfloppy"), or a table-less RAID or
    // encryption member. Read whatever the table says, because a member of
    // an array that carries a table inside it shows both.
    let fs_type = string(node, "fstype");
    let content = (!fs_type.is_empty()).then(|| classify("", &fs_type));
    let table = match string(node, "pttype").as_str() {
        "gpt" => Some(Table::Gpt),
        "dos" | "msdos" => Some(Table::Msdos),
        "" => None,
        // A table this installer does not know how to add to: it can still be
        // erased as a whole, but nothing on it can be judged.
        _ => {
            complete = false;
            None
        }
    };

    let mut holder = facts.holders.contains(&path);
    complete &= !facts.holders_unreadable.contains(&path);
    let mut read_only_part = false;
    let mut partitions = Vec::new();
    for child in children(node) {
        if string(child, "type") != "part" {
            // RAID, encryption or device-mapper directly on the disk.
            holder = true;
            continue;
        }
        let (partition, part_complete, part_ro) = partition_from(child, table);
        complete &= part_complete;
        read_only_part |= part_ro;
        holder |= facts.holders.contains(&partition.path) || children(child).next().is_some();
        complete &= !facts.holders_unreadable.contains(&partition.path);
        partitions.push(partition);
    }
    // Partitions with no table to hold them are something this reading does
    // not understand.
    complete &= table.is_some() || partitions.is_empty();

    let swaps = facts.swaps.as_deref().map(active_swaps);
    complete &= swaps.is_some();
    let swaps = swaps.unwrap_or_default();

    let medium = is_medium(node, &partitions, &mountpoints, facts);
    let all_mounts: Vec<String> = mountpoints
        .iter()
        .chain(partitions.iter().flat_map(|p| p.mountpoints.iter()))
        .cloned()
        .collect();
    let in_use: Vec<String> = all_mounts
        .iter()
        .filter(|m| *m != "[SWAP]")
        .cloned()
        .collect();
    let swapping = all_mounts.iter().any(|m| m == "[SWAP]")
        || swaps.contains(&path)
        || partitions.iter().any(|p| swaps.contains(&p.path));

    // In order of what is most useful to be told: a reason that cannot be
    // undone (this is the stick the system runs from) before one that can.
    let blocked = if medium {
        Some(Blocked::InstallationMedia)
    } else if !complete {
        Some(Blocked::Unknown)
    } else if read_only || read_only_part {
        Some(Blocked::ReadOnly)
    } else if holder {
        Some(Blocked::Holder)
    } else if !in_use.is_empty() {
        Some(Blocked::InUse {
            mountpoints: in_use,
        })
    } else if swapping {
        Some(Blocked::Swap)
    } else if size < MIN_INSTALL {
        Some(Blocked::TooSmall)
    } else {
        None
    };

    // Free space beside something written over the whole disk is not free:
    // a partition made there would be written into it.
    let free = match table {
        Some(table) if complete && content.is_none() => free_gaps(
            table,
            size,
            logical_sector,
            &partitions,
            &facts.extended_sizes,
        ),
        _ => Vec::new(),
    };

    Some(Disk {
        transport: transport(&string(node, "tran"), &path),
        path,
        diskseq: number(node, "disk-seq"),
        model: string(node, "model"),
        serial: string(node, "serial"),
        size,
        major_minor,
        logical_sector,
        removable: flag(node, "rm").unwrap_or(false),
        rotational: flag(node, "rota").unwrap_or(false),
        table,
        content,
        read_only,
        blocked,
        partitions,
        free,
    })
}

/// A partition of a disk whose table is `table`, whether everything needed
/// to judge it was there, and whether it is read-only.
fn partition_from(node: &Value, table: Option<Table>) -> (Partition, bool, bool) {
    let path = string(node, "name");
    let mut complete = device_path_ok(&path);
    let mut need = |value: Option<u64>| {
        value.unwrap_or_else(|| {
            complete = false;
            0
        })
    };
    let partn = need(number(node, "partn"));
    let start = need(number(node, "start").and_then(|s| s.checked_mul(512)));
    let size = need(number(node, "size"));
    let number = u32::try_from(partn).unwrap_or(0);
    complete &= number > 0;
    complete &= !string(node, "maj:min").is_empty();
    let read_only = flag(node, "ro");
    complete &= read_only.is_some();
    let mounts = mountpoints(node);
    complete &= mounts.is_some();
    let part_type = normal_part_type(node);
    let fs_type = string(node, "fstype");
    let bootable = match table {
        Some(Table::Msdos) => boot_flag(node).unwrap_or_else(|| {
            complete = false;
            false
        }),
        _ => false,
    };
    let partition = Partition {
        content: classify(&part_type, &fs_type),
        path,
        number,
        start,
        size,
        fs_type,
        label: string(node, "label"),
        part_type,
        uuid: string(node, "uuid"),
        mountpoints: mounts.unwrap_or_default(),
        bootable,
    };
    (partition, complete, read_only.unwrap_or(true))
}

/// Whether an msdos partition is marked active, from lsblk's `PARTFLAGS`:
/// libblkid gives an msdos entry's boot indicator as its flags — `0x80`
/// where it is marked, none where it is not (udev and lsblk leave flags of
/// nought out) — and finds no table at all in one whose indicator is
/// anything else. `None` for what is neither, which leaves the disk
/// unknown rather than guessed at.
fn boot_flag(node: &Value) -> Option<bool> {
    let flags = match node.get("partflags") {
        None | Some(Value::Null) => return Some(false),
        Some(Value::String(text)) if text.trim().is_empty() => return Some(false),
        Some(Value::String(text)) => text.trim().to_ascii_lowercase(),
        Some(_) => return None,
    };
    match u64::from_str_radix(flags.strip_prefix("0x")?, 16).ok()? {
        0 => Some(false),
        0x80 => Some(true),
        _ => None,
    }
}

/// What a partition appears to hold, from its type first (an EFI system
/// partition is one whatever is on it) and its filesystem second.
pub fn classify(part_type: &str, fs_type: &str) -> Content {
    match part_type {
        part_type::ESP | part_type::MBR_ESP => return Content::Efi,
        part_type::BIOS_BOOT => return Content::BiosBoot,
        part_type::MICROSOFT_RESERVED => return Content::MicrosoftReserved,
        part_type::WINDOWS_RECOVERY | part_type::MBR_WINDOWS_RECOVERY => {
            return Content::WindowsRecovery
        }
        part_type::LINUX_SWAP | part_type::MBR_SWAP => return Content::Swap,
        _ => {}
    }
    match fs_type {
        "ntfs" | "BitLocker" => Content::Windows,
        "vfat" | "exfat" | "msdos" => Content::Data,
        "ext2" | "ext3" | "ext4" | "btrfs" | "xfs" | "f2fs" | "jfs" | "reiserfs" | "bcachefs"
        | "nilfs2" => Content::Linux,
        "swap" => Content::Swap,
        "" if data_type(part_type) => Content::Empty,
        _ => Content::Other,
    }
}

/// Partition types meant for data, which hold nothing when they hold no
/// filesystem (as opposed to a type that belongs to something else).
fn data_type(part_type: &str) -> bool {
    matches!(
        part_type,
        "" | part_type::BASIC_DATA
            | part_type::LINUX
            | part_type::LINUX_ROOT_X86_64
            | part_type::LINUX_HOME
            | part_type::MBR_LINUX
            | part_type::MBR_NTFS
            | part_type::MBR_FAT32
            | part_type::MBR_FAT32_LBA
            | part_type::MBR_FAT16_LBA
    )
}

fn mbr_extended(part_type: &str) -> bool {
    matches!(
        part_type,
        part_type::MBR_EXTENDED | part_type::MBR_EXTENDED_LBA | part_type::MBR_EXTENDED_LINUX
    )
}

/// lsblk's `PARTTYPE`, lower case, with a DOS type padded to two hex digits.
fn normal_part_type(node: &Value) -> String {
    let raw = string(node, "parttype").to_ascii_lowercase();
    match raw
        .strip_prefix("0x")
        .map(|hex| u8::from_str_radix(hex, 16))
    {
        Some(Ok(value)) => format!("0x{value:02x}"),
        _ => raw,
    }
}

/// Whether the disk carries the live system: its filesystem UUID or label is
/// the one the kernel was told to find the medium by, or archiso has
/// something of it mounted.
fn is_medium(node: &Value, partitions: &[Partition], mounts: &[String], facts: &Facts) -> bool {
    let archiso_mount = mounts
        .iter()
        .chain(partitions.iter().flat_map(|p| p.mountpoints.iter()))
        .any(|m| m.starts_with(live::ARCHISO_RUN));
    let disk_device = string(node, "maj:min");
    let backs_a_mount = std::iter::once(disk_device.as_str())
        .chain(children(node).map(|c| c.get("maj:min").and_then(Value::as_str).unwrap_or("")))
        .any(|d| !d.is_empty() && facts.medium_devices.iter().any(|m| m == d));
    let named = facts.live.as_ref().is_some_and(|live| {
        let uuids =
            std::iter::once(string(node, "uuid")).chain(partitions.iter().map(|p| p.uuid.clone()));
        let labels = std::iter::once(string(node, "label"))
            .chain(partitions.iter().map(|p| p.label.clone()));
        let by_uuid = live.medium_uuid.as_deref().is_some_and(|wanted| {
            uuids
                .into_iter()
                .any(|u| !u.is_empty() && u.eq_ignore_ascii_case(wanted))
        });
        let by_label = live
            .medium_label
            .as_deref()
            .is_some_and(|wanted| labels.into_iter().any(|l| !l.is_empty() && l == wanted));
        by_uuid || by_label
    });
    archiso_mount || backs_a_mount || named
}

/// The device paths in a `/proc/swaps` text.
fn active_swaps(text: &str) -> BTreeSet<String> {
    text.lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .map(live::unescape)
        .collect()
}

/// The first and last byte offsets (end exclusive) a new partition may use on
/// a disk of `size` bytes with this table, both on a MiB boundary.
///
/// GPT keeps a backup header and entry array at the end of the disk; 33
/// logical sectors covers it for 128 entries (on a 4096-byte disk it needs
/// fewer, so this leaves a little more than needed). A DOS table counts
/// sectors in 32 bits, so nothing past the first 2³² sectors (2 TiB on
/// 512-byte sectors) is reachable by it. Everything starts at 1 MiB, which
/// is where every tool of the last fifteen years puts the first partition.
pub fn usable(table: Table, size: u64, logical_sector: u32) -> (u64, u64) {
    let sector = u64::from(logical_sector.max(512));
    let end = match table {
        Table::Gpt => size.saturating_sub(33 * sector),
        Table::Msdos => size.min((1u64 << 32) * sector),
    };
    (MIB, align_down(end))
}

/// The runs of free space on a disk, aligned to 1 MiB at both ends, in disk
/// order, and only those of at least [`MIN_GAP`].
///
/// On a DOS table an extended partition counts as occupied from its start to
/// its end, so free space inside it (between logical partitions) is never
/// offered: a new primary partition cannot go there. The kernel shows an
/// extended partition as a 1 KiB stub; its real end is the table's own size
/// for it when udev recorded one (`extended`), and otherwise is taken to be as
/// far as it could possibly reach — the next primary partition after it, or
/// the end of the disk. That may hide real free space after it, which is the
/// safe way to be wrong.
pub fn free_gaps(
    table: Table,
    size: u64,
    logical_sector: u32,
    partitions: &[Partition],
    extended: &BTreeMap<String, u64>,
) -> Vec<Gap> {
    let (first, last) = usable(table, size, logical_sector);
    let mut taken: Vec<(u64, u64)> = partitions
        .iter()
        .map(|p| {
            let mut end = p.start.saturating_add(p.size);
            if table == Table::Msdos && p.is_extended() {
                end = end.max(extended_end(p, partitions, extended, last));
            }
            (p.start, end)
        })
        .collect();
    taken.sort_unstable();

    let mut gaps = Vec::new();
    let mut cursor = first;
    for (start, end) in taken {
        if start > cursor {
            push_gap(&mut gaps, cursor, start.min(last));
        }
        cursor = cursor.max(end);
    }
    push_gap(&mut gaps, cursor, last);
    gaps
}

fn extended_end(
    extended: &Partition,
    partitions: &[Partition],
    table_sizes: &BTreeMap<String, u64>,
    last: u64,
) -> u64 {
    // Logical partitions are numbered from 5 and all lie inside it.
    let logical_end = partitions
        .iter()
        .filter(|p| p.number > 4)
        .map(|p| p.start.saturating_add(p.size))
        .max()
        .unwrap_or(0);
    let table_end = match table_sizes.get(&extended.path) {
        Some(size) => extended.start.saturating_add(*size),
        None => partitions
            .iter()
            .filter(|p| p.number <= 4 && p.start > extended.start)
            .map(|p| p.start)
            .min()
            .unwrap_or(last),
    };
    logical_end.max(table_end)
}

fn push_gap(gaps: &mut Vec<Gap>, from: u64, to: u64) {
    let start = align_up(from);
    let end = align_down(to);
    if end > start && end - start >= MIN_GAP {
        gaps.push(Gap {
            start,
            size: end - start,
        });
    }
}

fn align_up(bytes: u64) -> u64 {
    bytes.div_ceil(MIB).saturating_mul(MIB)
}

fn align_down(bytes: u64) -> u64 {
    bytes / MIB * MIB
}

fn transport(tran: &str, path: &str) -> Transport {
    let name = path.trim_start_matches("/dev/");
    match tran {
        "nvme" => Transport::Nvme,
        "sata" | "ata" => Transport::Sata,
        "usb" => Transport::Usb,
        "mmc" => Transport::Mmc,
        "virtio" => Transport::Virtio,
        // Older lsblk leaves TRAN empty for some buses the name still tells.
        "" if name.starts_with("nvme") => Transport::Nvme,
        "" if name.starts_with("vd") => Transport::Virtio,
        "" if name.starts_with("mmcblk") => Transport::Mmc,
        _ => Transport::Other,
    }
}

/// Block devices lsblk calls disks that are never somewhere to install.
fn ignored(path: &str) -> bool {
    let name = path.trim_start_matches("/dev/");
    let numbered = |prefix: &str| {
        name.strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|c| c.is_ascii_digit()))
    };
    let emmc_area = name.starts_with("mmcblk") && (name.contains("boot") || name.contains("rpmb"));
    ["zram", "loop", "ram", "sr", "fd"]
        .iter()
        .any(|prefix| numbered(prefix))
        || emmc_area
}

/// `/dev/<name>` with a plain kernel name: what lsblk `--paths` prints for
/// every disk and partition. Anything else is not something discovery made.
pub fn device_path_ok(path: &str) -> bool {
    path.strip_prefix("/dev/").is_some_and(|name| {
        !name.is_empty()
            && name.len() < 64
            && !name.starts_with('.')
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    })
}

fn every_node(tree: &Value) -> Vec<&Value> {
    fn walk<'a>(node: &'a Value, out: &mut Vec<&'a Value>) {
        out.push(node);
        for child in children(node) {
            walk(child, out);
        }
    }
    let mut out = Vec::new();
    for node in tree
        .get("blockdevices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        walk(node, &mut out);
    }
    out
}

fn children(node: &Value) -> impl Iterator<Item = &Value> {
    node.get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// A text column; empty when lsblk has nothing (`null`) for it.
fn string(node: &Value, key: &str) -> String {
    node.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

/// A number column. util-linux before 2.33 wrote numbers as strings, so
/// both are read.
fn number(node: &Value, key: &str) -> Option<u64> {
    match node.get(key)? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// A yes/no column: `true`/`false`, or `0`/`1` from older util-linux.
fn flag(node: &Value, key: &str) -> Option<bool> {
    match node.get(key)? {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_u64().map(|n| n != 0),
        Value::String(s) => match s.trim() {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        },
        _ => None,
    }
}

/// `MOUNTPOINTS`: util-linux 2.42 writes `[]` for nothing mounted and older
/// releases `[null]`; both mean none. A missing column is `None`.
fn mountpoints(node: &Value) -> Option<Vec<String>> {
    Some(
        node.get("mountpoints")?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .filter(|m| !m.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

/// What the chosen place holds once the partitions the installer must add
/// have been carved out of it. [`validate_storage`] judges sizes from it;
/// the interface can say how much room Linexin gets, and the storage half of
/// the worker decides from the same answer whether it adds an EFI or BIOS
/// boot partition.
///
/// Whether a reused EFI system partition has room for the kernels is only
/// known once the worker has looked inside it, which the interface cannot.
/// The room is therefore counted as if a Linux boot partition were made
/// whenever one is reused: a place offered is never one the worker then
/// finds too small or without a free table entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    /// Bytes left for the root filesystem.
    pub root: u64,
    /// The table the disk has once Linexin is installed: a new one for a
    /// whole disk (GPT on UEFI, DOS on BIOS), else the one it has.
    pub table: Table,
    /// An EFI system partition of [`NEW_ESP`] is made: UEFI, and the disk is
    /// erased or has none to reuse.
    pub new_esp: bool,
    /// A BIOS boot partition of [`BIOS_BOOT`] is made: BIOS on a GPT disk
    /// that has none.
    pub new_bios_boot: bool,
    /// A Linux boot partition of [`LINUX_BOOT`] is counted: UEFI, and the
    /// disk's own EFI system partition is reused. The worker makes it only
    /// when it finds too little room for the kernels in that partition.
    pub linux_boot: bool,
}

/// The room the chosen target gives, or why the target cannot be used at
/// all: it is not on the disk as reviewed (`TargetMissing`), it is a
/// partition the firmware starts from (`TargetIsEsp`) or one Windows keeps
/// for itself (`TargetProtected`), or the table has no entry left for the
/// partitions that must be added (`TableFull`).
///
/// A gap must be exactly one of `Disk::free`. A replaced partition keeps its
/// place and its entry unless an EFI, BIOS boot or Linux boot partition has
/// to be carved out of its space; then it is re-created after them, which
/// needs a free entry — and on a DOS table a primary one, so a logical
/// partition cannot give up space for one (deleting it would renumber the
/// logical partitions after it, and firmware only looks at primaries).
///
/// A layout made by hand has no one place to give room in: `LayoutRefused`,
/// its sizes being the rule set's to judge (`layout::shape`), as
/// [`validate_storage`] hands them to it.
pub fn room(config: &InstallConfig, firmware: Firmware) -> Result<Room, Invalid> {
    let disk = &config.disk;
    match &config.target {
        Target::Layout { .. } => Err(Invalid::LayoutRefused),
        Target::WholeDisk => {
            let table = match firmware {
                Firmware::Uefi => Table::Gpt,
                Firmware::Bios => Table::Msdos,
            };
            let (first, last) = usable(table, disk.size, disk.logical_sector);
            let span = last.saturating_sub(first);
            let new_esp = firmware == Firmware::Uefi;
            Ok(Room {
                root: span.saturating_sub(if new_esp { NEW_ESP } else { 0 }),
                table,
                new_esp,
                new_bios_boot: false,
                linux_boot: false,
            })
        }
        Target::FreeSpace { start, size } => {
            let table = disk.table.ok_or(Invalid::TargetMissing)?;
            let gap = Gap {
                start: *start,
                size: *size,
            };
            if !disk.free.contains(&gap) {
                return Err(Invalid::TargetMissing);
            }
            // Beside an extended partition a gap can only take a logical
            // partition, whose place sfdisk would choose itself; the worker
            // refuses to guess at it, so only replacing a part is offered.
            if table == Table::Msdos && disk.partitions.iter().any(Partition::is_extended) {
                return Err(Invalid::TableFull);
            }
            let (new_esp, new_bios_boot, linux_boot) = boot_parts(disk, firmware, table);
            let needed = 1 + extra_parts(new_esp, new_bios_boot, linux_boot);
            if free_entries(disk, table) < needed {
                return Err(Invalid::TableFull);
            }
            Ok(Room {
                root: carve(*size, new_esp, new_bios_boot, linux_boot),
                table,
                new_esp,
                new_bios_boot,
                linux_boot,
            })
        }
        Target::Partition { path } => {
            let table = disk.table.ok_or(Invalid::TargetMissing)?;
            let partition = disk.partition(path).ok_or(Invalid::TargetMissing)?;
            if !device_path_ok(path) || partition.is_extended() {
                return Err(Invalid::TargetMissing);
            }
            match partition.content {
                Content::Efi | Content::BiosBoot => return Err(Invalid::TargetIsEsp),
                // Windows needs its reserved partition to manage its disk and
                // its recovery partition to repair and update itself, so
                // neither is given up even though both look like spare room.
                Content::MicrosoftReserved | Content::WindowsRecovery => {
                    return Err(Invalid::TargetProtected)
                }
                _ => {}
            }
            let (new_esp, new_bios_boot, linux_boot) = boot_parts(disk, firmware, table);
            let extra = extra_parts(new_esp, new_bios_boot, linux_boot);
            if extra == 0 {
                return Ok(Room {
                    root: partition.size,
                    table,
                    new_esp,
                    new_bios_boot,
                    linux_boot,
                });
            }
            let primary = table == Table::Gpt || partition.number as usize <= MBR_PRIMARIES;
            if !primary || free_entries(disk, table) < extra {
                return Err(Invalid::TableFull);
            }
            let end = align_down(partition.start.saturating_add(partition.size));
            let span = end.saturating_sub(align_up(partition.start));
            Ok(Room {
                root: carve(span, new_esp, new_bios_boot, linux_boot),
                table,
                new_esp,
                new_bios_boot,
                linux_boot,
            })
        }
    }
}

/// The partitions a gap or a replaced partition must give room for besides
/// the root, on a disk that keeps its table: a new EFI system partition
/// under UEFI when the disk has none, else a Linux boot partition counted
/// beside the one it has; a BIOS boot partition under BIOS on a GPT disk
/// that has none.
fn boot_parts(disk: &Disk, firmware: Firmware, table: Table) -> (bool, bool, bool) {
    let has_esp = disk.esp().is_some();
    let has_bios_boot = disk
        .partitions
        .iter()
        .any(|p| p.content == Content::BiosBoot);
    let uefi = firmware == Firmware::Uefi;
    let new_bios_boot = firmware == Firmware::Bios && table == Table::Gpt && !has_bios_boot;
    (uefi && !has_esp, new_bios_boot, uefi && has_esp)
}

fn extra_parts(new_esp: bool, new_bios_boot: bool, linux_boot: bool) -> usize {
    usize::from(new_esp) + usize::from(new_bios_boot) + usize::from(linux_boot)
}

fn carve(span: u64, new_esp: bool, new_bios_boot: bool, linux_boot: bool) -> u64 {
    span.saturating_sub(if new_esp { NEW_ESP } else { 0 })
        .saturating_sub(if new_bios_boot { BIOS_BOOT } else { 0 })
        .saturating_sub(if linux_boot { LINUX_BOOT } else { 0 })
}

/// Entries left in the table for new partitions: the four primaries of a DOS
/// table (an extended partition holds one, its logical partitions none), or
/// GPT's usual 128.
fn free_entries(disk: &Disk, table: Table) -> usize {
    match table {
        Table::Msdos => {
            let primaries = disk
                .partitions
                .iter()
                .filter(|p| (1..=MBR_PRIMARIES as u32).contains(&p.number))
                .count();
            MBR_PRIMARIES.saturating_sub(primaries)
        }
        Table::Gpt => GPT_ENTRIES.saturating_sub(disk.partitions.len()),
    }
}

/// The storage half of the checks (target, /home, swap, boot loader for
/// this firmware and disk). Pure: judged from `config.disk` as given, so the
/// worker runs it on the disk the person reviewed and separately makes sure
/// the machine still has that disk exactly.
///
/// A separate /home is a partition of `config.disk`, or of the disk the
/// request carries for it (`Home::disk`), judged from that disk as it was
/// reviewed; the worker finds that disk again as it does the target. On the
/// disk Linexin goes on it cannot be the replaced partition, and a
/// whole-disk installation keeps none there: the disk is erased.
///
/// A layout made by hand is handed to the rule set (`model::layout`), as
/// [`validate_layout`] says.
pub fn validate_storage(config: &InstallConfig, firmware: Firmware) -> Result<(), Invalid> {
    let disk = &config.disk;
    if disk.blocked.is_some() || disk.read_only || !device_path_ok(&disk.path) {
        return Err(Invalid::DiskBlocked);
    }
    if let Target::Layout { disks: laid } = &config.target {
        return validate_layout(config, laid, firmware);
    }
    let room = room(config, firmware)?;
    if room.root < MIN_INSTALL {
        return Err(Invalid::TargetTooSmall);
    }
    if let Some(home) = &config.storage.home {
        validate_home(config, home)?;
    }
    let swap = u64::from(config.storage.swap_mib) * MIB;
    if swap > (room.root / 4).min(MAX_SWAP) {
        return Err(Invalid::SwapTooLarge);
    }
    if !bootloader_possible(config.bootloader, firmware, room.table) {
        return Err(Invalid::BootloaderUnavailable);
    }
    if !source_ok(&config.source) {
        return Err(Invalid::SourceInvalid);
    }
    Ok(())
}

/// [`validate_storage`] for a layout made by hand, judged by the rule set
/// the interface judges a plan by (`model::layout`), from the disks as they
/// were reviewed: the request's easy fields its pins (`layout::pinned`: a
/// swap file too large for `/` is `SwapTooLarge`, as on the easy path);
/// then its shape on this firmware, with the boot loader and the restore
/// points asked for (`layout::shape`: a boot loader that cannot start it is
/// `BootloaderUnavailable`, as on the easy path, which is how Desktop
/// outlines one); then the image's path. Anything else the rule set refuses
/// is `LayoutRefused`; what needs reading with privilege (`layout::measured`)
/// is not judged here.
fn validate_layout(
    config: &InstallConfig,
    laid: &[DiskLayout],
    firmware: Firmware,
) -> Result<(), Invalid> {
    layout::pinned(config).map_err(|unpinned| match unpinned {
        Unpinned::SwapTooLarge => Invalid::SwapTooLarge,
        _ => Invalid::LayoutRefused,
    })?;
    layout::shape(laid, firmware, config.bootloader, &config.software).map_err(|refusal| {
        match refusal {
            Refusal::Bootloader { .. } => Invalid::BootloaderUnavailable,
            _ => Invalid::LayoutRefused,
        }
    })?;
    if !source_ok(&config.source) {
        return Err(Invalid::SourceInvalid);
    }
    Ok(())
}

/// The separate /home's partition and the disk it is on: the disk the
/// request carries for it, or the one Linexin goes on.
pub fn home_partition<'a>(
    config: &'a InstallConfig,
    home: &'a Home,
) -> Option<(&'a Disk, &'a Partition)> {
    let disk = home.disk.as_ref().unwrap_or(&config.disk);
    disk.partition(&home.path).map(|part| (disk, part))
}

/// Whether a disk other than the one Linexin goes on may hold the person's
/// files: nothing in the way of writing to it now. A disk too small for
/// Linexin can still keep a home.
pub fn holds_homes(disk: &Disk) -> bool {
    matches!(disk.blocked, None | Some(Blocked::TooSmall))
        && !disk.read_only
        && device_path_ok(&disk.path)
}

fn validate_home(config: &InstallConfig, home: &Home) -> Result<(), Invalid> {
    match &home.disk {
        None => {
            if config.target == Target::WholeDisk {
                return Err(Invalid::HomeInvalid);
            }
            if matches!(&config.target, Target::Partition { path } if *path == home.path) {
                return Err(Invalid::HomeInvalid);
            }
        }
        // Another disk is never the one Linexin goes on under another name.
        Some(other) => {
            if other.path == config.disk.path || !holds_homes(other) {
                return Err(Invalid::HomeInvalid);
            }
        }
    }
    let (_, partition) = home_partition(config, home).ok_or(Invalid::HomeInvalid)?;
    if partition.size < MIN_HOME || partition.is_extended() || !device_path_ok(&partition.path) {
        return Err(Invalid::HomeInvalid);
    }
    match partition.content {
        Content::Efi
        | Content::BiosBoot
        | Content::MicrosoftReserved
        | Content::WindowsRecovery => Err(Invalid::HomeInvalid),
        Content::Linux => Ok(()),
        // Kept as it is, a /home has to be a filesystem Linux owns: an NTFS
        // or FAT partition kept as /home would have its owner changed under
        // a Windows that still uses it, and could not hold a home anyway.
        _ if home.format => Ok(()),
        _ => Err(Invalid::HomeInvalid),
    }
}

/// systemd-boot needs UEFI and a GPT disk; rEFInd needs UEFI; GRUB and the
/// automatic choice work everywhere. A shared EFI partition's room does not
/// matter to any of them: without room for the kernels, they are on a Linux
/// boot partition, which every loader reads.
pub fn bootloader_possible(choice: Bootloader, firmware: Firmware, table: Table) -> bool {
    match (choice, firmware) {
        (Bootloader::Automatic | Bootloader::Grub, _) => true,
        (Bootloader::SystemdBoot, Firmware::Uefi) => table == Table::Gpt,
        (Bootloader::Refind, Firmware::Uefi) => true,
        (Bootloader::SystemdBoot | Bootloader::Refind, Firmware::Bios) => false,
    }
}

/// An absolute path, written plainly: no `..`, no control characters, not
/// `/` itself. Whether it really is the live image is the worker's check.
fn source_ok(source: &Path) -> bool {
    let Some(text) = source.to_str() else {
        return false;
    };
    source.is_absolute()
        && source != Path::new("/")
        && text.len() < 4096
        && !text.chars().any(char::is_control)
        && source
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
}

/// The device paths an installation with `config` erases or formats, in a
/// stable order: what the confirmation dialog lists and what the worker
/// compares against `Request::consent`. A whole disk is its own path; a
/// replaced partition is that partition (also when an EFI partition is carved
/// out of it); a gap destroys nothing. A /home to be formatted comes after.
/// A reused ESP or a kept /home is never in it.
///
/// For a layout made by hand, every line of what it destroys, in the rule
/// set's fixed order (`layout::destroys`, `Destroy::line`) — a new table
/// with every partition it erases, each partition deleted, shrunk or
/// formatted, named by its path, number, start and sizes — which the plan's
/// paged question shows whole; its request has no separate /home
/// (`layout::pinned`).
pub fn consent(config: &InstallConfig) -> Vec<String> {
    let mut paths = Vec::new();
    match &config.target {
        Target::WholeDisk => paths.push(config.disk.path.clone()),
        Target::Partition { path } => paths.push(path.clone()),
        Target::FreeSpace { .. } => {}
        Target::Layout { disks } => {
            return layout::destroys(disks).iter().map(Destroy::line).collect()
        }
    }
    if let Some(home) = &config.storage.home {
        if home.format && !paths.contains(&home.path) {
            paths.push(home.path.clone());
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::{Account, Home, Region, Software, Storage, DEFAULT_SOURCE};

    // ---------------------------------------------------------------------
    // lsblk fixtures, hand-written in util-linux 2.42's shape. Every model,
    // serial and UUID is invented.

    fn disk_json(fields: &str, children: &str) -> String {
        format!(r#"{{"blockdevices":[{{{fields}, "children": [{children}]}}]}}"#)
    }

    fn facts() -> Facts {
        Facts {
            swaps: Some("Filename\tType\tSize\tUsed\tPriority\n".into()),
            ..Facts::default()
        }
    }

    fn only(json: &str, facts: &Facts) -> Disk {
        let disks = parse_lsblk(json, facts).unwrap();
        assert_eq!(disks.len(), 1, "{disks:?}");
        disks.into_iter().next().unwrap()
    }

    const EMPTY: &str = r#"{"blockdevices":[
        {"name":"/dev/sda", "type":"disk", "size":256060514304, "model":"Example SSD 860",
         "serial":"EXAMPLE-0001", "ro":false, "rm":false, "tran":"sata", "mountpoints":[],
         "maj:min":"8:0", "fstype":null, "label":null, "uuid":null, "parttype":null,
         "partn":null, "start":null, "pttype":null, "log-sec":512, "disk-seq":11}
    ]}"#;

    // A Windows disk: ESP, MSR, C: and the recovery partition at the very
    // end, with a gap of about 276 GiB between C: and recovery.
    const WINDOWS: &str = r#"{"blockdevices":[
        {"name":"/dev/sdb", "type":"disk", "size":512110190592, "model":"Example HDD 512",
         "serial":"EXAMPLE-0002", "ro":false, "rm":false, "tran":"sata", "mountpoints":[],
         "maj:min":"8:16", "fstype":null, "label":null, "uuid":null, "parttype":null,
         "partn":null, "start":null, "pttype":"gpt", "log-sec":512, "disk-seq":12,
         "children": [
            {"name":"/dev/sdb1", "type":"part", "size":104857600, "model":null, "serial":null,
             "ro":false, "rm":false, "tran":"sata", "mountpoints":[], "maj:min":"8:17",
             "fstype":"vfat", "label":null, "uuid":"AAAA-0001",
             "parttype":"c12a7328-f81f-11d2-ba4b-00a0c93ec93b", "partn":1, "start":2048,
             "pttype":"gpt", "log-sec":512, "disk-seq":12},
            {"name":"/dev/sdb2", "type":"part", "size":16777216, "model":null, "serial":null,
             "ro":false, "rm":false, "tran":"sata", "mountpoints":[], "maj:min":"8:18",
             "fstype":null, "label":null, "uuid":null,
             "parttype":"e3c9e316-0b5c-4db8-817d-f92df00215ae", "partn":2, "start":206848,
             "pttype":"gpt", "log-sec":512, "disk-seq":12},
            {"name":"/dev/sdb3", "type":"part", "size":214748364800, "model":null, "serial":null,
             "ro":false, "rm":false, "tran":"sata", "mountpoints":[], "maj:min":"8:19",
             "fstype":"ntfs", "label":"Windows", "uuid":"0123456789ABCDEF",
             "parttype":"ebd0a0a2-b9e5-4433-87c0-68b6b72699c7", "partn":3, "start":239616,
             "pttype":"gpt", "log-sec":512, "disk-seq":12},
            {"name":"/dev/sdb4", "type":"part", "size":734003200, "model":null, "serial":null,
             "ro":false, "rm":false, "tran":"sata", "mountpoints":[], "maj:min":"8:20",
             "fstype":"ntfs", "label":null, "uuid":"FEDCBA9876543210",
             "parttype":"de94bba4-06d1-4d40-a16a-bfd50179d6ac", "partn":4, "start":998780928,
             "pttype":"gpt", "log-sec":512, "disk-seq":12}
         ]}
    ]}"#;

    fn nvme_json() -> String {
        let part = |n: u32, start: u64, size: u64, fs: &str, ty: &str| {
            format!(
                r#"{{"name":"/dev/nvme0n1p{n}", "type":"part", "size":{size}, "model":null,
                "serial":null, "ro":false, "rm":false, "tran":"nvme", "mountpoints":[null],
                "maj:min":"259:{n}", "fstype":"{fs}", "label":null, "uuid":"uuid-{n}",
                "parttype":"{ty}", "partn":{n}, "start":{start}, "pttype":"gpt",
                "log-sec":512, "disk-seq":13}}"#
            )
        };
        disk_json(
            r#""name":"/dev/nvme0n1", "type":"disk", "size":1000204886016,
            "model":"Example NVMe 1TB", "serial":"EXAMPLE-0003", "ro":false, "rm":false,
            "tran":"nvme", "mountpoints":[null], "maj:min":"259:0", "fstype":null,
            "label":null, "uuid":null, "parttype":null, "partn":null, "start":null,
            "pttype":"gpt", "log-sec":512, "disk-seq":13"#,
            &[
                part(1, 2048, 512 * MIB, "vfat", part_type::ESP),
                part(2, 1050624, 100 * GIB, "ext4", part_type::LINUX),
            ]
            .join(","),
        )
    }

    // A DOS disk: NTFS primary, an extended partition with two logical
    // partitions and room left inside it, and a primary after it. lsblk
    // shows the extended partition as the kernel's 1 KiB stub.
    fn msdos_json() -> String {
        let part = |n: u32, start_mib: u64, size: u64, fs: &str, ty: &str| {
            format!(
                r#"{{"name":"/dev/sdc{n}", "type":"part", "size":{size}, "model":null,
                "serial":null, "ro":false, "rm":false, "tran":"sata", "mountpoints":[],
                "maj:min":"8:{minor}", "fstype":{fs}, "label":null, "uuid":null,
                "parttype":"{ty}", "partn":{n}, "start":{start}, "pttype":"dos",
                "log-sec":512, "disk-seq":14}}"#,
                minor = 32 + n,
                start = start_mib * 2048,
                fs = if fs.is_empty() {
                    "null".to_string()
                } else {
                    format!("\"{fs}\"")
                },
            )
        };
        disk_json(
            r#""name":"/dev/sdc", "type":"disk", "size":500107862016,
            "model":"Example Disk 500", "serial":"EXAMPLE-0004", "ro":false, "rm":false,
            "tran":"sata", "mountpoints":[], "maj:min":"8:32", "fstype":null, "label":null,
            "uuid":null, "parttype":null, "partn":null, "start":null, "pttype":"dos",
            "log-sec":512, "disk-seq":14"#,
            &[
                part(1, 1, 100 * GIB, "ntfs", "0x7"),
                part(2, 102401, 1024, "", "0x5"),
                part(3, 400000, 20 * GIB, "ext4", "0x83"),
                part(5, 102402, 50 * GIB, "ext4", "0x83"),
                part(6, 153603, 4 * GIB, "swap", "0x82"),
            ]
            .join(","),
        )
    }

    // The live USB stick: an ISO image written whole, so the disk itself and
    // its first partition carry the ISO's UUID and label.
    const LIVE_STICK: &str = r#"{"blockdevices":[
        {"name":"/dev/sdd", "type":"disk", "size":30752636928, "model":"Example Stick",
         "serial":"EXAMPLE-0005", "ro":false, "rm":true, "tran":"usb", "mountpoints":[],
         "maj:min":"8:48", "fstype":"iso9660", "label":"EXAMPLE_LIVE",
         "uuid":"2001-02-03-04-05-06-00", "parttype":null, "partn":null, "start":null,
         "pttype":"dos", "log-sec":512, "disk-seq":15,
         "children": [
            {"name":"/dev/sdd1", "type":"part", "size":3221225472, "model":null, "serial":null,
             "ro":false, "rm":true, "tran":"usb", "mountpoints":[], "maj:min":"8:49",
             "fstype":"iso9660", "label":"EXAMPLE_LIVE", "uuid":"2001-02-03-04-05-06-00",
             "parttype":"0x0", "partn":1, "start":0, "pttype":"dos", "log-sec":512,
             "disk-seq":15},
            {"name":"/dev/sdd2", "type":"part", "size":157286400, "model":null, "serial":null,
             "ro":false, "rm":true, "tran":"usb", "mountpoints":[], "maj:min":"8:50",
             "fstype":"vfat", "label":"EXAMPLE_EFI", "uuid":"BBBB-0002", "parttype":"0xef",
             "partn":2, "start":6291456, "pttype":"dos", "log-sec":512, "disk-seq":15}
         ]}
    ]}"#;

    // A 4Kn disk: 4096-byte logical sectors, its one partition at 1 MiB —
    // which lsblk still gives as START 2048, in 512-byte units.
    const FOUR_K: &str = r#"{"blockdevices":[
        {"name":"/dev/sde", "type":"disk", "size":2000398934016, "model":"Example 4Kn 2TB",
         "serial":"EXAMPLE-0006", "ro":false, "rm":false, "tran":"sata", "mountpoints":[],
         "maj:min":"8:64", "fstype":null, "label":null, "uuid":null, "parttype":null,
         "partn":null, "start":null, "pttype":"gpt", "log-sec":4096, "disk-seq":16,
         "children": [
            {"name":"/dev/sde1", "type":"part", "size":107374182400, "model":null,
             "serial":null, "ro":false, "rm":false, "tran":"sata", "mountpoints":[],
             "maj:min":"8:65", "fstype":"btrfs", "label":"data", "uuid":"cccc-0003",
             "parttype":"0fc63daf-8483-4772-8e79-3d69d8477de4", "partn":1, "start":2048,
             "pttype":"gpt", "log-sec":4096, "disk-seq":16}
         ]}
    ]}"#;

    fn mounted_json(mount: &str) -> String {
        disk_json(
            r#""name":"/dev/sdf", "type":"disk", "size":120034123776, "model":"Example SSD 120",
            "serial":"EXAMPLE-0007", "ro":false, "rm":false, "tran":"sata", "mountpoints":[],
            "maj:min":"8:80", "fstype":null, "label":null, "uuid":null, "parttype":null,
            "partn":null, "start":null, "pttype":"gpt", "log-sec":512, "disk-seq":17"#,
            &format!(
                r#"{{"name":"/dev/sdf1", "type":"part", "size":120032591872, "model":null,
                "serial":null, "ro":false, "rm":false, "tran":"sata", "mountpoints":[{mount}],
                "maj:min":"8:81", "fstype":"ext4", "label":null, "uuid":"dddd-0004",
                "parttype":"0fc63daf-8483-4772-8e79-3d69d8477de4", "partn":1, "start":2048,
                "pttype":"gpt", "log-sec":512, "disk-seq":17}}"#
            ),
        )
    }

    fn live_session(uuid: Option<&str>, label: Option<&str>) -> LiveSession {
        LiveSession {
            image: live::IMAGE.into(),
            medium_uuid: uuid.map(str::to_string),
            medium_label: label.map(str::to_string),
            medium_mounted: false,
        }
    }

    // ---------------------------------------------------------------------
    // Discovery

    #[test]
    fn an_empty_disk_offers_only_itself() {
        let disk = only(EMPTY, &facts());
        assert_eq!(disk.path, "/dev/sda");
        assert_eq!(disk.diskseq, Some(11));
        assert_eq!(disk.model, "Example SSD 860");
        assert_eq!(disk.transport, Transport::Sata);
        assert_eq!(disk.table, None);
        assert_eq!(disk.blocked, None);
        assert!(disk.partitions.is_empty());
        assert!(disk.free.is_empty());
        assert_eq!(disk.content, None);
        assert!(disk.is_blank());
        assert!(!disk.rotational, "a disk that does not say it spins");
    }

    #[test]
    fn a_disk_that_spins_says_so_in_whichever_form_lsblk_writes_it() {
        for rota in ["true", "1", "\"1\""] {
            let json = EMPTY.replace(r#""rm":false,"#, &format!(r#""rm":false, "rota":{rota},"#));
            assert!(only(&json, &facts()).rotational, "{rota}");
        }
        let json = EMPTY.replace(r#""rm":false,"#, r#""rm":false, "rota":false,"#);
        assert!(!only(&json, &facts()).rotational);
    }

    /// The empty disk with `fstype` written straight onto it, no table.
    fn whole(fstype: &str) -> Disk {
        let json = EMPTY.replace(r#""fstype":null"#, &format!(r#""fstype":"{fstype}""#));
        only(&json, &facts())
    }

    #[test]
    fn what_is_written_over_a_whole_disk_is_what_it_holds() {
        for (fstype, content) in [
            ("ext4", Content::Linux),
            ("btrfs", Content::Linux),
            ("ntfs", Content::Windows),
            ("BitLocker", Content::Windows),
            ("vfat", Content::Data),
            ("exfat", Content::Data),
            ("swap", Content::Swap),
            ("crypto_LUKS", Content::Other),
            ("LVM2_member", Content::Other),
            ("linux_raid_member", Content::Other),
            ("zfs_member", Content::Other),
        ] {
            let disk = whole(fstype);
            assert_eq!(disk.content, Some(content), "{fstype}");
            assert!(!disk.is_blank(), "{fstype}");
            // It can still be erased as a whole, by the person's own choice,
            // but none of it is free space.
            assert_eq!(disk.blocked, None, "{fstype}");
            assert_eq!(disk.table, None, "{fstype}");
            assert!(disk.partitions.is_empty(), "{fstype}");
            assert!(disk.free.is_empty(), "{fstype}");
        }
    }

    #[test]
    fn a_disk_signature_beside_a_table_leaves_no_free_space() {
        // An array member whose array carries a table: lsblk shows both. The
        // gap the table seems to leave belongs to the array.
        let json = WINDOWS.replacen(r#""fstype":null"#, r#""fstype":"linux_raid_member""#, 1);
        let disk = only(&json, &facts());
        assert_eq!(disk.content, Some(Content::Other));
        assert_eq!(disk.partitions.len(), 4);
        assert!(disk.free.is_empty(), "{:?}", disk.free);
        assert!(!only(WINDOWS, &facts()).free.is_empty());
    }

    #[test]
    fn a_disk_written_over_whole_is_named_in_the_consent_when_erased() {
        let c = config(whole("ext4"), Target::WholeDisk);
        assert_eq!(validate_storage(&c, Firmware::Uefi), Ok(()));
        assert_eq!(consent(&c), ["/dev/sda"]);
        // No gap and no partition to take instead.
        let gap = config(
            whole("ext4"),
            Target::FreeSpace {
                start: MIB,
                size: 200 * GIB,
            },
        );
        assert_eq!(
            validate_storage(&gap, Firmware::Uefi),
            Err(Invalid::TargetMissing)
        );
    }

    #[test]
    fn a_windows_disk_is_read_partition_by_partition() {
        let disk = only(WINDOWS, &facts());
        assert_eq!(disk.table, Some(Table::Gpt));
        assert_eq!(disk.blocked, None);
        let contents: Vec<Content> = disk.partitions.iter().map(|p| p.content).collect();
        assert_eq!(
            contents,
            [
                Content::Efi,
                Content::MicrosoftReserved,
                Content::Windows,
                Content::WindowsRecovery
            ]
        );
        assert_eq!(disk.esp().unwrap().path, "/dev/sdb1");
        let c = disk.partition("/dev/sdb3").unwrap();
        assert_eq!(
            (c.number, c.start, c.label.as_str()),
            (3, 239616 * 512, "Windows")
        );
        // Between C: (ends at 204917 MiB) and recovery (starts at 487686 MiB);
        // recovery reaches the last usable MiB, so nothing after it.
        assert_eq!(
            disk.free,
            [Gap {
                start: 204917 * MIB,
                size: (487686 - 204917) * MIB
            }]
        );
    }

    #[test]
    fn an_nvme_disk_keeps_the_paths_lsblk_gives() {
        let disk = only(&nvme_json(), &facts());
        assert_eq!(disk.transport, Transport::Nvme);
        let paths: Vec<&str> = disk.partitions.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(paths, ["/dev/nvme0n1p1", "/dev/nvme0n1p2"]);
        assert_eq!(disk.partitions[1].content, Content::Linux);
        // `[null]` (older util-linux) is "nothing mounted".
        assert_eq!(disk.blocked, None);
        // p2 ends at 1 MiB + 512 MiB + 100 GiB; the disk's last usable MiB
        // is 953869.
        let p2_end = 1 + 512 + 100 * 1024;
        assert_eq!(
            disk.free,
            [Gap {
                start: p2_end * MIB,
                size: (953869 - p2_end) * MIB
            }]
        );
    }

    #[test]
    fn free_space_inside_an_extended_partition_is_never_offered() {
        let mut facts = facts();
        let disk = only(&msdos_json(), &facts);
        assert_eq!(disk.table, Some(Table::Msdos));
        let types: Vec<&str> = disk
            .partitions
            .iter()
            .map(|p| p.part_type.as_str())
            .collect();
        assert_eq!(types, ["0x07", "0x05", "0x83", "0x83", "0x82"]);
        assert!(disk.partitions[1].is_extended());
        assert!(!disk.partitions[1].replaceable());
        assert_eq!(disk.partitions[4].content, Content::Swap);
        // Without the table's own size for it, the extended partition is
        // taken to reach the next primary (at 400000 MiB): only the space
        // after that primary is offered.
        assert_eq!(
            disk.free,
            [Gap {
                start: 420480 * MIB,
                size: (476940 - 420480) * MIB
            }]
        );
        // With it (200 GiB, from udev), the space between the extended
        // partition's end and the primary appears too — but still nothing
        // between or after the logical partitions inside it.
        facts.extended_sizes.insert("/dev/sdc2".into(), 200 * GIB);
        let disk = only(&msdos_json(), &facts);
        assert_eq!(
            disk.free,
            [
                Gap {
                    start: 307201 * MIB,
                    size: (400000 - 307201) * MIB
                },
                Gap {
                    start: 420480 * MIB,
                    size: (476940 - 420480) * MIB
                }
            ]
        );
    }

    /// **An msdos partition's boot flag is read as udev read the table**
    /// (lsblk's `PARTFLAGS`): `0x80` marks it active, flags of nought or
    /// none leave it unmarked; on GPT the flags are attribute bits, never a
    /// boot flag; and on msdos anything else — which libblkid never gives
    /// an msdos entry — leaves the disk unknown rather than guessed at.
    #[test]
    fn an_msdos_partitions_boot_flag_is_read_and_a_gpt_ones_never() {
        let marked = |flags: &str| {
            msdos_json().replace(
                r#""parttype":"0x7", "partn":1,"#,
                &format!(r#""parttype":"0x7", "partflags":{flags}, "partn":1,"#),
            )
        };
        let disk = only(&marked(r#""0x80""#), &facts());
        assert_eq!(disk.blocked, None);
        let flags: Vec<bool> = disk.partitions.iter().map(|p| p.bootable).collect();
        assert_eq!(flags, [true, false, false, false, false]);
        for unmarked in [r#""0x0""#, "null", r#""  ""#] {
            let disk = only(&marked(unmarked), &facts());
            assert_eq!(disk.blocked, None, "{unmarked}");
            assert!(disk.partitions.iter().all(|p| !p.bootable), "{unmarked}");
        }
        // Without the column at all, as an older listing has it: unmarked.
        assert!(only(&msdos_json(), &facts())
            .partitions
            .iter()
            .all(|p| !p.bootable));
        for odd in [r#""0x81""#, r#""active""#, r#""80""#, "128"] {
            let disk = only(&marked(odd), &facts());
            assert_eq!(disk.blocked, Some(Blocked::Unknown), "{odd}");
            assert!(!disk.partitions[0].bootable, "{odd}");
        }
        // GPT attribute bits are no boot flag, whatever they say.
        for flags in [r#""0x80""#, r#""0x8000000000000004""#, r#""odd""#] {
            let json = FOUR_K.replace(
                r#""partn":1,"#,
                &format!(r#""partflags":{flags}, "partn":1,"#),
            );
            let disk = only(&json, &facts());
            assert_eq!(disk.blocked, None, "{flags}");
            assert!(!disk.partitions[0].bootable, "{flags}");
        }
    }

    /// The boot flag travels in a request only where it is set: a partition
    /// unmarked is written as it was before the flag was read, and one read
    /// from a request written without it is unmarked.
    #[test]
    fn the_boot_flag_is_written_only_where_it_is_set() {
        let mut partition = only(&msdos_json(), &facts()).partitions[0].clone();
        let unmarked = serde_json::to_string(&partition).unwrap();
        assert!(!unmarked.contains("bootable"), "{unmarked}");
        let back: Partition = serde_json::from_str(&unmarked).unwrap();
        assert_eq!(back, partition);
        partition.bootable = true;
        let marked = serde_json::to_string(&partition).unwrap();
        assert!(marked.contains(r#""bootable":true"#), "{marked}");
        assert_eq!(
            serde_json::from_str::<Partition>(&marked).unwrap(),
            partition
        );
    }

    #[test]
    fn mounted_partitions_block_their_disk() {
        let disk = only(&mounted_json(r#""/run/media/someone/Data""#), &facts());
        assert_eq!(
            disk.blocked,
            Some(Blocked::InUse {
                mountpoints: vec!["/run/media/someone/Data".into()]
            })
        );
        let disk = only(&mounted_json(r#""[SWAP]""#), &facts());
        assert_eq!(disk.blocked, Some(Blocked::Swap));
        // Swap the kernel lists that lsblk did not mark.
        let mut swapping = facts();
        swapping.swaps =
            Some("Filename Type Size Used Priority\n/dev/sdf1 partition 4194300 0 -2\n".into());
        let disk = only(&mounted_json(""), &swapping);
        assert_eq!(disk.blocked, Some(Blocked::Swap));
        let disk = only(&mounted_json(""), &facts());
        assert_eq!(disk.blocked, None);
    }

    #[test]
    fn the_live_medium_is_recognised_by_its_name_or_its_mount() {
        // copytoram: nothing mounted, but the kernel was told its UUID.
        let mut by_uuid = facts();
        by_uuid.live = Some(live_session(Some("2001-02-03-04-05-06-00"), None));
        assert_eq!(
            only(LIVE_STICK, &by_uuid).blocked,
            Some(Blocked::InstallationMedia)
        );
        let mut by_label = facts();
        by_label.live = Some(live_session(None, Some("EXAMPLE_LIVE")));
        assert_eq!(
            only(LIVE_STICK, &by_label).blocked,
            Some(Blocked::InstallationMedia)
        );
        // Mounted by archiso: known by the device behind the boot mount.
        let mut by_device = facts();
        by_device.medium_devices = vec!["8:49".into()];
        assert_eq!(
            only(LIVE_STICK, &by_device).blocked,
            Some(Blocked::InstallationMedia)
        );
        let mounted = LIVE_STICK.replacen(
            r#""mountpoints":[], "maj:min":"8:49""#,
            r#""mountpoints":["/run/archiso/bootmnt"], "maj:min":"8:49""#,
            1,
        );
        assert_eq!(
            only(&mounted, &facts()).blocked,
            Some(Blocked::InstallationMedia)
        );
        // Another live session's stick is just a small disk.
        let mut other = facts();
        other.live = Some(live_session(Some("1999-01-01-00-00-00-00"), None));
        assert_eq!(only(LIVE_STICK, &other).blocked, None);
        assert_eq!(only(LIVE_STICK, &other).transport, Transport::Usb);
        assert!(only(LIVE_STICK, &other).removable);
    }

    #[test]
    fn a_4kn_disk_counts_starts_in_512_byte_units() {
        let disk = only(FOUR_K, &facts());
        assert_eq!(disk.logical_sector, 4096);
        assert_eq!(disk.partitions[0].start, MIB);
        // The backup GPT reserves 33 × 4096 bytes, which here reaches back
        // into the disk's last whole MiB.
        assert_eq!(
            disk.free,
            [Gap {
                start: (1 + 100 * 1024) * MIB,
                size: (1907728 - 1 - 100 * 1024) * MIB
            }]
        );
    }

    #[test]
    fn devices_that_are_never_places_are_left_out() {
        let json = r#"{"blockdevices":[
            {"name":"/dev/zram0", "type":"disk", "size":8589934592, "ro":false,
             "mountpoints":["[SWAP]"], "maj:min":"252:0", "log-sec":4096},
            {"name":"/dev/loop0", "type":"loop", "size":3000000000, "ro":true,
             "mountpoints":["/run/archiso/airootfs"], "maj:min":"7:0", "log-sec":512},
            {"name":"/dev/sr0", "type":"rom", "size":3000000000, "ro":false,
             "mountpoints":[], "maj:min":"11:0", "log-sec":2048},
            {"name":"/dev/mmcblk0boot0", "type":"disk", "size":4194304, "ro":true,
             "mountpoints":[], "maj:min":"179:8", "log-sec":512}
        ]}"#;
        assert!(parse_lsblk(json, &facts()).unwrap().is_empty());
        assert!(parse_lsblk("{}", &facts()).is_err());
        assert!(parse_lsblk("not json", &facts()).is_err());
    }

    #[test]
    fn whatever_cannot_be_read_blocks_the_disk() {
        let no_majmin = EMPTY.replace(r#""maj:min":"8:0", "#, "");
        assert_eq!(only(&no_majmin, &facts()).blocked, Some(Blocked::Unknown));
        let no_mounts = EMPTY.replace(r#""mountpoints":[],"#, "");
        assert_eq!(only(&no_mounts, &facts()).blocked, Some(Blocked::Unknown));
        let no_start = WINDOWS.replace(r#""start":206848"#, r#""start":null"#);
        let disk = only(&no_start, &facts());
        assert_eq!(disk.blocked, Some(Blocked::Unknown));
        assert!(disk.free.is_empty());
        let odd_table = EMPTY.replace(r#""pttype":null"#, r#""pttype":"atari""#);
        assert_eq!(only(&odd_table, &facts()).blocked, Some(Blocked::Unknown));
        let mut no_swaps = facts();
        no_swaps.swaps = None;
        assert_eq!(only(EMPTY, &no_swaps).blocked, Some(Blocked::Unknown));
        let mut unreadable = facts();
        unreadable.holders_unreadable.insert("/dev/sdb3".into());
        assert_eq!(only(WINDOWS, &unreadable).blocked, Some(Blocked::Unknown));
    }

    #[test]
    fn holders_read_only_and_small_disks_are_blocked() {
        let mut held = facts();
        held.holders.insert("/dev/sdb3".into());
        assert_eq!(only(WINDOWS, &held).blocked, Some(Blocked::Holder));
        let crypt = WINDOWS.replace(
            r#""partn":3, "start":239616,"#,
            r#""partn":3, "start":239616, "children":[{"name":"/dev/mapper/secret",
            "type":"crypt"}],"#,
        );
        assert_eq!(only(&crypt, &facts()).blocked, Some(Blocked::Holder));
        let read_only = EMPTY.replace(r#""ro":false"#, r#""ro":true"#);
        assert_eq!(only(&read_only, &facts()).blocked, Some(Blocked::ReadOnly));
        let small = EMPTY.replace("256060514304", "16008609792");
        assert_eq!(only(&small, &facts()).blocked, Some(Blocked::TooSmall));
    }

    #[test]
    fn the_same_text_always_reads_the_same() {
        let a = parse_lsblk(&msdos_json(), &facts()).unwrap();
        let b = parse_lsblk(&msdos_json(), &facts()).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            serde_json::from_str::<Vec<Disk>>(&serde_json::to_string(&a).unwrap()).unwrap(),
            a
        );
    }

    #[test]
    fn partitions_are_classified_by_type_before_filesystem() {
        assert_eq!(classify(part_type::ESP, "vfat"), Content::Efi);
        assert_eq!(classify("0xef", ""), Content::Efi);
        assert_eq!(classify(part_type::BIOS_BOOT, ""), Content::BiosBoot);
        assert_eq!(classify(part_type::BASIC_DATA, "exfat"), Content::Data);
        assert_eq!(
            classify(part_type::BASIC_DATA, "BitLocker"),
            Content::Windows
        );
        assert_eq!(classify(part_type::LINUX_ROOT_X86_64, ""), Content::Empty);
        assert_eq!(classify(part_type::LINUX, "crypto_LUKS"), Content::Other);
        assert_eq!(classify(part_type::LINUX_SWAP, ""), Content::Swap);
        assert_eq!(classify("0x05", ""), Content::Other);
        assert_eq!(classify("0x27", "ntfs"), Content::WindowsRecovery);
    }

    #[test]
    fn udev_properties_are_read_by_name() {
        let entry =
            "S:disk/by-uuid/x\nE:ID_PART_ENTRY_SCHEME=dos\nE:ID_PART_ENTRY_SIZE=419430400\n";
        assert_eq!(
            udev_property(entry, "ID_PART_ENTRY_SIZE"),
            Some("419430400")
        );
        assert_eq!(udev_property(entry, "ID_PART_ENTRY"), None);
    }

    // ---------------------------------------------------------------------
    // Validation and consent

    fn config(disk: Disk, target: Target) -> InstallConfig {
        InstallConfig {
            disk,
            target,
            storage: Storage::default(),
            bootloader: Bootloader::Automatic,
            desktop: Default::default(),
            software: Software::default(),
            region: Region::default(),
            account: Account::default(),
            source: DEFAULT_SOURCE.into(),
        }
    }

    fn windows() -> Disk {
        only(WINDOWS, &facts())
    }

    fn windows_gap() -> Target {
        let gap = windows().free[0];
        Target::FreeSpace {
            start: gap.start,
            size: gap.size,
        }
    }

    fn partition(path: &str) -> Target {
        Target::Partition { path: path.into() }
    }

    #[test]
    fn a_whole_disk_is_erased_and_listed_whole() {
        let c = config(only(EMPTY, &facts()), Target::WholeDisk);
        assert_eq!(validate_storage(&c, Firmware::Uefi), Ok(()));
        assert_eq!(validate_storage(&c, Firmware::Bios), Ok(()));
        let uefi = room(&c, Firmware::Uefi).unwrap();
        assert_eq!((uefi.table, uefi.new_esp), (Table::Gpt, true));
        assert_eq!(room(&c, Firmware::Bios).unwrap().table, Table::Msdos);
        assert_eq!(consent(&c), ["/dev/sda"]);
    }

    #[test]
    fn a_blocked_disk_is_refused_whatever_else_is_chosen() {
        let mut disk = only(EMPTY, &facts());
        disk.blocked = Some(Blocked::Swap);
        let c = config(disk, Target::WholeDisk);
        assert_eq!(
            validate_storage(&c, Firmware::Uefi),
            Err(Invalid::DiskBlocked)
        );
    }

    #[test]
    fn a_gap_reuses_the_disks_efi_partition() {
        let c = config(windows(), windows_gap());
        assert_eq!(validate_storage(&c, Firmware::Uefi), Ok(()));
        let room = room(&c, Firmware::Uefi).unwrap();
        assert!(!room.new_esp);
        // Windows' 100 MiB partition may have no room for the kernels, which
        // only the worker can see, so a Linux boot partition is counted.
        assert!(room.linux_boot);
        assert_eq!(room.root, (487686 - 204917) * MIB - LINUX_BOOT);
        assert!(consent(&c).is_empty());
        // A gap that is not exactly one the disk listed is not a place.
        let mut moved = c.clone();
        moved.target = Target::FreeSpace {
            start: 204917 * MIB + MIB,
            size: (487686 - 204917) * MIB - MIB,
        };
        assert_eq!(
            validate_storage(&moved, Firmware::Uefi),
            Err(Invalid::TargetMissing)
        );
    }

    #[test]
    fn bios_on_gpt_adds_a_bios_boot_partition() {
        let c = config(windows(), windows_gap());
        let room = room(&c, Firmware::Bios).unwrap();
        assert!(room.new_bios_boot && !room.new_esp);
        assert_eq!(room.root, (487686 - 204917) * MIB - BIOS_BOOT);
        assert_eq!(validate_storage(&c, Firmware::Bios), Ok(()));
    }

    #[test]
    fn the_size_counts_after_a_new_efi_partition() {
        // A gap just over 25 GB on a disk without an EFI partition: enough
        // under BIOS, not once 2 GiB are carved out of it under UEFI.
        let mut disk = only(&msdos_json(), &facts());
        // Primary partitions only (1 and 3): beside an extended partition no
        // gap is a place at all.
        disk.partitions
            .retain(|p| p.number <= 4 && !p.is_extended());
        let size = 24_000 * MIB; // 25.17 GB
        disk.free = vec![Gap {
            start: 420480 * MIB,
            size,
        }];
        let c = config(
            disk,
            Target::FreeSpace {
                start: 420480 * MIB,
                size,
            },
        );
        assert_eq!(validate_storage(&c, Firmware::Bios), Ok(()));
        assert_eq!(
            validate_storage(&c, Firmware::Uefi),
            Err(Invalid::TargetTooSmall)
        );
        assert!(room(&c, Firmware::Uefi).unwrap().new_esp);
        // With one primary entry left, it is the table that stops it first.
        let mut full = c.clone();
        let mut third = full.disk.partition("/dev/sdc3").unwrap().clone();
        third.path = "/dev/sdc4".into();
        third.number = 4;
        third.start = 450000 * MIB;
        full.disk.partitions.push(third);
        assert_eq!(validate_storage(&full, Firmware::Bios), Ok(()));
        assert_eq!(
            validate_storage(&full, Firmware::Uefi),
            Err(Invalid::TableFull)
        );
    }

    #[test]
    fn a_gap_beside_an_extended_partition_is_not_a_place() {
        // Only a logical partition could go there, whose place sfdisk would
        // choose itself; the worker refuses it, so the interface does too.
        let disk = only(&msdos_json(), &facts());
        let gap = disk.free[0];
        let c = config(
            disk,
            Target::FreeSpace {
                start: gap.start,
                size: gap.size,
            },
        );
        for firmware in [Firmware::Uefi, Firmware::Bios] {
            assert_eq!(
                validate_storage(&c, firmware),
                Err(Invalid::TableFull),
                "{firmware:?}"
            );
        }
    }

    #[test]
    fn a_full_dos_table_takes_no_new_partition() {
        let mut disk = only(&msdos_json(), &facts());
        // Four primary entries in use (1, 2 = extended, 3, and a fourth).
        let mut fourth = disk.partition("/dev/sdc3").unwrap().clone();
        fourth.path = "/dev/sdc4".into();
        fourth.number = 4;
        fourth.start = 440000 * MIB;
        disk.partitions.push(fourth);
        let gap = disk.free[0];
        let c = config(
            disk,
            Target::FreeSpace {
                start: gap.start,
                size: gap.size,
            },
        );
        assert_eq!(
            validate_storage(&c, Firmware::Bios),
            Err(Invalid::TableFull)
        );
        // Replacing a partition in place needs no new entry, logical or not.
        let mut replace = c.clone();
        replace.target = partition("/dev/sdc5");
        assert_eq!(validate_storage(&replace, Firmware::Bios), Ok(()));
        assert_eq!(consent(&replace), ["/dev/sdc5"]);
    }

    #[test]
    fn a_logical_partition_cannot_give_room_for_an_efi_partition() {
        let mut c = config(only(&msdos_json(), &facts()), partition("/dev/sdc5"));
        assert_eq!(
            validate_storage(&c, Firmware::Uefi),
            Err(Invalid::TableFull)
        );
        // A primary one can, while an entry is free.
        c.target = partition("/dev/sdc1");
        let room = room(&c, Firmware::Uefi).unwrap();
        assert!(room.new_esp);
        assert_eq!(room.root, 100 * GIB - NEW_ESP);
        assert_eq!(validate_storage(&c, Firmware::Uefi), Ok(()));
    }

    #[test]
    fn system_partitions_are_never_replaced() {
        let mut c = config(windows(), partition("/dev/sdb1"));
        assert_eq!(
            validate_storage(&c, Firmware::Uefi),
            Err(Invalid::TargetIsEsp)
        );
        c.target = partition("/dev/sdb2");
        assert_eq!(
            validate_storage(&c, Firmware::Uefi),
            Err(Invalid::TargetProtected)
        );
        c.target = partition("/dev/sdb9");
        assert_eq!(
            validate_storage(&c, Firmware::Uefi),
            Err(Invalid::TargetMissing)
        );
        let mut dos = config(only(&msdos_json(), &facts()), partition("/dev/sdc2"));
        assert_eq!(
            validate_storage(&dos, Firmware::Bios),
            Err(Invalid::TargetMissing)
        );
        dos.target = windows_gap();
        assert_eq!(
            validate_storage(&dos, Firmware::Bios),
            Err(Invalid::TargetMissing)
        );
        // Windows itself may be replaced, counted as giving room to a Linux
        // boot partition too; it is still the one part erased.
        c.target = partition("/dev/sdb3");
        assert_eq!(validate_storage(&c, Firmware::Uefi), Ok(()));
        assert_eq!(
            room(&c, Firmware::Uefi).unwrap().root,
            200 * GIB - LINUX_BOOT
        );
        assert_eq!(consent(&c), ["/dev/sdb3"]);
    }

    #[test]
    fn partitions_windows_keeps_are_refused_whatever_their_size() {
        // Microsoft reserved and Windows recovery are refused as protected,
        // not as too small: a recovery partition grown past the minimum is
        // still Windows's, and neither is offered in the first place.
        let mut disk = windows();
        for p in &mut disk.partitions {
            if matches!(
                p.content,
                Content::MicrosoftReserved | Content::WindowsRecovery
            ) {
                p.size = 100 * GIB;
                assert!(!p.replaceable(), "{}", p.path);
            }
        }
        let mut c = config(disk, partition("/dev/sdb4"));
        for firmware in [Firmware::Uefi, Firmware::Bios] {
            assert_eq!(
                validate_storage(&c, firmware),
                Err(Invalid::TargetProtected)
            );
        }
        c.target = partition("/dev/sdb2");
        assert_eq!(
            validate_storage(&c, Firmware::Uefi),
            Err(Invalid::TargetProtected)
        );
        // The EFI system partition keeps its own reason.
        c.target = partition("/dev/sdb1");
        assert_eq!(
            validate_storage(&c, Firmware::Uefi),
            Err(Invalid::TargetIsEsp)
        );
    }

    #[test]
    fn home_is_another_partition_of_the_same_disk() {
        let mut dos = config(only(&msdos_json(), &facts()), partition("/dev/sdc1"));
        let home = |path: &str, format: bool| {
            Some(Home {
                path: path.into(),
                format,
                disk: None,
            })
        };
        // Kept: a Linux filesystem.
        dos.storage.home = home("/dev/sdc5", false);
        assert_eq!(validate_storage(&dos, Firmware::Bios), Ok(()));
        assert_eq!(consent(&dos), ["/dev/sdc1"]);
        // Formatted: listed after the target.
        dos.storage.home = home("/dev/sdc5", true);
        assert_eq!(consent(&dos), ["/dev/sdc1", "/dev/sdc5"]);
        // Not the target itself, not the extended container, not elsewhere.
        for bad in ["/dev/sdc1", "/dev/sdc2", "/dev/sdb3"] {
            dos.storage.home = home(bad, true);
            assert_eq!(
                validate_storage(&dos, Firmware::Bios),
                Err(Invalid::HomeInvalid),
                "{bad}"
            );
        }
        // Windows' partitions: never the ESP; NTFS only if it is formatted.
        let mut win = config(windows(), windows_gap());
        win.storage.home = home("/dev/sdb1", true);
        assert_eq!(
            validate_storage(&win, Firmware::Uefi),
            Err(Invalid::HomeInvalid)
        );
        win.storage.home = home("/dev/sdb3", false);
        assert_eq!(
            validate_storage(&win, Firmware::Uefi),
            Err(Invalid::HomeInvalid)
        );
        win.storage.home = home("/dev/sdb3", true);
        assert_eq!(validate_storage(&win, Firmware::Uefi), Ok(()));
        assert_eq!(consent(&win), ["/dev/sdb3"]);
        // A whole disk is erased, so it has no partition left to keep.
        let mut whole = config(windows(), Target::WholeDisk);
        whole.storage.home = home("/dev/sdb3", false);
        assert_eq!(
            validate_storage(&whole, Firmware::Uefi),
            Err(Invalid::HomeInvalid)
        );
        // Too small to be anybody's home.
        let mut tiny = config(windows(), windows_gap());
        let mut small = tiny.disk.partition("/dev/sdb3").unwrap().clone();
        small.path = "/dev/sdb5".into();
        small.number = 5;
        small.size = 512 * MIB;
        small.content = Content::Linux;
        tiny.disk.partitions.push(small);
        tiny.storage.home = home("/dev/sdb5", true);
        assert_eq!(
            validate_storage(&tiny, Firmware::Uefi),
            Err(Invalid::HomeInvalid)
        );
    }

    #[test]
    fn home_may_be_a_partition_of_another_disk_carried_with_it() {
        // Linexin erases the whole empty disk; the files are kept on the one
        // ext4 partition of a 16 GB disk, too small for Linexin itself.
        let small = mounted_json("")
            .replace("120034123776", "16008609792")
            .replace("120032591872", "16007561216");
        let other = only(&small, &facts());
        assert_eq!(other.blocked, Some(Blocked::TooSmall));
        let mut c = config(only(EMPTY, &facts()), Target::WholeDisk);
        let home = |path: &str, format: bool, disk: &Disk| {
            Some(Home {
                path: path.into(),
                format,
                disk: Some(disk.clone()),
            })
        };
        c.storage.home = home("/dev/sdf1", false, &other);
        assert_eq!(validate_storage(&c, Firmware::Uefi), Ok(()));
        assert_eq!(consent(&c), ["/dev/sda"]);
        let (disk, part) = home_partition(&c, c.storage.home.as_ref().unwrap()).unwrap();
        assert_eq!(
            (disk.path.as_str(), part.path.as_str()),
            ("/dev/sdf", "/dev/sdf1")
        );
        // Formatted, the consent names it after the disk erased.
        c.storage.home = home("/dev/sdf1", true, &other);
        assert_eq!(validate_storage(&c, Firmware::Uefi), Ok(()));
        assert_eq!(consent(&c), ["/dev/sda", "/dev/sdf1"]);
        // It travels with the request and comes back the same; a request
        // without it is a /home on the target disk.
        let text = serde_json::to_string(&c).unwrap();
        let back: InstallConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(back.storage, c.storage);
        let bare = r#"{"path":"/dev/sdb5","format":true}"#;
        assert_eq!(serde_json::from_str::<Home>(bare).unwrap().disk, None);

        let refused = |c: &InstallConfig, why: &str| {
            assert_eq!(
                validate_storage(c, Firmware::Uefi),
                Err(Invalid::HomeInvalid),
                "{why}"
            );
        };
        let mut bad = c.clone();
        bad.storage.home = home("/dev/sdf9", true, &other);
        refused(&bad, "a partition not on the disk named for it");
        bad.storage.home = home("/dev/sda1", true, &c.disk);
        refused(&bad, "the disk Linexin goes on, named as another");
        for blocked in [
            Blocked::InUse {
                mountpoints: vec!["/run/media/example".into()],
            },
            Blocked::Swap,
            Blocked::Holder,
            Blocked::InstallationMedia,
            Blocked::Unknown,
        ] {
            let mut busy = other.clone();
            busy.blocked = Some(blocked.clone());
            bad.storage.home = home("/dev/sdf1", true, &busy);
            refused(&bad, &format!("{blocked:?}"));
        }
        let mut read_only = other.clone();
        read_only.read_only = true;
        bad.storage.home = home("/dev/sdf1", true, &read_only);
        refused(&bad, "a read-only disk");
        // Kept, it must be a Linux file system there too.
        let mut ntfs = other.clone();
        ntfs.partitions[0].content = Content::Windows;
        bad.storage.home = home("/dev/sdf1", false, &ntfs);
        refused(&bad, "a kept Windows partition");
    }

    #[test]
    fn swap_is_at_most_a_quarter_of_the_system_and_128_gib() {
        let mut c = config(only(&nvme_json(), &facts()), Target::WholeDisk);
        c.storage.swap_mib = 0;
        assert_eq!(validate_storage(&c, Firmware::Uefi), Ok(()));
        c.storage.swap_mib = 128 * 1024;
        assert_eq!(validate_storage(&c, Firmware::Uefi), Ok(()));
        c.storage.swap_mib = 128 * 1024 + 1;
        assert_eq!(
            validate_storage(&c, Firmware::Uefi),
            Err(Invalid::SwapTooLarge)
        );
        // A 30 GB place takes at most 7.5 GB of swap.
        let mut small = config(only(&msdos_json(), &facts()), Target::WholeDisk);
        small.disk.size = 30_000_000_000;
        small.storage.swap_mib = 6 * 1024;
        assert_eq!(validate_storage(&small, Firmware::Bios), Ok(()));
        small.storage.swap_mib = 8 * 1024;
        assert_eq!(
            validate_storage(&small, Firmware::Bios),
            Err(Invalid::SwapTooLarge)
        );
    }

    #[test]
    fn boot_loaders_follow_the_firmware_and_the_table() {
        let mut c = config(windows(), windows_gap());
        for choice in [Bootloader::SystemdBoot, Bootloader::Refind] {
            c.bootloader = choice;
            assert_eq!(validate_storage(&c, Firmware::Uefi), Ok(()));
            assert_eq!(
                validate_storage(&c, Firmware::Bios),
                Err(Invalid::BootloaderUnavailable)
            );
        }
        c.bootloader = Bootloader::Grub;
        assert_eq!(validate_storage(&c, Firmware::Bios), Ok(()));
        // systemd-boot needs GPT; erasing a DOS disk under UEFI makes one.
        let mut dos = config(only(&msdos_json(), &facts()), partition("/dev/sdc1"));
        dos.bootloader = Bootloader::SystemdBoot;
        assert_eq!(
            validate_storage(&dos, Firmware::Uefi),
            Err(Invalid::BootloaderUnavailable)
        );
        dos.bootloader = Bootloader::Refind;
        assert_eq!(validate_storage(&dos, Firmware::Uefi), Ok(()));
        dos.target = Target::WholeDisk;
        dos.bootloader = Bootloader::SystemdBoot;
        assert_eq!(validate_storage(&dos, Firmware::Uefi), Ok(()));
    }

    #[test]
    fn the_source_must_be_a_plain_absolute_path() {
        let mut c = config(only(EMPTY, &facts()), Target::WholeDisk);
        for bad in [
            "run/archiso/airootfs",
            "/",
            "/run/../etc",
            "/run/archiso/\nx",
        ] {
            c.source = bad.into();
            assert_eq!(
                validate_storage(&c, Firmware::Uefi),
                Err(Invalid::SourceInvalid),
                "{bad:?}"
            );
        }
    }

    /// Discovery starts lsblk itself rather than through `run`, so it asks
    /// the test build's fence itself, first thing, and the fence refuses it.
    /// The thread finds no program meanwhile (`fence::FindNothing`): were the
    /// question ever lost, or moved after the lookup, discovery would find no
    /// lsblk and this test would fail, with nothing started.
    #[test]
    fn discovery_asks_the_fence_before_lsblk() {
        use crate::worker::run::fence;
        let _nothing = fence::FindNothing::here();
        let words = fence::refusal(discover).expect("discovery went on past the fence");
        assert!(
            words.contains("test build: lsblk is not a program"),
            "{words}"
        );
    }

    /// **Discovery's look at what holds a device refuses in a test build
    /// before anything is read** (Unit Q, Q6), as its lsblk does at the head
    /// of `discover`: `held`, the reader of
    /// `/sys/class/block/<name>/holders`, asks the fence first. (Checked
    /// against its fault by inspection only — the refusal is its first
    /// statement — and by the strace record of the test run: a fence taken
    /// out would have a test read the machine.)
    #[test]
    fn discovery_reads_no_holders_in_a_test_build() {
        let words =
            crate::worker::run::fence::refusal(|| held("/dev/vdz")).expect("the holders were read");
        assert!(
            words.contains("test build: no test reads this computer's /sys/class/block"),
            "{words}"
        );
    }
}
