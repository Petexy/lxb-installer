//! Starting the root worker through polkit and following its events, finding
//! a worker that is already running, and asking the read-only measure entry
//! how full the disks' partitions are.
//!
//! The interface never writes a disk; it asks polkit to start
//! `/usr/bin/lxb-installer --worker` as root, hands it the request on
//! standard input and reads its events back, one JSON line each. pkexec
//! picks the action whose `exec.path` is the program and whose `exec.argv1`
//! is its first argument: `--worker` is the installer's action,
//! `io.github.petexy.lxb-installer.install`, and `--measure` the read-only
//! measurement's, `io.github.petexy.lxb-installer.measure`, whose question
//! says nothing is changed — so neither can be raised under the other's
//! question. On the live ISO polkit answers without a question (the live
//! user is in wheel, which the ISO lets do anything); on an installed system
//! the desktop's own password panel asks.
//!
//! **Nothing is started off the Linexin live medium** ([`live::guard`]):
//! [`start`] is the one place the interface starts a worker and [`measure`]
//! the one place it starts a measurement, and each asks the guard before any
//! thread or program, so on a development machine or an installed system it
//! answers with a refusal and polkit is never asked.
use crate::model::disks::Disk;
use crate::model::live;
use crate::model::measure::{MeasureAnswer, MeasureRequest, MAX_ANSWER};
use crate::model::protocol::{Event, Failure, Request, PROGRESS_FILE};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

/// The installed program polkit is asked to run. Only this path: a binary
/// anybody could replace must never be the one raised to root.
pub const PROGRAM: &str = "/usr/bin/lxb-installer";
#[cfg(not(test))]
const PKEXEC: &str = "/usr/bin/pkexec";
/// In a test build polkit's program is a path that cannot exist, so no
/// test, whatever it presses, can put a password panel on the screen — even
/// if the guard in [`start`] were ever wrong.
#[cfg(test)]
const PKEXEC: &str = "/nonexistent/lxb-installer-test/pkexec";
/// Where a running worker writes its process id (world-readable, no secret).
pub const PID_FILE: &str = "/run/lxb-installer/worker.pid";

fn failed(detail: impl Into<String>) -> Event {
    Event::Failed {
        step: None,
        failure: Failure::Internal,
        detail: detail.into(),
    }
}

/// Whether a file with this owner and mode may be raised to root: owned by
/// root and not writable by anybody else.
pub fn trusted(uid: u32, mode: u32) -> bool {
    uid == 0 && mode & 0o022 == 0
}

/// `PROGRAM` and every directory above it must be trusted, or a person
/// without root could swap the binary between the check and the start.
fn trusted_program() -> Result<(), String> {
    let path = Path::new(PROGRAM);
    let meta =
        std::fs::symlink_metadata(path).map_err(|e| format!("{PROGRAM} is not installed: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{PROGRAM} is not a regular file"));
    }
    for part in path.ancestors() {
        let meta = std::fs::metadata(part).map_err(|e| format!("{}: {e}", part.display()))?;
        if !trusted(meta.uid(), meta.mode()) {
            return Err(format!(
                "{} must be owned by root and writable only by it",
                part.display()
            ));
        }
    }
    Ok(())
}

/// Whether an event ends an installation.
pub fn terminal(event: &Event) -> bool {
    matches!(event, Event::Finished | Event::Failed { .. })
}

/// Starts `pkexec /usr/bin/lxb-installer --worker`, writes `request` to its
/// standard input and returns its events as they arrive. Never blocks the
/// caller; a worker that cannot be started arrives as one `Event::Failed`.
/// Off the Linexin live medium nothing is started: the one event is
/// `NotLiveSession`.
pub fn start(request: Request) -> Receiver<Event> {
    start_on(request, live::on_medium)
}

