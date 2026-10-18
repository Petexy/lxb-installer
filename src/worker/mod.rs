//! The root worker: the only code that writes to a disk.
//!
//! CONTRACT (phase 0): `main` is the `--worker` entry. It reads one
//! `Request` from standard input, writes `Event`s to standard output (and to
//! `PROGRESS_FILE`), and returns the process exit code. Owned by the system
//! work; storage (`storage`, `partition`) is owned by the storage work.
//!
//! The run has three parts. Before the plan: the worker makes sure it may
//! run at all (on the Linexin live medium — asked first, `live::guard` —
//! root, alone, a request that validates and that the person
//! agreed to, a live session whose own image is being copied, Secure Boot
//! off, the tools it needs) and looks at the image, which the plan depends
//! on; a refusal here names no step. Then `Event::Plan`, and the steps it
//! lists, in order: Check finds the disk again and makes sure nothing on it
//! is in use and that the internet answers if a choice needs it — as late as
//! possible, right before the first write — then storage partitions,
//! formats and mounts, and the system steps turn the copy into an installed
//! system. A step that stops reports where; every installer mount is
//! released before the last event, `Finished` or `Failed`, goes out.
pub mod layout;
pub mod manual;
pub mod measure;
pub mod partition;
pub mod quiet;
pub mod run;
pub mod storage;
pub mod system;
pub mod target;

use crate::model::config::{InstallConfig, Target};
use crate::model::live;
use crate::model::protocol::{Event, Failure, Step};
use layout::{Firmware, Stop};
use std::cell::Cell;
use std::ffi::OsString;
use std::fs::File;
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use system::image::{self, Image};
use system::{check, copy, network, plan, report, Install};

/// Where the installation's log is kept in the installed system.
pub const INSTALLED_LOG: &str = "var/log/lxb-installer.log";

/// The `--worker` entry point. Returns the exit status.
pub fn main() -> i32 {
    if let Some((failure, detail)) = first_refusal(live::on_medium, check::is_root) {
        return refuse(failure, detail);
    }
    check::harden();
    if let Err(detail) = report::prepare_directory() {
        return refuse(Failure::Internal, detail);
    }
    // Before the lock is held the files belong to whichever worker holds it,
    // so a refusal until then goes to standard output only.
    let lock = match check::lock() {
        Ok(lock) => lock,
        Err(stop) => return refuse(stop.failure, stop.detail),
    };
    if let Err(detail) = report::open_files() {
        return refuse(Failure::Internal, detail);
    }
    report::hold_logs();
    run::set_log(Box::new(report::log));
    let code = run_worker();
    report::close_files();
    drop(lock);
    code
}

/// What stops the worker before it does anything at all — before it makes
/// its directory or takes its lock. Off the Linexin live medium first, the
/// hard guard ([`live::guard`]), so that being root changes nothing there;
/// then not being root. `on_medium` and `is_root` are the machine's answers
/// everywhere but in a test.
pub(crate) fn first_refusal(
    on_medium: fn() -> bool,
    is_root: fn() -> bool,
) -> Option<(Failure, String)> {
    if let Err(detail) = live::guard(on_medium) {
        return Some((Failure::NotLiveSession, detail));
    }
    if !is_root() {
        return Some((Failure::NotRoot, "the worker is not running as root".into()));
    }
    None
}

fn refuse(failure: Failure, detail: impl Into<String>) -> i32 {
    report::emit(&Event::Failed {
        step: None,
        failure,
        detail: detail.into(),
    });
    1
}

fn failed(step: Option<Step>, stop: Stop) -> i32 {
    report::emit(&Event::Failed {
        step,
        failure: stop.failure,
        detail: stop.detail,
    });
    1
}

