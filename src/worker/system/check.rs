//! What the worker makes sure of before it writes anything.
//!
//! The interface checked all of this already, as the person; the worker
//! checks it again as root, against the machine as it is now, because the
//! interface is not trusted with a disk (a bug there must not be able to
//! erase something the person never agreed to) and because the machine can
//! change between the review and the press of Install — a stick pulled out,
//! a partition mounted by the file manager.
use super::safe_token;
use crate::model::config::{Filesystem, InstallConfig, Software, Target};
use crate::model::disks::{self, Blocked, Disk, Partition};
use crate::model::layout::{self, Fs};
use crate::model::live::LiveSession;
use crate::model::protocol::{Failure, Request, LOCK_FILE, MAX_REQUEST};
use crate::worker::layout::{Firmware, Stop};
use crate::worker::manual::resize::Tool;
use crate::worker::{manual, run};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

/// Makes the worker process hard to read from outside: nothing it creates is
/// readable by others, no core dump can carry the password to a disk, and no
/// unprivileged process may attach to it.
pub fn harden() {
    // SAFETY: plain system calls on this process with constant arguments.
    unsafe {
        libc::umask(0o077);
        libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
        let none = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::setrlimit(libc::RLIMIT_CORE, &none);
        // A session of its own: Ctrl+C in the terminal the interface was
        // started from, or that terminal closing, must not reach a worker
        // that is half way through writing a partition table.
        libc::setsid();
    }
}

pub fn is_root() -> bool {
    // SAFETY: geteuid cannot fail.
    unsafe { libc::geteuid() == 0 }
}

/// Takes the whole-run lock, or `AlreadyRunning`. The file is root's alone
/// and never opened through a symlink; the lock is released when the
/// returned file is dropped or the process ends, however it ends.
pub fn lock() -> Result<File, Stop> {
    let internal = |detail: String| Stop::new(Failure::Internal, detail);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(LOCK_FILE)
        .map_err(|e| internal(format!("cannot open {LOCK_FILE}: {e}")))?;
    let meta = file.metadata().map_err(|e| internal(e.to_string()))?;
    if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o077 != 0 {
        return Err(internal(format!(
            "{LOCK_FILE} has unsafe ownership or permissions"
        )));
    }
    // SAFETY: flock on a descriptor this function owns.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(Stop::new(
            Failure::AlreadyRunning,
            "another installation holds the lock",
        ));
    }
    Ok(file)
}

/// Reads the one request from `reader`, at most [`MAX_REQUEST`] bytes. No
/// error says anything about what was read — the bytes hold the passwords —
/// and the buffer is overwritten before it is let go. `reader` should be
/// unbuffered, or its own buffer keeps a copy.
pub fn read_request(reader: impl Read) -> Result<Request, Stop> {
    // Made at its largest size at once, so that reading never moves the
    // bytes to a bigger buffer and leaves an unwiped copy behind.
    let mut bytes = Vec::with_capacity(MAX_REQUEST as usize + 1);
    let read = reader.take(MAX_REQUEST + 1).read_to_end(&mut bytes);
    let result = match read {
        Err(error) => Err(Stop::new(
            Failure::InvalidRequest,
            format!("the request could not be read: {}", error.kind()),
        )),
        Ok(_) if bytes.len() as u64 > MAX_REQUEST => Err(Stop::new(
            Failure::InvalidRequest,
            "the request is larger than the worker accepts",
        )),
        Ok(_) => serde_json::from_slice::<Request>(&bytes).map_err(|error| {
            // Only where and what kind of mistake: serde's own message can
            // quote the value it choked on, which may be a password.
            Stop::new(
                Failure::InvalidRequest,
                format!(
                    "the request is not a valid installation ({:?} at line {}, column {})",
                    error.classify(),
                    error.line(),
                    error.column()
                ),
            )
        }),
    };
    wipe(&mut bytes);
    result
}

/// Overwrites a buffer that held a secret before it is freed.
pub fn wipe(bytes: &mut [u8]) {
    for byte in bytes.iter_mut() {
        // SAFETY: a valid, exclusive reference to each byte in turn.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
}

/// Everything in the request that can be judged without the image: the
/// account, the region's shape, the storage choices for this firmware, the
/// consent list, and the names of packages and Flatpaks ([`judged`]).
///
/// A layout made by hand is held to everything the interface held it to
/// and taken only where all it needs lies within one set QEMU proved
/// (`layout::PROVEN`, the QEMU campaign of Unit Q).
pub fn validate(request: &Request, firmware: Firmware) -> Result<(), Stop> {
    judged(request, firmware)
}

/// [`validate`], in order: the account and the region; the names of
/// packages and Flatpaks; the storage — for a layout made by hand,
/// `disks::validate_storage` hands it to the rule set the interface judges
/// a plan by, so the worker refuses whatever that refuses (the pins,
/// `layout::shape` on this firmware with the boot loader and restore points
/// asked for — every label within `layout::LABEL_REFUSES` among it — and the
/// image's path); then, for a layout, everything it needs within one set
/// QEMU proved — a mix one scenario that passed ran whole
/// (`layout::unproven`); then the consent, which must be exactly the list
/// of what it destroys that the person read. What needs reading with
/// privilege (`layout::measured`) is phase A's, on the worker's own
/// numbers.
fn judged(request: &Request, firmware: Firmware) -> Result<(), Stop> {
    let config = &request.config;
    let invalid = |what: crate::model::config::Invalid| {
        Stop::new(
            Failure::InvalidRequest,
            format!("the request does not validate: {what:?}"),
        )
    };
    crate::model::config::validate_account(&config.account).map_err(invalid)?;
    crate::model::config::validate_region(&config.region).map_err(invalid)?;
    software_names(&config.software)?;
    disks::validate_storage(config, firmware).map_err(invalid)?;
    if let Target::Layout { disks: laid } = &config.target {
        if let Some(unproven) = layout::unproven(laid, firmware) {
            return Err(Stop::new(Failure::InvalidRequest, unproven.line()));
        }
    }
    let expected = disks::consent(config);
    if expected != request.consent {
        return Err(Stop::new(
            Failure::ConsentMismatch,
            format!(
                "the request would erase {expected:?}, but the person agreed to {:?}",
                request.consent
            ),
        ));
    }
    Ok(())
}

/// A pacman package name: lower case letters, digits and `@._+-`, not
/// starting with `-` or `.`.
pub fn valid_package(name: &str) -> bool {
    safe_token(name) && !name.starts_with('.') && name.bytes().all(|b| !b.is_ascii_uppercase())
}

/// A Flatpak application ID: reverse-DNS, at least three parts of letters,
/// digits, `_` and `-`, the first character of each part not a digit or `-`.
pub fn valid_flatpak(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    id.len() <= 255
        && parts.len() >= 3
        && parts.iter().all(|part| {
            let mut bytes = part.bytes();
            match bytes.next() {
                Some(first) if first.is_ascii_alphabetic() || first == b'_' => {
                    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                }
                _ => false,
            }
        })
}

fn software_names(software: &Software) -> Result<(), Stop> {
    if let Some(bad) = software.remove.iter().position(|p| !valid_package(p)) {
        return Err(Stop::new(
            Failure::InvalidRequest,
            format!("package name number {} is not a package name", bad + 1),
        ));
    }
    if let Some(bad) = software.flatpaks.iter().position(|f| !valid_flatpak(f)) {
        return Err(Stop::new(
            Failure::InvalidRequest,
            format!("Flatpak number {} is not an application ID", bad + 1),
        ));
    }
    Ok(())
}

/// The image to copy must be the live session's own: the one archiso
/// mounted from the medium, not any other path the request names.
pub fn same_image(live: &LiveSession, config: &InstallConfig) -> Result<(), Stop> {
    let canonical = |path: &Path| std::fs::canonicalize(path).ok();
    match (canonical(&live.image), canonical(&config.source)) {
        (Some(live_image), Some(source)) if live_image == source => Ok(()),
        _ => Err(Stop::new(
            Failure::ImageInvalid,
            format!(
                "the request copies {}, but this live session runs from {}",
                config.source.display(),
                live.image.display()
            ),
        )),
    }
}

/// `SecureBoot-…` holds four bytes of attributes and then the value.
pub fn secure_boot_on(variable: &[u8]) -> bool {
    variable.get(4) == Some(&1)
}

/// Whether the firmware has Secure Boot on. A machine whose variables cannot
/// be read is taken as off: what this protects is a system that would not
/// start, not this machine, and the log says the variables were unreadable.
pub fn secure_boot() -> bool {
    let Ok(entries) = std::fs::read_dir("/sys/firmware/efi/efivars") else {
        run::log("UEFI variables are not readable; taking Secure Boot as off");
        return false;
    };
    for entry in entries.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("SecureBoot-")
        {
            return std::fs::read(entry.path())
                .map(|bytes| secure_boot_on(&bytes))
                .unwrap_or(false);
        }
    }
    false
}

