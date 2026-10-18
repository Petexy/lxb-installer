//! Every choice a person can make, planned and held to the rules a plan must
//! keep whatever was chosen.
//!
//! Each fixture disk, at 512 and 4096 byte sectors, under UEFI and BIOS, with
//! every place on it as the target (the whole disk, each listed gap, each
//! partition), both filesystems, with and without a swap file, every
//! separate /home (none; one on another disk, kept or formatted; each
//! partition of the fixture, kept or formatted), and a shared EFI system
//! partition with and without room for the kernels. A choice that cannot be
//! installed must be refused with the failure that says why; one that can
//! must give a plan that keeps every invariant in [`check`]; and a choice
//! the interface offers (`model::disks::validate_storage`) is never one the
//! planner refuses, whatever the worker finds inside a shared EFI partition.
use super::fixtures::{self, config, extent, on_disk, targets};
use super::plan::{
    is_extended, plan, Change, EspPlan, Plan, Role, SharedRoom, Span, ALIGN, BIOS_BOOT_SIZE,
    ESP_SIZE, LINUX_BOOT_SIZE, MBR_LIMIT, MIN_HOME, SWAP_SHARE,
};
use super::script::{script, table_commands};
use super::table::check_before_writing;
use crate::model::config::{Filesystem, Home, InstallConfig, Storage, Target};
use crate::model::disks::{
    validate_storage, Blocked, Content, Disk, Partition, Table, GIB, LINUX_BOOT, MIB, MIN_INSTALL,
};
use crate::model::protocol::Failure;
use crate::worker::layout::{Firmware, Stop};
use crate::worker::storage::format;

/// What the planner answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Planned,
    /// `InvalidRequest`: the choice is not one the installer offers.
    Refused,
    /// `NotEnoughSpace`: the place is too small for the system.
    TooSmall,
}

use Outcome::{Planned, Refused, TooSmall};

fn outcome(result: &Result<Plan, Stop>) -> Outcome {
    match result {
        Ok(_) => Planned,
        Err(stop) => match stop.failure {
            Failure::InvalidRequest => Refused,
            Failure::NotEnoughSpace => TooSmall,
            other => panic!("the planner stopped with {other:?}: {}", stop.detail),
        },
    }
}

/// The planner's answer for each fixture and target with no separate /home,
/// under UEFI and under BIOS. Sizes are in bytes in the fixtures, so the
/// answer is the same at both sector sizes; the filesystem and the swap file
/// change nothing either, as every root that fits is far bigger than twice
/// the swap file; nor does a shared EFI partition's room, as every place
/// beside one that fits also fits a Linux boot partition.
pub(super) const EXPECTED: &[(&str, &str, Outcome, Outcome)] = &[
    ("empty", "whole", Planned, Planned),
    ("gpt-windows", "whole", Planned, Planned),
    // UEFI shares Windows' EFI partition; BIOS adds a BIOS boot partition.
    ("gpt-windows", "gap1", Planned, Planned),
    // The EFI partition and the Microsoft reserved one are never replaced.
    ("gpt-windows", "p1", Refused, Refused),
    ("gpt-windows", "p2", Refused, Refused),
    // UEFI re-types Windows in place; BIOS carves a BIOS boot partition
    // out of it first.
    ("gpt-windows", "p3", Planned, Planned),
    // 750 MiB of recovery tools holds no system.
    ("gpt-windows", "p4", TooSmall, TooSmall),
    ("gpt-plain", "whole", Planned, Planned),
    ("gpt-plain", "gap1", Planned, Planned),
    ("gpt-plain", "p1", Planned, Planned),
    ("gpt-bios", "whole", Planned, Planned),
    ("gpt-bios", "gap1", Planned, Planned),
    ("gpt-bios", "p1", Refused, Refused),
    ("gpt-bios", "p2", Planned, Planned),
    ("msdos", "whole", Planned, Planned),
    ("msdos", "gap1", Planned, Planned),
    // 500 MiB: too small for a 2 GiB EFI partition, and for a system.
    ("msdos", "p1", TooSmall, TooSmall),
    ("msdos", "p2", Planned, Planned),
    ("msdos-esp", "whole", Planned, Planned),
    ("msdos-esp", "gap1", Planned, Planned),
    ("msdos-esp", "p1", Refused, Refused),
    ("msdos-esp", "p2", Planned, Planned),
    // All four primary slots are taken: the gap cannot take a partition,
    // and an EFI partition and a root need two slots where replacing one
    // partition frees one.
    ("msdos-full", "whole", Planned, Planned),
    ("msdos-full", "gap1", Refused, Refused),
    ("msdos-full", "p1", TooSmall, TooSmall),
    ("msdos-full", "p2", Refused, Planned),
    ("msdos-full", "p3", Refused, Planned),
    ("msdos-full", "p4", TooSmall, TooSmall),
    // Space beside an extended partition can only take a logical one, the
    // container itself is not a place, and splitting a logical partition
    // would renumber the ones after it.
    ("msdos-extended", "whole", Planned, Planned),
    ("msdos-extended", "gap1", Refused, Refused),
    ("msdos-extended", "p1", TooSmall, TooSmall),
    ("msdos-extended", "p2", Refused, Refused),
    ("msdos-extended", "p5", Refused, Planned),
];

