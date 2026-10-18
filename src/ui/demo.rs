//! A made-up machine, for `--demo` and for pictures.
//!
//! Three disks that belong to nobody — an empty NVMe drive, a SATA disk with
//! Windows on it and room beside it, and the USB stick the live system was
//! started from — built as `Disk` values directly, through the same types the
//! storage work's discovery fills in. Nothing of this machine is read for it
//! and nothing is written: the models and serials are invented and say so.
//!
//! A few time zones and keyboard layouts of its own, written here rather than
//! read from this computer's zoneinfo and XKB, so a picture of the time zone
//! or keyboard step is the same on every machine it is taken on.
//!
//! And a rehearsal of an installation: the events a worker would send, on a
//! timer, so the whole flow can be walked from Welcome to Done with no worker
//! started at all — a plan of Partitions' that shrinks a kept partition
//! shrinking it first, as the worker would, and one that shrinks Windows'
//! telling Done that Windows will check its disk. A demo never starts a
//! worker — that is decided by `Machine::Demo`, and this is what it gets
//! instead.

use crate::model::catalogs::{
    self, DesktopOffer, KeyboardLayout, KeyboardVariant, Package, PackageCategory, Zone,
};
use crate::model::config::{Desktop, InstallConfig, Target};
use crate::model::disks::{Blocked, Content, Disk, Gap, Partition, Table, Transport, MIB};
use crate::model::layout::{self, ImageSizes, Measure, Measured, Measures};
use crate::model::protocol::{Event, Step, Warning};

const ESP: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
const MSR: &str = "e3c9e316-0b5c-4db8-817d-f92df00215ae";
const BASIC_DATA: &str = "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7";
const RECOVERY: &str = "de94bba4-06d1-4d40-a16a-bfd50179d6ac";
const BIOS_BOOT: &str = "21686148-6449-6e6f-744e-656564454649";
const LINUX_ROOT: &str = "4f68bce3-e8cd-4db1-96e7-fbcaf984b709";
const LINUX_SWAP: &str = "0657fd6d-a4ab-43c4-84e5-0933c84b4f4f";
const LINUX_HOME: &str = "933ac7e1-2eb4-4f13-b844-0e14e2aef915";
const LINUX: &str = "0fc63daf-8483-4772-8e79-3d69d8477de4";

#[allow(clippy::too_many_arguments)]
fn part(
    disk: &str,
    number: u32,
    start_mib: u64,
    size_mib: u64,
    fs_type: &str,
    label: &str,
    part_type: &str,
    content: Content,
) -> Partition {
    Partition {
        path: format!("{disk}{number}"),
        number,
        start: start_mib * MIB,
        size: size_mib * MIB,
        fs_type: fs_type.into(),
        label: label.into(),
        part_type: part_type.into(),
        uuid: format!("0000DEMO-{number:04}"),
        mountpoints: Vec::new(),
        content,
        bootable: false,
    }
}

/// A made-up archive disk that only the picture of the plan's paged question
/// adds (`confirm-plan-pages`, `confirm-plan-last`): 2 TB with a gpt and
/// forty partitions of 40 GiB, one after another — NTFS, ext4 and exFAT by
/// turns, each labelled "Archive NN" — so a new table over it erases more
/// than any dialog holds on one page.
pub fn archive_disk() -> Disk {
    let size = 2_000_398_934_016u64;
    let kinds = [
        ("ntfs", BASIC_DATA, Content::Windows),
        ("ext4", LINUX, Content::Linux),
        ("exfat", BASIC_DATA, Content::Data),
    ];
    let partitions = (0..40u32)
        .map(|at| {
            let (fs_type, part_type, content) = kinds[at as usize % kinds.len()];
            let label = format!("Archive {:02}", at + 1);
            let start = 1 + u64::from(at) * 40 * 1024;
            part(
                "/dev/sde",
                at + 1,
                start,
                40 * 1024,
                fs_type,
                &label,
                part_type,
                content,
            )
        })
        .collect();
    let used = 1 + 40 * 40 * 1024;
    Disk {
        path: "/dev/sde".into(),
        diskseq: Some(6),
        model: "Example Archive HDD".into(),
        serial: "DEMO-SATA-0006".into(),
        size,
        major_minor: "8:64".into(),
        logical_sector: 512,
        transport: Transport::Sata,
        removable: false,
        rotational: true,
        table: Some(Table::Gpt),
        content: None,
        read_only: false,
        blocked: None,
        partitions,
        free: vec![Gap {
            start: used * MIB,
            size: (size / MIB - used - 1) * MIB,
        }],
    }
}

