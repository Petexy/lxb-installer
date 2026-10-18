//! **A kept filesystem made smaller** (§7.0, Unit Q, "The worker" 3 and
//! 4.1; step Q8): the exact commands, pure, and how each one's end is read.
//! Nothing here runs them — `shrinking` puts the dry run into phase A and
//! the shrink into phase B.1, each through the fence and `run::run_status`
//! (step Q9) — so here they are data, held to their words by the tests
//! below and run by the real tools only on image files a test made (the
//! `#[ignore]`d side-file tests, `partition::images`).
//!
//! A shrink is [`Shrink::of`] a `Planned::ShrinkFilesystem`, which `plan`
//! puts in phase B.1, before the `sfdisk -N` that makes its entry smaller in
//! B.2: the filesystem first, so that were the run to stop between the two,
//! a smaller filesystem is left in its old entry, never a filesystem
//! running past its entry's end. The size each tool is given never reaches
//! past the new entry ([`Shrink::target`]), and is always below the size of
//! the filesystem as phase A measured it: given a size at or above it,
//! resize2fs and ntfsresize would grow it, which no kept filesystem ever is
//! — so [`Shrink::of`] refuses one (and the fence again, by the size it was
//! handed), and the executor never asks it for one: an entry that holds the
//! filesystem as it is is made smaller alone (`shrinking::ready`, the rule
//! set's `Shrinkable::entry_only`). What is left is read back
//! ([`Shrink::fits`]): an ext4 by
//! `dumpe2fs -h`, an NTFS by its boot sector ([`ntfs_volume`]), since
//! `ntfsresize --info` refuses a volume Windows is now to check.
//!
//! - **ext4.** Phase A's dry run is `e2fsck -f -n`, which opens the
//!   filesystem read-only and answers every question no; anything it finds
//!   stops the run before a byte is written ([`Judge::E2fsckLook`]). What it
//!   does not see is a journal waiting to be played back: it skips the
//!   journal of such a filesystem and ends 0 ([`E2FSCK_LOOK`]), so phase A
//!   refuses one by its header before the dry run (`dumpe2fs -h` naming
//!   `needs_recovery`, `measure::Ext4Header::clean`). Phase
//!   B.1 is `e2fsck -f -y` — resize2fs refuses a filesystem not checked
//!   since it was last mounted, and `-n` never records a check — then
//!   `resize2fs <partition> <K>K`, K the new size in KiB rounded down.
//! - **NTFS.** Phase A's dry run is `ntfsresize --no-action` to the new
//!   size; phase B.1 the same with nothing but `--no-progress-bar`, and the
//!   one question ntfsresize asks before it writes answered `y` on standard
//!   input — never `--force` (see [`NTFS_SHRINK_FLAGS`]). Neither run can
//!   tell a hibernated Windows or an unclean journal: ntfsresize opens every
//!   volume "forensic" (§6, step Q3). The measure entry's read-only sign
//!   of hibernation (`measure`, `ntfscat … hiberfil.sys`) is what does, and
//!   `shrinking` asks it again in phase A, just before the shrink, by the
//!   same rule: only a `hiberfil.sys` of the volume's own, read not
//!   hibernated, lets a shrink go on — a volume with none (a Windows' data
//!   volume) is `measure::Hibernation::Absent`, never shrunk while its
//!   journal is not read.
//!
//! The flags below are what the tools' documentation and machine code say
//! (e2fsprogs 1.47.4 and ntfsprogs 2026.9.28, the 3.0 ISO's) and what they
//! were seen to do on image files; QEMU's `probe-resize-tools` confirms
//! each on the live system — each constant names the line of that probe
//! that does.
use super::plan::{Act, Planned};
use crate::model::disks::device_path_ok;
use crate::worker::partition::script::Command;
use crate::worker::run::Status;

/// `e2fsck`'s flags for phase A's dry run: force a full check, open
/// read-only and answer no. Confirmed by `probe-resize-tools`' line
/// "e2fsck `-f -n` and `-f -y` exit codes on a clean ext4 and a
/// needs-recovery one" (e2fsprogs 1.47.4 on the live system): 0 on the
/// clean one, the filesystem's bytes unchanged.
///
/// **Not a guard against a journal waiting to be played back.** On an ext4
/// left needing recovery it says "skipping journal recovery because doing
/// a read-only filesystem check" and ends **0** — what it found answered
/// no, none of it counted as an error left — its bytes unchanged: it does
/// not end with 4 in its code, as might be expected. So phase A never
/// leans on it for that: such a filesystem is refused by its header before
/// the dry run is run (`dumpe2fs -h` naming `needs_recovery` or
/// `orphan_present`, `measure::Ext4Header::clean`, read by the measure
/// entry and again by phase A), `FilesystemNeedsCheck`, nothing written.
pub const E2FSCK_LOOK: &[&str] = &["-f", "-n"];