fn expected(fixture: &str, target: &str, firmware: Firmware) -> Outcome {
    let (_, _, uefi, bios) = EXPECTED
        .iter()
        .find(|(f, t, _, _)| *f == fixture && *t == target)
        .unwrap_or_else(|| panic!("no expected outcome for {fixture} {target}"));
    match firmware {
        Firmware::Uefi => *uefi,
        Firmware::Bios => *bios,
    }
}

/// A partition on a disk that is not the target: the planner cannot see it,
/// so it accepts it, and the worker judges it on the machine.
const OTHER_DISK: &str = "/dev/vdy1";

/// Every separate /home a person could pick for an installation on `disk`.
fn homes(disk: &Disk) -> Vec<(String, Option<Home>)> {
    let mut list = vec![("none".to_string(), None)];
    let mut add = |name: String, path: &str| {
        for format in [false, true] {
            list.push((
                format!("{name} format={format}"),
                Some(Home {
                    path: path.to_string(),
                    format,
                    disk: None,
                }),
            ));
        }
    };
    add("other-disk".to_string(), OTHER_DISK);
    for part in &disk.partitions {
        add(format!("p{}", part.number), &part.path);
    }
    list
}

/// The rule for a separate /home: an existing partition, not on
/// the target, not an EFI, BIOS boot, Microsoft reserved, recovery or
/// extended partition, at least 1 GB, and kept only when it holds a Linux
/// filesystem (the new account takes ownership of its folder there).
fn home_allowed(disk: &Disk, target: &Target, home: &Home) -> bool {
    let Some(part) = disk.partition(&home.path) else {
        return true;
    };
    let erased = match target {
        Target::WholeDisk => true,
        Target::Partition { path } => *path == home.path,
        Target::FreeSpace { .. } => false,
        Target::Layout { .. } => unreachable!("the easy path's matrix holds no hand-made layout"),
    };
    let system = matches!(
        part.content,
        Content::Efi | Content::BiosBoot | Content::MicrosoftReserved | Content::WindowsRecovery
    ) || is_extended(&part.part_type);
    !erased && !system && part.size >= MIN_HOME && (home.format || part.content == Content::Linux)
}

/// The confirmation dialog's list, by the rule `model::disks::consent`
/// documents: the whole disk under its own name, or the replaced partition,
/// or nothing for free space; then a /home that is formatted. A reused EFI
/// partition and a kept /home are never in it.
fn documented_consent(config: &InstallConfig) -> Vec<String> {
    let mut paths = match &config.target {
        Target::WholeDisk => vec![config.disk.path.clone()],
        Target::Partition { path } => vec![path.clone()],
        Target::FreeSpace { .. } => Vec::new(),
        Target::Layout { .. } => unreachable!("the easy path's matrix holds no hand-made layout"),
    };
    if let Some(home) = config.storage.home.as_ref().filter(|h| h.format) {
        if !paths.contains(&home.path) {
            paths.push(home.path.clone());
        }
    }
    paths
}

/// What the plan itself erases or formats, in the same order.
fn erased_by(plan: &Plan) -> Vec<String> {
    let mut paths = match &plan.change {
        Change::Fresh => vec![plan.disk.clone()],
        Change::Add => Vec::new(),
        Change::Retype { path, .. } | Change::Replace { path, .. } => vec![path.clone()],
    };
    if let Some(home) = plan.home.as_ref().filter(|h| h.format) {
        if !paths.contains(&home.path) {
            paths.push(home.path.clone());
        }
    }
    paths
}

/// The sectors a partition may occupy, worked out from the tables' own
/// layout rather than the planner's: on GPT, past the protective MBR, the
/// header and its 128 entries of 128 bytes, and before their backup at the
/// end; on msdos, past the MBR and no further than an MBR's 32-bit count.
fn table_room(table: Table, total: u64, sector: u64) -> Span {
    match table {
        Table::Gpt => {
            let entries = (128 * 128u64).div_ceil(sector);
            Span {
                start: 2 + entries,
                end: total.saturating_sub(1 + entries),
            }
        }
        Table::Msdos => Span {
            start: 1,
            end: total.min(1 << 32),
        },
    }
}

/// Both things the worker can find inside a shared EFI system partition.
const ROOMS: [SharedRoom; 2] = [SharedRoom::Kernels, SharedRoom::LoaderOnly];

