//! Running external programs: by absolute path from a fixed list of
//! directories, with a cleared environment, never through a shell, output
//! captured. Every command line is logged; standard input never is (it is
//! where passwords travel).
//!
//! CONTRACT (phase 0), shared by both halves of the worker.
use super::layout::Stop;
use crate::model::protocol::Failure;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// The only places a program is looked up. The live system's own tools.
pub const SAFE_PATH: &str = "/usr/bin:/usr/sbin:/bin:/sbin";

type Sink = Box<dyn Fn(&str) + Send + Sync>;
static LOG: OnceLock<Sink> = OnceLock::new();

/// Where log lines go (the worker sends them to its log file and as
/// `Event::Log`). Set once by the worker's `main`; until then lines are
/// written to standard error.
pub fn set_log(sink: Sink) {
    let _ = LOG.set(sink);
}

pub fn log(line: &str) {
    match LOG.get() {
        Some(sink) => sink(line),
        None => eprintln!("{line}"),
    }
}

/// The absolute path of `name` in [`SAFE_PATH`], or `MissingTool`.
pub fn command_path(name: &str) -> Result<PathBuf, Stop> {
    for directory in SAFE_PATH.split(':') {
        // In a test build a thread can be made to find no program at all
        // (`fence::FindNothing`), for the tests of the starters that ask the
        // fence themselves: one whose question were ever lost finds nothing
        // to start, and its test fails without anything having run.
        #[cfg(test)]
        if fence::finding_nothing() {
            break;
        }
        let path = Path::new(directory).join(name);
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
                return Ok(path);
            }
        }
    }
    Err(Stop::new(
        Failure::MissingTool,
        format!("required command is missing: {name}"),
    ))
}

fn command(program: &str, args: &[&str]) -> Result<Command, Stop> {
    // In a test build nothing is looked up, let alone started, until the
    // fence has judged the whole command line (see `fence`).
    #[cfg(test)]
    fence::check(program, args);
    let mut command = Command::new(command_path(program)?);
    command
        .args(args)
        .env_clear()
        .env("PATH", SAFE_PATH)
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("HOME", "/root")
        .env("SYSTEMD_OFFLINE", "1");
    // The worker keeps a umask of 077 for the files it makes itself (the log
    // above all), but what a program makes inside the installed system has to
    // have the modes a normal system would give it: a directory grub-install
    // or useradd creates under 077 is unreadable to everyone but root for the
    // life of the installation. So every child starts from the usual 022.
    //
    // SAFETY: umask is async-signal-safe and touches nothing but the calling
    // process, which here is the freshly forked child before it execs.
    unsafe {
        command.pre_exec(|| {
            libc::umask(0o022);
            Ok(())
        });
    }
    Ok(command)
}

fn describe(program: &str, args: &[&str]) -> String {
    let mut line = format!("$ {program}");
    for arg in args {
        line.push(' ');
        line.push_str(arg);
    }
    line
}

fn failed(program: &str, error: impl std::fmt::Display) -> Stop {
    Stop::new(Failure::CommandFailed, format!("{program}: {error}"))
}

/// Runs `program args` and returns its standard output.
pub fn run(program: &str, args: &[&str]) -> Result<String, Stop> {
    run_input(program, args, &[], false)
}

/// Runs `program args` with `input` on standard input. With `secret`, the
/// program's standard error is not repeated into the failure or the log (a
/// tool that echoes its input must not put a password there).
pub fn run_input(program: &str, args: &[&str], input: &[u8], secret: bool) -> Result<String, Stop> {
    let (output, stderr) = output_of(program, args, input, secret)?;
    if !output.status.success() {
        return Err(failed(
            program,
            format!("{} {}", output.status, stderr.trim()),
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| failed(program, "printed invalid text"))
}

/// How a program run for its answer ended ([`run_status`]): the code it
/// exited with, and what it printed on each stream (standard error cut as
/// the log cuts it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `program args` with `input` on standard input and answers how it
/// ended, for the programs whose exit code is itself the answer rather than
/// a success or a failure: e2fsck's is the sum of what it found and what it
/// put right, and the shrink of a kept filesystem goes on or stops by it
/// (`manual::resize`, which holds each code to its tool's own rule).
///
/// **The exit-code rule**: whatever code the program exits with — 0 or any
/// other — is its answer, handed back with what it printed; it is never
/// taken for a failure here. Only a program that said nothing is a failure
/// (`CommandFailed`): one that could not be started, sent its input or
/// waited for, and one that ended with no code at all, killed by a signal
/// ([`code_of`]). Like every other runner, it passes the test build's fence
/// first ([`command`], and, in a test build, `fence::input` for what it
/// sends).
pub fn run_status(program: &str, args: &[&str], input: &[u8]) -> Result<Status, Stop> {
    let (output, stderr) = output_of(program, args, input, false)?;
    let code = code_of(output.status).map_err(|why| failed(program, why))?;
    Ok(Status {
        code,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr,
    })
}

/// How often a bounded run looks whether its program has ended
/// ([`run_status_within`]).
const POLL: std::time::Duration = std::time::Duration::from_millis(20);

/// The most a bounded run keeps of each of its program's streams: its
/// answer is a few lines; anything past this is read and let go of, so a
/// program that says too much never waits on a full pipe nor fills memory.
const KEPT: u64 = 1024 * 1024;

/// [`run_status`] with nothing on standard input, **bounded in time**: a
/// program that has not ended once it has run `limit` is ended (killed, and
/// waited for) and answered with an error, `CommandFailed`, which says so —
/// for the read-only reads of phase A whose program may never end on a disk
/// that does not answer (`manual::shrinking::WINDOWS_PATIENCE`); nothing a
/// read leaves half done. Whether it has run out of time is asked before
/// whether it has ended, so a `limit` of nothing ends it at once. Each
/// stream is read beside it ([`KEPT`] of it kept), so it never waits on a
/// full pipe. Like every other runner, it passes the test build's fence
/// first ([`command`]).
pub fn run_status_within(
    program: &str,
    args: &[&str],
    limit: std::time::Duration,
) -> Result<Status, Stop> {
    log(&describe(program, args));
    let mut child = command(program, args)?
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| failed(program, format!("could not start: {e}")))?;
    let deadline = std::time::Instant::now() + limit;
    let keep = |mut stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut kept = Vec::new();
            let _ = (&mut stream).take(KEPT).read_to_end(&mut kept);
            let _ = std::io::copy(&mut stream, &mut std::io::sink());
            kept
        })
    };
    let stdout = keep(Box::new(child.stdout.take().expect("stdout is piped")));
    let errors = keep(Box::new(child.stderr.take().expect("stderr is piped")));
    let ended = loop {
        if std::time::Instant::now() >= deadline {
            break None;
        }
        match child.try_wait() {
            Ok(Some(status)) => break Some(Ok(status)),
            Ok(None) => std::thread::sleep(POLL),
            Err(error) => break Some(Err(error)),
        }
    };
    if ended.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stdout = stdout.join().unwrap_or_default();
    let stderr: String = String::from_utf8_lossy(&errors.join().unwrap_or_default())
        .chars()
        .take(4000)
        .collect();
    if !stderr.trim().is_empty() {
        log(stderr.trim());
    }
    let status = match ended {
        None => {
            return Err(failed(
                program,
                format!(
                    "did not end within {} seconds, and was ended",
                    limit.as_secs()
                ),
            ))
        }
        Some(status) => status.map_err(|e| failed(program, format!("could not wait: {e}")))?,
    };
    let code = code_of(status).map_err(|why| failed(program, why))?;
    Ok(Status {
        code,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr,
    })
}

/// The code `status` says a program exited with, or why it says none: a
/// program killed by a signal ended without one.
fn code_of(status: std::process::ExitStatus) -> Result<i32, String> {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(code), _) => Ok(code),
        (None, Some(signal)) => Err(format!("ended by signal {signal}, with no exit code")),
        (None, None) => Err(format!("ended with no exit code ({status})")),
    }
}