/// `e2fsck`'s flags in phase B.1, just before resize2fs: a full check that
/// puts right what it may without a question nobody can answer. Confirmed
/// by the same probe line: 0 on the clean ext4. (On the one needing
/// recovery it played the journal back and ended 1, which goes on; phase A
/// refuses such a filesystem before it is ever reached, see
/// [`E2FSCK_LOOK`].)
pub const E2FSCK_REPAIR: &[&str] = &["-f", "-y"];

/// ntfsresize's flags for phase A's dry run (before `--size`): the volume
/// opened read-only, nothing written, no progress bar to read through.
/// Confirmed by `probe-resize-tools`' line "ntfsresize's prompt, `--force`
/// versus `--force --force` with stdin `/dev/null`, its exit codes": exit
/// 0 and "The read-only test run ended successfully." on dualboot-base.
pub const NTFS_DRY_RUN_FLAGS: &[&str] = &["--no-action", "--no-progress-bar"];

/// ntfsresize's flags for the shrink itself (before `--size`): no
/// `--force`, ever. Read from ntfsresize 2026.9.28's machine code:
///
/// - a volume marked for a check (Windows' dirty flag) is refused at the
///   mount unless a `--force` is spent there (`opt.force-- <= 0`, 0x8d0d);
/// - before writing, one `--force` left skips the question
///   `Are you sure you want to proceed (y/[n])?`; with none it is asked
///   (0x301d), and `proceed_question` (0x5f60) goes on when the answer
///   starts with `y` or `Y` — **and when standard input is at its end**
///   (`fgets` returning NULL goes straight on).
///
/// So `--force` with standard input `/dev/null` would shrink a volume
/// marked for a check: its one `--force` spent at the mount, the question
/// answered by the end of input. With no `--force`, such a volume is
/// refused before anything is written, and the question is answered by
/// [`NTFS_SHRINK_ANSWER`]. Confirmed by `probe-resize-tools`' line
/// "ntfsresize's prompt, `--force` versus `--force --force` with stdin
/// `/dev/null`, its exit codes, the dirty flag after a resize": on a
/// throwaway copy, with these flags and `y`, exit 0 and "Successfully
/// resized NTFS on device", and the volume marked for a check after it; on
/// a copy marked for a check, exit 1 and "Volume is scheduled for check"
/// with the copy byte-identical; and `--force` with `/dev/null` on such a
/// copy resizing it (why it is never given).
pub const NTFS_SHRINK_FLAGS: &[&str] = &["--no-progress-bar"];

/// What ntfsresize's question is answered with on standard input, without
/// `--force` ([`NTFS_SHRINK_FLAGS`]): the answer it takes as yes, and a
/// line end. Confirmed by the same probe line.
pub const NTFS_SHRINK_ANSWER: &str = "y\n";

/// The filesystems a kept partition is shrunk on in this version (NTFS and
/// ext4 first, by design), by the tools that do it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// e2fsck and resize2fs.
    Ext4,
    /// ntfsresize.
    Ntfs,
}

impl Tool {
    /// The tool for a filesystem lsblk names `fs`, if this version shrinks
    /// it.
    pub fn of(fs: &str) -> Option<Tool> {
        match fs {
            "ext4" => Some(Tool::Ext4),
            "ntfs" => Some(Tool::Ntfs),
            _ => None,
        }
    }

    /// Whether a filesystem of `bytes`, as its reader reads it (dumpe2fs's
    /// blocks times its block size; an NTFS volume's own size, by its boot
    /// sector), ends inside an entry `entry` bytes long: an ext4 at most the
    /// entry, an NTFS at least a sector short of it, which its backup boot
    /// sector takes. The executor's rule for what a shrink left
    /// ([`Shrink::fits`]), and the fence's before that entry is made
    /// smaller.
    pub fn ends_inside(self, bytes: u64, entry: u64) -> bool {
        match self {
            Tool::Ext4 => bytes <= entry,
            Tool::Ntfs => bytes < entry,
        }
    }
}

