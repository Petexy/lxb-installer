//! Reading a filesystem's identity from the device itself.
//!
//! Always the low-level probe (`blkid -p`): blkid's cache and udev's
//! database can still describe what a partition held before it was
//! formatted a moment ago, and the fstab must name what is there now.
use super::mounts::valid_uuid;
use crate::model::protocol::Failure;
use crate::worker::layout::Stop;
use crate::worker::run;

fn probe(device: &str, tag: &str) -> Result<String, Stop> {
    run::run("blkid", &["-p", "-s", tag, "-o", "value", device]).map(|out| out.trim().to_string())
}

/// The filesystem UUID of `device`.
pub fn uuid(device: &str) -> Result<String, Stop> {
    let uuid = probe(device, "UUID")?;
    if !valid_uuid(&uuid) {
        return Err(Stop::new(
            Failure::Internal,
            format!("{device} has no usable filesystem UUID ({uuid:?})"),
        ));
    }
    Ok(uuid)
}

/// The filesystem type on `device`, or empty when there is none (blkid
/// exits with an error when it finds nothing to report).
pub fn fstype(device: &str) -> Result<String, Stop> {
    match probe(device, "TYPE") {
        Ok(fstype) => Ok(fstype),
        Err(stop) if stop.failure == Failure::CommandFailed => Ok(String::new()),
        Err(stop) => Err(stop),
    }
}
