//! Carrying out a layout made by hand on Partitions (`Target::Layout`): the
//! executor that ends in the same [`Mounted`] the easy path's storage half
//! builds, so Copy, System, Accounts, Desktop, Bootloader and Finish run
//! after it as they do after the easy path (§7.0, Unit Q, "The worker").
//!
//! **Reached only for what QEMU proved**: `worker::install` hands a layout
//! to [`install_storage`] once `check::validate` has taken it, which it
//! does only where all the layout needs lies within one set of
//! `layout::PROVEN` — a mix one QEMU scenario (Unit Q) ran whole and
//! passed — as the interface's `ready()` takes a plan only there; and
//! the executor itself refuses any layout whose needs no one set of
//! `PROVEN` holds (`layout::unproven`). What decides is pure — command
//! lines, scripts, what is read back and the mounts, built as data from
//! the layout as the person reviewed it — and what touches the machine
//! goes through one trait, so that every phase can be held to its order,
//! and every command to the consent, on a development machine:
//!
//! - [`carry`]: the executor — the Check step's look at the disks, then
//!   phases A to G — and Finish's fill-in of the fstab;
//! - [`machine`]: everything it asks of the machine ([`machine::Machine`];
//!   the computer's own, [`machine::Real`], refuses every test);
//! - [`plan`]: phases B and C as the exact commands, disk by disk in path
//!   order — filesystems shrunk (here only their place in the order: the
//!   commands are [`resize`]'s), entries shrunk by number found at
//!   their reviewed start, deletions in one call from the highest number, a
//!   new table or the partitions added to a kept one, entries grown,
//!   partitions retyped and renamed, the boot flag — and what the disks
//!   must say once they are written, which maps each partition to its path
//!   now;
//! - [`resize`]: a kept ext4 or NTFS made smaller — phase A's dry run and
//!   phase B.1's commands, word for word, and how each one's end is read
//!   (step Q8);
//! - [`shrinking`]: where they meet the machine (step Q9) — phase A's
//!   reading again of every filesystem shrunk, and of every Windows volume
//!   of every disk where an NTFS is, under its own allowlist, the rule
//!   across every disk applied again, the dry runs; phase B.1 and its read
//!   back;
//! - [`fence`]: what a destructive command may be given at all;
//! - [`mounts`]: the installed system's mounts, in the order they are made,
//!   and its fstab;
//! - [`mounted`] (below): the [`Mounted`] the rest of the worker reads,
//!   assembled from the layout and what was read back;
//! - `matrix` (tests only): every phase's order and every command's
//!   arguments for the fixture disks, and the consent held against what the
//!   commands destroy by an oracle written apart from the rule set;
//! - `side_files` (tests only, each `#[ignore]`d and run by hand by name):
//!   the real e2fsprogs and ntfsprogs carrying [`resize`]'s commands out on
//!   image files the test made, read by the measure entry's own commands.
//!
//! The udisks rule is `worker::quiet`'s.
pub mod carry;
pub mod fence;
pub mod machine;
pub mod mounts;
pub mod plan;
pub mod resize;
pub mod shrinking;

pub use carry::{finish, install_storage, prepare};

#[cfg(test)]
pub(crate) mod matrix;
#[cfg(test)]
mod side_files;

use super::layout::{Esp, EspMount, Firmware, LinuxBoot, Mounted};
use crate::model::config::{Filesystem, InstallConfig};
use crate::model::disks::{Partition, Table};
use crate::model::layout::{self, DiskLayout, Fate, Fs, MountPoint, Which};
use crate::model::protocol::Step;
use std::path::PathBuf;

/// One partition a layout uses — at a mount point or as swap — as it will
/// be once the layout is carried out.
#[derive(Clone, Debug)]
pub struct Used<'a> {
    /// The disk it is on, as the layout has it.
    pub disk: &'a DiskLayout,
    pub which: Which,
    pub at: &'a MountPoint,
    /// The filesystem made on it; `None` where what is on it is kept.
    pub made: Option<Fs>,
    /// The partition as reviewed, for one that is there now.
    pub was: Option<&'a Partition>,
    /// Where it starts, in bytes (it never moves).
    pub start: u64,
}

