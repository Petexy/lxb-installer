//! Whether this is a Linexin live session, and which medium it started
//! from. Read-only; both processes ask it (the interface to offer Install at
//! all, the worker before writing). Owned by the data work.
//!
//! Two files answer it, and both are read as text so that the reading can be
//! tested on text written by hand: the kernel command line, which says
//! whether archiso started the system at all and how it found its medium, and
//! the mount table, which says whether the system image is really there as
//! the read-only thing the installer copies. The mount table is read from
//! `/proc/self/mountinfo` rather than through `findmnt`, so the answer does not
//! depend on another program's output or on which `findmnt` comes first on
//! somebody's PATH.
use serde::{Deserialize, Serialize};

/// Where archiso mounts the read-only system image (the squashfs or erofs
/// underneath the live system's writable overlay).
pub const IMAGE: &str = "/run/archiso/airootfs";

/// Where archiso keeps the boot medium mounted unless `copytoram` let go of it.
pub const BOOT_MOUNT: &str = "/run/archiso/bootmnt";

/// Everything archiso mounts lives below here: the medium, the image, the
/// disk an ISO file was started from (`img_dev`) and a persistent overlay
/// (`cow_device`). A disk with anything mounted below it carries the live
/// system.
pub const ARCHISO_RUN: &str = "/run/archiso/";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveSession {
    /// The read-only system image (`/run/archiso/airootfs`).
    pub image: std::path::PathBuf,
    /// `archisosearchuuid=` / `archisolabel=` from the kernel command line:
    /// what identifies the boot medium even after `copytoram` unmounted it.
    pub medium_uuid: Option<String>,
    pub medium_label: Option<String>,
    /// `/run/archiso/bootmnt` is still mounted (no copytoram).
    pub medium_mounted: bool,
}

/// `Some` when the kernel was started by archiso and the image is a
/// read-only squashfs/erofs on a loop device. On any other machine (a
/// development host included) it is `None`.
pub fn detect() -> Option<LiveSession> {
    let cmdline = std::fs::read_to_string("/proc/cmdline").ok()?;
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    parse(&cmdline, &mountinfo)
}

/// Whether this process is on the Linexin live medium at all: archiso's
/// image directory, [`IMAGE`], is there. Only root can make anything below
/// `/run`, so a person's program cannot pretend it is. The coarse first
/// question of the hard guard ([`guard`]); [`detect`] asks the finer one.
pub fn on_medium() -> bool {
    std::path::Path::new(IMAGE).exists()
}

/// The hard guard every road to the worker passes before anything else: the
/// interface asks it before starting one (`launch::start`), and the worker
/// before it does anything at all, root or not (`worker::main`). The two
/// roads to the read-only measurement pass it too: the interface asks it
/// before starting one (`launch::measure`, before any thread or program),
/// and the measure entry before anything at all, root or not
/// (`worker::measure::main`, through the worker's own `first_refusal`). Off
/// the Linexin live medium — a development machine, an installed system —
/// no installation is started, no worker runs and nothing is measured,
/// whatever was pressed or passed. `on_medium` is [`on_medium`] everywhere
/// but in a test, which hands in its own answer, so no test reads the
/// machine.
pub fn guard(on_medium: fn() -> bool) -> Result<(), String> {
    if on_medium() {
        Ok(())
    } else {
        Err(format!(
            "{IMAGE} is not there: this is not the Linexin live medium, \
             and nothing is installed from anywhere else"
        ))
    }
}

/// [`detect`] on the two texts it reads, so it can be tested without a live
/// system.
pub fn parse(cmdline: &str, mountinfo: &str) -> Option<LiveSession> {
    let parameters = kernel_parameters(cmdline);
    let value = |key: &str| {
        parameters
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    };
    // archiso's own hook needs `archisobasedir` to find the image at all, so
    // a kernel started without it was not started by archiso, whatever the
    // mount table happens to hold.
    parameters.iter().find(|(k, _)| k == "archisobasedir")?;

    // `archisodevice=UUID=…` / `LABEL=…` is the older way to name the medium
    // and still works; it only counts where the newer parameters are absent.
    let device = value("archisodevice").unwrap_or_default();
    let medium_uuid = value("archisosearchuuid")
        .or_else(|| device.strip_prefix("UUID=").map(str::to_string))
        .filter(|v| !v.is_empty());
    let medium_label = value("archisolabel")
        .or_else(|| device.strip_prefix("LABEL=").map(str::to_string))
        .filter(|v| !v.is_empty());

    let mounts = mounts(mountinfo);
    // The last mount at a path is the one on top, which is the one a reader
    // of that path sees.
    let image = mounts.iter().rev().find(|m| m.mount_point == IMAGE)?;
    let image_ok = matches!(image.fs_type.as_str(), "squashfs" | "erofs")
        && image.source.starts_with("/dev/loop")
        && image.options.iter().any(|o| o == "ro");
    if !image_ok {
        return None;
    }
    let medium_mounted = mounts.iter().any(|m| m.mount_point == BOOT_MOUNT);
    Some(LiveSession {
        image: IMAGE.into(),
        medium_uuid,
        medium_label,
        medium_mounted,
    })
}

