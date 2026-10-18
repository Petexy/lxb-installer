//! The arithmetic of Partitions' sheets — New partition, Resize, Label…,
//! Mount point · Other… — kept apart from their drawing so it is tested
//! without a renderer: how a held size steps and snaps, how a typed number
//! is read, which paths a mount point of one's own may be, and how long a
//! label each filesystem holds.
//!
//! Like the plan (`ui::partitions`), nothing here can reach a disk: it is
//! numbers and words over the `Disk`s the interface already holds.

use crate::model::disks::{GIB, MIB};
use crate::ui::partitions::{Mount, Operation, Plan};

/// How far one step of a held size goes in a room `most` bytes large: a GiB
/// where the room is under 64 GiB, 4 GiB under 512 GiB, 16 GiB above — so
/// crossing any room takes some tens of steps, and ten at a time (LB, RB,
/// Tab) a handful.
pub fn step_for(most: u64) -> u64 {
    if most < 64 * GIB {
        GIB
    } else if most < 512 * GIB {
        4 * GIB
    } else {
        16 * GIB
    }
}

/// Where a size held between `least` and `most` goes `steps` steps up (or
/// down, negative): from one whole step to the next — a size between two
/// whole steps goes to the next whole one first — the last step up snapping
/// to all of the room and the last step down to the least it may be. Every
/// size it gives is on a MiB.
pub fn stepped(size: u64, least: u64, most: u64, steps: i32) -> u64 {
    if most <= least {
        return most;
    }
    let step = step_for(most);
    let size = size.clamp(least, most);
    let below = size / step;
    let on = size.is_multiple_of(step);
    let target = if steps >= 0 {
        (below + steps as u64).saturating_mul(step)
    } else {
        let back = steps.unsigned_abs() as u64;
        let from = if on { below } else { below + 1 };
        from.saturating_sub(back).saturating_mul(step)
    };
    aligned(target.clamp(least, most)).clamp(least, most)
}

/// `bytes` down to a whole MiB.
pub fn aligned(bytes: u64) -> u64 {
    bytes / MIB * MIB
}

/// The longest a number typed for a size may be, in characters.
pub const TYPED_MOST: usize = 9;

/// What of `typed` a held size takes: its digits — ASCII, full-width, or
/// Devanagari, as an input method may write them — and a decimal mark,
/// a full stop or a comma in any language (whichever the keyboard has),
/// written as typed.
pub fn typed_keeps(letter: char) -> bool {
    digit(letter).is_some() || is_mark(letter)
}

fn digit(letter: char) -> Option<u32> {
    match letter {
        '0'..='9' => letter.to_digit(10),
        '\u{ff10}'..='\u{ff19}' => Some(letter as u32 - 0xff10),
        '\u{0966}'..='\u{096f}' => Some(letter as u32 - 0x0966),
        _ => None,
    }
}

fn is_mark(letter: char) -> bool {
    matches!(letter, '.' | ',' | '\u{ff0e}' | '\u{ff0c}' | '\u{066b}')
}

/// A number typed for a size, in GiB — whole or with one decimal mark, a
/// full stop or a comma, whatever the language writes ("120", "120.5",
/// "120,5", ",5") — as bytes, down to a whole MiB. `None` for nothing a
/// size can be read from: nothing typed, a mark alone, two marks, or
/// nothing at all.
pub fn parse_gib(typed: &str) -> Option<u64> {
    let typed = typed.trim();
    let mut whole: u64 = 0;
    let mut fraction: u64 = 0;
    let mut places: u32 = 0;
    let mut marked = false;
    let mut digits = 0;
    for letter in typed.chars() {
        if is_mark(letter) {
            if marked {
                return None;
            }
            marked = true;
            continue;
        }
        let value = u64::from(digit(letter)?);
        digits += 1;
        if marked {
            // Past a MiB's worth nothing more is read (a GiB has 1024 of
            // them, so four places are more than enough).
            if places < 6 {
                fraction = fraction * 10 + value;
                places += 1;
            }
        } else {
            whole = whole.checked_mul(10)?.checked_add(value)?;
        }
    }
    if digits == 0 {
        return None;
    }
    let bytes = whole.checked_mul(GIB)?;
    let part = (u128::from(fraction) * u128::from(GIB) / 10u128.pow(places)) as u64;
    Some(aligned(bytes.checked_add(part)?))
}