/// The live system's programs this installation runs (the installed
/// system's own are checked in the image). A layout made by hand adds the
/// tool of each filesystem it makes that the easy choices do not already
/// bring (`manual::makes`): xfs, Btrfs beside an ext4 `/`, FAT32 on a BIOS,
/// a swap partition; and where it shrinks a kept filesystem
/// (`layout::shrunk`, step Q9), the programs that read it again and shrink it
/// — dumpe2fs, e2fsck and resize2fs for an ext4, ntfsresize and ntfscat for
/// an NTFS — so a live system without them stops in Check as `MissingTool`,
/// not as a shrink refused. An easy request's list is as it was. (Every
/// string in this function is a program's name: packaging/check.sh reads them
/// all.)
pub fn required_tools(config: &InstallConfig, firmware: Firmware) -> Vec<&'static str> {
    let mut tools = vec![
        // Both halves: discovery, partitioning and formatting.
        "lsblk",
        "blkid",
        "findmnt",
        "sfdisk",
        "wipefs",
        "partx",
        "udevadm",
        "mount",
        "umount",
        "mkfs.ext4",
        // The system half.
        "arch-chroot",
        "rsync",
        "du",
        "sync",
        "systemctl",
        "timeout",
    ];
    if firmware == Firmware::Uefi {
        tools.extend(["mkfs.fat", "efibootmgr"]);
    }
    if config.storage.filesystem == Filesystem::Btrfs {
        tools.extend(["mkfs.btrfs", "btrfs"]);
    }
    if config.storage.swap_mib > 0 {
        tools.push("mkswap");
    }
    if config.software.prune_drivers {
        tools.push("lspci");
    }
    if let Target::Layout { disks: laid } = &config.target {
        let mut add = |programs: &[&'static str]| {
            for program in programs {
                if !tools.contains(program) {
                    tools.push(program);
                }
            }
        };
        if manual::makes(laid, Fs::Xfs) {
            add(&["mkfs.xfs"]);
        }
        if manual::makes(laid, Fs::Btrfs) {
            add(&["mkfs.btrfs", "btrfs"]);
        }
        if manual::makes(laid, Fs::Fat32) {
            add(&["mkfs.fat"]);
        }
        if manual::makes(laid, Fs::LinuxSwap) {
            add(&["mkswap"]);
        }
        let shrunk: Vec<Option<Tool>> = layout::shrunk(laid)
            .iter()
            .map(|part| Tool::of(&part.fs_type))
            .collect();
        if shrunk.contains(&Some(Tool::Ext4)) {
            add(&["dumpe2fs", "e2fsck", "resize2fs"]);
        }
        if shrunk.contains(&Some(Tool::Ntfs)) {
            add(&["ntfsresize", "ntfscat"]);
        }
    }
    tools
}

pub fn tools(config: &InstallConfig, firmware: Firmware) -> Result<(), Stop> {
    for tool in required_tools(config, firmware) {
        run::command_path(tool)?;
    }
    Ok(())
}

/// Gives the worker a mount namespace of its own. Every mount from here on —
/// the target, arch-chroot's binds, os-prober's looks at other partitions —
/// stays inside it and disappears with the worker; nothing the live session
/// mounts later reaches in.
pub fn isolate_mounts() -> Result<(), Stop> {
    // SAFETY: unshare with a constant flag affects only this process.
    if unsafe { libc::unshare(libc::CLONE_NEWNS) } != 0 {
        return Err(Stop::new(
            Failure::Internal,
            format!(
                "cannot isolate the installer's mounts: {}",
                std::io::Error::last_os_error()
            ),
        ));
    }
    run::run("mount", &["--make-rprivate", "/"]).map(|_| ())
}

/// The fields that make a partition the one the person reviewed. Mount
/// points are judged separately: a newly mounted partition is busy, not a
/// different partition. Its filesystem's UUID is compared as the worker
/// compares every UUID ([`disks::same_uuid`], without case).
fn same_partition(a: &Partition, b: &Partition) -> bool {
    a.path == b.path
        && a.number == b.number
        && a.start == b.start
        && a.size == b.size
        && a.fs_type == b.fs_type
        && a.label == b.label
        && a.part_type == b.part_type
        && disks::same_uuid(&a.uuid, &b.uuid)
}

/// Whether `now` is still the disk the person reviewed as `before`, and if
/// not, why: `DiskChanged` for a different or repartitioned disk, `DiskBusy`
/// for the same disk with something on it now in use.
pub fn same_disk(before: &Disk, now: &Disk) -> Result<(), (Failure, String)> {
    let identity = before.path == now.path
        && before.diskseq == now.diskseq
        && before.model == now.model
        && before.serial == now.serial
        && before.size == now.size
        && before.major_minor == now.major_minor
        && before.logical_sector == now.logical_sector
        && before.transport == now.transport;
    if !identity {
        return Err((
            Failure::DiskChanged,
            format!("{} is not the disk that was reviewed", before.path),
        ));
    }
    // What the disk holds is told apart from which disk it is: a new
    // partition table, or something written over the whole disk, is the same
    // disk repartitioned, and the detail says so rather than that another
    // disk took its place.
    let partitions = before.table == now.table
        && before.content == now.content
        && before.partitions.len() == now.partitions.len()
        && before
            .partitions
            .iter()
            .zip(&now.partitions)
            .all(|(a, b)| same_partition(a, b));
    if !partitions {
        return Err((
            Failure::DiskChanged,
            format!("the partitions on {} changed", before.path),
        ));
    }
    match &now.blocked {
        // The size is part of the identity above, so a disk too small now
        // was too small when it was reviewed: whether that matters (it does
        // for Linexin's own disk, not for one that keeps the person's files)
        // is the storage checks' judgement, made after this.
        None | Some(Blocked::TooSmall) => {}
        Some(Blocked::InUse { mountpoints }) => {
            return Err((
                Failure::DiskBusy,
                format!("{} is in use at {mountpoints:?}", now.path),
            ))
        }
        Some(Blocked::Swap) | Some(Blocked::Holder) => {
            return Err((Failure::DiskBusy, format!("{} is in use", now.path)))
        }
        Some(other) => {
            return Err((
                Failure::DiskChanged,
                format!("{} can no longer be installed to: {other:?}", now.path),
            ))
        }
    }
    if now.read_only {
        return Err((
            Failure::DiskChanged,
            format!("{} is read-only now", now.path),
        ));
    }
    Ok(())
}

/// Finds the reviewed disk again and compares it field by field, then runs
/// the storage checks once more against what is there now.
pub fn disk_unchanged(config: &InstallConfig, firmware: Firmware) -> Result<(), Stop> {
    let disks = disks::discover().map_err(|detail| {
        Stop::new(
            Failure::DiskChanged,
            format!("the disks cannot be read: {detail}"),
        )
    })?;
    let now = disks
        .iter()
        .find(|d| d.path == config.disk.path)
        .cloned()
        .ok_or_else(|| {
            Stop::new(
                Failure::DiskChanged,
                format!("{} is gone", config.disk.path),
            )
        })?;
    same_disk(&config.disk, &now).map_err(|(failure, detail)| Stop::new(failure, detail))?;
    let mut fresh = config.clone();
    fresh.disk = now;
    // A /home on another disk: that disk found again and compared the same
    // way, so a partition formatted there is the one the person was shown.
    if let Some(home) = fresh.storage.home.as_mut() {
        if let Some(before) = home.disk.as_mut() {
            let now = disks
                .iter()
                .find(|d| d.path == before.path)
                .ok_or_else(|| {
                    Stop::new(Failure::DiskChanged, format!("{} is gone", before.path))
                })?;
            same_disk(before, now).map_err(|(failure, detail)| Stop::new(failure, detail))?;
            *before = now.clone();
        }
    }
    disks::validate_storage(&fresh, firmware).map_err(|invalid| {
        Stop::new(
            Failure::DiskChanged,
            format!("the disk no longer suits the request: {invalid:?}"),
        )
    })
}

/// **Every disk of a layout made by hand found again** (§7.0, Unit Q, "The
/// worker" 1): each compared field by field with the disk as it was
/// reviewed (`same_disk`: `DiskChanged` for another or a repartitioned
/// disk, one gone, or one that can no longer be written; `DiskBusy` for one
/// with something on it in use now), then the layout judged again on the
/// disks as they are now — each substituted for its reviewed self, the
/// request's own `disk` too — by the rule set and the pins
/// (`disks::validate_storage`), a refusal there a `DiskChanged`. `found` is
/// what `disks::discover` lists now, handed in: the worker reads it, a test
/// writes it, and no test lists this computer's disks.
pub fn layout_unchanged(
    config: &InstallConfig,
    firmware: Firmware,
    found: Result<Vec<Disk>, String>,
) -> Result<(), Stop> {
    let disks = found.map_err(|detail| {
        Stop::new(
            Failure::DiskChanged,
            format!("the disks cannot be read: {detail}"),
        )
    })?;
    let mut fresh = config.clone();
    let root_disk = fresh.disk.path.clone();
    let Target::Layout { disks: laid } = &mut fresh.target else {
        return Err(Stop::new(
            Failure::Internal,
            "the request carries no layout",
        ));
    };
    let mut root_now = None;
    for one in laid.iter_mut() {
        let now = disks
            .iter()
            .find(|disk| disk.path == one.disk.path)
            .ok_or_else(|| Stop::new(Failure::DiskChanged, format!("{} is gone", one.disk.path)))?;
        same_disk(&one.disk, now).map_err(|(failure, detail)| Stop::new(failure, detail))?;
        if now.path == root_disk {
            root_now = Some(now.clone());
        }
        one.disk = now.clone();
    }
    if let Some(now) = root_now {
        fresh.disk = now;
    }
    disks::validate_storage(&fresh, firmware).map_err(|invalid| {
        Stop::new(
            Failure::DiskChanged,
            format!("the disks no longer suit the layout: {invalid:?}"),
        )
    })
}

