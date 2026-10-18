//! The system half of the worker: copy the image and turn it into an
//! installed system (accounts, region, desktop, software, drivers,
//! snapshots, initramfs, boot loader, live-session cleanup), plus the
//! checks and the reporting the whole worker runs on. Owned by the system
//! work.
//!
//! Every step is a function taking the one [`Install`] that describes this
//! run. Whatever a step decides is worked out by a pure function next to it
//! (which is what the unit tests exercise); the step itself only reads the
//! target, runs commands and writes files.
pub mod accounts;
pub mod boot;
pub mod check;
pub mod copy;
pub mod desktop;
pub mod drivers;
pub mod image;
pub mod initramfs;
pub mod network;
pub mod packages;
pub mod plan;
pub mod region;
pub mod report;
pub mod snapshots;
pub mod software;
pub mod tidy;

use super::layout::{Mounted, Stop};
use crate::model::config::InstallConfig;
use crate::model::protocol::{Event, Failure, Step, Warning};
use image::Image;
use std::path::Path;

/// What runs one command inside the target, given its program and its
/// arguments: `run::chroot` on the machine — in a test, a recorder that
/// runs nothing.
pub(crate) type Chroot<'a> = dyn FnMut(&str, &[&str]) -> Result<(), Stop> + 'a;

/// Everything a step needs to know about this run.
pub struct Install<'a> {
    pub config: &'a InstallConfig,
    pub mounted: &'a Mounted,
    pub image: &'a Image,
    /// The internet answered when the worker checked, before writing.
    pub online: bool,
    /// The package databases and the installed system have been brought up
    /// to date during this run, so a later install from the internet needs
    /// no second full upgrade.
    pub synced: bool,
    pub report: &'a mut dyn FnMut(Event),
}

impl Install<'_> {
    /// The target root, with everything below it mounted.
    pub fn root(&self) -> &Path {
        &self.mounted.root
    }

    pub fn warn(&mut self, warning: Warning) {
        (self.report)(Event::Warning { warning });
    }

    pub fn progress(&mut self, step: Step, done: u64, total: u64) {
        if total > 0 {
            (self.report)(Event::Progress {
                step,
                done: done.min(total),
                total,
            });
        }
    }
}

/// Runs one of the steps after the copy. The storage steps and the copy are
/// not here: storage runs them, and the copy is called on its own.
pub fn run_step(step: Step, install: &mut Install) -> Result<(), Stop> {
    match step {
        Step::System => tidy::run(install),
        Step::Region => region::run(install),
        Step::Accounts => accounts::run(install),
        Step::Desktop => desktop::run(install),
        Step::Updates => software::updates(install),
        Step::Packages => software::remove_packages(install),
        Step::Flatpaks => software::flatpaks(install),
        Step::Drivers => drivers::run(install),
        Step::Snapshots => snapshots::run(install),
        Step::Initramfs => initramfs::run(install),
        Step::Bootloader => boot::run(install),
        Step::Check
        | Step::Resize
        | Step::Partition
        | Step::Format
        | Step::Copy
        | Step::Swap
        | Step::Finish => Err(Stop::new(
            Failure::Internal,
            format!("{step:?} is not a system step"),
        )),
    }
}

/// A name the worker passes to a program as one argument: a package, a
/// kernel's version, a keymap. Letters, digits and `._+-@`, not starting
/// with `-` (so it can never be read as an option), at most 128 bytes.
pub fn safe_token(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 128
        && !text.starts_with('-')
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._+-@".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_cannot_become_options_or_paths() {
        assert!(safe_token("7.0.5-arch1-1"));
        assert!(safe_token("lib32-nvidia-utils"));
        assert!(safe_token("gtk+3"));
        for bad in ["", "-Rdd", "../linux", "a b", "x;reboot", "$(id)", "a/b"] {
            assert!(!safe_token(bad), "{bad:?}");
        }
        assert!(!safe_token(&"a".repeat(129)));
    }
}