/// [`start`], whether this is the live medium answered by `on_medium` (a
/// test's own answer). Refused where it says no before any thread is made or
/// any program run: the refusal is in the channel when this returns, and
/// nothing else ever will be.
fn start_on(request: Request, on_medium: fn() -> bool) -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    if let Err(detail) = live::guard(on_medium) {
        // The request holds the passwords, zeroed as it is dropped.
        drop(request);
        let _ = tx.send(Event::Failed {
            step: None,
            failure: Failure::NotLiveSession,
            detail,
        });
        return rx;
    }
    std::thread::spawn(move || {
        if let Err(detail) = follow_worker(request, &tx) {
            let _ = tx.send(failed(detail));
        }
    });
    rx
}

fn follow_worker(request: Request, tx: &Sender<Event>) -> Result<(), String> {
    trusted_program()?;
    let mut payload = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    drop(request);
    let child = Command::new(PKEXEC)
        .arg(PROGRAM)
        .arg("--worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            wipe(&mut payload);
            return Err(format!("could not start {PKEXEC}: {e}"));
        }
    };
    let written = child
        .stdin
        .take()
        .ok_or_else(|| "the worker has no standard input".to_string())
        .and_then(|mut stdin| stdin.write_all(&payload).map_err(|e| e.to_string()));
    // The JSON holds the passwords; it is not needed once the worker has it.
    wipe(&mut payload);
    if let Err(e) = written {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("could not hand the request to the worker: {e}"));
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "the worker has no standard output".to_string())?;
    let mut ended = false;
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        match serde_json::from_str::<Event>(&line) {
            Ok(event) => {
                ended |= terminal(&event);
                if tx.send(event).is_err() {
                    // The window was closed; the worker carries on regardless,
                    // and a reopened window finds it through `running`.
                    return Ok(());
                }
            }
            Err(_) => {
                let _ = tx.send(Event::Log {
                    line: format!("unreadable worker line: {line}"),
                });
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !ended {
        // pkexec exits 126 when the person dismissed the password panel and
        // 127 when they were not allowed.
        return Err(format!("the worker stopped before finishing ({status})"));
    }
    Ok(())
}

/// What asking the measure entry came to: its one line, or why there is none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeasureEvent {
    /// The measure entry's answer: what it read, or why it read nothing
    /// (`MeasureAnswer::Refused`: another installation holds the lock, a disk
    /// could not be listed …).
    Answer(MeasureAnswer),
    /// No answer at all: refused here before anything was started (off the
    /// live medium, `NotLiveSession`), or the measure entry could not be
    /// started, was not allowed to run (the password panel dismissed), or
    /// said nothing that could be read (`Internal`). `detail` is technical,
    /// for the journal only.
    Failed { failure: Failure, detail: String },
}

/// Asks the read-only measure entry — `pkexec /usr/bin/lxb-installer
/// --measure`, under polkit's own read-only action — what it reads of
/// `disks` (the disks as they were listed), and returns its one answer when
/// it comes. Never blocks the caller; a measurement that cannot be started
/// arrives as one `MeasureEvent::Failed`. Off the Linexin live medium
/// nothing is started: the one event is `NotLiveSession`.
///
/// Unlike [`start`] it writes no request that holds a secret, follows no
/// progress file and knows of no pid file (`PID_FILE` is the worker's
/// alone): the answer is one line on the measure entry's standard output,
/// read up to `MAX_ANSWER` and no further, and its standard error — its log
/// — is let go of.
pub fn measure(disks: Vec<Disk>) -> Receiver<MeasureEvent> {
    measure_on(disks, live::on_medium, follow_measure)
}

/// [`measure`], whether this is the live medium answered by `on_medium` and
/// the measurement itself done by `follow` (a test's own, for both).
/// Refused where `on_medium` says no before any thread is made or anything
/// else asked: the refusal is in the channel when this returns, and nothing
/// else ever will be.
fn measure_on(
    disks: Vec<Disk>,
    on_medium: fn() -> bool,
    follow: fn(&MeasureRequest) -> Result<MeasureAnswer, String>,
) -> Receiver<MeasureEvent> {
    let (tx, rx) = mpsc::channel();
    if let Err(detail) = live::guard(on_medium) {
        let _ = tx.send(MeasureEvent::Failed {
            failure: Failure::NotLiveSession,
            detail,
        });
        return rx;
    }
    let request = MeasureRequest { disks };
    // A thread that cannot be made drops its sender, which the interface
    // reads as a measurement that failed.
    let _ = std::thread::Builder::new()
        .name("lxb-installer-measuring".into())
        .spawn(move || {
            let event = match follow(&request) {
                Ok(answer) => MeasureEvent::Answer(answer),
                Err(detail) => MeasureEvent::Failed {
                    failure: Failure::Internal,
                    detail,
                },
            };
            let _ = tx.send(event);
        });
    rx
}

