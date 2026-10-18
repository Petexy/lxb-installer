//! What the read-only measure entry (`lxb-installer --measure`) is asked and
//! what it answers: how full the partitions Partitions shows are, and whether
//! each may be shrunk, which the interface cannot see without privilege.
//!
//! The interface asks once Partitions opens on the live system, through
//! polkit's own read-only action (`io.github.petexy.lxb-installer.measure`),
//! with the disks it listed; the measure entry finds each disk again, reads
//! what its tools say of each partition that could be kept or shrunk, and
//! answers with one line. Nothing it answers is trusted to write with: the
//! worker measures again itself before its first write. Plain data, as all
//! of `model` is; the reading is the worker's (`worker::measure`).
//!
//! **What is read** — [`Measure`], [`Measured`], [`Measures`],
//! [`ImageSizes`] and [`IMAGE_PATHS`] — is `model::layout`'s own, which
//! judges a layout by them (`layout::measured`): the measure entry answers
//! in the very shapes the rule set reads, so nothing stands between what is
//! read and what is judged.
use super::disks::Disk;
pub use super::layout::{ImageSizes, Measure, Measured, Measures, IMAGE_PATHS};
use super::protocol::Failure;
use serde::{Deserialize, Serialize};

/// The largest answer the measure entry prints, and so the most the
/// interface reads of it: a line the interface would have to cut short is
/// refused by the measure entry itself rather than sent. The largest answer
/// the interface can ask for — four disks of 128 partitions — is far below
/// it (`the_largest_answer_fits`).
pub const MAX_ANSWER: u64 = 256 * 1024;

/// What the interface asks: the disks as it listed them, each whole, as
/// Where to install and Partitions were shown them. The measure entry finds
/// each again and reads only the partitions of a disk that is still the one
/// listed, so a request can make it read nothing the person was not shown,
/// and never those of a disk listed as one nothing may be made on (the
/// installation medium, a read-only disk, one not read completely), which
/// the interface may send all the same. It holds no secret. At most
/// `protocol::MAX_REQUEST` bytes are read of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasureRequest {
    pub disks: Vec<Disk>,
}