/// The three disks of the made-up machine, in the order a machine lists them.
pub fn disks() -> Vec<Disk> {
    // An empty 512 GB NVMe drive: no partition table at all, every byte free.
    let nvme_size = 512_110_190_592u64;
    let nvme = Disk {
        path: "/dev/nvme0n1".into(),
        diskseq: Some(1),
        model: "Example NVMe SSD".into(),
        serial: "DEMO-NVME-0001".into(),
        size: nvme_size,
        major_minor: "259:0".into(),
        logical_sector: 512,
        transport: Transport::Nvme,
        removable: false,
        rotational: false,
        table: None,
        content: None,
        read_only: false,
        blocked: None,
        partitions: Vec::new(),
        free: vec![Gap {
            start: MIB,
            size: (nvme_size / MIB - 2) * MIB,
        }],
    };

    // A 1 TB SATA disk the way Windows leaves one after its partition was
    // shrunk: the EFI partition, Microsoft's reserved space, Windows itself,
    // 300 GB of room where it was shrunk, the recovery tools, and a data
    // partition filling the rest.
    let sata_size = 1_000_204_886_016u64;
    let sata_mib = sata_size / MIB;
    let windows = part(
        "/dev/sda",
        3,
        117,
        381_470,
        "ntfs",
        "Windows",
        BASIC_DATA,
        Content::Windows,
    );
    let gap_start = 117 + 381_470;
    let gap_size = 286_102;
    let recovery_start = gap_start + gap_size;
    let data_start = recovery_start + 750;
    let sata = Disk {
        path: "/dev/sda".into(),
        diskseq: Some(2),
        model: "Example SATA HDD".into(),
        serial: "DEMO-SATA-0002".into(),
        size: sata_size,
        major_minor: "8:0".into(),
        logical_sector: 512,
        transport: Transport::Sata,
        removable: false,
        rotational: true,
        table: Some(Table::Gpt),
        content: None,
        read_only: false,
        blocked: None,
        partitions: vec![
            part("/dev/sda", 1, 1, 100, "vfat", "", ESP, Content::Efi),
            part(
                "/dev/sda",
                2,
                101,
                16,
                "",
                "",
                MSR,
                Content::MicrosoftReserved,
            ),
            windows,
            part(
                "/dev/sda",
                4,
                recovery_start,
                750,
                "ntfs",
                "",
                RECOVERY,
                Content::WindowsRecovery,
            ),
            part(
                "/dev/sda",
                5,
                data_start,
                sata_mib - data_start - 1,
                "ntfs",
                "Data",
                BASIC_DATA,
                Content::Windows,
            ),
        ],
        free: vec![Gap {
            start: gap_start * MIB,
            size: gap_size * MIB,
        }],
    };

    // The USB stick the live system was started from: never offered.
    let stick_size = 61_035 * MIB;
    let stick = Disk {
        path: "/dev/sdb".into(),
        diskseq: Some(3),
        model: "Example USB Stick".into(),
        serial: "DEMO-USB-0003".into(),
        size: stick_size,
        major_minor: "8:16".into(),
        logical_sector: 512,
        transport: Transport::Usb,
        removable: true,
        rotational: false,
        table: Some(Table::Msdos),
        content: None,
        read_only: false,
        blocked: Some(Blocked::InstallationMedia),
        partitions: vec![
            part(
                "/dev/sdb",
                1,
                0,
                3_100,
                "iso9660",
                "LINEXIN_DEMO",
                "0x0",
                Content::Other,
            ),
            part(
                "/dev/sdb",
                2,
                3_100,
                15,
                "vfat",
                "LINEXIN_DEMO",
                "0xef",
                Content::Efi,
            ),
        ],
        free: Vec::new(),
    };

    vec![nvme, sata, stick]
}

/// The two kinds of disk the made-up machine does not have, for the picture
/// of every kind of disk at once (`target-kinds`): an empty SATA SSD and an
/// empty memory card.
pub fn other_kinds() -> Vec<Disk> {
    let blank = |path: &str, seq: u64, model: &str, size: u64, transport, removable| {
        let size = size * MIB;
        Disk {
            path: path.into(),
            diskseq: Some(seq),
            model: model.into(),
            serial: format!("DEMO-{seq:04}"),
            size,
            major_minor: format!("8:{}", seq * 16),
            logical_sector: 512,
            transport,
            removable,
            rotational: false,
            table: None,
            content: None,
            read_only: false,
            blocked: None,
            partitions: Vec::new(),
            free: vec![Gap {
                start: MIB,
                size: (size / MIB - 2) * MIB,
            }],
        }
    };
    vec![
        blank(
            "/dev/sdc",
            4,
            "Example SATA SSD",
            244_198,
            Transport::Sata,
            false,
        ),
        blank(
            "/dev/mmcblk0",
            5,
            "Example SD Card",
            61_035,
            Transport::Mmc,
            true,
        ),
    ]
}

