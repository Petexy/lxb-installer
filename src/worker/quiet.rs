//! Udisks kept off the disks a hand-made layout uses, for as long as the
//! installation runs — the installer blocks it itself, by design (§2) — so
//! that no file manager, LineXinBar's included, mounts a partition
//! halfway through being made, formatted or shrunk. LineXinBar is not
//! touched; the rule is the installer's own, in `/run`, which is gone at
//! the next start whatever happens.
//!
//! What putting the rule in and taking it out does is data first:
//! [`rules_text`], the rule file's text, and [`install_effects`] and
//! [`removal_effects`], the steps in order — the file written, udev told to
//! read its rules again, the disks and everything on them sent a change
//! event, and (going in) udev waited for. [`Quiet`] is the guard the worker
//! holds for the run: made before the first step is taken, it takes the
//! rule away when it is dropped, however the run ends — finished, refused
//! in Check before anything was written, failed halfway, or a step of its
//! own going in having failed. What carries the steps out is [`Hands`]: the
//! machine's ([`Udev`]), or a test's that writes them down and runs
//! nothing.
//!
//! It is a second wall, not the only one: the worker claims every device
//! exclusively just before it writes to it and reads `/proc/1/mountinfo`
//! before every format.
use crate::model::disks::{device_path_ok, Disk};
use crate::model::protocol::Failure;
use crate::worker::layout::Stop;
use crate::worker::partition::script::Command;
use crate::worker::run;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Where the rule is written while an installation runs.
pub const RULE_FILE: &str = "/run/udev/rules.d/99-lxb-installer-quiet.rules";

/// What each line asks of udisks: never show the device, never mount it.
const QUIET: &str = "ENV{UDISKS_IGNORE}=\"1\", ENV{UDISKS_AUTO}=\"0\"";

/// Whether `text` is a device's major and minor number as the kernel and
/// lsblk give them, `^[0-9]+:[0-9]+$`, and nothing else.
fn major_minor(text: &str) -> bool {
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit());
    text.split_once(':')
        .is_some_and(|(major, minor)| digits(major) && digits(minor))
}

/// The rule file's text for `disks` — every disk of the layout, written or
/// only mounted from: two lines each, one naming the disk by its kernel
/// name, one naming everything on it by the disk's major and minor numbers
/// (`ID_PART_ENTRY_DISK`, which udev's blkid gives every partition), so a
/// partition made during the run is caught as well as those there now.
///
/// Every field comes from something checked here first: the name from a
/// path [`device_path_ok`] passes (`/dev/` and letters, digits, `-`, `_`
/// and `.`, none of them a udev pattern character), the numbers from
/// `^[0-9]+:[0-9]+$`. Anything else, or no disk at all, is refused, and no
/// text is given.
pub fn rules_text<'a>(disks: impl IntoIterator<Item = &'a Disk>) -> Result<String, String> {
    let mut text = String::from(
        "# Written by lxb-installer for one installation, and removed when it ends:\n\
         # udisks neither shows nor mounts the disks it is changing.\n",
    );
    let mut any = false;
    for disk in disks {
        let name = device_path_ok(&disk.path)
            .then(|| disk.path.strip_prefix("/dev/"))
            .flatten()
            .ok_or_else(|| format!("{:?} is not a disk the rule can name", disk.path))?;
        if !major_minor(&disk.major_minor) {
            return Err(format!(
                "{:?} is not the major and minor numbers of {}",
                disk.major_minor, disk.path
            ));
        }
        text.push_str(&format!(
            "SUBSYSTEM==\"block\", KERNEL==\"{name}\", {QUIET}\n\
             SUBSYSTEM==\"block\", ENV{{ID_PART_ENTRY_DISK}}==\"{}\", {QUIET}\n",
            disk.major_minor
        ));
        any = true;
    }
    if !any {
        return Err("no disk to keep udisks off".to_string());
    }
    Ok(text)
}

/// Where the rule is written first, beside it: a name udev never reads
/// (only `*.rules` are), renamed over [`RULE_FILE`] once it is whole.
const RULE_BEING_WRITTEN: &str = "/run/udev/rules.d/.99-lxb-installer-quiet.rules.new";