fn run_worker() -> i32 {
    let firmware = Firmware::detect();
    // The request's bytes are wiped as soon as it is parsed; the passwords
    // live on only in the configuration's `Secret`s, which are cleared once
    // the accounts are made and zeroed when dropped.
    //
    // Standard input is read unbuffered: std's stdin buffer is a static that
    // would keep the request's bytes for the rest of the run.
    //
    // SAFETY: descriptor 0 is the process's standard input; ManuallyDrop
    // keeps the File from closing it.
    let stdin = std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(0) });
    let prepared = check::read_request(&*stdin).and_then(|request| {
        check::validate(&request, firmware)?;
        let config = request.config;
        let image = preflight(&config, firmware)?;
        Ok((config, image))
    });
    let (mut config, image) = match prepared {
        Ok(prepared) => prepared,
        Err(stop) => {
            report::release_logs();
            return failed(None, stop);
        }
    };

    let steps = plan::steps(&config, &image);
    report::emit(&Event::Plan {
        steps: steps.clone(),
    });
    report::release_logs();

    // The step a failure belongs to is the last one begun, whichever half
    // began it.
    let current = Cell::new(None);
    let mut reporter = |event: Event| {
        if let Event::Begin { step } = &event {
            current.set(Some(*step));
        }
        report::emit(&event);
    };
    let mut stage = None;
    // A hand-made layout's udisks rule (`quiet::Quiet`): let go of — and the
    // rule taken out — only once every installer mount is released, before
    // the last event, however the run ended.
    let mut quiet = None;
    let result = install(
        &mut config,
        &image,
        &steps,
        firmware,
        &mut reporter,
        &mut stage,
        &mut quiet,
    );
    drop(config);
    match result {
        Ok(()) => {
            drop(quiet);
            report::emit(&Event::Finished);
            0
        }
        Err(stop) => {
            // Released before the failure is reported, so that `Failed` is
            // the last event and nothing is left mounted when it is read.
            if let Some(stage) = &stage {
                tear_down(stage);
            }
            drop(quiet);
            failed(current.get(), stop)
        }
    }
}

/// Everything that decides whether there is an installation to plan.
fn preflight(config: &InstallConfig, firmware: Firmware) -> Result<Image, Stop> {
    let live = live::detect().ok_or_else(|| {
        Stop::new(
            Failure::NotLiveSession,
            "the system was not started from a Linexin live medium",
        )
    })?;
    check::same_image(&live, config)?;
    if firmware == Firmware::Uefi && check::secure_boot() {
        return Err(Stop::new(Failure::SecureBoot, "Secure Boot is on"));
    }
    check::tools(config, firmware)?;
    check::isolate_mounts()?;
    image::inspect(config)
}

/// The steps of the plan, from Check to Finish. A layout made by hand is
/// carried out by `manual::install_storage`, from its own Check on (its
/// udisks rule's guard left in `quiet_slot`); the easy path's Check and
/// storage half are as they were. Neither leaves the stage anywhere but in
/// `stage_slot`.
fn install(
    config: &mut InstallConfig,
    image: &Image,
    steps: &[Step],
    firmware: Firmware,
    report: &mut dyn FnMut(Event),
    stage_slot: &mut Option<PathBuf>,
    quiet_slot: &mut Option<quiet::Quiet>,
) -> Result<(), Stop> {
    report(Event::Begin { step: Step::Check });
    let laid = matches!(config.target, Target::Layout { .. });
    let (online, mounted) = if laid {
        manual::install_storage(
            config,
            image,
            firmware,
            &mut manual::machine::Real,
            report,
            stage_slot,
            quiet_slot,
        )?
    } else {
        check::disk_unchanged(config, firmware)?;
        check::not_busy(config)?;
        let online = network::check(config, image)?;

        let stage = make_stage()?;
        *stage_slot = Some(stage.clone());
        (online, storage::prepare(config, &stage, report)?)
    };
    let stage = stage_slot
        .clone()
        .ok_or_else(|| Stop::new(Failure::Internal, "no stage was made"))?;

    let mut synced = false;
    for step in plan::after_storage(steps) {
        report(Event::Begin { step });
        let mut install = Install {
            config,
            mounted: &mounted,
            image,
            online,
            synced,
            report: &mut *report,
        };
        match step {
            Step::Copy => copy::run(&mut install)?,
            _ => system::run_step(step, &mut install)?,
        }
        synced = install.synced;
        if step == Step::Accounts {
            forget_passwords(config);
        }
    }

    report(Event::Begin { step: Step::Finish });
    // A layout's FAT, exFAT and NTFS partitions at paths of one's own are
    // the new account's, whose numbers are known only now.
    if laid {
        manual::finish(&mounted.root, &config.account.username)?;
    }
    // After every step that installs packages, whose scripts may have put
    // files of root's into the person's new home.
    system::accounts::give_home_back(&mounted.root, config);
    let log = report::log_text().map_err(|detail| Stop::new(Failure::Internal, detail))?;
    target::write(&mounted.root, INSTALLED_LOG, &log, 0o600)?;
    storage::release(&stage)?;
    remove_stage(&stage);
    run::run("sync", &[])?;
    Ok(())
}