/// The two disks the made-up machine shows only on Partitions, for the
/// pictures of every kind of map at once (`partitions-*`): an old hard disk
/// with an msdos table — Windows, an extended partition holding Linux, its
/// swap and a shared FAT32 partition, and free space after it — and a SATA
/// SSD a Linux started from BIOS was installed on with a gpt table: a 1 MiB
/// `bios_grub` partition, its root, its swap, its `/home` and an encrypted
/// (LUKS) partition, which the installer cannot mount.
///
/// Kept off the machine Where to install lists, whose pictures, menus and
/// tests are of the three disks it has always had: two more disks there
/// would put two more rows in its disks' column and their partitions in its
/// "Your files" menu for no picture's sake.
pub fn partition_disks() -> Vec<Disk> {
    let old_mib = 152_627u64;
    let extended = part(
        "/dev/sdc",
        2,
        40_961,
        61_441,
        "",
        "",
        "0x0f",
        Content::Other,
    );
    let old = Disk {
        path: "/dev/sdc".into(),
        diskseq: Some(4),
        model: "Example Old HDD".into(),
        serial: "DEMO-SATA-0004".into(),
        size: old_mib * MIB,
        major_minor: "8:32".into(),
        logical_sector: 512,
        transport: Transport::Sata,
        removable: false,
        rotational: true,
        table: Some(Table::Msdos),
        content: None,
        read_only: false,
        blocked: None,
        partitions: vec![
            part(
                "/dev/sdc",
                1,
                1,
                40_960,
                "ntfs",
                "System",
                "0x07",
                Content::Windows,
            ),
            extended,
            part(
                "/dev/sdc",
                5,
                40_962,
                30_720,
                "ext4",
                "debian",
                "0x83",
                Content::Linux,
            ),
            part(
                "/dev/sdc",
                6,
                71_683,
                2_048,
                "swap",
                "",
                "0x82",
                Content::Swap,
            ),
            part(
                "/dev/sdc",
                7,
                73_732,
                28_670,
                "vfat",
                "SHARED",
                "0x0c",
                Content::Data,
            ),
        ],
        free: vec![Gap {
            start: 102_402 * MIB,
            size: (old_mib - 102_402) * MIB,
        }],
    };

    let ssd_mib = 244_198u64;
    let home_start = 69_634;
    let vault_start = 219_634;
    let ssd = Disk {
        path: "/dev/sdd".into(),
        diskseq: Some(5),
        model: "Example SATA SSD".into(),
        serial: "DEMO-SATA-0005".into(),
        size: ssd_mib * MIB,
        major_minor: "8:48".into(),
        logical_sector: 512,
        transport: Transport::Sata,
        removable: false,
        rotational: false,
        table: Some(Table::Gpt),
        content: None,
        read_only: false,
        blocked: None,
        partitions: vec![
            part("/dev/sdd", 1, 1, 1, "", "", BIOS_BOOT, Content::BiosBoot),
            part(
                "/dev/sdd",
                2,
                2,
                61_440,
                "ext4",
                "root",
                LINUX_ROOT,
                Content::Linux,
            ),
            part(
                "/dev/sdd",
                3,
                61_442,
                8_192,
                "swap",
                "",
                LINUX_SWAP,
                Content::Swap,
            ),
            part(
                "/dev/sdd",
                4,
                home_start,
                vault_start - home_start,
                "ext4",
                "home",
                LINUX_HOME,
                Content::Linux,
            ),
            part(
                "/dev/sdd",
                5,
                vault_start,
                ssd_mib - vault_start - 1,
                "crypto_LUKS",
                "vault",
                LINUX,
                Content::Other,
            ),
        ],
        free: Vec::new(),
    };
    vec![old, ssd]
}

/// Which of its readings the made-up machine is pictured with
/// ([`measured_as`]): two machines, so that both a shrink and a hibernated
/// Windows can be shown — the rule set holds every NTFS back while any
/// Windows of the measurement is hibernated (`Measures::shrink_of`), so on
/// one machine they could not both be.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reading {
    /// Every Windows on it shut down fully: what `--demo` reads, and every
    /// picture but the hibernated ones.
    #[default]
    ShutDown,
    /// The old disk's Windows hibernated, for the pictures of one
    /// (`partitions-hibernated`, `partitions-hibernated-other`).
    Hibernated,
}

/// What reading each of the made-up machine's partitions finds
/// ([`Reading::ShutDown`]; [`measured_as`]).
pub fn measured(part: &Partition) -> Option<Measure> {
    measured_as(part, Reading::ShutDown)
}

