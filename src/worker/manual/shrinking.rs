//! **A kept filesystem made smaller, carried out** (§7.0, Unit Q, "The
//! worker" 3 and 4.1; step Q9): what phase A reads again just before
//! anything is written, its dry runs, and phase B.1. The commands and how
//! each one's end is read are [`resize`]'s; this is where they meet the
//! machine.
//!
//! **Phase A** writes nothing:
//!
//! 1. each partition whose filesystem is shrunk is given the fence's leave
//!    at the size it was reviewed at, claimed exclusively (and let go of)
//!    and found in use nowhere — mounted in the live session
//!    (`/proc/1/mountinfo`), active swap, held by anything;
//! 2. what it holds is read again, by the measure entry's own commands and
//!    rules (`measure::read_ext4`, `measure::read_ntfs`): an ext4 by
//!    `dumpe2fs -h` and `resize2fs -P`; an NTFS by `ntfsresize --info` and
//!    the one read-only sign of hibernation allowed, `ntfscat
//!    <partition> hiberfil.sys`, its head alone. And where an NTFS is
//!    shrunk, **every NTFS and BitLocker volume of every disk the worker
//!    finds now** is read the same way — the layout's disks and every other
//!    one, each partition asked first whether it is in use; a disk nothing
//!    may be made on (the installation medium, a read-only one) answered
//!    unread, nothing read — as the measure entry answers them, so that the
//!    rule across every disk of the measurement (`Measures::shrink_of`: no
//!    NTFS is shrunk while any NTFS is signed hibernated, any could not be
//!    read whole, or any volume is BitLocker) is applied again on the
//!    worker's own numbers ([`read_again`]), by `layout::measured`, which
//!    the executor asks next;
//! 3. each shrink is built from what was read: the filesystem's own size
//!    handed to the builder, which refuses a size its tool is given at or
//!    above it, and to the fence — or, where the new entry already holds the
//!    filesystem as it is (`Shrinkable::entry_only`, the rule set's and the
//!    interface's rule: a partition grown without its filesystem, or a run
//!    that stopped between a filesystem's shrink and its entry), no shrink
//!    at all, only its entry made smaller, the fence handed what was read of
//!    it — an ext4's blocks, an NTFS volume's boot sector, which must say
//!    the volume and its backup boot sector lie inside the new entry
//!    ([`ready`]);
//! 4. each dry run, which writes nothing (`e2fsck -f -n`, `ntfsresize
//!    --no-action`), its end read by its tool's rule ([`dry_runs`]).
//!
//! Every command phase A runs only reads, and is held to **this module's
//! own allowlist** ([`allowed`]; the measure entry's is not the worker's) by
//! the executor before it asks, and by the machine itself again as it
//! starts one (`machine::Real`).
//!
//! **Phase B.1** ([`carry_out`]), before any entry is changed on any disk:
//! each filesystem shrunk by its own commands, each given the fence's leave
//! (which judges it by its shape, its place and its size), its partition
//! claimed and found in use nowhere just before it runs, its end read by
//! its tool's rule; then what it left read back — an ext4 by `dumpe2fs -h`,
//! an NTFS by its boot sector — and held inside its new entry
//! (`Shrink::fits`), then handed to the fence, which makes no entry smaller
//! before its filesystem is read back inside it
//! (`Fence::filesystem_read_back`). An NTFS shrunk is left marked for
//! Windows' own check (never `ntfsfix`): Done says Windows will check its
//! disk (`Warning::WindowsWillCheck`), and Linexin, which mounts it with
//! `ntfs3` and `nofail`, leaves it unmounted until Windows has run.
//!
//! A tool's verdict becomes the failure Stopped says ([`failure_of`]): a
//! check due is `FilesystemNeedsCheck`, anything else that stops
//! `ShrinkRefused`; a hibernated Windows found by the reading again
//! `WindowsHibernated` (the executor's mapping of `layout::measured`).
use super::carry;
use super::fence::Fence;
use super::machine::Machine;
use super::plan::Planned;
use super::resize::{self, Shrink, Tool, Verdict};
use crate::model::disks::{device_path_ok, Disk, Partition};
use crate::model::layout::{DiskLayout, Fate, Measure, Measured, Measures, Part};
use crate::model::protocol::{Failure, Warning};
use crate::worker::layout::Stop;
use crate::worker::measure::{self, Ext4Header, Head, Kind, Ran, Reads};
use crate::worker::partition::script::Command;
use crate::worker::storage::progress::Steps;
use crate::worker::system::check;
use std::time::{Duration, Instant};

