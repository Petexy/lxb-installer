//! Hand-made layouts carried out on paper and on image files (§7.0, Unit
//! Q, step Q4).
//!
//! - **Each fixture's commands, word for word and in order**: the Windows
//!   disk with Windows shrunk and partitions made after it, the blank disk
//!   given a new table (and the same on 4096-byte sectors), the msdos
//!   Windows disk with Windows formatted as `/` and grown, the msdos disk
//!   whose extended partition goes with its logical one, the old Linux
//!   disk whose last logical partition goes while the one before it grows
//!   into its place and the others stay, the same disk with its logical
//!   partitions numbered out of their order on it (step Q7) and under a
//!   new msdos table, a FAT32 data partition kept on a BIOS machine's
//!   msdos disk with `/` and swap made after it and the boot flag on `/`,
//!   the blank disk a BIOS starts given a gpt whose `bios_grub` partition
//!   is only wiped, and three disks (step Q6) — `/` on one, the EFI system
//!   partition and the kernels on another, the person's home kept on a
//!   third, which is given nothing.
//! - **Logical partitions** deleted from the last by number alone, wherever
//!   it stands; every other found by where it starts, under the number it
//!   was reviewed with, which it keeps (step Q7).
//! - **The boot flag** only where nothing on the disk is marked, on a
//!   primary `/boot` or `/`, never a logical one (step Q7).
//! - **A kept filesystem made smaller before its entry, and inside it**
//!   (step Q8): `resize::Shrink`'s commands for each shrink name only its
//!   partition, come before the `sfdisk -N` that gives its entry the size
//!   agreed, and give their tool a size inside that entry.
//! - **Every phase in its order**, disk by disk in path order, whatever
//!   order the layout lists its disks in; and every sfdisk script, a `-N`
//!   one as much as one that makes partitions, saying first its unit and
//!   its disk's sector size.
//! - **What the commands destroy is the consent.** [`documented_consent`]
//!   is the consent as the handoff writes it, written here from those words
//!   alone; [`destroyed_by`] reads what the commands themselves destroy —
//!   which device each writes, which numbers, starts and sizes it gives,
//!   which filesystem it makes — knowing only the disks as reviewed. Both
//!   are held equal to each other and to the rule set's own consent
//!   (`layout::destroys`), on the fixtures and on layouts made up from a
//!   seed, at 512 and 4096 bytes a sector: only an oracle written apart
//!   catches a bug the rule set shares with the interface — a partition
//!   made, or an entry grown, over one that stays among them. The fence lets
//!   every one of those commands through, and nothing else.
//! - **On image files**, with Q0's [`Image`], the image the only path any
//!   command is given: a new table over a used one, a deletion and then
//!   partitions added, `--part-type` and `--part-label`, `-N` shrinking and
//!   growing an entry and keeping its type, name and UUID, and the last
//!   logical partition deleted with the others found by where they start —
//!   and the table read back is the layout; every disk a layout writes to,
//!   each on an image of its own.
//! - **The [`Mounted`](crate::worker::layout::Mounted)** the rest of the
//!   worker reads, assembled from the layout and what was read back — and
//!   on a BIOS, GRUB given the disk the kernel says holds `/` as read back,
//!   its `bios_grub` partition or its first MiB there (step Q7).
use super::fence::{tests::Kernel as Sys, Fence};
use super::plan::{
    activation, check_table, format_commands, read_back, table_commands, writes, writes_to, Act,
    Context, Paths, Planned,
};
use super::resize::{Shrink, Tool};
use super::{mounted, own_devices, used, Fact, Facts};
use crate::model::config::{Bootloader, Software};
use crate::model::disks::{
    Blocked, Content, Disk, Partition, Table, GIB, MIB, MIN_HOME, MIN_INSTALL,
};
use crate::model::layout::{
    self, Create, DiskLayout, Fate, Fs, MountPoint, Part, Which, KEPT_HOME_FILESYSTEMS,
};
use crate::worker::layout::{EspMount, Firmware};
use crate::worker::partition::fixtures::{self, build, part_path, Spec};
use crate::worker::partition::images::{Image, IMAGES, TABLE_TOOLS};
use crate::worker::partition::table::{Entry, Kernel, OnDisk};
use crate::worker::run::command_path;
use crate::worker::system::boot::{grub_bios_args, parent_disk};
use std::collections::BTreeMap;

// -------------------------------------------------------------------------
// The fixture layouts.

fn kept(disk: Disk) -> DiskLayout {
    DiskLayout {
        parts: disk
            .partitions
            .iter()
            .map(|was| Part {
                path: was.path.clone(),
                number: was.number,
                start: was.start,
                size: was.size,
                fate: Fate::Keep { mount: None },
            })
            .collect(),
        disk,
        table: None,
        create: Vec::new(),
    }
}

fn fresh(disk: Disk, table: Table) -> DiskLayout {
    let mut one = kept(disk);
    for part in &mut one.parts {
        part.fate = Fate::Delete;
    }
    one.table = Some(table);
    one
}

fn with(mut one: DiskLayout, number: u32, fate: Fate) -> DiskLayout {
    let part = one
        .parts
        .iter_mut()
        .find(|part| part.number == number)
        .expect("the fixture has the partition");
    part.fate = fate;
    one
}

fn at(path: &str) -> Option<MountPoint> {
    Some(MountPoint::from(path.to_string()))
}

fn made(start_mib: u64, size_mib: u64, fs: Fs, mount: Option<MountPoint>, label: &str) -> Create {
    Create {
        start: start_mib * MIB,
        size: size_mib * MIB,
        fs,
        label: label.to_string(),
        mount,
    }
}

fn format(fs: Fs, size_mib: u64, mount: Option<MountPoint>) -> Fate {
    Fate::Format {
        fs,
        label: String::new(),
        size: size_mib * MIB,
        mount,
    }
}

fn spec(
    number: u32,
    start_mib: u64,
    size_mib: u64,
    part_type: &'static str,
    content: Content,
    fs_type: &'static str,
) -> Spec {
    Spec {
        number,
        start: start_mib * MIB,
        size: size_mib * MIB,
        part_type,
        content,
        fs_type,
    }
}

/// An old Linux disk on msdos (Q7's `msdos-extended-base`): an ext4 "old
/// root" of 28 GiB, then an extended partition holding an ext4 "Data", a
/// swap partition and an ext4 "old home", each after its own table. The
/// extended partition is typed `0f` (W95 extended, LBA), as parted, GParted
/// and Windows make one — the msdos disk Where to install's tests use has
/// fdisk's `05`. The fence's tests number its deletions by it too.
pub(crate) fn disk_msdos_logical(sector: u32) -> Disk {
    let mut disk = build(
        "/dev/vdx",
        sector,
        Some(Table::Msdos),
        vec![
            spec(1, 1, 28672, "0x83", Content::Linux, "ext4"),
            spec(2, 28673, 1, "0x0f", Content::Other, ""),
            spec(5, 28674, 8192, "0x83", Content::Linux, "ext4"),
            spec(6, 36867, 2048, "0x82", Content::Swap, "swap"),
            spec(7, 38916, 10240, "0x83", Content::Linux, "ext4"),
        ],
        vec![(49156, 65536 - 49156)],
    );
    disk.partitions[1].size = u64::from(sector).max(1024);
    disk
}

/// [`disk_msdos_logical`] with its logical partitions numbered out of their
/// order on the disk, as a table whose logical partitions were made in
/// another order numbers them (the kernel numbers them as their tables are
/// chained; fdisk's "fix partitions order" is for this): the Data partition
/// (6) stands first inside the extended one, the swap partition (7) next
/// and the old home (5) last — listed by number, as discovery lists them.
fn disk_msdos_jumbled(sector: u32) -> Disk {
    let mut disk = build(
        "/dev/vdx",
        sector,
        Some(Table::Msdos),
        vec![
            spec(1, 1, 28672, "0x83", Content::Linux, "ext4"),
            spec(2, 28673, 1, "0x0f", Content::Other, ""),
            spec(5, 38916, 10240, "0x83", Content::Linux, "ext4"),
            spec(6, 28674, 8192, "0x83", Content::Linux, "ext4"),
            spec(7, 36867, 2048, "0x82", Content::Swap, "swap"),
        ],
        vec![(49156, 65536 - 49156)],
    );
    disk.partitions[1].size = u64::from(sector).max(1024);
    disk
}

/// `disk` under another name, as a second disk of a layout.
fn renamed(mut disk: Disk, path: &str, major_minor: &str) -> Disk {
    disk.path = path.to_string();
    disk.major_minor = major_minor.to_string();
    for part in &mut disk.partitions {
        part.path = part_path(path, part.number);
    }
    disk
}

/// Windows kept at `/windows` and shrunk to 20 GiB, its EFI partition kept
/// at `/efi`; a Linux boot partition, `/` and swap made in what Windows
/// gave up and the space after it.
fn gpt_windows(sector: u32) -> DiskLayout {
    let mut one = kept(fixtures::disk_gpt_windows(sector));
    one = with(one, 1, Fate::Keep { mount: at("/efi") });
    one = with(
        one,
        3,
        Fate::Shrink {
            to: 20 * GIB,
            mount: at("/windows"),
        },
    );
    one.create = vec![
        made(20597, 2048, Fs::Fat32, at("/boot"), "BOOT"),
        made(22645, 60689 - 22645, Fs::Ext4, at("/"), "Linexin"),
        made(60689, 4096, Fs::LinuxSwap, at("swap"), ""),
    ];
    one
}

/// Windows deleted, and `/boot`, `/` and swap made where it was and after.
fn gpt_windows_replace(sector: u32) -> DiskLayout {
    let mut one = kept(fixtures::disk_gpt_windows(sector));
    one = with(one, 1, Fate::Keep { mount: at("/efi") });
    one = with(one, 3, Fate::Delete);
    one.create = vec![
        made(117, 2048, Fs::Fat32, at("/boot"), ""),
        made(2165, 60689 - 2165, Fs::Ext4, at("/"), ""),
        made(60689, 4096, Fs::LinuxSwap, at("swap"), ""),
    ];
    one
}

/// A new gpt over the whole Windows disk.
fn gpt_windows_new_table(sector: u32) -> DiskLayout {
    let mut one = fresh(fixtures::disk_gpt_windows(sector), Table::Gpt);
    one.create = vec![
        made(1, 1024, Fs::Fat32, at("/boot"), ""),
        made(1025, 63487 - 1025, Fs::Btrfs, at("/"), ""),
        made(63487, 2048, Fs::LinuxSwap, None, ""),
    ];
    one
}

/// The blank disk: a new gpt, the EFI partition alone at `/boot`, Btrfs
/// `/`, swap and an xfs `/home` (Q5's `manual-uefi-empty`, smaller).
fn empty(sector: u32) -> DiskLayout {
    let mut one = fresh(fixtures::disk_empty(sector), Table::Gpt);
    one.create = vec![
        made(1, 1024, Fs::Fat32, at("/boot"), ""),
        made(1025, 28672, Fs::Btrfs, at("/"), "Linexin"),
        made(29697, 2048, Fs::LinuxSwap, at("swap"), ""),
        made(31745, 65535 - 31745, Fs::Xfs, at("/home"), "Home"),
    ];
    one
}

/// The old Linux partition formatted as a Btrfs `/` and grown, the EFI
/// partition and swap made after it.
fn gpt_plain(sector: u32) -> DiskLayout {
    let mut one = kept(fixtures::disk_gpt_plain(sector));
    one = with(one, 1, format(Fs::Btrfs, 40960, at("/")));
    one.create = vec![
        made(40961, 1024, Fs::Fat32, at("/boot"), ""),
        made(41985, 4096, Fs::LinuxSwap, at("swap"), ""),
    ];
    one
}

/// The msdos Windows disk, System Reserved marked active as Windows marks
/// it.
fn disk_marked_msdos(sector: u32) -> Disk {
    let mut disk = fixtures::disk_msdos(sector);
    disk.partitions[0].bootable = true;
    disk
}

/// The msdos Windows disk on BIOS ([`disk_marked_msdos`]): Windows
/// formatted as `/` and grown, swap made after it, System Reserved kept.
fn msdos(sector: u32) -> DiskLayout {
    let mut one = kept(disk_marked_msdos(sector));
    one = with(one, 2, format(Fs::Ext4, 40960, at("/")));
    one.create = vec![made(41461, 4096, Fs::LinuxSwap, at("swap"), "")];
    one
}

/// The msdos disk with an extended partition, on BIOS: the extended
/// partition deleted with its logical one, `/` and swap made in their
/// place and after.
fn msdos_extended(sector: u32) -> DiskLayout {
    let mut one = kept(fixtures::disk_msdos_extended(sector));
    one = with(one, 2, Fate::Delete);
    one = with(one, 5, Fate::Delete);
    one.create = vec![
        made(501, 61440 - 501, Fs::Ext4, at("/"), ""),
        made(61440, 4096, Fs::LinuxSwap, at("swap"), ""),
    ];
    one
}

/// The old Linux disk on BIOS (Q7's `manual-bios-msdos-logical`): the old
/// root formatted as `/`, the last logical partition deleted, the swap
/// partition — a logical one — formatted afresh and its entry grown over
/// the deleted one's place to the extended partition's end (`-N` on a
/// logical partition, which only the last one standing may be), Data kept
/// at `/data`.
fn msdos_logical(sector: u32) -> DiskLayout {
    let mut one = kept(disk_msdos_logical(sector));
    one = with(one, 1, format(Fs::Ext4, 28672, at("/")));
    one = with(one, 5, Fate::Keep { mount: at("/data") });
    one = with(one, 6, format(Fs::LinuxSwap, 49156 - 36867, at("swap")));
    one = with(one, 7, Fate::Delete);
    one
}

/// A new msdos table over the old Linux disk on BIOS: its primary
/// partition, its `0f` extended one and the three logical ones inside it
/// erased, and three primary partitions made — `/` (marked active in the
/// table's own script), swap and `/home`.
fn msdos_new_table(sector: u32) -> DiskLayout {
    let mut one = fresh(disk_msdos_logical(sector), Table::Msdos);
    one.create = vec![
        made(1, 40960, Fs::Ext4, at("/"), "Linexin"),
        made(40961, 4096, Fs::LinuxSwap, at("swap"), ""),
        made(45057, 65536 - 45057, Fs::Ext4, at("/home"), ""),
    ];
    one
}

/// The UUID of the FAT32 filesystem on [`disk_fat_data`]'s one partition.
const DATA_UUID: &str = "1ABB-105E";

/// A BIOS machine's msdos data disk (Q7's `bios-base`): 40 GiB, one FAT32
/// partition labelled DATA from 1 MiB, 4 GiB long, typed `0c`, marked by
/// nothing, and free space after it.
fn disk_fat_data(sector: u32) -> Disk {
    let mut disk = build(
        "/dev/vdx",
        sector,
        Some(Table::Msdos),
        vec![spec(1, 1, 4096, "0x0c", Content::Data, "vfat")],
        vec![(4097, 40960 - 4097)],
    );
    disk.partitions[0].uuid = DATA_UUID.into();
    disk.partitions[0].label = "DATA".into();
    sized(disk, 40960, 42)
}

/// DATA kept at `/mnt/data` (Q7's `manual-bios-msdos-keep-fat`), and two
/// primary partitions made after it on the table it keeps: `/` — the boot
/// flag's, nothing on the disk being marked — and swap.
fn msdos_keep_fat(sector: u32) -> DiskLayout {
    let mut one = with(
        kept(disk_fat_data(sector)),
        1,
        Fate::Keep {
            mount: at("/mnt/data"),
        },
    );
    one.create = vec![
        made(4097, 32768, Fs::Ext4, at("/"), ""),
        made(36865, 40960 - 36865, Fs::LinuxSwap, at("swap"), ""),
    ];
    one
}

/// The blank disk a BIOS starts (Q7's `manual-bios-gpt`, 32 GiB): a new
/// gpt, the 1 MiB `bios_grub` partition GRUB's core goes in, then ext4 `/`
/// and an ext4 `/home` on the rest.
fn bios_gpt(sector: u32) -> DiskLayout {
    let mut one = fresh(sized(fixtures::disk_empty(sector), 32768, 42), Table::Gpt);
    one.create = vec![
        made(1, 1, Fs::BiosGrub, None, ""),
        made(2, 24576, Fs::Ext4, at("/"), "Linexin"),
        made(24578, 32767 - 24578, Fs::Ext4, at("/home"), "Home"),
    ];
    one
}

/// Two disks, listed out of their order: `/dev/vdy`, a plain disk whose
/// Linux partition is formatted at `/data` and an xfs `/archive` made after
/// it, and the blank disk laid out as [`empty`].
fn two_disks(sector: u32) -> Vec<DiskLayout> {
    let mut data = kept(renamed(
        fixtures::disk_gpt_plain(sector),
        "/dev/vdy",
        "254:16",
    ));
    data = with(data, 1, format(Fs::Ext4, 30 * 1024, at("/data")));
    data.create = vec![made(30721, 65535 - 30721, Fs::Xfs, at("/archive"), "")];
    vec![data, empty(sector)]
}

/// The UUID of the filesystem on [`disk_home`]'s one partition, as it was
/// made before Linexin and as discovery read it.
pub(crate) const HOME_UUID: &str = "5c0e6b1a-2d4f-4e8a-9b3c-7d1e2f3a4b5c";

/// `disk` as discovery would list a disk of `size_mib` MiB: too small for
/// Linexin's `/` below `MIN_INSTALL`, which a disk that does not hold it
/// may be all the same (`TooSmall`).
fn sized(mut disk: Disk, size_mib: u64, diskseq: u64) -> Disk {
    disk.size = size_mib * MIB;
    disk.diskseq = Some(diskseq);
    disk.blocked = (disk.size < MIN_INSTALL).then_some(Blocked::TooSmall);
    disk
}