/// The command line a measurement is started with: polkit's program, the
/// installed binary and `--measure`, which only the measure action names as
/// its `exec.argv1`. Nothing else is ever on it.
fn measure_command() -> Command {
    let mut command = Command::new(PKEXEC);
    command.arg(PROGRAM).arg("--measure");
    command
}

/// One measurement: [`trusted_program`] first, then the measure entry
/// started through polkit, `request` written to it and its one line read
/// back ([`read_answer`]).
///
/// The request is written from a thread of its own while the answer is read
/// here: a measure entry that refuses before reading it (another
/// installation holds the lock) prints its refusal and ends, and a request
/// longer than a pipe holds would otherwise wait for ever for a reader that
/// is gone, its refusal never read. An answer that cannot be read ends the
/// measure entry rather than leaving it writing into a pipe nobody reads.
fn follow_measure(request: &MeasureRequest) -> Result<MeasureAnswer, String> {
    trusted_program()?;
    let payload = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    let mut child = measure_command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start {PKEXEC}: {e}"))?;
    let stdin = child.stdin.take();
    let writer = std::thread::Builder::new()
        .name("lxb-installer-measure-request".into())
        .spawn(move || {
            if let Some(mut stdin) = stdin {
                // A measure entry that ended without reading it answered
                // already: its refusal is what is read.
                let _ = stdin.write_all(&payload);
            }
        });
    let answer = match child.stdout.take() {
        Some(stdout) => read_answer(stdout),
        None => Err("the measurement has no standard output".to_string()),
    };
    if answer.is_err() {
        let _ = child.kill();
    }
    if let Ok(writer) = writer {
        let _ = writer.join();
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    // pkexec exits 126 when the person dismissed the password panel and 127
    // when they were not allowed: no line, said with the status.
    answer.map_err(|detail| format!("{detail} ({status})"))
}

/// The measure entry's one line, read from `reader`: at most `MAX_ANSWER`
/// bytes are read, and a longer answer is refused rather than cut — as the
/// measure entry itself refuses to print one. Exactly one line: nothing, a
/// line that is not an answer, or anything after it, is no answer.
fn read_answer(reader: impl Read) -> Result<MeasureAnswer, String> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_ANSWER + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("the measurement could not be read: {}", e.kind()))?;
    if bytes.len() as u64 > MAX_ANSWER {
        return Err(format!(
            "the measurement said more than the {MAX_ANSWER} bytes an answer may be"
        ));
    }
    let (line, rest) = match bytes.iter().position(|byte| *byte == b'\n') {
        Some(end) => (&bytes[..end], &bytes[end + 1..]),
        None => (&bytes[..], &[][..]),
    };
    if line.is_empty() {
        return Err("the measurement said nothing".into());
    }
    if !rest.is_empty() {
        return Err("the measurement said more than one line".into());
    }
    serde_json::from_slice(line).map_err(|e| {
        format!(
            "the measurement's line is not an answer ({:?} at column {})",
            e.classify(),
            e.column()
        )
    })
}

