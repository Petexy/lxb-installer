//! Phases B and C of a hand-made layout as the exact commands that carry
//! them out, and what the disks must say between the two (§7.0, Unit Q,
//! "The worker" 3–5). Pure: built from the layout as the person reviewed it
//! and the tables read from the disks before anything is written (phase A),
//! so the order of every phase and the arguments of every command are
//! tested on a development machine, and the image tests run the very same
//! commands on image files.
//!
//! Phase B is done disk by disk, in path order, and on every disk before
//! anything is formatted (an exclusive claim of a whole disk fails once
//! anything on it is mounted). On each disk, in this order, so that every
//! step is harmless on its own were the run to stop after it — a
//! filesystem is shrunk before its entry, only what is made or formatted
//! grows, and nothing is made where something still stands:
//!
//! 1. kept filesystems shrunk ([`Planned::ShrinkFilesystem`]: here only their
//!    place; their commands are `resize::Shrink`'s, which `shrinking` runs,
//!    step Q9);
//! 2. entries made smaller — a shrunk partition's, and a formatted one's
//!    resized smaller — each by `sfdisk -N` with its number found at its
//!    reviewed start, never by its path, given the same start (its script,
//!    as every script here, saying first its unit and the disk's sector
//!    size);
//! 3. every partition deleted, in one `sfdisk --delete`, highest number
//!    first, so no logical partition is renumbered before it is deleted;
//! 4. or, for a new table, every signature wiped (`wipefs --all`) and the
//!    table written with every partition made in one script;
//! 5. the partitions made on a table that is kept, in one `--append`
//!    script, in start order;
//! 6. formatted partitions' entries grown;
//! 7. formatted partitions retyped, and renamed on GPT;
//! 8. the boot flag on the msdos disk that holds `/`, by the easy path's
//!    rule ([`activation`]: only when nothing is marked already).
//!
//! Then the disks are read back ([`read_back`]): each partition made found
//! at its exact start and size, each kept one at its start, its new size,
//! its number and its filesystem's UUID, nothing else there — which maps
//! every partition to its path now ([`Paths`]). Phase C formats each
//! partition made or formatted, found by that map: `wipefs --all`, then its
//! filesystem's tool with the label Partitions showed ([`format_commands`]).
//!
//! Partition types and GPT names are computed here, never carried in the
//! layout: the type by the rule set's own `layout::part_type`, the name by
//! [`gpt_name`]. A kept partition is never retyped or renamed.
use crate::model::disks::{same_uuid, Partition, Table};
use crate::model::layout::{self, DiskLayout, Fate, Fs, MountPoint, Part, Role, Which};
use crate::worker::layout::Firmware;
use crate::worker::partition::script::Command;
use crate::worker::partition::table::{Kernel, OnDisk};
use std::collections::BTreeMap;

/// What one planned command does, in the order the phases do it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Act {
    /// B.1: a kept filesystem shrunk to its new end.
    ShrinkFilesystem,
    /// B.2: an entry made smaller (`sfdisk -N`).
    ShrinkEntry,
    /// B.3: partitions deleted (`sfdisk --delete`).
    Delete,
    /// B.4: every signature on the disk wiped (`wipefs --all --lock`).
    Wipe,
    /// B.4: a new table with every partition made (`sfdisk --wipe always`).
    Fresh,
    /// B.5: partitions made on a kept table (`sfdisk --append`).
    Append,
    /// B.6: a formatted partition's entry grown (`sfdisk -N`).
    Grow,
    /// B.7: a formatted partition retyped (`sfdisk --part-type`).
    Retype,
    /// B.7: a formatted partition renamed on GPT (`sfdisk --part-label`).
    Rename,
    /// B.8: the boot flag (`sfdisk --activate`).
    Activate,
    /// C: a partition's signatures wiped (`wipefs --all`).
    WipeSignatures,
    /// C: a filesystem made (`mkfs.*`, `mkswap`).
    Make,
}

impl Act {
    /// Its step in the plan's numbering: phase B's 1 to 8 (B.4's two
    /// commands are one step, B.7's two another), and phase C after all of
    /// them.
    pub fn step(self) -> u8 {
        match self {
            Act::ShrinkFilesystem => 1,
            Act::ShrinkEntry => 2,
            Act::Delete => 3,
            Act::Wipe | Act::Fresh => 4,
            Act::Append => 5,
            Act::Grow => 6,
            Act::Retype | Act::Rename => 7,
            Act::Activate => 8,
            Act::WipeSignatures | Act::Make => 10,
        }
    }
}

/// One thing a phase does to one device, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Planned {
    /// B.1: the filesystem on `device` (`fs`, lsblk's name), kept, shrunk
    /// from `from` bytes to `to`. Data only: its commands, word for word,
    /// are [`resize::Shrink::of`](super::resize::Shrink::of) it (step Q8),
    /// which `shrinking` runs (step Q9).
    ShrinkFilesystem {
        device: String,
        fs: String,
        from: u64,
        to: u64,
    },
    /// One command, which writes `device`.
    Run {
        act: Act,
        device: String,
        command: Command,
    },
}

impl Planned {
    pub fn act(&self) -> Act {
        match self {
            Planned::ShrinkFilesystem { .. } => Act::ShrinkFilesystem,
            Planned::Run { act, .. } => *act,
        }
    }

