//! Making the filesystems and mounting the target.
//!
//! Every device is opened exclusively just before it is formatted, so a
//! partition that something took hold of after the table was written is
//! refused rather than formatted underneath it. The formatting tools are
//! given a single force flag each: enough to replace an old filesystem
//! without a question nobody can answer, not enough to format a device that
//! is mounted.
use super::blkid;
use super::mounts::{Facts, Mount};
use super::progress::Steps;
use super::Stage;
use crate::model::config::Filesystem;
use crate::model::protocol::Failure;
use crate::worker::layout::Stop;
use crate::worker::partition::exec::{claim, Written};
use crate::worker::partition::{EspPlan, Plan, Role};
use crate::worker::{run, target};
use std::path::Path;

/// The label of the root filesystem.
pub const ROOT_LABEL: &str = "Linexin";
/// The label of an EFI system partition the installer makes (FAT labels are
/// at most eleven upper-case characters).
pub const ESP_LABEL: &str = "EFI";
/// The label of the Linux boot partition the installer makes beside a
/// shared EFI system partition (the same FAT limits).
pub const BOOT_LABEL: &str = "BOOT";
/// The label of a separate /home the installer formats.
pub const HOME_LABEL: &str = "Home";

fn path_str(path: &Path) -> Result<&str, Stop> {
    path.to_str()
        .ok_or_else(|| Stop::new(Failure::Internal, "stage path is not UTF-8"))
}

/// Takes `device` for formatting: claims it exclusively, then erases every
/// filesystem signature on it. A partition being reused can carry an old
/// NTFS or ext4 signature in places a new filesystem does not overwrite (NTFS
/// keeps a backup boot sector at its end), and blkid reporting two types for
/// one partition confuses every tool that later asks what it holds. Without
/// --force, wipefs refuses a device that is in use, like the claim before it.
fn take(device: &str) -> Result<(), Stop> {
    claim(device)?;
    run::run("wipefs", &["--all", device])?;
    Ok(())
}

/// The units of work `make` reports, so the step's total is known up front.
pub fn units(plan: &Plan) -> u64 {
    let esp = u64::from(plan.esp == EspPlan::New);
    let boot = u64::from(plan.created(Role::LinuxBoot).is_some());
    let subvolumes = u64::from(!plan.subvolumes.is_empty());
    let home = u64::from(plan.home.as_ref().is_some_and(|h| h.format));
    // The root, then reading the UUIDs back.
    esp + boot + subvolumes + home + 2
}