/// What reading each of the made-up machine's partitions finds, as the
/// measure entry would read it on a live system (`--measure`): how much its
/// filesystem uses — what Partitions' details say as Used — and what its
/// size leaves free, by the partition's path, where that is known; the Linux
/// SSD's home ext4 with the least `resize2fs -P` names for it, above what it
/// uses and a margin, which Resize then starts from (`partitions-measured`).
/// A partition with no filesystem, and swap, have nothing read.
///
/// Windows' NTFS on the SATA disk, and the old disk's, are read as the
/// measurement reads one whose own `hiberfil.sys` says Windows is not
/// hibernated (the measure entry's read-only sign, `ntfscat …
/// hiberfil.sys`), so the demo can show a shrink; the SATA disk's Data
/// partition, a Windows' data volume, holds no `hiberfil.sys` of its own —
/// what it uses is known, but it is never offered to be shrunk. Pictured
/// [`Reading::Hibernated`], the old disk's Windows is hibernated
/// (`partitions-hibernated`), and with it every Windows partition is held
/// back (`partitions-hibernated-other`).
pub fn measured_as(part: &Partition, reading: Reading) -> Option<Measure> {
    let (used, least) = match part.path.as_str() {
        "/dev/sdc1" if reading == Reading::Hibernated => return Some(Measure::Hibernated),
        "/dev/sdc1" => (27_310, None),
        "/dev/sda1" => (31, None),
        "/dev/sda3" => (219_750, None),
        "/dev/sda4" => (522, None),
        "/dev/sda5" => {
            let used = 166_195 * MIB;
            return Some(Measure::NoHiberfile {
                used,
                free: part.size.saturating_sub(used),
            });
        }
        "/dev/sdb1" => (3_100, None),
        "/dev/sdb2" => (12, None),
        "/dev/sdc5" => (9_933, None),
        "/dev/sdc7" => (3_174, None),
        "/dev/sdd2" => (21_811, None),
        "/dev/sdd4" => (99_123, Some(112_640)),
        _ => return None,
    };
    let used = used * MIB;
    Some(Measure::Read {
        used,
        free: part.size.saturating_sub(used),
        least: least.map(|mib: u64| mib * MIB),
    })
}

/// Every partition of `disks` [`measured`] knows ([`Reading::ShutDown`];
/// [`measures_as`]).
pub fn measures(disks: &[Disk]) -> Measures {
    measures_as(disks, Reading::ShutDown)
}

/// Every partition of `disks` [`measured_as`] knows, as the measure entry
/// answers: named by the partition as `disks` hold it — its path,
/// filesystem, UUID and size — so a disk changed since is not this one.
pub fn measures_as(disks: &[Disk], reading: Reading) -> Measures {
    let parts = disks
        .iter()
        .flat_map(|disk| &disk.partitions)
        .filter_map(|part| {
            Some(Measured {
                path: part.path.clone(),
                fs: part.fs_type.clone(),
                uuid: part.uuid.clone(),
                size: part.size,
                measure: measured_as(part, reading)?,
            })
        })
        .collect();
    Measures { parts }
}

/// How much the made-up machine's image puts in each folder it fills, as
/// `du` of the live image's own would say (made up, in the proportions of
/// the 3.0 image: Flatpak's runtimes under `/var`).
pub fn image_sizes() -> ImageSizes {
    let paths = [
        ("/var", 3_276 * MIB),
        ("/opt", 214 * MIB),
        ("/srv", 0),
        ("/root", MIB),
        ("/tmp", 0),
    ];
    ImageSizes {
        paths: paths
            .into_iter()
            .map(|(path, size)| (path.to_string(), size))
            .collect(),
    }
}

