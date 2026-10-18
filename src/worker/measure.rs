//! The read-only measure entry: `lxb-installer --measure`, which polkit
//! starts as root under an action of its own
//! (`io.github.petexy.lxb-installer.measure`) so that Partitions can say how
//! full a partition is and whether it may be shrunk — which a person's own
//! process cannot read. It reads, and never writes to a disk.
//!
//! CONTRACT: [`main`] is the `--measure` entry, accepted only alone. It reads
//! one `MeasureRequest` from standard input, prints one `MeasureAnswer` line
//! on standard output, and returns the exit status (0 for a measurement, 1
//! for a refusal). Its log lines go to standard error and nowhere else.
//!
//! In this order, each step only once the one before it allowed it:
//!
//! 1. the hard guard, off the Linexin live medium first and root or not
//!    (`first_refusal`, the worker's own), then not being root;
//! 2. archiso started this system and its image is mounted as archiso
//!    mounts it (`live::detect`);
//! 3. the worker's hardening (`check::harden`);
//! 4. `/run/lxb-installer`, made or checked (`report::prepare_directory`), for
//!    the lock and the mount point below;
//! 5. the worker's lock (`check::lock`): `AlreadyRunning` beside a worker or
//!    another measurement. The worker's progress file, log and pid file are
//!    never opened — opening them starts them afresh, which would wipe an
//!    installation's record — so a measurement leaves no trace there;
//! 6. the request, read unbuffered and within `MAX_REQUEST`;
//! 7. every disk found again (`disks::discover`) and compared field by field
//!    with the one listed (`check::same_disk`): a disk that is not the one
//!    listed is not read at all, and is answered as `changed`; one listed as
//!    a disk nothing may be made on (the installation medium, read-only, not
//!    read completely) is not read either, and is named only if it is not
//!    exactly as listed any more. Of a disk not read, each Windows volume as
//!    listed — an NTFS, a BitLocker volume — is answered all the same,
//!    unread (`Unreadable`, `Encrypted`), with nothing read: the rule set
//!    judges a shrink of an NTFS across every disk of the measurement
//!    (`Measures::shrink_of`), and a Windows on a disk not read is one whose
//!    state is not known;
//! 8. every partition that would be read, asked first whether it is in use
//!    (`check::device_busy`): one that is answers `InUse` and is not read;
//! 9. a mount namespace of its own (`check::isolate_mounts`, the worker's
//!    own), whose `mount --make-rprivate /` is **the one command it runs as
//!    root outside [`allowed`]**: it only stops mounts made in this
//!    process's new namespace — the EFI partition's, below — from reaching
//!    the live session's, and back; it names no disk, writes nothing to
//!    one, and goes through `run::run`, which the test build's fence holds;
//! 10. the reading itself, each command held to [`allowed`] as it starts
//!     (and to the starter its output needs: the whole of it, or only its
//!     head, [`only_its_head`]);
//! 11. the one line out.
//!
//! What it reads, and nothing else ([`allowed`] holds every command but
//! step 9's to these shapes at the moment it starts, and
//! `measure_commands_are_read_only` pins them):
//!
//! - an ext4: `dumpe2fs -h` (its size, what is free, whether it was put away
//!   cleanly) and, if it was, `resize2fs -P` (the least it shrinks to; with
//!   `-P` resize2fs opens it read-only and stops);
//! - an NTFS (Windows' own, not its recovery partition):
//!   `ntfsresize --info --no-progress-bar`, which opens it read-only and
//!   refuses when a check is due. It cannot say whether Windows is
//!   hibernated, or was shut down by Fast Startup, or left its journal
//!   unclean — it opens every volume "forensic", and libntfs-3g then skips
//!   both checks — so an NTFS it reads cleanly is then asked the read-only
//!   sign of hibernation, the one such read allowed: `ntfscat <partition>
//!   hiberfil.sys`, its output cut at [`HIBERFILE_HEAD`] bytes and the
//!   program then ended ([`Reads::head`]), judged by libntfs-3g's own rule
//!   ([`hibernation`]). Signed not hibernated by a `hiberfil.sys` of its
//!   own, it is read; hibernated, it is `Hibernated`; with no `hiberfil.sys`
//!   — what a Windows' data volume is — `NoHiberfile`, its numbers known
//!   but never shrunk, since whether a hibernated Windows still keeps its
//!   metadata only its journal says ([`Hibernation::Absent`]); anything
//!   else `Unreadable` ([`signed`]). Whether one may be shrunk is the rule
//!   set's to say, across every NTFS of the answer (`Measures::shrink_of`):
//!   one signed hibernated, or unread, anywhere holds every other back. Its
//!   journal is not looked at: no read-only sign of an unclean journal was
//!   shown to be libntfs-3g's own rule (§6, Unit Q, step Q8);
//! - an EFI system partition: `esp::measure` — FAT mounted read-only in this
//!   process's own namespace for a moment, measured, unmounted;
//! - BitLocker and LUKS: nothing at all (`Encrypted`);
//! - the image: `du -sb --one-file-system` of each of `IMAGE_PATHS` in it.
//!
//! Never: e2fsck, any mkfs, sfdisk, wipefs, ntfsresize without `--info`,
//! resize2fs without `-P`, ntfscat of anything but `hiberfil.sys`'s head
//! (nor with `-f`, which would let it open a volume marked for a check), or
//! a mount of ext4, Btrfs or NTFS — even a read-only mount can replay a
//! journal. Nothing it says is trusted with a write: the worker measures
//! again itself before its first one.
//!
//! Everything it asks of the machine goes through one [`Machine`], so the
//! tests hand in their own answers and see what was asked, in what order,
//! with nothing of the computer they run on read; and its own starter asks
//! the test build's fence before anything is looked up.
use super::layout::Stop;
use super::run;
use super::storage::esp;
use super::system::{check, report};
use crate::model::config::DEFAULT_SOURCE;
use crate::model::disks::{Content, Disk, Partition};
use crate::model::live::{self, LiveSession};
use crate::model::measure::{
    ImageSizes, Measure, MeasureAnswer, MeasureRequest, Measured, Measures, IMAGE_PATHS, MAX_ANSWER,
};
use crate::model::protocol::{Failure, MAX_REQUEST};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// Where the mount point an EFI system partition is measured at is made: a
/// new directory below the worker's own, by `mkdtemp`, whose name ends in
/// six letters and digits.
const SCRATCH: &str = "/run/lxb-installer/measure-";

/// The `--measure` entry point. Returns the exit status.
pub fn main() -> i32 {
    let mut machine = Real::default();
    measure(live::on_medium, check::is_root, &mut machine)
}

/// One command the measure entry runs: a program from the live system and
/// its arguments, exactly as [`allowed`] judges it and as it is started.
/// Made only by its own builders below, and held to [`allowed`] again as it
/// starts whoever made it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    program: &'static str,
    args: Vec<String>,
}

impl Command {
    fn new(program: &'static str, args: &[&str]) -> Command {
        Command {
            program,
            args: args.iter().map(|arg| arg.to_string()).collect(),
        }
    }

    /// `dumpe2fs -h`: an ext4's superblock — its size, what is free on it and
    /// whether it was put away cleanly — and nothing of what it holds.
    pub fn ext4_header(device: &str) -> Command {
        Command::new("dumpe2fs", &["-h", device])
    }

    /// `resize2fs -P`: the least an ext4 could be shrunk to, in its own
    /// blocks. With `-P` resize2fs opens the filesystem read-only, prints its
    /// estimate and stops.
    pub fn ext4_least(device: &str) -> Command {
        Command::new("resize2fs", &["-P", device])
    }

    /// `ntfsresize --info`: what an NTFS holds and the least it could be
    /// shrunk to, the volume opened read-only; with no progress bar, whose
    /// redrawing is nothing to read.
    pub fn ntfs_info(device: &str) -> Command {
        Command::new("ntfsresize", &["--info", "--no-progress-bar", device])
    }

    /// `ntfscat <partition> hiberfil.sys`: the first bytes of Windows'
    /// hibernation file, by which libntfs-3g tells a hibernated Windows
    /// ([`hibernation`]). ntfscat opens the volume read-only — `ntfs_mount`
    /// with flags 1, `NTFS_MNT_RDONLY`, and no `-f`, so libntfs-3g opens the
    /// device `O_RDONLY` and takes only a read lock — and refuses one marked
    /// for a check. The file can be gigabytes: only its head is ever read
    /// ([`only_its_head`]).
    pub fn ntfs_hibernation(device: &str) -> Command {
        Command::new("ntfscat", &[device, "hiberfil.sys"])
    }

    /// `du -sb --one-file-system`: the bytes below one of `IMAGE_PATHS` in
    /// the live system's image.
    pub fn image_size(path: &str) -> Command {
        Command::new("du", &["-sb", "--one-file-system", path])
    }

    /// The program it starts.
    pub fn program(&self) -> &'static str {
        self.program
    }

    /// Its arguments, in order.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// The command line as the log shows it.
    pub fn line(&self) -> String {
        let mut line = format!("$ {}", self.program);
        for arg in &self.args {
            line.push(' ');
            line.push_str(arg);
        }
        line
    }
}

/// Whether the measure entry may run `command`: one of its few read-only
/// command lines, word for word, on a device path discovery could have
/// printed, an image path of its own list, or its own mount point. Asked as
/// each command starts, the EFI system partition's mount and unmount among
/// them, so no other line can run whatever builds it. The one other command
/// the measure entry runs as root, `check::isolate_mounts`'
/// `mount --make-rprivate /`, is the worker's own and is not asked here: it
/// names no disk and changes only how its own namespace sees mounts (the
/// module's docs, step 9).
pub fn allowed(command: &Command) -> bool {
    let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
    match (command.program, args.as_slice()) {
        ("dumpe2fs", ["-h", device])
        | ("resize2fs", ["-P", device])
        | ("ntfsresize", ["--info", "--no-progress-bar", device])
        | ("ntfscat", [device, "hiberfil.sys"]) => plain_device(device),
        ("du", ["-sb", "--one-file-system", path]) => IMAGE_PATHS
            .iter()
            .any(|place| *path == format!("{DEFAULT_SOURCE}{place}")),
        ("mount", ["-t", "vfat", "-o", "ro,nosuid,nodev,noexec", device, target]) => {
            plain_device(device) && scratch(target)
        }
        ("umount", [target]) => scratch(target),
        _ => false,
    }
}

/// Whether only the head of what `command` prints is read, the program
/// then ended ([`Reads::head`]): `ntfscat`'s, whose `hiberfil.sys` can be
/// gigabytes. Such a command is never given to the starter that reads a
/// whole output, nor any other to the one that reads a head.
pub fn only_its_head(command: &Command) -> bool {
    command.program == "ntfscat"
}

/// `/dev/<name>` with a plain kernel name, as lsblk `--paths` prints every
/// disk and partition (the rule of discovery's own `device_path_ok`): never
/// `LABEL=` or `UUID=`, which the e2fsprogs tools would look up themselves,
/// and nothing that could climb out of `/dev`.
fn plain_device(path: &str) -> bool {
    path.strip_prefix("/dev/").is_some_and(|name| {
        !name.is_empty()
            && name.len() < 64
            && !name.starts_with('.')
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    })
}

/// The mount point `mkdtemp` makes from [`SCRATCH`], and nothing else.
fn scratch(path: &str) -> bool {
    path.strip_prefix(SCRATCH)
        .is_some_and(|name| name.len() == 6 && name.bytes().all(|c| c.is_ascii_alphanumeric()))
}

/// What a command said: whether it ended well, and what it printed on each
/// stream. A command that ended badly is read too — ntfsresize's refusal is
/// what says a check is due.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ran {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl Ran {
    fn text(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

/// How much of `hiberfil.sys` is read: the header libntfs-3g reads to tell a
/// hibernated Windows (`NTFS_HIBERFILE_HEADER_SIZE`, the 4096 its
/// `ntfs_volume_check_hiberfile` asks `ntfs_attr_pread` for).
pub const HIBERFILE_HEAD: usize = 4096;

/// What a command whose output is cut short said ([`Reads::head`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Head {
    /// What it printed on standard output, up to [`HIBERFILE_HEAD`] bytes;
    /// anything after was never read.
    pub bytes: Vec<u8>,
    /// How it ended: on its own, well (`Some(true)`) or badly
    /// (`Some(false)`); or `None`, ended by the starter once it had printed
    /// [`HIBERFILE_HEAD`] bytes and had not ended by itself.
    pub ended: Option<bool>,
    pub stderr: String,
}

/// What stands at one of `IMAGE_PATHS` in the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Directory,
    /// Nothing: the copy puts nothing there, so it takes no room.
    Missing,
    /// A link or a file, which `du` would not count as the copy does.
    Other,
}

/// What reading a partition asks of the machine: one command [`allowed`]
/// lets through, read to its end or only its head. The measure entry's
/// [`Machine`] answers both; so does the worker's own reading of a
/// filesystem again before it is shrunk (phase A, step Q9), under its own
/// allowlist, by these very commands and rules (`read_ext4`, `read_ntfs`).
pub trait Reads {
    /// Runs one command [`allowed`] lets through and says what it printed;
    /// an error only when it could not be started at all.
    fn run(&mut self, command: &Command) -> Result<Ran, Stop>;
    /// Runs one command [`allowed`] lets through whose output is cut short
    /// ([`only_its_head`]): reads at most [`HIBERFILE_HEAD`] bytes of it and
    /// then ends it, and says what it printed and how it ended; an error
    /// only when it could not be started at all.
    fn head(&mut self, command: &Command) -> Result<Head, Stop>;
}

/// What the measure entry asks of the machine, each question in one place,
/// so that a test hands in its own answers to [`measure`] and sees what was
/// asked and in what order, with nothing of the computer it runs on read.
/// [`Real`] is the machine's own answers. Its commands are [`Reads`]'.
pub trait Machine: Reads {
    /// `live::detect`: the live session, when this is one.
    fn live(&mut self) -> Option<LiveSession>;
    /// `check::harden`.
    fn harden(&mut self);
    /// `report::prepare_directory`.
    fn prepare_directory(&mut self) -> Result<(), String>;
    /// `check::lock`, held until the process ends.
    fn lock(&mut self) -> Result<(), Stop>;
    /// The request, from standard input ([`read_request`]).
    fn request(&mut self) -> Result<MeasureRequest, Stop>;
    /// `disks::discover`.
    fn discover(&mut self) -> Result<Vec<Disk>, String>;
    /// Why `device` is in use (`check::device_busy`), or `None`; an error
    /// when that could not be read.
    fn busy(&mut self, device: &str) -> Result<Option<String>, String>;
    /// `check::isolate_mounts`.
    fn isolate_mounts(&mut self) -> Result<(), Stop>;
    /// The bytes free on the EFI system partition `device`
    /// (`esp::measure`, its mount and unmount held to [`allowed`]).
    fn esp_free(&mut self, device: &str) -> Result<u64, Stop>;
    /// What stands at `path`, not following a link.
    fn entry(&mut self, path: &str) -> Entry;
    /// Prints the answer's one line.
    fn answer(&mut self, line: &str);
}

