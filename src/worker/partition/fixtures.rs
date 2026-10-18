//! Invented disks for the storage tests, written by hand in the shape
//! discovery reports them: positions in bytes, paths as the kernel names
//! them, the same layout in bytes whatever the sector size. Nothing here is
//! read from the machine the tests run on.
use super::plan::{is_extended, plan, Plan, SharedRoom, Span};
use super::table::{Entry, OnDisk};
use crate::model::config::{
    Account, Bootloader, Desktop, InstallConfig, Region, Software, Storage, Target, DEFAULT_SOURCE,
};
use crate::model::disks::{self, Content, Disk, Gap, Partition, Table, Transport, GIB, MIB};
use crate::model::layout::{self, Create, DiskLayout, Fs, MountPoint};
use crate::model::protocol::Request;
use crate::worker::layout::Firmware;

/// Every fixture disk is 64 GiB.
pub const DISK_SIZE: u64 = 64 * GIB;

pub const GPT_ESP: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
pub const GPT_MSR: &str = "e3c9e316-0b5c-4db8-817d-f92df00215ae";
pub const GPT_BASIC_DATA: &str = "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7";
pub const GPT_WINRE: &str = "de94bba4-06d1-4d40-a16a-bfd50179d6ac";
pub const GPT_LINUX: &str = "0fc63daf-8483-4772-8e79-3d69d8477de4";
pub const GPT_BIOS_BOOT: &str = "21686148-6449-6e6f-744e-656564454649";

