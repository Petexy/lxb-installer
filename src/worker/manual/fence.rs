//! The Fence: what a destructive command of a hand-made layout may be given
//! at all (§7.0, Unit Q, "The worker" 7). Before every command that writes,
//! its device must be one of:
//!
//! - a disk the layout writes to (a disk only mounted from is never
//!   written, and so is never one), and a whole disk, not a partition;
//! - a partition the person agreed to have destroyed, found by its disk,
//!   its start and its size as they are when the command runs — a
//!   filesystem to be shrunk at the size it was reviewed at, a partition to
//!   be formatted at the size its entry was given — never by its path;
//! - a partition this run made and read back, at the path, start and size
//!   it was read back at;
//!
//! and its disk, as the kernel says (`/sys/class/block/<name>`, the link's
//! parent, `boot::parent_disk`), a disk the layout writes to. Anything else
//! stops the run with `Internal` before the command starts. It is built
//! from the consent itself (`layout::destroys`), which the worker has
//! already held equal to what the person agreed to, so a bug anywhere in
//! the plan's commands — a number, a path, a partition taken for another —
//! meets it before a disk does.
//!
//! **What is let through is kept by the kind of command as well as by the
//! place** ([`Act`]): a partition agreed to be shrunk is given only its
//! filesystem's shrink (and its entry, on its disk, only the new size
//! agreed), never a wipe or a filesystem made; a partition agreed to be
//! formatted, or made by this run, only its signatures wiped and its
//! filesystem made, at its new entry; a whole disk's signatures wiped and a
//! new table written only where that new table was agreed to, and the
//! entries of a disk that keeps its table changed only there — an entry
//! given a new size (`sfdisk -N`) only to the size agreed for the partition
//! that starts where its script says.
//!
//! **And by what the command actually runs** ([`Fence::check_planned`]):
//! the program and every flag of the command built, exactly as `plan`
//! builds the one its act names — never the act's tag alone — and every
//! partition number it gives sfdisk (`--delete`, `--part-type`,
//! `--part-label`, `-N`, `--activate`) the number the disk's own table
//! gives the place agreed: phase A's table, which the fence is handed,
//! for a partition deleted, retyped, renamed or given a new size; for the
//! boot flag, which may go on a partition this run made, the table read
//! just before it ([`Fence::written_table`]). That table still numbers
//! every partition that stays, since no deletion is let through that would
//! number one anew: a deletion names its partitions from the highest
//! number down, and on an msdos table a logical one only with every
//! logical one numbered after it, the extended one only with every logical
//! one; and nothing but a deletion names an extended partition. Nor does a
//! second deletion come after it to name a number it freed: a disk takes
//! one deletion, naming every partition agreed to be deleted there, each
//! once ([`Fence::let_through`], which the executor asks). And a new type
//! is one the rule set gives a partition it makes or formats on that
//! table, never an extended partition's frame.
//!
//! **A filesystem's shrink is judged by its shape, its place and its size**
//! (step Q9): exactly one of the commands `resize::Shrink` builds for phase
//! B.1 — `e2fsck -f -y`, `resize2fs <partition> <K>K`, or `ntfsresize
//! --no-progress-bar --size <bytes>` with its question answered `y` on
//! standard input, never `--force` — on a partition agreed to be shrunk, at
//! the size it was reviewed at (its entry is made smaller only after); by
//! the tool of the filesystem agreed (e2fsprogs for an ext4, ntfsresize for
//! an NTFS); and the size it is given inside the entry agreed and below the
//! filesystem's own size as phase A measured it ([`Fence::measured`]), which
//! must have been handed — at or above it, resize2fs and ntfsresize would
//! grow what is kept.
//!
//! **And the filesystem before its entry** (step Q9): the entry of a
//! partition agreed to be shrunk is made smaller (`sfdisk -N`, `ShrinkEntry`)
//! only once the executor has handed the fence this run's read-back of that
//! filesystem ([`Fence::filesystem_read_back`]) and it ends inside the new
//! entry by its tool's rule (`Tool::ends_inside`) — so were the executor ever
//! to run an entry before its filesystem, or past a shrink that left more
//! than its entry holds, the last wall stops it.
//!
//! What the kernel says is read through [`Sysfs`]; the tests give the fence
//! their own, and the machine's ([`Machine`]) refuses every test.
use super::plan::{Act, Paths, Planned};
use super::resize::Tool;
use crate::model::disks::{device_path_ok, Table};
use crate::model::layout::{self, Destroy, DiskLayout, Fs, MountPoint, Role, Which};
use crate::model::protocol::Failure;
use crate::worker::layout::Stop;
use crate::worker::partition::plan::is_extended;
use crate::worker::partition::script::Command;
use crate::worker::partition::table::OnDisk;
use crate::worker::system::boot::parent_disk;
use std::collections::{BTreeMap, BTreeSet};

/// What the kernel says of a block device, by its name (`sda2`).
pub trait Sysfs {
    /// Where `/sys/class/block/<name>` points (`../../devices/…/sda/sda2`),
    /// or `None` where the kernel has no such device.
    fn link(&self, name: &str) -> Option<String>;
    /// Where a partition starts and how long it is, in bytes; `None` for a
    /// whole disk, which has neither.
    fn place(&self, name: &str) -> Option<(u64, u64)>;
}

/// The machine's own `/sys/class/block`. In a test build it refuses before
/// anything is read: no test reads this computer's block devices, and a
/// test of the fence hands it a [`Sysfs`] of its own.
pub struct Machine;

/// sysfs counts a partition's start and size in 512-byte units, whatever
/// the disk's sector size.
const SYSFS_UNIT: u64 = 512;

impl Machine {
    fn read(name: &str, file: &str) -> Option<u64> {
        let text = std::fs::read_to_string(format!("/sys/class/block/{name}/{file}")).ok()?;
        text.trim().parse::<u64>().ok()?.checked_mul(SYSFS_UNIT)
    }
}

impl Sysfs for Machine {
    fn link(&self, name: &str) -> Option<String> {
        #[cfg(test)]
        crate::worker::run::fence::block_devices();
        let link = std::fs::read_link(format!("/sys/class/block/{name}")).ok()?;
        Some(link.to_string_lossy().into_owned())
    }

    fn place(&self, name: &str) -> Option<(u64, u64)> {
        #[cfg(test)]
        crate::worker::run::fence::block_devices();
        Some((Self::read(name, "start")?, Self::read(name, "size")?))
    }
}

/// Why a partition is let through, which decides what it may be given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Agreed {
    /// The person agreed to have its filesystem shrunk: only that.
    Shrink,
    /// The person agreed to have it formatted: its signatures wiped and a
    /// filesystem made.
    Format,
    /// This run made it: the same.
    Made,
}

impl Agreed {
    fn admits(self, act: Act) -> bool {
        match self {
            Agreed::Shrink => act == Act::ShrinkFilesystem,
            Agreed::Format | Agreed::Made => matches!(act, Act::WipeSignatures | Act::Make),
        }
    }

    fn words(self) -> &'static str {
        match self {
            Agreed::Shrink => "agreed only to be shrunk",
            Agreed::Format => "agreed only to be formatted",
            Agreed::Made => "made by this run, which is only formatted",
        }
    }
}

/// Whether `act` is done to a whole disk (every step of phase B but a
/// filesystem's shrink) rather than to a partition.
fn on_a_disk(act: Act) -> bool {
    !matches!(act, Act::ShrinkFilesystem | Act::WipeSignatures | Act::Make)
}

/// A partition a command may be given: on `disk`, at `start` and `size`
/// bytes, and — for one this run made — at `path`; and why.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Allowed {
    disk: String,
    start: u64,
    size: u64,
    path: Option<String>,
    agreed: Agreed,
}

/// An entry the person agreed to give a new size, on `disk`: the partition
/// that starts at `start`, `size` bytes long once `act` — made smaller, or
/// grown — is done.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    disk: String,
    start: u64,
    size: u64,
    act: Act,
}

/// A disk the layout writes to: whether it gets a new table, its sector
/// size as reviewed, which every script given it counts in, and — for a
/// table it keeps — that table as phase A read it, which numbers every
/// partition a command names.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Written {
    fresh: bool,
    sector: u64,
    table: Option<OnDisk>,
}

/// A filesystem the person agreed to have shrunk, on `disk` at `start`:
/// its entry's size after (`to`), the tool its filesystem is shrunk with
/// (none for a filesystem this version does not shrink), its own size as
/// phase A measured it, once handed ([`Fence::measured`]), and its size as
/// this run read it back, once handed ([`Fence::filesystem_read_back`]).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Shrunk {
    disk: String,
    start: u64,
    to: u64,
    tool: Option<Tool>,
    filesystem: Option<u64>,
    read_back: Option<u64>,
}

/// Everything a destructive command of one layout may be given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fence {
    /// The disks the layout writes to, by path.
    disks: BTreeMap<String, Written>,
    allowed: Vec<Allowed>,
    entries: Vec<Entry>,
    /// Each partition the person agreed to have deleted: its disk and
    /// where it starts, in bytes.
    deleted: Vec<(String, u64)>,
    /// Where the boot flag may go: the start of each partition the layout
    /// uses as `/` or `/boot` — formatted, made, or (an EFI system
    /// partition at `/boot`) kept — by disk.
    flags: Vec<(String, u64)>,
    /// Each disk's table read once its entries are changed (steps 1 to 7),
    /// which numbers a partition the run made: the boot flag's number is
    /// judged by it.
    after: BTreeMap<String, OnDisk>,
    /// The disks a deletion has been let through on: one deletion a disk,
    /// naming every partition agreed to be deleted there, so no second one
    /// can name a number the first has freed — a number phase A's table
    /// still gives the partition gone, which a partition the run made since
    /// may now hold.
    deleted_on: BTreeSet<String>,
    /// Each filesystem agreed to be shrunk, what it may be shrunk to and
    /// with, and what phase A measured it at.
    shrinks: Vec<Shrunk>,
}

fn refused(detail: String) -> Stop {
    Stop::new(Failure::Internal, format!("the fence refused {detail}"))
}

impl Fence {
    /// The fence for `layout`, from what it destroys: the disks it writes
    /// to, and which of them get a new table; each filesystem it shrinks at
    /// the size it was reviewed at, and its entry's new size; each
    /// partition it formats at the size its entry is given, and that size
    /// where it is another; and each partition it deletes. A deletion or a
    /// new table is carried out on the disk, never on the partition.
    /// `tables` are the tables phase A read, by disk: a disk that keeps its
    /// table is numbered by its own.
    pub fn new(layout: &[DiskLayout], tables: &BTreeMap<String, OnDisk>) -> Self {
        let mut disks = BTreeMap::new();
        let mut allowed = Vec::new();
        let mut entries = Vec::new();
        let mut deleted = Vec::new();
        let mut flags = Vec::new();
        let mut shrinks = Vec::new();
        for one in layout {
            if !super::plan::writes_to(one) {
                continue;
            }
            let disk = one.disk.path.clone();
            let flagged = |mount: Option<&MountPoint>| {
                matches!(mount, Some(MountPoint::Root | MountPoint::Boot))
            };
            for part in &one.parts {
                if flagged(part.fate.mount()) {
                    flags.push((disk.clone(), part.start));
                }
            }
            for made in &one.create {
                if flagged(made.mount.as_ref()) {
                    flags.push((disk.clone(), made.start));
                }
            }
            let mut entry = |start: u64, from: u64, to: u64| {
                let act = match to.cmp(&from) {
                    std::cmp::Ordering::Less => Act::ShrinkEntry,
                    std::cmp::Ordering::Greater => Act::Grow,
                    std::cmp::Ordering::Equal => return,
                };
                entries.push(Entry {
                    disk: disk.clone(),
                    start,
                    size: to,
                    act,
                });
            };
            let mut fresh = false;
            for destroyed in layout::destroys(std::slice::from_ref(one)) {
                let (start, size, agreed) = match destroyed {
                    Destroy::Shrink {
                        start,
                        from,
                        to,
                        ref fs,
                        ..
                    } => {
                        entry(start, from, to);
                        shrinks.push(Shrunk {
                            disk: disk.clone(),
                            start,
                            to,
                            tool: Tool::of(fs),
                            filesystem: None,
                            read_back: None,
                        });
                        (start, from, Agreed::Shrink)
                    }
                    Destroy::Format {
                        start, size, to, ..
                    } => {
                        entry(start, size, to);
                        (start, to, Agreed::Format)
                    }
                    Destroy::Table { .. } => {
                        fresh = true;
                        continue;
                    }
                    Destroy::Delete { start, .. } => {
                        deleted.push((disk.clone(), start));
                        continue;
                    }
                };
                allowed.push(Allowed {
                    disk: disk.clone(),
                    start,
                    size,
                    path: None,
                    agreed,
                });
            }
            let sector = u64::from(one.disk.logical_sector);
            let table = if one.table.is_none() {
                tables.get(&disk).cloned()
            } else {
                None
            };
            disks.insert(
                disk,
                Written {
                    fresh,
                    sector,
                    table,
                },
            );
        }
        Self {
            disks,
            allowed,
            entries,
            deleted,
            flags,
            after: BTreeMap::new(),
            deleted_on: BTreeSet::new(),
            shrinks,
        }
    }