/// Every partition `layout` uses, disk by disk in path order, each disk's
/// by where they start.
pub fn used(layout: &[DiskLayout]) -> Vec<Used<'_>> {
    let mut disks: Vec<&DiskLayout> = layout.iter().collect();
    disks.sort_by(|one, other| one.disk.path.cmp(&other.disk.path));
    let mut list = Vec::new();
    for one in disks {
        let mut here = Vec::new();
        for (part, was) in one.parts.iter().zip(&one.disk.partitions) {
            let Some(at) = part.fate.mount() else {
                continue;
            };
            let made = match &part.fate {
                Fate::Format { fs, .. } => Some(*fs),
                _ => None,
            };
            here.push(Used {
                disk: one,
                which: Which::Part(part.path.clone()),
                at,
                made,
                was: Some(was),
                start: part.start,
            });
        }
        for create in &one.create {
            let Some(at) = &create.mount else {
                continue;
            };
            here.push(Used {
                disk: one,
                which: Which::Made {
                    disk: one.disk.path.clone(),
                    start: create.start,
                },
                at,
                made: Some(create.fs),
                was: None,
                start: create.start,
            });
        }
        here.sort_by_key(|one| one.start);
        list.extend(here);
    }
    list
}

/// Whether `layout` makes `fs` anywhere — on a partition it formats or one
/// it makes — so that the live system must have the tool that does
/// (`check::required_tools`).
pub fn makes(layout: &[DiskLayout], fs: Fs) -> bool {
    layout.iter().any(|one| {
        let formatted = one.parts.iter().any(|part| match &part.fate {
            Fate::Format { fs: made, .. } => *made == fs,
            _ => false,
        });
        formatted || one.create.iter().any(|made| made.fs == fs)
    })
}

/// The storage steps a layout made by hand reports, in order, for the
/// worker's `Event::Plan`: its kept filesystems shrunk first where it shrinks
/// any (Resize, Q9), its tables written (Partition), its filesystems made
/// and the target mounted (Format), and the swap file where it has one
/// (Swap) — the easy path's own steps for the rest, so the interface counts
/// them alike.
pub fn planned_steps(config: &InstallConfig) -> Vec<Step> {
    let shrinks = match &config.target {
        crate::model::config::Target::Layout { disks } => !layout::shrunk(disks).is_empty(),
        _ => false,
    };
    let mut steps = if shrinks {
        vec![Step::Resize]
    } else {
        Vec::new()
    };
    steps.extend([Step::Partition, Step::Format]);
    if config.storage.swap_mib > 0 {
        steps.push(Step::Swap);
    }
    steps
}

/// What the worker read of one partition it uses once everything is made
/// (`blkid -p`): its path now, its filesystem's UUID and type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fact {
    pub which: Which,
    pub device: String,
    pub uuid: String,
    pub fstype: String,
}

/// Everything read of the partitions a layout uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub found: Vec<Fact>,
}

impl Facts {
    /// What was read of `which`.
    pub fn of(&self, which: &Which) -> Option<&Fact> {
        self.found.iter().find(|fact| fact.which == *which)
    }
}

/// The swap file's path inside the installed system for a `/` made as
/// `filesystem`, as the easy path puts it: on Btrfs in an `@swap`
/// subvolume of its own, which snapshots of `@` never hold.
pub fn swapfile_path(filesystem: Filesystem) -> &'static str {
    match filesystem {
        Filesystem::Ext4 => "/swapfile",
        Filesystem::Btrfs => "/swap/swapfile",
    }
}

/// What `/` is made with, as the request's easy field says it.
pub fn root_filesystem(layout: &[DiskLayout]) -> Result<(Used<'_>, Filesystem), String> {
    let root = used(layout)
        .into_iter()
        .find(|one| *one.at == MountPoint::Root)
        .ok_or("nothing is mounted at /")?;
    let filesystem = match root.made {
        Some(Fs::Ext4) => Filesystem::Ext4,
        Some(Fs::Btrfs) => Filesystem::Btrfs,
        _ => return Err("/ is not made as ext4 or Btrfs".to_string()),
    };
    Ok((root, filesystem))
}

/// The partitions os-prober is to take for this installation's own
/// (`Mounted::other_devices`), at their paths now (`paths`, read back once
/// the tables are written): every partition the layout formats or makes —
/// swap, and one given no mount point, included, so whatever an old system
/// left there is never offered beside Linexin — and a partition it keeps at
/// `/home`, which holds the person's files rather than a system. Disks in
/// path order, each disk's by where they start, each once. A partition kept
/// at a path of one's own is not among them: a kept NTFS may well be
/// Windows, which os-prober is there to find.
pub fn own_devices(layout: &[DiskLayout], paths: &plan::Paths) -> Vec<String> {
    let mut disks: Vec<&DiskLayout> = layout.iter().collect();
    disks.sort_by(|one, other| one.disk.path.cmp(&other.disk.path));
    let mut devices: Vec<String> = Vec::new();
    for one in disks {
        let formatted = one.parts.iter().filter_map(|part| {
            let own = match &part.fate {
                Fate::Format { .. } => true,
                fate => fate.keeps() && fate.mount() == Some(&MountPoint::Home),
            };
            own.then(|| (part.start, Which::Part(part.path.clone())))
        });
        let made = one.create.iter().map(|made| {
            let which = Which::Made {
                disk: one.disk.path.clone(),
                start: made.start,
            };
            (made.start, which)
        });
        let mut here: Vec<(u64, Which)> = formatted.chain(made).collect();
        here.sort_by_key(|(start, _)| *start);
        for (_, which) in here {
            if let Some(path) = paths.of(&which) {
                if !devices.iter().any(|device| device == path) {
                    devices.push(path.to_string());
                }
            }
        }
    }
    devices
}

