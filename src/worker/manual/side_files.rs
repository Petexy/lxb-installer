//! **The shrink's tools on side files** (§7.0, Unit Q, step Q8): the real
//! e2fsprogs and ntfsprogs carry [`resize`](super::resize)'s commands out on
//! image files the test made, and the measure entry's own commands and
//! parsers read what they leave.
//!
//! Every test here runs a file system tool, so each is `#[ignore]`d and run
//! by name, by hand, never by a plain `cargo test` (makepkg's `check()`
//! inside a build container that may share the host's `/dev`, among them) —
//! and **only through the sandbox of `scripts/side-files.sh`**, which builds
//! the test binary first and then has cargo start the one test inside
//! bubblewrap: a `/dev` of null, zero and urandom alone, no `/sys`, no
//! `/run`, every host path an overlay (so `/proc/mounts` names no device for
//! the tools' own mounted checks to look up), `/proc/swaps` and
//! `/proc/partitions` empty, and the images made on a tmpfs, the one place
//! written:
//!
//! ```sh
//! scripts/side-files.sh ext4     # the first test below
//! scripts/side-files.sh ntfs     # the second; --strace FILE for a record
//! ```
//!
//! Each tool is on `run::fence::ON_SIDE_FILES` by name, which the test
//! build's fence runs only for a binary asked for the tests set aside, and
//! only while the thread holds `images::Permit` for that program and that
//! image, every argument one of the words its tests give it
//! (`run::fence::side_file_words`: no blkid tag, no relative name, no
//! request file) and the image one the test made; every command goes
//! through `Image`'s own refusal first — the image the one path any
//! argument names, given once, and nothing in a script naming a path. A
//! machine without one of the tools skips the test, saying so.
//!
//! What they hold, on the tools of the machine they run on (e2fsprogs
//! 1.47.4 and ntfsprogs 2026.9.28 when written, the 3.0 ISO's), before
//! QEMU's `probe-resize-tools` holds them on the live system:
//!
//! - **ext4**: made by the worker's own `mkfs.ext4 -F`, filled by `debugfs`
//!   so that a file must move for the shrink (and a canary written at its
//!   start), an error made that `e2fsck -f -n` must find (4: a check due)
//!   and `-f -y` put right (1: goes on); then the shrink's dry run and its
//!   commands, word for word, each ending as its rule wants; the size read
//!   back inside the new entry, the canary moved and intact, `-f -n` clean,
//!   and the filesystem grown back and clean again — read throughout by the
//!   measure's own `dumpe2fs -h` and `resize2fs -P` parsers;
//! - **NTFS**: made by `mkntfs`; read by the measure's own `ntfsresize
//!   --info` and parser, and its read-only sign of hibernation through its
//!   own head starter (no `hiberfil.sys`: absent, ntfscat's own words for
//!   it and nothing else — which the measure answers unreadable, as it
//!   does a Windows' data volume, since only a journal check could say
//!   whether a hibernated Windows keeps it); the dry run leaving every
//!   byte as it was; the shrink, its volume read back from its boot sector
//!   inside the new entry; then marked for Windows' check (what Done says),
//!   so the measure reads a check due and the sign unreadable, and the
//!   shrink's own flags refuse it with every byte as it was — where
//!   `--force` with nothing on standard input would have shrunk it (why it
//!   is never given).
use super::resize::{ntfs_volume, Judge, Shrink, Tool, Verdict};
use crate::model::disks::{GIB, MIB};
use crate::model::layout::Measure;
use crate::worker::measure::{self, ext4_least, hibernation, Ext4Header, Hibernation, Least, Ran};
use crate::worker::partition::images::{Image, IMAGES};
use crate::worker::partition::script::Command;
use crate::worker::run::{command_path, Status};

/// The ext4 test's tools, each on `ON_SIDE_FILES`.
const EXT4_TOOLS: &[&str] = &["mkfs.ext4", "debugfs", "dumpe2fs", "e2fsck", "resize2fs"];

/// The NTFS test's tools, each on `ON_SIDE_FILES`.
const NTFS_TOOLS: &[&str] = &["mkntfs", "ntfsresize", "ntfscat"];

/// Whether this machine has every one of `tools` and room for an image;
/// says why not when it has not.
fn can_run(tools: &[&str]) -> bool {
    if let Some(stop) = tools.iter().find_map(|tool| command_path(tool).err()) {
        eprintln!("skipped: {}", stop.detail);
        return false;
    }
    if let Err(error) = std::fs::create_dir_all(IMAGES) {
        eprintln!("skipped: cannot make {IMAGES}: {error}");
        return false;
    }
    true
}

fn command(program: &'static str, args: &[&str]) -> Command {
    Command::new(program, args)
}