    /// The filesystem agreed to be shrunk on `disk` at `start` measured by
    /// phase A at `bytes`, just before anything is written: a shrink of it
    /// is let through only to less than that ([`check_planned`]), and none
    /// at all before this is said.
    ///
    /// [`check_planned`]: Self::check_planned
    pub fn measured(&mut self, disk: &str, start: u64, bytes: u64) {
        for shrunk in &mut self.shrinks {
            if shrunk.disk == disk && shrunk.start == start {
                shrunk.filesystem = Some(bytes);
            }
        }
    }

    /// The filesystem agreed to be shrunk on `disk` at `start` read back by
    /// this run at `bytes`, as its reader reads it (dumpe2fs's blocks times
    /// its block size; an NTFS volume's size by its boot sector), once phase
    /// B.1 has shrunk it: its entry is made smaller only once this is said,
    /// and only where that filesystem ends inside the new entry
    /// ([`check_planned`]).
    ///
    /// [`check_planned`]: Self::check_planned
    pub fn filesystem_read_back(&mut self, disk: &str, start: u64, bytes: u64) {
        for shrunk in &mut self.shrinks {
            if shrunk.disk == disk && shrunk.start == start {
                shrunk.read_back = Some(bytes);
            }
        }
    }

    /// `table`, read from `disk` once its entries are changed (phase B,
    /// steps 1 to 7): what the boot flag's number is judged by, since a
    /// partition this run made has none in phase A's table.
    pub fn written_table(&mut self, disk: &str, table: &OnDisk) {
        self.after.insert(disk.to_string(), table.clone());
    }

    /// Adds every partition `layout` makes, at the path it was read back at
    /// (`paths`, from `plan::read_back`), its start and its size.
    pub fn read_back(&mut self, layout: &[DiskLayout], paths: &Paths) {
        for one in layout {
            for made in &one.create {
                let which = Which::Made {
                    disk: one.disk.path.clone(),
                    start: made.start,
                };
                if let Some(path) = paths.of(&which) {
                    self.allowed.push(Allowed {
                        disk: one.disk.path.clone(),
                        start: made.start,
                        size: made.size,
                        path: Some(path.to_string()),
                        agreed: Agreed::Made,
                    });
                }
            }
        }
    }

    /// Whether a destructive command doing `act` may be given `device` now,
    /// as the kernel (`sysfs`) has it: `Internal` where it may not.
    pub fn check(&self, device: &str, act: Act, sysfs: &dyn Sysfs) -> Result<(), Stop> {
        if !device_path_ok(device) {
            return Err(refused(format!("{device:?}: not a device discovery names")));
        }
        let name = &device["/dev/".len()..];
        let link = sysfs
            .link(name)
            .ok_or_else(|| refused(format!("{device}: the kernel has no such device")))?;
        let parent = parent_disk(&link, name)
            .ok_or_else(|| refused(format!("{device}: the kernel places it at {link}")))?;
        let place = sysfs.place(name);
        if let Some(written) = self.disks.get(device) {
            if place.is_some() {
                return Err(refused(format!("{device}: a partition, not a whole disk")));
            }
            return match (act, written.fresh) {
                (act, _) if !on_a_disk(act) => Err(refused(format!(
                    "{device}: {act:?} is done to a partition, not to a whole disk"
                ))),
                (Act::Wipe | Act::Fresh, true) => Ok(()),
                (Act::Wipe | Act::Fresh, false) => Err(refused(format!(
                    "{device}: {act:?} only on a disk whose new table was agreed to, and none \
                     was for it"
                ))),
                (act, true) => Err(refused(format!(
                    "{device}: {act:?} on a disk that gets a new table, whose entries are \
                     never changed one by one"
                ))),
                (_, false) => Ok(()),
            };
        }
        let disk = format!("/dev/{parent}");
        if !self.disks.contains_key(&disk) {
            return Err(refused(format!(
                "{device}: on {disk}, which the layout does not write to"
            )));
        }
        if on_a_disk(act) {
            return Err(refused(format!(
                "{device}: {act:?} is done to a whole disk, not to a partition"
            )));
        }
        let (start, size) =
            place.ok_or_else(|| refused(format!("{device}: the kernel gives it no place")))?;
        let here: Vec<Agreed> = self
            .allowed
            .iter()
            .filter(|allowed| {
                allowed.disk == disk
                    && allowed.start == start
                    && allowed.size == size
                    && allowed.path.as_deref().is_none_or(|path| path == device)
            })
            .map(|allowed| allowed.agreed)
            .collect();
        match here.first() {
            None => Err(refused(format!(
                "{device}: bytes {start}+{size} of {disk} are nothing the person agreed to \
                 have destroyed, nor made by this run"
            ))),
            Some(_) if here.iter().any(|agreed| agreed.admits(act)) => Ok(()),
            Some(agreed) => Err(refused(format!(
                "{device}: {act:?} on a partition {}",
                agreed.words()
            ))),
        }
    }

    /// Whether an entry given a new size by `planned` (`sfdisk -N` on
    /// `disk`) is one the person agreed to: its script counts in the disk's
    /// own sectors, as its header says, and names one place — the start of
    /// a partition agreed to be made smaller (`ShrinkEntry`) or grown
    /// (`Grow`), at exactly the size agreed for it.
    fn entry_agreed(&self, disk: &str, act: Act, input: &str) -> Result<u64, Stop> {
        let sector = self.disks.get(disk).map_or(0, |written| written.sector);
        let mut lines = input.lines();
        let header = [lines.next(), lines.next()];
        let sector_size = format!("sector-size: {sector}");
        if header != [Some("unit: sectors"), Some(sector_size.as_str())] {
            return Err(refused(format!(
                "{disk}: {act:?}'s script does not say it counts in the disk's {sector}-byte \
                 sectors"
            )));
        }
        let place = |line: &str| -> Option<(u64, u64)> {
            let rest = line.strip_prefix("start=")?;
            let (start, size) = rest.split_once(", size=")?;
            Some((start.parse().ok()?, size.parse().ok()?))
        };
        let (start, size) = match (lines.next().and_then(place), lines.next()) {
            (Some((start, size)), None) => (start * sector, size * sector),
            _ => {
                return Err(refused(format!(
                    "{disk}: {act:?}'s script names no one entry's place"
                )))
            }
        };
        let agreed = self.entries.iter().any(|entry| {
            entry.disk == disk && entry.start == start && entry.size == size && entry.act == act
        });
        if !agreed {
            return Err(refused(format!(
                "{disk}: {act:?} to bytes {start}+{size}, which is no entry the person agreed \
                 to have given that size"
            )));
        }
        Ok(start)
    }

    /// Whether the entry starting at `start` on `disk` may be made smaller
    /// now: where the person agreed to have the filesystem there shrunk,
    /// only once this run has read that filesystem back
    /// ([`filesystem_read_back`]) and it ends inside the new entry by its
    /// tool's rule — the filesystem before its entry, held by the last wall
    /// as well as by the executor's order. An entry made smaller for a
    /// partition agreed to be formatted holds nothing that is kept.
    ///
    /// [`filesystem_read_back`]: Self::filesystem_read_back
    fn filesystem_inside(&self, disk: &str, start: u64) -> Result<(), Stop> {
        let Some(shrunk) = self
            .shrinks
            .iter()
            .find(|shrunk| shrunk.disk == disk && shrunk.start == start)
        else {
            return Ok(());
        };
        let Some(tool) = shrunk.tool else {
            return Err(refused(format!(
                "{disk}: the filesystem at byte {start} is none this version shrinks"
            )));
        };
        match shrunk.read_back {
            None => Err(refused(format!(
                "{disk}: the entry at byte {start} is made smaller only once this run has read \
                 its filesystem back inside it, and it has not"
            ))),
            Some(bytes) if !tool.ends_inside(bytes, shrunk.to) => Err(refused(format!(
                "{disk}: the filesystem at byte {start}, read back at {bytes} bytes, does not \
                 end inside its new entry of {}",
                shrunk.to
            ))),
            Some(_) => Ok(()),
        }
    }