/// The one line the measure entry prints on its standard output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "answer", rename_all = "kebab-case", deny_unknown_fields)]
pub enum MeasureAnswer {
    /// What was read of each partition that could be kept or shrunk on the
    /// disks asked about (a partition nothing is read of — a swap, a Btrfs —
    /// has no measure), what the image puts in each folder it fills, and the
    /// disks asked about that are not the disks listed any more — replugged,
    /// repartitioned, gone, or now unreadable — of which nothing was read:
    /// the interface lists the disks again. A disk listed as one nothing may
    /// be made on, and still exactly as listed, is not read nor named here:
    /// the listing is still true of it, so `changed` is empty when nothing
    /// changed, the installation medium beside. Of every disk not read,
    /// each NTFS and BitLocker volume as listed is answered unread all the
    /// same, so that every Windows volume of the disks asked about is in
    /// `measures` (`Measures::shrink_of` judges a shrink of an NTFS across
    /// them all).
    Measured {
        measures: Measures,
        image: ImageSizes,
        changed: Vec<String>,
    },
    /// Nothing was read: it is not the live medium, not root, another
    /// installation or measurement holds the lock, the request is not one,
    /// or the disks could not be listed. `detail` is technical, for the
    /// journal only.
    Refused { failure: Failure, detail: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::{Content, Partition, GIB, MIB};
    use crate::model::protocol::MAX_REQUEST;
    use crate::worker::partition::fixtures::disk_gpt_windows;

    fn partition(path: &str, uuid: &str, size: u64) -> Partition {
        Partition {
            path: path.into(),
            number: 3,
            start: GIB,
            size,
            fs_type: "ntfs".into(),
            label: String::new(),
            part_type: String::new(),
            uuid: uuid.into(),
            mountpoints: Vec::new(),
            content: Content::Windows,
            bootable: false,
        }
    }

    fn measured(path: &str, uuid: &str, size: u64, measure: Measure) -> Measured {
        Measured {
            path: path.into(),
            fs: "ntfs".into(),
            uuid: uuid.into(),
            size,
            measure,
        }
    }

    #[test]
    fn an_answer_is_one_line_and_comes_back_as_it_went() {
        let answers = [
            MeasureAnswer::Measured {
                measures: Measures {
                    parts: vec![
                        measured(
                            "/dev/vdx3",
                            "0A1B2C3D4E5F6071",
                            30 * GIB,
                            Measure::Read {
                                used: 12 * GIB,
                                free: 18 * GIB,
                                least: Some(13 * GIB),
                            },
                        ),
                        measured(
                            "/dev/vdx1",
                            "ABCD-1234",
                            100 * MIB,
                            Measure::Read {
                                used: 30 * MIB,
                                free: 70 * MIB,
                                least: None,
                            },
                        ),
                        measured("/dev/vdx5", "", GIB, Measure::NeedsCheck),
                    ],
                },
                image: ImageSizes {
                    paths: [("/var".to_string(), 5 * GIB)].into(),
                },
                changed: vec!["/dev/vdy".into()],
            },
            MeasureAnswer::Refused {
                failure: Failure::AlreadyRunning,
                detail: "another installation holds the lock".into(),
            },
        ];
        for answer in answers {
            let line = serde_json::to_string(&answer).unwrap();
            assert!(!line.contains('\n'));
            assert_eq!(
                serde_json::from_str::<MeasureAnswer>(&line).unwrap(),
                answer
            );
        }
        let line = serde_json::to_string(&Measure::InUse).unwrap();
        assert_eq!(line, r#"{"state":"in-use"}"#);
    }

    #[test]
    fn a_request_with_anything_unknown_in_it_is_no_request() {
        let request = MeasureRequest {
            disks: vec![disk_gpt_windows(512)],
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(
            serde_json::from_str::<MeasureRequest>(&json).unwrap(),
            request
        );
        let more = json.replacen('{', r#"{"run":"mkfs.ext4","#, 1);
        assert!(serde_json::from_str::<MeasureRequest>(&more).is_err());
        let deeper = json.replacen(r#""fs_type""#, r#""mount":"rw","fs_type""#, 1);
        assert!(serde_json::from_str::<MeasureRequest>(&deeper).is_err());
        let answer = r#"{"answer":"measured","measures":{"parts":[]},"image":{"paths":{}},"changed":[],"more":1}"#;
        assert!(serde_json::from_str::<MeasureAnswer>(answer).is_err());
        let fine = answer.replace(r#","more":1"#, "");
        assert!(serde_json::from_str::<MeasureAnswer>(&fine).is_ok());
    }

    #[test]
    fn a_measure_belongs_to_one_partition_only() {
        let measures = Measures {
            parts: vec![measured("/dev/vdx3", "0A1B", 30 * GIB, Measure::Hibernated)],
        };
        let read = partition("/dev/vdx3", "0A1B", 30 * GIB);
        assert_eq!(measures.of(&read), Some(&Measure::Hibernated));
        // Another filesystem at the same path, or the same one grown or
        // shrunk since, is not what was read.
        assert_eq!(measures.of(&partition("/dev/vdx3", "FFFF", 30 * GIB)), None);
        assert_eq!(measures.of(&partition("/dev/vdx3", "0A1B", 31 * GIB)), None);
        assert_eq!(measures.of(&partition("/dev/vdx4", "0A1B", 30 * GIB)), None);
    }

    /// The most the interface can ask about — four disks of 128 partitions,
    /// each with the longest measure — answered in one line well within
    /// [`MAX_ANSWER`], and asked within `MAX_REQUEST`.
    #[test]
    fn the_largest_answer_fits() {
        let mut disk = disk_gpt_windows(4096);
        let template = disk.partitions[2].clone();
        disk.partitions = (1..=128)
            .map(|n| Partition {
                path: format!("/dev/nvme0n1p{n}"),
                number: n,
                uuid: "01234567-89ab-cdef-0123-456789abcdef".into(),
                label: "A label as long as ext4's".into(),
                ..template.clone()
            })
            .collect();
        disk.path = "/dev/nvme0n1".into();
        let disks: Vec<Disk> = (0..4).map(|_| disk.clone()).collect();
        let request = serde_json::to_vec(&MeasureRequest {
            disks: disks.clone(),
        })
        .unwrap();
        assert!((request.len() as u64) < MAX_REQUEST, "{}", request.len());
        let parts = disks
            .iter()
            .flat_map(|d| d.partitions.iter())
            .map(|p| {
                let most = Measure::Read {
                    used: u64::MAX,
                    free: u64::MAX,
                    least: Some(u64::MAX),
                };
                measured(&p.path, &p.uuid, u64::MAX, most)
            })
            .collect();
        let image = ImageSizes {
            paths: IMAGE_PATHS
                .iter()
                .map(|p| (p.to_string(), u64::MAX))
                .collect(),
        };
        let changed = disks.iter().map(|d| d.path.clone()).collect();
        let answer = MeasureAnswer::Measured {
            measures: Measures { parts },
            image,
            changed,
        };
        let line = serde_json::to_string(&answer).unwrap();
        assert!((line.len() as u64) < MAX_ANSWER / 2, "{}", line.len());
    }
}
