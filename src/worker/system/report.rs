//! Where the worker's events go.
//!
//! Each event is one JSON line on standard output, for the interface that
//! started the worker; the same line is appended to `PROGRESS_FILE`, for an
//! interface that was closed and opened again while the installation runs
//! (it replays the file from the top); and a readable line goes into
//! `LOG_FILE`, which ends up in the installed system as
//! `/var/log/lxb-installer.log`.
//!
//! Standard output may stop being read at any moment — the person closed the
//! window — and that must not stop a disk operation half way, so a failed
//! write there is ignored. The files are the record that survives it.
use crate::model::protocol::{Event, LOG_FILE, PROGRESS_FILE};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::sync::Mutex;

/// The directory every worker file lives in.
pub const DIRECTORY: &str = "/run/lxb-installer";

/// The running worker's process id, so a reopened interface can tell a
/// worker that is still running from a progress file left by one that ended.
pub const PID_FILE: &str = "/run/lxb-installer/worker.pid";

struct Files {
    progress: File,
    log: File,
}

static FILES: Mutex<Option<Files>> = Mutex::new(None);

/// Log lines kept back until the plan has been sent, so that the plan is
/// the first event an interface reads (the checks before it log too). They
/// reach the log file at once; only their events wait.
static HELD: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// Sends one event everywhere it goes.
pub fn emit(event: &Event) {
    send(event, true);
}

fn send(event: &Event, to_log: bool) {
    let Ok(json) = serde_json::to_string(event) else {
        return;
    };
    {
        let mut stdout = std::io::stdout().lock();
        let _ = writeln!(stdout, "{json}");
        let _ = stdout.flush();
    }
    let mut files = FILES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(files) = files.as_mut() {
        // One write per line, so a reader following the file sees whole
        // events (it also waits for the newline before parsing one).
        let _ = files.progress.write_all(format!("{json}\n").as_bytes());
        if to_log {
            let _ = files
                .log
                .write_all(format!("{}\n", readable(event)).as_bytes());
        }
    }
}

/// One technical line: into the log file now, and out as `Event::Log`
/// now or, while lines are held, once the plan has gone out. This is where
/// the worker points `run::set_log`.
pub fn log(line: &str) {
    {
        let mut files = FILES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(files) = files.as_mut() {
            let _ = files.log.write_all(format!("{line}\n").as_bytes());
        }
    }
    {
        let mut held = HELD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(lines) = held.as_mut() {
            lines.push(line.to_string());
            return;
        }
    }
    send(
        &Event::Log {
            line: line.to_string(),
        },
        false,
    );
}

/// From now on log lines wait for [`release_logs`] before they go out as
/// events.
pub fn hold_logs() {
    *HELD.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Vec::new());
}

/// Sends the held log lines as events, in order, and stops holding them.
pub fn release_logs() {
    let lines = HELD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    for line in lines.into_iter().flatten() {
        send(&Event::Log { line }, false);
    }
}

/// The line the log keeps for an event: the technical lines as they are,
/// everything else marked so the steps stand out when reading it.
pub fn readable(event: &Event) -> String {
    match event {
        Event::Log { line } => line.clone(),
        Event::Plan { steps } => {
            let names: Vec<String> = steps.iter().map(|s| kebab(&format!("{s:?}"))).collect();
            format!("== plan: {}", names.join(", "))
        }
        Event::Begin { step } => format!("== {}", kebab(&format!("{step:?}"))),
        Event::Progress { step, done, total } => {
            format!("   {} {done}/{total}", kebab(&format!("{step:?}")))
        }
        Event::Warning { warning } => format!("!! warning: {}", kebab(&format!("{warning:?}"))),
        Event::Finished => "== finished".into(),
        Event::Failed {
            step,
            failure,
            detail,
        } => {
            let at = step
                .map(|s| format!(" in {}", kebab(&format!("{s:?}"))))
                .unwrap_or_default();
            format!(
                "!! failed{at}: {}: {detail}",
                kebab(&format!("{failure:?}"))
            )
        }
    }
}

/// `UpdatesSkippedOffline` → `updates-skipped-offline`, the protocol's own
/// spelling of the identifiers.
fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Makes `/run/lxb-installer` (root, 0755) or checks the one that is there:
/// a directory nobody else could have made, since `/run` belongs to root.
pub fn prepare_directory() -> Result<(), String> {
    let path = Path::new(DIRECTORY);
    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(path).map_err(|e| format!("cannot make {DIRECTORY}: {e}"))?;
        }
        Err(error) => return Err(format!("cannot inspect {DIRECTORY}: {error}")),
    }
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.uid() != 0 {
        return Err(format!("{DIRECTORY} is not a directory owned by root"));
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("cannot set the mode of {DIRECTORY}: {e}"))
}

/// Opens a worker file afresh: truncated, never through a symlink, owned by
/// root with exactly `mode` (the worker's own umask would make it 0600).
/// The worker is the only writer, so writing from the file's own offset is
/// as good as appending.
fn fresh(path: &str, mode: u32) -> Result<File, String> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| format!("cannot open {path}: {e}"))?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.uid() != 0 {
        return Err(format!("{path} is not a regular file owned by root"));
    }
    file.set_permissions(std::fs::Permissions::from_mode(mode))
        .map_err(|e| format!("cannot set the mode of {path}: {e}"))?;
    Ok(file)
}

/// Starts this run's files. Called only while holding the worker lock, so a
/// second worker that is refused never truncates the first one's record.
pub fn open_files() -> Result<(), String> {
    let progress = fresh(PROGRESS_FILE, 0o644)?;
    // The log can name disks, partitions and commands; it holds no secret,
    // but there is no reason for anyone but root to read it on the live
    // system. The progress file is what other users may read.
    let log = fresh(LOG_FILE, 0o600)?;
    let mut pid = fresh(PID_FILE, 0o644)?;
    pid.write_all(format!("{}\n", std::process::id()).as_bytes())
        .map_err(|e| format!("cannot write {PID_FILE}: {e}"))?;
    *FILES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Files { progress, log });
    Ok(())
}

/// Stops writing the files and removes the pid file: from here on no worker
/// is running. The progress file stays, as the record of how this run ended.
pub fn close_files() {
    let files = FILES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take();
    if let Some(files) = files {
        let _ = files.progress.sync_all();
        let _ = files.log.sync_all();
    }
    let _ = std::fs::remove_file(PID_FILE);
}

/// The whole log so far, to be copied into the installed system.
pub fn log_text() -> Result<String, String> {
    std::fs::read_to_string(LOG_FILE).map_err(|e| format!("cannot read {LOG_FILE}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::protocol::{Failure, Step, Warning};

    #[test]
    fn log_lines_name_the_steps_in_the_protocols_spelling() {
        assert_eq!(
            readable(&Event::Plan {
                steps: vec![Step::Check, Step::Bootloader]
            }),
            "== plan: check, bootloader"
        );
        assert_eq!(
            readable(&Event::Warning {
                warning: Warning::UpdatesSkippedOffline
            }),
            "!! warning: updates-skipped-offline"
        );
        assert_eq!(
            readable(&Event::Failed {
                step: Some(Step::Copy),
                failure: Failure::CommandFailed,
                detail: "rsync: exit status 23".into()
            }),
            "!! failed in copy: command-failed: rsync: exit status 23"
        );
        assert_eq!(
            readable(&Event::Log {
                line: "$ locale-gen".into()
            }),
            "$ locale-gen"
        );
    }
}
