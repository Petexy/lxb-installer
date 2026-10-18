//! From an [`InstallConfig`] and the firmware, the typed plan of what the
//! installer writes: which table, which partitions go, which change type,
//! which are made and where, which EFI system partition boots the system,
//! and how the root filesystem is laid out inside.
//!
//! One fact comes from outside the request: whether the disk's own EFI
//! system partition, when one is reused, has room for the kernels
//! ([`SharedRoom`]). Only the worker can look inside it, so it says what it
//! found; when the kernels do not fit, the plan adds a Linux boot partition
//! for them.
//!
//! Every position is in sectors of the disk's logical sector size, read from
//! the disk as reviewed; nothing here assumes 512 bytes (the legacy installer
//! did, and made a 16 GiB EFI partition on a 4Kn disk). Every partition the
//! installer makes starts and ends on a whole MiB, which is a whole number of
//! sectors for every sector size a disk reports.
use crate::model::config::{Filesystem, Home, InstallConfig, Target};
use crate::model::disks::{Content, Disk, Partition, Table, MIB, MIN_INSTALL};
use crate::model::protocol::Failure;
use crate::worker::layout::{EspMount, Firmware, Stop};

/// The numbers both halves build partitions by moved to the rule set the
/// interface shares (`model::layout`), and are read here as they were.
pub use crate::model::layout::{
    usable, Span, ALIGN, BIOS_BOOT_SIZE, ESP_SIZE, LINUX_BOOT_SIZE, MBR_LIMIT,
};

/// The smallest separate /home that is accepted: the legacy installer's 1 GB.
pub const MIN_HOME: u64 = 1_000_000_000;
/// A swap file may take at most this share of the root filesystem (one in
/// N), so a mistaken size cannot leave the system no room to be copied.
pub const SWAP_SHARE: u64 = 2;

pub const GPT_ESP: &str = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B";
pub const GPT_BIOS_BOOT: &str = "21686148-6449-6E6F-744E-656564454649";
/// "Linux root (x86-64)" from the Discoverable Partitions Specification:
/// the root partition says what it is, and tools that understand the
/// specification find it without being told.
pub const GPT_LINUX_ROOT: &str = "4F68BCE3-E8CD-4DB1-96E7-FBCAF984B709";
/// "Linux extended boot" (XBOOTLDR) from the same specification and the
/// Boot Loader Specification: a FAT partition beside the EFI system
/// partition that holds the kernels. It is not a second EFI system
/// partition, which Windows and some firmware handle badly.
pub const GPT_LINUX_BOOT: &str = "BC13C2FF-59E6-4262-A352-B275FD6F7172";
pub const MBR_ESP: &str = "ef";
pub const MBR_LINUX: &str = "83";
/// XBOOTLDR's type on an MBR, from the Boot Loader Specification.
pub const MBR_LINUX_BOOT: &str = "ea";

/// What a partition the installer makes is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Esp,
    BiosBoot,
    /// FAT, mounted at `/boot`, holding the kernels, beside a shared EFI
    /// system partition that holds only the boot loader.
    LinuxBoot,
    Root,
}

impl Role {
    /// The partition type: a GPT type GUID or an MBR type byte in hex, as
    /// sfdisk takes and prints them.
    pub fn part_type(self, table: Table) -> &'static str {
        match (self, table) {
            (Role::Esp, Table::Gpt) => GPT_ESP,
            (Role::BiosBoot, Table::Gpt) => GPT_BIOS_BOOT,
            (Role::Root, Table::Gpt) => GPT_LINUX_ROOT,
            (Role::LinuxBoot, Table::Gpt) => GPT_LINUX_BOOT,
            (Role::Esp, Table::Msdos) => MBR_ESP,
            (Role::LinuxBoot, Table::Msdos) => MBR_LINUX_BOOT,
            // An MBR has no BIOS boot type; GRUB lives in the gap after the
            // MBR there, so the planner never asks for one on msdos.
            (Role::BiosBoot | Role::Root, Table::Msdos) => MBR_LINUX,
        }
    }

    /// The GPT partition name (an MBR has none). Firmware menus and other
    /// systems' disk tools show it.
    pub fn name(self) -> &'static str {
        match self {
            Role::Esp => "EFI system",
            Role::BiosBoot => "BIOS boot",
            Role::LinuxBoot => "Linux boot",
            Role::Root => "Linexin",
        }
    }
}

