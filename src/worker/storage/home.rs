//! A separate /home, checked on the machine before anything is written.
//!
//! The request only describes the target disk, so a /home on another disk
//! cannot be judged from it; the planner checks what it can, and this looks
//! at the device itself: a partition, big enough, not a boot or system
//! partition, not in use, and, when its data is kept, a Linux filesystem
//! (the new account takes ownership of its folder there).
use super::blkid;
use crate::model::disks::Disk;
use crate::model::protocol::Failure;
use crate::worker::layout::Stop;
use crate::worker::partition::exec::claim;
use crate::worker::partition::plan::{is_extended, MIN_HOME};
use crate::worker::partition::Plan;
use crate::worker::run;
use serde::Deserialize;

/// The filesystems a kept /home may have.
pub const KEPT_HOME_FILESYSTEMS: &[&str] = &["ext4", "ext3", "ext2", "btrfs", "xfs", "f2fs"];

/// Partition types that are never a /home: EFI system, BIOS boot, Microsoft
/// reserved, Windows recovery (GPT, lower case), and the MBR EFI type.
const NEVER_HOME: &[&str] = &[
    "c12a7328-f81f-11d2-ba4b-00a0c93ec93b",
    "21686148-6449-6e6f-744e-656564454649",
    "e3c9e316-0b5c-4db8-817d-f92df00215ae",
    "de94bba4-06d1-4d40-a16a-bfd50179d6ac",
    "0xef",
];

#[derive(Debug, Deserialize)]
struct Output {
    blockdevices: Vec<Device>,
}

/// The /home device as lsblk describes it.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub pkname: Option<String>,
    #[serde(default)]
    pub size: Option<serde_json::Value>,
    #[serde(default)]
    pub parttype: Option<String>,
}

impl Device {
    fn bytes(&self) -> Option<u64> {
        match &self.size {
            Some(serde_json::Value::Number(n)) => n.as_u64(),
            Some(serde_json::Value::String(s)) => s.parse().ok(),
            _ => None,
        }
    }
}

/// `lsblk --json --bytes --paths --nodeps -o NAME,TYPE,PKNAME,SIZE,PARTTYPE`.
pub fn parse(json: &str, path: &str) -> Result<Device, String> {
    let output: Output =
        serde_json::from_str(json).map_err(|e| format!("lsblk printed unexpected JSON: {e}"))?;
    output
        .blockdevices
        .into_iter()
        .find(|d| d.name == path)
        .ok_or_else(|| format!("lsblk did not list {path}"))
}

/// Whether `device` can be the /home of an installation onto `plan`, judged
/// from lsblk's description alone.
pub fn judge(device: &Device, plan: &Plan, disk: &Disk) -> Result<(), Stop> {
    let path = &device.name;
    let refuse = |why: &str| Stop::new(Failure::InvalidRequest, format!("{path} {why}"));
    if device.kind != "part" {
        return Err(refuse("is not a partition"));
    }
    if device.pkname.as_deref() == Some(plan.disk.as_str()) && disk.partition(path).is_none() {
        return Err(Stop::new(
            Failure::DiskChanged,
            format!("{path} appeared on {} after it was reviewed", plan.disk),
        ));
    }
    // On another disk, it must still be a partition of the disk reviewed.
    if let Some(other) = plan.home.as_ref().and_then(|home| home.disk.as_ref()) {
        if device.pkname.as_deref() != Some(other.path.as_str()) {
            return Err(Stop::new(
                Failure::DiskChanged,
                format!(
                    "{path} is on {:?} now, not on {}, the disk reviewed for it",
                    device.pkname, other.path
                ),
            ));
        }
    }
    if device.bytes().is_none_or(|size| size < MIN_HOME) {
        return Err(refuse("is too small for /home"));
    }
    let part_type = device
        .parttype
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    if NEVER_HOME.contains(&part_type.as_str()) || is_extended(&part_type) {
        return Err(refuse("is a boot or system partition"));
    }
    Ok(())
}

