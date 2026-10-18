//! **Carrying a hand-made layout out** (§7.0, Unit Q, "The worker"): from
//! the Check step's first look at the disks to the [`Mounted`] the rest of
//! the worker reads, everything asked of the machine through [`Machine`].
//!
//! [`install_storage`] is what `worker::install` runs for a
//! `Target::Layout`, in this order — the plan's, so that nothing is written
//! before every check has passed:
//!
//! 1. nothing but what lies within one set of `layout::PROVEN` (asked a
//!    third time, after the interface's `ready()` and `check::validate`);
//! 2. the udisks rule put in (`quiet::Quiet`), its guard in the caller's
//!    slot so it is taken out however the run ends;
//! 3. every disk found again and the layout judged again on it
//!    (`check::layout_unchanged`);
//! 4. nothing it writes or mounts in use (`check::layout_not_busy`);
//! 5. the internet, where the choices use it;
//! 6. the stage, and [`prepare`]:
//!    - **phase A**, which writes nothing: every disk written to found at
//!      its reviewed size and sector size, and its table — `sfdisk
//!      --json`, a disk getting a new table included — exactly the
//!      reviewed one (`plan::check_table`); every partition used on a disk
//!      only mounted from still holding the filesystem it was reviewed
//!      with (`blkid -p`, read-only); every disk's commands built and the
//!      [`Fence`] built from the consent; every filesystem shrunk read
//!      again — and where an NTFS is, every Windows volume of every disk
//!      found now (`shrinking::read_again`); every kept EFI system
//!      partition measured, and what the image puts in each folder a
//!      formatted partition is mounted at, then `layout::measured` on all
//!      those numbers, the rule across every disk among them; each shrink
//!      readied from what was read and dry-run (`shrinking::ready`,
//!      `shrinking::dry_runs`); every command logged;
//!    - **phase B.1**, before any entry changes on any disk: each
//!      filesystem shrunk, given leave as every command is, and read back
//!      inside its new entry (`shrinking::carry_out`);
//!    - **phase B**, disk by disk in path order: each command let through
//!      by the fence, its disk claimed, then run; the boot flag; the disk
//!      read back (`plan::read_back`), each partition's path now;
//!    - **phase C**: each partition made or formatted let through by the
//!      fence, claimed, found mounted nowhere in the live session and used
//!      by no swap or stack, then wiped and given its filesystem; what
//!      `blkid -p` reads of each partition used, a kept one's UUID the one
//!      it was reviewed with;
//!    - **phases D to G**: Btrfs' subvolumes, the mounts in order, the swap
//!      file, the fstab — and the [`Mounted`].
//!
//! A filesystem shrunk (`Planned::ShrinkFilesystem`) is carried out only by
//! its own commands (`resize::Shrink::planned`, step Q9), all of them in
//! phase B.1, so a filesystem is always smaller before its entry is. A
//! mount never goes through the fence, which judges commands that destroy
//! something (and every argument naming `/dev` among them): the mounts are
//! given only the devices read back.
use super::fence::Fence;
use super::machine::Machine;
use super::mounts;
use super::plan::{self, Act, Context, Paths, Planned};
use super::shrinking;
use super::{mounted, root_filesystem, used, Fact, Facts};
use crate::model::config::{Filesystem, InstallConfig, Target};
use crate::model::disks::{same_uuid, Table};
use crate::model::layout::{
    self, image_fills, DiskLayout, ImageSizes, Measure, Measured, Measures, MountPoint, Refusal,
    Unshrinkable,
};
use crate::model::protocol::{Event, Failure, Step};
use crate::worker::layout::{Firmware, Mounted, Stop};
use crate::worker::partition::script::Command;
use crate::worker::partition::table::{self, OnDisk};
use crate::worker::partition::Swapfile;
use crate::worker::quiet::Quiet;
use crate::worker::storage::progress::Steps;
use crate::worker::storage::Stage;
use crate::worker::system::check;
use crate::worker::system::image::Image;
use crate::worker::target;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How many times the disk's partitions are looked at after its table is
/// written before it is taken to be other than the layout leaves it (with
/// [`Machine::pause`] between, a quarter of a second on the machine).
const READ_BACK_TRIES: u32 = 40;

fn internal(detail: impl Into<String>) -> Stop {
    Stop::new(Failure::Internal, detail)
}

fn changed(detail: impl Into<String>) -> Stop {
    Stop::new(Failure::DiskChanged, detail)
}

fn busy(detail: impl Into<String>) -> Stop {
    Stop::new(Failure::DiskBusy, detail)
}

/// The layout a request carries.
fn layout_of(config: &InstallConfig) -> Result<&[DiskLayout], Stop> {
    match &config.target {
        Target::Layout { disks } => Ok(disks),
        _ => Err(internal("the request carries no layout")),
    }
}

/// `layout`'s disks, in path order.
fn by_path(layout: &[DiskLayout]) -> Vec<&DiskLayout> {
    let mut disks: Vec<&DiskLayout> = layout.iter().collect();
    disks.sort_by(|one, other| one.disk.path.cmp(&other.disk.path));
    disks
}

/// **A hand-made layout carried out**, from the Check step on (see the
/// module): `Ok` with whether the internet answered and the target, mounted.
/// The udisks rule's guard is left in `quiet` and the stage in `stage`,
/// both for the caller to let go of once every installer mount is released.
pub fn install_storage(
    config: &InstallConfig,
    image: &Image,
    firmware: Firmware,
    machine: &mut dyn Machine,
    report: &mut dyn FnMut(Event),
    stage: &mut Option<PathBuf>,
    quiet: &mut Option<Quiet>,
) -> Result<(bool, Mounted), Stop> {
    let laid = layout_of(config)?;
    if let Some(unproven) = layout::unproven(laid, firmware) {
        return Err(internal(unproven.line()));
    }
    Quiet::install(laid.iter().map(|one| &one.disk), machine.udev(), quiet)?;
    check::layout_unchanged(config, firmware, machine.discover())?;
    let mountinfo = machine.session_mounts();
    let swaps = machine.swaps();
    check::layout_not_busy(config, mountinfo, &swaps, &mut |device| {
        machine.device_facts(device)
    })?;
    let online = machine.online(config, image)?;
    let made = machine.stage()?;
    *stage = Some(made.clone());
    let mounted = prepare(config, firmware, &made, machine, report)?;
    Ok((online, mounted))
}

/// `sfdisk --json` of `disk`: its table as the disk has it.
fn table_command(disk: &str) -> Command {
    Command::new("sfdisk", &["--json", disk])
}

/// `blkid -p` of one `tag` of `device`: read from the device itself, never
/// from a cache that may still describe what was there before.
fn blkid(device: &str, tag: &str) -> Command {
    Command::new("blkid", &["-p", "-s", tag, "-o", "value", device])
}

/// The table on `disk` now, which must be one sfdisk can read.
fn read_table(machine: &mut dyn Machine, disk: &str) -> Result<OnDisk, Stop> {
    let json = machine
        .read(&table_command(disk))
        .map_err(|stop| changed(format!("cannot read the table of {disk}: {}", stop.detail)))?;
    table::parse_sfdisk(&json).map_err(internal)
}

/// **Phase A, the disks**: every disk the layout writes to, in path order,
/// at its reviewed size and sector size as the kernel reports them, and its
/// table exactly the one reviewed (`plan::check_table`) — no table where
/// there was none, so a blank disk sfdisk finds no table on, and one
/// getting a new table held to the table it was reviewed with all the same.
/// The tables read, by disk.
fn tables_as_reviewed(
    layout: &[DiskLayout],
    machine: &mut dyn Machine,
) -> Result<BTreeMap<String, OnDisk>, Stop> {
    let mut tables = BTreeMap::new();
    for one in by_path(layout) {
        if !plan::writes_to(one) {
            continue;
        }
        let disk = &one.disk;
        let path = disk.path.as_str();
        let device = Command::new(
            "lsblk",
            &[
                "--json",
                "--bytes",
                "--paths",
                "--nodeps",
                "-o",
                "NAME,SIZE,LOG-SEC",
                path,
            ],
        );
        let (size, sector) =
            table::parse_device(&machine.read(&device)?, path).map_err(internal)?;
        if size != disk.size || sector != u64::from(disk.logical_sector) {
            return Err(changed(format!(
                "{path} is now {size} bytes in {sector} byte sectors, not {} in {}",
                disk.size, disk.logical_sector
            )));
        }
        let on_disk = match machine.read(&table_command(path)) {
            Ok(json) => Some(table::parse_sfdisk(&json).map_err(internal)?),
            // A disk with no table answers sfdisk with no JSON at all.
            Err(_) if disk.table.is_none() => None,
            Err(stop) => {
                return Err(changed(format!(
                    "cannot read the table of {path}: {}",
                    stop.detail
                )))
            }
        };
        plan::check_table(one, on_disk.as_ref()).map_err(changed)?;
        if let Some(on_disk) = on_disk {
            tables.insert(path.to_string(), on_disk);
        }
    }
    Ok(tables)
}

/// **Phase A, the disks only mounted from**: on each disk the layout
/// writes nothing to, every partition it keeps and uses — a `/home` from
/// before, one at a path of one's own — read by `blkid -p`, which only
/// reads, from the device itself and never from a cache, must still hold
/// the filesystem it was reviewed with: its UUID, where it was reviewed
/// with one (compared without case, `disks::same_uuid`). Else
/// `DiskChanged`, before anything is written to any disk. Check found the
/// disk again by lsblk's word (`same_disk`); this reads the partition
/// itself, as late as possible before the first write. A disk written to
/// is read back once its table is written (`read_back`), and every kept
/// partition is read again once everything is made (`read_facts`).
fn kept_as_reviewed(layout: &[DiskLayout], machine: &mut dyn Machine) -> Result<(), Stop> {
    for one in by_path(layout) {
        if plan::writes_to(one) {
            continue;
        }
        for (part, was) in one.parts.iter().zip(&one.disk.partitions) {
            let Some(at) = part.fate.mount() else {
                continue;
            };
            if was.uuid.is_empty() {
                continue;
            }
            let uuid = machine
                .read(&blkid(&part.path, "UUID"))
                .map_err(|stop| {
                    changed(format!(
                        "what {}, kept at {}, holds cannot be read: {}",
                        part.path,
                        at.path(),
                        stop.detail
                    ))
                })?
                .trim()
                .to_string();
            if !same_uuid(&uuid, &was.uuid) {
                return Err(changed(format!(
                    "{}, kept at {}, holds the filesystem {uuid}, not the {} it was reviewed \
                     with",
                    part.path,
                    at.path(),
                    was.uuid
                )));
            }
        }
    }
    Ok(())
}

/// **Phase A, the numbers**, read by the worker itself — never the
/// interface's: what every filesystem shrunk was read again as
/// (`shrinking::read_again`: for an NTFS, every NTFS and BitLocker volume
/// of every disk found now, read or answered unread, as the measure entry
/// answers them — the rule set judges a shrink of an NTFS across every disk
/// of the measurement, `Measures::shrink_of`, and a disk left out of it
/// would be a Windows it never saw), each kept EFI system partition's free
/// space, and what the image puts in each folder a partition made or
/// formatted is mounted at; then `layout::measured` on them, mapped by
/// [`refused_for`].
fn measured_again(
    config: &InstallConfig,
    layout: &[DiskLayout],
    stage: &Stage,
    machine: &mut dyn Machine,
    steps: &mut Steps,
    shrunk: Measures,
) -> Result<(), Stop> {
    let mut measures = shrunk;
    let mut images = ImageSizes::default();
    for one in used(layout) {
        match (one.made, one.was, one.at) {
            (None, Some(was), MountPoint::Efi | MountPoint::Boot) => {
                let free = machine.esp_free(&was.path, &stage.esp)?;
                steps.log(format!(
                    "layout: the EFI system partition {} has {free} bytes free",
                    was.path
                ));
                measures.parts.push(Measured {
                    path: was.path.clone(),
                    fs: was.fs_type.clone(),
                    uuid: was.uuid.clone(),
                    size: was.size,
                    measure: Measure::Read {
                        used: was.size.saturating_sub(free),
                        free,
                        least: None,
                    },
                });
            }
            (Some(_), _, MountPoint::Other(path)) if image_fills(path) => {
                match machine.image_size(&config.source, path) {
                    Some(size) => {
                        steps.log(format!("layout: the image puts {size} bytes at {path}"));
                        images.paths.insert(path.clone(), size);
                    }
                    None => steps.log(format!("layout: what the image puts at {path} is unknown")),
                }
            }
            _ => {}
        }
    }
    layout::measured(layout, &measures, &images).map_err(|refusal| {
        Stop::new(
            refused_for(&refusal, layout, &measures),
            format!("the layout's numbers refuse it: {refusal:?}"),
        )
    })
}

/// **What Stopped says of a refusal of the worker's own numbers** (phase
/// A): a kept EFI system partition too full is `EspTooFull`, a folder too
/// small for what the image puts there `NotEnoughSpace`, what the image puts
/// there not known `ImageInvalid`; and of a shrink (step Q9), a Windows on
/// this computer hibernated or shut down by Fast Startup —
/// the one shrunk, or another the rule across every disk saw —
/// `WindowsHibernated`; the filesystem shrunk asking for a check first
/// `FilesystemNeedsCheck`, in use `DiskBusy`; and **each cause the rule
/// across every disk finds in another Windows partition its own words**, by
/// what `measures` say of it: in use `WindowsInUse`, marked for Windows'
/// check `WindowsNeedsCheck`, not read (its tool or sign failed, not in time,
/// its disk not read) `WindowsUnread`; a BitLocker volume `BitLocker`.
/// Anything else that holds it back — its own unread, no hibernation file of
/// its own, more used now than the size it was to be shrunk to leaves room
/// for, not read at all — `ShrinkRefused`. Anything else is a request the
/// rule set refuses, `InvalidRequest`.
fn refused_for(refusal: &Refusal, layout: &[DiskLayout], measures: &Measures) -> Failure {
    let shrunk = |which: &layout::Which| {
        layout.iter().flat_map(|one| &one.parts).any(|part| {
            matches!(part.fate, layout::Fate::Shrink { .. })
                && *which == layout::Which::Part(part.path.clone())
        })
    };
    match refusal {
        Refusal::EspFull { .. } => Failure::EspTooFull,
        Refusal::TooSmall { .. } => Failure::NotEnoughSpace,
        Refusal::ImageUnknown { .. } => Failure::ImageInvalid,
        Refusal::Unshrinkable { why, .. } => match why {
            Unshrinkable::Itself(Measure::Hibernated) | Unshrinkable::Hibernated { .. } => {
                Failure::WindowsHibernated
            }
            Unshrinkable::Itself(Measure::NeedsCheck) => Failure::FilesystemNeedsCheck,
            Unshrinkable::Itself(Measure::InUse) => Failure::DiskBusy,
            Unshrinkable::Unread { other } => {
                let measure = measures
                    .parts
                    .iter()
                    .find(|one| one.path == *other)
                    .map(|one| one.measure);
                match measure {
                    Some(Measure::InUse) => Failure::WindowsInUse,
                    Some(Measure::NeedsCheck) => Failure::WindowsNeedsCheck,
                    _ => Failure::WindowsUnread,
                }
            }
            Unshrinkable::BitLocker { .. } => Failure::BitLocker,
            _ => Failure::ShrinkRefused,
        },
        Refusal::ShrinkBelow { .. } => Failure::ShrinkRefused,
        Refusal::NotRead { part } if shrunk(part) => Failure::ShrinkRefused,
        _ => Failure::InvalidRequest,
    }
}

/// **The leave one command that destroys something must have**: the fence
/// lets it through for what it does, where the kernel has its device now;
/// its device is claimed exclusively; a partition about to be wiped, given
/// a filesystem or have its filesystem shrunk is mounted nowhere in the
/// live session (`/proc/1/mountinfo`, which the worker's own namespace does
/// not show), no swap, held by nothing. Gives the command, for the caller
/// to run then and only then. A filesystem's shrink as such
/// (`Planned::ShrinkFilesystem`) runs nothing: only its own commands do
/// (`resize::Shrink::planned`).
pub(super) fn leave<'p>(
    planned: &'p Planned,
    fence: &mut Fence,
    machine: &mut dyn Machine,
) -> Result<&'p Command, Stop> {
    let Planned::Run {
        act,
        device,
        command,
    } = planned
    else {
        return Err(internal(format!(
            "{}: a filesystem's shrink is carried out only by its own commands",
            planned.device()
        )));
    };
    fence.let_through(planned, machine.sysfs())?;
    machine.claim(device)?;
    if matches!(act, Act::WipeSignatures | Act::Make | Act::ShrinkFilesystem) {
        let mountinfo = machine
            .session_mounts()
            .map_err(|e| busy(format!("cannot read the live system's mounts: {e}")))?;
        let swaps = machine.swaps();
        let (major_minor, held) = machine.device_facts(device).map_err(busy)?;
        if let Some(why) = check::device_busy(device, &major_minor, held, &mountinfo, &swaps) {
            return Err(busy(why));
        }
    }
    Ok(command)
}

/// **One command that destroys something, given leave** ([`leave`]), and
/// only then run.
fn destroy(planned: &Planned, fence: &mut Fence, machine: &mut dyn Machine) -> Result<(), Stop> {
    let command = leave(planned, fence, machine)?;
    machine.write(command)
}

/// **Phase B, step 9**: `one`'s disk read back once its table is written
/// (`plan::read_back`), looked at again a little while as the kernel and
/// udev catch up: each partition at its place, each kept one with its
/// filesystem's UUID, nothing else, and every path a block device. Gives
/// each partition's path now; `DiskChanged` where the disk never comes to
/// say what the layout leaves.
fn read_back(one: &DiskLayout, machine: &mut dyn Machine) -> Result<Paths, Stop> {
    let disk = one.disk.path.as_str();
    let partitions = Command::new(
        "lsblk",
        &[
            "--json",
            "--bytes",
            "--paths",
            "-o",
            "NAME,START,SIZE,PARTN",
            disk,
        ],
    );
    let mut last = String::new();
    for attempt in 0..READ_BACK_TRIES {
        if attempt > 0 {
            machine.pause();
        }
        let kernel =
            table::parse_partitions(&machine.read(&partitions)?, disk).map_err(internal)?;
        let mut uuids = BTreeMap::new();
        for found in &kernel {
            if let Ok(uuid) = machine.read(&blkid(&found.path, "UUID")) {
                uuids.insert(found.path.clone(), uuid.trim().to_string());
            }
        }
        match plan::read_back(one, &kernel, &|path| uuids.get(path).cloned()) {
            Ok(paths) => {
                let mut ready = true;
                for (_, path) in paths.iter() {
                    ready &= machine.is_block_device(path);
                }
                if ready {
                    return Ok(paths);
                }
                last = "a partition has no device node yet".to_string();
            }
            Err(why) => last = why,
        }
    }
    Err(changed(format!(
        "{disk} is not as the layout leaves it: {last}"
    )))
}

