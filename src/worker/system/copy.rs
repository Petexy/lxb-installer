//! Copying the system image onto the new root.
//!
//! rsync copies the read-only image into the mounted target, keeping hard
//! links, ACLs, extended attributes and numeric owners (the legacy copy had
//! neither `-H` nor `--numeric-ids`), and reports its progress over the whole
//! transfer (`--info=progress2` with the file list built first, so the
//! percentage does not jump back as it finds more files). What the target
//! mounts for itself — the EFI partition at `/boot` or `/efi`, a separate or
//! Btrfs `/home` — and the directories the kernel fills at run time are left
//! out, then made empty with the modes an installed system has. The image's
//! `/home` holds only the live account, which the installed system does not
//! keep; leaving it out means the copy never writes into a kept /home.
use super::Install;
use crate::model::protocol::{Event, Failure, Step};
use crate::worker::layout::Stop;
use crate::worker::{run, target};

/// What rsync leaves out, relative to the image's root.
pub const EXCLUDES: &[&str] = &[
    "/boot/***",
    "/efi/***",
    "/dev/***",
    "/proc/***",
    "/sys/***",
    "/run/***",
    "/tmp/***",
    "/mnt/***",
    "/media/***",
    "/home/***",
];

/// The directories made empty after the copy, with their modes: the
/// kernel's own filesystems are read-only to everyone, `/tmp` is the shared
/// sticky directory, and the rest are ordinary. One that already exists is
/// something mounted there (the EFI partition, a /home) and is left as it is.
pub const EMPTY_DIRECTORIES: &[(&str, u32)] = &[
    ("boot", 0o755),
    ("dev", 0o755),
    ("proc", 0o555),
    ("sys", 0o555),
    ("run", 0o755),
    ("tmp", 0o1777),
    ("mnt", 0o755),
    ("media", 0o755),
    ("home", 0o755),
];

/// rsync's arguments for copying `source` into `root`. Both end in `/`, so
/// the image's contents land in the root rather than a directory named after
/// it.
pub fn rsync_args(source: &str, root: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "-aHAX",
        "--numeric-ids",
        "--one-file-system",
        "--info=progress2",
        "--no-inc-recursive",
    ]
    .iter()
    .map(|a| a.to_string())
    .collect();
    args.extend(
        EXCLUDES
            .iter()
            .map(|pattern| format!("--exclude={pattern}")),
    );
    args.push("--".into());
    args.push(format!("{}/", source.trim_end_matches('/')));
    args.push(format!("{}/", root.trim_end_matches('/')));
    args
}

/// One `--info=progress2` line as (bytes copied, percent), or `None` for any
/// other line. The byte count may carry digit separators (rsync groups
/// digits by default, with `,` or `.` depending on the locale's decimal
/// point); a count with units (`-h`) is not a byte count and is refused.
///
/// `      1,234,567  42%   12.34MB/s    0:00:05 (xfr#12, to-chk=3/45)`
pub fn parse_progress(line: &str) -> Option<(u64, u8)> {
    let mut fields = line.split_whitespace();
    let bytes = fields.next()?;
    let percent = fields.next()?.strip_suffix('%')?;
    if !bytes.starts_with(|c: char| c.is_ascii_digit())
        || !bytes
            .chars()
            .all(|c| c.is_ascii_digit() || ",.'".contains(c))
    {
        return None;
    }
    let digits: String = bytes.chars().filter(char::is_ascii_digit).collect();
    let bytes = digits.parse::<u64>().ok()?;
    let percent = percent.parse::<u8>().ok().filter(|p| *p <= 100)?;
    // A line of progress always goes on to the rate.
    fields.next()?.ends_with("/s").then_some((bytes, percent))
}

/// The size `du -sb` printed (`3123456789\t/run/archiso/airootfs`).
pub fn parse_du(output: &str) -> Option<u64> {
    output
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()
        .filter(|size| *size > 0)
}

/// Turns rsync's progress lines into progress events, one per whole percent
/// at most, so a copy of a hundred thousand files does not become a hundred
/// thousand events.
pub struct Meter {
    /// The image's size in bytes, when `du` could say; otherwise progress is
    /// reported in rsync's own percent.
    total: Option<u64>,
    last: Option<u64>,
}

impl Meter {
    pub fn new(total: Option<u64>) -> Self {
        Self { total, last: None }
    }

    /// The (done, total) to report for a progress line, or `None` when it
    /// would say nothing new.
    pub fn update(&mut self, bytes: u64, percent: u8) -> Option<(u64, u64)> {
        let (done, total, whole) = match self.total {
            Some(total) => {
                let done = bytes.min(total);
                (done, total, done.saturating_mul(100) / total)
            }
            None => (u64::from(percent), 100, u64::from(percent)),
        };
        if self.last == Some(whole) {
            return None;
        }
        self.last = Some(whole);
        Some((done, total))
    }

    /// The last event: everything copied.
    pub fn finish(&self) -> (u64, u64) {
        let total = self.total.unwrap_or(100);
        (total, total)
    }
}

