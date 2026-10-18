//! Which mounts lie below the stage, read from `/proc/self/mountinfo`, so
//! releasing works from what is really mounted rather than from what a
//! half-finished `prepare` believed it had mounted.
use std::path::{Path, PathBuf};

/// mountinfo escapes space, tab, newline and backslash in paths as octal.
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 4 <= bytes.len() {
            let digits = &bytes[i + 1..i + 4];
            if digits.iter().all(|c| (b'0'..=b'7').contains(c)) {
                let value = digits
                    .iter()
                    .fold(0u32, |value, c| value * 8 + u32::from(c - b'0'));
                if let Ok(value) = u8::try_from(value) {
                    out.push(value);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Every mount point in a mountinfo text, in the order the kernel lists them
/// (the order they were mounted in).
pub fn mount_points(mountinfo: &str) -> Vec<PathBuf> {
    mountinfo
        .lines()
        .filter_map(|line| line.split(' ').nth(4))
        .map(|field| PathBuf::from(unescape(field)))
        .collect()
}

/// The mount points strictly below `stage` (the stage itself belongs to
/// whoever made it).
pub fn below(points: &[PathBuf], stage: &Path) -> Vec<PathBuf> {
    points
        .iter()
        .filter(|p| p.as_path() != stage && p.starts_with(stage))
        .cloned()
        .collect()
}

/// Of `points`, the ones not inside another: unmounting each of these
/// recursively releases them all.
pub fn outermost(points: &[PathBuf]) -> Vec<PathBuf> {
    let mut tops: Vec<PathBuf> = Vec::new();
    for point in points {
        let inside = points
            .iter()
            .any(|other| other != point && point.starts_with(other));
        if !inside && !tops.contains(point) {
            tops.push(point.clone());
        }
    }
    tops
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTINFO: &str = "\
22 1 0:21 / / rw,relatime shared:1 - overlay airootfs rw
30 22 0:5 / /dev rw,nosuid shared:2 - devtmpfs dev rw
90 22 0:40 / /run/stage rw - tmpfs tmpfs rw
91 90 252:2 / /run/stage/root rw,noatime - btrfs /dev/vdx2 rw,subvol=/@
92 91 252:2 /@home /run/stage/root/home rw,noatime - btrfs /dev/vdx2 rw,subvol=/@home
93 91 252:1 / /run/stage/root/boot rw - vfat /dev/vdx1 rw,umask=0077
94 90 252:2 / /run/stage/btrfs rw - btrfs /dev/vdx2 rw,subvolid=5
95 22 252:9 / /run/stage-other rw - ext4 /dev/vdy9 rw
96 90 252:5 / /run/stage/with\\040space rw - ext4 /dev/vdy5 rw
";

    #[test]
    fn only_mounts_below_the_stage_are_released_outermost_first() {
        let points = mount_points(MOUNTINFO);
        let stage = Path::new("/run/stage");
        let below = below(&points, stage);
        assert_eq!(below.len(), 5, "not the stage itself, not a sibling");
        let tops = outermost(&below);
        assert_eq!(
            tops,
            [
                PathBuf::from("/run/stage/root"),
                PathBuf::from("/run/stage/btrfs"),
                PathBuf::from("/run/stage/with space"),
            ]
        );
    }

    #[test]
    fn escapes_are_read_and_stray_backslashes_kept() {
        assert_eq!(unescape("a\\040b\\011c"), "a b\tc");
        assert_eq!(unescape("a\\b"), "a\\b");
        assert_eq!(unescape("end\\04"), "end\\04");
    }
}