/// **Whether phase A may run `command`**: one of its few read-only command
/// lines, word for word, on a device discovery could name (`LABEL=`,
/// `UUID=` and `..` never), with nothing on standard input —
///
/// - `dumpe2fs -h <partition>` and `resize2fs -P <partition>`: an ext4's
///   size, what it uses and the least it shrinks to;
/// - `ntfsresize --info --no-progress-bar <partition>` and `ntfscat
///   <partition> hiberfil.sys` (its head alone, [`only_its_head`]): an
///   NTFS's numbers and its sign of hibernation;
/// - `e2fsck -f -n <partition>` and `ntfsresize --no-action
///   --no-progress-bar --size <bytes> <partition>`: the dry runs.
///
/// Never e2fsck without `-n`, resize2fs with a size, ntfsresize without
/// `--info` or `--no-action` or with `--force`, ntfscat of anything else or
/// with `-f`, or any other program: whatever writes is phase B.1's, and goes
/// through the fence.
pub fn allowed(command: &Command) -> bool {
    if command.input.is_some() {
        return false;
    }
    let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
    match (command.program, args.as_slice()) {
        ("dumpe2fs", ["-h", device])
        | ("resize2fs", ["-P", device])
        | ("ntfsresize", ["--info", "--no-progress-bar", device])
        | ("ntfscat", [device, "hiberfil.sys"])
        | ("e2fsck", ["-f", "-n", device]) => device_path_ok(device),
        ("ntfsresize", ["--no-action", "--no-progress-bar", "--size", size, device]) => {
            bytes(size) && device_path_ok(device)
        }
        _ => false,
    }
}

/// Whether only the head of what `command` prints is read, the program then
/// ended: ntfscat's, whose `hiberfil.sys` can be gigabytes. Such a command
/// is never read whole, nor any other only by its head.
pub fn only_its_head(command: &Command) -> bool {
    command.program == "ntfscat"
}

/// A size in bytes as the builders write one: digits, more than nothing,
/// no sign and no leading zero.
fn bytes(text: &str) -> bool {
    text.parse::<u64>()
        .is_ok_and(|n| n > 0 && n.to_string() == text)
}

/// The command line as the log shows it.
fn shown(command: &Command) -> String {
    std::iter::once(command.program)
        .chain(command.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

/// **How long phase A reads the Windows volumes of every disk, at the
/// most** (a slow or failing disk — `ntfsresize --info` walks a volume's
/// whole MFT — would keep the run in Check for as long as it took): a minute,
/// all of them together. Why a minute: the interface offered this shrink only
/// because the measure entry read these very volumes, and the rest of the
/// disks with them, within the minute it waits for its answer
/// (`partitions::READING_PATIENCE`), else they were unread and nothing was
/// offered; read again by the same commands moments later, their metadata now
/// in the page cache, they need no more. Past it, a volume not read — its
/// reading ended, or never begun — is unread (`Measure::Unreadable`), which
/// holds every NTFS back (`Measures::shrink_of`): the bound can only refuse a
/// shrink, never offer one, and nothing is written before it.
pub const WINDOWS_PATIENCE: Duration = Duration::from_secs(60);

/// The measure entry's command as the worker runs one.
fn command_of(command: &measure::Command) -> Command {
    let args: Vec<&str> = command.args().iter().map(String::as_str).collect();
    Command::new(command.program(), &args)
}

/// **What a tool's verdict stops a shrink with**, or `None` where it goes
/// on: a check due (errors left, a check resize2fs asks first, Windows'
/// own mark, an inconsistency) is `FilesystemNeedsCheck`; e2fsck asking
/// for a restart, or a tool that did not do what it was asked,
/// `ShrinkRefused`.
pub fn failure_of(verdict: Verdict) -> Option<Failure> {
    match verdict {
        Verdict::GoOn => None,
        Verdict::NeedsCheck => Some(Failure::FilesystemNeedsCheck),
        Verdict::Restart | Verdict::Failed => Some(Failure::ShrinkRefused),
    }
}

/// The measure entry's readings, asked of the worker's machine under phase
/// A's own allowlist: a command the allowlist refuses is never asked, and
/// the first such is kept, so the reading stops with it rather than with
/// the unread answer the measure's rules would give it. So is a program the
/// live system does not have (`MissingTool`, which `check::required_tools`
/// asks after first): Stopped says to use a newer USB stick, not that a
/// partition could not be read.
struct Again<'a> {
    machine: &'a mut dyn Machine,
    refused: Option<Stop>,
    /// Until when each command may run, where the reading is bounded
    /// ([`WINDOWS_PATIENCE`]): each is given what is left, and none is begun
    /// once nothing is.
    until: Option<Instant>,
}

impl Again<'_> {
    fn refuse(&mut self, command: &Command, why: &str) -> Stop {
        let stop = Stop::new(
            Failure::Internal,
            format!("{} is not a command phase A {why}", shown(command)),
        );
        self.refused.get_or_insert_with(|| stop.clone());
        stop
    }

    /// What is left of the time the reading is bounded by, or `None` where
    /// it is not bounded; `Err` once nothing is left, no command begun.
    fn left(&mut self, command: &Command) -> Result<Option<Duration>, Stop> {
        let Some(until) = self.until else {
            return Ok(None);
        };
        match until.checked_duration_since(self.machine.now()) {
            Some(left) if !left.is_zero() => Ok(Some(left)),
            _ => Err(Stop::new(
                Failure::CommandFailed,
                format!("{}: not begun, phase A's time for it is up", shown(command)),
            )),
        }
    }

    /// `stop`, kept as what the reading stops with where it says a program
    /// is missing.
    fn missing(&mut self, stop: Stop) -> Stop {
        if stop.failure == Failure::MissingTool {
            self.refused.get_or_insert_with(|| stop.clone());
        }
        stop
    }
}