/// Starts `program args` with `input` on standard input and waits for it:
/// its whole output, and its standard error as the log has it (empty with
/// `secret`). How it ended is the caller's to judge.
fn output_of(
    program: &str,
    args: &[&str],
    input: &[u8],
    secret: bool,
) -> Result<(std::process::Output, String), Stop> {
    log(&describe(program, args));
    // In a test build the fence reads standard input too (see `fence`).
    #[cfg(test)]
    fence::input(program, input);
    let mut child = command(program, args)?
        .stdin(if input.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| failed(program, format!("could not start: {e}")))?;
    if !input.is_empty() {
        let mut stdin = child.stdin.take().expect("stdin is piped");
        if let Err(error) = stdin.write_all(input) {
            drop(stdin);
            let _ = child.wait();
            return Err(failed(program, format!("could not send input: {error}")));
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| failed(program, format!("could not wait: {e}")))?;
    let stderr = if secret {
        String::new()
    } else {
        String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(4000)
            .collect()
    };
    if !stderr.trim().is_empty() {
        log(stderr.trim());
    }
    Ok((output, stderr))
}

/// Runs `program args`, calling `line` with every line it writes to
/// standard output as it arrives (rsync's progress, pacman's output). A
/// carriage return ends a line too, which is how rsync redraws its progress.
pub fn run_streaming(program: &str, args: &[&str], line: &mut dyn FnMut(&str)) -> Result<(), Stop> {
    log(&describe(program, args));
    let mut child = command(program, args)?
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| failed(program, format!("could not start: {e}")))?;
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut pending = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = match stdout.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for &byte in &chunk[..read] {
            if byte == b'\n' || byte == b'\r' {
                if !pending.is_empty() {
                    line(&String::from_utf8_lossy(&pending));
                    pending.clear();
                }
            } else {
                pending.push(byte);
            }
        }
    }
    if !pending.is_empty() {
        line(&String::from_utf8_lossy(&pending));
    }
    let status = child
        .wait()
        .map_err(|e| failed(program, format!("could not wait: {e}")))?;
    let stderr: String = errors
        .join()
        .unwrap_or_default()
        .chars()
        .take(4000)
        .collect();
    if !stderr.trim().is_empty() {
        log(stderr.trim());
    }
    if !status.success() {
        return Err(failed(program, format!("{status} {}", stderr.trim())));
    }
    Ok(())
}

/// Runs `/usr/bin/<program>` inside the target root through `arch-chroot`.
pub fn chroot(root: &Path, program: &str, args: &[&str]) -> Result<String, Stop> {
    let root = utf8(root)?;
    let program = format!("/usr/bin/{program}");
    let mut all = vec![root, program.as_str()];
    all.extend_from_slice(args);
    run("arch-chroot", &all)
}

/// [`chroot`] with `input` on standard input; `secret` as for [`run_input`].
/// This is how a password reaches `chpasswd` inside the target: on a pipe,
/// never on a command line or in a file.
pub fn chroot_input(
    root: &Path,
    program: &str,
    args: &[&str],
    input: &[u8],
    secret: bool,
) -> Result<String, Stop> {
    let root = utf8(root)?;
    let program = format!("/usr/bin/{program}");
    let mut all = vec![root, program.as_str()];
    all.extend_from_slice(args);
    run_input("arch-chroot", &all, input, secret)
}

/// [`chroot`] with every line of output handed to `line` as it arrives, for
/// the long programs (pacman, flatpak) whose progress belongs in the log
/// while they run rather than all at once when they end.
pub fn chroot_streaming(
    root: &Path,
    program: &str,
    args: &[&str],
    line: &mut dyn FnMut(&str),
) -> Result<(), Stop> {
    let root = utf8(root)?;
    let program = format!("/usr/bin/{program}");
    let mut all = vec![root, program.as_str()];
    all.extend_from_slice(args);
    run_streaming("arch-chroot", &all, line)
}

/// A path as the text a command line needs, or `Internal`: every path the
/// worker builds is from validated parts, so one that is not UTF-8 is a bug.
pub fn utf8(path: &Path) -> Result<&str, Stop> {
    path.to_str().ok_or_else(|| {
        Stop::new(
            Failure::Internal,
            format!("path is not UTF-8: {}", path.display()),
        )
    })
}

/// The test build's fence around every program the worker's code runs, and
/// around the network.
///
/// The unit tests drive the worker's code with made-up disks, on the
/// developer's own computer, and makepkg's `check()` runs them again inside
/// a build container that may share the host's `/dev`. A mistake in a test,
/// or in the code it tests, must never become a command run against that
/// machine. So in a test build [`command`] asks [`check`] first, before a
/// program is even looked up, and a command line it refuses stops the test
/// with a panic (never an error, which the code under test could turn into
/// something else). It refuses:
///
/// - every program when the effective user is root: `sudo cargo test` would
///   otherwise reach every disk;
/// - every argument that points at the machine ([`reaches_the_machine`]):
///   one that names something under `/dev`, `/proc`, `/sys` or `/run`
///   wherever in the argument it stands (`--opt=/dev/x`, `-d/dev/x`), one
///   that starts with `/dev`, one with `..` in it (`../../dev/x`), and one
///   that names, through any symbolic link, a device node or a place that
///   does — judged by its letters before anything is looked up, so an
///   argument under one of those four is refused without a look at it;
/// - every program not on the lists below: [`ANY_TEST`] with arguments of
///   its own and [`ANY_TEST_LINES`] exactly as written, for every test, and
///   the partitioning tools of [`ON_IMAGES`] only while the thread holds
///   the image tests' `images::Permit`. `Image::run` takes one for a single
///   call, after its own checks, naming the one program and the one image
///   file it may be given, and the fence holds the command line to both.
///
/// [`run_input`] asks [`input`] of what it is about to send on standard
/// input as well, which is held to a stricter rule than an argument.
///
/// The fence is asked by more than [`command`]. What starts a program,
/// opens a device or opens a connection without passing through here asks
/// it itself, first thing, before a program is looked up or a name
/// resolved:
///
/// - `exec::claim`, which opens a device itself rather than through a
///   program, asks [`claimed`] the same questions of its path;
/// - the three programs the model starts itself, each asking [`check`]
///   with its own command line, which refuses all three since none is on
///   any list: lsblk (`disks::lsblk`, which `disks::discover` runs, and
///   with it the worker's `check::disk_unchanged`), nmcli
///   (`catalogs::nm_connectivity_full`, which `catalogs::online` runs, and
///   with it the worker's `network::check`) and pacman
///   (`catalogs::removable_packages`, the Desktop screen's);
/// - the measure entry's starter (`measure::start`), which keeps what a
///   failing command printed and so does not go through `run`: once its own
///   allowlist has let a command through, it asks [`check`] with that
///   command line, which refuses every one of them, since no program the
///   measure runs is on any list;
/// - the one place the installer opens a connection, `catalogs::reaches`,
///   asks [`network`], which refuses every test;
/// - the one place it reads the computer's firmware, `Firmware::detect`
///   (`worker::layout`), asks [`firmware`], which refuses every test too:
///   a look at `/sys` does no harm, but a test whose answer hung on it
///   would be a question about the machine rather than the code, so the
///   interface's tests off the demo pretend one (`State::pretend_firmware`);
/// - the hand-made layout's fence, which reads `/sys/class/block` through
///   `manual::fence::Machine`, asks [`block_devices`] before it looks,
///   which refuses every test for the same reason: the fence's tests give
///   it a kernel of their own;
/// - everything else carrying out a hand-made layout asks of the machine —
///   the session's mounts and swaps, the devices' facts, the udev rule, the
///   stage, mounting, the swap file, and a kept filesystem's shrink (its
///   readings again, its dry run, its commands and a partition's first
///   sector read back) — goes through
///   `manual::machine::Real` and the rule's `quiet::Udev`, which ask
///   [`the_machine`] first, refusing every test: their tests hand in a
///   machine that writes down what it is asked and runs nothing.
///
/// The tests of those starters hold [`FindNothing`], so that a starter whose
/// question were ever lost would find nothing to start and fail its test,
/// rather than list or read this computer's disks or ask its network.
///
/// What is left outside are the interface's own, out of every test's reach
/// by construction: polkit's `pkexec` (`launch.rs`), which in a test build
/// is a path that cannot exist and is asked only once the live medium's
/// guard has let a worker or a measurement start — and a measurement only
/// through `State::measure`, which in a test build refuses to be used, so a
/// test standing on a live session hands in its own — and the restart's
/// `systemctl reboot` (`State::reboot` in `ui/app.rs`), asked only on a live
/// session and only from `State::frame`, a window's real frame, never from
/// the tests' `frame_at`.
///
/// Nor does the fence stand in main.rs's own test binary: there the library
/// is built as it is for the release, without `#[cfg(test)]`, so it has no
/// fence at all. Those tests only refuse command lines before the installer
/// opens; one that went further (`--list-disks`, a window, `--worker`,
/// `--measure`) would reach the machine unfenced, and belongs in the
/// library's tests. Each of those roads stops first in a test build
/// (`not_in_a_test` in main.rs).
///
/// The release build has none of it: the module is `#[cfg(test)]`, and so
/// are the lines that ask it.
#[cfg(test)]
pub(crate) mod fence {
    use std::cell::{Cell, RefCell};
    use std::os::unix::fs::FileTypeExt;