    /// Whether each partition number `named` gives sfdisk on `disk`, for
    /// `act`, is the number the disk's own table gives a place agreed for
    /// it: for a deletion, a partition agreed to be deleted; for a new type
    /// or name, one agreed to be formatted; for a new size, the start its
    /// script gives (`start`); all by phase A's table. For the boot flag, a
    /// primary partition the layout uses as `/` or `/boot`, on an msdos
    /// table, by the table read just before it ([`written_table`]). Each number
    /// written as sfdisk writes one, and none twice.
    ///
    /// Phase A's table numbers a partition only while no deletion has
    /// numbered it anew, which on an msdos table one would: libfdisk
    /// deletes the numbers in the order given, numbers every logical
    /// partition after one deleted one lower, and takes every logical
    /// partition with the extended one. So a deletion names its numbers
    /// from the highest down, a logical partition only with every logical
    /// one numbered after it, the extended one only with every logical one
    /// — and then it numbers nothing that stays anew, neither for itself
    /// nor for any command after it — and no other command names an
    /// extended partition.
    ///
    /// [`written_table`]: Self::written_table
    fn numbers_agreed(
        &self,
        disk: &str,
        act: Act,
        named: &[&str],
        start: Option<u64>,
    ) -> Result<(), Stop> {
        let Some(written) = self.disks.get(disk) else {
            return Err(refused(format!("{disk}: not a disk the layout writes to")));
        };
        let table = match act {
            Act::Activate => self.after.get(disk),
            _ => written.table.as_ref(),
        };
        let Some(table) = table else {
            return Err(refused(format!(
                "{disk}: {act:?} names a partition, and no table of {disk} was read to number it"
            )));
        };
        let mut numbers: Vec<u32> = Vec::new();
        for text in named {
            let number = text
                .parse::<u32>()
                .ok()
                .filter(|number| *number > 0 && number.to_string() == *text)
                .ok_or_else(|| {
                    refused(format!(
                        "{disk}: {act:?} names {text:?}, not a partition number"
                    ))
                })?;
            if numbers.contains(&number) {
                return Err(refused(format!(
                    "{disk}: {act:?} names partition {number} twice"
                )));
            }
            // sfdisk deletes the numbers in the order given, so each is
            // still the number phase A's table gives it only if nothing
            // deleted before it was numbered lower.
            let lower = |previous: &&u32| act == Act::Delete && number > **previous;
            if let Some(previous) = numbers.last().filter(lower) {
                return Err(refused(format!(
                    "{disk}: {act:?} names partition {number} after {previous}: a deletion names \
                     its partitions from the highest number down"
                )));
            }
            numbers.push(number);
        }
        // On an msdos table libfdisk numbers every logical partition after
        // one deleted one lower, and takes every logical partition with the
        // extended one: a deletion that leaves one of them standing would
        // number it anew, under the number of another — in the deletion
        // itself, or for every command after it, which phase A's table
        // numbers. So a logical partition goes only with every logical one
        // numbered after it, the extended one only with every logical one,
        // and nothing but a deletion names the extended partition.
        let logical = |number: u32| table.table == Table::Msdos && number > 4;
        let frame = |number: u32, kind: &str| {
            table.table == Table::Msdos && number <= 4 && is_extended(kind)
        };
        let standing = |above: u32| {
            table
                .partitions
                .iter()
                .map(|entry| entry.number)
                .find(|other| logical(*other) && *other > above && !numbers.contains(other))
        };
        for &number in &numbers {
            let entry = table
                .partitions
                .iter()
                .find(|entry| entry.number == number)
                .ok_or_else(|| {
                    refused(format!(
                        "{disk}: {act:?} names partition {number}, which its table does not have"
                    ))
                })?;
            let extended = frame(number, &entry.part_type);
            match (act, standing(if extended { 4 } else { number })) {
                (Act::Delete, Some(other)) if extended => {
                    return Err(refused(format!(
                        "{disk}: {act:?} names the extended partition {number} but not logical \
                         partition {other}, which goes with it"
                    )))
                }
                (Act::Delete, Some(other)) if logical(number) => {
                    return Err(refused(format!(
                        "{disk}: {act:?} names logical partition {number} but not {other}, \
                         numbered after it, which its going would number anew"
                    )))
                }
                (Act::Delete, _) => {}
                _ if extended => {
                    return Err(refused(format!(
                        "{disk}: {act:?} names partition {number}, the extended partition, which \
                         only a deletion may name"
                    )))
                }
                _ => {}
            }
            let at = entry.start * written.sector;
            let (agreed, what) = match act {
                Act::Delete => (
                    self.deleted.contains(&(disk.to_string(), at)),
                    "no partition was agreed to be deleted",
                ),
                Act::Retype | Act::Rename => (
                    self.allowed.iter().any(|allowed| {
                        allowed.disk == disk
                            && allowed.start == at
                            && allowed.agreed == Agreed::Format
                    }),
                    "no partition was agreed to be formatted",
                ),
                Act::ShrinkEntry | Act::Grow => (
                    start == Some(at),
                    "its script does not give that entry a new size",
                ),
                Act::Activate => (
                    table.table == Table::Msdos
                        && number <= 4
                        && self.flags.contains(&(disk.to_string(), at)),
                    "no primary partition is used as / or /boot",
                ),
                _ => (false, "no partition is named by it"),
            };
            if !agreed {
                return Err(refused(format!(
                    "{disk}: {act:?} names partition {number}, which its table starts at byte \
                     {at}, where {what}"
                )));
            }
        }
        // A deletion names every partition agreed to be deleted on its
        // disk, each once: there is one deletion a disk (`check_planned`),
        // so none is left for a second to name.
        if act == Act::Delete {
            let left = self
                .deleted
                .iter()
                .filter(|(on, _)| on == disk)
                .find_map(|(_, start)| {
                    let entry = table
                        .partitions
                        .iter()
                        .find(|entry| entry.start * written.sector == *start);
                    match entry {
                        Some(entry) if numbers.contains(&entry.number) => None,
                        Some(entry) => Some(format!("partition {}", entry.number)),
                        None => Some(format!("the partition at byte {start}, not in its table")),
                    }
                });
            if let Some(left) = left {
                return Err(refused(format!(
                    "{disk}: {act:?} does not name {left}, agreed to be deleted: one deletion \
                     names every one"
                )));
            }
        }
        Ok(())
    }

    /// Whether `planned` may run now: its device, and every device its
    /// command line names, each by [`check`](Self::check) for what it does;
    /// its device among them, and no device in its script; the command
    /// exactly the one its act runs ([`runs`]: the program and every flag,
    /// so the act's tag never carries another); an entry given a new size
    /// only to the size agreed ([`entry_agreed`]), and one made smaller where
    /// a filesystem was agreed to be shrunk only once this run has read that
    /// filesystem back inside it ([`filesystem_inside`]); and every partition
    /// number it gives sfdisk the one the disk's own table gives a place
    /// agreed for that act ([`numbers_agreed`]). Every argument holding
    /// `/dev` is judged as a device: the only words a command is given that
    /// the person typed, a filesystem's label, hold no `/`
    /// (`layout::LABEL_REFUSES`), so none of them is ever taken for one.
    ///
    /// [`entry_agreed`]: Self::entry_agreed
    /// [`filesystem_inside`]: Self::filesystem_inside
    /// [`numbers_agreed`]: Self::numbers_agreed
    pub fn check_planned(&self, planned: &Planned, sysfs: &dyn Sysfs) -> Result<(), Stop> {
        let act = planned.act();
        self.check(planned.device(), act, sysfs)?;
        let Some(command) = planned.command() else {
            return Ok(());
        };
        let named: Vec<&String> = command
            .args
            .iter()
            .filter(|arg| arg.contains("/dev"))
            .collect();
        for arg in &named {
            self.check(arg, act, sysfs)?;
        }
        if !named.iter().any(|arg| arg.as_str() == planned.device()) {
            return Err(refused(format!(
                "{}: not on the command line of {}",
                planned.device(),
                command.program
            )));
        }
        if command
            .input
            .as_deref()
            .is_some_and(|input| input.contains("/dev"))
        {
            return Err(refused(format!(
                "a device named in {}'s script",
                command.program
            )));
        }
        let named = runs(act, planned.device(), command)
            .map_err(|why| refused(format!("{}: {why}", planned.device())))?;
        if act == Act::Retype {
            self.type_given(planned.device(), command)?;
        }
        if act == Act::ShrinkFilesystem {
            self.shrink_agreed(planned.device(), command, sysfs)?;
        }

        let start = if matches!(act, Act::ShrinkEntry | Act::Grow) {
            let input = command.input.as_deref().unwrap_or("");
            let start = self.entry_agreed(planned.device(), act, input)?;
            if act == Act::ShrinkEntry {
                self.filesystem_inside(planned.device(), start)?;
            }
            Some(start)
        } else {
            None
        };
        if !named.is_empty() {
            self.numbers_agreed(planned.device(), act, &named, start)?;
        }
        Ok(())
    }

    /// [`check_planned`](Self::check_planned), and what letting `planned`
    /// through leaves the fence holding: a deletion takes its disk's one
    /// deletion, so a second on that disk is refused — it could only name
    /// numbers the first freed, which phase A's table still gives the
    /// partitions gone and a partition this run made since may hold. The
    /// executor asks this before every command it runs; a test of one
    /// command's judgement asks `check_planned`.
    pub fn let_through(&mut self, planned: &Planned, sysfs: &dyn Sysfs) -> Result<(), Stop> {
        let deletion = planned.act() == Act::Delete;
        if deletion && self.deleted_on.contains(planned.device()) {
            return Err(refused(format!(
                "{}: a second deletion: one deletion a disk names every partition agreed to be \
                 deleted there",
                planned.device()
            )));
        }
        self.check_planned(planned, sysfs)?;
        if deletion {
            self.deleted_on.insert(planned.device().to_string());
        }
        Ok(())
    }

    /// Whether a filesystem's shrink, `command` on `device` (whose shape
    /// [`runs`] has judged), is the one agreed for the partition the kernel
    /// has there: run by the tool of the filesystem agreed — e2fsck and
    /// resize2fs for an ext4, ntfsresize for an NTFS — once phase A has
    /// measured that filesystem ([`measured`]), and given a size, where it
    /// is given one, of more than nothing, no more than the entry agreed and
    /// less than the filesystem's own: at or above it, resize2fs and
    /// ntfsresize would grow what is kept.
    ///
    /// [`measured`]: Self::measured
    fn shrink_agreed(
        &self,
        device: &str,
        command: &Command,
        sysfs: &dyn Sysfs,
    ) -> Result<(), Stop> {
        let name = &device["/dev/".len()..];
        let disk = sysfs
            .link(name)
            .and_then(|link| parent_disk(&link, name).map(|parent| format!("/dev/{parent}")));
        let start = sysfs.place(name).map(|(start, _)| start);
        let shrunk = self
            .shrinks
            .iter()
            .find(|shrunk| Some(&shrunk.disk) == disk.as_ref() && Some(shrunk.start) == start)
            .ok_or_else(|| {
                refused(format!(
                    "{device}: no filesystem was agreed to be shrunk there"
                ))
            })?;
        let tool = match command.program {
            "e2fsck" | "resize2fs" => Tool::Ext4,
            _ => Tool::Ntfs,
        };
        if shrunk.tool != Some(tool) {
            return Err(refused(format!(
                "{device}: {} shrinks no filesystem agreed to be shrunk there as {tool:?}",
                command.program
            )));
        }
        let Some(filesystem) = shrunk.filesystem else {
            return Err(refused(format!(
                "{device}: its filesystem was not measured before its shrink"
            )));
        };
        let given = match (command.program, command.args.as_slice()) {
            ("resize2fs", [_, size]) => size
                .strip_suffix('K')
                .and_then(|kib| kib.parse::<u64>().ok())
                .and_then(|kib| kib.checked_mul(1024)),
            ("ntfsresize", [.., flag, size, _]) if flag == "--size" => size.parse::<u64>().ok(),
            _ => return Ok(()),
        };
        match given {
            Some(size) if size > 0 && size <= shrunk.to && size < filesystem => Ok(()),
            Some(size) if size >= filesystem => Err(refused(format!(
                "{device}: {size} bytes is not less than its filesystem's own {filesystem}, \
                 which {} would grow or leave as it is",
                command.program
            ))),
            Some(size) => Err(refused(format!(
                "{device}: {size} bytes is no size inside the entry of {} agreed",
                shrunk.to
            ))),
            None => Err(refused(format!("{device}: no size its shrink is given"))),
        }
    }

    /// Whether the type a `--part-type` command gives (its last word) is one
    /// the rule set gives a partition made or formatted on the disk's table
    /// ([`given_type`]) — never an extended partition's frame, which would
    /// turn a formatted partition into a table of logical ones.
    fn type_given(&self, disk: &str, command: &Command) -> Result<(), Stop> {
        let kind = command.args.last().map_or("", String::as_str);
        let table = self
            .disks
            .get(disk)
            .and_then(|written| written.table.as_ref())
            .map(|table| table.table)
            .ok_or_else(|| refused(format!("{disk}: a new type on a table that was not read")))?;
        if table == Table::Msdos && is_extended(kind) {
            return Err(refused(format!(
                "{disk}: --part-type {kind} would make a partition an extended one's frame"
            )));
        }
        if !given_type(table, kind) {
            return Err(refused(format!(
                "{disk}: --part-type {kind}, a type the rule set never gives a partition it \
                 makes or formats on {table:?}"
            )));
        }
        Ok(())
    }
}

/// Whether `kind`, as sfdisk is given it (an MBR type without its `0x`),
/// is a type the rule set gives a partition made or formatted on `table`
/// (`layout::part_type`), for any filesystem, role and mount point there is.
/// An extended partition's (`05`, `0f`, `85`) never is, nor anything else
/// the rule set does not give.
fn given_type(table: Table, kind: &str) -> bool {
    let bare = |text: &str| {
        let text = text.trim().to_ascii_lowercase();
        text.strip_prefix("0x").unwrap_or(&text).to_string()
    };
    let filesystems = Fs::ALL.into_iter().chain([Fs::BiosGrub]);
    let roles = [Role::Esp, Role::LinuxBoot, Role::Plain];
    let mounts = [
        None,
        Some(MountPoint::Root),
        Some(MountPoint::Home),
        Some(MountPoint::Efi),
        Some(MountPoint::Boot),
        Some(MountPoint::Swap),
        Some(MountPoint::Other("/srv".into())),
    ];
    filesystems.into_iter().any(|fs| {
        roles.iter().any(|role| {
            mounts
                .iter()
                .any(|at| bare(layout::part_type(table, fs, at.as_ref(), *role)) == bare(kind))
        })
    })
}