impl Reads for Again<'_> {
    fn run(&mut self, command: &measure::Command) -> Result<Ran, Stop> {
        let command = command_of(command);
        if !allowed(&command) || only_its_head(&command) {
            return Err(self.refuse(&command, "reads whole"));
        }
        let read = match self.left(&command)? {
            None => self.machine.read_status(&command),
            Some(left) => self.machine.read_status_within(&command, left),
        };
        let status = match read {
            Ok(status) => status,
            Err(stop) => return Err(self.missing(stop)),
        };
        Ok(Ran {
            success: status.code == 0,
            stdout: status.stdout,
            stderr: status.stderr,
        })
    }

    fn head(&mut self, command: &measure::Command) -> Result<Head, Stop> {
        let command = command_of(command);
        if !allowed(&command) || !only_its_head(&command) {
            return Err(self.refuse(&command, "reads the head of"));
        }
        let head = match self.left(&command)? {
            None => self.machine.head(&command),
            Some(left) => self.machine.head_within(&command, left),
        };
        head.map_err(|stop| self.missing(stop))
    }
}

/// Every partition `layout` shrinks the filesystem of: its disk, its part
/// and the partition as reviewed, disks in path order, each by where it
/// starts.
fn shrunk(layout: &[DiskLayout]) -> Vec<(&DiskLayout, &Part, &Partition)> {
    let mut found: Vec<_> = layout
        .iter()
        .flat_map(|one| {
            one.parts
                .iter()
                .zip(&one.disk.partitions)
                .filter(|(part, _)| matches!(part.fate, Fate::Shrink { .. }))
                .map(move |(part, was)| (one, part, was))
        })
        .collect();
    found.sort_by(|a, b| (&a.0.disk.path, a.1.start).cmp(&(&b.0.disk.path, b.1.start)));
    found
}

/// Why `device` is in use — mounted in the live session, active swap, held
/// by anything (`check::device_busy`) — or `None`; an error where that could
/// not be read.
fn in_use(machine: &mut dyn Machine, device: &str) -> Result<Option<String>, String> {
    let mountinfo = machine
        .session_mounts()
        .map_err(|e| format!("cannot read the live system's mounts: {e}"))?;
    let swaps = machine.swaps();
    let (major_minor, held) = machine.device_facts(device)?;
    Ok(check::device_busy(
        device,
        &major_minor,
        held,
        &mountinfo,
        &swaps,
    ))
}

