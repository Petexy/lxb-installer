//! Writing a plan's partition table on the disk: the only code in the
//! storage half that changes a partition table.
//!
//! Before the first write the disk is checked once more against the plan
//! (its size and sector size from the kernel, its table from the disk), and
//! before every write it is opened exclusively and closed again, which the
//! kernel refuses while anything on it is mounted, swapped on or held by
//! device-mapper. Afterwards the new partitions are found again by their
//! exact start and size in what the kernel reports, so their paths are the
//! kernel's and never built from the disk's name.
use super::plan::{Change, Plan, Role};
use super::script::{self, Command};
use super::table::{self, OnDisk};
use crate::model::protocol::{Failure, Step};
use crate::worker::layout::Stop;
use crate::worker::run;
use crate::worker::storage::progress::Steps;
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::time::Duration;

/// How long the new partitions may take to appear after the table is
/// written: tries, and the pause between them.
const READ_BACK_TRIES: u32 = 40;
const READ_BACK_PAUSE: Duration = Duration::from_millis(250);

/// A partition as the worker will use it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub path: String,
    pub number: u32,
}

/// The partitions the rest of the storage half formats and mounts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub root: Device,
    /// An EFI system partition the plan created (a shared one is not here:
    /// it is not written).
    pub esp: Option<Device>,
    /// The Linux boot partition the plan created beside a shared EFI system
    /// partition too small for the kernels.
    pub boot: Option<Device>,
}

/// Opens `device` exclusively and closes it again: `DiskBusy` when something
/// holds it. Called immediately before every write to it.
pub fn claim(device: &str) -> Result<(), Stop> {
    // In a test build no device is opened, whatever a test hands this
    // (`run::fence`): a regular file a test made is all it may claim.
    #[cfg(test)]
    run::fence::claimed(device);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_EXCL)
        .open(device)
        .map_err(|e| match e.raw_os_error() {
            Some(libc::EBUSY) => Stop::new(Failure::DiskBusy, format!("{device} is in use")),
            Some(libc::ENOENT | libc::ENXIO | libc::ENODEV) => {
                Stop::new(Failure::DiskChanged, format!("{device} is gone: {e}"))
            }
            _ => Stop::new(Failure::Internal, format!("cannot open {device}: {e}")),
        })?;
    let meta = file
        .metadata()
        .map_err(|e| Stop::new(Failure::Internal, format!("cannot inspect {device}: {e}")))?;
    if !meta.file_type().is_block_device() {
        return Err(Stop::new(
            Failure::DiskChanged,
            format!("{device} is not a block device"),
        ));
    }
    Ok(())
}

fn changed(detail: impl Into<String>) -> Stop {
    Stop::new(Failure::DiskChanged, detail)
}

fn internal(detail: impl Into<String>) -> Stop {
    Stop::new(Failure::Internal, detail)
}

/// Runs one command of a plan.
pub fn run_command(command: &Command) -> Result<String, Stop> {
    let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
    match &command.input {
        Some(input) => run::run_input(command.program, &args, input.as_bytes(), false),
        None => run::run(command.program, &args),
    }
}

/// The table on the disk now. A disk whose table cannot be read any more
/// has changed since it was reviewed.
fn read_table(disk: &str) -> Result<OnDisk, Stop> {
    let json = run::run("sfdisk", &["--json", disk])
        .map_err(|stop| changed(format!("cannot read the table of {disk}: {}", stop.detail)))?;
    table::parse_sfdisk(&json).map_err(internal)
}

/// The size and sector size the kernel reports now must be the reviewed
/// ones: everything the plan says is counted in those sectors.
fn check_device(plan: &Plan) -> Result<(), Stop> {
    let json = run::run(
        "lsblk",
        &[
            "--json",
            "--bytes",
            "--paths",
            "--nodeps",
            "-o",
            "NAME,SIZE,LOG-SEC",
            &plan.disk,
        ],
    )?;
    let (size, sector) = table::parse_device(&json, &plan.disk).map_err(internal)?;
    if size != plan.size || sector != plan.sector {
        return Err(changed(format!(
            "{} is now {size} bytes in {sector} byte sectors, not {} in {}",
            plan.disk, plan.size, plan.sector
        )));
    }
    Ok(())
}

/// Lets the kernel and udev catch up with a written table. sfdisk already
/// tells the kernel; `partx -u` repairs a view it could only partly update,
/// and settling waits for the device nodes. Neither failing is fatal on its
/// own: finding the partitions afterwards is what decides.
fn settle(disk: &str, steps: &mut Steps) {
    if let Err(stop) = run::run("partx", &["-u", disk]) {
        steps.log(format!("partx: {}", stop.detail));
    }
    if let Err(stop) = run::run("udevadm", &["settle", "--timeout=60"]) {
        steps.log(format!("udevadm: {}", stop.detail));
    }
}