/// One thing putting the rule in or taking it out does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// [`RULE_FILE`] written with this text: root's, mode 0644, whole or
    /// not at all (written beside it, then renamed over it, so a killed
    /// run's file is replaced rather than refusing a later one).
    Write(String),
    /// [`RULE_FILE`] removed, if it is there.
    Remove,
    /// One `udevadm` command.
    Run(Command),
}

/// What carries the rule's effects out.
pub trait Hands {
    fn apply(&mut self, effect: &Effect) -> Result<(), Stop>;
}

/// The kernel names of `disks` (`sda`, `nvme0n1`), each from a path
/// [`device_path_ok`] passes, or why one is refused.
fn names<'a>(disks: impl IntoIterator<Item = &'a Disk>) -> Result<Vec<String>, String> {
    disks
        .into_iter()
        .map(|disk| {
            device_path_ok(&disk.path)
                .then(|| disk.path.strip_prefix("/dev/"))
                .flatten()
                .map(str::to_string)
                .ok_or_else(|| format!("{:?} is not a disk the rule can name", disk.path))
        })
        .collect()
}

/// `udevadm trigger` for every one of `names` and everything on it: a
/// change event for each disk and each of its partitions — those made
/// during the run as well — so udisks reads the rule's answer for them.
fn trigger(names: &[String]) -> Command {
    let parents: Vec<String> = names
        .iter()
        .map(|name| format!("--parent-match=/sys/class/block/{name}"))
        .collect();
    let mut args = vec!["trigger", "--action=change"];
    args.extend(parents.iter().map(String::as_str));
    Command::new("udevadm", &args)
}

/// **Putting the rule in for `disks`**, in order: the file written, udev
/// told to read its rules again, a change event for every disk and
/// everything on it, and udev waited for (at most a minute) — so no disk
/// of the layout is shown or mounted by udisks from here on. Refused, with
/// no step, for a disk the rule cannot name ([`rules_text`]).
pub fn install_effects<'a>(
    disks: impl IntoIterator<Item = &'a Disk> + Clone,
) -> Result<Vec<Effect>, String> {
    let text = rules_text(disks.clone())?;
    let names = names(disks)?;
    Ok(vec![
        Effect::Write(text),
        Effect::Run(Command::new("udevadm", &["control", "--reload"])),
        Effect::Run(trigger(&names)),
        Effect::Run(Command::new("udevadm", &["settle", "--timeout=60"])),
    ])
}

/// **Taking the rule out again** for the disks named `names`: the file
/// removed, udev told to read its rules again, and a change event for
/// every disk and everything on it, so udisks sees them as it would have
/// without the run. Nothing is waited for: the run is over.
pub fn removal_effects(names: &[String]) -> Vec<Effect> {
    vec![
        Effect::Remove,
        Effect::Run(Command::new("udevadm", &["control", "--reload"])),
        Effect::Run(trigger(names)),
    ]
}

// A panic in the run takes the rule out too — but only while panics unwind:
// `Quiet`'s `Drop` runs as the worker's stack is unwound past the slot
// `run_worker` holds it in, and a build that aborts on a panic would leave
// the rule in `/run` until the next start, udisks kept off the person's
// disks for the rest of the live session. So the package is built to
// unwind (`Cargo.toml`'s `[profile.release]`), and a build told otherwise
// — the profile, or `-C panic=abort` in RUSTFLAGS — does not compile.
#[cfg(panic = "abort")]
compile_error!(
    "lxb-installer must be built with panic = \"unwind\": the worker's udisks \
     rule (worker::quiet::Quiet) is taken out by a Drop that a panic runs only \
     while it unwinds"
);

/// **The guard that keeps udisks off a layout's disks for the run** (the
/// installer blocks it itself): while it lives, the rule is in;
/// dropped, it is taken out ([`removal_effects`], every step tried, a
/// failure only logged), whichever way the run ends — a panic in it
/// included, which unwinds past the slot it is held in (the build is
/// pinned to unwind, above). The worker holds it until every installer
/// mount is released and only then lets it go.
pub struct Quiet {
    names: Vec<String>,
    hands: Box<dyn Hands>,
}