fn wipe(bytes: &mut [u8]) {
    for byte in bytes.iter_mut() {
        // SAFETY: a plain write through a valid exclusive reference; volatile
        // so the compiler cannot drop it as a dead store.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
}

/// Whether a process's `/proc/<pid>/cmdline` (NUL-separated) is the worker.
pub fn is_worker(cmdline: &[u8]) -> bool {
    let mut parts = cmdline.split(|b| *b == 0).filter(|p| !p.is_empty());
    let program = parts.next().unwrap_or_default();
    let program_ok = program == PROGRAM.as_bytes()
        || program.rsplit(|b| *b == b'/').next() == Some(b"lxb-installer".as_slice());
    program_ok && parts.next() == Some(b"--worker".as_slice()) && parts.next().is_none()
}

/// The process id in the worker's pid file.
pub fn parse_pid(text: &str) -> Option<u32> {
    text.trim().parse().ok().filter(|pid| *pid > 1)
}

/// A worker that is already running (the interface was closed and opened
/// again): its events so far and from now on, read from `PROGRESS_FILE`.
pub fn running() -> Option<Receiver<Event>> {
    let pid = parse_pid(&std::fs::read_to_string(PID_FILE).ok()?)?;
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if !is_worker(&cmdline) {
        return None;
    }
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || replay(pid, &tx));
    Some(rx)
}