/// What `blkid -p` reads of every partition `layout` uses, at its path now:
/// its filesystem's UUID, which must be read, and its type (none where
/// blkid finds nothing). A partition kept as it is — a `/home` from
/// before, an EFI system partition, one at a path of one's own, on a disk
/// written to or only mounted from — must still hold the filesystem it was
/// reviewed with, where it was reviewed with a UUID: that UUID, which
/// nothing in the run changes and the fstab names it by (compared without
/// case, `disks::same_uuid`); else `DiskChanged`, before anything is
/// mounted. Whether each holds what it must is the fstab's to judge
/// (`mounts::lines`).
fn read_facts(
    layout: &[DiskLayout],
    paths: &Paths,
    machine: &mut dyn Machine,
) -> Result<Facts, Stop> {
    let mut found = Vec::new();
    for one in used(layout) {
        let device = paths.of(&one.which).ok_or_else(|| {
            internal(format!(
                "what is used at {} was not found again",
                one.at.path()
            ))
        })?;
        let uuid = machine.read(&blkid(device, "UUID"))?.trim().to_string();
        if let (None, Some(was)) = (one.made, one.was) {
            if !was.uuid.is_empty() && !same_uuid(&uuid, &was.uuid) {
                return Err(changed(format!(
                    "{device}, kept at {}, holds the filesystem {uuid}, not the {} it was \
                     reviewed with",
                    one.at.path(),
                    was.uuid
                )));
            }
        }
        let fstype = match machine.read(&blkid(device, "TYPE")) {
            Ok(fstype) => fstype.trim().to_string(),
            Err(stop) if stop.failure == Failure::CommandFailed => String::new(),
            Err(stop) => return Err(stop),
        };
        found.push(Fact {
            which: one.which.clone(),
            device: device.to_string(),
            uuid,
            fstype,
        });
    }
    Ok(Facts { found })
}

/// **The layout's storage, phases A to G** (see the module), below `stage`
/// (an empty private directory the caller made): reports `Step::Partition`,
/// `Step::Format` and `Step::Swap` through `report`, as the easy path's
/// storage half does, and gives the [`Mounted`] the system half reads.
/// Phase A runs before `Step::Partition` begins, so a refusal there stops
/// in Check: nothing was written.
pub fn prepare(
    config: &InstallConfig,
    firmware: Firmware,
    stage: &Path,
    machine: &mut dyn Machine,
    report: &mut dyn FnMut(Event),
) -> Result<Mounted, Stop> {
    let laid = layout_of(config)?;
    let dirs = Stage::new(stage);
    let mut steps = Steps::new(report);
    let swap_mib = config.storage.swap_mib;

    // Phase A: nothing written.
    let tables = tables_as_reviewed(laid, machine)?;
    kept_as_reviewed(laid, machine)?;
    let ctx = Context::new(laid, firmware);
    let mut written = Vec::new();
    for one in by_path(laid) {
        if !plan::writes_to(one) {
            continue;
        }
        let path = one.disk.path.as_str();
        let commands = plan::table_commands(ctx, one, tables.get(path), path).map_err(internal)?;
        written.push((one, commands));
    }
    let mut fence = Fence::new(laid, &tables);
    // Every filesystem shrunk read again, the rule across every disk with
    // it, just before anything is written; then its shrink readied from
    // what was read, and dry-run.
    let shrunk = shrinking::read_again(laid, &fence, machine, &mut steps)?;
    measured_again(config, laid, &dirs, machine, &mut steps, shrunk.clone())?;
    let shrinks: Vec<&Planned> = written
        .iter()
        .flat_map(|(_, commands)| commands)
        .filter(|planned| planned.act() == Act::ShrinkFilesystem)
        .collect();
    let readied = shrinking::ready(laid, &shrinks, &shrunk, &mut fence, machine, &mut steps)?;
    shrinking::dry_runs(&readied, machine, &mut steps)?;
    for planned in written.iter().flat_map(|(_, commands)| commands) {
        steps.log(format!("layout: {}", planned.shown()));
    }

    // Phase B.1: every filesystem shrunk, before any entry is changed on
    // any disk.
    if !readied.is_empty() {
        steps.begin(Step::Resize, shrinking::units(&readied));
        shrinking::carry_out(&readied, &mut fence, machine, &mut steps)?;
    }

    // Phase B: the tables, disk by disk — a filesystem's shrink, done
    // already, runs nothing here.
    for (_, commands) in &mut written {
        commands.retain(|planned| planned.act() != Act::ShrinkFilesystem);
    }
    let units = written
        .iter()
        .map(|(_, commands)| commands.len() as u64 + 1)
        .sum();
    steps.begin(Step::Partition, units);
    let mut paths = Paths::default();
    for one in by_path(laid) {
        if !plan::writes_to(one) {
            paths.extend(Paths::as_reviewed(one));
        }
    }
    for (one, commands) in &written {
        let disk = one.disk.path.as_str();
        for (index, planned) in commands.iter().enumerate() {
            if index > 0 {
                // Each sfdisk call reads the table again; udev finishes with
                // the last one before the next takes the lock.
                machine.settle(Some(disk));
            }
            destroy(planned, &mut fence, machine)?;
            steps.advance();
        }
        machine.settle(Some(disk));
        if one.table.is_none() && one.disk.table == Some(Table::Msdos) {
            let after = read_table(machine, disk)?;
            if let Some(flag) = plan::activation(one, &after, disk) {
                fence.written_table(disk, &after);
                steps.log(format!("layout: {}", flag.shown()));
                destroy(&flag, &mut fence, machine)?;
                machine.settle(Some(disk));
            }
        }
        paths.extend(read_back(one, machine)?);
        steps.advance();
    }
    fence.read_back(laid, &paths);
    for (which, path) in paths.iter() {
        steps.log(format!("layout: {which:?} is {path}"));
    }

    // Phase C: the filesystems.
    let formats = plan::format_commands(laid, &paths).map_err(internal)?;
    let table = mounts::table(laid, swap_mib).map_err(internal)?;
    steps.begin(Step::Format, (formats.len() + table.len()) as u64 + 2);
    for planned in &formats {
        steps.log(format!("layout: {}", planned.shown()));
        destroy(planned, &mut fence, machine)?;
        steps.advance();
    }
    machine.settle(None);
    let facts = read_facts(laid, &paths, machine)?;
    for fact in &facts.found {
        steps.log(format!(
            "layout: {} holds {} UUID {}",
            fact.device, fact.fstype, fact.uuid
        ));
    }
    steps.advance();

    // Phase D: Btrfs' subvolumes.
    let (root, filesystem) = root_filesystem(laid).map_err(internal)?;
    if filesystem == Filesystem::Btrfs {
        let device = facts
            .of(&root.which)
            .ok_or_else(|| internal("nothing was read of /"))?
            .device
            .clone();
        let names: Vec<&str> = mounts::subvolumes(laid, swap_mib)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        machine.subvolumes(&names, &device, &dirs.top)?;
    }
    steps.advance();

    // Phase E: the mounts, each parent before what is inside it.
    machine.make_root(&dirs.root)?;
    for mount in &table {
        machine.mount_dir(&dirs.root, &mount.at)?;
        if mount.during {
            let device = &facts
                .of(&mount.which)
                .ok_or_else(|| internal(format!("nothing was read of {}", mount.at)))?
                .device;
            let options = mount.run_options();
            machine.mount(
                &mount.fstype,
                options.as_deref(),
                device,
                &dirs.root,
                &mount.at,
            )?;
            if let Some(mode) = mounts::mode_after_mount(&mount.at) {
                machine.set_mode(&dirs.root, &mount.at, mode)?;
            }
        }
        steps.advance();
    }

    // Phase F: the swap file, never beside a swap partition.
    let swapfile = mounts::swapfile(laid, swap_mib).map_err(internal)?;
    if let Some(path) = swapfile {
        let swap = Swapfile {
            path,
            mib: swap_mib,
        };
        machine.swapfile(swap, filesystem, &dirs.root, &mut steps)?;
    }

    // Phase G: the fstab, and what the system half reads.
    let lines = mounts::lines(&table, &mounts::swaps(laid), &facts).map_err(internal)?;
    let fstab = mounts::fstab(&lines, swapfile);
    for line in fstab
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        steps.log(format!("fstab: {line}"));
    }
    mounted(
        laid,
        firmware,
        &facts,
        &paths,
        dirs.root.clone(),
        swap_mib,
        fstab,
    )
    .map_err(internal)
}

