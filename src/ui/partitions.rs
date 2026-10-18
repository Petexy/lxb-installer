//! The plan Partitions makes: what a person has asked to be done to the
//! disks, in order, as GParted keeps its pending operations — and the disks
//! as they will be once it is done, which is what the page draws.
//!
//! **It never leaves the interface as itself.** A [`Plan`] is a list of
//! [`Operation`]s over the `Disk`s the interface already holds, named by
//! their device paths, and nothing here can write anything. [`apply`]
//! builds a *copy* of a disk as the plan leaves it, for drawing only: it is
//! never handed to anything but the map and the details. [`layout`] says
//! the plan as a request carries it — the end state of each disk
//! (`model::layout::DiskLayout`, in `Target::Layout`) — for the rule set to
//! judge, and for `State::config` to put in a request while the plan is the
//! choice in use, which it becomes in the demo, or on a live system where
//! all it needs lies within one set QEMU proved (`layout::PROVEN`); the
//! worker carries one out only there (`check::validate`).
//!
//! **A partition the plan makes** has no device yet: the kernel numbers it
//! when it is made. It is named as GParted names one, "New Partition #1"
//! ([`NewPart::number`]), and inside the plan by a path of its own that no
//! device can have ([`new_path`]). One operation makes it, carrying its
//! place, its size, its filesystem, its mount point and its label; whatever
//! is asked of it afterwards — another mount point, another filesystem,
//! another size or label — is folded into that one operation, and deleting
//! it takes the operation away.
//!
//! **What an operation makes moot is folded away**, so the list says what
//! would really be done and nothing twice:
//!
//! - a mount point set again replaces the one set before; set to none, it
//!   takes the earlier one away and adds nothing (a partition the plan has
//!   not touched is mounted nowhere);
//! - a format again replaces the format before; "keep" takes it away; a
//!   format takes away a label set before it (the filesystem it was set on
//!   will not be there);
//! - a size or a label set again replaces the one before; set back to what
//!   the disk has, it takes the earlier one away;
//! - deleting a partition takes away every earlier operation on it (a format
//!   or a mount point of something that will not be there);
//! - a new partition table takes away every earlier operation on that disk,
//!   an earlier new table included: it deletes every partition on it.
//!
//! The folded operation goes to the end of the list, as the latest thing
//! asked for. **Undo last operation** takes the last line of the list away —
//! the list as the person sees it, so it never brings back what a fold took
//! away: that line was not on the list any more. **Undo changes to a
//! partition** takes away every line naming it; **Clear all** every line.
//! A partition made in space that an undone line had freed (a shrink taken
//! back) has nowhere to be any more, and goes with it ([`Plan::prune`]).
//!
//! **What Continue needs** ([`missing`]) is the rule set's answer
//! (`model::layout`: `shape`, then `measured`), the very rules the worker
//! judges a request by, so what the foot refuses and what the worker would
//! refuse cannot drift apart; the foot says it in GParted's words
//! ([`refusal_words`]).

use crate::i18n;
use crate::model::config::{Bootloader, Software};
use crate::model::disks::{self, part_type, Content, Disk, Gap, Partition, Table, MIB};
use crate::model::firmware::Firmware;
use crate::model::layout::logical;
use crate::model::layout::{self as rules, Role};
/// The rule set's own, read here where they were (`model::layout`).
pub use crate::model::layout::{
    can_create, margin, primaries, Fs, MountPoint as Mount, NoNew, SWAP_LEAST,
};
use crate::model::layout::{
    Capability, Create, DiskLayout, Fate, ImageSizes, Measure, Measures, Part, Refusal, Shrinkable,
    Unproven, Unshrinkable, Which,
};
use crate::ui::map::{self, Stop, Tile};

/// The filesystems a new partition can be made with on a machine started by
/// `firmware`, on a disk whose table is `table`: Format as's, and on a BIOS
/// machine's gpt disk the `bios_grub` partition GRUB's core goes in.
pub fn new_filesystems(firmware: Firmware, table: Option<Table>) -> Vec<Fs> {
    let mut all = Fs::ALL.to_vec();
    if firmware == Firmware::Bios && table == Some(Table::Gpt) {
        all.push(Fs::BiosGrub);
    }
    all
}

/// A partition the plan makes: GParted's "New Partition #N", where it
/// starts and how long it is (both on a MiB), what it is made with, and
/// where it is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPart {
    /// The N of "New Partition #N", which names it until the kernel numbers
    /// it when it is made.
    pub number: u32,
    pub start: u64,
    pub size: u64,
    pub fs: Fs,
    pub mount: Option<Mount>,
    /// Its filesystem's label; empty for none.
    pub label: String,
}

impl NewPart {
    /// The path the plan names it by ([`new_path`]).
    pub fn path(&self) -> String {
        new_path(self.number)
    }
}

/// The path the plan names New Partition #`number` by, which no device can
/// have: what an operation on it, a tile's menu and the light hold on to.
pub fn new_path(number: u32) -> String {
    format!("new#{number}")
}

/// The N of a path [`new_path`] made, if it is one.
pub fn new_number(path: &str) -> Option<u32> {
    path.strip_prefix("new#")?.parse().ok()
}

/// How a partition is named where a person reads it: its device, or for one
/// the plan makes "New Partition #N", from the catalogs.
pub fn part_name(path: &str) -> String {
    match new_number(path) {
        Some(number) => crate::message!("partitions-new-name", "number" => number as i64),
        None => path.to_string(),
    }
}

/// One thing the plan will do, named by device paths: the disk it is done
/// on, and the partition where it is done to one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Use the partition at a mount point.
    Mount {
        disk: String,
        part: String,
        at: Mount,
    },
    /// Make a new filesystem on the partition.
    Format { disk: String, part: String, fs: Fs },
    /// Delete the partition: its space becomes unallocated.
    Delete { disk: String, part: String },
    /// Write a new, empty partition table over the disk: every partition on
    /// it goes.
    Table { disk: String, table: Table },
    /// Make a new partition in unallocated space; `part` is its
    /// [`new_path`].
    Create {
        disk: String,
        part: String,
        new: NewPart,
    },
    /// Make a partition `to` bytes long from its start, which never moves:
    /// shrunk from its end, or grown into the unallocated space after it.
    /// `from` is how long the disk has it now.
    Resize {
        disk: String,
        part: String,
        from: u64,
        to: u64,
    },
    /// Set its filesystem's label; empty clears it.
    Label {
        disk: String,
        part: String,
        label: String,
    },
}

impl Operation {
    /// The disk it is done on.
    pub fn disk(&self) -> &str {
        match self {
            Operation::Mount { disk, .. }
            | Operation::Format { disk, .. }
            | Operation::Delete { disk, .. }
            | Operation::Table { disk, .. }
            | Operation::Create { disk, .. }
            | Operation::Resize { disk, .. }
            | Operation::Label { disk, .. } => disk,
        }
    }

    /// The partition it is done to, if it is done to one.
    pub fn part(&self) -> Option<&str> {
        match self {
            Operation::Mount { part, .. }
            | Operation::Format { part, .. }
            | Operation::Delete { part, .. }
            | Operation::Create { part, .. }
            | Operation::Resize { part, .. }
            | Operation::Label { part, .. } => Some(part),
            Operation::Table { .. } => None,
        }
    }
}

/// What Partitions will do, in the order it was asked for. See the module's
/// notes for how one operation folds another away.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    operations: Vec<Operation>,
}

impl Plan {
    /// Every operation, the oldest first.
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// How many operations are done on the disk at `disk`.
    pub fn on_disk(&self, disk: &str) -> usize {
        self.operations
            .iter()
            .filter(|op| op.disk() == disk)
            .count()
    }

    /// Whether any operation is done to the partition at `part`.
    pub fn on_part(&self, part: &str) -> bool {
        self.operations.iter().any(|op| op.part() == Some(part))
    }

    /// The partition the plan makes at `part` ([`new_path`]), if it makes
    /// one.
    pub fn created(&self, part: &str) -> Option<&NewPart> {
        self.operations.iter().find_map(|op| match op {
            Operation::Create {
                part: here, new, ..
            } if here == part => Some(new),
            _ => None,
        })
    }

    /// The mount point the plan gives the partition at `part`, if any.
    pub fn mount_of(&self, part: &str) -> Option<Mount> {
        if let Some(new) = self.created(part) {
            return new.mount.clone();
        }
        self.operations.iter().find_map(|op| match op {
            Operation::Mount { part: here, at, .. } if here == part => Some(at.clone()),
            _ => None,
        })
    }

    /// The filesystem the plan formats the partition at `part` as — or makes
    /// it with — if any.
    pub fn format_of(&self, part: &str) -> Option<Fs> {
        if let Some(new) = self.created(part) {
            return Some(new.fs);
        }
        self.operations.iter().find_map(|op| match op {
            Operation::Format { part: here, fs, .. } if here == part => Some(*fs),
            _ => None,
        })
    }

    /// The size the plan gives the partition at `part`, where it changes
    /// one's size or makes one.
    pub fn size_of(&self, part: &str) -> Option<u64> {
        if let Some(new) = self.created(part) {
            return Some(new.size);
        }
        self.operations.iter().find_map(|op| match op {
            Operation::Resize { part: here, to, .. } if here == part => Some(*to),
            _ => None,
        })
    }

    /// The label the plan gives the partition at `part`, where it sets one
    /// or makes the partition.
    pub fn label_of(&self, part: &str) -> Option<&str> {
        if let Some(new) = self.created(part) {
            return Some(&new.label);
        }
        self.operations.iter().find_map(|op| match op {
            Operation::Label {
                part: here, label, ..
            } if here == part => Some(label.as_str()),
            _ => None,
        })
    }

    /// Whether the plan uses a partition — one it keeps or one it makes —
    /// at `at`.
    pub fn uses(&self, at: &Mount) -> bool {
        self.operations.iter().any(|op| match op {
            Operation::Mount { at: here, .. } => here == at,
            Operation::Create { new, .. } => new.mount.as_ref() == Some(at),
            _ => false,
        })
    }

    /// The N the next partition the plan makes is named by: one more than
    /// the highest it names, so two new partitions are never named alike.
    pub fn next_number(&self) -> u32 {
        self.operations
            .iter()
            .filter_map(|op| match op {
                Operation::Create { new, .. } => Some(new.number),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Change the partition the plan makes at `part` with `change`, folding
    /// it into the one operation that makes it, which then goes to the end
    /// of the list as the latest thing asked for. Answers whether it
    /// changed; nothing moves where it did not.
    fn refold(&mut self, part: &str, change: impl FnOnce(&mut NewPart)) -> bool {
        let Some(at) = self
            .operations
            .iter()
            .position(|op| matches!(op, Operation::Create { part: here, .. } if here == part))
        else {
            return false;
        };
        let mut op = self.operations[at].clone();
        let Operation::Create { new, .. } = &mut op else {
            return false;
        };
        let before = new.clone();
        change(new);
        if *new == before {
            return false;
        }
        self.operations.remove(at);
        self.operations.push(op);
        true
    }

    /// Use the partition at `part` of `disk` at `at`, or — `None` — at no
    /// mount point. Answers whether the plan changed.
    pub fn mount(&mut self, disk: &str, part: &str, at: Option<Mount>) -> bool {
        if self.created(part).is_some() {
            return self.refold(part, |new| new.mount = at);
        }
        let before = self.clone();
        self.operations
            .retain(|op| !matches!(op, Operation::Mount { part: here, .. } if here == part));
        if let Some(at) = at {
            self.operations.push(Operation::Mount {
                disk: disk.into(),
                part: part.into(),
                at,
            });
        }
        *self != before
    }

    /// Format the partition at `part` of `disk` as `fs`, or — `None` — keep
    /// what is on it. A label set before goes with the filesystem it was set
    /// on; and keeping what a format was planned for takes away the size
    /// set for that format too — a kept partition's entry is never the one
    /// a fresh filesystem was given (it would grow a kept partition, or
    /// shrink one nothing has read). Answers whether the plan changed.
    pub fn format(&mut self, disk: &str, part: &str, fs: Option<Fs>) -> bool {
        if self.created(part).is_some() {
            // A partition the plan makes is made with a filesystem: there is
            // nothing on it to keep. Its label is cut to what the new one
            // holds, and written as it writes labels, as New partition's
            // File system row cuts it.
            return fs.is_some_and(|fs| {
                self.refold(part, |new| {
                    new.fs = fs;
                    new.label = crate::ui::sheet::label_with("", &new.label, Some(fs.name())).0;
                })
            });
        }
        let before = self.clone();
        let unformat = fs.is_none() && self.format_of(part).is_some();
        self.operations.retain(|op| match op {
            Operation::Format { part: here, .. } | Operation::Label { part: here, .. } => {
                here != part
            }
            Operation::Resize { part: here, .. } => here != part || !unformat,
            _ => true,
        });
        if let Some(fs) = fs {
            self.operations.push(Operation::Format {
                disk: disk.into(),
                part: part.into(),
                fs,
            });
        }
        *self != before
    }

    /// Delete the partition at `part` of `disk`; whatever else was to be
    /// done to it is moot. A partition the plan makes is simply not made.
    pub fn delete(&mut self, disk: &str, part: &str) {
        let new = self.created(part).is_some();
        self.operations.retain(|op| op.part() != Some(part));
        if !new {
            self.operations.push(Operation::Delete {
                disk: disk.into(),
                part: part.into(),
            });
        }
    }

    /// Write a new `table` over the disk at `disk`; whatever else was to be
    /// done on it is moot.
    pub fn table(&mut self, disk: &str, table: Table) {
        self.operations.retain(|op| op.disk() != disk);
        self.operations.push(Operation::Table {
            disk: disk.into(),
            table,
        });
    }

    /// Make `new` on `disk`.
    pub fn create(&mut self, disk: &str, new: NewPart) {
        self.operations.push(Operation::Create {
            disk: disk.into(),
            part: new.path(),
            new,
        });
    }

    /// Make the partition at `part` of `disk`, `from` bytes long on the disk
    /// now, `to` bytes long. Back to `from`, nothing is done to its size.
    /// Answers whether the plan changed.
    pub fn resize(&mut self, disk: &str, part: &str, from: u64, to: u64) -> bool {
        if self.created(part).is_some() {
            return self.refold(part, |new| new.size = to);
        }
        let before = self.clone();
        self.operations
            .retain(|op| !matches!(op, Operation::Resize { part: here, .. } if here == part));
        if to != from {
            self.operations.push(Operation::Resize {
                disk: disk.into(),
                part: part.into(),
                from,
                to,
            });
        }
        *self != before
    }

    /// Set the label of the partition at `part` of `disk` to `label`, `was`
    /// being the label it has without this (none on a filesystem the plan
    /// makes). Back to `was`, nothing is done to it. A label is set only as
    /// a filesystem is made (only when formatting, by design): on a
    /// partition the plan keeps nothing is done. Answers whether the plan
    /// changed.
    pub fn label(&mut self, disk: &str, part: &str, was: &str, label: &str) -> bool {
        if self.created(part).is_some() {
            return self.refold(part, |new| new.label = label.to_string());
        }
        if self.format_of(part).is_none() {
            return false;
        }
        let before = self.clone();
        self.operations
            .retain(|op| !matches!(op, Operation::Label { part: here, .. } if here == part));
        if label != was {
            self.operations.push(Operation::Label {
                disk: disk.into(),
                part: part.into(),
                label: label.into(),
            });
        }
        *self != before
    }

    /// Take the last operation away. Answers whether there was one.
    pub fn undo_last(&mut self) -> bool {
        self.operations.pop().is_some()
    }

    /// Take away every operation on the partition at `part`. Answers whether
    /// there was one.
    pub fn undo_part(&mut self, part: &str) -> bool {
        let before = self.operations.len();
        self.operations.retain(|op| op.part() != Some(part));
        self.operations.len() != before
    }

    /// Take every operation away.
    pub fn clear(&mut self) {
        self.operations.clear();
    }

    /// Take away what the rest of the plan leaves no place for on `disks`:
    /// a partition the plan makes that has nowhere to be any more (one made
    /// in the space a shrink freed, once the shrink is taken back), and a
    /// size set for a format that is no longer there (a format taken back
    /// by Undo last operation) where a kept partition could not have it —
    /// larger than it is, for a kept partition never grows, or smaller where
    /// its filesystem is not one that shrinks. Answers whether any went.
    pub fn prune(&mut self, disks: &[Disk], firmware: Firmware) -> bool {
        let before = self.operations.len();
        let formatted: Vec<String> = self
            .operations
            .iter()
            .filter_map(|op| match op {
                Operation::Format { part, .. } => Some(part.clone()),
                _ => None,
            })
            .collect();
        self.operations.retain(|op| {
            let Operation::Resize {
                disk,
                part,
                from,
                to,
                ..
            } = op
            else {
                return true;
            };
            if formatted.contains(part) {
                return true;
            }
            let was = disks
                .iter()
                .filter(|one| one.path == *disk)
                .find_map(|one| one.partition(part));
            to < from && was.is_some_and(rules::shrinkable)
        });
        let mut gone = self.operations.len() != before;
        let mut at = 0;
        while at < self.operations.len() {
            let Operation::Create { disk, new, .. } = &self.operations[at] else {
                at += 1;
                continue;
            };
            let Some(on) = disks.iter().find(|one| one.path == *disk) else {
                at += 1;
                continue;
            };
            let mut without = self.clone();
            without.operations.remove(at);
            let (start, end) = (new.start, new.start + new.size);
            let fits = apply(&without, on, firmware)
                .disk
                .free
                .iter()
                .any(|run| run.start <= start && end <= run.start + run.size);
            if fits {
                at += 1;
            } else {
                self.operations.remove(at);
                gone = true;
            }
        }
        gone
    }
}

/// What the plan does to one partition that is still there once it is
/// carried out: where it is used, what it is formatted as, and the
/// partition as it is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marks {
    pub mount: Option<Mount>,
    pub format: Option<Fs>,
    /// The partition as the disk holds it now: what a format replaces, and
    /// what a resize resizes. For a partition the plan makes, itself.
    pub was: Partition,
    /// The N of "New Partition #N", for a partition the plan makes.
    pub new: Option<u32>,
}

/// A disk as the plan leaves it: a copy of it with the deleted partitions
/// gone and their space unallocated, merged with the unallocated space
/// beside it, the formatted ones holding their new filesystems, the resized
/// ones at their new sizes and the new ones in place — or, under a new
/// table, nothing but the new ones in one run of unallocated space — and
/// what the plan does to each partition on it. For drawing only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    /// The disk as it will be: what the map lays out.
    pub disk: Disk,
    /// One for each of `disk.partitions`, in its order.
    pub parts: Vec<Marks>,
    /// The partitions the plan deletes, as the disk holds them now.
    pub deleted: Vec<Partition>,
    /// The new partition table written over it, if one is.
    pub table: Option<Table>,
}

impl Planned {
    /// The partitions deleted whose space lies in the run of unallocated
    /// space from `start`, `size` long.
    pub fn deleted_in(&self, start: u64, size: u64) -> Vec<&Partition> {
        self.deleted
            .iter()
            .filter(|part| part.start < start + size && part.start + part.size.max(1) > start)
            .collect()
    }