/// The person's disk of homes from before (Q6's `home-base`): 16 GiB at
/// `/dev/vdc`, a gpt and one ext4 partition filling it, labelled "Home",
/// whose filesystem has [`HOME_UUID`].
fn disk_home(sector: u32) -> Disk {
    let mut disk = build(
        "/dev/vdc",
        sector,
        Some(Table::Gpt),
        vec![spec(
            1,
            1,
            16382,
            fixtures::GPT_LINUX,
            Content::Linux,
            "ext4",
        )],
        Vec::new(),
    );
    disk.major_minor = "254:32".into();
    disk.partitions[0].uuid = HOME_UUID.into();
    disk.partitions[0].label = "Home".into();
    sized(disk, 16384, 3)
}

/// **Three disks** (Q6's `manual-uefi-three-disks`), listed against their
/// order: `/dev/vda`, blank, 32 GiB — a new gpt, ext4 `/` and swap;
/// `/dev/vdb`, blank, 8 GiB and so too small for `/` — a new gpt, the EFI
/// system partition at `/efi` and a FAT32 `/boot` beside it for the
/// kernels, so the firmware starts Linexin from another disk than `/`'s;
/// and [`disk_home`], whose ext4 is the person's home from before, kept at
/// `/home` — a disk nothing is written to, only mounted from.
fn three_disks(sector: u32) -> Vec<DiskLayout> {
    let mut root = fresh(
        sized(
            renamed(fixtures::disk_empty(sector), "/dev/vda", "254:0"),
            32768,
            1,
        ),
        Table::Gpt,
    );
    root.create = vec![
        made(1, 30718, Fs::Ext4, at("/"), "Linexin"),
        made(30719, 2048, Fs::LinuxSwap, at("swap"), ""),
    ];
    let mut starts = fresh(
        sized(
            renamed(fixtures::disk_empty(sector), "/dev/vdb", "254:16"),
            8192,
            2,
        ),
        Table::Gpt,
    );
    starts.create = vec![
        made(1, 1024, Fs::Fat32, at("/efi"), "EFI"),
        made(1025, 2048, Fs::Fat32, at("/boot"), "BOOT"),
    ];
    let home = with(
        kept(disk_home(sector)),
        1,
        Fate::Keep { mount: at("/home") },
    );
    vec![home, starts, root]
}

/// A fixture layout: its name, the firmware it is for, its disks.
pub(crate) struct Case {
    pub(crate) name: &'static str,
    pub(crate) firmware: Firmware,
    pub(crate) layout: Vec<DiskLayout>,
}

/// Every fixture layout at `sector` bytes a sector, each held to the rule
/// set first: a layout the interface could never send is no test of what
/// the worker does with one.
pub(crate) fn cases(sector: u32) -> Vec<Case> {
    let case = |name, firmware, layout| Case {
        name,
        firmware,
        layout,
    };
    let cases = vec![
        case("gpt-windows", Firmware::Uefi, vec![gpt_windows(sector)]),
        case(
            "gpt-windows-replace",
            Firmware::Uefi,
            vec![gpt_windows_replace(sector)],
        ),
        case(
            "gpt-windows-new-table",
            Firmware::Uefi,
            vec![gpt_windows_new_table(sector)],
        ),
        case("empty", Firmware::Uefi, vec![empty(sector)]),
        case("gpt-plain", Firmware::Uefi, vec![gpt_plain(sector)]),
        case("msdos", Firmware::Bios, vec![msdos(sector)]),
        case(
            "msdos-extended",
            Firmware::Bios,
            vec![msdos_extended(sector)],
        ),
        case("msdos-logical", Firmware::Bios, vec![msdos_logical(sector)]),
        case("msdos-jumbled", Firmware::Bios, vec![msdos_jumbled(sector)]),
        case(
            "msdos-new-table",
            Firmware::Bios,
            vec![msdos_new_table(sector)],
        ),
        case(
            "msdos-keep-fat",
            Firmware::Bios,
            vec![msdos_keep_fat(sector)],
        ),
        case("bios-gpt", Firmware::Bios, vec![bios_gpt(sector)]),
        case("two-disks", Firmware::Uefi, two_disks(sector)),
        case("three-disks", Firmware::Uefi, three_disks(sector)),
    ];
    for case in &cases {
        let judged = layout::shape(
            &case.layout,
            case.firmware,
            Bootloader::Automatic,
            &Software::default(),
        );
        assert_eq!(judged, Ok(()), "{} at {sector}", case.name);
    }
    cases
}

// -------------------------------------------------------------------------
// Carried out on paper.

/// The table on `one`'s disk once phase B's steps 1 to 7 are done, as
/// sfdisk would read it — what is kept at its new size, what is formatted
/// at its new type, each partition made under the lowest number free (on
/// msdos among the primary ones), as libfdisk numbers them — and the
/// kernel's partitions then.
pub(crate) fn after_writing(
    ctx: Context,
    one: &DiskLayout,
    before: Option<&OnDisk>,
) -> (OnDisk, Vec<Kernel>) {
    let disk = &one.disk;
    let table = layout::table_after(one).expect("a table after");
    let sector = u64::from(disk.logical_sector);
    let mut entries = Vec::new();
    let mut kernel = Vec::new();
    if one.table.is_none() {
        let before = before.expect("the table before");
        for entry in &before.partitions {
            let part = one
                .parts
                .iter()
                .find(|part| part.number == entry.number)
                .expect("reviewed");
            let mut entry = entry.clone();
            let size = match &part.fate {
                Fate::Delete => continue,
                Fate::Keep { .. } => part.size,
                Fate::Shrink { to, .. } => *to,
                Fate::Format {
                    fs, size, mount, ..
                } => {
                    entry.part_type = ctx.part_type(table, *fs, mount.as_ref()).to_string();
                    *size
                }
            };
            if !matches!(part.fate, Fate::Keep { .. }) {
                entry.size = size / sector;
            }
            kernel.push(Kernel {
                path: part.path.clone(),
                number: part.number,
                start: part.start,
                size,
            });
            entries.push(entry);
        }
    }
    let mut made: Vec<&Create> = one.create.iter().collect();
    made.sort_by_key(|made| made.start);
    for made in made {
        let number = (1..)
            .find(|number| {
                !entries.iter().any(|entry: &Entry| entry.number == *number)
                    && (table == Table::Gpt || *number <= 4)
            })
            .expect("a number free");
        entries.push(Entry {
            number,
            start: made.start / sector,
            size: made.size / sector,
            part_type: ctx
                .part_type(table, made.fs, made.mount.as_ref())
                .to_string(),
            bootable: false,
            name: String::new(),
            uuid: String::new(),
        });
        kernel.push(Kernel {
            path: part_path(&disk.path, number),
            number,
            start: made.start,
            size: made.size,
        });
    }
    let on_disk = OnDisk {
        table,
        sector: Some(sector),
        partitions: entries,
    };
    (on_disk, kernel)
}

/// Everything the worker would run for a layout, from the tables phase A
/// reads to phase C, with the disks read back as [`after_writing`] has
/// them.
pub(crate) struct OnPaper {
    /// Phase B, steps 1 to 8, disk by disk in path order.
    pub(crate) tables: Vec<Planned>,
    /// Phase C.
    pub(crate) formats: Vec<Planned>,
    /// The tables phase A read, by disk.
    before: BTreeMap<String, OnDisk>,
    /// Each written disk's table once steps 1 to 7 are done, which the
    /// boot flag is numbered by.
    after: BTreeMap<String, OnDisk>,
    /// The kernel's partitions once phase B is done, by disk written.
    kernel: BTreeMap<String, Vec<Kernel>>,
    paths: Paths,
}

impl OnPaper {
    pub(crate) fn all(&self) -> Vec<&Planned> {
        self.tables.iter().chain(&self.formats).collect()
    }
}

/// A partition's filesystem UUID as `blkid -p` reads it after phase B:
/// what it was reviewed with.
fn uuid_of(layout: &[DiskLayout]) -> impl Fn(&str) -> Option<String> + '_ {
    move |path: &str| {
        layout
            .iter()
            .flat_map(|one| &one.disk.partitions)
            .find(|part| part.path == path)
            .map(|part| part.uuid.clone())
    }
}

pub(crate) fn on_paper(firmware: Firmware, layout: &[DiskLayout]) -> OnPaper {
    let ctx = Context::new(layout, firmware);
    let before: BTreeMap<String, OnDisk> = layout
        .iter()
        .filter(|one| one.disk.table.is_some())
        .map(|one| (one.disk.path.clone(), fixtures::on_disk(&one.disk)))
        .collect();
    for one in layout {
        check_table(one, before.get(&one.disk.path)).expect("phase A");
    }
    let mut disks: Vec<&DiskLayout> = layout.iter().collect();
    disks.sort_by(|one, other| one.disk.path.cmp(&other.disk.path));
    let mut tables = Vec::new();
    let mut kernel = BTreeMap::new();
    let mut written = BTreeMap::new();
    let mut paths = Paths::default();
    for one in disks {
        let path = &one.disk.path;
        let read = before.get(path);
        tables.extend(table_commands(ctx, one, read, path).expect("phase B"));
        if !writes_to(one) {
            paths.extend(Paths::as_reviewed(one));
            continue;
        }
        let (after, found) = after_writing(ctx, one, read);
        let flag = activation(one, &after, path);
        // The boot flag is set exactly where the rule set judged, from the
        // disk as it was reviewed, that the plan sets it.
        assert_eq!(
            flag.is_some(),
            layout::activates(one),
            "the boot flag on {path}: {one:?}"
        );
        tables.extend(flag);
        paths.extend(read_back(one, &found, &uuid_of(layout)).expect("read back"));
        kernel.insert(path.clone(), found);
        written.insert(path.clone(), after);
    }
    // The worker's own walk over the disks is the same as this one's.
    let walked: Vec<Planned> = tables
        .iter()
        .filter(|planned| planned.act() != Act::Activate)
        .cloned()
        .collect();
    assert_eq!(writes(ctx, layout, &before).unwrap(), walked);
    let formats = format_commands(layout, &paths).expect("phase C");
    OnPaper {
        tables,
        formats,
        before,
        after: written,
        kernel,
        paths,
    }
}

// -------------------------------------------------------------------------
// The commands, word for word.

fn shown(planned: &[&Planned]) -> Vec<String> {
    planned.iter().map(|planned| planned.shown()).collect()
}

fn case(sector: u32, name: &str) -> Case {
    cases(sector)
        .into_iter()
        .find(|case| case.name == name)
        .expect("a fixture layout")
}

const ROOT_GPT: &str = "4f68bce3-e8cd-4db1-96e7-fbcaf984b709";
const ESP_GPT: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
const XBOOTLDR_GPT: &str = "bc13c2ff-59e6-4262-a352-b275fd6f7172";
const SWAP_GPT: &str = "0657fd6d-a4ab-43c4-84e5-0933c84b4f4f";
const HOME_GPT: &str = "933ac7e1-2eb4-4f13-b844-0e14e2aef915";
const BIOS_BOOT_GPT: &str = "21686148-6449-6e6f-744e-656564454649";

#[test]
fn each_fixtures_commands_are_these_in_this_order() {
    let expect = |sector: u32, name: &str, want: Vec<String>| {
        let case = case(sector, name);
        let paper = on_paper(case.firmware, &case.layout);
        assert_eq!(shown(&paper.all()), want, "{name} at {sector}");
    };
    let lines = |lines: &[&str]| {
        lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
    };

    expect(
        512,
        "gpt-windows",
        vec![
            "shrink ntfs on /dev/vdx3 from 32212254720 to 21474836480 bytes".into(),
            "sfdisk --lock -N 3 /dev/vdx <<\n  unit: sectors\n  sector-size: 512\n  \
             start=239616, size=41943040"
                .into(),
            format!(
                "sfdisk --lock --append --wipe-partitions always /dev/vdx <<\n  \
                 unit: sectors\n  sector-size: 512\n  \
                 start=42182656, size=4194304, type={XBOOTLDR_GPT}, name=\"Linux boot\"\n  \
                 start=46376960, size=77914112, type={ROOT_GPT}, name=\"Linexin\"\n  \
                 start=124291072, size=8388608, type={SWAP_GPT}, name=\"Linux swap\""
            ),
            "wipefs --all /dev/vdx5".into(),
            "mkfs.fat -F 32 -n BOOT /dev/vdx5".into(),
            "wipefs --all /dev/vdx6".into(),
            "mkfs.ext4 -F -L Linexin /dev/vdx6".into(),
            "wipefs --all /dev/vdx7".into(),
            "mkswap /dev/vdx7".into(),
        ],
    );

    let blank = |sector: u32, at: [(u64, u64); 4]| {
        let mut want = vec![
            "wipefs --all --lock /dev/vdx".to_string(),
            format!(
                "sfdisk --lock --wipe always --wipe-partitions always /dev/vdx <<\n  \
                 label: gpt\n  unit: sectors\n  sector-size: {sector}\n  \
                 start={}, size={}, type={ESP_GPT}, name=\"EFI system\"\n  \
                 start={}, size={}, type={ROOT_GPT}, name=\"Linexin\"\n  \
                 start={}, size={}, type={SWAP_GPT}, name=\"Linux swap\"\n  \
                 start={}, size={}, type={HOME_GPT}, name=\"Home\"",
                at[0].0, at[0].1, at[1].0, at[1].1, at[2].0, at[2].1, at[3].0, at[3].1
            ),
        ];
        want.extend(lines(&[
            "wipefs --all /dev/vdx1",
            "mkfs.fat -F 32 /dev/vdx1",
            "wipefs --all /dev/vdx2",
            "mkfs.btrfs -f -L Linexin /dev/vdx2",
            "wipefs --all /dev/vdx3",
            "mkswap /dev/vdx3",
            "wipefs --all /dev/vdx4",
            "mkfs.xfs -f -L Home /dev/vdx4",
        ]));
        want
    };
    expect(
        512,
        "empty",
        blank(
            512,
            [
                (2048, 2097152),
                (2099200, 58720256),
                (60819456, 4194304),
                (65013760, 69201920),
            ],
        ),
    );
    // The 4Kn disk: the same bytes in its own, eight times larger, sectors.
    expect(
        4096,
        "empty",
        blank(
            4096,
            [
                (256, 262144),
                (262400, 7340032),
                (7602432, 524288),
                (8126720, 8650240),
            ],
        ),
    );

    // System Reserved is marked active, as Windows marks it, and stays: no
    // boot flag is set (on paper as on the image, which lays the mark down).
    expect(
        512,
        "msdos",
        lines(&[
            "sfdisk --lock --append --wipe-partitions always /dev/vdx <<\n  \
             unit: sectors\n  sector-size: 512\n  start=84912128, size=8388608, type=82",
            "sfdisk --lock -N 2 /dev/vdx <<\n  unit: sectors\n  sector-size: 512\n  \
             start=1026048, size=83886080",
            "sfdisk --lock --part-type /dev/vdx 2 83",
            "wipefs --all /dev/vdx2",
            "mkfs.ext4 -F /dev/vdx2",
            "wipefs --all /dev/vdx3",
            "mkswap /dev/vdx3",
        ]),
    );

    expect(
        512,
        "msdos-extended",
        lines(&[
            "sfdisk --lock --delete /dev/vdx 5 2",
            "sfdisk --lock --append --wipe-partitions always /dev/vdx <<\n  \
             unit: sectors\n  sector-size: 512\n  \
             start=1026048, size=124803072, type=83\n  \
             start=125829120, size=8388608, type=82",
            "sfdisk --lock --activate /dev/vdx 2",
            "wipefs --all /dev/vdx2",
            "mkfs.ext4 -F /dev/vdx2",
            "wipefs --all /dev/vdx3",
            "mkswap /dev/vdx3",
        ]),
    );

    // The last logical partition deleted alone, by its number; then the
    // swap partition, a logical one, grown over where it stood, by the
    // number its start has; nothing numbered anew.
    expect(
        512,
        "msdos-logical",
        lines(&[
            "sfdisk --lock --delete /dev/vdx 7",
            "sfdisk --lock -N 6 /dev/vdx <<\n  unit: sectors\n  sector-size: 512\n  \
             start=75503616, size=25167872",
            "sfdisk --lock --part-type /dev/vdx 1 83",
            "sfdisk --lock --part-type /dev/vdx 6 82",
            "sfdisk --lock --activate /dev/vdx 1",
            "wipefs --all /dev/vdx1",
            "mkfs.ext4 -F /dev/vdx1",
            "wipefs --all /dev/vdx6",
            "mkswap /dev/vdx6",
        ]),
    );

    // Its logical partitions numbered out of their order on the disk: the
    // swap partition, numbered last though it stands between, deleted by
    // its number alone; the old home, numbered first but standing last,
    // retyped by the number its start has — 5, as it was reviewed.
    expect(
        512,
        "msdos-jumbled",
        lines(&[
            "sfdisk --lock --delete /dev/vdx 7",
            "sfdisk --lock --part-type /dev/vdx 1 83",
            "sfdisk --lock --part-type /dev/vdx 5 83",
            "sfdisk --lock --activate /dev/vdx 1",
            "wipefs --all /dev/vdx1",
            "mkfs.ext4 -F /dev/vdx1",
            "wipefs --all /dev/vdx5",
            "mkfs.ext4 -F /dev/vdx5",
        ]),
    );

    // A new msdos table over primaries and an extended frame: the disk
    // wiped whole, then the table written with its three primary partitions
    // in one script, / marked active in it — no boot flag after.
    expect(
        512,
        "msdos-new-table",
        lines(&[
            "wipefs --all --lock /dev/vdx",
            "sfdisk --lock --wipe always --wipe-partitions always /dev/vdx <<\n  \
             label: dos\n  unit: sectors\n  sector-size: 512\n  \
             start=2048, size=83886080, type=83, bootable\n  \
             start=83888128, size=8388608, type=82\n  \
             start=92276736, size=41940992, type=83",
            "wipefs --all /dev/vdx1",
            "mkfs.ext4 -F -L Linexin /dev/vdx1",
            "wipefs --all /dev/vdx2",
            "mkswap /dev/vdx2",
            "wipefs --all /dev/vdx3",
            "mkfs.ext4 -F /dev/vdx3",
        ]),
    );

    // DATA kept on its table, / and swap appended after it as primary
    // partitions 2 and 3, and the boot flag on / — nothing being marked —
    // by the number the table read once they are made gives it. DATA is
    // given nothing.
    expect(
        512,
        "msdos-keep-fat",
        lines(&[
            "sfdisk --lock --append --wipe-partitions always /dev/vdx <<\n  \
             unit: sectors\n  sector-size: 512\n  \
             start=8390656, size=67108864, type=83\n  \
             start=75499520, size=8386560, type=82",
            "sfdisk --lock --activate /dev/vdx 2",
            "wipefs --all /dev/vdx2",
            "mkfs.ext4 -F /dev/vdx2",
            "wipefs --all /dev/vdx3",
            "mkswap /dev/vdx3",
        ]),
    );

    // The blank disk a BIOS starts: a new gpt whose first partition is the
    // bios_grub one, typed and named for GRUB's core — only wiped, never
    // given a filesystem — then / and /home; no boot flag on a gpt.
    expect(
        512,
        "bios-gpt",
        vec![
            "wipefs --all --lock /dev/vdx".into(),
            format!(
                "sfdisk --lock --wipe always --wipe-partitions always /dev/vdx <<\n  \
                 label: gpt\n  unit: sectors\n  sector-size: 512\n  \
                 start=2048, size=2048, type={BIOS_BOOT_GPT}, name=\"BIOS boot\"\n  \
                 start=4096, size=50331648, type={ROOT_GPT}, name=\"Linexin\"\n  \
                 start=50335744, size=16771072, type={HOME_GPT}, name=\"Home\""
            ),
            "wipefs --all /dev/vdx1".into(),
            "wipefs --all /dev/vdx2".into(),
            "mkfs.ext4 -F -L Linexin /dev/vdx2".into(),
            "wipefs --all /dev/vdx3".into(),
            "mkfs.ext4 -F -L Home /dev/vdx3".into(),
        ],
    );

    // Three disks, listed /dev/vdc, /dev/vdb, /dev/vda: the two blank ones
    // written in path order, each its wipe and its new table, then their
    // filesystems in the same order — and nothing at all given the disk
    // that keeps the person's home.
    expect(
        512,
        "three-disks",
        vec![
            "wipefs --all --lock /dev/vda".into(),
            format!(
                "sfdisk --lock --wipe always --wipe-partitions always /dev/vda <<\n  \
                 label: gpt\n  unit: sectors\n  sector-size: 512\n  \
                 start=2048, size=62910464, type={ROOT_GPT}, name=\"Linexin\"\n  \
                 start=62912512, size=4194304, type={SWAP_GPT}, name=\"Linux swap\""
            ),
            "wipefs --all --lock /dev/vdb".into(),
            format!(
                "sfdisk --lock --wipe always --wipe-partitions always /dev/vdb <<\n  \
                 label: gpt\n  unit: sectors\n  sector-size: 512\n  \
                 start=2048, size=2097152, type={ESP_GPT}, name=\"EFI system\"\n  \
                 start=2099200, size=4194304, type={XBOOTLDR_GPT}, name=\"Linux boot\""
            ),
            "wipefs --all /dev/vda1".into(),
            "mkfs.ext4 -F -L Linexin /dev/vda1".into(),
            "wipefs --all /dev/vda2".into(),
            "mkswap /dev/vda2".into(),
            "wipefs --all /dev/vdb1".into(),
            "mkfs.fat -F 32 -n EFI /dev/vdb1".into(),
            "wipefs --all /dev/vdb2".into(),
            "mkfs.fat -F 32 -n BOOT /dev/vdb2".into(),
        ],
    );
}