/// Finds the plan's partitions in the kernel's view, waiting for them a
/// little while if they are not there yet.
fn read_back(plan: &Plan) -> Result<Written, Stop> {
    let mut last = String::new();
    for attempt in 0..READ_BACK_TRIES {
        if attempt > 0 {
            std::thread::sleep(READ_BACK_PAUSE);
        }
        let json = run::run(
            "lsblk",
            &[
                "--json",
                "--bytes",
                "--paths",
                "-o",
                "NAME,START,SIZE,PARTN",
                &plan.disk,
            ],
        )?;
        let kernel = table::parse_partitions(&json, &plan.disk).map_err(internal)?;
        let found = table::find_created(plan, &kernel).unwrap_or_default();
        let device = |role: Role| {
            found.iter().find(|f| f.role == role).map(|f| Device {
                path: f.path.clone(),
                number: f.number,
            })
        };
        let root = match &plan.change {
            Change::Retype { number, path, span } => {
                table::find_kept(path, *number, *span, plan.sector, &kernel).then(|| Device {
                    path: path.clone(),
                    number: *number,
                })
            }
            _ => device(Role::Root),
        };
        let complete = found.len() == plan.create.len();
        if let (true, Some(root)) = (complete, root) {
            let written = Written {
                root,
                esp: device(Role::Esp),
                boot: device(Role::LinuxBoot),
            };
            let made = [&written.esp, &written.boot];
            if is_block_device(&written.root.path)
                && made
                    .iter()
                    .all(|d| d.as_ref().is_none_or(|d| is_block_device(&d.path)))
            {
                return Ok(written);
            }
        }
        last = json;
    }
    Err(internal(format!(
        "the new partitions of {} did not appear; the kernel reports {}",
        plan.disk,
        last.split_whitespace().collect::<Vec<_>>().join(" ")
    )))
}

fn is_block_device(path: &str) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.file_type().is_block_device())
}

/// Writes the plan's table changes and returns the partitions to format.
/// Reports `Step::Partition`.
pub fn write(plan: &Plan, steps: &mut Steps) -> Result<Written, Stop> {
    let commands = script::table_commands(plan, &plan.disk);
    // One unit for the checks, one per command, one for settling, one for
    // finding the partitions again.
    steps.begin(Step::Partition, commands.len() as u64 + 3);
    for line in plan.summary() {
        steps.log(line);
    }
    check_device(plan)?;
    if plan.change != Change::Fresh {
        let on_disk = read_table(&plan.disk)?;
        table::check_before_writing(plan, &on_disk).map_err(changed)?;
    }
    steps.advance();
    for (index, command) in commands.iter().enumerate() {
        if index > 0 {
            // Each sfdisk call re-reads the table; let udev finish with the
            // last one before the next takes the lock.
            settle(&plan.disk, steps);
        }
        claim(&plan.disk)?;
        run_command(command)?;
        steps.advance();
    }
    settle(&plan.disk, steps);
    steps.advance();
    let written = read_back(plan)?;
    if plan.wants_boot_flag() && plan.change != Change::Fresh {
        let on_disk = read_table(&plan.disk)?;
        if let Some(command) = script::activation(plan, &on_disk, written.root.number, &plan.disk) {
            claim(&plan.disk)?;
            run_command(&command)?;
            settle(&plan.disk, steps);
        }
    }
    steps.log(format!("storage: root partition {}", written.root.path));
    if let Some(esp) = &written.esp {
        steps.log(format!("storage: new EFI system partition {}", esp.path));
    }
    if let Some(boot) = &written.boot {
        steps.log(format!("storage: new Linux boot partition {}", boot.path));
    }
    steps.advance();
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_regular_file_is_never_taken_for_a_disk() {
        let dir = std::env::temp_dir().join(format!("lxb-claim-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("not-a-disk");
        std::fs::write(&file, b"").unwrap();
        let stop = claim(file.to_str().unwrap()).unwrap_err();
        assert_eq!(stop.failure, Failure::DiskChanged);
        let stop = claim(dir.join("missing").to_str().unwrap()).unwrap_err();
        assert_eq!(stop.failure, Failure::DiskChanged);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The claim's fence stopped it, in its own words.
    fn fenced(device: &str) {
        let words =
            run::fence::refusal(|| claim(device)).unwrap_or_else(|| panic!("{device} was opened"));
        assert!(words.contains("test build:"), "{device}: {words}");
    }

    #[test]
    fn claim_refuses_dev_in_a_test_build() {
        // Every path here would be harmless to open were the fence to let
        // it through (read-only, and no block device among them), so a
        // fence that failed shows as this test failing, never as a disk
        // that was opened.
        let dir = std::env::temp_dir().join(format!("lxb-claim-fence-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let link = dir.join("disk");
        std::os::unix::fs::symlink("/dev/null", &link).unwrap();
        let climb = format!("{}/../../dev/null", dir.display());
        for device in [
            "/dev/null",
            "/dev/lxb-installer-no-such-device",
            "/dev",
            climb.as_str(),
            link.to_str().unwrap(),
        ] {
            fenced(device);
        }
        // Root claims nothing, not even a regular file a test made.
        let file = dir.join("not-a-disk");
        std::fs::write(&file, b"").unwrap();
        {
            let _root = run::fence::Pretend::root();
            fenced(file.to_str().unwrap());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