/// `partition`'s measure, named by the partition as it was read.
fn named(partition: &Partition, measure: Measure) -> Measured {
    Measured {
        path: partition.path.clone(),
        fs: partition.fs_type.clone(),
        uuid: partition.uuid.clone(),
        size: partition.size,
        measure,
    }
}

/// **Phase A, steps 1 and 2: every filesystem `layout` shrinks read again**,
/// just before anything is written (the module's 1. and 2.): each
/// partition shrunk given the fence's leave, claimed and found in use
/// nowhere; each ext4 shrunk read by the measure's own reading; and, where
/// an NTFS is shrunk, every NTFS and BitLocker volume of every disk found
/// now (`windows_volumes`). Gives what was read, for `layout::measured`
/// to judge each shrink by (`Measures::shrink_of`, the rule the interface
/// judged it by); nothing where nothing is shrunk.
pub fn read_again(
    layout: &[DiskLayout],
    fence: &Fence,
    machine: &mut dyn Machine,
    steps: &mut Steps,
) -> Result<Measures, Stop> {
    let shrunk = shrunk(layout);
    if shrunk.is_empty() {
        return Ok(Measures::default());
    }
    for (_, part, _) in &shrunk {
        let device = part.path.as_str();
        fence.check(device, super::plan::Act::ShrinkFilesystem, machine.sysfs())?;
        machine.claim(device)?;
        match in_use(machine, device) {
            Ok(None) => {}
            Ok(Some(why)) => return Err(Stop::new(Failure::DiskBusy, why)),
            Err(why) => return Err(Stop::new(Failure::DiskBusy, why)),
        }
    }
    let mut again = Again {
        machine,
        refused: None,
        until: None,
    };
    let mut parts = Vec::new();
    for (_, _, was) in shrunk.iter().filter(|(_, _, was)| was.fs_type == "ext4") {
        parts.push(named(
            was,
            measure::read_ext4(&mut again, &was.path, was.size),
        ));
    }
    if shrunk.iter().any(|(_, _, was)| was.fs_type == "ntfs") {
        let checked: Vec<&str> = shrunk
            .iter()
            .map(|(_, part, _)| part.path.as_str())
            .collect();
        parts.extend(windows_volumes(layout, &checked, &mut again)?);
    }
    if let Some(stop) = again.refused {
        return Err(stop);
    }
    for one in &parts {
        steps.log(format!(
            "layout: read again before the shrink: {} ({}) {:?}",
            one.path, one.fs, one.measure
        ));
    }
    Ok(Measures { parts })
}

/// **Every NTFS and BitLocker volume of every disk the worker finds now**,
/// as the measure entry answers them: a disk of the layout (found again as
/// reviewed by Check) by its partitions as reviewed; any other disk by its
/// partitions as found, each asked first whether it is in use (`InUse`,
/// unread, where it is; `Unreadable` where that cannot be said); a disk
/// nothing may be made on (`measure::unusable`: the installation medium, a
/// read-only disk, one not read completely) answered unread, nothing read
/// (`measure::unread`). The partitions in `checked` — those shrunk, already
/// claimed and found in use nowhere — are read without being asked again.
/// **All of it within [`WINDOWS_PATIENCE`]**: each command is given what is
/// left of it and ended past it, and a volume reached once it is up is not
/// read at all — unread, either way.
fn windows_volumes(
    layout: &[DiskLayout],
    checked: &[&str],
    again: &mut Again,
) -> Result<Vec<Measured>, Stop> {
    let found: Vec<Disk> = again.machine.discover().map_err(|why| {
        Stop::new(
            Failure::DiskChanged,
            format!("the disks cannot be read again before the shrink: {why}"),
        )
    })?;
    let until = again.machine.now() + WINDOWS_PATIENCE;
    again.until = Some(until);
    let parts = windows_volumes_of(layout, checked, &found, until, again);
    again.until = None;
    Ok(parts)
}

