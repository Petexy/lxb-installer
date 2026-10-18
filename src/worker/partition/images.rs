//! The plans carried out by the real sfdisk, on image files.
//!
//! For every plan the fixtures allow (ext4, no swap file, no separate /home:
//! the other choices never change the table; beside a shared EFI system
//! partition, once with room for the kernels in it and once without, which
//! adds a Linux boot partition), at 512 and 4096 byte sectors:
//! a sparse image file is made under this tree's `artifacts/`, the fixture's
//! table is written into it, the plan's own commands run on it exactly as
//! the worker runs them on a disk (with the image's sector size stated, as
//! a regular file has none of its own), and the table sfdisk then reads back
//! must be the plan. Nothing but those image files is ever given to a
//! partitioning tool, and they are removed again. Without sfdisk the tests
//! say why they did nothing and pass.
//!
//! Two fences stand between a test here and a disk. [`Image::run`] runs a
//! command only when the image is its one argument naming a path (given
//! once, absolute, with no `..`), nothing else in it or in its script names
//! a path, the program is on the test's own list, and the effective user is
//! not root; and then only once [`Image::guard`] has seen the image is still
//! the regular file the test made. And the test build's fence in
//! `run::fence`, which every program the worker's code starts passes
//! through, runs sfdisk and wipefs for no thread but one holding a
//! [`Permit`], which `Image::run` takes for that one call and which names
//! the program and the image.
//!
//! A test that runs a file system tool (`mkfs.ext4`, `e2fsck`, `resize2fs`,
//! `ntfsresize` …) on an image file is `#[ignore]`d and run by name, by
//! hand, only inside the sandbox of `scripts/side-files.sh`, so makepkg's
//! `check()` inside a build container that may share the host's `/dev` never
//! runs it and no result depends on which tools a machine has; the fence
//! refuses those tools to a test binary not asked for the tests set aside
//! (`run::fence::ON_SIDE_FILES`; the tests are `manual::side_files`, which
//! use [`Image::run_status`] and [`Image::head`] beside [`Image::run`]). The
//! partitioning tests below need only sfdisk and wipefs, which every machine
//! that builds the installer has, and run in every `cargo test`.
use super::exec::run_command;
use super::fixtures::{self, config, extent, part_path, targets};
use super::matrix::{Outcome::Planned, EXPECTED};
use super::plan::{plan, Change, EspPlan, Plan, Role, SharedRoom};
use super::script::{activation, table_commands, Command};
use super::table::{check_before_writing, find_created, find_kept, parse_sfdisk, Kernel, OnDisk};
use crate::model::disks::{Disk, Table};
use crate::worker::layout::Firmware;
use crate::worker::measure::{self, Head};
use crate::worker::run::{self, command_path, fence, Status};
use std::path::{Path, PathBuf};

/// Where the images are made: this tree's own scratch directory, which git
/// ignores.
pub(in crate::worker) const IMAGES: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/artifacts/storage-tests");

/// The programs the partitioning tests run on their images: the ones the
/// worker writes a table with.
pub(in crate::worker) const TABLE_TOOLS: &[&str] = &["sfdisk", "wipefs"];

/// How many plans the fixtures allow at each sector size: every row the
/// matrix expects to be planned, under each firmware it is planned for, and
/// under UEFI twice where the place is beside the fixture's own EFI system
/// partition (with room for the kernels in it and without).
fn planned_count() -> usize {
    let with_esp: Vec<&str> = fixtures::all(512)
        .into_iter()
        .filter(|(_, disk)| disk.esp().is_some())
        .map(|(name, _)| name)
        .collect();
    EXPECTED
        .iter()
        .map(|(fixture, target, uefi, bios)| {
            let shared = *target != "whole" && with_esp.contains(fixture);
            usize::from(*uefi == Planned) * (1 + usize::from(shared))
                + usize::from(*bios == Planned)
        })
        .sum()
}

/// Leave for this thread to run one program on one image file, for the one
/// call [`Image::run`] makes once its own checks have passed; dropped, it
/// ends. The test build's fence (`run::fence`) runs sfdisk and wipefs for no
/// thread without one, and holds the command line to the program and the
/// image it names. It is the thread's, not the test's: the image tests carry
/// their fixtures out side by side, each on a thread of its own.
pub(crate) struct Permit(());