/// One kept filesystem made smaller: the partition, the tool, its entry's
/// size now and after, and the filesystem's own size as phase A measured it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shrink {
    pub device: String,
    pub tool: Tool,
    /// The partition's size now, in bytes.
    pub from: u64,
    /// Its entry's new size in bytes, inside which the filesystem must end.
    pub to: u64,
    /// The filesystem's own size in bytes, as phase A read it just before
    /// (an ext4's blocks, an NTFS volume's size: what it uses and what is
    /// free on it): never more than its partition, and more than the size
    /// its tool is given ([`Shrink::target_bytes`]).
    pub filesystem: u64,
}

/// How a command's end is read: which tool's rule ([`Judge::verdict`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Judge {
    /// `e2fsck -f -n`: only 0 goes on.
    E2fsckLook,
    /// `e2fsck -f -y`: 0 or 1 go on.
    E2fsckRepair,
    /// `resize2fs`: only 0 goes on.
    Resize2fs,
    /// `ntfsresize --no-action`: 0, and its own words for a test run that
    /// passed.
    NtfsDryRun,
    /// `ntfsresize`: 0, and its own words for a volume resized.
    NtfsShrink,
}

/// One command of a shrink, and how its end is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub command: Command,
    pub judge: Judge,
}

/// What a command's end says for the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The run goes on.
    GoOn,
    /// The filesystem needs a check first: errors found and left (e2fsck's
    /// 4), a check e2fsck must make first (resize2fs), Windows' own mark
    /// for one or an inconsistency (ntfsresize). `shrinking`'s
    /// `Failure::FilesystemNeedsCheck`.
    NeedsCheck,
    /// e2fsck put something right and asks for the system to be restarted
    /// (its 2), which for a partition mounted nowhere it never should: the
    /// run stops, the filesystem left as e2fsck left it.
    Restart,
    /// The tool did not do what it was asked: an operational or usage
    /// error, a code its rule does not know, or an end without its own
    /// words for success. The run stops.
    Failed,
}

impl Shrink {
    /// The shrink `planned` asks for, of a filesystem phase A measured at
    /// `filesystem` bytes, or why it is none this version carries out: a
    /// kept ext4 or NTFS on a partition discovery could name, to a size
    /// above nothing and below its own — and below the filesystem's own,
    /// which itself is no more than its partition. A size at or above the
    /// filesystem's is refused rather than skipped: resize2fs and
    /// ntfsresize would make the filesystem larger, and a kept filesystem
    /// only ever shrinks.
    pub fn of(planned: &Planned, filesystem: u64) -> Result<Shrink, String> {
        let Planned::ShrinkFilesystem {
            device,
            fs,
            from,
            to,
        } = planned
        else {
            return Err(format!("{} shrinks no filesystem", planned.shown()));
        };
        let tool = Tool::of(fs).ok_or_else(|| format!("{device}: {fs:?} is not shrunk"))?;
        if !device_path_ok(device) {
            return Err(format!("{device:?} is not a device discovery names"));
        }
        if *to == 0 || to >= from {
            return Err(format!("{device}: {to} bytes is not smaller than {from}"));
        }
        let shrink = Shrink {
            device: device.clone(),
            tool,
            from: *from,
            to: *to,
            filesystem,
        };
        if shrink.tool == Tool::Ext4 && *to < 1024 {
            return Err(format!("{device}: {to} bytes is less than a KiB"));
        }
        if filesystem > *from {
            return Err(format!(
                "{device}: its filesystem of {filesystem} bytes runs past its partition of {from}"
            ));
        }
        if shrink.target_bytes() >= filesystem {
            return Err(format!(
                "{device}: its filesystem is {filesystem} bytes, which {} would not shrink but \
                 grow or leave as it is",
                shrink.target()
            ));
        }
        Ok(shrink)
    }

    /// The new size as its tool is given it, never past the new entry:
    /// resize2fs in KiB rounded down (`<K>K`; it then rounds down to its own
    /// blocks), ntfsresize in bytes (it leaves the volume at least a sector
    /// smaller than the size it is given, so its backup boot sector is
    /// inside the entry too).
    pub fn target(&self) -> String {
        match self.tool {
            Tool::Ext4 => format!("{}K", self.to / 1024),
            Tool::Ntfs => self.to.to_string(),
        }
    }

    /// The bytes [`target`](Self::target) stands for.
    pub fn target_bytes(&self) -> u64 {
        match self.tool {
            Tool::Ext4 => self.to / 1024 * 1024,
            Tool::Ntfs => self.to,
        }
    }