impl Quiet {
    /// Puts the rule in for `disks` with `hands`. The guard is put in
    /// `slot` before the first step is taken, so a rule written while a
    /// later step going in failed is still taken out when the slot is let
    /// go of. Writing the rule, udev reading it and the change events must
    /// each succeed, or the run is refused (`Internal`) before anything is
    /// written to a disk; udev not settling within the minute is only
    /// logged, as the worker's other waits for it are.
    pub fn install<'a>(
        disks: impl IntoIterator<Item = &'a Disk> + Clone,
        hands: Box<dyn Hands>,
        slot: &mut Option<Quiet>,
    ) -> Result<(), Stop> {
        let internal = |detail: String| Stop::new(Failure::Internal, detail);
        let effects = install_effects(disks.clone()).map_err(internal)?;
        let names = names(disks).map_err(internal)?;
        let quiet = slot.insert(Quiet { names, hands });
        for effect in &effects {
            let settle = matches!(effect, Effect::Run(command) if command.args[0] == "settle");
            match quiet.hands.apply(effect) {
                Ok(()) => {}
                Err(stop) if settle => run::log(&format!("quiet: {}", stop.detail)),
                Err(stop) => return Err(stop),
            }
        }
        run::log(&format!(
            "quiet: udisks kept off {} for the installation",
            quiet.names.join(", ")
        ));
        Ok(())
    }
}

impl Drop for Quiet {
    fn drop(&mut self) {
        for effect in removal_effects(&self.names) {
            if let Err(stop) = self.hands.apply(&effect) {
                run::log(&format!("quiet: {}", stop.detail));
            }
        }
        run::log("quiet: udisks may see the disks again");
    }
}

/// The machine's hands: the file in `/run/udev/rules.d`, `udevadm` through
/// the worker's runner. In a test build they refuse before anything is
/// touched (`run::fence::the_machine`): a test hands in its own.
pub struct Udev;

impl Hands for Udev {
    fn apply(&mut self, effect: &Effect) -> Result<(), Stop> {
        #[cfg(test)]
        run::fence::the_machine("udev rules");
        match effect {
            Effect::Write(text) => write_rule(text),
            Effect::Remove => match fs::remove_file(RULE_FILE) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(Stop::new(
                    Failure::Internal,
                    format!("cannot remove {RULE_FILE}: {error}"),
                )),
            },
            Effect::Run(command) => {
                let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
                run::run(command.program, &args).map(drop)
            }
        }
    }
}

