//! The swap file.
//!
//! On ext4 the file is created readable by root only before a single byte
//! is in it, then given its size in one call (fallocate); a filesystem that
//! cannot allocate that way gets zeros written instead, with the progress
//! measured. On Btrfs, `btrfs filesystem mkswapfile` makes a file that is
//! not copy-on-write and not compressed, which is what the kernel needs to
//! swap to it.
use super::progress::Steps;
use crate::model::config::Filesystem;
use crate::model::disks::MIB;
use crate::model::protocol::{Failure, Step};
use crate::worker::layout::Stop;
use crate::worker::partition::Swapfile;
use crate::worker::{run, target};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Zeros are written this much at a time when the filesystem cannot
/// allocate the file in one call.
const CHUNK: usize = MIB as usize;
/// Progress is reported every this many chunks.
const REPORT_EVERY: u64 = 64;

fn io(path: &Path, what: &str, error: std::io::Error) -> Stop {
    let failure = if error.raw_os_error() == Some(libc::ENOSPC) {
        Failure::NotEnoughSpace
    } else {
        Failure::Internal
    };
    Stop::new(failure, format!("{what} {}: {error}", path.display()))
}

fn path_str(path: &Path) -> Result<&str, Stop> {
    path.to_str()
        .ok_or_else(|| Stop::new(Failure::Internal, "stage path is not UTF-8"))
}

/// Makes `swap` inside the target mounted at `root`. Reports `Step::Swap`.
pub fn make(
    swap: Swapfile,
    filesystem: Filesystem,
    root: &Path,
    steps: &mut Steps,
) -> Result<(), Stop> {
    let path = target::path(root, swap.path.trim_start_matches('/'))?;
    let path_text = path_str(&path)?;
    match filesystem {
        Filesystem::Btrfs => {
            steps.begin(Step::Swap, 1);
            let size = format!("{}m", swap.mib);
            run::run(
                "btrfs",
                &["filesystem", "mkswapfile", "--size", &size, path_text],
            )?;
            steps.advance();
        }
        Filesystem::Ext4 => {
            steps.begin(Step::Swap, 2);
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&path)
                .map_err(|e| io(&path, "cannot create", e))?;
            allocate(&file, &path, swap.bytes(), steps)?;
            file.sync_all().map_err(|e| io(&path, "cannot write", e))?;
            drop(file);
            // Back to the step's own two units after any measured writing.
            steps.measure(1, 2);
            run::run("mkswap", &[path_text])?;
            steps.measure(2, 2);
        }
    }
    Ok(())
}

fn allocate(file: &File, path: &Path, bytes: u64, steps: &mut Steps) -> Result<(), Stop> {
    let length = libc::off_t::try_from(bytes)
        .map_err(|_| Stop::new(Failure::InvalidRequest, "swap file size out of range"))?;
    // SAFETY: the descriptor is open for writing and owned by `file` for the
    // whole call; fallocate only changes the file's allocation.
    if unsafe { libc::fallocate(file.as_raw_fd(), 0, 0, length) } == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() != Some(libc::EOPNOTSUPP) {
        return Err(io(path, "cannot allocate", error));
    }
    let zeros = vec![0u8; CHUNK];
    let chunks = bytes / CHUNK as u64;
    let mut writer = file;
    for chunk in 0..chunks {
        writer
            .write_all(&zeros)
            .map_err(|e| io(path, "cannot write", e))?;
        if chunk % REPORT_EVERY == 0 {
            steps.measure(chunk, chunks);
        }
    }
    let rest = (bytes % CHUNK as u64) as usize;
    writer
        .write_all(&zeros[..rest])
        .map_err(|e| io(path, "cannot write", e))?;
    Ok(())
}
