//! The steps this run takes, worked out once from the request and the image
//! and announced as `Event::Plan` before the first step begins. The worker
//! then runs exactly these steps in this order, so what the interface counts
//! ("step 3 of 11") is what happens.
use super::image::Image;
use crate::model::config::{Filesystem, InstallConfig, Target};
use crate::model::protocol::Step;
use crate::worker::{manual, storage};
use std::collections::BTreeSet;

/// Packages removed from every installed system when the image has them:
/// the installers (this one included) and the partition tools that only a
/// live medium needs, as the legacy installer removed them.
pub const ALWAYS_REMOVE: &[&str] = &[
    "linexin-installer",
    "lxb-installer",
    "archinstall",
    "gparted",
];

/// The packages the Packages step removes: what the person deselected, then
/// the installers, each once and only when the image has it.
pub fn removals(config: &InstallConfig, installed: &BTreeSet<String>) -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    let chosen = config.software.remove.iter().map(String::as_str);
    for name in chosen.chain(ALWAYS_REMOVE.iter().copied()) {
        if installed.contains(name) && !list.iter().any(|n| n == name) {
            list.push(name.to_string());
        }
    }
    list
}

/// Every step of this run, in the order it runs. The storage steps are the
/// ones storage reports (`storage::planned_steps`) — or, for a layout made
/// by hand, its executor's (`manual::planned_steps`) — and they all run
/// before the copy: the swap file is made while the target is being
/// mounted.
pub fn steps(config: &InstallConfig, image: &Image) -> Vec<Step> {
    let software = &config.software;
    let mut steps = vec![Step::Check];
    match &config.target {
        Target::Layout { .. } => steps.extend(manual::planned_steps(config)),
        Target::WholeDisk | Target::FreeSpace { .. } | Target::Partition { .. } => {
            steps.extend(storage::planned_steps(config))
        }
    }
    steps.extend([
        Step::Copy,
        Step::System,
        Step::Region,
        Step::Accounts,
        Step::Desktop,
    ]);
    if software.updates {
        steps.push(Step::Updates);
    }
    if !removals(config, &image.packages).is_empty() {
        steps.push(Step::Packages);
    }
    if !software.flatpaks.is_empty() {
        steps.push(Step::Flatpaks);
    }
    if software.snapshots && config.storage.filesystem == Filesystem::Btrfs {
        steps.push(Step::Snapshots);
    }
    // After every step that may install packages: drivers are removed without
    // their dependents, and a later transaction that resolves those again
    // (Snapshots installing Timeshift with a full upgrade did) puts the
    // default provider straight back — seen in QEMU, where the removed
    // lib32-nvidia-utils returned as the provider of lib32-vulkan-driver.
    // The removal itself fires no snapshot: the hook is on upgrades only.
    if software.prune_drivers {
        steps.push(Step::Drivers);
    }
    steps.extend([Step::Initramfs, Step::Bootloader, Step::Finish]);
    steps
}

