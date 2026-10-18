//! Everything carrying out a hand-made layout asks of the machine, in one
//! place: [`Machine`]. The executor (`manual::install_storage`) reads and
//! writes the computer only through it, so its order — nothing written
//! before the disks are found again, nothing on them is in use and the
//! fence has let the command through; the udisks rule taken out on every
//! way out — is tested with a machine a test writes down by hand, which
//! answers from made-up disks and runs nothing.
//!
//! [`Real`] is the computer's. Each of its answers asks the test-build fence
//! first (`run::fence::the_machine`), which refuses every test: no test
//! reaches it, whatever it is handed.
use super::fence::{self, Sysfs};
use crate::model::config::{Filesystem, InstallConfig};
use crate::model::disks::{self, Disk};
use crate::model::protocol::Failure;
use crate::worker::layout::Stop;
use crate::worker::manual::shrinking;
use crate::worker::measure::{self, Head};
use crate::worker::partition::script::Command;
use crate::worker::partition::{exec, Swapfile};
use crate::worker::quiet::{self, Hands};
use crate::worker::run::Status;
use crate::worker::storage::{self, esp, format, progress::Steps, swap};
use crate::worker::system::check::{self, DeviceFacts};
use crate::worker::system::image::Image;
use crate::worker::system::network;
use crate::worker::{run, target};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// What carrying out a layout asks of the machine.
pub trait Machine {
    /// The disks as `disks::discover` lists them now.
    fn discover(&mut self) -> Result<Vec<Disk>, String>;
    /// The live session's mounts, `/proc/1/mountinfo`.
    fn session_mounts(&mut self) -> Result<String, String>;
    /// The active swap, `/proc/swaps` (nothing where it cannot be read).
    fn swaps(&mut self) -> String;
    /// One device's `major:minor` and whether anything holds it
    /// (`check::device_facts`).
    fn device_facts(&mut self, device: &str) -> DeviceFacts;
    /// Whether the internet answers, where the choices use it
    /// (`network::check`).
    fn online(&mut self, config: &InstallConfig, image: &Image) -> Result<bool, Stop>;
    /// The hands that put the udisks rule in and take it out again, which
    /// the rule's guard keeps for the run (`quiet::Quiet`).
    fn udev(&mut self) -> Box<dyn Hands>;
    /// A new private directory the target is mounted below.
    fn stage(&mut self) -> Result<PathBuf, Stop>;
    /// A command that only reads: `lsblk`, `sfdisk --json`, `blkid -p`.
    fn read(&mut self, command: &Command) -> Result<String, Stop>;
    /// A command that only reads, whose end is read whatever code it exits
    /// with (`run::run_status`): phase A's reading of a filesystem again
    /// before it is shrunk, and its dry runs — each one phase A's own
    /// allowlist lets through (`shrinking::allowed`), nothing on standard
    /// input.
    fn read_status(&mut self, command: &Command) -> Result<Status, Stop>;
    /// A command that only reads and whose head alone is read, the program
    /// then ended (`measure::head_of`): `ntfscat <partition> hiberfil.sys`,
    /// phase A's sign of hibernation (`shrinking::allowed` and
    /// `only_its_head`).
    fn head(&mut self, command: &Command) -> Result<Head, Stop>;
    /// [`read_status`](Self::read_status), its program ended once it has
    /// run `limit` without ending — it only reads, so nothing is left half
    /// done — and an error then (`run::run_status_within`): phase A's
    /// reading of the Windows volumes, bounded in time
    /// (`shrinking::WINDOWS_PATIENCE`).
    fn read_status_within(&mut self, command: &Command, limit: Duration) -> Result<Status, Stop>;
    /// [`head`](Self::head), bounded in time likewise
    /// (`measure::head_within`).
    fn head_within(&mut self, command: &Command, limit: Duration) -> Result<Head, Stop>;
    /// The time now, by which phase A bounds that reading: the machine's
    /// clock, or a test's own, which its commands move on.
    fn now(&mut self) -> Instant;
    /// What the kernel says of block devices, for the fence.
    fn sysfs(&self) -> &dyn Sysfs;
    /// `device` opened exclusively and closed again (`exec::claim`):
    /// `DiskBusy` while anything holds it.
    fn claim(&mut self, device: &str) -> Result<(), Stop>;
    /// One command that writes a disk — a table changed, a partition's
    /// signatures wiped, a filesystem made — given only once the fence has
    /// let it through and its device was claimed.
    fn write(&mut self, command: &Command) -> Result<(), Stop>;
    /// One command that writes a disk and whose end is read by its tool's
    /// own rule rather than by its success alone (`run::run_status`): a
    /// filesystem's shrink, phase B.1 — e2fsck's code is a sum, ntfsresize
    /// goes on only with its own words — given only once the fence has let
    /// it through, its partition was claimed and is in use nowhere.
    fn write_status(&mut self, command: &Command) -> Result<Status, Stop>;
    /// The first 512 bytes of the partition `device`, read and nothing
    /// else: a shrunk NTFS's boot sector, which says its volume's size.
    fn first_sector(&mut self, device: &str) -> Result<Vec<u8>, Stop>;
    /// The kernel and udev given time to catch up: `partx -u` on `disk`
    /// where one is named, then `udevadm settle`. Neither failing stops
    /// anything: what is read back afterwards decides.
    fn settle(&mut self, disk: Option<&str>);
    /// A short wait before the kernel's partitions are looked at again.
    fn pause(&mut self);
    /// Whether `path` is a block device now.
    fn is_block_device(&mut self, path: &str) -> bool;
    /// How many bytes are free on the kept EFI system partition `device`,
    /// mounted read-only at `dir` for the look and unmounted again
    /// (`esp::measure`).
    fn esp_free(&mut self, device: &str, dir: &Path) -> Result<u64, Stop>;
    /// How many bytes the image at `source` puts at `path` (`du -sb
    /// --one-file-system`): nothing where it has nothing there, `None` where
    /// that cannot be said.
    fn image_size(&mut self, source: &Path, path: &str) -> Option<u64>;
    /// The Btrfs subvolumes `names` made on `device` (`format::subvolumes`).
    fn subvolumes(&mut self, names: &[&str], device: &str, top: &Path) -> Result<(), Stop>;
    /// The target's root made, an empty directory only root may enter.
    fn make_root(&mut self, root: &Path) -> Result<(), Stop>;
    /// `at`'s directory made inside the target (`mounts::mount_dir`).
    fn mount_dir(&mut self, root: &Path, at: &str) -> Result<(), Stop>;
    /// `device` mounted as `fstype`, with `options`, at `at` in the target.
    fn mount(
        &mut self,
        fstype: &str,
        options: Option<&str>,
        device: &str,
        root: &Path,
        at: &str,
    ) -> Result<(), Stop>;
    /// The mount point `at` in the target given `mode`.
    fn set_mode(&mut self, root: &Path, at: &str, mode: u32) -> Result<(), Stop>;
    /// The swap file made in the target (`swap::make`).
    fn swapfile(
        &mut self,
        swap: Swapfile,
        filesystem: Filesystem,
        root: &Path,
        steps: &mut Steps,
    ) -> Result<(), Stop>;
}