/// [`windows_volumes`] of the disks `found`, read until `until`.
fn windows_volumes_of(
    layout: &[DiskLayout],
    checked: &[&str],
    found: &[Disk],
    until: Instant,
    again: &mut Again,
) -> Vec<Measured> {
    let mut parts = Vec::new();
    for disk in found {
        let laid = layout.iter().find(|one| one.disk.path == disk.path);
        if laid.is_none() && measure::unusable(disk) {
            for partition in &disk.partitions {
                if let Some(unread) = measure::unread(partition) {
                    crate::worker::run::log(&format!(
                        "layout: {} is on {}, which nothing may be made on: not read",
                        partition.path, disk.path
                    ));
                    parts.push(named(partition, unread));
                }
            }
            continue;
        }
        let partitions = laid.map_or(&disk.partitions, |one| &one.disk.partitions);
        for partition in partitions {
            let Some(kind @ (Kind::Ntfs | Kind::Encrypted)) = measure::kind_of(partition) else {
                continue;
            };
            let device = partition.path.as_str();
            if again.machine.now() >= until {
                crate::worker::run::log(&format!(
                    "layout: {device} not read: phase A's {} seconds for the Windows volumes \
                     are up",
                    WINDOWS_PATIENCE.as_secs()
                ));
                parts.push(named(partition, Measure::Unreadable));
                continue;
            }
            let busy = if checked.contains(&device) {
                Ok(None)
            } else {
                in_use(again.machine, device)
            };
            let read = match busy {
                Err(why) => {
                    crate::worker::run::log(&format!("layout: {why}"));
                    Measure::Unreadable
                }
                Ok(Some(why)) => {
                    crate::worker::run::log(&format!("layout: {why}"));
                    Measure::InUse
                }
                Ok(None) if kind == Kind::Encrypted => Measure::Encrypted,
                Ok(None) => measure::read_ntfs(again, device, partition.size),
            };
            parts.push(named(partition, read));
        }
    }
    parts
}

/// One filesystem phase A has readied to be shrunk: the shrink, and its
/// partition's disk and start, by which the fence knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ready {
    pub shrink: Shrink,
    pub disk: String,
    pub start: u64,
}

/// **Phase A, step 3: each shrink built from what was read** (`planned`,
/// the layout's `Planned::ShrinkFilesystem`s), by the rule set's reading of
/// it (`Measures::shrink_of`): the filesystem's own size — what it uses and
/// what is free on it, as read again — handed to the builder, which refuses
/// a size its tool is given at or above it (`Shrink::of`; `ShrinkRefused`),
/// and to the fence, which then lets that shrink's commands through to less
/// than it alone.
///
/// **Where the new entry holds the filesystem as it is**
/// (`Shrinkable::entry_only`, as the rule set and the interface judged the
/// layout), nothing is readied: the filesystem is never handed to its tool,
/// which would grow it or leave it, and only its entry is made smaller in
/// phase B. The fence, which makes that entry smaller only once it has
/// what this run read of the filesystem inside it, is handed it now: an
/// ext4's blocks as `dumpe2fs -h` said them just before; an NTFS volume's
/// size as its boot sector says it ([`entry_holds`]).
pub fn ready(
    layout: &[DiskLayout],
    planned: &[&Planned],
    measures: &Measures,
    fence: &mut Fence,
    machine: &mut dyn Machine,
    steps: &mut Steps,
) -> Result<Vec<Ready>, Stop> {
    let refused = |detail: String| Stop::new(Failure::ShrinkRefused, detail);
    let mut readied = Vec::new();
    for planned in planned {
        let Planned::ShrinkFilesystem { device, to, .. } = planned else {
            continue;
        };
        let (one, part, was) = shrunk(layout)
            .into_iter()
            .find(|(_, part, _)| part.path == *device)
            .ok_or_else(|| refused(format!("{device} is shrunk by no part of the layout")))?;
        let shrinkable = match measures.shrink_of(was) {
            Some(Ok(shrinkable)) => shrinkable,
            other => {
                return Err(refused(format!(
                    "{device} was not read again before its shrink: {other:?}"
                )))
            }
        };
        let filesystem = shrinkable.filesystem;
        if shrinkable.entry_only(*to) {
            let bytes = entry_holds(device, &was.fs_type, filesystem, *to, machine)?;
            steps.log(format!(
                "layout: {device}'s filesystem is {bytes} bytes, inside its new entry of {to}: \
                 only its entry is made smaller"
            ));
            fence.filesystem_read_back(&one.disk.path, part.start, bytes);
            continue;
        }
        let shrink = Shrink::of(planned, filesystem).map_err(refused)?;
        fence.measured(&one.disk.path, part.start, filesystem);
        readied.push(Ready {
            shrink,
            disk: one.disk.path.clone(),
            start: part.start,
        });
    }
    Ok(readied)
}