    /// Where the partition at `path` is among the disk's, if it is there.
    pub fn index_of(&self, path: &str) -> Option<usize> {
        self.disk
            .partitions
            .iter()
            .position(|part| part.path == path)
    }
}

/// Where each extended partition of `disk` really ends (`map::extent`), by
/// its path: the kernel reports one as a stub, and its logical partitions
/// say how far it goes.
fn extended_ends(disk: &Disk) -> Vec<(String, u64)> {
    map::tiles(disk)
        .iter()
        .filter_map(|tile| match tile {
            Tile::Extended { at, inside } => {
                let part = &disk.partitions[*at];
                Some((
                    part.path.clone(),
                    part.start + map::extent(disk, *at, inside),
                ))
            }
            _ => None,
        })
        .collect()
}

/// `new` as the disk will hold it, on a disk left with `table`, in the
/// `role` its mount point gives it: typed as the rule set types it
/// (`layout::part_type`), so the details' flags (`map::flags`) say what it
/// will be.
fn new_partition(new: &NewPart, table: Option<Table>, role: Role) -> Partition {
    let part_type = rules::part_type(
        table.unwrap_or(Table::Gpt),
        new.fs,
        new.mount.as_ref(),
        role,
    );
    Partition {
        path: new.path(),
        // Numbered by the kernel when it is made; counted as a primary
        // partition meanwhile (`logical`).
        number: 0,
        start: new.start,
        size: new.size,
        fs_type: new.fs.lsblk().to_string(),
        label: new.label.clone(),
        part_type: part_type.to_string(),
        uuid: String::new(),
        mountpoints: Vec::new(),
        content: if role == Role::Esp {
            Content::Efi
        } else {
            new.fs.content()
        },
        // A new msdos table marks one in its own script; on a kept one the
        // last step marks one only where nothing is (`layout::activates`).
        bootable: false,
    }
}

/// `disk` as the plan leaves it, on a machine started by `firmware` (which
/// decides the types of what is made and formatted at `/efi` and `/boot`).
pub fn apply(plan: &Plan, disk: &Disk, firmware: Firmware) -> Planned {
    let efi_used = plan.uses(&Mount::Efi);
    let ours: Vec<&Operation> = plan
        .operations
        .iter()
        .filter(|op| op.disk() == disk.path)
        .collect();
    let creates: Vec<&NewPart> = ours
        .iter()
        .filter_map(|op| match op {
            Operation::Create { new, .. } => Some(new),
            _ => None,
        })
        .collect();
    // A new table: every partition goes, and the whole of the room it
    // leaves is one run of unallocated space — then the partitions made in
    // it. Nothing else on this disk can come after it (there is no
    // partition left to name but the new ones, whose operations fold into
    // the ones that make them).
    let table = ours.iter().rev().find_map(|op| match op {
        Operation::Table { table, .. } => Some(*table),
        _ => None,
    });
    let mut planned = match table {
        Some(table) => {
            let (first, last) = disks::usable(table, disk.size, disk.logical_sector);
            let mut empty = disk.clone();
            empty.table = Some(table);
            empty.content = None;
            empty.partitions.clear();
            empty.free = (last > first)
                .then(|| Gap {
                    start: first,
                    size: last - first,
                })
                .into_iter()
                .collect();
            Planned {
                disk: empty,
                parts: Vec::new(),
                deleted: disk.partitions.clone(),
                table: Some(table),
            }
        }
        None => existing(&ours, disk, firmware, efi_used),
    };
    if creates.is_empty() {
        return planned;
    }
    let table = planned.disk.table;
    let mut taken = Vec::new();
    for new in creates {
        let role = rules::role(firmware, new.mount.as_ref(), efi_used);
        let part = new_partition(new, table, role);
        taken.push((part.start, part.start + part.size));
        planned.parts.push(Marks {
            mount: new.mount.clone(),
            format: Some(new.fs),
            was: part.clone(),
            new: Some(new.number),
        });
        planned.disk.partitions.push(part);
    }
    // In disk order, the marks with their partitions.
    let mut both: Vec<(Partition, Marks)> = std::mem::take(&mut planned.disk.partitions)
        .into_iter()
        .zip(std::mem::take(&mut planned.parts))
        .collect();
    both.sort_by_key(|(part, _)| part.start);
    (planned.disk.partitions, planned.parts) = both.into_iter().unzip();
    let runs = planned
        .disk
        .free
        .iter()
        .map(|gap| (gap.start, gap.start + gap.size))
        .collect();
    planned.disk.free = aligned(subtract(runs, &taken));
    planned
}

/// `disk` with the plan's operations on its existing partitions carried
/// out: mount points, formats and labels marked, a formatted one retyped by
/// where it is used (`efi_used`: the plan uses `/efi`), sizes changed,
/// deleted partitions gone and their space and the space a shrink frees
/// unallocated.
fn existing(ours: &[&Operation], disk: &Disk, firmware: Firmware, efi_used: bool) -> Planned {
    let mut planned = Planned {
        disk: disk.clone(),
        parts: disk
            .partitions
            .iter()
            .map(|part| Marks {
                mount: None,
                format: None,
                was: part.clone(),
                new: None,
            })
            .collect(),
        deleted: Vec::new(),
        table: None,
    };
    let mut gone = Vec::new();
    let mut resized = false;
    for op in ours {
        let Some(at) = op
            .part()
            .and_then(|path| disk.partitions.iter().position(|one| one.path == path))
        else {
            continue;
        };
        match op {
            Operation::Mount { at: mount, .. } => planned.parts[at].mount = Some(mount.clone()),
            Operation::Format { fs, .. } => {
                planned.parts[at].format = Some(*fs);
                let part = &mut planned.disk.partitions[at];
                part.fs_type = fs.lsblk().to_string();
                part.label.clear();
                part.uuid.clear();
            }
            Operation::Label { label, .. } => {
                planned.disk.partitions[at].label = label.clone();
            }
            Operation::Resize { to, .. } => {
                planned.disk.partitions[at].size = *to;
                resized = true;
            }
            Operation::Delete { .. } => gone.push(at),
            Operation::Table { .. } | Operation::Create { .. } => {}
        }
    }
    // A formatted partition is typed by where it is used, as one made is:
    // once every mount point is known, whatever order they came in.
    for (part, marks) in planned.disk.partitions.iter_mut().zip(&planned.parts) {
        let (Some(fs), Some(table)) = (marks.format, disk.table) else {
            continue;
        };
        let role = rules::role(firmware, marks.mount.as_ref(), efi_used);
        part.part_type = rules::part_type(table, fs, marks.mount.as_ref(), role).to_string();
        part.content = if role == Role::Esp {
            Content::Efi
        } else {
            fs.content()
        };
    }
    if gone.is_empty() && !resized {
        return planned;
    }
    let ends = extended_ends(disk);
    gone.sort_unstable();
    for at in gone.iter().rev() {
        planned
            .deleted
            .insert(0, planned.disk.partitions.remove(*at));
        planned.parts.remove(*at);
    }
    // An extended partition still there is given its whole reach, so the
    // unallocated space a deleted logical partition leaves stands inside it.
    for part in &mut planned.disk.partitions {
        if let Some((_, end)) = ends.iter().find(|(path, _)| *path == part.path) {
            part.size = end.saturating_sub(part.start).max(part.size);
        }
    }
    // What a shrink frees: from the partition's new end to its old one.
    let freed: Vec<(bool, u64, u64)> = planned
        .disk
        .partitions
        .iter()
        .zip(&planned.parts)
        .filter(|(part, marks)| !part.is_extended() && part.size < marks.was.size)
        .map(|(part, marks)| {
            (
                logical(disk.table, part),
                part.start + part.size,
                marks.was.start + marks.was.size,
            )
        })
        .collect();
    planned.disk.free = unallocated(
        disk,
        &planned.disk.partitions,
        &planned.deleted,
        &freed,
        &ends,
    );
    planned
}

/// The runs of unallocated space on `disk` once `deleted` are gone,
/// `remaining` stay at their sizes in the plan and the spans `freed` by a
/// shrink are let go of: the gaps discovery listed, and the space of each
/// deleted partition and each span a shrink freed reaching out to the
/// partitions still beside it — the unallocated space next to it, listed or
/// too small to be listed, merged with it — in disk order, less what a grown
/// partition now takes. Inside an extended partition that stays, the space a
/// logical partition leaves is a run inside it — a deleted one's reaching
/// out to its neighbours, a shrunk one's only from its new end to its old
/// one, since the next logical partition's table (its EBR) stands past
/// that; an extended partition deleted (with nothing left in it) leaves its
/// whole reach. On a disk something is written over as a whole
/// (`layout::written_whole`), where discovery lists no free space at all
/// since what lies between and after its partitions may be that content's,
/// a deleted partition leaves its own span and no more, and a shrink what
/// it gave up. Aligned to a MiB at both ends, as discovery's gaps are; what
/// discovery did not list and nothing in the plan touches is still not
/// listed.
fn unallocated(
    disk: &Disk,
    remaining: &[Partition],
    deleted: &[Partition],
    freed: &[(bool, u64, u64)],
    ends: &[(String, u64)],
) -> Vec<Gap> {
    let Some(table) = disk.table else {
        return disk.free.clone();
    };
    let (first, last) = disks::usable(table, disk.size, disk.logical_sector);
    let end_of = |part: &Partition| {
        ends.iter()
            .find(|(path, _)| *path == part.path)
            .map_or(part.start + part.size, |(_, end)| *end)
    };
    // Reach out from `from`..`to` to the nearest of `taken` either side, or
    // to `low`..`high` — but on a disk written over as a whole, not past
    // `from`..`to` itself.
    let whole = rules::written_whole(disk);
    let widen = |from: u64, to: u64, taken: &[(u64, u64)], low: u64, high: u64| {
        if whole {
            return (from, to);
        }
        let left = taken
            .iter()
            .filter(|(_, end)| *end <= from)
            .map(|(_, end)| *end)
            .fold(low, u64::max);
        let right = taken
            .iter()
            .filter(|(start, _)| *start >= to)
            .map(|(start, _)| *start)
            .fold(high, u64::min);
        (left.min(from), right.max(to))
    };
    let mut runs: Vec<(u64, u64)> = disk
        .free
        .iter()
        .map(|gap| (gap.start, gap.start + gap.size))
        .collect();
    // The top level: primary partitions and extended ones, each to its end.
    let top: Vec<(u64, u64)> = remaining
        .iter()
        .filter(|part| !logical(disk.table, part))
        .map(|part| (part.start, end_of(part)))
        .collect();
    for part in deleted.iter().filter(|part| !logical(disk.table, part)) {
        runs.push(widen(part.start, end_of(part), &top, first, last));
    }
    for (_, from, to) in freed.iter().filter(|(inner, ..)| !inner) {
        runs.push(widen(*from, *to, &top, first, last));
    }
    // Inside each extended partition that stays: its logical partitions.
    let mut inner: Vec<(u64, u64)> = Vec::new();
    for extended in remaining.iter().filter(|part| part.is_extended()) {
        let (low, high) = (extended.start, end_of(extended));
        let inside: Vec<(u64, u64)> = remaining
            .iter()
            .filter(|part| logical(disk.table, part))
            .filter(|part| part.start >= low && part.start < high)
            .map(|part| (part.start, part.start + part.size))
            .collect();
        for part in deleted
            .iter()
            .filter(|part| logical(disk.table, part) && part.start >= low && part.start < high)
        {
            inner.push(widen(
                part.start,
                part.start + part.size,
                &inside,
                low,
                high,
            ));
        }
        // What a shrink frees in here is what the partition gave up and no
        // more: past its old end stands the next logical partition's own
        // table (its EBR), which discovery never shows, and a partition
        // grown into the space after it would be grown over that.
        for (_, from, to) in freed
            .iter()
            .filter(|(inner, from, _)| *inner && *from >= low && *from < high)
        {
            inner.push((*from, *to));
        }
    }
    // What a grown partition reaches into is no longer unallocated.
    let taken: Vec<(u64, u64)> = remaining
        .iter()
        .filter(|part| !part.is_extended())
        .map(|part| (part.start, part.start + part.size))
        .collect();
    let mut gaps = aligned(subtract(union(runs), &taken));
    gaps.extend(aligned(subtract(union(inner), &taken)));
    gaps.sort_by_key(|gap| gap.start);
    gaps
}

/// `runs` (start, end) joined where they meet or overlap, in disk order.
fn union(mut runs: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    runs.sort_unstable();
    let mut joined: Vec<(u64, u64)> = Vec::new();
    for (start, end) in runs {
        match joined.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => joined.push((start, end)),
        }
    }
    joined
}

/// `runs` less every span of `taken` (start, end).
fn subtract(runs: Vec<(u64, u64)>, taken: &[(u64, u64)]) -> Vec<(u64, u64)> {
    let mut left = runs;
    for (start, end) in taken {
        left = left
            .into_iter()
            .flat_map(|(from, to)| {
                if *end <= from || *start >= to {
                    return vec![(from, to)];
                }
                [(from, *start), (*end, to)]
                    .into_iter()
                    .filter(|(a, b)| b > a)
                    .collect()
            })
            .collect();
    }
    left
}

/// `runs` aligned to a MiB at both ends, the empty ones dropped.
fn aligned(runs: Vec<(u64, u64)>) -> Vec<Gap> {
    runs.into_iter()
        .filter_map(|(start, end)| {
            let start = start.div_ceil(MIB) * MIB;
            let end = end / MIB * MIB;
            (end > start).then(|| Gap {
                start,
                size: end - start,
            })
        })
        .collect()
}

// -------------------------------------------------------------------------
// What can be done where.

/// A row of a tile's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileRow {
    /// New…, on unallocated space.
    New,
    /// Create a partition table, on the unallocated space of a disk that has
    /// none: offered where the person stands rather than from the disk's
    /// chip, where a menu would be confusing.
    Table,
    /// Mount point · none ›
    Mount,
    /// Format as · keep ›
    Format,
    /// Resize…
    Resize,
    /// Label…
    Label,
    Delete,
    /// Undo changes to /dev/sdaN, once the plan does anything to it.
    Undo,
}

/// What a menu leaves out of unallocated space, and the details say why
/// instead (`model::layout::NoNew`), in the catalogs' words.
impl NoNew {
    pub fn words(self) -> &'static str {
        match self {
            NoNew::Whole => i18n::text("partitions-why-whole"),
            NoNew::Table => i18n::text("partitions-why-table"),
            NoNew::Primaries => i18n::text("partitions-why-primaries"),
            NoNew::Logical => i18n::text("partitions-why-logical"),
            NoNew::BesideExtended => i18n::text("partitions-why-beside-extended"),
        }
    }
}

/// How small and how large a partition can be made by Resize: from `least`
/// to `most`, its start never moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room {
    pub least: u64,
    pub most: u64,
}

/// What reading says `part`, kept, may be shrunk by — what it uses and its
/// tool's least — if it may be (`Measures::shrink_of`, the rule set's own
/// reading of a measurement).
pub fn shrinkable(measures: &Measures, part: &Partition) -> Option<Shrinkable> {
    measures.shrink_of(part).and_then(Result::ok)
}

/// How far the partition at `at` of `planned` can be resized, its start
/// never moving, or `None` where it cannot be:
///
/// - **kept**, it only shrinks — what is on it is never made larger
///   (§7.0, Unit Q) — and only an ext4 or NTFS one (`layout::shrinkable`)
///   that reading says may be shrunk (`shrinkable`, `Measures::shrink_of`:
///   from the demo's numbers, or the measure entry's; an NTFS judged across
///   every disk of the measurement): from what it uses and a margin
///   (`layout::shrink_floor`, never below its tool's own least) — or, for a
///   filesystem already smaller than that, from its own size, an entry that
///   holds it being made smaller alone (`Shrinkable::least_entry`, the rule
///   set's) — its filesystem's least and its mount point's, to its own
///   size;
/// - **made or formatted** by the plan, it holds nothing yet: its entry
///   alone is resized, whatever its filesystem, from its filesystem's and
///   mount point's least to the end of the unallocated space right after it
///   (inside its extended partition, for a logical one — and never past
///   where it ended while a logical partition stands after it on the disk,
///   whatever their numbers: before each stands its own table, its EBR,
///   which discovery never shows, as `layout::shape` holds; and on a disk
///   written over as a whole only over space that begins where it ends,
///   which there is only what a partition deleted or shrunk gave up —
///   [`unallocated`] — as `layout::shape` holds too).
pub fn resize_room(planned: &Planned, at: usize, shrinkable: Option<Shrinkable>) -> Option<Room> {
    let part = planned.disk.partitions.get(at)?;
    let marks = planned.parts.get(at)?;
    if part.is_extended() || matches!(marks.was.content, Content::MicrosoftReserved) {
        return None;
    }
    let sector = planned.disk.logical_sector;
    let mount_least = marks.mount.as_ref().map_or(0, Mount::least);
    let Some(fs) = marks.format else {
        if !rules::shrinkable(&marks.was) {
            return None;
        }
        let shrinkable: Shrinkable = shrinkable?;
        let fs_least = map::filesystem(&marks.was)
            .and_then(Fs::named)
            .map_or(0, |fs| rules::fs_least(fs, sector));
        let least = shrinkable.least_entry().max(fs_least).max(mount_least);
        let most = marks.was.size;
        return (least < most).then_some(Room { least, most });
    };
    if fs == Fs::BiosGrub {
        return None;
    }
    // Never more than it is now: a made one smaller than its least may
    // still grow.
    let least = rules::fs_least(fs, sector)
        .max(mount_least)
        .min(marks.was.size.max(MIB));
    let end = part.start + part.size;
    let frame = (planned.disk.table == Some(Table::Msdos) && part.number > 4)
        .then(|| {
            planned
                .disk
                .partitions
                .iter()
                .find(|one| one.is_extended() && part.start >= one.start)
                .map(|one| one.start + one.size)
        })
        .flatten();
    let next = planned
        .disk
        .partitions
        .iter()
        .filter(|one| one.start >= end)
        .map(|one| one.start)
        .min();
    // On a disk written over as a whole only a run that begins where the
    // partition ends: what lies between may be the whole-disk content's.
    let whole = rules::written_whole(&planned.disk);
    let after = planned.disk.free.iter().find(|run| {
        run.start >= end
            && (!whole || run.start == end)
            && next.is_none_or(|next| run.start < next)
            && frame.is_none_or(|frame| run.start + run.size <= frame)
    });
    let most = match after {
        Some(run) => (run.start + run.size - part.start) / MIB * MIB,
        None => part.size,
    }
    .max(part.size);
    // What a shrink frees in the frame is only what it gave up
    // ([`unallocated`]), but what a deleted logical partition leaves reaches
    // up to the next one's start, over that one's table — and in a table
    // chained out of the disk's order the one deleted may stand before a
    // kept one numbered lower. So a logical partition grows back only to
    // where it ended while any logical partition stands after it on the
    // disk.
    let followed = frame.is_some()
        && planned
            .disk
            .partitions
            .iter()
            .any(|one| logical(planned.disk.table, one) && one.start > part.start);
    let most = if followed {
        most.min(marks.was.size.max(part.size))
    } else {
        most
    };
    (least < most).then_some(Room { least, most })
}

/// Why a kept partition is given no mount point: its filesystem is none the
/// installer mounts (`layout::MOUNTABLE`) — BitLocker, LUKS, an LVM or RAID
/// member, swap, nothing at all. `None` for one that can be mounted, and
/// for one the plan makes or formats.
fn unmountable(marks: &Marks) -> bool {
    marks.format.is_none() && marks.new.is_none() && !rules::mountable(&marks.was.fs_type)
}

/// Whether the partition at `at` of `planned` may be deleted: not a
/// logical partition while one after it is kept (the kernel would number
/// the kept one anew halfway through a run), nor an extended one with
/// anything left in it.
fn deletable(planned: &Planned, at: usize) -> bool {
    let table = planned.disk.table;
    let Some(part) = planned.disk.partitions.get(at) else {
        return false;
    };
    let later_kept = |part: &Partition| {
        planned
            .disk
            .partitions
            .iter()
            .any(|one| logical(table, one) && one.number > part.number)
    };
    if part.is_extended() {
        return !planned
            .disk
            .partitions
            .iter()
            .any(|one| logical(table, one));
    }
    !(logical(table, part) && later_kept(part))
}

/// What the details say a partition offers less for, under them: a kept
/// filesystem the installer cannot mount ("crypto_LUKS can't be mounted:
/// format it to use it"), or a logical partition that cannot go before the
/// ones after it. `None` where nothing is held back.
pub fn held_back(planned: &Planned, at: usize) -> Option<String> {
    let (part, marks) = (planned.disk.partitions.get(at)?, planned.parts.get(at)?);
    let holds_one = !part.is_extended()
        && !matches!(part.content, Content::MicrosoftReserved | Content::BiosBoot)
        && part.part_type != part_type::MICROSOFT_RESERVED
        && part.part_type != part_type::BIOS_BOOT;
    if holds_one && unmountable(marks) {
        return Some(match map::filesystem(&marks.was) {
            Some(fs) => crate::message!("partitions-why-unmountable", "filesystem" => fs),
            None => i18n::text("partitions-why-unformatted").to_string(),
        });
    }
    let logical_held = logical(planned.disk.table, part) && !deletable(planned, at);
    logical_held.then(|| i18n::text("partitions-why-logical-delete").to_string())
}

/// Whether Options offers a new partition table over `planned`'s disk: one
/// that has a table as the plan leaves it (a disk with none is given one
/// from its unallocated space instead), nothing written over the whole of
/// it, which the rule set writes no table over (`layout::written_whole`;
/// the details say why, [`whole_held_back`]), and a logical sector the rule
/// set lays partitions out in (`layout::sector_fits`; [`sectors_held_back`]).
pub fn table_offered(planned: &Planned) -> bool {
    planned.disk.table.is_some()
        && !rules::written_whole(&planned.disk)
        && rules::sector_fits(&planned.disk)
}

/// What the details say first under every tile of a disk whose logical
/// sector the rule set lays no partitions out in (`layout::sector_fits`:
/// larger than 4096 bytes, or no power of two): "Nothing can be changed on
/// /dev/sdX" — the rule set refuses any plan that touches it, so no tile of
/// it offers anything ([`tile_rows`]) and Options no new table. `None` on
/// every other disk.
pub fn sectors_held_back(planned: &Planned) -> Option<String> {
    (!rules::sector_fits(&planned.disk))
        .then(|| crate::message!("partitions-blocked", "disk" => planned.disk.path.as_str()))
}

/// What the details say under every tile of a disk something is written
/// over as a whole (`layout::written_whole`) where the tile holds nothing
/// back of its own: that no partition table and no partition can be made on
/// it — why Options offers no new table there and no space on it New….
/// `None` on every other disk.
pub fn whole_held_back(planned: &Planned) -> Option<String> {
    rules::written_whole(&planned.disk).then(|| NoNew::Whole.words().to_string())
}