    /// The device it writes: a disk, or a partition on one.
    pub fn device(&self) -> &str {
        match self {
            Planned::ShrinkFilesystem { device, .. } | Planned::Run { device, .. } => device,
        }
    }

    pub fn command(&self) -> Option<&Command> {
        match self {
            Planned::ShrinkFilesystem { .. } => None,
            Planned::Run { command, .. } => Some(command),
        }
    }

    /// The line the log carries for it: the command as it is run, its
    /// script after it on lines of its own.
    pub fn shown(&self) -> String {
        match self {
            Planned::ShrinkFilesystem {
                device,
                fs,
                from,
                to,
            } => format!("shrink {fs} on {device} from {from} to {to} bytes"),
            Planned::Run { command, .. } => {
                let mut line = command.program.to_string();
                for arg in &command.args {
                    line.push(' ');
                    line.push_str(arg);
                }
                if let Some(input) = &command.input {
                    line.push_str(" <<");
                    for script in input.lines() {
                        line.push_str("\n  ");
                        line.push_str(script);
                    }
                }
                line
            }
        }
    }
}

/// What a layout's partitions are typed and named by: the firmware, and
/// whether the layout uses an EFI system partition at `/efi` (a FAT32
/// `/boot` beside one is a Linux boot partition, alone it is the EFI system
/// partition).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    pub firmware: Firmware,
    pub efi_used: bool,
}

impl Context {
    pub fn new(layout: &[DiskLayout], firmware: Firmware) -> Self {
        Self {
            firmware,
            efi_used: layout::efi_used(layout),
        }
    }

    fn role(self, at: Option<&MountPoint>) -> Role {
        layout::role(self.firmware, at, self.efi_used)
    }

    /// The type a partition made or formatted as `fs` and used at `at` is
    /// given on `table`, as sfdisk takes it (an MBR type without its `0x`).
    pub fn part_type(self, table: Table, fs: Fs, at: Option<&MountPoint>) -> &'static str {
        let named = layout::part_type(table, fs, at, self.role(at));
        named.strip_prefix("0x").unwrap_or(named)
    }

    /// Its GPT name ([`gpt_name`]).
    pub fn name(self, fs: Fs, at: Option<&MountPoint>) -> &'static str {
        gpt_name(fs, at, self.role(at))
    }
}

/// The name a partition made or formatted on a GPT is given — firmware
/// menus and other systems' disk tools show it — by what it is used as, as
/// the easy path names its own: "EFI system", "Linux boot", "BIOS boot",
/// "Linexin" for `/`, "Home", "Linux swap", and "Linux filesystem" (the
/// type's own name) for everything else. Never a path or a label: a name is
/// quoted in sfdisk's script, and these hold no quote, no `/` and nothing a
/// script could read as anything but a name.
pub fn gpt_name(fs: Fs, at: Option<&MountPoint>, role: Role) -> &'static str {
    match (fs, role, at) {
        (Fs::BiosGrub, _, _) => "BIOS boot",
        (_, Role::Esp, _) => "EFI system",
        (_, Role::LinuxBoot, _) => "Linux boot",
        (Fs::LinuxSwap, _, _) => "Linux swap",
        (_, _, Some(MountPoint::Root)) => "Linexin",
        (_, _, Some(MountPoint::Home)) => "Home",
        _ => "Linux filesystem",
    }
}

/// `bytes` in sectors of `sector` bytes, which it must be a whole number of.
fn sectors(bytes: u64, sector: u64) -> Result<u64, String> {
    if !bytes.is_multiple_of(sector) {
        return Err(format!(
            "{bytes} bytes is not a whole number of {sector} byte sectors"
        ));
    }
    Ok(bytes / sector)
}

/// A partition type as both sfdisk and lsblk write it, compared without
/// case, a `0x` or leading zeros (`0x07`, `7`; a GUID in either case).
fn same_type(one: &str, other: &str) -> bool {
    let bare = |text: &str| {
        let text = text.trim().to_ascii_lowercase();
        let text = text.strip_prefix("0x").unwrap_or(&text).to_string();
        text.trim_start_matches('0').to_string()
    };
    bare(one) == bare(other)
}

/// Where an extended partition's frame reaches at the least: as far as the
/// logical partitions in it go (the kernel shows the frame as a stub).
fn extended_reach(partitions: &[Partition], extended: &Partition) -> u64 {
    partitions
        .iter()
        .filter(|part| part.number > 4)
        .map(|part| part.start + part.size)
        .fold(extended.start + extended.size, u64::max)
}

