//! The installed system's mounts for a hand-made layout, worked out once
//! and used twice — to mount the target while installing (phase E) and to
//! write its fstab (phase G) — and the directories they are made on (§7.0,
//! Unit Q, "fstab and mount points"). The easy path's `storage::mounts` is
//! left as it is; its fstab writer, its line and its UUID and type checks
//! are reused.
//!
//! Every filesystem is named by its UUID. The order is the order they are
//! mounted in, a parent always before what is mounted inside it: `/` first,
//! then by how deep the path is, then bytewise; the swap partitions after
//! the mounts, and the swap file — the easy path's, never beside a swap
//! partition — last.
//!
//! | Line | Type | Options | Pass |
//! |---|---|---|---|
//! | `/` ext4 | ext4 | `defaults` | 1 |
//! | `/` Btrfs (`@`, and `@home` and `@swap` as the easy path makes them) | btrfs | `subvol=@,compress=zstd,noatime` | 0 |
//! | `/boot`, `/efi` | vfat | `umask=0077` | 2 |
//! | ext4 formatted elsewhere | ext4 | `defaults` | 2 |
//! | xfs formatted elsewhere | xfs | `defaults` | 0 (`fsck.xfs` does nothing) |
//! | Btrfs formatted elsewhere | btrfs | `compress=zstd,noatime` | 0 |
//! | `/home` kept | its own | `defaults` | 2, 0 for Btrfs, xfs and f2fs |
//! | a Linux filesystem kept at a path of one's own | its own | `defaults,nofail,x-systemd.device-timeout=10s` | 2, or 0 |
//! | FAT or exFAT at a path of one's own, kept or formatted | vfat, exfat | `nofail,x-systemd.device-timeout=10s,uid=…,gid=…,umask=0022` | 0 |
//! | NTFS kept at a path of one's own | ntfs3 | `nofail,x-systemd.device-timeout=10s,uid=…,gid=…,windows_names` | 0 |
//! | each swap partition | swap (at `none`) | `defaults` | 0 |
//!
//! A partition kept at a path of one's own is **never mounted while
//! Linexin is installed** — only its directory is made — so the copy, the
//! new account's `chown -R` and the live account's removal cannot reach
//! it, and it is written to only where it is shrunk. A shrunk NTFS is left
//! marked for Windows to check, which `ntfs3` refuses until Windows has
//! run; `nofail` keeps the start going meanwhile. FAT, exFAT and NTFS are
//! the new account's to write: their lines carry [`UID`] and [`GID`] until
//! Finish fills in the account's numbers ([`fill_ids`]) from the target's
//! own `/etc/passwd` ([`ids`]).
use super::{root_filesystem, swapfile_path, used, Facts, Used};
use crate::model::config::Filesystem;
use crate::model::layout::{
    self, DiskLayout, Fs, MountPoint, Which, KEPT_HOME_FILESYSTEMS, PATH_MOST,
};
use crate::model::protocol::Failure;
use crate::worker::layout::Stop;
use crate::worker::storage::mounts::{
    valid_fstype, valid_uuid, Line, BTRFS_OPTIONS, DEFAULTS, ESP_OPTIONS,
};
use crate::worker::target;
use std::fs::{self, OpenOptions};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Where the new account's user number goes in an fstab line until Finish
/// knows it.
pub const UID: &str = "@uid@";
/// Where its group number goes.
pub const GID: &str = "@gid@";

/// What every partition kept at a path of one's own, and every FAT at one,
/// is mounted with: a start that does not wait for it, nor fail without it.
const NOFAIL: &str = "nofail,x-systemd.device-timeout=10s";

/// One mount of the installed system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    /// The partition it comes from.
    pub which: Which,
    /// Absolute inside the installed system.
    pub at: String,
    /// What `blkid -p` must find on it (`ntfs`, `vfat`, `ext4`…).
    pub found: String,
    /// The type `mount -t` and the fstab take (`ntfs3` for NTFS).
    pub fstype: String,
    /// The fstab's options, with [`UID`] and [`GID`] where the account's
    /// numbers go.
    pub options: String,
    /// fsck's pass.
    pub pass: u8,
    /// Whether it is mounted while Linexin is installed: never for a
    /// partition kept at a path of one's own, whose directory alone is made.
    pub during: bool,
}