/// The rows of the menu of the tile at `stop` of `planned`, what cannot be
/// done left out rather than greyed (the details say why, [`held_back`]),
/// `measures` saying what reading the disks found — none at all on a disk
/// whose logical sector the rule set lays nothing out in
/// ([`sectors_held_back`]):
///
/// - unallocated space offers New… where a partition can be made in it
///   ([`can_create`]), and on a disk with no partition table the table to
///   make first;
/// - a partition the disk has is given a mount point — where what is on it
///   can be mounted, or the plan formats it — formatted, resized where
///   [`resize_room`] allows (a kept one only shrinks, ext4 or NTFS, once
///   read), labelled only where the plan formats it (a label is set only as
///   a filesystem is made), and deleted — but a logical partition only once
///   the ones after it are gone; Microsoft's reserved space and a `bios_grub`
///   partition, which hold no filesystem anything mounts, only deleted; an
///   extended partition only deleted, and only once nothing is left in it.
///   Undo once the plan does anything to it;
/// - a partition the plan makes is given a mount point, made with another
///   filesystem (nothing on it to keep), resized, labelled and deleted — the
///   deletion is its undoing.
pub fn tile_rows(plan: &Plan, planned: &Planned, stop: Stop, measures: &Measures) -> Vec<TileRow> {
    // A disk the rule set lays nothing out on offers nothing at all
    // ([`sectors_held_back`]).
    if !rules::sector_fits(&planned.disk) {
        return Vec::new();
    }
    let at = match stop {
        Stop::Part(at) | Stop::Extended(at) => at,
        Stop::Free { start, .. } => {
            return match can_create(&planned.disk, start) {
                Ok(()) => vec![TileRow::New],
                Err(NoNew::Table) => vec![TileRow::Table],
                Err(_) => Vec::new(),
            };
        }
        Stop::Whole => return Vec::new(),
    };
    let (Some(part), Some(marks)) = (planned.disk.partitions.get(at), planned.parts.get(at)) else {
        return Vec::new();
    };
    let label = |rows: &mut Vec<TileRow>| {
        if crate::ui::sheet::label_limit(map::filesystem(part)).is_some() {
            rows.push(TileRow::Label);
        }
    };
    let mut rows = Vec::new();
    if part.is_extended() {
        if deletable(planned, at) {
            rows.push(TileRow::Delete);
        }
    } else if marks.new.is_some() {
        if marks.format != Some(Fs::BiosGrub) {
            rows.push(TileRow::Mount);
        }
        rows.push(TileRow::Format);
        if resize_room(planned, at, None).is_some() {
            rows.push(TileRow::Resize);
        }
        label(&mut rows);
        rows.push(TileRow::Delete);
        return rows;
    } else {
        let holds_one = !matches!(part.content, Content::MicrosoftReserved | Content::BiosBoot)
            && part.part_type != part_type::MICROSOFT_RESERVED
            && part.part_type != part_type::BIOS_BOOT;
        if holds_one {
            if !unmountable(marks) {
                rows.push(TileRow::Mount);
            }
            rows.push(TileRow::Format);
            if resize_room(planned, at, shrinkable(measures, &marks.was)).is_some() {
                rows.push(TileRow::Resize);
            }
            if marks.format.is_some() {
                label(&mut rows);
            }
        }
        if deletable(planned, at) {
            rows.push(TileRow::Delete);
        }
    }
    if plan.on_part(&part.path) {
        rows.push(TileRow::Undo);
    }
    rows
}

/// The mount points the Mount point menu offers the partition `marks`
/// stands for, by name — Other… and none come after them where
/// [`offers_other`] says — : a kept EFI system partition `/efi` and `/boot`
/// alone (it is kept only as one); `/` only where what the partition will
/// hold can be `/` — ext4 or Btrfs made or formatted, or a filesystem kept
/// that a format can still make one; everything else to every partition.
pub fn mount_offers(marks: &Marks) -> Vec<Mount> {
    if marks.format.is_none() && marks.new.is_none() && rules::kept_esp(&marks.was) {
        return vec![Mount::Efi, Mount::Boot];
    }
    Mount::ALL
        .iter()
        .filter(|at| **at != Mount::Root || marks.format.is_none_or(|fs| rules::made_suits(at, fs)))
        .cloned()
        .collect()
}

/// Whether the Mount point menu of the partition `marks` stands for offers a
/// path of one's own: every partition but a kept EFI system partition.
pub fn offers_other(marks: &Marks) -> bool {
    !(marks.format.is_none() && marks.new.is_none() && rules::kept_esp(&marks.was))
}

/// The filesystems the Format as menu offers a partition used at `at`, in
/// Format as's order: at `/` ext4 and Btrfs alone (`/` is made as nothing
/// else), everywhere else all of them.
pub fn format_offers(at: Option<&Mount>) -> Vec<Fs> {
    Fs::ALL
        .iter()
        .copied()
        .filter(|fs| at != Some(&Mount::Root) || rules::made_suits(&Mount::Root, *fs))
        .collect()
}

/// The partition tables a disk may be given on a machine started by
/// `firmware`: GPT, and msdos only where a BIOS starts it (a UEFI machine
/// starts from GPT, and the worker makes one there).
pub fn tables(firmware: Firmware) -> Vec<Table> {
    match firmware {
        Firmware::Uefi => vec![Table::Gpt],
        Firmware::Bios => vec![Table::Gpt, Table::Msdos],
    }
}

/// A table's name as parted gives it, never translated.
pub fn table_name(table: Table) -> &'static str {
    match table {
        Table::Gpt => "gpt",
        Table::Msdos => "msdos",
    }
}

// -------------------------------------------------------------------------
// Words.

/// One pending operation as GParted says it — "Delete /dev/sda4 (ntfs,
/// 750 MiB)", "Format /dev/sda2 as ext4", "Set mount point of /dev/sda5 to
/// /", "Create gpt partition table on /dev/sdb", "Create Primary Partition
/// #1 (ext4, 120.0 GiB, /) on /dev/sda", "Shrink /dev/sda3 from 372.5 GiB
/// to 250.0 GiB", "Set file system label “Home” on /dev/sda5" — the device,
/// the filesystem, the size and the path handed in as values, never
/// translated. A deleted partition is said as the disk holds it now
/// (`disks`).
pub fn sentence(op: &Operation, disks: &[Disk]) -> String {
    match op {
        Operation::Mount { part, at, .. } => crate::message!(
            "partitions-op-mount",
            "partition" => part_name(part),
            "path" => at.path()
        ),
        Operation::Format { part, fs, .. } => crate::message!(
            "partitions-op-format",
            "partition" => part_name(part),
            "filesystem" => fs.name()
        ),
        Operation::Delete { disk, part } => {
            let was = disks
                .iter()
                .filter(|one| one.path == *disk)
                .find_map(|one| one.partition(part));
            let (filesystem, size) = was.map_or_else(
                || (i18n::text("partitions-unknown").to_string(), String::new()),
                |was| (map::filesystem_words(was).to_string(), map::size(was.size)),
            );
            crate::message!(
                "partitions-op-delete",
                "partition" => part.as_str(),
                "filesystem" => filesystem,
                "size" => size
            )
        }
        Operation::Table { disk, table } => crate::message!(
            "partitions-op-table",
            "table" => table_name(*table),
            "disk" => disk.as_str()
        ),
        Operation::Create { disk, new, .. } => match &new.mount {
            Some(at) => crate::message!(
                "partitions-op-create-at",
                "number" => new.number as i64,
                "filesystem" => new.fs.name(),
                "size" => map::size(new.size),
                "path" => at.path(),
                "disk" => disk.as_str()
            ),
            None => crate::message!(
                "partitions-op-create",
                "number" => new.number as i64,
                "filesystem" => new.fs.name(),
                "size" => map::size(new.size),
                "disk" => disk.as_str()
            ),
        },
        Operation::Resize { part, from, to, .. } if to < from => crate::message!(
            "partitions-op-shrink",
            "partition" => part.as_str(),
            "from" => map::size(*from),
            "to" => map::size(*to)
        ),
        Operation::Resize { part, from, to, .. } => crate::message!(
            "partitions-op-grow",
            "partition" => part.as_str(),
            "from" => map::size(*from),
            "to" => map::size(*to)
        ),
        Operation::Label { part, label, .. } if label.is_empty() => {
            crate::message!("partitions-op-label-clear", "partition" => part.as_str())
        }
        Operation::Label { part, label, .. } => crate::message!(
            "partitions-op-label",
            "label" => label.as_str(),
            "partition" => part.as_str()
        ),
    }
}

/// The details' Pending for a partition, `part` as the plan leaves it: what
/// the plan does to it, in short — "Create", "Format as ext4", "Shrink to
/// 250.0 GiB" — or nothing. The mount point and the label are said in rows
/// of their own over it, so Pending names the mount point ("Mount at /")
/// only where it is all the plan does: said twice, the two did not fit one
/// line on a display standing on its side, and the details ran out of room.
pub fn pending_words(marks: &Marks, part: &Partition) -> Vec<String> {
    if marks.new.is_some() {
        return vec![i18n::text("partitions-do-create").to_string()];
    }
    let mut said = Vec::new();
    if let Some(fs) = marks.format {
        said.push(crate::message!("partitions-do-format", "filesystem" => fs.name()));
    }
    if part.size < marks.was.size {
        said.push(crate::message!("partitions-do-shrink", "size" => map::size(part.size)));
    } else if part.size > marks.was.size {
        said.push(crate::message!("partitions-do-grow", "size" => map::size(part.size)));
    }
    if said.is_empty() {
        if let Some(at) = &marks.mount {
            said.push(crate::message!("partitions-do-mount", "path" => at.path()));
        }
    }
    said
}

// -------------------------------------------------------------------------
// What Continue needs.

/// The plan as a request carries it (`model::layout::DiskLayout`): for every
/// disk the plan does anything on, in the order the machine lists them, the
/// disk as it was reviewed, what becomes of each of its partitions and what
/// is made on it. Sizes come from the resize operations and the reviewed
/// disk — never from [`apply`]'s drawing, which stretches an extended
/// partition to its reach. A partition the plan makes travels as its start,
/// size, filesystem, label and mount point: `new#N` never leaves here.
///
/// Refused where the plan names a disk the machine does not have, or labels
/// a partition it keeps (a label is set only as a filesystem is made).
pub fn layout(plan: &Plan, disks: &[Disk]) -> Result<Vec<DiskLayout>, Refusal> {
    let mut touched: Vec<&str> = Vec::new();
    for op in plan.operations() {
        if !touched.contains(&op.disk()) {
            touched.push(op.disk());
        }
    }
    let mut laid = Vec::new();
    for path in touched {
        let Some(disk) = disks.iter().find(|disk| disk.path == path) else {
            return Err(Refusal::Blocked { disk: path.into() });
        };
        let ours: Vec<&Operation> = plan
            .operations()
            .iter()
            .filter(|op| op.disk() == path)
            .collect();
        let table = ours.iter().rev().find_map(|op| match op {
            Operation::Table { table, .. } => Some(*table),
            _ => None,
        });
        let mut parts = Vec::new();
        for was in &disk.partitions {
            let deleted = ours
                .iter()
                .any(|op| matches!(op, Operation::Delete { part, .. } if *part == was.path));
            let fate = if table.is_some() || deleted {
                Fate::Delete
            } else {
                let mount = plan.mount_of(&was.path);
                let size = plan.size_of(&was.path);
                let label = plan.label_of(&was.path);
                match (plan.format_of(&was.path), label) {
                    (Some(fs), label) => Fate::Format {
                        fs,
                        label: label.unwrap_or_default().to_string(),
                        size: size.unwrap_or(was.size),
                        mount,
                    },
                    (None, Some(_)) => {
                        return Err(Refusal::Mismatch {
                            disk: disk.path.clone(),
                        })
                    }
                    (None, None) => match size {
                        Some(to) => Fate::Shrink { to, mount },
                        None => Fate::Keep { mount },
                    },
                }
            };
            parts.push(Part {
                path: was.path.clone(),
                number: was.number,
                start: was.start,
                size: was.size,
                fate,
            });
        }
        let mut create: Vec<Create> = ours
            .iter()
            .filter_map(|op| match op {
                Operation::Create { new, .. } => Some(Create {
                    start: new.start,
                    size: new.size,
                    fs: new.fs,
                    label: new.label.clone(),
                    mount: new.mount.clone(),
                }),
                _ => None,
            })
            .collect();
        create.sort_by_key(|made| made.start);
        laid.push(DiskLayout {
            disk: disk.clone(),
            table,
            parts,
            create,
        });
    }
    laid.sort_by_key(|one| disks.iter().position(|disk| disk.path == one.disk.path));
    Ok(laid)
}

/// What a plan is judged with besides itself and the disks: the machine's
/// firmware, the boot loader and the restore points chosen, and what reading
/// the disks found (the demo's own numbers; on a live system none yet, so a
/// shrink is not offered and a kept EFI partition counts as unread).
pub struct Judge<'a> {
    pub firmware: Firmware,
    pub bootloader: Bootloader,
    pub software: &'a Software,
    pub measures: &'a Measures,
    pub images: &'a ImageSizes,
}

/// The first thing Continue still needs, or what the plan does that cannot
/// be: the rule set's own answer (`model::layout`), the very rules the
/// worker judges a request by — [`layout`], then `shape`, then `measured`
/// on the measurements in hand. `None` once nothing is refused.
///
/// Restore points are judged as they will be asked for: switched off beside
/// a separate `/var` (decision 8 of Unit Q), which is no refusal of the
/// plan.
pub fn missing(plan: &Plan, disks: &[Disk], judge: &Judge) -> Option<Refusal> {
    let laid = match layout(plan, disks) {
        Ok(laid) => laid,
        Err(refusal) => return Some(refusal),
    };
    let software = Software {
        snapshots: judge.software.snapshots && !rules::separate_var(&laid),
        ..judge.software.clone()
    };
    rules::shape(&laid, judge.firmware, judge.bootloader, &software)
        .and_then(|()| rules::measured(&laid, judge.measures, judge.images))
        .err()
}

/// How a refusal names a partition: its device, or for one the plan makes
/// "New Partition #N", found in `plan` by its disk and its start.
pub fn which_name(which: &Which, plan: &Plan) -> String {
    match which {
        Which::Part(path) => path.clone(),
        Which::Made { disk, start } => plan
            .operations()
            .iter()
            .find_map(|op| match op {
                Operation::Create { disk: on, new, .. } if on == disk && new.start == *start => {
                    Some(part_name(&new.path()))
                }
                _ => None,
            })
            .unwrap_or_else(|| disk.clone()),
    }
}

/// What the foot says of a refusal, and Continue is refused with: from the
/// catalogs, the devices, paths, filesystems and sizes handed in as values,
/// a partition the plan makes named as GParted names it (`plan`).
pub fn refusal_words(refusal: &Refusal, plan: &Plan) -> String {
    let name = |which: &Which| which_name(which, plan);
    match refusal {
        Refusal::Blocked { disk } => {
            crate::message!("partitions-blocked", "disk" => disk.as_str())
        }
        Refusal::Mismatch { disk } => {
            crate::message!("partitions-refused-mismatch", "disk" => disk.as_str())
        }
        Refusal::Unaligned { part } => {
            crate::message!("partitions-refused-unaligned", "partition" => name(part))
        }
        Refusal::Outside { part } => {
            crate::message!("partitions-refused-outside", "partition" => name(part))
        }
        Refusal::Overlap { part } => {
            crate::message!("partitions-refused-overlap", "partition" => name(part))
        }
        Refusal::Primaries { disk } => {
            crate::message!("partitions-primaries", "disk" => disk.as_str())
        }
        Refusal::BesideExtended { disk } => {
            crate::message!("partitions-refused-beside-extended", "disk" => disk.as_str())
        }
        Refusal::LogicalOrder { part } => {
            crate::message!("partitions-refused-logical", "partition" => name(part))
        }
        Refusal::Entries { disk } => {
            crate::message!("partitions-refused-entries", "disk" => disk.as_str())
        }
        Refusal::CantShrink { part } => {
            crate::message!("partitions-refused-shrink", "partition" => name(part))
        }
        Refusal::Grows { part } => {
            crate::message!("partitions-refused-grow", "partition" => name(part))
        }
        Refusal::BiosGrub { part } => {
            crate::message!("partitions-refused-bios-grub", "partition" => name(part))
        }
        Refusal::Label { part } => {
            crate::message!("partitions-refused-label", "partition" => name(part))
        }
        Refusal::Twice { at } => crate::message!("partitions-twice", "path" => at.path()),
        Refusal::Root => i18n::text("partitions-missing-root").to_string(),
        Refusal::TooSmall { at, least } => crate::message!(
            "partitions-too-small",
            "path" => at.path(),
            "size" => map::size(*least)
        ),
        Refusal::MustFormat { part, at } => crate::message!(
            "partitions-must-format",
            "partition" => name(part),
            "path" => at.path()
        ),
        Refusal::Needs { at } => match at {
            Mount::Root => crate::message!("partitions-needs-root", "path" => at.path()),
            Mount::Home | Mount::Other(_) => {
                crate::message!("partitions-needs-linux", "path" => at.path())
            }
            Mount::Efi | Mount::Boot => crate::message!(
                "partitions-needs",
                "path" => at.path(),
                "filesystem" => Fs::Fat32.name()
            ),
            Mount::Swap => crate::message!(
                "partitions-needs",
                "path" => at.path(),
                "filesystem" => Fs::LinuxSwap.name()
            ),
        },
        Refusal::Unmountable { part, fs } => {
            let filesystem = match fs.as_str() {
                "" => i18n::text("partitions-unknown").to_string(),
                fs => fs.to_string(),
            };
            crate::message!(
                "partitions-refused-unmountable",
                "filesystem" => filesystem,
                "partition" => name(part)
            )
        }
        Refusal::Path { why } => why.words(),
        Refusal::InsideKept { at, .. } => {
            crate::message!("partitions-refused-inside-kept", "path" => at.path())
        }
        Refusal::FsTooSmall { part, fs, .. } => crate::message!(
            "partitions-refused-fs-small",
            "partition" => name(part),
            "filesystem" => fs.name()
        ),
        Refusal::RestorePoints => i18n::text("partitions-refused-restore-points").to_string(),
        Refusal::Efi => i18n::text("partitions-missing-efi").to_string(),
        Refusal::Boot => i18n::text("partitions-missing-boot").to_string(),
        Refusal::BootElsewhere { beside } => {
            crate::message!("partitions-refused-boot-disk", "path" => beside.path())
        }
        Refusal::EfiOnBios => i18n::text("partitions-refused-efi-bios").to_string(),
        Refusal::BiosGrubMissing { disk } => {
            crate::message!("partitions-missing-bios-grub", "disk" => disk.as_str())
        }
        Refusal::GrubGap { disk } => {
            crate::message!("partitions-refused-grub-gap", "disk" => disk.as_str())
        }
        Refusal::Bootloader { loader } => crate::message!(
            "partitions-refused-bootloader",
            "loader" => crate::ui::screens::desktop::bootloader_name(*loader)
        ),
        Refusal::NotRead { part } => {
            crate::message!("partitions-refused-not-read", "partition" => name(part))
        }
        Refusal::ShrinkBelow { part, least } => crate::message!(
            "partitions-refused-shrink-below",
            "partition" => name(part),
            "size" => map::size(*least)
        ),
        Refusal::Unshrinkable { part, why } => unshrinkable_words(&name(part), why),
        Refusal::EspFull { part, least } => crate::message!(
            "partitions-refused-esp-full",
            "partition" => name(part),
            "size" => map::size(*least)
        ),
        Refusal::ImageUnknown { at } => {
            crate::message!("partitions-refused-image-unknown", "path" => at.path())
        }
    }
}

/// Why `partition` cannot be shrunk, as reading said, in a line beside the
/// buttons — what the foot says of a shrink refused for it. Of it:
/// "/dev/sda3 can't be shrunk: Windows is hibernated", "… until it is
/// checked", "… is in use", "… is encrypted", "… can't be read", and
/// "/dev/sda5 can't be shrunk here: shrink it in Windows" of an NTFS with no
/// hibernation file of its own — a Windows' data partition, or the
/// partition of a Windows whose hibernation is switched off, which reading
/// cannot tell apart, so the words hold for both; of another Windows volume
/// of the measurement: Windows hibernated (the same words), "Can't shrink:
/// Windows on /dev/sdc1 couldn't be read", "Can't shrink: /dev/sdc1 is
/// locked by BitLocker". The details under the partition say what to do
/// ([`unshrunk_words`]).
pub fn unshrinkable_words(partition: &str, why: &Unshrinkable) -> String {
    match why {
        Unshrinkable::Itself(Measure::Hibernated) | Unshrinkable::Hibernated { .. } => {
            crate::message!("partitions-refused-hibernated", "partition" => partition)
        }
        Unshrinkable::Itself(Measure::NeedsCheck) => {
            crate::message!("partitions-refused-needs-check", "partition" => partition)
        }
        Unshrinkable::Itself(Measure::Encrypted) => {
            crate::message!("partitions-refused-encrypted", "partition" => partition)
        }
        Unshrinkable::Itself(Measure::InUse) => {
            crate::message!("partitions-refused-in-use", "partition" => partition)
        }
        Unshrinkable::Itself(
            Measure::Unreadable | Measure::Read { .. } | Measure::NoHiberfile { .. },
        ) => crate::message!("partitions-refused-unreadable", "partition" => partition),
        Unshrinkable::NoHiberfile => {
            crate::message!("partitions-refused-no-hiberfile", "partition" => partition)
        }
        Unshrinkable::Unread { other } => {
            crate::message!("partitions-refused-windows-unread", "other" => other.as_str())
        }
        Unshrinkable::BitLocker { other } => {
            crate::message!("partitions-refused-bitlocker", "other" => other.as_str())
        }
    }
}