/// The rule set's own, read here where they were (`model::layout`).
pub use crate::model::layout::{label_limit, normal_path, Limit, PathRefused, SYSTEM_FOLDERS};

/// Why a path typed for a mount point of one's own cannot be used
/// (`model::layout::PathRefused`), in the catalogs' words.
impl PathRefused {
    /// What is said under the field.
    pub fn words(&self) -> String {
        match self {
            PathRefused::NotWhole => crate::i18n::text("partitions-path-whole").to_string(),
            PathRefused::Long => crate::i18n::text("partitions-path-long").to_string(),
            PathRefused::Spaces => crate::i18n::text("partitions-path-spaces").to_string(),
            PathRefused::Under(path) => {
                crate::message!("partitions-path-under", "path" => path.as_str())
            }
            PathRefused::Twice(path) => {
                crate::message!("partitions-path-twice", "path" => path.as_str())
            }
            PathRefused::System(path) => {
                crate::message!("partitions-path-system", "path" => path.as_str())
            }
        }
    }
}

/// The mount point `typed` is for the partition at `part`, held to the
/// rules every mount point of one's own keeps: a whole path, normalised
/// ([`normal_path`]); one of the mount points offered by name is that one
/// (`/home` typed is `/home`); never the system's own folders or anything
/// under them; and never where another partition of `plan` is used.
pub fn check_path(typed: &str, plan: &Plan, part: &str) -> Result<Mount, PathRefused> {
    let mount = crate::model::layout::mount_path(typed)?;
    let path = mount.path().to_string();
    let elsewhere = plan.operations().iter().any(|op| {
        let (here, at) = match op {
            Operation::Mount { part, at, .. } => (part.as_str(), Some(at)),
            Operation::Create { part, new, .. } => (part.as_str(), new.mount.as_ref()),
            _ => return false,
        };
        here != part && at.is_some_and(|at| at.path() == path)
    });
    if elsewhere {
        return Err(PathRefused::Twice(path));
    }
    Ok(mount)
}

/// How long a label a filesystem holds (`model::layout::Limit`), in the
/// catalogs' words.
impl Limit {
    /// What is said of it under the field: "ext4 labels hold 16 bytes at
    /// most".
    pub fn words(self, filesystem: &str) -> String {
        match self {
            Limit::Bytes(most) => crate::message!(
                "partitions-label-bytes",
                "filesystem" => filesystem,
                "count" => most as i64
            ),
            Limit::Chars(most) => crate::message!(
                "partitions-label-chars",
                "filesystem" => filesystem,
                "count" => most as i64
            ),
        }
    }
}

/// Why what was typed into a label was not all taken ([`label_with`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelRefused {
    /// A letter its filesystem does not hold, or one past its limit: the
    /// limit is said ([`Limit::words`]).
    Limit,
    /// A `/`, which no label holds (`layout::LABEL_REFUSES`): "A label can't
    /// hold “/”".
    Slash,
}

/// `label` with `typed` written after it as a label of `filesystem` (by
/// `map::filesystem`'s name) takes it — FAT's letters made capitals, as
/// GParted writes them (`layout::label_letter`) — up to the first letter it
/// cannot take or that would not fit (`label_limit`), and nothing after it;
/// and, where any of it had to be left out, why: a `/` (which no label
/// holds), or a letter the filesystem's label does not take or has no room
/// for.
pub fn label_with(
    label: &str,
    typed: &str,
    filesystem: Option<&str>,
) -> (String, Option<LabelRefused>) {
    let limit = label_limit(filesystem);
    let mut out = label.to_string();
    for letter in typed.chars().filter(|letter| !letter.is_control()) {
        if letter == crate::model::layout::LABEL_REFUSES {
            return (out, Some(LabelRefused::Slash));
        }
        let Some(letter) = crate::model::layout::label_letter(filesystem, letter, out.is_empty())
        else {
            return (out, Some(LabelRefused::Limit));
        };
        out.push(letter);
        if limit.is_some_and(|limit| !limit.fits(&out)) {
            out.pop();
            return (out, Some(LabelRefused::Limit));
        }
    }
    (out, None)
}

