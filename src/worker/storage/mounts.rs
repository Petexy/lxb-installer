//! The installed system's mounts, worked out once and used twice: to mount
//! the target while installing, and to write its fstab. Pure, so both are
//! tested here; the worker only fills in the devices' UUIDs.
use crate::model::config::Filesystem;
use crate::worker::layout::EspMount;
use crate::worker::partition::{Plan, Role};

/// Btrfs mount options for every subvolume, as the GTK installer used them.
pub const BTRFS_OPTIONS: &str = "compress=zstd,noatime";
/// The EFI system partition is readable by root only: it holds the boot
/// loader's random seed, which must not be world-readable. A Linux boot
/// partition beside it is mounted the same way (systemd-boot keeps its
/// seed and entries wherever `/boot` is).
pub const ESP_OPTIONS: &str = "umask=0077";
/// fstab's word for "no options".
pub const DEFAULTS: &str = "defaults";

/// Which device a mount comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Root,
    Esp,
    /// The Linux boot partition beside a shared EFI system partition.
    Boot,
    Home,
}

/// The filesystem a mount has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ext4,
    Btrfs,
    Vfat,
    /// A kept /home: whatever Linux filesystem it already has, read from
    /// the device before anything is written.
    Kept,
}

/// One mount of the installed system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    pub source: Source,
    /// Absolute inside the installed system.
    pub at: &'static str,
    pub kind: Kind,
    pub options: String,
    /// fsck's pass: 1 for an ext4 root, 2 for the others it checks, 0 for
    /// Btrfs (whose fsck does nothing at boot).
    pub pass: u8,
}

impl Mount {
    /// The options `mount -o` takes, or `None` for the defaults.
    pub fn mount_options(&self) -> Option<&str> {
        (self.options != DEFAULTS).then_some(self.options.as_str())
    }
}

/// The mounts in the order they are made: a parent always before what is
/// mounted inside it, so the root's own, then a Linux boot partition at
/// `/boot`, then the EFI system partition at `/boot` or `/efi`.
pub fn table(plan: &Plan, esp: Option<EspMount>) -> Vec<Mount> {
    let mut mounts = Vec::new();
    match plan.filesystem {
        Filesystem::Ext4 => mounts.push(Mount {
            source: Source::Root,
            at: "/",
            kind: Kind::Ext4,
            options: DEFAULTS.into(),
            pass: 1,
        }),
        Filesystem::Btrfs => {
            for subvolume in &plan.subvolumes {
                mounts.push(Mount {
                    source: Source::Root,
                    at: subvolume.at,
                    kind: Kind::Btrfs,
                    options: format!("subvol={},{BTRFS_OPTIONS}", subvolume.name),
                    pass: 0,
                });
            }
        }
    }
    if let Some(home) = &plan.home {
        // Straight after the root, where @home would otherwise have been.
        mounts.insert(
            1,
            Mount {
                source: Source::Home,
                at: "/home",
                kind: if home.format { Kind::Ext4 } else { Kind::Kept },
                options: DEFAULTS.into(),
                pass: 2,
            },
        );
    }
    if plan.created(Role::LinuxBoot).is_some() {
        mounts.push(Mount {
            source: Source::Boot,
            at: "/boot",
            kind: Kind::Vfat,
            options: ESP_OPTIONS.into(),
            pass: 2,
        });
    }
    if let Some(esp) = esp {
        mounts.push(Mount {
            source: Source::Esp,
            at: esp.path(),
            kind: Kind::Vfat,
            options: ESP_OPTIONS.into(),
            pass: 2,
        });
    }
    mounts
}

/// What the worker learned about the devices once they were formatted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub root: String,
    pub root_uuid: String,
    pub esp: Option<String>,
    pub esp_uuid: Option<String>,
    /// A Linux boot partition the plan made.
    pub boot: Option<String>,
    pub boot_uuid: Option<String>,
    pub home: Option<String>,
    pub home_uuid: Option<String>,
    /// A kept /home's filesystem type (`ext4`, `btrfs`, `xfs`…).
    pub home_fstype: Option<String>,
}