impl Permit {
    fn new(program: &'static str, image: &str) -> Permit {
        fence::grant(program, image);
        Permit(())
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        fence::withdraw();
    }
}

/// A sparse image file this test made, removed again when it is dropped.
/// The hand-made layout's tests (`manual::matrix`) carry their commands out
/// on it too, through the same [`Image::run`].
pub(in crate::worker) struct Image {
    path: PathBuf,
    pub(in crate::worker) text: String,
    pub(in crate::worker) sector: u64,
    /// The programs this test runs on the image: its own list, from the
    /// tools the fence knows for image files.
    programs: &'static [&'static str],
}

impl Image {
    pub(in crate::worker) fn create(
        name: &str,
        size: u64,
        sector: u64,
        programs: &'static [&'static str],
    ) -> Image {
        assert!(!name.contains('/') && !name.contains(".."), "{name}");
        assert!(
            programs
                .iter()
                .all(|p| fence::ON_IMAGES.contains(p) || fence::ON_SIDE_FILES.contains(p)),
            "{programs:?}"
        );
        let path = Path::new(IMAGES).join(format!("{name}-{}.img", std::process::id()));
        // Only a leftover of this same test, interrupted, can be there;
        // `create_new` refuses anything that appears in between.
        let _ = std::fs::remove_file(&path);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.set_len(size).unwrap();
        let text = path.to_str().expect("the image path is UTF-8").to_string();
        Image {
            path,
            text,
            sector,
            programs,
        }
    }

    /// Stops the test unless the path is still the regular file it made,
    /// directly inside the image directory.
    fn guard(&self) {
        let meta = std::fs::symlink_metadata(&self.path).unwrap();
        assert!(
            meta.file_type().is_file(),
            "{} is not a regular file",
            self.text
        );
        let dir = std::fs::canonicalize(IMAGES).unwrap();
        let real = std::fs::canonicalize(&self.path).unwrap();
        assert_eq!(real.parent(), Some(dir.as_path()), "{} moved", self.text);
    }

    /// Why `command` may not run on this image, or `None` when it may. It
    /// may only when the effective user is not root; the program is on this
    /// test's list; the image is a plain absolute path, and the one argument
    /// naming a path at all, given once (so neither a relative path, nor one
    /// glued to an option, `--opt=/x`, `-O/x`, can name another file); a
    /// side-file tool's every word is one of its tests' own (no blkid tag,
    /// no relative name, no request file — the fence's judge); no argument
    /// holds `..`; and its script, if it has one, names no path either (none
    /// of the planner's scripts does, nor needs to).
    fn refusal(&self, command: &Command) -> Option<String> {
        let paths: Vec<&String> = command.args.iter().filter(|a| a.contains('/')).collect();
        let why = if fence::euid() == 0 {
            "no image test runs as root".to_string()
        } else if !self.programs.contains(&command.program) {
            format!("{} is not on this test's list", command.program)
        } else if !self.text.starts_with('/') || self.text.contains("..") {
            format!("the image {:?} is not a plain absolute path", self.text)
        } else if paths != [&self.text] {
            format!("the image must be the one path given, once, not {paths:?}")
        } else if let Some(why) = self.side_file_refusal(command) {
            why
        } else if command.args.iter().any(|a| a.contains("..")) {
            "an argument holds ..".to_string()
        } else if command
            .input
            .as_deref()
            .is_some_and(|input| input.contains('/') || input.contains(".."))
        {
            "its script names a path".to_string()
        } else {
            return None;
        };
        Some(why)
    }

    /// Why a side-file tool may not be given `command`'s words
    /// (`run::fence::side_file_words`, the fence's own judge, asked here
    /// first too): `None` for any other program.
    fn side_file_refusal(&self, command: &Command) -> Option<String> {
        if !fence::ON_SIDE_FILES.contains(&command.program) {
            return None;
        }
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        fence::side_file_words(command.program, &args, &self.text).err()
    }

    /// Runs one partitioning command with this image as the only file it is
    /// given.
    pub(in crate::worker) fn run(&self, command: &Command) -> String {
        if let Some(why) = self.refusal(command) {
            panic!("test build: {why}: {command:?}");
        }
        self.guard();
        let mut command = command.clone();
        if command.program == "sfdisk" {
            command
                .args
                .splice(0..0, ["--sector-size".to_string(), self.sector.to_string()]);
        }
        let _permit = Permit::new(command.program, &self.text);
        run_command(&command).unwrap_or_else(|stop| panic!("{command:?}: {}", stop.detail))
    }

    /// Runs one command as [`run`](Self::run) runs it — the same refusal,
    /// guard and permit — and answers how it ended, whatever its code
    /// (`run::run_status`): for the tools whose exit code is their answer.
    pub(in crate::worker) fn run_status(&self, command: &Command) -> Status {
        if let Some(why) = self.refusal(command) {
            panic!("test build: {why}: {command:?}");
        }
        self.guard();
        let _permit = Permit::new(command.program, &self.text);
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        let input = command.input.as_deref().unwrap_or("").as_bytes();
        run::run_status(command.program, &args, input)
            .unwrap_or_else(|stop| panic!("{command:?}: {}", stop.detail))
    }

    /// Runs one command as [`run`](Self::run) runs it, through the measure
    /// entry's own starter for a command whose output is cut short
    /// (`measure::head_of`: at most `HIBERFILE_HEAD` bytes, the program then
    /// ended).
    pub(in crate::worker) fn head(&self, command: &Command) -> Head {
        if let Some(why) = self.refusal(command) {
            panic!("test build: {why}: {command:?}");
        }
        self.guard();
        let _permit = Permit::new(command.program, &self.text);
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        measure::head_of(command.program, &args)
            .unwrap_or_else(|stop| panic!("{command:?}: {}", stop.detail))
    }

    /// The first `len` bytes of the image file, once it is still the file
    /// the test made.
    pub(in crate::worker) fn bytes(&self, len: usize) -> Vec<u8> {
        use std::io::Read;
        self.guard();
        let mut bytes = Vec::new();
        std::fs::File::open(&self.path)
            .unwrap()
            .take(len as u64)
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    }

    /// A digest of every byte of the image file, to see a command left it
    /// as it was.
    pub(in crate::worker) fn digest(&self) -> u64 {
        use std::hash::Hasher;
        use std::io::Read;
        self.guard();
        let mut file = std::fs::File::open(&self.path).unwrap();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut chunk = vec![0u8; 1 << 20];
        loop {
            let read = file.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            hasher.write(&chunk[..read]);
        }
        hasher.finish()
    }

    pub(in crate::worker) fn sfdisk(&self, args: &[&str], input: Option<String>) -> String {
        let mut args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        args.push(self.text.clone());
        self.run(&Command {
            program: "sfdisk",
            args,
            input,
        })
    }

    pub(in crate::worker) fn read(&self) -> OnDisk {
        parse_sfdisk(&self.sfdisk(&["--json"], None)).unwrap()
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The partition marked active in a fixture's starting table: Windows marks
/// its own first partition on an MBR disk, and the installer must leave the
/// mark there.
fn active(fixture: &str) -> Option<u32> {
    (fixture == "msdos").then_some(1)
}

/// Writes the fixture's table into the image and reads it back, checking
/// that the image now holds the fixture.
fn lay_down(image: &Image, disk: &Disk, active: Option<u32>) -> OnDisk {
    let sector = image.sector;
    let label = match disk.table.expect("the fixture has a table") {
        Table::Gpt => "gpt",
        Table::Msdos => "dos",
    };
    let mut text = format!("label: {label}\nunit: sectors\nsector-size: {sector}\n");
    for part in &disk.partitions {
        let span = extent(disk, part, sector);
        let part_type = part.part_type.trim_start_matches("0x");
        text.push_str(&format!(
            "start={}, size={}, type={part_type}",
            span.start,
            span.len()
        ));
        if active == Some(part.number) {
            text.push_str(", bootable");
        }
        text.push('\n');
    }
    image.sfdisk(&["--lock"], Some(text));
    let before = image.read();
    let places: Vec<_> = before
        .partitions
        .iter()
        .map(|e| (e.number, e.start, e.size))
        .collect();
    let fixture: Vec<_> = fixtures::on_disk(disk)
        .partitions
        .iter()
        .map(|e| (e.number, e.start, e.size))
        .collect();
    assert_eq!(places, fixture, "the image holds the fixture");
    before
}

/// One partition the table must hold afterwards. A new partition's number
/// is libfdisk's choice; a kept one keeps its own.
struct Want {
    number: Option<u32>,
    start: u64,
    size: u64,
    part_type: String,
    name: String,
    bootable: bool,
}

/// The table the plan describes, from the table before it.
fn wanted(plan: &Plan, before: Option<&OnDisk>, root_number: u32) -> Vec<Want> {
    let gpt = plan.table == Table::Gpt;
    let name = |role: Role| if gpt { role.name() } else { "" }.to_string();
    let replaced = match &plan.change {
        Change::Retype { number, .. } | Change::Replace { number, .. } => Some(*number),
        Change::Fresh | Change::Add => None,
    };
    let kept = match plan.change {
        Change::Fresh => &[][..],
        _ => &before.expect("a kept table").partitions[..],
    };
    // On a kept msdos table root is marked active only when no partition
    // that stays is marked already, and only when it is a primary one; a
    // fresh table marks it in the same write.
    let others_active = kept
        .iter()
        .any(|e| e.bootable && Some(e.number) != replaced);
    let root_was_active = kept.iter().any(|e| {
        e.bootable && matches!(plan.change, Change::Retype { number, .. } if number == e.number)
    });
    let root_active = plan.table == Table::Msdos
        && (plan.change == Change::Fresh
            || root_was_active
            || (!others_active && (1..=4).contains(&root_number)));

    let mut want = Vec::new();
    for entry in kept {
        match &plan.change {
            Change::Replace { number, .. } if entry.number == *number => {}
            Change::Retype { number, .. } if entry.number == *number => want.push(Want {
                number: Some(entry.number),
                start: entry.start,
                size: entry.size,
                part_type: Role::Root.part_type(plan.table).to_string(),
                name: name(Role::Root),
                bootable: root_active,
            }),
            _ => want.push(Want {
                number: Some(entry.number),
                start: entry.start,
                size: entry.size,
                part_type: entry.part_type.clone(),
                name: entry.name.clone(),
                bootable: entry.bootable,
            }),
        }
    }
    for part in &plan.create {
        want.push(Want {
            number: None,
            start: part.start,
            size: part.size,
            part_type: part.role.part_type(plan.table).to_string(),
            name: name(part.role),
            bootable: part.role == Role::Root && root_active,
        });
    }
    want.sort_by_key(|w| w.start);
    want
}

fn compare(case: &str, plan: &Plan, after: &OnDisk, want: &[Want]) {
    assert_eq!(after.table, plan.table, "{case}: the table");
    assert_eq!(after.sector, Some(plan.sector), "{case}: the sector size");
    let mut got: Vec<_> = after.partitions.iter().collect();
    got.sort_by_key(|e| e.start);
    assert_eq!(got.len(), want.len(), "{case}: {got:#?}");
    for (got, want) in got.iter().zip(want) {
        assert_eq!(
            (got.start, got.size),
            (want.start, want.size),
            "{case}: a partition's place"
        );
        assert!(
            got.part_type.eq_ignore_ascii_case(&want.part_type),
            "{case}: type {} where {} was planned",
            got.part_type,
            want.part_type
        );
        assert_eq!(got.name, want.name, "{case}: name at {}", got.start);
        assert_eq!(
            got.bootable, want.bootable,
            "{case}: boot flag at {}",
            got.start
        );
        if let Some(number) = want.number {
            assert_eq!(
                got.number, number,
                "{case}: a kept partition was renumbered"
            );
        }
    }
}

/// Carries `plan` out on an image of `disk` and checks the result.
fn carry_out(
    fixture: &str,
    disk: &Disk,
    target_name: &str,
    firmware: Firmware,
    shared: SharedRoom,
    plan: &Plan,
) {
    let sector = plan.sector;
    let case = format!("{fixture} {sector}B {firmware:?} {target_name} {shared:?}");
    let name = format!("{fixture}-{sector}-{firmware:?}-{target_name}-{shared:?}").to_lowercase();
    let image = Image::create(&name, disk.size, sector, TABLE_TOOLS);
    let before = disk.table.map(|_| lay_down(&image, disk, active(fixture)));

    // What the worker does: check the table, write, find root by its place,
    // mark it active when a kept msdos table wants that.
    if plan.change != Change::Fresh {
        check_before_writing(plan, before.as_ref().unwrap())
            .unwrap_or_else(|e| panic!("{case}: {e}"));
    }
    for command in table_commands(plan, &image.text) {
        image.run(&command);
    }
    let mut after = image.read();
    let root_number = match &plan.change {
        Change::Retype { number, .. } => *number,
        _ => {
            let root = plan.new_root().unwrap();
            after
                .partitions
                .iter()
                .find(|e| e.start == root.start && e.size == root.size)
                .unwrap_or_else(|| panic!("{case}: root is not where it was planned"))
                .number
        }
    };
    if plan.wants_boot_flag() && plan.change != Change::Fresh {
        if let Some(command) = activation(plan, &after, root_number, &image.text) {
            image.run(&command);
            after = image.read();
        }
    }

    compare(
        &case,
        plan,
        &after,
        &wanted(plan, before.as_ref(), root_number),
    );

    // The Linux boot partition, spelt out rather than through the planner's
    // own constants: the type the Boot Loader Specification gives XBOOTLDR,
    // first in the space, and never a second EFI system partition.
    if let Some(boot) = plan.created(Role::LinuxBoot) {
        let entry = after
            .partitions
            .iter()
            .find(|e| e.start == boot.start)
            .unwrap_or_else(|| panic!("{case}: no Linux boot partition"));
        let want = match plan.table {
            Table::Gpt => "BC13C2FF-59E6-4262-A352-B275FD6F7172",
            Table::Msdos => "ea",
        };
        assert!(
            entry.part_type.eq_ignore_ascii_case(want),
            "{case}: Linux boot type {}",
            entry.part_type
        );
        assert_eq!(entry.size * sector, 2 * 1024 * 1024 * 1024, "{case}");
        assert_eq!(boot.start, plan.space.start, "{case}");
        let esps = after
            .partitions
            .iter()
            .filter(|e| {
                e.part_type
                    .eq_ignore_ascii_case("C12A7328-F81F-11D2-BA4B-00A0C93EC93B")
                    || e.part_type.eq_ignore_ascii_case("ef")
            })
            .count();
        assert_eq!(esps, 1, "{case}: EFI system partitions");
    }

    // The worker finds the new partitions again by their exact place in the
    // kernel's view; the table's own numbers stand in for the kernel here.
    let kernel: Vec<Kernel> = after
        .partitions
        .iter()
        .map(|e| Kernel {
            path: part_path(&plan.disk, e.number),
            number: e.number,
            start: e.start * sector,
            size: e.size * sector,
        })
        .collect();
    let found = find_created(plan, &kernel)
        .unwrap_or_else(|| panic!("{case}: a new partition is not found again"));
    let roles: Vec<Role> = found.iter().map(|f| f.role).collect();
    let planned: Vec<Role> = plan.create.iter().map(|p| p.role).collect();
    assert_eq!(roles, planned, "{case}");
    if let Change::Retype { number, path, span } = &plan.change {
        assert!(
            find_kept(path, *number, *span, sector, &kernel),
            "{case}: the re-typed root is not found again"
        );
    }
}

fn carry_out_every_plan(sector: u32) {
    if let Err(stop) = command_path("sfdisk") {
        eprintln!("skipped: {}", stop.detail);
        return;
    }
    // A source tree that cannot be written (a packaging sandbox) has no
    // room for images; that is not a fault in what is tested.
    if let Err(error) = std::fs::create_dir_all(IMAGES) {
        eprintln!("skipped: cannot make {IMAGES}: {error}");
        return;
    }
    let wipefs = command_path("wipefs").is_ok();
    // Every write ends in an fsync of the image, so the fixtures are carried
    // out side by side, each in its own images.
    let counts: Vec<(usize, usize)> = std::thread::scope(|scope| {
        let workers: Vec<_> = fixtures::all(sector)
            .into_iter()
            .map(|(fixture, disk)| {
                scope.spawn(move || {
                    let (mut done, mut skipped) = (0, 0);
                    for (target_name, target) in targets(&disk) {
                        for firmware in [Firmware::Uefi, Firmware::Bios] {
                            let mut config = config(disk.clone(), target.clone());
                            config.storage.swap_mib = 0;
                            let Ok(first) = plan(&config, firmware, SharedRoom::Kernels) else {
                                continue;
                            };
                            let mut plans = Vec::new();
                            if matches!(first.esp, EspPlan::Shared { .. }) {
                                let small = plan(&config, firmware, SharedRoom::LoaderOnly)
                                    .expect("beside a small shared ESP");
                                plans.push((SharedRoom::LoaderOnly, small));
                            }
                            plans.insert(0, (SharedRoom::Kernels, first));
                            for (shared, plan) in plans {
                                if plan.change == Change::Fresh && !wipefs {
                                    eprintln!(
                                        "skipped {fixture} {target_name} {firmware:?}: no wipefs"
                                    );
                                    skipped += 1;
                                    continue;
                                }
                                carry_out(fixture, &disk, &target_name, firmware, shared, &plan);
                                done += 1;
                            }
                        }
                    }
                    (done, skipped)
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().unwrap()).collect()
    });
    let done: usize = counts.iter().map(|c| c.0).sum();
    let skipped: usize = counts.iter().map(|c| c.1).sum();
    assert_eq!(done + skipped, planned_count(), "plans carried out");
}

#[test]
fn plans_are_carried_out_by_sfdisk_on_512_byte_sector_images() {
    carry_out_every_plan(512);
}

#[test]
fn plans_are_carried_out_by_sfdisk_on_4096_byte_sector_images() {
    carry_out_every_plan(4096);
}

#[test]
fn image_run_refuses_relative_and_embedded_paths() {
    if let Err(error) = std::fs::create_dir_all(IMAGES) {
        eprintln!("skipped: cannot make {IMAGES}: {error}");
        return;
    }
    let image = Image::create("refusals", 1024 * 1024, 512, &["sfdisk"]);
    let text = image.text.as_str();
    let sfdisk = |args: &[&str], input: Option<&str>| Command {
        program: "sfdisk",
        args: args.iter().map(|a| a.to_string()).collect(),
        input: input.map(str::to_string),
    };
    // The one shape it runs: the image, once, as the only path.
    assert_eq!(image.refusal(&sfdisk(&["--json", text], None)), None);
    assert_eq!(
        image.refusal(&sfdisk(&["--lock", text], Some("label: gpt\n"))),
        None
    );

    let glued = format!("--opt={text}");
    let refused = [
        ("no image", sfdisk(&["--json"], None)),
        ("the image twice", sfdisk(&["--json", text, text], None)),
        (
            "a relative path",
            sfdisk(&["--json", "storage-tests/refusals.img"], None),
        ),
        (
            "a climbing path",
            sfdisk(&["--json", "../../dev/sda"], None),
        ),
        (
            "the image glued to an option",
            sfdisk(&["--json", &glued], None),
        ),
        (
            "a second path glued to an option",
            sfdisk(&["--json", text, "--backup-file=/tmp/lxb-x"], None),
        ),
        (
            "a second path glued to a short option",
            sfdisk(&["--json", text, "-O/tmp/lxb-x"], None),
        ),
        ("two dots", sfdisk(&["--json", text, "-d.."], None)),
        (
            "a path in the script",
            sfdisk(&["--lock", text], Some("device: /dev/sda\n")),
        ),
        (
            "a program off this test's list",
            Command {
                program: "wipefs",
                args: vec!["--all".to_string(), text.to_string()],
                input: None,
            },
        ),
    ];
    for (what, command) in &refused {
        assert!(image.refusal(command).is_some(), "{what}: {command:?}");
    }

    // An image whose own path is relative is refused whatever it is given.
    // (It is never dropped, so nothing is removed by a relative path.)
    let relative = std::mem::ManuallyDrop::new(Image {
        path: PathBuf::from("refusals.img"),
        text: "refusals.img".to_string(),
        sector: 512,
        programs: &["sfdisk"],
    });
    assert!(relative
        .refusal(&sfdisk(&["--json", "refusals.img"], None))
        .is_some());

    {
        let _root = fence::Pretend::root();
        assert!(image.refusal(&sfdisk(&["--json", text], None)).is_some());
    }

    // `run` stops before anything starts. Were it not to, sfdisk would only
    // be asked to read a relative path that does not exist.
    let words = fence::refusal(|| image.run(&sfdisk(&["--json", text, "no-such-folder/x"], None)))
        .expect("refused");
    assert!(words.contains("test build:"), "{words}");
}

/// `scripts/side-files.sh`, the one way the side-file tests run (§5 rule
/// 9), follows the manifest to where [`IMAGES`] makes the images and
/// refuses a folder anywhere but its worktree's `artifacts/storage-tests`
/// and `target/` in its own words — "side-files: …", exit 2 — before it
/// makes a folder, a scratch directory or a sandbox. Bash looks a function
/// up only when a line calls it, so every function the script defines
/// stands above the first line outside a function that calls it: called
/// before it, `refuse` would be "command not found", exit 127, and the
/// check held only by `set -e`. Read from the script's text: running it is
/// no test's business.
#[test]
fn the_side_file_sandbox_refuses_an_image_folder_elsewhere_in_its_own_words() {
    let script = include_str!("../../../scripts/side-files.sh");
    let lines: Vec<&str> = script.lines().collect();
    // A line's code: what stands before a `#` that begins a word (`$#` is
    // code).
    let code = |line: &str| -> String {
        let mut previous = ' ';
        for (at, c) in line.char_indices() {
            if c == '#' && previous.is_whitespace() {
                return line[..at].to_string();
            }
            previous = c;
        }
        line.to_string()
    };
    let names = |code: &str, name: &str| {
        let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
        code.match_indices(name).any(|(at, _)| {
            !code[..at].chars().next_back().is_some_and(word)
                && !code[at + name.len()..].chars().next().is_some_and(word)
        })
    };

    // Every function, by the line that opens it and the `}` that closes it.
    let functions: Vec<(&str, usize, usize)> = lines
        .iter()
        .enumerate()
        .filter_map(|(at, line)| {
            let name = line.strip_suffix("() {")?;
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            let end = at + lines[at..].iter().position(|line| *line == "}")?;
            Some((name, at, end))
        })
        .collect();
    let inside = |line: usize| {
        functions
            .iter()
            .any(|(_, start, end)| (*start..=*end).contains(&line))
    };
    for (name, start, _) in &functions {
        let callers: Vec<usize> = (0..*start)
            .filter(|at| !inside(*at) && names(&code(lines[*at]), name))
            .map(|at| at + 1)
            .collect();
        assert!(
            callers.is_empty(),
            "side-files.sh calls {name} on line {callers:?}, above its definition on line {}",
            start + 1
        );
    }

    // `refuse` says the script's name and what it refused, and ends with 2.
    let (_, start, end) = functions
        .iter()
        .find(|(name, ..)| *name == "refuse")
        .expect("side-files.sh defines refuse");
    let body = &lines[*start..=*end];
    assert!(
        body.iter()
            .any(|line| line.trim() == "echo \"side-files: $*\" >&2"),
        "{body:?}"
    );
    assert!(body.iter().any(|line| line.trim() == "exit 2"), "{body:?}");

    // The images' folder is checked before anything is made or started.
    assert!(IMAGES.ends_with("/artifacts/storage-tests"), "{IMAGES}");
    let line_of = |what: &dyn Fn(&str) -> bool| {
        (0..lines.len()).find(|at| !inside(*at) && what(code(lines[*at]).trim()))
    };
    let made = line_of(&|line| line == "made=$(realpath -m \"$manifest/artifacts/storage-tests\")")
        .expect("side-files.sh follows the manifest to the images' folder");
    let check = line_of(&|line| line == "case \"$made\" in").expect("the folder's check");
    let close = check
        + lines[check..]
            .iter()
            .position(|line| line.trim() == "esac")
            .expect("its esac");
    let arms = &lines[check..=close];
    assert!(made < check, "the folder is checked before it is known");
    assert!(
        arms.iter()
            .any(|line| line.trim() == "\"$images\" | \"$here/target/\"*) ;;"),
        "{arms:?}"
    );
    assert!(
        arms.iter()
            .any(|line| line.trim().starts_with("*) refuse \"")),
        "{arms:?}"
    );
    for (what, first) in [
        ("a folder made", line_of(&|line| line.contains("mkdir "))),
        (
            "a scratch directory",
            line_of(&|line| line.contains("mktemp ")),
        ),
        (
            "the test binary built",
            line_of(&|line| line.contains("toolkit-tree.sh\" test")),
        ),
        (
            "the sandbox started",
            line_of(&|line| line.starts_with("\"${bw[@]}\"")),
        ),
    ] {
        let first = first.unwrap_or_else(|| panic!("side-files.sh has {what}"));
        assert!(
            close < first,
            "{what} on line {} before the folder's check ends on line {}",
            first + 1,
            close + 1
        );
    }
}