/// The made-up machine's time zones: a handful of places in each part of the
/// world, each with its country as `iso3166.tab` names it, split and sorted
/// the way `catalogs::zones` splits and sorts the live system's.
pub fn zones() -> Vec<Zone> {
    let places = [
        ("Africa/Cairo", "EG", "Egypt"),
        ("Africa/Johannesburg", "ZA", "South Africa"),
        ("Africa/Lagos", "NG", "Nigeria"),
        ("Africa/Nairobi", "KE", "Kenya"),
        ("America/Argentina/Buenos_Aires", "AR", "Argentina"),
        ("America/Chicago", "US", "United States"),
        ("America/Denver", "US", "United States"),
        ("America/Los_Angeles", "US", "United States"),
        ("America/Mexico_City", "MX", "Mexico"),
        ("America/New_York", "US", "United States"),
        ("America/Sao_Paulo", "BR", "Brazil"),
        ("America/Toronto", "CA", "Canada"),
        ("Asia/Dubai", "AE", "United Arab Emirates"),
        ("Asia/Kolkata", "IN", "India"),
        ("Asia/Seoul", "KR", "Korea (South)"),
        ("Asia/Shanghai", "CN", "China"),
        ("Asia/Singapore", "SG", "Singapore"),
        ("Asia/Tokyo", "JP", "Japan"),
        ("Atlantic/Reykjavik", "IS", "Iceland"),
        ("Australia/Perth", "AU", "Australia"),
        ("Australia/Sydney", "AU", "Australia"),
        ("Europe/Berlin", "DE", "Germany"),
        ("Europe/Lisbon", "PT", "Portugal"),
        ("Europe/London", "GB", "Britain (UK)"),
        ("Europe/Madrid", "ES", "Spain"),
        ("Europe/Moscow", "RU", "Russia"),
        ("Europe/Paris", "FR", "France"),
        ("Europe/Rome", "IT", "Italy"),
        ("Europe/Vienna", "AT", "Austria"),
        ("Europe/Warsaw", "PL", "Poland"),
        ("Pacific/Auckland", "NZ", "New Zealand"),
        ("Pacific/Honolulu", "US", "United States"),
        ("UTC", "", ""),
    ];
    let mut zones: Vec<Zone> = places
        .iter()
        .map(|(name, code, country)| {
            let (region, rest) = name.split_once('/').unwrap_or((name, ""));
            let city = rest.replace('_', " ").replace('/', " / ");
            Zone {
                name: name.to_string(),
                region: region.to_string(),
                shown_city: city.rsplit(" / ").next().unwrap_or_default().to_string(),
                city,
                country: country.to_string(),
                code: code.to_string(),
                shown_country: country.to_string(),
            }
        })
        .collect();
    catalogs::name_zones(&mut zones, &crate::model::places::Names::none());
    zones
}

/// The made-up machine's keyboard layouts, sorted by name as
/// `catalogs::keyboard_layouts` sorts XKB's.
pub fn layouts() -> Vec<KeyboardLayout> {
    let layout = |layout: &str, description: &str, variants: &[(&str, &str)]| KeyboardLayout {
        layout: layout.into(),
        description: description.into(),
        variants: variants
            .iter()
            .map(|(variant, description)| KeyboardVariant {
                variant: (*variant).into(),
                description: (*description).into(),
            })
            .collect(),
    };
    vec![
        layout(
            "gb",
            "English (UK)",
            &[
                ("extd", "English (UK, extended, with Win keys)"),
                ("dvorak", "English (UK, Dvorak)"),
            ],
        ),
        layout(
            "us",
            "English (US)",
            &[
                ("intl", "English (US, intl., with dead keys)"),
                ("dvorak", "English (Dvorak)"),
                ("colemak", "English (Colemak)"),
            ],
        ),
        layout(
            "fr",
            "French",
            &[
                ("nodeadkeys", "French (no dead keys)"),
                ("bepo", "French (BEPO)"),
            ],
        ),
        layout(
            "de",
            "German",
            &[
                ("nodeadkeys", "German (no dead keys)"),
                ("neo", "German (Neo 2)"),
            ],
        ),
        layout("pl", "Polish", &[("dvorak", "Polish (Dvorak)")]),
        layout(
            "br",
            "Portuguese (Brazil)",
            &[("abnt2", "Portuguese (Brazil, ABNT2)")],
        ),
        layout("ru", "Russian", &[("phonetic", "Russian (phonetic)")]),
        layout("es", "Spanish", &[("nodeadkeys", "Spanish (no dead keys)")]),
    ]
}

/// The console keymap the made-up machine pairs with a layout: the layout's
/// own name for the few it has keymaps of, as kbd names them, and `us`
/// otherwise — the same last resort as `catalogs::console_keymap`.
pub fn console_keymap(layout: &str) -> String {
    match layout {
        "de" | "fr" | "es" | "ru" => layout.to_string(),
        "gb" => "uk".to_string(),
        "pl" => "pl2".to_string(),
        "br" => "br-abnt2".to_string(),
        _ => "us".to_string(),
    }
}

/// The made-up machine's image: the Linexin 3.0 live medium the installer
/// ships on, which carries GNOME and LineXinBar with CEDM, so Linexin and
/// LineXinBar install from it with no internet and Kinexin is downloaded —
/// what `catalogs::desktops` reads off that medium.
pub fn offers() -> Vec<DesktopOffer> {
    vec![
        DesktopOffer {
            desktop: Desktop::Linexin,
            in_image: true,
        },
        DesktopOffer {
            desktop: Desktop::Kinexin,
            in_image: false,
        },
        DesktopOffer {
            desktop: Desktop::LineXinBar,
            in_image: true,
        },
    ]
}