/// A partition the installer creates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewPartition {
    pub role: Role,
    /// First sector and length in logical sectors, both whole MiB.
    pub start: u64,
    pub size: u64,
}

impl NewPartition {
    pub fn span(self) -> Span {
        Span {
            start: self.start,
            end: self.start + self.size,
        }
    }
}

/// What happens to the partition table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// A new table replaces everything on the disk.
    Fresh,
    /// The new partitions go into free space; nothing that exists changes.
    Add,
    /// The replaced partition keeps its exact place and only its type (and
    /// on GPT its name) changes, so nothing else on the disk moves.
    Retype {
        number: u32,
        path: String,
        span: Span,
    },
    /// The replaced partition is deleted and the new partitions take its
    /// place (an EFI, BIOS boot or Linux boot partition had to be carved
    /// out of it).
    Replace {
        number: u32,
        path: String,
        span: Span,
    },
}

/// Which EFI system partition the installed system boots from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EspPlan {
    /// BIOS: there is none.
    None,
    /// The installer makes one (it is in `Plan::create`).
    New,
    /// The disk's own, shared with whatever else boots from it: never
    /// formatted. Whether it also holds the kernels is what the worker
    /// found inside it ([`SharedRoom`]); when it does not, the plan makes a
    /// Linux boot partition.
    Shared { path: String },
}

/// What the worker found inside the disk's own EFI system partition, when
/// the plan reuses it. The interface cannot look inside without privilege,
/// so it counts as if the kernels did not fit (`model::disks::room`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedRoom {
    /// Room for the kernels too: it is mounted at `/boot`.
    Kernels,
    /// Room for a boot loader only: it is mounted at `/efi`, and a new
    /// Linux boot partition at `/boot` holds the kernels, which the
    /// firmware's own FAT support then reads for any boot loader.
    LoaderOnly,
}

impl SharedRoom {
    /// Where the shared EFI system partition is mounted in the target.
    pub fn esp_mount(self) -> EspMount {
        match self {
            SharedRoom::Kernels => EspMount::Boot,
            SharedRoom::LoaderOnly => EspMount::Efi,
        }
    }
}

/// A Btrfs subvolume and where it is mounted in the installed system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subvolume {
    pub name: &'static str,
    pub at: &'static str,
}

/// The swap file: its path inside the installed system and its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swapfile {
    pub path: &'static str,
    pub mib: u32,
}

impl Swapfile {
    pub fn bytes(self) -> u64 {
        u64::from(self.mib) * MIB
    }
}

/// Everything the storage half writes, decided before anything is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The whole disk (`/dev/sda`).
    pub disk: String,
    /// The disk's size in bytes, as reviewed.
    pub size: u64,
    /// The disk's logical sector size in bytes.
    pub sector: u64,
    /// The table the disk has once the plan is carried out.
    pub table: Table,
    pub change: Change,
    /// The part of the disk the plan writes into. Nothing that is kept may
    /// overlap it; the worker checks that against the disk again before
    /// writing.
    pub space: Span,
    /// The partitions to create, in disk order.
    pub create: Vec<NewPartition>,
    pub esp: EspPlan,
    pub filesystem: Filesystem,
    /// On Btrfs, `@` first; empty on ext4.
    pub subvolumes: Vec<Subvolume>,
    pub home: Option<Home>,
    pub swap: Option<Swapfile>,
}

impl Plan {
    /// The root partition, when the installer creates it.
    pub fn new_root(&self) -> Option<NewPartition> {
        self.create.iter().copied().find(|p| p.role == Role::Root)
    }

    /// The new partition with this role, if the plan makes one.
    pub fn created(&self, role: Role) -> Option<NewPartition> {
        self.create.iter().copied().find(|p| p.role == role)
    }

    /// The size of the root partition in bytes.
    pub fn root_bytes(&self) -> u64 {
        match (&self.change, self.new_root()) {
            (_, Some(root)) => root.size * self.sector,
            (Change::Retype { span, .. }, None) => span.len() * self.sector,
            _ => 0,
        }
    }

