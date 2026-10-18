//! What the disk says about itself: the table on the disk (`sfdisk --json`)
//! and the partitions the kernel knows (`lsblk --json`), and the checks the
//! worker makes with them before and after writing. Pure parsing, so it is
//! tested with small hand-written outputs.
//!
//! The two views are used for different things on purpose. Before writing,
//! the table on the disk is what sfdisk is about to change, so that is what
//! must still match the plan. After writing, the kernel's partitions are what
//! has a device node, so the new partitions' paths are read from lsblk and
//! never built from the disk's name.
use super::plan::{is_extended, Change, NewPartition, Plan, Role, Span};
use crate::model::disks::Table;
use serde::{Deserialize, Deserializer};

/// The kernel counts a partition's start in 512-byte units whatever the
/// disk's sector size (`/sys/class/block/*/start`, lsblk's START).
pub const KERNEL_SECTOR: u64 = 512;

/// A partition table as `sfdisk --json` prints it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnDisk {
    pub table: Table,
    /// Absent from the output of older sfdisk versions.
    pub sector: Option<u64>,
    pub partitions: Vec<Entry>,
}

/// One partition in the table, in the table's sectors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub number: u32,
    pub start: u64,
    pub size: u64,
    pub part_type: String,
    pub bootable: bool,
    pub name: String,
    pub uuid: String,
}

impl Entry {
    pub fn span(&self) -> Span {
        Span {
            start: self.start,
            end: self.start + self.size,
        }
    }
}

#[derive(Deserialize)]
struct SfdiskOutput {
    partitiontable: SfdiskTable,
}

#[derive(Deserialize)]
struct SfdiskTable {
    label: String,
    #[serde(default, deserialize_with = "number")]
    sectorsize: Option<u64>,
    #[serde(default)]
    partitions: Vec<SfdiskPartition>,
}

#[derive(Deserialize)]
struct SfdiskPartition {
    node: String,
    #[serde(deserialize_with = "number")]
    start: Option<u64>,
    #[serde(deserialize_with = "number")]
    size: Option<u64>,
    #[serde(default, rename = "type")]
    part_type: String,
    #[serde(default)]
    bootable: bool,
    #[serde(default)]
    name: String,
    #[serde(default)]
    uuid: String,
}

/// A number that some util-linux versions print as a JSON string.
fn number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    use serde::de::Error;
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => n
            .as_u64()
            .map(Some)
            .ok_or_else(|| D::Error::custom("not a whole number")),
        Some(serde_json::Value::String(s)) if s.is_empty() => Ok(None),
        Some(serde_json::Value::String(s)) => s.parse().map(Some).map_err(D::Error::custom),
        Some(_) => Err(D::Error::custom("not a number")),
    }
}

/// The number at the end of a partition node sfdisk names. libfdisk always
/// writes the partition number last (`sda3`, `nvme0n1p3`, `mmcblk0p12`,
/// `disk.img3`), so the trailing digits are the number; this reads a number
/// out of a name and never makes a path from one.
fn trailing_number(node: &str) -> Option<u32> {
    let digits = node.len() - node.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    node[node.len() - digits..].parse().ok()
}

pub fn parse_sfdisk(json: &str) -> Result<OnDisk, String> {
    let output: SfdiskOutput =
        serde_json::from_str(json).map_err(|e| format!("sfdisk printed unexpected JSON: {e}"))?;
    let table = match output.partitiontable.label.as_str() {
        "gpt" => Table::Gpt,
        "dos" => Table::Msdos,
        other => return Err(format!("unsupported partition table {other:?}")),
    };
    let mut partitions = Vec::new();
    for part in output.partitiontable.partitions {
        let (Some(start), Some(size), Some(number)) =
            (part.start, part.size, trailing_number(&part.node))
        else {
            return Err(format!(
                "sfdisk printed an incomplete partition {}",
                part.node
            ));
        };
        partitions.push(Entry {
            number,
            start,
            size,
            part_type: part.part_type,
            bootable: part.bootable,
            name: part.name,
            uuid: part.uuid,
        });
    }
    Ok(OnDisk {
        table,
        sector: output.partitiontable.sectorsize,
        partitions,
    })
}