    /// Phase A's dry run of this shrink, which writes nothing: `e2fsck -f
    /// -n`, or `ntfsresize --no-action` to the new size.
    pub fn dry_run(&self) -> Step {
        let device = self.device.as_str();
        match self.tool {
            Tool::Ext4 => {
                let mut args = E2FSCK_LOOK.to_vec();
                args.push(device);
                Step {
                    command: Command::new("e2fsck", &args),
                    judge: Judge::E2fsckLook,
                }
            }
            Tool::Ntfs => {
                let target = self.target();
                let mut args = NTFS_DRY_RUN_FLAGS.to_vec();
                args.extend(["--size", &target, device]);
                Step {
                    command: Command::new("ntfsresize", &args),
                    judge: Judge::NtfsDryRun,
                }
            }
        }
    }

    /// Phase B.1's commands for this shrink, in order: `e2fsck -f -y` and
    /// `resize2fs <partition> <K>K`; or ntfsresize to the new size, its
    /// question answered on standard input.
    pub fn steps(&self) -> Vec<Step> {
        let device = self.device.as_str();
        let target = self.target();
        match self.tool {
            Tool::Ext4 => {
                let mut check = E2FSCK_REPAIR.to_vec();
                check.push(device);
                vec![
                    Step {
                        command: Command::new("e2fsck", &check),
                        judge: Judge::E2fsckRepair,
                    },
                    Step {
                        command: Command::new("resize2fs", &[device, &target]),
                        judge: Judge::Resize2fs,
                    },
                ]
            }
            Tool::Ntfs => {
                let mut args = NTFS_SHRINK_FLAGS.to_vec();
                args.extend(["--size", &target, device]);
                vec![Step {
                    command: Command::new("ntfsresize", &args)
                        .with_input(NTFS_SHRINK_ANSWER.to_string()),
                    judge: Judge::NtfsShrink,
                }]
            }
        }
    }

    /// Phase B.1's commands ([`steps`](Self::steps)) as the executor
    /// carries them out: each a `Planned::Run` of `Act::ShrinkFilesystem`
    /// on the partition — which the fence judges by its shape, its place and
    /// its size — and how its end is read.
    pub fn planned(&self) -> Vec<(Planned, Judge)> {
        self.steps()
            .into_iter()
            .map(|step| {
                let run = Planned::Run {
                    act: Act::ShrinkFilesystem,
                    device: self.device.clone(),
                    command: step.command,
                };
                (run, step.judge)
            })
            .collect()
    }

    /// Whether a filesystem of `bytes`, read back once it is shrunk
    /// (dumpe2fs's blocks times its block size; the NTFS volume's own size),
    /// ends inside the new entry: an ext4 at most the entry, an NTFS at
    /// least a sector short of it, which its backup boot sector takes
    /// ([`Tool::ends_inside`]).
    pub fn fits(&self, bytes: u64) -> bool {
        self.tool.ends_inside(bytes, self.to)
    }
}

/// The size an NTFS volume's boot sector, `boot` (its first 512 bytes at
/// least), gives it in bytes — its sectors (the eight bytes at 0x28) times
/// its bytes a sector (the two at 0x0B) — or `None` where `boot` is no NTFS
/// boot sector (its OEM name at 3 is not `NTFS    `, a sector size not a
/// power of two from 256 to 4096, no sectors). How a shrunk NTFS is read
/// back: Windows' mark for a check, which the shrink leaves set, makes
/// `ntfsresize --info` refuse it.
pub fn ntfs_volume(boot: &[u8]) -> Option<u64> {
    if boot.len() < 512 || &boot[3..11] != b"NTFS    " {
        return None;
    }
    let per_sector = u64::from(u16::from_le_bytes([boot[0x0b], boot[0x0c]]));
    if !per_sector.is_power_of_two() || !(256..=4096).contains(&per_sector) {
        return None;
    }
    let sectors = u64::from_le_bytes(boot[0x28..0x30].try_into().ok()?);
    if sectors == 0 {
        return None;
    }
    sectors.checked_mul(per_sector)
}