/// **Phase A's check of one disk's table**, read with `sfdisk --json`
/// before anything is written: the table the person reviewed — no table
/// where there was none; else the same kind, in the same sector size, with
/// exactly the reviewed partitions, each at its number, start, size and
/// type (an extended partition's frame reaching at least as far as its
/// logical partitions, which is all discovery can say of it), and marked
/// active or not as it was reviewed — so the boot flag step 8 sets where
/// nothing standing is marked ([`activation`]) is set exactly where the
/// rule set judged the plan would set it (`layout::activates`). Any
/// difference refuses before a byte is written.
pub fn check_table(one: &DiskLayout, on_disk: Option<&OnDisk>) -> Result<(), String> {
    let disk = &one.disk;
    let sector = u64::from(disk.logical_sector);
    let on_disk = match (disk.table, on_disk) {
        (None, None) => return Ok(()),
        (None, Some(_)) => return Err(format!("{} now has a partition table", disk.path)),
        (Some(_), None) => return Err(format!("{} has lost its partition table", disk.path)),
        (Some(table), Some(on_disk)) if on_disk.table != table => {
            return Err(format!(
                "{} now has a {:?} table, not {table:?}",
                disk.path, on_disk.table
            ))
        }
        (Some(_), Some(on_disk)) => on_disk,
    };
    if on_disk.sector.is_some_and(|read| read != sector) {
        return Err(format!(
            "{} now has {:?} byte sectors, not {sector}",
            disk.path, on_disk.sector
        ));
    }
    if on_disk.partitions.len() != disk.partitions.len() {
        return Err(format!(
            "{} now has {} partitions, not {}",
            disk.path,
            on_disk.partitions.len(),
            disk.partitions.len()
        ));
    }
    for was in &disk.partitions {
        let changed = || format!("{} is not as it was reviewed", was.path);
        let entry = on_disk
            .partitions
            .iter()
            .find(|entry| entry.number == was.number)
            .ok_or_else(changed)?;
        let end = (entry.start + entry.size) * sector;
        let size_holds = if was.is_extended() {
            end >= extended_reach(&disk.partitions, was)
        } else {
            entry.size * sector == was.size
        };
        if entry.start * sector != was.start
            || !size_holds
            || !same_type(&entry.part_type, &was.part_type)
            || entry.bootable != was.bootable
        {
            return Err(changed());
        }
    }
    Ok(())
}

/// The number of the entry that starts at `start` bytes on the table read
/// (`on_disk`): an entry is always found by where it starts, never by its
/// path.
fn number_at(on_disk: &OnDisk, start: u64, sector: u64) -> Result<u32, String> {
    let at = sectors(start, sector)?;
    on_disk
        .partitions
        .iter()
        .find(|entry| entry.start == at)
        .map(|entry| entry.number)
        .ok_or_else(|| format!("no partition starts at sector {at}"))
}