/// What this run read of a filesystem whose new entry, `to` bytes, holds it
/// as it is ([`ready`]), for the fence: an ext4's size as phase A's `dumpe2fs
/// -h` gave it (`filesystem`); an NTFS volume's as its boot sector says it
/// (512 bytes read, `resize::ntfs_volume`) — ntfsresize counts only its
/// clusters and a sector, while the boot sector's count, which the backup
/// boot sector follows, may run further — and it must end inside the entry
/// by its tool's rule (`Tool::ends_inside`: a sector short of it, for the
/// backup), else `ShrinkRefused`, nothing written.
fn entry_holds(
    device: &str,
    fs: &str,
    filesystem: u64,
    to: u64,
    machine: &mut dyn Machine,
) -> Result<u64, Stop> {
    let refused = |detail: String| Stop::new(Failure::ShrinkRefused, detail);
    let tool = Tool::of(fs).ok_or_else(|| refused(format!("{device}: {fs:?} is not shrunk")))?;
    let bytes = match tool {
        Tool::Ext4 => filesystem,
        Tool::Ntfs => {
            let boot = machine.first_sector(device)?;
            resize::ntfs_volume(&boot)
                .ok_or_else(|| refused(format!("{device} holds no NTFS boot sector")))?
        }
    };
    if !tool.ends_inside(bytes, to) {
        return Err(refused(format!(
            "{device}'s filesystem is {bytes} bytes by its own count, which a new entry of {to} \
             does not hold whole: only its entry was to be made smaller"
        )));
    }
    Ok(bytes)
}

/// **Phase A, step 4: each shrink's dry run**, which writes nothing —
/// `e2fsck -f -n`, or `ntfsresize --no-action` to the new size — held to
/// phase A's allowlist, its end read by its tool's rule ([`failure_of`]).
pub fn dry_runs(
    readied: &[Ready],
    machine: &mut dyn Machine,
    steps: &mut Steps,
) -> Result<(), Stop> {
    for one in readied {
        let step = one.shrink.dry_run();
        if !allowed(&step.command) {
            return Err(Stop::new(
                Failure::Internal,
                format!("{} is not a command phase A runs", shown(&step.command)),
            ));
        }
        let status = machine.read_status(&step.command)?;
        steps.log(format!(
            "layout: the dry run {} ended {}",
            shown(&step.command),
            status.code
        ));
        if let Some(failure) = failure_of(step.judge.verdict(&status)) {
            return Err(Stop::new(
                failure,
                format!(
                    "the dry run of {}'s shrink refused it: {} ended {}: {}",
                    one.shrink.device,
                    shown(&step.command),
                    status.code,
                    format!("{}\n{}", status.stdout, status.stderr).trim()
                ),
            ));
        }
    }
    Ok(())
}

/// How many units of the Resize step [`carry_out`] reports for `readied`:
/// each command, and each read back.
pub fn units(readied: &[Ready]) -> u64 {
    readied
        .iter()
        .map(|one| one.shrink.planned().len() as u64 + 1)
        .sum()
}

/// **Phase B.1: each filesystem shrunk, before any entry is changed on any
/// disk**: each of its commands given the fence's leave, its partition
/// claimed and found in use nowhere, then run, its end read by its tool's
/// rule ([`failure_of`]); what it left read back (`read_back`), held
/// inside its new entry (`Shrink::fits`) and handed to the fence, which
/// makes that entry smaller only then (`Fence::filesystem_read_back`). Once
/// an NTFS is shrunk, Done is told Windows will check its disk.
pub fn carry_out(
    readied: &[Ready],
    fence: &mut Fence,
    machine: &mut dyn Machine,
    steps: &mut Steps,
) -> Result<(), Stop> {
    let mut warned = false;
    for one in readied {
        let shrink = &one.shrink;
        for (run, judge) in shrink.planned() {
            steps.log(format!("layout: {}", run.shown()));
            let command = carry::leave(&run, fence, machine)?.clone();
            let status = machine.write_status(&command)?;
            if let Some(failure) = failure_of(judge.verdict(&status)) {
                return Err(Stop::new(
                    failure,
                    format!(
                        "{} ended {}: {}",
                        shown(&command),
                        status.code,
                        format!("{}\n{}", status.stdout, status.stderr).trim()
                    ),
                ));
            }
            steps.advance();
        }
        let left = read_back(shrink, machine)?;
        if !shrink.fits(left) {
            return Err(Stop::new(
                Failure::ShrinkRefused,
                format!(
                    "{}'s filesystem is {left} bytes once shrunk, not inside its new entry of {}",
                    shrink.device, shrink.to
                ),
            ));
        }
        steps.log(format!(
            "layout: {}'s filesystem is {left} bytes, inside its new entry of {}",
            shrink.device, shrink.to
        ));
        fence.filesystem_read_back(&one.disk, one.start, left);
        steps.advance();
        if shrink.tool == Tool::Ntfs && !warned {
            warned = true;
            steps.warn(Warning::WindowsWillCheck);
        }
    }
    Ok(())
}