/// The steps the worker runs itself once storage has mounted the target:
/// everything from the copy up to, not including, `Finish`.
pub fn after_storage(steps: &[Step]) -> Vec<Step> {
    steps
        .iter()
        .copied()
        .skip_while(|step| *step != Step::Copy)
        .take_while(|step| *step != Step::Finish)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::{Desktop, Target};
    use crate::worker::partition::fixtures::{config, disk_empty};
    use crate::worker::system::image::fixture;

    fn quiet() -> InstallConfig {
        let mut config = config(disk_empty(512), Target::WholeDisk);
        config.storage.swap_mib = 0;
        config.software.prune_drivers = false;
        config.software.snapshots = false;
        config
    }

    #[test]
    fn the_smallest_run_takes_only_the_steps_every_run_needs() {
        let image = fixture(&[Desktop::Linexin], &["gnome-shell"]);
        assert_eq!(
            steps(&quiet(), &image),
            [
                Step::Check,
                Step::Partition,
                Step::Format,
                Step::Copy,
                Step::System,
                Step::Region,
                Step::Accounts,
                Step::Desktop,
                Step::Initramfs,
                Step::Bootloader,
                Step::Finish,
            ]
        );
    }

    #[test]
    fn every_choice_adds_its_step_in_its_place() {
        let image = fixture(&[Desktop::Linexin], &["gnome-maps", "linexin-installer"]);
        let mut config = quiet();
        config.storage.swap_mib = 4096;
        config.storage.filesystem = Filesystem::Btrfs;
        config.software.updates = true;
        config.software.flatpaks = vec!["org.example.App".into()];
        config.software.prune_drivers = true;
        config.software.snapshots = true;
        assert_eq!(
            steps(&config, &image),
            [
                Step::Check,
                Step::Partition,
                Step::Format,
                Step::Swap,
                Step::Copy,
                Step::System,
                Step::Region,
                Step::Accounts,
                Step::Desktop,
                Step::Updates,
                Step::Packages,
                Step::Flatpaks,
                Step::Snapshots,
                Step::Drivers,
                Step::Initramfs,
                Step::Bootloader,
                Step::Finish,
            ]
        );
    }

    /// **The steps an easy request takes are as they were, and a layout
    /// made by hand reports its executor's** (Unit Q, Q5): for every
    /// fixture request — each fixture disk at 512 and 4096 bytes a sector,
    /// every place on it, ext4 and Btrfs, a swap file or none, with every
    /// software choice on and off — the steps are those the code before
    /// planned, with storage's own steps in their place; a layout's are
    /// `manual::planned_steps` in the same place: its kept filesystems
    /// shrunk first where it shrinks any (Q9), its tables, its filesystems,
    /// and a swap file's step only beside a swap file.
    #[test]
    fn the_easy_paths_steps_are_as_they_were_and_a_layout_reports_its_own() {
        use crate::worker::partition::fixtures::{all, layout_request, targets};
        let image = fixture(&[Desktop::Linexin], &["gnome-maps", "linexin-installer"]);
        // `steps` as it was before a layout made by hand reported its own,
        // word for word.
        let before = |config: &InstallConfig| {
            let software = &config.software;
            let mut steps = vec![Step::Check];
            steps.extend(storage::planned_steps(config));
            steps.extend([
                Step::Copy,
                Step::System,
                Step::Region,
                Step::Accounts,
                Step::Desktop,
            ]);
            if software.updates {
                steps.push(Step::Updates);
            }
            if !removals(config, &image.packages).is_empty() {
                steps.push(Step::Packages);
            }
            if !software.flatpaks.is_empty() {
                steps.push(Step::Flatpaks);
            }
            if software.snapshots && config.storage.filesystem == Filesystem::Btrfs {
                steps.push(Step::Snapshots);
            }
            if software.prune_drivers {
                steps.push(Step::Drivers);
            }
            steps.extend([Step::Initramfs, Step::Bootloader, Step::Finish]);
            steps
        };
        for sector in [512, 4096] {
            for (name, disk) in all(sector) {
                for (place, target) in targets(&disk) {
                    let mut easy = config(disk.clone(), target);
                    for filesystem in [Filesystem::Ext4, Filesystem::Btrfs] {
                        for swap_mib in [0, 2048] {
                            for every in [false, true] {
                                easy.storage.filesystem = filesystem;
                                easy.storage.swap_mib = swap_mib;
                                easy.software.updates = every;
                                easy.software.snapshots = every;
                                easy.software.prune_drivers = every;
                                easy.software.flatpaks = if every {
                                    vec!["org.example.App".into()]
                                } else {
                                    Vec::new()
                                };
                                assert_eq!(
                                    steps(&easy, &image),
                                    before(&easy),
                                    "{name} {sector} {place} {filesystem:?} {swap_mib} {every}"
                                );
                            }
                        }
                    }
                }
            }
        }
        let mut laid = layout_request(512).config;
        laid.storage.swap_mib = 0;
        laid.software.prune_drivers = false;
        laid.software.snapshots = false;
        assert_eq!(
            manual::planned_steps(&laid),
            [Step::Partition, Step::Format]
        );
        assert_eq!(
            steps(&laid, &image)[..4],
            [Step::Check, Step::Partition, Step::Format, Step::Copy]
        );
        laid.storage.swap_mib = 1024;
        assert_eq!(
            steps(&laid, &image)[..5],
            [
                Step::Check,
                Step::Partition,
                Step::Format,
                Step::Swap,
                Step::Copy
            ]
        );
        // A layout that shrinks a kept filesystem shrinks it first (Q9);
        // one that only formats a partition at another size does not.
        use crate::model::layout::Fate;
        for case in crate::worker::manual::matrix::cases(512) {
            let mut config = laid.clone();
            config.target = Target::Layout {
                disks: case.layout.clone(),
            };
            config.storage.swap_mib = 0;
            let shrinks = case
                .layout
                .iter()
                .flat_map(|one| &one.parts)
                .any(|part| matches!(part.fate, Fate::Shrink { .. }));
            let mut wanted = vec![Step::Check];
            if shrinks {
                wanted.push(Step::Resize);
            }
            wanted.extend([Step::Partition, Step::Format, Step::Copy]);
            assert_eq!(
                steps(&config, &image)[..wanted.len()],
                wanted,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn snapshots_need_btrfs() {
        let image = fixture(&[Desktop::Linexin], &[]);
        let mut config = quiet();
        config.software.snapshots = true;
        assert!(!steps(&config, &image).contains(&Step::Snapshots));
        config.storage.filesystem = Filesystem::Btrfs;
        assert!(steps(&config, &image).contains(&Step::Snapshots));
    }

    #[test]
    fn packages_run_when_anything_is_there_to_remove() {
        let bare = fixture(&[Desktop::Linexin], &["gnome-maps"]);
        let mut config = quiet();
        assert!(!steps(&config, &bare).contains(&Step::Packages));
        // A deselected package the image does not have is nothing to remove.
        config.software.remove = vec!["not-in-the-image".into()];
        assert!(!steps(&config, &bare).contains(&Step::Packages));
        config.software.remove = vec!["gnome-maps".into()];
        assert!(steps(&config, &bare).contains(&Step::Packages));
        // The installer itself is always removed when the image has it.
        let with_installer = fixture(&[Desktop::Linexin], &["linexin-installer"]);
        assert!(steps(&quiet(), &with_installer).contains(&Step::Packages));
    }

    #[test]
    fn removals_keep_the_persons_order_then_the_installers_once_each() {
        let installed: BTreeSet<String> = ["gparted", "gnome-maps", "linexin-installer", "totem"]
            .iter()
            .map(|p| p.to_string())
            .collect();
        let mut config = quiet();
        config.software.remove = vec!["totem".into(), "gparted".into(), "gnome-maps".into()];
        assert_eq!(
            removals(&config, &installed),
            ["totem", "gparted", "gnome-maps", "linexin-installer"]
        );
    }

    #[test]
    fn the_worker_runs_from_the_copy_to_before_finish() {
        let image = fixture(&[Desktop::Linexin], &[]);
        let all = steps(&quiet(), &image);
        let mine = after_storage(&all);
        assert_eq!(mine.first(), Some(&Step::Copy));
        assert_eq!(mine.last(), Some(&Step::Bootloader));
        assert!(!mine.contains(&Step::Finish) && !mine.contains(&Step::Format));
    }
}