/// Refuses unless the table on the disk is still the one the plan was made
/// from, as far as the plan touches it: the same kind of table and sector
/// size, the replaced partition at exactly its old place under its old
/// number, and nothing else inside the space the plan writes into.
pub fn check_before_writing(plan: &Plan, on_disk: &OnDisk) -> Result<(), String> {
    if on_disk.table != plan.table {
        return Err(format!(
            "{} now has a {:?} table, not {:?}",
            plan.disk, on_disk.table, plan.table
        ));
    }
    if let Some(sector) = on_disk.sector {
        if sector != plan.sector {
            return Err(format!(
                "{} now has {sector} byte sectors, not {}",
                plan.disk, plan.sector
            ));
        }
    }
    let replaced = match &plan.change {
        Change::Fresh => return Ok(()),
        Change::Add => None,
        Change::Retype { number, span, .. } | Change::Replace { number, span, .. } => {
            Some((*number, *span))
        }
    };
    if let Some((number, span)) = replaced {
        if !on_disk
            .partitions
            .iter()
            .any(|e| e.number == number && e.span() == span)
        {
            return Err(format!(
                "partition {number} of {} is no longer at sectors {}..{}",
                plan.disk, span.start, span.end
            ));
        }
    }
    let retype = matches!(plan.change, Change::Retype { .. });
    for entry in &on_disk.partitions {
        if Some((entry.number, entry.span())) == replaced {
            continue;
        }
        // A re-typed logical partition lies inside its extended container,
        // which is not touched; nothing else may share the space.
        if retype && is_extended(&entry.part_type) {
            continue;
        }
        if entry.span().overlaps(plan.space) {
            return Err(format!(
                "partition {} of {} now lies in sectors {}..{}",
                entry.number, plan.disk, plan.space.start, plan.space.end
            ));
        }
    }
    Ok(())
}

/// A partition as the kernel has it, start and size in bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kernel {
    pub path: String,
    pub number: u32,
    pub start: u64,
    pub size: u64,
}

#[derive(Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Deserialize)]
struct LsblkDevice {
    name: String,
    #[serde(default, deserialize_with = "number")]
    start: Option<u64>,
    #[serde(default, deserialize_with = "number")]
    size: Option<u64>,
    #[serde(default, deserialize_with = "number")]
    partn: Option<u64>,
    #[serde(default, rename = "log-sec", deserialize_with = "number")]
    log_sec: Option<u64>,
    #[serde(default)]
    children: Vec<LsblkDevice>,
}

fn parse_lsblk(json: &str) -> Result<LsblkOutput, String> {
    serde_json::from_str(json).map_err(|e| format!("lsblk printed unexpected JSON: {e}"))
}

/// The size in bytes and logical sector size of `disk`, from
/// `lsblk --json --bytes --paths --nodeps -o NAME,SIZE,LOG-SEC <disk>`.
pub fn parse_device(json: &str, disk: &str) -> Result<(u64, u64), String> {
    let output = parse_lsblk(json)?;
    let device = output
        .blockdevices
        .iter()
        .find(|d| d.name == disk)
        .ok_or_else(|| format!("lsblk did not list {disk}"))?;
    match (device.size, device.log_sec) {
        (Some(size), Some(sector)) => Ok((size, sector)),
        _ => Err(format!("lsblk did not give the size of {disk}")),
    }
}

/// The partitions of `disk` from
/// `lsblk --json --bytes --paths -o NAME,START,SIZE,PARTN <disk>`.
pub fn parse_partitions(json: &str, disk: &str) -> Result<Vec<Kernel>, String> {
    let output = parse_lsblk(json)?;
    let device = output
        .blockdevices
        .iter()
        .find(|d| d.name == disk)
        .ok_or_else(|| format!("lsblk did not list {disk}"))?;
    let mut partitions = Vec::new();
    for child in &device.children {
        // Anything stacked on the disk that is not a partition (it has no
        // start or number) is not one of ours.
        if let (Some(start), Some(size), Some(number)) = (child.start, child.size, child.partn) {
            partitions.push(Kernel {
                path: child.name.clone(),
                number: u32::try_from(number).map_err(|_| "partition number out of range")?,
                start: start * KERNEL_SECTOR,
                size,
            });
        }
    }
    Ok(partitions)
}

/// A partition the plan made, found again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub role: Role,
    pub path: String,
    pub number: u32,
}

/// The kernel partition at exactly this place, if there is one.
fn at(kernel: &[Kernel], part: NewPartition, sector: u64) -> Option<&Kernel> {
    kernel
        .iter()
        .find(|k| k.start == part.start * sector && k.size == part.size * sector)
}

/// Every partition the plan created, by its exact start and size, or `None`
/// while the kernel does not show all of them yet. Matching the size as
/// well as the start keeps a stale entry for a partition that was deleted
/// at the same start from being taken for the new one.
pub fn find_created(plan: &Plan, kernel: &[Kernel]) -> Option<Vec<Found>> {
    plan.create
        .iter()
        .map(|part| {
            at(kernel, *part, plan.sector).map(|k| Found {
                role: part.role,
                path: k.path.clone(),
                number: k.number,
            })
        })
        .collect()
}