/// The partition numbers `command` gives sfdisk, if it is exactly the
/// command `plan` builds for `act` on `device` — its program, every flag in
/// its place, a script where one is read and none where none is, a type, a
/// name or a label only where one stands — or why it is not. A filesystem's
/// shrink (`resize::Shrink`'s phase B.1) is `e2fsck -f -y`, `resize2fs` to
/// a size in KiB, or ntfsresize with no progress bar to a size in bytes, its
/// one question answered `y` on standard input and never `--force`; its
/// size is judged apart ([`Fence::shrink_agreed`]).
fn runs<'a>(act: Act, device: &str, command: &'a Command) -> Result<Vec<&'a str>, String> {
    let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
    let script = command.input.is_some();
    // A type, a GPT name or a label: something, which no tool could take for
    // a flag or a path.
    let value = |text: &str| !text.is_empty() && !text.starts_with('-') && !text.contains('/');
    // The device last, a label flagged with `flag` before it, or none.
    let labelled = |rest: &[&str], flag: &str| match rest {
        [last] => *last == device,
        [given, label, last] => *given == flag && value(label) && *last == device,
        _ => false,
    };
    // A size in bytes as the shrink's builder writes one: digits, no sign,
    // no leading zero, more than nothing.
    let bytes = |text: &str| {
        text.parse::<u64>()
            .is_ok_and(|n| n > 0 && n.to_string() == text)
    };
    let answered = command.input.as_deref() == Some("y\n");
    let numbers = match (act, command.program, args.as_slice(), script) {
        (Act::ShrinkFilesystem, "e2fsck", ["-f", "-y", part], false) if *part == device => {
            Some(vec![])
        }
        (Act::ShrinkFilesystem, "resize2fs", [part, size], false)
            if *part == device && size.strip_suffix('K').is_some_and(bytes) =>
        {
            Some(vec![])
        }
        (
            Act::ShrinkFilesystem,
            "ntfsresize",
            ["--no-progress-bar", "--size", size, part],
            true,
        ) if *part == device && bytes(size) && answered => Some(vec![]),
        (Act::ShrinkEntry | Act::Grow, "sfdisk", ["--lock", "-N", number, disk], true)
            if *disk == device =>
        {
            Some(vec![*number])
        }
        (Act::Delete, "sfdisk", ["--lock", "--delete", disk, numbers @ ..], false)
            if *disk == device && !numbers.is_empty() =>
        {
            Some(numbers.to_vec())
        }
        (Act::Wipe, "wipefs", ["--all", "--lock", disk], false) if *disk == device => Some(vec![]),
        (
            Act::Fresh,
            "sfdisk",
            ["--lock", "--wipe", "always", "--wipe-partitions", "always", disk],
            true,
        ) if *disk == device => Some(vec![]),
        (
            Act::Append,
            "sfdisk",
            ["--lock", "--append", "--wipe-partitions", "always", disk],
            true,
        ) if *disk == device => Some(vec![]),
        (Act::Retype, "sfdisk", ["--lock", "--part-type", disk, number, kind], false)
            if *disk == device && value(kind) =>
        {
            Some(vec![*number])
        }
        (Act::Rename, "sfdisk", ["--lock", "--part-label", disk, number, name], false)
            if *disk == device && value(name) =>
        {
            Some(vec![*number])
        }
        (Act::Activate, "sfdisk", ["--lock", "--activate", disk, number], false)
            if *disk == device =>
        {
            Some(vec![*number])
        }
        (Act::WipeSignatures, "wipefs", ["--all", part], false) if *part == device => Some(vec![]),
        (Act::Make, "mkfs.ext4", ["-F", rest @ ..], false)
        | (Act::Make, "mkfs.btrfs" | "mkfs.xfs", ["-f", rest @ ..], false)
        | (Act::Make, "mkswap", rest, false)
            if labelled(rest, "-L") =>
        {
            Some(vec![])
        }
        (Act::Make, "mkfs.fat", ["-F", "32", rest @ ..], false) if labelled(rest, "-n") => {
            Some(vec![])
        }
        _ => None,
    };
    numbers.ok_or_else(|| {
        let mut line = command.program.to_string();
        for arg in &args {
            line.push(' ');
            line.push_str(arg);
        }
        if script {
            line.push_str(" <<script");
        }
        format!("`{line}` is not the command {act:?} runs")
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::disks::{Disk, GIB, MIB};
    use crate::model::layout::{Create, Fate, Fs, MountPoint, Part};
    use crate::worker::manual::matrix::disk_msdos_logical;
    use crate::worker::partition::fixtures::{build, disk_gpt_windows, on_disk, Spec};
    use crate::worker::partition::script::Command;
    use crate::worker::run::fence as test_fence;
    use std::collections::BTreeMap;

    /// A kernel written by hand: each device's link and place.
    #[derive(Default)]
    pub(crate) struct Kernel {
        pub(crate) devices: BTreeMap<String, (String, Option<(u64, u64)>)>,
    }

    impl Kernel {
        pub(crate) fn disk(&mut self, name: &str) -> &mut Self {
            self.devices.insert(
                name.into(),
                (format!("../../devices/virtual/block/{name}"), None),
            );
            self
        }

        pub(crate) fn part(&mut self, disk: &str, name: &str, start: u64, size: u64) -> &mut Self {
            self.devices.insert(
                name.into(),
                (
                    format!("../../devices/virtual/block/{disk}/{name}"),
                    Some((start, size)),
                ),
            );
            self
        }
    }

    impl Sysfs for Kernel {
        fn link(&self, name: &str) -> Option<String> {
            self.devices.get(name).map(|(link, _)| link.clone())
        }

        fn place(&self, name: &str) -> Option<(u64, u64)> {
            self.devices.get(name).and_then(|(_, place)| *place)
        }
    }

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

    /// The Windows fixture with its Windows shrunk, its recovery formatted
    /// and resized, and a partition made after Windows; beside it a second
    /// disk only mounted from.
    fn layout() -> Vec<DiskLayout> {
        let mut windows = kept(disk_gpt_windows(512));
        windows.parts[2].fate = Fate::Shrink {
            to: 20 * GIB,
            mount: None,
        };
        windows.parts[3].fate = Fate::Format {
            fs: Fs::Ext4,
            label: String::new(),
            size: 500 * MIB,
            mount: None,
        };
        windows.create.push(Create {
            start: (117 + 20 * 1024) * MIB,
            size: 30 * GIB,
            fs: Fs::Ext4,
            label: String::new(),
            mount: Some(MountPoint::Root),
        });
        let mut other = kept(build(
            "/dev/vdy",
            512,
            Some(crate::model::disks::Table::Gpt),
            vec![Spec {
                number: 1,
                start: MIB,
                size: 10 * GIB,
                part_type: crate::worker::partition::fixtures::GPT_LINUX,
                content: crate::model::disks::Content::Linux,
                fs_type: "ext4",
            }],
            Vec::new(),
        ));
        other.parts[0].fate = Fate::Keep {
            mount: Some(MountPoint::Other("/data".into())),
        };
        vec![windows, other]
    }

    /// [`layout`], Windows' disk alone.
    fn layout_shrunk() -> Vec<DiskLayout> {
        let mut layout = layout();
        layout.truncate(1);
        layout
    }

    /// The kernel once phase B is done: Windows shrunk, recovery resized,
    /// the new partition made as vdx5.
    fn written() -> Kernel {
        let mut kernel = Kernel::default();
        kernel
            .disk("vdx")
            .part("vdx", "vdx1", MIB, 100 * MIB)
            .part("vdx", "vdx2", 101 * MIB, 16 * MIB)
            .part("vdx", "vdx3", 117 * MIB, 20 * GIB)
            .part("vdx", "vdx4", 64785 * MIB, 500 * MIB)
            .part("vdx", "vdx5", (117 + 20 * 1024) * MIB, 30 * GIB)
            .disk("vdy")
            .part("vdy", "vdy1", MIB, 10 * GIB)
            .disk("vdz")
            .part("vdz", "vdz1", MIB, 30 * GIB);
        kernel
    }

    /// The fence's refusal of `device` for `act`, in its words.
    fn refuses(fence: &Fence, device: &str, act: Act, kernel: &Kernel) -> String {
        let stop = fence.check(device, act, kernel).expect_err(device);
        assert_eq!(stop.failure, Failure::Internal, "{device}");
        assert!(
            stop.detail.starts_with("the fence refused"),
            "{}",
            stop.detail
        );
        stop.detail
    }

    /// The tables phase A would read of `layout`'s disks: each as reviewed.
    fn tables(layout: &[DiskLayout]) -> BTreeMap<String, OnDisk> {
        layout
            .iter()
            .filter(|one| one.disk.table.is_some())
            .map(|one| (one.disk.path.clone(), on_disk(&one.disk)))
            .collect()
    }

    /// Refused for being on a disk the layout does not write to.
    fn elsewhere(fence: &Fence, device: &str, kernel: &Kernel) {
        let words = refuses(fence, device, Act::Make, kernel);
        assert!(
            words.contains("which the layout does not write to"),
            "{words}"
        );
    }

    #[test]
    fn the_fence_lets_through_only_the_disks_written_and_what_was_agreed_to() {
        let layout = layout();
        let mut fence = Fence::new(&layout, &tables(&layout));
        let mut before = Kernel::default();
        before
            .disk("vdx")
            .part("vdx", "vdx3", 117 * MIB, 30 * GIB)
            .part("vdx", "vdx4", 64785 * MIB, 750 * MIB)
            .disk("vdy");
        // Before phase B: the disk written to, and Windows at the size it was
        // reviewed at — for its filesystem to be shrunk.
        fence.check("/dev/vdx", Act::ShrinkEntry, &before).unwrap();
        fence
            .check("/dev/vdx3", Act::ShrinkFilesystem, &before)
            .unwrap();
        // The recovery partition at its reviewed size is not what is to be
        // formatted: its entry is resized first.
        refuses(&fence, "/dev/vdx4", Act::Make, &before);
        // A disk only mounted from is never written.
        refuses(&fence, "/dev/vdy", Act::ShrinkEntry, &before);

        let after = written();
        fence.check("/dev/vdx", Act::Retype, &after).unwrap();
        fence.check("/dev/vdx4", Act::Make, &after).unwrap();
        // Windows shrunk is not formatted, nor shrunk again.
        refuses(&fence, "/dev/vdx3", Act::Make, &after);
        refuses(&fence, "/dev/vdx3", Act::ShrinkFilesystem, &after);
        // Kept partitions, on the disk written or not, are nothing agreed to.
        for kept in ["/dev/vdx1", "/dev/vdx2"] {
            refuses(&fence, kept, Act::Make, &after);
        }
        elsewhere(&fence, "/dev/vdy1", &after);
        // A disk the layout never named, and a partition on it.
        refuses(&fence, "/dev/vdz", Act::ShrinkEntry, &after);
        elsewhere(&fence, "/dev/vdz1", &after);
        // The partition made, until it is read back; then at its path.
        refuses(&fence, "/dev/vdx5", Act::Make, &after);
        let mut paths = Paths::default();
        paths.insert(
            Which::Made {
                disk: "/dev/vdx".into(),
                start: (117 + 20 * 1024) * MIB,
            },
            "/dev/vdx5".into(),
        );
        fence.read_back(&layout, &paths);
        fence.check("/dev/vdx5", Act::Make, &after).unwrap();
        // The same place under another name is not what was read back.
        let mut renamed = written();
        renamed.devices.remove("vdx5");
        renamed.part("vdx", "vdx6", (117 + 20 * 1024) * MIB, 30 * GIB);
        refuses(&fence, "/dev/vdx6", Act::Make, &renamed);
    }

    #[test]
    fn the_fence_judges_by_what_the_kernel_says_never_by_a_name() {
        let layout = layout();
        let mut fence = Fence::new(&layout, &tables(&layout));
        let mut paths = Paths::default();
        paths.insert(
            Which::Made {
                disk: "/dev/vdx".into(),
                start: (117 + 20 * 1024) * MIB,
            },
            "/dev/vdx5".into(),
        );
        fence.read_back(&layout, &paths);
        // The agreed place on another disk is not agreed to.
        let mut moved = written();
        moved.part("vdz", "vdx4", 64785 * MIB, 500 * MIB);
        elsewhere(&fence, "/dev/vdx4", &moved);
        // The agreed name at another place is not agreed to.
        let mut shifted = written();
        shifted.part("vdx", "vdx4", 64786 * MIB, 500 * MIB);
        refuses(&fence, "/dev/vdx4", Act::Make, &shifted);
        let mut longer = written();
        longer.part("vdx", "vdx4", 64785 * MIB, 750 * MIB);
        refuses(&fence, "/dev/vdx4", Act::Make, &longer);
        // A disk the kernel calls a partition, a partition with no place, a
        // device the kernel does not have, a link that does not end in it.
        let mut odd = written();
        odd.part("vdy", "vdx", 0, GIB);
        refuses(&fence, "/dev/vdx", Act::ShrinkEntry, &odd);
        let mut placeless = written();
        placeless.devices.insert(
            "vdx4".into(),
            ("../../devices/virtual/block/vdx/vdx4".into(), None),
        );
        refuses(&fence, "/dev/vdx4", Act::Make, &placeless);
        let mut gone = written();
        gone.devices.remove("vdx4");
        refuses(&fence, "/dev/vdx4", Act::Make, &gone);
        let mut elsewhere = written();
        elsewhere.devices.insert(
            "vdx4".into(),
            (
                "../../devices/virtual/block/vdx/sda4".into(),
                Some((64785 * MIB, 500 * MIB)),
            ),
        );
        refuses(&fence, "/dev/vdx4", Act::Make, &elsewhere);
        // What discovery never names is refused before the kernel is asked.
        for device in [
            "vdx4",
            "/dev/../dev/vdx4",
            "/dev/mapper/vdx4",
            "/dev/vdx4 ",
            "/dev/",
        ] {
            refuses(&fence, device, Act::Make, &written());
        }
    }

    #[test]
    fn a_command_is_let_through_only_for_its_own_device_and_no_other() {
        let layout = layout();
        let fence = Fence::new(&layout, &tables(&layout));
        let after = written();
        let run = |device: &str, args: &[&str], input: Option<&str>| Planned::Run {
            act: super::super::plan::Act::Make,
            device: device.into(),
            command: Command {
                program: "mkfs.ext4",
                args: args.iter().map(|a| a.to_string()).collect(),
                input: input.map(str::to_string),
            },
        };
        fence
            .check_planned(&run("/dev/vdx4", &["-F", "/dev/vdx4"], None), &after)
            .unwrap();
        let stop = |planned: Planned| {
            fence
                .check_planned(&planned, &after)
                .expect_err(&format!("{planned:?}"))
        };
        // Another device on its command line, agreed to or not.
        stop(run("/dev/vdx4", &["-F", "/dev/vdx4", "/dev/vdy1"], None));
        stop(run("/dev/vdx4", &["-F", "/dev/vdx1"], None));
        stop(run(
            "/dev/vdx4",
            &["-F", "--opt=/dev/vdy1", "/dev/vdx4"],
            None,
        ));
        // Its device not on its command line at all.
        stop(run("/dev/vdx4", &["-F", "/tmp/x"], None));
        // A device in its script.
        stop(run(
            "/dev/vdx",
            &["--lock", "/dev/vdx"],
            Some("device: /dev/vdy\n"),
        ));
        // A filesystem shrink is judged by its device.
        let shrink = |device: &str| Planned::ShrinkFilesystem {
            device: device.into(),
            fs: "ntfs".into(),
            from: 30 * GIB,
            to: 20 * GIB,
        };
        assert!(fence.check_planned(&shrink("/dev/vdx3"), &after).is_err());
        let mut before = written();
        before.part("vdx", "vdx3", 117 * MIB, 30 * GIB);
        fence.check_planned(&shrink("/dev/vdx3"), &before).unwrap();
    }

    /// The refusal of `planned`, in its words.
    fn stopped(fence: &Fence, planned: &Planned, kernel: &Kernel) -> String {
        let stop = fence
            .check_planned(planned, kernel)
            .expect_err(&planned.shown());
        assert_eq!(stop.failure, Failure::Internal);
        stop.detail
    }

    /// `act` on `device`, `program` given `args` and `input`.
    fn planned(
        act: Act,
        device: &str,
        program: &'static str,
        args: &[&str],
        input: Option<&str>,
    ) -> Planned {
        Planned::Run {
            act,
            device: device.into(),
            command: Command {
                program,
                args: args.iter().map(|arg| arg.to_string()).collect(),
                input: input.map(str::to_string),
            },
        }
    }

    /// **A filesystem's shrink is judged by its shape, its place and its
    /// size** (Unit Q, Q9): exactly the commands `resize::Shrink` builds for
    /// phase B.1, on the partition agreed to be shrunk at the size it was
    /// reviewed at, by the tool of the filesystem agreed, only once phase A
    /// has measured that filesystem, and to a size inside the entry agreed
    /// and below the filesystem's own — never at or above it, where
    /// resize2fs and ntfsresize would grow what is kept. Each refusal says
    /// why. Windows' NTFS on the Windows disk, and an ext4 on a disk of its
    /// own.
    #[test]
    fn the_fence_judges_a_filesystems_shrink_by_its_shape_place_and_size() {
        use crate::worker::manual::resize::Shrink;
        let says = |words: String, why: &str| assert!(words.contains(why), "{why}: {words}");
        let run = |device: &str, program: &'static str, args: &[&str], input: Option<&str>| {
            planned(Act::ShrinkFilesystem, device, program, args, input)
        };
        let size = (20 * GIB).to_string();

        // Windows, agreed to be shrunk to 20 GiB, its volume a sector short
        // of its 30 GiB partition.
        let layout = layout();
        let mut fence = Fence::new(&layout, &tables(&layout));
        let mut before = Kernel::default();
        before
            .disk("vdx")
            .part("vdx", "vdx3", 117 * MIB, 30 * GIB)
            .part("vdx", "vdx4", 64785 * MIB, 750 * MIB)
            .disk("vdy")
            .part("vdy", "vdy1", MIB, 10 * GIB);
        let asked = Planned::ShrinkFilesystem {
            device: "/dev/vdx3".into(),
            fs: "ntfs".into(),
            from: 30 * GIB,
            to: 20 * GIB,
        };
        let volume = 30 * GIB - 512;
        let shrink = Shrink::of(&asked, volume).unwrap();
        let (ntfsresize, _) = shrink.planned().remove(0);
        // Not before phase A has measured the filesystem.
        says(
            stopped(&fence, &ntfsresize, &before),
            "its filesystem was not measured before its shrink",
        );
        fence.measured("/dev/vdx", 117 * MIB, volume);
        fence.check_planned(&ntfsresize, &before).unwrap();
        // Its shape: no other flag, no --force, its answer exactly `y`, a
        // size in bytes, the partition last; the dry run is not a shrink.
        let not_its = "is not the command ShrinkFilesystem runs";
        for (args, input) in [
            (
                vec!["--force", "--no-progress-bar", "--size", &size, "/dev/vdx3"],
                Some("y\n"),
            ),
            (
                vec!["--no-progress-bar", "--size", &size, "/dev/vdx3"],
                None,
            ),
            (
                vec!["--no-progress-bar", "--size", &size, "/dev/vdx3"],
                Some("Y\n"),
            ),
            (
                vec!["--no-progress-bar", "--size", &size, "/dev/vdx3"],
                Some("y\ny\n"),
            ),
            (
                vec![
                    "--no-action",
                    "--no-progress-bar",
                    "--size",
                    &size,
                    "/dev/vdx3",
                ],
                None,
            ),
            (
                vec!["--no-progress-bar", "--size", "20G", "/dev/vdx3"],
                Some("y\n"),
            ),
            (
                vec!["--no-progress-bar", "--size", "021474836480", "/dev/vdx3"],
                Some("y\n"),
            ),
            (
                vec!["--no-progress-bar", "--size", "0", "/dev/vdx3"],
                Some("y\n"),
            ),
            (
                vec!["--no-progress-bar", "/dev/vdx3", "--size", &size],
                Some("y\n"),
            ),
            (vec!["--info", "--no-progress-bar", "/dev/vdx3"], None),
        ] {
            let wrong = run("/dev/vdx3", "ntfsresize", &args, input);
            says(stopped(&fence, &wrong, &before), not_its);
        }
        says(
            stopped(
                &fence,
                &run("/dev/vdx3", "ntfsfix", &["/dev/vdx3"], None),
                &before,
            ),
            not_its,
        );
        // Its tool: e2fsprogs never touch an NTFS agreed to be shrunk.
        for wrong in [
            run("/dev/vdx3", "e2fsck", &["-f", "-y", "/dev/vdx3"], None),
            run("/dev/vdx3", "resize2fs", &["/dev/vdx3", "20971520K"], None),
        ] {
            says(
                stopped(&fence, &wrong, &before),
                "shrinks no filesystem agreed to be shrunk there as Ext4",
            );
        }
        // Its size: inside the entry agreed, and below the volume.
        let sized = |bytes: u64| {
            let text = bytes.to_string();
            run(
                "/dev/vdx3",
                "ntfsresize",
                &["--no-progress-bar", "--size", &text, "/dev/vdx3"],
                Some("y\n"),
            )
        };
        fence
            .check_planned(&sized(20 * GIB - MIB), &before)
            .unwrap();
        says(
            stopped(&fence, &sized(20 * GIB + 512), &before),
            "is no size inside the entry of 21474836480 agreed",
        );
        // A volume measured smaller than the entry agreed: its tool would
        // grow it, so nothing at or above it.
        fence.measured("/dev/vdx", 117 * MIB, 20 * GIB);
        says(
            stopped(&fence, &sized(20 * GIB), &before),
            "is not less than its filesystem's own 21474836480",
        );
        fence
            .check_planned(&sized(20 * GIB - 512), &before)
            .unwrap();
        fence.measured("/dev/vdx", 117 * MIB, volume);
        // Its place: Windows at the size it was reviewed at, never once its
        // entry is made smaller; never another partition, agreed formatted,
        // kept, or on a disk only mounted from.
        let after = written();
        says(
            stopped(&fence, &ntfsresize, &after),
            "nothing the person agreed to have destroyed",
        );
        let elsewhere = |device: &str| {
            let text = size.clone();
            run(
                device,
                "ntfsresize",
                &["--no-progress-bar", "--size", &text, device],
                Some("y\n"),
            )
        };
        says(
            stopped(&fence, &elsewhere("/dev/vdx4"), &before),
            "nothing the person agreed to have destroyed",
        );
        says(
            stopped(&fence, &elsewhere("/dev/vdy1"), &before),
            "which the layout does not write to",
        );

        // An ext4 kept on a disk of its own, agreed to be shrunk to 20 GiB.
        let mut data = kept(crate::worker::partition::fixtures::disk_gpt_plain(512));
        data.parts[0].fate = Fate::Shrink {
            to: 20 * GIB,
            mount: None,
        };
        let layout = vec![data];
        let mut fence = Fence::new(&layout, &tables(&layout));
        let mut kernel = Kernel::default();
        kernel.disk("vdx").part("vdx", "vdx1", MIB, 30 * GIB);
        let asked = Planned::ShrinkFilesystem {
            device: "/dev/vdx1".into(),
            fs: "ext4".into(),
            from: 30 * GIB,
            to: 20 * GIB,
        };
        let shrink = Shrink::of(&asked, 30 * GIB).unwrap();
        let steps: Vec<Planned> = shrink.planned().into_iter().map(|(run, _)| run).collect();
        assert_eq!(steps.len(), 2);
        for step in &steps {
            says(
                stopped(&fence, step, &kernel),
                "its filesystem was not measured before its shrink",
            );
        }
        fence.measured("/dev/vdx", MIB, 30 * GIB);
        for step in &steps {
            fence.check_planned(step, &kernel).unwrap();
        }
        for (program, args) in [
            ("e2fsck", vec!["-f", "-n", "/dev/vdx1"]),
            (
                "e2fsck",
                vec!["-f", "-y", "-E", "journal_only", "/dev/vdx1"],
            ),
            ("e2fsck", vec!["-fy", "/dev/vdx1"]),
            ("resize2fs", vec!["-f", "/dev/vdx1", "20971520K"]),
            ("resize2fs", vec!["-M", "/dev/vdx1"]),
            ("resize2fs", vec!["/dev/vdx1", "20G"]),
            ("resize2fs", vec!["/dev/vdx1", "20971520"]),
            ("resize2fs", vec!["/dev/vdx1"]),
        ] {
            let wrong = run("/dev/vdx1", program, &args, None);
            says(stopped(&fence, &wrong, &kernel), not_its);
        }
        let resized = |kib: u64| {
            let text = format!("{kib}K");
            run("/dev/vdx1", "resize2fs", &["/dev/vdx1", &text], None)
        };
        says(
            stopped(&fence, &resized(20 * GIB / 1024 + 1), &kernel),
            "is no size inside the entry of 21474836480 agreed",
        );
        says(
            stopped(
                &fence,
                &run(
                    "/dev/vdx1",
                    "ntfsresize",
                    &["--no-progress-bar", "--size", &size, "/dev/vdx1"],
                    Some("y\n"),
                ),
                &kernel,
            ),
            "shrinks no filesystem agreed to be shrunk there as Ntfs",
        );
        // An ext4 already smaller than its partition: resize2fs to 25 GiB
        // would grow it to the entry, so only to less than its own size.
        let mut fence = Fence::new(&layout, &tables(&layout));
        fence.measured("/dev/vdx", MIB, 18 * GIB);
        says(
            stopped(&fence, &resized(18 * GIB / 1024), &kernel),
            "is not less than its filesystem's own 19327352832",
        );
        fence
            .check_planned(&resized(18 * GIB / 1024 - 4), &kernel)
            .unwrap();
    }

    /// **The last wall sees the filesystem before its entry** (Unit Q, Q9):
    /// the entry of a partition agreed to be shrunk is made smaller (`sfdisk
    /// -N`) only once this run's read-back of its filesystem is handed to the
    /// fence and ends inside the new entry by its tool's rule — Windows' NTFS
    /// volume a sector short of it at the most, where its backup boot sector
    /// lies; an ext4 at most the entry. Phase A's measurement is no
    /// read-back, and a read-back of another place counts for nothing; an
    /// entry made smaller for a partition agreed to be formatted holds
    /// nothing kept, and needs none. Each refusal says why.
    #[test]
    fn an_entry_is_made_smaller_only_once_its_filesystem_is_read_back_inside_it() {
        let says = |words: String, why: &str| assert!(words.contains(why), "{why}: {words}");
        let not_yet = "is made smaller only once this run has read its filesystem back inside \
                       it, and it has not";
        let past = |start: u64, bytes: u64| {
            format!(
                "/dev/vdx: the filesystem at byte {start}, read back at {bytes} bytes, does not \
                 end inside its new entry of {}",
                20 * GIB
            )
        };
        let entry = |number: &str, script: &str| {
            planned(
                Act::ShrinkEntry,
                "/dev/vdx",
                "sfdisk",
                &["--lock", "-N", number, "/dev/vdx"],
                Some(script),
            )
        };

        // Windows, agreed to be shrunk to 20 GiB.
        let layout = layout();
        let mut fence = Fence::new(&layout, &tables(&layout));
        let mut before = Kernel::default();
        before
            .disk("vdx")
            .part("vdx", "vdx3", 117 * MIB, 30 * GIB)
            .part("vdx", "vdx4", 64785 * MIB, 750 * MIB)
            .disk("vdy")
            .part("vdy", "vdy1", MIB, 10 * GIB);
        let windows = entry(
            "3",
            "unit: sectors\nsector-size: 512\nstart=239616, size=41943040\n",
        );
        says(
            stopped(&fence, &windows, &before),
            &format!("/dev/vdx: the entry at byte {} {not_yet}", 117 * MIB),
        );
        fence.measured("/dev/vdx", 117 * MIB, 30 * GIB - 512);
        says(stopped(&fence, &windows, &before), not_yet);
        fence.filesystem_read_back("/dev/vdx", 64785 * MIB, 400 * MIB);
        fence.filesystem_read_back("/dev/vdy", 117 * MIB, 20 * GIB - 4096);
        says(stopped(&fence, &windows, &before), not_yet);
        // Read back as large as it was, or as large as its new entry: its
        // backup boot sector would lie past the entry.
        for bytes in [30 * GIB - 512, 20 * GIB] {
            fence.filesystem_read_back("/dev/vdx", 117 * MIB, bytes);
            says(stopped(&fence, &windows, &before), &past(117 * MIB, bytes));
        }
        fence.filesystem_read_back("/dev/vdx", 117 * MIB, 20 * GIB - 512);
        fence.check_planned(&windows, &before).unwrap();
        // The recovery partition's entry, made smaller to be formatted.
        let recovery = entry(
            "4",
            "unit: sectors\nsector-size: 512\nstart=132679680, size=1024000\n",
        );
        Fence::new(&layout, &tables(&layout))
            .check_planned(&recovery, &before)
            .unwrap();

        // An ext4 on a disk of its own, agreed to be shrunk to 20 GiB.
        let mut data = kept(crate::worker::partition::fixtures::disk_gpt_plain(512));
        data.parts[0].fate = Fate::Shrink {
            to: 20 * GIB,
            mount: None,
        };
        let layout = vec![data];
        let mut fence = Fence::new(&layout, &tables(&layout));
        let mut kernel = Kernel::default();
        kernel.disk("vdx").part("vdx", "vdx1", MIB, 30 * GIB);
        let data = entry(
            "1",
            "unit: sectors\nsector-size: 512\nstart=2048, size=41943040\n",
        );
        says(stopped(&fence, &data, &kernel), not_yet);
        fence.filesystem_read_back("/dev/vdx", MIB, 20 * GIB + 4096);
        says(stopped(&fence, &data, &kernel), &past(MIB, 20 * GIB + 4096));
        fence.filesystem_read_back("/dev/vdx", MIB, 20 * GIB);
        fence.check_planned(&data, &kernel).unwrap();
    }

    /// What is let through is kept by the kind of command as well as by its
    /// place: a partition agreed to be shrunk only has its filesystem shrunk,
    /// never wiped or made a filesystem; one agreed to be formatted, or made
    /// by this run, only wiped and made one, never shrunk; a whole disk's
    /// signatures wiped and a new table written only where a new table was
    /// agreed to, and a kept table's entries changed only where the table is
    /// kept; a partition never given what is done to a disk, nor a disk what
    /// is done to a partition; an entry given a new size only to the size
    /// agreed for the partition starting where its script says, counted in
    /// the disk's own sectors. Each refusal says why.
    #[test]
    fn the_fence_admits_each_place_only_the_commands_agreed_for_it() {
        let layout = layout();
        let mut fence = Fence::new(&layout, &tables(&layout));
        let mut before = Kernel::default();
        before
            .disk("vdx")
            .part("vdx", "vdx3", 117 * MIB, 30 * GIB)
            .part("vdx", "vdx4", 64785 * MIB, 750 * MIB);
        let says = |words: String, why: &str| assert!(words.contains(why), "{words}");

        // Windows, agreed to be shrunk: its filesystem's shrink, nothing else.
        fence
            .check("/dev/vdx3", Act::ShrinkFilesystem, &before)
            .unwrap();
        for act in [Act::WipeSignatures, Act::Make] {
            says(
                refuses(&fence, "/dev/vdx3", act, &before),
                "on a partition agreed only to be shrunk",
            );
        }
        for program in ["wipefs", "mkfs.ext4"] {
            let act = if program == "wipefs" {
                Act::WipeSignatures
            } else {
                Act::Make
            };
            let wipe = planned(act, "/dev/vdx3", program, &["/dev/vdx3"], None);
            says(stopped(&fence, &wipe, &before), "agreed only to be shrunk");
        }
        // The recovery partition, agreed to be formatted: at its new entry
        // wiped and made a filesystem, never shrunk; at its old one, nothing.
        let after = written();
        fence
            .check("/dev/vdx4", Act::WipeSignatures, &after)
            .unwrap();
        fence.check("/dev/vdx4", Act::Make, &after).unwrap();
        says(
            refuses(&fence, "/dev/vdx4", Act::ShrinkFilesystem, &after),
            "on a partition agreed only to be formatted",
        );
        says(
            refuses(&fence, "/dev/vdx4", Act::Make, &before),
            "nothing the person agreed to have destroyed",
        );
        // The partition made, once read back: formatted, never shrunk.
        let mut paths = Paths::default();
        paths.insert(
            Which::Made {
                disk: "/dev/vdx".into(),
                start: (117 + 20 * 1024) * MIB,
            },
            "/dev/vdx5".into(),
        );
        fence.read_back(&layout, &paths);
        fence.check("/dev/vdx5", Act::Make, &after).unwrap();
        says(
            refuses(&fence, "/dev/vdx5", Act::ShrinkFilesystem, &after),
            "made by this run, which is only formatted",
        );

        // The disk keeps its table: its entries are changed, but its
        // signatures are never wiped whole, nor a new table written.
        for act in [
            Act::ShrinkEntry,
            Act::Delete,
            Act::Append,
            Act::Grow,
            Act::Retype,
            Act::Rename,
            Act::Activate,
        ] {
            fence.check("/dev/vdx", act, &before).unwrap();
        }
        for act in [Act::Wipe, Act::Fresh] {
            says(
                refuses(&fence, "/dev/vdx", act, &before),
                "only on a disk whose new table was agreed to",
            );
        }
        let wipe_whole = planned(
            Act::Wipe,
            "/dev/vdx",
            "wipefs",
            &["--all", "--lock", "/dev/vdx"],
            None,
        );
        says(
            stopped(&fence, &wipe_whole, &before),
            "Wipe only on a disk whose new table was agreed to",
        );
        // What is done to a partition is never done to a disk, nor the other
        // way round.
        says(
            refuses(&fence, "/dev/vdx", Act::Make, &before),
            "is done to a partition, not to a whole disk",
        );
        says(
            refuses(&fence, "/dev/vdx4", Act::Delete, &after),
            "is done to a whole disk, not to a partition",
        );

        // A disk given a new table: wiped whole and written afresh, its
        // entries never changed one by one.
        let mut blank = kept(build(
            "/dev/vdz",
            512,
            Some(crate::model::disks::Table::Gpt),
            vec![Spec {
                number: 1,
                start: MIB,
                size: 30 * GIB,
                part_type: crate::worker::partition::fixtures::GPT_LINUX,
                content: crate::model::disks::Content::Linux,
                fs_type: "ext4",
            }],
            Vec::new(),
        ));
        blank.parts[0].fate = Fate::Delete;
        blank.table = Some(crate::model::disks::Table::Gpt);
        let fresh = Fence::new(std::slice::from_ref(&blank), &tables(&[blank.clone()]));
        let mut vdz = Kernel::default();
        vdz.disk("vdz").part("vdz", "vdz1", MIB, 30 * GIB);
        fresh.check("/dev/vdz", Act::Wipe, &vdz).unwrap();
        fresh.check("/dev/vdz", Act::Fresh, &vdz).unwrap();
        for act in [Act::Delete, Act::ShrinkEntry, Act::Append] {
            says(
                refuses(&fresh, "/dev/vdz", act, &vdz),
                "on a disk that gets a new table",
            );
        }
        says(
            refuses(&fresh, "/dev/vdz1", Act::WipeSignatures, &vdz),
            "nothing the person agreed to have destroyed",
        );

        // An entry given a new size: only the size agreed, for the
        // partition starting where its script says, in the disk's sectors.
        let numbered = |act: Act, number: &str, script: &str| {
            planned(
                act,
                "/dev/vdx",
                "sfdisk",
                &["--lock", "-N", number, "/dev/vdx"],
                Some(script),
            )
        };
        let entry = |act: Act, script: &str| numbered(act, "3", script);
        let windows = "unit: sectors\nsector-size: 512\nstart=239616, size=41943040\n";
        let recovery = "unit: sectors\nsector-size: 512\nstart=132679680, size=1024000\n";
        // Windows' entry once its filesystem is read back inside it.
        fence.filesystem_read_back("/dev/vdx", 117 * MIB, 20 * GIB - 4096);
        fence
            .check_planned(&entry(Act::ShrinkEntry, windows), &before)
            .unwrap();
        fence
            .check_planned(&numbered(Act::ShrinkEntry, "4", recovery), &before)
            .unwrap();
        for (act, script, why) in [
            (
                Act::ShrinkEntry,
                "unit: sectors\nsector-size: 512\nstart=239616, size=52428800\n",
                "which is no entry the person agreed to have given that size",
            ),
            (
                Act::Grow,
                windows,
                "which is no entry the person agreed to have given that size",
            ),
            (
                Act::ShrinkEntry,
                "unit: sectors\nsector-size: 512\nstart=2048, size=204800\n",
                "which is no entry the person agreed to have given that size",
            ),
            (
                Act::ShrinkEntry,
                "start=239616, size=41943040\n",
                "does not say it counts in the disk's 512-byte sectors",
            ),
            (
                Act::ShrinkEntry,
                "unit: sectors\nsector-size: 4096\nstart=29952, size=5242880\n",
                "does not say it counts in the disk's 512-byte sectors",
            ),
            (
                Act::ShrinkEntry,
                "unit: sectors\nsector-size: 512\nstart=239616, size=41943040\nstart=1, size=1\n",
                "names no one entry's place",
            ),
        ] {
            says(stopped(&fence, &entry(act, script), &before), why);
        }
    }

    /// **The fence judges what a command actually runs, and every partition
    /// number it gives sfdisk** (Unit Q, Q5): Windows' disk keeping
    /// its table, Windows formatted at `/data` and its recovery partition
    /// deleted. A command is let through only as exactly the command its
    /// act names — the program and every flag in place, a type, a name or a
    /// label only where one stands — so an act's tag never carries another
    /// command (a deletion tagged a new type, an agreed one among them); and
    /// each number it names is the one phase A's own table gives a place
    /// agreed for that act: deleted for `--delete`, formatted for
    /// `--part-type` and `--part-label`, the start its script gives for
    /// `-N` — written as sfdisk writes a number, and none twice. Each
    /// refusal says why.
    #[test]
    fn the_fence_judges_what_a_command_runs_and_every_number_it_names() {
        let mut windows = kept(disk_gpt_windows(512));
        windows.parts[2].fate = Fate::Format {
            fs: Fs::Ext4,
            label: String::new(),
            size: 30 * GIB,
            mount: Some(MountPoint::Other("/data".into())),
        };
        windows.parts[3].fate = Fate::Delete;
        let layout = vec![windows];
        let fence = Fence::new(&layout, &tables(&layout));
        let mut kernel = Kernel::default();
        kernel
            .disk("vdx")
            .part("vdx", "vdx3", 117 * MIB, 30 * GIB)
            .part("vdx", "vdx4", 64785 * MIB, 750 * MIB);
        let linux = crate::worker::partition::fixtures::GPT_LINUX;
        let on = |act: Act, program: &'static str, args: &[&str]| {
            let device = if matches!(act, Act::WipeSignatures | Act::Make) {
                "/dev/vdx3"
            } else {
                "/dev/vdx"
            };
            planned(act, device, program, args, None)
        };
        // Exactly what each act runs, on exactly the places agreed.
        for good in [
            on(
                Act::Delete,
                "sfdisk",
                &["--lock", "--delete", "/dev/vdx", "4"],
            ),
            on(
                Act::Retype,
                "sfdisk",
                &["--lock", "--part-type", "/dev/vdx", "3", linux],
            ),
            on(
                Act::Rename,
                "sfdisk",
                &[
                    "--lock",
                    "--part-label",
                    "/dev/vdx",
                    "3",
                    "Linux filesystem",
                ],
            ),
            on(Act::WipeSignatures, "wipefs", &["--all", "/dev/vdx3"]),
            on(Act::Make, "mkfs.ext4", &["-F", "/dev/vdx3"]),
            on(Act::Make, "mkfs.ext4", &["-F", "-L", "Data", "/dev/vdx3"]),
            on(Act::Make, "mkfs.xfs", &["-f", "/dev/vdx3"]),
            on(
                Act::Make,
                "mkfs.fat",
                &["-F", "32", "-n", "DATA", "/dev/vdx3"],
            ),
            on(Act::Make, "mkswap", &["/dev/vdx3"]),
        ] {
            fence
                .check_planned(&good, &kernel)
                .unwrap_or_else(|stop| panic!("{}: {}", good.shown(), stop.detail));
        }
        let says = |planned: Planned, why: &str| {
            let words = stopped(&fence, &planned, &kernel);
            assert!(words.contains(why), "{}: {words}", planned.shown());
        };
        // An act's tag never carries another command — not even one agreed.
        let not = |act: Act| format!("is not the command {act:?} runs");
        for (act, program, args) in [
            (
                Act::Retype,
                "sfdisk",
                &["--lock", "--delete", "/dev/vdx", "4"][..],
            ),
            (
                Act::Activate,
                "sfdisk",
                &["--lock", "--delete", "/dev/vdx", "4"],
            ),
            (
                Act::Rename,
                "sfdisk",
                &["--lock", "--part-type", "/dev/vdx", "3", linux],
            ),
            (
                Act::Delete,
                "wipefs",
                &["--lock", "--delete", "/dev/vdx", "4"],
            ),
            // Every partition deleted, which `--delete` alone does.
            (Act::Delete, "sfdisk", &["--lock", "--delete", "/dev/vdx"]),
            (Act::Delete, "sfdisk", &["--delete", "/dev/vdx", "4"]),
            (
                Act::Retype,
                "sfdisk",
                &["--lock", "--part-type", "/dev/vdx", "3"],
            ),
            (
                Act::Retype,
                "sfdisk",
                &["--lock", "--part-type", "/dev/vdx", "3", "--wipe"],
            ),
            (
                Act::Rename,
                "sfdisk",
                &["--lock", "--part-label", "/dev/vdx", "3", "a/b"],
            ),
            (Act::Make, "mkfs.ext4", &["/dev/vdx3"]),
            (
                Act::Make,
                "mkfs.ext4",
                &["-F", "-E", "nodiscard", "/dev/vdx3"],
            ),
            (Act::Make, "mkfs.ext4", &["-F", "-L", "-x", "/dev/vdx3"]),
            (Act::Make, "mkfs.ntfs", &["-F", "/dev/vdx3"]),
            (
                Act::Make,
                "mkfs.fat",
                &["-F", "32", "-L", "DATA", "/dev/vdx3"],
            ),
            (
                Act::WipeSignatures,
                "wipefs",
                &["--all", "--force", "/dev/vdx3"],
            ),
        ] {
            says(on(act, program, args), &not(act));
        }
        // A command that reads a script is given one, and one that reads
        // none is given none.
        says(
            planned(
                Act::Delete,
                "/dev/vdx",
                "sfdisk",
                &["--lock", "--delete", "/dev/vdx", "4"],
                Some("unit: sectors\n"),
            ),
            &not(Act::Delete),
        );
        // Each number, by phase A's own table.
        let delete = |numbers: &[&str]| {
            let mut args = vec!["--lock", "--delete", "/dev/vdx"];
            args.extend(numbers);
            on(Act::Delete, "sfdisk", &args)
        };
        says(
            delete(&["3"]),
            "names partition 3, which its table starts at byte 122683392, where no partition \
             was agreed to be deleted",
        );
        says(delete(&["4", "4"]), "names partition 4 twice");
        says(delete(&["4", "3"]), "names partition 3");
        says(
            delete(&["9"]),
            "names partition 9, which its table does not have",
        );
        for odd in ["04", "+4", "0", "4x", ""] {
            says(delete(&[odd]), "not a partition number");
        }
        for (number, act, program, value) in [
            ("1", Act::Retype, "--part-type", linux),
            ("4", Act::Retype, "--part-type", linux),
            ("2", Act::Rename, "--part-label", "x"),
            ("4", Act::Rename, "--part-label", "x"),
        ] {
            says(
                on(
                    act,
                    "sfdisk",
                    &["--lock", program, "/dev/vdx", number, value],
                ),
                "where no partition was agreed to be formatted",
            );
        }
        // An entry given a new size: the number phase A's table gives the
        // start its script names (Windows' filesystem read back inside it).
        let shrunk = layout_shrunk();
        let mut fence = Fence::new(&shrunk, &tables(&shrunk));
        fence.filesystem_read_back("/dev/vdx", 117 * MIB, 20 * GIB - 4096);
        let mut before = Kernel::default();
        before.disk("vdx");
        let windows = "unit: sectors\nsector-size: 512\nstart=239616, size=41943040\n";
        let entry = |number: &str| {
            planned(
                Act::ShrinkEntry,
                "/dev/vdx",
                "sfdisk",
                &["--lock", "-N", number, "/dev/vdx"],
                Some(windows),
            )
        };
        fence.check_planned(&entry("3"), &before).unwrap();
        for number in ["4", "1"] {
            let words = stopped(&fence, &entry(number), &before);
            assert!(
                words.contains("where its script does not give that entry a new size"),
                "{words}"
            );
        }
        let words = stopped(&fence, &entry("03"), &before);
        assert!(words.contains("not a partition number"), "{words}");

        // **On an msdos table, no deletion numbers anew a partition that
        // stays** (step Q7). libfdisk deletes the numbers given in their
        // order; a logical partition deleted numbers every logical one after
        // it one lower, and the extended partition deleted takes every
        // logical one with it — so a number phase A's table gives an agreed
        // place could, by the time it is read, name another, in the deletion
        // itself or in a command after it. The old Linux disk: 1, the
        // extended partition 2 (`0f`), and inside it the logical partitions
        // 5, 6 and 7. A deletion names its numbers from the highest down; a
        // logical partition only with every logical one numbered after it;
        // the extended one only with every logical one; and nothing but a
        // deletion names the extended partition. Each layout refused here the
        // rule set refuses too (`LogicalOrder`, `Mismatch`): the fence
        // refuses its commands on its own.
        for sector in [512, 4096] {
            let bytes = u64::from(sector);
            let old = |fates: Vec<(u32, Fate)>| {
                let mut one = kept(disk_msdos_logical(sector));
                for (number, fate) in fates {
                    let part = one.parts.iter_mut().find(|part| part.number == number);
                    part.expect("the old Linux disk has it").fate = fate;
                }
                vec![one]
            };
            let mut vdx = Kernel::default();
            vdx.disk("vdx");
            let delete = |numbers: &[&str]| {
                let mut args = vec!["--lock", "--delete", "/dev/vdx"];
                args.extend(numbers);
                planned(Act::Delete, "/dev/vdx", "sfdisk", &args, None)
            };
            let lets = |layout: &[DiskLayout], planned: Planned| {
                Fence::new(layout, &tables(layout))
                    .check_planned(&planned, &vdx)
                    .unwrap_or_else(|stop| {
                        panic!("{sector}: {}: {}", planned.shown(), stop.detail)
                    });
            };
            let says = |layout: &[DiskLayout], planned: Planned, why: &str| {
                let words = stopped(&Fence::new(layout, &tables(layout)), &planned, &vdx);
                assert!(
                    words.contains(why),
                    "{sector}: {}: {words}",
                    planned.shown()
                );
            };
            let swap = |size: u64| Fate::Format {
                fs: Fs::LinuxSwap,
                label: String::new(),
                size,
                mount: Some(MountPoint::Swap),
            };
            // The last logical partition by number goes alone; two go from
            // the highest, never from the lowest.
            lets(&old(vec![(7, Fate::Delete)]), delete(&["7"]));
            let two = old(vec![(6, Fate::Delete), (7, Fate::Delete)]);
            lets(&two, delete(&["7", "6"]));
            says(
                &two,
                delete(&["6", "7"]),
                "names partition 7 after 6: a deletion names its partitions from the highest \
                 number down",
            );
            // 5 deleted beside 7 while 6, numbered after it, stays — the
            // root formatted and 6 grown over 7's place, so the plan's
            // `--delete 7 5` would leave `-N 6` and `--part-type 6` naming
            // what had been 7.
            let five = old(vec![
                (
                    1,
                    Fate::Format {
                        fs: Fs::Ext4,
                        label: String::new(),
                        size: 28 * GIB,
                        mount: Some(MountPoint::Root),
                    },
                ),
                (5, Fate::Delete),
                (6, swap((49156 - 36867) * MIB)),
                (7, Fate::Delete),
            ]);
            let not_six = "names logical partition 5 but not 6, numbered after it, which its \
                           going would number anew";
            says(&five, delete(&["7", "5"]), not_six);
            says(&old(vec![(5, Fate::Delete)]), delete(&["5"]), not_six);
            // The extended partition goes only with every logical one in it,
            // and after them.
            let all = old(vec![
                (2, Fate::Delete),
                (5, Fate::Delete),
                (6, Fate::Delete),
                (7, Fate::Delete),
            ]);
            lets(&all, delete(&["7", "6", "5", "2"]));
            says(
                &all,
                delete(&["2", "7", "6", "5"]),
                "names partition 7 after 2",
            );
            says(
                &all,
                delete(&["6", "5", "2"]),
                "names logical partition 6 but not 7",
            );
            says(
                &old(vec![(2, Fate::Delete)]),
                delete(&["2"]),
                "names the extended partition 2 but not logical partition 5, which goes with it",
            );
            // Nothing but a deletion names the extended partition — not even
            // as agreed: here it formatted as `/` and grown, which the rule
            // set refuses, so that every check but this one lets each
            // command through.
            let frame = old(vec![(
                2,
                Fate::Format {
                    fs: Fs::Ext4,
                    label: String::new(),
                    size: 20 * GIB,
                    mount: Some(MountPoint::Root),
                },
            )]);
            let mut fence = Fence::new(&frame, &tables(&frame));
            fence.written_table("/dev/vdx", &on_disk(&frame[0].disk));
            let grown = format!(
                "unit: sectors\nsector-size: {sector}\nstart={}, size={}\n",
                28673 * MIB / bytes,
                20 * GIB / bytes
            );
            let sfdisk = |act: Act, args: &[&str], input: Option<&str>| {
                planned(act, "/dev/vdx", "sfdisk", args, input)
            };
            for command in [
                sfdisk(
                    Act::Grow,
                    &["--lock", "-N", "2", "/dev/vdx"],
                    Some(grown.as_str()),
                ),
                sfdisk(
                    Act::Retype,
                    &["--lock", "--part-type", "/dev/vdx", "2", "83"],
                    None,
                ),
                sfdisk(
                    Act::Rename,
                    &["--lock", "--part-label", "/dev/vdx", "2", "Linexin"],
                    None,
                ),
                sfdisk(
                    Act::Activate,
                    &["--lock", "--activate", "/dev/vdx", "2"],
                    None,
                ),
            ] {
                let words = stopped(&fence, &command, &vdx);
                assert!(
                    words.contains(
                        "names partition 2, the extended partition, which only a deletion may name"
                    ),
                    "{sector}: {}: {words}",
                    command.shown()
                );
            }
        }
    }

    /// **A new type never makes a partition an extended one's frame**
    /// (Unit Q, Q7): `--part-type` gives a partition agreed to be formatted
    /// only a type the rule set gives a partition it makes or formats on that
    /// table (`layout::part_type`, every filesystem, role and mount point) —
    /// on msdos never `05`, `0f` or `85`, which would turn it into a table of
    /// logical partitions, nor anything else it never gives (Windows' `07`,
    /// its basic data GUID).
    #[test]
    fn a_new_type_is_one_the_rule_set_gives_never_an_extended_frame() {
        for sector in [512, 4096] {
            let mut old = kept(disk_msdos_logical(sector));
            old.parts[0].fate = Fate::Format {
                fs: Fs::Ext4,
                label: String::new(),
                size: 28 * GIB,
                mount: Some(MountPoint::Root),
            };
            let layout = vec![old];
            let fence = Fence::new(&layout, &tables(&layout));
            let mut vdx = Kernel::default();
            vdx.disk("vdx");
            let retype = |kind: &str| {
                planned(
                    Act::Retype,
                    "/dev/vdx",
                    "sfdisk",
                    &["--lock", "--part-type", "/dev/vdx", "1", kind],
                    None,
                )
            };
            for kind in ["83", "82", "ef", "ea", "0c"] {
                fence
                    .check_planned(&retype(kind), &vdx)
                    .unwrap_or_else(|stop| panic!("{sector}: {kind}: {}", stop.detail));
            }
            for kind in ["5", "05", "f", "0f", "F", "85", "0x0f"] {
                let words = stopped(&fence, &retype(kind), &vdx);
                assert!(
                    words.contains("would make a partition an extended one's frame"),
                    "{sector}: {kind}: {words}"
                );
            }
            for kind in ["7", "07", "82x", "17"] {
                let words = stopped(&fence, &retype(kind), &vdx);
                assert!(
                    words.contains("a type the rule set never gives"),
                    "{sector}: {kind}: {words}"
                );
            }
        }
        // On gpt too: Linux's own types, never Windows' basic data.
        let mut windows = kept(disk_gpt_windows(512));
        windows.parts[2].fate = Fate::Format {
            fs: Fs::Ext4,
            label: String::new(),
            size: 30 * GIB,
            mount: Some(MountPoint::Home),
        };
        let layout = vec![windows];
        let fence = Fence::new(&layout, &tables(&layout));
        let mut vdx = Kernel::default();
        vdx.disk("vdx");
        let retype = |kind: &str| {
            planned(
                Act::Retype,
                "/dev/vdx",
                "sfdisk",
                &["--lock", "--part-type", "/dev/vdx", "3", kind],
                None,
            )
        };
        let home = crate::model::disks::part_type::LINUX_HOME;
        let upper = home.to_ascii_uppercase();
        for kind in [home, upper.as_str()] {
            fence.check_planned(&retype(kind), &vdx).unwrap();
        }
        let basic = crate::worker::partition::fixtures::GPT_BASIC_DATA;
        let words = stopped(&fence, &retype(basic), &vdx);
        assert!(words.contains("a type the rule set never gives"), "{words}");
    }

    /// **One deletion a disk, naming every partition agreed to be deleted
    /// there** (Unit Q, Q7): were each deletion judged by phase A's table
    /// alone, a second one naming numbers the first had freed — which phase
    /// A's table still gives the partitions gone, and a partition the run
    /// made since may hold — would go through. The executor's road
    /// (`let_through`) refuses a second deletion on a disk, and every
    /// deletion must name each partition agreed to be deleted on its disk.
    #[test]
    fn a_disk_takes_one_deletion_naming_every_partition_agreed() {
        let mut windows = kept(disk_gpt_windows(512));
        windows.parts[3].fate = Fate::Delete;
        let layout = vec![windows];
        let mut vdx = Kernel::default();
        vdx.disk("vdx");
        let delete = |numbers: &[&str]| {
            let mut args = vec!["--lock", "--delete", "/dev/vdx"];
            args.extend(numbers);
            planned(Act::Delete, "/dev/vdx", "sfdisk", &args, None)
        };
        let mut fence = Fence::new(&layout, &tables(&layout));
        fence.let_through(&delete(&["4"]), &vdx).unwrap();
        // The same deletion again — once a partition made since may hold 4.
        let again = fence
            .let_through(&delete(&["4"]), &vdx)
            .expect_err("a second deletion");
        assert!(
            again.detail.contains("a second deletion"),
            "{}",
            again.detail
        );
        // Judged alone, it is still the deletion agreed: only the road the
        // executor takes keeps count.
        fence.check_planned(&delete(&["4"]), &vdx).unwrap();
        // A deletion that leaves out a partition agreed, for a second to
        // name: refused, whichever is left out.
        for sector in [512, 4096] {
            let mut old = kept(disk_msdos_logical(sector));
            for part in old.parts.iter_mut().filter(|part| part.number >= 5) {
                part.fate = Fate::Delete;
            }
            let layout = vec![old];
            let mut fence = Fence::new(&layout, &tables(&layout));
            for (numbers, left) in [
                (&["7", "6"][..], "partition 5"),
                (&["7"][..], "partition 5"),
            ] {
                let words = stopped(&fence, &delete(numbers), &vdx);
                assert!(
                    words.contains(&format!("does not name {left}, agreed to be deleted")),
                    "{sector}: {numbers:?}: {words}"
                );
            }
            fence.let_through(&delete(&["7", "6", "5"]), &vdx).unwrap();
            let words = fence
                .let_through(&delete(&["7", "6", "5"]), &vdx)
                .expect_err("a second deletion")
                .detail;
            assert!(words.contains("a second deletion"), "{sector}: {words}");
        }
    }

    /// **The boot flag goes only where the layout uses a primary partition
    /// as `/` or `/boot`, numbered by the table read just before it**
    /// (Unit Q, Q5): an msdos disk keeping its table, its third partition
    /// formatted as `/`. `--activate` is refused until the table read once
    /// the entries are changed is handed to the fence (a partition this run
    /// made has no number in phase A's); then let through for the partition
    /// at `/`'s start, and refused for a kept Windows, a partition unused,
    /// and `/` were that table to number it as a logical partition.
    #[test]
    fn the_boot_flag_goes_only_on_a_primary_partition_at_root_or_boot() {
        let mut msdos = kept(crate::worker::partition::fixtures::disk_msdos_full(512));
        msdos.parts[2].fate = Fate::Format {
            fs: Fs::Ext4,
            label: String::new(),
            size: 26 * GIB,
            mount: Some(MountPoint::Root),
        };
        let layout = vec![msdos];
        let mut fence = Fence::new(&layout, &tables(&layout));
        let mut kernel = Kernel::default();
        kernel.disk("vdx");
        let flag = |number: &str| {
            planned(
                Act::Activate,
                "/dev/vdx",
                "sfdisk",
                &["--lock", "--activate", "/dev/vdx", number],
                None,
            )
        };
        let says = |fence: &Fence, number: &str, why: &str| {
            let words = stopped(fence, &flag(number), &kernel);
            assert!(words.contains(why), "{number}: {words}");
        };
        says(&fence, "3", "no table of /dev/vdx was read to number it");
        let after = on_disk(&layout[0].disk);
        fence.written_table("/dev/vdx", &after);
        fence.check_planned(&flag("3"), &kernel).unwrap();
        for number in ["1", "2", "4"] {
            says(
                &fence,
                number,
                "where no primary partition is used as / or /boot",
            );
        }
        let mut logical = after.clone();
        logical.partitions[2].number = 5;
        fence.written_table("/dev/vdx", &logical);
        says(
            &fence,
            "5",
            "where no primary partition is used as / or /boot",
        );
    }

    #[test]
    fn no_test_reads_this_computers_block_devices() {
        for words in [
            test_fence::refusal(|| Machine.link("sda")),
            test_fence::refusal(|| Machine.place("sda1")),
        ] {
            let words = words.expect("refused");
            assert!(words.contains("test build:"), "{words}");
        }
    }
}