/// The computer itself. In a test build every answer is refused before
/// anything is read or written (`run::fence::the_machine`).
pub struct Real;

/// Asks the test-build fence, which refuses every test, before the machine
/// is touched for `what`.
fn fenced(_what: &str) {
    #[cfg(test)]
    run::fence::the_machine(_what);
}

fn internal(detail: String) -> Stop {
    Stop::new(Failure::Internal, detail)
}

/// A pause between two looks at the kernel's partitions, as `exec` waits
/// for the easy path's.
const READ_BACK_PAUSE: Duration = Duration::from_millis(250);

impl Machine for Real {
    fn discover(&mut self) -> Result<Vec<Disk>, String> {
        fenced("disks");
        disks::discover()
    }

    fn session_mounts(&mut self) -> Result<String, String> {
        fenced("live session's mounts");
        std::fs::read_to_string("/proc/1/mountinfo").map_err(|e| e.to_string())
    }

    fn swaps(&mut self) -> String {
        fenced("swap");
        std::fs::read_to_string("/proc/swaps").unwrap_or_default()
    }

    fn device_facts(&mut self, device: &str) -> DeviceFacts {
        fenced("block devices");
        check::device_facts(device)
    }

    fn online(&mut self, config: &InstallConfig, image: &Image) -> Result<bool, Stop> {
        fenced("network");
        network::check(config, image)
    }