/// The measure entry on `machine`, `on_medium` and `is_root` being the
/// answers to the hard guard's two questions (the machine's in [`main`], a
/// test's own in a test). Prints its one line through `machine` and returns
/// the exit status: 0 for a measurement, 1 for a refusal.
pub fn measure(on_medium: fn() -> bool, is_root: fn() -> bool, machine: &mut dyn Machine) -> i32 {
    let answer = measured(on_medium, is_root, machine).unwrap_or_else(|stop| {
        run::log(&format!("measure: {}", stop.detail));
        MeasureAnswer::Refused {
            failure: stop.failure,
            detail: stop.detail,
        }
    });
    let (line, code) = line_of(&answer);
    machine.answer(&line);
    code
}

/// The line `answer` is printed as, and the exit status that goes with it.
/// A measurement whose line is longer than the interface reads
/// ([`MAX_ANSWER`]) is refused instead of being cut short.
fn line_of(answer: &MeasureAnswer) -> (String, i32) {
    let code = match answer {
        MeasureAnswer::Measured { .. } => 0,
        MeasureAnswer::Refused { .. } => 1,
    };
    let line = serde_json::to_string(answer).unwrap_or_default();
    if !line.is_empty() && line.len() as u64 <= MAX_ANSWER {
        return (line, code);
    }
    let refused = MeasureAnswer::Refused {
        failure: Failure::Internal,
        detail: format!(
            "the measurement is {} bytes, more than the interface reads",
            line.len()
        ),
    };
    (serde_json::to_string(&refused).unwrap_or_default(), 1)
}

/// What a partition that is read is read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::worker) enum Kind {
    Ext4,
    Ntfs,
    Esp,
    Encrypted,
}

/// Which partitions are read, and how: an ext4 or NTFS that could be shrunk,
/// an EFI system partition that could be kept, and BitLocker or LUKS, which
/// are answered without being read. Nothing else is read in this version —
/// no other filesystem is shrunk, and a kept one needs no numbers.
pub(in crate::worker) fn kind_of(partition: &Partition) -> Option<Kind> {
    match (partition.fs_type.as_str(), partition.content) {
        ("BitLocker" | "crypto_LUKS", _) => Some(Kind::Encrypted),
        ("ext4", Content::Linux) => Some(Kind::Ext4),
        ("ntfs", Content::Windows) => Some(Kind::Ntfs),
        ("vfat", Content::Efi) => Some(Kind::Esp),
        _ => None,
    }
}

/// What a Windows volume of a disk that is not read is answered, with
/// nothing read: an NTFS `Unreadable`, a BitLocker volume `Encrypted` — so
/// the rule set, judging a shrink of an NTFS across every disk of the
/// measurement, sees it as a Windows whose state is not known. Nothing else
/// of such a disk is answered (a LUKS volume, an ext4, an EFI partition
/// stand for no Windows).
pub(in crate::worker) fn unread(partition: &Partition) -> Option<Measure> {
    match (kind_of(partition)?, partition.fs_type.as_str()) {
        (Kind::Ntfs, _) => Some(Measure::Unreadable),
        (Kind::Encrypted, "BitLocker") => Some(Measure::Encrypted),
        _ => None,
    }
}

/// A partition's measure before anything is read: answered already, or to
/// be read as its kind once the mounts are the process's own.
enum Pending {
    Answered(Measure),
    Read(Kind),
}

/// The measurement in the plan's order (see the module), or the refusal
/// that stopped it.
fn measured(
    on_medium: fn() -> bool,
    is_root: fn() -> bool,
    machine: &mut dyn Machine,
) -> Result<MeasureAnswer, Stop> {
    // Off the Linexin live medium first, so that being root changes nothing
    // there; then root. Nothing else is asked before either.
    if let Some((failure, detail)) = super::first_refusal(on_medium, is_root) {
        return Err(Stop::new(failure, detail));
    }
    if machine.live().is_none() {
        return Err(Stop::new(
            Failure::NotLiveSession,
            "the system was not started from a Linexin live medium",
        ));
    }
    machine.harden();
    machine
        .prepare_directory()
        .map_err(|detail| Stop::new(Failure::Internal, detail))?;
    machine.lock()?;
    let request = machine.request()?;
    judge(&request)?;
    let found = machine.discover().map_err(|detail| {
        Stop::new(
            Failure::DiskChanged,
            format!("the disks cannot be read: {detail}"),
        )
    })?;

    // Everything that is not read is decided before anything is.
    let mut pending: Vec<(Partition, Pending)> = Vec::new();
    let mut changed = Vec::new();
    for disk in &request.disks {
        let now = match found_again(disk, &found) {
            Again::Read(now) => now,
            again => {
                if let Again::Changed = again {
                    changed.push(disk.path.clone());
                }
                for partition in &disk.partitions {
                    if let Some(measure) = unread(partition) {
                        pending.push((partition.clone(), Pending::Answered(measure)));
                    }
                }
                continue;
            }
        };
        for partition in &now.partitions {
            let Some(kind) = kind_of(partition) else {
                continue;
            };
            let next = match machine.busy(&partition.path) {
                Err(detail) => {
                    run::log(&format!("measure: {detail}"));
                    Pending::Answered(Measure::Unreadable)
                }
                Ok(Some(why)) => {
                    run::log(&format!("measure: {why}"));
                    Pending::Answered(Measure::InUse)
                }
                Ok(None) if kind == Kind::Encrypted => Pending::Answered(Measure::Encrypted),
                Ok(None) => Pending::Read(kind),
            };
            pending.push((partition.clone(), next));
        }
    }

    machine.isolate_mounts()?;
    let parts = pending
        .into_iter()
        .map(|(partition, pending)| {
            let measure = match pending {
                Pending::Answered(measure) => measure,
                Pending::Read(kind) => read(machine, &partition, kind),
            };
            Measured {
                path: partition.path,
                fs: partition.fs_type,
                uuid: partition.uuid,
                size: partition.size,
                measure,
            }
        })
        .collect();
    let image = image_sizes(machine);
    Ok(MeasureAnswer::Measured {
        measures: Measures { parts },
        image,
        changed,
    })
}

/// Refuses a request that could not have come from the interface: a disk
/// asked about twice, or a path discovery would never print. The paths
/// themselves are not repeated — they are not known to be plain text.
fn judge(request: &MeasureRequest) -> Result<(), Stop> {
    let invalid = |detail: String| Stop::new(Failure::InvalidRequest, detail);
    let mut seen = BTreeSet::new();
    for (n, disk) in request.disks.iter().enumerate() {
        let plain =
            plain_device(&disk.path) && disk.partitions.iter().all(|p| plain_device(&p.path));
        if !plain {
            return Err(invalid(format!(
                "disk number {} names something that is not a device",
                n + 1
            )));
        }
        if !seen.insert(disk.path.as_str()) {
            return Err(invalid(format!("{} is asked about twice", disk.path)));
        }
    }
    Ok(())
}

/// What became of a disk asked about, found again ([`found_again`]).
enum Again<'a> {
    /// Still the disk listed, and one a plan may use: the disk as it is now,
    /// whose partitions are then the ones listed, each asked about alone.
    Read(&'a Disk),
    /// Still exactly the disk listed, and listed as one nothing may be made
    /// on ([`unusable`]): no plan can use it, so nothing of it is read (its
    /// Windows volumes answered unread, [`unread`]), and the listing is
    /// still true of it, so it is not named either.
    Left,
    /// Not the disk listed any more: nothing of it is read (its Windows
    /// volumes as listed answered unread, [`unread`]), and the answer names
    /// it as changed, so the interface lists the disks again.
    Changed,
}

/// `before`, the disk as the interface listed it, found again among `found`.
///
/// A disk listed as one nothing may be made on — the installation medium, a
/// read-only disk, one whose state was not read completely — is only asked
/// whether it is still exactly that: the same disk with the same partitions
/// (`check::same_disk`, what blocks it set aside), blocked the same way. If
/// it is, it is [`Again::Left`]: named as changed, it would send the
/// interface to list the disks again, and again, for ever — and on a live
/// system the installation medium is always listed. Any difference makes it
/// [`Again::Changed`].
///
/// Any other disk must still be that disk (`check::same_disk`) to be read.
/// One with something on it in use is still the same disk; whether each
/// partition may be read is asked of the partition alone. Any other
/// difference — another disk, other partitions, a disk that is now the
/// installation medium, read-only, unreadable or gone — is
/// [`Again::Changed`].
fn found_again<'a>(before: &Disk, found: &'a [Disk]) -> Again<'a> {
    let Some(now) = found.iter().find(|d| d.path == before.path) else {
        run::log(&format!("measure: {} is gone", before.path));
        return Again::Changed;
    };
    if unusable(before) {
        let as_listed = now.blocked == before.blocked
            && now.read_only == before.read_only
            && check::same_disk(&unblocked(before), &unblocked(now)).is_ok();
        if as_listed {
            return Again::Left;
        }
        run::log(&format!("measure: {} is not as it was listed", before.path));
        return Again::Changed;
    }
    match check::same_disk(before, now) {
        Ok(()) | Err((Failure::DiskBusy, _)) => Again::Read(now),
        Err((_, detail)) => {
            run::log(&format!("measure: {detail}"));
            Again::Changed
        }
    }
}

/// Whether nothing may be made on `disk` as it is: what `check::same_disk`
/// refuses as no longer installable to (`DiskChanged`) even when the disk
/// is compared with itself — the installation medium, a read-only disk, one
/// whose state was not read completely — where a disk in use is only
/// `DiskBusy`. Asked of `same_disk` itself rather than written out again, so
/// a kind of block it learns later is judged here as it judges it.
pub(in crate::worker) fn unusable(disk: &Disk) -> bool {
    matches!(check::same_disk(disk, disk), Err((Failure::DiskChanged, _)))
}

/// `disk` with what blocks it set aside, for `check::same_disk` to compare
/// only which disk it is and what is on it.
fn unblocked(disk: &Disk) -> Disk {
    Disk {
        read_only: false,
        blocked: None,
        ..disk.clone()
    }
}

/// Reads one partition that is not in use, as its kind. An EFI system
/// partition says only what is free on it, by `statvfs` while it is
/// mounted; what is in use is the rest of it, its file allocation tables
/// counted in, as GParted counts them, and no tool names a least size.
fn read(machine: &mut dyn Machine, partition: &Partition, kind: Kind) -> Measure {
    let device = &partition.path;
    match kind {
        Kind::Ext4 => read_ext4(machine, device, partition.size),
        Kind::Ntfs => read_ntfs(machine, device, partition.size),
        Kind::Esp => match machine.esp_free(device) {
            Ok(free) if free <= partition.size => Measure::Read {
                used: partition.size - free,
                free,
                least: None,
            },
            Ok(free) => unreadable(&format!(
                "{device} has {free} bytes free, more than it holds"
            )),
            Err(stop) => unreadable(&stop.detail),
        },
        // Answered before anything is read; never read.
        Kind::Encrypted => Measure::Encrypted,
    }
}

fn unreadable(detail: &str) -> Measure {
    run::log(&format!("measure: {detail}"));
    Measure::Unreadable
}

/// An ext4 `size` bytes long: its header first, and only when that says it
/// was put away cleanly, the least it shrinks to. One whose tool names no
/// least is unreadable rather than read without one: ext4 is read to be
/// shrunk.
pub(in crate::worker) fn read_ext4<R: Reads + ?Sized>(
    machine: &mut R,
    device: &str,
    size: u64,
) -> Measure {
    let header = match machine.run(&Command::ext4_header(device)) {
        Ok(ran) if ran.success => ran.stdout,
        Ok(ran) => return unreadable(ran.stderr.trim()),
        Err(stop) => return unreadable(&stop.detail),
    };
    let Some(header) = Ext4Header::parse(&header) else {
        return unreadable(&format!("dumpe2fs said nothing readable of {device}"));
    };
    if !header.clean {
        return Measure::NeedsCheck;
    }
    if header.bytes() > size {
        return unreadable(&format!("{device} holds a filesystem larger than itself"));
    }
    let least = match machine.run(&Command::ext4_least(device)) {
        Ok(ran) => ext4_least(&ran),
        Err(stop) => return unreadable(&stop.detail),
    };
    match least {
        Least::Blocks(blocks) => Measure::Read {
            used: header.used(),
            free: header.free(),
            least: Some(blocks.saturating_mul(header.block_size)),
        },
        Least::NeedsCheck => Measure::NeedsCheck,
        Least::Unknown => unreadable(&format!("resize2fs said nothing readable of {device}")),
    }
}

/// An NTFS `size` bytes long: `ntfsresize --info`, then — only when that read
/// it cleanly — its read-only sign of hibernation ([`signed`]).
pub(in crate::worker) fn read_ntfs<R: Reads + ?Sized>(
    machine: &mut R,
    device: &str,
    size: u64,
) -> Measure {
    match machine.run(&Command::ntfs_info(device)) {
        Ok(ran) => match ntfs_measure(&ran, size) {
            read @ Measure::Read { .. } => signed(machine, device, read),
            said => said,
        },
        Err(stop) => unreadable(&stop.detail),
    }
}

/// What `dumpe2fs -h` says of an ext4 that the measure needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ext4Header {
    pub block_count: u64,
    pub free_blocks: u64,
    pub block_size: u64,
    /// Put away cleanly: its state is `clean`, and neither its journal nor
    /// its orphan file waits to be played back (`needs_recovery`,
    /// `orphan_present`).
    pub clean: bool,
}