/// The size of the filesystem `shrink` left, read back: an ext4's blocks by
/// `dumpe2fs -h` (held to phase A's allowlist; one not put away cleanly
/// asks for a check), an NTFS volume's by its boot sector
/// (`resize::ntfs_volume`), since `ntfsresize --info` refuses a volume
/// marked for Windows' check, which the shrink leaves.
fn read_back(shrink: &Shrink, machine: &mut dyn Machine) -> Result<u64, Stop> {
    let refused = |detail: String| Stop::new(Failure::ShrinkRefused, detail);
    match shrink.tool {
        Tool::Ext4 => {
            let command = command_of(&measure::Command::ext4_header(&shrink.device));
            if !allowed(&command) {
                return Err(Stop::new(
                    Failure::Internal,
                    format!("{} is not a command phase A runs", shown(&command)),
                ));
            }
            let status = machine.read_status(&command)?;
            let header = Ext4Header::parse(&status.stdout)
                .filter(|_| status.code == 0)
                .ok_or_else(|| {
                    refused(format!(
                        "dumpe2fs said nothing readable of {} once shrunk",
                        shrink.device
                    ))
                })?;
            if !header.clean {
                return Err(Stop::new(
                    Failure::FilesystemNeedsCheck,
                    format!("{} was not left clean by its shrink", shrink.device),
                ));
            }
            Ok(header.bytes())
        }
        Tool::Ntfs => {
            let boot = machine.first_sector(&shrink.device)?;
            resize::ntfs_volume(&boot).ok_or_else(|| {
                refused(format!(
                    "{} holds no NTFS boot sector once shrunk",
                    shrink.device
                ))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::GIB;

    fn command(program: &'static str, args: &[&str]) -> Command {
        Command::new(program, args)
    }

    /// **Phase A runs only its own read-only command lines** (Q9): each
    /// the builders make — the measure's readings of an ext4 and an NTFS and
    /// its sign of hibernation, and each shrink's dry run — word for word,
    /// and nothing else: no command that writes (the shrink's own among
    /// them), no other flag, file or program, no device discovery would
    /// not name, nothing on standard input; and ntfscat's alone only by its
    /// head.
    #[test]
    fn phase_a_runs_only_its_own_read_only_commands() {
        let device = "/dev/nvme0n1p3";
        let built = [
            command_of(&measure::Command::ext4_header(device)),
            command_of(&measure::Command::ext4_least(device)),
            command_of(&measure::Command::ntfs_info(device)),
            command_of(&measure::Command::ntfs_hibernation(device)),
        ];
        for one in &built {
            assert!(allowed(one), "{}", shown(one));
            assert_eq!(only_its_head(one), one.program == "ntfscat");
        }
        let planned = |fs: &str| Planned::ShrinkFilesystem {
            device: device.into(),
            fs: fs.into(),
            from: 30 * GIB,
            to: 20 * GIB,
        };
        for fs in ["ext4", "ntfs"] {
            let shrink = Shrink::of(&planned(fs), 30 * GIB).unwrap();
            let dry = shrink.dry_run().command;
            assert!(allowed(&dry), "{}", shown(&dry));
            assert!(!only_its_head(&dry));
            // What phase B.1 runs writes, and is never phase A's.
            for (run, _) in shrink.planned() {
                let written = run.command().unwrap();
                assert!(!allowed(written), "{}", shown(written));
            }
        }
        let size = (20 * GIB).to_string();
        for (program, args) in [
            ("e2fsck", vec!["-f", "-y", device]),
            ("e2fsck", vec!["-f", "-p", device]),
            ("e2fsck", vec!["-fn", device]),
            ("e2fsck", vec!["-n", device]),
            ("e2fsck", vec!["-f", "-n", "-E", "discard", device]),
            ("e2fsck", vec!["-f", "-n", "-z", "/tmp/undo", device]),
            ("resize2fs", vec![device, "20971520K"]),
            ("resize2fs", vec!["-P", "-f", device]),
            ("resize2fs", vec!["-M", device]),
            ("dumpe2fs", vec![device]),
            ("dumpe2fs", vec!["-h", "-o", "superblock=32768", device]),
            (
                "ntfsresize",
                vec!["--no-progress-bar", "--size", &size, device],
            ),
            (
                "ntfsresize",
                vec![
                    "--force",
                    "--no-action",
                    "--no-progress-bar",
                    "--size",
                    &size,
                    device,
                ],
            ),
            ("ntfsresize", vec!["--no-action", "--size", &size, device]),
            (
                "ntfsresize",
                vec!["--no-action", "--no-progress-bar", "--size", "20G", device],
            ),
            (
                "ntfsresize",
                vec!["--no-action", "--no-progress-bar", "--size", "0", device],
            ),
            (
                "ntfsresize",
                vec!["--no-action", "--no-progress-bar", "--size", "-1", device],
            ),
            ("ntfsresize", vec!["--info", device]),
            (
                "ntfsresize",
                vec!["--info", "--force", "--no-progress-bar", device],
            ),
            ("ntfscat", vec!["-f", device, "hiberfil.sys"]),
            ("ntfscat", vec![device, "pagefile.sys"]),
            ("ntfscat", vec![device, "HIBERFIL.SYS"]),
            ("ntfscat", vec!["-i", "2", device]),
            ("ntfscat", vec![device]),
            ("ntfsinfo", vec!["-m", device]),
            ("ntfsfix", vec![device]),
            ("tune2fs", vec!["-l", device]),
            ("debugfs", vec!["-R", "stats", device]),
            ("mkfs.ext4", vec!["-F", device]),
            ("sfdisk", vec!["--json", "/dev/nvme0n1"]),
        ] {
            assert!(!allowed(&command(program, &args)), "{program} {args:?}");
        }
        // Only a device discovery would name.
        for odd in [
            "LABEL=Windows",
            "UUID=0123",
            "/dev/../dev/sda3",
            "/tmp/x.img",
            "sda3",
            "/dev/",
        ] {
            for one in [
                command("dumpe2fs", &["-h", odd]),
                command("e2fsck", &["-f", "-n", odd]),
                command("ntfscat", &[odd, "hiberfil.sys"]),
                command(
                    "ntfsresize",
                    &["--no-action", "--no-progress-bar", "--size", &size, odd],
                ),
            ] {
                assert!(!allowed(&one), "{}", shown(&one));
            }
        }
        // Nothing on standard input, not even the shrink's own answer.
        let answered = command("e2fsck", &["-f", "-n", device]).with_input("y\n".into());
        assert!(!allowed(&answered));
    }

    /// **Phase A gives the Windows volumes the minute the interface gave
    /// the measure entry** (`WINDOWS_PATIENCE`, why it is a minute): no
    /// longer, so a shrink offered on what was read in a minute is not
    /// refused for a reading again that took as long.
    #[test]
    fn phase_a_waits_for_windows_as_long_as_partitions_did() {
        let partitions = crate::ui::screens::partitions::READING_PATIENCE;
        assert_eq!(WINDOWS_PATIENCE, Duration::from_secs_f32(partitions));
    }

    /// **Each tool's verdict is the failure Stopped says** (Q9): going on
    /// is none, a check due `FilesystemNeedsCheck`, a restart asked for or
    /// a tool that did not do what it was asked `ShrinkRefused`.
    #[test]
    fn each_verdict_stops_a_shrink_with_its_own_failure() {
        assert_eq!(failure_of(Verdict::GoOn), None);
        assert_eq!(
            failure_of(Verdict::NeedsCheck),
            Some(Failure::FilesystemNeedsCheck)
        );
        assert_eq!(failure_of(Verdict::Restart), Some(Failure::ShrinkRefused));
        assert_eq!(failure_of(Verdict::Failed), Some(Failure::ShrinkRefused));
    }
}
