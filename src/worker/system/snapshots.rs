//! Pre-update Btrfs snapshots.
//!
//! The legacy installer's own tool, unchanged: `linexin-snapshot` takes a
//! Timeshift snapshot of `@` before every upgrade (its pacman hook fires
//! once per upgrade transaction) and, with systemd-boot, a recovery entry
//! that boots it. Timeshift is not on the image, so this needs the
//! internet; without it, or when anything here fails, the system is
//! installed without snapshots and the interface says so.
use super::{packages, Install};
use crate::model::config::Filesystem;
use crate::model::protocol::Warning;
use crate::worker::layout::Stop;
use crate::worker::{run, target};

/// The legacy tool and its hook, as the legacy installer shipped them.
pub const SCRIPT: &str = include_str!("assets/linexin-snapshot");
pub const HOOK: &str = include_str!("assets/00-linexin-snapshot.hook");

pub const SCRIPT_PATH: &str = "usr/local/bin/linexin-snapshot";
pub const HOOK_PATH: &str = "etc/pacman.d/hooks/00-linexin-snapshot.hook";
/// What `linexin-snapshot configure` writes when it has set Timeshift up.
/// The tool says nothing and succeeds when its own checks find nothing to
/// do, so this file is how the installer knows it did something.
pub const TIMESHIFT_CONFIG: &str = "etc/timeshift/timeshift.json";

/// `Step::Snapshots`.
pub fn run(install: &mut Install) -> Result<(), Stop> {
    if install.mounted.filesystem != Filesystem::Btrfs || !install.config.software.snapshots {
        return Ok(());
    }
    if let Err(detail) = set_up(install) {
        run::log(&format!("snapshots: {detail}"));
        let root = install.root().to_path_buf();
        for path in [HOOK_PATH, SCRIPT_PATH] {
            let _ = target::remove(&root, path);
        }
        install.warn(Warning::SnapshotsUnavailable);
    }
    Ok(())
}

fn set_up(install: &mut Install) -> Result<(), String> {
    let root = install.root().to_path_buf();
    let missing = missing_packages(
        target::exists(&root, "usr/bin/timeshift"),
        target::exists(&root, "usr/bin/btrfs"),
    );
    if !missing.is_empty() {
        if !install.online {
            return Err(format!(
                "{} must be downloaded and the internet did not answer",
                missing.join(" and ")
            ));
        }
        packages::install(install, &missing).map_err(|stop| stop.detail)?;
    }
    let detail = |stop: Stop| stop.detail;
    target::create_dir(&root, "usr/local/bin").map_err(detail)?;
    target::write(&root, SCRIPT_PATH, SCRIPT, 0o755).map_err(detail)?;
    let root_text = run::utf8(&root).map_err(detail)?;
    let script = format!("/{SCRIPT_PATH}");
    let said = run::run("arch-chroot", &[root_text, &script, "configure"]).map_err(detail)?;
    for line in said.lines() {
        run::log(line);
    }
    if !target::is_file(&root, TIMESHIFT_CONFIG) {
        return Err("linexin-snapshot did not configure Timeshift".into());
    }
    target::create_dir(&root, "etc/pacman.d/hooks").map_err(detail)?;
    target::write(&root, HOOK_PATH, HOOK, 0o644).map_err(detail)?;
    Ok(())
}

/// The packages the snapshots need that the target does not have.
pub fn missing_packages(timeshift: bool, btrfs: bool) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if !timeshift {
        missing.push("timeshift");
    }
    if !btrfs {
        missing.push("btrfs-progs");
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_what_is_missing_is_downloaded() {
        assert!(missing_packages(true, true).is_empty());
        assert_eq!(missing_packages(false, true), ["timeshift"]);
        assert_eq!(missing_packages(false, false), ["timeshift", "btrfs-progs"]);
    }

    #[test]
    fn the_legacy_tool_and_hook_are_carried_whole() {
        assert!(SCRIPT.starts_with("#!/bin/bash\n"));
        assert!(SCRIPT.contains("create|configure"));
        assert!(HOOK.contains("Exec = /usr/local/bin/linexin-snapshot create pre-update"));
        assert!(HOOK.contains("When = PreTransaction"));
        assert!(HOOK.contains("NeedsTargets"));
        assert_eq!(format!("/{SCRIPT_PATH}"), "/usr/local/bin/linexin-snapshot");
    }
}