/// The made-up machine's software a person may leave out: a few invented
/// packages in each group, never this computer's own list.
pub fn packages() -> Vec<Package> {
    let one = |name: &str, description: &str, category| Package {
        name: name.into(),
        description: description.into(),
        category,
    };
    vec![
        one(
            "example-office",
            "Documents, spreadsheets and slides",
            PackageCategory::Applications,
        ),
        one(
            "example-paint",
            "A program for drawing and photos",
            PackageCategory::Applications,
        ),
        one(
            "example-player",
            "Plays films and music",
            PackageCategory::Multimedia,
        ),
        one(
            "example-codecs",
            "More kinds of video and sound",
            PackageCategory::Multimedia,
        ),
        one(
            "example-share",
            "Shares files over the network",
            PackageCategory::Network,
        ),
        one(
            "example-fonts-cjk",
            "Chinese, Japanese and Korean letters",
            PackageCategory::Fonts,
        ),
        one(
            "example-fonts-emoji",
            "Colour emoji",
            PackageCategory::Fonts,
        ),
        one(
            "python-example",
            "A library other programs use",
            PackageCategory::Libraries,
        ),
    ]
}

/// A rehearsal of an installation: what a worker would say, and when.
#[derive(Debug, Clone)]
pub struct Rehearsal {
    began: f32,
    said: usize,
    /// Whether what is rehearsed shrinks a kept partition (a plan of
    /// Partitions'), which the worker does first (`Step::Resize`), and
    /// whether one of those is Windows' — after which Done says Windows
    /// will check its disk (`Warning::WindowsWillCheck`).
    shrinks: bool,
    windows: bool,
}

/// How long a rehearsal shrinks what a plan keeps smaller, in seconds,
/// before its first easy step after Check.
const RESIZE: f32 = 1.5;

/// The steps a rehearsal takes and how long each lasts, in seconds.
const ACTS: [(Step, f32); 12] = [
    (Step::Check, 1.0),
    (Step::Partition, 1.0),
    (Step::Format, 1.0),
    (Step::Copy, 8.0),
    (Step::Swap, 0.5),
    (Step::System, 1.0),
    (Step::Region, 0.5),
    (Step::Accounts, 0.5),
    (Step::Desktop, 1.0),
    (Step::Initramfs, 2.0),
    (Step::Bootloader, 1.0),
    (Step::Finish, 0.5),
];

/// How many progress reports the copy sends.
const COPY_REPORTS: u64 = 32;

impl Rehearsal {
    /// A rehearsal of `config` starting at `now` on the installer's clock:
    /// for a plan of Partitions' that shrinks a kept partition, its shrink
    /// first (`layout::shrunk`).
    pub fn start(now: f32, config: &InstallConfig) -> Self {
        let shrunk = match &config.target {
            Target::Layout { disks } => layout::shrunk(disks),
            _ => Vec::new(),
        };
        Self {
            began: now,
            said: 0,
            shrinks: !shrunk.is_empty(),
            windows: shrunk.iter().any(|part| part.fs_type == "ntfs"),
        }
    }

    /// The steps this rehearsal takes and how long each lasts: [`ACTS`],
    /// and Resize after Check where it shrinks anything.
    fn acts(&self) -> Vec<(Step, f32)> {
        let mut acts = ACTS.to_vec();
        if self.shrinks {
            acts.insert(1, (Step::Resize, RESIZE));
        }
        acts
    }

    /// Every event of the whole rehearsal, each with the time after the
    /// start it is said at.
    fn script(&self) -> Vec<(f32, Event)> {
        let acts = self.acts();
        let mut script = vec![(
            0.0,
            Event::Plan {
                steps: acts.iter().map(|(step, _)| *step).collect(),
            },
        )];
        let mut at = 0.0;
        for (step, lasts) in acts {
            script.push((at, Event::Begin { step }));
            script.push((
                at,
                Event::Log {
                    line: format!("rehearsal: {step:?} (nothing is written in a demo)"),
                },
            ));
            if step == Step::Copy {
                for report in 1..=COPY_REPORTS {
                    let when = at + lasts * report as f32 / COPY_REPORTS as f32;
                    script.push((
                        when,
                        Event::Progress {
                            step,
                            done: report,
                            total: COPY_REPORTS,
                        },
                    ));
                    // A line now and then, as a copy's log has, so Details
                    // has something to show while the bar fills.
                    if report % 4 == 0 {
                        script.push((
                            when,
                            Event::Log {
                                line: format!(
                                    "rehearsal: copied {report} of {COPY_REPORTS} parts of the image"
                                ),
                            },
                        ));
                    }
                }
            }
            at += lasts;
            // As the worker says it: once Windows' partition is shrunk.
            if step == Step::Resize && self.windows {
                script.push((
                    at,
                    Event::Warning {
                        warning: Warning::WindowsWillCheck,
                    },
                ));
            }
        }
        script.push((at, Event::Finished));
        script
    }