    /// On msdos the root partition should carry the boot flag: some BIOSes
    /// will not start a disk on which no partition is marked active.
    pub fn wants_boot_flag(&self) -> bool {
        self.table == Table::Msdos
    }

    /// The plan in a few technical lines, for the log.
    pub fn summary(&self) -> Vec<String> {
        let table = match self.table {
            Table::Gpt => "gpt",
            Table::Msdos => "msdos",
        };
        let change = match &self.change {
            Change::Fresh => "new table over the whole disk".to_string(),
            Change::Add => "add to free space".to_string(),
            Change::Retype { number, path, .. } => format!("retype partition {number} ({path})"),
            Change::Replace { number, path, .. } => {
                format!("delete partition {number} ({path}) and create in its place")
            }
        };
        let mut lines = vec![format!(
            "storage plan: {} {table}, {} byte sectors, {change}, space {}..{}",
            self.disk, self.sector, self.space.start, self.space.end
        )];
        for part in &self.create {
            lines.push(format!(
                "storage plan: create {:?} start={} size={}",
                part.role, part.start, part.size
            ));
        }
        lines.push(format!(
            "storage plan: esp {:?}, {:?}, subvolumes {:?}, home {:?}, swap {:?}",
            self.esp,
            self.filesystem,
            self.subvolumes.iter().map(|s| s.name).collect::<Vec<_>>(),
            self.home.as_ref().map(|h| (&h.path, h.format)),
            self.swap.map(|s| (s.path, s.mib)),
        ));
        lines
    }
}

fn invalid(detail: impl Into<String>) -> Stop {
    Stop::new(Failure::InvalidRequest, detail)
}

fn too_small(detail: impl Into<String>) -> Stop {
    Stop::new(Failure::NotEnoughSpace, detail)
}

/// Works out the plan. Pure: judged from `config.disk` as the person
/// reviewed it, and from what the worker found inside a shared EFI system
/// partition (`shared`, which nothing else uses). The worker checks that
/// the disk still matches before it carries the plan out.
///
/// A layout made by hand is not the easy path's to plan: `InvalidRequest`,
/// before anything else is looked at (the worker refuses it sooner still,
/// in `check::validate`; its executor is `worker::manual`).
pub fn plan(config: &InstallConfig, firmware: Firmware, shared: SharedRoom) -> Result<Plan, Stop> {
    if let Target::Layout { .. } = config.target {
        return Err(invalid(
            "a hand-made layout is not planned as an easy place",
        ));
    }
    let disk = &config.disk;
    if disk.blocked.is_some() || disk.read_only {
        return Err(invalid(format!("{} cannot be chosen", disk.path)));
    }
    let sector = sector_size(disk)?;
    let total = disk.size / sector;
    let placed = match &config.target {
        Target::WholeDisk => whole_disk(firmware, sector, total),
        Target::FreeSpace { start, size } => {
            free_space(disk, firmware, shared, sector, *start, *size)?
        }
        Target::Partition { path } => replace(disk, firmware, shared, sector, path)?,
        Target::Layout { .. } => return Err(invalid("a hand-made layout has no easy place")),
    };
    let mut plan = Plan {
        disk: disk.path.clone(),
        size: disk.size,
        sector,
        table: placed.table,
        change: placed.change,
        space: placed.space,
        create: placed.create,
        esp: placed.esp,
        filesystem: config.storage.filesystem,
        subvolumes: Vec::new(),
        home: None,
        swap: None,
    };
    let root = plan.root_bytes();
    if root < MIN_INSTALL {
        return Err(too_small(format!(
            "the root partition would be {root} bytes, less than {MIN_INSTALL}"
        )));
    }
    plan.home = home(config)?;
    plan.swap = swap(config, root)?;
    plan.subvolumes = subvolumes(plan.filesystem, plan.home.is_some(), plan.swap.is_some());
    Ok(plan)
}

/// What a target turns into before the filesystem layout is added.
struct Placed {
    table: Table,
    change: Change,
    space: Span,
    create: Vec<NewPartition>,
    esp: EspPlan,
}