/// **The [`Mounted`] the rest of the worker reads**, assembled from the
/// layout and what was read once everything was made: the target mounted
/// at `root`, the swap file of `swap_mib` (none beside a swap partition, as
/// the pins have it) and the fstab written for it; `paths`, every
/// partition's path now (read back once the tables were written), names
/// what os-prober is to take for this installation's own
/// ([`own_devices`]), and the kernel is told to leave GPT auto-discovery
/// off (`gpt_auto_off`), which only a layout made by hand asks for.
///
/// `disk` and `table` are the EFI system partition's disk and that disk's
/// table on UEFI, `/`'s on BIOS: their only readers are the boot loader's
/// choice, GRUB's BIOS target and the firmware entry's `efibootmgr --disk`,
/// so an EFI system partition on another disk than `/`, systemd-boot's need
/// for a GPT on the disk it starts from, and GRUB's BIOS target are right
/// by construction (§7.0, Unit Q, "Boot over a hand-made layout" 1). The
/// EFI system partition is mounted at `/efi` beside a `/boot` for the
/// kernels (a Linux boot partition), or alone at `/boot`; kept, it is
/// shared, and never formatted.
pub fn mounted(
    layout: &[DiskLayout],
    firmware: Firmware,
    facts: &Facts,
    paths: &plan::Paths,
    root: PathBuf,
    swap_mib: u32,
    fstab: String,
) -> Result<Mounted, String> {
    let (root_use, filesystem) = root_filesystem(layout)?;
    let all = used(layout);
    let fact = |one: &Used| {
        facts
            .of(&one.which)
            .ok_or_else(|| format!("nothing was read of what is used at {}", one.at.path()))
    };
    let root_fact = fact(&root_use)?;
    let at = |mount: MountPoint| all.iter().find(|one| *one.at == mount);
    let swap_partition = all.iter().any(|one| *one.at == MountPoint::Swap);
    if swap_mib > 0 && swap_partition {
        return Err("a swap file beside a swap partition".to_string());
    }

    let (esp, linux_boot) = match firmware {
        Firmware::Bios => (None, None),
        Firmware::Uefi => {
            let (esp_use, mount) = match (at(MountPoint::Efi), at(MountPoint::Boot)) {
                (Some(efi), _) => (efi, EspMount::Efi),
                (None, Some(boot)) => (boot, EspMount::Boot),
                (None, None) => return Err("nothing is mounted at /efi or /boot".to_string()),
            };
            let esp_fact = fact(esp_use)?;
            let esp = Esp {
                device: esp_fact.device.clone(),
                uuid: esp_fact.uuid.clone(),
                mount,
                shared: esp_use.made.is_none(),
            };
            let linux_boot = match (mount, at(MountPoint::Boot)) {
                (EspMount::Efi, Some(boot)) => {
                    let boot_fact = fact(boot)?;
                    Some(LinuxBoot {
                        device: boot_fact.device.clone(),
                        uuid: boot_fact.uuid.clone(),
                    })
                }
                (EspMount::Efi, None) => {
                    return Err("an EFI system partition at /efi with no /boot".to_string())
                }
                (EspMount::Boot, _) => None,
            };
            (Some((esp, esp_use.disk)), linux_boot)
        }
    };
    let starts = match &esp {
        Some((_, disk)) => *disk,
        None => root_use.disk,
    };
    let table: Table = layout::table_after(starts)
        .ok_or_else(|| format!("{} is left with no partition table", starts.disk.path))?;
    Ok(Mounted {
        firmware,
        table,
        disk: starts.disk.path.clone(),
        root,
        root_device: root_fact.device.clone(),
        root_uuid: root_fact.uuid.clone(),
        filesystem,
        root_subvolume: (filesystem == Filesystem::Btrfs).then(|| "@".to_string()),
        esp: esp.map(|(esp, _)| esp),
        linux_boot,
        home_device: match at(MountPoint::Home) {
            Some(home) => Some(fact(home)?.device.clone()),
            None => None,
        },
        swapfile: (swap_mib > 0).then(|| PathBuf::from(swapfile_path(filesystem))),
        fstab,
        gpt_auto_off: true,
        other_devices: own_devices(layout, paths),
    })
}