impl Mount {
    /// The options it is mounted with while Linexin is installed, or `None`
    /// for the defaults: the fstab's, less what only the installed system's
    /// start reads (`nofail`, `x-systemd.…`) and the owner, which Finish
    /// fills in.
    pub fn run_options(&self) -> Option<String> {
        let kept: Vec<&str> = self
            .options
            .split(',')
            .filter(|option| {
                !(*option == DEFAULTS
                    || *option == "nofail"
                    || option.starts_with("x-systemd.")
                    || option.starts_with("uid=")
                    || option.starts_with("gid="))
            })
            .collect();
        (!kept.is_empty()).then(|| kept.join(","))
    }
}

/// How deep `path` is: `/` none, `/data` one, `/data/games` two.
fn depth(path: &str) -> usize {
    path.split('/').filter(|part| !part.is_empty()).count()
}

/// fsck's pass for a Linux filesystem kept as it is: ext2, ext3 and ext4
/// are checked; Btrfs, xfs and f2fs have nothing for it to do at the start.
fn kept_pass(fs: &str) -> u8 {
    if matches!(fs, "ext2" | "ext3" | "ext4") {
        2
    } else {
        0
    }
}

/// The options a FAT or exFAT partition at a path of one's own is mounted
/// with: the new account's to write.
fn owned_fat() -> String {
    format!("{NOFAIL},uid={UID},gid={GID},umask=0022")
}

/// The mount of one partition a layout uses at a mount point that is not
/// `/` (which [`table`] makes itself) or swap (which is not mounted).
fn mount_of(one: &Used) -> Result<Mount, String> {
    let at = one.at.path().to_string();
    let boots = matches!(one.at, MountPoint::Efi | MountPoint::Boot);
    let line = |found: &str, fstype: &str, options: String, pass: u8, during: bool| Mount {
        which: one.which.clone(),
        at: at.clone(),
        found: found.to_string(),
        fstype: fstype.to_string(),
        options,
        pass,
        during,
    };
    let wrong = || format!("nothing can be mounted at {at} as the layout has it");
    match (one.made, one.was) {
        (Some(Fs::Fat32), _) if boots => Ok(line("vfat", "vfat", ESP_OPTIONS.into(), 2, true)),
        (Some(_), _) if boots => Err(wrong()),
        (Some(Fs::Fat32), _) => Ok(line("vfat", "vfat", owned_fat(), 0, true)),
        (Some(Fs::Ext4), _) => Ok(line("ext4", "ext4", DEFAULTS.into(), 2, true)),
        (Some(Fs::Xfs), _) => Ok(line("xfs", "xfs", DEFAULTS.into(), 0, true)),
        (Some(Fs::Btrfs), _) => Ok(line("btrfs", "btrfs", BTRFS_OPTIONS.into(), 0, true)),
        (Some(Fs::LinuxSwap | Fs::BiosGrub), _) => Err(wrong()),
        (None, Some(was)) => {
            let fs = was.fs_type.as_str();
            if !layout::mountable(fs) {
                return Err(format!(
                    "{} holds {fs:?}, which cannot be mounted",
                    was.path
                ));
            }
            match one.at {
                MountPoint::Efi | MountPoint::Boot if fs == "vfat" => {
                    Ok(line("vfat", "vfat", ESP_OPTIONS.into(), 2, true))
                }
                MountPoint::Home if KEPT_HOME_FILESYSTEMS.contains(&fs) => {
                    Ok(line(fs, fs, DEFAULTS.into(), kept_pass(fs), true))
                }
                MountPoint::Other(_) => Ok(match fs {
                    "vfat" | "exfat" => line(fs, fs, owned_fat(), 0, false),
                    "ntfs" => line(
                        "ntfs",
                        "ntfs3",
                        format!("{NOFAIL},uid={UID},gid={GID},windows_names"),
                        0,
                        false,
                    ),
                    linux => line(
                        linux,
                        linux,
                        format!("{DEFAULTS},{NOFAIL}"),
                        kept_pass(linux),
                        false,
                    ),
                }),
                _ => Err(wrong()),
            }
        }
        (None, None) => Err(wrong()),
    }
}