    /// The program any test may run with arguments of its own: `printf`,
    /// for `run_streaming`'s line splitting and for the tests of the fence.
    /// It only prints what it is given (it opens nothing and reads nothing
    /// on standard input), and its arguments are judged like any other.
    pub(crate) const ANY_TEST: &[&str] = &["printf"];

    /// The command lines any test may run, each exactly as written here and
    /// no other line of the same program: a shell runs whatever it is given,
    /// so `sh` on the list with arguments of its own would let a test run
    /// any program at all, under any name, with a path put together as it
    /// runs (`d=de; cat "/${d}v/sda"`) or a script sent on standard input,
    /// none of which the fence could read. The two lines are the two tests'
    /// that need a shell:
    ///
    /// - `sh -c 'cat >&2; exit 3'`, the accounts test that a password piped
    ///   to a failing program reaches neither the failure nor the log; `cat`
    ///   copies standard input to standard error and nothing reads it as a
    ///   command;
    /// - `sh -c true`, the program `run_refuses_root` shows refused as root
    ///   and run as anyone else.
    pub(crate) const ANY_TEST_LINES: &[&[&str]] =
        &[&["sh", "-c", "cat >&2; exit 3"], &["sh", "-c", "true"]];

    /// The programs a test may run only on an image file it made, while it
    /// holds `images::Permit` for that one call: the partitioning tools the
    /// image tests carry the plans out with.
    pub(crate) const ON_IMAGES: &[&str] = &["sfdisk", "wipefs"];

    /// The programs that make, fill, check, measure, read or resize a file
    /// system on an image file it was given, each added by name in the step
    /// that first needs it. The side-file tests (step Q8)
    /// (`manual::side_files`): e2fsprogs' `mkfs.ext4` (an ext4 made),
    /// `debugfs` (filled, a canary written and read, an error made for
    /// e2fsck to find), `dumpe2fs`, `e2fsck` and `resize2fs`; ntfsprogs'
    /// `mkntfs` (an NTFS made), `ntfsresize` and `ntfscat`.
    ///
    /// Every argument of theirs is judged by its letters first
    /// ([`side_file_words`]): the side file, and nothing a tool could take
    /// for another file. Besides the permit, the fence runs one of them only
    /// when the test binary was asked for the tests set aside with
    /// `#[ignore]`, and then only on a side file the test made
    /// ([`side_file_made`]). That is the convention for every test that runs
    /// a file system tool on an image file: it is `#[ignore]`d, and run by
    /// name, by hand, through the sandbox of `scripts/side-files.sh` alone,
    ///
    /// ```sh
    /// scripts/side-files.sh ext4    # or ntfs
    /// ```
    ///
    /// (`scripts/toolkit-tree.sh test --release --locked --lib -- --ignored
    /// --exact <name>`, inside bubblewrap), so that makepkg's `check()` (a
    /// plain `cargo test`, inside a build container that may share the host's
    /// `/dev`) never runs it, and no test's result depends on which tools a
    /// machine happens to have. A test that runs one of these programs and
    /// was not set aside fails loudly in a plain `cargo test` instead of
    /// quietly running a file system tool where it should not.
    pub(crate) const ON_SIDE_FILES: &[&str] = &[
        "mkfs.ext4",
        "debugfs",
        "dumpe2fs",
        "e2fsck",
        "resize2fs",
        "mkntfs",
        "ntfsresize",
        "ntfscat",
    ];