/// What a command said, as the measure entry's parsers take it.
fn ran(status: &Status) -> Ran {
    Ran {
        success: status.code == 0,
        stdout: status.stdout.clone(),
        stderr: status.stderr.clone(),
    }
}

/// One debugfs request on `image`, which must end well and say nothing on
/// standard error but its own banner; what it printed.
fn debugfs(image: &Image, write: bool, request: &str) -> String {
    let mut args = Vec::new();
    if write {
        args.push("-w");
    }
    args.extend(["-R", request, image.text.as_str()]);
    let status = image.run_status(&command("debugfs", &args));
    let complaints: Vec<&str> = status
        .stderr
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with("debugfs "))
        .collect();
    assert!(
        status.code == 0 && complaints.is_empty(),
        "debugfs {request}: {status:?}"
    );
    status.stdout
}

/// The physical block debugfs maps the file `name`'s logical block `block`
/// to.
fn physical(image: &Image, name: &str, block: u64) -> u64 {
    let said = debugfs(image, false, &format!("bmap {name} {block}"));
    said.trim()
        .parse()
        .unwrap_or_else(|_| panic!("bmap {name} {block}: {said:?}"))
}

fn header(image: &Image) -> Ext4Header {
    let status = image.run_status(&command("dumpe2fs", &["-h", &image.text]));
    assert_eq!(status.code, 0, "{status:?}");
    Ext4Header::parse(&status.stdout).unwrap_or_else(|| panic!("{}", status.stdout))
}

fn e2fsck(image: &Image, flags: &[&str]) -> Status {
    let mut args = flags.to_vec();
    args.push(&image.text);
    image.run_status(&command("e2fsck", &args))
}

/// **An ext4 on a side file is shrunk by its commands, checks clean and
/// grows back** (the module's first item).
#[test]
#[ignore = "runs e2fsprogs on an image file: run it by name with --ignored"]
fn an_ext4_on_a_side_file_is_shrunk_by_its_commands_checks_clean_and_grows_back() {
    if !can_run(EXT4_TOOLS) {
        return;
    }
    const BLOCK: u64 = 4096;
    let image = Image::create("side-ext4", GIB, 512, EXT4_TOOLS);
    let path = image.text.clone();
    // The worker's own command for an ext4 with no label (`plan::make`).
    let made = image.run_status(&command("mkfs.ext4", &["-F", &path]));
    assert_eq!(made.code, 0, "{made:?}");
    let empty = header(&image);
    assert_eq!(
        (empty.block_size, empty.block_count, empty.clean),
        (BLOCK, GIB / BLOCK, true)
    );

    // Filled: "head" takes 70,000 blocks from the start, "tail" 50 MiB
    // after it with a canary at both ends; then "head" goes, leaving "tail"
    // past where the filesystem will end.
    let head_blocks = 70_000u64;
    let tail_blocks = 12_800u64;
    for (name, blocks) in [("head", head_blocks), ("tail", tail_blocks)] {
        debugfs(&image, true, &format!("mknod {name} p"));
        debugfs(&image, true, &format!("sif {name} mode 0100644"));
        debugfs(&image, true, &format!("fallocate {name} 0 {}", blocks - 1));
        debugfs(&image, true, &format!("sif {name} size {}", blocks * BLOCK));
    }
    debugfs(&image, true, "zap_block -f tail -p 0x5a 0");
    debugfs(
        &image,
        true,
        &format!("zap_block -f tail -p 0xa5 {}", tail_blocks - 1),
    );
    debugfs(&image, true, "rm head");
    let to = 256 * MIB;
    let new_end = to / BLOCK;
    let tail_at = physical(&image, "tail", 0);
    assert!(
        tail_at >= new_end,
        "the canary at block {tail_at} would not have to move below {new_end}"
    );

    // An error e2fsck must find: a link count wrong by one. The dry run's
    // flags find it and stop (4); the repair's put it right and go on (1);
    // then the dry run is clean (0). Each by its rule.
    debugfs(&image, true, "sif tail links_count 2");
    let found = e2fsck(&image, super::resize::E2FSCK_LOOK);
    assert_eq!(found.code & 4, 4, "{found:?}");
    assert_eq!(Judge::E2fsckLook.verdict(&found), Verdict::NeedsCheck);
    let fixed = e2fsck(&image, super::resize::E2FSCK_REPAIR);
    eprintln!(
        "record: e2fsck -f -n ended {} on the error, -f -y ended {}",
        found.code, fixed.code
    );
    assert_eq!(fixed.code, 1, "{fixed:?}");
    assert_eq!(Judge::E2fsckRepair.verdict(&fixed), Verdict::GoOn);
    let clean = e2fsck(&image, super::resize::E2FSCK_LOOK);
    assert_eq!(clean.code, 0, "{clean:?}");

    // The measure's own reading of it: its header and the least it shrinks
    // to, by the measure's parsers on the real tools' words.
    let filled = header(&image);
    assert!(filled.clean);
    let least = image.run_status(&command("resize2fs", &["-P", &path]));
    let Least::Blocks(least) = ext4_least(&ran(&least)) else {
        panic!("resize2fs -P: {least:?}");
    };
    assert!(least * BLOCK < to, "least {least} blocks");
    assert!(filled.used() >= tail_blocks * BLOCK);

    // The shrink: phase A's dry run, then phase B.1, word for word as
    // `resize` builds them for this image in place of a partition.
    let shrink = Shrink {
        device: path.clone(),
        tool: Tool::Ext4,
        from: GIB,
        to,
        filesystem: filled.bytes(),
    };
    let dry = shrink.dry_run();
    assert_eq!(dry.command.args, ["-f", "-n", path.as_str()]);
    let status = image.run_status(&dry.command);
    assert_eq!(dry.judge.verdict(&status), Verdict::GoOn, "{status:?}");
    for step in shrink.steps() {
        let status = image.run_status(&step.command);
        assert_eq!(
            step.judge.verdict(&status),
            Verdict::GoOn,
            "{:?}: {status:?}",
            step.command
        );
    }

    // Read back: inside the new entry, exactly as asked; clean; the canary
    // moved below the new end, both its ends intact.
    let shrunk = header(&image);
    assert_eq!(shrunk.bytes(), to);
    assert!(shrink.fits(shrunk.bytes()));
    assert!(shrunk.clean);
    let clean = e2fsck(&image, super::resize::E2FSCK_LOOK);
    assert_eq!(clean.code, 0, "{clean:?}");
    let moved = physical(&image, "tail", 0);
    eprintln!(
        "record: used {} bytes, resize2fs -P least {least} blocks; the canary at block \
         {tail_at}, moved to {moved}, the new end {new_end}; {} bytes after the shrink",
        filled.used(),
        shrunk.bytes()
    );
    assert!(moved < new_end, "the canary is at block {moved}");
    let first = debugfs(&image, false, "bd -f tail 0");
    assert!(first.contains("5a5a 5a5a 5a5a 5a5a"), "{first}");
    let last = debugfs(&image, false, &format!("bd -f tail {}", tail_blocks - 1));
    assert!(last.contains("a5a5 a5a5 a5a5 a5a5"), "{last}");

    // Grown back to the whole image: as large as it was, and clean.
    let grow = image.run_status(&command("resize2fs", &[&path, "1048576K"]));
    assert_eq!(Judge::Resize2fs.verdict(&grow), Verdict::GoOn, "{grow:?}");
    let grown = header(&image);
    assert_eq!(grown.block_count, GIB / BLOCK);
    let clean = e2fsck(&image, super::resize::E2FSCK_LOOK);
    assert_eq!(clean.code, 0, "{clean:?}");
    let first = debugfs(&image, false, "bd -f tail 0");
    assert!(first.contains("5a5a 5a5a 5a5a 5a5a"), "{first}");
}