/// The Btrfs subvolumes a layout's `/` gets, with where each is mounted,
/// by the easy path's rule: `@` at `/`; `@home` at `/home` unless a
/// partition is used there; `@swap` at `/swap` beside a swap file of
/// `swap_mib` (none for 0). Made in this order by `format::subvolumes`
/// (phase D), and mounted by [`table`].
pub fn subvolumes(layout: &[DiskLayout], swap_mib: u32) -> Vec<(&'static str, &'static str)> {
    let separate_home = used(layout).iter().any(|one| *one.at == MountPoint::Home);
    let mut subvolumes = vec![("@", "/")];
    if !separate_home {
        subvolumes.push(("@home", "/home"));
    }
    if swap_mib > 0 {
        subvolumes.push(("@swap", "/swap"));
    }
    subvolumes
}

/// **The installed system's mounts**, in the order they are made (see the
/// module), for `layout` with a swap file of `swap_mib` (none beside a swap
/// partition): `/` — on Btrfs its `@`, an `@home` where no partition is
/// used at `/home`, and an `@swap` for the swap file, as the easy path lays
/// one out — then everything else used at a mount point.
pub fn table(layout: &[DiskLayout], swap_mib: u32) -> Result<Vec<Mount>, String> {
    let (root, filesystem) = root_filesystem(layout)?;
    let all = used(layout);
    let mut mounts = Vec::new();
    let mut root_at = |at: &str, found: &str, options: String, pass: u8| {
        mounts.push(Mount {
            which: root.which.clone(),
            at: at.to_string(),
            found: found.to_string(),
            fstype: found.to_string(),
            options,
            pass,
            during: true,
        })
    };
    match filesystem {
        Filesystem::Ext4 => root_at("/", "ext4", DEFAULTS.into(), 1),
        Filesystem::Btrfs => {
            for (name, at) in subvolumes(layout, swap_mib) {
                root_at(at, "btrfs", format!("subvol={name},{BTRFS_OPTIONS}"), 0);
            }
        }
    }
    for one in &all {
        if !matches!(one.at, MountPoint::Root | MountPoint::Swap) {
            mounts.push(mount_of(one)?);
        }
    }
    mounts.sort_by(|one, other| {
        let key = |mount: &Mount| (mount.at != "/", depth(&mount.at), mount.at.clone());
        key(one).cmp(&key(other))
    });
    Ok(mounts)
}

/// The swap partitions `layout` uses, disk by disk in path order: formatted
/// only, never switched on while Linexin is installed.
pub fn swaps(layout: &[DiskLayout]) -> Vec<Which> {
    used(layout)
        .into_iter()
        .filter(|one| *one.at == MountPoint::Swap)
        .map(|one| one.which)
        .collect()
}

/// The swap file's path inside the installed system, for a swap file of
/// `swap_mib` (none for 0) beside `layout`'s `/`; never beside a swap
/// partition.
pub fn swapfile(layout: &[DiskLayout], swap_mib: u32) -> Result<Option<&'static str>, String> {
    if swap_mib == 0 {
        return Ok(None);
    }
    if !swaps(layout).is_empty() {
        return Err("a swap file beside a swap partition".to_string());
    }
    let (_, filesystem) = root_filesystem(layout)?;
    Ok(Some(swapfile_path(filesystem)))
}

/// Whether `at` can stand in an fstab's column as it is: a whole path in
/// its normal form, at most `PATH_MOST` bytes, with no whitespace, control
/// character or backslash — refused rather than escaped, by the rule set
/// first and here again.
fn path_ok(at: &str) -> bool {
    layout::normal_path(at).as_deref() == Some(at)
        && at.len() <= PATH_MOST
        && !at
            .chars()
            .any(|letter| letter.is_whitespace() || letter.is_control() || letter == '\\')
}