impl Ext4Header {
    /// The header in `dumpe2fs -h`'s output, or `None` when a line it needs
    /// is missing or not a number.
    pub fn parse(text: &str) -> Option<Ext4Header> {
        let field = |name: &str| {
            text.lines().find_map(|line| {
                let (key, value) = line.split_once(':')?;
                (key.trim() == name).then(|| value.trim().to_string())
            })
        };
        let number = |name: &str| field(name)?.parse::<u64>().ok();
        let block_size = number("Block size").filter(|size| *size > 0)?;
        let block_count = number("Block count")?;
        let free_blocks = number("Free blocks").filter(|free| *free <= block_count)?;
        let state = field("Filesystem state")?;
        let features = field("Filesystem features")?;
        let pending = features
            .split_whitespace()
            .any(|feature| feature == "needs_recovery" || feature == "orphan_present");
        Some(Ext4Header {
            block_count,
            free_blocks,
            block_size,
            clean: state == "clean" && !pending,
        })
    }

    /// The filesystem's size in bytes.
    pub fn bytes(&self) -> u64 {
        self.block_count.saturating_mul(self.block_size)
    }

    /// The bytes its blocks in use take.
    pub fn used(&self) -> u64 {
        (self.block_count - self.free_blocks).saturating_mul(self.block_size)
    }

    /// The bytes its free blocks hold, those kept for root among them.
    pub fn free(&self) -> u64 {
        self.free_blocks.saturating_mul(self.block_size)
    }
}

/// What `resize2fs -P` said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Least {
    /// The least the filesystem could be shrunk to, in its own blocks.
    Blocks(u64),
    /// It asks for e2fsck first.
    NeedsCheck,
    Unknown,
}

pub fn ext4_least(ran: &Ran) -> Least {
    if ran.text().contains("Please run 'e2fsck") {
        return Least::NeedsCheck;
    }
    if !ran.success {
        return Least::Unknown;
    }
    ran.stdout
        .lines()
        .find_map(|line| line.strip_prefix("Estimated minimum size of the filesystem:"))
        .and_then(|blocks| blocks.trim().parse().ok())
        .map_or(Least::Unknown, Least::Blocks)
}

/// What `ntfsresize --info --no-progress-bar` said of an NTFS `size` bytes
/// long. A check that is due first, wherever it printed it and however it
/// ended: scheduled (Windows' own dirty flag), an inconsistent filesystem,
/// bad sectors — each of which it asks chkdsk to put right, and so names it.
/// Not by "bad sector" alone: every run says it is "Checking for bad
/// sectors". Then, only from a run that ended well, the bytes in use
/// (`Space in use`, in its millions of bytes, rounded up), the least size it
/// names (`You might resize at N bytes`) and the volume's own size, none of
/// them larger than the partition; what is free is the rest of the volume.
///
/// What it never says is that Windows is hibernated, or that its journal is
/// unclean: it prints its messages for those only for an error the checks
/// it skips would set (see [`signed`]), so they are not looked
/// for here.
pub fn ntfs_measure(ran: &Ran, size: u64) -> Measure {
    let text = ran.text();
    let due = ["scheduled for check", "is inconsistent", "chkdsk"];
    if due.iter().any(|words| text.contains(words)) {
        return Measure::NeedsCheck;
    }
    if !ran.success {
        return unreadable(ran.stderr.trim());
    }
    // The first number after a label, however far the label is padded.
    let first_number = |rest: &str| rest.split_whitespace().next()?.parse::<u64>().ok();
    let field = |label: &str| {
        ran.stdout.lines().find_map(|line| {
            let (name, rest) = line.split_once(':')?;
            (name.trim() == label).then(|| first_number(rest)).flatten()
        })
    };
    let volume = field("Current volume size");
    let used = field("Space in use").and_then(|mb| mb.checked_mul(1_000_000));
    let least = ran
        .stdout
        .lines()
        .find_map(|line| first_number(line.strip_prefix("You might resize at ")?));
    match (volume, used, least) {
        (Some(volume), Some(used), Some(least)) if volume <= size && least <= volume => {
            let used = used.min(volume);
            Measure::Read {
                used,
                free: volume - used,
                least: Some(least),
            }
        }
        _ => unreadable("ntfsresize said nothing readable"),
    }
}

/// An NTFS ntfsresize read cleanly (`read`), answered by the read-only sign
/// of hibernation: read, only if a `hiberfil.sys` of its own says Windows
/// is not hibernated; `Hibernated`, if it says it is; `NoHiberfile` — what
/// it holds known, but never offered to be shrunk — if it holds no
/// `hiberfil.sys` ([`Hibernation::Absent`]); `Unreadable` if what ntfscat
/// said is none of these, or if it could not be started. Whether one read
/// may be shrunk is then the rule set's to say, across every NTFS of the
/// answer (`Measures::shrink_of`).
///
/// ntfsresize cannot tell this itself: it opens every volume with
/// `NTFS_MNT_FORENSIC`, and libntfs-3g's `ntfs_device_mount` then skips both
/// its look at `hiberfil.sys` and its journal check, for `--info`,
/// `--no-action` and a real resize alike (read from the machine code of
/// ntfsprogs 2026.9.28, the 3.0 ISO's, and its libntfs-3g; §6, step Q3).
/// Shrinking a Windows that is hibernated, or was shut down by Fast
/// Startup, loses what it comes back to — so the sign is asked, read-only,
/// the one such read allowed, and an answer it cannot read is never taken
/// for "not hibernated".
///
/// Nor is a volume with no `hiberfil.sys`: libntfs-3g's rule calls it not
/// hibernated, but its read-write mount protects such a volume by its
/// journal check alone ([`Hibernation::Absent`]), and the journal is not
/// looked at here: libntfs-3g judges it by the more recent of two restart
/// pages, each checked through, and refuses a version-2.0 one, which no read
/// cut at the first restart page can say (§6, Unit Q, step Q8). An unclean
/// journal stays a known risk (§6).
fn signed<R: Reads + ?Sized>(machine: &mut R, device: &str, read: Measure) -> Measure {
    let head = match machine.head(&Command::ntfs_hibernation(device)) {
        Ok(head) => head,
        Err(stop) => return unreadable(&stop.detail),
    };
    match hibernation(&head) {
        Hibernation::NotHibernated => read,
        Hibernation::Hibernated => {
            run::log(&format!(
                "measure: {device}: Windows is hibernated there (hiberfil.sys), \
                 so it is not offered to be shrunk"
            ));
            Measure::Hibernated
        }
        Hibernation::Absent => {
            run::log(&format!(
                "measure: {device} holds no hiberfil.sys of its own, so nothing read here says \
                 whether a hibernated Windows keeps its metadata (only its journal could), and \
                 it is not offered to be shrunk"
            ));
            match read {
                Measure::Read { used, free, .. } => Measure::NoHiberfile { used, free },
                _ => Measure::Unreadable,
            }
        }
        Hibernation::Unreadable(why) => unreadable(&format!(
            "{device}: whether Windows is hibernated there could not be read: {why}"
        )),
    }
}

/// What the head of `hiberfil.sys` says ([`hibernation`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hibernation {
    /// A `hiberfil.sys` of its own, whose first word is not `hibr`.
    NotHibernated,
    Hibernated,
    /// No `hiberfil.sys` on the volume. libntfs-3g's rule calls that not
    /// hibernated, but only beside its journal check, which is not read
    /// here, and it is the only one of its checks that protects such a
    /// volume. A Windows keeps hiberfil.sys on its own system volume alone,
    /// so this is every NTFS that is not one — the data volumes (a "D:") of
    /// a Windows hibernated or shut down by Fast Startup (the default in
    /// Windows 10 and 11) among them — and the system volume of a Windows
    /// with hibernation switched off. In `libntfs-3g.so.91.0.0` the
    /// read-write mount (`ntfs_device_mount`) passes such a volume through
    /// `ntfs_volume_check_hiberfile` (0x50073; its ENOENT is "not
    /// hibernated") and refuses a Fast-Startup data volume only at
    /// `ntfs_check_logfile` (0x500d9) and the version-2.0 restart page
    /// after it (`cmpw $0x2,0x1c(%r10)`, 0x500f9, then 0x5093d: "Metadata
    /// kept in Windows cache, refused to mount."). Shrinking such a volume
    /// while Windows still keeps its metadata in memory would leave it
    /// corrupt once Windows resumes, so `signed` answers it `NoHiberfile`,
    /// never shrunk.
    Absent,
    /// Neither could be read from what ntfscat said: why.
    Unreadable(String),
}

/// What `ntfscat <partition> hiberfil.sys`, cut at [`HIBERFILE_HEAD`]
/// bytes, says by libntfs-3g's own rule (`ntfs_volume_check_hiberfile`,
/// read from its machine code; §6, step Q3):
///
/// - no `hiberfil.sys` (its lookup failing with ENOENT, which ntfscat says
///   as [`ABSENT`] — only when that is all it said, since it says so for
///   any failure of the lookup) is [`Hibernation::Absent`]: libntfs-3g
///   calls it not hibernated and leaves the rest to its journal check,
///   which is not read here;
/// - one shorter than [`HIBERFILE_HEAD`] bytes is hibernated — libntfs-3g's
///   "Hibernated non-system partition";
/// - one whose first word is `hibr` or `HIBR` is hibernated;
/// - any other first word is not.
///
/// Anything else — ntfscat refusing the volume, its lookup failing for
/// another reason (a line of its own before [`ABSENT`]), failing to read
/// the file, or stopped before it printed the whole header — is
/// `Unreadable`, never "not hibernated".
pub fn hibernation(head: &Head) -> Hibernation {
    if head.bytes.len() >= HIBERFILE_HEAD {
        return match &head.bytes[..4] {
            b"hibr" | b"HIBR" => Hibernation::Hibernated,
            _ => Hibernation::NotHibernated,
        };
    }
    let said = || head.stderr.trim().to_string();
    match head.ended {
        Some(true) => Hibernation::Hibernated,
        Some(false) if head.bytes.is_empty() && absent(&head.stderr) => Hibernation::Absent,
        Some(false) => Hibernation::Unreadable(format!("ntfscat failed: {}", said())),
        None => Hibernation::Unreadable(format!(
            "ntfscat was ended after {} bytes, short of the header",
            head.bytes.len()
        )),
    }
}

/// ntfscat's words when the volume has no such file: its lookup
/// (`ntfs_pathname_to_inode`) failing, said through `ntfs_log_perror`, with
/// the error's own text after it (the measure runs it in the C language).
///
/// They are wider than libntfs-3g's own "no such file": in
/// `libntfs-3g.so.91.0.0` (ntfsprogs 2026.9.28, the 3.0 ISO's),
/// `ntfs_pathname_to_inode` sets ENOENT for **every** failure of the lookup
/// it calls (`ntfs_inode_lookup_by_name`, 0x2721f: on -1 it takes `mov
/// $0x2,%ebp` at 0x27386 or 0x27580 and stores it in errno at 0x274b9), so
/// an index that could not be read or is corrupt is said in these words
/// too — after a line of the lookup's own ("Failed to read vcn …", "Index
/// root attribute missing in directory inode …", "Index block size … is
/// invalid", "Index root entry out of bounds …", "Failed to open index
/// allocation …"). libntfs-3g's own rule (`ntfs_volume_check_hiberfile`)
/// asks the lookup itself (0x4f20a) and takes only its ENOENT for no file
/// (`cmpl $0x2,(%rax)`, 0x4f39d). So these words count only when they are
/// all ntfscat said ([`hibernation`]).
pub const ABSENT: &str = "ERROR: Couldn't open inode: No such file or directory";

/// Whether ntfscat said that there is no such file and nothing else:
/// [`ABSENT`] the one line on its standard error that is not blank. Any
/// other line beside it is the lookup failing for another reason, which
/// ntfscat says in the same words ([`ABSENT`]'s note).
fn absent(stderr: &str) -> bool {
    let mut said = stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    said.next() == Some(ABSENT) && said.next().is_none()
}

/// The bytes `du -sb` printed for `path`: one line, the number and the path.
pub fn du_bytes(stdout: &str, path: &str) -> Option<u64> {
    let mut lines = stdout.lines().filter(|line| !line.trim().is_empty());
    let line = lines.next()?;
    if lines.next().is_some() {
        return None;
    }
    let (bytes, named) = line.split_once('\t')?;
    (named == path).then(|| bytes.trim().parse().ok()).flatten()
}

/// The size of each of `IMAGE_PATHS` in the image. A path the image has
/// nothing at takes nothing; one that is not a directory, or that `du` did
/// not answer for, is left out — unknown, so never guessed.
fn image_sizes(machine: &mut dyn Machine) -> ImageSizes {
    let mut paths = BTreeMap::new();
    for place in IMAGE_PATHS {
        let path = format!("{DEFAULT_SOURCE}{place}");
        let size = match machine.entry(&path) {
            Entry::Missing => Some(0),
            Entry::Other => None,
            Entry::Directory => match machine.run(&Command::image_size(&path)) {
                Ok(ran) if ran.success => du_bytes(&ran.stdout, &path),
                Ok(ran) => {
                    run::log(ran.stderr.trim());
                    None
                }
                Err(stop) => {
                    run::log(&stop.detail);
                    None
                }
            },
        };
        match size {
            Some(size) => {
                paths.insert(place.to_string(), size);
            }
            None => run::log(&format!("measure: the size of {path} is not known")),
        }
    }
    ImageSizes { paths }
}

/// Reads the one request from `reader`, at most `MAX_REQUEST` bytes. It
/// holds no secret, but an error says only where and what kind of mistake,
/// as the worker's own does. `reader` should be unbuffered.
pub fn read_request(reader: impl Read) -> Result<MeasureRequest, Stop> {
    let invalid = |detail: String| Stop::new(Failure::InvalidRequest, detail);
    let mut bytes = Vec::new();
    reader
        .take(MAX_REQUEST + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| invalid(format!("the request could not be read: {}", error.kind())))?;
    if bytes.len() as u64 > MAX_REQUEST {
        return Err(invalid(
            "the request is larger than the measure accepts".into(),
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        invalid(format!(
            "the request is not a measurement ({:?} at line {}, column {})",
            error.classify(),
            error.line(),
            error.column()
        ))
    })
}

/// Starts one command, once [`allowed`] has let it through, and says what it
/// printed. Started here rather than through `run`, which keeps nothing of
/// what a failing command printed on its standard output — and NTFS's
/// refusals are what is read — so in a test build it asks the fence itself,
/// first, before the program is even looked up (as discovery's lsblk does,
/// see `run::fence`). The environment is cleared and the language is C, so
/// what is read is what the parsers were written for.
fn start(command: &Command) -> Result<Ran, Stop> {
    if !allowed(command) {
        return Err(Stop::new(
            Failure::Internal,
            format!("{} is not a command the measure runs", command.line()),
        ));
    }
    if only_its_head(command) {
        return Err(Stop::new(
            Failure::Internal,
            format!(
                "{}: only the head of what it prints is read",
                command.line()
            ),
        ));
    }
    let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
    #[cfg(test)]
    run::fence::check(command.program, &args);
    run::log(&command.line());
    let program = run::command_path(command.program)?;
    let output = std::process::Command::new(program)
        .args(&args)
        .env_clear()
        .env("PATH", run::SAFE_PATH)
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            Stop::new(
                Failure::CommandFailed,
                format!("{}: could not start: {e}", command.program),
            )
        })?;
    let stderr: String = String::from_utf8_lossy(&output.stderr)
        .chars()
        .take(4000)
        .collect();
    if !stderr.trim().is_empty() {
        run::log(stderr.trim());
    }
    Ok(Ran {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr,
    })
}