/// Checks the separate /home on the machine. Returns the filesystem type of
/// a kept /home, `None` for one that will be formatted or for no /home.
pub fn check(plan: &Plan, disk: &Disk) -> Result<Option<String>, Stop> {
    let Some(home) = &plan.home else {
        return Ok(None);
    };
    let json = run::run(
        "lsblk",
        &[
            "--json",
            "--bytes",
            "--paths",
            "--nodeps",
            "-o",
            "NAME,TYPE,PKNAME,SIZE,PARTTYPE",
            &home.path,
        ],
    )
    .map_err(|stop| {
        Stop::new(
            Failure::DiskChanged,
            format!("{} cannot be found: {}", home.path, stop.detail),
        )
    })?;
    let device = parse(&json, &home.path).map_err(|e| Stop::new(Failure::Internal, e))?;
    judge(&device, plan, disk)?;
    claim(&home.path)?;
    if home.format {
        return Ok(None);
    }
    let fstype = blkid::fstype(&home.path)?;
    if !KEPT_HOME_FILESYSTEMS.contains(&fstype.as_str()) {
        return Err(Stop::new(
            Failure::InvalidRequest,
            format!(
                "{} holds {fstype:?}, not a Linux filesystem to keep as /home",
                home.path
            ),
        ));
    }
    Ok(Some(fstype))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::{Home, Target};
    use crate::worker::layout::Firmware;
    use crate::worker::partition::fixtures::{config, disk_gpt_windows, plan_for};

    fn setup(home: &str) -> (Plan, Disk) {
        let disk = disk_gpt_windows(512);
        let mut config = config(
            disk.clone(),
            Target::Partition {
                path: "/dev/vdx3".into(),
            },
        );
        config.storage.home = Some(Home {
            path: home.into(),
            format: true,
            disk: None,
        });
        (plan_for(&config, Firmware::Uefi), disk)
    }

    fn device(json: &str, path: &str) -> Device {
        parse(json, path).unwrap()
    }

    #[test]
    fn a_partition_on_another_disk_is_accepted() {
        let (plan, disk) = setup("/dev/vdy1");
        let json = r#"{"blockdevices":[{"name":"/dev/vdy1","type":"part","pkname":"/dev/vdy",
            "size":500107862016,"parttype":"0fc63daf-8483-4772-8e79-3d69d8477de4"}]}"#;
        assert!(judge(&device(json, "/dev/vdy1"), &plan, &disk).is_ok());
    }

    #[test]
    fn whole_disks_boot_partitions_and_small_ones_are_refused() {
        let (plan, disk) = setup("/dev/vdy1");
        for json in [
            r#"{"blockdevices":[{"name":"/dev/vdy1","type":"disk","pkname":null,"size":500107862016}]}"#,
            r#"{"blockdevices":[{"name":"/dev/vdy1","type":"part","pkname":"/dev/vdy","size":"999999999"}]}"#,
            r#"{"blockdevices":[{"name":"/dev/vdy1","type":"part","pkname":"/dev/vdy","size":500107862016,
                "parttype":"C12A7328-F81F-11D2-BA4B-00A0C93EC93B"}]}"#,
            r#"{"blockdevices":[{"name":"/dev/vdy1","type":"part","pkname":"/dev/vdy","size":500107862016,
                "parttype":"0x5"}]}"#,
        ] {
            let stop = judge(&device(json, "/dev/vdy1"), &plan, &disk).unwrap_err();
            assert_eq!(stop.failure, Failure::InvalidRequest, "{json}");
        }
    }

    #[test]
    fn a_home_on_a_disk_named_for_it_must_still_be_there() {
        let (mut plan, disk) = setup("/dev/vdy1");
        let mut other = disk.clone();
        other.path = "/dev/vdy".into();
        if let Some(home) = plan.home.as_mut() {
            home.disk = Some(other);
        }
        let on = |parent: &str| {
            format!(
                r#"{{"blockdevices":[{{"name":"/dev/vdy1","type":"part","pkname":"{parent}",
                "size":500107862016,"parttype":"0fc63daf-8483-4772-8e79-3d69d8477de4"}}]}}"#
            )
        };
        assert!(judge(&device(&on("/dev/vdy"), "/dev/vdy1"), &plan, &disk).is_ok());
        // The same name on another disk now: a replug, or a different disk.
        let stop = judge(&device(&on("/dev/vdz"), "/dev/vdy1"), &plan, &disk).unwrap_err();
        assert_eq!(stop.failure, Failure::DiskChanged);
    }

    #[test]
    fn a_partition_that_appeared_on_the_target_disk_means_it_changed() {
        let (plan, disk) = setup("/dev/vdx9");
        let json = r#"{"blockdevices":[{"name":"/dev/vdx9","type":"part","pkname":"/dev/vdx",
            "size":500107862016}]}"#;
        let stop = judge(&device(json, "/dev/vdx9"), &plan, &disk).unwrap_err();
        assert_eq!(stop.failure, Failure::DiskChanged);
    }
}