/// One partition: number, start and size in MiB (or bytes for the tiny
/// extended entry), type, content and filesystem.
pub struct Spec {
    pub number: u32,
    pub start: u64,
    pub size: u64,
    pub part_type: &'static str,
    pub content: Content,
    pub fs_type: &'static str,
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

/// A partition path the way the kernel names it: a `p` before the number
/// when the disk's name ends in a digit.
pub fn part_path(disk: &str, number: u32) -> String {
    if disk.ends_with(|c: char| c.is_ascii_digit()) {
        format!("{disk}p{number}")
    } else {
        format!("{disk}{number}")
    }
}

pub fn build(
    path: &str,
    sector: u32,
    table: Option<Table>,
    specs: Vec<Spec>,
    free: Vec<(u64, u64)>,
) -> Disk {
    Disk {
        path: path.into(),
        diskseq: Some(42),
        model: "Example Disk".into(),
        serial: "TEST-0001".into(),
        size: DISK_SIZE,
        major_minor: "254:0".into(),
        logical_sector: sector,
        transport: Transport::Virtio,
        removable: false,
        rotational: false,
        table,
        content: None,
        read_only: false,
        blocked: None,
        partitions: specs
            .into_iter()
            .map(|s| Partition {
                path: part_path(path, s.number),
                number: s.number,
                start: s.start,
                size: s.size,
                fs_type: s.fs_type.into(),
                label: String::new(),
                part_type: s.part_type.into(),
                uuid: String::new(),
                mountpoints: Vec::new(),
                content: s.content,
                bootable: false,
            })
            .collect(),
        free: free
            .into_iter()
            .map(|(start, size)| Gap {
                start: start * MIB,
                size: size * MIB,
            })
            .collect(),
    }
}

/// A new disk with no table at all.
pub fn disk_empty(sector: u32) -> Disk {
    build("/dev/vdx", sector, None, Vec::new(), Vec::new())
}

/// The usual Windows layout with room left: ESP, MSR, Windows, a gap,
/// recovery at the end (the last MiB holds the backup GPT).
pub fn disk_gpt_windows(sector: u32) -> Disk {
    build(
        "/dev/vdx",
        sector,
        Some(Table::Gpt),
        vec![
            spec(1, 1, 100, GPT_ESP, Content::Efi, "vfat"),
            spec(2, 101, 16, GPT_MSR, Content::MicrosoftReserved, ""),
            spec(3, 117, 30 * 1024, GPT_BASIC_DATA, Content::Windows, "ntfs"),
            spec(4, 64785, 750, GPT_WINRE, Content::WindowsRecovery, "ntfs"),
        ],
        vec![(30837, 64785 - 30837)],
    )
}

/// GPT with no EFI or BIOS boot partition: one Linux data partition and a gap.
pub fn disk_gpt_plain(sector: u32) -> Disk {
    build(
        "/dev/vdx",
        sector,
        Some(Table::Gpt),
        vec![spec(1, 1, 30 * 1024, GPT_LINUX, Content::Linux, "ext4")],
        vec![(30721, 65535 - 30721)],
    )
}

/// GPT made for a BIOS: a BIOS boot partition, Linux, a gap.
pub fn disk_gpt_bios(sector: u32) -> Disk {
    build(
        "/dev/vdx",
        sector,
        Some(Table::Gpt),
        vec![
            spec(1, 1, 1, GPT_BIOS_BOOT, Content::BiosBoot, ""),
            spec(2, 2, 30 * 1024, GPT_LINUX, Content::Linux, "ext4"),
        ],
        vec![(30722, 65535 - 30722)],
    )
}

/// An MBR Windows layout: System Reserved, Windows, a gap to the end.
pub fn disk_msdos(sector: u32) -> Disk {
    build(
        "/dev/vdx",
        sector,
        Some(Table::Msdos),
        vec![
            spec(1, 1, 500, "0x07", Content::Windows, "ntfs"),
            spec(2, 501, 30 * 1024, "0x07", Content::Windows, "ntfs"),
        ],
        vec![(31221, 65536 - 31221)],
    )
}

/// An MBR disk that already has an EFI system partition.
pub fn disk_msdos_esp(sector: u32) -> Disk {
    build(
        "/dev/vdx",
        sector,
        Some(Table::Msdos),
        vec![
            spec(1, 1, 300, "0xef", Content::Efi, "vfat"),
            spec(2, 301, 30 * 1024, "0x07", Content::Windows, "ntfs"),
        ],
        vec![(31021, 65536 - 31021)],
    )
}

/// An MBR disk with all four primary slots taken and a gap between them.
pub fn disk_msdos_full(sector: u32) -> Disk {
    build(
        "/dev/vdx",
        sector,
        Some(Table::Msdos),
        vec![
            spec(1, 1, 500, "0x07", Content::Windows, "ntfs"),
            spec(2, 501, 30 * 1024, "0x07", Content::Windows, "ntfs"),
            spec(3, 31221, 26 * 1024, "0x83", Content::Linux, "ext4"),
            spec(4, 64512, 1024, "0x83", Content::Linux, "ext4"),
        ],
        vec![(57845, 64512 - 57845)],
    )
}

/// An MBR disk with an extended partition holding one logical partition.
/// The kernel shows the extended entry as one tiny partition.
pub fn disk_msdos_extended(sector: u32) -> Disk {
    let mut disk = build(
        "/dev/vdx",
        sector,
        Some(Table::Msdos),
        vec![
            spec(1, 1, 500, "0x07", Content::Windows, "ntfs"),
            spec(2, 501, 1, "0x05", Content::Other, ""),
            spec(5, 502, 30 * 1024, "0x83", Content::Linux, "ext4"),
        ],
        vec![(31222, 65536 - 31222)],
    );
    disk.partitions[1].size = u64::from(sector).max(1024);
    disk
}

/// Every fixture with a name, for the tests that go through them all.
pub fn all(sector: u32) -> Vec<(&'static str, Disk)> {
    vec![
        ("empty", disk_empty(sector)),
        ("gpt-windows", disk_gpt_windows(sector)),
        ("gpt-plain", disk_gpt_plain(sector)),
        ("gpt-bios", disk_gpt_bios(sector)),
        ("msdos", disk_msdos(sector)),
        ("msdos-esp", disk_msdos_esp(sector)),
        ("msdos-full", disk_msdos_full(sector)),
        ("msdos-extended", disk_msdos_extended(sector)),
    ]
}

/// Every place on `disk` a person could pick, with a short name.
pub fn targets(disk: &Disk) -> Vec<(String, Target)> {
    let mut list = vec![("whole".to_string(), Target::WholeDisk)];
    for (index, gap) in disk.free.iter().enumerate() {
        list.push((
            format!("gap{}", index + 1),
            Target::FreeSpace {
                start: gap.start,
                size: gap.size,
            },
        ));
    }
    for part in &disk.partitions {
        list.push((
            format!("p{}", part.number),
            Target::Partition {
                path: part.path.clone(),
            },
        ));
    }
    list
}

/// A partition's sectors as its table has them. The kernel lists an msdos
/// extended partition as one tiny entry, but in the table it holds every
/// logical partition, so its extent runs to the end of the last one.
pub fn extent(disk: &Disk, part: &Partition, sector: u64) -> Span {
    let mut end = (part.start + part.size) / sector;
    if is_extended(&part.part_type) {
        for logical in disk.partitions.iter().filter(|p| p.number > 4) {
            end = end.max((logical.start + logical.size) / sector);
        }
    }
    Span {
        start: part.start / sector,
        end,
    }
}

/// The fixture's table the way `sfdisk --json` would read it from the disk.
pub fn on_disk(disk: &Disk) -> OnDisk {
    let sector = u64::from(disk.logical_sector);
    OnDisk {
        table: disk.table.expect("the fixture has a partition table"),
        sector: Some(sector),
        partitions: disk
            .partitions
            .iter()
            .map(|part| {
                let span = extent(disk, part, sector);
                Entry {
                    number: part.number,
                    start: span.start,
                    size: span.len(),
                    part_type: part.part_type.clone(),
                    bootable: part.bootable,
                    name: String::new(),
                    uuid: String::new(),
                }
            })
            .collect(),
    }
}

/// A request with every choice at its default.
pub fn config(disk: Disk, target: Target) -> InstallConfig {
    InstallConfig {
        disk,
        target,
        storage: Storage::default(),
        bootloader: Bootloader::Automatic,
        desktop: Desktop::default(),
        software: Software::default(),
        region: Region::default(),
        account: Account::default(),
        source: DEFAULT_SOURCE.into(),
    }
}

/// A request carrying a layout made by hand (`Target::Layout`) over the
/// empty fixture disk at `sector` bytes a sector: a new gpt, a 1 GiB FAT32
/// EFI system partition alone at `/boot` and ext4 at `/` on the rest. Its
/// easy fields are its pins (`layout::pins`, the swap file Storage's
/// default) and its consent is what it destroys, as the interface builds
/// one — so it passes every rule a request can be held to, and is refused
/// all the same: no build carries a layout out yet.
pub fn layout_request(sector: u32) -> Request {
    let disk = disk_empty(sector);
    let room = layout::usable(Table::Gpt, disk.size / u64::from(sector), u64::from(sector));
    let first = room.start * u64::from(sector);
    let last = room.end * u64::from(sector);
    let made = |start: u64, size: u64, fs: Fs, mount: MountPoint| Create {
        start,
        size,
        fs,
        label: String::new(),
        mount: Some(mount),
    };
    let laid = vec![DiskLayout {
        disk: disk.clone(),
        table: Some(Table::Gpt),
        parts: Vec::new(),
        create: vec![
            made(first, GIB, Fs::Fat32, MountPoint::Boot),
            made(first + GIB, last - first - GIB, Fs::Ext4, MountPoint::Root),
        ],
    }];
    let mut config = config(disk, Target::Layout { disks: Vec::new() });
    let pins = layout::pins(&laid, config.storage.swap_mib).expect("the layout pins");
    config.disk = pins.disk;
    config.storage = Storage {
        filesystem: pins.filesystem,
        swap_mib: pins.swap_mib,
        home: None,
    };
    config.target = Target::Layout { disks: laid };
    Request {
        consent: disks::consent(&config),
        config,
    }
}

/// The plan for a shared EFI system partition with room for the kernels,
/// or a test failure that says why there is none.
pub fn plan_for(config: &InstallConfig, firmware: Firmware) -> Plan {
    plan_in(config, firmware, SharedRoom::Kernels)
}

/// The plan for what was found inside a shared EFI system partition, or a
/// test failure that says why there is none.
pub fn plan_in(config: &InstallConfig, firmware: Firmware, shared: SharedRoom) -> Plan {
    plan(config, firmware, shared)
        .unwrap_or_else(|stop| panic!("{:?}: {}", stop.failure, stop.detail))
}