/// **At Finish, the new account's numbers in the fstab** (§7.0, Unit Q,
/// "The worker" 6): where the installed system's `/etc/fstab` holds the
/// places for them (`mounts::UID`, `mounts::GID` — the lines of FAT, exFAT
/// and NTFS partitions at paths of one's own), they are filled in from its
/// own `/etc/passwd`, so those partitions are the person's to write. An
/// fstab without them is left as it is. Run before the home is given back.
pub fn finish(root: &Path, user: &str) -> Result<(), Stop> {
    let Some(fstab) = target::read(root, "etc/fstab")? else {
        return Err(internal("the installed system has no /etc/fstab"));
    };
    if !fstab.contains(mounts::UID) && !fstab.contains(mounts::GID) {
        return Ok(());
    }
    let passwd = target::read(root, "etc/passwd")?.unwrap_or_default();
    let (uid, gid) = mounts::ids(&passwd, user).ok_or_else(|| {
        internal(format!(
            "{user} is not in the installed system's /etc/passwd"
        ))
    })?;
    target::write(
        root,
        "etc/fstab",
        &mounts::fill_ids(&fstab, uid, gid),
        0o644,
    )?;
    crate::worker::run::log(&format!(
        "layout: the fstab gives {user}'s partitions to {uid}:{gid}"
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::{Desktop, Storage};
    use crate::model::disks::{Disk, MIB};
    use crate::model::layout::{Capability, Fate, PretendProven};
    use crate::model::protocol::Warning;
    use crate::worker::manual::fence::tests::Kernel as Sys;
    use crate::worker::manual::machine::Machine;
    use crate::worker::manual::matrix::{after_writing, cases, filling, on_paper, HOME_UUID};
    use crate::worker::measure::{Head, HIBERFILE_HEAD};
    use crate::worker::partition::fixtures::{self, layout_request, part_path};
    use crate::worker::partition::table::Kernel;
    use crate::worker::quiet::{Effect, Hands};
    use crate::worker::run::Status;
    use crate::worker::system::check::DeviceFacts;
    use crate::worker::system::image;
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::rc::Rc;

    const EVERY: &[Capability] = layout::EVERY_CAPABILITY;
    /// One set holding every capability, as if a scenario had needed them
    /// all: every layout lies within it.
    const EVERY_SET: &[&[Capability]] = &[EVERY];

    /// What a machine written by hand was asked, line by line, and which of
    /// the questions that can fail is made to (counted from 0).
    #[derive(Default)]
    struct Said {
        lines: Vec<String>,
        asked: usize,
        fail_at: Option<usize>,
        /// The question made to fail, and where it stands among `lines`.
        failed: Option<(String, usize)>,
        /// Every question asked, in order.
        questions: Vec<String>,
        /// What the udisks rule's hands were given, in order.
        effects: Vec<Effect>,
        /// The question at which the run panics, as a bug would.
        panic_at: Option<String>,
        /// Each bounded reading asked, and the time it was given.
        limits: Vec<(String, std::time::Duration)>,
    }

    impl Said {
        /// `line` written down as a question that can fail: `Err` when it
        /// is the one made to.
        fn ask(&mut self, line: String) -> Result<(), Stop> {
            self.lines.push(line.clone());
            self.questions.push(line.clone());
            if self.panic_at.as_ref() == Some(&line) {
                panic!("a test's own panic, at {line:?}");
            }
            let number = self.asked;
            self.asked += 1;
            if self.fail_at == Some(number) {
                self.failed = Some((line.clone(), self.lines.len() - 1));
                return Err(Stop::new(
                    Failure::CommandFailed,
                    format!("made to fail: {line}"),
                ));
            }
            Ok(())
        }

        /// `line` written down as something that cannot fail.
        fn note(&mut self, line: String) {
            self.lines.push(line);
        }
    }

    type Shared = Rc<RefCell<Said>>;

    /// The udisks rule's hands: each effect written down, nothing done.
    struct Rule(Shared);

    impl Hands for Rule {
        fn apply(&mut self, effect: &Effect) -> Result<(), Stop> {
            let line = match effect {
                Effect::Write(_) => "rule write".to_string(),
                Effect::Remove => "rule remove".to_string(),
                Effect::Run(command) => format!("rule udevadm {}", command.args[0]),
            };
            let mut said = self.0.borrow_mut();
            said.effects.push(effect.clone());
            said.ask(line)
        }
    }

    /// A command as one line.
    fn line(command: &Command) -> String {
        let mut text = command.program.to_string();
        for arg in &command.args {
            text.push(' ');
            text.push_str(arg);
        }
        text
    }

    /// What the hand-written machine's filesystem tools find on one
    /// partition: a filesystem `bytes` long (an NTFS by its boot sector's
    /// count), `used` of it in use, the least its tool shrinks it to, put
    /// away cleanly (or, for an NTFS, not marked for Windows' check) or not;
    /// for an NTFS, what ntfscat prints of its `hiberfil.sys`, and what
    /// ntfsresize counts of it where not as it does (`counted`, [`info`]).
    ///
    /// [`info`]: Fsys::info
    #[derive(Clone, Debug)]
    struct Fsys {
        bytes: u64,
        used: u64,
        least: u64,
        clean: bool,
        sign: Head,
        counted: Option<u64>,
    }

    impl Fsys {
        /// An ext4 filling a partition of `size` bytes, 4 GiB of it in use.
        fn ext4(size: u64) -> Fsys {
            Fsys {
                bytes: size,
                used: 4 * crate::model::disks::GIB,
                least: 4 * crate::model::disks::GIB + 256 * MIB,
                clean: true,
                sign: Head::default(),
                counted: None,
            }
        }

        /// Windows' NTFS on a partition of `size` bytes, its volume a sector
        /// short of it, 4 GB in use, shut down properly with hibernation on:
        /// a `hiberfil.sys` of its own beginning `wake`.
        fn windows(size: u64) -> Fsys {
            Fsys {
                bytes: size - 512,
                used: 4_000_000_000,
                least: 4_500_000_000,
                clean: true,
                sign: Fsys::hiberfile(b"wake"),
                counted: None,
            }
        }

        /// The whole header ntfscat prints of a `hiberfil.sys` beginning
        /// `first`, then ended by the starter.
        fn hiberfile(first: &[u8]) -> Head {
            let mut bytes = first.to_vec();
            bytes.resize(HIBERFILE_HEAD, 0);
            Head {
                bytes,
                ended: None,
                stderr: String::new(),
            }
        }

        /// What `dumpe2fs -h` prints of it: one not put away cleanly as
        /// e2fsprogs 1.47.4 printed an ext4 shut down with its journal
        /// unplayed on the live system (QEMU's `probe-resize-tools`) — its
        /// state still `clean`, and `needs_recovery` among its features.
        fn header(&self) -> String {
            format!(
                "Filesystem volume name:   Data\n\
                 Filesystem features:      has_journal ext_attr extent 64bit{}\n\
                 Filesystem state:         clean\n\
                 Block count:              {}\n\
                 Free blocks:              {}\n\
                 Block size:               4096\n",
                if self.clean { "" } else { " needs_recovery" },
                self.bytes / 4096,
                (self.bytes - self.used) / 4096
            )
        }

        /// What `ntfsresize --info` prints of it: the volume counted as
        /// ntfsresize counts one — its 4096-byte clusters, as many as the
        /// boot sector's count holds whole, and one sector more for the
        /// backup boot sector (its `vol_size`, seen on ntfsprogs 2026.9.28
        /// by the NTFS side-file test) — unless `counted` says otherwise.
        fn info(&self) -> Status {
            if !self.clean {
                return ended(1, "", "ERROR: Volume is scheduled for check.\n");
            }
            let counted = self.counted.unwrap_or(self.bytes / 4096 * 4096 + 512);
            ended(
                0,
                &format!(
                    "Current volume size: {} bytes ({} MB)\n\
                     Space in use       : {} MB (13.3%)\n\
                     You might resize at {} bytes or {} MB (freeing 1 MB).\n",
                    counted,
                    counted / 1_000_000,
                    self.used / 1_000_000,
                    self.least,
                    self.least / 1_000_000
                ),
                "",
            )
        }

        /// Its first sector: an NTFS boot sector saying its volume's size.
        fn boot(&self) -> Vec<u8> {
            let mut boot = vec![0u8; 512];
            boot[3..11].copy_from_slice(b"NTFS    ");
            boot[0x0b..0x0d].copy_from_slice(&512u16.to_le_bytes());
            boot[0x28..0x30].copy_from_slice(&(self.bytes / 512).to_le_bytes());
            boot
        }
    }

    fn ended(code: i32, stdout: &str, stderr: &str) -> Status {
        Status {
            code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    /// A machine written down by hand: it answers from the layout's own
    /// disks — as reviewed until a disk is written, then as
    /// `matrix::after_writing` has it — writes down every question, and
    /// runs nothing.
    struct Hand {
        said: Shared,
        laid: Vec<DiskLayout>,
        found: Vec<Disk>,
        before: BTreeMap<String, OnDisk>,
        after: BTreeMap<String, (OnDisk, Vec<Kernel>)>,
        wrote: BTreeSet<String>,
        made: BTreeMap<String, &'static str>,
        sys: Sys,
        mounts: String,
        esp_free: u64,
        /// The sector size the kernel reports now, where it is another.
        sector_now: Option<u32>,
        /// Where the kernel places a device (its link and its start and
        /// size), by its name, where that is not where the layout has it.
        moved: BTreeMap<String, (String, Option<(u64, u64)>)>,
        /// The filesystem UUID blkid reads of a device, where it is not
        /// what this machine's disks otherwise hold.
        uuid_read: BTreeMap<String, String>,
        /// The same, but only once a disk has been written to: a
        /// filesystem changed behind the run's back after phase A.
        uuid_read_later: BTreeMap<String, String>,
        /// What the filesystem tools find on each partition read before a
        /// shrink, by its device — every Windows NTFS and every ext4 shrunk
        /// of the layout's disks, and those a test puts on other disks.
        filesystems: BTreeMap<String, Fsys>,
        /// How a command line ends, where not as the filesystem it reads or
        /// shrinks would have it.
        ending: BTreeMap<String, Status>,
        /// The programs this live system does not have: asked to run one,
        /// it answers as `run::command_path` does, `MissingTool`.
        missing: Vec<&'static str>,
        /// The machine's clock, which only its commands move on: when it
        /// began, and how long it has run since.
        began: std::time::Instant,
        clock: std::time::Duration,
        /// How long a command line takes, where it takes any time at all.
        takes: BTreeMap<String, std::time::Duration>,
    }

    impl Hand {
        fn new(laid: &[DiskLayout], firmware: Firmware) -> Hand {
            let ctx = Context::new(laid, firmware);
            let mut before = BTreeMap::new();
            let mut after = BTreeMap::new();
            for one in laid {
                if one.disk.table.is_some() {
                    before.insert(one.disk.path.clone(), fixtures::on_disk(&one.disk));
                }
                if plan::writes_to(one) {
                    let written = after_writing(ctx, one, before.get(&one.disk.path));
                    after.insert(one.disk.path.clone(), written);
                }
            }
            let mut hand = Hand {
                said: Rc::default(),
                laid: laid.to_vec(),
                found: laid.iter().map(|one| one.disk.clone()).collect(),
                before,
                after,
                wrote: BTreeSet::new(),
                made: BTreeMap::new(),
                sys: Sys::default(),
                mounts: "22 1 0:21 / /proc rw - proc proc rw\n".into(),
                esp_free: 64 * MIB,
                sector_now: None,
                moved: BTreeMap::new(),
                uuid_read: BTreeMap::new(),
                uuid_read_later: BTreeMap::new(),
                filesystems: BTreeMap::new(),
                ending: BTreeMap::new(),
                missing: Vec::new(),
                began: std::time::Instant::now(),
                clock: std::time::Duration::ZERO,
                takes: BTreeMap::new(),
            };
            for one in laid {
                for (part, was) in one.parts.iter().zip(&one.disk.partitions) {
                    let shrunk = matches!(part.fate, Fate::Shrink { .. });
                    let fsys = match (was.fs_type.as_str(), was.content) {
                        ("ntfs", crate::model::disks::Content::Windows) => Fsys::windows(was.size),
                        ("ext4", _) if shrunk => Fsys::ext4(was.size),
                        _ => continue,
                    };
                    hand.filesystems.insert(was.path.clone(), fsys);
                }
            }
            hand.sys = hand.kernel_now();
            hand
        }

        /// How the filesystem tools' command `command`, which reads, ends.
        fn tool_said(&self, command: &Command) -> Status {
            let said = line(command);
            if let Some(status) = self.ending.get(&said) {
                return status.clone();
            }
            let device = command.args.last().cloned().unwrap_or_default();
            let fsys = self
                .filesystems
                .get(&device)
                .unwrap_or_else(|| panic!("the hand-written machine has no filesystem for {said}"));
            match command.program {
                "dumpe2fs" => ended(0, &fsys.header(), "dumpe2fs 1.47.4 (6-Mar-2025)\n"),
                "resize2fs" => ended(
                    0,
                    &format!(
                        "Estimated minimum size of the filesystem: {}\n",
                        fsys.least / 4096
                    ),
                    "",
                ),
                // `-f -n` passes an ext4 whose journal waits to be played
                // back, as the live system's did: it skips the journal.
                "e2fsck" if fsys.clean => ended(0, "", ""),
                "e2fsck" => ended(
                    0,
                    "Warning: skipping journal recovery because doing a read-only filesystem check.\n",
                    "",
                ),
                "ntfsresize" if command.args[0] == "--info" => fsys.info(),
                "ntfsresize" if fsys.clean => ended(
                    0,
                    "Checking filesystem consistency ...\n\
                     The read-only test run ended successfully.\n",
                    "",
                ),
                "ntfsresize" => ended(1, "", "ERROR: Volume is scheduled for check.\n"),
                _ => panic!("the hand-written machine was asked to read {said}"),
            }
        }

        /// `MissingTool` where `command`'s program is one this live system
        /// does not have.
        fn has(&self, command: &Command) -> Result<(), Stop> {
            if self.missing.contains(&command.program) {
                return Err(Stop::new(
                    Failure::MissingTool,
                    format!("required command is missing: {}", command.program),
                ));
            }
            Ok(())
        }

        /// `command` run within `limit` on this machine's clock: the time it
        /// takes passes, and where that is more than `limit`, only `limit`
        /// does and it is ended — `CommandFailed`, as the real starters say.
        fn within(&mut self, command: &Command, limit: std::time::Duration) -> Result<(), Stop> {
            let said = line(command);
            self.said.borrow_mut().limits.push((said.clone(), limit));
            let takes = self.takes.get(&said).copied().unwrap_or_default();
            if takes > limit {
                self.clock += limit;
                return Err(Stop::new(
                    Failure::CommandFailed,
                    format!("{said}: did not end within {} seconds", limit.as_secs()),
                ));
            }
            self.clock += takes;
            Ok(())
        }

        fn failing_at(self, question: usize) -> Hand {
            self.said.borrow_mut().fail_at = Some(question);
            self
        }

        /// What the kernel says now: each disk, and its partitions as
        /// reviewed or, once it was written, as written.
        fn kernel_now(&self) -> Sys {
            let mut sys = Sys::default();
            for one in &self.laid {
                let disk = one.disk.path.trim_start_matches("/dev/");
                sys.disk(disk);
                let parts: Vec<(String, u64, u64)> = match self.after.get(&one.disk.path) {
                    Some((_, kernel)) if self.wrote.contains(&one.disk.path) => kernel
                        .iter()
                        .map(|k| (k.path.clone(), k.start, k.size))
                        .collect(),
                    _ => one
                        .disk
                        .partitions
                        .iter()
                        .map(|p| (p.path.clone(), p.start, p.size))
                        .collect(),
                };
                for (path, start, size) in parts {
                    sys.part(disk, path.trim_start_matches("/dev/"), start, size);
                }
            }
            for (name, place) in &self.moved {
                sys.devices.insert(name.clone(), place.clone());
            }
            sys
        }

        fn disk(&self, path: &str) -> Option<&DiskLayout> {
            self.laid.iter().find(|one| one.disk.path == path)
        }

        /// The kernel's partitions of `disk` now.
        fn partitions(&self, disk: &DiskLayout) -> Vec<Kernel> {
            match self.after.get(&disk.disk.path) {
                Some((_, kernel)) if self.wrote.contains(&disk.disk.path) => kernel.clone(),
                _ => disk
                    .disk
                    .partitions
                    .iter()
                    .map(|p| Kernel {
                        path: p.path.clone(),
                        number: p.number,
                        start: p.start,
                        size: p.size,
                    })
                    .collect(),
            }
        }

        /// A filesystem UUID made up from a device's path.
        fn uuid(device: &str) -> String {
            let hash = device.bytes().fold(0x811c_9dc5_u32, |h, b| {
                (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
            });
            format!("{hash:08x}-0000-4000-8000-000000000000")
        }

        /// What `blkid -p -s UUID` reads of `device`: a new UUID for a
        /// filesystem this run made, or one it held when reviewed —
        /// reviewed with one, that one; else one made up from its path.
        fn uuid_now(&self, device: &str) -> String {
            let later = self.uuid_read_later.get(device);
            if let Some(uuid) = later.filter(|_| !self.wrote.is_empty()) {
                return uuid.clone();
            }
            if let Some(uuid) = self.uuid_read.get(device) {
                return uuid.clone();
            }
            let reviewed = self
                .laid
                .iter()
                .flat_map(|one| &one.disk.partitions)
                .find(|part| part.path == device)
                .map(|part| part.uuid.clone())
                .filter(|uuid| !uuid.is_empty());
            match reviewed {
                Some(uuid) if !self.made.contains_key(device) => uuid,
                _ => Hand::uuid(device),
            }
        }

        /// What `blkid -p -s TYPE` reads of `device`: what this run made on
        /// it, or what it held when reviewed.
        fn fstype(&self, device: &str) -> Option<String> {
            if let Some(made) = self.made.get(device) {
                return Some(made.to_string());
            }
            self.laid
                .iter()
                .flat_map(|one| &one.disk.partitions)
                .find(|part| part.path == device)
                .map(|part| part.fs_type.clone())
                .filter(|fs| !fs.is_empty())
        }

        fn answer(&self, command: &Command) -> Result<String, Stop> {
            let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
            let none = || {
                Stop::new(
                    Failure::CommandFailed,
                    format!("{} found nothing", line(command)),
                )
            };
            match (command.program, args.as_slice()) {
                ("lsblk", [.., "--nodeps", "-o", "NAME,SIZE,LOG-SEC", path]) => {
                    let disk = &self.disk(path).ok_or_else(none)?.disk;
                    Ok(format!(
                        r#"{{"blockdevices":[{{"name":"{}","size":{},"log-sec":{}}}]}}"#,
                        disk.path,
                        disk.size,
                        self.sector_now.unwrap_or(disk.logical_sector)
                    ))
                }
                ("lsblk", [.., "NAME,START,SIZE,PARTN", path]) => {
                    let one = self.disk(path).ok_or_else(none)?;
                    let children: Vec<String> = self
                        .partitions(one)
                        .iter()
                        .map(|k| {
                            format!(
                                r#"{{"name":"{}","start":{},"size":{},"partn":{}}}"#,
                                k.path,
                                k.start / 512,
                                k.size,
                                k.number
                            )
                        })
                        .collect();
                    Ok(format!(
                        r#"{{"blockdevices":[{{"name":"{path}","children":[{}]}}]}}"#,
                        children.join(",")
                    ))
                }
                ("sfdisk", ["--json", path]) => {
                    let table = match self.after.get(*path) {
                        Some((after, _)) if self.wrote.contains(*path) => after,
                        _ => self.before.get(*path).ok_or_else(none)?,
                    };
                    let label = match table.table {
                        Table::Gpt => "gpt",
                        Table::Msdos => "dos",
                    };
                    let entries: Vec<String> = table
                        .partitions
                        .iter()
                        .map(|e| {
                            format!(
                                r#"{{"node":"{}","start":{},"size":{},"type":"{}","bootable":{}}}"#,
                                part_path(path, e.number),
                                e.start,
                                e.size,
                                e.part_type,
                                e.bootable
                            )
                        })
                        .collect();
                    Ok(format!(
                        r#"{{"partitiontable":{{"label":"{label}","sectorsize":{},"partitions":[{}]}}}}"#,
                        table.sector.unwrap_or(512),
                        entries.join(",")
                    ))
                }
                ("blkid", ["-p", "-s", "UUID", "-o", "value", device]) => {
                    match self.fstype(device) {
                        Some(_) => Ok(format!("{}\n", self.uuid_now(device))),
                        None => Err(none()),
                    }
                }
                ("blkid", ["-p", "-s", "TYPE", "-o", "value", device]) => self
                    .fstype(device)
                    .map(|fs| format!("{fs}\n"))
                    .ok_or_else(none),
                _ => panic!(
                    "the hand-written machine was asked to read {}",
                    line(command)
                ),
            }
        }
    }

    impl Machine for Hand {
        fn discover(&mut self) -> Result<Vec<Disk>, String> {
            let asked = self.said.borrow_mut().ask("discover".into());
            asked
                .map(|()| self.found.clone())
                .map_err(|stop| stop.detail)
        }

        fn session_mounts(&mut self) -> Result<String, String> {
            let asked = self.said.borrow_mut().ask("mounts".into());
            asked
                .map(|()| self.mounts.clone())
                .map_err(|stop| stop.detail)
        }

        fn swaps(&mut self) -> String {
            self.said.borrow_mut().note("swaps".into());
            "Filename\tType\tSize\tUsed\tPriority\n".into()
        }

        fn device_facts(&mut self, device: &str) -> DeviceFacts {
            let asked = self.said.borrow_mut().ask(format!("facts {device}"));
            asked
                .map(|()| ("254:250".to_string(), false))
                .map_err(|stop| stop.detail)
        }

        fn online(&mut self, _: &InstallConfig, _: &Image) -> Result<bool, Stop> {
            self.said.borrow_mut().ask("online".into()).map(|()| false)
        }

        fn udev(&mut self) -> Box<dyn Hands> {
            Box::new(Rule(Rc::clone(&self.said)))
        }

        fn stage(&mut self) -> Result<PathBuf, Stop> {
            self.said.borrow_mut().ask("stage".into())?;
            Ok("/nonexistent-lxb-stage".into())
        }

        fn read(&mut self, command: &Command) -> Result<String, Stop> {
            self.said
                .borrow_mut()
                .ask(format!("read {}", line(command)))?;
            self.answer(command)
        }

        fn read_status(&mut self, command: &Command) -> Result<Status, Stop> {
            assert!(
                shrinking::allowed(command) && !shrinking::only_its_head(command),
                "phase A was made to read {}",
                line(command)
            );
            self.said
                .borrow_mut()
                .ask(format!("read {}", line(command)))?;
            self.has(command)?;
            Ok(self.tool_said(command))
        }

        fn head(&mut self, command: &Command) -> Result<Head, Stop> {
            assert!(
                shrinking::allowed(command) && shrinking::only_its_head(command),
                "phase A was made to read the head of {}",
                line(command)
            );
            self.said
                .borrow_mut()
                .ask(format!("head {}", line(command)))?;
            self.has(command)?;
            let device = &command.args[0];
            let fsys = self.filesystems.get(device).expect("an NTFS");
            if !fsys.clean {
                // ntfscat refuses a volume marked for Windows' check.
                return Ok(Head {
                    bytes: Vec::new(),
                    ended: Some(false),
                    stderr: "Volume is scheduled for check.\n".into(),
                });
            }
            Ok(fsys.sign.clone())
        }

        fn read_status_within(
            &mut self,
            command: &Command,
            limit: std::time::Duration,
        ) -> Result<Status, Stop> {
            self.within(command, limit)?;
            self.read_status(command)
        }

        fn head_within(
            &mut self,
            command: &Command,
            limit: std::time::Duration,
        ) -> Result<Head, Stop> {
            self.within(command, limit)?;
            self.head(command)
        }

        fn now(&mut self) -> std::time::Instant {
            self.began + self.clock
        }

        fn write_status(&mut self, command: &Command) -> Result<Status, Stop> {
            self.said
                .borrow_mut()
                .ask(format!("write {}", line(command)))?;
            let said = line(command);
            if let Some(status) = self.ending.get(&said) {
                return Ok(status.clone());
            }
            let device = command.args.last().cloned().unwrap_or_default();
            match (command.program, command.args.as_slice()) {
                ("e2fsck", [.., device]) => {
                    // `-f -y` plays a waiting journal back and ends 1, as
                    // the live system's did; on a clean one, 0.
                    let fsys = self.filesystems.get_mut(device).expect("an ext4");
                    if fsys.clean {
                        return Ok(ended(0, "", ""));
                    }
                    fsys.clean = true;
                    Ok(ended(
                        1,
                        "Data: recovering journal\n\nData: ***** FILE SYSTEM WAS MODIFIED *****\n",
                        "",
                    ))
                }
                ("resize2fs", [device, size]) => {
                    let kib: u64 = size.trim_end_matches('K').parse().unwrap();
                    let fsys = self.filesystems.get_mut(device).expect("an ext4");
                    fsys.bytes = kib * 1024 / 4096 * 4096;
                    Ok(ended(
                        0,
                        &format!(
                            "The filesystem on {device} is now {} (4k) blocks long.\n",
                            fsys.bytes / 4096
                        ),
                        "",
                    ))
                }
                ("ntfsresize", [.., flag, size, _]) if flag == "--size" => {
                    let size: u64 = size.parse().unwrap();
                    let fsys = self.filesystems.get_mut(&device).expect("an NTFS");
                    if !fsys.clean {
                        return Ok(ended(1, "", "ERROR: Volume is scheduled for check.\n"));
                    }
                    // A sector short of what it was given, and marked for
                    // Windows' own check.
                    fsys.bytes = size - 4096;
                    fsys.clean = false;
                    Ok(ended(
                        0,
                        &format!("Successfully resized NTFS on device '{device}'.\n"),
                        "",
                    ))
                }
                _ => panic!("the hand-written machine was asked to shrink by {said}"),
            }
        }

        fn first_sector(&mut self, device: &str) -> Result<Vec<u8>, Stop> {
            self.said.borrow_mut().ask(format!("sector {device}"))?;
            Ok(self.filesystems.get(device).expect("an NTFS").boot())
        }

        fn sysfs(&self) -> &dyn crate::worker::manual::fence::Sysfs {
            self.said.borrow_mut().note("fence".into());
            &self.sys
        }

        fn claim(&mut self, device: &str) -> Result<(), Stop> {
            self.said.borrow_mut().ask(format!("claim {device}"))
        }

        fn write(&mut self, command: &Command) -> Result<(), Stop> {
            self.said
                .borrow_mut()
                .ask(format!("write {}", line(command)))?;
            let disk = command
                .args
                .iter()
                .find(|arg| self.disk(arg).is_some())
                .cloned();
            if let Some(disk) = disk {
                self.wrote.insert(disk);
                self.sys = self.kernel_now();
            }
            let made = match command.program {
                "mkfs.ext4" => Some("ext4"),
                "mkfs.btrfs" => Some("btrfs"),
                "mkfs.xfs" => Some("xfs"),
                "mkfs.fat" => Some("vfat"),
                "mkswap" => Some("swap"),
                _ => None,
            };
            if let (Some(fs), Some(device)) = (made, command.args.last()) {
                self.made.insert(device.clone(), fs);
            }
            Ok(())
        }

        fn settle(&mut self, disk: Option<&str>) {
            let said = format!("settle {}", disk.unwrap_or("all"));
            self.said.borrow_mut().note(said);
        }

        fn pause(&mut self) {
            self.said.borrow_mut().note("pause".into());
        }

        fn is_block_device(&mut self, path: &str) -> bool {
            self.said.borrow_mut().note(format!("node {path}"));
            true
        }

        fn esp_free(&mut self, device: &str, _: &Path) -> Result<u64, Stop> {
            self.said.borrow_mut().ask(format!("esp {device}"))?;
            Ok(self.esp_free)
        }

        fn image_size(&mut self, _: &Path, path: &str) -> Option<u64> {
            self.said.borrow_mut().note(format!("image {path}"));
            Some(0)
        }

        fn subvolumes(&mut self, names: &[&str], device: &str, _: &Path) -> Result<(), Stop> {
            let said = format!("subvolumes {} on {device}", names.join(" "));
            self.said.borrow_mut().ask(said)
        }

        fn make_root(&mut self, _: &Path) -> Result<(), Stop> {
            self.said.borrow_mut().ask("root".into())
        }

        fn mount_dir(&mut self, _: &Path, at: &str) -> Result<(), Stop> {
            self.said.borrow_mut().ask(format!("dir {at}"))
        }

        fn mount(
            &mut self,
            fstype: &str,
            options: Option<&str>,
            device: &str,
            _: &Path,
            at: &str,
        ) -> Result<(), Stop> {
            let said = format!("mount {fstype} {} {device} {at}", options.unwrap_or("-"));
            self.said.borrow_mut().ask(said)
        }

        fn set_mode(&mut self, _: &Path, at: &str, mode: u32) -> Result<(), Stop> {
            self.said.borrow_mut().ask(format!("mode {at} {mode:o}"))
        }

        fn swapfile(
            &mut self,
            swap: Swapfile,
            _: Filesystem,
            _: &Path,
            _: &mut Steps,
        ) -> Result<(), Stop> {
            let said = format!("swapfile {} {}", swap.path, swap.mib);
            self.said.borrow_mut().ask(said)
        }
    }

    /// The request for `laid`, its easy fields its pins.
    fn config_for(laid: Vec<DiskLayout>, swap_mib: u32) -> InstallConfig {
        let pins = layout::pins(&laid, swap_mib).expect("the layout pins");
        let mut config = fixtures::config(pins.disk, Target::Layout { disks: laid });
        config.storage = Storage {
            filesystem: pins.filesystem,
            swap_mib: pins.swap_mib,
            home: None,
        };
        config
    }

    /// The layout the storage fixtures' blank-disk request carries: a new
    /// gpt, the EFI partition alone at `/boot`, ext4 `/`.
    fn blank(sector: u32) -> Vec<DiskLayout> {
        let Target::Layout { disks } = layout_request(sector).config.target else {
            unreachable!()
        };
        disks
    }

    /// What one run came to.
    struct Ran {
        result: Result<(bool, Mounted), Stop>,
        said: Shared,
        quiet: Option<Quiet>,
        stage: Option<PathBuf>,
        events: Vec<Event>,
    }

    impl Ran {
        fn lines(&self) -> Vec<String> {
            self.said.borrow().lines.clone()
        }

        fn failure(&self) -> Failure {
            match &self.result {
                Ok(_) => panic!("the run went through"),
                Err(stop) => stop.failure,
            }
        }
    }

    /// `config` carried out on `hand`.
    fn run(config: &InstallConfig, firmware: Firmware, mut hand: Hand) -> Ran {
        let image = image::fixture(&[Desktop::Linexin], &[]);
        let said = Rc::clone(&hand.said);
        let mut events = Vec::new();
        let mut report = |event: Event| events.push(event);
        let mut stage = None;
        let mut quiet = None;
        let result = install_storage(
            config,
            &image,
            firmware,
            &mut hand,
            &mut report,
            &mut stage,
            &mut quiet,
        );
        Ran {
            result,
            said,
            quiet,
            stage,
            events,
        }
    }

    /// **Nothing that writes before every check has passed, and each write
    /// given leave** (the order the plan holds the executor to): the first
    /// claim and the first write come after the disks were found again
    /// (`discover`), each device asked whether it is in use (`facts`) and
    /// the internet (`online`); every claim straight after the fence was
    /// asked; every write straight after its claim — or, for a partition
    /// wiped or given a filesystem, after its claim and a look at the
    /// session's mounts, swaps and the partition's own facts.
    fn holds_the_order(lines: &[String], case: &str) {
        let first = |prefix: &str| lines.iter().position(|line| line.starts_with(prefix));
        let touched = lines
            .iter()
            .position(|line| line.starts_with("claim ") || line.starts_with("write "));
        if let Some(touched) = touched {
            for checked in ["discover", "facts ", "online"] {
                let at = first(checked).unwrap_or_else(|| panic!("{case}: no {checked}"));
                assert!(at < touched, "{case}: {checked} after a disk was touched");
            }
        }
        for (index, said) in lines.iter().enumerate() {
            if said.starts_with("claim ") {
                assert_eq!(
                    lines[index - 1],
                    "fence",
                    "{case}: {said} not after the fence"
                );
            }
            if let Some(command) = said.strip_prefix("write ") {
                let device = command
                    .split_whitespace()
                    .rfind(|word| word.starts_with("/dev/"))
                    .unwrap_or_default();
                let partition = command.starts_with("mkfs")
                    || command.starts_with("mkswap")
                    || command == format!("wipefs --all {device}")
                    || ["e2fsck ", "resize2fs ", "ntfsresize "]
                        .iter()
                        .any(|tool| command.starts_with(tool));
                if partition {
                    assert_eq!(
                        lines[index - 4..index],
                        [
                            format!("claim {device}"),
                            "mounts".to_string(),
                            "swaps".to_string(),
                            format!("facts {device}"),
                        ],
                        "{case}: {said}"
                    );
                } else {
                    assert!(
                        lines[index - 1].starts_with("claim /dev/"),
                        "{case}: {said} not straight after a claim"
                    );
                }
            }
        }
    }

    /// The udisks rule's removal: the last three things done.
    fn taken_out(ran: Ran, case: &str) {
        let said = Rc::clone(&ran.said);
        let before = said.borrow().lines.len();
        drop(ran);
        let lines = said.borrow().lines.clone();
        assert_eq!(
            lines[before..],
            [
                "rule remove",
                "rule udevadm control",
                "rule udevadm trigger"
            ],
            "{case}: the rule was not taken out once, when let go of"
        );
    }

    /// **The blank disk carried out, word for word** (Unit Q, Q5): the
    /// whole conversation with the machine, in order — the rule in, the
    /// disk found again and judged, nothing in use, the internet, the
    /// stage; phase A's look at the disk (its size, and no table on it);
    /// the wipe and the new table, each after the fence and its claim; the
    /// disk read back; each partition wiped and given its filesystem after
    /// the fence, its claim and a look at what is mounted; what blkid
    /// reads; the mounts; the swap file — and a `Mounted` the rest of the
    /// worker reads, GPT auto-discovery off. Nothing outside the
    /// conversation: the machine is written by hand and runs nothing.
    #[test]
    fn the_blank_disk_is_carried_out_in_the_plans_order() {
        let _proven = PretendProven::these(EVERY_SET);
        let laid = blank(512);
        let config = config_for(laid.clone(), 2048);
        let ran = run(&config, Firmware::Uefi, Hand::new(&laid, Firmware::Uefi));
        let (online, mounted) = ran.result.as_ref().expect("carried out");
        assert!(!online);
        assert_eq!(
            ran.stage.as_deref(),
            Some(Path::new("/nonexistent-lxb-stage"))
        );
        let script = "label: gpt\nunit: sectors\nsector-size: 512\n\
                      start=2048, size=2097152, type=c12a7328-f81f-11d2-ba4b-00a0c93ec93b, name=\"EFI system\"\n\
                      start=2099200, size=132116480, type=4f68bce3-e8cd-4db1-96e7-fbcaf984b709, name=\"Linexin\"\n";
        let lsblk_parts = "read lsblk --json --bytes --paths -o NAME,START,SIZE,PARTN /dev/vdx";
        let expected: Vec<String> = [
            "rule write",
            "rule udevadm control",
            "rule udevadm trigger",
            "rule udevadm settle",
            "discover",
            "mounts",
            "swaps",
            "facts /dev/vdx",
            "online",
            "stage",
            "read lsblk --json --bytes --paths --nodeps -o NAME,SIZE,LOG-SEC /dev/vdx",
            "read sfdisk --json /dev/vdx",
            "fence",
            "claim /dev/vdx",
            "write wipefs --all --lock /dev/vdx",
            "settle /dev/vdx",
            "fence",
            "claim /dev/vdx",
            "write sfdisk --lock --wipe always --wipe-partitions always /dev/vdx",
            "settle /dev/vdx",
            lsblk_parts,
            "read blkid -p -s UUID -o value /dev/vdx1",
            "read blkid -p -s UUID -o value /dev/vdx2",
            "node /dev/vdx1",
            "node /dev/vdx2",
            "fence",
            "claim /dev/vdx1",
            "mounts",
            "swaps",
            "facts /dev/vdx1",
            "write wipefs --all /dev/vdx1",
            "fence",
            "claim /dev/vdx1",
            "mounts",
            "swaps",
            "facts /dev/vdx1",
            "write mkfs.fat -F 32 /dev/vdx1",
            "fence",
            "claim /dev/vdx2",
            "mounts",
            "swaps",
            "facts /dev/vdx2",
            "write wipefs --all /dev/vdx2",
            "fence",
            "claim /dev/vdx2",
            "mounts",
            "swaps",
            "facts /dev/vdx2",
            "write mkfs.ext4 -F /dev/vdx2",
            "settle all",
            "read blkid -p -s UUID -o value /dev/vdx1",
            "read blkid -p -s TYPE -o value /dev/vdx1",
            "read blkid -p -s UUID -o value /dev/vdx2",
            "read blkid -p -s TYPE -o value /dev/vdx2",
            "root",
            "dir /",
            "mount ext4 - /dev/vdx2 /",
            "dir /boot",
            "mount vfat umask=0077 /dev/vdx1 /boot",
            "swapfile /swapfile 2048",
        ]
        .iter()
        .map(|line| line.to_string())
        .collect();
        assert_eq!(ran.lines(), expected);
        holds_the_order(&ran.lines(), "blank");
        // The new table's script is the one the matrix holds word for word.
        let paper = on_paper(Firmware::Uefi, &laid);
        assert_eq!(
            paper.tables[1].command().unwrap().input.as_deref(),
            Some(script)
        );
        assert_eq!(mounted.disk, "/dev/vdx");
        assert_eq!(mounted.root_device, "/dev/vdx2");
        assert_eq!(mounted.root_uuid, Hand::uuid("/dev/vdx2"));
        assert!(mounted.gpt_auto_off);
        assert_eq!(mounted.other_devices, ["/dev/vdx1", "/dev/vdx2"]);
        assert_eq!(mounted.swapfile.as_deref(), Some(Path::new("/swapfile")));
        let fstab: Vec<&str> = mounted
            .fstab
            .lines()
            .filter(|l| l.starts_with("UUID=") || l.starts_with('/'))
            .collect();
        assert_eq!(
            fstab,
            [
                format!("UUID={}\t/\text4\tdefaults\t0 1", Hand::uuid("/dev/vdx2")),
                format!(
                    "UUID={}\t/boot\tvfat\tumask=0077\t0 2",
                    Hand::uuid("/dev/vdx1")
                ),
                "/swapfile\tnone\tswap\tdefaults\t0 0".to_string(),
            ]
        );
        // Phase A in Check; the tables in Partition, the filesystems in
        // Format; each command logged before it is run.
        let begun: Vec<Step> = ran
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Begin { step } => Some(*step),
                _ => None,
            })
            .collect();
        assert_eq!(begun, [Step::Partition, Step::Format]);
        let logged = |text: &str| {
            ran.events
                .iter()
                .any(|event| matches!(event, Event::Log { line } if line.contains(text)))
        };
        assert!(logged("layout: wipefs --all --lock /dev/vdx"));
        assert!(logged("layout: mkfs.ext4 -F /dev/vdx2"));
        assert!(logged("fstab: UUID="));
        taken_out(ran, "blank");
    }

    /// The matrix's three disks (Q6's), listed `/dev/vdc` — the person's
    /// home, kept — `/dev/vdb` — the EFI system partition and `/boot` —
    /// and `/dev/vda` — `/` and swap.
    fn three(sector: u32) -> Vec<DiskLayout> {
        cases(sector)
            .into_iter()
            .find(|case| case.name == "three-disks")
            .expect("the three disks")
            .layout
    }

    /// What the udisks rule names for disks listed in the order of
    /// `names` (kernel name and major:minor), and its change events.
    fn rule_for(names: &[(&str, &str)]) -> (String, Vec<String>) {
        let mut text = String::from(
            "# Written by lxb-installer for one installation, and removed when it ends:\n\
             # udisks neither shows nor mounts the disks it is changing.\n",
        );
        let mut trigger = vec!["trigger".to_string(), "--action=change".to_string()];
        for (name, numbers) in names {
            text.push_str(&format!(
                "SUBSYSTEM==\"block\", KERNEL==\"{name}\", ENV{{UDISKS_IGNORE}}=\"1\", \
                 ENV{{UDISKS_AUTO}}=\"0\"\n\
                 SUBSYSTEM==\"block\", ENV{{ID_PART_ENTRY_DISK}}==\"{numbers}\", \
                 ENV{{UDISKS_IGNORE}}=\"1\", ENV{{UDISKS_AUTO}}=\"0\"\n"
            ));
            trigger.push(format!("--parent-match=/sys/class/block/{name}"));
        }
        (text, trigger)
    }

    /// **Three disks carried out disk by disk, word for word** (Unit Q,
    /// Q6): the whole conversation with the machine for `/` and swap on
    /// one blank disk, the EFI system partition at `/efi` and `/boot` on a
    /// second, and the person's home kept at `/home` on a third, listed
    /// against their order — the udisks rule put in for all three, the disk
    /// only mounted from among them, and taken out for all three; each disk
    /// found again and what each uses asked whether it is in use, in path
    /// order (the two written whole, the third only its home); phase A on
    /// every disk written before anything is written to any; each written
    /// disk's wipe and new table, then its read back, in path order; every
    /// filesystem made in the same order; what blkid reads of each
    /// partition used, the kept home's UUID the one it was reviewed with;
    /// the mounts, a parent before what is inside it, the home's once. The
    /// home's disk is never claimed nor written, nor its table read; the
    /// `Mounted` starts from the EFI system partition's disk.
    #[test]
    fn three_disks_are_carried_out_disk_by_disk_in_the_plans_order() {
        let _proven = PretendProven::these(EVERY_SET);
        let laid = three(512);
        let config = config_for(laid.clone(), 0);
        assert_eq!(config.disk.path, "/dev/vda", "the disk that holds /");
        let ran = run(&config, Firmware::Uefi, Hand::new(&laid, Firmware::Uefi));
        let (_, mounted) = ran.result.as_ref().expect("carried out");
        let mut expected: Vec<String> = [
            "rule write",
            "rule udevadm control",
            "rule udevadm trigger",
            "rule udevadm settle",
            "discover",
            "mounts",
            "swaps",
            "facts /dev/vda",
            "facts /dev/vdb",
            "facts /dev/vdc1",
            "online",
            "stage",
        ]
        .iter()
        .map(|line| line.to_string())
        .collect();
        for disk in ["/dev/vda", "/dev/vdb"] {
            expected.push(format!(
                "read lsblk --json --bytes --paths --nodeps -o NAME,SIZE,LOG-SEC {disk}"
            ));
            expected.push(format!("read sfdisk --json {disk}"));
        }
        // The home, on the disk only mounted from, read itself before
        // anything is written.
        expected.push("read blkid -p -s UUID -o value /dev/vdc1".into());
        for disk in ["/dev/vda", "/dev/vdb"] {
            for (index, command) in [
                format!("wipefs --all --lock {disk}"),
                format!("sfdisk --lock --wipe always --wipe-partitions always {disk}"),
            ]
            .iter()
            .enumerate()
            {
                if index > 0 {
                    expected.push(format!("settle {disk}"));
                }
                expected.push("fence".into());
                expected.push(format!("claim {disk}"));
                expected.push(format!("write {command}"));
            }
            expected.push(format!("settle {disk}"));
            expected.push(format!(
                "read lsblk --json --bytes --paths -o NAME,START,SIZE,PARTN {disk}"
            ));
            for number in [1, 2] {
                expected.push(format!("read blkid -p -s UUID -o value {disk}{number}"));
            }
            for number in [1, 2] {
                expected.push(format!("node {disk}{number}"));
            }
        }
        for (device, make) in [
            ("/dev/vda1", "mkfs.ext4 -F -L Linexin"),
            ("/dev/vda2", "mkswap"),
            ("/dev/vdb1", "mkfs.fat -F 32 -n EFI"),
            ("/dev/vdb2", "mkfs.fat -F 32 -n BOOT"),
        ] {
            for command in ["wipefs --all", make] {
                for line in [
                    "fence".to_string(),
                    format!("claim {device}"),
                    "mounts".into(),
                    "swaps".into(),
                    format!("facts {device}"),
                    format!("write {command} {device}"),
                ] {
                    expected.push(line);
                }
            }
        }
        expected.push("settle all".into());
        for device in [
            "/dev/vda1",
            "/dev/vda2",
            "/dev/vdb1",
            "/dev/vdb2",
            "/dev/vdc1",
        ] {
            expected.push(format!("read blkid -p -s UUID -o value {device}"));
            expected.push(format!("read blkid -p -s TYPE -o value {device}"));
        }
        for line in [
            "root",
            "dir /",
            "mount ext4 - /dev/vda1 /",
            "dir /boot",
            "mount vfat umask=0077 /dev/vdb2 /boot",
            "dir /efi",
            "mount vfat umask=0077 /dev/vdb1 /efi",
            "dir /home",
            "mount ext4 - /dev/vdc1 /home",
        ] {
            expected.push(line.into());
        }
        assert_eq!(ran.lines(), expected);
        holds_the_order(&ran.lines(), "three disks");
        // The home's disk: never claimed, written or read for its table;
        // its partition asked whether it is in use, read by blkid, and
        // mounted once, at /home, as the person's own.
        let lines = ran.lines();
        let home: Vec<&str> = lines
            .iter()
            .filter(|line| line.contains("/dev/vdc"))
            .map(String::as_str)
            .collect();
        assert_eq!(
            home,
            [
                "facts /dev/vdc1",
                "read blkid -p -s UUID -o value /dev/vdc1",
                "read blkid -p -s UUID -o value /dev/vdc1",
                "read blkid -p -s TYPE -o value /dev/vdc1",
                "mount ext4 - /dev/vdc1 /home",
            ]
        );
        // The udisks rule names all three disks, the one only mounted from
        // among them, as the layout lists them, and is taken out for all
        // three.
        let (text, trigger) = rule_for(&[("vdc", "254:32"), ("vdb", "254:16"), ("vda", "254:0")]);
        let effects = ran.said.borrow().effects.clone();
        assert_eq!(effects[0], Effect::Write(text));
        assert!(
            matches!(&effects[2], Effect::Run(command) if command.args == trigger),
            "{:?}",
            effects[2]
        );
        // The firmware starts from the EFI system partition's disk.
        assert_eq!(
            (mounted.disk.as_str(), mounted.table),
            ("/dev/vdb", Table::Gpt)
        );
        let esp = mounted.esp.as_ref().expect("an EFI system partition");
        assert_eq!(
            (esp.device.as_str(), esp.mount, esp.shared),
            ("/dev/vdb1", crate::worker::layout::EspMount::Efi, false)
        );
        assert_eq!(mounted.linux_boot.as_ref().unwrap().device, "/dev/vdb2");
        assert_eq!(mounted.root_device, "/dev/vda1");
        assert_eq!(mounted.home_device.as_deref(), Some("/dev/vdc1"));
        assert_eq!(
            mounted.other_devices,
            [
                "/dev/vda1",
                "/dev/vda2",
                "/dev/vdb1",
                "/dev/vdb2",
                "/dev/vdc1"
            ]
        );
        assert_eq!(mounted.swapfile, None);
        let fstab: Vec<&str> = mounted
            .fstab
            .lines()
            .filter(|l| l.starts_with("UUID="))
            .collect();
        assert_eq!(
            fstab,
            [
                format!("UUID={}\t/\text4\tdefaults\t0 1", Hand::uuid("/dev/vda1")),
                format!(
                    "UUID={}\t/boot\tvfat\tumask=0077\t0 2",
                    Hand::uuid("/dev/vdb2")
                ),
                format!(
                    "UUID={}\t/efi\tvfat\tumask=0077\t0 2",
                    Hand::uuid("/dev/vdb1")
                ),
                format!("UUID={HOME_UUID}\t/home\text4\tdefaults\t0 2"),
                format!(
                    "UUID={}\tnone\tswap\tdefaults\t0 0",
                    Hand::uuid("/dev/vda2")
                ),
            ]
        );
        let said = Rc::clone(&ran.said);
        taken_out(ran, "three disks");
        let effects = said.borrow().effects.clone();
        let removal = &effects[effects.len() - 3..];
        assert_eq!(removal[0], Effect::Remove);
        assert!(
            matches!(&removal[2], Effect::Run(command) if command.args == trigger),
            "{:?}",
            removal[2]
        );
    }

    /// **The firmware entry is made on the EFI system partition's own disk,
    /// under the number the kernel gives it** (Unit Q, Q6): the three disks
    /// carried out, `/` on `/dev/vda` and the EFI system partition on
    /// `/dev/vdb`, the entry `boot::entry_args` builds from the `Mounted`
    /// and a kernel written by hand (each partition's link under its own
    /// disk, its number as the tables were written) is `efibootmgr --disk
    /// /dev/vdb --part 1`; given another number by the kernel, that number;
    /// and a `Mounted` naming `/`'s disk instead is refused, since the
    /// kernel puts the EFI system partition on another.
    #[test]
    fn the_firmware_entry_names_the_efi_partitions_own_disk_and_number() {
        use crate::worker::system::boot;
        let _proven = PretendProven::these(EVERY_SET);
        let laid = three(512);
        let config = config_for(laid.clone(), 0);
        let ran = run(&config, Firmware::Uefi, Hand::new(&laid, Firmware::Uefi));
        let (_, mounted) = ran.result.as_ref().expect("carried out");
        let esp = mounted.esp.as_ref().expect("an EFI system partition");
        assert_eq!(mounted.root_device, "/dev/vda1");
        // The kernel's partitions once the tables are written, each with
        // its disk.
        let ctx = Context::new(&laid, Firmware::Uefi);
        let mut kernel: Vec<(String, Kernel)> = Vec::new();
        for one in &laid {
            let found = if plan::writes_to(one) {
                after_writing(ctx, one, None).1
            } else {
                one.disk
                    .partitions
                    .iter()
                    .map(|p| Kernel {
                        path: p.path.clone(),
                        number: p.number,
                        start: p.start,
                        size: p.size,
                    })
                    .collect()
            };
            kernel.extend(found.into_iter().map(|k| (one.disk.path.clone(), k)));
        }
        let find = |name: &str| {
            kernel
                .iter()
                .find(|(_, k)| k.path == format!("/dev/{name}"))
                .cloned()
        };
        let link = |name: &str| {
            find(name)
                .map(|(disk, _)| {
                    let disk = disk.trim_start_matches("/dev/");
                    format!("../../devices/pci0000:00/0000:00:05.0/virtio3/block/{disk}/{name}")
                })
                .ok_or_else(|| format!("the kernel has no {name}"))
        };
        let number = |name: &str| find(name).map(|(_, k)| format!("{}\n", k.number));
        let loader = r"\EFI\systemd\systemd-bootx64.efi";
        let entry = |mounted: &Mounted, number: &dyn Fn(&str) -> Option<String>| {
            boot::entry_args(mounted, esp, boot::LABEL, loader, &link, number)
        };
        let made = |disk: &str, part: &str| {
            [
                "--quiet", "--create", "--disk", disk, "--part", part, "--loader", loader,
                "--label", "Linexin",
            ]
            .map(String::from)
            .to_vec()
        };
        assert_eq!(entry(mounted, &number), Ok(made("/dev/vdb", "1")));
        // The number is the kernel's, never one cut off the name.
        let third = |_: &str| Some("3\n".to_string());
        assert_eq!(entry(mounted, &third), Ok(made("/dev/vdb", "3")));
        // A Mounted naming the disk / is on: refused, the kernel having the
        // EFI system partition on another.
        let mut rooted = mounted.clone();
        rooted.disk = "/dev/vda".into();
        let refused = entry(&rooted, &number).expect_err("an entry on /'s disk");
        assert!(
            refused.starts_with("/dev/vdb1 is not on /dev/vda"),
            "{refused}"
        );
        taken_out(ran, "the firmware entry");
    }

    /// **The three disks need what Q6 proves, and nothing less carries them
    /// out** (Unit Q, Q6): a new table, a kept `/home` and several disks,
    /// on GPT under UEFI — with only what Q5 proves pretended, or with
    /// either of Q6's two left out, the executor refuses them before it
    /// asks the machine anything, naming the first missing; with Q6's two
    /// each proven but by two sets, never one (decision 20), it refuses
    /// them as a mix no scenario ran whole; with both in one set, they are
    /// carried out.
    #[test]
    fn three_disks_need_several_disks_and_a_kept_home_proven() {
        use Capability::{ChangeEntries, FreshTable, Gpt, KeepTable, KeptHome, SeveralDisks, Uefi};
        const Q5: &[&[Capability]] = &[&[FreshTable, KeepTable, ChangeEntries, Uefi, Gpt]];
        const Q5_DISKS: &[&[Capability]] = &[&[
            FreshTable,
            KeepTable,
            ChangeEntries,
            SeveralDisks,
            Uefi,
            Gpt,
        ]];
        const Q5_HOME: &[&[Capability]] =
            &[&[FreshTable, KeepTable, ChangeEntries, KeptHome, Uefi, Gpt]];
        const APART: &[&[Capability]] = &[
            &[
                FreshTable,
                KeepTable,
                ChangeEntries,
                SeveralDisks,
                Uefi,
                Gpt,
            ],
            &[FreshTable, KeepTable, ChangeEntries, KeptHome, Uefi, Gpt],
        ];
        const Q6: &[&[Capability]] = &[&[
            FreshTable,
            KeepTable,
            ChangeEntries,
            KeptHome,
            SeveralDisks,
            Uefi,
            Gpt,
        ]];
        let laid = three(512);
        assert_eq!(
            layout::needs(&laid, Firmware::Uefi),
            [FreshTable, KeptHome, SeveralDisks, Uefi, Gpt]
        );
        let config = config_for(laid.clone(), 0);
        for (proven, missing) in [
            (Q5, Some("needs KeptHome,")),
            (Q5_DISKS, Some("needs KeptHome,")),
            (Q5_HOME, Some("needs SeveralDisks,")),
            (
                APART,
                Some("needs [FreshTable, KeptHome, SeveralDisks, Uefi, Gpt] together,"),
            ),
            (Q6, None),
        ] {
            let _pretended = PretendProven::these(proven);
            let ran = run(&config, Firmware::Uefi, Hand::new(&laid, Firmware::Uefi));
            match missing {
                Some(missing) => {
                    let stop = ran.result.as_ref().expect_err("carried out");
                    assert_eq!(stop.failure, Failure::Internal, "{proven:?}");
                    assert!(stop.detail.contains(missing), "{}", stop.detail);
                    assert!(ran.lines().is_empty(), "{:?}", ran.lines());
                    assert!(ran.quiet.is_none());
                }
                None => {
                    assert!(ran.result.is_ok(), "{proven:?}");
                    taken_out(ran, "Q5's and Q6's");
                }
            }
        }
    }

    /// **A panic inside the run takes the rule out** (Unit Q, Q6): the blank
    /// disk carried out on the machine written by hand, which panics — as a
    /// bug would — at the first write of phase B, at a filesystem made in
    /// phase C and at the first mount; each time the panic unwinds past the
    /// slot the rule's guard is held in, as it does past `run_worker`'s, and
    /// the guard's `Drop` takes the rule out — written down by the rule's
    /// hands, never run. The build unwinds on a panic (`panic = "unwind"`,
    /// which `quiet.rs` refuses to compile without).
    #[test]
    fn a_panic_inside_the_run_takes_the_rule_out() {
        let _proven = PretendProven::these(EVERY_SET);
        let laid = blank(512);
        let config = config_for(laid.clone(), 2048);
        for at in [
            "write wipefs --all --lock /dev/vdx",
            "write mkfs.ext4 -F /dev/vdx2",
            "mount ext4 - /dev/vdx2 /",
        ] {
            let hand = Hand::new(&laid, Firmware::Uefi);
            let said = Rc::clone(&hand.said);
            said.borrow_mut().panic_at = Some(at.to_string());
            let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run(&config, Firmware::Uefi, hand)
            }));
            assert!(ran.is_err(), "{at}: the run did not panic");
            let said = said.borrow();
            let panicked = said.lines.iter().position(|line| line == at).unwrap();
            assert_eq!(
                said.lines[panicked + 1..],
                [
                    "rule remove",
                    "rule udevadm control",
                    "rule udevadm trigger"
                ],
                "{at}: the rule was not taken out, once, as the panic unwound"
            );
            assert_eq!(said.effects.len(), 4 + 3, "{at}");
            assert_eq!(said.effects[4], Effect::Remove, "{at}");
        }
    }

    /// **What is proven on GPT never carries out an msdos disk** (Unit Q,
    /// Q5): each of the matrix's msdos layouts — a kept msdos table changed,
    /// its extended partition's logical ones among it, and (Q7) a new msdos
    /// table written over one; and since decision 20 any layout with an msdos
    /// disk, one only mounted from too — with every capability but `Msdos`
    /// pretended proven, `Bios` among them, is refused naming `Msdos` before
    /// the machine is asked anything; with `Msdos` too, it is carried out.
    #[test]
    fn what_is_proven_on_gpt_never_carries_out_an_msdos_disk() {
        let all_but_msdos: &'static [Capability] = EVERY
            .iter()
            .copied()
            .filter(|capability| *capability != Capability::Msdos)
            .collect::<Vec<_>>()
            .leak();
        let all_but_msdos: &'static [&'static [Capability]] = vec![all_but_msdos].leak();
        let mut msdos = 0;
        for sector in [512, 4096] {
            for case in cases(sector) {
                let on_msdos = case
                    .layout
                    .iter()
                    .any(|one| layout::table_after(one) == Some(Table::Msdos));
                if !on_msdos {
                    continue;
                }
                msdos += 1;
                let name = format!("{} at {sector}", case.name);
                let config = config_for(case.layout.clone(), 0);
                {
                    let _proven = PretendProven::these(all_but_msdos);
                    let hand = Hand::new(&case.layout, case.firmware);
                    let ran = run(&config, case.firmware, hand);
                    let stop = ran.result.as_ref().expect_err(&name);
                    assert_eq!(stop.failure, Failure::Internal, "{name}");
                    assert_eq!(
                        stop.detail, "the layout needs Msdos, which this build has not proven",
                        "{name}"
                    );
                    assert!(ran.lines().is_empty(), "{name}: {:?}", ran.lines());
                    assert!(ran.quiet.is_none(), "{name}");
                }
                let _proven = PretendProven::these(EVERY_SET);
                let hand = Hand::new(&case.layout, case.firmware);
                let ran = run(&config, case.firmware, hand);
                assert!(ran.result.is_ok(), "{name}");
                taken_out(ran, &name);
            }
        }
        assert_eq!(msdos, 12, "six msdos layouts at each sector size");
    }

    /// **A home kept on a disk of its own is mounted only while it is the
    /// filesystem the person reviewed** (Unit Q, Q6; phase A's look): the
    /// three disks' kept `/home` with another UUID than it was reviewed with
    /// — another filesystem made there since, or another partition at its
    /// path — is `DiskChanged`. Read so in phase A, the run stops before
    /// anything is written to any disk, in Check, nothing claimed; changed
    /// only once the tables are written, the look once everything is made
    /// still stops it before anything is mounted, no fstab written. Its disk
    /// is never claimed nor written either way.
    #[test]
    fn a_kept_home_holding_another_filesystem_now_is_never_mounted() {
        let _proven = PretendProven::these(EVERY_SET);
        let laid = three(512);
        let config = config_for(laid.clone(), 0);
        let other = "0d1e2f3a-0000-4000-8000-000000000000";
        let refused = |ran: &Ran| {
            assert_eq!(ran.failure(), Failure::DiskChanged);
            let detail = ran.result.as_ref().err().unwrap().detail.clone();
            assert!(
                detail.contains("/dev/vdc1, kept at /home") && detail.contains(HOME_UUID),
                "{detail}"
            );
            let lines = ran.lines();
            assert_eq!(
                lines.last().map(String::as_str),
                Some("read blkid -p -s UUID -o value /dev/vdc1")
            );
            assert!(!lines.iter().any(|l| l == "root" || l.starts_with("mount ")));
            assert!(!lines.iter().any(|l| {
                (l.starts_with("claim ") || l.starts_with("write ")) && l.contains("/dev/vdc")
            }));
            assert!(!ran
                .events
                .iter()
                .any(|event| matches!(event, Event::Log { line } if line.starts_with("fstab:"))));
        };
        // Another filesystem there already: phase A stops the run before
        // anything is claimed or written, in Check.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.uuid_read.insert("/dev/vdc1".into(), other.into());
        let ran = run(&config, Firmware::Uefi, hand);
        refused(&ran);
        assert!(
            !ran.lines()
                .iter()
                .any(|l| l.starts_with("claim ") || l.starts_with("write ")),
            "{:?}",
            ran.lines()
        );
        assert!(!ran
            .events
            .iter()
            .any(|event| matches!(event, Event::Begin { .. })));
        taken_out(ran, "the home another filesystem before the run");
        // Another filesystem there only once the tables are written: the
        // look once everything is made stops it before any mount.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.uuid_read_later
            .insert("/dev/vdc1".into(), other.into());
        let ran = run(&config, Firmware::Uefi, hand);
        refused(&ran);
        assert!(ran.lines().iter().any(|l| l.starts_with("write mkfs")));
        taken_out(ran, "the home another filesystem during the run");
    }

    /// **A kept partition's UUID is its own whatever its case** (Unit Q, Q6):
    /// the three disks' kept home read by blkid with its reviewed UUID in
    /// capitals — as one tool writes what another writes in small letters —
    /// is the filesystem the person reviewed, and is carried out, the fstab
    /// naming it as blkid read it.
    #[test]
    fn a_kept_homes_uuid_in_other_letters_is_still_its_own() {
        let _proven = PretendProven::these(EVERY_SET);
        let laid = three(512);
        let config = config_for(laid.clone(), 0);
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        let capitals = HOME_UUID.to_ascii_uppercase();
        hand.uuid_read.insert("/dev/vdc1".into(), capitals.clone());
        let ran = run(&config, Firmware::Uefi, hand);
        let (_, mounted) = ran
            .result
            .as_ref()
            .unwrap_or_else(|stop| panic!("{:?}: {}", stop.failure, stop.detail));
        let home = format!("UUID={capitals}\t/home\text4\tdefaults\t0 2");
        assert!(
            mounted.fstab.lines().any(|line| line == home),
            "{}",
            mounted.fstab
        );
        taken_out(ran, "the home's UUID in capitals");
    }

    /// **On any of three disks, a refusal stops everything after it**
    /// (Unit Q, Q6), each disk's check made in its own loop and judged
    /// alone: the home's disk changed or gone since it was reviewed, or its
    /// home mounted in the live session — refused in Check, no disk touched;
    /// a table on the last disk written now, which was blank — refused in
    /// phase A, after every disk written was looked at and before anything
    /// is written to the first; the last disk written read back other than
    /// the layout leaves it — refused once both tables are written, before
    /// any filesystem is made on either; the kernel taking the last disk
    /// for a partition of another — the fence refuses its first command,
    /// the first disk written, nothing made. The rule taken out each time.
    #[test]
    fn on_any_of_three_disks_a_refusal_stops_everything_after_it() {
        let _proven = PretendProven::these(EVERY_SET);
        let laid = three(512);
        let config = config_for(laid.clone(), 0);
        let touched = |ran: &Ran| -> Vec<String> {
            ran.lines()
                .into_iter()
                .filter(|l| l.starts_with("claim ") || l.starts_with("write "))
                .collect()
        };
        let made = |ran: &Ran| {
            ran.lines().iter().any(|l| {
                l.starts_with("write mkfs") || l.starts_with("write mkswap") || {
                    let device = l.split_whitespace().last().unwrap_or_default();
                    l.starts_with("write wipefs --all /dev/") && device.ends_with(char::is_numeric)
                }
            })
        };
        // In Check: the home's disk another, gone, or its home in use.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.found[0].partitions[0].uuid = "0d1e2f3a-0000-4000-8000-000000000000".into();
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::DiskChanged);
        assert!(touched(&ran).is_empty(), "{:?}", ran.lines());
        taken_out(ran, "the home another");
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.found.remove(0);
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::DiskChanged);
        assert!(touched(&ran).is_empty(), "{:?}", ran.lines());
        taken_out(ran, "the home's disk gone");
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.mounts = "41 1 254:33 / /run/media/zoe/Home rw - ext4 /dev/vdc1 rw\n".into();
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::DiskBusy);
        assert!(touched(&ran).is_empty(), "{:?}", ran.lines());
        taken_out(ran, "the home in use");
        // In phase A: a table on /dev/vdb now. Both disks were looked at,
        // nothing was written to either.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        let mut table = fixtures::on_disk(&fixtures::disk_gpt_plain(512));
        table.partitions.clear();
        hand.before.insert("/dev/vdb".into(), table);
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::DiskChanged);
        for disk in ["/dev/vda", "/dev/vdb"] {
            let read = format!("read sfdisk --json {disk}");
            assert!(ran.lines().contains(&read), "{read}");
        }
        assert!(touched(&ran).is_empty(), "{:?}", ran.lines());
        taken_out(ran, "a table on /dev/vdb now");
        // In phase B: /dev/vdb read back without its /boot. Both tables
        // written, in path order; no filesystem made on either disk.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.after.get_mut("/dev/vdb").unwrap().1.pop();
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::DiskChanged);
        let detail = ran.result.as_ref().err().unwrap().detail.clone();
        assert!(
            detail.starts_with("/dev/vdb is not as the layout leaves it"),
            "{detail}"
        );
        let writes: Vec<String> = touched(&ran)
            .into_iter()
            .filter(|l| l.starts_with("write "))
            .collect();
        assert_eq!(
            writes,
            [
                "write wipefs --all --lock /dev/vda",
                "write sfdisk --lock --wipe always --wipe-partitions always /dev/vda",
                "write wipefs --all --lock /dev/vdb",
                "write sfdisk --lock --wipe always --wipe-partitions always /dev/vdb",
            ]
        );
        assert!(!made(&ran));
        taken_out(ran, "/dev/vdb read back short");
        // In phase B: /dev/vdb a partition of another disk, as the kernel
        // has it now. The fence refuses its first command, before its claim.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.moved.insert(
            "vdb".into(),
            (
                "../../devices/virtual/block/vdz/vdb".into(),
                Some((MIB, 8 * crate::model::disks::GIB)),
            ),
        );
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::Internal);
        let detail = ran.result.as_ref().err().unwrap().detail.clone();
        assert!(detail.starts_with("the fence refused /dev/vdb"), "{detail}");
        assert!(touched(&ran).iter().all(|l| l.ends_with("/dev/vda")));
        assert!(!ran.lines().contains(&"claim /dev/vdb".to_string()));
        assert!(!made(&ran));
        taken_out(ran, "/dev/vdb past the fence");
    }

    /// **Every fixture layout is carried out by exactly the commands the
    /// matrix holds** (Unit Q, Q5; Q9): each of `manual::matrix`'s layouts at
    /// 512 and 4096 bytes a sector run on a machine written by hand, every
    /// capability pretended proven: the writes are, in order, the very
    /// commands the matrix's paper run gives — each filesystem shrunk by
    /// its own commands first, before any entry on any disk, then the
    /// tables, the boot flag, the filesystems — each given leave in the
    /// plan's order; the `Mounted` is the layout's; the rule is taken out
    /// once it is let go of.
    #[test]
    fn every_fixture_layout_is_carried_out_by_the_matrix_commands() {
        let _proven = PretendProven::these(EVERY_SET);
        let mut carried = 0;
        let mut shrunk = 0;
        for sector in [512, 4096] {
            for case in cases(sector) {
                let name = format!("{} at {sector}", case.name);
                let config = config_for(case.layout.clone(), 0);
                let ran = run(
                    &config,
                    case.firmware,
                    Hand::new(&case.layout, case.firmware),
                );
                holds_the_order(&ran.lines(), &name);
                let mounted = match &ran.result {
                    Ok((_, mounted)) => mounted,
                    Err(stop) => panic!("{name}: {:?}: {}", stop.failure, stop.detail),
                };
                assert!(mounted.gpt_auto_off, "{name}");
                assert_eq!(mounted.firmware, case.firmware, "{name}");
                let paper = on_paper(case.firmware, &case.layout);
                let lines = |planned: &Planned| -> Vec<String> {
                    match filling(planned) {
                        Ok(shrink) => shrink
                            .planned()
                            .iter()
                            .map(|(run, _)| format!("write {}", line(run.command().unwrap())))
                            .collect(),
                        Err(_) => vec![format!("write {}", line(planned.command().unwrap()))],
                    }
                };
                let (shrinks, rest): (Vec<&Planned>, Vec<&Planned>) = paper
                    .all()
                    .into_iter()
                    .partition(|planned| planned.act() == Act::ShrinkFilesystem);
                let wanted: Vec<String> = shrinks
                    .iter()
                    .chain(&rest)
                    .flat_map(|planned| lines(planned))
                    .collect();
                let written: Vec<String> = ran
                    .lines()
                    .into_iter()
                    .filter(|l| l.starts_with("write "))
                    .collect();
                assert_eq!(written, wanted, "{name}");
                shrunk += usize::from(!shrinks.is_empty());
                carried += 1;
                taken_out(ran, &name);
            }
        }
        assert_eq!(carried, 28, "fourteen fixture layouts at each sector size");
        assert_eq!(shrunk, 2, "the Windows disk's at each sector size");
    }

    /// The questions whose failure only puts the run on its way the longer
    /// way, and never stops it: udev not settling once the rule is in, the
    /// table sfdisk cannot read of a disk of `laid` that is blank (there is
    /// none), a UUID blkid cannot read while a disk is read back (a
    /// partition with nothing on it has none).
    fn only_noted(question: &str, laid: &[DiskLayout]) -> bool {
        let blank = |disk: &str| {
            laid.iter()
                .any(|one| one.disk.path == disk && one.disk.table.is_none())
        };
        question == "rule udevadm settle"
            || question
                .strip_prefix("read sfdisk --json ")
                .is_some_and(blank)
            || question.starts_with("read blkid -p -s UUID")
    }

    /// **The rule is taken out, and nothing is written past a failure, on
    /// every way out** (Unit Q, Q5): the blank disk, Windows' disk with its
    /// EFI partition kept at `/efi`, (Q6) the three disks, and (Q9) Windows
    /// shrunk and an ext4 shrunk — every reading again, dry run, shrink and
    /// read back among the questions — each run once
    /// for every question the machine can fail, that one failed. The guard is held whatever
    /// the run came to, and takes the rule out, once, when it is let go of;
    /// the order holds; a failure that stops the run is followed by no
    /// claim and no write — and, before the internet was asked, touched no
    /// disk at all; and only a failure that is only noted lets the run go
    /// through.
    #[test]
    fn the_rule_is_taken_out_and_nothing_written_past_a_failure_on_every_way_out() {
        let _proven = PretendProven::these(EVERY_SET);
        let windows = cases(512)
            .into_iter()
            .find(|case| case.name == "gpt-windows-replace")
            .unwrap();
        for (name, laid) in [
            ("blank", blank(512)),
            ("gpt-windows-replace", windows.layout),
            ("three disks", three(512)),
            ("gpt-windows, Windows shrunk", self::windows(512)),
            ("an ext4 kept shrunk", ext4_kept(512)),
        ] {
            let config = config_for(laid.clone(), 0);
            let whole = run(&config, Firmware::Uefi, Hand::new(&laid, Firmware::Uefi));
            assert!(whole.result.is_ok(), "{name}");
            let questions = whole.said.borrow().questions.clone();
            let online = questions.iter().position(|q| q == "online").unwrap();
            drop(whole);
            let mut stopped = 0;
            for (failing, asked) in questions.iter().enumerate() {
                let hand = Hand::new(&laid, Firmware::Uefi).failing_at(failing);
                let ran = run(&config, Firmware::Uefi, hand);
                let case = format!("{name}, {asked:?} failed");
                assert!(ran.quiet.is_some(), "{case}: no guard held");
                holds_the_order(&ran.lines(), &case);
                let (question, at) = ran.said.borrow().failed.clone().expect("it failed");
                assert_eq!(&question, asked, "{case}");
                let touched = |l: &String| l.starts_with("claim ") || l.starts_with("write ");
                match &ran.result {
                    Err(_) => {
                        stopped += 1;
                        if failing <= online {
                            let any = ran.lines().iter().any(touched);
                            assert!(!any, "{case}: a disk was touched");
                        }
                        assert!(
                            !ran.lines()[at + 1..].iter().any(touched),
                            "{case}: a disk was touched past the failure"
                        );
                    }
                    Ok(_) => assert!(only_noted(&question, &laid), "{case}: the run went through"),
                }
                taken_out(ran, &case);
            }
            // Every question but those only noted stopped the run (a UUID
            // read once the filesystems are made is not only noted: it
            // stops it too).
            let noted = questions.iter().filter(|q| only_noted(q, &laid)).count();
            assert!(stopped + noted >= questions.len(), "{name}");
            assert!(
                stopped > noted,
                "{name}: {stopped} of {} stopped",
                questions.len()
            );
        }
    }

    /// **For what is not proven the executor asks the machine nothing at
    /// all** (Unit Q, Q5): after `check::validate`, it refuses on its own
    /// any layout that needs what no one set of `PROVEN` holds, before the
    /// rule is even put in — each fixture layout, pretended proven by
    /// nothing (as were no set written), and with `PROVEN` itself, at 512
    /// bytes a sector, the five it refuses: a primary partition's entry
    /// grown (gpt-plain, msdos) and the extended frame emptied and deleted
    /// (msdos-extended), which no scenario ran, and mixes no one scenario
    /// ran whole — a new msdos table (msdos-new-table), a new gpt on one
    /// disk beside a partition formatted on another (two-disks); at 4096
    /// every one of them, a disk of 4096-byte sectors being what no
    /// scenario ran (`LargeSectors`).
    #[test]
    fn with_nothing_proven_the_executor_asks_nothing() {
        let asks_nothing = |case: &crate::worker::manual::matrix::Case| {
            let config = config_for(case.layout.clone(), 0);
            let ran = run(
                &config,
                case.firmware,
                Hand::new(&case.layout, case.firmware),
            );
            assert_eq!(ran.failure(), Failure::Internal, "{}", case.name);
            assert!(ran.lines().is_empty(), "{}: {:?}", case.name, ran.lines());
            assert!(ran.quiet.is_none());
        };
        for sector in [512, 4096] {
            let (mut every, mut refused) = (Vec::new(), Vec::new());
            for case in cases(sector) {
                every.push(case.name);
                if layout::unproven(&case.layout, case.firmware).is_some() {
                    refused.push(case.name);
                    asks_nothing(&case);
                }
                let _none = PretendProven::these(&[]);
                asks_nothing(&case);
            }
            if sector == 512 {
                assert_eq!(
                    refused,
                    [
                        "gpt-plain",
                        "msdos",
                        "msdos-extended",
                        "msdos-new-table",
                        "two-disks"
                    ],
                    "at {sector}"
                );
            } else {
                assert_eq!(refused, every, "at {sector}");
            }
        }
    }

    /// **A disk not as reviewed, a disk in use, a fence that will not let a
    /// command through, a table not the one reviewed, a kept EFI partition
    /// too full: each refused before any disk is written** (Unit Q, Q5),
    /// and the rule taken out.
    #[test]
    fn nothing_is_written_to_a_disk_not_as_reviewed_or_in_use_or_past_the_fence() {
        let _proven = PretendProven::these(EVERY_SET);
        let untouched = |ran: &Ran, case: &str| {
            let touched = ran
                .lines()
                .iter()
                .any(|l| l.starts_with("claim ") || l.starts_with("write "));
            assert!(!touched, "{case}: a disk was touched: {:?}", ran.lines());
        };
        let laid = blank(512);
        let config = config_for(laid.clone(), 0);
        // Another disk at its path since it was reviewed: another diskseq,
        // as the kernel numbers a disk plugged in again or another one.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.found[0].diskseq = Some(7);
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::DiskChanged);
        untouched(&ran, "another disk");
        taken_out(ran, "another disk");
        // Mounted in the live session.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.mounts = "41 1 254:0 / /run/media/zoe/x rw - ext4 /dev/vdx rw\n".into();
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::DiskBusy);
        untouched(&ran, "busy");
        taken_out(ran, "busy");
        // Its sectors another size than reviewed, as the kernel reports it
        // just before anything is written.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.sector_now = Some(4096);
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::DiskChanged);
        untouched(&ran, "another sector size");
        taken_out(ran, "another sector size");
        // A table on the blank disk now, which the kernel's listing missed.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.before.insert(
            "/dev/vdx".into(),
            fixtures::on_disk(&fixtures::disk_gpt_plain(512)),
        );
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::DiskChanged);
        untouched(&ran, "a table now");
        taken_out(ran, "a table now");
        // The disk the kernel says is a partition of another: the fence
        // refuses the first command, before its claim.
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        hand.sys.devices.insert(
            "vdx".into(),
            (
                "../../devices/virtual/block/vdz/vdx".into(),
                Some((MIB, 64 * crate::model::disks::GIB)),
            ),
        );
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::Internal);
        let detail = ran.result.as_ref().err().unwrap().detail.clone();
        assert!(detail.starts_with("the fence refused"), "{detail}");
        untouched(&ran, "past the fence");
        taken_out(ran, "past the fence");
        // A new table over Windows' disk, whose table is not the one
        // reviewed: refused in phase A, before the wipe.
        let fresh = cases(512)
            .into_iter()
            .find(|case| case.name == "gpt-windows-new-table")
            .unwrap();
        let config = config_for(fresh.layout.clone(), 0);
        let mut hand = Hand::new(&fresh.layout, Firmware::Uefi);
        hand.before.get_mut("/dev/vdx").unwrap().partitions[2].size -= 1;
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::DiskChanged);
        untouched(&ran, "a new table over a changed table");
        assert!(ran
            .lines()
            .contains(&"read sfdisk --json /dev/vdx".to_string()));
        taken_out(ran, "a new table over a changed table");
        // The EFI partition kept at /efi with less free than a loader needs.
        let replace = cases(512)
            .into_iter()
            .find(|case| case.name == "gpt-windows-replace")
            .unwrap();
        let config = config_for(replace.layout.clone(), 0);
        let mut hand = Hand::new(&replace.layout, Firmware::Uefi);
        hand.esp_free = MIB;
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::EspTooFull);
        untouched(&ran, "a full EFI partition");
        taken_out(ran, "a full EFI partition");
    }

    /// **A home kept on a disk of its own stays the person's, after the
    /// storage half too** (Unit Q, Q6): for the three disks' request, whose
    /// `/home` is the kept ext4 on `/dev/vdc`, `home_kept()` holds, and on a
    /// target in a scratch folder this test makes — the live account's home
    /// with a file in it and its autostart, the skeleton's, the new
    /// account's Plasma layout — with every command recorded and none run:
    /// the live account is removed without its home (`userdel liveuser`, no
    /// `-r`) and nothing under `/home` is removed, its autostart included;
    /// the new account's home is not given to it wholesale (no `chown -R`);
    /// its Plasma launchers are left as they were, the skeleton's fitted.
    /// The same disks with that partition formatted at `/home` instead do
    /// all of it, as for a home the installer made.
    #[test]
    fn a_home_kept_on_a_disk_of_its_own_stays_the_persons_after_storage() {
        use crate::worker::system::desktop::{fit_launchers, LAUNCHER_SECTION, PLASMA_APPLETS};
        use crate::worker::system::{accounts, tidy};
        let kept = {
            let mut config = config_for(three(512), 0);
            config.account.username = "zoe".into();
            config
        };
        let formatted = {
            let mut laid = three(512);
            let size = laid[0].parts[0].size;
            laid[0].parts[0].fate = Fate::Format {
                fs: crate::model::layout::Fs::Ext4,
                label: String::new(),
                size,
                mount: Some(crate::model::layout::MountPoint::Home),
            };
            let mut config = config_for(laid, 0);
            config.account.username = "zoe".into();
            config
        };
        assert!(kept.home_kept() && !accounts::home_is_new(&kept));
        assert!(!formatted.home_kept() && accounts::home_is_new(&formatted));
        let layout = "[Containments][1]\nlaunchers=applications:old.desktop\n";
        let applets = format!("{LAUNCHER_SECTION}\nlaunchers=applications:old.desktop\n");
        let canary = "home/liveuser/Documents/canary.txt";
        let autostart = "home/liveuser/.config/autostart/lxb-installer.desktop";
        let skel_autostart = "etc/skel/.config/autostart/lxb-installer.desktop";
        let zoe_applets = format!("home/zoe/{PLASMA_APPLETS}");
        let skel_applets = format!("etc/skel/{PLASMA_APPLETS}");
        for (name, config) in [("kept", &kept), ("formatted", &formatted)] {
            /// The scratch folder, removed however the test ends.
            struct Scratch(PathBuf);
            impl Drop for Scratch {
                fn drop(&mut self) {
                    let _ = std::fs::remove_dir_all(&self.0);
                }
            }
            let scratch = Scratch(
                std::env::temp_dir().join(format!("lxb-kept-home-{name}-{}", std::process::id())),
            );
            let root = scratch.0.clone();
            let _ = std::fs::remove_dir_all(&root);
            let write = |relative: &str, text: &str| {
                let path = root.join(relative);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, text).unwrap();
            };
            write(
                "etc/passwd",
                "root:x:0:0::/root:/bin/bash\nliveuser:x:1000:1000::/home/liveuser:/bin/zsh\n",
            );
            write("etc/group", "root:x:0:\nliveuser:x:1000:\n");
            write(canary, "the person's own\n");
            write(autostart, "[Desktop Entry]\n");
            write(skel_autostart, "[Desktop Entry]\n");
            write(&zoe_applets, &format!("{layout}{applets}"));
            write(&skel_applets, &format!("{layout}{applets}"));
            let mut ran: Vec<String> = Vec::new();
            let mut chroot = |program: &str, args: &[&str]| {
                ran.push(format!("{program} {}", args.join(" ")));
                Ok(())
            };
            let home_kept = config.home_kept();
            tidy::remove_live_user(&root, home_kept, &mut chroot).unwrap();
            tidy::remove_autostarts(&root, home_kept).unwrap();
            fit_launchers(&root, config);
            accounts::give_home_back_by(config, &mut chroot);
            let there = |relative: &str| root.join(relative).exists();
            let zoe = std::fs::read_to_string(root.join(&zoe_applets)).unwrap();
            let skel = std::fs::read_to_string(root.join(&skel_applets)).unwrap();
            assert!(!there(skel_autostart), "{name}: the skeleton's autostart");
            let fitted =
                |text: &str| text.starts_with(layout) && text != format!("{layout}{applets}");
            assert!(fitted(&skel), "{name}: {skel}");
            if home_kept {
                assert_eq!(ran, ["userdel liveuser", "groupdel liveuser"]);
                assert!(there(canary) && there(autostart), "{name}");
                assert_eq!(
                    std::fs::read_to_string(root.join(canary)).unwrap(),
                    "the person's own\n"
                );
                assert_eq!(zoe, format!("{layout}{applets}"), "{name}");
            } else {
                assert_eq!(
                    ran,
                    [
                        "userdel -r liveuser",
                        "groupdel liveuser",
                        "chown -R -h -- zoe: /home/zoe",
                    ]
                );
                assert!(!there(canary) && !there(autostart), "{name}");
                assert!(fitted(&zoe), "{name}: {zoe}");
            }
            drop(scratch);
            assert!(!root.exists(), "{name}: the scratch folder stayed");
        }
    }

    /// **At Finish the new account's numbers fill the fstab**, read from the
    /// installed system's own `/etc/passwd`, in a scratch folder this test
    /// makes; an fstab with no place for them is left as it is, and one
    /// whose account is not in `/etc/passwd` is refused.
    #[test]
    fn finish_gives_the_new_account_its_partitions() {
        let root = std::env::temp_dir().join(format!("lxb-finish-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("etc")).unwrap();
        std::fs::write(
            root.join("etc/passwd"),
            "root:x:0:0::/root:/bin/bash\nzoe:x:1001:984::/home/zoe:/bin/bash\n",
        )
        .unwrap();
        let line = "UUID=1\t/windows\tntfs3\tnofail,uid=@uid@,gid=@gid@,windows_names\t0 0\n";
        std::fs::write(root.join("etc/fstab"), line).unwrap();
        finish(&root, "zoe").unwrap();
        let filled = std::fs::read_to_string(root.join("etc/fstab")).unwrap();
        assert_eq!(
            filled,
            "UUID=1\t/windows\tntfs3\tnofail,uid=1001,gid=984,windows_names\t0 0\n"
        );
        // Nothing to fill: left as it is.
        finish(&root, "nobody-at-all").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("etc/fstab")).unwrap(),
            filled
        );
        // The account missing: refused, the fstab as it was.
        std::fs::write(root.join("etc/fstab"), line).unwrap();
        assert_eq!(
            finish(&root, "anna").unwrap_err().failure,
            Failure::Internal
        );
        assert_eq!(
            std::fs::read_to_string(root.join("etc/fstab")).unwrap(),
            line
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    // ----- Step Q9: kept filesystems shrunk -----

    /// The Windows disk's layout (`matrix`'s `gpt-windows`): Windows' NTFS
    /// shrunk from 30 GiB to 20 GiB and kept at `/windows`, its EFI
    /// partition kept at `/efi`, `/boot`, `/` and swap made after it.
    fn windows(sector: u32) -> Vec<DiskLayout> {
        cases(sector)
            .into_iter()
            .find(|case| case.name == "gpt-windows")
            .expect("the Windows disk's layout")
            .layout
    }

    /// An ext4 of 30 GiB on a disk of its own, shrunk to 20 GiB and kept at
    /// `/data`; in what it gives up and after it, the EFI partition alone
    /// at `/boot` and `/`.
    fn ext4_kept(sector: u32) -> Vec<DiskLayout> {
        let disk = fixtures::disk_gpt_plain(sector);
        let mut one = DiskLayout {
            parts: disk
                .partitions
                .iter()
                .map(|was| layout::Part {
                    path: was.path.clone(),
                    number: was.number,
                    start: was.start,
                    size: was.size,
                    fate: Fate::Shrink {
                        to: 20 * crate::model::disks::GIB,
                        mount: Some(MountPoint::Other("/data".into())),
                    },
                })
                .collect(),
            disk,
            table: None,
            create: Vec::new(),
        };
        one.create = vec![
            layout::Create {
                start: 20481 * MIB,
                size: 2048 * MIB,
                fs: layout::Fs::Fat32,
                label: "EFI".into(),
                mount: Some(MountPoint::Boot),
            },
            layout::Create {
                start: 22529 * MIB,
                size: (65535 - 22529) * MIB,
                fs: layout::Fs::Ext4,
                label: "Linexin".into(),
                mount: Some(MountPoint::Root),
            },
        ];
        vec![one]
    }

    /// Whether anything was written to a disk: a filesystem shrunk, a table
    /// changed, a filesystem made. (Phase A's claim of a partition to be
    /// shrunk opens it exclusively and closes it again, and writes nothing.)
    fn wrote(ran: &Ran) -> bool {
        ran.lines().iter().any(|line| line.starts_with("write "))
    }

    /// The steps the run began, in order.
    fn begun(ran: &Ran) -> Vec<Step> {
        ran.events
            .iter()
            .filter_map(|event| match event {
                Event::Begin { step } => Some(*step),
                _ => None,
            })
            .collect()
    }

    /// The lines from the first that starts with `from` to the first after
    /// it that starts with `to`, both included.
    fn between(lines: &[String], from: &str, to: &str) -> Vec<String> {
        let start = lines.iter().position(|l| l.starts_with(from)).expect(from);
        let end = start
            + lines[start..]
                .iter()
                .position(|l| l.starts_with(to))
                .expect(to);
        lines[start..=end].to_vec()
    }

    /// **Windows is shrunk after everything is read again and dry-run, and
    /// before its entry** (Unit Q, Q9), word for word at 512 and 4096 bytes
    /// a sector: phase A reads the disk and its table, gives Windows'
    /// partition the fence's leave at its reviewed size, claims it, finds it
    /// in use nowhere, lists the disks again and reads every Windows volume
    /// of them — Windows' own, `ntfsresize --info` and its sign of
    /// hibernation, `hiberfil.sys` beginning `wake` — then the EFI
    /// partition's room and the dry run; only then, in Resize, the shrink
    /// itself (the fence, the claim, the session's mounts, swaps and facts
    /// again first, its answer `y` on standard input), its boot sector read
    /// back inside the new entry, and Done told Windows will check its disk
    /// — before Partition begins with `sfdisk -N 3`.
    #[test]
    fn windows_is_shrunk_after_everything_is_read_again_and_before_its_entry() {
        let _proven = PretendProven::these(EVERY_SET);
        for sector in [512, 4096] {
            let laid = windows(sector);
            let config = config_for(laid.clone(), 0);
            let ran = run(&config, Firmware::Uefi, Hand::new(&laid, Firmware::Uefi));
            assert!(
                ran.result.is_ok(),
                "{sector}: {:?}",
                ran.result.as_ref().err()
            );
            let lines = ran.lines();
            holds_the_order(&lines, "windows");
            assert_eq!(
                between(&lines, "read lsblk", "write sfdisk --lock -N 3 /dev/vdx"),
                [
                    "read lsblk --json --bytes --paths --nodeps -o NAME,SIZE,LOG-SEC /dev/vdx",
                    "read sfdisk --json /dev/vdx",
                    "fence",
                    "claim /dev/vdx3",
                    "mounts",
                    "swaps",
                    "facts /dev/vdx3",
                    "discover",
                    "read ntfsresize --info --no-progress-bar /dev/vdx3",
                    "head ntfscat /dev/vdx3 hiberfil.sys",
                    "esp /dev/vdx1",
                    "read ntfsresize --no-action --no-progress-bar --size 21474836480 /dev/vdx3",
                    "fence",
                    "claim /dev/vdx3",
                    "mounts",
                    "swaps",
                    "facts /dev/vdx3",
                    "write ntfsresize --no-progress-bar --size 21474836480 /dev/vdx3",
                    "sector /dev/vdx3",
                    "fence",
                    "claim /dev/vdx",
                    "write sfdisk --lock -N 3 /dev/vdx",
                ],
                "{sector}"
            );
            assert_eq!(
                begun(&ran),
                [Step::Resize, Step::Partition, Step::Format],
                "{sector}"
            );
            let warned: Vec<usize> = ran
                .events
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    matches!(
                        e,
                        Event::Warning {
                            warning: Warning::WindowsWillCheck
                        }
                    )
                })
                .map(|(at, _)| at)
                .collect();
            let partition = ran
                .events
                .iter()
                .position(|e| {
                    *e == Event::Begin {
                        step: Step::Partition,
                    }
                })
                .unwrap();
            assert_eq!(warned.len(), 1, "{sector}: said once");
            assert!(
                warned[0] < partition,
                "{sector}: said once Windows is shrunk"
            );
            // The fstab mounts the shrunk Windows only once Windows has run.
            let (_, mounted) = ran.result.as_ref().unwrap();
            assert!(
                mounted.fstab.contains("\t/windows\tntfs3\tnofail,"),
                "{sector}"
            );
            assert!(
                !lines.iter().any(|l| l.starts_with("mount ntfs")),
                "{sector}"
            );
            taken_out(ran, "windows");
        }
    }

    /// **A kept ext4 is checked, shrunk and read back before its entry**
    /// (Unit Q, Q9), at 512 and 4096 bytes a sector: phase A reads it again
    /// by its header and the least it shrinks to, and dry-runs `e2fsck -f
    /// -n`; Resize runs `e2fsck -f -y` and `resize2fs` to 20 GiB in KiB,
    /// each after the fence, its claim and a look at the session, and reads
    /// its header back inside the new entry; then `sfdisk -N 1`. No other
    /// disk is listed again (nothing is NTFS), no hibernation is asked, and
    /// Done is told nothing.
    #[test]
    fn a_kept_ext4_is_checked_shrunk_and_read_back_before_its_entry() {
        let _proven = PretendProven::these(EVERY_SET);
        for sector in [512, 4096] {
            let laid = ext4_kept(sector);
            let config = config_for(laid.clone(), 0);
            let ran = run(&config, Firmware::Uefi, Hand::new(&laid, Firmware::Uefi));
            assert!(
                ran.result.is_ok(),
                "{sector}: {:?}",
                ran.result.as_ref().err()
            );
            let lines = ran.lines();
            holds_the_order(&lines, "ext4");
            assert_eq!(
                between(&lines, "read lsblk", "write sfdisk --lock -N 1 /dev/vdx"),
                [
                    "read lsblk --json --bytes --paths --nodeps -o NAME,SIZE,LOG-SEC /dev/vdx",
                    "read sfdisk --json /dev/vdx",
                    "fence",
                    "claim /dev/vdx1",
                    "mounts",
                    "swaps",
                    "facts /dev/vdx1",
                    "read dumpe2fs -h /dev/vdx1",
                    "read resize2fs -P /dev/vdx1",
                    "read e2fsck -f -n /dev/vdx1",
                    "fence",
                    "claim /dev/vdx1",
                    "mounts",
                    "swaps",
                    "facts /dev/vdx1",
                    "write e2fsck -f -y /dev/vdx1",
                    "fence",
                    "claim /dev/vdx1",
                    "mounts",
                    "swaps",
                    "facts /dev/vdx1",
                    "write resize2fs /dev/vdx1 20971520K",
                    "read dumpe2fs -h /dev/vdx1",
                    "fence",
                    "claim /dev/vdx",
                    "write sfdisk --lock -N 1 /dev/vdx",
                ],
                "{sector}"
            );
            assert_eq!(lines.iter().filter(|l| *l == "discover").count(), 1);
            assert!(!lines.iter().any(|l| l.contains("ntfs")), "{sector}");
            assert_eq!(begun(&ran), [Step::Resize, Step::Partition, Step::Format]);
            assert!(!ran
                .events
                .iter()
                .any(|e| matches!(e, Event::Warning { .. })));
            let (_, mounted) = ran.result.as_ref().unwrap();
            assert!(
                mounted.fstab.contains("\t/data\text4\tdefaults,nofail,"),
                "{sector}"
            );
            taken_out(ran, "ext4");
        }
    }

    /// **An ext4 whose journal waits to be played back is refused by its
    /// header, though its dry run would pass it** (QEMU's
    /// `probe-resize-tools`: on the live system `e2fsck -f -n`
    /// skipped such a filesystem's journal and ended 0, so the dry run is no
    /// guard against it; `resize::E2FSCK_LOOK`), at 512 and 4096 bytes a
    /// sector: an ext4 whose `dumpe2fs -h` says its state is clean and names
    /// `needs_recovery`, as the live system's did, stops in Check as
    /// `FilesystemNeedsCheck` once its header is read again — its dry run
    /// never run, `resize2fs -P` not asked, nothing written and Resize not
    /// begun — while the hand-written machine's `e2fsck -f -n` on it ends 0,
    /// as the live system's did.
    #[test]
    fn an_ext4_whose_journal_waits_is_refused_by_its_header_though_its_dry_run_passes() {
        let _proven = PretendProven::these(EVERY_SET);
        for sector in [512, 4096] {
            let laid = ext4_kept(sector);
            let config = config_for(laid.clone(), 0);
            let mut hand = Hand::new(&laid, Firmware::Uefi);
            hand.filesystems.get_mut("/dev/vdx1").unwrap().clean = false;
            let look = hand.tool_said(&Command::new("e2fsck", &["-f", "-n", "/dev/vdx1"]));
            assert_eq!(look.code, 0, "{sector}: {look:?}");
            assert!(
                look.stdout.contains("skipping journal recovery"),
                "{sector}"
            );
            let ran = run(&config, Firmware::Uefi, hand);
            holds_the_order(&ran.lines(), "ext4 needing recovery");
            assert_eq!(
                ran.failure(),
                Failure::FilesystemNeedsCheck,
                "{sector}: {:?}",
                ran.result.as_ref().err()
            );
            let lines = ran.lines();
            assert!(
                lines.iter().any(|l| l == "read dumpe2fs -h /dev/vdx1"),
                "{sector}: {lines:?}"
            );
            for never in ["read e2fsck", "read resize2fs"] {
                assert!(
                    !lines.iter().any(|l| l.starts_with(never)),
                    "{sector}: {never} asked: {lines:?}"
                );
            }
            assert!(!wrote(&ran) && begun(&ran).is_empty(), "{sector}");
            taken_out(ran, "ext4 needing recovery");
        }
    }

    /// **Every reading again and every dry run comes before the first
    /// command that writes, and every filesystem is shrunk before its
    /// entry** (Unit Q, Q9): Windows' and the ext4's, at 512 and 4096.
    #[test]
    fn every_dry_run_comes_before_the_first_command_that_writes() {
        let _proven = PretendProven::these(EVERY_SET);
        for sector in [512, 4096] {
            for (name, laid, entry) in [
                (
                    "windows",
                    windows(sector),
                    "write sfdisk --lock -N 3 /dev/vdx",
                ),
                (
                    "ext4",
                    ext4_kept(sector),
                    "write sfdisk --lock -N 1 /dev/vdx",
                ),
            ] {
                let config = config_for(laid.clone(), 0);
                let ran = run(&config, Firmware::Uefi, Hand::new(&laid, Firmware::Uefi));
                let lines = ran.lines();
                let first = lines.iter().position(|l| l.starts_with("write ")).unwrap();
                let read_first = [
                    "read e2fsck -f -n",
                    "read ntfsresize --no-action",
                    "read ntfsresize --info",
                    "head ntfscat",
                    "read resize2fs -P",
                ];
                let mut dry = 0;
                for (at, said) in lines.iter().enumerate() {
                    if read_first.iter().any(|read| said.starts_with(read)) {
                        assert!(at < first, "{name} {sector}: {said} after a write");
                        dry += usize::from(said.contains("-n ") || said.contains("--no-action"));
                    }
                }
                assert_eq!(dry, 1, "{name} {sector}: one dry run");
                let shrunk = lines
                    .iter()
                    .rposition(|l| {
                        ["write resize2fs", "write ntfsresize"]
                            .iter()
                            .any(|w| l.starts_with(w))
                    })
                    .unwrap();
                let entry_at = lines.iter().position(|l| l == entry).unwrap();
                assert!(
                    shrunk < entry_at,
                    "{name} {sector}: the entry before its filesystem"
                );
                taken_out(ran, name);
            }
        }
    }

    /// Another disk beside the Windows disk, which the layout does not
    /// name: `/dev/vdy`, its partitions `(number, filesystem)` from 1 MiB on,
    /// 10 GiB each, NTFS ones Windows' content.
    fn other_disk(parts: &[(u32, &'static str)]) -> Disk {
        use crate::model::disks::Content;
        use crate::worker::partition::fixtures::{build, Spec, GPT_BASIC_DATA};
        let specs = parts
            .iter()
            .map(|(number, fs)| Spec {
                number: *number,
                start: MIB + u64::from(number - 1) * 10 * crate::model::disks::GIB,
                size: 10 * crate::model::disks::GIB,
                part_type: GPT_BASIC_DATA,
                content: Content::Windows,
                fs_type: fs,
            })
            .collect();
        let mut disk = build("/dev/vdy", 512, Some(Table::Gpt), specs, Vec::new());
        disk.major_minor = "254:16".into();
        disk
    }

    /// **A hibernated or unread Windows anywhere stops the shrink in Check,
    /// before anything is written** (Unit Q, Q9; the measure's rule across
    /// every disk, `Measures::shrink_of`, applied again on the worker's own
    /// reading), **each cause said as itself**: Windows' own `hiberfil.sys`
    /// signed hibernated, or another disk's Windows signed so, is
    /// `WindowsHibernated`; another disk's Windows whose sign is unreadable,
    /// and the installation medium's NTFS (never read), `WindowsUnread`;
    /// another disk's Windows in use `WindowsInUse`, marked for Windows'
    /// check `WindowsNeedsCheck`; a BitLocker volume anywhere `BitLocker`;
    /// ntfsresize or ntfscat missing from the live system `MissingTool`;
    /// Windows' own with no `hiberfil.sys`, or more used on it now than the
    /// 20 GiB leaves room for, `ShrinkRefused`; Windows' own marked for its
    /// own check, `FilesystemNeedsCheck`. In none is anything written, nor
    /// Resize begun; another disk's Windows read and shut down properly holds
    /// nothing back.
    #[test]
    fn a_hibernated_or_unread_windows_anywhere_stops_the_shrink_before_any_write() {
        let _proven = PretendProven::these(EVERY_SET);
        let laid = windows(512);
        let config = config_for(laid.clone(), 0);
        let hibr = Fsys::hiberfile(b"hibr");
        type Change = Box<dyn Fn(&mut Hand)>;
        let other = |parts: &'static [(u32, &'static str)]| -> Change {
            Box::new(move |hand: &mut Hand| {
                let disk = other_disk(parts);
                for part in &disk.partitions {
                    if part.fs_type == "ntfs" {
                        hand.filesystems
                            .insert(part.path.clone(), Fsys::windows(part.size));
                    }
                }
                hand.found.push(disk);
            })
        };
        let cases: Vec<(&str, Change, Option<Failure>)> = vec![
            (
                "Windows' own hibernated",
                Box::new(move |hand: &mut Hand| {
                    hand.filesystems.get_mut("/dev/vdx3").unwrap().sign = hibr.clone();
                }),
                Some(Failure::WindowsHibernated),
            ),
            (
                "another disk's Windows hibernated",
                Box::new(move |hand: &mut Hand| {
                    other(&[(1, "ntfs")])(hand);
                    hand.filesystems.get_mut("/dev/vdy1").unwrap().sign = Fsys::hiberfile(b"HIBR");
                }),
                Some(Failure::WindowsHibernated),
            ),
            (
                "another disk's Windows unread",
                Box::new(move |hand: &mut Hand| {
                    other(&[(1, "ntfs")])(hand);
                    hand.filesystems.get_mut("/dev/vdy1").unwrap().sign = Head {
                        bytes: Vec::new(),
                        ended: Some(false),
                        stderr: "Failed to read vcn 0x0 from inode 5: Input/output error\n".into(),
                    };
                }),
                Some(Failure::WindowsUnread),
            ),
            (
                "another disk's Windows in use",
                Box::new(move |hand: &mut Hand| {
                    other(&[(1, "ntfs")])(hand);
                    hand.mounts
                        .push_str("41 1 8:17 / /run/media/x rw - ntfs3 /dev/vdy1 rw\n");
                }),
                Some(Failure::WindowsInUse),
            ),
            (
                "another disk's Windows marked for its check",
                Box::new(move |hand: &mut Hand| {
                    other(&[(1, "ntfs")])(hand);
                    hand.filesystems.get_mut("/dev/vdy1").unwrap().clean = false;
                }),
                Some(Failure::WindowsNeedsCheck),
            ),
            (
                "a BitLocker volume on another disk",
                other(&[(1, "ntfs"), (2, "BitLocker")]),
                Some(Failure::BitLocker),
            ),
            (
                "the installation medium's NTFS",
                Box::new(move |hand: &mut Hand| {
                    other(&[(1, "ntfs")])(hand);
                    hand.found.last_mut().unwrap().blocked =
                        Some(crate::model::disks::Blocked::InstallationMedia);
                }),
                Some(Failure::WindowsUnread),
            ),
            (
                "ntfscat missing from the live system",
                Box::new(|hand: &mut Hand| hand.missing.push("ntfscat")),
                Some(Failure::MissingTool),
            ),
            (
                "ntfsresize missing from the live system",
                Box::new(|hand: &mut Hand| hand.missing.push("ntfsresize")),
                Some(Failure::MissingTool),
            ),
            (
                "Windows' own with no hiberfil.sys",
                Box::new(|hand: &mut Hand| {
                    hand.filesystems.get_mut("/dev/vdx3").unwrap().sign = Head {
                        bytes: Vec::new(),
                        ended: Some(false),
                        stderr: format!("{}\n", crate::worker::measure::ABSENT),
                    };
                }),
                Some(Failure::ShrinkRefused),
            ),
            (
                "Windows marked for its own check",
                Box::new(|hand: &mut Hand| {
                    hand.filesystems.get_mut("/dev/vdx3").unwrap().clean = false;
                }),
                Some(Failure::FilesystemNeedsCheck),
            ),
            (
                "more used on Windows than 20 GiB leaves room for",
                Box::new(|hand: &mut Hand| {
                    hand.filesystems.get_mut("/dev/vdx3").unwrap().used = 20_000_000_000;
                }),
                Some(Failure::ShrinkRefused),
            ),
            (
                "another disk's Windows shut down properly",
                other(&[(1, "ntfs")]),
                None,
            ),
        ];
        for (case, change, failure) in cases {
            let mut hand = Hand::new(&laid, Firmware::Uefi);
            change(&mut hand);
            let ran = run(&config, Firmware::Uefi, hand);
            holds_the_order(&ran.lines(), case);
            let lines = ran.lines();
            let medium = case.contains("medium");
            if case.contains("another disk") || case.contains("BitLocker") {
                // The other disk's Windows was asked about, and read where
                // it was not in use.
                assert!(lines.contains(&"facts /dev/vdy1".to_string()), "{case}");
                let read = lines
                    .iter()
                    .any(|l| l.starts_with("read ntfsresize --info --no-progress-bar /dev/vdy1"));
                assert_eq!(read, !case.contains("in use"), "{case}: {lines:?}");
            }
            if case.contains("BitLocker") {
                assert!(lines.contains(&"facts /dev/vdy2".to_string()), "{case}");
                assert!(
                    !lines
                        .iter()
                        .any(|l| l.contains("vdy2") && !l.starts_with("facts")),
                    "{case}"
                );
            }
            if medium {
                assert!(
                    !lines.iter().any(|l| l.contains("/dev/vdy")),
                    "{case}: {lines:?}"
                );
            }
            match failure {
                None => {
                    assert!(
                        ran.result.is_ok(),
                        "{case}: {:?}",
                        ran.result.as_ref().err()
                    );
                    assert!(wrote(&ran), "{case}");
                }
                Some(failure) => {
                    assert_eq!(
                        ran.failure(),
                        failure,
                        "{case}: {:?}",
                        ran.result.as_ref().err()
                    );
                    assert!(!wrote(&ran), "{case}: written: {lines:?}");
                    assert!(begun(&ran).is_empty(), "{case}: {:?}", begun(&ran));
                    let dry = lines
                        .iter()
                        .any(|l| l.starts_with("read ntfsresize --no-action"));
                    assert!(!dry, "{case}: dry-run all the same");
                }
            }
            taken_out(ran, case);
        }
    }

    /// **Phase A reads the Windows volumes for a minute at the most**
    /// (`shrinking::WINDOWS_PATIENCE`), on the hand-written
    /// machine's own clock, with Windows' disk and another of three Windows
    /// partitions: each reading is given what is left of the minute, and a
    /// volume reached once it is up is not read at all. Another disk's
    /// Windows whose `ntfsresize --info` would take 70 seconds is ended at
    /// the minute; one whose sign comes once 59 seconds have gone is given
    /// the one left and ended — each unread, the volumes after it never
    /// read, and every shrink held back as `WindowsUnread`, nothing written,
    /// no dry run. Every volume read in 59 seconds together goes through.
    #[test]
    fn phase_a_reads_the_windows_volumes_for_a_minute_at_the_most() {
        use std::time::Duration;
        let _proven = PretendProven::these(EVERY_SET);
        let laid = windows(512);
        let config = config_for(laid.clone(), 0);
        let secs = Duration::from_secs;
        let info = |device: &str| format!("ntfsresize --info --no-progress-bar {device}");
        let sign = |device: &str| format!("ntfscat {device} hiberfil.sys");
        type Case<'a> = (
            &'a str,
            Vec<(String, Duration)>,
            Option<Failure>,
            &'a [&'a str],
        );
        let cases: Vec<Case> = vec![
            (
                "another disk's Windows too slow to read",
                vec![(info("/dev/vdy1"), secs(70))],
                Some(Failure::WindowsUnread),
                &["/dev/vdy2", "/dev/vdy3"],
            ),
            (
                "the minute up during another Windows' sign",
                vec![
                    (info("/dev/vdx3"), secs(20)),
                    (info("/dev/vdy1"), secs(30)),
                    (sign("/dev/vdy1"), secs(5)),
                    (info("/dev/vdy2"), secs(4)),
                    (sign("/dev/vdy2"), secs(2)),
                ],
                Some(Failure::WindowsUnread),
                &["/dev/vdy3"],
            ),
            (
                "every Windows read within the minute",
                vec![
                    (info("/dev/vdx3"), secs(20)),
                    (info("/dev/vdy1"), secs(20)),
                    (info("/dev/vdy2"), secs(10)),
                    (info("/dev/vdy3"), secs(9)),
                ],
                None,
                &[],
            ),
        ];
        for (case, takes, failure, never) in cases {
            let mut hand = Hand::new(&laid, Firmware::Uefi);
            let disk = other_disk(&[(1, "ntfs"), (2, "ntfs"), (3, "ntfs")]);
            for part in &disk.partitions {
                hand.filesystems
                    .insert(part.path.clone(), Fsys::windows(part.size));
            }
            hand.found.push(disk);
            let takes: BTreeMap<String, Duration> = takes.into_iter().collect();
            hand.takes = takes.clone();
            let ran = run(&config, Firmware::Uefi, hand);
            holds_the_order(&ran.lines(), case);
            let lines = ran.lines();
            for device in never {
                assert!(
                    !lines
                        .iter()
                        .any(|l| l.ends_with(device) || l.contains(&format!("{device} "))),
                    "{case}: {device} was read: {lines:?}"
                );
            }
            // Each reading given exactly what was left of the minute.
            let limits = ran.said.borrow().limits.clone();
            assert_eq!(limits[0], (info("/dev/vdx3"), secs(60)), "{case}");
            let mut gone = Duration::ZERO;
            for (said, limit) in &limits {
                assert_eq!(*limit, secs(60) - gone, "{case}: {said}");
                gone += takes.get(said).copied().unwrap_or_default().min(*limit);
            }
            match failure {
                None => {
                    assert!(
                        ran.result.is_ok(),
                        "{case}: {:?}",
                        ran.result.as_ref().err()
                    );
                    assert!(wrote(&ran), "{case}");
                    assert_eq!(limits.len(), 8, "{case}: every volume read: {limits:?}");
                }
                Some(failure) => {
                    assert_eq!(
                        ran.failure(),
                        failure,
                        "{case}: {:?}",
                        ran.result.as_ref().err()
                    );
                    assert!(!wrote(&ran) && begun(&ran).is_empty(), "{case}");
                    assert!(
                        !lines
                            .iter()
                            .any(|l| l.starts_with("read ntfsresize --no-action")),
                        "{case}: dry-run all the same"
                    );
                }
            }
            if case.contains("during") {
                let given = limits.iter().find(|(l, _)| *l == sign("/dev/vdy2"));
                assert_eq!(given.map(|(_, limit)| *limit), Some(secs(1)), "{case}");
            }
            taken_out(ran, case);
        }
    }

    /// The Windows layout's and the ext4's runs, a command line ending as
    /// `ending` has it.
    fn ending_as(laid: &[DiskLayout], said: &[(&str, Status)]) -> Ran {
        let config = config_for(laid.to_vec(), 0);
        let mut hand = Hand::new(laid, Firmware::Uefi);
        for (line, status) in said {
            hand.ending.insert(line.to_string(), status.clone());
        }
        run(&config, Firmware::Uefi, hand)
    }

    /// **A dry run, or a shrink, that ends badly stops with what its tool
    /// said** (Unit Q, Q9; `shrinking::failure_of`): in Check, before
    /// anything is written, a dry run that finds a check due is
    /// `FilesystemNeedsCheck` and one its tool refuses `ShrinkRefused`; in
    /// Resize, a shrink that finds a check due is `FilesystemNeedsCheck`,
    /// one that asks for a restart, says it did nothing, or leaves a
    /// filesystem that does not fit its new entry `ShrinkRefused` — and
    /// nothing is written after it, no entry changed.
    #[test]
    fn a_dry_run_or_a_shrink_that_ends_badly_stops_with_what_its_tool_said() {
        let _proven = PretendProven::these(EVERY_SET);
        let ntfs_dry = "read ntfsresize --no-action --no-progress-bar --size 21474836480 /dev/vdx3";
        let ntfs = "write ntfsresize --no-progress-bar --size 21474836480 /dev/vdx3";
        let line_of = |said: &str| said.split_once(' ').unwrap().1.to_string();
        type Case<'a> = (&'a str, Vec<DiskLayout>, &'a str, Status, Failure, bool);
        let cases: Vec<Case> = vec![
            (
                "an NTFS too full for its dry run",
                windows(512),
                ntfs_dry,
                ended(1, "", "ERROR: New size can't be less than the space already occupied by data.\n"),
                Failure::ShrinkRefused,
                false,
            ),
            (
                "an NTFS whose dry run finds Windows' mark",
                windows(512),
                ntfs_dry,
                ended(1, "", "ERROR: Volume is scheduled for check.\nRun chkdsk /f and please try again, or see option -f.\n"),
                Failure::FilesystemNeedsCheck,
                false,
            ),
            (
                "an ext4 whose dry run finds errors",
                ext4_kept(512),
                "read e2fsck -f -n /dev/vdx1",
                ended(4, "", ""),
                Failure::FilesystemNeedsCheck,
                false,
            ),
            (
                "an NTFS found inconsistent by its shrink",
                windows(512),
                ntfs,
                ended(1, "NTFS is inconsistent. Run chkdsk /f on Windows then reboot it TWICE!\n", ""),
                Failure::FilesystemNeedsCheck,
                true,
            ),
            (
                "an NTFS shrink that says it had nothing to do",
                windows(512),
                ntfs,
                ended(0, "Nothing to do: NTFS volume size is already OK.\n", ""),
                Failure::ShrinkRefused,
                true,
            ),
            (
                "an NTFS that says it is resized, its boot sector not",
                windows(512),
                ntfs,
                ended(0, "Successfully resized NTFS on device '/dev/vdx3'.\n", ""),
                Failure::ShrinkRefused,
                true,
            ),
            (
                "an ext4 check that asks for a restart",
                ext4_kept(512),
                "write e2fsck -f -y /dev/vdx1",
                ended(2, "", ""),
                Failure::ShrinkRefused,
                true,
            ),
            (
                "an ext4 resize2fs asks to check first",
                ext4_kept(512),
                "write resize2fs /dev/vdx1 20971520K",
                ended(1, "", "Please run 'e2fsck -f /dev/vdx1' first.\n\n"),
                Failure::FilesystemNeedsCheck,
                true,
            ),
            (
                "an ext4 that says it is resized, its header not",
                ext4_kept(512),
                "write resize2fs /dev/vdx1 20971520K",
                ended(0, "The filesystem on /dev/vdx1 is now 5242880 (4k) blocks long.\n", ""),
                Failure::ShrinkRefused,
                true,
            ),
        ];
        for (case, laid, said, status, failure, in_resize) in cases {
            let ran = ending_as(&laid, &[(&line_of(said), status)]);
            holds_the_order(&ran.lines(), case);
            assert_eq!(
                ran.failure(),
                failure,
                "{case}: {:?}",
                ran.result.as_ref().err()
            );
            let lines = ran.lines();
            let at = lines
                .iter()
                .position(|l| l == said)
                .unwrap_or_else(|| panic!("{case}: {said}"));
            let written_after = lines[at + 1..].iter().any(|l| l.starts_with("write "));
            assert!(!written_after, "{case}: written past it: {lines:?}");
            if in_resize {
                assert_eq!(begun(&ran), [Step::Resize], "{case}");
            } else {
                assert!(!wrote(&ran) && begun(&ran).is_empty(), "{case}");
            }
            assert!(
                !lines.iter().any(|l| l.starts_with("write sfdisk")),
                "{case}"
            );
            taken_out(ran, case);
        }
    }

    /// **A filesystem its new entry already holds has only its entry made
    /// smaller** (Partitions and the worker agree on a filesystem smaller
    /// than its partition, `Shrinkable::entry_only`), at 512 and 4096 bytes a
    /// sector: the ext4 read again as exactly the 20 GiB its entry is to be
    /// given — what a run that stopped between its shrink and its entry left,
    /// retried — or as 18 GiB, a partition grown without its filesystem; and
    /// Windows' volume as a shrink to 20 GiB left it, retried once Windows
    /// has checked it. Each goes through with no tool given it — no dry run,
    /// no e2fsck, resize2fs or ntfsresize writing, nothing read back after,
    /// no Resize begun, no word of Windows' check — the fence handed what
    /// phase A read of it (dumpe2fs' blocks; the NTFS boot sector, read in
    /// phase A, after every Windows volume and the EFI partition's room), and
    /// its entry made smaller by `sfdisk -N`. An NTFS whose boot sector
    /// counts more than its new entry holds whole, where ntfsresize counts
    /// less, is refused in Check: `ShrinkRefused`, nothing written.
    #[test]
    fn a_filesystem_its_new_entry_already_holds_has_only_its_entry_made_smaller() {
        let _proven = PretendProven::these(EVERY_SET);
        let gib = crate::model::disks::GIB;
        for sector in [512, 4096] {
            for (case, laid, device, bytes, entry) in [
                (
                    "an ext4 retried",
                    ext4_kept(sector),
                    "/dev/vdx1",
                    20 * gib,
                    "write sfdisk --lock -N 1 /dev/vdx",
                ),
                (
                    "an ext4 smaller than its partition",
                    ext4_kept(sector),
                    "/dev/vdx1",
                    18 * gib,
                    "write sfdisk --lock -N 1 /dev/vdx",
                ),
                (
                    "Windows retried",
                    windows(sector),
                    "/dev/vdx3",
                    20 * gib - 4096,
                    "write sfdisk --lock -N 3 /dev/vdx",
                ),
            ] {
                let case = format!("{case} at {sector}");
                let config = config_for(laid.clone(), 0);
                let mut hand = Hand::new(&laid, Firmware::Uefi);
                hand.filesystems.get_mut(device).unwrap().bytes = bytes;
                let ran = run(&config, Firmware::Uefi, hand);
                assert!(
                    ran.result.is_ok(),
                    "{case}: {:?}",
                    ran.result.as_ref().err()
                );
                let lines = ran.lines();
                holds_the_order(&lines, &case);
                for tool in [
                    "read e2fsck",
                    "read ntfsresize --no-action",
                    "write e2fsck",
                    "write resize2fs",
                    "write ntfsresize",
                ] {
                    assert!(
                        !lines.iter().any(|l| l.starts_with(tool)),
                        "{case}: {tool}: {lines:?}"
                    );
                }
                let at = |said: &str| lines.iter().position(|l| l == said);
                let entry_at = at(entry).unwrap_or_else(|| panic!("{case}: {lines:?}"));
                if device == "/dev/vdx1" {
                    let header = format!("read dumpe2fs -h {device}");
                    assert_eq!(
                        lines.iter().filter(|l| **l == header).count(),
                        1,
                        "{case}: read once, in phase A"
                    );
                    assert!(at(&header).unwrap() < entry_at, "{case}");
                } else {
                    let sector_at = at("sector /dev/vdx3").expect("its boot sector read");
                    let esp_at = at("esp /dev/vdx1").unwrap();
                    assert!(esp_at < sector_at && sector_at < entry_at, "{case}");
                }
                assert_eq!(begun(&ran), [Step::Partition, Step::Format], "{case}");
                assert!(
                    !ran.events
                        .iter()
                        .any(|e| matches!(e, Event::Warning { .. })),
                    "{case}"
                );
                let logged = ran.events.iter().any(|e| {
                    matches!(e, Event::Log { line } if line.contains(&format!("{device}'s filesystem is ")) && line.ends_with("only its entry is made smaller"))
                });
                assert!(logged, "{case}");
                taken_out(ran, &case);
            }
        }
        // The boot sector counts the volume past its new entry, though
        // ntfsresize counts it inside: refused before anything is written.
        let laid = windows(512);
        let config = config_for(laid.clone(), 0);
        let mut hand = Hand::new(&laid, Firmware::Uefi);
        let windows = hand.filesystems.get_mut("/dev/vdx3").unwrap();
        windows.bytes = 20 * gib;
        windows.counted = Some(20 * gib - 3584);
        let ran = run(&config, Firmware::Uefi, hand);
        assert_eq!(ran.failure(), Failure::ShrinkRefused);
        let detail = &ran.result.as_ref().err().unwrap().detail;
        assert!(detail.contains("does not hold whole"), "{detail}");
        assert!(ran.lines().contains(&"sector /dev/vdx3".to_string()));
        assert!(!wrote(&ran) && begun(&ran).is_empty());
        taken_out(ran, "Windows' boot sector past its entry");
    }

    /// **A partition formatted at another size has only its entry
    /// resized** (Unit Q, Q9): every fixture layout that formats a partition
    /// and gives its entry another size, and shrinks nothing, is carried out
    /// with no filesystem tool asked anything of it — no reading again, no
    /// dry run, no e2fsck, resize2fs or ntfsresize — no Resize begun, and
    /// its entry given its new size by `sfdisk -N` alone.
    #[test]
    fn a_partition_formatted_at_another_size_has_only_its_entry_resized() {
        let _proven = PretendProven::these(EVERY_SET);
        let mut held = 0;
        for sector in [512, 4096] {
            for case in cases(sector) {
                let parts = || case.layout.iter().flat_map(|one| &one.parts);
                let resized = parts().any(
                    |part| matches!(part.fate, Fate::Format { size, .. } if size != part.size),
                );
                let shrinks = parts().any(|part| matches!(part.fate, Fate::Shrink { .. }));
                if !resized || shrinks {
                    continue;
                }
                let config = config_for(case.layout.clone(), 0);
                let ran = run(
                    &config,
                    case.firmware,
                    Hand::new(&case.layout, case.firmware),
                );
                assert!(ran.result.is_ok(), "{}", case.name);
                let lines = ran.lines();
                for tool in ["e2fsck", "resize2fs", "ntfsresize", "ntfscat", "dumpe2fs"] {
                    assert!(
                        !lines.iter().any(|l| l.contains(tool)),
                        "{}: {tool} was asked",
                        case.name
                    );
                }
                assert!(
                    lines
                        .iter()
                        .any(|l| l.starts_with("write sfdisk --lock -N ")),
                    "{}",
                    case.name
                );
                assert!(!begun(&ran).contains(&Step::Resize), "{}", case.name);
                assert!(!crate::worker::manual::planned_steps(&config).contains(&Step::Resize));
                held += 1;
                taken_out(ran, case.name);
            }
        }
        assert!(
            held >= 6,
            "{held} fixture layouts format a partition at another size"
        );
    }
}