impl Judge {
    /// What `status`, how the command ended, says for the run.
    ///
    /// e2fsck's code is a sum (e2fsck(8)): 1 errors put right, 2 put right
    /// and the system should be restarted, 4 errors left, 8 an operational
    /// error, 16 a usage error, 32 cancelled, 128 a shared library's error.
    /// Read bit by bit: errors left (4) are a check due whatever else it
    /// says; then anything from 8 up is a failure — it could not do what it
    /// was asked, which tells nothing of the filesystem; then the dry run
    /// goes on only from 0, since anything it found it would have put right
    /// (1 or 2: -n never puts anything right, so these say it found
    /// something), and the repair from 0 or 1, 2 asking for a restart.
    /// (The plan's "4 or more is FilesystemNeedsCheck" read as the sum it
    /// is: 8, 16, 32 and 128 alone say nothing of the filesystem.)
    ///
    /// resize2fs ends 0 when it resized; anything else stops, a check due
    /// where it says to run e2fsck first. ntfsresize ends 0 when it did what
    /// it was asked, and only that run goes on which also says so in its own
    /// words; a refusal is a check due where it names Windows' mark for one,
    /// an inconsistency or chkdsk.
    pub fn verdict(self, status: &Status) -> Verdict {
        let text = format!("{}\n{}", status.stdout, status.stderr);
        let code = status.code;
        match self {
            Judge::E2fsckLook | Judge::E2fsckRepair if code < 0 => Verdict::Failed,
            Judge::E2fsckLook | Judge::E2fsckRepair if code & 4 != 0 => Verdict::NeedsCheck,
            Judge::E2fsckLook | Judge::E2fsckRepair if code & !3 != 0 => Verdict::Failed,
            Judge::E2fsckLook => match code {
                0 => Verdict::GoOn,
                _ => Verdict::NeedsCheck,
            },
            Judge::E2fsckRepair => match code {
                0 | 1 => Verdict::GoOn,
                _ => Verdict::Restart,
            },
            Judge::Resize2fs => match code {
                0 => Verdict::GoOn,
                _ if text.contains("Please run 'e2fsck -f") => Verdict::NeedsCheck,
                _ => Verdict::Failed,
            },
            Judge::NtfsDryRun | Judge::NtfsShrink if code != 0 => {
                let due = ["scheduled for check", "is inconsistent", "chkdsk"];
                if due.iter().any(|words| text.contains(words)) {
                    Verdict::NeedsCheck
                } else {
                    Verdict::Failed
                }
            }
            Judge::NtfsDryRun if text.contains("The read-only test run ended successfully.") => {
                Verdict::GoOn
            }
            Judge::NtfsShrink if text.contains("Successfully resized NTFS on device") => {
                Verdict::GoOn
            }
            Judge::NtfsDryRun | Judge::NtfsShrink => Verdict::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::GIB;

    fn words(step: &Step) -> Vec<&str> {
        std::iter::once(step.command.program)
            .chain(step.command.args.iter().map(String::as_str))
            .collect()
    }

    fn planned(device: &str, fs: &str, from: u64, to: u64) -> Planned {
        Planned::ShrinkFilesystem {
            device: device.into(),
            fs: fs.into(),
            from,
            to,
        }
    }

    /// The shrink of `planned`, its filesystem filling its partition.
    fn of(planned: &Planned) -> Result<Shrink, String> {
        let Planned::ShrinkFilesystem { from, .. } = planned else {
            return Shrink::of(planned, 0);
        };
        Shrink::of(planned, *from)
    }

    /// Each builder's command line, word for word: the dry run phase A
    /// makes and the commands of phase B.1, for an ext4 and an NTFS.
    #[test]
    fn each_shrinks_commands_are_these_word_for_word() {
        let to = 20 * GIB + 3 * 512;
        let ext4 = of(&planned("/dev/vdx2", "ext4", 24 * GIB, to)).unwrap();
        assert_eq!(ext4.tool, Tool::Ext4);
        // KiB rounded down: 20 GiB and a sector and a half are 20 GiB and a KiB.
        assert_eq!(ext4.target(), "20971521K");
        assert_eq!(ext4.target_bytes(), 20 * GIB + 1024);
        assert_eq!(words(&ext4.dry_run()), ["e2fsck", "-f", "-n", "/dev/vdx2"]);
        assert_eq!(ext4.dry_run().judge, Judge::E2fsckLook);
        assert_eq!(ext4.dry_run().command.input, None);
        let steps = ext4.steps();
        assert_eq!(
            steps.iter().map(words).collect::<Vec<_>>(),
            [
                vec!["e2fsck", "-f", "-y", "/dev/vdx2"],
                vec!["resize2fs", "/dev/vdx2", "20971521K"],
            ]
        );
        assert_eq!(
            steps.iter().map(|step| step.judge).collect::<Vec<_>>(),
            [Judge::E2fsckRepair, Judge::Resize2fs]
        );
        assert!(steps.iter().all(|step| step.command.input.is_none()));

        let ntfs = of(&planned("/dev/nvme0n1p3", "ntfs", 200 * GIB, 120 * GIB)).unwrap();
        assert_eq!(ntfs.target(), (120 * GIB).to_string());
        assert_eq!(ntfs.target_bytes(), 120 * GIB);
        let size = (120 * GIB).to_string();
        assert_eq!(
            words(&ntfs.dry_run()),
            [
                "ntfsresize",
                "--no-action",
                "--no-progress-bar",
                "--size",
                &size,
                "/dev/nvme0n1p3"
            ]
        );
        assert_eq!(ntfs.dry_run().judge, Judge::NtfsDryRun);
        assert_eq!(ntfs.dry_run().command.input, None);
        let steps = ntfs.steps();
        assert_eq!(steps.len(), 1);
        assert_eq!(
            words(&steps[0]),
            [
                "ntfsresize",
                "--no-progress-bar",
                "--size",
                &size,
                "/dev/nvme0n1p3"
            ]
        );
        // Never --force: its one question answered yes on standard input.
        assert_eq!(steps[0].command.input.as_deref(), Some("y\n"));
        assert_eq!(steps[0].judge, Judge::NtfsShrink);
        for step in ntfs.steps().iter().chain([&ntfs.dry_run()]) {
            assert!(
                !step
                    .command
                    .args
                    .iter()
                    .any(|arg| arg == "--force" || arg == "-f"),
                "{:?}",
                step.command
            );
        }
    }

    /// Only a kept ext4 or NTFS on a partition discovery could name, made
    /// smaller and not to nothing, is a shrink.
    #[test]
    fn only_a_kept_ext4_or_ntfs_made_smaller_is_a_shrink() {
        let ok =
            |device: &str, fs: &str, from: u64, to: u64| of(&planned(device, fs, from, to)).is_ok();
        assert!(ok("/dev/vdx3", "ntfs", 30 * GIB, 20 * GIB));
        assert!(ok("/dev/mmcblk0p2", "ext4", 30 * GIB, 30 * GIB - 512 * 2));
        for fs in [
            "btrfs",
            "xfs",
            "vfat",
            "exfat",
            "ext3",
            "ntfs3",
            "BitLocker",
            "",
        ] {
            assert!(!ok("/dev/vdx3", fs, 30 * GIB, 20 * GIB), "{fs}");
        }
        // Not smaller, or to nothing, or to less than resize2fs can say.
        assert!(!ok("/dev/vdx3", "ntfs", 20 * GIB, 20 * GIB));
        assert!(!ok("/dev/vdx3", "ntfs", 20 * GIB, 30 * GIB));
        assert!(!ok("/dev/vdx3", "ntfs", 20 * GIB, 0));
        assert!(!ok("/dev/vdx2", "ext4", 20 * GIB, 512));
        // A name a tool would look up itself, or one that climbs out of /dev.
        for device in [
            "LABEL=Data",
            "UUID=5b7a3c1e",
            "/dev/../dev/sda1",
            "/tmp/x.img",
            "vdx3",
        ] {
            assert!(!ok(device, "ext4", 30 * GIB, 20 * GIB), "{device}");
        }
        // Any other planned command is no shrink.
        let wipe = Planned::Run {
            act: super::super::plan::Act::Wipe,
            device: "/dev/vdx".into(),
            command: Command::new("wipefs", &["--all", "--lock", "/dev/vdx"]),
        };
        assert!(of(&wipe).is_err());
    }

    /// **The size a tool is given is always below the filesystem's own, as
    /// phase A measured it** (steps Q8 and Q9): given its own size or more,
    /// resize2fs and ntfsresize would leave the filesystem as it is or grow
    /// it — a kept filesystem only ever shrinks — so such a shrink is
    /// refused, as is a filesystem measured past its partition; just below,
    /// it is a shrink.
    #[test]
    fn a_shrink_never_hands_its_tool_the_filesystems_own_size_or_more() {
        for (fs, partition) in [("ext4", 30 * GIB), ("ntfs", 30 * GIB)] {
            let asked = planned("/dev/vdx3", fs, partition, 20 * GIB);
            // A filesystem smaller than its partition, already as small as
            // the entry asked for, or smaller still: its tool would grow it.
            for filesystem in [20 * GIB, 20 * GIB - 512, 12 * GIB, 0] {
                let refused = Shrink::of(&asked, filesystem).unwrap_err();
                assert!(refused.contains("grow"), "{fs} of {filesystem}: {refused}");
            }
            // One that runs past its partition is no measure of it.
            assert!(Shrink::of(&asked, partition + 4096).is_err(), "{fs}");
            // Just more than the size its tool is given, up to its partition.
            for filesystem in [20 * GIB + 1024 + 1, partition - 512, partition] {
                let shrink = Shrink::of(&asked, filesystem).unwrap();
                assert!(shrink.target_bytes() < shrink.filesystem, "{fs}");
                assert_eq!(shrink.filesystem, filesystem);
            }
        }
        // resize2fs's KiB rounded down is what is held to it: 20 GiB and a
        // sector and a half is 20 GiB and a KiB to resize2fs.
        let odd = planned("/dev/vdx2", "ext4", 30 * GIB, 20 * GIB + 3 * 512);
        assert!(Shrink::of(&odd, 20 * GIB + 1024).is_err());
        assert!(Shrink::of(&odd, 20 * GIB + 1025).is_ok());
    }

    /// An NTFS volume's size is read from its boot sector, and nothing else
    /// is taken for one.
    #[test]
    fn an_ntfs_volume_is_read_from_its_boot_sector() {
        let boot = |oem: &[u8; 8], per_sector: u16, sectors: u64| {
            let mut bytes = vec![0u8; 512];
            bytes[3..11].copy_from_slice(oem);
            bytes[0x0b..0x0d].copy_from_slice(&per_sector.to_le_bytes());
            bytes[0x28..0x30].copy_from_slice(&sectors.to_le_bytes());
            bytes
        };
        // mkntfs's volume of a 1 GiB side file, a sector short of it, and
        // after the shrink to 512 MiB (the side-file test's own record).
        assert_eq!(
            ntfs_volume(&boot(b"NTFS    ", 512, 2_097_151)),
            Some(1_073_741_312)
        );
        assert_eq!(
            ntfs_volume(&boot(b"NTFS    ", 512, 1_048_568)),
            Some(536_866_816)
        );
        assert_eq!(
            ntfs_volume(&boot(b"NTFS    ", 4096, 262_143)),
            Some(1_073_737_728)
        );
        for (oem, per_sector, sectors) in [
            (b"EXFAT   ", 512, 2_097_151),
            (b"MSDOS5.0", 512, 2_097_151),
            (b"NTFS    ", 0, 2_097_151),
            (b"NTFS    ", 1000, 2_097_151),
            (b"NTFS    ", 8192, 2_097_151),
            (b"NTFS    ", 512, 0),
        ] {
            assert_eq!(ntfs_volume(&boot(oem, per_sector, sectors)), None);
        }
        assert_eq!(ntfs_volume(&boot(b"NTFS    ", 512, 2_097_151)[..511]), None);
        assert_eq!(ntfs_volume(&[]), None);
    }

    /// The filesystem ends inside its new entry: the size each tool is given
    /// is never more than the entry, and what is read back fits by each
    /// filesystem's own measure.
    #[test]
    fn the_size_a_tool_is_given_never_reaches_past_the_new_entry() {
        for to in [GIB, GIB + 512, GIB + 1023, GIB + 1024, 20 * GIB + 3 * 512] {
            for (tool, fs) in [(Tool::Ext4, "ext4"), (Tool::Ntfs, "ntfs")] {
                let shrink = of(&planned("/dev/vdx2", fs, 30 * GIB, to)).unwrap();
                assert_eq!(shrink.tool, tool);
                assert!(shrink.target_bytes() <= to, "{tool:?} at {to}");
                assert!(to - shrink.target_bytes() < 1024, "{tool:?} at {to}");
            }
        }
        let ext4 = of(&planned("/dev/vdx2", "ext4", 30 * GIB, 20 * GIB)).unwrap();
        assert!(ext4.fits(20 * GIB) && ext4.fits(20 * GIB - 4096));
        assert!(!ext4.fits(20 * GIB + 4096));
        let ntfs = of(&planned("/dev/vdx3", "ntfs", 30 * GIB, 20 * GIB)).unwrap();
        assert!(ntfs.fits(20 * GIB - 512));
        assert!(!ntfs.fits(20 * GIB), "its backup boot sector is outside");
    }

    fn ended(code: i32, stdout: &str, stderr: &str) -> Status {
        Status {
            code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    /// e2fsck's code read as the sum it is, for the dry run and the repair.
    #[test]
    fn e2fsck_codes_go_on_only_from_a_clean_filesystem() {
        use Verdict::*;
        let look = |code| Judge::E2fsckLook.verdict(&ended(code, "", ""));
        let repair = |code| Judge::E2fsckRepair.verdict(&ended(code, "", ""));
        // code: dry run, repair.
        for (code, dry, fixed) in [
            (0, GoOn, GoOn),
            (1, NeedsCheck, GoOn),
            (2, NeedsCheck, Restart),
            (3, NeedsCheck, Restart),
            (4, NeedsCheck, NeedsCheck),
            (5, NeedsCheck, NeedsCheck),
            (6, NeedsCheck, NeedsCheck),
            (8, Failed, Failed),
            (9, Failed, Failed),
            (10, Failed, Failed),
            (12, NeedsCheck, NeedsCheck),
            (16, Failed, Failed),
            (32, Failed, Failed),
            (36, NeedsCheck, NeedsCheck),
            (128, Failed, Failed),
            (-1, Failed, Failed),
        ] {
            assert_eq!(look(code), dry, "e2fsck -f -n ended {code}");
            assert_eq!(repair(code), fixed, "e2fsck -f -y ended {code}");
        }
    }

    // What the tools print, written by hand in their shape (e2fsprogs 1.47,
    // ntfsprogs 2026.9, read with `strings` from the binaries); the
    // side-file tests see the real tools print it.

    #[test]
    fn resize2fs_goes_on_only_once_it_resized() {
        let judge = |status| Judge::Resize2fs.verdict(&status);
        assert_eq!(
            judge(ended(
                0,
                "Resizing the filesystem on /dev/vdx2 to 5242880 (4k) blocks.\n\
                 The filesystem on /dev/vdx2 is now 5242880 (4k) blocks long.\n\n",
                "resize2fs 1.47.4 (6-Mar-2025)\n"
            )),
            Verdict::GoOn
        );
        assert_eq!(
            judge(ended(1, "", "Please run 'e2fsck -f /dev/vdx2' first.\n\n")),
            Verdict::NeedsCheck
        );
        assert_eq!(
            judge(ended(
                1,
                "",
                "resize2fs: New size smaller than minimum (2961408)\n"
            )),
            Verdict::Failed
        );
        assert_eq!(judge(ended(8, "", "")), Verdict::Failed);
    }

    #[test]
    fn ntfsresize_goes_on_only_when_it_says_it_did_what_it_was_asked() {
        let dry = |status| Judge::NtfsDryRun.verdict(&status);
        let shrink = |status| Judge::NtfsShrink.verdict(&status);
        let passed = "Checking filesystem consistency ...\n\
                      New volume size    : 21474836480 bytes (21475 MB)\n\
                      The read-only test run ended successfully.\n";
        let resized = "Syncing device ...\n\
                       Successfully resized NTFS on device '/dev/vdx3'.\n\
                       You can go on to shrink the device for example with Linux fdisk.\n";
        assert_eq!(dry(ended(0, passed, "")), Verdict::GoOn);
        assert_eq!(shrink(ended(0, resized, "")), Verdict::GoOn);
        // Each only by its own words: a run that ended well and said the
        // other, or nothing it knows, stops.
        assert_eq!(dry(ended(0, resized, "")), Verdict::Failed);
        assert_eq!(shrink(ended(0, passed, "")), Verdict::Failed);
        let nothing = "Nothing to do: NTFS volume size is already OK.\n";
        assert_eq!(shrink(ended(0, nothing, "")), Verdict::Failed);
        // Windows' mark for a check, an inconsistency: a check due.
        let marked = ended(
            1,
            "",
            "ERROR: Volume is scheduled for check.\n\
             Run chkdsk /f and please try again, or see option -f.\n",
        );
        assert_eq!(dry(marked.clone()), Verdict::NeedsCheck);
        assert_eq!(shrink(marked), Verdict::NeedsCheck);
        let inconsistent = ended(
            1,
            "NTFS is inconsistent. Run chkdsk /f on Windows then reboot it TWICE!\n",
            "",
        );
        assert_eq!(shrink(inconsistent), Verdict::NeedsCheck);
        // Refused for anything else — too small, the question answered no.
        let small = ended(
            1,
            "",
            "ERROR: New size can't be less than the space already occupied by data.\n",
        );
        assert_eq!(dry(small), Verdict::Failed);
        let quit = ended(
            1,
            "Are you sure you want to proceed (y/[n])? \
             OK quitting. NO CHANGES have been made to your NTFS volume.\n",
            "",
        );
        assert_eq!(shrink(quit), Verdict::Failed);
    }
}