/// Every rule a plan keeps, whatever was chosen. `shared` is what the worker
/// found inside a shared EFI system partition.
fn check(case: &str, config: &InstallConfig, firmware: Firmware, shared: SharedRoom, plan: &Plan) {
    let disk = &config.disk;
    let sector = u64::from(disk.logical_sector);
    let total = disk.size / sector;
    assert_eq!(
        (plan.disk.as_str(), plan.size, plan.sector),
        (disk.path.as_str(), disk.size, sector),
        "{case}: the plan is for the disk as reviewed"
    );

    // The table: a fresh one by the firmware, otherwise the disk's own.
    let table = match (&config.target, firmware) {
        (Target::WholeDisk, Firmware::Uefi) => Table::Gpt,
        (Target::WholeDisk, Firmware::Bios) if total <= MBR_LIMIT => Table::Msdos,
        (Target::WholeDisk, Firmware::Bios) => Table::Gpt,
        _ => disk.table.expect("a plan into a kept table needs a table"),
    };
    assert_eq!(plan.table, table, "{case}: the table");

    // The kind of change follows from the target, and a replaced partition
    // is named exactly as the disk lists it.
    match (&config.target, &plan.change) {
        (Target::WholeDisk, Change::Fresh) | (Target::FreeSpace { .. }, Change::Add) => {}
        (
            Target::Partition { path },
            Change::Retype {
                number,
                path: named,
                span,
            }
            | Change::Replace {
                number,
                path: named,
                span,
            },
        ) => {
            let part = disk.partition(path).expect("the target is on the disk");
            assert_eq!(
                (named, *number, *span),
                (path, part.number, extent(disk, part, sector)),
                "{case}: the replaced partition"
            );
        }
        (target, change) => panic!("{case}: {target:?} was planned as {change:?}"),
    }
    let replaced = match &plan.change {
        Change::Retype { number, .. } | Change::Replace { number, .. } => Some(*number),
        Change::Fresh | Change::Add => None,
    };
    let kept: Vec<&Partition> = match plan.change {
        Change::Fresh => Vec::new(),
        _ => disk
            .partitions
            .iter()
            .filter(|p| Some(p.number) != replaced)
            .collect(),
    };

    // Every new partition starts and ends on a whole MiB, lies inside the
    // disk's usable sectors and the plan's space, in disk order, and
    // overlaps nothing that is kept.
    let usable = table_room(plan.table, total, sector);
    let mut end_of_last = 0;
    for part in &plan.create {
        let span = part.span();
        assert!(part.size > 0, "{case}: {part:?} is empty");
        assert!(
            (part.start * sector).is_multiple_of(ALIGN)
                && (part.size * sector).is_multiple_of(ALIGN),
            "{case}: {part:?} is not on whole MiB"
        );
        assert!(
            usable.contains(span) && span.end <= total,
            "{case}: {part:?} lies outside the usable {usable:?} of {total} sectors"
        );
        assert!(
            plan.space.contains(span),
            "{case}: {part:?} lies outside the space {:?}",
            plan.space
        );
        assert!(
            span.start >= end_of_last,
            "{case}: {part:?} overlaps the one before it or is out of order"
        );
        end_of_last = span.end;
        for other in &kept {
            assert!(
                !extent(disk, other, sector).overlaps(span),
                "{case}: {part:?} overlaps {}, which is kept",
                other.path
            );
        }
        if plan.table == Table::Msdos {
            assert!(span.end <= MBR_LIMIT, "{case}: {part:?} is past the MBR");
        }
    }
    for other in &kept {
        // A re-typed logical partition lies inside its extended container,
        // which is kept as it is; nothing else that is kept may share the
        // space the plan writes into.
        let container =
            matches!(plan.change, Change::Retype { .. }) && is_extended(&other.part_type);
        assert!(
            container || !extent(disk, other, sector).overlaps(plan.space),
            "{case}: the space {:?} overlaps {}, which is kept",
            plan.space,
            other.path
        );
    }

    // One root, after the boot partitions, taking the rest of the space;
    // a re-typed partition is the root and nothing is created.
    let count = |role: Role| plan.create.iter().filter(|p| p.role == role).count();
    match plan.change {
        Change::Retype { .. } => assert!(plan.create.is_empty(), "{case}: re-typing creates"),
        _ => {
            assert_eq!(count(Role::Root), 1, "{case}: one new root");
            let root = plan.new_root().unwrap();
            assert_eq!(plan.create.last(), Some(&root), "{case}: root comes last");
            assert_eq!(
                root.span().end,
                plan.space.end,
                "{case}: root takes the rest"
            );
            assert_eq!(
                plan.create.first().map(|p| p.start),
                Some(plan.space.start),
                "{case}: the space is used from its start"
            );
        }
    }
    assert!(
        plan.root_bytes() >= MIN_INSTALL,
        "{case}: a root of {} bytes",
        plan.root_bytes()
    );
    for part in &plan.create {
        match part.role {
            Role::Esp => assert_eq!(part.size * sector, ESP_SIZE, "{case}: ESP size"),
            Role::BiosBoot => assert_eq!(part.size * sector, BIOS_BOOT_SIZE, "{case}: BIOS boot"),
            Role::LinuxBoot => {
                assert_eq!(part.size * sector, LINUX_BOOT_SIZE, "{case}: Linux boot")
            }
            Role::Root => {}
        }
    }
    assert_eq!(
        LINUX_BOOT_SIZE, LINUX_BOOT,
        "the interface counts the Linux boot partition the planner makes"
    );

    // The EFI system partition: a disk's own is reused whenever it is
    // kept, and then never deleted, re-typed or formatted; at most one per
    // disk afterwards; none under BIOS.
    let kept_esps = kept.iter().filter(|p| p.content == Content::Efi).count();
    match (firmware, &plan.esp) {
        (Firmware::Uefi, EspPlan::New) => {
            assert_eq!(count(Role::Esp), 1, "{case}: the new ESP");
            assert_eq!(kept_esps, 0, "{case}: an ESP made beside the disk's own");
            assert_eq!(
                count(Role::LinuxBoot),
                0,
                "{case}: Linux boot beside a new ESP"
            );
        }
        (Firmware::Uefi, EspPlan::Shared { path }) => {
            assert_eq!(
                count(Role::Esp),
                0,
                "{case}: an ESP made beside the shared one"
            );
            let own = disk.esp().expect("a shared ESP is the disk's own");
            assert_eq!(*path, own.path, "{case}: the shared ESP");
            assert!(
                kept.iter().any(|p| p.path == *path),
                "{case}: the shared ESP is deleted or re-typed"
            );
            assert!(
                plan.home.as_ref().is_none_or(|h| h.path != *path),
                "{case}: the shared ESP is formatted as /home"
            );
            // The kernels get a Linux boot partition, the first thing in
            // the space, exactly when the shared ESP has no room for them;
            // then a replaced partition is re-created after it.
            let linux_boot = shared == SharedRoom::LoaderOnly;
            assert_eq!(
                count(Role::LinuxBoot),
                usize::from(linux_boot),
                "{case}: Linux boot partitions"
            );
            if linux_boot {
                assert_eq!(
                    plan.create.first().map(|p| p.role),
                    Some(Role::LinuxBoot),
                    "{case}: Linux boot first"
                );
                assert!(
                    !matches!(plan.change, Change::Retype { .. }),
                    "{case}: re-typed with a Linux boot partition to make"
                );
            }
            let formatted_home = plan.home.as_ref().is_some_and(|h| h.format);
            assert_eq!(
                format::units(plan),
                2 + u64::from(!plan.subvolumes.is_empty())
                    + u64::from(formatted_home)
                    + u64::from(linux_boot),
                "{case}: the format step has an ESP to make"
            );
        }
        (Firmware::Bios, EspPlan::None) => {
            assert_eq!(count(Role::Esp), 0, "{case}: an ESP under BIOS");
            assert_eq!(count(Role::LinuxBoot), 0, "{case}: Linux boot under BIOS");
        }
        (firmware, esp) => panic!("{case}: {esp:?} under {firmware:?}"),
    }
    assert!(
        kept_esps + count(Role::Esp) <= 1,
        "{case}: more than one ESP"
    );

    // BIOS: exactly one BIOS boot partition on GPT; on msdos none, and the
    // root carries the boot flag (in the same write on a fresh table).
    let kept_bios_boot = kept
        .iter()
        .filter(|p| p.content == Content::BiosBoot)
        .count();
    match (firmware, plan.table) {
        (Firmware::Bios, Table::Gpt) => assert_eq!(
            kept_bios_boot + count(Role::BiosBoot),
            1,
            "{case}: BIOS boot partitions"
        ),
        (Firmware::Bios, Table::Msdos) => {
            assert_eq!(count(Role::BiosBoot), 0, "{case}: BIOS boot on msdos");
            assert!(plan.wants_boot_flag(), "{case}: no boot flag wanted");
            if plan.change == Change::Fresh {
                let root = plan.new_root().unwrap();
                let text = script(plan).unwrap();
                let line = text
                    .lines()
                    .find(|l| l.starts_with(&format!("start={},", root.start)))
                    .unwrap();
                assert!(line.ends_with(", bootable"), "{case}: {line}");
            }
        }
        (Firmware::Uefi, _) => assert_eq!(count(Role::BiosBoot), 0, "{case}: BIOS boot"),
    }
    if plan.table == Table::Msdos {
        let primaries = kept.iter().filter(|p| (1..=4).contains(&p.number)).count();
        assert!(
            primaries + plan.create.len() <= 4,
            "{case}: more than four primary partitions"
        );
    }

    // The filesystem layout inside the root.
    let storage = &config.storage;
    assert_eq!(plan.filesystem, storage.filesystem, "{case}");
    assert_eq!(plan.home, storage.home, "{case}");
    match storage.swap_mib {
        0 => assert_eq!(plan.swap, None, "{case}"),
        mib => {
            let swap = plan.swap.expect("a swap file");
            let path = match storage.filesystem {
                Filesystem::Ext4 => "/swapfile",
                Filesystem::Btrfs => "/swap/swapfile",
            };
            assert_eq!((swap.path, swap.mib), (path, mib), "{case}");
            assert!(swap.bytes() <= plan.root_bytes() / SWAP_SHARE, "{case}");
        }
    }
    let subvolumes: Vec<&str> = plan.subvolumes.iter().map(|s| s.name).collect();
    let mut want = Vec::new();
    if storage.filesystem == Filesystem::Btrfs {
        want.push("@");
        if storage.home.is_none() {
            want.push("@home");
        }
        if storage.swap_mib > 0 {
            want.push("@swap");
        }
    }
    assert_eq!(subvolumes, want, "{case}: subvolumes");

    // What is erased is what the person consented to, and the layout
    // touches exactly the partitions that list names (all of them, under
    // the disk's own name, for a fresh table).
    assert_eq!(
        erased_by(plan),
        documented_consent(config),
        "{case}: consent"
    );
    let touched: Vec<&str> = disk
        .partitions
        .iter()
        .filter(|p| {
            plan.change == Change::Fresh
                || Some(p.number) == replaced
                || plan
                    .create
                    .iter()
                    .any(|n| n.span().overlaps(extent(disk, p, sector)))
        })
        .map(|p| p.path.as_str())
        .collect();
    let named: Vec<&str> = match &plan.change {
        Change::Fresh => disk.partitions.iter().map(|p| p.path.as_str()).collect(),
        Change::Add => Vec::new(),
        Change::Retype { path, .. } | Change::Replace { path, .. } => vec![path.as_str()],
    };
    assert_eq!(touched, named, "{case}: partitions the layout touches");

    // The commands touch this disk only, in the shape the change needs.
    let commands = table_commands(plan, &plan.disk);
    for command in &commands {
        assert_eq!(
            command.args.iter().filter(|a| **a == plan.disk).count(),
            1,
            "{case}: {command:?}"
        );
        assert!(
            command
                .args
                .iter()
                .all(|a| *a == plan.disk || !a.starts_with('/')),
            "{case}: {command:?} names another device"
        );
    }
    let programs: Vec<&str> = commands.iter().map(|c| c.program).collect();
    let input = script(plan);
    assert_eq!(input.is_some(), !plan.create.is_empty(), "{case}: script");
    if let Some(text) = &input {
        let lines = text.lines().filter(|l| l.starts_with("start=")).count();
        assert_eq!(lines, plan.create.len(), "{case}: {text}");
        assert!(text.contains(&format!("sector-size: {sector}\n")), "{case}");
        assert_eq!(
            text.contains("label:"),
            plan.change == Change::Fresh,
            "{case}"
        );
    }
    let has = |index: usize, arg: &str| commands[index].args.iter().any(|a| a == arg);
    match &plan.change {
        Change::Fresh => {
            assert_eq!(programs, ["wipefs", "sfdisk"], "{case}");
            let label = match plan.table {
                Table::Gpt => "label: gpt\n",
                Table::Msdos => "label: dos\n",
            };
            assert!(
                commands[1].input.as_deref().unwrap().starts_with(label),
                "{case}"
            );
        }
        Change::Add => {
            assert_eq!(programs, ["sfdisk"], "{case}");
            assert!(has(0, "--append"), "{case}");
        }
        Change::Retype { number, .. } => {
            let want: &[&str] = match plan.table {
                Table::Gpt => &["sfdisk", "sfdisk"],
                Table::Msdos => &["sfdisk"],
            };
            assert_eq!(programs, want, "{case}");
            assert!(
                has(0, "--part-type") && has(0, &number.to_string()),
                "{case}"
            );
        }
        Change::Replace { number, .. } => {
            assert_eq!(programs, ["sfdisk", "sfdisk"], "{case}");
            assert_eq!(
                commands[0].args,
                [
                    "--lock",
                    "--delete",
                    plan.disk.as_str(),
                    number.to_string().as_str()
                ],
                "{case}"
            );
            assert!(has(1, "--append"), "{case}");
        }
    }

    // The disk exactly as reviewed passes the check made before writing.
    if plan.change != Change::Fresh {
        check_before_writing(plan, &on_disk(disk)).unwrap_or_else(|e| panic!("{case}: {e}"));
    }
}