/// **The fstab's lines** for `mounts` and the swap partitions `swaps`, from
/// what was read of each partition once everything was made (`facts`):
/// each by its UUID, which must be one `blkid` could print; each holding
/// the filesystem it must (what was made, or what a kept partition was
/// reviewed with); each at a path that needs no escape.
pub fn lines(mounts: &[Mount], swaps: &[Which], facts: &Facts) -> Result<Vec<Line>, String> {
    let mut lines = Vec::new();
    let fact_of = |which: &Which, at: &str, found: &str| {
        let fact = facts
            .of(which)
            .ok_or_else(|| format!("nothing was read of what is used at {at}"))?;
        if !valid_uuid(&fact.uuid) {
            return Err(format!("no usable UUID for {at}"));
        }
        if fact.fstype != found {
            return Err(format!(
                "{} holds {:?}, not {found}, at {at}",
                fact.device, fact.fstype
            ));
        }
        Ok(fact)
    };
    for mount in mounts {
        if !path_ok(&mount.at) {
            return Err(format!("{:?} cannot stand in the fstab", mount.at));
        }
        if !valid_fstype(&mount.fstype) {
            return Err(format!("no usable filesystem type for {}", mount.at));
        }
        let fact = fact_of(&mount.which, &mount.at, &mount.found)?;
        lines.push(Line {
            uuid: fact.uuid.clone(),
            at: mount.at.clone(),
            fstype: mount.fstype.clone(),
            options: mount.options.clone(),
            pass: mount.pass,
        });
    }
    for which in swaps {
        let fact = fact_of(which, "swap", "swap")?;
        lines.push(Line {
            uuid: fact.uuid.clone(),
            at: "none".to_string(),
            fstype: "swap".to_string(),
            options: DEFAULTS.to_string(),
            pass: 0,
        });
    }
    Ok(lines)
}

/// The installed system's `/etc/fstab` text: the easy path's writer, with
/// the swap file last.
pub fn fstab(lines: &[Line], swapfile: Option<&str>) -> String {
    crate::worker::storage::mounts::fstab(lines, swapfile)
}

/// The user and group numbers of `user` in the text of an `/etc/passwd`.
pub fn ids(passwd: &str, user: &str) -> Option<(u32, u32)> {
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != user {
            return None;
        }
        let _password = fields.next()?;
        let uid = fields.next()?.parse().ok()?;
        let gid = fields.next()?.parse().ok()?;
        Some((uid, gid))
    })
}

/// The fstab text with the new account's numbers where [`UID`] and [`GID`]
/// stand, which Finish writes once the account is made.
pub fn fill_ids(fstab: &str, uid: u32, gid: u32) -> String {
    fstab
        .replace(&format!("uid={UID}"), &format!("uid={uid}"))
        .replace(&format!("gid={GID}"), &format!("gid={gid}"))
}

fn internal(detail: impl std::fmt::Display) -> Stop {
    Stop::new(Failure::Internal, detail.to_string())
}

/// Makes `at`'s directory inside the target at `root`, and every missing
/// one above it, each with mode 0755 — given outright, since the worker's
/// own umask is 077 — and each through `target::path`, so never through a
/// symlink out of the target. A directory already there is left as it is;
/// anything else there is refused. (`target::create_dir`, the easy path's,
/// is left as it is.)
pub fn mount_dir(root: &Path, at: &str) -> Result<(), Stop> {
    let relative = at.trim_start_matches('/');
    if relative.is_empty() {
        return Ok(());
    }
    let mut prefix = String::new();
    for part in relative.split('/') {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        let path = target::path(root, &prefix)?;
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                return Err(internal(format!(
                    "{} is not a directory to mount on",
                    path.display()
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&path).map_err(internal)?;
                let made = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
                    .open(&path)
                    .map_err(internal)?;
                made.set_permissions(fs::Permissions::from_mode(0o755))
                    .map_err(internal)?;
            }
            Err(error) => return Err(internal(error)),
        }
    }
    Ok(())
}