/// What the details say under a kept ext4 or NTFS that Resize is not
/// offered for, as reading said — and what to do, where something can be
/// done: "Windows on /dev/sdc1: shut it down with Fast Startup off" (of a
/// Windows hibernated there, or on another partition of the measurement),
/// "Can't shrink until Windows on /dev/sdc1 can be read", "To shrink, turn
/// off BitLocker on /dev/sdb2", and "To shrink it, use Disk Management in
/// Windows" of an NTFS with no hibernation file of its own (Windows itself
/// can shrink it, whether it is a data partition or Windows' own with its
/// hibernation switched off) — the partition itself the one the light is
/// on, so only the other is named, in the two lines the narrowest display
/// gives them; otherwise the foot's own words ([`unshrinkable_words`]).
pub fn unshrunk_words(partition: &str, why: &Unshrinkable) -> String {
    match why {
        Unshrinkable::Itself(Measure::Hibernated) => {
            crate::message!("partitions-why-hibernated", "other" => partition)
        }
        Unshrinkable::Hibernated { other } => {
            crate::message!("partitions-why-hibernated", "other" => other.as_str())
        }
        Unshrinkable::Unread { other } => {
            crate::message!("partitions-why-windows-unread", "other" => other.as_str())
        }
        Unshrinkable::BitLocker { other } => {
            crate::message!("partitions-why-bitlocker", "other" => other.as_str())
        }
        Unshrinkable::NoHiberfile => i18n::text("partitions-why-no-hiberfile").to_string(),
        Unshrinkable::Itself(_) => unshrinkable_words(partition, why),
    }
}

/// What a live system says when Continue would carry out a plan that needs
/// `capability`, which no scenario this version passed in QEMU needed
/// (`model::layout::PROVEN`): in the foot's own line, as Partitions has
/// always said it cannot be applied.
pub fn capability_words(capability: Capability) -> &'static str {
    match capability {
        Capability::FreshTable => i18n::text("partitions-not-yet-fresh-table"),
        // Partitions made, and partitions deleted — primary or logical — as
        // the one capability they were split from said them: "Making or
        // deleting partitions".
        Capability::KeepTable | Capability::DeleteEntries | Capability::DeleteLogical => {
            i18n::text("partitions-not-yet-keep-table")
        }
        Capability::ChangeEntries => i18n::text("partitions-not-yet-change-entries"),
        Capability::KeptHome => i18n::text("partitions-not-yet-kept-home"),
        Capability::SeveralDisks => i18n::text("partitions-not-yet-several-disks"),
        Capability::Bios => i18n::text("partitions-not-yet-bios"),
        // A formatted partition's entry made smaller or grown, primary or
        // logical (which "Formatting a partition" does not name), a machine
        // UEFI starts, a kind of table, a logical partition, the frame left
        // empty, the extended frame, `/` or `/boot` on a logical partition,
        // the boot flag set, a `/boot` under BIOS or a disk whose sectors
        // are not 512 bytes: Partitions' own sentence, which says that the
        // plan cannot be applied yet, fits each whole.
        Capability::SmallerEntry
        | Capability::LargerEntry
        | Capability::LargerLogical
        | Capability::Uefi
        | Capability::Gpt
        | Capability::Msdos
        | Capability::Logical
        | Capability::EmptyFrame
        | Capability::Extended
        | Capability::RootOnLogical
        | Capability::Activate
        | Capability::BootOnBios
        | Capability::LargeSectors => i18n::text("partitions-not-yet"),
        // Either filesystem shrunk, as the one shrink they were split from
        // said it: shrinking is what is refused.
        Capability::ShrinkExt4 | Capability::ShrinkNtfs => i18n::text("partitions-not-yet-shrink"),
    }
}

/// What a live system says when Continue would carry out a plan this
/// version cannot yet (`model::layout::unproven`): a capability no scenario
/// that passed needed, in its own words (`capability_words`); a mix whose
/// every capability some scenario needed, but no one scenario all of them,
/// Partitions' own sentence — "Partitions can't be applied in this version
/// yet." — since no one thing it asks is what is missing.
pub fn unproven_words(unproven: &Unproven) -> &'static str {
    match unproven {
        Unproven::Never(capability) => capability_words(*capability),
        Unproven::Together(_) => i18n::text("partitions-not-yet"),
    }
}

/// Whether the plan uses a partition as swap: Options then offers no swap
/// file.
pub fn swap_partition(plan: &Plan) -> bool {
    plan.uses(&Mount::Swap)
}