#[test]
fn every_choice_is_planned_within_the_rules_or_refused() {
    let mut used = vec![false; EXPECTED.len()];
    let (mut planned, mut refused, mut kept_homes, mut formatted_homes) = (0, 0, 0, 0);
    let (mut offered, mut linux_boots) = (0, 0);
    for sector in [512, 4096] {
        for (fixture, disk) in fixtures::all(sector) {
            for (target_name, target) in targets(&disk) {
                let row = EXPECTED
                    .iter()
                    .position(|(f, t, _, _)| *f == fixture && *t == target_name);
                used[row.unwrap_or_else(|| {
                    panic!("no expected outcome for {fixture} {target_name}")
                })] = true;
                for firmware in [Firmware::Uefi, Firmware::Bios] {
                    let placed = expected(fixture, &target_name, firmware);
                    for filesystem in [Filesystem::Ext4, Filesystem::Btrfs] {
                        for swap_mib in [0, 4096] {
                            for ((home_name, home), shared) in homes(&disk)
                                .into_iter()
                                .flat_map(|home| ROOMS.map(|room| (home.clone(), room)))
                            {
                                let case = format!(
                                    "{fixture} {sector}B {firmware:?} {target_name} \
                                     {filesystem:?} swap={swap_mib} home={home_name} {shared:?}"
                                );
                                let mut config = config(disk.clone(), target.clone());
                                config.storage = Storage {
                                    filesystem,
                                    swap_mib,
                                    home: home.clone(),
                                };
                                let result = plan(&config, firmware, shared);
                                // Where the system goes is judged before the
                                // /home, so a place that fails fails first.
                                let home_ok = home
                                    .as_ref()
                                    .is_none_or(|h| home_allowed(&disk, &target, h));
                                let want = match placed {
                                    Planned if !home_ok => Refused,
                                    other => other,
                                };
                                let detail = result.as_ref().err().map(|s| s.detail.clone());
                                assert_eq!(outcome(&result), want, "{case}: {detail:?}");
                                // What the interface offers, the worker
                                // installs, whatever it finds inside a
                                // shared EFI partition.
                                if validate_storage(&config, firmware).is_ok() {
                                    assert_eq!(
                                        outcome(&result),
                                        Planned,
                                        "{case}: offered but refused: {detail:?}"
                                    );
                                    offered += 1;
                                }
                                let Ok(plan) = result else {
                                    refused += 1;
                                    continue;
                                };
                                check(&case, &config, firmware, shared, &plan);
                                planned += 1;
                                linux_boots += usize::from(plan.created(Role::LinuxBoot).is_some());
                                if let Some(home) =
                                    home.filter(|h| disk.partition(&h.path).is_some())
                                {
                                    if home.format {
                                        formatted_homes += 1;
                                    } else {
                                        kept_homes += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        used.iter().all(|u| *u),
        "an expected outcome matched no target"
    );
    // The matrix must reach every kind of answer, including a /home on the
    // target disk that is kept and one that is formatted.
    assert!(
        planned > 1000 && refused > 1000,
        "{planned} planned, {refused} refused"
    );
    assert!(kept_homes > 0 && formatted_homes > 0);
    assert!(
        offered > 100 && linux_boots > 100,
        "{offered} {linux_boots}"
    );
}

/// A GPT disk of `size` bytes with these partitions and free gaps (MiB).
fn sized(sector: u32, size: u64, specs: Vec<fixtures::Spec>, free: Vec<(u64, u64)>) -> Disk {
    let mut disk = fixtures::build("/dev/vdx", sector, Some(Table::Gpt), specs, free);
    disk.size = size;
    disk
}

fn linux(number: u32, start_mib: u64, size_mib: u64) -> fixtures::Spec {
    fixtures::Spec {
        number,
        start: start_mib * MIB,
        size: size_mib * MIB,
        part_type: fixtures::GPT_LINUX,
        content: Content::Linux,
        fs_type: "ext4",
    }
}

fn esp(number: u32, start_mib: u64, size_mib: u64) -> fixtures::Spec {
    fixtures::Spec {
        number,
        start: start_mib * MIB,
        size: size_mib * MIB,
        part_type: fixtures::GPT_ESP,
        content: Content::Efi,
        fs_type: "vfat",
    }
}

/// The smallest root, in whole MiB.
fn min_root_mib() -> u64 {
    MIN_INSTALL.div_ceil(MIB)
}

/// Plans `config` for each size `n` around `threshold` (and a few far
/// below it), with `shared` found inside a shared EFI partition: below it
/// the answer is `NotEnoughSpace`, from it on a plan that keeps every rule.
/// Never a layout squeezed into too little room. Whatever the interface
/// offers at any size is planned with either room.
fn sweep(
    name: &str,
    threshold: u64,
    firmware: Firmware,
    shared: SharedRoom,
    make: impl Fn(u64) -> InstallConfig,
) {
    let sizes = [1, 2, 100, 2049, threshold / 2]
        .into_iter()
        .chain(threshold - 3..=threshold + 3);
    for n in sizes {
        let config = make(n);
        let case = format!("{name} {firmware:?} {shared:?} n={n} MiB");
        if validate_storage(&config, firmware).is_ok() {
            for room in ROOMS {
                assert!(
                    plan(&config, firmware, room).is_ok(),
                    "{case}: offered, but refused with {room:?}"
                );
            }
        }
        match plan(&config, firmware, shared) {
            Ok(plan) => {
                assert!(
                    n >= threshold,
                    "{case}: planned below the threshold {threshold}"
                );
                check(&case, &config, firmware, shared, &plan);
            }
            Err(stop) => {
                assert!(
                    n < threshold,
                    "{case}: refused at the threshold: {}",
                    stop.detail
                );
                assert_eq!(
                    stop.failure,
                    Failure::NotEnoughSpace,
                    "{case}: {}",
                    stop.detail
                );
            }
        }
    }
}

#[test]
fn space_that_cannot_hold_the_system_is_refused_not_squeezed() {
    use SharedRoom::{Kernels, LoaderOnly};
    let esp_mib = ESP_SIZE / MIB;
    let bios_boot_mib = BIOS_BOOT_SIZE / MIB;
    let linux_boot_mib = LINUX_BOOT_SIZE / MIB;
    for sector in [512, 4096] {
        // A gap of n MiB after a small Linux partition, the last MiB of the
        // disk left to the backup GPT.
        let gap = |n: u64| {
            let disk = sized(
                sector,
                (1 + 1024 + n + 1) * MIB,
                vec![linux(1, 1, 1024)],
                vec![(1025, n)],
            );
            let start = disk.free[0].start;
            let size = disk.free[0].size;
            config(disk, Target::FreeSpace { start, size })
        };
        let root = min_root_mib();
        sweep("gap", esp_mib + root, Firmware::Uefi, Kernels, gap);
        sweep("gap", bios_boot_mib + root, Firmware::Bios, Kernels, gap);

        // The same gap beside a 100 MiB ESP: nothing more is made while the
        // kernels fit in it, a Linux boot partition when they do not.
        let gap_beside = |n: u64| {
            let disk = sized(
                sector,
                (1 + 100 + n + 1) * MIB,
                vec![esp(1, 1, 100)],
                vec![(101, n)],
            );
            let start = disk.free[0].start;
            let size = disk.free[0].size;
            config(disk, Target::FreeSpace { start, size })
        };
        sweep("gap beside", root, Firmware::Uefi, Kernels, gap_beside);
        sweep(
            "gap beside",
            linux_boot_mib + root,
            Firmware::Uefi,
            LoaderOnly,
            gap_beside,
        );

        // The whole disk: GPT keeps its first and last MiB, msdos only its
        // first.
        let whole = |n: u64| {
            config(
                sized(sector, n * MIB, Vec::new(), Vec::new()),
                Target::WholeDisk,
            )
        };
        sweep("whole", 2 + esp_mib + root, Firmware::Uefi, Kernels, whole);
        sweep("whole", 1 + root, Firmware::Bios, Kernels, whole);

        // A partition of n MiB on a disk with no ESP: replaced by an ESP and
        // a root under UEFI, by a BIOS boot partition and a root under BIOS.
        let alone = |n: u64| {
            let disk = sized(sector, (n + 2) * MIB, vec![linux(1, 1, n)], Vec::new());
            config(
                disk,
                Target::Partition {
                    path: "/dev/vdx1".into(),
                },
            )
        };
        sweep("replace", esp_mib + root, Firmware::Uefi, Kernels, alone);
        sweep(
            "replace",
            bios_boot_mib + root,
            Firmware::Bios,
            Kernels,
            alone,
        );

        // Beside an ESP with room for the kernels the partition is only
        // re-typed under UEFI; beside one without, a Linux boot partition
        // is carved out of it.
        let beside = |n: u64| {
            let disk = sized(
                sector,
                (101 + n + 1) * MIB,
                vec![esp(1, 1, 100), linux(2, 101, n)],
                Vec::new(),
            );
            config(
                disk,
                Target::Partition {
                    path: "/dev/vdx2".into(),
                },
            )
        };
        sweep("retype", root, Firmware::Uefi, Kernels, beside);
        sweep(
            "carve",
            linux_boot_mib + root,
            Firmware::Uefi,
            LoaderOnly,
            beside,
        );
    }
}

#[test]
fn whole_disk_past_what_an_mbr_addresses_is_gpt_with_a_bios_boot_partition() {
    // 3 TiB: more sectors than an MBR counts at 512 bytes, fewer at 4096.
    for (sector, table) in [(512, Table::Gpt), (4096, Table::Msdos)] {
        let mut disk = fixtures::disk_empty(sector);
        disk.size = 3 * 1024 * GIB;
        let config = config(disk, Target::WholeDisk);
        let plan = fixtures::plan_for(&config, Firmware::Bios);
        assert_eq!(plan.table, table, "{sector}");
        let roles: Vec<Role> = plan.create.iter().map(|p| p.role).collect();
        let want: &[Role] = match table {
            Table::Gpt => &[Role::BiosBoot, Role::Root],
            Table::Msdos => &[Role::Root],
        };
        assert_eq!(roles, want, "{sector}");
        check(
            &format!("3 TiB {sector}B BIOS whole"),
            &config,
            Firmware::Bios,
            SharedRoom::Kernels,
            &plan,
        );
    }
}

#[test]
fn a_swap_file_larger_than_half_the_root_is_refused() {
    for sector in [512, 4096] {
        let mut config = config(
            fixtures::disk_gpt_windows(sector),
            Target::Partition {
                path: "/dev/vdx3".into(),
            },
        );
        config.storage.swap_mib = 0;
        let root = fixtures::plan_for(&config, Firmware::Uefi).root_bytes();
        let half = u32::try_from(root / SWAP_SHARE / MIB).unwrap();
        config.storage.swap_mib = half;
        let fits = fixtures::plan_for(&config, Firmware::Uefi);
        check(
            "half the root",
            &config,
            Firmware::Uefi,
            SharedRoom::Kernels,
            &fits,
        );
        for too_big in [half + 1, u32::MAX] {
            config.storage.swap_mib = too_big;
            let stop = plan(&config, Firmware::Uefi, SharedRoom::Kernels).unwrap_err();
            assert_eq!(stop.failure, Failure::InvalidRequest, "{too_big}");
        }
    }
}

#[test]
fn requests_that_do_not_match_the_disk_are_refused() {
    let refused = |config: &InstallConfig, why: &str| {
        for firmware in [Firmware::Uefi, Firmware::Bios] {
            for shared in ROOMS {
                let result = plan(config, firmware, shared);
                assert_eq!(
                    outcome(&result),
                    Refused,
                    "{why} under {firmware:?} {shared:?}"
                );
            }
        }
    };
    let gap = fixtures::disk_gpt_windows(512).free[0];
    let base = || config(fixtures::disk_gpt_windows(512), Target::WholeDisk);

    let mut c = base();
    c.target = Target::FreeSpace {
        start: gap.start + MIB,
        size: gap.size - MIB,
    };
    refused(&c, "a gap the disk did not list");

    let mut c = base();
    c.target = Target::Partition {
        path: "/dev/vdy3".into(),
    };
    refused(&c, "a partition of another disk");

    // A partition that does not start on a whole 4096 byte sector cannot
    // be replaced, nor can space be judged free beside it.
    let mut c = base();
    c.disk.logical_sector = 4096;
    c.disk.partitions[3].start += 512;
    c.target = Target::Partition {
        path: "/dev/vdx4".into(),
    };
    refused(&c, "a partition off the disk's sectors");
    c.target = Target::FreeSpace {
        start: gap.start,
        size: gap.size,
    };
    refused(&c, "a gap beside a partition off the disk's sectors");

    let mut c = base();
    c.disk.free[0].start -= 2 * MIB;
    c.disk.free[0].size += 2 * MIB;
    let listed = c.disk.free[0];
    c.target = Target::FreeSpace {
        start: listed.start,
        size: listed.size,
    };
    refused(&c, "a listed gap that overlaps a partition");

    for (start, size) in [(64 * GIB - MIB, 2 * MIB), (u64::MAX - MIB, 2 * MIB)] {
        let mut c = base();
        c.disk.free.push(crate::model::disks::Gap { start, size });
        c.target = Target::FreeSpace { start, size };
        refused(
            &c,
            &format!("a listed gap at {start}+{size} that is not on the disk"),
        );
    }

    for sector in [0, 520, 8192] {
        let mut c = base();
        c.disk.logical_sector = sector;
        refused(&c, &format!("a logical sector of {sector} bytes"));
    }

    let mut c = base();
    c.disk.blocked = Some(Blocked::InstallationMedia);
    refused(&c, "a blocked disk");
    let mut c = base();
    c.disk.read_only = true;
    refused(&c, "a read-only disk");

    for target in [
        Target::FreeSpace {
            start: MIB,
            size: 30 * GIB,
        },
        Target::Partition {
            path: "/dev/vdx1".into(),
        },
    ] {
        let mut disk = fixtures::disk_empty(512);
        disk.free = vec![crate::model::disks::Gap {
            start: MIB,
            size: 30 * GIB,
        }];
        refused(&config(disk, target), "a place on a disk with no table");
    }

    for path in ["/dev/vdx", "/home/someone", "vdy1"] {
        let mut c = base();
        c.storage.home = Some(Home {
            path: path.into(),
            format: true,
            disk: None,
        });
        refused(&c, &format!("/home on {path}"));
    }
}

#[test]
fn a_home_on_another_disk_is_judged_on_the_disk_named_for_it() {
    // Linexin erases the whole of /dev/vdx; /home is the one partition of
    // /dev/vdy, carried in the request as it was reviewed.
    let other = fixtures::build(
        "/dev/vdy",
        512,
        Some(Table::Gpt),
        vec![linux(1, 1, 10 * 1024)],
        Vec::new(),
    );
    let with_home = |path: &str, format: bool, disk: Disk| {
        let mut c = config(fixtures::disk_empty(512), Target::WholeDisk);
        c.storage.filesystem = Filesystem::Btrfs;
        c.storage.home = Some(Home {
            path: path.into(),
            format,
            disk: Some(disk),
        });
        c
    };
    for format in [false, true] {
        let c = with_home("/dev/vdy1", format, other.clone());
        assert_eq!(
            validate_storage(&c, Firmware::Uefi),
            Ok(()),
            "format={format}"
        );
        let planned = plan(&c, Firmware::Uefi, SharedRoom::Kernels).expect("planned");
        assert_eq!(planned.home, c.storage.home, "format={format}");
        let names: Vec<&str> = planned.subvolumes.iter().map(|s| s.name).collect();
        assert_eq!(names, ["@", "@swap"], "no @home beside a separate /home");
        assert_eq!(erased_by(&planned), documented_consent(&c));
    }

    let refused = |c: &InstallConfig, why: &str| {
        let result = plan(c, Firmware::Uefi, SharedRoom::Kernels);
        assert_eq!(outcome(&result), Refused, "{why}");
        assert!(validate_storage(c, Firmware::Uefi).is_err(), "{why}");
    };
    // The disk named for it is the target itself under another name.
    let mut target_again = fixtures::disk_empty(512);
    target_again.partitions = other.partitions.clone();
    refused(
        &with_home("/dev/vdy1", false, target_again),
        "a /home disk with the target's own path",
    );
    // The partition is not on the disk named for it.
    refused(
        &with_home("/dev/vdy9", true, other.clone()),
        "a /home not on its disk",
    );
    // Kept, it must still be a Linux file system.
    let mut windows = other.clone();
    windows.partitions[0].content = Content::Windows;
    windows.partitions[0].fs_type = "ntfs".into();
    refused(
        &with_home("/dev/vdy1", false, windows),
        "a kept NTFS /home on another disk",
    );
}