/// Starts one command whose output is cut short ([`only_its_head`]), once
/// [`allowed`] has let it through, and reads at most [`HIBERFILE_HEAD`]
/// bytes of what it prints before ending it ([`head_of`]).
fn start_head(command: &Command) -> Result<Head, Stop> {
    if !allowed(command) || !only_its_head(command) {
        return Err(Stop::new(
            Failure::Internal,
            format!(
                "{} is not a command the measure reads the head of",
                command.line()
            ),
        ));
    }
    let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
    head_of(command.program, &args)
}

/// Starts `program args` as [`start`] starts a command — in a test build
/// asking the fence first, before the program is looked up; the environment
/// cleared, the language C, nothing on standard input — and reads at most
/// [`HIBERFILE_HEAD`] bytes of its standard output. Once it has them, the
/// program is ended (killed: it may have gigabytes more to print, which
/// nobody reads) and waited for; one that ends sooner is waited for as it
/// is. Its standard error is read beside it to the end, so a program that
/// says much there never waits on a full pipe, and kept as the log keeps it.
pub(in crate::worker) fn head_of(program: &str, args: &[&str]) -> Result<Head, Stop> {
    head_within(program, args, None)
}

/// How often a bounded head looks whether its reading is done
/// ([`head_within`]).
const POLL: std::time::Duration = std::time::Duration::from_millis(20);

/// [`head_of`], and where `limit` says so, **bounded in time** (phase A's
/// Windows reads, `manual::shrinking::WINDOWS_PATIENCE`): the head is read
/// beside, and a program that has not given it, nor ended, once it has run
/// `limit` is ended (killed, and waited for) and answered with an error,
/// `CommandFailed`, which says so. Whether it has run out of time is asked
/// before whether the head is read, so a `limit` of nothing ends it at
/// once. Without one (the measure entry's own reading, which the interface
/// lets go of after a minute), the head is read as [`head_of`] always has.
pub(in crate::worker) fn head_within(
    program: &str,
    args: &[&str],
    limit: Option<std::time::Duration>,
) -> Result<Head, Stop> {
    #[cfg(test)]
    run::fence::check(program, args);
    let mut shown = format!("$ {program}");
    for arg in args {
        shown.push(' ');
        shown.push_str(arg);
    }
    run::log(&shown);
    let path = run::command_path(program)?;
    let failed = |what: &str, error: std::io::Error| {
        Stop::new(
            Failure::CommandFailed,
            format!("{program}: could not {what}: {error}"),
        )
    };
    let mut child = std::process::Command::new(path)
        .args(args)
        .env_clear()
        .env("PATH", run::SAFE_PATH)
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| failed("start", e))?;
    let mut errors = child.stderr.take().expect("stderr is piped");
    let reader = std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            match errors.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) if kept.len() < 16 * 1024 => kept.extend_from_slice(&chunk[..n]),
                Ok(_) => {}
            }
        }
        kept
    });
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let read_head = |stdout: &mut std::process::ChildStdout| {
        let mut bytes = Vec::with_capacity(HIBERFILE_HEAD);
        let read = stdout.take(HIBERFILE_HEAD as u64).read_to_end(&mut bytes);
        (bytes, read)
    };
    let (bytes, read, stdout) = match limit {
        None => {
            let (bytes, read) = read_head(&mut stdout);
            (bytes, read, stdout)
        }
        Some(limit) => {
            let deadline = std::time::Instant::now() + limit;
            let (done, told) = std::sync::mpsc::channel();
            let reading = std::thread::spawn(move || {
                let (bytes, read) = read_head(&mut stdout);
                let _ = done.send(());
                (bytes, read, stdout)
            });
            let in_time = loop {
                if std::time::Instant::now() >= deadline {
                    break false;
                }
                match told.try_recv() {
                    Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break true,
                    Err(std::sync::mpsc::TryRecvError::Empty) => std::thread::sleep(POLL),
                }
            };
            if !in_time {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reading.join();
                let stderr: String = String::from_utf8_lossy(&reader.join().unwrap_or_default())
                    .chars()
                    .take(4000)
                    .collect();
                if !stderr.trim().is_empty() {
                    run::log(stderr.trim());
                }
                return Err(Stop::new(
                    Failure::CommandFailed,
                    format!(
                        "{program}: did not give its head within {} seconds, and was ended",
                        limit.as_secs()
                    ),
                ));
            }
            reading.join().map_err(|_| {
                Stop::new(Failure::Internal, format!("{program}: its reader failed"))
            })?
        }
    };
    let cut = bytes.len() >= HIBERFILE_HEAD;
    if cut || read.is_err() {
        let _ = child.kill();
    }
    drop(stdout);
    let status = child.wait().map_err(|e| failed("wait", e));
    let stderr: String = String::from_utf8_lossy(&reader.join().unwrap_or_default())
        .chars()
        .take(4000)
        .collect();
    if !stderr.trim().is_empty() {
        run::log(stderr.trim());
    }
    read.map_err(|e| failed("read what it printed", e))?;
    let status = status?;
    let ended = match status.code() {
        Some(_) => Some(status.success()),
        None if cut => None,
        None => Some(false),
    };
    Ok(Head {
        bytes,
        ended,
        stderr,
    })
}

/// A new mount point below [`SCRATCH`]'s directory, made by `mkdtemp`: its
/// name cannot be guessed, nothing else can have made it first, and only
/// root can enter it.
fn scratch_dir() -> Result<PathBuf, Stop> {
    let mut template = format!("{SCRATCH}XXXXXX\0").into_bytes();
    // SAFETY: the template is a writable, NUL-terminated buffer that mkdtemp
    // fills in place and does not keep.
    let made = unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) };
    if made.is_null() {
        return Err(Stop::new(
            Failure::Internal,
            format!(
                "cannot make a mount point in {}: {}",
                report::DIRECTORY,
                std::io::Error::last_os_error()
            ),
        ));
    }
    template.pop();
    Ok(PathBuf::from(OsString::from_vec(template)))
}

/// The machine's own answers.
#[derive(Default)]
pub struct Real {
    /// The lock, from [`Machine::lock`] until the process ends.
    lock: Option<File>,
    /// `/proc/1/mountinfo` and `/proc/swaps`, read once for every
    /// partition's busy check.
    mounts: Option<(String, String)>,
}

impl Reads for Real {
    fn run(&mut self, command: &Command) -> Result<Ran, Stop> {
        start(command)
    }

    fn head(&mut self, command: &Command) -> Result<Head, Stop> {
        start_head(command)
    }
}

impl Machine for Real {
    fn live(&mut self) -> Option<LiveSession> {
        live::detect()
    }

    fn harden(&mut self) {
        check::harden();
    }

    fn prepare_directory(&mut self) -> Result<(), String> {
        report::prepare_directory()
    }

    fn lock(&mut self) -> Result<(), Stop> {
        self.lock = Some(check::lock()?);
        Ok(())
    }