/// Formats what the plan formats, creates the Btrfs subvolumes, and reads
/// back the devices' identities.
pub fn make(
    plan: &Plan,
    written: &Written,
    kept_home_fstype: Option<String>,
    stage: &Stage,
    steps: &mut Steps,
) -> Result<Facts, Stop> {
    if let (EspPlan::New, Some(esp)) = (&plan.esp, &written.esp) {
        take(&esp.path)?;
        run::run("mkfs.fat", &["-F", "32", "-n", ESP_LABEL, &esp.path])?;
        steps.advance();
    }
    // FAT32 like an EFI partition, so that the firmware itself reads the
    // kernels on it for whichever boot loader starts them.
    if let Some(boot) = &written.boot {
        take(&boot.path)?;
        run::run("mkfs.fat", &["-F", "32", "-n", BOOT_LABEL, &boot.path])?;
        steps.advance();
    }
    let root = &written.root.path;
    take(root)?;
    match plan.filesystem {
        Filesystem::Ext4 => run::run("mkfs.ext4", &["-F", "-L", ROOT_LABEL, root])?,
        Filesystem::Btrfs => run::run("mkfs.btrfs", &["-f", "-L", ROOT_LABEL, root])?,
    };
    steps.advance();
    if !plan.subvolumes.is_empty() {
        let names: Vec<&str> = plan.subvolumes.iter().map(|one| one.name).collect();
        subvolumes(&names, root, &stage.top)?;
        steps.advance();
    }
    if let Some(home) = plan.home.as_ref().filter(|h| h.format) {
        take(&home.path)?;
        run::run("mkfs.ext4", &["-F", "-L", HOME_LABEL, &home.path])?;
        steps.advance();
    }
    // Let udev see the new filesystems before anything asks about them.
    if let Err(stop) = run::run("udevadm", &["settle", "--timeout=60"]) {
        steps.log(format!("udevadm: {}", stop.detail));
    }
    let esp = match &plan.esp {
        EspPlan::None => None,
        EspPlan::New => written.esp.as_ref().map(|e| e.path.clone()),
        EspPlan::Shared { path } => Some(path.clone()),
    };
    let boot = written.boot.as_ref().map(|b| b.path.clone());
    let facts = Facts {
        root: root.clone(),
        root_uuid: blkid::uuid(root)?,
        esp_uuid: esp.as_deref().map(blkid::uuid).transpose()?,
        esp,
        boot_uuid: boot.as_deref().map(blkid::uuid).transpose()?,
        boot,
        home: plan.home.as_ref().map(|h| h.path.clone()),
        home_uuid: plan
            .home
            .as_ref()
            .map(|h| blkid::uuid(&h.path))
            .transpose()?,
        home_fstype: kept_home_fstype,
    };
    steps.log(format!(
        "storage: root {} UUID {}, EFI {:?} UUID {:?}, Linux boot {:?} UUID {:?}, \
         home {:?} UUID {:?}",
        facts.root,
        facts.root_uuid,
        facts.esp,
        facts.esp_uuid,
        facts.boot,
        facts.boot_uuid,
        facts.home,
        facts.home_uuid
    ));
    steps.advance();
    Ok(facts)
}

/// Creates the subvolumes `names`, in their order, in the top level of the
/// new Btrfs filesystem on `device`, mounted privately at `top` for the
/// purpose and unmounted again: the easy path's plan's (`@`, `@home` where
/// no separate /home is, `@swap` beside a swap file), and a hand-made
/// layout's, which `manual` works out by the same rule.
pub fn subvolumes(names: &[&str], device: &str, top: &Path) -> Result<(), Stop> {
    super::make_dir(top)?;
    let top_str = path_str(top)?;
    run::run(
        "mount",
        &["-t", "btrfs", "-o", "subvolid=5", device, top_str],
    )?;
    let created = names.iter().try_for_each(|name| {
        let path = top.join(name);
        run::run("btrfs", &["subvolume", "create", path_str(&path)?]).map(drop)
    });
    // Unmounted even when a subvolume failed; `release` would also do it,
    // but the top level is never left mounted longer than needed.
    let unmounted = run::run("umount", &[top_str]);
    created?;
    unmounted?;
    let _ = std::fs::remove_dir(top);
    Ok(())
}

/// Mounts the target at `stage.root`, in the mount table's order.
pub fn mount_all(
    mounts: &[Mount],
    facts: &Facts,
    stage: &Stage,
    steps: &mut Steps,
) -> Result<(), Stop> {
    super::make_dir(&stage.root)?;
    for mount in mounts {
        let device = facts
            .device(mount.source)
            .ok_or_else(|| Stop::new(Failure::Internal, format!("no device for {}", mount.at)))?;
        let fstype = facts.fstype(mount.kind).ok_or_else(|| {
            Stop::new(
                Failure::Internal,
                format!("no filesystem type for {}", mount.at),
            )
        })?;
        let relative = mount.at.trim_start_matches('/');
        let at = if relative.is_empty() {
            stage.root.clone()
        } else {
            // Inside the new root, never through a symlink out of it.
            target::create_dir(&stage.root, relative)?;
            target::path(&stage.root, relative)?
        };
        let at = path_str(&at)?;
        let mut args = vec!["-t", fstype];
        if let Some(options) = mount.mount_options() {
            args.extend(["-o", options]);
        }
        args.extend([device, at]);
        run::run("mount", &args)?;
        steps.advance();
    }
    Ok(())
}