fn sector_size(disk: &Disk) -> Result<u64, Stop> {
    let sector = u64::from(disk.logical_sector);
    // Linux block devices report 512 to 4096; the alignment below needs the
    // sector to divide a MiB, which every power of two in that range does.
    if !(512..=4096).contains(&sector) || !sector.is_power_of_two() {
        return Err(invalid(format!(
            "{} reports a logical sector size of {sector}",
            disk.path
        )));
    }
    Ok(sector)
}

fn align_up(sectors: u64, align: u64) -> u64 {
    sectors.div_ceil(align) * align
}

fn align_down(sectors: u64, align: u64) -> u64 {
    sectors / align * align
}

/// Lays `roles` out from the start of `space` and gives the rest to root.
fn lay_out(roles: &[Role], space: Span, sector: u64) -> Result<Vec<NewPartition>, Stop> {
    let mut create = Vec::new();
    let mut cursor = space.start;
    for role in roles {
        let size = match role {
            Role::Esp => ESP_SIZE / sector,
            Role::BiosBoot => BIOS_BOOT_SIZE / sector,
            Role::LinuxBoot => LINUX_BOOT_SIZE / sector,
            Role::Root => unreachable!("root takes what is left"),
        };
        create.push(NewPartition {
            role: *role,
            start: cursor,
            size,
        });
        cursor += size;
    }
    if cursor >= space.end {
        return Err(too_small(format!(
            "sectors {}..{} leave no room for root",
            space.start, space.end
        )));
    }
    create.push(NewPartition {
        role: Role::Root,
        start: cursor,
        size: space.end - cursor,
    });
    Ok(create)
}

fn whole_disk(firmware: Firmware, sector: u64, total: u64) -> Placed {
    // UEFI gets GPT. A BIOS gets the msdos table the GTK installer made, so
    // old firmware that only starts an MBR disk with an active partition is
    // still served, unless the disk is too big for an MBR: then GPT with a
    // BIOS boot partition, which GRUB supports.
    let table = match firmware {
        Firmware::Uefi => Table::Gpt,
        Firmware::Bios if total <= MBR_LIMIT => Table::Msdos,
        Firmware::Bios => Table::Gpt,
    };
    let space = usable(table, total, sector);
    let roles: &[Role] = match (firmware, table) {
        (Firmware::Uefi, _) => &[Role::Esp],
        (Firmware::Bios, Table::Gpt) => &[Role::BiosBoot],
        (Firmware::Bios, Table::Msdos) => &[],
    };
    // A disk too small for even the boot partitions has an empty root here,
    // which the size check in `plan` refuses with the right failure.
    let create = lay_out(roles, space, sector).unwrap_or_default();
    Placed {
        table,
        change: Change::Fresh,
        space,
        create,
        esp: match firmware {
            Firmware::Uefi => EspPlan::New,
            Firmware::Bios => EspPlan::None,
        },
    }
}

/// A partition's exact sectors.
fn span_of(part: &Partition, sector: u64) -> Result<Span, Stop> {
    if !part.start.is_multiple_of(sector) || !part.size.is_multiple_of(sector) {
        return Err(invalid(format!(
            "{} is not on whole sectors of {sector} bytes",
            part.path
        )));
    }
    Ok(Span {
        start: part.start / sector,
        end: (part.start + part.size) / sector,
    })
}

/// An msdos extended partition: a container for logical partitions, which
/// lsblk reports as a tiny partition although it spans all of them.
pub fn is_extended(part_type: &str) -> bool {
    let bare = part_type
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    matches!(
        bare.trim_start_matches('0').to_ascii_lowercase().as_str(),
        "5" | "f" | "85"
    )
}

fn has_bios_boot(disk: &Disk) -> bool {
    disk.partitions
        .iter()
        .any(|p| p.content == Content::BiosBoot)
}

/// The primary slots an msdos table has left after `deleting` is gone.
fn free_primaries(disk: &Disk, deleting: Option<u32>) -> usize {
    let used = disk
        .partitions
        .iter()
        .filter(|p| (1..=4).contains(&p.number) && Some(p.number) != deleting)
        .count();
    4usize.saturating_sub(used)
}