    /// The events whose time has come by `now`, each said once.
    pub fn poll(&mut self, now: f32) -> Vec<Event> {
        let gone = now - self.began;
        let script = self.script();
        let due: Vec<Event> = script
            .into_iter()
            .skip(self.said)
            .take_while(|(at, _)| *at <= gone)
            .map(|(_, event)| event)
            .collect();
        self.said += due.len();
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::MIN_INSTALL;
    use crate::ui::install::Install;

    #[test]
    fn the_made_up_machine_is_the_one_the_design_describes() {
        let disks = disks();
        assert_eq!(disks.len(), 3);
        let [nvme, sata, stick] = [&disks[0], &disks[1], &disks[2]];
        assert!(nvme.partitions.is_empty() && nvme.table.is_none() && nvme.blocked.is_none());
        assert!(nvme.free[0].size >= MIN_INSTALL);
        assert_eq!(sata.esp().map(|esp| esp.size), Some(100 * MIB));
        assert_eq!(sata.free.len(), 1);
        assert!(
            (sata.free[0].size as f64 / 1e9 - 300.0).abs() < 0.5,
            "the gap is 300 GB"
        );
        assert_eq!(stick.blocked, Some(Blocked::InstallationMedia));
    }

    /// **The made-up machine is judged by the live rule**
    /// (`Measures::shrink_of`), on each of its two readings: shut down,
    /// Windows' own partition on the SATA disk and the old disk's may be
    /// shrunk, the Data partition (a Windows' data volume, no hibernation
    /// file of its own) never; with the old disk's Windows hibernated, it
    /// is not shrunk and holds the SATA disk's Windows back too — while the
    /// Linux SSD's home ext4 is judged by its own reading alone, on both.
    #[test]
    fn the_made_up_machine_is_judged_by_the_live_rule_on_each_reading() {
        use crate::model::layout::Unshrinkable;
        let all: Vec<Disk> = disks().into_iter().chain(partition_disks()).collect();
        let part = |path: &str| {
            all.iter()
                .find_map(|disk| disk.partition(path))
                .unwrap_or_else(|| panic!("{path}"))
                .clone()
        };
        let shut_down = measures(&all);
        for path in ["/dev/sda3", "/dev/sdc1", "/dev/sdd4"] {
            assert!(
                matches!(shut_down.shrink_of(&part(path)), Some(Ok(_))),
                "{path}"
            );
        }
        assert_eq!(
            shut_down.shrink_of(&part("/dev/sda5")),
            Some(Err(Unshrinkable::NoHiberfile))
        );
        let hibernated = measures_as(&all, Reading::Hibernated);
        assert_eq!(
            hibernated.shrink_of(&part("/dev/sdc1")),
            Some(Err(Unshrinkable::Itself(Measure::Hibernated)))
        );
        assert_eq!(
            hibernated.shrink_of(&part("/dev/sda3")),
            Some(Err(Unshrinkable::Hibernated {
                other: "/dev/sdc1".into()
            }))
        );
        assert!(matches!(
            hibernated.shrink_of(&part("/dev/sdd4")),
            Some(Ok(_))
        ));
    }

    #[test]
    fn no_part_of_the_windows_disk_overlaps_another_or_runs_off_its_end() {
        let sata = &disks()[1];
        let mut spans: Vec<(u64, u64)> = sata
            .partitions
            .iter()
            .map(|part| (part.start, part.start + part.size))
            .chain(
                sata.free
                    .iter()
                    .map(|gap| (gap.start, gap.start + gap.size)),
            )
            .collect();
        spans.sort();
        for pair in spans.windows(2) {
            assert!(
                pair[0].1 <= pair[1].0,
                "{:?} overlaps {:?}",
                pair[0],
                pair[1]
            );
        }
        assert!(spans.last().unwrap().1 <= sata.size);
    }

    #[test]
    fn the_disks_shown_only_on_partitions_are_what_the_map_must_show() {
        let disks = partition_disks();
        let [old, ssd] = [&disks[0], &disks[1]];
        // An msdos table with an extended partition, every logical partition
        // inside it, and nothing overlapping or running off the disk.
        assert_eq!(old.table, Some(Table::Msdos));
        let extended = old.partitions.iter().find(|p| p.is_extended()).unwrap();
        let logical: Vec<&Partition> = old.partitions.iter().filter(|p| p.number > 4).collect();
        assert!(logical.len() >= 2);
        for part in logical {
            assert!(part.start > extended.start);
            assert!(part.start + part.size <= extended.start + extended.size);
        }
        // A Linux started from BIOS on gpt: a 1 MiB bios_grub, its root, its
        // swap, its /home and an encrypted partition, which nothing mounts.
        assert_eq!(ssd.table, Some(Table::Gpt));
        let kinds: Vec<Content> = ssd.partitions.iter().map(|p| p.content).collect();
        assert_eq!(
            kinds,
            [
                Content::BiosBoot,
                Content::Linux,
                Content::Swap,
                Content::Linux,
                Content::Other
            ]
        );
        assert_eq!(ssd.partitions[4].fs_type, "crypto_LUKS");
        assert_eq!(ssd.partitions[0].size, MIB);
        for disk in [old, ssd] {
            let mut spans: Vec<(u64, u64)> = disk
                .partitions
                .iter()
                .filter(|p| !p.is_extended())
                .map(|p| (p.start, p.start + p.size))
                .chain(disk.free.iter().map(|g| (g.start, g.start + g.size)))
                .collect();
            spans.sort();
            for pair in spans.windows(2) {
                assert!(pair[0].1 <= pair[1].0, "{}: {pair:?}", disk.path);
            }
            assert!(spans.last().unwrap().1 <= disk.size);
            // What a partition uses is never more than it holds, and the
            // least its tool names is between the two.
            for part in &disk.partitions {
                if let Some(Measure::Read { used, free, least }) = measured(part) {
                    assert!(used <= part.size, "{}", part.path);
                    assert_eq!(used + free, part.size, "{}", part.path);
                    if let Some(least) = least {
                        assert!(used <= least && least < part.size, "{}", part.path);
                    }
                }
            }
        }
        // Never the paths of the disks Where to install lists.
        let listed: Vec<String> = disks_paths(&super::disks());
        assert!(disks_paths(&disks)
            .iter()
            .all(|path| !listed.contains(path)));
    }

    fn disks_paths(disks: &[Disk]) -> Vec<String> {
        disks.iter().map(|disk| disk.path.clone()).collect()
    }

    #[test]
    fn a_rehearsal_says_every_event_once_and_ends() {
        let easy =
            crate::worker::partition::fixtures::config(disks()[0].clone(), Target::WholeDisk);
        let mut rehearsal = Rehearsal::start(10.0, &easy);
        let mut install = Install::default();
        let mut said = 0;
        let mut now = 10.0;
        while now < 40.0 {
            for event in rehearsal.poll(now) {
                said += 1;
                install.take(event);
            }
            now += 0.1;
        }
        assert_eq!(said, rehearsal.script().len());
        assert!(install.finished());
        assert_eq!(install.steps.len(), ACTS.len());
        assert!(!install.steps.contains(&Step::Resize));
        assert!(install.warnings.is_empty());
        assert!(rehearsal.poll(100.0).is_empty(), "something was said twice");
    }

    /// **A plan that shrinks Windows is rehearsed as the worker carries it
    /// out** (Unit Q, Q9): Resize first, after Check, and once it is done
    /// Done is told Windows will check its disk; a plan that shrinks only an
    /// ext4 shrinks it first and tells Done nothing; an easy installation,
    /// neither.
    #[test]
    fn a_plan_that_shrinks_windows_is_rehearsed_with_its_shrink_and_dones_warning() {
        use crate::model::layout::{DiskLayout, Fate, Part};
        let shrinking = |disk: Disk, number: u32| {
            let parts = disk
                .partitions
                .iter()
                .map(|was| Part {
                    path: was.path.clone(),
                    number: was.number,
                    start: was.start,
                    size: was.size,
                    fate: if was.number == number {
                        Fate::Shrink {
                            to: was.size / 2,
                            mount: None,
                        }
                    } else {
                        Fate::Keep { mount: None }
                    },
                })
                .collect();
            let mut config =
                crate::worker::partition::fixtures::config(disk.clone(), Target::WholeDisk);
            config.target = Target::Layout {
                disks: vec![DiskLayout {
                    disk,
                    table: None,
                    parts,
                    create: Vec::new(),
                }],
            };
            config
        };
        let sata = disks()[1].clone();
        let windows = sata
            .partitions
            .iter()
            .find(|part| part.fs_type == "ntfs" && part.content == Content::Windows)
            .unwrap()
            .number;
        let ssd = partition_disks()[1].clone();
        let home = ssd
            .partitions
            .iter()
            .rfind(|part| part.fs_type == "ext4")
            .unwrap()
            .number;
        for (config, resize, warned) in [
            (shrinking(sata, windows), true, true),
            (shrinking(ssd, home), true, false),
        ] {
            let mut rehearsal = Rehearsal::start(0.0, &config);
            let mut install = Install::default();
            let mut steps = Vec::new();
            for event in rehearsal.poll(100.0) {
                if let Event::Begin { step } = event {
                    steps.push(step);
                }
                install.take(event);
            }
            assert!(install.finished());
            assert_eq!(steps[..2] == [Step::Check, Step::Resize], resize);
            assert_eq!(install.steps, steps);
            assert_eq!(
                install.warnings == [Warning::WindowsWillCheck],
                warned,
                "{:?}",
                install.warnings
            );
        }
    }
}