/// Writes [`RULE_FILE`]: its directory made (0755, root's) where it is
/// missing and refused where it is anything but a directory; the text
/// written whole beside it, never through a link and never into a file
/// that was there (a killed run's is removed first), given mode 0644 (the
/// worker's own umask is 077) and synced; then renamed over the rule.
fn write_rule(text: &str) -> Result<(), Stop> {
    let internal = |detail: String| Stop::new(Failure::Internal, detail);
    let dir = Path::new(RULE_FILE)
        .parent()
        .ok_or_else(|| internal(format!("{RULE_FILE} has no directory")))?;
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Err(internal(format!("{} is not a directory", dir.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(dir)
                .and_then(|()| fs::set_permissions(dir, fs::Permissions::from_mode(0o755)))
                .map_err(|e| internal(format!("cannot make {}: {e}", dir.display())))?;
        }
        Err(error) => return Err(internal(format!("{}: {error}", dir.display()))),
    }
    match fs::remove_file(RULE_BEING_WRITTEN) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(internal(format!("{RULE_BEING_WRITTEN}: {error}"))),
    }
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(RULE_BEING_WRITTEN)
        .and_then(|mut file| {
            file.set_permissions(fs::Permissions::from_mode(0o644))?;
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(RULE_BEING_WRITTEN, RULE_FILE));
    written.map_err(|e| internal(format!("cannot write {RULE_FILE}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::partition::fixtures::disk_empty;

    fn disk(path: &str, major_minor: &str) -> Disk {
        let mut disk = disk_empty(512);
        disk.path = path.to_string();
        disk.major_minor = major_minor.to_string();
        disk
    }

    #[test]
    fn the_rule_names_each_disk_and_everything_on_it() {
        let disks = [
            disk("/dev/sda", "8:0"),
            disk("/dev/nvme0n1", "259:0"),
            disk("/dev/vda", "254:0"),
        ];
        assert_eq!(
            rules_text(&disks).unwrap(),
            "# Written by lxb-installer for one installation, and removed when it ends:\n\
             # udisks neither shows nor mounts the disks it is changing.\n\
             SUBSYSTEM==\"block\", KERNEL==\"sda\", ENV{UDISKS_IGNORE}=\"1\", ENV{UDISKS_AUTO}=\"0\"\n\
             SUBSYSTEM==\"block\", ENV{ID_PART_ENTRY_DISK}==\"8:0\", ENV{UDISKS_IGNORE}=\"1\", ENV{UDISKS_AUTO}=\"0\"\n\
             SUBSYSTEM==\"block\", KERNEL==\"nvme0n1\", ENV{UDISKS_IGNORE}=\"1\", ENV{UDISKS_AUTO}=\"0\"\n\
             SUBSYSTEM==\"block\", ENV{ID_PART_ENTRY_DISK}==\"259:0\", ENV{UDISKS_IGNORE}=\"1\", ENV{UDISKS_AUTO}=\"0\"\n\
             SUBSYSTEM==\"block\", KERNEL==\"vda\", ENV{UDISKS_IGNORE}=\"1\", ENV{UDISKS_AUTO}=\"0\"\n\
             SUBSYSTEM==\"block\", ENV{ID_PART_ENTRY_DISK}==\"254:0\", ENV{UDISKS_IGNORE}=\"1\", ENV{UDISKS_AUTO}=\"0\"\n"
        );
    }

    #[test]
    fn nothing_but_a_checked_name_and_numbers_reaches_the_rule() {
        for path in [
            "/dev/sd\"a",
            "/dev/sd*",
            "/dev/sd?",
            "/dev/sd[ab]",
            "/dev/../sda",
            "/dev/.hidden",
            "/dev/mapper/x",
            "/dev/",
            "sda",
            "/dev/sda\n",
            "/dev/sda, KERNEL==\"*\"",
        ] {
            assert!(rules_text(&[disk(path, "8:0")]).is_err(), "{path:?}");
        }
        for numbers in [
            "", "8", "8:", ":0", "8:0\n", "8:0,", "a:b", "8:0:1", " 8:0", "8 :0", "8:-1",
        ] {
            assert!(
                rules_text(&[disk("/dev/sda", numbers)]).is_err(),
                "{numbers:?}"
            );
        }
        // One disk refused refuses the whole text.
        assert!(rules_text(&[disk("/dev/sda", "8:0"), disk("/dev/sd*", "8:16")]).is_err());
        assert!(rules_text(&[]).is_err(), "no disk, no rule");
    }

    fn run(args: &[&str]) -> Effect {
        Effect::Run(Command::new("udevadm", args))
    }

    /// **The rule goes in and comes out as planned** (Unit Q, Q5): written,
    /// read again, a change event for every disk and everything on it, and
    /// udev waited for; taken out, read again, the same change events —
    /// for every disk of the layout, written to or only mounted from. A
    /// disk the rule cannot name refuses the whole of it.
    #[test]
    fn the_rule_goes_in_and_comes_out_as_planned() {
        let disks = [disk("/dev/vda", "254:0"), disk("/dev/nvme0n1", "259:0")];
        let parents = [
            "trigger",
            "--action=change",
            "--parent-match=/sys/class/block/vda",
            "--parent-match=/sys/class/block/nvme0n1",
        ];
        assert_eq!(
            install_effects(&disks).unwrap(),
            [
                Effect::Write(rules_text(&disks).unwrap()),
                run(&["control", "--reload"]),
                run(&parents),
                run(&["settle", "--timeout=60"]),
            ]
        );
        let names = ["vda".to_string(), "nvme0n1".to_string()];
        assert_eq!(
            removal_effects(&names),
            [Effect::Remove, run(&["control", "--reload"]), run(&parents)]
        );
        assert!(install_effects(&[disk("/dev/sd*", "8:0")]).is_err());
        assert!(install_effects(&[] as &[Disk]).is_err());
    }

    /// Hands that write down every effect asked of them, as a line, and
    /// fail the ones numbered in `failing` (counted from 0) — and run
    /// nothing.
    struct Written {
        said: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
        failing: Vec<usize>,
    }

    impl Hands for Written {
        fn apply(&mut self, effect: &Effect) -> Result<(), Stop> {
            let mut said = self.said.borrow_mut();
            let index = said.len();
            said.push(match effect {
                Effect::Write(_) => "write".to_string(),
                Effect::Remove => "remove".to_string(),
                Effect::Run(command) => format!("udevadm {}", command.args[0]),
            });
            if self.failing.contains(&index) {
                return Err(Stop::new(Failure::CommandFailed, format!("effect {index}")));
            }
            Ok(())
        }
    }

    /// **The rule is taken out on every way out** (Unit Q, Q5): whichever
    /// step of putting it in fails — the file, udev reading it, the change
    /// events — the guard is already in its slot, and letting it go takes
    /// the rule out, every step of that tried even where one fails; udev
    /// not settling is no refusal. Put in whole, it is taken out once,
    /// when the guard is let go of, and never before.
    #[test]
    fn the_rule_is_taken_out_on_every_way_out() {
        let disks = [disk("/dev/vda", "254:0")];
        let removal = ["remove", "udevadm control", "udevadm trigger"];
        let going_in = [
            "write",
            "udevadm control",
            "udevadm trigger",
            "udevadm settle",
        ];
        for failing in 0..going_in.len() {
            let said = std::rc::Rc::default();
            let hands = Written {
                said: std::rc::Rc::clone(&said),
                failing: vec![failing],
            };
            let mut slot = None;
            let result = Quiet::install(&disks, Box::new(hands), &mut slot);
            let settled = failing == going_in.len() - 1;
            assert_eq!(result.is_ok(), settled, "step {failing} failed");
            assert!(slot.is_some(), "the guard is held whatever happened");
            let tried = if settled { going_in.len() } else { failing + 1 };
            assert_eq!(said.borrow().as_slice(), &going_in[..tried]);
            drop(slot);
            let after: Vec<String> = said.borrow()[tried..].to_vec();
            assert_eq!(after, removal, "step {failing} failed");
        }
        // Put in whole: out once, when let go of; every step of taking it
        // out tried though each fails.
        let said = std::rc::Rc::default();
        let hands = Written {
            said: std::rc::Rc::clone(&said),
            failing: vec![4, 5, 6],
        };
        let mut slot = None;
        Quiet::install(&disks, Box::new(hands), &mut slot).unwrap();
        assert_eq!(said.borrow().as_slice(), going_in);
        slot = None;
        assert_eq!(&said.borrow()[going_in.len()..], removal);
        assert!(slot.is_none());
        // A disk the rule cannot name: refused, nothing done, no guard.
        let said = std::rc::Rc::default();
        let hands = Written {
            said: std::rc::Rc::clone(&said),
            failing: Vec::new(),
        };
        let mut slot = None;
        assert!(Quiet::install(&[disk("/dev/sd*", "8:0")], Box::new(hands), &mut slot).is_err());
        assert!(slot.is_none() && said.borrow().is_empty());
    }

    /// The machine's hands refuse every test before anything is touched.
    #[test]
    fn no_test_writes_this_computers_udev_rules() {
        for effect in [
            Effect::Write("# nothing\n".into()),
            Effect::Remove,
            run(&["--version"]),
        ] {
            let words = crate::worker::run::fence::refusal(move || Udev.apply(&effect))
                .expect("the machine's udev was reached");
            assert!(words.contains("test build:"), "{words}");
        }
    }
}