/// The partitions the firmware needs made at the start of the space used,
/// and which EFI system partition boots: under UEFI the disk's own when it
/// has one (with a Linux boot partition when the kernels do not fit in it),
/// else a new one; under BIOS on GPT a BIOS boot partition unless the disk
/// has one.
fn boot_roles(
    disk: &Disk,
    firmware: Firmware,
    table: Table,
    shared: SharedRoom,
) -> (&'static [Role], EspPlan) {
    match firmware {
        Firmware::Uefi => match disk.esp() {
            Some(esp) => {
                let roles: &[Role] = match shared {
                    SharedRoom::Kernels => &[],
                    SharedRoom::LoaderOnly => &[Role::LinuxBoot],
                };
                let path = esp.path.clone();
                (roles, EspPlan::Shared { path })
            }
            None => (&[Role::Esp], EspPlan::New),
        },
        Firmware::Bios if table == Table::Gpt && !has_bios_boot(disk) => {
            (&[Role::BiosBoot], EspPlan::None)
        }
        Firmware::Bios => (&[], EspPlan::None),
    }
}

fn free_space(
    disk: &Disk,
    firmware: Firmware,
    shared: SharedRoom,
    sector: u64,
    start: u64,
    size: u64,
) -> Result<Placed, Stop> {
    let table = disk
        .table
        .ok_or_else(|| invalid(format!("{} has no partition table", disk.path)))?;
    if !disk.free.iter().any(|g| g.start == start && g.size == size) {
        return Err(invalid(format!(
            "free space at {start}+{size} is not one {} listed",
            disk.path
        )));
    }
    // The gap list is not part of the identity the worker checks the disk
    // by again, so a gap that does not lie on the disk is refused here
    // rather than wrapping around in the arithmetic below.
    if start.checked_add(size).is_none_or(|end| end > disk.size) {
        return Err(invalid(format!(
            "free space at {start}+{size} is not on {}",
            disk.path
        )));
    }
    if table == Table::Msdos && disk.partitions.iter().any(|p| is_extended(&p.part_type)) {
        // Space beside or inside an extended partition can only take a
        // logical partition, whose place sfdisk chooses itself; refused
        // rather than guessed at.
        return Err(invalid(format!(
            "{} has an extended partition; only replacing a partition is possible",
            disk.path
        )));
    }
    let align = ALIGN / sector;
    let usable = usable(table, disk.size / sector, sector);
    let space = Span {
        start: align_up(start.div_ceil(sector), align).max(usable.start),
        end: align_down((start + size) / sector, align).min(usable.end),
    };
    if space.is_empty() {
        return Err(too_small(format!(
            "free space at {start}+{size} holds no whole MiB"
        )));
    }
    for part in &disk.partitions {
        if span_of(part, sector)?.overlaps(space) {
            return Err(invalid(format!(
                "free space at {start}+{size} overlaps {}",
                part.path
            )));
        }
    }
    let (roles, esp) = boot_roles(disk, firmware, table, shared);
    let create = lay_out(roles, space, sector)?;
    if table == Table::Msdos && create.len() > free_primaries(disk, None) {
        return Err(invalid(format!(
            "{} has no free primary partition slot",
            disk.path
        )));
    }
    Ok(Placed {
        table,
        change: Change::Add,
        space,
        create,
        esp,
    })
}

fn replace(
    disk: &Disk,
    firmware: Firmware,
    shared: SharedRoom,
    sector: u64,
    path: &str,
) -> Result<Placed, Stop> {
    let table = disk
        .table
        .ok_or_else(|| invalid(format!("{} has no partition table", disk.path)))?;
    let part = disk
        .partition(path)
        .ok_or_else(|| invalid(format!("{path} is not a partition of {}", disk.path)))?;
    if matches!(
        part.content,
        Content::Efi | Content::BiosBoot | Content::MicrosoftReserved
    ) || is_extended(&part.part_type)
    {
        return Err(invalid(format!("{path} cannot be replaced")));
    }
    let span = span_of(part, sector)?;
    let (carve, esp) = boot_roles(disk, firmware, table, shared);
    if carve.is_empty() {
        return Ok(Placed {
            table,
            change: Change::Retype {
                number: part.number,
                path: part.path.clone(),
                span,
            },
            space: span,
            create: Vec::new(),
            esp,
        });
    }
    if table == Table::Msdos && part.number > 4 {
        // Deleting a logical partition renumbers every logical partition
        // after it, which would change the paths of partitions the person
        // chose to keep (a separate /home among them).
        return Err(invalid(format!(
            "{path} is a logical partition and cannot be split"
        )));
    }
    let align = ALIGN / sector;
    let space = Span {
        start: align_up(span.start, align),
        end: align_down(span.end, align),
    };
    if space.is_empty() {
        return Err(too_small(format!("{path} holds no whole MiB")));
    }
    let create = lay_out(carve, space, sector)?;
    if table == Table::Msdos && create.len() > free_primaries(disk, Some(part.number)) {
        return Err(invalid(format!(
            "{} has no free primary partition slot",
            disk.path
        )));
    }
    Ok(Placed {
        table,
        change: Change::Replace {
            number: part.number,
            path: part.path.clone(),
            span,
        },
        space,
        create,
        esp,
    })
}