/// Sends every event already in the progress file, then follows it until a
/// terminal event, or until the worker is gone without writing one.
fn replay(pid: u32, tx: &Sender<Event>) {
    let Ok(file) = std::fs::File::open(PROGRESS_FILE) else {
        let _ = tx.send(failed("the running installation's progress cannot be read"));
        return;
    };
    let mut reader = BufReader::new(file);
    let mut pending = String::new();
    loop {
        let mut chunk = String::new();
        match reader.read_to_string(&mut chunk) {
            Ok(0) | Err(_) => {
                if !Path::new(&format!("/proc/{pid}")).exists() {
                    let _ = tx.send(failed("the installation stopped without saying why"));
                    return;
                }
                std::thread::sleep(Duration::from_millis(250));
                continue;
            }
            Ok(_) => pending.push_str(&chunk),
        }
        while let Some(end) = pending.find('\n') {
            let line: String = pending.drain(..=end).collect();
            if let Ok(event) = serde_json::from_str::<Event>(line.trim()) {
                let done = terminal(&event);
                if tx.send(event).is_err() || done {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::Target;
    use crate::model::measure::{ImageSizes, Measures};
    use crate::worker::partition::fixtures::{config, disk_empty};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::TryRecvError;

    #[test]
    fn off_the_live_medium_no_worker_is_started() {
        let request = Request {
            config: config(disk_empty(512), Target::WholeDisk),
            consent: Vec::new(),
        };
        let events = start_on(request, || false);
        // Refused before any thread: the answer is there already…
        match events.try_recv() {
            Ok(Event::Failed {
                step: None,
                failure: Failure::NotLiveSession,
                ..
            }) => {}
            other => panic!("not refused off the medium: {other:?}"),
        }
        // …and it is the last word: nothing holds the other end, so no
        // thread was left to start anything.
        assert_eq!(events.try_recv(), Err(TryRecvError::Disconnected));
    }

    #[test]
    fn a_test_build_has_no_polkit_to_reach() {
        assert!(PKEXEC.starts_with("/nonexistent/"), "{PKEXEC}");
    }

    #[test]
    fn only_a_root_owned_unwritable_file_is_trusted() {
        assert!(trusted(0, 0o100755));
        assert!(!trusted(1000, 0o100755));
        assert!(!trusted(0, 0o100775));
        assert!(!trusted(0, 0o100757));
    }

    #[test]
    fn the_worker_is_recognised_by_its_exact_command_line() {
        assert!(is_worker(b"/usr/bin/lxb-installer\0--worker\0"));
        assert!(!is_worker(b"/usr/bin/lxb-installer\0--demo\0"));
        assert!(!is_worker(b"/usr/bin/lxb-installer\0--worker\0--extra\0"));
        assert!(!is_worker(b"/usr/bin/bash\0--worker\0"));
        assert!(!is_worker(b""));
    }

    #[test]
    fn a_pid_file_names_one_ordinary_process() {
        assert_eq!(parse_pid("4242\n"), Some(4242));
        assert_eq!(parse_pid("1"), None);
        assert_eq!(parse_pid("not a number"), None);
    }

    #[test]
    fn finished_and_failed_end_an_installation() {
        assert!(terminal(&Event::Finished));
        assert!(terminal(&failed("x")));
        assert!(!terminal(&Event::Log { line: "x".into() }));
    }

    #[test]
    fn wiping_leaves_nothing_behind() {
        let mut bytes = b"{\"password\":\"hunter22\"}".to_vec();
        wipe(&mut bytes);
        assert!(bytes.iter().all(|b| *b == 0));
    }

    /// How many times `followed` was asked to measure: only
    /// `launch_measure_asks_the_live_guard_first` hands it in.
    static FOLLOWED: AtomicUsize = AtomicUsize::new(0);

    /// A measurement of the test's own, in place of polkit and the measure
    /// entry: it counts that it was asked and answers with nothing read.
    fn followed(request: &MeasureRequest) -> Result<MeasureAnswer, String> {
        FOLLOWED.fetch_add(1, Ordering::SeqCst);
        Ok(MeasureAnswer::Measured {
            measures: Measures::default(),
            image: ImageSizes::default(),
            changed: request.disks.iter().map(|disk| disk.path.clone()).collect(),
        })
    }

    /// The guard is asked before anything else: off the live medium the
    /// refusal is in the channel as `measure` returns, nothing holds the
    /// other end (no thread was made) and the measurement was never asked
    /// for; on it — the test's own answer, never this computer's — the
    /// measurement is asked once, on a thread, and its answer arrives.
    #[test]
    fn launch_measure_asks_the_live_guard_first() {
        let disks = || vec![disk_empty(512)];
        let refused = measure_on(disks(), || false, followed);
        match refused.try_recv() {
            Ok(MeasureEvent::Failed {
                failure: Failure::NotLiveSession,
                ..
            }) => {}
            other => panic!("not refused off the medium: {other:?}"),
        }
        assert_eq!(refused.try_recv(), Err(TryRecvError::Disconnected));
        assert_eq!(
            FOLLOWED.load(Ordering::SeqCst),
            0,
            "measured off the medium"
        );
        let asked = measure_on(disks(), || true, followed);
        match asked.recv_timeout(Duration::from_secs(10)) {
            Ok(MeasureEvent::Answer(MeasureAnswer::Measured { changed, .. })) => {
                assert_eq!(changed, ["/dev/vdx"]);
            }
            other => panic!("no answer on the medium: {other:?}"),
        }
        assert_eq!(FOLLOWED.load(Ordering::SeqCst), 1);
    }

    /// A measurement is started as `pkexec /usr/bin/lxb-installer
    /// --measure` and nothing else — the measure action's own `exec.argv1`,
    /// never `--worker` — and in a test build its polkit is the path that
    /// cannot exist. The command is only built here, never started.
    #[test]
    fn a_measurement_is_started_with_the_measure_flag_alone() {
        let command = measure_command();
        assert_eq!(command.get_program(), PKEXEC);
        let args: Vec<&std::ffi::OsStr> = command.get_args().collect();
        assert_eq!(args, [PROGRAM, "--measure"]);
        assert!(PKEXEC.starts_with("/nonexistent/"));
    }

    /// The answer is one line, read to `MAX_ANSWER` and no further: a
    /// measure entry that never stops talking is refused once that much is
    /// read (an endless reader here), and nothing, a second line, or a line
    /// that is not an answer is no answer either.
    #[test]
    fn an_answer_is_one_line_read_within_its_bound() {
        let answer = MeasureAnswer::Refused {
            failure: Failure::AlreadyRunning,
            detail: "the lock is held".into(),
        };
        let line = serde_json::to_string(&answer).unwrap() + "\n";
        assert_eq!(read_answer(line.as_bytes()), Ok(answer.clone()));
        let unended = line.trim_end().to_string();
        assert_eq!(read_answer(unended.as_bytes()), Ok(answer));
        let endless = std::io::repeat(b'x');
        let said = read_answer(endless).unwrap_err();
        assert!(said.contains("more than"), "{said}");
        for wrong in ["", "\n", "not json\n", "{\"answer\":\"mkfs\"}\n"] {
            assert!(read_answer(wrong.as_bytes()).is_err(), "{wrong:?}");
        }
        let twice = format!("{line}{line}");
        assert!(read_answer(twice.as_bytes()).is_err());
    }
}