impl Facts {
    /// The device a mount comes from.
    pub fn device(&self, source: Source) -> Option<&str> {
        match source {
            Source::Root => Some(&self.root),
            Source::Esp => self.esp.as_deref(),
            Source::Boot => self.boot.as_deref(),
            Source::Home => self.home.as_deref(),
        }
    }

    fn uuid(&self, source: Source) -> Option<&str> {
        match source {
            Source::Root => Some(&self.root_uuid),
            Source::Esp => self.esp_uuid.as_deref(),
            Source::Boot => self.boot_uuid.as_deref(),
            Source::Home => self.home_uuid.as_deref(),
        }
    }

    /// The filesystem type a mount has, as `mount -t` and fstab take it.
    pub fn fstype(&self, kind: Kind) -> Option<&str> {
        match kind {
            Kind::Ext4 => Some("ext4"),
            Kind::Btrfs => Some("btrfs"),
            Kind::Vfat => Some("vfat"),
            Kind::Kept => self.home_fstype.as_deref(),
        }
    }
}

/// One fstab line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub uuid: String,
    pub at: String,
    pub fstype: String,
    pub options: String,
    pub pass: u8,
}

/// A UUID as blkid prints it: hex digits and dashes (`3f2a…-…`, or a FAT
/// volume's `ABCD-1234`). Anything else would let output from a tool break
/// the fstab's columns.
pub fn valid_uuid(uuid: &str) -> bool {
    !uuid.is_empty()
        && uuid.len() <= 64
        && uuid.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-')
        && uuid.bytes().any(|c| c.is_ascii_hexdigit())
}