/// The starts of the partitions on `one`'s disk the boot flag may go on,
/// in the order it is offered them: a `/boot` on that disk, then `/` —
/// none where `/` is not on it.
fn flag_candidates(one: &DiskLayout) -> Vec<u64> {
    let at = |mount: &MountPoint| {
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
    let Some(root) = at(&MountPoint::Root) else {
        return Vec::new();
    };
    at(&MountPoint::Boot).into_iter().chain([root]).collect()
}

/// What every sfdisk script of a layout begins with, after a new table's
/// label: the unit and the disk's sector size as it was reviewed — so were
/// the device's own sector size ever another, sfdisk converts rather than
/// places everything eight times off. A script that makes partitions and
/// one that gives an entry a new size (`-N`) alike.
fn header(sector: u64) -> String {
    format!("unit: sectors\nsector-size: {sector}\n")
}

/// The sfdisk script that makes `one`'s partitions: a new table's label
/// first where `fresh` is one, then the unit and the disk's sector size
/// ([`header`]), then each partition made, in start order, at its exact
/// start and size, its type computed, its GPT name quoted. A new msdos
/// table marks the partition the boot flag goes on in the same write.
fn script(ctx: Context, one: &DiskLayout, fresh: Option<Table>) -> Result<String, String> {
    let disk = &one.disk;
    let sector = u64::from(disk.logical_sector);
    let table = fresh
        .or(disk.table)
        .ok_or_else(|| format!("{} has no partition table to make partitions in", disk.path))?;
    let mut text = String::new();
    if let Some(fresh) = fresh {
        text.push_str(match fresh {
            Table::Gpt => "label: gpt\n",
            Table::Msdos => "label: dos\n",
        });
    }
    text.push_str(&header(sector));
    let flagged = match fresh {
        Some(Table::Msdos) => flag_candidates(one).first().copied(),
        _ => None,
    };
    let mut made: Vec<_> = one.create.iter().collect();
    made.sort_by_key(|made| made.start);
    for made in made {
        let at = made.mount.as_ref();
        text.push_str(&format!(
            "start={}, size={}, type={}",
            sectors(made.start, sector)?,
            sectors(made.size, sector)?,
            ctx.part_type(table, made.fs, at)
        ));
        match table {
            Table::Gpt => text.push_str(&format!(", name=\"{}\"", ctx.name(made.fs, at))),
            Table::Msdos if flagged == Some(made.start) => text.push_str(", bootable"),
            Table::Msdos => {}
        }
        text.push('\n');
    }
    Ok(text)
}

/// Whether `one` writes to its disk at all (the rule set's own, which
/// judges what a layout needs proven by it too): a new table, a partition
/// made, or any reviewed partition not simply kept. A disk only mounted
/// from is never written, and no destructive command is ever given it.
pub use crate::model::layout::writes_to;

/// **Phase B on one disk, steps 1 to 7**, in order, on `device` (the
/// disk's own path when the worker runs them; an image file's in a test),
/// with every entry found in `on_disk`, the table phase A read from it. The
/// boot flag (step 8) is [`activation`]'s, once the partitions exist.
pub fn table_commands(
    ctx: Context,
    one: &DiskLayout,
    on_disk: Option<&OnDisk>,
    device: &str,
) -> Result<Vec<Planned>, String> {
    let disk = &one.disk;
    let sector = u64::from(disk.logical_sector);
    let run = |act: Act, command: Command| Planned::Run {
        act,
        device: device.to_string(),
        command,
    };
    let mut planned = Vec::new();
    if let Some(table) = one.table {
        if one.parts.iter().any(|part| part.fate != Fate::Delete) {
            return Err(format!("{} keeps a partition under a new table", disk.path));
        }
        // Every signature on the disk goes first (an old GPT's backup
        // header at the end of it included), so nothing that was there can
        // be found again beside the new table.
        planned.push(run(
            Act::Wipe,
            Command::new("wipefs", &["--all", "--lock", device]),
        ));
        planned.push(run(
            Act::Fresh,
            Command::new(
                "sfdisk",
                &[
                    "--lock",
                    "--wipe",
                    "always",
                    "--wipe-partitions",
                    "always",
                    device,
                ],
            )
            .with_input(script(ctx, one, Some(table))?),
        ));
        return Ok(planned);
    }
    if !writes_to(one) {
        return Ok(planned);
    }
    let on_disk =
        on_disk.ok_or_else(|| format!("{} has no partition table to change", disk.path))?;
    check_table(one, Some(on_disk))?;
    let table = on_disk.table;
    let mut parts: Vec<(&Part, &Partition)> = one.parts.iter().zip(&disk.partitions).collect();
    parts.sort_by_key(|(part, _)| part.start);
    let number = |part: &Part| number_at(on_disk, part.start, sector).map(|n| n.to_string());
    // An entry given a new size, its start the same: in the disk's own
    // sectors, said so in the script's header as every script of a layout
    // says it.
    let resize = |act: Act, part: &Part, size: u64| -> Result<Planned, String> {
        let input = format!(
            "{}start={}, size={}\n",
            header(sector),
            sectors(part.start, sector)?,
            sectors(size, sector)?
        );
        Ok(run(
            act,
            Command::new("sfdisk", &["--lock", "-N", &number(part)?, device]).with_input(input),
        ))
    };

    // 1. Each kept filesystem shrunk, before its entry is.
    for (part, was) in &parts {
        if let Fate::Shrink { to, .. } = part.fate {
            planned.push(Planned::ShrinkFilesystem {
                device: part.path.clone(),
                fs: was.fs_type.clone(),
                from: part.size,
                to,
            });
        }
    }
    // 2. Entries made smaller: the shrunk filesystems', and formatted
    // partitions resized smaller.
    for (part, _) in &parts {
        let smaller = match part.fate {
            Fate::Shrink { to, .. } => Some(to),
            Fate::Format { size, .. } if size < part.size => Some(size),
            _ => None,
        };
        if let Some(size) = smaller {
            planned.push(resize(Act::ShrinkEntry, part, size)?);
        }
    }
    // 3. Every partition deleted, in one call, the highest number first:
    // deleting a logical partition renumbers those numbered after it, and
    // deleting an extended one takes its logical partitions with it. Never
    // `--delete` alone, which would delete every partition.
    let mut deleted = Vec::new();
    for (part, _) in &parts {
        if part.fate == Fate::Delete {
            deleted.push(number_at(on_disk, part.start, sector)?);
        }
    }
    if !deleted.is_empty() {
        deleted.sort_unstable_by(|one, other| other.cmp(one));
        let numbers: Vec<String> = deleted.iter().map(u32::to_string).collect();
        let mut args = vec!["--lock", "--delete", device];
        args.extend(numbers.iter().map(String::as_str));
        planned.push(run(Act::Delete, Command::new("sfdisk", &args)));
    }
    // 5. The partitions made, in one script. Old signatures inside them are
    // wiped, so a filesystem that once lived there is never found beside
    // the new one.
    if !one.create.is_empty() {
        planned.push(run(
            Act::Append,
            Command::new(
                "sfdisk",
                &["--lock", "--append", "--wipe-partitions", "always", device],
            )
            .with_input(script(ctx, one, None)?),
        ));
    }
    // 6. Formatted partitions' entries grown, into what was freed.
    for (part, _) in &parts {
        if let Fate::Format { size, .. } = part.fate {
            if size > part.size {
                planned.push(resize(Act::Grow, part, size)?);
            }
        }
    }
    // 7. Formatted partitions typed — and named, on GPT — for what they
    // are used as.
    for (part, _) in &parts {
        if let Fate::Format { fs, mount, .. } = &part.fate {
            let n = number(part)?;
            let at = mount.as_ref();
            planned.push(run(
                Act::Retype,
                Command::new(
                    "sfdisk",
                    &[
                        "--lock",
                        "--part-type",
                        device,
                        &n,
                        ctx.part_type(table, *fs, at),
                    ],
                ),
            ));
            if table == Table::Gpt {
                planned.push(run(
                    Act::Rename,
                    Command::new(
                        "sfdisk",
                        &["--lock", "--part-label", device, &n, ctx.name(*fs, at)],
                    ),
                ));
            }
        }
    }
    Ok(planned)
}

/// **Phase B, step 8**: on a kept msdos table on the disk that holds `/`,
/// the command that marks a `/boot` there — or `/` — active, by the easy
/// path's rule: only when no partition is marked already (Windows keeps the
/// mark on its own, and GRUB in the MBR never reads it, so moving it would
/// only matter were the run to stop before GRUB is written, and then that
/// system could no longer start), and only on a primary partition, which
/// alone the flag can start. `after` is the table read once steps 1 to 7
/// are done, which numbers the partitions made. A new msdos table marks it
/// in its own script.
pub fn activation(one: &DiskLayout, after: &OnDisk, device: &str) -> Option<Planned> {
    if one.table.is_some() || after.table != Table::Msdos {
        return None;
    }
    if after.partitions.iter().any(|entry| entry.bootable) {
        return None;
    }
    let sector = u64::from(one.disk.logical_sector);
    let number = flag_candidates(one).into_iter().find_map(|start| {
        let number = number_at(after, start, sector).ok()?;
        (1..=4).contains(&number).then_some(number)
    })?;
    Some(Planned::Run {
        act: Act::Activate,
        device: device.to_string(),
        command: Command::new(
            "sfdisk",
            &["--lock", "--activate", device, &number.to_string()],
        ),
    })
}

/// **Phase B on every disk, steps 1 to 7**, disk by disk in path order,
/// each on its own path, from the tables phase A read (by disk path).
pub fn writes(
    ctx: Context,
    layout: &[DiskLayout],
    tables: &BTreeMap<String, OnDisk>,
) -> Result<Vec<Planned>, String> {
    let mut disks: Vec<&DiskLayout> = layout.iter().collect();
    disks.sort_by(|one, other| one.disk.path.cmp(&other.disk.path));
    let mut planned = Vec::new();
    for one in disks {
        let path = &one.disk.path;
        planned.extend(table_commands(ctx, one, tables.get(path), path)?);
    }
    Ok(planned)
}

/// Where each partition a layout keeps or makes is now, once phase B is
/// done: what phase C formats and phase E mounts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Paths(BTreeMap<Which, String>);

impl Paths {
    /// The path of `which` now.
    pub fn of(&self, which: &Which) -> Option<&str> {
        self.0.get(which).map(String::as_str)
    }

    /// The partition at `device` now, if it is one of the layout's.
    pub fn which(&self, device: &str) -> Option<&Which> {
        self.0
            .iter()
            .find(|(_, path)| path.as_str() == device)
            .map(|(which, _)| which)
    }

    pub fn insert(&mut self, which: Which, path: String) {
        self.0.insert(which, path);
    }

    /// Every partition with its path now.
    pub fn iter(&self) -> impl Iterator<Item = (&Which, &str)> {
        self.0.iter().map(|(which, path)| (which, path.as_str()))
    }

    /// The paths of a disk nothing is written to: each partition it keeps
    /// where it was reviewed (found again as it was, by `same_disk`).
    pub fn as_reviewed(one: &DiskLayout) -> Paths {
        let mut paths = Paths::default();
        for part in &one.parts {
            if part.fate != Fate::Delete {
                paths.insert(Which::Part(part.path.clone()), part.path.clone());
            }
        }
        paths
    }

    /// Every path of `other` added to these.
    pub fn extend(&mut self, other: Paths) {
        self.0.extend(other.0);
    }
}

/// **Phase B, step 9: what the disk must say once it is written**, from
/// the kernel's partitions (`kernel`, as lsblk lists them, starts in
/// bytes) and each one's filesystem UUID as `blkid -p` reads it
/// (`uuid_of`): every partition made at exactly its start and size; every
/// reviewed partition that is not deleted at its start, under its number,
/// at its new size (kept, shrunk, or formatted and resized), and — where
/// it was reviewed with one — its filesystem's UUID unchanged (compared
/// without case, `disks::same_uuid`); and nothing
/// else, so every partition deleted is gone. Any difference refuses before
/// anything is formatted. Gives every partition's path now, the kernel's
/// own, never built from the disk's name.
pub fn read_back(
    one: &DiskLayout,
    kernel: &[Kernel],
    uuid_of: &dyn Fn(&str) -> Option<String>,
) -> Result<Paths, String> {
    let disk = &one.disk.path;
    let mut taken = vec![false; kernel.len()];
    let mut find = |start: u64, size: u64, number: Option<u32>| {
        let index = kernel.iter().enumerate().position(|(index, found)| {
            !taken[index]
                && found.start == start
                && found.size == size
                && number.is_none_or(|number| found.number == number)
        })?;
        taken[index] = true;
        Some(&kernel[index])
    };
    let mut paths = Paths::default();
    for made in &one.create {
        let found = find(made.start, made.size, None).ok_or_else(|| {
            format!(
                "the partition made at byte {} of {disk} is not there",
                made.start
            )
        })?;
        paths.insert(
            Which::Made {
                disk: disk.clone(),
                start: made.start,
            },
            found.path.clone(),
        );
    }
    for (part, was) in one.parts.iter().zip(&one.disk.partitions) {
        let size = match &part.fate {
            Fate::Delete => continue,
            Fate::Keep { .. } => part.size,
            Fate::Shrink { to, .. } => *to,
            Fate::Format { size, .. } => *size,
        };
        let found = find(part.start, size, Some(part.number))
            .ok_or_else(|| format!("{} is not where the layout leaves it", part.path))?;
        if !was.uuid.is_empty() {
            let uuid = uuid_of(&found.path).unwrap_or_default();
            if !same_uuid(&uuid, &was.uuid) {
                return Err(format!(
                    "{} holds the filesystem {uuid:?}, not {:?}",
                    found.path, was.uuid
                ));
            }
        }
        paths.insert(Which::Part(part.path.clone()), found.path.clone());
    }
    if let Some(left) = kernel.iter().zip(&taken).find(|(_, taken)| !**taken) {
        return Err(format!(
            "{} is on {disk}, which the layout does not leave there",
            left.0.path
        ));
    }
    Ok(paths)
}

/// The command that makes `fs` on `device`, labelled `label` (none when it
/// is empty), as Partitions showed it; `None` for `bios_grub`, which holds
/// no filesystem. Each tool is given one force flag, as the easy path's:
/// enough to replace an old filesystem without a question nobody can
/// answer, not enough to format a device that is mounted.
fn make(fs: Fs, label: &str, device: &str) -> Option<Command> {
    let with = |first: &[&'static str], flag: &'static str| {
        let mut args: Vec<&str> = first.to_vec();
        if !label.is_empty() {
            args.extend([flag, label]);
        }
        args.push(device);
        args
    };
    Some(match fs {
        Fs::Ext4 => Command::new("mkfs.ext4", &with(&["-F"], "-L")),
        Fs::Btrfs => Command::new("mkfs.btrfs", &with(&["-f"], "-L")),
        Fs::Xfs => Command::new("mkfs.xfs", &with(&["-f"], "-L")),
        Fs::Fat32 => Command::new("mkfs.fat", &with(&["-F", "32"], "-n")),
        Fs::LinuxSwap => Command::new("mkswap", &with(&[], "-L")),
        Fs::BiosGrub => return None,
    })
}

/// **Phase C as commands**: for every partition made or formatted, disk by
/// disk in path order and by where each starts, found at its path now
/// (`paths`, from [`read_back`]): its signatures wiped (an old filesystem
/// can leave one where a new one writes nothing, as NTFS keeps a backup at
/// its end), then its filesystem made. A `bios_grub` partition is only
/// wiped.
pub fn format_commands(layout: &[DiskLayout], paths: &Paths) -> Result<Vec<Planned>, String> {
    let mut disks: Vec<&DiskLayout> = layout.iter().collect();
    disks.sort_by(|one, other| one.disk.path.cmp(&other.disk.path));
    let mut planned = Vec::new();
    for one in disks {
        let formatted = one.parts.iter().filter_map(|part| match &part.fate {
            Fate::Format { fs, label, .. } => {
                Some((part.start, Which::Part(part.path.clone()), *fs, label))
            }
            _ => None,
        });
        let made = one.create.iter().map(|made| {
            let which = Which::Made {
                disk: one.disk.path.clone(),
                start: made.start,
            };
            (made.start, which, made.fs, &made.label)
        });
        let mut targets: Vec<_> = formatted.chain(made).collect();
        targets.sort_by_key(|(start, ..)| *start);
        for (_, which, fs, label) in targets {
            let device = paths
                .of(&which)
                .ok_or_else(|| format!("{which:?} was not found again"))?;
            planned.push(Planned::Run {
                act: Act::WipeSignatures,
                device: device.to_string(),
                command: Command::new("wipefs", &["--all", device]),
            });
            if let Some(command) = make(fs, label, device) {
                planned.push(Planned::Run {
                    act: Act::Make,
                    device: device.to_string(),
                    command,
                });
            }
        }
    }
    Ok(planned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::GIB;
    use crate::worker::partition::fixtures::{disk_gpt_windows, disk_msdos_extended, on_disk};

    fn kept(disk: crate::model::disks::Disk) -> DiskLayout {
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

    #[test]
    fn a_table_is_the_reviewed_one_or_nothing_is_written() {
        for sector in [512, 4096] {
            let one = kept(disk_gpt_windows(sector));
            let read = on_disk(&one.disk);
            assert_eq!(check_table(&one, Some(&read)), Ok(()));
            assert!(check_table(&one, None).is_err(), "the table is gone");
            let mut other = read.clone();
            other.table = Table::Msdos;
            assert!(check_table(&one, Some(&other)).is_err(), "another kind");
            let mut other = read.clone();
            other.sector = Some(if sector == 512 { 4096 } else { 512 });
            assert!(check_table(&one, Some(&other)).is_err(), "another sector");
            let mut other = read.clone();
            other.partitions[2].size -= 1;
            assert!(check_table(&one, Some(&other)).is_err(), "another size");
            let mut other = read.clone();
            other.partitions[2].start += 1;
            assert!(check_table(&one, Some(&other)).is_err(), "another start");
            let mut other = read.clone();
            other.partitions[2].number = 9;
            assert!(check_table(&one, Some(&other)).is_err(), "another number");
            let mut other = read.clone();
            other.partitions[2].part_type = "0FC63DAF-8483-4772-8E79-3D69D8477DE4".into();
            assert!(check_table(&one, Some(&other)).is_err(), "another type");
            let mut other = read.clone();
            other.partitions.pop();
            assert!(check_table(&one, Some(&other)).is_err(), "one gone");
            // sfdisk's spelling of the same types is the same table.
            let mut spelt = read.clone();
            for entry in &mut spelt.partitions {
                entry.part_type = entry.part_type.to_ascii_uppercase();
            }
            assert_eq!(check_table(&one, Some(&spelt)), Ok(()));
            // A disk with no table must still have none.
            let empty = kept(crate::worker::partition::fixtures::disk_empty(sector));
            assert_eq!(check_table(&empty, None), Ok(()));
            assert!(check_table(&empty, Some(&read)).is_err());
        }
    }

    /// **The boot flag is as it was reviewed, or nothing is written**: on an
    /// msdos disk, a partition marked active now that was reviewed
    /// unmarked, or unmarked now that was reviewed marked, is a table that
    /// is not the reviewed one — for step 8 sets the flag only where
    /// nothing standing is marked, as the rule set judged the plan from
    /// what was reviewed (`layout::activates`).
    #[test]
    fn the_boot_flag_is_the_reviewed_one_or_nothing_is_written() {
        for sector in [512, 4096] {
            let mut disk = crate::worker::partition::fixtures::disk_msdos(sector);
            disk.partitions[0].bootable = true;
            let one = kept(disk);
            let read = on_disk(&one.disk);
            assert!(read.partitions[0].bootable && !read.partitions[1].bootable);
            assert_eq!(check_table(&one, Some(&read)), Ok(()));
            let mut cleared = read.clone();
            cleared.partitions[0].bootable = false;
            assert!(check_table(&one, Some(&cleared)).is_err(), "unmarked now");
            let mut moved = cleared.clone();
            moved.partitions[1].bootable = true;
            assert!(check_table(&one, Some(&moved)).is_err(), "marked elsewhere");
            // Reviewed with nothing marked, a mark now refuses as well.
            let unmarked = kept(crate::worker::partition::fixtures::disk_msdos(sector));
            assert_eq!(check_table(&unmarked, Some(&cleared)), Ok(()));
            assert!(check_table(&unmarked, Some(&read)).is_err(), "marked now");
        }
    }

    #[test]
    fn an_extended_frame_holds_its_logical_partitions_and_types_read_as_sfdisk_writes_them() {
        let one = kept(disk_msdos_extended(512));
        let mut read = on_disk(&one.disk);
        for entry in &mut read.partitions {
            entry.part_type = entry.part_type.trim_start_matches("0x").to_string();
            entry.part_type = entry.part_type.trim_start_matches('0').to_string();
        }
        assert_eq!(check_table(&one, Some(&read)), Ok(()));
        // A frame reaching further than its last logical partition is the
        // same frame; one that stops short of it is not.
        let mut longer = read.clone();
        longer.partitions[1].size += 2048;
        assert_eq!(check_table(&one, Some(&longer)), Ok(()));
        let mut shorter = read.clone();
        shorter.partitions[1].size -= 1;
        assert!(check_table(&one, Some(&shorter)).is_err());
    }

    #[test]
    fn names_and_types_are_for_what_a_partition_is_used_as() {
        let uefi = Context {
            firmware: Firmware::Uefi,
            efi_used: true,
        };
        let alone = Context {
            efi_used: false,
            ..uefi
        };
        let bios = Context {
            firmware: Firmware::Bios,
            efi_used: false,
        };
        let other = MountPoint::Other("/data".into());
        let cases = [
            (uefi, Fs::Fat32, Some(&MountPoint::Efi), "EFI system", "ef"),
            (uefi, Fs::Fat32, Some(&MountPoint::Boot), "Linux boot", "ea"),
            (
                alone,
                Fs::Fat32,
                Some(&MountPoint::Boot),
                "EFI system",
                "ef",
            ),
            (
                bios,
                Fs::Fat32,
                Some(&MountPoint::Boot),
                "Linux filesystem",
                "0c",
            ),
            (bios, Fs::BiosGrub, None, "BIOS boot", "83"),
            (uefi, Fs::Ext4, Some(&MountPoint::Root), "Linexin", "83"),
            (uefi, Fs::Xfs, Some(&MountPoint::Home), "Home", "83"),
            (
                uefi,
                Fs::LinuxSwap,
                Some(&MountPoint::Swap),
                "Linux swap",
                "82",
            ),
            (uefi, Fs::LinuxSwap, None, "Linux swap", "82"),
            (uefi, Fs::Btrfs, Some(&other), "Linux filesystem", "83"),
            (uefi, Fs::Ext4, None, "Linux filesystem", "83"),
        ];
        for (ctx, fs, at, name, mbr) in cases {
            assert_eq!(ctx.name(fs, at), name, "{fs:?} at {at:?}");
            assert_eq!(ctx.part_type(Table::Msdos, fs, at), mbr, "{fs:?} at {at:?}");
            let name = ctx.name(fs, at);
            assert!(!name.contains(['"', '/', '\\', ',', '\n']), "{name}");
        }
        assert_eq!(
            uefi.part_type(Table::Gpt, Fs::Ext4, Some(&MountPoint::Root)),
            "4f68bce3-e8cd-4db1-96e7-fbcaf984b709"
        );
        assert_eq!(
            uefi.part_type(Table::Gpt, Fs::Fat32, Some(&MountPoint::Boot)),
            "bc13c2ff-59e6-4262-a352-b275fd6f7172"
        );
    }

    #[test]
    fn a_disk_only_mounted_from_is_given_nothing() {
        let mut one = kept(disk_gpt_windows(512));
        one.parts[2].fate = Fate::Keep {
            mount: Some(MountPoint::Other("/windows".into())),
        };
        let ctx = Context::new(std::slice::from_ref(&one), Firmware::Uefi);
        assert!(!writes_to(&one));
        let read = on_disk(&one.disk);
        assert_eq!(
            table_commands(ctx, &one, Some(&read), "/dev/vdx"),
            Ok(vec![])
        );
        // Without its table read it is given nothing either: it is never
        // asked for one.
        assert_eq!(table_commands(ctx, &one, None, "/dev/vdx"), Ok(vec![]));
        // But a disk written to is never written from a table that is not
        // the reviewed one.
        one.parts[2].fate = Fate::Delete;
        let mut other = read.clone();
        other.partitions[2].start += 8;
        assert!(table_commands(ctx, &one, Some(&other), "/dev/vdx").is_err());
        assert!(table_commands(ctx, &one, None, "/dev/vdx").is_err());
    }

    fn kernel(path: &str, number: u32, start: u64, size: u64) -> Kernel {
        Kernel {
            path: path.into(),
            number,
            start,
            size,
        }
    }

    #[test]
    fn the_disk_read_back_maps_each_partition_to_its_path_now() {
        let disk = disk_gpt_windows(512);
        let mut one = kept(disk.clone());
        one.disk.partitions[2].uuid = "0A1B2C3D4E5F6A7B".into();
        let (p3, p4) = (&disk.partitions[2], &disk.partitions[3]);
        let to = 20 * GIB;
        one.parts[2].fate = Fate::Shrink {
            to,
            mount: Some(MountPoint::Other("/windows".into())),
        };
        let made_start = p3.start + to;
        one.create.push(layout::Create {
            start: made_start,
            size: 4 * GIB,
            fs: Fs::Ext4,
            label: String::new(),
            mount: None,
        });
        let good = vec![
            kernel(
                "/dev/vdx1",
                1,
                disk.partitions[0].start,
                disk.partitions[0].size,
            ),
            kernel(
                "/dev/vdx2",
                2,
                disk.partitions[1].start,
                disk.partitions[1].size,
            ),
            kernel("/dev/vdx3", 3, p3.start, to),
            kernel("/dev/vdx4", 4, p4.start, p4.size),
            kernel("/dev/vdx5", 5, made_start, 4 * GIB),
        ];
        let uuid = |path: &str| (path == "/dev/vdx3").then(|| "0a1b2c3d4e5f6a7b".to_string());
        let paths = read_back(&one, &good, &uuid).unwrap();
        assert_eq!(
            paths.of(&Which::Part("/dev/vdx3".into())),
            Some("/dev/vdx3")
        );
        let made = Which::Made {
            disk: "/dev/vdx".into(),
            start: made_start,
        };
        assert_eq!(paths.of(&made), Some("/dev/vdx5"));
        assert_eq!(paths.which("/dev/vdx5"), Some(&made));

        let refused = |kernel: Vec<Kernel>, why: &str| {
            assert!(read_back(&one, &kernel, &uuid).is_err(), "{why}");
        };
        let mut missing = good.clone();
        missing.pop();
        refused(missing, "the partition made is missing");
        let mut unshrunk = good.clone();
        unshrunk[2].size = p3.size;
        refused(unshrunk, "the shrunk entry is at its old size");
        let mut renumbered = good.clone();
        renumbered[3].number = 6;
        refused(renumbered, "a kept partition was renumbered");
        let mut stray = good.clone();
        stray.push(kernel("/dev/vdx6", 6, 64000 * 1024 * 1024, GIB));
        refused(stray, "something is there that the layout does not leave");
        let other = |path: &str| (path == "/dev/vdx3").then(|| "FFFF".to_string());
        assert!(
            read_back(&one, &good, &other).is_err(),
            "another filesystem"
        );
        assert!(read_back(&one, &good, &|_| None).is_err(), "no filesystem");

        // A partition deleted must be gone.
        let mut deleting = one.clone();
        deleting.parts[3].fate = Fate::Delete;
        assert!(read_back(&deleting, &good, &uuid).is_err());
        let gone: Vec<Kernel> = good.iter().filter(|k| k.number != 4).cloned().collect();
        assert!(read_back(&deleting, &gone, &uuid).is_ok());
    }

    #[test]
    fn an_extended_stub_is_found_where_it_was_reviewed() {
        let disk = disk_msdos_extended(512);
        let one = kept(disk.clone());
        let found: Vec<Kernel> = disk
            .partitions
            .iter()
            .map(|p| kernel(&p.path, p.number, p.start, p.size))
            .collect();
        let paths = read_back(&one, &found, &|_| None).unwrap();
        assert_eq!(paths, Paths::as_reviewed(&one));
    }

    #[test]
    fn a_label_is_given_only_where_there_is_one_and_bios_grub_only_wiped() {
        assert_eq!(
            make(Fs::Ext4, "", "/dev/vdx2").unwrap().args,
            ["-F", "/dev/vdx2"]
        );
        assert_eq!(
            make(Fs::Fat32, "EFI", "/dev/vdx1").unwrap().args,
            ["-F", "32", "-n", "EFI", "/dev/vdx1"]
        );
        assert_eq!(
            make(Fs::LinuxSwap, "swap one", "/dev/vdx3").unwrap().args,
            ["-L", "swap one", "/dev/vdx3"]
        );
        assert_eq!(make(Fs::BiosGrub, "", "/dev/vdx4"), None);
    }
}