    fn udev(&mut self) -> Box<dyn Hands> {
        fenced("udev rules");
        Box::new(quiet::Udev)
    }

    fn stage(&mut self) -> Result<PathBuf, Stop> {
        fenced("installer's stage");
        crate::worker::make_stage()
    }

    fn read(&mut self, command: &Command) -> Result<String, Stop> {
        fenced("disks");
        exec::run_command(command)
    }

    fn read_status(&mut self, command: &Command) -> Result<Status, Stop> {
        fenced("disks");
        if !shrinking::allowed(command) || shrinking::only_its_head(command) {
            return Err(internal(format!(
                "{} is not a command phase A reads whole",
                command.program
            )));
        }
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        run::run_status(command.program, &args, b"")
    }

    fn head(&mut self, command: &Command) -> Result<Head, Stop> {
        fenced("disks");
        if !shrinking::allowed(command) || !shrinking::only_its_head(command) {
            return Err(internal(format!(
                "{} is not a command phase A reads the head of",
                command.program
            )));
        }
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        measure::head_of(command.program, &args)
    }

    fn read_status_within(&mut self, command: &Command, limit: Duration) -> Result<Status, Stop> {
        fenced("disks");
        if !shrinking::allowed(command) || shrinking::only_its_head(command) {
            return Err(internal(format!(
                "{} is not a command phase A reads whole",
                command.program
            )));
        }
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        run::run_status_within(command.program, &args, limit)
    }

    fn head_within(&mut self, command: &Command, limit: Duration) -> Result<Head, Stop> {
        fenced("disks");
        if !shrinking::allowed(command) || !shrinking::only_its_head(command) {
            return Err(internal(format!(
                "{} is not a command phase A reads the head of",
                command.program
            )));
        }
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        measure::head_within(command.program, &args, Some(limit))
    }

    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn sysfs(&self) -> &dyn Sysfs {
        fenced("block devices");
        &fence::Machine
    }

    fn claim(&mut self, device: &str) -> Result<(), Stop> {
        fenced("disks");
        exec::claim(device)
    }

    fn write(&mut self, command: &Command) -> Result<(), Stop> {
        fenced("disks");
        exec::run_command(command).map(drop)
    }

    fn write_status(&mut self, command: &Command) -> Result<Status, Stop> {
        fenced("disks");
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        let input = command.input.as_deref().unwrap_or("");
        run::run_status(command.program, &args, input.as_bytes())
    }

    fn first_sector(&mut self, device: &str) -> Result<Vec<u8>, Stop> {
        fenced("disks");
        use std::io::Read;
        let mut file = std::fs::File::open(device)
            .map_err(|e| internal(format!("cannot open {device} to read: {e}")))?;
        let meta = file
            .metadata()
            .map_err(|e| internal(format!("cannot inspect {device}: {e}")))?;
        if !meta.file_type().is_block_device() {
            return Err(Stop::new(
                Failure::DiskChanged,
                format!("{device} is not a block device"),
            ));
        }
        let mut sector = vec![0u8; 512];
        file.read_exact(&mut sector)
            .map_err(|e| internal(format!("cannot read {device}'s first sector: {e}")))?;
        Ok(sector)
    }