/// Nothing after the accounts needs a password, so they are overwritten
/// rather than kept for the rest of the run.
fn forget_passwords(config: &mut InstallConfig) {
    config.account.password.clear();
    if let Some(mut root) = config.account.root_password.take() {
        root.clear();
    }
}

/// A new private directory below `/run/lxb-installer` that the target is
/// mounted under: made by `mkdtemp`, so its name cannot be guessed and
/// nothing else can have made it first, and only root can enter it.
pub(crate) fn make_stage() -> Result<PathBuf, Stop> {
    let mut template = format!("{}/stage-XXXXXX\0", report::DIRECTORY).into_bytes();
    // SAFETY: the template is a writable, NUL-terminated buffer that mkdtemp
    // fills in place and does not keep.
    let made = unsafe { libc::mkdtemp(template.as_mut_ptr().cast()) };
    if made.is_null() {
        return Err(Stop::new(
            Failure::Internal,
            format!(
                "cannot make the installer's private directory: {}",
                std::io::Error::last_os_error()
            ),
        ));
    }
    template.pop();
    Ok(PathBuf::from(OsString::from_vec(template)))
}

/// Removes the stage once nothing is mounted below it. Only the empty
/// directory itself, never anything recursive.
fn remove_stage(stage: &Path) {
    if let Err(error) = std::fs::remove_dir(stage) {
        run::log(&format!(
            "worker: {} was not removed: {error}",
            stage.display()
        ));
    }
}

/// After a failure: every installer mount released, as far as it will go.
fn tear_down(stage: &Path) {
    match storage::release(stage) {
        Ok(()) => remove_stage(stage),
        Err(stop) => run::log(&format!("worker: {}", stop.detail)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::Target;
    use crate::model::secret::Secret;
    use crate::worker::partition::fixtures::{config, disk_empty};

    #[test]
    fn off_the_live_medium_the_worker_does_nothing_even_as_root() {
        let failure = |on_medium: fn() -> bool, is_root: fn() -> bool| {
            first_refusal(on_medium, is_root).map(|(failure, _)| failure)
        };
        assert_eq!(failure(|| false, || true), Some(Failure::NotLiveSession));
        assert_eq!(failure(|| false, || false), Some(Failure::NotLiveSession));
        assert_eq!(failure(|| true, || false), Some(Failure::NotRoot));
        assert_eq!(failure(|| true, || true), None);
    }

    #[test]
    fn passwords_are_forgotten_once_the_accounts_are_made() {
        let mut config = config(disk_empty(512), Target::WholeDisk);
        config.account.password = Secret::new("person-secret");
        config.account.root_password = Some(Secret::new("root-secret"));
        forget_passwords(&mut config);
        assert!(config.account.password.is_empty());
        assert!(config.account.root_password.is_none());
    }
}