/// `line` with every `start=` and `size=` divided by eight and its
/// `sector-size: 512` made 4096: the same command on 4096-byte sectors.
fn in_4096_sectors(line: &str) -> String {
    let line = line.replace("sector-size: 512", "sector-size: 4096");
    let mut out = String::new();
    let mut rest = line.as_str();
    while let Some((index, key)) = ["start=", "size="]
        .iter()
        .filter_map(|key| rest.find(key).map(|index| (index, key.len())))
        .min()
    {
        out.push_str(&rest[..index + key]);
        rest = &rest[index + key..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let number: u64 = rest[..digits].parse().unwrap();
        assert_eq!(number % 8, 0, "{line}");
        out.push_str(&(number / 8).to_string());
        rest = &rest[digits..];
    }
    out.push_str(rest);
    out
}

#[test]
fn on_4096_byte_sectors_every_command_counts_in_its_own_sectors() {
    for (small, large) in cases(512).iter().zip(cases(4096)) {
        let in_small = on_paper(small.firmware, &small.layout);
        let in_large = on_paper(large.firmware, &large.layout);
        let small_lines: Vec<String> = shown(&in_small.all())
            .iter()
            .map(|line| in_4096_sectors(line))
            .collect();
        assert_eq!(shown(&in_large.all()), small_lines, "{}", large.name);
    }
}

// -------------------------------------------------------------------------
// The order.

/// The disk `device` is or is on, by the layout's disks.
fn disk_of<'a>(layout: &'a [DiskLayout], device: &str) -> &'a str {
    layout
        .iter()
        .find(|one| {
            one.disk.path == device || one.disk.partitions.iter().any(|part| part.path == device)
        })
        .map(|one| one.disk.path.as_str())
        .unwrap_or_else(|| panic!("{device} is on none of the layout's disks"))
}

/// Holds `paper` to the plan's order: phase B disk by disk in path order,
/// each disk's steps from 1 to 8 and never back, and phase C only after
/// every disk's phase B, disk by disk too.
fn holds_the_order(name: &str, layout: &[DiskLayout], paper: &OnPaper) {
    let mut last: Option<(&str, u8)> = None;
    for planned in &paper.tables {
        let step = planned.act().step();
        assert!((1..=8).contains(&step), "{name}: {planned:?} in phase B");
        let disk = disk_of(layout, planned.device());
        if let Some((was, was_step)) = last {
            assert!(
                (was, was_step) <= (disk, step),
                "{name}: {} after step {was_step} on {was}",
                planned.shown()
            );
        }
        last = Some((disk, step));
    }
    let mut last_disk = "";
    for planned in &paper.formats {
        assert_eq!(planned.act().step(), 10, "{name}: {planned:?} in phase C");
        let which = paper
            .paths
            .which(planned.device())
            .unwrap_or_else(|| panic!("{name}: {} was not read back", planned.device()));
        let disk = match which {
            Which::Part(path) => disk_of(layout, path),
            Which::Made { disk, .. } => disk.as_str(),
        };
        assert!(
            last_disk <= disk,
            "{name}: {} out of order",
            planned.shown()
        );
        last_disk = disk;
    }
}

/// The shrink `planned` asks for, of a filesystem that fills its partition
/// as the fixtures' do (phase A measures it on a machine).
pub(crate) fn filling(planned: &Planned) -> Result<Shrink, String> {
    let Planned::ShrinkFilesystem { from, .. } = planned else {
        return Err(format!("{} shrinks no filesystem", planned.shown()));
    };
    Shrink::of(planned, *from)
}

/// Holds every filesystem `paper` shrinks to its place and its size (step
/// Q8): its commands, as [`Shrink`] builds them for its
/// `Planned::ShrinkFilesystem`, name its partition and nothing else, and
/// come before the `sfdisk -N` that makes its entry smaller — the one whose
/// script gives the partition's own start the size agreed — and the size
/// each tool is given lies inside that entry. Gives how many it held.
fn shrinks_fit_their_entries(name: &str, layout: &[DiskLayout], paper: &OnPaper) -> usize {
    let mut held = 0;
    for (at, planned) in paper.tables.iter().enumerate() {
        let Planned::ShrinkFilesystem { device, to, .. } = planned else {
            continue;
        };
        let shrink = filling(planned).unwrap_or_else(|why| panic!("{name}: {why}"));
        let one = layout
            .iter()
            .find(|one| one.parts.iter().any(|part| part.path == *device))
            .unwrap_or_else(|| panic!("{name}: {device} is on no disk of the layout"));
        let part = one.parts.iter().find(|part| part.path == *device).unwrap();
        assert!(
            matches!(part.fate, Fate::Shrink { to: agreed, .. } if agreed == *to),
            "{name}: {device} is shrunk to {to}, not as agreed: {:?}",
            part.fate
        );
        let sector = u64::from(one.disk.logical_sector);
        let script = format!("start={}, size={}\n", part.start / sector, to / sector);
        let entry = paper
            .tables
            .iter()
            .position(|other| {
                other.act() == Act::ShrinkEntry
                    && other.device() == one.disk.path
                    && other
                        .command()
                        .and_then(|command| command.input.as_deref())
                        .is_some_and(|input| input.ends_with(&script))
            })
            .unwrap_or_else(|| panic!("{name}: no entry is made {to} bytes for {device}"));
        assert!(
            at < entry,
            "{name}: {device}'s entry is made smaller before its filesystem"
        );
        assert!(
            shrink.target_bytes() <= *to,
            "{name}: {device}: {} for an entry of {to} bytes",
            shrink.target()
        );
        for step in shrink.steps().iter().chain([&shrink.dry_run()]) {
            let paths: Vec<&String> = step
                .command
                .args
                .iter()
                .filter(|arg| arg.contains('/'))
                .collect();
            assert_eq!(paths, [device], "{name}: {:?}", step.command);
        }
        held += 1;
    }
    held
}

/// **Each filesystem is made smaller inside its new entry, before its
/// entry** (step Q8): on the Windows disk, Windows' NTFS shrunk to 20 GiB
/// by ntfsresize — its dry run and its shrink word for word, never
/// `--force` — before `sfdisk -N 3` gives its entry 20 GiB; and every
/// fixture's shrinks so (`holds_everything` holds the seeded layouts' too).
#[test]
fn every_filesystem_is_shrunk_inside_its_new_entry_before_the_entry() {
    for sector in [512, 4096] {
        let windows = case(sector, "gpt-windows");
        let paper = on_paper(windows.firmware, &windows.layout);
        assert_eq!(
            shrinks_fit_their_entries(windows.name, &windows.layout, &paper),
            1
        );
        let shrunk = &paper.tables[0];
        let shrink = filling(shrunk).unwrap();
        assert_eq!(
            (shrink.device.as_str(), shrink.tool, shrink.to),
            ("/dev/vdx3", Tool::Ntfs, 20 * GIB)
        );
        let words = |command: &crate::worker::partition::script::Command| {
            let mut line = vec![command.program.to_string()];
            line.extend(command.args.iter().cloned());
            line.join(" ")
        };
        assert_eq!(
            words(&shrink.dry_run().command),
            "ntfsresize --no-action --no-progress-bar --size 21474836480 /dev/vdx3"
        );
        let steps = shrink.steps();
        assert_eq!(
            steps
                .iter()
                .map(|step| words(&step.command))
                .collect::<Vec<_>>(),
            ["ntfsresize --no-progress-bar --size 21474836480 /dev/vdx3"]
        );
        assert_eq!(steps[0].command.input.as_deref(), Some("y\n"));
        let entry = paper.tables[1].command().unwrap();
        assert_eq!(words(entry), "sfdisk --lock -N 3 /dev/vdx");
        let per_sector = 20 * GIB / u64::from(sector);
        assert!(entry
            .input
            .as_deref()
            .unwrap()
            .ends_with(&format!("size={per_sector}\n")));

        let held: usize = cases(sector)
            .iter()
            .map(|case| {
                let paper = on_paper(case.firmware, &case.layout);
                shrinks_fit_their_entries(case.name, &case.layout, &paper)
            })
            .sum();
        assert_eq!(held, 1, "only the Windows disk shrinks a filesystem");
    }
}

#[test]
fn every_phase_comes_in_its_order_disk_by_disk() {
    for sector in [512, 4096] {
        for case in cases(sector) {
            let paper = on_paper(case.firmware, &case.layout);
            holds_the_order(case.name, &case.layout, &paper);
        }
    }
    // Listed out of order, the disks are still written in path order.
    let two = case(512, "two-disks");
    assert_eq!(two.layout[0].disk.path, "/dev/vdy");
    let paper = on_paper(two.firmware, &two.layout);
    let tables: Vec<&str> = paper
        .tables
        .iter()
        .map(|planned| disk_of(&two.layout, planned.device()))
        .collect();
    let first_vdy = tables.iter().position(|disk| *disk == "/dev/vdy").unwrap();
    assert!(first_vdy > 0 && tables[..first_vdy].iter().all(|disk| *disk == "/dev/vdx"));
    assert!(tables[first_vdy..].iter().all(|disk| *disk == "/dev/vdy"));
    assert_eq!(paper.tables[0].act(), Act::Wipe);
    // Three disks listed against their order: every command on the two
    // written in path order, phase B and phase C alike, and none on the
    // disk only mounted from — which is read back as it was reviewed.
    for sector in [512, 4096] {
        let three = case(sector, "three-disks");
        let listed: Vec<&str> = three
            .layout
            .iter()
            .map(|one| one.disk.path.as_str())
            .collect();
        assert_eq!(listed, ["/dev/vdc", "/dev/vdb", "/dev/vda"]);
        let paper = on_paper(three.firmware, &three.layout);
        let disks: Vec<&str> = paper
            .all()
            .iter()
            .map(|planned| match paper.paths.which(planned.device()) {
                Some(Which::Made { disk, .. }) => disk.as_str(),
                Some(Which::Part(path)) => disk_of(&three.layout, path),
                None => disk_of(&three.layout, planned.device()),
            })
            .collect();
        let tables = paper.tables.len();
        let sorted = |disks: &[&str]| disks.windows(2).all(|pair| pair[0] <= pair[1]);
        assert!(
            sorted(&disks[..tables]) && sorted(&disks[tables..]),
            "{disks:?}"
        );
        assert!(disks.contains(&"/dev/vda") && disks.contains(&"/dev/vdb"));
        assert!(!disks.contains(&"/dev/vdc"), "{disks:?}");
        assert_eq!(
            paper.paths.of(&Which::Part("/dev/vdc1".into())),
            Some("/dev/vdc1")
        );
        assert!(!writes_to(&three.layout[0]));
    }
}

// -------------------------------------------------------------------------
// Logical partitions.

/// The jumbled disk on BIOS: the old root formatted as `/`, the swap
/// partition — numbered last though it stands between the two others —
/// deleted, the old home — numbered first, standing last — formatted at
/// `/home`, Data kept at `/data`.
fn msdos_jumbled(sector: u32) -> DiskLayout {
    let mut one = kept(disk_msdos_jumbled(sector));
    one = with(one, 1, format(Fs::Ext4, 28672, at("/")));
    one = with(one, 5, format(Fs::Ext4, 10240, at("/home")));
    one = with(one, 6, Fate::Keep { mount: at("/data") });
    one = with(one, 7, Fate::Delete);
    one
}

/// Every partition number `planned` gives sfdisk, with the start (in
/// bytes) of the partition it names — by the table phase A read
/// (`before`), or for the boot flag the table read once the partitions
/// are made (`after`): `--delete`'s, `--part-type`'s, `--part-label`'s,
/// `-N`'s (its script's start) and `--activate`'s.
fn numbers_named(
    planned: &Planned,
    sector: u64,
    before: &OnDisk,
    after: &OnDisk,
) -> Vec<(u32, Option<u64>)> {
    let Some(command) = planned.command() else {
        return Vec::new();
    };
    let start_in = |table: &OnDisk, number: u32| {
        table
            .partitions
            .iter()
            .find(|entry| entry.number == number)
            .map(|entry| entry.start * sector)
    };
    let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
    let number = |text: &str| text.parse::<u32>().expect("a number");
    match args.as_slice() {
        ["--lock", "--delete", _, numbers @ ..] => numbers
            .iter()
            .map(|n| (number(n), start_in(before, number(n))))
            .collect(),
        ["--lock", "--part-type" | "--part-label", _, n, _] => {
            vec![(number(n), start_in(before, number(n)))]
        }
        ["--lock", "-N", n, _] => vec![(number(n), start_in(before, number(n)))],
        ["--lock", "--activate", _, n] => vec![(number(n), start_in(after, number(n)))],
        _ => Vec::new(),
    }
}