/// The mode a mount point is given once its filesystem is mounted on it:
/// a `/tmp` of its own everyone's to write in, as `/tmp` is (1777).
pub fn mode_after_mount(at: &str) -> Option<u32> {
    (at == "/tmp").then_some(0o1777)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::{Content, Disk, Table, GIB, MIB};
    use crate::model::layout::{Create, Fate, Part};
    use crate::worker::manual::Fact;
    use crate::worker::partition::fixtures::{build, Spec, GPT_ESP, GPT_LINUX};

    fn spec(
        number: u32,
        start_gib: u64,
        size_gib: u64,
        content: Content,
        fs: &'static str,
    ) -> Spec {
        Spec {
            number,
            start: start_gib * GIB + MIB,
            size: size_gib * GIB,
            part_type: if content == Content::Efi {
                GPT_ESP
            } else {
                GPT_LINUX
            },
            content,
            fs_type: fs,
        }
    }

    fn laid(disk: Disk, fates: Vec<Fate>, create: Vec<Create>) -> DiskLayout {
        DiskLayout {
            parts: disk
                .partitions
                .iter()
                .zip(fates)
                .map(|(was, fate)| Part {
                    path: was.path.clone(),
                    number: was.number,
                    start: was.start,
                    size: was.size,
                    fate,
                })
                .collect(),
            disk,
            table: None,
            create,
        }
    }

    fn at(path: &str) -> Option<MountPoint> {
        Some(MountPoint::at(path))
    }

    fn keep(path: &str) -> Fate {
        Fate::Keep { mount: at(path) }
    }

    fn format(fs: Fs, path: &str) -> Fate {
        Fate::Format {
            fs,
            label: String::new(),
            size: GIB,
            mount: at(path),
        }
    }

    fn made(start_gib: u64, fs: Fs, mount: Option<MountPoint>) -> Create {
        Create {
            start: (40 + start_gib) * GIB,
            size: GIB,
            fs,
            label: String::new(),
            mount,
        }
    }

    /// Every kind of line beside an ext4 `/`: a kept EFI partition at
    /// `/efi` with a Linux boot partition made at `/boot`, a kept ext4
    /// `/home`, kept Linux filesystems, FAT, exFAT and NTFS at paths of
    /// one's own, xfs, ext4, Btrfs and FAT formatted or made at others (one
    /// inside another), and two swap partitions.
    fn every_kind() -> Vec<DiskLayout> {
        let disk = build(
            "/dev/vdx",
            512,
            Some(Table::Gpt),
            vec![
                spec(1, 0, 1, Content::Efi, "vfat"),
                spec(2, 1, 1, Content::Linux, "ext4"),
                spec(3, 2, 1, Content::Linux, "ext4"),
                spec(4, 3, 1, Content::Linux, "btrfs"),
                spec(5, 4, 1, Content::Data, "vfat"),
                spec(6, 5, 1, Content::Data, "exfat"),
                spec(7, 6, 1, Content::Windows, "ntfs"),
                spec(8, 7, 1, Content::Linux, "ext4"),
                spec(9, 8, 1, Content::Swap, "swap"),
            ],
            Vec::new(),
        );
        let fates = vec![
            keep("/efi"),
            keep("/home"),
            keep("/old"),
            keep("/store"),
            keep("/usb"),
            keep("/share"),
            keep("/windows"),
            format(Fs::Xfs, "/srv"),
            format(Fs::LinuxSwap, "swap"),
        ];
        let create = vec![
            made(0, Fs::Fat32, at("/boot")),
            made(1, Fs::Ext4, at("/")),
            made(2, Fs::Ext4, at("/data")),
            made(3, Fs::Btrfs, at("/data/games")),
            made(4, Fs::Fat32, at("/camera")),
            made(5, Fs::LinuxSwap, at("swap")),
        ];
        vec![laid(disk, fates, create)]
    }

    /// What blkid reads of each partition used: a made-up UUID by where it
    /// is, and the filesystem it must hold.
    fn facts(layout: &[DiskLayout]) -> Facts {
        let mut found = Vec::new();
        for (index, one) in used(layout).iter().enumerate() {
            let fstype = match (one.made, one.was) {
                (Some(fs), _) => fs.lsblk().to_string(),
                (None, Some(was)) => was.fs_type.clone(),
                (None, None) => unreachable!(),
            };
            found.push(Fact {
                which: one.which.clone(),
                device: format!("/dev/vdx{}", 20 + index),
                uuid: format!("0000000{index:x}-1111-2222-3333-444455556666"),
                fstype,
            });
        }
        Facts { found }
    }

    fn entries(text: &str) -> Vec<&str> {
        text.lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    }

    #[test]
    fn every_kind_of_line_in_the_order_the_mounts_are_made() {
        let layout = every_kind();
        let mounts = table(&layout, 0).unwrap();
        let order: Vec<(&str, bool)> = mounts.iter().map(|m| (m.at.as_str(), m.during)).collect();
        assert_eq!(
            order,
            [
                ("/", true),
                ("/boot", true),
                ("/camera", true),
                ("/data", true),
                ("/efi", true),
                ("/home", true),
                ("/old", false),
                ("/share", false),
                ("/srv", true),
                ("/store", false),
                ("/usb", false),
                ("/windows", false),
                ("/data/games", true),
            ],
            "/ first, then by depth, then bytewise; kept paths of one's own never mounted"
        );
        let facts = facts(&layout);
        let text = fstab(
            &lines(&mounts, &swaps(&layout), &facts).unwrap(),
            swapfile(&layout, 0).unwrap(),
        );
        let uuid = |index: usize| format!("0000000{index:x}-1111-2222-3333-444455556666");
        // `used` numbers them by disk order and start: p1…p9, then made.
        let owned = "nofail,x-systemd.device-timeout=10s,uid=@uid@,gid=@gid@";
        assert_eq!(
            entries(&text),
            [
                format!("UUID={}\t/\text4\tdefaults\t0 1", uuid(10)),
                format!("UUID={}\t/boot\tvfat\tumask=0077\t0 2", uuid(9)),
                format!("UUID={}\t/camera\tvfat\t{owned},umask=0022\t0 0", uuid(13)),
                format!("UUID={}\t/data\text4\tdefaults\t0 2", uuid(11)),
                format!("UUID={}\t/efi\tvfat\tumask=0077\t0 2", uuid(0)),
                format!("UUID={}\t/home\text4\tdefaults\t0 2", uuid(1)),
                format!(
                    "UUID={}\t/old\text4\tdefaults,nofail,x-systemd.device-timeout=10s\t0 2",
                    uuid(2)
                ),
                format!("UUID={}\t/share\texfat\t{owned},umask=0022\t0 0", uuid(5)),
                format!("UUID={}\t/srv\txfs\tdefaults\t0 0", uuid(7)),
                format!(
                    "UUID={}\t/store\tbtrfs\tdefaults,nofail,x-systemd.device-timeout=10s\t0 0",
                    uuid(3)
                ),
                format!("UUID={}\t/usb\tvfat\t{owned},umask=0022\t0 0", uuid(4)),
                format!(
                    "UUID={}\t/windows\tntfs3\t{owned},windows_names\t0 0",
                    uuid(6)
                ),
                format!(
                    "UUID={}\t/data/games\tbtrfs\tcompress=zstd,noatime\t0 0",
                    uuid(12)
                ),
                format!("UUID={}\tnone\tswap\tdefaults\t0 0", uuid(8)),
                format!("UUID={}\tnone\tswap\tdefaults\t0 0", uuid(14)),
            ]
        );
        // Mounted while installing with what the installation needs only.
        let run: Vec<(&str, Option<String>)> = mounts
            .iter()
            .filter(|m| m.during)
            .map(|m| (m.at.as_str(), m.run_options()))
            .collect();
        assert_eq!(
            run,
            [
                ("/", None),
                ("/boot", Some("umask=0077".into())),
                ("/camera", Some("umask=0022".into())),
                ("/data", None),
                ("/efi", Some("umask=0077".into())),
                ("/home", None),
                ("/srv", None),
                ("/data/games", Some("compress=zstd,noatime".into())),
            ]
        );
    }

    #[test]
    fn a_btrfs_root_has_the_easy_paths_subvolumes_and_the_swap_file_last() {
        let disk = build(
            "/dev/vdx",
            4096,
            Some(Table::Gpt),
            vec![
                spec(1, 0, 1, Content::Efi, "vfat"),
                spec(2, 1, 30, Content::Linux, "xfs"),
            ],
            Vec::new(),
        );
        // A kept xfs /home: no @home, and fsck leaves it alone.
        let with_home = vec![laid(
            disk.clone(),
            vec![keep("/boot"), keep("/home")],
            vec![made(0, Fs::Btrfs, at("/"))],
        )];
        let mounts = table(&with_home, 2048).unwrap();
        let facts = facts(&with_home);
        let text = fstab(
            &lines(&mounts, &swaps(&with_home), &facts).unwrap(),
            swapfile(&with_home, 2048).unwrap(),
        );
        let uuid = |index: usize| format!("0000000{index:x}-1111-2222-3333-444455556666");
        assert_eq!(
            entries(&text),
            [
                format!(
                    "UUID={}\t/\tbtrfs\tsubvol=@,compress=zstd,noatime\t0 0",
                    uuid(2)
                ),
                format!("UUID={}\t/boot\tvfat\tumask=0077\t0 2", uuid(0)),
                format!("UUID={}\t/home\txfs\tdefaults\t0 0", uuid(1)),
                format!(
                    "UUID={}\t/swap\tbtrfs\tsubvol=@swap,compress=zstd,noatime\t0 0",
                    uuid(2)
                ),
                "/swap/swapfile\tnone\tswap\tdefaults\t0 0".to_string(),
            ]
        );
        // With no /home of its own, @home; with no swap file, no @swap.
        let alone = vec![laid(
            disk,
            vec![keep("/boot"), Fate::Delete],
            vec![made(0, Fs::Btrfs, at("/"))],
        )];
        let ats: Vec<String> = table(&alone, 0)
            .unwrap()
            .into_iter()
            .map(|m| m.at)
            .collect();
        assert_eq!(ats, ["/", "/boot", "/home"]);
        assert_eq!(swapfile(&alone, 0), Ok(None));
        // An ext4 / keeps its swap file at the top.
        let ext4 = vec![laid(
            build(
                "/dev/vdx",
                512,
                Some(Table::Gpt),
                vec![spec(1, 0, 1, Content::Efi, "vfat")],
                Vec::new(),
            ),
            vec![keep("/boot")],
            vec![made(0, Fs::Ext4, at("/"))],
        )];
        assert_eq!(swapfile(&ext4, 1024), Ok(Some("/swapfile")));
        // Never beside a swap partition.
        let mut both = ext4.clone();
        both[0].create.push(made(1, Fs::LinuxSwap, at("swap")));
        assert!(swapfile(&both, 1024).is_err());
    }

    #[test]
    fn nothing_but_what_was_made_or_kept_reaches_the_fstab() {
        let layout = every_kind();
        let mounts = table(&layout, 0).unwrap();
        let swaps = swaps(&layout);
        let good = facts(&layout);
        assert!(lines(&mounts, &swaps, &good).is_ok());
        let spoil = |index: usize, change: &dyn Fn(&mut Fact)| {
            let mut facts = good.clone();
            change(&mut facts.found[index]);
            lines(&mounts, &swaps, &facts).is_err()
        };
        // A kept partition found holding another filesystem, a made one not
        // holding what was made, a UUID that could break the line, a swap
        // partition holding no swap, nothing read at all.
        assert!(
            spoil(6, &|fact| fact.fstype = "ext4".into()),
            "NTFS no more"
        );
        assert!(
            spoil(10, &|fact| fact.fstype = "btrfs".into()),
            "/ not ext4"
        );
        assert!(spoil(1, &|fact| fact.uuid = "x y\n/ ext4".into()));
        assert!(spoil(8, &|fact| fact.fstype = "ext4".into()), "no swap");
        let mut none = good.clone();
        none.found.remove(3);
        assert!(lines(&mounts, &swaps, &none).is_err());
        // A path that would need escaping, though the rule set refuses it
        // first, is refused again.
        for at in [
            "/my data",
            "/tab\there",
            "/back\\slash",
            "data",
            "/data/",
            "/a//b",
        ] {
            let mut odd = mounts.clone();
            odd[2].at = at.to_string();
            assert!(lines(&odd, &swaps, &good).is_err(), "{at:?}");
        }
        // What cannot be mounted is never given a line.
        let mut swap_kept = every_kind();
        swap_kept[0].parts[8].fate = keep("/swapped");
        assert!(table(&swap_kept, 0).is_err());
        let mut ntfs_home = every_kind();
        ntfs_home[0].parts[1].fate = Fate::Delete;
        ntfs_home[0].parts[6].fate = keep("/home");
        assert!(table(&ntfs_home, 0).is_err());
        let mut xfs_boot = every_kind();
        xfs_boot[0].create[0].fs = Fs::Xfs;
        assert!(table(&xfs_boot, 0).is_err());
    }

    #[test]
    fn the_new_accounts_numbers_are_filled_in_at_finish() {
        let passwd = "root:x:0:0::/root:/bin/bash\n\
                      liveuser:x:1000:1000::/home/liveuser:/bin/bash\n\
                      anna:x:1001:984::/home/anna:/bin/bash\n";
        assert_eq!(ids(passwd, "anna"), Some((1001, 984)));
        assert_eq!(ids(passwd, "ann"), None);
        assert_eq!(ids("anna:x:one:984::/:/bin/sh\n", "anna"), None);
        let text = "UUID=1\t/windows\tntfs3\tnofail,uid=@uid@,gid=@gid@,windows_names\t0 0\n\
                    UUID=2\t/usb\tvfat\tnofail,uid=@uid@,gid=@gid@,umask=0022\t0 0\n";
        let filled = fill_ids(text, 1001, 984);
        assert_eq!(
            filled,
            "UUID=1\t/windows\tntfs3\tnofail,uid=1001,gid=984,windows_names\t0 0\n\
             UUID=2\t/usb\tvfat\tnofail,uid=1001,gid=984,umask=0022\t0 0\n"
        );
        assert!(!filled.contains(UID) && !filled.contains(GID));
    }

    #[test]
    fn mount_points_are_made_0755_inside_the_target_and_never_through_a_link() {
        let base = std::env::temp_dir().join(format!("lxb-mount-dirs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("root");
        std::fs::create_dir_all(&root).unwrap();
        let mode = |relative: &str| {
            std::fs::symlink_metadata(root.join(relative))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777
        };
        mount_dir(&root, "/data/games/old").unwrap();
        for made in ["data", "data/games", "data/games/old"] {
            assert_eq!(mode(made), 0o755, "{made}");
        }
        // A directory already there is left as it is.
        std::fs::create_dir(root.join("srv")).unwrap();
        std::fs::set_permissions(root.join("srv"), fs::Permissions::from_mode(0o700)).unwrap();
        mount_dir(&root, "/srv").unwrap();
        assert_eq!(mode("srv"), 0o700);
        mount_dir(&root, "/").unwrap();
        // Never through a link out of the target, nor onto a file.
        let outside = base.join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("var")).unwrap();
        assert!(mount_dir(&root, "/var/log").is_err());
        assert!(mount_dir(&root, "/var").is_err());
        assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
        std::fs::write(root.join("file"), b"").unwrap();
        assert!(mount_dir(&root, "/file").is_err());
        assert!(mount_dir(&root, "/file/below").is_err());
        assert_eq!(mode_after_mount("/tmp"), Some(0o1777));
        assert_eq!(mode_after_mount("/var/tmp"), None);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