    /// What a side-file tool may be given beside its side file, word by
    /// word: the flags it takes alone, the flags that take a value and that
    /// value's shape, and the words it takes after the side file, each by
    /// its shape ([`side_file_words`]). Exactly what the side-file tests
    /// (`manual::side_files`) give it, and nothing a tool could read as
    /// another file: never a blkid tag, a relative name, a request file, a
    /// file to dump to or write from, an undo file or an external journal.
    struct Shape {
        alone: &'static [&'static str],
        valued: &'static [(&'static str, Value)],
        after: &'static [Value],
    }

    /// The shape of a value a side-file tool is given ([`Shape`]).
    #[derive(Clone, Copy)]
    enum Value {
        /// A size as resize2fs and ntfsresize take one: digits, then at
        /// most one of `K`, `M`, `G` or `s` (`536870912`, `1048576K`).
        Size,
        /// The one file ntfscat is asked for, `hiberfil.sys`.
        Hiberfile,
        /// One debugfs request of the few the tests make inside the file
        /// system: its verb one of [`REQUESTS`], each word after it letters,
        /// digits, `_` and `-` (an inode's name, a number, `-f` naming the
        /// file inside the file system) — no path, no `=`, no dot, no
        /// second request.
        Request,
    }

    /// The debugfs requests a side-file test makes (`manual::side_files`):
    /// each reads or writes inside the file system on the side file alone.
    /// Never `dump`, `rdump`, `dump_inode` or `write`, which copy a file out
    /// of it or into it from this computer, nor `logdump`, `open` or
    /// anything else.
    const REQUESTS: &[&str] = &["mknod", "sif", "fallocate", "zap_block", "rm", "bmap", "bd"];

    /// The blkid tags a tool that links libblkid looks a file system up by
    /// among this computer's disks (`LABEL=…`, `UUID=…`): never given,
    /// anywhere in an argument, in either case. The longer first, so each is named as itself.
    const BLKID_TAGS: &[&str] = &["PARTUUID=", "PARTLABEL=", "LABEL=", "UUID=", "TYPE=", "ID="];

    fn shape(program: &str) -> Option<Shape> {
        let (alone, valued, after): (&[&str], &[(&str, Value)], &[Value]) = match program {
            "mkfs.ext4" => (&["-F"], &[], &[]),
            "mkntfs" => (&["-F", "-Q"], &[], &[]),
            "dumpe2fs" => (&["-h"], &[], &[]),
            "e2fsck" => (&["-f", "-n", "-y"], &[], &[]),
            "resize2fs" => (&["-P"], &[], &[Value::Size]),
            "ntfsresize" => (
                &["--info", "--no-action", "--no-progress-bar", "--force"],
                &[("--size", Value::Size)],
                &[],
            ),
            "ntfscat" => (&[], &[], &[Value::Hiberfile]),
            "debugfs" => (&["-w"], &[("-R", Value::Request)], &[]),
            _ => return None,
        };
        Some(Shape {
            alone,
            valued,
            after,
        })
    }

    impl Value {
        /// Why `text` is not a value of this shape, or `None` when it is.
        fn refusal(self, text: &str) -> Option<String> {
            match self {
                Value::Size => {
                    let digits = text.trim_end_matches(['K', 'M', 'G', 's']);
                    let suffix = text.len() - digits.len();
                    let fits = !digits.is_empty()
                        && digits.bytes().all(|c| c.is_ascii_digit())
                        && suffix <= 1;
                    (!fits).then(|| format!("{text:?} is not a size"))
                }
                Value::Hiberfile => (text != "hiberfil.sys")
                    .then(|| format!("{text:?} is not the one file it is asked for")),
                Value::Request => {
                    let mut words = text.split(' ');
                    let verb = words.next().unwrap_or_default();
                    if ["dump", "rdump", "dump_inode", "write"].contains(&verb) {
                        return Some(format!(
                            "{verb:?} copies a file between the side file and this computer"
                        ));
                    }
                    if !REQUESTS.contains(&verb) {
                        return Some(format!("{verb:?} is not a request a side-file test makes"));
                    }
                    let plain = |word: &str| {
                        !word.is_empty()
                            && word
                                .bytes()
                                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                    };
                    words.find(|word| !plain(word)).map(|word| {
                        format!("the request {text:?} holds {word:?}, which no request needs")
                    })
                }
            }
        }
    }

    /// Why one flag is never given to a side-file tool, where it is one of
    /// the flags that would have it read or write another file: debugfs'
    /// request file (`-f`), data source device (`-d`) and undo file (`-z`);
    /// e2fsck's undo file (`-z`), external journal (`-j`) and extended
    /// options (`-E`, which name files); mke2fs' directory to fill it from
    /// (`-d`), undo file (`-z`) and journal options (`-J`).
    fn flag_refusal(program: &str, flag: &str) -> String {
        let named = match (program, flag.get(..2).unwrap_or(flag)) {
            ("debugfs", "-f") => "a file of requests, which the fence cannot read",
            ("debugfs", "-d") => "a data source device",
            ("debugfs" | "e2fsck" | "mkfs.ext4", "-z") => "an undo file",
            ("e2fsck", "-j") => "an external journal",
            ("e2fsck", "-E") => "extended options, which name files",
            ("mkfs.ext4", "-d") => "a directory to fill it from",
            ("mkfs.ext4", "-J") => "journal options, which name a device",
            _ => "a flag its side-file tests never give it",
        };
        format!("{flag:?} is {named}")
    }

    /// **What a side-file tool may be given** (`ON_SIDE_FILES`; step Q8),
    /// judged by the letters of every argument before anything is looked up:
    /// no blkid tag anywhere ([`BLKID_TAGS`]); the side file `image` given
    /// once, and every other word a flag of its [`Shape`] or a value of the
    /// shape its flag or its place after the side file asks for — so no
    /// relative name, no request file, no file to dump to or write from, no
    /// undo file, no external journal. `Err` says why. The side file itself
    /// is then held to being the one the test made ([`side_file_made`]), once
    /// the tests set aside were asked for.
    pub(crate) fn side_file_words(program: &str, args: &[&str], image: &str) -> Result<(), String> {
        for arg in args {
            let upper = arg.to_ascii_uppercase();
            if let Some(tag) = BLKID_TAGS.iter().find(|tag| upper.contains(*tag)) {
                return Err(format!(
                    "{arg:?} names a file system by its blkid tag ({tag}), which the tool \
                     would look up among this computer's disks itself"
                ));
            }
        }
        let shape = shape(program).ok_or_else(|| format!("{program} has no side-file shape"))?;
        let mut words = args.iter();
        let mut after = shape.after.iter();
        let mut given = false;
        while let Some(&arg) = words.next() {
            if arg == image {
                if given {
                    return Err(format!("the side file {image} is given twice"));
                }
                given = true;
            } else if shape.alone.contains(&arg) {
            } else if let Some((flag, value)) = shape.valued.iter().find(|(flag, _)| *flag == arg) {
                let text = words
                    .next()
                    .ok_or_else(|| format!("{flag} is given no value"))?;
                if let Some(why) = value.refusal(text) {
                    return Err(format!("{flag}: {why}"));
                }
            } else if arg.starts_with('-') {
                return Err(flag_refusal(program, arg));
            } else if let (true, Some(value)) = (given, after.next()) {
                if let Some(why) = value.refusal(arg) {
                    return Err(why);
                }
            } else {
                return Err(format!(
                    "{arg:?} is a name relative to where it runs, not its side file"
                ));
            }
        }
        if !given {
            return Err(format!("it is not given its side file {image}"));
        }
        Ok(())
    }

    /// Whether `image` is the side file a test made: a regular file directly
    /// in the images' folder (`images::IMAGES`), each compared as the kernel
    /// resolves it. Asked only once the tests set aside were asked for, the
    /// last thing before a side-file tool is started.
    pub(crate) fn side_file_made(image: &str) -> Result<(), String> {
        let folder = std::fs::canonicalize(crate::worker::partition::images::IMAGES).ok();
        let real = std::fs::canonicalize(image).ok();
        let made = real
            .as_deref()
            .is_some_and(|real| real.is_file() && real.parent() == folder.as_deref());
        if !made {
            return Err(format!(
                "{image} is not a side file a test made in {}",
                crate::worker::partition::images::IMAGES
            ));
        }
        Ok(())
    }

    /// What `images::Permit` lets this thread run for one call.
    struct Leave {
        program: &'static str,
        image: String,
    }

    thread_local! {
        static LEAVE: RefCell<Option<Leave>> = const { RefCell::new(None) };
        static PRETENDED_EUID: Cell<Option<libc::uid_t>> = const { Cell::new(None) };
        static FINDING_NOTHING: Cell<bool> = const { Cell::new(false) };
    }

    /// Gives this thread leave to run `program` on `image` until
    /// [`withdraw`]. Only `images::Permit` calls it, once `Image::run`'s own
    /// checks have passed (and the fence's own tests, to see it refuse a
    /// command line that strays from its permit); a second leave while one
    /// is held is a bug in a test.
    pub(in crate::worker) fn grant(program: &'static str, image: &str) {
        LEAVE.with(|leave| {
            let mut leave = leave.borrow_mut();
            assert!(leave.is_none(), "test build: a permit is already held");
            *leave = Some(Leave {
                program,
                image: image.to_string(),
            });
        });
    }

    /// Ends this thread's leave, if it has one.
    pub(in crate::worker) fn withdraw() {
        LEAVE.with(|leave| *leave.borrow_mut() = None);
    }

    /// The effective user the fence judges by: the real one, or the one a
    /// test makes this thread pretend to be (see [`Pretend`]).
    pub(crate) fn euid() -> libc::uid_t {
        PRETENDED_EUID.get().unwrap_or_else(|| {
            // SAFETY: geteuid cannot fail.
            unsafe { libc::geteuid() }
        })
    }

    /// While it lives, the fence takes this thread to run as root, so that
    /// the refusal of root is tested without root. It can only pretend root:
    /// a pretence the other way would hide a test really run as root from
    /// the fence, which is only ever to get stricter. Dropped, it gives back
    /// whatever the thread pretended before.
    pub(crate) struct Pretend(Option<libc::uid_t>);

    impl Pretend {
        pub(crate) fn root() -> Pretend {
            Pretend(PRETENDED_EUID.replace(Some(0)))
        }
    }

    impl Drop for Pretend {
        fn drop(&mut self) {
            PRETENDED_EUID.set(self.0);
        }
    }

    /// While it lives, [`command_path`](super::command_path) finds no
    /// program on this thread: every name is missing.
    ///
    /// It is for the tests that a starter outside [`command`](super::command)
    /// asks the fence itself (lsblk, nmcli, pacman). Each such starter looks
    /// its program up with `command_path` after asking, so with this held a
    /// starter that asks is stopped by the fence's panic, and one whose
    /// question were ever lost, or moved after the lookup, finds nothing and
    /// returns its ordinary "missing" answer instead: its test fails, and
    /// nothing was started. The fence's other tests keep the same rule, that
    /// a fence which failed shows as a test that failed, never as a program
    /// that ran.
    pub(crate) struct FindNothing(());

    impl FindNothing {
        pub(crate) fn here() -> FindNothing {
            FINDING_NOTHING.set(true);
            FindNothing(())
        }
    }

    impl Drop for FindNothing {
        fn drop(&mut self) {
            FINDING_NOTHING.set(false);
        }
    }

    /// Whether this thread holds [`FindNothing`].
    pub(crate) fn finding_nothing() -> bool {
        FINDING_NOTHING.get()
    }

    /// Whether `text` points at the machine itself by its letters: it names
    /// something under `/dev`, `/proc`, `/sys` or `/run` anywhere in it (an
    /// option's glued value included, `--opt=/dev/x`, `-d/dev/x`), starts
    /// with `/dev`, or holds `..`. The last is stricter than a `..` path
    /// component on purpose: an option glued to its value (`-d..`,
    /// `--opt=..`) has no separator to split at, and no test needs two dots
    /// in a row. `/run` (the live medium's `/run/archiso`, udev's rules, the
    /// installer's own directory) too: without it, an argument the
    /// measure's probe test hands the fence, `/run/archiso/airootfs/var`, is
    /// looked up component by component (`readlink`, as an strace record of
    /// the tests showed) before its program is refused.
    pub(crate) fn reaches_the_machine(text: &str) -> bool {
        let under =
            |tree: &str| text.contains(&format!("/{tree}/")) || text.ends_with(&format!("/{tree}"));
        under("dev")
            || under("proc")
            || under("sys")
            || under("run")
            || text.starts_with("/dev")
            || text.contains("..")
    }

    /// Panics unless `text`, an argument or a path about to be opened, keeps
    /// away from the machine: by its letters ([`reaches_the_machine`]), and,
    /// when it names something that exists, by what that really is. So a
    /// link to a disk made in a scratch folder is refused like the disk's
    /// own path, and so is any device node, wherever it is.
    fn keep_away(text: &str) {
        assert!(
            !reaches_the_machine(text),
            "test build: {text:?} points at the machine"
        );
        if let Ok(real) = std::fs::canonicalize(text) {
            let shown = real.to_string_lossy();
            assert!(
                !reaches_the_machine(&shown),
                "test build: {text:?} is {shown:?}"
            );
            let device = std::fs::metadata(&real).is_ok_and(|meta| {
                meta.file_type().is_block_device() || meta.file_type().is_char_device()
            });
            assert!(!device, "test build: {text:?} is a device");
        }
    }

    /// Whether the test binary was asked for the tests set aside with
    /// `#[ignore]` (libtest's `--ignored` or `--include-ignored`).
    pub(crate) fn ignored_tests_asked_for(mut args: impl Iterator<Item = String>) -> bool {
        args.any(|arg| arg == "--ignored" || arg == "--include-ignored")
    }

    /// Panics unless a test may run `program` with `args` (see the module).
    pub(crate) fn check(program: &str, args: &[&str]) {
        assert_ne!(
            euid(),
            0,
            "test build: no test runs a program as root ({program})"
        );
        for arg in args {
            keep_away(arg);
        }
        if ANY_TEST.contains(&program) {
            return;
        }
        let lines = ANY_TEST_LINES.iter().filter_map(|line| line.split_first());
        let mut own_lines = lines.filter(|(name, _)| **name == program).peekable();
        if own_lines.peek().is_some() {
            assert!(
                own_lines.any(|(_, line)| line == args),
                "test build: {program} may run only as one of its lines, not with {args:?}"
            );
            return;
        }
        let allowed = LEAVE.with(|leave| match &*leave.borrow() {
            Some(leave) if leave.program == program => {
                let paths: Vec<&str> = args.iter().copied().filter(|a| a.contains('/')).collect();
                assert!(
                    paths == [leave.image.as_str()],
                    "test build: {program} may be given {} once and no other path: {args:?}",
                    leave.image
                );
                if ON_IMAGES.contains(&program) {
                    return true;
                }
                if !ON_SIDE_FILES.contains(&program) {
                    return false;
                }
                if let Err(why) = side_file_words(program, args, &leave.image) {
                    panic!("test build: {program} refused: {why}");
                }
                if !ignored_tests_asked_for(std::env::args()) {
                    return false;
                }
                if let Err(why) = side_file_made(&leave.image) {
                    panic!("test build: {program} refused: {why}");
                }
                true
            }
            _ => false,
        });
        assert!(
            allowed,
            "test build: {program} is not a program this test may run"
        );
    }

    /// Panics unless a test may open `device` as `exec::claim` opens a disk:
    /// never as root, and never a path [`keep_away`] refuses (anything under
    /// `/dev` among them).
    pub(crate) fn claimed(device: &str) {
        assert_ne!(euid(), 0, "test build: no test claims a device as root");
        keep_away(device);
    }

    /// Panics unless `input`, what a test is about to send `program` on
    /// standard input, keeps away from the machine.
    ///
    /// It is held to a stricter rule than an argument, by its letters alone:
    /// no `/dev`, `/proc`, `/sys`, `/run` or `..` anywhere in it, with or
    /// without a slash after, since no test has a reason to send a path at
    /// all (the image tests' sfdisk scripts carry none, which
    /// `Image::refusal` holds them to, and the accounts test sends a name
    /// and a password). The
    /// programs a test may run never take their standard input for a
    /// command, which the exact lines of [`ANY_TEST_LINES`] see to; this is
    /// the second wall. The refusal never repeats the input: it may be a
    /// password.
    pub(crate) fn input(program: &str, input: &[u8]) {
        let text = String::from_utf8_lossy(input);
        let near = ["/dev", "/proc", "/sys", "/run", ".."]
            .iter()
            .any(|place| text.contains(place));
        assert!(
            !near,
            "test build: what {program} would read on standard input points at the machine"
        );
    }

    /// Panics, always: no test reaches the network. `catalogs::reaches`, the
    /// one place the installer opens a connection, asks it before a name is
    /// even resolved.
    pub(crate) fn network(address: &str) {
        panic!("test build: no test reaches the network ({address})");
    }

    /// Panics, always: no test reads the firmware of the computer it runs
    /// on. `Firmware::detect`, the one place the installer reads it, asks it
    /// before it looks; a test of anything judged by the firmware is given
    /// one (`State::pretend_firmware`, or a `Firmware` passed in), so its
    /// answer is never a fact about the computer running it.
    pub(crate) fn firmware() {
        panic!("test build: no test reads this computer's firmware; give the test one");
    }

    /// Panics, always: no test reads this computer's block devices in
    /// `/sys/class/block`. Every reader of them asks this before it looks:
    /// the hand-made layout's fence (`manual::fence::Machine`), the
    /// firmware entry's EFI partition's disk and number and GRUB's BIOS
    /// target, the disk `/`'s partition is on (`boot`'s own readers), a
    /// device's `dev` and `holders` for the busy checks
    /// (`check::device_facts`, the executor's and the measure entry's, and
    /// the easy path's `check::not_busy`, which asks before it reads the
    /// session's mounts too) and discovery's holders (`disks::held`, behind
    /// lsblk's own refusal at the head of `discover`). A test of any of them
    /// hands in a kernel, or facts, written by hand.
    pub(crate) fn block_devices() {
        panic!(
            "test build: no test reads this computer's /sys/class/block; \
             give the fence a kernel of its own"
        );
    }

    /// Panics, always: no test reads or writes what carrying out a layout
    /// made by hand reads and writes of this computer (`what`) — its
    /// session's mounts, its block devices' facts, the udev rule that keeps
    /// udisks off a layout's disks, the installer's stage. Everything the
    /// executor asks of the machine goes through `manual::machine::Machine`,
    /// whose real one (`manual::machine::Real`) and the rule's real hands
    /// (`quiet::Udev`) ask this first; a test hands in a machine of its own
    /// that runs nothing.
    pub(crate) fn the_machine(what: &str) {
        panic!(
            "test build: no test reaches this computer's {what}; \
             carry the layout out on a machine of the test's own"
        );
    }

    /// The fence's words when `f` was stopped by a panic, or `None` when it
    /// ran to its end. For the tests of the fence.
    pub(crate) fn refusal<R>(f: impl FnOnce() -> R + std::panic::UnwindSafe) -> Option<String> {
        let payload = std::panic::catch_unwind(f).err()?;
        Some(
            payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_outside_the_safe_path_are_missing() {
        let error = command_path("definitely-not-a-real-tool-name").unwrap_err();
        assert_eq!(error.failure, Failure::MissingTool);
    }

    #[test]
    fn command_lines_are_described_without_input() {
        assert_eq!(
            describe("sfdisk", &["--lock", "/dev/vda"]),
            "$ sfdisk --lock /dev/vda"
        );
    }

    #[test]
    fn streaming_splits_on_carriage_returns_too() {
        let mut lines = Vec::new();
        run_streaming("printf", &["a\\rb\\nc"], &mut |l| lines.push(l.to_string())).unwrap();
        assert_eq!(lines, ["a", "b", "c"]);
    }

    /// `run_status`'s exit-code rule: whatever code a program ends with is
    /// its answer — 0, and a code `run` would take for a failure alike —
    /// handed back with what it printed and what it was sent; only a
    /// program that ended with no code (killed by a signal) or was never
    /// found said nothing, and is a `Stop`. The programs are the two lines
    /// of the shell any test may run and printf (the fence's lists).
    #[test]
    fn run_status_answers_the_code_a_program_ended_with() {
        // A code `run` refuses is an answer here, with its standard error:
        // the accounts line copies what it is sent there, then exits 3.
        let three = run_status("sh", &["-c", "cat >&2; exit 3"], b"plain words")
            .expect("a code is an answer");
        assert_eq!(three.code, 3);
        assert_eq!(three.stderr.trim(), "plain words");
        assert_eq!(three.stdout, "");
        assert!(run("sh", &["-c", "cat >&2; exit 3"]).is_err());
        // 0, and what it printed.
        let zero = run_status("sh", &["-c", "true"], b"").unwrap();
        assert_eq!((zero.code, zero.stdout.as_str()), (0, ""));
        let printed = run_status("printf", &["%s", "plain"], b"").unwrap();
        assert_eq!((printed.code, printed.stdout.as_str()), (0, "plain"));

        // How an end is read: a code, whatever it is, or none (a signal).
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;
        for code in [0, 1, 2, 4, 8, 12, 128] {
            assert_eq!(code_of(ExitStatus::from_raw(code << 8)), Ok(code));
        }
        let killed = code_of(ExitStatus::from_raw(9)).expect_err("a signal is no code");
        assert!(killed.contains("signal 9"), "{killed}");

        // A program that is not there said nothing either.
        let _nothing = fence::FindNothing::here();
        let missing = run_status("printf", &["x"], b"").expect_err("nothing was found");
        assert_eq!(missing.failure, Failure::MissingTool);
    }

    /// **A bounded run answers as `run_status` does, and ends a program out
    /// of time** (for phase A's Windows reads): within its
    /// time, the code and what was printed — far more than a pipe holds, the
    /// part kept; given no time, the program it started is ended at once
    /// and the answer is an error that says so. On the programs the fence
    /// lets any test run; and like every runner, nothing is found with the
    /// fence's `FindNothing` held.
    #[test]
    fn a_bounded_run_answers_in_time_or_ends_its_program() {
        use std::time::Duration;
        let printed = run_status_within("printf", &["%s", "plain"], Duration::from_secs(30))
            .expect("in time");
        assert_eq!((printed.code, printed.stdout.as_str()), (0, "plain"));
        let zero = run_status_within("sh", &["-c", "true"], Duration::from_secs(30)).unwrap();
        assert_eq!((zero.code, zero.stdout.as_str()), (0, ""));
        let long = run_status_within("printf", &["%1000000s", "x"], Duration::from_secs(30))
            .expect("in time");
        assert_eq!(long.stdout.len(), 1_000_000);
        let late =
            run_status_within("printf", &["%1000000s", "x"], Duration::ZERO).expect_err("no time");
        assert_eq!(late.failure, Failure::CommandFailed);
        assert!(
            late.detail.contains("did not end within 0 seconds"),
            "{}",
            late.detail
        );
        let _nothing = fence::FindNothing::here();
        let missing = run_status_within("printf", &["x"], Duration::from_secs(30))
            .expect_err("nothing was found");
        assert_eq!(missing.failure, Failure::MissingTool);
    }

    /// The fence stopped `f`, in its own words.
    fn fenced<R>(what: &str, f: impl FnOnce() -> R + std::panic::UnwindSafe) {
        let words = fence::refusal(f).unwrap_or_else(|| panic!("{what} ran"));
        assert!(words.contains("test build:"), "{what}: {words}");
    }

    // In the tests of the fence every command it must refuse is one that
    // would do no harm were it let through (printf printing a path, a
    // program asked for its version): a fence that failed shows as a test
    // that failed, never as a disk that was touched.

    #[test]
    fn run_refuses_a_device_argument_in_a_test_build() {
        let scratch = std::env::temp_dir().join(format!("lxb-fence-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        let link = scratch.join("disk");
        std::os::unix::fs::symlink("/dev/null", &link).unwrap();
        let link = link.to_str().unwrap();
        for argument in [
            "/dev/sda",
            "--opt=/dev/sda",
            "-d/dev/x",
            "../../dev/x",
            "/proc/1/mountinfo",
            "/sys/block/x",
            "/dev",
            "-d/dev",
            "--opt=..",
            link,
        ] {
            // printf is on the list, so the argument is the only reason left.
            fenced(argument, || run("printf", &["%s", argument]));
            fenced(argument, || run_input("printf", &[argument], b"", false));
            fenced(argument, || {
                run_streaming("printf", &[argument], &mut |_| {})
            });
            fenced(argument, || run_status("printf", &[argument], b""));
            fenced(argument, || {
                run_status_within("printf", &[argument], std::time::Duration::from_secs(5))
            });
        }
        std::fs::remove_dir_all(&scratch).unwrap();
        // The same program with a plain argument runs.
        assert_eq!(run("printf", &["%s", "plain"]).unwrap(), "plain");
    }

    /// An argument under `/run` is refused by its letters, before anything
    /// is looked up (without it, an strace record of the tests showed one
    /// resolved component by component, `readlink /run`, before its program
    /// was refused). Checked by inspection and by the record, never by
    /// running the fence without it.
    #[test]
    fn run_refuses_an_argument_under_run_before_looking_at_it() {
        for argument in [
            "/run/archiso/airootfs/var",
            "--opt=/run/udev/rules.d",
            "-d/run/lxb-installer",
            "/run",
        ] {
            fenced(argument, || run("printf", &["%s", argument]));
            let words = fence::refusal(|| run("printf", &["%s", argument])).unwrap();
            assert!(words.contains("points at the machine"), "{words}");
        }
    }

    #[test]
    fn run_refuses_a_program_off_the_test_list() {
        fenced("true", || run("true", &[]));
        fenced("echo", || run_streaming("echo", &["x"], &mut |_| {}));
        // The partitioning tools without the image tests' permit.
        fenced("sfdisk", || run("sfdisk", &["--version"]));
        fenced("wipefs", || run("wipefs", &["--version"]));
        // A tool whose exit code is its answer, asked for its version only.
        fenced("e2fsck", || run_status("e2fsck", &["-V"], b""));
        // A chroot is a program too (arch-chroot), and on no list.
        fenced("arch-chroot", || {
            chroot(Path::new("/nonexistent-lxb-root"), "true", &[])
        });
        // A permit is for its own program, and for its own image given once.
        // (Granted here as `images::Permit` grants it, and withdrawn however
        // the block ends.)
        struct Granted;
        impl Drop for Granted {
            fn drop(&mut self) {
                fence::withdraw();
            }
        }
        let image = "/nonexistent-lxb-images/fence.img";
        {
            fence::grant("wipefs", image);
            let _granted = Granted;
            fenced("sfdisk with wipefs' permit", || {
                run("sfdisk", &["--version", image])
            });
        }
        {
            fence::grant("sfdisk", image);
            let _granted = Granted;
            fenced("sfdisk without its image", || run("sfdisk", &["--version"]));
            fenced("sfdisk with its image twice", || {
                run("sfdisk", &["--version", image, image])
            });
            fenced("sfdisk with another path", || {
                run(
                    "sfdisk",
                    &["--version", image, "/nonexistent-lxb-images/other"],
                )
            });
        }
    }

    #[test]
    fn run_refuses_root() {
        {
            let _root = fence::Pretend::root();
            fenced("printf as root", || run("printf", &["x"]));
            fenced("sh as root", || {
                run_streaming("sh", &["-c", "true"], &mut |_| {})
            });
        }
        // Root was the only reason: the same commands run as anyone else.
        assert_eq!(run("printf", &["x"]).unwrap(), "x");
        run_streaming("sh", &["-c", "true"], &mut |_| {}).unwrap();
    }

    #[test]
    fn run_refuses_a_shell_off_its_two_lines() {
        // Each would print a word or end at once, were it let through.
        fenced("a script on standard input", || {
            run_input("sh", &[], b"exit 0", false)
        });
        fenced("sh -s", || run_input("sh", &["-s"], b"exit 0", false));
        fenced("a script of its own", || run("sh", &["-c", "exit 0"]));
        // A path put together as the script runs, which no rule of letters
        // could see in the argument.
        fenced("a path put together", || {
            run("sh", &["-c", "d=de; printf %s \"/${d}v/null\""])
        });
        fenced("a line with more after it", || {
            run("sh", &["-c", "true", "extra"])
        });
        fenced("the accounts line with more after it", || {
            run_input("sh", &["-c", "cat >&2; exit 3", "extra"], b"", false)
        });
        fenced("a line through run_streaming", || {
            run_streaming("sh", &["-c", "exit 0"], &mut |_| {})
        });
        fenced("another shell", || run("bash", &["-c", "true"]));
        // The two lines run, exactly as written: one ends at once, the other
        // repeats its standard input on standard error and fails.
        run_streaming("sh", &["-c", "true"], &mut |_| {}).unwrap();
        let stop = run_input("sh", &["-c", "cat >&2; exit 3"], b"plain words", false)
            .expect_err("the accounts line fails");
        assert!(stop.detail.contains("plain words"), "{}", stop.detail);
    }

    #[test]
    fn run_input_refuses_standard_input_that_points_at_the_machine() {
        // printf reads nothing on standard input, so what is sent is the only
        // reason left, and harmless were it let through.
        for input in [
            &b"/dev/sda"[..],
            b"device: /dev/vda\n",
            b"cat /dev > x",
            b"/proc/1/mountinfo",
            b"/sys/block/x",
            b"/run/udev/rules.d/x",
            b"../../x",
            b"secret-7f3a/dev/sda",
        ] {
            for secret in [false, true] {
                let words = fence::refusal(|| run_input("printf", &["x"], input, secret))
                    .unwrap_or_else(|| panic!("{input:?} was sent"));
                assert!(words.contains("test build:"), "{words}");
                // The refusal never repeats what was sent: it may be a password.
                let sent = String::from_utf8_lossy(input);
                assert!(!words.contains(sent.trim()), "{words}");
                assert!(!words.contains("secret-7f3a"), "{words}");
            }
        }
        // Words without a path in them are sent (and here repeated back on
        // standard error by the accounts test's line).
        let stop = run_input("sh", &["-c", "cat >&2; exit 3"], b"zoe:7f3a", false)
            .expect_err("the accounts line fails");
        assert!(stop.detail.contains("zoe:7f3a"), "{}", stop.detail);
    }

    #[test]
    fn a_thread_finding_nothing_finds_no_program() {
        {
            let _nothing = fence::FindNothing::here();
            let stop = command_path("printf").expect_err("printf was found");
            assert_eq!(stop.failure, Failure::MissingTool);
            // Only this thread: another finds printf meanwhile.
            let found = std::thread::spawn(|| command_path("printf").is_ok());
            assert!(found.join().unwrap());
        }
        assert!(command_path("printf").is_ok());
    }

    #[test]
    fn no_test_reads_this_computers_firmware() {
        // A look at /sys/firmware/efi, harmless were it let through.
        fenced(
            "the firmware's reader",
            crate::worker::layout::Firmware::detect,
        );
    }

    #[test]
    fn file_system_tools_wait_for_the_tests_set_aside() {
        let asked =
            |args: &[&str]| fence::ignored_tests_asked_for(args.iter().map(|a| a.to_string()));
        assert!(!asked(&["lxb_installer-test"]));
        assert!(!asked(&["lxb_installer-test", "images", "--exact"]));
        assert!(asked(&["lxb_installer-test", "--ignored", "images"]));
        assert!(asked(&["lxb_installer-test", "--include-ignored"]));
        // No tool is on two lists, and none of the image tools is one any
        // test may run.
        for tool in fence::ON_IMAGES.iter().chain(fence::ON_SIDE_FILES) {
            assert!(!fence::ANY_TEST.contains(tool), "{tool}");
        }
        for tool in fence::ON_SIDE_FILES {
            assert!(!fence::ON_IMAGES.contains(tool), "{tool}");
        }
    }

    /// **The side-file fence judges every argument** (step Q8), not only
    /// those holding a `/`: each side-file tool goes on with its side file
    /// and the words its tests give it, and is refused a blkid tag anywhere,
    /// a name relative to where it runs, a flag that names another file
    /// (debugfs' request file, data source device and undo file; e2fsck's
    /// undo file, external journal and extended options; mke2fs' directory
    /// and journal options), a debugfs request that copies a file out or in,
    /// or any other — each for what it is. Judged by `side_file_words` itself
    /// and, a permit held, by the fence's `check`, which only judges: nothing
    /// in this test starts a program, whatever either answers. (Checked by
    /// inspection, never by a build without the fence.)
    #[test]
    fn the_side_file_fence_judges_every_argument() {
        struct Granted;
        impl Drop for Granted {
            fn drop(&mut self) {
                fence::withdraw();
            }
        }
        let image = "/nonexistent-lxb-images/side.img";
        let fine: &[(&str, &[&str])] = &[
            ("mkfs.ext4", &["-F", image]),
            ("mkntfs", &["-F", "-Q", image]),
            ("dumpe2fs", &["-h", image]),
            ("e2fsck", &["-f", "-n", image]),
            ("e2fsck", &["-f", "-y", image]),
            ("resize2fs", &["-P", image]),
            ("resize2fs", &[image, "262144K"]),
            ("ntfsresize", &["--info", "--no-progress-bar", image]),
            (
                "ntfsresize",
                &[
                    "--no-action",
                    "--no-progress-bar",
                    "--size",
                    "536870912",
                    image,
                ],
            ),
            (
                "ntfsresize",
                &["--no-progress-bar", "--size", "536870912", image],
            ),
            (
                "ntfsresize",
                &["--force", "--no-progress-bar", "--size", "469762048", image],
            ),
            ("ntfscat", &[image, "hiberfil.sys"]),
            ("debugfs", &["-w", "-R", "mknod head p", image]),
            ("debugfs", &["-w", "-R", "sif head mode 0100644", image]),
            ("debugfs", &["-w", "-R", "fallocate head 0 69999", image]),
            (
                "debugfs",
                &["-w", "-R", "zap_block -f tail -p 0x5a 0", image],
            ),
            ("debugfs", &["-w", "-R", "sif tail links_count 2", image]),
            ("debugfs", &["-w", "-R", "rm head", image]),
            ("debugfs", &["-R", "bmap tail 0", image]),
            ("debugfs", &["-R", "bd -f tail 12799", image]),
        ];
        for (program, args) in fine {
            assert_eq!(
                fence::side_file_words(program, args, image),
                Ok(()),
                "{program} {args:?}"
            );
        }
        // Each refused for its reason — by its letters, and through the
        // fence where the image is the one path given (the others meet the
        // fence's rule of one path first).
        let tag = "by its blkid tag";
        let relative = "a name relative to where it runs";
        let copies = "copies a file between the side file and this computer";
        let refused: &[(&str, &[&str], &str)] = &[
            ("e2fsck", &["-f", "-n", "LABEL=root", image], "(LABEL=)"),
            ("e2fsck", &["-f", "-n", "label=root", image], "(LABEL=)"),
            ("dumpe2fs", &["-h", "UUID=0a1b", image], "(UUID=)"),
            ("resize2fs", &["-P", "PARTUUID=1234", image], "(PARTUUID=)"),
            (
                "e2fsck",
                &["-f", "-n", "PARTLABEL=Linux", image],
                "(PARTLABEL=)",
            ),
            ("mkfs.ext4", &["-F", "TYPE=ext4", image], "(TYPE=)"),
            ("debugfs", &["-R", "bd ID=7 0", image], "(ID=)"),
            (
                "ntfsresize",
                &["--info", "--no-progress-bar", "UUID=x", image],
                tag,
            ),
            ("e2fsck", &["-f", "-n", "side.img", image], relative),
            ("resize2fs", &["-P", "sda1", image], relative),
            ("mkntfs", &["-F", "-Q", "volume", image], relative),
            (
                "ntfscat",
                &[image, "hiberfil.sys", "pagefile.sys"],
                relative,
            ),
            ("debugfs", &["-f", "requests", image], "a file of requests"),
            ("debugfs", &["-d", "data", image], "a data source device"),
            ("debugfs", &["-z", "undo", image], "an undo file"),
            ("debugfs", &["-w", "-R", "dump tail out", image], copies),
            ("debugfs", &["-w", "-R", "rdump tail out", image], copies),
            (
                "debugfs",
                &["-w", "-R", "dump_inode tail out", image],
                copies,
            ),
            ("debugfs", &["-w", "-R", "write in tail", image], copies),
            (
                "debugfs",
                &["-R", "logdump -f out", image],
                "not a request a side-file test",
            ),
            (
                "debugfs",
                &["-R", "bd -f tail 0;rm head", image],
                "which no request needs",
            ),
            (
                "debugfs",
                &["-R", "bd -f ../tail 0", image],
                "which no request needs",
            ),
            ("debugfs", &["-R"], "is given no value"),
            ("e2fsck", &["-f", "-z", "undo", image], "an undo file"),
            (
                "e2fsck",
                &["-f", "-j", "journal", image],
                "an external journal",
            ),
            (
                "e2fsck",
                &["-f", "-E", "journal_only", image],
                "extended options",
            ),
            ("e2fsck", &["-fzundo", image], "never give it"),
            (
                "mkfs.ext4",
                &["-F", "-d", "tree", image],
                "a directory to fill it from",
            ),
            (
                "mkfs.ext4",
                &["-F", "-J", "device=x", image],
                "journal options",
            ),
            ("mkfs.ext4", &["-F", "-z", "undo", image], "an undo file"),
            ("ntfsresize", &["--size", "1e9", image], "is not a size"),
            ("ntfsresize", &["--size", "--force", image], "is not a size"),
            ("resize2fs", &[image, "12KK"], "is not a size"),
            ("ntfscat", &[image, "pagefile.sys"], "not the one file"),
            ("ntfscat", &["-f", image, "hiberfil.sys"], "never give it"),
            ("dumpe2fs", &["-h"], "not given its side file"),
            ("dumpe2fs", &["-h", image, image], "given twice"),
            ("sfdisk", &["--json", image], "no side-file shape"),
        ];
        for (program, args, why) in refused {
            let said = fence::side_file_words(program, args, image)
                .expect_err(&format!("{program} {args:?} went through"));
            assert!(said.contains(why), "{program} {args:?}: {said}");
            let one_path = args.iter().filter(|arg| arg.contains('/')).count() == 1;
            if !one_path || !fence::ON_SIDE_FILES.contains(program) {
                continue;
            }
            fence::grant(program, image);
            let _granted = Granted;
            let words = fence::refusal(|| fence::check(program, args))
                .unwrap_or_else(|| panic!("the fence let {program} {args:?} through"));
            assert!(words.contains(why), "{program} {args:?}: {words}");
        }
    }

    /// The side file itself, once the tests set aside are asked for: only a
    /// regular file the test made, directly in the images' folder, as the
    /// kernel resolves it — not a path that is not there, a file elsewhere,
    /// nor a link in the folder to one elsewhere.
    #[test]
    fn a_side_file_is_one_a_test_made_in_the_images_folder() {
        use crate::worker::partition::images::{Image, IMAGES, TABLE_TOOLS};
        std::fs::create_dir_all(IMAGES).unwrap();
        let made = Image::create("side-fence", 4096, 512, TABLE_TOOLS);
        assert_eq!(fence::side_file_made(&made.text), Ok(()));
        let scratch = std::env::temp_dir().join(format!("lxb-side-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        let elsewhere = scratch.join("side.img");
        std::fs::write(&elsewhere, b"").unwrap();
        let link =
            std::path::Path::new(IMAGES).join(format!("side-link-{}.img", std::process::id()));
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        for (what, path) in [
            ("not there", "/nonexistent-lxb-images/side.img".to_string()),
            ("elsewhere", elsewhere.to_string_lossy().into_owned()),
            ("a link to elsewhere", link.to_string_lossy().into_owned()),
        ] {
            let said = fence::side_file_made(&path).expect_err(what);
            assert!(
                said.contains("is not a side file a test made"),
                "{what}: {said}"
            );
        }
        std::fs::remove_file(&link).unwrap();
        std::fs::remove_dir_all(&scratch).unwrap();
    }

    /// The file system tools on `ON_SIDE_FILES` (step Q8's) are refused, a
    /// permit for each held, to a test binary not asked for the tests set
    /// aside — a plain `cargo test`, makepkg's `check()` among them — given
    /// exactly the words of their tests, so that this is the reason. Each
    /// would only be given an image path that cannot exist, were it let
    /// through, which the fence's last question (`side_file_made`) refuses
    /// too. (Run with `--include-ignored`, the fence lets them through to
    /// that question by design, so the test then says so and stops.)
    #[test]
    fn side_file_tools_wait_for_the_tests_set_aside_even_with_a_permit() {
        if fence::ignored_tests_asked_for(std::env::args()) {
            eprintln!("asked for the tests set aside: the side-file tools may run");
            return;
        }
        struct Granted;
        impl Drop for Granted {
            fn drop(&mut self) {
                fence::withdraw();
            }
        }
        let image = "/nonexistent-lxb-images/side.img";
        // Each tool given only what its side-file tests give it, so that the
        // tests set aside are the one reason left.
        let shaped: &[(&str, &[&str])] = &[
            ("mkfs.ext4", &["-F", image]),
            ("debugfs", &["-R", "bmap tail 0", image]),
            ("dumpe2fs", &["-h", image]),
            ("e2fsck", &["-f", "-n", image]),
            ("resize2fs", &["-P", image]),
            ("mkntfs", &["-F", "-Q", image]),
            ("ntfsresize", &["--info", "--no-progress-bar", image]),
            ("ntfscat", &[image, "hiberfil.sys"]),
        ];
        assert_eq!(
            shaped.iter().map(|(tool, _)| *tool).collect::<Vec<_>>(),
            fence::ON_SIDE_FILES
        );
        for (tool, args) in shaped {
            fence::grant(tool, image);
            let _granted = Granted;
            let words = fence::refusal(|| run(tool, args)).expect("ran");
            assert!(
                words.contains("is not a program this test may run"),
                "{words}"
            );
            let words = fence::refusal(|| run_status(tool, args, b"")).expect("ran");
            assert!(
                words.contains("is not a program this test may run"),
                "{words}"
            );
        }
    }
}