/// **Logical partitions go from the last by number; every other is found
/// by where it starts, and keeps its number** (Unit Q, Q7; the rule set's
/// `LogicalOrder`, Q1). On the old Linux disk, its logical partitions in
/// their order on the disk, and on the same disk numbered out of it (Data,
/// numbered 6, first inside the extended partition; the swap partition,
/// numbered 7, next; the old home, numbered 5, last): the one `--delete`
/// names the logical partition numbered last, 7, alone, wherever it
/// stands; every other number a command gives — a new type, `-N`, the
/// boot flag — is the number the table gives the reviewed partition at
/// the start it acts on, its own reviewed number; and once the tables are
/// written every partition that stays has the number it was reviewed
/// with. Deleting one numbered before another that stays is refused; were
/// it carried out all the same, sfdisk would number the one after it
/// anew between the deletion and its new type, which the consent written
/// apart sees — and which the fence, judging that deletion, now stops
/// before a disk is touched.
#[test]
fn logical_partitions_go_from_the_last_by_number_and_the_rest_are_found_where_they_start() {
    for sector in [512, 4096] {
        let bytes = u64::from(sector);
        for name in ["msdos-logical", "msdos-jumbled"] {
            let case = case(sector, name);
            let one = &case.layout[0];
            let paper = on_paper(case.firmware, &case.layout);
            let before = &paper.before[&one.disk.path];
            let after = &paper.after[&one.disk.path];
            let at = format!("{name} at {sector}");
            let deletes: Vec<&Planned> = paper
                .tables
                .iter()
                .filter(|planned| planned.act() == Act::Delete)
                .collect();
            assert_eq!(deletes.len(), 1, "{at}");
            assert_eq!(
                numbers_named(deletes[0], bytes, before, after),
                [(7, Some(one.parts[4].start))],
                "{at}: the last logical partition by number, alone"
            );
            for planned in &paper.tables {
                for (number, start) in numbers_named(planned, bytes, before, after) {
                    let reviewed = one
                        .parts
                        .iter()
                        .find(|part| Some(part.start) == start)
                        .unwrap_or_else(|| panic!("{at}: {} names no place", planned.shown()));
                    assert_eq!(number, reviewed.number, "{at}: {}", planned.shown());
                }
            }
            for part in one.parts.iter().filter(|part| part.fate != Fate::Delete) {
                let entry = after
                    .partitions
                    .iter()
                    .find(|entry| entry.start * bytes == part.start)
                    .unwrap_or_else(|| panic!("{at}: {} gone", part.path));
                assert_eq!(entry.number, part.number, "{at}: {} renumbered", part.path);
            }
            // A kernel that numbered the two logical partitions that stay
            // anew — each at its own place under the other's number and
            // name — is refused by the read back, before anything is
            // formatted.
            let found = &paper.kernel[&one.disk.path];
            let uuids = uuid_of(&case.layout);
            assert!(read_back(one, found, &uuids).is_ok(), "{at}");
            let mut anew = found.clone();
            let five = anew.iter().position(|k| k.number == 5).unwrap();
            let six = anew.iter().position(|k| k.number == 6).unwrap();
            anew[five].number = 6;
            anew[six].number = 5;
            let (path_five, path_six) = (anew[five].path.clone(), anew[six].path.clone());
            anew[five].path = path_six;
            anew[six].path = path_five;
            assert!(read_back(one, &anew, &uuids).is_err(), "{at}: renumbered");
        }
        // A logical partition numbered before one that stays may not go —
        // on the disk in its order (5, with 6 formatted), and numbered out
        // of it, the swap partition numbered 7 formatted rather than
        // deleted (5, standing last; 6, standing first) — whatever stands
        // where.
        let jumbled = || {
            let one = case(sector, "msdos-jumbled").layout.remove(0);
            with(one, 7, format(Fs::LinuxSwap, 2048, at("swap")))
        };
        for (name, number, one) in [
            (
                "msdos-logical",
                5,
                case(sector, "msdos-logical").layout.remove(0),
            ),
            ("msdos-jumbled", 5, jumbled()),
            ("msdos-jumbled", 6, jumbled()),
        ] {
            let layout = vec![with(one, number, Fate::Delete)];
            let path = format!("/dev/vdx{number}");
            assert_eq!(
                layout::shape(
                    &layout,
                    Firmware::Bios,
                    Bootloader::Automatic,
                    &Software::default()
                ),
                Err(layout::Refusal::LogicalOrder {
                    part: Which::Part(path)
                }),
                "{name}: {number} deleted at {sector}"
            );
            // Carried out regardless, a command after the deletion names a
            // number sfdisk has given another partition, or none: the
            // consent written apart is not what the commands destroy.
            let paper = on_paper(Firmware::Bios, &layout);
            assert_ne!(
                destroyed_by(&layout, &paper.before, &paper.all(), &paper.paths),
                documented_consent(&layout),
                "{name}: {number} deleted at {sector}"
            );
            // And the fence stops it before a disk is touched (step Q7): the
            // first command it refuses is the deletion, which names that
            // logical partition without the one numbered after it, which
            // stays and which its going would number anew.
            let mut fence = Fence::new(&layout, &paper.before);
            for (disk, after) in &paper.after {
                fence.written_table(disk, after);
            }
            let before = kernel_of(&layout, None);
            let (refused, stop) = paper
                .tables
                .iter()
                .find_map(|planned| {
                    let stop = fence.check_planned(planned, &before).err()?;
                    Some((planned, stop))
                })
                .unwrap_or_else(|| panic!("{name}: {number} deleted at {sector}: let through"));
            assert_eq!(refused.act(), Act::Delete, "{name}: {}", refused.shown());
            let why = format!("names logical partition {number} but not {}", number + 1);
            assert!(
                stop.detail.contains(&why),
                "{name}: {number} deleted at {sector}: {}",
                stop.detail
            );
        }
    }
}

// -------------------------------------------------------------------------
// The boot flag.

/// `one` read as phase B leaves it ([`after_writing`]), with partition
/// `marked` — `None` for none — the one marked active.
fn after_marked(firmware: Firmware, one: &DiskLayout, marked: Option<u32>) -> OnDisk {
    let ctx = Context::new(std::slice::from_ref(one), firmware);
    let before = one.disk.table.map(|_| fixtures::on_disk(&one.disk));
    let (mut after, _) = after_writing(ctx, one, before.as_ref());
    for entry in &mut after.partitions {
        entry.bootable = Some(entry.number) == marked;
    }
    after
}

/// **The boot flag goes on only where nothing is marked** (Unit Q, Q7; the
/// easy path's rule, `plan::activation`): on a kept msdos table on the
/// disk that holds `/`, read once its partitions are made, it goes on a
/// primary `/boot` there, else on `/` — and only while no partition at all
/// is marked: a mark on Windows' System Reserved, on `/` itself, on a
/// partition the run made, on a logical one or on the extended frame each
/// leaves the table as it is. Never on a logical `/`, which the flag cannot
/// start; never on a gpt; never after a new msdos table, whose own script
/// marks `/` and nothing else; never on a disk `/` is not on.
#[test]
fn the_boot_flag_goes_on_only_where_nothing_is_marked() {
    let flag = |one: &DiskLayout, after: &OnDisk| {
        activation(one, after, &one.disk.path).map(|planned| planned.shown())
    };
    let on = |number: u32| Some(format!("sfdisk --lock --activate /dev/vdx {number}"));
    for sector in [512, 4096] {
        let fixture = |name: &str| case(sector, name).layout.remove(0);
        // Windows formatted as / beside its System Reserved, swap made.
        let msdos = fixture("msdos");
        let bios = Firmware::Bios;
        assert_eq!(flag(&msdos, &after_marked(bios, &msdos, None)), on(2));
        for marked in [1, 2, 3] {
            let after = after_marked(bios, &msdos, Some(marked));
            assert_eq!(flag(&msdos, &after), None, "{marked} marked at {sector}");
        }
        // The old Linux disk: / on its primary partition; a mark on a
        // logical partition, or on the extended frame, is a mark all the
        // same.
        let logical = fixture("msdos-logical");
        assert_eq!(flag(&logical, &after_marked(bios, &logical, None)), on(1));
        for marked in [2, 5, 6] {
            let after = after_marked(bios, &logical, Some(marked));
            assert_eq!(flag(&logical, &after), None, "{marked} marked at {sector}");
        }
        // DATA kept, / made after it: the flag on /, by the number the table
        // read after gives what was made.
        let fat = fixture("msdos-keep-fat");
        assert_eq!(flag(&fat, &after_marked(bios, &fat, None)), on(2));
        assert_eq!(flag(&fat, &after_marked(bios, &fat, Some(1))), None);
        // A primary /boot made before / on the same disk takes it first.
        let boot = boot_beside_root(sector);
        let judged = layout::shape(
            std::slice::from_ref(&boot),
            bios,
            Bootloader::Automatic,
            &Software::default(),
        );
        assert_eq!(judged, Ok(()), "a /boot beside / at {sector}");
        assert_eq!(flag(&boot, &after_marked(bios, &boot, None)), on(2));
        // / on a logical partition, nothing marked: never (the flag's rule
        // alone, whatever the rule set says of the rest).
        let mut logical_root = fixture("msdos-logical");
        logical_root.parts[0].fate = Fate::Keep { mount: None };
        logical_root.parts[2].fate = format(Fs::Ext4, 8192, at("/"));
        logical_root.parts[3].fate = format(Fs::LinuxSwap, 2048, at("swap"));
        let after = after_marked(bios, &logical_root, None);
        assert_eq!(flag(&logical_root, &after), None, "a logical / at {sector}");
        // Never on a gpt, nor after a new msdos table, whose script marks
        // / alone.
        let gpt = fixture("bios-gpt");
        assert_eq!(flag(&gpt, &after_marked(bios, &gpt, None)), None);
        let fresh = fixture("msdos-new-table");
        assert_eq!(flag(&fresh, &after_marked(bios, &fresh, None)), None);
        let paper = on_paper(bios, std::slice::from_ref(&fresh));
        let script = paper.tables[1].command().unwrap().input.clone().unwrap();
        let marked: Vec<&str> = script
            .lines()
            .filter(|line| line.ends_with(", bootable"))
            .collect();
        assert_eq!(marked.len(), 1, "{script}");
        assert!(marked[0].starts_with(&format!("start={},", MIB / u64::from(sector))));
        // Never on a disk / is not on: DATA's disk keeping DATA and given an
        // archive, / on a blank gpt disk beside it.
        let two = root_beside_data(sector);
        let judged = layout::shape(&two, bios, Bootloader::Automatic, &Software::default());
        assert_eq!(judged, Ok(()), "/ on another disk at {sector}");
        let data = &two[0];
        assert_eq!(flag(data, &after_marked(bios, data, None)), None);
    }
}

/// [`msdos_keep_fat`] with a FAT32 `/boot` made before `/` on its disk.
fn boot_beside_root(sector: u32) -> DiskLayout {
    let mut boot = msdos_keep_fat(sector);
    boot.create = vec![
        made(4097, 1024, Fs::Fat32, at("/boot"), ""),
        made(5121, 32768, Fs::Ext4, at("/"), ""),
        made(37889, 40960 - 37889, Fs::LinuxSwap, at("swap"), ""),
    ];
    boot
}

/// Two disks a BIOS starts: DATA's msdos disk (`/dev/vdx`), DATA kept at
/// `/mnt/data` and an xfs `/archive` made after it, and [`bios_gpt`]'s
/// layout on `/dev/vdy` — `/` on the second disk by path.
fn root_beside_data(sector: u32) -> Vec<DiskLayout> {
    let mut data = msdos_keep_fat(sector);
    data.create = vec![made(4097, 40960 - 4097, Fs::Xfs, at("/archive"), "")];
    let mut root = bios_gpt(sector);
    root.disk = renamed(root.disk, "/dev/vdy", "254:16");
    root.disk.diskseq = Some(43);
    vec![data, root]
}

// -------------------------------------------------------------------------
// GRUB on a machine a BIOS starts.

/// **GRUB's BIOS target is the disk that holds `/`, as read back** (Unit
/// Q, Q7): for every fixture layout a BIOS starts, a `/boot` made beside
/// `/`, and `/` on a blank gpt disk beside DATA's msdos disk (listed after
/// it and before it), the `Mounted` assembled from what phase B read back
/// gives grub-install — `boot::grub_bios_args`, judging by the kernel as
/// phase B left it — the disk `/` is on, by the link of `/`'s partition
/// as read back: the disk `/boot` is on too, where a gpt has the
/// `bios_grub` partition made for GRUB's core, and an msdos disk its first
/// MiB free. Never another: the disk named swapped for the layout's other
/// disk, or for one the kernel does not put `/` on, or `/` named on a
/// partition of the other disk, is refused before GRUB is asked anything.
#[test]
fn grub_on_a_bios_goes_on_the_disk_that_holds_root_as_read_back() {
    let bios = Firmware::Bios;
    for sector in [512, 4096] {
        let mut layouts: Vec<(String, Vec<DiskLayout>)> = cases(sector)
            .into_iter()
            .filter(|case| case.firmware == bios)
            .map(|case| (case.name.to_string(), case.layout))
            .collect();
        assert_eq!(layouts.len(), 7, "the msdos and bios_grub fixtures");
        layouts.push(("a /boot beside /".into(), vec![boot_beside_root(sector)]));
        let two = root_beside_data(sector);
        let turned: Vec<DiskLayout> = two.iter().rev().cloned().collect();
        layouts.push(("/ on the second disk".into(), two));
        layouts.push(("/ on the second disk, listed first".into(), turned));
        for (name, layout) in &layouts {
            let at = format!("{name} at {sector}");
            let judged = layout::shape(layout, bios, Bootloader::Automatic, &Software::default());
            assert_eq!(judged, Ok(()), "{at}");
            let paper = on_paper(bios, layout);
            let kernel = kernel_of(layout, Some(&paper));
            let link = |name: &str| {
                use super::fence::Sysfs;
                kernel.link(name).ok_or_else(|| format!("no {name}"))
            };
            let parent = |device: &str| {
                let name = device.trim_start_matches("/dev/");
                let link = link(name).unwrap();
                parent_disk(&link, name).map(|disk| format!("/dev/{disk}"))
            };
            let assembled = mounted(
                layout,
                bios,
                &facts(layout, &paper.paths),
                &paper.paths,
                "/run/lxb-installer/stage-test/root".into(),
                0,
                String::new(),
            )
            .unwrap_or_else(|why| panic!("{at}: {why}"));
            let args =
                grub_bios_args(&assembled, &link).unwrap_or_else(|why| panic!("{at}: {why}"));
            let holds = |mount: &MountPoint| {
                used(layout)
                    .into_iter()
                    .find(|one| one.at == mount)
                    .map(|one| one.disk.disk.path.clone())
            };
            let root_disk = holds(&MountPoint::Root).unwrap();
            assert_eq!(args, ["--target=i386-pc", root_disk.as_str()], "{at}");
            assert_eq!(
                parent(&assembled.root_device),
                Some(root_disk.clone()),
                "{at}"
            );
            if let Some(boot) = holds(&MountPoint::Boot) {
                assert_eq!(boot, root_disk, "{at}: /boot");
            }
            let one = layout
                .iter()
                .find(|one| one.disk.path == root_disk)
                .unwrap();
            match layout::table_after(one).unwrap() {
                Table::Gpt => {
                    let core = one
                        .create
                        .iter()
                        .find(|made| made.fs == Fs::BiosGrub)
                        .unwrap_or_else(|| panic!("{at}: no bios_grub partition"));
                    let which = Which::Made {
                        disk: root_disk.clone(),
                        start: core.start,
                    };
                    let path = paper.paths.of(&which).unwrap();
                    assert_eq!(parent(path), Some(root_disk.clone()), "{at}: bios_grub");
                }
                Table::Msdos => {
                    let sector = u64::from(one.disk.logical_sector);
                    let first = paper.after[&root_disk]
                        .partitions
                        .iter()
                        .map(|entry| entry.start * sector)
                        .min()
                        .unwrap();
                    assert!(first >= MIB, "{at}: GRUB's gap");
                }
            }
            // Never another disk.
            let other = layout
                .iter()
                .map(|one| one.disk.path.clone())
                .find(|path| *path != root_disk)
                .unwrap_or_else(|| "/dev/vdw".into());
            let mut elsewhere = assembled.clone();
            elsewhere.disk = other.clone();
            let refused = grub_bios_args(&elsewhere, &link).unwrap_err();
            assert!(refused.contains("is not on"), "{at}: {refused}");
            if let Some(foreign) = layout
                .iter()
                .filter(|one| one.disk.path != root_disk)
                .flat_map(|one| &one.disk.partitions)
                .next()
            {
                let mut moved = assembled.clone();
                moved.root_device = foreign.path.clone();
                assert!(grub_bios_args(&moved, &link).is_err(), "{at}: / moved");
            }
        }
    }
}

// -------------------------------------------------------------------------
// The consent, written apart, and what the commands destroy.

/// The consent as the handoff documents it (§7.0, Unit Q, "The request" 4,
/// and Q1's reading 2), written here from those words alone and never from
/// `layout::destroys`: the disks by path; on each, a new table first —
/// `table <disk> gpt|msdos`, then `erases` and every partition on the disk
/// by number, where it has any — and nothing else of that disk; otherwise
/// each reviewed partition by number: deleted `delete <path> <number>
/// start=<bytes> size=<bytes> <fs>`, shrunk `shrink <path> <number> start=
/// from= to= <fs>`, formatted `format <path> <number> start= size= to=
/// <new fs>`, kept nothing. `<fs>` is lsblk's name for what is there (`-`
/// for nothing); the new one is GParted's name.
fn documented_consent(layout: &[DiskLayout]) -> Vec<String> {
    let mut disks: Vec<&DiskLayout> = layout.iter().collect();
    disks.sort_by(|one, other| one.disk.path.cmp(&other.disk.path));
    let mut lines = Vec::new();
    for one in disks {
        let mut parts: Vec<(&Part, &Partition)> =
            one.parts.iter().zip(&one.disk.partitions).collect();
        parts.sort_by_key(|(part, _)| part.number);
        if let Some(table) = one.table {
            let mut line = format!(
                "table {} {}",
                one.disk.path,
                if table == Table::Gpt { "gpt" } else { "msdos" }
            );
            if !parts.is_empty() {
                line.push_str(" erases");
                for (part, _) in &parts {
                    line.push(' ');
                    line.push_str(&part.path);
                }
            }
            lines.push(line);
            continue;
        }
        for (part, was) in parts {
            let fs = if was.fs_type.is_empty() {
                "-"
            } else {
                was.fs_type.as_str()
            };
            let (path, number, start, size) = (&part.path, part.number, part.start, part.size);
            match &part.fate {
                Fate::Keep { .. } => {}
                Fate::Delete => lines.push(format!(
                    "delete {path} {number} start={start} size={size} {fs}"
                )),
                Fate::Shrink { to, .. } => lines.push(format!(
                    "shrink {path} {number} start={start} from={size} to={to} {fs}"
                )),
                Fate::Format {
                    fs: new, size: to, ..
                } => lines.push(format!(
                    "format {path} {number} start={start} size={size} to={to} {}",
                    new.name()
                )),
            }
        }
    }
    lines
}