/// The size an NTFS boot sector gives its volume, in bytes, read as the
/// worker reads a shrunk NTFS back (`resize::ntfs_volume`).
fn volume_bytes(image: &Image) -> u64 {
    let boot = image.bytes(512);
    ntfs_volume(&boot).unwrap_or_else(|| panic!("no NTFS boot sector: {:?}", &boot[..16]))
}

fn ntfs_info(image: &Image) -> Measure {
    let info = image.run_status(&measure_command("ntfsresize", &image.text));
    measure::ntfs_measure(&ran(&info), GIB)
}

/// The measure entry's own command lines for `device`, with this image in
/// its place.
fn measure_command(program: &str, device: &str) -> Command {
    match program {
        "ntfsresize" => command("ntfsresize", &["--info", "--no-progress-bar", device]),
        "ntfscat" => command("ntfscat", &[device, "hiberfil.sys"]),
        other => panic!("{other} is not one of the measure's NTFS commands"),
    }
}

/// **An NTFS on a side file is read as the measure reads it and shrunk by
/// its commands** (the module's second item).
#[test]
#[ignore = "runs ntfsprogs on an image file: run it by name with --ignored"]
fn an_ntfs_on_a_side_file_is_read_as_the_measure_reads_it_and_shrunk_by_its_commands() {
    if !can_run(NTFS_TOOLS) {
        return;
    }
    let image = Image::create("side-ntfs", GIB, 512, NTFS_TOOLS);
    let path = image.text.clone();
    let made = image.run_status(&command("mkntfs", &["-F", "-Q", &path]));
    assert_eq!(made.code, 0, "{made:?}");
    let volume = volume_bytes(&image);
    assert!((GIB - 4096..GIB).contains(&volume), "{volume}");

    // The measure's reading: ntfsresize's numbers, then its sign of
    // hibernation through its own starter — no hiberfil.sys on a volume
    // mkntfs made, which ntfscat says in the words the parser knows, and
    // nothing else (the parser takes them for no file only when they are
    // all ntfscat said). Absent, the measure answers it unreadable; the
    // tools are carried on below all the same, on this side file.
    let Measure::Read { used, free, least } = ntfs_info(&image) else {
        panic!("{:?}", ntfs_info(&image));
    };
    // ntfsresize counts a volume as its clusters and one sector more, for
    // the backup boot sector (its `vol_size`): what the rule set reads as
    // the filesystem's own size. The boot sector's own count runs past the
    // clusters to the partition's last sector but one, where mkntfs (and
    // Windows) put the backup — so only the boot sector says whether an
    // entry made smaller holds the volume whole (`shrinking::ready`).
    eprintln!(
        "record: ntfsresize counts the volume {} bytes, its boot sector {volume}",
        used + free
    );
    assert_eq!(used + free, volume / 4096 * 4096 + 512, "{volume}");
    assert!(used < 64 * MIB, "{used}");
    let least = least.expect("ntfsresize names a least");
    let sign = image.head(&measure_command("ntfscat", &path));
    eprintln!("record: ntfscat said {:?} of no hiberfil.sys", sign.stderr);
    assert_eq!(sign.bytes, b"");
    assert_eq!(sign.ended, Some(false), "{sign:?}");
    let said: Vec<&str> = sign
        .stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(said, [measure::ABSENT], "{sign:?}");
    assert_eq!(hibernation(&sign), Hibernation::Absent);

    // The shrink: phase A's dry run leaves every byte as it was.
    let to = 512 * MIB;
    assert!(least < to);
    let shrink = Shrink {
        device: path.clone(),
        tool: Tool::Ntfs,
        from: GIB,
        to,
        filesystem: volume,
    };
    let before = image.digest();
    let dry = shrink.dry_run();
    let status = image.run_status(&dry.command);
    assert_eq!(dry.judge.verdict(&status), Verdict::GoOn, "{status:?}");
    assert_eq!(image.digest(), before, "the dry run wrote to the volume");

    // Phase B.1, its question answered on standard input, never --force.
    for step in shrink.steps() {
        let status = image.run_status(&step.command);
        assert_eq!(
            step.judge.verdict(&status),
            Verdict::GoOn,
            "{:?}: {status:?}",
            step.command
        );
        assert!(status.stdout.contains("Are you sure"), "{status:?}");
    }
    let shrunk = volume_bytes(&image);
    eprintln!(
        "record: the volume {volume} bytes, {used} in use, least {least}; \
         {shrunk} bytes after the shrink to {to}"
    );
    assert!(shrink.fits(shrunk), "{shrunk} for an entry of {to}");
    assert!(shrunk >= to - 4096, "{shrunk}");

    // Left marked for Windows' own check: the measure reads a check due,
    // and its sign cannot be read — never "not hibernated".
    assert_eq!(ntfs_info(&image), Measure::NeedsCheck);
    let sign = image.head(&measure_command("ntfscat", &path));
    assert!(
        matches!(hibernation(&sign), Hibernation::Unreadable(_)),
        "{sign:?}"
    );

    // Marked, it is refused by the shrink's own flags before anything is
    // written: a check due, every byte as it was.
    let marked = image.digest();
    let again = Shrink {
        to: 448 * MIB,
        filesystem: shrunk,
        ..shrink.clone()
    };
    for step in again.steps() {
        let status = image.run_status(&step.command);
        assert_eq!(
            step.judge.verdict(&status),
            Verdict::NeedsCheck,
            "{status:?}"
        );
        assert!(
            status.stderr.contains("Volume is scheduled for check")
                || status.stdout.contains("Volume is scheduled for check"),
            "{status:?}"
        );
    }
    assert_eq!(image.digest(), marked, "a marked volume was written to");
    assert_eq!(volume_bytes(&image), shrunk);

    // Why never --force: one --force, spent on the mark, and nothing on
    // standard input to answer the question it then asks — and ntfsresize
    // shrinks the marked volume all the same.
    let size = (448 * MIB).to_string();
    let forced = image.run_status(&command(
        "ntfsresize",
        &["--force", "--no-progress-bar", "--size", &size, &path],
    ));
    assert_eq!(forced.code, 0, "{forced:?}");
    assert!(forced.stdout.contains("Are you sure"), "{forced:?}");
    assert!(
        forced
            .stdout
            .contains("Successfully resized NTFS on device"),
        "{forced:?}"
    );
    eprintln!(
        "record: --force with nothing on standard input left the marked volume {} bytes",
        volume_bytes(&image)
    );
    assert!(volume_bytes(&image) < shrunk);
}