/// The devices (as `major:minor`) that hold the live system's medium: what
/// is mounted at [`BOOT_MOUNT`], and whatever else archiso mounted below
/// `/run/archiso/` from a block device. Disk discovery marks the disks these
/// belong to as the installation medium.
pub fn medium_devices(mountinfo: &str) -> Vec<String> {
    let mut devices: Vec<String> = mounts(mountinfo)
        .into_iter()
        .filter(|m| m.mount_point.starts_with(ARCHISO_RUN))
        // The image is on a loop device, which is no disk, and so is the
        // medium when an ISO file was started from a disk; that disk is then
        // mounted below /run/archiso/ in its own right (img_dev).
        .filter(|m| m.source.starts_with("/dev/") && !m.source.starts_with("/dev/loop"))
        .map(|m| m.major_minor)
        .collect();
    devices.sort();
    devices.dedup();
    devices
}

/// One line of `/proc/<pid>/mountinfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    /// The device the filesystem is on, as `major:minor` (`0:NN` for a
    /// filesystem without one of its own, such as Btrfs or tmpfs).
    pub major_minor: String,
    /// The directory within the filesystem that is mounted (`/@` for a Btrfs
    /// subvolume).
    pub root: String,
    pub mount_point: String,
    /// The per-mount options (`ro`, `nosuid`…).
    pub options: Vec<String>,
    pub fs_type: String,
    /// What was mounted (`/dev/loop0`, `/dev/sda1`, `tmpfs`).
    pub source: String,
}

/// Every mount in a mountinfo text, in the order the kernel lists them
/// (oldest first). Lines without the documented shape are skipped.
pub fn mounts(mountinfo: &str) -> Vec<Mount> {
    mountinfo.lines().filter_map(mount_line).collect()
}

fn mount_line(line: &str) -> Option<Mount> {
    // id parent major:minor root mount-point options [optional…] - fstype source super-options
    let fields: Vec<&str> = line.split(' ').collect();
    let separator = fields.iter().position(|f| *f == "-")?;
    if separator < 6 || fields.len() < separator + 3 {
        return None;
    }
    Some(Mount {
        major_minor: fields[2].to_string(),
        root: unescape(fields[3]),
        mount_point: unescape(fields[4]),
        options: fields[5].split(',').map(str::to_string).collect(),
        fs_type: fields[separator + 1].to_string(),
        source: unescape(fields[separator + 2]),
    })
}