/// The stop of `planned`'s map that holds the byte at `offset` — a
/// partition starting there first, else the run of unallocated space or the
/// partition it falls in — for the light to stay on what it was on, or on
/// what a deleted partition became.
pub fn stop_at(planned: &Planned, offset: u64) -> Option<usize> {
    let walk = map::stops(&map::tiles(&planned.disk));
    let span = |stop: &Stop| -> (u64, u64) {
        match stop {
            Stop::Part(at) | Stop::Extended(at) => {
                let part = &planned.disk.partitions[*at];
                (part.start, part.start + part.size.max(1))
            }
            Stop::Free { start, size } => (*start, start + size),
            Stop::Whole => (0, planned.disk.size),
        }
    };
    walk.iter()
        .position(|stop| matches!(stop, Stop::Part(_)) && span(stop).0 == offset)
        .or_else(|| {
            walk.iter().position(|stop| {
                !matches!(stop, Stop::Extended(_)) && {
                    let (start, end) = span(stop);
                    (start..end).contains(&offset)
                }
            })
        })
        .or_else(|| {
            // Between two tiles (space too small to list): the nearest after.
            walk.iter().position(|stop| span(stop).0 >= offset)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::{GIB, MIN_GAP, MIN_INSTALL};
    use crate::model::layout::{BIOS_BOOT_SIZE, ESP_BOOT_FREE};
    use crate::ui::demo;

    /// The made-up machine's Windows disk.
    fn sda() -> Disk {
        demo::disks().remove(1)
    }

    /// The old msdos disk Partitions shows: Windows, an extended partition
    /// holding Linux, its swap and FAT32, and free space.
    fn sdc() -> Disk {
        demo::partition_disks().remove(0)
    }

    /// The Linux disk started by BIOS, with a `bios_grub`.
    fn sdd() -> Disk {
        demo::partition_disks().remove(1)
    }

    /// The old msdos disk with its logical partitions numbered out of the
    /// disk's order, as a table whose logical partitions were made in
    /// another order numbers them (the kernel numbers them as their tables
    /// are chained): on the disk sdc6 (debian), sdc7 (its swap), then sdc5
    /// (SHARED), listed by number as discovery lists them.
    fn jumbled() -> Disk {
        let mut disk = sdc();
        let mut logical = disk.partitions.split_off(2);
        for (part, number) in logical.iter_mut().zip([6, 7, 5]) {
            part.number = number;
            part.path = format!("/dev/sdc{number}");
            part.uuid = format!("0000JMBL-{number:04}");
        }
        logical.sort_by_key(|part| part.number);
        disk.partitions.extend(logical);
        disk
    }

    /// The Linux disk with something written over the whole of it beside
    /// its gpt — as lsblk reports a stick a hybrid ISO image was written to,
    /// or a disk once formatted whole and partitioned since without its old
    /// signature wiped: no table and no partition is made over it. Its swap
    /// and its LUKS end 2 GiB short of what comes after them, and as
    /// discovery lists such a disk no free space is listed there: what lies
    /// in those gaps may be the whole-disk content's.
    fn written_over() -> Disk {
        let mut disk = sdd();
        disk.content = Some(Content::Other);
        disk.partitions[2].size -= 2 * GIB;
        disk.partitions[4].size -= 2 * GIB;
        disk.free.clear();
        disk
    }

    /// The Linux disk with logical sectors of 8192 bytes, larger than the
    /// rule set lays partitions out in (`layout::sector_fits`).
    fn wide_sectors() -> Disk {
        let mut disk = sdd();
        disk.logical_sector = 8192;
        disk
    }

    /// A blank disk, no table on it yet, with logical sectors of 8192 bytes.
    fn wide_blank() -> Disk {
        let mut disk = wide_sectors();
        disk.table = None;
        disk.partitions.clear();
        disk.free.clear();
        disk
    }

    fn all() -> Vec<Disk> {
        demo::disks()
            .into_iter()
            .chain(demo::partition_disks())
            .collect()
    }

    fn names(planned: &Planned) -> Vec<String> {
        map::stops(&map::tiles(&planned.disk))
            .iter()
            .map(|stop| match stop {
                Stop::Part(at) | Stop::Extended(at) => {
                    map::device(&planned.disk.partitions[*at].path).to_string()
                }
                Stop::Free { size, .. } => format!("free {}", size / MIB),
                Stop::Whole => "whole".into(),
            })
            .collect()
    }

    /// **A blank disk too small for Linexin is offered a table and
    /// partitions, and may hold what starts the computer** (Unit Q, Q6's
    /// disk B): an 8 GiB disk with no table, too small for `/`
    /// (`TooSmall`), is offered a partition table where one stands on its
    /// unallocated space, and New… there once it has one. With the EFI
    /// system partition at `/efi` and a FAT32 `/boot` made on it, `/` and
    /// swap on the NVMe drive and the Linux SSD's home kept at `/home`,
    /// nothing is refused: the plan travels as those three disks, needing a
    /// new table, a kept home and several disks. A `/` made on the small
    /// disk instead is refused for its size.
    #[test]
    fn a_blank_disk_too_small_for_linexin_may_hold_what_starts_the_computer() {
        use crate::model::disks::{Blocked, Transport};
        let mut small = demo::disks().remove(0);
        small.path = "/dev/sde".into();
        small.major_minor = "8:64".into();
        small.diskseq = Some(6);
        small.model = "Example Small SSD".into();
        small.serial = "DEMO-SATA-0006".into();
        small.transport = Transport::Sata;
        small.size = 8 * GIB;
        small.free = vec![Gap {
            start: MIB,
            size: 8 * GIB - 2 * MIB,
        }];
        small.blocked = Some(Blocked::TooSmall);
        let disks: Vec<Disk> = all().into_iter().chain([small.clone()]).collect();
        let measures = demo::measures(&disks);
        let offered = |plan: &Plan| {
            let planned = apply(plan, &small, Firmware::Uefi);
            let stops = map::stops(&map::tiles(&planned.disk));
            assert_eq!(stops.len(), 1, "{stops:?}");
            (tile_rows(plan, &planned, stops[0], &measures), planned)
        };
        let mut plan = Plan::default();
        assert_eq!(offered(&plan).0, [TileRow::Table]);
        plan.table("/dev/sde", Table::Gpt);
        let (rows, planned) = offered(&plan);
        assert_eq!(rows, [TileRow::New]);
        let free = planned.disk.free[0];
        let made = |plan: &mut Plan, disk: &str, start, size, fs, mount| {
            let number = plan.next_number();
            plan.create(
                disk,
                NewPart {
                    number,
                    start,
                    size,
                    fs,
                    mount: Some(mount),
                    label: String::new(),
                },
            );
        };
        made(
            &mut plan,
            "/dev/sde",
            free.start,
            GIB,
            Fs::Fat32,
            Mount::Efi,
        );
        let boot = free.start + GIB;
        made(&mut plan, "/dev/sde", boot, 2 * GIB, Fs::Fat32, Mount::Boot);
        let nvme = disks.iter().find(|disk| disk.path == "/dev/nvme0n1");
        plan.table("/dev/nvme0n1", Table::Gpt);
        let room = apply(&plan, nvme.unwrap(), Firmware::Uefi).disk.free[0];
        let root = room.size - 4 * GIB;
        made(
            &mut plan,
            "/dev/nvme0n1",
            room.start,
            root,
            Fs::Ext4,
            Mount::Root,
        );
        let swap = room.start + root;
        made(
            &mut plan,
            "/dev/nvme0n1",
            swap,
            4 * GIB,
            Fs::LinuxSwap,
            Mount::Swap,
        );
        plan.mount("/dev/sdd", "/dev/sdd4", Some(Mount::Home));
        assert_eq!(judged(&plan, &disks, Firmware::Uefi, &measures), None);
        let laid = layout(&plan, &disks).unwrap();
        let mut paths: Vec<&str> = laid.iter().map(|one| one.disk.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(paths, ["/dev/nvme0n1", "/dev/sdd", "/dev/sde"]);
        assert_eq!(
            rules::needs(&laid, Firmware::Uefi),
            [
                rules::Capability::FreshTable,
                rules::Capability::KeptHome,
                rules::Capability::SeveralDisks,
                rules::Capability::Uefi,
                rules::Capability::Gpt
            ]
        );
        // `/` on the small disk: refused for its size.
        let mut on_small = Plan::default();
        on_small.table("/dev/sde", Table::Gpt);
        made(
            &mut on_small,
            "/dev/sde",
            free.start,
            GIB,
            Fs::Fat32,
            Mount::Boot,
        );
        let rest = 8 * GIB - 3 * MIB - GIB;
        made(&mut on_small, "/dev/sde", boot, rest, Fs::Ext4, Mount::Root);
        assert_eq!(
            judged(&on_small, &disks, Firmware::Uefi, &measures),
            Some(Refusal::TooSmall {
                at: Mount::Root,
                least: MIN_INSTALL
            })
        );
    }

    /// A disk whose logical sector is larger than the rule set lays
    /// partitions out in is refused whole by it, whatever a plan asks of it:
    /// so nothing is offered on it — no new table, from Options or from its
    /// unallocated space, no New…, nothing on a partition — and the details
    /// say "Nothing can be changed on" it. The same disk at 4096 bytes a
    /// sector is offered all of it.
    #[test]
    fn nothing_is_offered_on_a_disk_whose_sector_the_rules_refuse() {
        let measures = Measures::default();
        let nothing = "Nothing can be changed on /dev/sdd".to_string();
        for disk in [wide_sectors(), wide_blank()] {
            let planned = apply(&Plan::default(), &disk, Firmware::Uefi);
            assert!(!table_offered(&planned), "{:?}", disk.table);
            assert_eq!(sectors_held_back(&planned), Some(nothing.clone()));
            let stops = map::stops(&map::tiles(&planned.disk));
            assert!(!stops.is_empty());
            for stop in stops {
                assert_eq!(
                    tile_rows(&Plan::default(), &planned, stop, &measures),
                    [],
                    "{stop:?}"
                );
            }
        }
        // Why: anything a plan asks of it is refused whole.
        let wide = wide_sectors();
        let mut plan = Plan::default();
        plan.mount("/dev/sdd", "/dev/sdd4", Some(Mount::Home));
        let laid = layout(&plan, std::slice::from_ref(&wide)).unwrap();
        assert_eq!(
            rules::shape(
                &laid,
                Firmware::Uefi,
                Bootloader::Automatic,
                &Software::default()
            ),
            Err(Refusal::Mismatch {
                disk: "/dev/sdd".into()
            })
        );
        // At 4096 bytes a sector: the table, and the menus, offered.
        let mut fits = wide_sectors();
        fits.logical_sector = 4096;
        let planned = apply(&Plan::default(), &fits, Firmware::Uefi);
        assert!(table_offered(&planned));
        assert_eq!(sectors_held_back(&planned), None);
        let home = planned.index_of("/dev/sdd4").unwrap();
        assert!(!tile_rows(&Plan::default(), &planned, Stop::Part(home), &measures).is_empty());
        let mut blank = wide_blank();
        blank.logical_sector = 4096;
        let planned = apply(&Plan::default(), &blank, Firmware::Uefi);
        let stop = map::stops(&map::tiles(&planned.disk))[0];
        assert_eq!(
            tile_rows(&Plan::default(), &planned, stop, &measures),
            [TileRow::Table]
        );
    }

    #[test]
    fn a_mount_point_set_twice_is_one_operation_and_none_takes_it_away() {
        let mut plan = Plan::default();
        assert!(plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Home)));
        assert!(plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root)));
        assert_eq!(plan.len(), 1);
        assert_eq!(plan.mount_of("/dev/sda5"), Some(Mount::Root));
        assert!(!plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root)));
        assert!(plan.mount("/dev/sda", "/dev/sda5", None));
        assert!(plan.is_empty());
        // None on a partition the plan never touched changes nothing.
        assert!(!plan.mount("/dev/sda", "/dev/sda3", None));
    }

    #[test]
    fn a_format_again_replaces_the_one_before_and_keep_takes_it_away() {
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Btrfs));
        // The folded format is the latest thing asked for.
        assert_eq!(
            plan.operations().last(),
            Some(&Operation::Format {
                disk: "/dev/sda".into(),
                part: "/dev/sda5".into(),
                fs: Fs::Btrfs
            })
        );
        assert_eq!(plan.len(), 2);
        assert!(plan.format("/dev/sda", "/dev/sda5", None));
        assert_eq!(plan.len(), 1);
        assert_eq!(plan.format_of("/dev/sda5"), None);
    }

    #[test]
    fn deleting_takes_away_what_was_to_be_done_to_the_partition() {
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda4", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda4", Some(Mount::Home));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        plan.delete("/dev/sda", "/dev/sda4");
        assert_eq!(
            plan.operations(),
            [
                Operation::Mount {
                    disk: "/dev/sda".into(),
                    part: "/dev/sda5".into(),
                    at: Mount::Root
                },
                Operation::Delete {
                    disk: "/dev/sda".into(),
                    part: "/dev/sda4".into()
                }
            ]
        );
        // Undo the last: the partition is back, as it was — the format the
        // delete folded away was no longer on the list.
        assert!(plan.undo_last());
        assert!(!plan.on_part("/dev/sda4"));
        assert_eq!(plan.len(), 1);
    }

    #[test]
    fn a_new_table_takes_away_everything_else_on_its_disk_only() {
        let mut plan = Plan::default();
        plan.delete("/dev/sda", "/dev/sda4");
        plan.format("/dev/sdd", "/dev/sdd2", Some(Fs::Ext4));
        plan.table("/dev/sda", Table::Msdos);
        plan.table("/dev/sda", Table::Gpt);
        assert_eq!(plan.on_disk("/dev/sda"), 1);
        assert_eq!(plan.on_disk("/dev/sdd"), 1);
        assert_eq!(
            plan.operations().last(),
            Some(&Operation::Table {
                disk: "/dev/sda".into(),
                table: Table::Gpt
            })
        );
    }

    #[test]
    fn undo_takes_the_last_line_or_every_line_of_a_partition_and_clear_all() {
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda1", Some(Mount::Efi));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        assert!(plan.undo_part("/dev/sda5"));
        assert_eq!(plan.len(), 1);
        assert!(!plan.undo_part("/dev/sda5"));
        assert!(plan.undo_last());
        assert!(!plan.undo_last());
        plan.delete("/dev/sda", "/dev/sda4");
        plan.table("/dev/sdd", Table::Gpt);
        plan.clear();
        assert!(plan.is_empty());
    }

    #[test]
    fn a_disk_the_plan_does_not_touch_is_the_disk_itself() {
        let mut plan = Plan::default();
        plan.format("/dev/sdd", "/dev/sdd2", Some(Fs::Ext4));
        for disk in all() {
            if disk.path == "/dev/sdd" {
                continue;
            }
            let planned = apply(&plan, &disk, Firmware::Uefi);
            assert_eq!(planned.disk, disk);
            assert!(planned.deleted.is_empty() && planned.table.is_none());
        }
    }

    #[test]
    fn a_deleted_partition_is_unallocated_merged_with_the_space_beside_it() {
        let disk = sda();
        let mut plan = Plan::default();
        // The recovery partition, right after the 279 GiB gap.
        plan.delete("/dev/sda", "/dev/sda4");
        let planned = apply(&plan, &disk, Firmware::Uefi);
        let gap = disk.free[0];
        let run = format!("free {}", gap.size / MIB + 750);
        assert_eq!(
            names(&planned),
            ["sda1", "sda2", "sda3", run.as_str(), "sda5"]
        );
        assert_eq!(planned.deleted.len(), 1);
        let merged = planned.disk.free[0];
        assert_eq!(merged.start, gap.start);
        assert_eq!(planned.deleted_in(merged.start, merged.size).len(), 1);
        // Microsoft's 16 MiB, too small to be listed, is a run all the same
        // once it is deleted: room the person made.
        plan.delete("/dev/sda", "/dev/sda2");
        let planned = apply(&plan, &disk, Firmware::Uefi);
        assert_eq!(names(&planned)[1], "free 16");
        // Windows between them deleted too: one run from the EFI partition
        // to Data.
        plan.delete("/dev/sda", "/dev/sda3");
        let planned = apply(&plan, &disk, Firmware::Uefi);
        let parts = disk.partitions.clone();
        assert_eq!(names(&planned).len(), 3, "{:?}", names(&planned));
        assert_eq!(planned.disk.free[0].start, parts[0].start + parts[0].size);
        assert_eq!(
            planned.disk.free[0].start + planned.disk.free[0].size,
            parts[4].start
        );
        // The light finds what a deleted partition became.
        assert_eq!(stop_at(&planned, parts[3].start), Some(1));
        assert_eq!(stop_at(&planned, parts[4].start), Some(2));
    }

    #[test]
    fn deleting_the_last_partition_reaches_the_end_of_the_room() {
        let disk = sda();
        let mut plan = Plan::default();
        plan.delete("/dev/sda", "/dev/sda5");
        let planned = apply(&plan, &disk, Firmware::Uefi);
        let last = planned.disk.free.last().unwrap();
        let (_, end) = disks::usable(Table::Gpt, disk.size, disk.logical_sector);
        assert_eq!(last.start + last.size, end);
        assert_eq!(last.start % MIB, 0);
    }

    #[test]
    fn a_deleted_logical_partition_leaves_its_space_inside_the_extended_frame() {
        let disk = sdc();
        let mut plan = Plan::default();
        plan.delete("/dev/sdc", "/dev/sdc6");
        let planned = apply(&plan, &disk, Firmware::Uefi);
        let tiles = map::tiles(&planned.disk);
        let inside = tiles
            .iter()
            .find_map(|tile| match tile {
                Tile::Extended { inside, .. } => Some(inside.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(inside.len(), 3, "{inside:?}");
        assert!(matches!(inside[1], Tile::Free { .. }));
        // Nothing left in it: the frame alone may be deleted, and its whole
        // reach then joins the free space after it.
        let ext = |planned: &Planned| {
            map::stops(&map::tiles(&planned.disk))
                .into_iter()
                .find(|stop| matches!(stop, Stop::Extended(_)))
        };
        assert!(tile_rows(
            &plan,
            &planned,
            ext(&planned).unwrap(),
            &Measures::default()
        )
        .is_empty());
        plan.delete("/dev/sdc", "/dev/sdc5");
        plan.delete("/dev/sdc", "/dev/sdc7");
        let planned = apply(&plan, &disk, Firmware::Uefi);
        let frame = ext(&planned).unwrap();
        assert_eq!(
            tile_rows(&plan, &planned, frame, &Measures::default()),
            [TileRow::Delete]
        );
        plan.delete("/dev/sdc", "/dev/sdc2");
        let planned = apply(&plan, &disk, Firmware::Uefi);
        assert_eq!(names(&planned).len(), 2, "{:?}", names(&planned));
        let free = planned.disk.free[0];
        assert_eq!(free.start, disk.partitions[1].start);
        assert_eq!(
            free.start + free.size,
            disk.free[0].start + disk.free[0].size
        );
    }

    #[test]
    fn a_new_table_leaves_one_run_of_unallocated_space() {
        for (path, table) in [
            ("/dev/sda", Table::Gpt),
            ("/dev/sdc", Table::Msdos),
            ("/dev/nvme0n1", Table::Gpt),
        ] {
            let disk = all().into_iter().find(|disk| disk.path == path).unwrap();
            let mut plan = Plan::default();
            plan.table(path, table);
            let planned = apply(&plan, &disk, Firmware::Uefi);
            assert_eq!(planned.disk.table, Some(table));
            let walk = map::stops(&map::tiles(&planned.disk));
            assert_eq!(walk.len(), 1, "{path}");
            assert!(matches!(walk[0], Stop::Free { start: MIB, .. }), "{path}");
            assert_eq!(planned.deleted.len(), disk.partitions.len());
        }
    }

    #[test]
    fn a_formatted_partition_holds_its_new_filesystem_and_no_label() {
        let disk = sda();
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        let planned = apply(&plan, &disk, Firmware::Uefi);
        let part = &planned.disk.partitions[4];
        assert_eq!(map::filesystem(part), Some("ext4"));
        assert!(part.label.is_empty());
        assert_eq!(planned.parts[4].was.label, "Data");
        assert_eq!(planned.parts[4].mount, Some(Mount::Root));
        // Pending says the format; the mount point has a row of its own.
        assert_eq!(
            pending_words(&planned.parts[4], &planned.disk.partitions[4]),
            ["Format as ext4"]
        );
        let mut only = Plan::default();
        only.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        let alone = apply(&only, &disk, Firmware::Uefi);
        assert_eq!(
            pending_words(&alone.parts[4], &alone.disk.partitions[4]),
            ["Mount at /"]
        );
    }

    /// What the demo reads of the made-up machine's partitions.
    fn read() -> Measures {
        demo::measures(&all())
    }

    #[test]
    fn each_tile_offers_what_can_be_done_to_it() {
        let disk = sda();
        let mut plan = Plan::default();
        let planned = apply(&plan, &disk, Firmware::Uefi);
        let measures = read();
        let rows = |plan: &Plan, planned: &Planned, at| {
            tile_rows(plan, planned, Stop::Part(at), &measures)
        };
        // The EFI partition, kept: a mount point, a format, a deletion — no
        // Resize (FAT is not shrunk) and no Label (a label is set only as a
        // filesystem is made).
        assert_eq!(
            rows(&plan, &planned, 0),
            [TileRow::Mount, TileRow::Format, TileRow::Delete]
        );
        // Windows, NTFS read and signed not hibernated by its own
        // hibernation file: Resize too, which only shrinks. Data, a
        // Windows' data volume read with no hibernation file of its own:
        // never Resize.
        assert_eq!(
            rows(&plan, &planned, 2),
            [
                TileRow::Mount,
                TileRow::Format,
                TileRow::Resize,
                TileRow::Delete
            ]
        );
        assert_eq!(
            rows(&plan, &planned, 4),
            [TileRow::Mount, TileRow::Format, TileRow::Delete]
        );
        // Unread (a live system before it is measured), no Resize.
        assert_eq!(
            tile_rows(&plan, &planned, Stop::Part(2), &Measures::default()),
            [TileRow::Mount, TileRow::Format, TileRow::Delete]
        );
        // Microsoft's reserved space: only deleted.
        assert_eq!(rows(&plan, &planned, 1), [TileRow::Delete]);
        // Unallocated space: New….
        let gap = disk.free[0];
        let free = Stop::Free {
            start: gap.start,
            size: gap.size,
        };
        assert_eq!(tile_rows(&plan, &planned, free, &measures), [TileRow::New]);
        // Formatted, a label may be set; Undo once something is planned.
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        let planned = apply(&plan, &disk, Firmware::Uefi);
        assert!(rows(&plan, &planned, 4).contains(&TileRow::Label));
        assert_eq!(rows(&plan, &planned, 4).last(), Some(&TileRow::Undo));
        assert_eq!(rows(&plan, &planned, 2).last(), Some(&TileRow::Delete));
        // A bios_grub: only deleted.
        let linux = sdd();
        let planned = apply(&Plan::default(), &linux, Firmware::Uefi);
        assert_eq!(
            tile_rows(&Plan::default(), &planned, Stop::Part(0), &measures),
            [TileRow::Delete]
        );
        // An extended partition with logical ones in it: nothing.
        let old = sdc();
        let planned = apply(&Plan::default(), &old, Firmware::Uefi);
        assert!(tile_rows(&Plan::default(), &planned, Stop::Extended(1), &measures).is_empty());
    }

    #[test]
    fn what_cannot_be_mounted_has_no_mount_point_and_the_details_say_why() {
        // An encrypted partition and one holding nothing, kept: no Mount
        // point; formatted, it has one.
        let mut disk = sda();
        disk.partitions[4].fs_type = "crypto_LUKS".into();
        disk.partitions[4].content = Content::Other;
        let planned = apply(&Plan::default(), &disk, Firmware::Uefi);
        let rows = tile_rows(&Plan::default(), &planned, Stop::Part(4), &read());
        assert_eq!(rows, [TileRow::Format, TileRow::Delete]);
        assert_eq!(
            held_back(&planned, 4).as_deref(),
            Some("crypto_LUKS can't be mounted: format it to use it")
        );
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        let formatted = apply(&plan, &disk, Firmware::Uefi);
        assert!(tile_rows(&plan, &formatted, Stop::Part(4), &read()).contains(&TileRow::Mount));
        assert_eq!(held_back(&formatted, 4), None);
        disk.partitions[4].fs_type.clear();
        disk.partitions[4].content = Content::Empty;
        let planned = apply(&Plan::default(), &disk, Firmware::Uefi);
        assert_eq!(
            held_back(&planned, 4).as_deref(),
            Some("No file system: format it to use it")
        );
        // Swap kept is no mount either, until it is made swap afresh.
        let linux = sdd();
        let planned = apply(&Plan::default(), &linux, Firmware::Uefi);
        let swap = tile_rows(&Plan::default(), &planned, Stop::Part(2), &read());
        assert!(!swap.contains(&TileRow::Mount));
        // Windows, the ext4 /home, and the EFI partition mount.
        for (disk, at) in [(sda(), 2), (sdd(), 3), (sda(), 0)] {
            let planned = apply(&Plan::default(), &disk, Firmware::Uefi);
            let rows = tile_rows(&Plan::default(), &planned, Stop::Part(at), &read());
            assert!(
                rows.contains(&TileRow::Mount),
                "{}",
                disk.partitions[at].path
            );
            assert_eq!(held_back(&planned, at), None);
        }
    }

    #[test]
    fn a_logical_partition_goes_only_after_the_ones_behind_it() {
        let old = sdc();
        let measures = read();
        let delete = |plan: &Plan, path: &str| {
            let planned = apply(plan, &old, Firmware::Bios);
            let at = planned.index_of(path).unwrap();
            let rows = tile_rows(plan, &planned, Stop::Part(at), &measures);
            (rows.contains(&TileRow::Delete), held_back(&planned, at))
        };
        let mut plan = Plan::default();
        let numbered_after = "Delete the logical partitions numbered after it first".to_string();
        assert_eq!(
            delete(&plan, "/dev/sdc5"),
            (false, Some(numbered_after.clone()))
        );
        // On a disk numbered out of its order the ones numbered after it
        // may stand before it, and the details say "numbered", not "after
        // it" on the map: sdc5 stands last there, behind sdc6 and sdc7.
        let jumbled = jumbled();
        let planned = apply(&plan, &jumbled, Firmware::Bios);
        let at = planned.index_of("/dev/sdc5").unwrap();
        assert!(planned
            .disk
            .partitions
            .iter()
            .filter(|part| part.number > 5)
            .all(|part| part.start < planned.disk.partitions[at].start));
        assert!(!tile_rows(&plan, &planned, Stop::Part(at), &measures).contains(&TileRow::Delete));
        assert_eq!(held_back(&planned, at), Some(numbered_after));
        assert_eq!(delete(&plan, "/dev/sdc7"), (true, None));
        plan.delete("/dev/sdc", "/dev/sdc7");
        // The swap partition may go now; kept, it is no mount (it is made
        // swap afresh to be used), which its details say.
        let swap = "linux-swap can't be mounted: format it to use it".to_string();
        assert_eq!(delete(&plan, "/dev/sdc6"), (true, Some(swap)));
        plan.delete("/dev/sdc", "/dev/sdc6");
        assert_eq!(delete(&plan, "/dev/sdc5"), (true, None));
        // A primary one goes whenever.
        assert_eq!(delete(&Plan::default(), "/dev/sdc1"), (true, None));
    }

    #[test]
    fn the_mount_point_and_format_menus_offer_what_the_partition_can_be() {
        let disk = sda();
        let planned = apply(&Plan::default(), &disk, Firmware::Uefi);
        // The EFI partition kept: /efi and /boot, no path of one's own.
        assert_eq!(mount_offers(&planned.parts[0]), [Mount::Efi, Mount::Boot]);
        assert!(!offers_other(&planned.parts[0]));
        // Windows kept: everything — / too, which a format can still make.
        assert_eq!(mount_offers(&planned.parts[2]), Mount::ALL);
        assert!(offers_other(&planned.parts[2]));
        // Formatted xfs, / is no longer offered; formatted btrfs it is.
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda3", Some(Fs::Xfs));
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Btrfs));
        plan.format("/dev/sda", "/dev/sda1", Some(Fs::Fat32));
        let planned = apply(&plan, &disk, Firmware::Uefi);
        assert!(!mount_offers(&planned.parts[2]).contains(&Mount::Root));
        assert!(mount_offers(&planned.parts[4]).contains(&Mount::Root));
        // The EFI partition formatted is a partition like any other.
        assert_eq!(mount_offers(&planned.parts[0]), Mount::ALL[1..]);
        // Format as at / offers ext4 and btrfs alone.
        assert_eq!(format_offers(Some(&Mount::Root)), [Fs::Ext4, Fs::Btrfs]);
        assert_eq!(format_offers(Some(&Mount::Home)), Fs::ALL);
        assert_eq!(format_offers(None), Fs::ALL);
    }

    #[test]
    fn a_label_is_set_only_on_what_the_plan_formats_or_makes() {
        let mut plan = Plan::default();
        assert!(!plan.label("/dev/sda", "/dev/sda5", "Data", "Photos"));
        assert!(plan.is_empty());
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        assert!(plan.label("/dev/sda", "/dev/sda5", "", "photos"));
        assert_eq!(plan.label_of("/dev/sda5"), Some("photos"));
        // Format › keep takes the label away with the format, and the size
        // set for it: a kept partition is never grown, nor shrunk unread.
        plan.resize("/dev/sda", "/dev/sda5", sda().partitions[4].size, 100 * GIB);
        assert!(plan.format("/dev/sda", "/dev/sda5", None));
        assert!(plan.is_empty());
        // A kept partition's own shrink stays where nothing was formatted.
        plan.resize("/dev/sda", "/dev/sda3", sda().partitions[2].size, 250 * GIB);
        assert!(!plan.format("/dev/sda", "/dev/sda3", None));
        assert_eq!(plan.size_of("/dev/sda3"), Some(250 * GIB));
    }

    #[test]
    fn msdos_is_offered_only_where_a_bios_starts_the_machine() {
        assert_eq!(tables(Firmware::Uefi), [Table::Gpt]);
        assert_eq!(tables(Firmware::Bios), [Table::Gpt, Table::Msdos]);
    }

    #[test]
    fn the_pending_operations_are_said_in_gparteds_words() {
        let disks = all();
        let said = |op: Operation| sentence(&op, &disks);
        assert_eq!(
            said(Operation::Delete {
                disk: "/dev/sda".into(),
                part: "/dev/sda4".into()
            }),
            "Delete /dev/sda4 (ntfs, 750\u{a0}MiB)"
        );
        assert_eq!(
            said(Operation::Format {
                disk: "/dev/sda".into(),
                part: "/dev/sda2".into(),
                fs: Fs::Ext4
            }),
            "Format /dev/sda2 as ext4"
        );
        assert_eq!(
            said(Operation::Mount {
                disk: "/dev/sda".into(),
                part: "/dev/sda5".into(),
                at: Mount::Root
            }),
            "Set mount point of /dev/sda5 to /"
        );
        assert_eq!(
            said(Operation::Table {
                disk: "/dev/sdb".into(),
                table: Table::Gpt
            }),
            "Create gpt partition table on /dev/sdb"
        );
    }

    /// The plan judged on `disks`, started by `firmware`, with `measures`
    /// and the made-up machine's image.
    fn judged(
        plan: &Plan,
        disks: &[Disk],
        firmware: Firmware,
        measures: &Measures,
    ) -> Option<Refusal> {
        let software = Software::default();
        let images = demo::image_sizes();
        let judge = Judge {
            firmware,
            bootloader: Bootloader::Automatic,
            software: &software,
            measures,
            images: &images,
        };
        missing(plan, disks, &judge)
    }

    /// The plan judged on the made-up machine, with what the demo reads.
    fn missing_on(plan: &Plan, firmware: Firmware) -> Option<Refusal> {
        let disks = all();
        judged(plan, &disks, firmware, &demo::measures(&disks))
    }

    fn part(path: &str) -> Which {
        Which::Part(path.into())
    }

    #[test]
    fn continue_needs_a_root_big_enough_and_formatted() {
        let mut plan = Plan::default();
        assert_eq!(missing_on(&plan, Firmware::Uefi), Some(Refusal::Root));
        // The 16 MiB of Microsoft's: too small for /.
        plan.mount("/dev/sda", "/dev/sda2", Some(Mount::Root));
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::TooSmall {
                at: Mount::Root,
                least: MIN_INSTALL
            })
        );
        // Data, kept: it must be formatted.
        plan.mount("/dev/sda", "/dev/sda2", None);
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::MustFormat {
                part: part("/dev/sda5"),
                at: Mount::Root
            })
        );
        // As FAT32: not a filesystem / can be; nor xfs (Unit Q).
        for fs in [Fs::Fat32, Fs::Xfs] {
            plan.format("/dev/sda", "/dev/sda5", Some(fs));
            assert_eq!(
                missing_on(&plan, Firmware::Uefi),
                Some(Refusal::Needs { at: Mount::Root }),
                "{fs:?}"
            );
        }
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        assert_eq!(missing_on(&plan, Firmware::Uefi), Some(Refusal::Efi));
    }

    #[test]
    fn a_mount_point_is_used_once_but_swap_may_be_used_twice() {
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        plan.mount("/dev/sdd", "/dev/sdd2", Some(Mount::Root));
        assert_eq!(
            missing_on(&plan, Firmware::Bios),
            Some(Refusal::Twice { at: Mount::Root })
        );
        plan.mount("/dev/sdd", "/dev/sdd2", None);
        plan.format("/dev/sdd", "/dev/sdd3", Some(Fs::LinuxSwap));
        plan.mount("/dev/sdd", "/dev/sdd3", Some(Mount::Swap));
        plan.format("/dev/sdc", "/dev/sdc6", Some(Fs::LinuxSwap));
        plan.mount("/dev/sdc", "/dev/sdc6", Some(Mount::Swap));
        assert_ne!(
            missing_on(&plan, Firmware::Bios),
            Some(Refusal::Twice { at: Mount::Swap })
        );
    }

    #[test]
    fn only_home_and_the_disks_own_esp_keep_what_is_on_them() {
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        // The Linux disk's /home, kept.
        plan.mount("/dev/sdd", "/dev/sdd4", Some(Mount::Home));
        // Its swap, kept: a fresh one is needed.
        plan.mount("/dev/sdd", "/dev/sdd3", Some(Mount::Swap));
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::MustFormat {
                part: part("/dev/sdd3"),
                at: Mount::Swap
            })
        );
        plan.format("/dev/sdd", "/dev/sdd3", Some(Fs::LinuxSwap));
        // The old disk's shared FAT32, kept at /efi: not an EFI partition.
        plan.mount("/dev/sdc", "/dev/sdc7", Some(Mount::Efi));
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::MustFormat {
                part: part("/dev/sdc7"),
                at: Mount::Efi
            })
        );
        // Windows at /home, kept: ntfs is no home.
        plan.mount("/dev/sdc", "/dev/sdc7", None);
        plan.mount("/dev/sdd", "/dev/sdd4", None);
        plan.mount("/dev/sda", "/dev/sda3", Some(Mount::Home));
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::Needs { at: Mount::Home })
        );
        // ext4 at /efi: it needs FAT32.
        plan.mount("/dev/sda", "/dev/sda3", None);
        plan.format("/dev/sdd", "/dev/sdd2", Some(Fs::Ext4));
        plan.mount("/dev/sdd", "/dev/sdd2", Some(Mount::Efi));
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::Needs { at: Mount::Efi })
        );
    }

    #[test]
    fn a_kept_esp_at_efi_needs_a_boot_beside_it_and_alone_at_boot_its_room_read() {
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        // Windows' 100 MiB EFI partition at /efi: a /boot beside it for the
        // kernels, always.
        plan.mount("/dev/sda", "/dev/sda1", Some(Mount::Efi));
        assert_eq!(missing_on(&plan, Firmware::Uefi), Some(Refusal::Boot));
        // The 750 MiB recovery partition as /boot, formatted FAT32: enough.
        plan.format("/dev/sda", "/dev/sda4", Some(Fs::Fat32));
        plan.mount("/dev/sda", "/dev/sda4", Some(Mount::Boot));
        assert_eq!(missing_on(&plan, Firmware::Uefi), None);
        // Kept, it would not do.
        plan.format("/dev/sda", "/dev/sda4", None);
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::MustFormat {
                part: part("/dev/sda4"),
                at: Mount::Boot
            })
        );
        // Microsoft's 16 MiB as /boot is too small.
        plan.mount("/dev/sda", "/dev/sda4", None);
        plan.format("/dev/sda", "/dev/sda2", Some(Fs::Fat32));
        plan.mount("/dev/sda", "/dev/sda2", Some(Mount::Boot));
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::TooSmall {
                at: Mount::Boot,
                least: ESP_BOOT_FREE
            })
        );
        // A roomy EFI partition at /efi, however much is free on it, still
        // wants a /boot beside it (a roomy one is never taken for the
        // kernels at /efi, Unit Q); alone at /boot it holds them, once what
        // is free on it is read — never guessed.
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        // The EFI partition made roomy: 4 GiB, Microsoft's and Windows after
        // it (Windows 4 GiB the smaller).
        let mut big = all();
        let roomy = &mut big[1].partitions;
        roomy[0].size = 4 * GIB;
        roomy[1].start += 4 * GIB;
        roomy[2].start += 4 * GIB;
        roomy[2].size -= 4 * GIB;
        let read = |used: u64| Measures {
            parts: vec![rules::Measured {
                path: "/dev/sda1".into(),
                fs: "vfat".into(),
                uuid: big[1].partitions[0].uuid.clone(),
                size: 4 * GIB,
                measure: Measure::Read {
                    used,
                    free: 4 * GIB - used,
                    least: None,
                },
            }],
        };
        plan.mount("/dev/sda", "/dev/sda1", Some(Mount::Efi));
        assert_eq!(
            judged(&plan, &big, Firmware::Uefi, &read(31 * MIB)),
            Some(Refusal::Boot)
        );
        plan.mount("/dev/sda", "/dev/sda1", Some(Mount::Boot));
        assert_eq!(
            judged(&plan, &big, Firmware::Uefi, &Measures::default()),
            Some(Refusal::NotRead {
                part: part("/dev/sda1")
            })
        );
        assert_eq!(judged(&plan, &big, Firmware::Uefi, &read(31 * MIB)), None);
        assert_eq!(
            judged(&plan, &big, Firmware::Uefi, &read(4 * GIB - 100 * MIB)),
            Some(Refusal::EspFull {
                part: part("/dev/sda1"),
                least: ESP_BOOT_FREE
            })
        );
        // Formatted, its size is its room.
        plan.format("/dev/sda", "/dev/sda1", Some(Fs::Fat32));
        assert_eq!(
            judged(&plan, &big, Firmware::Uefi, &Measures::default()),
            None
        );
    }

    #[test]
    fn on_bios_a_gpt_disk_needs_its_bios_grub() {
        // The Linux disk, its root formatted: its bios_grub is there.
        let mut plan = Plan::default();
        plan.format("/dev/sdd", "/dev/sdd2", Some(Fs::Ext4));
        plan.mount("/dev/sdd", "/dev/sdd2", Some(Mount::Root));
        assert_eq!(missing_on(&plan, Firmware::Bios), None);
        plan.delete("/dev/sdd", "/dev/sdd1");
        assert_eq!(
            missing_on(&plan, Firmware::Bios),
            Some(Refusal::BiosGrubMissing {
                disk: "/dev/sdd".into()
            })
        );
        // On msdos nothing more is needed.
        let mut plan = Plan::default();
        plan.format("/dev/sdc", "/dev/sdc5", Some(Fs::Ext4));
        plan.mount("/dev/sdc", "/dev/sdc5", Some(Mount::Root));
        assert_eq!(missing_on(&plan, Firmware::Bios), None);
    }

    #[test]
    fn nothing_may_be_planned_on_a_disk_that_cannot_be_used() {
        let mut plan = Plan::default();
        plan.table("/dev/sdb", Table::Gpt);
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::Blocked {
                disk: "/dev/sdb".into()
            })
        );
        // Nor on one the machine does not have.
        let mut plan = Plan::default();
        plan.table("/dev/sdz", Table::Gpt);
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::Blocked {
                disk: "/dev/sdz".into()
            })
        );
    }

    #[test]
    fn every_refusal_is_said_with_its_values() {
        let mut plan = Plan::default();
        let gap = sda().free[0];
        plan.create("/dev/sda", new_part(1, gap.start, 128 * MIB, None));
        let made = Which::Made {
            disk: "/dev/sda".into(),
            start: gap.start,
        };
        for (refusal, words) in [
            (Refusal::Root, "Missing: a partition mounted at /"),
            (
                Refusal::MustFormat {
                    part: part("/dev/sda5"),
                    at: Mount::Root,
                },
                "Format /dev/sda5 to use it at /",
            ),
            (Refusal::Needs { at: Mount::Efi }, "/efi needs fat32"),
            (Refusal::Needs { at: Mount::Root }, "/ needs ext4 or btrfs"),
            (
                Refusal::TooSmall {
                    at: Mount::Root,
                    least: MIN_INSTALL,
                },
                "/ needs at least 23.3\u{a0}GiB",
            ),
            (
                Refusal::FsTooSmall {
                    part: made.clone(),
                    fs: Fs::Btrfs,
                    least: 256 * MIB,
                },
                "New Partition #1 is too small for btrfs",
            ),
            (
                Refusal::Unshrinkable {
                    part: part("/dev/sda3"),
                    why: Unshrinkable::Itself(Measure::Hibernated),
                },
                "/dev/sda3 can't be shrunk: Windows is hibernated",
            ),
            (
                Refusal::Unshrinkable {
                    part: part("/dev/sda3"),
                    why: Unshrinkable::Hibernated {
                        other: "/dev/sdc1".into(),
                    },
                },
                "/dev/sda3 can't be shrunk: Windows is hibernated",
            ),
            (
                Refusal::Unshrinkable {
                    part: part("/dev/sda5"),
                    why: Unshrinkable::NoHiberfile,
                },
                "/dev/sda5 can't be shrunk here: shrink it in Windows",
            ),
            (
                Refusal::Unshrinkable {
                    part: part("/dev/sda3"),
                    why: Unshrinkable::Unread {
                        other: "/dev/sdc1".into(),
                    },
                },
                "Can't shrink: Windows on /dev/sdc1 couldn't be read",
            ),
            (
                Refusal::Unshrinkable {
                    part: part("/dev/sda3"),
                    why: Unshrinkable::BitLocker {
                        other: "/dev/sdb2".into(),
                    },
                },
                "Can't shrink: /dev/sdb2 is locked by BitLocker",
            ),
            (
                Refusal::Unmountable {
                    part: part("/dev/sdd5"),
                    fs: "crypto_LUKS".into(),
                },
                "crypto_LUKS on /dev/sdd5 can't be mounted",
            ),
            (
                Refusal::Path {
                    why: rules::PathRefused::Under("/home".into()),
                },
                "Nothing can be mounted inside /home",
            ),
            (
                Refusal::Boot,
                "Add a /boot, or mount the EFI partition at /boot",
            ),
            (
                Refusal::InsideKept {
                    part: part("/dev/sda3"),
                    at: Mount::Other("/data".into()),
                },
                "/data is kept, so nothing can be mounted inside it",
            ),
        ] {
            assert_eq!(refusal_words(&refusal, &plan), words);
        }
        // A partition made somewhere the plan does not know is its disk.
        let elsewhere = Which::Made {
            disk: "/dev/sdc".into(),
            start: MIB,
        };
        assert_eq!(which_name(&elsewhere, &plan), "/dev/sdc");
        assert_eq!(which_name(&made, &plan), "New Partition #1");
        for capability in [Capability::ShrinkExt4, Capability::ShrinkNtfs] {
            assert_eq!(
                capability_words(capability),
                "Shrinking a partition can't be applied in this version yet."
            );
            assert_eq!(
                unproven_words(&Unproven::Never(capability)),
                capability_words(capability)
            );
        }
        // Partitions deleted, primary or logical, as partitions made:
        // "Making or deleting".
        for deleted in [Capability::DeleteEntries, Capability::DeleteLogical] {
            assert_eq!(
                capability_words(deleted),
                capability_words(Capability::KeepTable)
            );
            assert_eq!(
                capability_words(deleted),
                "Making or deleting partitions can't be applied in this version yet."
            );
        }
        // An entry resized, primary or logical, a machine UEFI starts, the
        // kinds of table, a logical partition, the frame left empty, the
        // extended frame, `/` or `/boot` on a logical partition, the boot
        // flag set, a `/boot` under BIOS and a disk whose sectors are not
        // 512 bytes, and a mix proven only apart: Partitions' own sentence.
        let not_yet = "Partitions can't be applied in this version yet.";
        for capability in [
            Capability::SmallerEntry,
            Capability::LargerEntry,
            Capability::LargerLogical,
            Capability::Uefi,
            Capability::Gpt,
            Capability::Msdos,
            Capability::Logical,
            Capability::EmptyFrame,
            Capability::Extended,
            Capability::RootOnLogical,
            Capability::Activate,
            Capability::BootOnBios,
            Capability::LargeSectors,
        ] {
            assert_eq!(capability_words(capability), not_yet);
            assert_eq!(unproven_words(&Unproven::Never(capability)), not_yet);
        }
        let together = Unproven::Together(vec![
            Capability::KeepTable,
            Capability::Bios,
            Capability::Gpt,
        ]);
        assert_eq!(unproven_words(&together), not_yet);
    }

    /// Under the details, why a kept ext4 or NTFS is not shrunk and what to
    /// do (`unshrunk_words`): a Windows hibernated — there, or on another
    /// partition — names it, with Fast Startup; one not read, and
    /// BitLocker, name the other partition; an NTFS with no hibernation
    /// file of its own is shrunk in Windows; the rest are the foot's words.
    #[test]
    fn why_a_partition_is_not_shrunk_says_what_to_do() {
        for (why, words) in [
            (
                Unshrinkable::Itself(Measure::Hibernated),
                "Windows on /dev/sda3: shut it down with Fast Startup off",
            ),
            (
                Unshrinkable::Hibernated {
                    other: "/dev/sdc1".into(),
                },
                "Windows on /dev/sdc1: shut it down with Fast Startup off",
            ),
            (
                Unshrinkable::Unread {
                    other: "/dev/sdc1".into(),
                },
                "Can't shrink until Windows on /dev/sdc1 can be read",
            ),
            (
                Unshrinkable::BitLocker {
                    other: "/dev/sdb2".into(),
                },
                "To shrink, turn off BitLocker on /dev/sdb2",
            ),
            (
                Unshrinkable::NoHiberfile,
                "To shrink it, use Disk Management in Windows",
            ),
            (
                Unshrinkable::Itself(Measure::InUse),
                "/dev/sda3 is in use, so it can't be shrunk",
            ),
            (
                Unshrinkable::Itself(Measure::Unreadable),
                "/dev/sda3 can't be read, so it can't be shrunk",
            ),
        ] {
            assert_eq!(unshrunk_words("/dev/sda3", &why), words, "{why:?}");
        }
    }

    /// **An NTFS with no hibernation file of its own is told what can be
    /// done, in words true of either partition it can be** — a Windows'
    /// data partition, or the partition of a Windows whose hibernation is
    /// switched off, which reading cannot tell apart ("Only the partition
    /// Windows starts from can be shrunk" would be false of the second and
    /// say nothing to do). In every language the foot names
    /// the partition, as every other reason of its own does, and says it is
    /// shrunk in Windows; the details say what to use there, words of their
    /// own and never the foot's again.
    #[test]
    fn an_ntfs_with_no_hibernation_file_is_told_to_shrink_it_in_windows() {
        for (tag, _) in crate::i18n::RESOURCES {
            let _here = crate::i18n::speak_here(tag);
            let foot = unshrinkable_words("/dev/sda5", &Unshrinkable::NoHiberfile);
            let details = unshrunk_words("/dev/sda5", &Unshrinkable::NoHiberfile);
            assert!(foot.contains("/dev/sda5"), "{tag}: {foot}");
            assert!(foot.contains("Windows"), "{tag}: {foot}");
            assert!(details.contains("Windows"), "{tag}: {details}");
            assert_ne!(details, foot, "{tag}: the details said the foot's words");
        }
    }

    #[test]
    fn swap_partitions_put_the_swap_file_away() {
        let mut plan = Plan::default();
        assert!(!swap_partition(&plan));
        plan.mount("/dev/sdd", "/dev/sdd3", Some(Mount::Swap));
        assert!(swap_partition(&plan));
    }

    #[test]
    fn small_gaps_untouched_by_a_deletion_stay_unlisted() {
        // A gap too small to be listed (under MIN_GAP) stays out of the map
        // where no deletion reaches it.
        let mut disk = sda();
        disk.partitions[4].start += 100 * MIB;
        disk.partitions[4].size -= 100 * MIB;
        let mut plan = Plan::default();
        plan.delete("/dev/sda", "/dev/sda2");
        let planned = apply(&plan, &disk, Firmware::Uefi);
        assert!(planned
            .disk
            .free
            .iter()
            .all(|gap| gap.size >= MIN_GAP || gap.size == 16 * MIB));
    }

    /// New Partition #`number`, `size` long from `start`, ext4 at `at`.
    fn new_part(number: u32, start: u64, size: u64, at: Option<Mount>) -> NewPart {
        NewPart {
            number,
            start,
            size,
            fs: Fs::Ext4,
            mount: at,
            label: String::new(),
        }
    }

    #[test]
    fn a_new_partition_is_one_operation_and_what_is_asked_of_it_folds_into_it() {
        let gap = sda().free[0];
        let mut plan = Plan::default();
        assert_eq!(plan.next_number(), 1);
        plan.create(
            "/dev/sda",
            new_part(1, gap.start, 120 * GIB, Some(Mount::Root)),
        );
        plan.mount("/dev/sda", "/dev/sda1", Some(Mount::Efi));
        let part = new_path(1);
        assert_eq!(new_number(&part), Some(1));
        assert_eq!(plan.next_number(), 2);
        // Another mount point, filesystem, size and label: still one
        // operation, gone to the end of the list as the latest asked for.
        assert!(plan.mount("/dev/sda", &part, Some(Mount::Home)));
        assert!(matches!(
            plan.operations().last(),
            Some(Operation::Create { .. })
        ));
        assert!(plan.format("/dev/sda", &part, Some(Fs::Xfs)));
        // A label made for one filesystem is cut to another's.
        assert!(plan.label("/dev/sda", &part, "", "a label for btrfs"));
        assert!(plan.format("/dev/sda", &part, Some(Fs::Fat32)));
        assert_eq!(plan.label_of(&part), Some("A LABEL FOR"));
        assert!(plan.label("/dev/sda", &part, "", ""));
        assert!(plan.format("/dev/sda", &part, Some(Fs::Xfs)));
        // Keep means nothing on a partition the plan makes.
        assert!(!plan.format("/dev/sda", &part, None));
        assert!(plan.resize("/dev/sda", &part, 120 * GIB, 100 * GIB));
        assert!(plan.label("/dev/sda", &part, "", "Home"));
        assert_eq!(plan.len(), 2);
        let made = plan.created(&part).unwrap();
        assert_eq!(
            (made.fs, made.size, made.label.as_str(), made.mount.clone()),
            (Fs::Xfs, 100 * GIB, "Home", Some(Mount::Home))
        );
        assert_eq!(plan.mount_of(&part), Some(Mount::Home));
        assert_eq!(plan.format_of(&part), Some(Fs::Xfs));
        assert_eq!(plan.size_of(&part), Some(100 * GIB));
        assert_eq!(plan.label_of(&part), Some("Home"));
        // Asked again for what it already is, nothing moves.
        let before = plan.clone();
        assert!(!plan.mount("/dev/sda", &part, Some(Mount::Home)));
        assert_eq!(plan, before);
        // Deleted, it is simply not made: no Delete line for it.
        plan.delete("/dev/sda", &part);
        assert_eq!(plan.len(), 1);
        assert!(plan.created(&part).is_none());
        // Named for a person to read as GParted names it.
        assert_eq!(part_name(&part), "New Partition #1");
        assert_eq!(part_name("/dev/sda3"), "/dev/sda3");
    }

    #[test]
    fn a_size_and_a_label_set_again_fold_and_back_to_the_disks_are_nothing() {
        let disk = sda();
        let windows = disk.partitions[2].size;
        let mut plan = Plan::default();
        assert!(plan.resize("/dev/sda", "/dev/sda3", windows, 300 * GIB));
        assert!(plan.resize("/dev/sda", "/dev/sda3", windows, 250 * GIB));
        assert_eq!(plan.len(), 1);
        assert_eq!(plan.size_of("/dev/sda3"), Some(250 * GIB));
        assert!(plan.resize("/dev/sda", "/dev/sda3", windows, windows));
        assert!(plan.is_empty());
        // A label is set as a filesystem is made: on Data formatted, set
        // again it folds, and back to none it is nothing.
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        assert!(plan.label("/dev/sda", "/dev/sda5", "", "Photos"));
        assert!(plan.label("/dev/sda", "/dev/sda5", "", "home"));
        assert_eq!(plan.len(), 2);
        assert!(plan.label("/dev/sda", "/dev/sda5", "", ""));
        assert_eq!(plan.len(), 1);
        // A format again takes the label with the filesystem it was set on.
        plan.label("/dev/sda", "/dev/sda5", "", "home");
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Btrfs));
        assert_eq!(plan.label_of("/dev/sda5"), None);
        plan.label("/dev/sda", "/dev/sda5", "", "home");
        assert_eq!(plan.len(), 2);
        // A resize then a delete: the delete alone.
        plan.resize("/dev/sda", "/dev/sda3", windows, 250 * GIB);
        plan.delete("/dev/sda", "/dev/sda3");
        assert!(!plan
            .operations()
            .iter()
            .any(|op| matches!(op, Operation::Resize { .. })));
        assert!(matches!(
            plan.operations().last(),
            Some(Operation::Delete { part, .. }) if part == "/dev/sda3"
        ));
    }

    #[test]
    fn a_new_partition_stands_where_it_is_made_and_the_rest_stays_unallocated() {
        let disk = sda();
        let gap = disk.free[0];
        // At the start of the space: the rest after it.
        let mut plan = Plan::default();
        plan.create(
            "/dev/sda",
            new_part(1, gap.start, 120 * GIB, Some(Mount::Root)),
        );
        let planned = apply(&plan, &disk, Firmware::Uefi);
        let left = format!("free {}", (gap.size - 120 * GIB) / MIB);
        assert_eq!(
            names(&planned),
            [
                "sda1",
                "sda2",
                "sda3",
                "new#1",
                left.as_str(),
                "sda4",
                "sda5"
            ]
        );
        let at = planned.index_of("new#1").unwrap();
        let made = &planned.disk.partitions[at];
        assert_eq!((made.start, made.size), (gap.start, 120 * GIB));
        assert_eq!(map::filesystem(made), Some("ext4"));
        assert_eq!(planned.parts[at].new, Some(1));
        assert_eq!(planned.parts[at].mount, Some(Mount::Root));
        let free = planned.disk.free[0];
        assert_eq!(free.start, gap.start + 120 * GIB);
        assert_eq!(free.start % MIB + free.size % MIB, 0);
        // At its end: the rest before it.
        let mut plan = Plan::default();
        let end = gap.start + gap.size - 120 * GIB;
        plan.create("/dev/sda", new_part(1, end, 120 * GIB, None));
        let planned = apply(&plan, &disk, Firmware::Uefi);
        assert_eq!(names(&planned)[3], left);
        assert_eq!(names(&planned)[4], "new#1");
        // All of it: no unallocated space left there.
        let mut plan = Plan::default();
        plan.create("/dev/sda", new_part(1, gap.start, gap.size, None));
        assert!(apply(&plan, &disk, Firmware::Uefi).disk.free.is_empty());
        // Under a new table, in the one run it leaves.
        let mut plan = Plan::default();
        plan.table("/dev/sda", Table::Gpt);
        plan.create("/dev/sda", new_part(1, MIB, 2 * GIB, Some(Mount::Efi)));
        let planned = apply(&plan, &disk, Firmware::Uefi);
        assert_eq!(names(&planned).len(), 2, "{:?}", names(&planned));
        assert_eq!(planned.disk.partitions[0].part_type, part_type::ESP);
        assert_eq!(map::flags(&planned.disk.partitions[0]), ["boot", "esp"]);
    }

    #[test]
    fn a_shrink_frees_the_space_after_it_and_a_grow_takes_from_it() {
        let disk = sda();
        let gap = disk.free[0];
        let windows = disk.partitions[2].clone();
        let mut plan = Plan::default();
        plan.resize("/dev/sda", "/dev/sda3", windows.size, 250 * GIB);
        let planned = apply(&plan, &disk, Firmware::Uefi);
        assert_eq!(planned.disk.partitions[2].size, 250 * GIB);
        let free = planned.disk.free[0];
        assert_eq!(free.start, windows.start + 250 * GIB);
        assert_eq!(free.start + free.size, gap.start + gap.size);
        assert_eq!(stop_at(&planned, windows.start), Some(2));
        // Grown into the space after it, the space shrinks from its start.
        let mut plan = Plan::default();
        plan.resize(
            "/dev/sda",
            "/dev/sda3",
            windows.size,
            windows.size + 100 * GIB,
        );
        let planned = apply(&plan, &disk, Firmware::Uefi);
        let free = planned.disk.free[0];
        assert_eq!(free.start, windows.start + windows.size + 100 * GIB);
        assert_eq!(free.start + free.size, gap.start + gap.size);
        // Grown into all of it: none left.
        let most = gap.start + gap.size - windows.start;
        let mut plan = Plan::default();
        plan.resize("/dev/sda", "/dev/sda3", windows.size, most);
        assert!(apply(&plan, &disk, Firmware::Uefi).disk.free.is_empty());
        // A logical partition shrunk frees space inside its extended one.
        let old = sdc();
        let debian = old.partitions[2].clone();
        let mut plan = Plan::default();
        plan.resize("/dev/sdc", "/dev/sdc5", debian.size, debian.size - 10 * GIB);
        let planned = apply(&plan, &old, Firmware::Uefi);
        let inside = map::tiles(&planned.disk)
            .into_iter()
            .find_map(|tile| match tile {
                Tile::Extended { inside, .. } => Some(inside),
                _ => None,
            })
            .unwrap();
        assert!(matches!(inside[1], Tile::Free { size, .. } if size >= 10 * GIB - MIB));
    }

    #[test]
    fn a_size_set_for_a_format_taken_back_goes_with_it() {
        let disk = sda();
        let disks = vec![disk.clone()];
        let windows = disk.partitions[2].size;
        // Formatted, Windows' entry grew into the room after it; the format
        // asked again (another filesystem), then taken back by Undo last.
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda3", Some(Fs::Ext4));
        plan.resize("/dev/sda", "/dev/sda3", windows, windows + 100 * GIB);
        plan.format("/dev/sda", "/dev/sda3", Some(Fs::Btrfs));
        assert!(plan.undo_last());
        assert!(
            plan.prune(&disks, Firmware::Uefi),
            "a kept Windows never grows"
        );
        assert!(plan.is_empty());
        // Shrunk, kept NTFS may stay shrunk; kept FAT32 may not.
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda3", Some(Fs::Ext4));
        plan.resize("/dev/sda", "/dev/sda3", windows, 250 * GIB);
        plan.format("/dev/sda", "/dev/sda3", Some(Fs::Xfs));
        plan.undo_last();
        assert!(!plan.prune(&disks, Firmware::Uefi));
        assert_eq!(plan.size_of("/dev/sda3"), Some(250 * GIB));
        let mut fat = disks.clone();
        fat[0].partitions[2].fs_type = "vfat".into();
        assert!(plan.prune(&fat, Firmware::Uefi));
        assert!(plan.is_empty());
    }

    #[test]
    fn a_partition_made_where_a_shrink_was_taken_back_goes_with_it() {
        let disk = sda();
        let windows = disk.partitions[2].clone();
        let disks = vec![disk.clone()];
        let mut plan = Plan::default();
        plan.resize("/dev/sda", "/dev/sda3", windows.size, 250 * GIB);
        let freed = windows.start + 250 * GIB;
        plan.create("/dev/sda", new_part(1, freed, 50 * GIB, None));
        assert!(
            !plan.prune(&disks, Firmware::Uefi),
            "it fits while the shrink stands"
        );
        plan.undo_part("/dev/sda3");
        assert!(plan.prune(&disks, Firmware::Uefi));
        assert!(plan.is_empty());
        // One made in the space discovery listed stays.
        let gap = disk.free[0];
        plan.create("/dev/sda", new_part(1, gap.start, 50 * GIB, None));
        assert!(!plan.prune(&disks, Firmware::Uefi));
        assert_eq!(plan.len(), 1);
    }

    #[test]
    fn new_is_offered_where_a_partition_can_be_made_and_said_why_not_elsewhere() {
        let measures = Measures::default();
        // A disk with no table: create one first.
        let nvme = demo::disks().remove(0);
        let planned = apply(&Plan::default(), &nvme, Firmware::Uefi);
        let whole = Stop::Free {
            start: 0,
            size: nvme.size,
        };
        assert_eq!(can_create(&planned.disk, 0), Err(NoNew::Table));
        assert_eq!(
            tile_rows(&Plan::default(), &planned, whole, &measures),
            [TileRow::Table]
        );
        assert!(NoNew::Table.words().contains("partition table"));
        // With a table planned on it: New….
        let mut plan = Plan::default();
        plan.table("/dev/nvme0n1", Table::Gpt);
        let planned = apply(&plan, &nvme, Firmware::Uefi);
        let run = planned.disk.free[0];
        let stop = Stop::Free {
            start: run.start,
            size: run.size,
        };
        assert_eq!(tile_rows(&plan, &planned, stop, &measures), [TileRow::New]);
        // msdos: beside the extended partition the old disk keeps, nothing
        // (by design, as on Where to install).
        let old = sdc();
        let gap = old.free[0];
        let planned = apply(&Plan::default(), &old, Firmware::Bios);
        assert_eq!(
            can_create(&planned.disk, gap.start),
            Err(NoNew::BesideExtended)
        );
        let stop = Stop::Free {
            start: gap.start,
            size: gap.size,
        };
        assert!(tile_rows(&Plan::default(), &planned, stop, &measures).is_empty());
        assert!(NoNew::BesideExtended.words().contains("extended"));
        // With it gone and all that was in it: Windows and three more, then
        // none.
        let mut plan = Plan::default();
        for part in ["/dev/sdc7", "/dev/sdc6", "/dev/sdc5", "/dev/sdc2"] {
            plan.delete("/dev/sdc", part);
        }
        let planned = apply(&plan, &old, Firmware::Bios);
        let room = planned.disk.free[0];
        assert_eq!(can_create(&planned.disk, room.start), Ok(()));
        for number in 1..=3 {
            let start = room.start + u64::from(number - 1) * 10 * GIB;
            plan.create("/dev/sdc", new_part(number, start, 10 * GIB, None));
        }
        let planned = apply(&plan, &old, Firmware::Bios);
        assert_eq!(primaries(&planned.disk), 4);
        let rest = *planned.disk.free.last().unwrap();
        assert_eq!(can_create(&planned.disk, rest.start), Err(NoNew::Primaries));
        let stop = Stop::Free {
            start: rest.start,
            size: rest.size,
        };
        assert!(tile_rows(&plan, &planned, stop, &measures).is_empty());
        assert_eq!(missing_on(&plan, Firmware::Bios), Some(Refusal::Root));
        // A fourth made all the same (never by the page) is what Continue
        // refuses.
        plan.create("/dev/sdc", new_part(4, rest.start, 10 * GIB, None));
        assert_eq!(
            missing_on(&plan, Firmware::Bios),
            Some(Refusal::Primaries {
                disk: "/dev/sdc".into()
            })
        );
        // Inside an extended partition: no logical partitions.
        let mut plan = Plan::default();
        plan.delete("/dev/sdc", "/dev/sdc6");
        let planned = apply(&plan, &old, Firmware::Bios);
        let inner = planned
            .disk
            .free
            .iter()
            .find(|run| run.start < gap.start)
            .copied()
            .unwrap();
        assert_eq!(can_create(&planned.disk, inner.start), Err(NoNew::Logical));
    }

    #[test]
    fn nothing_the_rules_refuse_is_offered_on_a_disk_written_over_as_a_whole() {
        // The rule set writes no new table over what is written on the
        // whole of a disk, so Options offers none where such a disk has a
        // table too — and a partition deleted there leaves space that offers
        // no New…, which the rule set refuses as well; the details say why
        // under every tile of the disk (step Q1).
        let measures = Measures::default();
        let disk = written_over();
        let clean = apply(&Plan::default(), &sdd(), Firmware::Bios);
        assert!(table_offered(&clean), "the same disk, nothing over it");
        assert_eq!(whole_held_back(&clean), None);
        let planned = apply(&Plan::default(), &disk, Firmware::Bios);
        assert!(!table_offered(&planned));
        let why = whole_held_back(&planned);
        assert_eq!(why.as_deref(), Some(NoNew::Whole.words()));
        assert!(NoNew::Whole.words().contains("whole disk"));
        // A partition deleted on it leaves space no partition is made in.
        let mut plan = Plan::default();
        plan.delete("/dev/sdd", "/dev/sdd5");
        let planned = apply(&plan, &disk, Firmware::Bios);
        let run = *planned.disk.free.last().expect("the space sdd5 leaves");
        assert_eq!(can_create(&planned.disk, run.start), Err(NoNew::Whole));
        let stop = Stop::Free {
            start: run.start,
            size: run.size,
        };
        assert!(tile_rows(&plan, &planned, stop, &measures).is_empty());
        // What was offered is what the rule set refuses.
        let mut disks = all();
        disks.retain(|one| one.path != disk.path);
        disks.push(disk);
        let refused = Some(Refusal::Mismatch {
            disk: "/dev/sdd".into(),
        });
        let mut table = Plan::default();
        table.table("/dev/sdd", Table::Gpt);
        assert_eq!(judged(&table, &disks, Firmware::Bios, &measures), refused);
        plan.create("/dev/sdd", new_part(1, run.start, run.size, None));
        assert_eq!(judged(&plan, &disks, Firmware::Bios, &measures), refused);
    }

    #[test]
    fn a_kept_partition_only_shrinks_once_read_and_one_made_reaches_the_space_after_it() {
        let disk = sda();
        let gap = disk.free[0];
        let planned = apply(&Plan::default(), &disk, Firmware::Uefi);
        let windows = &disk.partitions[2];
        let read = |used: u64, least: Option<u64>| {
            Some(Shrinkable {
                used,
                least,
                filesystem: windows.size,
            })
        };
        // Kept and read: from what is used and a margin to its own size —
        // it never grows into the room after it.
        let Some(Measure::Read { used, .. }) = demo::measured(windows) else {
            panic!("the demo reads Windows");
        };
        let room = resize_room(&planned, 2, read(used, None)).unwrap();
        assert_eq!(room.least, (used + margin(used)).div_ceil(MIB) * MIB);
        assert_eq!(room.most, windows.size);
        assert_eq!(margin(10 * GIB), GIB);
        assert_eq!(margin(200 * GIB), 20 * GIB);
        // Its tool's own least is never gone below.
        let tool = 300 * GIB;
        let room = resize_room(&planned, 2, read(used, Some(tool))).unwrap();
        assert_eq!(room.least, tool);
        // A filesystem smaller than that already — its partition grown
        // without it, or a run stopped before its entry was made smaller —
        // from its own size, up to a whole MiB: an entry that holds it is
        // made smaller alone, the rule set's least (`least_entry`).
        let small = Shrinkable {
            used,
            least: Some(tool),
            filesystem: used + 100 * MIB + 5,
        };
        let room = resize_room(&planned, 2, Some(small)).unwrap();
        assert_eq!(room.least, used + 101 * MIB);
        assert_eq!(room.least, small.least_entry());
        assert!(small.entry_only(room.least) && !small.entry_only(room.least - MIB));
        // Unread, or read as what cannot shrink: no Resize at all.
        assert_eq!(resize_room(&planned, 2, None), None);
        for why in [
            Measure::Hibernated,
            Measure::NeedsCheck,
            Measure::Encrypted,
            Measure::InUse,
            Measure::Unreadable,
            Measure::NoHiberfile { used, free: 0 },
        ] {
            let measures = Measures {
                parts: vec![rules::Measured {
                    path: windows.path.clone(),
                    fs: windows.fs_type.clone(),
                    uuid: windows.uuid.clone(),
                    size: windows.size,
                    measure: why,
                }],
            };
            assert_eq!(shrinkable(&measures, windows), None, "{why:?}");
        }
        // Full to its margin, nothing to shrink.
        let full = read(windows.size - GIB, None);
        assert_eq!(resize_room(&planned, 2, full), None);
        // Microsoft's reserved space, the EFI partition and the recovery
        // tools, never, whatever is read.
        for at in [0, 1, 3] {
            assert_eq!(resize_room(&planned, at, read(MIB, None)), None, "{at}");
        }
        // xfs, btrfs and FAT32 kept are not shrunk in this version.
        for fs in ["xfs", "btrfs", "vfat"] {
            let mut other = disk.clone();
            other.partitions[2].fs_type = fs.into();
            other.partitions[2].content = Content::Linux;
            let planned = apply(&Plan::default(), &other, Firmware::Uefi);
            assert_eq!(resize_room(&planned, 2, read(used, None)), None, "{fs}");
        }
        // Formatted afresh it holds nothing: its filesystem's least alone, and
        // its entry may grow into the space after it.
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda3", Some(Fs::Ext4));
        let planned = apply(&plan, &disk, Firmware::Uefi);
        let room = resize_room(&planned, 2, None).unwrap();
        assert_eq!(room.least, Fs::Ext4.least());
        assert_eq!(room.most, gap.start + gap.size - windows.start);
        // At / it may not be made smaller than / must be.
        plan.mount("/dev/sda", "/dev/sda3", Some(Mount::Root));
        let planned = apply(&plan, &disk, Firmware::Uefi);
        assert_eq!(
            resize_room(&planned, 2, None).unwrap().least,
            Mount::Root.least()
        );
    }

    #[test]
    fn a_logical_partition_grows_back_only_to_where_it_ended_while_one_after_it_stays() {
        // Step Q1's case: sdc5 formatted at /, shrunk, then grown as far as
        // Resize offers — never up to sdc6's start, over sdc6's own table
        // (its EBR) in the MiB between them, which would lose sdc6 and sdc7
        // from the table with no word of it in the consent.
        let disk = sdc();
        let debian = disk.partitions[2].clone();
        let sdc6 = disk.partitions[3].start;
        assert_eq!(sdc6 - (debian.start + debian.size), MIB, "sdc6's table");
        let shrunk = 20_000 * MIB;
        let mut plan = Plan::default();
        plan.format("/dev/sdc", "/dev/sdc5", Some(Fs::Ext4));
        plan.mount("/dev/sdc", "/dev/sdc5", Some(Mount::Root));
        plan.resize("/dev/sdc", "/dev/sdc5", debian.size, shrunk);
        // What the shrink freed is what it gave up, and no more.
        let planned = apply(&plan, &disk, Firmware::Bios);
        let freed: Vec<Gap> = planned
            .disk
            .free
            .iter()
            .copied()
            .filter(|gap| gap.start < sdc6)
            .collect();
        assert_eq!(
            freed,
            [Gap {
                start: debian.start + shrunk,
                size: debian.size - shrunk,
            }]
        );
        let at = planned.index_of("/dev/sdc5").unwrap();
        let room = resize_room(&planned, at, None).unwrap();
        assert_eq!(room.most, debian.size, "back to where it ended");
        // Grown back that far, nothing is refused for where it stands.
        plan.resize("/dev/sdc", "/dev/sdc5", debian.size, room.most);
        assert_eq!(missing_on(&plan, Firmware::Bios), None);
        // Up to sdc6's start the rule set refuses it, were it ever asked.
        plan.resize("/dev/sdc", "/dev/sdc5", debian.size, sdc6 - debian.start);
        assert_eq!(
            missing_on(&plan, Firmware::Bios),
            Some(Refusal::Overlap {
                part: part("/dev/sdc5")
            })
        );
        // With every logical partition after it deleted, from the last, it
        // may take their space, their tables gone with them.
        plan.resize("/dev/sdc", "/dev/sdc5", debian.size, shrunk);
        plan.delete("/dev/sdc", "/dev/sdc7");
        plan.delete("/dev/sdc", "/dev/sdc6");
        let planned = apply(&plan, &disk, Firmware::Bios);
        let at = planned.index_of("/dev/sdc5").unwrap();
        let room = resize_room(&planned, at, None).unwrap();
        let sdc7 = &disk.partitions[4];
        assert_eq!(debian.start + room.most, sdc7.start + sdc7.size);
        plan.resize("/dev/sdc", "/dev/sdc5", debian.size, room.most);
        assert_eq!(missing_on(&plan, Firmware::Bios), None);
    }

    #[test]
    fn a_logical_partition_grows_over_no_table_of_one_standing_after_it_on_the_disk() {
        // On a disk whose logical partitions are numbered out of its order —
        // sdc6 first, sdc7 next, sdc5 last — sdc7, numbered last, may be
        // deleted, and the space it leaves reaches up to sdc5's start
        // (step Q1). Were sdc6 formatted at / offered to grow into all of it,
        // it would grow over sdc5's own table (its EBR), and a rule set
        // judging "after" by number would find nothing kept after it.
        let disk = jumbled();
        let debian = disk.partition("/dev/sdc6").unwrap().clone();
        let shared = disk.partition("/dev/sdc5").unwrap().clone();
        let mut disks = all();
        disks.retain(|one| one.path != disk.path);
        disks.push(disk.clone());
        let measures = demo::measures(&disks);
        let mut plan = Plan::default();
        plan.format("/dev/sdc", "/dev/sdc6", Some(Fs::Ext4));
        plan.mount("/dev/sdc", "/dev/sdc6", Some(Mount::Root));
        let planned = apply(&plan, &disk, Firmware::Bios);
        let swap = planned.index_of("/dev/sdc7").unwrap();
        let rows = tile_rows(&plan, &planned, Stop::Part(swap), &measures);
        assert!(rows.contains(&TileRow::Delete), "sdc7 is numbered last");
        plan.delete("/dev/sdc", "/dev/sdc7");
        let planned = apply(&plan, &disk, Firmware::Bios);
        // The space sdc7 leaves runs up to sdc5's start...
        let debian_end = debian.start + debian.size;
        let freed = planned
            .disk
            .free
            .iter()
            .find(|gap| gap.start >= debian_end && gap.start < shared.start);
        assert_eq!(freed.map(|gap| gap.start + gap.size), Some(shared.start));
        // ...but sdc6 grows back only to where it ended, sdc5 standing
        // after it on the disk though it is numbered before it.
        let at = planned.index_of("/dev/sdc6").unwrap();
        let room = resize_room(&planned, at, None).unwrap();
        assert_eq!(room.most, debian.size, "no further than where it ended");
        assert_eq!(judged(&plan, &disks, Firmware::Bios, &measures), None);
        // Grown up to sdc5's start, the rule set refuses it, were it asked.
        plan.resize(
            "/dev/sdc",
            "/dev/sdc6",
            debian.size,
            shared.start - debian.start,
        );
        assert_eq!(
            judged(&plan, &disks, Firmware::Bios, &measures),
            Some(Refusal::Overlap {
                part: part("/dev/sdc6")
            })
        );
    }

    #[test]
    fn continue_counts_the_partitions_the_plan_makes() {
        let gap = sda().free[0];
        let mut plan = Plan::default();
        // / on a new ext4 partition of 120 GiB: / is there.
        plan.create(
            "/dev/sda",
            new_part(1, gap.start, 120 * GIB, Some(Mount::Root)),
        );
        assert_eq!(missing_on(&plan, Firmware::Uefi), Some(Refusal::Efi));
        // Made too small for it: said so.
        plan.resize("/dev/sda", &new_path(1), 120 * GIB, 10 * GIB);
        assert_eq!(
            missing_on(&plan, Firmware::Uefi),
            Some(Refusal::TooSmall {
                at: Mount::Root,
                least: MIN_INSTALL
            })
        );
        plan.resize("/dev/sda", &new_path(1), 10 * GIB, 120 * GIB);
        // A new FAT32 EFI partition of 2 GiB alone at /boot beside it:
        // nothing more is needed. (At /efi it would want a /boot too.)
        let mut efi = new_part(2, gap.start + 120 * GIB, 2 * GIB, Some(Mount::Efi));
        efi.fs = Fs::Fat32;
        plan.create("/dev/sda", efi);
        assert_eq!(missing_on(&plan, Firmware::Uefi), Some(Refusal::Boot));
        plan.mount("/dev/sda", &new_path(2), Some(Mount::Boot));
        assert_eq!(missing_on(&plan, Firmware::Uefi), None);
        // A new swap: the swap file is not offered.
        assert!(!swap_partition(&plan));
        let mut swap = new_part(3, gap.start + 122 * GIB, 4 * GIB, Some(Mount::Swap));
        swap.fs = Fs::LinuxSwap;
        plan.create("/dev/sda", swap);
        assert!(swap_partition(&plan));
        // A path of one's own keeps what is on it, if it mounts.
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Other("/data".into())));
        assert_eq!(missing_on(&plan, Firmware::Uefi), None);
        // Nothing is mounted inside it while it is kept (the folder would be
        // made in it at the first start); formatted afresh, it may hold one.
        let games = new_part(
            4,
            gap.start + 126 * GIB,
            4 * GIB,
            Some(Mount::Other("/data/games".into())),
        );
        plan.create("/dev/sda", games);
        let inside = missing_on(&plan, Firmware::Uefi).unwrap();
        assert_eq!(
            inside,
            Refusal::InsideKept {
                part: part("/dev/sda5"),
                at: Mount::Other("/data".into())
            }
        );
        assert_eq!(
            refusal_words(&inside, &plan),
            "/data is kept, so nothing can be mounted inside it"
        );
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        assert_eq!(missing_on(&plan, Firmware::Uefi), None);
        // On BIOS with gpt, a new bios_grub is GRUB's.
        let mut plan = Plan::default();
        plan.create(
            "/dev/sda",
            new_part(1, gap.start, 120 * GIB, Some(Mount::Root)),
        );
        assert_eq!(
            missing_on(&plan, Firmware::Bios),
            Some(Refusal::BiosGrubMissing {
                disk: "/dev/sda".into()
            })
        );
        let mut grub = new_part(2, gap.start + 120 * GIB, BIOS_BOOT_SIZE, None);
        grub.fs = Fs::BiosGrub;
        plan.create("/dev/sda", grub);
        assert_eq!(missing_on(&plan, Firmware::Bios), None);
        assert_eq!(new_filesystems(Firmware::Bios, Some(Table::Gpt)).len(), 6);
        assert_eq!(new_filesystems(Firmware::Bios, Some(Table::Msdos)).len(), 5);
        assert_eq!(new_filesystems(Firmware::Uefi, Some(Table::Gpt)).len(), 5);
    }

    #[test]
    fn the_new_operations_are_said_in_gparteds_words() {
        let disks = all();
        let said = |op: Operation| sentence(&op, &disks);
        assert_eq!(
            said(Operation::Create {
                disk: "/dev/sda".into(),
                part: new_path(1),
                new: new_part(1, 0, 120 * GIB, Some(Mount::Root)),
            }),
            "Create Primary Partition #1 (ext4, 120.0\u{a0}GiB, /) on /dev/sda"
        );
        assert_eq!(
            said(Operation::Create {
                disk: "/dev/sda".into(),
                part: new_path(2),
                new: new_part(2, 0, 4 * GIB, None),
            }),
            "Create Primary Partition #2 (ext4, 4.0\u{a0}GiB) on /dev/sda"
        );
        assert_eq!(
            said(Operation::Resize {
                disk: "/dev/sda".into(),
                part: "/dev/sda3".into(),
                from: 381_440 * MIB,
                to: 250 * GIB,
            }),
            "Shrink /dev/sda3 from 372.5\u{a0}GiB to 250.0\u{a0}GiB"
        );
        assert_eq!(
            said(Operation::Resize {
                disk: "/dev/sda".into(),
                part: "/dev/sda3".into(),
                from: 250 * GIB,
                to: 300 * GIB,
            }),
            "Grow /dev/sda3 from 250.0\u{a0}GiB to 300.0\u{a0}GiB"
        );
        assert_eq!(
            said(Operation::Label {
                disk: "/dev/sda".into(),
                part: "/dev/sda5".into(),
                label: "Home".into(),
            }),
            "Set file system label “Home” on /dev/sda5"
        );
        assert_eq!(
            said(Operation::Label {
                disk: "/dev/sda".into(),
                part: "/dev/sda5".into(),
                label: String::new(),
            }),
            "Clear file system label on /dev/sda5"
        );
        // The details' Pending, in short.
        let disk = sda();
        let mut plan = Plan::default();
        plan.resize("/dev/sda", "/dev/sda3", disk.partitions[2].size, 250 * GIB);
        plan.create(
            "/dev/sda",
            new_part(1, disk.free[0].start + 200 * GIB, GIB, None),
        );
        let planned = apply(&plan, &disk, Firmware::Uefi);
        assert_eq!(
            pending_words(&planned.parts[2], &planned.disk.partitions[2]),
            ["Shrink to 250.0\u{a0}GiB"]
        );
        let at = planned.index_of("new#1").unwrap();
        assert_eq!(
            pending_words(&planned.parts[at], &planned.disk.partitions[at]),
            ["Create"]
        );
    }

    // ---------------------------------------------------------------------
    // Walks: whatever the menus offer, in whatever order, the plan they make
    // is one the rule set finds well shaped — refused, where it is, only for
    // what the foot can tell a person — and what would travel is what the
    // page draws (§7.0, Unit Q, "Oracles").

    /// A small seeded generator (xorshift64*), so a walk that fails is told
    /// by its seed and replayed exactly.
    struct Dice(u64);

    impl Dice {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        fn below(&mut self, count: usize) -> usize {
            (self.next() % count.max(1) as u64) as usize
        }

        fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
            (!items.is_empty()).then(|| &items[self.below(items.len())])
        }

        /// A size from `least` to `most`, whole MiB from `least`; now and
        /// then one of the two ends, which is where rules break.
        fn size(&mut self, least: u64, most: u64) -> u64 {
            if most <= least {
                return most;
            }
            match self.below(4) {
                0 => least,
                1 => most,
                _ => least + (self.next() % ((most - least) / MIB + 1)) * MIB,
            }
        }
    }

    /// `disk` as another disk of its own at `path`, its partitions named
    /// after it: for a walk to walk beside the disk it is made from.
    fn renamed(mut disk: Disk, path: &str, seq: u64) -> Disk {
        for part in &mut disk.partitions {
            part.path = part.path.replacen(&disk.path, path, 1);
        }
        disk.path = path.into();
        disk.diskseq = Some(seq);
        disk.serial = format!("WALK-{seq:04}");
        disk.major_minor = format!("8:{}", seq * 16);
        disk
    }

    /// Every filesystem of `disks` read, a third of it in use.
    fn a_third_used<'a>(disks: impl IntoIterator<Item = &'a Disk>) -> Vec<rules::Measured> {
        disks
            .into_iter()
            .flat_map(|disk| &disk.partitions)
            .filter(|part| !part.fs_type.is_empty())
            .map(|part| rules::Measured {
                path: part.path.clone(),
                fs: part.fs_type.clone(),
                uuid: part.uuid.clone(),
                size: part.size,
                measure: Measure::Read {
                    used: part.size / 3,
                    free: part.size - part.size / 3,
                    least: None,
                },
            })
            .collect()
    }

    /// The machines walked: the made-up one (its own disks, the two shown
    /// only on Partitions, the old one again with its logical partitions
    /// numbered out of the disk's order, [`jumbled`], the Linux SSD again
    /// with something written over the whole of it, [`written_over`], and
    /// with logical sectors of 8192 bytes, [`wide_sectors`], beside a blank
    /// disk of such sectors, [`wide_blank`]),
    /// and the worker's fixtures at 512 and 4096 bytes a sector (each given
    /// a path of its own), each started by UEFI and by BIOS, with what
    /// reading them would find.
    fn machines() -> Vec<(String, Vec<Disk>, Firmware, Measures)> {
        let mut machines = Vec::new();
        for firmware in [Firmware::Uefi, Firmware::Bios] {
            let mut demo = all();
            let extra = [
                renamed(jumbled(), "/dev/sde", 6),
                renamed(written_over(), "/dev/sdf", 7),
                renamed(wide_sectors(), "/dev/sdg", 8),
                renamed(wide_blank(), "/dev/sdh", 9),
            ];
            let mut measures = demo::measures(&demo);
            measures.parts.extend(a_third_used(&extra));
            demo.extend(extra);
            machines.push((format!("demo, {firmware:?}"), demo, firmware, measures));
            for sector in [512, 4096] {
                let mut disks = Vec::new();
                let fixtures = crate::worker::partition::fixtures::all(sector);
                for (index, (_, mut disk)) in fixtures.into_iter().enumerate() {
                    let path = format!("/dev/vd{}", char::from(b'a' + index as u8));
                    for part in &mut disk.partitions {
                        part.path = part.path.replacen("/dev/vdx", &path, 1);
                    }
                    disk.path = path;
                    disks.push(disk);
                }
                let parts = a_third_used(&disks);
                let name = format!("fixtures at {sector}, {firmware:?}");
                machines.push((name, disks, firmware, Measures { parts }));
            }
        }
        machines
    }

    /// One press of what the page offers on `disks`: a tile's menu and a
    /// choice in it (or in the sheet it opens) as the screens make it, or
    /// Options' undo, clear or new table — then the plan pruned, as every
    /// change prunes it.
    fn press(
        dice: &mut Dice,
        plan: &mut Plan,
        disks: &[Disk],
        firmware: Firmware,
        measures: &Measures,
    ) {
        let usable: Vec<&Disk> = disks
            .iter()
            .filter(|disk| disks::holds_homes(disk))
            .collect();
        let Some(disk) = dice.pick(&usable).copied() else {
            return;
        };
        let path = disk.path.as_str();
        let planned = apply(plan, disk, firmware);
        match dice.below(16) {
            0 => {
                plan.undo_last();
            }
            1 if dice.below(3) == 0 => plan.clear(),
            // Options' new table, where Options offers it.
            2 if table_offered(&planned) => {
                let table = *dice.pick(&tables(firmware)).unwrap();
                plan.table(path, table);
            }
            _ => {
                let stops = map::stops(&map::tiles(&planned.disk));
                let Some(stop) = dice.pick(&stops).copied() else {
                    return;
                };
                let rows = tile_rows(plan, &planned, stop, measures);
                if let Some(row) = dice.pick(&rows).copied() {
                    choose(dice, plan, &planned, stop, row, firmware, measures);
                }
            }
        }
        plan.prune(disks, firmware);
    }

    /// `row` of the menu of the tile at `stop`, chosen with what the menu or
    /// sheet it opens offers, as the screens choose it.
    fn choose(
        dice: &mut Dice,
        plan: &mut Plan,
        planned: &Planned,
        stop: Stop,
        row: TileRow,
        firmware: Firmware,
        measures: &Measures,
    ) {
        let path = planned.disk.path.as_str();
        let labels = ["home", "Linexin root 1234567", "-x", "Zażółć", "boot", ""];
        let paths = ["/data", "/data/games", "/srv/media", "/var", "/opt/games"];
        match (row, stop) {
            (TileRow::Table, _) => {
                let table = *dice.pick(&tables(firmware)).unwrap();
                plan.table(path, table);
            }
            (TileRow::New, Stop::Free { start, size }) => {
                // New partition: its filesystem, its mount point (the File
                // system row letting go of one it does not suit, a mount
                // point making it suited), its size and place, its label.
                let fs = *dice
                    .pick(&new_filesystems(firmware, planned.disk.table))
                    .unwrap();
                let mut mounts: Vec<Option<Mount>> = Mount::ALL.iter().cloned().map(Some).collect();
                mounts.extend(
                    paths
                        .iter()
                        .map(|path| Some(Mount::Other(path.to_string()))),
                );
                mounts.push(None);
                let mount = dice
                    .pick(&mounts)
                    .unwrap()
                    .clone()
                    .filter(|_| fs != Fs::BiosGrub);
                let fs = match &mount {
                    Some(at) => at.suited(fs),
                    None => fs,
                };
                let most = size / MIB * MIB;
                let least = match fs {
                    Fs::BiosGrub => BIOS_BOOT_SIZE,
                    fs => fs.least().max(mount.as_ref().map_or(0, Mount::least)),
                };
                if least > most {
                    return;
                }
                let new_size = if fs == Fs::BiosGrub {
                    least
                } else {
                    dice.size(least, most)
                };
                let at = if dice.below(2) == 0 {
                    start
                } else {
                    (start + size - new_size) / MIB * MIB
                };
                let typed = dice.pick(&labels).unwrap();
                let label = crate::ui::sheet::label_with("", typed, Some(fs.name())).0;
                let label = if fs == Fs::BiosGrub {
                    String::new()
                } else {
                    label
                };
                let number = plan.next_number();
                plan.create(
                    path,
                    NewPart {
                        number,
                        start: at,
                        size: new_size,
                        fs,
                        mount,
                        label,
                    },
                );
            }
            (TileRow::Mount, Stop::Part(at)) => {
                let marks = &planned.parts[at];
                let mut mounts: Vec<Option<Mount>> =
                    mount_offers(marks).into_iter().map(Some).collect();
                if offers_other(marks) {
                    mounts.extend(
                        paths
                            .iter()
                            .map(|path| Some(Mount::Other(path.to_string()))),
                    );
                }
                mounts.push(None);
                let mount = dice.pick(&mounts).unwrap().clone();
                plan.mount(path, &planned.disk.partitions[at].path, mount);
            }
            (TileRow::Format, Stop::Part(at)) => {
                let part = planned.disk.partitions[at].path.clone();
                let mut formats: Vec<Option<Fs>> = format_offers(plan.mount_of(&part).as_ref())
                    .into_iter()
                    .map(Some)
                    .collect();
                if plan.created(&part).is_none() {
                    formats.push(None);
                }
                let fs = *dice.pick(&formats).unwrap();
                plan.format(path, &part, fs);
            }
            (TileRow::Resize, Stop::Part(at)) => {
                let marks = &planned.parts[at];
                let part = &planned.disk.partitions[at];
                let room = resize_room(planned, at, shrinkable(measures, &marks.was)).unwrap();
                // The sheet's: the end lands on a MiB, the start never moves.
                let size = dice.size(room.least, room.most);
                let to = ((part.start + size) / MIB * MIB)
                    .saturating_sub(part.start)
                    .clamp(room.least, room.most);
                plan.resize(path, &part.path, marks.was.size, to);
            }
            (TileRow::Label, Stop::Part(at)) => {
                let part = &planned.disk.partitions[at];
                let typed = dice.pick(&labels).unwrap();
                let label = crate::ui::sheet::label_with("", typed, map::filesystem(part)).0;
                plan.label(path, &part.path, "", &label);
            }
            (TileRow::Delete, Stop::Part(at) | Stop::Extended(at)) => {
                plan.delete(path, &planned.disk.partitions[at].path);
            }
            (TileRow::Undo, Stop::Part(at) | Stop::Extended(at)) => {
                plan.undo_part(&planned.disk.partitions[at].path);
            }
            _ => {}
        }
    }

    /// What every plan the menus can make holds to; `told` names the walk.
    fn holds(
        plan: &Plan,
        disks: &[Disk],
        firmware: Firmware,
        measures: &Measures,
        told: &str,
    ) -> bool {
        let software = Software::default();
        let images = demo::image_sizes();
        let judge = Judge {
            firmware,
            bootloader: Bootloader::Automatic,
            software: &software,
            measures,
            images: &images,
        };
        let refused = missing(plan, disks, &judge);
        let laid = layout(plan, disks).unwrap_or_else(|why| panic!("{told}: {why:?}\n{plan:#?}"));
        // Never mis-shaped: what the page makes stands where its disk has
        // room, apart, on whole MiB, done only to what may be done to it.
        if let Some(refusal) = &refused {
            let misshaped = matches!(
                refusal,
                Refusal::Blocked { .. }
                    | Refusal::Mismatch { .. }
                    | Refusal::Unaligned { .. }
                    | Refusal::Outside { .. }
                    | Refusal::Overlap { .. }
                    | Refusal::Entries { .. }
                    | Refusal::CantShrink { .. }
                    | Refusal::Grows { .. }
                    | Refusal::BiosGrub { .. }
                    | Refusal::Label { .. }
            );
            assert!(!misshaped, "{told}: {refusal:?}\n{plan:#?}");
            let words = refusal_words(refusal, plan);
            assert!(!words.is_empty() && !words.contains('{'), "{told}: {words}");
        } else {
            // Nothing refused: both halves of the rules pass on what would
            // travel, and it asks the worker for something.
            let software = Software {
                snapshots: software.snapshots && !rules::separate_var(&laid),
                ..software.clone()
            };
            assert_eq!(
                rules::shape(&laid, firmware, Bootloader::Automatic, &software),
                Ok(()),
                "{told}"
            );
            assert_eq!(rules::measured(&laid, measures, &images), Ok(()), "{told}");
            assert!(!rules::needs(&laid, firmware).is_empty(), "{told}");
        }
        // What travels is what the page draws: every partition kept, shrunk
        // or formatted at its size, in its filesystem, at its mount point;
        // every one made where it is made; nothing more.
        for one in &laid {
            let planned = apply(plan, &one.disk, firmware);
            let drawn = |path: &str| {
                planned
                    .disk
                    .partitions
                    .iter()
                    .zip(&planned.parts)
                    .find(|(part, _)| part.path == path)
            };
            let mut standing = 0;
            for (part, was) in one.parts.iter().zip(&one.disk.partitions) {
                let Some((there, marks)) = drawn(&part.path) else {
                    assert_eq!(part.fate, Fate::Delete, "{told}: {} not drawn", part.path);
                    continue;
                };
                standing += 1;
                assert_eq!(
                    marks.mount.as_ref(),
                    part.fate.mount(),
                    "{told}: {}",
                    part.path
                );
                let (size, fs_type) = match &part.fate {
                    Fate::Keep { .. } => (part.size, was.fs_type.as_str()),
                    Fate::Shrink { to, .. } => (*to, was.fs_type.as_str()),
                    Fate::Format { fs, size, .. } => (*size, fs.lsblk()),
                    Fate::Delete => panic!("{told}: {} deleted but drawn", part.path),
                };
                if !was.is_extended() {
                    assert_eq!(there.size, size, "{told}: {}", part.path);
                }
                assert_eq!(there.fs_type, fs_type, "{told}: {}", part.path);
            }
            for made in &one.create {
                let (there, marks) = planned
                    .disk
                    .partitions
                    .iter()
                    .zip(&planned.parts)
                    .find(|(part, marks)| marks.new.is_some() && part.start == made.start)
                    .unwrap_or_else(|| {
                        panic!("{told}: a partition made at {} not drawn", made.start)
                    });
                standing += 1;
                assert_eq!(
                    (
                        there.size,
                        there.fs_type.as_str(),
                        marks.mount.as_ref(),
                        there.label.as_str()
                    ),
                    (
                        made.size,
                        made.fs.lsblk(),
                        made.mount.as_ref(),
                        made.label.as_str()
                    ),
                    "{told}"
                );
            }
            assert_eq!(
                planned.disk.partitions.len(),
                standing,
                "{told}: drawn more"
            );
        }
        // It travels whole.
        let text = serde_json::to_string(&laid).unwrap();
        let back: Vec<DiskLayout> = serde_json::from_str(&text).unwrap();
        assert_eq!(back, laid, "{told}");
        // The consent names each table written and each partition deleted,
        // shrunk or formatted once, disk by disk.
        let destroyed = rules::destroys(&laid);
        let expected: usize = laid
            .iter()
            .map(|one| match one.table {
                Some(_) => 1,
                None => one
                    .parts
                    .iter()
                    .filter(|part| !matches!(part.fate, Fate::Keep { .. }))
                    .count(),
            })
            .sum();
        assert_eq!(destroyed.len(), expected, "{told}");
        let lines: Vec<String> = destroyed.iter().map(rules::Destroy::line).collect();
        let mut sorted = lines.clone();
        sorted.sort_by_key(|line| line.split(' ').nth(1).map(str::to_string));
        let disk_of = |line: &String| {
            let device = line.split(' ').nth(1).unwrap_or_default().to_string();
            laid.iter()
                .find(|one| device == one.disk.path || one.disk.partition(&device).is_some())
                .map(|one| one.disk.path.clone())
        };
        let order: Vec<Option<String>> = lines.iter().map(disk_of).collect();
        let mut by_disk = order.clone();
        by_disk.sort();
        assert_eq!(order, by_disk, "{told}: {lines:?}");
        refused.is_none()
    }

    #[test]
    fn whatever_the_menus_are_asked_the_plan_is_well_shaped_and_travels_as_drawn() {
        for (name, disks, firmware, measures) in machines() {
            let mut accepted = 0;
            for seed in 1..=150u64 {
                let mut dice = Dice(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
                let mut plan = Plan::default();
                for step in 0..40 {
                    press(&mut dice, &mut plan, &disks, firmware, &measures);
                    let told = format!("{name}, seed {seed}, step {step}");
                    if holds(&plan, &disks, firmware, &measures, &told) {
                        accepted += 1;
                    }
                }
            }
            // The walks reach plans the rules accept, not only refusals.
            assert!(accepted > 0, "{name}: no walk made a plan the rules accept");
        }
    }

    /// The walks again on the disk written over as a whole alone
    /// ([`written_over`]), every press on it: what discovery lists there as
    /// no free space — the gaps after its swap and its LUKS, and its end —
    /// no partition is ever grown into, however its partitions are deleted,
    /// formatted and resized.
    #[test]
    fn whatever_the_menus_are_asked_on_a_disk_written_over_as_a_whole_nothing_grows_into_it() {
        let disks = vec![renamed(written_over(), "/dev/sdf", 7)];
        let measures = Measures {
            parts: a_third_used(&disks),
        };
        for firmware in [Firmware::Uefi, Firmware::Bios] {
            for seed in 1..=150u64 {
                let mut dice = Dice(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
                let mut plan = Plan::default();
                for step in 0..40 {
                    press(&mut dice, &mut plan, &disks, firmware, &measures);
                    let told = format!("written over, {firmware:?}, seed {seed}, step {step}");
                    holds(&plan, &disks, firmware, &measures, &told);
                }
            }
        }
    }
}