/// The re-typed root partition, still at its place under its path and number.
pub fn find_kept(path: &str, number: u32, span: Span, sector: u64, kernel: &[Kernel]) -> bool {
    kernel.iter().any(|k| {
        k.path == path
            && k.number == number
            && k.start == span.start * sector
            && k.size == span.len() * sector
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GPT: &str = r#"{
       "partitiontable": {
          "label": "gpt", "id": "11111111-2222-3333-4444-555555555555",
          "device": "/dev/nvme9n1", "unit": "sectors", "firstlba": 256,
          "lastlba": 16777210, "sectorsize": 4096,
          "partitions": [
             {"node": "/dev/nvme9n1p1", "start": 256, "size": 25600,
              "type": "C12A7328-F81F-11D2-BA4B-00A0C93EC93B",
              "uuid": "AAAAAAAA-0000-0000-0000-000000000001", "name": "EFI system partition"},
             {"node": "/dev/nvme9n1p12", "start": 29952, "size": 3145728,
              "type": "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7",
              "uuid": "AAAAAAAA-0000-0000-0000-000000000003", "name": "Basic data partition"}
          ]
       }
    }"#;

    const DOS: &str = r#"{
       "partitiontable": {
          "label": "dos", "id": "0x0badcafe", "device": "/dev/sdx",
          "unit": "sectors", "sectorsize": "512",
          "partitions": [
             {"node": "/dev/sdx1", "start": 2048, "size": 1024000, "type": "7", "bootable": true},
             {"node": "/dev/sdx2", "start": 1026048, "size": 40960000, "type": "5"},
             {"node": "/dev/sdx5", "start": 1028096, "size": 20000000, "type": "83"}
          ]
       }
    }"#;

    #[test]
    fn sfdisk_tables_are_read_with_their_numbers_and_units() {
        let gpt = parse_sfdisk(GPT).unwrap();
        assert_eq!(gpt.table, Table::Gpt);
        assert_eq!(gpt.sector, Some(4096));
        assert_eq!(gpt.partitions[1].number, 12);
        assert_eq!(gpt.partitions[1].name, "Basic data partition");
        let dos = parse_sfdisk(DOS).unwrap();
        assert_eq!(dos.table, Table::Msdos);
        assert_eq!(dos.sector, Some(512));
        assert!(dos.partitions[0].bootable);
        assert!(!dos.partitions[2].bootable);
        assert_eq!(dos.partitions[2].number, 5);
        assert!(parse_sfdisk(r#"{"partitiontable":{"label":"sun"}}"#).is_err());
        assert!(parse_sfdisk("not json").is_err());
    }

    #[test]
    fn trailing_numbers_survive_digits_inside_the_disk_name() {
        assert_eq!(trailing_number("/dev/nvme0n1p3"), Some(3));
        assert_eq!(trailing_number("/dev/mmcblk1p12"), Some(12));
        assert_eq!(trailing_number("/dev/sda7"), Some(7));
        assert_eq!(trailing_number("/dev/sda"), None);
    }

    const LSBLK: &str = r#"{
       "blockdevices": [
          {"name": "/dev/nvme9n1", "start": null, "size": 68719476736, "partn": null,
           "children": [
              {"name": "/dev/nvme9n1p1", "start": 2048, "size": 104857600, "partn": 1},
              {"name": "/dev/nvme9n1p5", "start": "25427968", "size": "27951108096", "partn": "5"},
              {"name": "/dev/mapper/held", "start": null, "size": 1048576, "partn": null}
           ]}
       ]
    }"#;

    #[test]
    fn kernel_starts_are_512_byte_units_whatever_the_sector() {
        let parts = parse_partitions(LSBLK, "/dev/nvme9n1").unwrap();
        assert_eq!(parts.len(), 2, "a stacked device is not a partition");
        assert_eq!(parts[0].start, 2048 * 512);
        assert_eq!(parts[1].number, 5);
        assert_eq!(parts[1].start, 25427968 * 512);
        assert!(parse_partitions(LSBLK, "/dev/sdx").is_err());
    }

    #[test]
    fn device_size_and_sector_are_read() {
        let json = r#"{"blockdevices":[{"name":"/dev/sdx","size":68719476736,"log-sec":4096}]}"#;
        assert_eq!(parse_device(json, "/dev/sdx").unwrap(), (68719476736, 4096));
        let missing = r#"{"blockdevices":[{"name":"/dev/sdx","size":1}]}"#;
        assert!(parse_device(missing, "/dev/sdx").is_err());
    }
}