/// The kernel writes a space, tab, newline or backslash in a mountinfo or
/// swaps path as a three-digit octal escape (`\040`), so a path is read back
/// by undoing exactly that and nothing else.
pub fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1..i + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let value = u32::from(bytes[i + 1] - b'0') * 64
                + u32::from(bytes[i + 2] - b'0') * 8
                + u32::from(bytes[i + 3] - b'0');
            if let Ok(byte) = u8::try_from(value) {
                out.push(byte);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The kernel command line as `key=value` pairs (a bare word has an empty
/// value). A value may be wrapped in double quotes to hold spaces, which is
/// how the kernel itself splits it.
fn kernel_parameters(cmdline: &str) -> Vec<(String, String)> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    for c in cmdline.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
        .into_iter()
        .map(|w| match w.split_once('=') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (w, String::new()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Invented, but shaped exactly like an archiso session: the medium on a
    // USB stick's first partition, the image on a loop device from it, the
    // overlay on top of it as `/`.
    const CMDLINE: &str = "BOOT_IMAGE=/arch/boot/x86_64/vmlinuz-linux archisobasedir=arch \
        archisosearchuuid=2001-02-03-04-05-06-00 copytoram=n quiet splash\n";
    const MOUNTINFO: &str = "\
22 1 0:21 / /proc rw,nosuid,nodev,noexec,relatime shared:5 - proc proc rw
25 1 0:24 / /run rw,nosuid,nodev shared:6 - tmpfs run rw,mode=755
30 25 8:17 / /run/archiso/bootmnt ro,relatime shared:9 - iso9660 /dev/sdb1 ro,nojoliet
31 25 0:30 / /run/archiso/cowspace rw,relatime shared:10 - tmpfs cowspace rw,size=262144k
32 25 7:0 / /run/archiso/airootfs ro,relatime shared:11 - squashfs /dev/loop0 ro,errors=continue
1 0 0:31 / / rw,relatime shared:1 - overlay airootfs rw,lowerdir=/run/archiso/airootfs
";

    #[test]
    fn the_guard_lets_nothing_through_off_the_medium() {
        let refused = guard(|| false).expect_err("off the medium");
        assert!(refused.contains(IMAGE), "the refusal says what is missing");
        assert_eq!(guard(|| true), Ok(()));
    }

    #[test]
    fn an_archiso_session_is_recognised() {
        let live = parse(CMDLINE, MOUNTINFO).expect("a live session");
        assert_eq!(live.image, std::path::PathBuf::from(IMAGE));
        assert_eq!(live.medium_uuid.as_deref(), Some("2001-02-03-04-05-06-00"));
        assert_eq!(live.medium_label, None);
        assert!(live.medium_mounted);
    }

    #[test]
    fn copytoram_lets_go_of_the_medium_but_keeps_its_name() {
        let cmdline = "archisobasedir=arch archisolabel=\"EXAMPLE LIVE\" copytoram=y";
        let mountinfo = "\
25 1 0:24 / /run rw shared:6 - tmpfs run rw
33 25 7:1 / /run/archiso/airootfs ro,relatime - erofs /dev/loop1 ro
";
        let live = parse(cmdline, mountinfo).expect("a live session");
        assert_eq!(live.medium_label.as_deref(), Some("EXAMPLE LIVE"));
        assert!(!live.medium_mounted);
    }

    #[test]
    fn the_older_device_parameter_still_names_the_medium() {
        let cmdline = "archisobasedir=arch archisodevice=UUID=ABCD-1234";
        let live = parse(cmdline, MOUNTINFO).unwrap();
        assert_eq!(live.medium_uuid.as_deref(), Some("ABCD-1234"));
    }

    #[test]
    fn an_ordinary_system_is_not_a_live_session() {
        let cmdline = "BOOT_IMAGE=/vmlinuz-linux root=UUID=1111 rw quiet";
        assert_eq!(parse(cmdline, MOUNTINFO), None);
        // archiso's parameter without its image mounted is not enough either.
        let no_image = "1 0 0:31 / / rw,relatime shared:1 - ext4 /dev/sda2 rw\n";
        assert_eq!(parse(CMDLINE, no_image), None);
    }

    #[test]
    fn an_image_that_is_writable_or_not_on_a_loop_is_refused() {
        let writable = MOUNTINFO.replace(
            "7:0 / /run/archiso/airootfs ro,relatime",
            "7:0 / /run/archiso/airootfs rw,relatime",
        );
        assert_eq!(parse(CMDLINE, &writable), None);
        let not_loop = MOUNTINFO.replace("squashfs /dev/loop0", "squashfs /dev/sdb2");
        assert_eq!(parse(CMDLINE, &not_loop), None);
        let wrong_type = MOUNTINFO.replace("squashfs /dev/loop0", "ext4 /dev/loop0");
        assert_eq!(parse(CMDLINE, &wrong_type), None);
    }

    #[test]
    fn the_topmost_mount_at_the_image_path_decides() {
        let covered =
            format!("{MOUNTINFO}40 25 0:40 / /run/archiso/airootfs rw,relatime - tmpfs tmpfs rw\n");
        assert_eq!(parse(CMDLINE, &covered), None);
    }

    #[test]
    fn the_medium_device_is_what_backs_the_boot_mount() {
        assert_eq!(medium_devices(MOUNTINFO), ["8:17"]);
        let img_dev = "50 25 8:33 / /run/archiso/img_dev ro - ext4 /dev/sdc1 ro\n\
            51 25 7:3 / /run/archiso/bootmnt ro - iso9660 /dev/loop3 ro\n";
        assert_eq!(medium_devices(img_dev), ["8:33"]);
    }

    #[test]
    fn mount_paths_are_unescaped() {
        let line = "60 25 8:2 / /run/media/someone/My\\040Disk rw - ext4 /dev/sda2 rw\n";
        let mounts = mounts(line);
        assert_eq!(mounts[0].mount_point, "/run/media/someone/My Disk");
        assert_eq!(mounts[0].major_minor, "8:2");
        assert_eq!(unescape("a\\134b"), "a\\b");
        assert_eq!(unescape("trailing\\04"), "trailing\\04");
    }

    #[test]
    fn optional_mountinfo_fields_are_skipped() {
        let line = "70 25 8:3 / /mnt rw shared:1 master:2 propagate_from:3 - xfs /dev/sda3 rw\n";
        let mount = &mounts(line)[0];
        assert_eq!(mount.fs_type, "xfs");
        assert_eq!(mount.source, "/dev/sda3");
        assert!(mounts("garbage line\n").is_empty());
    }
}