impl LabelRefused {
    /// What is said under the label of `filesystem` (by `map::filesystem`'s
    /// name) when it is refused: its limit, or that no label holds a `/`.
    pub fn words(self, filesystem: &str) -> Option<String> {
        match self {
            LabelRefused::Limit => {
                label_limit(Some(filesystem)).map(|limit| limit.words(filesystem))
            }
            LabelRefused::Slash => Some(crate::i18n::text("partitions-label-slash").to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::partitions::{Fs, NewPart};

    #[test]
    fn a_step_is_a_gib_four_or_sixteen_by_the_room() {
        assert_eq!(step_for(20 * GIB), GIB);
        assert_eq!(step_for(64 * GIB - MIB), GIB);
        assert_eq!(step_for(64 * GIB), 4 * GIB);
        assert_eq!(step_for(279 * GIB), 4 * GIB);
        assert_eq!(step_for(512 * GIB), 16 * GIB);
    }

    #[test]
    fn steps_go_from_whole_step_to_whole_step_and_snap_to_the_ends() {
        // A room of 279.4 GiB, in 4 GiB steps, from a least of 23.3 GiB.
        let most = 286_102 * MIB;
        let least = 23_842 * MIB;
        // From all of it, one down is the whole step under it.
        assert_eq!(stepped(most, least, most, -1), 276 * GIB);
        assert_eq!(stepped(276 * GIB, least, most, -1), 272 * GIB);
        // Up from the last whole step snaps to all of the room.
        assert_eq!(stepped(276 * GIB, least, most, 1), most);
        assert_eq!(stepped(most, least, most, 1), most);
        // Between two whole steps: to the next one, either way.
        assert_eq!(stepped(121 * GIB, least, most, 1), 124 * GIB);
        assert_eq!(stepped(121 * GIB, least, most, -1), 120 * GIB);
        // Ten at a time.
        assert_eq!(stepped(120 * GIB, least, most, 10), 160 * GIB);
        assert_eq!(stepped(120 * GIB, least, most, -10), 80 * GIB);
        // Down to the least, which is not a whole step, and no further.
        assert_eq!(stepped(24 * GIB, least, most, -1), least);
        assert_eq!(stepped(least, least, most, -10), least);
        assert_eq!(stepped(least, least, most, 1), 24 * GIB);
        // Every size on a MiB.
        for steps in -12..12 {
            assert_eq!(stepped(100 * GIB + 7, least, most, steps) % MIB, 0);
        }
        // A room no larger than its least does not move.
        assert_eq!(stepped(MIB, MIB, MIB, 1), MIB);
    }

    #[test]
    fn a_typed_size_is_gib_with_either_decimal_mark() {
        assert_eq!(parse_gib("120"), Some(120 * GIB));
        assert_eq!(parse_gib("120.5"), Some(120 * GIB + 512 * MIB));
        assert_eq!(parse_gib("120,5"), Some(120 * GIB + 512 * MIB));
        assert_eq!(parse_gib(",25"), Some(256 * MIB));
        assert_eq!(parse_gib("7."), Some(7 * GIB));
        assert_eq!(parse_gib(" 2 "), Some(2 * GIB));
        // Full-width digits and mark, as a Chinese input method writes them,
        // and Devanagari digits.
        assert_eq!(parse_gib("１２０．５"), Some(120 * GIB + 512 * MIB));
        assert_eq!(parse_gib("१२०"), Some(120 * GIB));
        // Down to a whole MiB.
        assert_eq!(parse_gib("0.0001").map(|bytes| bytes % MIB), Some(0));
        for nothing in ["", ".", ",", "1.2.3", "1,2.3", "12a", "-3", "1e3"] {
            assert_eq!(parse_gib(nothing), None, "{nothing}");
        }
        assert!(typed_keeps('7') && typed_keeps(',') && typed_keeps('.'));
        assert!(!typed_keeps('a') && !typed_keeps(' ') && !typed_keeps('-'));
    }

    #[test]
    fn a_path_is_made_whole_and_the_systems_folders_are_refused() {
        assert_eq!(normal_path("/data//photos/"), Some("/data/photos".into()));
        assert_eq!(normal_path("//"), Some("/".into()));
        assert_eq!(normal_path("/srv/./www/."), Some("/srv/www".into()));
        assert_eq!(normal_path("data"), None);
        assert_eq!(normal_path("/srv/../etc"), None);
        assert_eq!(normal_path(""), None);
        let plan = Plan::default();
        let part = "/dev/sda5";
        assert_eq!(
            check_path("/data/", &plan, part),
            Ok(Mount::Other("/data".into()))
        );
        // A mount point offered by name is that one.
        assert_eq!(check_path("/home/", &plan, part), Ok(Mount::Home));
        assert_eq!(check_path("//", &plan, part), Ok(Mount::Root));
        for system in ["/usr", "/usr/local", "/etc/", "/lib64", "/proc/1", "//dev"] {
            assert!(
                matches!(check_path(system, &plan, part), Err(PathRefused::System(_))),
                "{system}"
            );
        }
        // Folders only named like them are not theirs.
        assert!(check_path("/library", &plan, part).is_ok());
        assert!(check_path("/user", &plan, part).is_ok());
        assert_eq!(check_path("data", &plan, part), Err(PathRefused::NotWhole));
        // The rules every path of one's own keeps, said in their words.
        for (typed, refused, said) in [
            ("/my data", PathRefused::Spaces, "spaces"),
            ("/home/piotr", PathRefused::Under("/home".into()), "/home"),
            ("/boot/efi", PathRefused::Under("/boot".into()), "/boot"),
            ("/swap", PathRefused::System("/swap".into()), "/swap"),
        ] {
            let checked = check_path(typed, &plan, part);
            assert_eq!(checked, Err(refused), "{typed}");
            assert!(checked.unwrap_err().words().contains(said), "{typed}");
        }
        let long = format!("/{}", "x".repeat(300));
        assert_eq!(check_path(&long, &plan, part), Err(PathRefused::Long));
    }

    #[test]
    fn a_path_used_by_another_partition_is_refused_but_its_own_is_not() {
        let mut plan = Plan::default();
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Other("/data".into())));
        plan.create(
            "/dev/sda",
            NewPart {
                number: 1,
                start: 0,
                size: GIB,
                fs: Fs::Ext4,
                mount: Some(Mount::Other("/srv".into())),
                label: String::new(),
            },
        );
        assert_eq!(
            check_path("/data", &plan, "/dev/sda3"),
            Err(PathRefused::Twice("/data".into()))
        );
        assert_eq!(
            check_path("/srv/", &plan, "/dev/sda3"),
            Err(PathRefused::Twice("/srv".into()))
        );
        assert!(check_path("/data", &plan, "/dev/sda5").is_ok());
        assert!(check_path("/srv", &plan, "new#1").is_ok());
        plan.mount("/dev/sda", "/dev/sda1", Some(Mount::Home));
        assert_eq!(
            check_path("/home", &plan, "/dev/sda3"),
            Err(PathRefused::Twice("/home".into()))
        );
    }

    #[test]
    fn each_filesystem_holds_a_label_as_long_as_it_allows() {
        assert_eq!(label_limit(Some("ext4")), Some(Limit::Bytes(16)));
        assert_eq!(label_limit(Some("fat32")), Some(Limit::Bytes(11)));
        assert_eq!(label_limit(Some("xfs")), Some(Limit::Bytes(12)));
        assert_eq!(label_limit(Some("btrfs")), Some(Limit::Bytes(255)));
        assert_eq!(label_limit(Some("ntfs")), Some(Limit::Chars(128)));
        assert_eq!(label_limit(None), None);
        assert_eq!(label_limit(Some("bios_grub")), None);
        // Bytes are counted for ext4: "Zażółć" is six letters and ten bytes.
        let (label, refused) = label_with("", "Zażółć gęślą", Some("ext4"));
        assert!(label.len() <= 16 && refused.is_some(), "{label}");
        assert_eq!(label, "Zażółć gęś");
        let (label, refused) = label_with("HOME", "DATA", Some("fat32"));
        assert_eq!((label.as_str(), refused), ("HOMEDATA", None));
        let (label, refused) = label_with("", &"x".repeat(130), Some("ntfs"));
        assert_eq!(
            (label.chars().count(), refused),
            (128, Some(LabelRefused::Limit))
        );
        // FAT's letters are capitals, and nothing outside them goes in; no
        // label starts with "-".
        let limit = Some(LabelRefused::Limit);
        assert_eq!(label_with("", "boot", Some("fat32")), ("BOOT".into(), None));
        assert_eq!(
            label_with("", "zażółć", Some("fat32")),
            ("ZA".into(), limit)
        );
        assert_eq!(
            label_with("", " EFI", Some("fat32")),
            (String::new(), limit)
        );
        assert_eq!(label_with("", "-L", Some("ext4")), (String::new(), limit));
        assert_eq!(label_with("a", "-b", Some("ext4")), ("a-b".into(), None));
        // Nor a "/", anywhere: the label goes onto the command line that
        // makes the filesystem, beside the device. Said for what it is, in
        // FAT's labels too, whose letters never held one.
        let slash = Some(LabelRefused::Slash);
        for filesystem in ["ext4", "btrfs", "xfs", "linux-swap", "fat32"] {
            assert_eq!(
                label_with("", "a/devops", Some(filesystem)).1,
                slash,
                "{filesystem}"
            );
            assert_eq!(
                label_with("", "/dev/vdz", Some(filesystem)),
                (String::new(), slash)
            );
            assert_eq!(
                label_with("AB", "/", Some(filesystem)),
                ("AB".into(), slash)
            );
            assert_eq!(
                LabelRefused::Slash.words(filesystem).as_deref(),
                Some("A label can't hold “/”")
            );
        }
        assert_eq!(label_with("", "a/devops", Some("ext4")).0, "a");
        assert_eq!(label_with("", "a/devops", Some("fat32")).0, "A");
        // A "/" after a letter the label had no room for is that letter's
        // refusal, said as the limit.
        assert_eq!(
            label_with("", &format!("{}/", "x".repeat(17)), Some("ext4")).1,
            limit
        );
        assert_eq!(
            LabelRefused::Limit.words("ext4").as_deref(),
            Some("ext4 labels hold 16 bytes at most")
        );
        assert!(Limit::Bytes(16).words("ext4").contains("16"));
    }

    /// Whatever is typed, and onto whatever was typed before, the label the
    /// sheet writes is one the rule set takes (`layout::label_fits`) for
    /// every filesystem a label can be given with.
    #[test]
    fn whatever_is_typed_the_label_is_one_the_rules_take() {
        let mut typed: Vec<String> = (' '..='~')
            .flat_map(|letter| [letter.to_string(), format!("a{letter}b")])
            .collect();
        typed.extend(
            [
                "a/devops",
                "x/dev",
                "/dev/vdz",
                "--opt=/dev/vdy",
                "Zażółć gęślą jaźń",
                "tab\there",
                "日本語のラベル",
            ]
            .map(String::from),
        );
        typed.push("L".repeat(300));
        for fs in [Fs::Ext4, Fs::Btrfs, Fs::Xfs, Fs::Fat32, Fs::LinuxSwap] {
            for before in ["", "AB"] {
                for typed in &typed {
                    let (label, _) = label_with(before, typed, Some(fs.name()));
                    assert!(
                        crate::model::layout::label_fits(fs, &label),
                        "{fs:?}: {typed:?} after {before:?} gave {label:?}"
                    );
                }
            }
        }
    }
}
