//! The partition table: what it must look like after the installer has
//! written it, and writing it.
//!
//! Owned by the storage work. The split is deliberate. [`plan`] is pure: from
//! the person's choices and the disk exactly as they reviewed it, it works out
//! every partition that is deleted, re-typed or created, in sectors of the
//! disk's own size, so that every combination of firmware, table, target and
//! sector size can be tested on a development machine. [`script`] turns a
//! plan into the exact sfdisk input and command lines, [`table`] reads what
//! sfdisk and lsblk say about a disk, and [`exec`] is the only part that
//! touches a device.
pub mod exec;
pub mod plan;
pub mod script;
pub mod table;

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
pub(in crate::worker) mod images;
#[cfg(test)]
mod matrix;

pub use plan::{
    plan, Change, EspPlan, NewPartition, Plan, Role, SharedRoom, Span, Subvolume, Swapfile,
};