/// Copies the image into the target, then puts the kernel where the preset
/// looks for it and makes the directories the copy left out.
pub fn run(install: &mut Install) -> Result<(), Stop> {
    let root = install.root().to_path_buf();
    let source = install.image.path.clone();
    let source_text = run::utf8(&source)?.to_string();
    let root_text = run::utf8(&root)?.to_string();

    let total = run::run(
        "du",
        &["-sb", "--one-file-system", &format!("{source_text}/")],
    )
    .ok()
    .and_then(|output| parse_du(&output));
    if total.is_none() {
        run::log("copy: the image's size is unknown; progress is rsync's percent");
    }
    let mut meter = Meter::new(total);
    let args = rsync_args(&source_text, &root_text);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    {
        let report = &mut *install.report;
        run::run_streaming("rsync", &args, &mut |line| match parse_progress(line) {
            Some((bytes, percent)) => {
                if let Some((done, total)) = meter.update(bytes, percent) {
                    report(Event::Progress {
                        step: Step::Copy,
                        done,
                        total,
                    });
                }
            }
            None => run::log(line),
        })?;
    }
    let (done, total) = meter.finish();
    install.progress(Step::Copy, done, total);

    for (directory, mode) in EMPTY_DIRECTORIES {
        if !target::exists(&root, directory) {
            target::create_dir(&root, directory)?;
            target::set_mode(&root, directory, *mode)?;
        } else if !target::is_dir(&root, directory) {
            return Err(Stop::new(
                Failure::Internal,
                format!("{directory} in the target is not a directory"),
            ));
        }
    }

    let kernel = install.image.boot_kernel();
    target::copy_file(&install.image.kernel_file(), &root, &kernel, 0o644)?;
    run::log(&format!(
        "copy: kernel {} to /{kernel}",
        install.image.kernel.version
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsync_keeps_links_and_owners_and_leaves_out_what_the_target_mounts() {
        let args = rsync_args("/run/archiso/airootfs", "/run/lxb-installer/stage-x/root/");
        for wanted in [
            "-aHAX",
            "--numeric-ids",
            "--one-file-system",
            "--info=progress2",
        ] {
            assert!(args.iter().any(|a| a == wanted), "{wanted}");
        }
        for excluded in [
            "/boot/***",
            "/efi/***",
            "/home/***",
            "/proc/***",
            "/tmp/***",
        ] {
            assert!(args.contains(&format!("--exclude={excluded}")));
        }
        let n = args.len();
        assert_eq!(args[n - 3], "--");
        assert_eq!(args[n - 2], "/run/archiso/airootfs/");
        assert_eq!(args[n - 1], "/run/lxb-installer/stage-x/root/");
    }

    #[test]
    fn progress_lines_are_read_with_or_without_separators() {
        assert_eq!(
            parse_progress("          32,768   0%    0.00kB/s    0:00:00 (xfr#0, to-chk=0/1)"),
            Some((32_768, 0))
        );
        assert_eq!(
            parse_progress(
                "  1,234,567,890  42%  117.74MB/s    0:00:10 (xfr#23456, to-chk=81234/123456)"
            ),
            Some((1_234_567_890, 42))
        );
        assert_eq!(
            parse_progress(
                "  2.960.123.456 100%   98,12MB/s    0:00:28 (xfr#98765, to-chk=0/123456)"
            ),
            Some((2_960_123_456, 100))
        );
        assert_eq!(
            parse_progress("    123456789   4%   12.00MB/s    0:00:09"),
            Some((123_456_789, 4))
        );
    }

    #[test]
    fn other_lines_are_not_progress() {
        for line in [
            "",
            "sending incremental file list",
            "rsync: [sender] send_files failed to open \"/x\": Permission denied (13)",
            "1.23G  42%  117.74MB/s    0:00:10",
            "1,234 142%  1.00MB/s",
            "1,234 42%",
            "total size is 3,000,000  speedup is 1.00",
        ] {
            assert_eq!(parse_progress(line), None, "{line:?}");
        }
    }

    #[test]
    fn du_output_gives_the_size() {
        assert_eq!(
            parse_du("3123456789\t/run/archiso/airootfs/\n"),
            Some(3_123_456_789)
        );
        assert_eq!(parse_du("0\t/empty\n"), None);
        assert_eq!(parse_du("du: cannot access"), None);
    }

    #[test]
    fn the_meter_reports_each_whole_percent_once() {
        let mut meter = Meter::new(Some(1000));
        assert_eq!(meter.update(0, 0), Some((0, 1000)));
        assert_eq!(meter.update(5, 0), None);
        assert_eq!(meter.update(10, 1), Some((10, 1000)));
        assert_eq!(meter.update(19, 1), None);
        // More bytes than du counted never reports past the total.
        assert_eq!(meter.update(5000, 100), Some((1000, 1000)));
        assert_eq!(meter.finish(), (1000, 1000));

        let mut by_percent = Meter::new(None);
        assert_eq!(by_percent.update(123, 7), Some((7, 100)));
        assert_eq!(by_percent.update(456, 7), None);
        assert_eq!(by_percent.finish(), (100, 100));
    }
}
