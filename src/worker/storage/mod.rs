//! The storage half of the worker: from a validated request to a target
//! root that is partitioned, formatted and mounted ([`Mounted`]).
//!
//! CONTRACT (phase 0), owned by the storage work. The system work calls
//! exactly `prepare` and `release` (and may ask `planned_steps` which steps
//! to announce).
//!
//! Everything that can refuse without writing happens before the first
//! write: the plan (pure), a look inside a shared EFI system partition
//! (whose room for the kernels decides whether the plan adds a Linux boot
//! partition), and the checks on a separate /home. Only then is the table written, the
//! filesystems made and the target mounted. Below the stage directory the
//! caller made, `root` is the target, `btrfs` briefly holds a new Btrfs
//! filesystem's top level, and `esp` briefly holds a shared EFI partition
//! while it is measured.
pub mod blkid;
pub mod esp;
pub mod format;
pub mod home;
pub mod mountinfo;
pub mod mounts;
pub mod progress;
pub mod swap;

use super::layout::{Esp, EspMount, Firmware, LinuxBoot, Mounted, Stop};
use super::partition::{self, EspPlan, SharedRoom};
use crate::model::config::{Filesystem, InstallConfig};
use crate::model::protocol::{Event, Failure, Step};
use progress::Steps;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The directories `prepare` uses below the caller's stage.
pub struct Stage {
    pub root: PathBuf,
    pub top: PathBuf,
    pub esp: PathBuf,
}

impl Stage {
    pub fn new(stage: &Path) -> Self {
        Self {
            root: stage.join("root"),
            top: stage.join("btrfs"),
            esp: stage.join("esp"),
        }
    }
}