/// What happened to one reviewed partition, as the commands tell it.
#[derive(Default)]
struct Told {
    deleted: bool,
    /// Its entry's new size in bytes, from `-N`.
    entry: Option<u64>,
    moved: bool,
    /// Its filesystem shrunk: (filesystem, from, to).
    shrunk: Option<(String, u64, u64)>,
    retyped: bool,
    wiped: bool,
    made: Option<&'static str>,
}

/// One disk as the commands tell it.
#[derive(Default)]
struct DiskTold {
    wiped: bool,
    label: Option<String>,
    bare_delete: bool,
    /// Partitions made: start and end, in bytes.
    spans: Vec<(u64, u64)>,
    /// Each reviewed partition by its number as reviewed.
    parts: BTreeMap<u32, Told>,
    /// The numbering as it is now: (number now, number reviewed).
    numbering: Vec<(u32, u32)>,
    odd: Vec<String>,
}

/// The `start=` and `size=` of a script line, in sectors.
fn start_and_size(line: &str) -> Option<(u64, u64)> {
    let mut start = None;
    let mut size = None;
    for field in line.split(',') {
        let field = field.trim();
        if let Some(value) = field.strip_prefix("start=") {
            start = value.parse().ok();
        } else if let Some(value) = field.strip_prefix("size=") {
            size = value.parse().ok();
        }
    }
    Some((start?, size?))
}

/// What one disk's `sfdisk --delete` does to its numbering, as sfdisk does
/// it, one number at a time: on msdos a logical partition's going numbers
/// those after it one lower, and an extended partition's takes every
/// logical partition with it.
fn delete_one(told: &mut DiskTold, one: &DiskLayout, disk: &str, number: u32) {
    let msdos = one.disk.table == Some(Table::Msdos);
    let Some(index) = told.numbering.iter().position(|(now, _)| *now == number) else {
        told.odd
            .push(format!("--delete {number} on {disk}: none there"));
        return;
    };
    let (_, was) = told.numbering.remove(index);
    told.parts.entry(was).or_default().deleted = true;
    if !msdos {
        return;
    }
    if number > 4 {
        for (now, _) in &mut told.numbering {
            if *now > number {
                *now -= 1;
            }
        }
        return;
    }
    let extended = one
        .disk
        .partitions
        .iter()
        .find(|part| part.number == was)
        .is_some_and(Partition::is_extended);
    if extended {
        let logical: Vec<u32> = told
            .numbering
            .iter()
            .filter(|(now, _)| *now > 4)
            .map(|(_, was)| *was)
            .collect();
        for was in logical {
            told.parts.entry(was).or_default().deleted = true;
        }
        told.numbering.retain(|(now, _)| *now <= 4);
    }
}

/// **What the commands destroy**, read from nothing but the commands, the
/// disks as they were reviewed and the tables read before (to say what a
/// number or a device was): which partitions a `wipefs` and a new table
/// erase, which `--delete` takes — numbered as sfdisk numbers them while it
/// deletes — which entries `-N` resizes and to what, which filesystems are
/// shrunk, which devices are wiped and made a filesystem, and whether
/// anything made, or any entry `-N` gives a new size, lies over a partition
/// that stays — inside an extended partition, over the table that stands
/// before a logical one. Said in the consent's
/// own lines, with a line of its own for anything the consent has no line
/// for.
fn destroyed_by(
    layout: &[DiskLayout],
    before: &BTreeMap<String, OnDisk>,
    planned: &[&Planned],
    paths: &Paths,
) -> Vec<String> {
    let mut disks: BTreeMap<String, DiskTold> = BTreeMap::new();
    for one in layout {
        let mut told = DiskTold::default();
        if let Some(read) = before.get(&one.disk.path) {
            let sector = u64::from(one.disk.logical_sector);
            for entry in &read.partitions {
                if let Some(was) = one
                    .disk
                    .partitions
                    .iter()
                    .find(|was| was.start == entry.start * sector)
                {
                    told.numbering.push((entry.number, was.number));
                }
            }
        }
        disks.insert(one.disk.path.clone(), told);
    }
    let disk_named = |disk: &str| layout.iter().find(|one| one.disk.path == disk);
    let reviewed = |device: &str| {
        layout.iter().find_map(|one| {
            one.disk
                .partitions
                .iter()
                .find(|was| was.path == device)
                .map(|was| (one.disk.path.clone(), was.number))
        })
    };
    let reviewed_number = |told: &DiskTold, number: &str| {
        let number: u32 = number.parse().ok()?;
        told.numbering
            .iter()
            .find(|(now, _)| *now == number)
            .map(|(_, was)| *was)
    };
    let mut odd = Vec::new();
    for planned in planned {
        let command = match planned {
            Planned::ShrinkFilesystem {
                device,
                fs,
                from,
                to,
            } => {
                match reviewed(device) {
                    Some((disk, number)) => {
                        let told = disks.get_mut(&disk).unwrap();
                        told.parts.entry(number).or_default().shrunk =
                            Some((fs.clone(), *from, *to));
                    }
                    None => odd.push(format!("shrink of {device}, nothing reviewed")),
                }
                continue;
            }
            Planned::Run { command, .. } => command,
        };
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        match (command.program, args.as_slice()) {
            ("wipefs", ["--all", "--lock", disk]) if disks.contains_key(*disk) => {
                disks.get_mut(*disk).unwrap().wiped = true;
            }
            ("sfdisk", ["--lock", how, .., disk])
                if disks.contains_key(*disk) && (*how == "--wipe" || *how == "--append") =>
            {
                let sector = u64::from(disk_named(disk).unwrap().disk.logical_sector);
                let told = disks.get_mut(*disk).unwrap();
                let shape: &[&str] = if *how == "--wipe" {
                    &["--lock", "--wipe", "always", "--wipe-partitions", "always"]
                } else {
                    &["--lock", "--append", "--wipe-partitions", "always"]
                };
                if args[..args.len() - 1] != *shape {
                    told.odd.push(format!("sfdisk {args:?}"));
                }
                let mut unit = None;
                for line in command.input.as_deref().unwrap_or("").lines() {
                    if let Some(label) = line.strip_prefix("label: ") {
                        if *how == "--wipe" {
                            told.label = Some(label.to_string());
                        } else {
                            told.odd.push(format!("a label given {disk} by --append"));
                        }
                    } else if let Some(size) = line.strip_prefix("sector-size: ") {
                        unit = size.parse::<u64>().ok();
                    } else if let Some((start, size)) = start_and_size(line) {
                        let unit = unit.unwrap_or(sector);
                        told.spans.push((start * unit, (start + size) * unit));
                    }
                }
                if unit != Some(sector) {
                    told.odd
                        .push(format!("{disk} counted in {unit:?} byte sectors"));
                }
            }
            ("sfdisk", ["--lock", "-N", number, disk]) if disks.contains_key(*disk) => {
                let one = disk_named(disk).unwrap();
                let sector = u64::from(one.disk.logical_sector);
                let told = disks.get_mut(*disk).unwrap();
                // One entry's place, in the unit its script's header says:
                // a script saying none, or another than the disk's, is said.
                let mut unit = None;
                let mut places = Vec::new();
                for line in command.input.as_deref().unwrap_or("").lines() {
                    if let Some(size) = line.strip_prefix("sector-size: ") {
                        unit = size.parse::<u64>().ok();
                    } else if let Some(place) = start_and_size(line) {
                        places.push(place);
                    }
                }
                if unit != Some(sector) {
                    told.odd.push(format!(
                        "-N {number} on {disk} counted in {unit:?} byte sectors"
                    ));
                }
                let unit = unit.unwrap_or(sector);
                match (reviewed_number(told, number), places.as_slice()) {
                    (Some(was), [(start, size)]) => {
                        let reviewed_start = one
                            .disk
                            .partitions
                            .iter()
                            .find(|part| part.number == was)
                            .unwrap()
                            .start;
                        let part = told.parts.entry(was).or_default();
                        part.entry = Some(size * unit);
                        part.moved |= start * unit != reviewed_start;
                    }
                    _ => told.odd.push(format!("-N {number} on {disk}")),
                }
            }
            ("sfdisk", ["--lock", "--delete", disk, numbers @ ..]) if disks.contains_key(*disk) => {
                let one = disk_named(disk).unwrap();
                let told = disks.get_mut(*disk).unwrap();
                if numbers.is_empty() {
                    told.bare_delete = true;
                }
                for number in numbers {
                    match number.parse::<u32>() {
                        Ok(number) => delete_one(told, one, disk, number),
                        Err(_) => told.odd.push(format!("--delete {number}")),
                    }
                }
            }
            ("sfdisk", ["--lock", "--part-type" | "--part-label", disk, number, _])
                if disks.contains_key(*disk) =>
            {
                let told = disks.get_mut(*disk).unwrap();
                match reviewed_number(told, number) {
                    Some(was) => told.parts.entry(was).or_default().retyped = true,
                    None => told.odd.push(format!("type of {number} on {disk}")),
                }
            }
            ("sfdisk", ["--lock", "--activate", disk, _]) if disks.contains_key(*disk) => {}
            (program, [.., device]) if program == "wipefs" || program.starts_with("mk") => {
                let made = match (program, args.as_slice()) {
                    ("wipefs", ["--all", _]) => None,
                    ("mkfs.ext4", ["-F", ..]) => Some("ext4"),
                    ("mkfs.btrfs", ["-f", ..]) => Some("btrfs"),
                    ("mkfs.xfs", ["-f", ..]) => Some("xfs"),
                    ("mkfs.fat", ["-F", "32", ..]) => Some("fat32"),
                    ("mkswap", _) => Some("linux-swap"),
                    _ => {
                        odd.push(format!("{program} {args:?}"));
                        continue;
                    }
                };
                match paths.which(device) {
                    Some(Which::Part(path)) => {
                        let (disk, number) = reviewed(path).expect("a reviewed partition");
                        let part = disks
                            .get_mut(&disk)
                            .unwrap()
                            .parts
                            .entry(number)
                            .or_default();
                        if made.is_some() {
                            part.made = made;
                        } else {
                            part.wiped = true;
                        }
                    }
                    Some(Which::Made { .. }) => {}
                    None => odd.push(format!("{program} on {device}, nothing of the layout")),
                }
            }
            (program, args) => odd.push(format!("{program} {args:?}")),
        }
    }

    let mut lines = Vec::new();
    for (path, told) in &disks {
        let one = disk_named(path).unwrap();
        let mut reviewed: Vec<&Partition> = one.disk.partitions.iter().collect();
        reviewed.sort_by_key(|part| part.number);
        if told.bare_delete {
            lines.push(format!("every partition of {path} deleted"));
        }
        match (told.wiped, &told.label) {
            (true, Some(label)) => {
                let name = if label == "dos" {
                    "msdos"
                } else {
                    label.as_str()
                };
                let mut line = format!("table {path} {name}");
                if !reviewed.is_empty() {
                    line.push_str(" erases");
                    for part in &reviewed {
                        line.push(' ');
                        line.push_str(&part.path);
                    }
                }
                lines.push(line);
            }
            (false, None) => {}
            _ => lines.push(format!("half a new table on {path}")),
        }
        for was in &reviewed {
            let Some(part) = told.parts.get(&was.number) else {
                continue;
            };
            let fs = if was.fs_type.is_empty() {
                "-"
            } else {
                was.fs_type.as_str()
            };
            let (p, number, start, size) = (&was.path, was.number, was.start, was.size);
            if part.deleted {
                lines.push(format!(
                    "delete {p} {number} start={start} size={size} {fs}"
                ));
                if part.made.is_some() || part.wiped || part.entry.is_some() {
                    lines.push(format!("{p} written to as well as deleted"));
                }
            } else if part.made.is_some() || part.wiped {
                let to = part.entry.unwrap_or(size);
                let new = part.made.unwrap_or("bios_grub");
                lines.push(format!(
                    "format {p} {number} start={start} size={size} to={to} {new}"
                ));
                if part.shrunk.is_some() {
                    lines.push(format!("{p} shrunk as well as formatted"));
                }
            } else if let Some((shrunk_fs, from, to)) = &part.shrunk {
                if part.entry == Some(*to) && *from == size && shrunk_fs == fs {
                    lines.push(format!(
                        "shrink {p} {number} start={start} from={size} to={to} {fs}"
                    ));
                } else {
                    lines.push(format!("{p}'s filesystem and entry shrunk apart"));
                }
            } else if part.entry.is_some() {
                lines.push(format!("{p}'s entry resized with nothing agreed"));
            }
            if !part.deleted && part.made.is_none() && !part.wiped && part.retyped {
                lines.push(format!("{p} retyped with nothing agreed"));
            }
            if part.moved {
                lines.push(format!("{p} moved"));
            }
        }
        // Nothing made may lie over a partition that stays, as it stays.
        let stays = |was: &Partition| !told.parts.get(&was.number).is_some_and(|part| part.deleted);
        let end_of = |was: &Partition| {
            if was.is_extended() {
                one.disk
                    .partitions
                    .iter()
                    .filter(|part| part.number > 4)
                    .map(|part| part.start + part.size)
                    .fold(was.start + was.size, u64::max)
            } else {
                was.start
                    + told
                        .parts
                        .get(&was.number)
                        .and_then(|part| part.entry)
                        .unwrap_or(was.size)
            }
        };
        if !told.wiped {
            for was in reviewed.iter().filter(|was| stays(was)) {
                let end = end_of(was);
                if told
                    .spans
                    .iter()
                    .any(|(start, stop)| *start < end && was.start < *stop)
                {
                    lines.push(format!("{} written over by a partition made", was.path));
                }
            }
        }
        // Nor may an entry given a new size (`-N`) reach over a partition
        // that stays: another's place as it stays, an extended partition's
        // frame — or, inside one, the table that stands before each logical
        // partition, between it and the end of the logical partition before
        // it on the disk (or the frame's start), which discovery never shows.
        let logical = |part: &Partition| one.disk.table == Some(Table::Msdos) && part.number > 4;
        let guarded_from = |other: &Partition| {
            if !logical(other) {
                return other.start;
            }
            let frame = one
                .disk
                .partitions
                .iter()
                .find(|part| part.is_extended())
                .map_or(other.start, |frame| frame.start);
            one.disk
                .partitions
                .iter()
                .filter(|part| logical(part) && part.start < other.start)
                .map(|part| part.start + part.size)
                .fold(frame, u64::max)
        };
        if !told.wiped {
            for was in reviewed.iter().filter(|was| stays(was)) {
                if told
                    .parts
                    .get(&was.number)
                    .and_then(|part| part.entry)
                    .is_none()
                {
                    continue;
                }
                let (from, to) = (was.start, end_of(was));
                for other in reviewed.iter().filter(|other| stays(other)) {
                    let inside_its_frame = other.is_extended() && logical(was);
                    if other.number == was.number || inside_its_frame {
                        continue;
                    }
                    if guarded_from(other) < to && from < end_of(other) {
                        lines.push(format!(
                            "{} written over by {}'s entry",
                            other.path, was.path
                        ));
                    }
                }
            }
        }
        lines.extend(told.odd.iter().cloned());
    }
    lines.extend(odd);
    lines
}

/// The rule set's own consent for `layout`, as the request carries it.
fn consent(layout: &[DiskLayout]) -> Vec<String> {
    layout::destroys(layout).iter().map(|d| d.line()).collect()
}

#[test]
fn what_the_commands_destroy_is_the_consent_at_512_and_4096() {
    for sector in [512, 4096] {
        for case in cases(sector) {
            let paper = on_paper(case.firmware, &case.layout);
            let documented = documented_consent(&case.layout);
            assert_eq!(
                destroyed_by(&case.layout, &paper.before, &paper.all(), &paper.paths),
                documented,
                "{} at {sector}",
                case.name
            );
            assert_eq!(
                consent(&case.layout),
                documented,
                "{} at {sector}",
                case.name
            );
        }
    }
}

/// **The consent names logical partitions by the numbers they were
/// reviewed with** (Unit Q, Q7): on the old Linux disk in its order and
/// numbered out of it, each line the person agrees to — and the worker
/// holds the request to — names a logical partition by its path and its
/// reviewed number, in number order whatever stands where; that number is
/// the one phase A's table gives the partition at the line's `start=`, and
/// the one every command acting on it gives sfdisk; and a partition
/// formatted is made at the path that number names once the tables are
/// written. The question, the consent and the commands name one partition.
#[test]
fn the_consent_names_logical_partitions_by_their_reviewed_numbers() {
    let mib = |n: u64| n * MIB;
    for sector in [512, 4096] {
        let bytes = u64::from(sector);
        let root = format!(
            "format /dev/vdx1 1 start={} size={} to={} ext4",
            mib(1),
            mib(28672),
            mib(28672)
        );
        for (name, lines) in [
            (
                "msdos-logical",
                vec![
                    root.clone(),
                    format!(
                        "format /dev/vdx6 6 start={} size={} to={} linux-swap",
                        mib(36867),
                        mib(2048),
                        mib(12289)
                    ),
                    format!(
                        "delete /dev/vdx7 7 start={} size={} ext4",
                        mib(38916),
                        mib(10240)
                    ),
                ],
            ),
            (
                "msdos-jumbled",
                vec![
                    root.clone(),
                    format!(
                        "format /dev/vdx5 5 start={} size={} to={} ext4",
                        mib(38916),
                        mib(10240),
                        mib(10240)
                    ),
                    format!(
                        "delete /dev/vdx7 7 start={} size={} swap",
                        mib(36867),
                        mib(2048)
                    ),
                ],
            ),
        ] {
            let at = format!("{name} at {sector}");
            let case = case(sector, name);
            assert_eq!(consent(&case.layout), lines, "{at}");
            let paper = on_paper(case.firmware, &case.layout);
            let (before, after) = (&paper.before["/dev/vdx"], &paper.after["/dev/vdx"]);
            for line in &lines {
                let words: Vec<&str> = line.split_whitespace().collect();
                let (path, number) = (words[1], words[2].parse::<u32>().unwrap());
                let start: u64 = words[3].strip_prefix("start=").unwrap().parse().unwrap();
                assert_eq!(path, format!("/dev/vdx{number}"), "{at}");
                let entry = before.partitions.iter().find(|e| e.start * bytes == start);
                assert_eq!(entry.map(|e| e.number), Some(number), "{at}: {line}");
                let named: Vec<u32> = paper
                    .tables
                    .iter()
                    .flat_map(|planned| numbers_named(planned, bytes, before, after))
                    .filter(|(_, named_at)| *named_at == Some(start))
                    .map(|(named, _)| named)
                    .collect();
                assert!(!named.is_empty(), "{at}: nothing names {line}");
                assert!(named.iter().all(|n| *n == number), "{at}: {named:?}");
                if words[0] == "format" {
                    let made = paper.formats.iter().any(|planned| planned.device() == path);
                    assert!(made, "{at}: {path} is not made");
                }
            }
        }
    }
}