    fn settle(&mut self, disk: Option<&str>) {
        fenced("disks");
        if let Some(disk) = disk {
            if let Err(stop) = run::run("partx", &["-u", disk]) {
                run::log(&format!("partx: {}", stop.detail));
            }
        }
        if let Err(stop) = run::run("udevadm", &["settle", "--timeout=60"]) {
            run::log(&format!("udevadm: {}", stop.detail));
        }
    }

    fn pause(&mut self) {
        std::thread::sleep(READ_BACK_PAUSE);
    }

    fn is_block_device(&mut self, path: &str) -> bool {
        fenced("block devices");
        std::fs::metadata(path).is_ok_and(|meta| meta.file_type().is_block_device())
    }

    fn esp_free(&mut self, device: &str, dir: &Path) -> Result<u64, Stop> {
        fenced("EFI system partition");
        storage::make_dir(dir)?;
        let free = esp::measure(device, dir);
        let _ = std::fs::remove_dir(dir);
        free
    }

    fn image_size(&mut self, source: &Path, path: &str) -> Option<u64> {
        fenced("image");
        let full = source.join(path.trim_start_matches('/'));
        let text = full.to_str()?;
        match std::fs::symlink_metadata(&full) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(0),
            Ok(meta) if meta.is_dir() => {
                let out = run::run("du", &["-sb", "--one-file-system", text]).ok()?;
                measure::du_bytes(&out, text)
            }
            _ => None,
        }
    }

    fn subvolumes(&mut self, names: &[&str], device: &str, top: &Path) -> Result<(), Stop> {
        fenced("disks");
        format::subvolumes(names, device, top)
    }

    fn make_root(&mut self, root: &Path) -> Result<(), Stop> {
        fenced("installer's stage");
        storage::make_dir(root)
    }

    fn mount_dir(&mut self, root: &Path, at: &str) -> Result<(), Stop> {
        fenced("target");
        super::mounts::mount_dir(root, at)
    }

    fn mount(
        &mut self,
        fstype: &str,
        options: Option<&str>,
        device: &str,
        root: &Path,
        at: &str,
    ) -> Result<(), Stop> {
        fenced("disks");
        let relative = at.trim_start_matches('/');
        let point = if relative.is_empty() {
            root.to_path_buf()
        } else {
            target::path(root, relative)?
        };
        let point = run::utf8(&point)?;
        let mut args = vec!["-t", fstype];
        if let Some(options) = options {
            args.extend(["-o", options]);
        }
        args.extend([device, point]);
        run::run("mount", &args).map(drop)
    }

    fn set_mode(&mut self, root: &Path, at: &str, mode: u32) -> Result<(), Stop> {
        fenced("target");
        let point = target::path(root, at.trim_start_matches('/'))?;
        std::fs::set_permissions(&point, std::fs::Permissions::from_mode(mode))
            .map_err(|e| internal(format!("cannot give {} its mode: {e}", point.display())))
    }

    fn swapfile(
        &mut self,
        swap: Swapfile,
        filesystem: Filesystem,
        root: &Path,
        steps: &mut Steps,
    ) -> Result<(), Stop> {
        fenced("target");
        swap::make(swap, filesystem, root, steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::run::fence::refusal;

    /// No test reaches the computer through the real machine: every answer
    /// it gives asks the fence first, which refuses every test — a test of
    /// the executor hands in a machine of its own.
    #[test]
    fn no_test_reaches_this_computer_through_the_real_machine() {
        let root = Path::new("/nonexistent-lxb-root");
        type Probe = Box<dyn FnOnce() + std::panic::UnwindSafe>;
        let probes: Vec<(&str, Probe)> = vec![
            ("discover", Box::new(|| drop(Real.discover()))),
            ("mounts", Box::new(|| drop(Real.session_mounts()))),
            ("swaps", Box::new(|| drop(Real.swaps()))),
            ("facts", Box::new(|| drop(Real.device_facts("/dev/vdz")))),
            ("udev", Box::new(|| drop(Real.udev()))),
            ("stage", Box::new(|| drop(Real.stage()))),
            (
                "read",
                Box::new(|| drop(Real.read(&Command::new("lsblk", &["--version"])))),
            ),
            (
                "read status",
                Box::new(|| {
                    drop(Real.read_status(&Command::new("e2fsck", &["-f", "-n", "/dev/vdz1"])))
                }),
            ),
            (
                "head",
                Box::new(|| {
                    drop(Real.head(&Command::new("ntfscat", &["/dev/vdz1", "hiberfil.sys"])))
                }),
            ),
            (
                "read status within",
                Box::new(|| {
                    let ntfs =
                        Command::new("ntfsresize", &["--info", "--no-progress-bar", "/dev/vdz1"]);
                    drop(Real.read_status_within(&ntfs, Duration::from_secs(1)))
                }),
            ),
            (
                "head within",
                Box::new(|| {
                    let sign = Command::new("ntfscat", &["/dev/vdz1", "hiberfil.sys"]);
                    drop(Real.head_within(&sign, Duration::from_secs(1)))
                }),
            ),
            ("sysfs", Box::new(|| drop(Real.sysfs().link("vdz")))),
            ("claim", Box::new(|| drop(Real.claim("/dev/vdz")))),
            (
                "write",
                Box::new(|| drop(Real.write(&Command::new("wipefs", &["--version"])))),
            ),
            (
                "write status",
                Box::new(|| {
                    drop(Real.write_status(&Command::new("e2fsck", &["-f", "-y", "/dev/vdz1"])))
                }),
            ),
            ("sector", Box::new(|| drop(Real.first_sector("/dev/vdz1")))),
            ("settle", Box::new(|| Real.settle(Some("/dev/vdz")))),
            (
                "block",
                Box::new(|| assert!(!Real.is_block_device("/dev/vdz"))),
            ),
            (
                "esp",
                Box::new(move || drop(Real.esp_free("/dev/vdz1", root))),
            ),
            (
                "image",
                Box::new(move || assert!(Real.image_size(root, "/var").is_none())),
            ),
            (
                "subvolumes",
                Box::new(move || drop(Real.subvolumes(&["@"], "/dev/vdz2", root))),
            ),
            ("root", Box::new(move || drop(Real.make_root(root)))),
            ("dir", Box::new(move || drop(Real.mount_dir(root, "/data")))),
            (
                "mount",
                Box::new(move || drop(Real.mount("ext4", None, "/dev/vdz2", root, "/"))),
            ),
            (
                "mode",
                Box::new(move || drop(Real.set_mode(root, "/tmp", 0o1777))),
            ),
        ];
        let mut probes = probes;
        probes.push((
            "online",
            Box::new(|| {
                use crate::model::config::{Desktop, Target};
                use crate::worker::partition::fixtures::{config, disk_empty};
                let mut asked = config(disk_empty(512), Target::WholeDisk);
                asked.software.updates = true;
                let image = crate::worker::system::image::fixture(&[Desktop::Linexin], &[]);
                drop(Real.online(&asked, &image));
            }),
        ));
        probes.push((
            "swapfile",
            Box::new(move || {
                let mut report = |_| {};
                let mut steps = Steps::new(&mut report);
                let swap = Swapfile {
                    path: "/swapfile",
                    mib: 1,
                };
                drop(Real.swapfile(swap, Filesystem::Ext4, root, &mut steps));
            }),
        ));
        for (what, probe) in probes {
            let words = refusal(probe).unwrap_or_else(|| panic!("{what}: the machine was reached"));
            assert!(words.contains("test build:"), "{what}: {words}");
        }
    }
}