/// A filesystem type word safe for an fstab column.
pub(crate) fn valid_fstype(fstype: &str) -> bool {
    !fstype.is_empty()
        && fstype.len() <= 32
        && fstype
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

/// The fstab lines for `mounts`, with each device's UUID filled in.
pub fn lines(mounts: &[Mount], facts: &Facts) -> Result<Vec<Line>, String> {
    let mut lines = Vec::new();
    for mount in mounts {
        let uuid = facts
            .uuid(mount.source)
            .filter(|u| valid_uuid(u))
            .ok_or_else(|| format!("no usable UUID for {}", mount.at))?;
        let fstype = facts
            .fstype(mount.kind)
            .filter(|t| valid_fstype(t))
            .ok_or_else(|| format!("no usable filesystem type for {}", mount.at))?;
        // A kept /home that happens to be Btrfs is not given to fsck.
        let pass = if fstype == "btrfs" { 0 } else { mount.pass };
        lines.push(Line {
            uuid: uuid.to_string(),
            at: mount.at.to_string(),
            fstype: fstype.to_string(),
            options: mount.options.clone(),
            pass,
        });
    }
    Ok(lines)
}

/// The installed system's `/etc/fstab`: every filesystem by UUID, so it does
/// not depend on the order disks are found in, then the swap file.
pub fn fstab(lines: &[Line], swapfile: Option<&str>) -> String {
    let mut text = String::from(
        "# Static information about the filesystems, written by lxb-installer.\n\
         # <file system>\t<dir>\t<type>\t<options>\t<dump> <pass>\n\n",
    );
    for line in lines {
        text.push_str(&format!(
            "UUID={}\t{}\t{}\t{}\t0 {}\n",
            line.uuid, line.at, line.fstype, line.options, line.pass
        ));
    }
    if let Some(path) = swapfile {
        text.push_str(&format!("{path}\tnone\tswap\tdefaults\t0 0\n"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::Home;
    use crate::model::config::Target;
    use crate::worker::layout::Firmware;
    use crate::worker::partition::fixtures::{config, disk_gpt_windows, plan_for, plan_in};
    use crate::worker::partition::{SharedRoom, Swapfile};

    fn btrfs_plan(home: Option<Home>, swap_mib: u32) -> Plan {
        let mut config = config(disk_gpt_windows(512), Target::WholeDisk);
        config.storage.filesystem = Filesystem::Btrfs;
        config.storage.home = home;
        config.storage.swap_mib = swap_mib;
        plan_for(&config, Firmware::Uefi)
    }

    fn facts() -> Facts {
        Facts {
            root: "/dev/vdx2".into(),
            root_uuid: "0b1d6f6e-1111-4c2e-9a77-000000000002".into(),
            esp: Some("/dev/vdx1".into()),
            esp_uuid: Some("ABCD-1234".into()),
            boot: Some("/dev/vdx5".into()),
            boot_uuid: Some("5E7A-0B11".into()),
            home: Some("/dev/vdy1".into()),
            home_uuid: Some("0b1d6f6e-1111-4c2e-9a77-000000000003".into()),
            home_fstype: Some("xfs".into()),
        }
    }

    #[test]
    fn btrfs_mounts_every_subvolume_with_compression_and_the_esp_last() {
        let plan = btrfs_plan(None, 4096);
        let mounts = table(&plan, Some(EspMount::Boot));
        let summary: Vec<_> = mounts.iter().map(|m| (m.at, m.options.as_str())).collect();
        assert_eq!(
            summary,
            [
                ("/", "subvol=@,compress=zstd,noatime"),
                ("/home", "subvol=@home,compress=zstd,noatime"),
                ("/swap", "subvol=@swap,compress=zstd,noatime"),
                ("/boot", "umask=0077"),
            ]
        );
        assert!(mounts[..3]
            .iter()
            .all(|m| m.kind == Kind::Btrfs && m.pass == 0));
    }

    #[test]
    fn a_separate_home_replaces_the_home_subvolume() {
        let home = Home {
            path: "/dev/vdy1".into(),
            format: false,
            disk: None,
        };
        let plan = btrfs_plan(Some(home), 0);
        let mounts = table(&plan, Some(EspMount::Efi));
        let summary: Vec<_> = mounts.iter().map(|m| (m.at, m.source, m.kind)).collect();
        assert_eq!(
            summary,
            [
                ("/", Source::Root, Kind::Btrfs),
                ("/home", Source::Home, Kind::Kept),
                ("/efi", Source::Esp, Kind::Vfat),
            ]
        );
    }

    #[test]
    fn ext4_is_one_root_with_a_checked_pass() {
        let mut config = config(disk_gpt_windows(4096), Target::WholeDisk);
        config.storage.home = Some(Home {
            path: "/dev/vdy1".into(),
            format: true,
            disk: None,
        });
        let plan = plan_for(&config, Firmware::Bios);
        let mounts = table(&plan, None);
        assert_eq!(mounts.len(), 2);
        assert_eq!((mounts[0].at, mounts[0].pass), ("/", 1));
        assert_eq!(mounts[0].mount_options(), None);
        assert_eq!((mounts[1].at, mounts[1].kind), ("/home", Kind::Ext4));
    }

    #[test]
    fn fstab_is_written_by_uuid_in_the_legacy_format() {
        let plan = btrfs_plan(None, 4096);
        let mounts = table(&plan, Some(EspMount::Boot));
        let text = fstab(
            &lines(&mounts, &facts()).unwrap(),
            plan.swap.map(|s: Swapfile| s.path),
        );
        let entries: Vec<&str> = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();
        assert_eq!(
            entries,
            [
                "UUID=0b1d6f6e-1111-4c2e-9a77-000000000002\t/\tbtrfs\tsubvol=@,compress=zstd,noatime\t0 0",
                "UUID=0b1d6f6e-1111-4c2e-9a77-000000000002\t/home\tbtrfs\tsubvol=@home,compress=zstd,noatime\t0 0",
                "UUID=0b1d6f6e-1111-4c2e-9a77-000000000002\t/swap\tbtrfs\tsubvol=@swap,compress=zstd,noatime\t0 0",
                "UUID=ABCD-1234\t/boot\tvfat\tumask=0077\t0 2",
                "/swap/swapfile\tnone\tswap\tdefaults\t0 0",
            ]
        );
    }

    #[test]
    fn ext4_root_and_kept_home_lines() {
        let mut config = config(disk_gpt_windows(512), Target::WholeDisk);
        config.storage.home = Some(Home {
            path: "/dev/vdy1".into(),
            format: false,
            disk: None,
        });
        let plan = plan_for(&config, Firmware::Uefi);
        let mounts = table(&plan, Some(EspMount::Boot));
        let text = fstab(&lines(&mounts, &facts()).unwrap(), Some("/swapfile"));
        assert!(
            text.contains("UUID=0b1d6f6e-1111-4c2e-9a77-000000000002\t/\text4\tdefaults\t0 1\n")
        );
        assert!(
            text.contains("UUID=0b1d6f6e-1111-4c2e-9a77-000000000003\t/home\txfs\tdefaults\t0 2\n")
        );
        assert!(text.ends_with("/swapfile\tnone\tswap\tdefaults\t0 0\n"));
    }

    /// The Windows fixture's gap, beside its 100 MiB EFI partition.
    fn beside_windows(filesystem: Filesystem, shared: SharedRoom) -> Plan {
        let disk = disk_gpt_windows(512);
        let gap = disk.free[0];
        let mut config = config(
            disk,
            Target::FreeSpace {
                start: gap.start,
                size: gap.size,
            },
        );
        config.storage.filesystem = filesystem;
        plan_in(&config, Firmware::Uefi, shared)
    }

    #[test]
    fn a_linux_boot_partition_is_mounted_after_the_root_and_before_the_esp() {
        let plan = beside_windows(Filesystem::Btrfs, SharedRoom::LoaderOnly);
        let mounts = table(&plan, Some(SharedRoom::LoaderOnly.esp_mount()));
        let summary: Vec<_> = mounts
            .iter()
            .map(|m| (m.at, m.source, m.kind, m.options.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                (
                    "/",
                    Source::Root,
                    Kind::Btrfs,
                    "subvol=@,compress=zstd,noatime"
                ),
                (
                    "/home",
                    Source::Root,
                    Kind::Btrfs,
                    "subvol=@home,compress=zstd,noatime"
                ),
                (
                    "/swap",
                    Source::Root,
                    Kind::Btrfs,
                    "subvol=@swap,compress=zstd,noatime"
                ),
                ("/boot", Source::Boot, Kind::Vfat, "umask=0077"),
                ("/efi", Source::Esp, Kind::Vfat, "umask=0077"),
            ]
        );
        // With room for the kernels in the shared partition there is none.
        let roomy = beside_windows(Filesystem::Ext4, SharedRoom::Kernels);
        let mounts = table(&roomy, Some(SharedRoom::Kernels.esp_mount()));
        let summary: Vec<_> = mounts.iter().map(|m| (m.at, m.source)).collect();
        assert_eq!(summary, [("/", Source::Root), ("/boot", Source::Esp)]);
    }

    #[test]
    fn fstab_names_the_linux_boot_partition_and_the_esp_by_uuid() {
        let plan = beside_windows(Filesystem::Ext4, SharedRoom::LoaderOnly);
        let mounts = table(&plan, Some(EspMount::Efi));
        let text = fstab(&lines(&mounts, &facts()).unwrap(), Some("/swapfile"));
        let entries: Vec<&str> = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();
        assert_eq!(
            entries,
            [
                "UUID=0b1d6f6e-1111-4c2e-9a77-000000000002\t/\text4\tdefaults\t0 1",
                "UUID=5E7A-0B11\t/boot\tvfat\tumask=0077\t0 2",
                "UUID=ABCD-1234\t/efi\tvfat\tumask=0077\t0 2",
                "/swapfile\tnone\tswap\tdefaults\t0 0",
            ]
        );
        let mut missing = facts();
        missing.boot_uuid = None;
        assert!(lines(&mounts, &missing).is_err());
    }

    #[test]
    fn a_strange_uuid_or_type_never_reaches_the_fstab() {
        let plan = btrfs_plan(None, 0);
        let mounts = table(&plan, Some(EspMount::Boot));
        let mut bad = facts();
        bad.root_uuid = "x y\n/ ext4".into();
        assert!(lines(&mounts, &bad).is_err());
        let mut missing = facts();
        missing.esp_uuid = None;
        assert!(lines(&mounts, &missing).is_err());
        assert!(valid_uuid("ABCD-1234"));
        assert!(!valid_uuid("----"));
        assert!(!valid_uuid(""));
    }
}