/// The devices a layout made by hand must find free before anything is
/// written: every disk it writes to, whole, with each partition on it as
/// reviewed — the claim on the whole disk before each write fails anyway
/// while any is in use, but this says which — and, on a disk it only
/// mounts from, each partition it uses. Disks in path order.
pub fn layout_devices(laid: &[crate::model::layout::DiskLayout]) -> Vec<String> {
    let mut disks: Vec<&crate::model::layout::DiskLayout> = laid.iter().collect();
    disks.sort_by(|one, other| one.disk.path.cmp(&other.disk.path));
    let mut devices = Vec::new();
    for one in disks {
        if manual::plan::writes_to(one) {
            devices.push(one.disk.path.clone());
            devices.extend(one.disk.partitions.iter().map(|part| part.path.clone()));
        } else {
            let used = one.parts.iter().filter(|part| part.fate.mount().is_some());
            devices.extend(used.map(|part| part.path.clone()));
        }
    }
    devices
}

/// What [`device_facts`] answers of one device: its `major:minor` and
/// whether anything holds it, or why that cannot be said.
pub type DeviceFacts = Result<(String, bool), String>;

/// **Nothing a layout made by hand writes or mounts is in use**
/// ([`layout_devices`]): none mounted in the live session, none active
/// swap, none held by RAID, encryption or device-mapper — the rules
/// [`not_busy`] holds the easy path to, one device at a time by
/// [`device_busy`] — else `DiskBusy`, before anything is written. What it
/// reads of the machine is handed in: `mountinfo` (`/proc/1/mountinfo`,
/// which a worker in a namespace of its own still reads as the session's),
/// `swaps` (`/proc/swaps`) and `facts`, each device's `major:minor` and
/// whether anything holds it ([`device_facts`]), whose failure — a device
/// gone, holders that cannot be read — is a refusal too.
pub fn layout_not_busy(
    config: &InstallConfig,
    mountinfo: Result<String, String>,
    swaps: &str,
    facts: &mut dyn FnMut(&str) -> DeviceFacts,
) -> Result<(), Stop> {
    let busy = |detail: String| Stop::new(Failure::DiskBusy, detail);
    let Target::Layout { disks: laid } = &config.target else {
        return Err(Stop::new(
            Failure::Internal,
            "the request carries no layout",
        ));
    };
    let mountinfo =
        mountinfo.map_err(|e| busy(format!("cannot read the live system's mounts: {e}")))?;
    for device in layout_devices(laid) {
        let (major_minor, held) = facts(&device).map_err(busy)?;
        if let Some(why) = device_busy(&device, &major_minor, held, &mountinfo, swaps) {
            return Err(busy(why));
        }
    }
    Ok(())
}

/// Whether `/proc/<pid>/mountinfo` text shows the device mounted: by its
/// `major:minor` (field 3), or by name as the mount source — Btrfs mounts
/// carry an anonymous device number in field 3, so the number alone would
/// miss them.
pub fn mountinfo_uses(mountinfo: &str, major_minor: &str, device: &str) -> bool {
    mountinfo.lines().any(|line| {
        let mut fields = line.split_whitespace();
        if fields.nth(2) == Some(major_minor) {
            return true;
        }
        // After the optional fields comes " - fstype source options".
        match line.split_once(" - ") {
            Some((_, rest)) => rest.split_whitespace().nth(1) == Some(device),
            None => false,
        }
    })
}

/// Whether `/proc/swaps` lists the device as active swap.
pub fn swaps_use(swaps: &str, device: &str) -> bool {
    swaps
        .lines()
        .skip(1)
        .any(|line| line.split_whitespace().next() == Some(device))
}

/// The devices this installation writes: the disk, its partitions, and a
/// separate /home elsewhere.
fn written_devices(config: &InstallConfig) -> Vec<String> {
    let mut devices = vec![config.disk.path.clone()];
    devices.extend(config.disk.partitions.iter().map(|p| p.path.clone()));
    if let Some(home) = &config.storage.home {
        if !devices.contains(&home.path) {
            devices.push(home.path.clone());
        }
    }
    devices
}

/// Refuses when any device this installation writes is mounted in the live
/// session (which the worker's own namespace no longer shows), active swap,
/// or held by RAID, encryption or device-mapper.
///
/// In a test build it refuses before anything is read — the session's
/// mounts and swap, and the `dev` and `holders` of each device under
/// `/sys/class/block` (`run::fence::block_devices`): no test reads this
/// computer's block devices, and the busy rules are tested on text
/// ([`device_busy`]).
pub fn not_busy(config: &InstallConfig) -> Result<(), Stop> {
    #[cfg(test)]
    run::fence::block_devices();
    let busy = |detail: String| Stop::new(Failure::DiskBusy, detail);
    let mountinfo = std::fs::read_to_string("/proc/1/mountinfo")
        .map_err(|e| busy(format!("cannot read the live system's mounts: {e}")))?;
    let swaps = std::fs::read_to_string("/proc/swaps").unwrap_or_default();
    for device in written_devices(config) {
        let name = Path::new(&device)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| busy(format!("{device} is not a device path")))?;
        let class = Path::new("/sys/class/block").join(name);
        let major_minor = std::fs::read_to_string(class.join("dev"))
            .map_err(|e| busy(format!("{device} is not a block device: {e}")))?;
        if mountinfo_uses(&mountinfo, major_minor.trim(), &device) {
            return Err(busy(format!("{device} is mounted in the live session")));
        }
        if swaps_use(&swaps, &device) {
            return Err(busy(format!("{device} is active swap")));
        }
        let held = std::fs::read_dir(class.join("holders"))
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);
        if held {
            return Err(busy(format!("{device} is held by another device")));
        }
    }
    Ok(())
}

/// Why one device is in use, or `None` when nothing uses it: mounted in the
/// live session (`mountinfo` is `/proc/1/mountinfo`, the session's own, which
/// a process in a namespace of its own still reads), active swap (`swaps` is
/// `/proc/swaps`), or `held` by something stacked on it — RAID, encryption
/// or device-mapper, `/sys/class/block/<name>/holders` not being empty. The
/// questions [`not_busy`] asks of every device an installation writes, put
/// for one device and answered from text, so that the measure entry judges a
/// partition busy by the same rules before it reads anything of it, and the
/// judgement is tested on text written by hand. `major_minor` is the
/// device's own, from `/sys/class/block/<name>/dev` ([`device_facts`]).
pub fn device_busy(
    device: &str,
    major_minor: &str,
    held: bool,
    mountinfo: &str,
    swaps: &str,
) -> Option<String> {
    if mountinfo_uses(mountinfo, major_minor.trim(), device) {
        return Some(format!("{device} is mounted in the live session"));
    }
    if swaps_use(swaps, device) {
        return Some(format!("{device} is active swap"));
    }
    if held {
        return Some(format!("{device} is held by another device"));
    }
    None
}