/// Makes `path` an empty directory only root can enter, refusing anything
/// else already there (a leftover from an earlier run must not be mounted
/// over or written into).
pub(crate) fn make_dir(path: &Path) -> Result<(), Stop> {
    let internal = |e: std::io::Error| {
        Stop::new(
            Failure::Internal,
            format!("cannot prepare {}: {e}", path.display()),
        )
    };
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => {
            if std::fs::read_dir(path).map_err(internal)?.next().is_some() {
                return Err(Stop::new(
                    Failure::Internal,
                    format!("{} is not empty", path.display()),
                ));
            }
        }
        Ok(_) => {
            return Err(Stop::new(
                Failure::Internal,
                format!("{} is not a directory", path.display()),
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(path).map_err(internal)?
        }
        Err(e) => return Err(internal(e)),
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(internal)
}

/// The steps `prepare` reports for `config`, in order, for the worker's
/// `Event::Plan`.
pub fn planned_steps(config: &InstallConfig) -> Vec<Step> {
    let mut steps = vec![Step::Partition, Step::Format];
    if config.storage.swap_mib > 0 {
        steps.push(Step::Swap);
    }
    steps
}

/// Partitions, formats and mounts the target below `stage` (an empty private
/// directory the caller made), makes the swap file and builds the fstab.
/// Reports `Begin`/`Progress`/`Log` for `Step::Partition`, `Step::Format`
/// and `Step::Swap` through `report`. It runs before the copy: the system
/// half copies into `Mounted::root` afterwards. The swap file is made here
/// because where it goes depends on the filesystem layout.
pub fn prepare(
    config: &InstallConfig,
    stage: &Path,
    report: &mut dyn FnMut(Event),
) -> Result<Mounted, Stop> {
    let firmware = Firmware::detect();
    // Planned first as if a shared EFI partition held the kernels: a request
    // that cannot be installed is refused before anything is mounted, and
    // the plan names the partition to look inside.
    let first = partition::plan(config, firmware, SharedRoom::Kernels)?;
    let dirs = Stage::new(stage);
    let mut steps = Steps::new(report);

    let shared = match &first.esp {
        EspPlan::Shared { path } => Some(path.clone()),
        EspPlan::None | EspPlan::New => None,
    };
    let (plan, esp_mount) = match shared {
        None => {
            let mount = (first.esp == EspPlan::New).then_some(EspMount::Boot);
            (first, mount)
        }
        Some(path) => {
            make_dir(&dirs.esp)?;
            let free = esp::measure(&path, &dirs.esp)?;
            let _ = std::fs::remove_dir(&dirs.esp);
            let room = esp::decide(free)?;
            steps.log(format!(
                "storage: shared EFI system partition {path} has {free} bytes free, \
                 mounted at {}{}",
                room.esp_mount().path(),
                match room {
                    SharedRoom::Kernels => "",
                    SharedRoom::LoaderOnly => ", the kernels on a new Linux boot partition",
                }
            ));
            let plan = match room {
                SharedRoom::Kernels => first,
                SharedRoom::LoaderOnly => partition::plan(config, firmware, room)?,
            };
            (plan, Some(room.esp_mount()))
        }
    };
    let kept_home_fstype = home::check(&plan, &config.disk)?;

    let written = partition::exec::write(&plan, &mut steps)?;

    let table = mounts::table(&plan, esp_mount);
    steps.begin(Step::Format, format::units(&plan) + table.len() as u64);
    let facts = format::make(&plan, &written, kept_home_fstype, &dirs, &mut steps)?;
    format::mount_all(&table, &facts, &dirs, &mut steps)?;

    if let Some(swapfile) = plan.swap {
        swap::make(swapfile, plan.filesystem, &dirs.root, &mut steps)?;
    }

    let lines = mounts::lines(&table, &facts).map_err(|e| Stop::new(Failure::Internal, e))?;
    let fstab = mounts::fstab(&lines, plan.swap.map(|s| s.path));
    for line in fstab
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        steps.log(format!("fstab: {line}"));
    }

    Ok(Mounted {
        firmware,
        table: plan.table,
        disk: plan.disk.clone(),
        root: dirs.root.clone(),
        root_device: facts.root.clone(),
        root_uuid: facts.root_uuid.clone(),
        filesystem: plan.filesystem,
        root_subvolume: (plan.filesystem == Filesystem::Btrfs).then(|| "@".to_string()),
        esp: match (esp_mount, &facts.esp, &facts.esp_uuid) {
            (Some(mount), Some(device), Some(uuid)) => Some(Esp {
                device: device.clone(),
                uuid: uuid.clone(),
                mount,
                shared: matches!(plan.esp, EspPlan::Shared { .. }),
            }),
            _ => None,
        },
        linux_boot: match (&facts.boot, &facts.boot_uuid) {
            (Some(device), Some(uuid)) => Some(LinuxBoot {
                device: device.clone(),
                uuid: uuid.clone(),
            }),
            _ => None,
        },
        home_device: facts.home.clone(),
        swapfile: plan.swap.map(|s| PathBuf::from(s.path)),
        fstab,
        gpt_auto_off: false,
        other_devices: Vec::new(),
    })
}

fn mount_points_below(stage: &Path) -> Result<Vec<PathBuf>, Stop> {
    let text = std::fs::read_to_string("/proc/self/mountinfo").map_err(|e| {
        Stop::new(
            Failure::Internal,
            format!("cannot read the mount table: {e}"),
        )
    })?;
    Ok(mountinfo::below(&mountinfo::mount_points(&text), stage))
}

/// Unmounts everything below `stage`, innermost first. Safe to call after a
/// partial `prepare` (it is also what cleans up a failure), and again after
/// that: with nothing mounted there it does nothing.
pub fn release(stage: &Path) -> Result<(), Stop> {
    // The mount table holds resolved paths; a stage that is gone has nothing
    // mounted below it.
    let Ok(stage) = std::fs::canonicalize(stage) else {
        return Ok(());
    };
    let mut errors = Vec::new();
    let mounted = mount_points_below(&stage)?;
    // Last mounted first; `umount -R` takes each one's own mounts innermost
    // first.
    for top in mountinfo::outermost(&mounted).iter().rev() {
        let Some(path) = top.to_str() else {
            errors.push(format!("{} is not UTF-8", top.display()));
            continue;
        };
        if let Err(stop) = super::run::run("umount", &["-R", path]) {
            errors.push(stop.detail);
        }
    }
    let left = mount_points_below(&stage)?;
    if !left.is_empty() {
        return Err(Stop::new(
            Failure::CommandFailed,
            format!(
                "still mounted below {}: {}; {}",
                stage.display(),
                left.iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                errors.join("; ")
            ),
        ));
    }
    // Only the empty mount points this half made; never anything recursive.
    let dirs = Stage::new(&stage);
    for dir in [&dirs.esp, &dirs.top, &dirs.root] {
        let _ = std::fs::remove_dir(dir);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::Target;
    use crate::worker::partition::fixtures::{config, disk_gpt_windows};

    #[test]
    fn swap_is_announced_only_when_there_is_a_swap_file() {
        let mut config = config(disk_gpt_windows(512), Target::WholeDisk);
        assert_eq!(
            planned_steps(&config),
            [Step::Partition, Step::Format, Step::Swap]
        );
        config.storage.swap_mib = 0;
        assert_eq!(planned_steps(&config), [Step::Partition, Step::Format]);
    }

    #[test]
    fn stage_directories_must_be_empty_directories() {
        let base = std::env::temp_dir().join(format!("lxb-stage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let dirs = Stage::new(&base);
        make_dir(&dirs.root).unwrap();
        make_dir(&dirs.root).unwrap();
        std::fs::write(dirs.root.join("left-over"), b"").unwrap();
        assert!(make_dir(&dirs.root).is_err());
        std::fs::write(&dirs.top, b"").unwrap();
        assert!(make_dir(&dirs.top).is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn releasing_a_stage_with_nothing_mounted_does_nothing() {
        let base = std::env::temp_dir().join(format!("lxb-release-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("root")).unwrap();
        release(&base).unwrap();
        assert!(
            !base.join("root").exists(),
            "its empty mount point is tidied"
        );
        release(&base.join("gone")).unwrap();
        std::fs::remove_dir_all(&base).unwrap();
    }
}