/// The arguments of a planned command, for a test to change.
fn args_of(planned: &mut Planned) -> &mut Vec<String> {
    match planned {
        Planned::Run { command, .. } => &mut command.args,
        Planned::ShrinkFilesystem { .. } => unreachable!("no command"),
    }
}

/// The planned command of `act`, for a test to change.
fn first(planned: &mut [Planned], act: Act) -> &mut Planned {
    planned
        .iter_mut()
        .find(|planned| planned.act() == act)
        .expect("planned")
}

#[test]
fn the_oracle_sees_a_command_that_destroys_what_was_not_agreed_to() {
    // Each change below is what a bug in the commands would look like; the
    // oracle must tell each from the consent.
    let check = |name: &str, change: &dyn Fn(&mut Vec<Planned>)| {
        let case = case(512, name);
        let paper = on_paper(case.firmware, &case.layout);
        let mut planned: Vec<Planned> = paper.all().into_iter().cloned().collect();
        change(&mut planned);
        let refs: Vec<&Planned> = planned.iter().collect();
        let told = destroyed_by(&case.layout, &paper.before, &refs, &paper.paths);
        assert_ne!(told, documented_consent(&case.layout), "{name}: {told:#?}");
    };
    let input_of = |planned: &mut Planned, input: &str| {
        if let Planned::Run { command, .. } = planned {
            command.input = Some(input.to_string());
        }
    };
    // The deletions lowest first: the extended partition takes its logical
    // one with it, and 5 is then no partition at all.
    check("msdos-extended", &|planned| {
        args_of(first(planned, Act::Delete)).splice(3.., ["2".to_string(), "5".to_string()]);
    });
    // `--delete` with no number deletes every partition.
    check("msdos-logical", &|planned| {
        args_of(first(planned, Act::Delete)).truncate(3);
    });
    // A number off by one: Data, kept, deleted instead.
    check("msdos-logical", &|planned| {
        *args_of(first(planned, Act::Delete)).last_mut().unwrap() = "5".into();
    });
    // The entry shrunk without its filesystem.
    check("gpt-windows", &|planned| {
        planned.retain(|p| p.act() != Act::ShrinkFilesystem);
    });
    // The entry given another start.
    check("gpt-windows", &|planned| {
        input_of(
            first(planned, Act::ShrinkEntry),
            "unit: sectors\nsector-size: 512\nstart=241664, size=41943040\n",
        );
    });
    // The entry's place given with no unit or sector size, or counted in
    // sectors of another size than the disk's.
    check("gpt-windows", &|planned| {
        input_of(
            first(planned, Act::ShrinkEntry),
            "start=239616, size=41943040\n",
        );
    });
    check("gpt-windows", &|planned| {
        input_of(
            first(planned, Act::ShrinkEntry),
            "unit: sectors\nsector-size: 4096\nstart=29952, size=5242880\n",
        );
    });
    // Windows' recovery partition formatted, or the EFI partition, kept.
    check("gpt-windows", &|planned| {
        *args_of(first(planned, Act::Make)).last_mut().unwrap() = "/dev/vdx4".into();
    });
    check("gpt-windows", &|planned| {
        planned.push(Planned::Run {
            act: Act::WipeSignatures,
            device: "/dev/vdx1".into(),
            command: crate::worker::partition::script::Command::new(
                "wipefs",
                &["--all", "/dev/vdx1"],
            ),
        });
    });
    // A partition made over Windows as it stays.
    check("gpt-windows", &|planned| {
        if let Planned::Run { command, .. } = first(planned, Act::Append) {
            let input = command.input.take().unwrap();
            command.input = Some(input.replace("start=42182656", "start=40000000"));
        }
    });
    // A kept partition retyped.
    check("msdos-logical", &|planned| {
        args_of(first(planned, Act::Retype))[3] = "5".into();
    });
    // A new table without its wipe.
    check("empty", &|planned| planned.retain(|p| p.act() != Act::Wipe));
    // Formatted to another filesystem than agreed.
    check("msdos", &|planned| {
        if let Planned::Run { command, .. } = first(planned, Act::Make) {
            command.program = "mkfs.btrfs";
            command.args = vec!["-f".into(), "/dev/vdx2".into()];
        }
    });
    // Grown to another size than agreed.
    check("msdos", &|planned| {
        input_of(
            first(planned, Act::Grow),
            "unit: sectors\nsector-size: 512\nstart=1026048, size=83888128\n",
        );
    });
}

/// A layout whose consent and commands agree — the rule set's lines and the
/// ones written apart alike — but whose formatted partition's entry is grown
/// over a partition that stays: a bug the rule set would share with the
/// interface, which only the oracle reading the commands can catch. Each is
/// one the rule set refuses (it is no layout the interface could send); the
/// oracle must see it as well, from the `-N` command alone.
#[test]
fn the_oracle_sees_an_entry_grown_over_a_partition_that_stays() {
    for sector in [512, 4096] {
        // Windows formatted and its entry grown into Windows' recovery
        // partition, which is kept.
        let windows = with(
            kept(fixtures::disk_gpt_windows(sector)),
            3,
            format(Fs::Ext4, 64785 + 100 - 117, at("/")),
        );
        // A logical partition formatted and grown up to the next one's
        // start, over the table that stands before it.
        let logical = with(
            kept(disk_msdos_logical(sector)),
            5,
            format(Fs::Ext4, 36867 - 28674, at("/data")),
        );
        for (name, firmware, one, grown, over) in [
            ("windows", Firmware::Uefi, windows, "/dev/vdx3", "/dev/vdx4"),
            ("logical", Firmware::Bios, logical, "/dev/vdx5", "/dev/vdx6"),
        ] {
            let layout = vec![one];
            let name = format!("{name} at {sector}");
            assert!(
                matches!(
                    layout::shape(
                        &layout,
                        firmware,
                        Bootloader::Automatic,
                        &Software::default()
                    ),
                    Err(layout::Refusal::Overlap { .. })
                ),
                "{name}: the rule set refuses it"
            );
            let paper = on_paper(firmware, &layout);
            assert!(
                paper
                    .tables
                    .iter()
                    .any(|planned| planned.act() == Act::Grow),
                "{name}: grown by -N"
            );
            let documented = documented_consent(&layout);
            assert_eq!(consent(&layout), documented, "{name}");
            let told = destroyed_by(&layout, &paper.before, &paper.all(), &paper.paths);
            let seen = format!("{over} written over by {grown}'s entry");
            assert!(told.contains(&seen), "{name}: {told:#?}");
            assert_ne!(told, documented, "{name}");
        }
    }
}

// -------------------------------------------------------------------------
// Layouts made up from a seed.

/// A small seeded generator (xorshift64*), so a layout that fails is told
/// by its seed and made again exactly.
struct Dice(u64);