/// What [`device_busy`] judges a device by that only the device's own
/// directory under `/sys/class/block` says: its `major:minor`, and whether
/// anything holds it. Read-only. A device that is not there, or whose
/// holders cannot be read, is an error rather than a guess: what is not
/// known to be free is not read. In a test build it refuses before
/// anything is read (`run::fence::block_devices`): a test hands its own
/// facts in.
pub fn device_facts(device: &str) -> Result<(String, bool), String> {
    #[cfg(test)]
    run::fence::block_devices();
    let name = Path::new(device)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|_| device.starts_with("/dev/"))
        .ok_or_else(|| format!("{device} is not a device path"))?;
    let class = Path::new("/sys/class/block").join(name);
    let major_minor = std::fs::read_to_string(class.join("dev"))
        .map_err(|e| format!("{device} is not a block device: {e}"))?;
    let held = std::fs::read_dir(class.join("holders"))
        .map(|mut entries| entries.next().is_some())
        .map_err(|e| format!("the holders of {device} cannot be read: {e}"))?;
    Ok((major_minor.trim().to_string(), held))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::{Content, Table, Transport};
    use crate::worker::partition::fixtures::layout_request;

    fn disk() -> Disk {
        Disk {
            path: "/dev/vdz".into(),
            diskseq: Some(42),
            model: "Example Disk".into(),
            serial: "EXAMPLE-0001".into(),
            size: 64 * disks::GIB,
            major_minor: "254:0".into(),
            logical_sector: 512,
            transport: Transport::Virtio,
            removable: false,
            rotational: false,
            table: Some(Table::Gpt),
            content: None,
            read_only: false,
            blocked: None,
            partitions: vec![Partition {
                path: "/dev/vdz1".into(),
                number: 1,
                start: disks::MIB,
                size: 100 * disks::MIB,
                fs_type: "vfat".into(),
                label: "SYSTEM".into(),
                part_type: "c12a7328-f81f-11d2-ba4b-00a0c93ec93b".into(),
                uuid: "ABCD-1234".into(),
                mountpoints: vec![],
                content: Content::Efi,
                bootable: false,
            }],
            free: vec![],
        }
    }

    #[test]
    fn the_request_is_bounded_and_never_echoed() {
        let error = read_request(vec![b'{'; MAX_REQUEST as usize + 1].as_slice())
            .err()
            .expect("refused");
        assert_eq!(error.failure, Failure::InvalidRequest);
        let error = read_request(br#"{"password":"do-not-echo"}"#.as_slice())
            .err()
            .expect("refused");
        assert_eq!(error.failure, Failure::InvalidRequest);
        assert!(!error.detail.contains("do-not-echo"));
        let error = read_request(br#"{"config": "do-not-echo"}"#.as_slice())
            .err()
            .expect("refused");
        assert!(!error.detail.contains("do-not-echo"));
    }

    #[test]
    fn package_and_flatpak_names_are_checked() {
        assert!(valid_package("gnome-maps"));
        assert!(valid_package("lib32-nvidia-utils"));
        for bad in ["", "-Rdd", "Gnome", ".hidden", "a b", "a/b"] {
            assert!(!valid_package(bad), "{bad:?}");
        }
        assert!(valid_flatpak("app.zen_browser.zen"));
        assert!(valid_flatpak("com.heroicgameslauncher.hgl"));
        for bad in [
            "",
            "org.example",
            "org..example",
            "org.example.-x",
            "org.example.a b",
        ] {
            assert!(!valid_flatpak(bad), "{bad:?}");
        }
    }

    #[test]
    fn secure_boot_is_the_byte_after_the_attributes() {
        assert!(secure_boot_on(&[6, 0, 0, 0, 1]));
        assert!(!secure_boot_on(&[6, 0, 0, 0, 0]));
        assert!(!secure_boot_on(&[]));
    }

    #[test]
    fn a_changed_or_busy_disk_is_told_apart() {
        let before = disk();
        assert!(same_disk(&before, &before.clone()).is_ok());

        let mut replugged = before.clone();
        replugged.diskseq = Some(43);
        assert_eq!(
            same_disk(&before, &replugged).unwrap_err().0,
            Failure::DiskChanged
        );

        let mut repartitioned = before.clone();
        repartitioned.partitions[0].size += disks::MIB;
        assert_eq!(
            same_disk(&before, &repartitioned).unwrap_err().0,
            Failure::DiskChanged
        );

        // A blank disk that was given a partition table after the review is
        // the same disk with new partitions, and the detail says that.
        let mut blank = before.clone();
        blank.table = None;
        blank.partitions.clear();
        let (failure, detail) = same_disk(&blank, &before).unwrap_err();
        assert_eq!(failure, Failure::DiskChanged);
        assert!(detail.contains("partitions"), "{detail}");
        let (_, detail) = same_disk(&before, &replugged).unwrap_err();
        assert!(detail.contains("not the disk"), "{detail}");

        // A disk too small for Linexin is still the same disk; the storage
        // checks judge whether it may be used for what is asked of it.
        let mut small = before.clone();
        small.blocked = Some(Blocked::TooSmall);
        assert!(same_disk(&small, &small.clone()).is_ok());

        let mut written_over = before.clone();
        written_over.content = Some(Content::Linux);
        assert_eq!(
            same_disk(&before, &written_over).unwrap_err().0,
            Failure::DiskChanged
        );

        let mut mounted = before.clone();
        mounted.partitions[0].mountpoints = vec!["/run/media/example".into()];
        mounted.blocked = Some(Blocked::InUse {
            mountpoints: vec!["/run/media/example".into()],
        });
        assert_eq!(
            same_disk(&before, &mounted).unwrap_err().0,
            Failure::DiskBusy
        );
    }

    /// **A UUID is the same whatever its case** (Unit Q, Q6): a
    /// partition found again with its filesystem's UUID written in other
    /// letters is the one reviewed — the way the executor compares every
    /// UUID (`disks::same_uuid`) — and one with another UUID is not.
    #[test]
    fn a_partitions_uuid_is_compared_without_case() {
        let before = disk();
        let mut now = before.clone();
        now.partitions[0].uuid = "abcd-1234".into();
        assert_eq!(same_disk(&before, &now), Ok(()));
        now.partitions[0].uuid = "abcd-1235".into();
        assert_eq!(
            same_disk(&before, &now).unwrap_err().0,
            Failure::DiskChanged
        );
        assert!(disks::same_uuid("ABCD-1234", "abcd-1234"));
        assert!(!disks::same_uuid("ABCD-1234", "ABCD-12340"));
    }

    /// **No test reads this computer's block devices for the busy checks**
    /// (Unit Q, Q6): `device_facts`, the executor's and the
    /// measure entry's reader of `/sys/class/block/<name>/{dev,holders}`,
    /// and the easy path's `not_busy`, which reads the session's mounts and
    /// the same two files of every device it writes, each refuse in a test
    /// build before anything is read. (Checked against its fault by
    /// inspection only — each refusal is the reader's first statement — and
    /// by the strace record of the test run: a fence taken out would have a
    /// test read the machine.)
    #[test]
    fn no_test_reads_this_computers_block_devices_for_the_busy_checks() {
        use crate::worker::run::fence::refusal;
        let config = crate::worker::partition::fixtures::config(disk(), Target::WholeDisk);
        for words in [
            refusal(|| device_facts("/dev/vdz1")),
            refusal(|| not_busy(&config)),
        ] {
            let words = words.expect("refused");
            assert!(
                words.contains("test build: no test reads this computer's /sys/class/block"),
                "{words}"
            );
        }
    }

    #[test]
    fn mounts_are_found_by_number_and_by_name() {
        let mountinfo = "\
22 1 0:21 / /proc rw,nosuid shared:5 - proc proc rw
41 1 254:2 / /mnt/data rw,relatime shared:1 - ext4 /dev/vdz2 rw
57 1 0:48 /@ /mnt/other rw,relatime shared:30 - btrfs /dev/vdz3 rw,subvol=/@
";
        assert!(mountinfo_uses(mountinfo, "254:2", "/dev/vdz2"));
        assert!(mountinfo_uses(mountinfo, "254:3", "/dev/vdz3"));
        assert!(!mountinfo_uses(mountinfo, "254:1", "/dev/vdz1"));
        let swaps = "Filename\tType\tSize\tUsed\tPriority\n/dev/vdz4 partition 1024 0 -2\n";
        assert!(swaps_use(swaps, "/dev/vdz4"));
        assert!(!swaps_use(swaps, "/dev/vdz1"));
    }

    /// One partition judged busy by the rules `not_busy` holds a whole
    /// installation to, every fact written by hand: nothing of this machine
    /// is read (`device_facts`, which reads `/sys`, is not called).
    #[test]
    fn one_device_is_busy_by_the_same_rules() {
        let mountinfo = "\
22 1 0:21 / /proc rw,nosuid shared:5 - proc proc rw
41 1 254:2 / /mnt/data rw,relatime shared:1 - ext4 /dev/vdz2 rw
57 1 0:48 /@ /mnt/other rw,relatime shared:30 - btrfs /dev/vdz3 rw,subvol=/@
";
        let swaps = "Filename\tType\tSize\tUsed\tPriority\n/dev/vdz4 partition 1024 0 -2\n";
        let busy = |device: &str, major_minor: &str, held: bool| {
            device_busy(device, major_minor, held, mountinfo, swaps)
        };
        // By its number, with sysfs's newline still on it.
        let mounted = busy("/dev/vdz2", "254:2\n", false).expect("mounted");
        assert!(mounted.contains("mounted"), "{mounted}");
        // By its name, as Btrfs is mounted.
        assert!(busy("/dev/vdz3", "254:3", false).is_some());
        let swap = busy("/dev/vdz4", "254:4", false).expect("swap");
        assert!(swap.contains("swap"), "{swap}");
        let held = busy("/dev/vdz5", "254:5", true).expect("held");
        assert!(held.contains("held"), "{held}");
        assert_eq!(busy("/dev/vdz1", "254:1", false), None);
    }

    #[test]
    fn tools_follow_the_choices() {
        // A configuration needs a disk; the tool list does not look at it.
        let config = InstallConfig {
            disk: disk(),
            target: crate::model::config::Target::WholeDisk,
            storage: crate::model::config::Storage {
                filesystem: Filesystem::Btrfs,
                swap_mib: 0,
                home: None,
            },
            bootloader: Default::default(),
            desktop: Default::default(),
            software: Software {
                prune_drivers: false,
                ..Default::default()
            },
            region: Default::default(),
            account: Default::default(),
            source: crate::model::config::DEFAULT_SOURCE.into(),
        };
        let bios = required_tools(&config, Firmware::Bios);
        assert!(bios.contains(&"btrfs"));
        assert!(!bios.contains(&"efibootmgr"));
        assert!(!bios.contains(&"mkswap"));
        assert!(!bios.contains(&"lspci"));
        assert!(required_tools(&config, Firmware::Uefi).contains(&"efibootmgr"));
    }

    /// `required_tools` as it was before a layout made by hand could add its
    /// own (Unit Q, Q5), word for word.
    fn required_tools_before(config: &InstallConfig, firmware: Firmware) -> Vec<&'static str> {
        let mut tools = vec![
            "lsblk",
            "blkid",
            "findmnt",
            "sfdisk",
            "wipefs",
            "partx",
            "udevadm",
            "mount",
            "umount",
            "mkfs.ext4",
            "arch-chroot",
            "rsync",
            "du",
            "sync",
            "systemctl",
            "timeout",
        ];
        if firmware == Firmware::Uefi {
            tools.extend(["mkfs.fat", "efibootmgr"]);
        }
        if config.storage.filesystem == Filesystem::Btrfs {
            tools.extend(["mkfs.btrfs", "btrfs"]);
        }
        if config.storage.swap_mib > 0 {
            tools.push("mkswap");
        }
        if config.software.prune_drivers {
            tools.push("lspci");
        }
        tools
    }

    /// A blank fixture disk laid out by hand under a new gpt, making each
    /// of `made` (a filesystem and where it is used) one after another.
    fn laid_out(made: &[(crate::model::layout::Fs, &str)]) -> InstallConfig {
        use crate::model::disks::{Table, GIB};
        use crate::model::layout::{Create, DiskLayout, MountPoint};
        use crate::worker::partition::fixtures::{config, disk_empty};
        let disk = disk_empty(512);
        let create = made
            .iter()
            .enumerate()
            .map(|(index, (fs, at))| Create {
                start: (1 + 4 * index as u64) * GIB,
                size: 4 * GIB,
                fs: *fs,
                label: String::new(),
                mount: (!at.is_empty()).then(|| MountPoint::at(at)),
            })
            .collect();
        let mut config = config(disk.clone(), Target::WholeDisk);
        config.storage.swap_mib = 0;
        config.target = Target::Layout {
            disks: vec![DiskLayout {
                disk,
                table: Some(Table::Gpt),
                parts: Vec::new(),
                create,
            }],
        };
        config
    }

    /// **The tools an easy request needs are as they were, and a layout made
    /// by hand adds those of the filesystems it makes** (Unit Q, Q5): for
    /// every fixture request — each fixture disk at 512 and 4096 bytes a
    /// sector, every place on it, ext4 and Btrfs, with a swap file and
    /// without, unused drivers removed or kept, on either firmware — the
    /// list is the code's before, in its order; a layout adds `mkfs.xfs`
    /// for xfs, Btrfs' tools beside an ext4 `/`, `mkfs.fat` for FAT32 where
    /// the firmware brings none, `mkswap` for a swap partition, each once —
    /// and (Q9) for a kept ext4 it shrinks dumpe2fs, e2fsck and resize2fs,
    /// for a kept NTFS ntfsresize and ntfscat, after what it makes.
    #[test]
    fn the_tools_an_easy_request_needs_are_as_they_were_and_a_layout_adds_its_own() {
        use crate::model::layout::Fs;
        use crate::worker::partition::fixtures::{all, config, targets};
        for sector in [512, 4096] {
            for (name, disk) in all(sector) {
                for (place, target) in targets(&disk) {
                    let mut easy = config(disk.clone(), target);
                    for filesystem in [Filesystem::Ext4, Filesystem::Btrfs] {
                        for swap_mib in [0, 4096] {
                            for prune_drivers in [false, true] {
                                easy.storage.filesystem = filesystem;
                                easy.storage.swap_mib = swap_mib;
                                easy.software.prune_drivers = prune_drivers;
                                for firmware in [Firmware::Uefi, Firmware::Bios] {
                                    assert_eq!(
                                        required_tools(&easy, firmware),
                                        required_tools_before(&easy, firmware),
                                        "{name} {sector} {place} {filesystem:?} swap {swap_mib} \
                                         prune {prune_drivers} {firmware:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        let added = |config: &InstallConfig, firmware: Firmware| -> Vec<&'static str> {
            let before = required_tools_before(config, firmware);
            let now = required_tools(config, firmware);
            assert_eq!(
                &now[..before.len()],
                before.as_slice(),
                "the easy list first"
            );
            now[before.len()..].to_vec()
        };
        let plain = laid_out(&[(Fs::Fat32, "/boot"), (Fs::Ext4, "/")]);
        assert!(added(&plain, Firmware::Uefi).is_empty());
        assert_eq!(added(&plain, Firmware::Bios), ["mkfs.fat"]);
        let every = laid_out(&[
            (Fs::Fat32, "/boot"),
            (Fs::Ext4, "/"),
            (Fs::Xfs, "/home"),
            (Fs::LinuxSwap, "swap"),
            (Fs::LinuxSwap, ""),
            (Fs::Btrfs, "/data"),
            (Fs::Btrfs, "/srv"),
        ]);
        assert_eq!(
            added(&every, Firmware::Uefi),
            ["mkfs.xfs", "mkfs.btrfs", "btrfs", "mkswap"]
        );
        // What the easy choices already bring is not named twice.
        let mut btrfs_root = every.clone();
        btrfs_root.storage.filesystem = Filesystem::Btrfs;
        btrfs_root.storage.swap_mib = 1024;
        assert_eq!(added(&btrfs_root, Firmware::Bios), ["mkfs.xfs", "mkfs.fat"]);
        let tools = required_tools(&btrfs_root, Firmware::Bios);
        for tool in &tools {
            assert_eq!(tools.iter().filter(|t| *t == tool).count(), 1, "{tool}");
        }
        // What shrinks a kept filesystem: the matrix's fixtures that keep
        // Windows smaller, and an ext4 kept smaller beside it.
        use crate::model::layout::{DiskLayout, Fate};
        let shrinking = |laid: Vec<DiskLayout>| {
            let mut asked = plain.clone();
            asked.target = Target::Layout { disks: laid };
            asked
        };
        let windows = manual::matrix::cases(512)
            .into_iter()
            .find(|case| case.name == "gpt-windows")
            .expect("the Windows disk's layout")
            .layout;
        assert!(!layout::shrunk(&windows).is_empty());
        assert_eq!(
            added(&shrinking(windows.clone()), Firmware::Uefi),
            ["mkswap", "ntfsresize", "ntfscat"]
        );
        let mut ext4 = windows.clone();
        let data = crate::worker::partition::fixtures::disk_gpt_plain(512);
        ext4.push(DiskLayout {
            parts: data
                .partitions
                .iter()
                .map(|was| layout::Part {
                    path: was.path.clone(),
                    number: was.number,
                    start: was.start,
                    size: was.size,
                    fate: Fate::Shrink {
                        to: was.size / 2 / disks::MIB * disks::MIB,
                        mount: None,
                    },
                })
                .collect(),
            disk: data,
            table: None,
            create: Vec::new(),
        });
        assert_eq!(
            added(&shrinking(ext4), Firmware::Bios),
            [
                "mkfs.fat",
                "mkswap",
                "dumpe2fs",
                "e2fsck",
                "resize2fs",
                "ntfsresize",
                "ntfscat"
            ]
        );
    }

    /// Before it writes, the worker finds the reviewed disk again with
    /// `disks::discover`, which starts lsblk itself. In a test build that
    /// asks the fence and is refused, so no test of this check (nor of a
    /// later one that finds disks again) lists this computer's disks. With
    /// `fence::FindNothing` held, were the question lost the check would
    /// find no lsblk and stop as `DiskChanged`, and this test would fail.
    #[test]
    fn the_disk_found_again_is_fenced_in_a_test_build() {
        use crate::model::config::Target;
        use crate::worker::partition::fixtures::config;
        use crate::worker::run::fence;
        let config = config(disk(), Target::WholeDisk);
        let _nothing = fence::FindNothing::here();
        let words = fence::refusal(|| disk_unchanged(&config, Firmware::Uefi))
            .expect("the disk was looked for past the fence");
        assert!(
            words.contains("test build: lsblk is not a program"),
            "{words}"
        );
    }

    /// **The worker judges a layout made by hand whole, and takes it only
    /// where QEMU proved it** (§7.0, Unit Q, Q2; the campaign): one that
    /// passes every rule there is for it — its easy fields its pins, its
    /// shape, its consent the very list of what it destroys — at 512 and
    /// 4096 bytes a sector, read whole from the bytes the interface would
    /// write. With no account it is refused for that, as any request is;
    /// with one, a new gpt on a blank disk is taken under UEFI at 512 bytes
    /// a sector, as `manual-uefi-empty` ran it, and refused at 4096 for the
    /// disk's sectors alone (`LargeSectors`: every scenario's disks had
    /// 512-byte ones); under BIOS the rule set itself refuses it at both,
    /// before anything proven is asked (its gpt disk has no `bios_grub`
    /// partition for GRUB's core); and the easy path's planner refuses it,
    /// were it ever handed one.
    #[test]
    fn the_worker_judges_a_hand_made_layout_whole() {
        use crate::model::layout;
        use crate::worker::partition::{plan, SharedRoom};
        for sector in [512, 4096] {
            let request = layout_request(sector);
            assert_eq!(
                disks::validate_storage(&request.config, Firmware::Uefi),
                Ok(())
            );
            assert_eq!(layout::pinned(&request.config), Ok(()));
            assert_eq!(request.consent, disks::consent(&request.config));
            assert_eq!(request.consent, ["table /dev/vdx gpt"]);
            let bytes = serde_json::to_vec(&request).unwrap();
            let read = read_request(bytes.as_slice())
                .unwrap_or_else(|stop| panic!("{:?}: {}", stop.failure, stop.detail));
            for firmware in [Firmware::Uefi, Firmware::Bios] {
                let stop = validate(&read, firmware).expect_err("no account was taken");
                assert_eq!(stop.failure, Failure::InvalidRequest);
                assert!(
                    stop.detail.starts_with("the request does not validate:"),
                    "{}",
                    stop.detail
                );
            }
            let accounted = blank_request(sector);
            let bytes = serde_json::to_vec(&accounted).unwrap();
            let read = read_request(bytes.as_slice())
                .unwrap_or_else(|stop| panic!("{:?}: {}", stop.failure, stop.detail));
            if sector == 512 {
                if let Err(stop) = validate(&read, Firmware::Uefi) {
                    panic!("{:?}: {}", stop.failure, stop.detail);
                }
            } else {
                let stop = validate(&read, Firmware::Uefi).expect_err("taken at 4096");
                assert_eq!(stop.failure, Failure::InvalidRequest);
                assert_eq!(
                    stop.detail,
                    "the layout needs LargeSectors, which this build has not proven"
                );
            }
            let stop = validate(&read, Firmware::Bios).expect_err("taken under BIOS");
            assert_eq!(stop.failure, Failure::InvalidRequest);
            assert_eq!(stop.detail, "the request does not validate: LayoutRefused");
            let Target::Layout { disks: laid } = &read.config.target else {
                unreachable!()
            };
            let config = &read.config;
            let shaped = layout::shape(laid, Firmware::Bios, config.bootloader, &config.software);
            assert!(
                matches!(shaped, Err(layout::Refusal::BiosGrubMissing { .. })),
                "{shaped:?}"
            );
            for shared in [SharedRoom::Kernels, SharedRoom::LoaderOnly] {
                let planned = plan(&read.config, Firmware::Uefi, shared);
                let stop = planned.expect_err("the easy planner planned a layout");
                assert_eq!(stop.failure, Failure::InvalidRequest);
            }
        }
    }

    /// A request carrying `laid` as the interface builds one: its easy
    /// fields its pins (no swap file asked for), an account and a region
    /// that pass, and its consent every line of what it destroys.
    fn request_for(laid: Vec<crate::model::layout::DiskLayout>) -> Request {
        use crate::model::config::{Account, Storage};
        use crate::model::secret::Secret;
        use crate::worker::partition::fixtures::config;
        let pins = layout::pins(&laid, 0).expect("the layout pins");
        let mut config = config(pins.disk, Target::Layout { disks: laid });
        config.storage = Storage {
            filesystem: pins.filesystem,
            swap_mib: pins.swap_mib,
            home: None,
        };
        config.account = Account {
            full_name: "Zoë Example".into(),
            username: "zoe".into(),
            password: Secret::new("Example-2026"),
            hostname: "zoe-pc".into(),
            root_password: None,
        };
        Request {
            consent: disks::consent(&config),
            config,
        }
    }

    /// The storage fixtures' request for a blank disk laid out by hand
    /// (`fixtures::layout_request`), with an account and a region that pass.
    fn blank_request(sector: u32) -> Request {
        let Target::Layout { disks: laid } = layout_request(sector).config.target else {
            unreachable!()
        };
        request_for(laid)
    }

    /// Every capability there is.
    const EVERY_CAPABILITY: &[layout::Capability] = layout::EVERY_CAPABILITY;
    /// One set holding every capability, as if a scenario had needed them
    /// all: every layout lies within it.
    const EVERY_SET: &[&[layout::Capability]] = &[EVERY_CAPABILITY];

    /// The fixture layouts the executor's tests carry out (`manual::matrix`,
    /// each held to the rule set) at 512 and 4096 bytes a sector, and the
    /// request the storage fixtures make for a blank disk.
    fn every_fixture_request() -> Vec<(String, Firmware, Request)> {
        use crate::worker::manual::matrix::cases;
        let mut every = Vec::new();
        for sector in [512, 4096] {
            for case in cases(sector) {
                let name = format!("{} at {sector}", case.name);
                every.push((name, case.firmware, request_for(case.layout)));
            }
            let name = format!("layout_request at {sector}");
            every.push((name, Firmware::Uefi, blank_request(sector)));
        }
        every
    }

    /// **With nothing proven, every layout is refused** (Unit Q, Q5) — as
    /// it would be were no set of the campaign written, pretended so here
    /// (`layout::PretendProven`): each fixture layout, every rule it can be
    /// held to passing, is refused by `validate` for the first capability
    /// it needs, which QEMU would then not have proven.
    #[test]
    fn with_nothing_proven_every_layout_is_refused() {
        let _none = layout::PretendProven::these(&[]);
        for (name, firmware, request) in every_fixture_request() {
            let Target::Layout { disks: laid } = &request.config.target else {
                unreachable!()
            };
            assert_eq!(
                disks::validate_storage(&request.config, firmware),
                Ok(()),
                "{name}"
            );
            let first = layout::needs(laid, firmware)[0];
            let refused = validate(&request, firmware).expect_err(&name);
            assert_eq!(refused.failure, Failure::InvalidRequest, "{name}");
            assert_eq!(
                refused.detail,
                format!("the layout needs {first:?}, which this build has not proven"),
                "{name}"
            );
        }
    }

    /// **The worker judges a proven layout whole** (Unit Q, Q5), with every
    /// capability pretended proven (`layout::PretendProven`): with every
    /// capability a fixture layout needs proven it passes, while one
    /// capability left unproven refuses it for that one, and a consent that
    /// is not exactly what it destroys, in its order, is a `ConsentMismatch`
    /// — an empty one too, where a layout (Q7's DATA kept, partitions made
    /// after it) destroys nothing: a line more is refused as any is.
    #[test]
    fn a_proven_layout_is_judged_whole() {
        let mut nothing_destroyed = 0;
        for (name, firmware, request) in every_fixture_request() {
            let Target::Layout { disks: laid } = &request.config.target else {
                unreachable!()
            };
            let needs = layout::needs(laid, firmware);
            {
                let _proven = layout::PretendProven::these(EVERY_SET);
                if let Err(stop) = validate(&request, firmware) {
                    panic!("{name}: {:?}: {}", stop.failure, stop.detail);
                }
                let mut changed = Vec::new();
                if request.consent.is_empty() {
                    nothing_destroyed += 1;
                } else {
                    let mut fewer = request.clone();
                    fewer.consent.pop();
                    changed.push(fewer);
                }
                let mut more = request.clone();
                more.consent
                    .push("delete /dev/vdz1 1 start=0 size=0 -".into());
                changed.push(more);
                if request.consent.len() > 1 {
                    let mut turned = request.clone();
                    turned.consent.reverse();
                    changed.push(turned);
                }
                let mut easy = request.clone();
                easy.consent = vec![request.config.disk.path.clone()];
                changed.push(easy);
                for other in changed {
                    let stop = validate(&other, firmware).expect_err(&name);
                    assert_eq!(stop.failure, Failure::ConsentMismatch, "{name}");
                }
            }
            for left_out in &needs {
                let rest: Vec<layout::Capability> = EVERY_CAPABILITY
                    .iter()
                    .copied()
                    .filter(|capability| capability != left_out)
                    .collect();
                let _proven = layout::PretendProven::these(vec![&*rest.leak()].leak());
                let stop = validate(&request, firmware).expect_err(&name);
                assert_eq!(
                    stop.detail,
                    format!("the layout needs {left_out:?}, which this build has not proven"),
                    "{name}"
                );
            }
        }
        assert_eq!(nothing_destroyed, 2, "msdos-keep-fat at 512 and 4096");
    }

    /// **The worker takes only a mix one scenario ran whole** (decision 20):
    /// with `PROVEN` itself — every set of the campaign's table, each of its
    /// scenarios having passed — each fixture layout whose needs lie within
    /// one set is judged whole, and each whose needs lie within none —
    /// every capability of it in some set — is refused saying the mix it
    /// needs, or, where one capability of it no set holds (msdos-extended's
    /// extended frame emptied and deleted; gpt-plain's and msdos' primary
    /// partition formatted and grown, which no scenario grows), saying that
    /// one; fixtures of each kind are among them. At 4096 bytes a sector
    /// none is taken: each is refused alone, for the disk's sectors
    /// (`LargeSectors`, which no scenario ran) where nothing earlier in
    /// `Capability`'s order is held by no set. Pretended proven by nothing,
    /// as were no set written, each is refused for the first capability it
    /// needs.
    #[test]
    fn only_a_mix_one_scenario_ran_is_taken() {
        let sets = layout::PROVEN;
        assert!(layout::CAMPAIGN.iter().all(|(_, set)| sets.contains(set)));
        let (mut taken, mut refused, mut never_said, mut large) = (0, 0, 0, 0);
        for (name, firmware, request) in every_fixture_request() {
            let Target::Layout { disks: laid } = &request.config.target else {
                unreachable!()
            };
            let needs = layout::needs(laid, firmware);
            {
                let _none = layout::PretendProven::these(&[]);
                assert_eq!(
                    validate(&request, firmware).expect_err(&name).detail,
                    layout::Unproven::Never(needs[0]).line(),
                    "{name}"
                );
            }
            let within = |set: &&[layout::Capability]| needs.iter().all(|one| set.contains(one));
            let on_large = name.ends_with(" at 4096");
            assert_eq!(
                needs.contains(&layout::Capability::LargeSectors),
                on_large,
                "{name}"
            );
            if sets.iter().any(within) {
                assert!(!on_large, "{name}: taken on 4096-byte sectors");
                taken += 1;
                if let Err(stop) = validate(&request, firmware) {
                    panic!("{name}: {needs:?}: {:?}: {}", stop.failure, stop.detail);
                }
            } else {
                refused += 1;
                let stop = validate(&request, firmware).expect_err(&name);
                assert_eq!(stop.failure, Failure::InvalidRequest, "{name}");
                // What no set holds at all (the extended frame left empty,
                // a primary partition's entry grown) is said alone; a mix
                // each part of which some set holds, as the mix.
                let never = needs
                    .iter()
                    .copied()
                    .find(|one| !sets.iter().any(|set| set.contains(one)));
                let said = match never {
                    Some(alone) => {
                        never_said += 1;
                        if alone == layout::Capability::LargeSectors {
                            large += 1;
                        }
                        layout::Unproven::Never(alone).line()
                    }
                    None => format!(
                        "the layout needs {needs:?} together, which no one scenario this build has proven needed"
                    ),
                };
                assert_eq!(stop.detail, said, "{name}");
            }
        }
        assert!(taken > 0 && refused > 0, "{taken} taken, {refused} refused");
        // msdos-extended empties and deletes its extended frame; gpt-plain
        // and msdos each grow a partition's entry that is not a logical
        // one's: at 512 and 4096. And at 4096 every other one of the
        // fifteen is refused for its sectors alone.
        assert_eq!(never_said, 3 + 15, "{never_said} said alone");
        assert_eq!(large, 15 - 3, "{large} said for their sectors");
    }

    /// **The worker refuses whatever the rule set refuses** (Unit Q, Q4 and
    /// Q5): a layout reaches `layout::shape` and its pins through
    /// `disks::validate_storage`, so — every capability pretended proven — a
    /// label the rule set refuses (one holding "/", `layout::LABEL_REFUSES`,
    /// or one too long), `/` moved a MiB back over the EFI partition
    /// (partitions that overlap), the EFI partition at `/boot` made ext4
    /// rather than FAT32, a boot loader that cannot start it (systemd-boot on
    /// msdos), easy fields that are not its pins (a filesystem not `/`'s, a
    /// swap file too large) and, on a BIOS, a partition kept from sector 63
    /// of `/`'s msdos disk, in GRUB's gap, are each refused before anything
    /// else.
    #[test]
    fn the_worker_refuses_what_the_rule_set_refuses() {
        use crate::model::config::{Bootloader, Invalid};
        use crate::model::layout::Fs;
        let _proven = layout::PretendProven::these(EVERY_SET);
        let good = blank_request(512);
        assert!(judged(&good, Firmware::Uefi).is_ok());
        let refused = |request: &Request, why: Invalid| {
            let stop = judged(request, Firmware::Uefi).expect_err(&format!("{why:?}"));
            assert_eq!(stop.failure, Failure::InvalidRequest);
            assert_eq!(
                stop.detail,
                format!("the request does not validate: {why:?}"),
            );
        };
        let with = |change: &dyn Fn(&mut crate::model::layout::DiskLayout)| {
            let mut request = good.clone();
            let Target::Layout { disks: laid } = &mut request.config.target else {
                unreachable!()
            };
            change(&mut laid[0]);
            request.consent = disks::consent(&request.config);
            request
        };
        for label in ["a/devops", "/dev/vdx", "x/dev", "Linexin-root-label"] {
            refused(
                &with(&|one| one.create[1].label = label.to_string()),
                Invalid::LayoutRefused,
            );
        }
        let fits = with(&|one| one.create[1].label = "Linexin".into());
        assert!(judged(&fits, Firmware::Uefi).is_ok(), "a label that fits");
        refused(
            &with(&|one| one.create[1].start -= crate::model::disks::MIB),
            Invalid::LayoutRefused,
        );
        refused(
            &with(&|one| one.create[0].fs = Fs::Ext4),
            Invalid::LayoutRefused,
        );
        let mut msdos = with(&|one| one.table = Some(crate::model::disks::Table::Msdos));
        msdos.config.bootloader = Bootloader::SystemdBoot;
        refused(&msdos, Invalid::BootloaderUnavailable);
        let mut unpinned = good.clone();
        unpinned.config.storage.filesystem = crate::model::config::Filesystem::Btrfs;
        refused(&unpinned, Invalid::LayoutRefused);
        let mut swap = good.clone();
        swap.config.storage.swap_mib = 1024 * 1024;
        refused(&swap, Invalid::SwapTooLarge);
        // On a BIOS (Q7): a partition kept in the first MiB of /'s msdos
        // disk, where GRUB's core goes — the matrix's DATA kept, but from
        // sector 63, as tools of DOS's time began a disk's first partition.
        let keep_fat = crate::worker::manual::matrix::cases(512)
            .into_iter()
            .find(|case| case.name == "msdos-keep-fat")
            .unwrap()
            .layout;
        assert!(judged(&request_for(keep_fat.clone()), Firmware::Bios).is_ok());
        let mut at_63 = keep_fat;
        at_63[0].disk.partitions[0].start = 63 * 512;
        at_63[0].parts[0].start = 63 * 512;
        let stop = judged(&request_for(at_63), Firmware::Bios).expect_err("GRUB's gap");
        assert_eq!(stop.failure, Failure::InvalidRequest);
        assert_eq!(
            stop.detail,
            format!(
                "the request does not validate: {:?}",
                Invalid::LayoutRefused
            ),
        );
    }

    /// The blank disk laid out by hand (`/dev/vdx`, a new gpt) beside a
    /// second disk only mounted from: `/dev/vdy`, whose first partition is
    /// kept at `/data` and whose second is left as it is, unused.
    fn beside_a_data_disk() -> Request {
        use crate::model::layout::{Fate, MountPoint, Part};
        use crate::worker::partition::fixtures::{build, Spec, GPT_LINUX};
        let Target::Layout { disks: mut laid } = layout_request(512).config.target else {
            unreachable!()
        };
        let spec = |number: u32, start: u64| Spec {
            number,
            start,
            size: 10 * disks::GIB,
            part_type: GPT_LINUX,
            content: Content::Linux,
            fs_type: "ext4",
        };
        let mut data = build(
            "/dev/vdy",
            512,
            Some(Table::Gpt),
            vec![spec(1, disks::MIB), spec(2, disks::MIB + 10 * disks::GIB)],
            Vec::new(),
        );
        data.major_minor = "254:16".into();
        data.diskseq = Some(43);
        let parts = data
            .partitions
            .iter()
            .zip([Some(MountPoint::Other("/data".into())), None])
            .map(|(was, mount)| Part {
                path: was.path.clone(),
                number: was.number,
                start: was.start,
                size: was.size,
                fate: Fate::Keep { mount },
            })
            .collect();
        laid.push(crate::model::layout::DiskLayout {
            disk: data,
            table: None,
            parts,
            create: Vec::new(),
        });
        request_for(laid)
    }

    /// The disks of `request`'s layout, as discovery would list them again
    /// with nothing changed.
    fn as_reviewed(request: &Request) -> Vec<Disk> {
        let Target::Layout { disks: laid } = &request.config.target else {
            unreachable!()
        };
        laid.iter().map(|one| one.disk.clone()).collect()
    }

    /// **Every disk of a layout is found again as it was reviewed, and the
    /// layout judged again on them** (Unit Q, Q5), from disks a test lists:
    /// listed as they were, it passes; a disk gone, listed no more, given
    /// another partition or another number, written over, or read-only now
    /// is `DiskChanged`; a disk with something mounted on it now is
    /// `DiskBusy`; one still the same but that the rule set no longer
    /// takes (`/`'s disk too small for Linexin now) is `DiskChanged` too.
    #[test]
    fn every_disk_of_a_layout_is_found_again_as_it_was_reviewed() {
        let request = beside_a_data_disk();
        let config = &request.config;
        assert_eq!(disks::validate_storage(config, Firmware::Uefi), Ok(()));
        let reviewed = as_reviewed(&request);
        assert!(layout_unchanged(config, Firmware::Uefi, Ok(reviewed.clone())).is_ok());
        let refused = |found: Result<Vec<Disk>, String>| {
            layout_unchanged(config, Firmware::Uefi, found)
                .expect_err("found again")
                .failure
        };
        assert_eq!(refused(Err("lsblk failed".into())), Failure::DiskChanged);
        assert_eq!(refused(Ok(reviewed[..1].to_vec())), Failure::DiskChanged);
        let changed = |change: &dyn Fn(&mut Vec<Disk>)| {
            let mut found = reviewed.clone();
            change(&mut found);
            refused(Ok(found))
        };
        assert_eq!(
            changed(&|found| found[1].partitions[1].size -= disks::MIB),
            Failure::DiskChanged
        );
        assert_eq!(
            changed(&|found| found[1].diskseq = Some(99)),
            Failure::DiskChanged
        );
        assert_eq!(
            changed(&|found| found[0].table = Some(Table::Msdos)),
            Failure::DiskChanged
        );
        assert_eq!(
            changed(&|found| found[0].read_only = true),
            Failure::DiskChanged
        );
        assert_eq!(
            changed(&|found| {
                found[1].partitions[1].mountpoints = vec!["/run/media/zoe/x".into()];
                found[1].blocked = Some(Blocked::InUse {
                    mountpoints: vec!["/run/media/zoe/x".into()],
                });
            }),
            Failure::DiskBusy
        );
        // The same disk, which the rule set no longer takes for /.
        let stop = layout_unchanged(config, Firmware::Uefi, {
            let mut found = reviewed.clone();
            found[0].blocked = Some(Blocked::TooSmall);
            Ok(found)
        })
        .expect_err("judged again");
        assert_eq!(stop.failure, Failure::DiskChanged);
        assert!(stop.detail.contains("no longer suit"), "{}", stop.detail);
        // Listed in another order, beside a disk the layout does not use.
        let mut more = reviewed.clone();
        more.reverse();
        let mut other = reviewed[1].clone();
        other.path = "/dev/vdz".into();
        more.push(other);
        assert!(layout_unchanged(config, Firmware::Uefi, Ok(more)).is_ok());
    }

    /// **Nothing a layout writes or mounts is in use** (Unit Q, Q5), from
    /// what a test writes of the machine: the written disk and each of its
    /// partitions, and the data disk's partition kept at `/data` — not its
    /// unused one — are each asked of, in path order; mounted, swap, held,
    /// gone, or the session's mounts unreadable, each is `DiskBusy`.
    #[test]
    fn nothing_a_layout_writes_or_mounts_is_in_use() {
        let request = beside_a_data_disk();
        let config = &request.config;
        let Target::Layout { disks: laid } = &config.target else {
            unreachable!()
        };
        assert_eq!(layout_devices(laid), ["/dev/vdx", "/dev/vdy1"]);
        let numbers = |device: &str| match device {
            "/dev/vdx" => "254:0",
            "/dev/vdy1" => "254:17",
            "/dev/vdy2" => "254:18",
            _ => "254:99",
        };
        let quiet_mounts = "22 1 0:21 / /proc rw - proc proc rw\n";
        let no_swaps = "Filename\tType\tSize\tUsed\tPriority\n";
        let mut asked = Vec::new();
        let checked = layout_not_busy(config, Ok(quiet_mounts.into()), no_swaps, &mut |device| {
            asked.push(device.to_string());
            Ok((numbers(device).to_string(), false))
        });
        assert!(checked.is_ok());
        assert_eq!(asked, ["/dev/vdx", "/dev/vdy1"]);
        let busy = |mountinfo: Result<String, String>,
                    swaps: &str,
                    held: Option<&str>,
                    gone: Option<&str>| {
            layout_not_busy(config, mountinfo, swaps, &mut |device| {
                if Some(device) == gone {
                    return Err(format!("{device} is not a block device"));
                }
                Ok((numbers(device).to_string(), Some(device) == held))
            })
        };
        let refused = |result: Result<(), Stop>| result.expect_err("busy").failure;
        let mounted = "41 1 254:17 / /run/media/zoe/data rw - ext4 /dev/vdy1 rw\n";
        assert_eq!(
            refused(busy(Ok(mounted.into()), no_swaps, None, None)),
            Failure::DiskBusy
        );
        let swapped = format!("{no_swaps}/dev/vdx partition 1024 0 -2\n");
        assert_eq!(
            refused(busy(Ok(quiet_mounts.into()), &swapped, None, None)),
            Failure::DiskBusy
        );
        assert_eq!(
            refused(busy(
                Ok(quiet_mounts.into()),
                no_swaps,
                Some("/dev/vdx"),
                None
            )),
            Failure::DiskBusy
        );
        assert_eq!(
            refused(busy(
                Ok(quiet_mounts.into()),
                no_swaps,
                None,
                Some("/dev/vdy1")
            )),
            Failure::DiskBusy
        );
        assert_eq!(
            refused(busy(Err("no /proc".into()), no_swaps, None, None)),
            Failure::DiskBusy
        );
        // The data disk's unused partition is nothing the layout touches.
        let elsewhere = "41 1 254:18 / /run/media/zoe/other rw - ext4 /dev/vdy2 rw\n";
        assert!(busy(Ok(elsewhere.into()), no_swaps, Some("/dev/vdy2"), None).is_ok());
    }

    /// The largest request the interface allows (§7.0, Unit Q, "The
    /// request" 5): four disks of 128 GPT entries each, every one at the
    /// longest a value can be — device names of 63 bytes, a model and a
    /// serial of 64, an NTFS label of 128 letters three bytes long, a
    /// filesystem UUID of 36, sizes in nineteen digits, a run of
    /// unallocated space after every partition — each under a new gpt
    /// erasing all 128, and 128 partitions made on it again, each with a
    /// label and a mount point of its own at their longest (255 bytes);
    /// `/` on the first disk, which the request carries a second time as
    /// its `disk`. The account at its longest in four-byte letters, a region
    /// at its longest, every Flatpak and 300 packages left out. One the rule
    /// set lets through, its easy fields its pins and its consent what it
    /// destroys.
    fn largest_request() -> Request {
        use crate::model::config::{Account, Keyboard, Region, Storage};
        use crate::model::disks::{part_type, Content, Gap, Transport, GIB, MIB};
        use crate::model::layout::{self, Create, DiskLayout, Fs, MountPoint};
        use crate::model::secret::Secret;
        use crate::worker::partition::fixtures::config;
        const BASE: u64 = 1 << 59;
        const STEP: u64 = GIB + 256 * MIB;
        let longest = |stem: String, most: usize| {
            let pad = most - stem.len();
            format!("{stem}{}", "x".repeat(pad))
        };
        let mut laid = Vec::new();
        for d in 0..4u64 {
            let name = format!("/dev/{}{d}", "n".repeat(57));
            let partitions: Vec<Partition> = (0..128u64)
                .map(|i| Partition {
                    path: format!("{name}p{}", i + 1),
                    number: u32::try_from(i + 1).unwrap(),
                    start: BASE + i * STEP,
                    size: GIB,
                    fs_type: "ntfs".into(),
                    label: "語".repeat(128),
                    part_type: part_type::BASIC_DATA.into(),
                    uuid: "0".repeat(36),
                    mountpoints: Vec::new(),
                    content: Content::Windows,
                    bootable: false,
                })
                .collect();
            let mut free: Vec<Gap> = (0..128u64)
                .map(|i| Gap {
                    start: BASE + i * STEP + GIB,
                    size: 256 * MIB,
                })
                .collect();
            free.push(Gap {
                start: BASE + 128 * STEP,
                size: BASE - 128 * STEP - GIB,
            });
            let disk = Disk {
                path: name.clone(),
                diskseq: Some(u64::MAX),
                model: "M".repeat(64),
                serial: "S".repeat(64),
                size: 1 << 60,
                major_minor: "259:1048575".into(),
                logical_sector: 512,
                transport: Transport::Nvme,
                removable: false,
                rotational: false,
                table: Some(crate::model::disks::Table::Gpt),
                content: None,
                read_only: false,
                blocked: None,
                partitions,
                free,
            };
            let create = (0..128u64)
                .map(|i| {
                    let (fs, label, mount) = match (d, i) {
                        (0, 0) => (Fs::Fat32, "L".repeat(11), MountPoint::Boot),
                        (0, 1) => (Fs::Ext4, "L".repeat(16), MountPoint::Root),
                        _ => (
                            Fs::Btrfs,
                            "L".repeat(255),
                            MountPoint::Other(longest(format!("/data-{d}-{i:03}-"), 255)),
                        ),
                    };
                    let size = if (d, i) == (0, 1) { 64 * GIB } else { GIB };
                    Create {
                        start: BASE + 128 * STEP + i * 65 * GIB,
                        size,
                        fs,
                        label,
                        mount: Some(mount),
                    }
                })
                .collect();
            let parts = disk
                .partitions
                .iter()
                .map(|was| layout::Part {
                    path: was.path.clone(),
                    number: was.number,
                    start: was.start,
                    size: was.size,
                    fate: layout::Fate::Delete,
                })
                .collect();
            laid.push(DiskLayout {
                disk,
                table: Some(crate::model::disks::Table::Gpt),
                parts,
                create,
            });
        }
        let pins = layout::pins(&laid, 0).expect("the layout pins");
        let mut config = config(pins.disk, Target::Layout { disks: laid });
        config.storage = Storage {
            filesystem: pins.filesystem,
            swap_mib: 16 * 1024,
            home: None,
        };
        let four = "😀".repeat(128);
        config.account = Account {
            full_name: four.clone(),
            username: "a".repeat(32),
            password: Secret::new(four.clone()),
            hostname: "h".repeat(63),
            root_password: Some(Secret::new(four)),
        };
        config.region = Region {
            locale: format!("{}.UTF-8", "a".repeat(57)),
            timezone: longest("Area/".into(), 99),
            keyboard: Keyboard {
                layout: "l".repeat(63),
                variant: "v".repeat(63),
                console: "c".repeat(63),
            },
        };
        config.software.updates = true;
        config.software.flatpaks = crate::model::catalogs::flatpaks()
            .iter()
            .map(|app| app.id.to_string())
            .collect();
        config.software.remove = (0..300).map(|n| longest(format!("p{n}-"), 32)).collect();
        Request {
            consent: disks::consent(&config),
            config,
        }
    }

    /// **The largest request the interface allows is read whole**: within
    /// `MAX_REQUEST`, by the worker's own reader, back to the request it was
    /// — and taken whole by the worker's judgement, its four new gpts under
    /// UEFI lying within a set QEMU proved.
    #[test]
    fn the_largest_request_the_interface_allows_is_read_whole() {
        use crate::model::config::{validate_account, validate_region};
        use crate::model::layout;
        let request = largest_request();
        let config = &request.config;
        let Target::Layout { disks: laid } = &config.target else {
            panic!("no layout");
        };
        assert_eq!(
            layout::shape(laid, Firmware::Uefi, config.bootloader, &config.software),
            Ok(())
        );
        assert_eq!(layout::pinned(config), Ok(()));
        assert_eq!(validate_account(&config.account), Ok(()));
        assert_eq!(validate_region(&config.region), Ok(()));
        assert!(software_names(&config.software).is_ok());
        assert_eq!(request.consent.len(), 4);
        let bytes = serde_json::to_vec(&request).unwrap();
        eprintln!(
            "the largest request is {} bytes of {MAX_REQUEST}",
            bytes.len()
        );
        assert!(
            bytes.len() as u64 <= MAX_REQUEST,
            "{} bytes, past {MAX_REQUEST}",
            bytes.len()
        );
        let read = read_request(bytes.as_slice())
            .unwrap_or_else(|stop| panic!("{:?}: {}", stop.failure, stop.detail));
        assert_eq!(read.consent, request.consent);
        assert_eq!(serde_json::to_vec(&read).unwrap(), bytes);
        // New gpts over several disks under UEFI: within the sets two
        // scenarios passed with (`manual-uefi-three-disks`,
        // `manual-uefi-shrink-ext4`), so the worker takes it whole.
        assert_eq!(
            layout::needs(laid, Firmware::Uefi),
            [
                layout::Capability::FreshTable,
                layout::Capability::SeveralDisks,
                layout::Capability::Uefi,
                layout::Capability::Gpt
            ]
        );
        if let Err(stop) = validate(&read, Firmware::Uefi) {
            panic!("{:?}: {}", stop.failure, stop.detail);
        }
    }
}