    fn request(&mut self) -> Result<MeasureRequest, Stop> {
        // SAFETY: descriptor 0 is the process's standard input; ManuallyDrop
        // keeps the File from closing it. Read unbuffered, as the worker
        // reads its own.
        let stdin = std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(0) });
        read_request(&*stdin)
    }

    fn discover(&mut self) -> Result<Vec<Disk>, String> {
        crate::model::disks::discover()
    }

    fn busy(&mut self, device: &str) -> Result<Option<String>, String> {
        if self.mounts.is_none() {
            let mountinfo = std::fs::read_to_string("/proc/1/mountinfo")
                .map_err(|e| format!("cannot read the live system's mounts: {e}"))?;
            let swaps = std::fs::read_to_string("/proc/swaps")
                .map_err(|e| format!("cannot read the live system's swap: {e}"))?;
            self.mounts = Some((mountinfo, swaps));
        }
        let Some((mountinfo, swaps)) = &self.mounts else {
            return Err("the live system's mounts were not read".into());
        };
        let (major_minor, held) = check::device_facts(device)?;
        Ok(check::device_busy(
            device,
            &major_minor,
            held,
            mountinfo,
            swaps,
        ))
    }

    fn isolate_mounts(&mut self) -> Result<(), Stop> {
        check::isolate_mounts()
    }

    fn esp_free(&mut self, device: &str) -> Result<u64, Stop> {
        let dir = scratch_dir()?;
        let target = run::utf8(&dir)?.to_string();
        let mount = Command::new("mount", &esp::mount_arguments(device, &target));
        let unmount = Command::new("umount", &[&target]);
        let measured = if allowed(&mount) && allowed(&unmount) {
            esp::measure(device, &dir)
        } else {
            Err(Stop::new(
                Failure::Internal,
                format!("{} is not a mount the measure makes", mount.line()),
            ))
        };
        if let Err(error) = std::fs::remove_dir(&dir) {
            run::log(&format!("measure: {target} was not removed: {error}"));
        }
        measured
    }

    fn entry(&mut self, path: &str) -> Entry {
        match std::fs::symlink_metadata(Path::new(path)) {
            Ok(meta) if meta.is_dir() => Entry::Directory,
            Ok(_) => Entry::Other,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Entry::Missing,
            Err(_) => Entry::Other,
        }
    }

    fn answer(&mut self, line: &str) {
        let mut stdout = std::io::stdout().lock();
        let _ = writeln!(stdout, "{line}");
        let _ = stdout.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::{Blocked, Table, GIB, MIB};
    use crate::model::layout::Unshrinkable;
    use crate::worker::partition::fixtures::{
        build, Spec, GPT_BASIC_DATA, GPT_ESP, GPT_LINUX, GPT_WINRE,
    };
    use crate::worker::run::fence;

    // Every tool's output below is written by hand in the shape the tool
    // prints it (e2fsprogs 1.47, ntfsprogs 2026.9): no test here starts one.

    const DUMPE2FS_CLEAN: &str = "\
Filesystem volume name:   Data
Last mounted on:          /mnt/data
Filesystem UUID:          5b7a3c1e-2f4d-4e8a-9c6b-1d2e3f4a5b6c
Filesystem magic number:  0xEF53
Filesystem revision #:    1 (dynamic)
Filesystem features:      has_journal ext_attr resize_inode dir_index orphan_file filetype extent 64bit flex_bg metadata_csum_seed sparse_super large_file huge_file dir_nlink extra_isize metadata_csum
Filesystem flags:         signed_directory_hash 
Default mount options:    user_xattr acl
Filesystem state:         clean
Errors behavior:          Continue
Filesystem OS type:       Linux
Inode count:              1572864
Block count:              6291456
Reserved block count:     314572
Overhead clusters:        142250
Free blocks:              3407872
Free inodes:              1572001
First block:              0
Block size:               4096
Fragment size:            4096
Group descriptor size:    64
Reserved GDT blocks:      1024
Blocks per group:         32768
Filesystem created:       Mon Oct 12 10:41:07 2026
Last mount time:          Mon Oct 12 11:02:15 2026
Last write time:          Mon Oct 12 11:02:40 2026
Mount count:              3
Maximum mount count:      -1
Journal backup:           inode blocks
Checksum type:            crc32c
Journal features:         journal_incompat_revoke journal_64bit journal_checksum_v3
Total journal size:       128M
Total journal blocks:     32768
Journal sequence:         0x00000012
Journal start:            0
";

    /// The same filesystem pulled out without being unmounted: the state
    /// still says clean, but the journal waits to be played back.
    fn dumpe2fs_needs_recovery() -> String {
        DUMPE2FS_CLEAN
            .replace(" filetype extent", " filetype needs_recovery extent")
            .replace("Journal start:            0", "Journal start:            1")
    }

    fn ran(success: bool, stdout: &str, stderr: &str) -> Ran {
        Ran {
            success,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    const RESIZE2FS_BANNER: &str = "resize2fs 1.47.4 (6-Mar-2025)\n";

    const NTFSRESIZE_CLEAN: &str = "\
ntfsresize v2026.9.28 (libntfs-3g)
Device name        : /dev/vdx3
NTFS volume version: 3.1
Cluster size       : 4096 bytes
Current volume size: 32212254208 bytes (32213 MB)
Current device size: 32212254720 bytes (32213 MB)
Checking for bad sectors ...
Checking filesystem consistency ...
Accounting clusters ...
Space in use       : 13254 MB (41.1%)
Collecting resizing constraints ...
You might resize at 13253541888 bytes or 13254 MB (freeing 18959 MB).
Please make a test run using both the -n and -s options before real resizing!
";

    // ntfsresize opens a volume "forensic", so libntfs-3g never looks at
    // hiberfil.sys or the journal for it: no output below is a hibernated
    // Windows's, because ntfsresize prints none (see `signed`).

    /// The head ntfscat prints of a `hiberfil.sys` whose first word is
    /// `first` — the whole header, so the program was ended after it.
    fn hiberfile(first: &[u8]) -> Head {
        let mut bytes = first.to_vec();
        bytes.resize(HIBERFILE_HEAD, 0);
        Head {
            bytes,
            ended: None,
            stderr: String::new(),
        }
    }

    /// What ntfscat says of a volume with no `hiberfil.sys`.
    fn no_hiberfile() -> Head {
        Head {
            bytes: Vec::new(),
            ended: Some(false),
            stderr: format!("{ABSENT}\n"),
        }
    }

    fn ntfsresize_check_scheduled() -> Ran {
        ran(
            false,
            "ntfsresize v2026.9.28 (libntfs-3g)\n\
             Device name        : /dev/vdx4\n\
             NTFS volume version: 3.1\n",
            "ERROR: Volume is scheduled for check.\n\
             Run chkdsk /f and please try again, or see option -f.\n",
        )
    }

    // ----- The parsers -----

    #[test]
    fn dumpe2fs_header_is_read_clean_and_needing_recovery() {
        let clean = Ext4Header::parse(DUMPE2FS_CLEAN).expect("read");
        assert_eq!(
            clean,
            Ext4Header {
                block_count: 6_291_456,
                free_blocks: 3_407_872,
                block_size: 4096,
                clean: true,
            }
        );
        assert_eq!(clean.bytes(), 24 * GIB);
        assert_eq!(clean.used(), 11 * GIB);
        let recovering = Ext4Header::parse(&dumpe2fs_needs_recovery()).expect("read");
        assert!(!recovering.clean);
        assert_eq!(recovering.used(), clean.used());
        let not_clean = DUMPE2FS_CLEAN.replace(
            "Filesystem state:         clean",
            "Filesystem state:         not clean",
        );
        assert!(!Ext4Header::parse(&not_clean).unwrap().clean);
        let errors = DUMPE2FS_CLEAN.replace(
            "Filesystem state:         clean",
            "Filesystem state:         clean with errors",
        );
        assert!(!Ext4Header::parse(&errors).unwrap().clean);
        let orphans = DUMPE2FS_CLEAN.replace(" orphan_file ", " orphan_file orphan_present ");
        assert!(!Ext4Header::parse(&orphans).unwrap().clean);
        // A header missing a number it needs, or with more free than there
        // is, is not read at all.
        let no_size = DUMPE2FS_CLEAN.replace("Block size:               4096\n", "");
        assert_eq!(Ext4Header::parse(&no_size), None);
        let too_free = DUMPE2FS_CLEAN.replace("3407872", "6291457");
        assert_eq!(Ext4Header::parse(&too_free), None);
        assert_eq!(
            Ext4Header::parse("dumpe2fs: Bad magic number in super-block"),
            None
        );
    }

    #[test]
    fn resize2fs_estimate_is_read() {
        let estimate = ran(
            true,
            "Estimated minimum size of the filesystem: 2961408\n",
            RESIZE2FS_BANNER,
        );
        assert_eq!(ext4_least(&estimate), Least::Blocks(2_961_408));
        let check_first = ran(
            false,
            "",
            &format!("{RESIZE2FS_BANNER}Please run 'e2fsck -f /dev/vdx2' first.\n\n"),
        );
        assert_eq!(ext4_least(&check_first), Least::NeedsCheck);
        let failed = ran(false, "", "resize2fs: Bad magic number in super-block\n");
        assert_eq!(ext4_least(&failed), Least::Unknown);
        assert_eq!(ext4_least(&ran(true, "", RESIZE2FS_BANNER)), Least::Unknown);
    }

    #[test]
    fn ntfsresize_info_is_read_clean_and_check_due() {
        let size = 30 * GIB;
        assert_eq!(
            ntfs_measure(&ran(true, NTFSRESIZE_CLEAN, ""), size),
            Measure::Read {
                used: 13_254_000_000,
                free: 32_212_254_208 - 13_254_000_000,
                least: Some(13_253_541_888),
            }
        );
        assert_eq!(
            ntfs_measure(&ntfsresize_check_scheduled(), size),
            Measure::NeedsCheck
        );
        let inconsistent = ran(
            false,
            "Checking for bad sectors ...\n\
             Checking filesystem consistency ...\n\
             NTFS is inconsistent. Run chkdsk /f on Windows then reboot it TWICE!\n\
             The usage of the /f parameter is very IMPORTANT! No modification was\n\
             and will be made to NTFS by this software until it gets repaired.\n",
            "",
        );
        assert_eq!(ntfs_measure(&inconsistent, size), Measure::NeedsCheck);
        let bad_sectors = ran(
            false,
            "Checking for bad sectors ...\n\
             This software has detected that the disk has at least 3 bad sectors.\n\
             * WARNING: The disk has bad sector. This means physical damage on the disk *\n\
             * run 'chkdsk /f /r' on Windows and rebooot it TWICE! Then you can resize  *\n",
            "",
        );
        assert_eq!(ntfs_measure(&bad_sectors, size), Measure::NeedsCheck);
        // A partition smaller than the volume its tool reads is not believed;
        // nor a run that ended badly with nothing it recognises.
        assert_eq!(
            ntfs_measure(&ran(true, NTFSRESIZE_CLEAN, ""), 10 * GIB),
            Measure::Unreadable
        );
        let invalid = ran(false, "", "ERROR(22): Opening '/dev/vdx3' as NTFS failed\n");
        assert_eq!(ntfs_measure(&invalid, size), Measure::Unreadable);
        let cut_short = NTFSRESIZE_CLEAN.replace("You might resize at", "You might");
        assert_eq!(
            ntfs_measure(&ran(true, &cut_short, ""), size),
            Measure::Unreadable
        );
    }

    /// What the head of `hiberfil.sys` says, by libntfs-3g's own rule, on
    /// bytes and words written by hand: a first word other than hibr is not
    /// hibernated; hibr or HIBR, or a file shorter than its header, is;
    /// none there is absent — libntfs-3g's "not hibernated", which leaves
    /// the rest to a journal check not read here; and anything else ntfscat
    /// says — the volume refused, the file unread, its lookup failing for
    /// another reason (which ntfscat says in [`ABSENT`]'s words after a
    /// line of the lookup's own), stopped short — is unreadable, never "not
    /// hibernated".
    #[test]
    fn hibernation_is_read_by_libntfs_3gs_own_rule() {
        use Hibernation::*;
        assert_eq!(hibernation(&no_hiberfile()), Absent);
        // Its words alone, blank lines around them or not.
        let mut alone = no_hiberfile();
        alone.stderr = format!("\n  {ABSENT}  \n\n");
        assert_eq!(hibernation(&alone), Absent);
        for first in [&b"hibr"[..], b"HIBR"] {
            assert_eq!(hibernation(&hiberfile(first)), Hibernated, "{first:?}");
        }
        for first in [
            &b"wake"[..],
            b"\0\0\0\0",
            b"RSTR",
            b"Hibr",
            b"hib",
            b"xhibr",
            b"HIBERFIL",
        ] {
            assert_eq!(hibernation(&hiberfile(first)), NotHibernated, "{first:?}");
        }
        // The whole header read, however the program ended after it.
        for ended in [None, Some(true), Some(false)] {
            let mut head = hiberfile(b"HIBR");
            head.ended = ended;
            assert_eq!(hibernation(&head), Hibernated);
            head.bytes[..4].copy_from_slice(b"wake");
            assert_eq!(hibernation(&head), NotHibernated);
        }
        // Shorter than its header, ended well: a non-system partition's.
        for length in [0, 1, 4, 512, HIBERFILE_HEAD - 1] {
            let head = Head {
                bytes: vec![0; length],
                ended: Some(true),
                stderr: String::new(),
            };
            assert_eq!(hibernation(&head), Hibernated, "{length} bytes");
        }
        // Everything else ntfscat can say is unreadable.
        let failed = |bytes: Vec<u8>, stderr: &str| Head {
            bytes,
            ended: Some(false),
            stderr: stderr.into(),
        };
        for head in [
            // A volume marked for a check, which ntfscat refuses without -f.
            failed(
                Vec::new(),
                "Volume is dirty.\nRun chkdsk and try again, or use the --force option.\n",
            ),
            failed(
                Vec::new(),
                "Failed to mount '/dev/vdx3': Invalid argument\n",
            ),
            failed(
                Vec::new(),
                "ERROR: Couldn't open inode: Input/output error\n",
            ),
            failed(Vec::new(), "ERROR: Couldn't open inode\n"),
            failed(Vec::new(), ""),
            // Absent, said on a line with more on it, is not its words.
            failed(Vec::new(), &format!("x {ABSENT}\n")),
            // ntfscat says ABSENT for every failure of its lookup
            // (`ntfs_pathname_to_inode` sets ENOENT for each): an index
            // that could not be read, or is corrupt, after the lookup's own
            // line — libntfs-3g's own rule takes neither for no file.
            failed(
                Vec::new(),
                &format!("Failed to read vcn 0x0 from inode 5: Input/output error\n{ABSENT}\n"),
            ),
            failed(
                Vec::new(),
                &format!(
                    "Index root attribute missing in directory inode 5: \
                     No such file or directory\n{ABSENT}\n"
                ),
            ),
            failed(
                Vec::new(),
                &format!("Index block size 0 is invalid.\n{ABSENT}\n"),
            ),
            // ... or with anything said after it.
            failed(Vec::new(), &format!("{ABSENT}\nsomething more\n")),
            failed(Vec::new(), &format!("{ABSENT}\n{ABSENT}\n")),
            // Part of the file printed, then a failure: not its length.
            failed(vec![0; 100], &format!("{ABSENT}\n")),
            failed(b"HIBR".to_vec(), "ERROR: Couldn't read file\n"),
            // Stopped before the whole header, which the starter never does.
            Head {
                bytes: vec![0; 100],
                ended: None,
                stderr: String::new(),
            },
        ] {
            assert!(
                matches!(hibernation(&head), Unreadable(_)),
                "{head:?}: {:?}",
                hibernation(&head)
            );
        }
    }

    /// An NTFS ntfsresize reads cleanly is read only once the read-only
    /// sign of hibernation says Windows is not hibernated there, by a
    /// `hiberfil.sys` of its own: hibernated it is `Hibernated`; with no
    /// `hiberfil.sys` it is `NoHiberfile`, its numbers kept but never
    /// shrunk; unreadable or unasked it is `Unreadable`; and the sign is
    /// asked of nothing ntfsresize did not read cleanly (a check due is
    /// still answered as one, unsigned). Alone, and through a whole run,
    /// where ntfsresize and then ntfscat are the only commands run on it.
    /// Whether one read may be shrunk is the rule set's to say, across every
    /// Windows volume of the answer (`Measures::shrink_of`): here a check due
    /// on vdx4 holds vdx3 back.
    #[test]
    fn an_ntfs_read_cleanly_is_offered_to_shrink_only_once_signed_not_hibernated() {
        let read = ntfs_measure(&ran(true, NTFSRESIZE_CLEAN, ""), 30 * GIB);
        assert!(matches!(read, Measure::Read { .. }), "{read:?}");
        let sign = Command::ntfs_hibernation("/dev/vdx3").line();
        let signed_by = |head: Option<Head>| {
            let mut fake = Fake::default();
            if let Some(head) = head {
                fake.heads.insert(sign.clone(), head);
            }
            let measure = signed(&mut fake, "/dev/vdx3", read);
            assert_eq!(fake.asked, std::slice::from_ref(&sign));
            measure
        };
        let Measure::Read { used, free, .. } = read else {
            unreachable!()
        };
        let no_file = Measure::NoHiberfile { used, free };
        assert_eq!(signed_by(Some(hiberfile(b"wake"))), read);
        assert_eq!(signed_by(Some(no_hiberfile())), no_file);
        assert_eq!(signed_by(Some(hiberfile(b"HIBR"))), Measure::Hibernated);
        assert_eq!(signed_by(Some(hiberfile(b"hibr"))), Measure::Hibernated);
        let refused = Head {
            bytes: Vec::new(),
            ended: Some(false),
            stderr: "Volume is dirty.\n".into(),
        };
        assert_eq!(signed_by(Some(refused)), Measure::Unreadable);
        // ntfscat could not be started at all.
        assert_eq!(signed_by(None), Measure::Unreadable);

        // Through a whole run: vdx3 signed not hibernated is read, vdx4's
        // check due answered unsigned.
        let ntfs_of = |fake: &mut Fake| -> Vec<(String, Measure)> {
            assert_eq!(measure(|| true, || true, fake), 0);
            let MeasureAnswer::Measured { measures, .. } = answer_of(fake) else {
                panic!("refused: {:?}", fake.answers);
            };
            let listed = every_kind();
            measures
                .parts
                .iter()
                .filter(|one| {
                    listed
                        .partition(&one.path)
                        .is_some_and(|p| p.fs_type == "ntfs")
                })
                .map(|one| (one.path.clone(), one.measure))
                .collect()
        };
        let mut fake = every_kind_machine();
        assert_eq!(
            ntfs_of(&mut fake),
            [
                ("/dev/vdx3".to_string(), read),
                ("/dev/vdx4".to_string(), Measure::NeedsCheck)
            ]
        );
        let on = |fake: &Fake, device: &str| -> Vec<String> {
            fake.asked
                .iter()
                .filter(|asked| asked.contains(&format!(" {device}")))
                .cloned()
                .collect()
        };
        assert_eq!(
            on(&fake, "/dev/vdx3"),
            [
                "busy /dev/vdx3",
                "$ ntfsresize --info --no-progress-bar /dev/vdx3",
                "$ ntfscat /dev/vdx3 hiberfil.sys"
            ]
        );
        assert_eq!(
            on(&fake, "/dev/vdx4"),
            [
                "busy /dev/vdx4",
                "$ ntfsresize --info --no-progress-bar /dev/vdx4"
            ]
        );
        // Read and signed, vdx3 is still held back by the rule set: vdx4,
        // another NTFS, asks for Windows' check, so whether a Windows keeps
        // vdx3 is not known.
        let MeasureAnswer::Measured { measures, .. } = answer_of(&fake) else {
            unreachable!()
        };
        let vdx3 = every_kind().partitions[2].clone();
        assert_eq!(
            measures.shrink_of(&vdx3),
            Some(Err(Unshrinkable::Unread {
                other: "/dev/vdx4".into()
            }))
        );
        // Signed hibernated, holding no hiberfil.sys, or not signed at all.
        for (head, answered) in [
            (Some(hiberfile(b"HIBR")), Measure::Hibernated),
            (Some(no_hiberfile()), no_file),
            (None, Measure::Unreadable),
        ] {
            let mut fake = every_kind_machine();
            match head {
                Some(head) => fake.heads.insert(sign.clone(), head),
                None => fake.heads.remove(&sign),
            };
            assert_eq!(ntfs_of(&mut fake)[0], ("/dev/vdx3".to_string(), answered));
        }
    }

    /// A Windows' data volume (a "D:") holds no `hiberfil.sys` — Windows
    /// keeps it on its system volume alone — so the sign says nothing of
    /// whether a Windows hibernated, or shut down by Fast Startup (the
    /// default in Windows 10 and 11), still keeps its metadata in memory:
    /// libntfs-3g refuses such a volume only by its journal check, which is
    /// not read. It is never offered to be shrunk, whatever the system
    /// volume beside it says: `NoHiberfile` — what it holds known — beside a
    /// Windows signed not hibernated, and beside one signed hibernated,
    /// which is itself `Hibernated`. Its sign is still asked, after
    /// ntfsresize read it cleanly, and nothing else is run on it. And it
    /// holds no other back: the system volume, read and signed, is held back
    /// by the Windows of the disk replugged since it was listed (vdy3, not
    /// read), not by it.
    #[test]
    fn a_windows_data_volume_with_no_hiberfil_sys_is_never_offered_to_shrink() {
        // vdx4, 2 GiB, read cleanly this time: Windows' "D:".
        let data_volume = "\
ntfsresize v2026.9.28 (libntfs-3g)
Device name        : /dev/vdx4
NTFS volume version: 3.1
Cluster size       : 4096 bytes
Current volume size: 2147483136 bytes (2148 MB)
Current device size: 2147483648 bytes (2148 MB)
Checking for bad sectors ...
Checking filesystem consistency ...
Accounting clusters ...
Space in use       : 512 MB (23.8%)
Collecting resizing constraints ...
You might resize at 511311872 bytes or 512 MB (freeing 1636 MB).
Please make a test run using both the -n and -s options before real resizing!
";
        let read_alone = ntfs_measure(&ran(true, data_volume, ""), 2 * GIB);
        assert!(matches!(read_alone, Measure::Read { .. }), "{read_alone:?}");
        let system = Command::ntfs_hibernation("/dev/vdx3").line();
        let data = Command::ntfs_hibernation("/dev/vdx4").line();
        for (system_sign, system_answer) in [
            (hiberfile(b"wake"), None),
            (hiberfile(b"HIBR"), Some(Measure::Hibernated)),
        ] {
            let mut fake = every_kind_machine();
            fake.printed.insert(
                Command::ntfs_info("/dev/vdx4").line(),
                ran(true, data_volume, ""),
            );
            fake.heads.insert(system.clone(), system_sign);
            fake.heads.insert(data.clone(), no_hiberfile());
            assert_eq!(measure(|| true, || true, &mut fake), 0);
            let MeasureAnswer::Measured { measures, .. } = answer_of(&fake) else {
                panic!("refused: {:?}", fake.answers);
            };
            let of = |path: &str| {
                measures
                    .parts
                    .iter()
                    .find(|one| one.path == path)
                    .map(|one| one.measure)
                    .unwrap()
            };
            assert_eq!(
                of("/dev/vdx4"),
                Measure::NoHiberfile {
                    used: 512_000_000,
                    free: 2_147_483_136 - 512_000_000,
                }
            );
            let partition = |path: &str| every_kind().partition(path).unwrap().clone();
            assert_eq!(
                measures.shrink_of(&partition("/dev/vdx4")),
                Some(Err(Unshrinkable::NoHiberfile))
            );
            let system_judged = measures.shrink_of(&partition("/dev/vdx3"));
            match system_answer {
                Some(answer) => {
                    assert_eq!(of("/dev/vdx3"), answer);
                    assert_eq!(system_judged, Some(Err(Unshrinkable::Itself(answer))));
                }
                None => {
                    assert!(matches!(of("/dev/vdx3"), Measure::Read { .. }));
                    assert_eq!(
                        system_judged,
                        Some(Err(Unshrinkable::Unread {
                            other: "/dev/vdy3".into()
                        }))
                    );
                }
            }
            let on_data: Vec<&String> = fake
                .asked
                .iter()
                .filter(|asked| asked.contains(" /dev/vdx4"))
                .collect();
            assert_eq!(
                on_data,
                [
                    "busy /dev/vdx4",
                    "$ ntfsresize --info --no-progress-bar /dev/vdx4",
                    "$ ntfscat /dev/vdx4 hiberfil.sys"
                ]
            );
        }
    }

    #[test]
    fn du_is_read_for_its_own_path_only() {
        let var = "/run/archiso/airootfs/var";
        assert_eq!(
            du_bytes("1342177280\t/run/archiso/airootfs/var\n", var),
            Some(1_342_177_280)
        );
        assert_eq!(
            du_bytes("1342177280\t/run/archiso/airootfs/opt\n", var),
            None
        );
        assert_eq!(
            du_bytes("12\t/a\n34\t/run/archiso/airootfs/var\n", var),
            None
        );
        assert_eq!(du_bytes("", var), None);
        assert_eq!(du_bytes("many\t/run/archiso/airootfs/var\n", var), None);
    }

    // ----- The commands -----

    /// A command line built by hand, not by one of `Command`'s own builders:
    /// what [`allowed`] must refuse.
    fn by_hand(program: &'static str, args: &[&str]) -> Command {
        Command {
            program,
            args: args.iter().map(|arg| arg.to_string()).collect(),
        }
    }

    #[test]
    fn measure_commands_are_read_only() {
        let device = "/dev/nvme0n1p3";
        let words = |command: Command| -> Vec<String> {
            assert!(allowed(&command), "{}", command.line());
            std::iter::once(command.program.to_string())
                .chain(command.args)
                .collect()
        };
        // Each builder's command line, word for word.
        assert_eq!(
            words(Command::ext4_header(device)),
            ["dumpe2fs", "-h", device]
        );
        assert_eq!(
            words(Command::ext4_least(device)),
            ["resize2fs", "-P", device]
        );
        assert_eq!(
            words(Command::ntfs_info(device)),
            ["ntfsresize", "--info", "--no-progress-bar", device]
        );
        assert_eq!(
            words(Command::ntfs_hibernation(device)),
            ["ntfscat", device, "hiberfil.sys"]
        );
        // Only ntfscat's head is read; everything else is read whole.
        assert!(only_its_head(&Command::ntfs_hibernation(device)));
        for whole in [
            Command::ext4_header(device),
            Command::ext4_least(device),
            Command::ntfs_info(device),
            Command::image_size(&format!("{DEFAULT_SOURCE}/var")),
        ] {
            assert!(!only_its_head(&whole), "{}", whole.line());
        }
        for place in IMAGE_PATHS {
            let path = format!("{DEFAULT_SOURCE}{place}");
            assert_eq!(
                words(Command::image_size(&path)),
                ["du", "-sb", "--one-file-system", path.as_str()]
            );
        }
        // The EFI system partition's mount, as `esp::measure` makes it.
        let target = "/run/lxb-installer/measure-a1B2c3";
        let mount = Command::new("mount", &esp::mount_arguments(device, target));
        assert_eq!(
            words(mount),
            [
                "mount",
                "-t",
                "vfat",
                "-o",
                "ro,nosuid,nodev,noexec",
                device,
                target
            ]
        );
        assert_eq!(words(Command::new("umount", &[target])), ["umount", target]);
        // That mount point is the measure's own, below the worker's directory.
        assert!(SCRATCH.starts_with(&format!("{}/", report::DIRECTORY)));

        // Everything that could write, check, repair or replay is refused,
        // and so is a read-only line of another shape.
        let refused = [
            by_hand("e2fsck", &["-f", "-n", device]),
            by_hand("e2fsck", &["-f", "-y", device]),
            by_hand("mkfs.ext4", &["-F", device]),
            by_hand("mkfs.fat", &["-F", "32", device]),
            by_hand("mkntfs", &["-f", device]),
            by_hand("sfdisk", &["--json", "/dev/nvme0n1"]),
            by_hand("wipefs", &["--no-act", device]),
            by_hand("ntfsresize", &[device]),
            by_hand("ntfsresize", &["--no-action", "--size", "20G", device]),
            by_hand(
                "ntfsresize",
                &["--info", "--no-progress-bar", "--force", device],
            ),
            by_hand("ntfsresize", &["--info", device]),
            by_hand("ntfsfix", &[device]),
            // ntfscat of anything but hiberfil.sys's head, as it is built.
            by_hand("ntfscat", &["-f", device, "hiberfil.sys"]),
            by_hand("ntfscat", &["--force", device, "hiberfil.sys"]),
            by_hand("ntfscat", &["-i", "2", device]),
            by_hand("ntfscat", &[device, "pagefile.sys"]),
            by_hand("ntfscat", &[device, "HIBERFIL.SYS"]),
            by_hand("ntfscat", &[device, "/hiberfil.sys"]),
            by_hand("ntfscat", &[device, "hiberfil.sys", "pagefile.sys"]),
            by_hand("ntfscat", &[device]),
            by_hand("ntfscat", &["-a", "0x80", device, "hiberfil.sys"]),
            by_hand("ntfscat", &["hiberfil.sys", device]),
            by_hand("ntfscat", &["LABEL=Windows", "hiberfil.sys"]),
            by_hand("ntfscat", &["/dev/../dev/sda3", "hiberfil.sys"]),
            by_hand("ntfscat", &["/tmp/windows.img", "hiberfil.sys"]),
            by_hand("ntfsinfo", &["-F", "hiberfil.sys", device]),
            by_hand("ntfscp", &[device, "x", "hiberfil.sys"]),
            by_hand("resize2fs", &[device, "20G"]),
            by_hand("resize2fs", &["-M", device]),
            by_hand("resize2fs", &["-P", "-f", device]),
            by_hand("tune2fs", &["-l", device]),
            by_hand("dumpe2fs", &[device]),
            by_hand("dumpe2fs", &["-h", "LABEL=Data"]),
            by_hand("dumpe2fs", &["-h", "UUID=5b7a3c1e"]),
            by_hand("dumpe2fs", &["-h", "/dev/../dev/sda1"]),
            by_hand("dumpe2fs", &["-h", "/dev/.."]),
            by_hand("dumpe2fs", &["-h", "/tmp/disk.img"]),
            by_hand("dumpe2fs", &["-h", device, "/dev/sda1"]),
            by_hand("du", &["-sb", "--one-file-system", "/"]),
            by_hand("du", &["-sb", "--one-file-system", "/run/archiso/airootfs"]),
            by_hand("du", &["-sb", "/run/archiso/airootfs/var"]),
            by_hand(
                "du",
                &["-sb", "--one-file-system", "/run/archiso/airootfs/var/.."],
            ),
            by_hand("mount", &["-t", "ext4", "-o", "ro", device, target]),
            by_hand("mount", &["-t", "ext4", "-o", "ro,noload", device, target]),
            by_hand("mount", &["-t", "btrfs", "-o", "ro", device, target]),
            by_hand("mount", &["-t", "ntfs3", "-o", "ro", device, target]),
            by_hand("mount", &["-o", "ro,nosuid,nodev,noexec", device, target]),
            by_hand(
                "mount",
                &["-t", "vfat", "-o", "rw,nosuid,nodev,noexec", device, target],
            ),
            by_hand(
                "mount",
                &["-t", "vfat", "-o", "ro,nosuid,nodev,noexec", device, "/mnt"],
            ),
            by_hand(
                "mount",
                &[
                    "-t",
                    "vfat",
                    "-o",
                    "ro,nosuid,nodev,noexec",
                    device,
                    "/run/lxb-installer/measure-a1B2c3/x",
                ],
            ),
            by_hand("umount", &["/"]),
            by_hand("umount", &["-l", target]),
            by_hand("umount", &["/run/lxb-installer/stage-a1B2c3"]),
        ];
        for command in refused {
            assert!(!allowed(&command), "{} was allowed", command.line());
        }
    }

    /// The real starter holds every command to the allowlist before anything
    /// else, so a line off it is turned down without a program being looked
    /// up: here with the fence's `FindNothing` held, so were it ever looked
    /// up nothing would be found and started either.
    #[test]
    fn a_command_off_the_allowlist_is_never_started() {
        let _nothing = fence::FindNothing::here();
        for command in [
            by_hand("e2fsck", &["-f", "-n", "/dev/vdz1"]),
            by_hand("printf", &["%s", "x"]),
            by_hand("ntfscat", &["-f", "/dev/vdz1", "hiberfil.sys"]),
        ] {
            let stop = start(&command).expect_err("started");
            assert_eq!(stop.failure, Failure::Internal, "{}", stop.detail);
            assert!(stop.detail.contains("not a command"), "{}", stop.detail);
            let stop = start_head(&command).expect_err("started");
            assert_eq!(stop.failure, Failure::Internal, "{}", stop.detail);
            assert!(stop.detail.contains("not a command"), "{}", stop.detail);
        }
        // An allowed command only by the starter its output needs: ntfscat's
        // hiberfil.sys never read whole, nothing else read only in part.
        let stop = start(&Command::ntfs_hibernation("/dev/vdz1")).expect_err("read whole");
        assert_eq!(stop.failure, Failure::Internal, "{}", stop.detail);
        assert!(stop.detail.contains("only the head"), "{}", stop.detail);
        for whole in [
            Command::ext4_header("/dev/vdz1"),
            Command::ntfs_info("/dev/vdz1"),
        ] {
            let stop = start_head(&whole).expect_err("cut short");
            assert_eq!(stop.failure, Failure::Internal, "{}", stop.detail);
            assert!(stop.detail.contains("not a command"), "{}", stop.detail);
        }
    }

    /// The real starter asks the test build's fence before the program is
    /// looked up, as discovery's lsblk does: an allowed command is refused
    /// there in a test (no probe is on any of the fence's lists, and a
    /// device path points at the machine). With `FindNothing` held, a
    /// starter whose question were lost would find nothing to start and
    /// answer "missing" instead of panicking, and this test would fail.
    #[test]
    fn probes_ask_the_fence_before_anything_starts() {
        let _nothing = fence::FindNothing::here();
        for command in [
            Command::ext4_header("/dev/vdz1"),
            Command::ext4_least("/dev/vdz1"),
            Command::ntfs_info("/dev/vdz1"),
            Command::image_size(&format!("{DEFAULT_SOURCE}/var")),
        ] {
            let words = fence::refusal(|| start(&command))
                .unwrap_or_else(|| panic!("{} went past the fence", command.line()));
            assert!(words.contains("test build:"), "{words}");
        }
        // And the starter that reads only a head.
        let command = Command::ntfs_hibernation("/dev/vdz1");
        let words = fence::refusal(|| start_head(&command))
            .unwrap_or_else(|| panic!("{} went past the fence", command.line()));
        assert!(words.contains("test build:"), "{words}");
    }

    /// The head starter reads at most `HIBERFILE_HEAD` bytes and ends the
    /// program once it has them: seen on real programs the fence lets any
    /// test run — printf printing far more than a pipe holds (so it is
    /// still printing, and is killed: ended by the starter), a little, and
    /// the shell's line that prints nothing and fails.
    #[test]
    fn the_head_starter_reads_no_further_than_the_header_and_ends_the_program() {
        let long = head_of("printf", &["%1000000s", "x"]).expect("started");
        assert_eq!(long.bytes, vec![b' '; HIBERFILE_HEAD]);
        assert_eq!(long.ended, None, "ended by the starter");
        assert_eq!(hibernation(&long), Hibernation::NotHibernated);

        let short = head_of("printf", &["%9s", "x"]).expect("started");
        assert_eq!(short.bytes, b"        x");
        assert_eq!(short.ended, Some(true));
        assert_eq!(hibernation(&short), Hibernation::Hibernated);

        let failed = head_of("sh", &["-c", "cat >&2; exit 3"]).expect("started");
        assert_eq!(failed.bytes, b"");
        assert_eq!(failed.ended, Some(false));
        assert!(matches!(hibernation(&failed), Hibernation::Unreadable(_)));
    }

    /// **A bounded head reads as the head starter does in time, and ends a
    /// program out of time** (for phase A's Windows reads): within its time,
    /// the same heads as above; given no time, the program it started is
    /// ended at once and the answer is an error that says so. The fence is
    /// asked first, as by the unbounded starter.
    #[test]
    fn a_bounded_head_reads_in_time_or_ends_its_program() {
        use std::time::Duration;
        let time = Some(Duration::from_secs(30));
        let long = head_within("printf", &["%1000000s", "x"], time).expect("started");
        assert_eq!(long.bytes, vec![b' '; HIBERFILE_HEAD]);
        assert_eq!(long.ended, None, "ended by the starter");
        let short = head_within("printf", &["%9s", "x"], time).expect("started");
        assert_eq!(
            (short.bytes.as_slice(), short.ended),
            (&b"        x"[..], Some(true))
        );
        let late =
            head_within("printf", &["%1000000s", "x"], Some(Duration::ZERO)).expect_err("no time");
        assert_eq!(late.failure, Failure::CommandFailed);
        assert!(late.detail.contains("within 0 seconds"), "{}", late.detail);
        let words =
            fence::refusal(|| head_within("printf", &["/dev/vdz1"], time)).expect("past the fence");
        assert!(words.contains("test build:"), "{words}");
    }

    // ----- The whole measure, on a machine of the test's own -----

    /// The machine a test hands in: every answer written by hand, every
    /// question written down in the order it was asked. A command is held to
    /// [`allowed`] as the real starter holds it.
    #[derive(Default)]
    struct Fake {
        live: Option<LiveSession>,
        request: Option<MeasureRequest>,
        disks: Vec<Disk>,
        busy: BTreeMap<String, String>,
        unreadable: BTreeSet<String>,
        printed: BTreeMap<String, Ran>,
        heads: BTreeMap<String, Head>,
        esp: BTreeMap<String, u64>,
        entries: BTreeMap<String, Entry>,
        asked: Vec<String>,
        answers: Vec<String>,
    }

    impl Machine for Fake {
        fn live(&mut self) -> Option<LiveSession> {
            self.asked.push("live".into());
            self.live.clone()
        }
        fn harden(&mut self) {
            self.asked.push("harden".into());
        }
        fn prepare_directory(&mut self) -> Result<(), String> {
            self.asked.push("prepare_directory".into());
            Ok(())
        }
        fn lock(&mut self) -> Result<(), Stop> {
            self.asked.push("lock".into());
            Ok(())
        }
        fn request(&mut self) -> Result<MeasureRequest, Stop> {
            self.asked.push("request".into());
            self.request
                .clone()
                .ok_or_else(|| Stop::new(Failure::InvalidRequest, "no request"))
        }
        fn discover(&mut self) -> Result<Vec<Disk>, String> {
            self.asked.push("discover".into());
            Ok(self.disks.clone())
        }
        fn busy(&mut self, device: &str) -> Result<Option<String>, String> {
            self.asked.push(format!("busy {device}"));
            if self.unreadable.contains(device) {
                return Err(format!("the holders of {device} cannot be read"));
            }
            Ok(self.busy.get(device).cloned())
        }
        fn isolate_mounts(&mut self) -> Result<(), Stop> {
            self.asked.push("isolate_mounts".into());
            Ok(())
        }
        fn esp_free(&mut self, device: &str) -> Result<u64, Stop> {
            self.asked.push(format!("esp {device}"));
            self.esp
                .get(device)
                .copied()
                .ok_or_else(|| Stop::new(Failure::CommandFailed, "mount failed"))
        }
        fn entry(&mut self, path: &str) -> Entry {
            self.asked.push(format!("entry {path}"));
            self.entries.get(path).copied().unwrap_or(Entry::Missing)
        }
        fn answer(&mut self, line: &str) {
            self.asked.push("answer".into());
            self.answers.push(line.into());
        }
    }

    impl Reads for Fake {
        fn run(&mut self, command: &Command) -> Result<Ran, Stop> {
            assert!(allowed(command), "asked to run {}", command.line());
            assert!(
                !only_its_head(command),
                "asked to read all of {}",
                command.line()
            );
            let line = command.line();
            self.asked.push(line.clone());
            self.printed
                .get(&line)
                .cloned()
                .ok_or_else(|| Stop::new(Failure::MissingTool, "not written"))
        }
        fn head(&mut self, command: &Command) -> Result<Head, Stop> {
            assert!(allowed(command), "asked to run {}", command.line());
            assert!(
                only_its_head(command),
                "asked for the head of {}",
                command.line()
            );
            let line = command.line();
            self.asked.push(line.clone());
            self.heads
                .get(&line)
                .cloned()
                .ok_or_else(|| Stop::new(Failure::MissingTool, "not written"))
        }
    }

    fn live_session() -> LiveSession {
        LiveSession {
            image: live::IMAGE.into(),
            medium_uuid: Some("2001-02-03-04-05-06-00".into()),
            medium_label: None,
            medium_mounted: true,
        }
    }

    fn spec(
        number: u32,
        start_mib: u64,
        size_mib: u64,
        kind: (&'static str, Content, &'static str),
    ) -> Spec {
        let (part_type, content, fs_type) = kind;
        Spec {
            number,
            start: start_mib * MIB,
            size: size_mib * MIB,
            part_type,
            content,
            fs_type,
        }
    }

    /// A disk with one of everything the measure tells apart: an ESP, ext4s
    /// clean, recovering, mounted and unreadable, NTFSs clean and scheduled
    /// for a check, LUKS, BitLocker, Windows' recovery, Btrfs and swap.
    fn every_kind() -> Disk {
        let esp = (GPT_ESP, Content::Efi, "vfat");
        let ext4 = (GPT_LINUX, Content::Linux, "ext4");
        let ntfs = (GPT_BASIC_DATA, Content::Windows, "ntfs");
        let mut disk = build(
            "/dev/vdx",
            512,
            Some(Table::Gpt),
            vec![
                spec(1, 1, 100, esp),
                spec(2, 101, 24 * 1024, ext4),
                spec(3, 24677, 30 * 1024, ntfs),
                spec(4, 55397, 2048, ntfs),
                spec(5, 57445, 1024, (GPT_LINUX, Content::Other, "crypto_LUKS")),
                spec(6, 58469, 1024, ext4),
                spec(7, 59493, 1024, ext4),
                spec(8, 60517, 1024, (GPT_LINUX, Content::Linux, "btrfs")),
                spec(9, 61541, 1024, (GPT_LINUX, Content::Swap, "swap")),
                spec(
                    10,
                    62565,
                    1024,
                    (GPT_BASIC_DATA, Content::Windows, "BitLocker"),
                ),
                spec(11, 63589, 512, ext4),
                spec(
                    12,
                    64101,
                    750,
                    (GPT_WINRE, Content::WindowsRecovery, "ntfs"),
                ),
            ],
            Vec::new(),
        );
        for partition in &mut disk.partitions {
            partition.uuid = format!("uuid-{}", partition.number);
        }
        disk
    }

    /// The machine for [`every_kind`], beside a disk that changed since it
    /// was listed and one that is gone: p7 mounted (so the disk is found in
    /// use), p11's holders unreadable, every tool's output written.
    fn every_kind_machine() -> Fake {
        let listed = every_kind();
        let mut now = listed.clone();
        now.partitions[6].mountpoints = vec!["/run/media/liveuser/Data".into()];
        now.blocked = Some(Blocked::InUse {
            mountpoints: vec!["/run/media/liveuser/Data".into()],
        });
        let mut replugged = build("/dev/vdy", 512, Some(Table::Gpt), Vec::new(), Vec::new());
        replugged.partitions = listed.partitions.clone();
        for partition in &mut replugged.partitions {
            partition.path = partition.path.replace("/dev/vdx", "/dev/vdy");
        }
        let mut found_replugged = replugged.clone();
        found_replugged.diskseq = Some(43);
        let mut gone = build("/dev/vdw", 512, Some(Table::Gpt), Vec::new(), Vec::new());
        gone.partitions = vec![listed.partitions[1].clone()];
        gone.partitions[0].path = "/dev/vdw2".into();

        let mut fake = Fake {
            live: Some(live_session()),
            request: Some(MeasureRequest {
                disks: vec![listed, replugged, gone],
            }),
            disks: vec![now, found_replugged],
            ..Fake::default()
        };
        fake.busy.insert(
            "/dev/vdx7".into(),
            "/dev/vdx7 is mounted in the live session".into(),
        );
        fake.unreadable.insert("/dev/vdx11".into());
        fake.esp.insert("/dev/vdx1".into(), 70 * MIB);
        let write = |fake: &mut Fake, command: Command, printed: Ran| {
            fake.printed.insert(command.line(), printed);
        };
        write(
            &mut fake,
            Command::ext4_header("/dev/vdx2"),
            ran(true, DUMPE2FS_CLEAN, ""),
        );
        write(
            &mut fake,
            Command::ext4_least("/dev/vdx2"),
            ran(
                true,
                "Estimated minimum size of the filesystem: 2961408\n",
                RESIZE2FS_BANNER,
            ),
        );
        write(
            &mut fake,
            Command::ntfs_info("/dev/vdx3"),
            ran(true, NTFSRESIZE_CLEAN, ""),
        );
        write(
            &mut fake,
            Command::ntfs_info("/dev/vdx4"),
            ntfsresize_check_scheduled(),
        );
        // Windows on vdx3 resumed and shut down properly: its hiberfil.sys
        // begins with no hibr.
        fake.heads.insert(
            Command::ntfs_hibernation("/dev/vdx3").line(),
            hiberfile(b"wake"),
        );
        write(
            &mut fake,
            Command::ext4_header("/dev/vdx6"),
            ran(true, &dumpe2fs_needs_recovery(), ""),
        );
        for (place, entry) in [
            ("/var", Entry::Directory),
            ("/opt", Entry::Directory),
            ("/srv", Entry::Missing),
            ("/root", Entry::Directory),
            ("/tmp", Entry::Other),
        ] {
            fake.entries
                .insert(format!("{DEFAULT_SOURCE}{place}"), entry);
        }
        for (place, bytes) in [("/var", 1_342_177_280u64), ("/opt", 734_003_200)] {
            let path = format!("{DEFAULT_SOURCE}{place}");
            write(
                &mut fake,
                Command::image_size(&path),
                ran(true, &format!("{bytes}\t{path}\n"), ""),
            );
        }
        // du on /root fails (nothing written for it), so its size is unknown.
        fake
    }

    fn answer_of(fake: &Fake) -> MeasureAnswer {
        assert_eq!(fake.answers.len(), 1, "one line, and only one");
        serde_json::from_str(&fake.answers[0]).expect("the line is an answer")
    }

    #[test]
    fn every_partition_is_answered_in_order_and_only_what_is_free_is_read() {
        let mut fake = every_kind_machine();
        let listed = fake.request.clone().unwrap().disks;
        assert_eq!(measure(|| true, || true, &mut fake), 0);
        let MeasureAnswer::Measured {
            measures,
            image,
            changed,
        } = answer_of(&fake)
        else {
            panic!("refused: {:?}", fake.answers);
        };
        let of = |path: &str| {
            measures
                .parts
                .iter()
                .find(|m| m.path == path)
                .map(|m| m.measure)
                .unwrap_or_else(|| panic!("{path} has no measure"))
        };
        // An ESP: what is free, and the rest of it in use; no least.
        assert_eq!(
            of("/dev/vdx1"),
            Measure::Read {
                used: 30 * MIB,
                free: 70 * MIB,
                least: None,
            }
        );
        assert_eq!(
            of("/dev/vdx2"),
            Measure::Read {
                used: 11 * GIB,
                free: 13 * GIB,
                least: Some(2_961_408 * 4096),
            }
        );
        // An NTFS ntfsresize read cleanly, and signed not hibernated.
        assert_eq!(
            of("/dev/vdx3"),
            Measure::Read {
                used: 13_254_000_000,
                free: 32_212_254_208 - 13_254_000_000,
                least: Some(13_253_541_888),
            }
        );
        assert_eq!(of("/dev/vdx4"), Measure::NeedsCheck);
        assert_eq!(of("/dev/vdx5"), Measure::Encrypted);
        assert_eq!(of("/dev/vdx6"), Measure::NeedsCheck);
        assert_eq!(of("/dev/vdx7"), Measure::InUse);
        assert_eq!(of("/dev/vdx10"), Measure::Encrypted);
        assert_eq!(of("/dev/vdx11"), Measure::Unreadable);
        // Btrfs, swap and Windows' recovery partition are not read at all.
        for path in ["/dev/vdx8", "/dev/vdx9", "/dev/vdx12"] {
            assert!(measures.parts.iter().all(|m| m.path != path), "{path}");
        }
        // The replugged disk and the gone one are named as changed, and
        // nothing of either is asked: of each, only its Windows volumes as
        // listed are answered, unread (the gone one has none).
        assert_eq!(changed, ["/dev/vdy", "/dev/vdw"]);
        let unread: Vec<(&str, Measure)> = measures
            .parts
            .iter()
            .filter(|m| !m.path.starts_with("/dev/vdx"))
            .map(|m| (m.path.as_str(), m.measure))
            .collect();
        assert_eq!(
            unread,
            [
                ("/dev/vdy3", Measure::Unreadable),
                ("/dev/vdy4", Measure::Unreadable),
                ("/dev/vdy10", Measure::Encrypted),
            ]
        );
        assert!(fake
            .asked
            .iter()
            .all(|asked| !asked.contains("/dev/vdy") && !asked.contains("/dev/vdw")));
        // Each measure is the listed partition's own: path, filesystem, UUID
        // and size.
        for one in &measures.parts {
            let partition = listed
                .iter()
                .find_map(|disk| disk.partition(&one.path))
                .unwrap();
            assert_eq!(one.fs, partition.fs_type);
            assert_eq!(measures.of(partition), Some(&one.measure));
        }
        // On each disk in the partitions' own order.
        let order: Vec<&str> = measures
            .parts
            .iter()
            .map(|m| m.path.as_str())
            .filter(|path| path.starts_with("/dev/vdx"))
            .collect();
        assert_eq!(
            order,
            [
                "/dev/vdx1",
                "/dev/vdx2",
                "/dev/vdx3",
                "/dev/vdx4",
                "/dev/vdx5",
                "/dev/vdx6",
                "/dev/vdx7",
                "/dev/vdx10",
                "/dev/vdx11",
            ]
        );
        assert_eq!(
            image.paths,
            [
                ("/opt".to_string(), 734_003_200),
                ("/srv".to_string(), 0),
                ("/var".to_string(), 1_342_177_280),
            ]
            .into()
        );
    }

    /// The plan's order, question by question: the guard, the live session,
    /// hardening, the directory, the lock, the request, the disks, every busy
    /// question before the namespace, and only then anything read.
    #[test]
    fn the_measure_asks_in_the_plans_order() {
        let mut fake = every_kind_machine();
        measure(|| true, || true, &mut fake);
        let image = |place: &str| format!("{DEFAULT_SOURCE}{place}");
        let mut expected: Vec<String> = [
            "live",
            "harden",
            "prepare_directory",
            "lock",
            "request",
            "discover",
        ]
        .map(String::from)
        .to_vec();
        for n in [1, 2, 3, 4, 5, 6, 7, 10, 11] {
            expected.push(format!("busy /dev/vdx{n}"));
        }
        expected.push("isolate_mounts".into());
        expected.extend([
            "esp /dev/vdx1".to_string(),
            "$ dumpe2fs -h /dev/vdx2".into(),
            "$ resize2fs -P /dev/vdx2".into(),
            "$ ntfsresize --info --no-progress-bar /dev/vdx3".into(),
            // Read cleanly: then its sign of hibernation.
            "$ ntfscat /dev/vdx3 hiberfil.sys".into(),
            // A check due: no sign.
            "$ ntfsresize --info --no-progress-bar /dev/vdx4".into(),
            // Recovering: its header is read, its least is not asked.
            "$ dumpe2fs -h /dev/vdx6".into(),
            format!("entry {}", image("/var")),
            format!("$ du -sb --one-file-system {}", image("/var")),
            format!("entry {}", image("/opt")),
            format!("$ du -sb --one-file-system {}", image("/opt")),
            format!("entry {}", image("/srv")),
            format!("entry {}", image("/root")),
            format!("$ du -sb --one-file-system {}", image("/root")),
            format!("entry {}", image("/tmp")),
            "answer".into(),
        ]);
        assert_eq!(fake.asked, expected);
    }

    /// A disk listed as one nothing may be made on — the installation
    /// medium, a read-only disk, one not read completely — is left alone
    /// while it is still exactly as listed: nothing of it is read, and it is
    /// not named as changed, which would only send the interface to list the
    /// disks again and ask again, for ever (the medium is listed on every
    /// live system). One that is not as listed any more is named, as is a
    /// disk a plan could use that has become one of them; none is read. Of
    /// each disk not read, its Windows volumes as listed — each NTFS but
    /// Windows' recovery tools, each BitLocker volume — are answered unread
    /// all the same, with nothing read, so the rule set sees them as Windows
    /// whose state is not known (`Measures::shrink_of`); nothing else of it.
    #[test]
    fn a_disk_nothing_may_be_made_on_is_left_alone_while_it_is_as_listed() {
        let ext4 = (GPT_LINUX, Content::Linux, "ext4");
        let one = |path: &str| {
            build(
                path,
                512,
                Some(Table::Gpt),
                vec![spec(1, 1, 1024, ext4)],
                Vec::new(),
            )
        };
        let mut medium = every_kind();
        medium.blocked = Some(Blocked::InstallationMedia);
        let mut read_only = one("/dev/vdy");
        read_only.read_only = true;
        read_only.blocked = Some(Blocked::ReadOnly);
        let mut unknown = one("/dev/vdv");
        unknown.blocked = Some(Blocked::Unknown);
        let usable = one("/dev/vdu");
        let measured = |listed: Vec<Disk>, now: Vec<Disk>| {
            let mut fake = Fake {
                live: Some(live_session()),
                request: Some(MeasureRequest { disks: listed }),
                disks: now,
                ..Fake::default()
            };
            assert_eq!(measure(|| true, || true, &mut fake), 0);
            let MeasureAnswer::Measured {
                measures, changed, ..
            } = answer_of(&fake)
            else {
                panic!("refused: {:?}", fake.answers);
            };
            // Nothing read; only the medium's Windows volumes, as listed,
            // answered unread.
            let unread: Vec<(&str, &str, Measure)> = measures
                .parts
                .iter()
                .map(|one| (one.path.as_str(), one.fs.as_str(), one.measure))
                .collect();
            assert_eq!(
                unread,
                [
                    ("/dev/vdx3", "ntfs", Measure::Unreadable),
                    ("/dev/vdx4", "ntfs", Measure::Unreadable),
                    ("/dev/vdx10", "BitLocker", Measure::Encrypted),
                ]
            );
            let asked_of_a_disk: Vec<&String> = fake
                .asked
                .iter()
                .filter(|asked| asked.contains("/dev/"))
                .collect();
            assert_eq!(asked_of_a_disk, Vec::<&String>::new());
            changed
        };

        // Each still exactly as listed: nothing read, nothing named.
        let blocked = vec![medium.clone(), read_only.clone(), unknown.clone()];
        assert_eq!(
            measured(blocked.clone(), blocked.clone()),
            Vec::<String>::new()
        );

        // None is as listed any more, nor is the usable disk, which has
        // become the medium: every one is named.
        let listed = [blocked, vec![usable.clone()]].concat();
        let mut medium_now = medium.clone();
        medium_now.partitions.pop();
        let mut writable_now = read_only.clone();
        writable_now.read_only = false;
        writable_now.blocked = None;
        let mut unknown_now = unknown.clone();
        unknown_now.diskseq = Some(43);
        let mut usable_is_the_medium = usable.clone();
        usable_is_the_medium.blocked = Some(Blocked::InstallationMedia);
        assert_eq!(
            measured(
                listed.clone(),
                vec![medium_now, writable_now, unknown_now, usable_is_the_medium]
            ),
            ["/dev/vdx", "/dev/vdy", "/dev/vdv", "/dev/vdu"]
        );

        // Gone, each is named too.
        assert_eq!(
            measured(listed, Vec::new()),
            ["/dev/vdx", "/dev/vdy", "/dev/vdv", "/dev/vdu"]
        );

        // What is left alone is what `same_disk` refuses of a disk compared
        // with itself, and only that: a disk in use or too small is read.
        assert!(unusable(&medium) && unusable(&read_only) && unusable(&unknown));
        let mut busy = one("/dev/vdt");
        busy.blocked = Some(Blocked::InUse {
            mountpoints: vec!["/run/media/liveuser/Data".into()],
        });
        let mut small = one("/dev/vds");
        small.blocked = Some(Blocked::TooSmall);
        assert!(!unusable(&usable) && !unusable(&busy) && !unusable(&small));
    }

    #[test]
    fn measure_is_refused_off_the_medium_even_as_root() {
        let refused = |on_medium: fn() -> bool, is_root: fn() -> bool, live: bool| {
            let mut fake = every_kind_machine();
            if !live {
                fake.live = None;
            }
            assert_eq!(measure(on_medium, is_root, &mut fake), 1);
            match answer_of(&fake) {
                MeasureAnswer::Refused { failure, .. } => (failure, fake.asked),
                measured => panic!("measured: {measured:?}"),
            }
        };
        let answered = vec!["answer".to_string()];
        // Off the medium, root or not: nothing at all is asked of the
        // machine but to print the refusal.
        assert_eq!(
            refused(|| false, || true, true),
            (Failure::NotLiveSession, answered.clone())
        );
        assert_eq!(
            refused(|| false, || false, true),
            (Failure::NotLiveSession, answered.clone())
        );
        assert_eq!(
            refused(|| true, || false, true),
            (Failure::NotRoot, answered.clone())
        );
        // On something that only looks like it: the live session is asked,
        // and nothing after it.
        assert_eq!(
            refused(|| true, || true, false),
            (
                Failure::NotLiveSession,
                vec!["live".to_string(), "answer".into()]
            )
        );
    }

    /// The worker's progress file, log and pid file are an installation's
    /// record, and opening them starts them afresh. Through a whole measure
    /// on an injected machine the only questions before the request are the
    /// four the plan names — the directory, but no file in it — and the
    /// measure's own code names none of the reporting that opens or writes
    /// those files, nor `run`'s log sink (so its log lines go to standard
    /// error). Comments are left out of the reading: they may name what the
    /// code never calls.
    #[test]
    fn measure_never_opens_the_report_files() {
        let mut fake = every_kind_machine();
        assert_eq!(measure(|| true, || true, &mut fake), 0);
        let before_the_request: Vec<&str> = fake
            .asked
            .iter()
            .map(String::as_str)
            .take_while(|asked| *asked != "request")
            .collect();
        assert_eq!(
            before_the_request,
            ["live", "harden", "prepare_directory", "lock"]
        );

        let source = include_str!("measure.rs");
        let code: String = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .unwrap()
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for name in [
            "open_files",
            "close_files",
            "hold_logs",
            "release_logs",
            "log_text",
            "report::emit",
            "report::log",
            "set_log",
            "PROGRESS_FILE",
            "LOG_FILE",
            "PID_FILE",
        ] {
            assert!(!code.contains(name), "the measure's code names {name}");
        }
        let reporting: BTreeSet<&str> = code
            .match_indices("report::")
            .map(|(at, _)| {
                code[at..]
                    .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
                    .next()
                    .unwrap()
            })
            .collect();
        assert_eq!(
            reporting,
            ["report::DIRECTORY", "report::prepare_directory"].into()
        );
    }

    #[test]
    fn a_request_is_read_within_its_bound_and_judged() {
        let request = MeasureRequest {
            disks: vec![every_kind()],
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        assert_eq!(read_request(bytes.as_slice()).unwrap(), request);
        let too_big = vec![b' '; MAX_REQUEST as usize + 1];
        assert_eq!(
            read_request(too_big.as_slice()).unwrap_err().failure,
            Failure::InvalidRequest
        );
        let unknown = br#"{"disks":[],"also":"mkfs"}"#;
        assert_eq!(
            read_request(unknown.as_slice()).unwrap_err().failure,
            Failure::InvalidRequest
        );
        assert!(judge(&request).is_ok());

        let twice = MeasureRequest {
            disks: vec![every_kind(), every_kind()],
        };
        assert_eq!(judge(&twice).unwrap_err().failure, Failure::InvalidRequest);
        let mut labelled = every_kind();
        labelled.partitions[1].path = "LABEL=Data".into();
        let labelled = MeasureRequest {
            disks: vec![labelled],
        };
        assert_eq!(
            judge(&labelled).unwrap_err().failure,
            Failure::InvalidRequest
        );
        let mut climbing = every_kind();
        climbing.path = "/dev/../dev/sda".into();
        let climbing = MeasureRequest {
            disks: vec![climbing],
        };
        let refused = judge(&climbing).unwrap_err();
        assert!(!refused.detail.contains(".."), "{}", refused.detail);

        // Refused as a whole, after the lock and before the disks.
        let mut fake = every_kind_machine();
        fake.request = Some(twice);
        assert_eq!(measure(|| true, || true, &mut fake), 1);
        assert!(!fake.asked.iter().any(|asked| asked == "discover"));
    }

    #[test]
    fn an_answer_too_long_for_the_interface_is_refused_rather_than_cut() {
        let parts = (0..2000)
            .map(|n| Measured {
                path: format!("/dev/vdx{n}"),
                fs: "ntfs".into(),
                uuid: "x".repeat(200),
                size: GIB,
                measure: Measure::Unreadable,
            })
            .collect();
        let (line, code) = line_of(&MeasureAnswer::Measured {
            measures: Measures { parts },
            image: ImageSizes::default(),
            changed: Vec::new(),
        });
        assert_eq!(code, 1);
        assert!(line.len() < 200, "{line}");
        assert!(matches!(
            serde_json::from_str::<MeasureAnswer>(&line).unwrap(),
            MeasureAnswer::Refused {
                failure: Failure::Internal,
                ..
            }
        ));
    }

    // ----- The polkit actions -----

    /// The policy file holds both actions, each matched by pkexec on the
    /// program and its first argument, so `--worker` can only ever be the
    /// installation and `--measure` only the read-only measurement; both ask
    /// an administrator once and keep the answer a moment, and both speak
    /// the ten languages.
    #[test]
    fn the_policy_has_both_actions_and_both_argv1() {
        let policy = include_str!("../../data/io.github.petexy.lxb-installer.policy");
        assert_eq!(policy.matches("<action id=").count(), 2);
        let action = |id: &str| {
            let start = policy
                .find(&format!(
                    "<action id=\"io.github.petexy.lxb-installer.{id}\">"
                ))
                .unwrap_or_else(|| panic!("no {id} action"));
            let end = start + policy[start..].find("</action>").expect("closed");
            &policy[start..end]
        };
        let annotation = |key: &str, value: &str| {
            format!("<annotate key=\"org.freedesktop.policykit.exec.{key}\">{value}</annotate>")
        };
        for (id, argument) in [("install", "--worker"), ("measure", "--measure")] {
            let action = action(id);
            assert!(
                action.contains(&annotation("path", "/usr/bin/lxb-installer")),
                "{id}"
            );
            assert!(action.contains(&annotation("argv1", argument)), "{id}");
            assert_eq!(action.matches("org.freedesktop.policykit.exec.").count(), 2);
            for default in [
                "<allow_any>no</allow_any>",
                "<allow_inactive>no</allow_inactive>",
                "<allow_active>auth_admin_keep</allow_active>",
            ] {
                assert!(action.contains(default), "{id}: {default}");
            }
            for tag in ["description", "message"] {
                assert_eq!(action.matches(&format!("<{tag}>")).count(), 1, "{id} {tag}");
                for language in [
                    "de", "en_GB", "es", "fr", "hi", "pl", "pt_BR", "ru", "zh_CN",
                ] {
                    let opened = format!("<{tag} xml:lang=\"{language}\">");
                    assert_eq!(action.matches(&opened).count(), 1, "{id} {tag} {language}");
                }
            }
        }
        // The measurement's own words say nothing is changed.
        assert!(action("measure").contains("Nothing on them is changed."));
    }
}