impl Dice {
    fn new(seed: u64) -> Self {
        Dice(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn one_in(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

/// Every fixture disk, each filesystem given a UUID, so the read back looks
/// for it, and the msdos Windows disk's System Reserved marked active as
/// Windows marks it ([`disk_marked_msdos`]).
fn every_disk(sector: u32) -> Vec<Disk> {
    let mut disks: Vec<Disk> = fixtures::all(sector)
        .into_iter()
        .map(|(name, disk)| match name {
            "msdos" => disk_marked_msdos(sector),
            _ => disk,
        })
        .collect();
    disks.push(disk_msdos_logical(sector));
    for disk in &mut disks {
        for part in &mut disk.partitions {
            if !part.fs_type.is_empty() {
                part.uuid = format!("{:04X}-{:04X}", part.number, part.start / MIB);
            }
        }
    }
    disks
}

/// Where nothing stands on `one`'s disk once the layout is carried out,
/// within what its table reaches, each run trimmed to whole MiB; none on an
/// msdos disk that keeps an extended partition, beside which nothing is
/// made.
fn free_spans(one: &DiskLayout) -> Vec<(u64, u64)> {
    let Some(table) = layout::table_after(one) else {
        return Vec::new();
    };
    let disk = &one.disk;
    let sector = u64::from(disk.logical_sector);
    let room = layout::usable(table, disk.size / sector, sector);
    let (first, last) = (room.start * sector, room.end * sector);
    let mut standing = Vec::new();
    if one.table.is_none() {
        for (part, was) in one.parts.iter().zip(&disk.partitions) {
            if layout::logical(disk.table, was) || part.fate == Fate::Delete {
                continue;
            }
            if was.is_extended() {
                return Vec::new();
            }
            let end = match &part.fate {
                Fate::Shrink { to, .. } => part.start + to,
                Fate::Format { size, .. } => part.start + size,
                _ => part.start + part.size,
            };
            standing.push((part.start, end));
        }
    }
    for made in &one.create {
        standing.push((made.start, made.start + made.size));
    }
    standing.sort_unstable();
    let mut spans = Vec::new();
    let mut cursor = first;
    for (start, end) in standing {
        if start > cursor {
            spans.push((cursor, start));
        }
        cursor = cursor.max(end);
    }
    if last > cursor {
        spans.push((cursor, last));
    }
    spans
        .into_iter()
        .map(|(start, end)| (start.div_ceil(MIB) * MIB, end / MIB * MIB))
        .filter(|(start, end)| end > start)
        .collect()
}

/// Random fates for `one`'s reviewed partitions, their own paths named
/// with `own` and the partition's number.
fn random_fates(dice: &mut Dice, one: &mut DiskLayout, own: char) {
    for (part, was) in one.parts.iter_mut().zip(&one.disk.partitions) {
        if was.is_extended() {
            continue;
        }
        let own_path = || at(&format!("/{own}{}", was.number));
        let half = part.size / 2 / MIB * MIB;
        part.fate = match dice.below(7) {
            0 | 1 => Fate::Keep { mount: None },
            2 => Fate::Delete,
            3 => format(Fs::Ext4, part.size / MIB, own_path()),
            4 if layout::shrinkable(was) && half > 0 => Fate::Shrink {
                to: half,
                mount: if dice.one_in(2) { own_path() } else { None },
            },
            4 => format(Fs::Xfs, part.size / MIB, None),
            5 if layout::mountable(&was.fs_type) => Fate::Keep { mount: own_path() },
            5 => Fate::Delete,
            _ if part.size >= 2 * GIB => format(Fs::Btrfs, part.size / MIB / 2, own_path()),
            _ => Fate::Keep { mount: None },
        };
    }
    // An extended partition goes once every logical partition in it does.
    let logical_kept = one
        .parts
        .iter()
        .zip(&one.disk.partitions)
        .any(|(part, was)| layout::logical(one.disk.table, was) && part.fate != Fate::Delete);
    for (part, was) in one.parts.iter_mut().zip(&one.disk.partitions) {
        if was.is_extended() && !logical_kept && dice.one_in(2) {
            part.fate = Fate::Delete;
        }
    }
}

/// Now and then a formatted partition's entry grown into the free space
/// straight after it, by half of it.
fn grow_some(dice: &mut Dice, one: &mut DiskLayout) {
    let spans = free_spans(one);
    for index in 0..one.parts.len() {
        let part = &one.parts[index];
        let Fate::Format { size, .. } = part.fate else {
            continue;
        };
        let end = part.start + size;
        let after = spans.iter().find(|(start, _)| *start == end);
        if let (Some((start, stop)), true) = (after, dice.one_in(2)) {
            let more = (stop - start) / 2 / MIB * MIB;
            if let Fate::Format { size, .. } = &mut one.parts[index].fate {
                *size += more;
            }
            return;
        }
    }
}

/// What starts the computer and `/`, put on `one`: in its largest free
/// span where there is room, else `/` on a partition it formats.
fn boot_and_root(dice: &mut Dice, one: &mut DiskLayout, firmware: Firmware) {
    let table = layout::table_after(one).expect("a table");
    let mut spans = free_spans(one);
    spans.sort_by_key(|(start, end)| std::cmp::Reverse(end - start));
    let (mut cursor, end) = spans.first().copied().unwrap_or((0, 0));
    let root_fs = if dice.one_in(2) { Fs::Ext4 } else { Fs::Btrfs };
    let room = end.saturating_sub(cursor);
    let mut boot = None;
    match firmware {
        Firmware::Uefi => {
            let esp = one
                .parts
                .iter()
                .zip(&one.disk.partitions)
                .position(|(part, was)| layout::kept_esp(was) && part.fate.keeps());
            match esp {
                Some(index) if dice.one_in(2) || room < 27 * GIB => {
                    one.parts[index].fate = Fate::Keep { mount: at("/boot") };
                }
                _ => boot = Some((GIB, Fs::Fat32, at("/boot"))),
            }
        }
        Firmware::Bios if table == Table::Gpt => boot = Some((MIB, Fs::BiosGrub, None)),
        Firmware::Bios => {}
    }
    if room < 27 * GIB {
        // `/` on a partition the layout formats, large enough.
        if let Some(part) = one
            .parts
            .iter_mut()
            .filter(|part| matches!(part.fate, Fate::Format { .. } | Fate::Keep { mount: None }))
            .find(|part| part.size >= 26 * GIB)
        {
            part.fate = format(root_fs, part.size / MIB, at("/"));
        }
    }
    let mut places = Vec::new();
    if let Some((size, fs, mount)) = boot {
        if room >= size {
            places.push((size, fs, mount));
        }
    }
    if room >= 27 * GIB {
        let swap = if dice.one_in(3) { GIB } else { 0 };
        let taken: u64 = places.iter().map(|(size, ..)| size).sum();
        places.push((room - taken - swap, root_fs, at("/")));
        if swap > 0 {
            places.push((swap, Fs::LinuxSwap, at("swap")));
        }
    }
    for (size, fs, mount) in places {
        one.create.push(Create {
            start: cursor,
            size,
            fs,
            label: String::new(),
            mount,
        });
        cursor += size;
    }
    one.create.sort_by_key(|made| made.start);
}

/// The disk that holds `/`, laid out from the dice.
fn root_disk(dice: &mut Dice, disk: Disk, firmware: Firmware) -> DiskLayout {
    let mut one = if disk.table.is_none() || dice.one_in(4) {
        let table = if firmware == Firmware::Bios && dice.one_in(2) {
            Table::Msdos
        } else {
            Table::Gpt
        };
        fresh(disk, table)
    } else {
        let mut one = kept(disk);
        random_fates(dice, &mut one, 'd');
        grow_some(dice, &mut one);
        one
    };
    boot_and_root(dice, &mut one, firmware);
    one
}

/// A second disk, laid out from the dice: data only.
fn other_disk(dice: &mut Dice, disk: Disk) -> DiskLayout {
    let mut one = if disk.table.is_none() || dice.one_in(3) {
        fresh(disk, Table::Gpt)
    } else {
        let mut one = kept(disk);
        random_fates(dice, &mut one, 'e');
        one
    };
    if dice.one_in(2) {
        let largest = free_spans(&one)
            .into_iter()
            .max_by_key(|(start, end)| end - start);
        if let Some((start, end)) = largest {
            one.create.push(Create {
                start,
                size: end - start,
                fs: Fs::Xfs,
                label: "Archive".into(),
                mount: at("/archive"),
            });
            one.create.sort_by_key(|made| made.start);
        }
    }
    one
}

/// A third disk, laid out from the dice (Q6): a partition on it kept as it
/// is at `/home` where one can be — the person's home from before, on a
/// disk nothing is written to unless the dice change more of it first —
/// else a `/home` made on a new table.
fn home_disk(dice: &mut Dice, disk: Disk) -> DiskLayout {
    let mut one = kept(disk);
    if dice.one_in(3) {
        random_fates(dice, &mut one, 'f');
    }
    let home = one
        .parts
        .iter()
        .zip(&one.disk.partitions)
        .position(|(part, was)| {
            part.fate == Fate::Keep { mount: None }
                && KEPT_HOME_FILESYSTEMS.contains(&was.fs_type.as_str())
                && was.size >= MIN_HOME
        });
    match home {
        Some(index) => one.parts[index].fate = Fate::Keep { mount: at("/home") },
        None => {
            one = fresh(one.disk, Table::Gpt);
            if let Some((start, end)) = free_spans(&one).into_iter().next() {
                one.create.push(Create {
                    start,
                    size: end - start,
                    fs: Fs::Ext4,
                    label: "Home".into(),
                    mount: at("/home"),
                });
            }
        }
    }
    one
}

/// A layout made up from `seed`: one disk, or two in either order — and
/// now and then a third, which keeps or makes the home, listed anywhere
/// among them.
fn seeded(seed: u64, sector: u32, firmware: Firmware) -> Vec<DiskLayout> {
    let mut dice = Dice::new(seed);
    let disks = every_disk(sector);
    let first = disks[dice.below(disks.len() as u64) as usize].clone();
    let mut layout = vec![root_disk(&mut dice, first, firmware)];
    if dice.one_in(4) {
        let second = disks[dice.below(disks.len() as u64) as usize].clone();
        layout.push(other_disk(&mut dice, renamed(second, "/dev/vdy", "254:16")));
        if dice.one_in(2) {
            layout.reverse();
        }
        if dice.one_in(3) {
            let third = disks[dice.below(disks.len() as u64) as usize].clone();
            layout.push(home_disk(&mut dice, renamed(third, "/dev/vdw", "254:32")));
            let at = dice.below(3) as usize;
            layout.swap(at, 2);
        }
    }
    layout
}

/// The kernel's view of every disk of `layout` as reviewed — what the fence
/// is asked against before phase B — or, with `paper`, once phase B is done.
fn kernel_of(layout: &[DiskLayout], paper: Option<&OnPaper>) -> Sys {
    let mut sys = Sys::default();
    for one in layout {
        let disk = one.disk.path.trim_start_matches("/dev/");
        sys.disk(disk);
        let written = paper.and_then(|paper| paper.kernel.get(&one.disk.path));
        let parts: Vec<(&str, u64, u64)> = match written {
            Some(found) => found
                .iter()
                .map(|part| (part.path.as_str(), part.start, part.size))
                .collect(),
            None => one
                .disk
                .partitions
                .iter()
                .map(|part| (part.path.as_str(), part.start, part.size))
                .collect(),
        };
        for (path, start, size) in parts {
            sys.part(disk, path.trim_start_matches("/dev/"), start, size);
        }
    }
    sys
}

/// Holds every sfdisk script of `paper` to saying first — after a new
/// table's label — its unit and the sector size of the disk it writes, as
/// that disk was reviewed: the scripts that make partitions, and those that
/// give one entry a new size (`-N`), alike.
fn says_its_sectors(name: &str, layout: &[DiskLayout], paper: &OnPaper) {
    for planned in &paper.tables {
        let Some(input) = planned.command().and_then(|command| command.input.as_ref()) else {
            continue;
        };
        let one = layout
            .iter()
            .find(|one| one.disk.path == planned.device())
            .unwrap_or_else(|| panic!("{name}: a script for {}", planned.device()));
        let body = ["label: gpt\n", "label: dos\n"]
            .iter()
            .find_map(|label| input.strip_prefix(label))
            .unwrap_or(input);
        let head = format!("unit: sectors\nsector-size: {}\n", one.disk.logical_sector);
        assert!(body.starts_with(&head), "{name}: {}", planned.shown());
    }
}

/// Holds one layout to everything: the order, every script's unit and
/// sector size, the consent written apart, the rule set's consent, and the
/// fence over every command. Gives what
/// the commands did, for the tests to see every kind was asked for.
fn holds_everything(name: &str, firmware: Firmware, layout: &[DiskLayout]) -> Vec<Act> {
    let paper = on_paper(firmware, layout);
    holds_the_order(name, layout, &paper);
    shrinks_fit_their_entries(name, layout, &paper);
    says_its_sectors(name, layout, &paper);
    let documented = documented_consent(layout);
    assert_eq!(
        destroyed_by(layout, &paper.before, &paper.all(), &paper.paths),
        documented,
        "{name}: {layout:#?}"
    );
    assert_eq!(consent(layout), documented, "{name}");
    let mut fence = Fence::new(layout, &paper.before);
    for (disk, after) in &paper.after {
        fence.written_table(disk, after);
    }
    let before = kernel_of(layout, None);
    for planned in &paper.tables {
        fence
            .check_planned(planned, &before)
            .unwrap_or_else(|stop| panic!("{name}: {}: {}", planned.shown(), stop.detail));
        // A filesystem's shrink is carried out by its own commands, each
        // judged by the fence once phase A has measured the filesystem; its
        // entry is made smaller only once the fence is handed what the
        // shrink left, read back — on paper, what its tool is asked to
        // leave: an ext4 its size in whole KiB, an NTFS a sector short of
        // its entry.
        let Ok(shrink) = filling(planned) else {
            continue;
        };
        let one = layout
            .iter()
            .find(|one| one.parts.iter().any(|part| part.path == shrink.device))
            .expect("the shrunk partition's disk");
        let start = one
            .parts
            .iter()
            .find(|part| part.path == shrink.device)
            .unwrap()
            .start;
        fence.measured(&one.disk.path, start, shrink.filesystem);
        for (run, _) in shrink.planned() {
            fence
                .check_planned(&run, &before)
                .unwrap_or_else(|stop| panic!("{name}: {}: {}", run.shown(), stop.detail));
        }
        let left = match shrink.tool {
            Tool::Ext4 => shrink.target_bytes(),
            Tool::Ntfs => shrink.to - u64::from(one.disk.logical_sector),
        };
        fence.filesystem_read_back(&one.disk.path, start, left);
    }
    fence.read_back(layout, &paper.paths);
    let after = kernel_of(layout, Some(&paper));
    for planned in &paper.formats {
        fence
            .check_planned(planned, &after)
            .unwrap_or_else(|stop| panic!("{name}: {}: {}", planned.shown(), stop.detail));
    }
    paper.all().iter().map(|planned| planned.act()).collect()
}

#[test]
fn every_fixture_layout_meets_the_fence_and_its_consent() {
    for sector in [512, 4096] {
        for case in cases(sector) {
            holds_everything(case.name, case.firmware, &case.layout);
        }
    }
}

/// **The disk a layout only mounts from is given nothing** (Unit Q, Q6):
/// of the three disks, the one that keeps the person's home at `/home` is
/// written by no command — every kind there is, the disk's own and a
/// partition's, aimed at it or at its home partition, is refused by the
/// fence with the reason that the layout does not write to that disk, as
/// the kernel has it before phase B and after it; and the layout's own
/// commands still meet it. (Were the disk taken for one whose table is
/// kept, a deletion, a new type or a boot flag there would be let through.)
#[test]
fn the_fence_gives_the_disk_only_mounted_from_nothing() {
    use crate::worker::partition::script::Command;
    for sector in [512, 4096] {
        let three = case(sector, "three-disks");
        holds_everything("three-disks", three.firmware, &three.layout);
        let paper = on_paper(three.firmware, &three.layout);
        let mut fence = Fence::new(&three.layout, &paper.before);
        fence.read_back(&three.layout, &paper.paths);
        let home = &three.layout[0].disk;
        assert_eq!(home.path, "/dev/vdc");
        let part = &home.partitions[0];
        let at = part.start / u64::from(sector);
        let size = part.size / u64::from(sector);
        let header = format!("unit: sectors\nsector-size: {sector}\n");
        let script = |line: &str| Some(format!("{header}{line}\n"));
        let run = |act: Act, device: &str, args: &[&str], input: Option<String>| {
            let mut command = Command::new(
                match act {
                    Act::Wipe | Act::WipeSignatures => "wipefs",
                    Act::Make => "mkfs.ext4",
                    _ => "sfdisk",
                },
                args,
            );
            command.input = input;
            Planned::Run {
                act,
                device: device.to_string(),
                command,
            }
        };
        let aimed = [
            Planned::ShrinkFilesystem {
                device: part.path.clone(),
                fs: "ext4".into(),
                from: part.size,
                to: part.size / 2,
            },
            run(
                Act::ShrinkEntry,
                "/dev/vdc",
                &["--lock", "-N", "1", "/dev/vdc"],
                script(&format!("start={at}, size={}", size / 2)),
            ),
            run(
                Act::Delete,
                "/dev/vdc",
                &["--lock", "--delete", "/dev/vdc", "1"],
                None,
            ),
            run(
                Act::Wipe,
                "/dev/vdc",
                &["--all", "--lock", "/dev/vdc"],
                None,
            ),
            run(
                Act::Fresh,
                "/dev/vdc",
                &[
                    "--lock",
                    "--wipe",
                    "always",
                    "--wipe-partitions",
                    "always",
                    "/dev/vdc",
                ],
                Some(format!("label: gpt\n{header}")),
            ),
            run(
                Act::Append,
                "/dev/vdc",
                &[
                    "--lock",
                    "--append",
                    "--wipe-partitions",
                    "always",
                    "/dev/vdc",
                ],
                script(&format!("start={at}, size={size}, type={HOME_GPT}")),
            ),
            run(
                Act::Grow,
                "/dev/vdc",
                &["--lock", "-N", "1", "/dev/vdc"],
                script(&format!("start={at}, size={}", size + 1)),
            ),
            run(
                Act::Retype,
                "/dev/vdc",
                &["--lock", "--part-type", "/dev/vdc", "1", HOME_GPT],
                None,
            ),
            run(
                Act::Rename,
                "/dev/vdc",
                &["--lock", "--part-label", "/dev/vdc", "1", "Home"],
                None,
            ),
            run(
                Act::Activate,
                "/dev/vdc",
                &["--lock", "--activate", "/dev/vdc", "1"],
                None,
            ),
            run(
                Act::WipeSignatures,
                &part.path,
                &["--all", &part.path],
                None,
            ),
            run(Act::Make, &part.path, &["-F", &part.path], None),
        ];
        let acts: Vec<Act> = aimed.iter().map(Planned::act).collect();
        for act in [
            Act::ShrinkFilesystem,
            Act::ShrinkEntry,
            Act::Delete,
            Act::Wipe,
            Act::Fresh,
            Act::Append,
            Act::Grow,
            Act::Retype,
            Act::Rename,
            Act::Activate,
            Act::WipeSignatures,
            Act::Make,
        ] {
            assert!(acts.contains(&act), "no {act:?} aimed at the home's disk");
        }
        for (when, kernel) in [
            ("before phase B", kernel_of(&three.layout, None)),
            ("after it", kernel_of(&three.layout, Some(&paper))),
        ] {
            for planned in &aimed {
                let stop = fence
                    .check_planned(planned, &kernel)
                    .expect_err(&format!("{} let through {when}", planned.shown()));
                assert_eq!(stop.failure, crate::model::protocol::Failure::Internal);
                assert!(
                    stop.detail.contains("which the layout does not write to"),
                    "{} {when}: {}",
                    planned.shown(),
                    stop.detail
                );
            }
        }
    }
}

/// **Layouts made up from seeds** — 1,500 on each firmware, at 512 and
/// 4096 bytes a sector: one disk, two in either order, and now and then a
/// third (Q6) that keeps the home, or makes one, listed anywhere among
/// them — each the rule set takes held to everything (`holds_everything`):
/// the order disk by disk, every script's sectors, the consent written
/// apart, the rule set's, and the fence; every kind of command among them.
#[test]
fn made_up_layouts_destroy_only_what_their_consent_names() {
    let software = Software {
        snapshots: false,
        ..Software::default()
    };
    for sector in [512, 4096] {
        let mut judged = 0;
        let mut several = 0;
        let mut three = 0;
        let mut only_mounted_from = 0;
        let mut acts = std::collections::BTreeSet::new();
        for firmware in [Firmware::Uefi, Firmware::Bios] {
            for seed in 0..1500 {
                let layout = seeded(seed, sector, firmware);
                if layout::shape(&layout, firmware, Bootloader::Automatic, &software).is_err() {
                    continue;
                }
                let name = format!("seed {seed} at {sector} on {firmware:?}");
                acts.extend(holds_everything(&name, firmware, &layout));
                judged += 1;
                several += usize::from(layout.len() > 1);
                three += usize::from(layout.len() > 2);
                only_mounted_from += usize::from(
                    layout::keeps_home(&layout) && layout.iter().any(|one| !writes_to(one)),
                );
            }
        }
        assert!(
            judged >= 1800,
            "only {judged} made-up layouts passed the rule set at {sector}"
        );
        assert!(several >= 400, "only {several} of two disks at {sector}");
        assert!(three >= 120, "only {three} of three disks at {sector}");
        assert!(
            only_mounted_from >= 40,
            "only {only_mounted_from} with a home kept on a disk only mounted from at {sector}"
        );
        for act in [
            Act::ShrinkFilesystem,
            Act::ShrinkEntry,
            Act::Delete,
            Act::Wipe,
            Act::Fresh,
            Act::Append,
            Act::Grow,
            Act::Retype,
            Act::Rename,
            Act::Activate,
            Act::WipeSignatures,
            Act::Make,
        ] {
            assert!(
                acts.contains(&act),
                "no made-up layout at {sector} needs {act:?}"
            );
        }
    }
}

/// Labels to give a partition: every printable ASCII letter alone, first,
/// inside and last; device paths and what only looks like one; letters
/// past ASCII; and labels as long as each filesystem holds and one longer.
fn labels() -> Vec<String> {
    let mut labels: Vec<String> = (' '..='~')
        .flat_map(|letter| {
            [
                letter.to_string(),
                format!("{letter}ab"),
                format!("a{letter}b"),
                format!("ab{letter}"),
            ]
        })
        .collect();
    labels.extend(
        [
            "a/devops",
            "x/dev",
            "/dev",
            "/dev/",
            "/dev/vdz",
            "dev/vdx",
            "--opt=/dev/vdy",
            "a b",
            "Zażółć gęś",
            "日本語",
            "LINEXIN",
        ]
        .map(String::from),
    );
    for most in [11, 12, 16, 255] {
        labels.push("L".repeat(most));
        labels.push("L".repeat(most + 1));
    }
    labels
}

/// Every label `one` gives, with the filesystem it is given with: each
/// formatted partition's, then each made one's.
fn labels_of(one: &mut DiskLayout) -> Vec<(Fs, &mut String)> {
    let formatted = one
        .parts
        .iter_mut()
        .filter_map(|part| match &mut part.fate {
            Fate::Format { fs, label, .. } => Some((*fs, label)),
            _ => None,
        });
    let made = one.create.iter_mut().map(|made| (made.fs, &mut made.label));
    formatted.chain(made).collect()
}

/// A label goes onto the command line of the tool that makes its
/// filesystem, where the fence looks at every argument naming a device: so
/// every label the rule set takes must meet the fence, and the oracle, as
/// the label it replaced did. Each fixture's every label is given each of
/// [`labels`] in turn, and the paths of its own disks and partitions.
#[test]
fn every_label_the_rules_take_meets_the_fence() {
    let mut taken = 0;
    for case in cases(512) {
        let mut tried = labels();
        for one in &case.layout {
            tried.push(one.disk.path.clone());
            tried.extend(one.disk.partitions.iter().map(|part| part.path.clone()));
        }
        for disk in 0..case.layout.len() {
            let given = labels_of(&mut case.layout.clone()[disk]).len();
            for at in 0..given {
                for label in &tried {
                    let mut layout = case.layout.clone();
                    let (fs, slot) = labels_of(&mut layout[disk]).swap_remove(at);
                    label.clone_into(slot);
                    let judged = layout::shape(
                        &layout,
                        case.firmware,
                        Bootloader::Automatic,
                        &Software::default(),
                    );
                    let name = format!("{}: {fs:?} labelled {label:?}", case.name);
                    assert_eq!(judged.is_ok(), layout::label_fits(fs, label), "{name}");
                    match judged {
                        Ok(()) => {
                            holds_everything(&name, case.firmware, &layout);
                            taken += 1;
                        }
                        Err(refusal) => assert!(
                            matches!(refusal, layout::Refusal::Label { .. }),
                            "{name}: {refusal:?}"
                        ),
                    }
                }
            }
        }
    }
    assert!(taken >= 8000, "only {taken} labels taken");
}

// -------------------------------------------------------------------------
// On image files.

/// Writes `disk`'s table into the image, each GPT partition named so that a
/// name kept can be told and each msdos one marked active as the fixture
/// marks it (`Partition::bootable`), and reads it back, checking it is the
/// fixture.
fn lay_down(image: &Image, disk: &Disk) -> OnDisk {
    let sector = image.sector;
    let gpt = disk.table == Some(Table::Gpt);
    let label = if gpt { "gpt" } else { "dos" };
    let mut text = format!("label: {label}\nunit: sectors\nsector-size: {sector}\n");
    for part in &disk.partitions {
        let span = fixtures::extent(disk, part, sector);
        let part_type = part.part_type.trim_start_matches("0x");
        text.push_str(&format!(
            "start={}, size={}, type={part_type}",
            span.start,
            span.len()
        ));
        if gpt {
            text.push_str(&format!(", name=\"Old {}\"", part.number));
        }
        if part.bootable {
            text.push_str(", bootable");
        }
        text.push('\n');
    }
    image.sfdisk(&["--lock"], Some(text));
    let before = image.read();
    let places: Vec<_> = before
        .partitions
        .iter()
        .map(|e| (e.number, e.start, e.size))
        .collect();
    let fixture: Vec<_> = fixtures::on_disk(disk)
        .partitions
        .iter()
        .map(|e| (e.number, e.start, e.size, e.bootable))
        .collect();
    let places: Vec<_> = places
        .into_iter()
        .zip(&before.partitions)
        .map(|((number, start, size), e)| (number, start, size, e.bootable))
        .collect();
    assert_eq!(places, fixture, "the image holds the fixture");
    before
}

/// A partition type, compared as sfdisk and lsblk both write them.
fn same_type(one: &str, other: &str) -> bool {
    let bare = |text: &str| {
        let text = text.to_ascii_lowercase();
        let text = text.trim_start_matches("0x").to_string();
        text.trim_start_matches('0').to_string()
    };
    bare(one) == bare(other)
}

/// One entry the table must hold once the layout is carried out. A kept or
/// formatted partition keeps its number and its UUID; one made is numbered
/// by libfdisk.
#[derive(Debug)]
struct Want {
    number: Option<u32>,
    start: u64,
    size: u64,
    part_type: String,
    name: String,
    uuid: Option<String>,
    bootable: bool,
}

/// The table the layout leaves, from the table before it and — for which
/// number libfdisk gave a partition made — the table after.
fn wanted(ctx: Context, one: &DiskLayout, before: Option<&OnDisk>, after: &OnDisk) -> Vec<Want> {
    let table = layout::table_after(one).unwrap();
    let sector = u64::from(one.disk.logical_sector);
    let gpt = table == Table::Gpt;
    let mut want = Vec::new();
    if one.table.is_none() {
        for entry in &before.expect("a kept table").partitions {
            let part = one
                .parts
                .iter()
                .find(|part| part.start == entry.start * sector)
                .expect("reviewed");
            let keep = |size: u64, part_type: &str, name: &str| Want {
                number: Some(entry.number),
                start: entry.start,
                size,
                part_type: part_type.to_string(),
                name: name.to_string(),
                uuid: Some(entry.uuid.clone()),
                bootable: entry.bootable,
            };
            match &part.fate {
                Fate::Delete => {}
                Fate::Keep { .. } => want.push(keep(entry.size, &entry.part_type, &entry.name)),
                Fate::Shrink { to, .. } => {
                    want.push(keep(to / sector, &entry.part_type, &entry.name))
                }
                Fate::Format {
                    fs, size, mount, ..
                } => {
                    let at = mount.as_ref();
                    let name = if gpt { ctx.name(*fs, at) } else { &entry.name };
                    want.push(keep(size / sector, ctx.part_type(table, *fs, at), name));
                }
            }
        }
    }
    for made in &one.create {
        let at = made.mount.as_ref();
        want.push(Want {
            number: None,
            start: made.start / sector,
            size: made.size / sector,
            part_type: ctx.part_type(table, made.fs, at).to_string(),
            name: if gpt { ctx.name(made.fs, at) } else { "" }.to_string(),
            uuid: None,
            bootable: false,
        });
    }
    // The boot flag, by the rule written out again: on msdos, where the disk
    // holds `/` and nothing that stays is marked, on a `/boot` there, else
    // on `/` — and on a kept table only where that is a primary partition.
    let used_at = |mount: &MountPoint| {
        let kept = one
            .parts
            .iter()
            .filter(|part| part.fate.mount() == Some(mount))
            .map(|part| part.start);
        let made = one
            .create
            .iter()
            .filter(|made| made.mount.as_ref() == Some(mount))
            .map(|made| made.start);
        kept.chain(made).next()
    };
    if table == Table::Msdos && !want.iter().any(|w| w.bootable) {
        if let Some(root) = used_at(&MountPoint::Root) {
            let candidates = used_at(&MountPoint::Boot).into_iter().chain([root]);
            for start in candidates {
                let number = after
                    .partitions
                    .iter()
                    .find(|entry| entry.start * sector == start)
                    .map(|entry| entry.number)
                    .unwrap();
                if one.table.is_some() || number <= 4 {
                    let marked = want.iter_mut().find(|w| w.start * sector == start).unwrap();
                    marked.bootable = true;
                    break;
                }
            }
        }
    }
    want.sort_by_key(|want| want.start);
    want
}

fn compare(case: &str, table: Table, sector: u64, after: &OnDisk, want: &[Want]) {
    assert_eq!(after.table, table, "{case}: the table");
    assert_eq!(after.sector, Some(sector), "{case}: the sector size");
    let mut got: Vec<&Entry> = after.partitions.iter().collect();
    got.sort_by_key(|entry| entry.start);
    assert_eq!(got.len(), want.len(), "{case}: {got:#?} where {want:#?}");
    for (got, want) in got.iter().zip(want) {
        let at = got.start;
        assert_eq!(
            (got.start, got.size),
            (want.start, want.size),
            "{case}: a place"
        );
        assert!(
            same_type(&got.part_type, &want.part_type),
            "{case}: type {} at {at} where {} was meant",
            got.part_type,
            want.part_type
        );
        assert_eq!(got.name, want.name, "{case}: the name at {at}");
        if let Some(uuid) = &want.uuid {
            assert!(
                got.uuid.eq_ignore_ascii_case(uuid),
                "{case}: the UUID at {at}"
            );
        }
        if let Some(number) = want.number {
            assert_eq!(got.number, number, "{case}: renumbered at {at}");
        }
        assert_eq!(got.bootable, want.bootable, "{case}: the boot flag at {at}");
    }
}

/// Whether the image tests can run here: sfdisk and wipefs on this
/// machine, and room for images in this tree.
fn images_can_be_made() -> bool {
    for tool in TABLE_TOOLS {
        if let Err(stop) = command_path(tool) {
            eprintln!("skipped: {}", stop.detail);
            return false;
        }
    }
    if let Err(error) = std::fs::create_dir_all(IMAGES) {
        eprintln!("skipped: cannot make {IMAGES}: {error}");
        return false;
    }
    true
}

/// Phase B's table commands for `one`, a disk of `case` the layout writes
/// to, carried out by the real sfdisk and wipefs on an image of it —
/// exactly as the worker gives them to the disk, the image in the disk's
/// place — and the table read back held to the layout. (A filesystem
/// shrunk is Q8's and Q9's, and nothing here holds a filesystem.)
fn on_an_image(case: &Case, one: &DiskLayout, sector: u32) {
    let ctx = Context::new(&case.layout, case.firmware);
    let name = match case.layout.len() {
        1 => format!("manual-{}-{sector}", case.name),
        _ => format!(
            "manual-{}-{}-{sector}",
            case.name,
            one.disk.path.trim_start_matches("/dev/")
        ),
    };
    let image = Image::create(&name, one.disk.size, u64::from(sector), TABLE_TOOLS);
    let before = one.disk.table.map(|_| lay_down(&image, &one.disk));
    let planned = table_commands(ctx, one, before.as_ref(), &image.text)
        .unwrap_or_else(|why| panic!("{}: {why}", case.name));
    for planned in &planned {
        if let Some(command) = planned.command() {
            image.run(command);
        }
    }
    let mut after = image.read();
    let flag = activation(one, &after, &image.text);
    // sfdisk's own table, read back once steps 1 to 7 are done, sets the
    // flag exactly where the rule set judged the plan sets it.
    assert_eq!(
        flag.is_some(),
        layout::activates(one),
        "{} {}: the boot flag",
        case.name,
        one.disk.path
    );
    if let Some(flag) = flag {
        image.run(flag.command().unwrap());
        after = image.read();
    }
    let table = layout::table_after(one).unwrap();
    let want = wanted(ctx, one, before.as_ref(), &after);
    compare(
        &format!("{} {} {sector}B", case.name, one.disk.path),
        table,
        u64::from(sector),
        &after,
        &want,
    );
}

/// Every disk a fixture layout writes to, each on an image of its own: the
/// twelve single-disk fixtures' disks, and each disk the two- and three-disk
/// layouts write (the one that keeps the person's home is written to by
/// nothing, and so has no image).
fn carry_out_on_images(sector: u32) {
    if !images_can_be_made() {
        return;
    }
    let cases = cases(sector);
    let disks: Vec<(&Case, &DiskLayout)> = cases
        .iter()
        .flat_map(|case| {
            case.layout
                .iter()
                .filter(|one| writes_to(one))
                .map(move |one| (case, one))
        })
        .collect();
    assert_eq!(disks.len(), 16);
    std::thread::scope(|scope| {
        for (case, one) in disks {
            scope.spawn(move || on_an_image(case, one, sector));
        }
    });
}

#[test]
fn layouts_are_carried_out_by_sfdisk_on_512_byte_sector_images() {
    carry_out_on_images(512);
}

#[test]
fn layouts_are_carried_out_by_sfdisk_on_4096_byte_sector_images() {
    carry_out_on_images(4096);
}

/// **`-N` gives one entry a new size and nothing else** (Q4; on a logical
/// partition too, Q7): Windows' entry shrunk and a Linux one grown on gpt,
/// Windows' grown on msdos, and — the last logical partition deleted first,
/// as phase B deletes before it grows — the swap partition, a logical one,
/// grown over where that one stood to its extended partition's end. Each
/// keeps its number, start, type, name, UUID and boot flag; every other
/// entry that stays is as it was, the extended partition's `0f` frame
/// among them.
#[test]
fn an_entry_resized_by_number_keeps_its_type_name_and_uuid() {
    if !images_can_be_made() {
        return;
    }
    for sector in [512, 4096] {
        for name in ["gpt-windows", "gpt-plain", "msdos", "msdos-logical"] {
            let case = case(sector, name);
            let one = &case.layout[0];
            let ctx = Context::new(&case.layout, case.firmware);
            let image = Image::create(
                &format!("manual-resized-{name}-{sector}"),
                one.disk.size,
                u64::from(sector),
                TABLE_TOOLS,
            );
            let before = lay_down(&image, &one.disk);
            let planned = table_commands(ctx, one, Some(&before), &image.text).unwrap();
            for deleting in planned.iter().filter(|p| p.act() == Act::Delete) {
                image.run(deleting.command().unwrap());
            }
            let resizing: Vec<&Planned> = planned
                .iter()
                .filter(|p| matches!(p.act(), Act::ShrinkEntry | Act::Grow))
                .collect();
            assert_eq!(resizing.len(), 1, "{name}: {resizing:?}");
            image.run(resizing[0].command().unwrap());
            let after = image.read();
            let staying: Vec<&Entry> = before
                .partitions
                .iter()
                .filter(|was| {
                    let part = one.parts.iter().find(|p| p.number == was.number).unwrap();
                    part.fate != Fate::Delete
                })
                .collect();
            assert_eq!(after.partitions.len(), staying.len(), "{name}");
            for (was, now) in staying.into_iter().zip(&after.partitions) {
                let part = one.parts.iter().find(|p| p.number == was.number).unwrap();
                let case = format!("{name} {sector}B partition {}", was.number);
                assert_eq!(now.number, was.number, "{case}");
                assert_eq!(now.start, was.start, "{case}: the start");
                let size = match part.fate {
                    Fate::Shrink { to, .. } => to / u64::from(sector),
                    Fate::Format { size, .. } => size / u64::from(sector),
                    _ => was.size,
                };
                assert_eq!(now.size, size, "{case}: the size");
                assert_eq!(now.part_type, was.part_type, "{case}: the type");
                assert_eq!(now.name, was.name, "{case}: the name");
                assert_eq!(now.uuid, was.uuid, "{case}: the UUID");
                assert_eq!(now.bootable, was.bootable, "{case}: the boot flag");
            }
        }
    }
}

// -------------------------------------------------------------------------
// The Mounted the rest of the worker reads.

/// What `blkid -p` reads of each partition `layout` uses once everything is
/// made, at its path now.
fn facts(layout: &[DiskLayout], paths: &Paths) -> Facts {
    let found = used(layout)
        .iter()
        .enumerate()
        .map(|(index, one)| Fact {
            which: one.which.clone(),
            device: paths.of(&one.which).unwrap().to_string(),
            uuid: format!("{index:08x}-0000-4000-8000-000000000000"),
            fstype: match (one.made, one.was) {
                (Some(fs), _) => fs.lsblk().to_string(),
                (None, Some(was)) => was.fs_type.clone(),
                (None, None) => unreachable!(),
            },
        })
        .collect();
    Facts { found }
}

#[test]
fn the_mounted_system_is_assembled_from_the_layout_and_what_was_read() {
    let assemble = |name: &str, layout: &[DiskLayout], firmware: Firmware, swap_mib: u32| {
        let paper = on_paper(firmware, layout);
        mounted(
            layout,
            firmware,
            &facts(layout, &paper.paths),
            &paper.paths,
            "/run/lxb-installer/stage-test/root".into(),
            swap_mib,
            "fstab".into(),
        )
        .unwrap_or_else(|why| panic!("{name}: {why}"))
    };
    // Windows' EFI partition kept at /efi, the kernels on a Linux boot
    // partition made at /boot.
    let windows = case(512, "gpt-windows");
    let m = assemble("gpt-windows", &windows.layout, Firmware::Uefi, 0);
    assert_eq!((m.disk.as_str(), m.table), ("/dev/vdx", Table::Gpt));
    let esp = m.esp.as_ref().unwrap();
    assert_eq!(
        (esp.device.as_str(), esp.mount, esp.shared),
        ("/dev/vdx1", EspMount::Efi, true)
    );
    assert_eq!(m.linux_boot.as_ref().unwrap().device, "/dev/vdx5");
    assert_eq!(m.root_device, "/dev/vdx6");
    assert_eq!(m.root_subvolume, None);
    assert_eq!(
        (m.home_device.as_deref(), m.swapfile.as_ref()),
        (None, None)
    );
    assert_eq!(m.fstab, "fstab");
    // GPT auto-discovery left off, and os-prober told what this installation
    // made: the Linux boot partition, / and swap — not Windows, kept at a
    // path of its own, nor its EFI partition beyond what it already knows.
    assert!(m.gpt_auto_off);
    assert_eq!(m.other_devices, ["/dev/vdx5", "/dev/vdx6", "/dev/vdx7"]);

    // A new EFI partition alone at /boot, Btrfs /, a /home of its own.
    let blank = case(512, "empty");
    let m = assemble("empty", &blank.layout, Firmware::Uefi, 0);
    let esp = m.esp.as_ref().unwrap();
    assert_eq!(
        (esp.device.as_str(), esp.mount, esp.shared),
        ("/dev/vdx1", EspMount::Boot, false)
    );
    assert!(m.linux_boot.is_none());
    assert_eq!(m.root_subvolume.as_deref(), Some("@"));
    assert_eq!(m.home_device.as_deref(), Some("/dev/vdx4"));
    assert_eq!(
        m.other_devices,
        ["/dev/vdx1", "/dev/vdx2", "/dev/vdx3", "/dev/vdx4"]
    );
    // A swap file never beside a swap partition.
    let paper = on_paper(Firmware::Uefi, &blank.layout);
    let refused = mounted(
        &blank.layout,
        Firmware::Uefi,
        &facts(&blank.layout, &paper.paths),
        &paper.paths,
        "/x".into(),
        1024,
        String::new(),
    );
    assert!(refused.is_err());
    // With no swap partition, the swap file in Btrfs' @swap.
    let mut no_swap = blank.layout.clone();
    no_swap[0].create[2].mount = None;
    let m = assemble("empty, a swap file", &no_swap, Firmware::Uefi, 1024);
    assert_eq!(
        m.swapfile.as_deref(),
        Some(std::path::Path::new("/swap/swapfile"))
    );

    // Q7's msdos disk: the old root formatted at /, swap formatted, the
    // last logical partition deleted, Data kept at /data — os-prober is
    // told of the two formatted, never of Data nor of what is gone; kept at
    // /home instead, Data is this installation's too.
    let logical = case(512, "msdos-logical");
    let m = assemble("msdos-logical", &logical.layout, Firmware::Bios, 0);
    assert_eq!(m.other_devices, ["/dev/vdx1", "/dev/vdx6"]);
    let mut home = logical.layout.clone();
    home[0] = with(home[0].clone(), 5, Fate::Keep { mount: at("/home") });
    let paper = on_paper(Firmware::Bios, &home);
    assert_eq!(
        own_devices(&home, &paper.paths),
        ["/dev/vdx1", "/dev/vdx5", "/dev/vdx6"]
    );

    // BIOS: no EFI partition; GRUB goes on /'s disk.
    let old = case(512, "msdos");
    let m = assemble("msdos", &old.layout, Firmware::Bios, 0);
    assert_eq!((m.disk.as_str(), m.table), ("/dev/vdx", Table::Msdos));
    assert!(m.esp.is_none() && m.linux_boot.is_none());
    assert_eq!(m.root_device, "/dev/vdx2");
    // A blank gpt disk a BIOS starts: GRUB goes on /'s disk, its core in
    // the bios_grub partition there, which os-prober is told is this
    // installation's as everything it made is.
    let gpt = case(512, "bios-gpt");
    let m = assemble("bios-gpt", &gpt.layout, Firmware::Bios, 0);
    assert_eq!((m.disk.as_str(), m.table), ("/dev/vdx", Table::Gpt));
    assert!(m.esp.is_none() && m.linux_boot.is_none());
    assert_eq!(
        (m.root_device.as_str(), m.home_device.as_deref()),
        ("/dev/vdx2", Some("/dev/vdx3"))
    );
    assert_eq!(m.other_devices, ["/dev/vdx1", "/dev/vdx2", "/dev/vdx3"]);

    // The EFI partition on another disk than /: the firmware's disk and
    // table are the EFI partition's, so systemd-boot is judged by its GPT
    // although / is on msdos.
    let mut root = fresh(fixtures::disk_empty(512), Table::Msdos);
    root.create = vec![made(1, 60000, Fs::Ext4, at("/"), "")];
    let mut starts = fresh(
        renamed(fixtures::disk_empty(512), "/dev/vdy", "254:16"),
        Table::Gpt,
    );
    starts.create = vec![
        made(1, 1024, Fs::Fat32, at("/efi"), ""),
        made(1025, 2048, Fs::Fat32, at("/boot"), ""),
    ];
    let apart = vec![root, starts];
    assert_eq!(
        layout::shape(
            &apart,
            Firmware::Uefi,
            Bootloader::Automatic,
            &Software::default()
        ),
        Ok(())
    );
    let m = assemble("apart", &apart, Firmware::Uefi, 0);
    assert_eq!((m.disk.as_str(), m.table), ("/dev/vdy", Table::Gpt));
    let esp = m.esp.as_ref().unwrap();
    assert_eq!(
        (esp.device.as_str(), esp.mount),
        ("/dev/vdy1", EspMount::Efi)
    );
    assert_eq!(m.linux_boot.as_ref().unwrap().device, "/dev/vdy2");
    assert_eq!(m.root_device, "/dev/vdx1");
}