/// The separate /home, checked against the disk it is on as far as it can
/// be from the request: the target disk, or the disk the request carries for
/// it. The device itself is checked on the machine by the worker before
/// anything is written.
fn home(config: &InstallConfig) -> Result<Option<Home>, Stop> {
    let Some(home) = &config.storage.home else {
        return Ok(None);
    };
    let disk = &config.disk;
    let elsewhere = home.disk.as_ref();
    if !home.path.starts_with("/dev/")
        || home.path == disk.path
        || elsewhere.is_some_and(|other| other.path == disk.path || home.path == other.path)
    {
        return Err(invalid(format!("{} cannot be /home", home.path)));
    }
    let on = elsewhere.unwrap_or(disk);
    // A partition said to be on another disk must be found there.
    if elsewhere.is_some() && on.partition(&home.path).is_none() {
        return Err(invalid(format!(
            "{} is not on {}, the disk named for /home",
            home.path, on.path
        )));
    }
    if let Some(part) = on.partition(&home.path) {
        let erased = elsewhere.is_none()
            && match &config.target {
                Target::WholeDisk => true,
                Target::Partition { path } => *path == home.path,
                Target::FreeSpace { .. } => false,
                // Never reached (`plan` refuses a layout first); refused
                // were it ever.
                Target::Layout { .. } => true,
            };
        let unusable = matches!(
            part.content,
            Content::Efi
                | Content::BiosBoot
                | Content::MicrosoftReserved
                | Content::WindowsRecovery
        ) || is_extended(&part.part_type);
        // Kept data is only mounted as /home when it is a Linux filesystem:
        // the new account takes ownership of its home folder there, which
        // must never happen to a Windows or FAT partition.
        let foreign = !home.format && part.content != Content::Linux;
        if erased || unusable || foreign || part.size < MIN_HOME {
            return Err(invalid(format!("{} cannot be /home", home.path)));
        }
    }
    Ok(Some(home.clone()))
}

fn swap(config: &InstallConfig, root: u64) -> Result<Option<Swapfile>, Stop> {
    let mib = config.storage.swap_mib;
    if mib == 0 {
        return Ok(None);
    }
    let swapfile = Swapfile {
        path: match config.storage.filesystem {
            Filesystem::Ext4 => "/swapfile",
            // Its own subvolume, so snapshots of @ never include it (a
            // Btrfs swap file cannot be in a snapshotted subvolume).
            Filesystem::Btrfs => "/swap/swapfile",
        },
        mib,
    };
    if swapfile.bytes() > root / SWAP_SHARE {
        return Err(invalid(format!(
            "a swap file of {mib} MiB does not fit a root of {root} bytes"
        )));
    }
    Ok(Some(swapfile))
}

fn subvolumes(filesystem: Filesystem, separate_home: bool, swap: bool) -> Vec<Subvolume> {
    if filesystem != Filesystem::Btrfs {
        return Vec::new();
    }
    let mut list = vec![Subvolume { name: "@", at: "/" }];
    if !separate_home {
        list.push(Subvolume {
            name: "@home",
            at: "/home",
        });
    }
    if swap {
        list.push(Subvolume {
            name: "@swap",
            at: "/swap",
        });
    }
    list
}
