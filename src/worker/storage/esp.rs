//! A shared EFI system partition: whether it has room for the kernels.
//!
//! The interface cannot see inside the partition without privilege, so the
//! worker looks before writing anything: it mounts the partition read-only
//! in its own private mount namespace, measures the free space and unmounts
//! it again. A FAT filesystem mounted read-only is never written, not even
//! its dirty flag.
use crate::model::protocol::Failure;
use crate::worker::layout::Stop;
use crate::worker::partition::SharedRoom;
use crate::worker::run;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// How much an EFI system partition must have free moved to the rule set the
/// interface shares (`model::layout`), and is read here as it was.
pub use crate::model::layout::{ESP_BOOT_FREE, ESP_MIN_FREE};

/// What a shared EFI system partition with `free` bytes has room for, or
/// why it cannot be used. Every boot loader works either way: beside a
/// partition with room for the loader only, the kernels are on a Linux boot
/// partition, which systemd-boot reads as well as GRUB and rEFInd do.
pub fn decide(free: u64) -> Result<SharedRoom, Stop> {
    if free < ESP_MIN_FREE {
        return Err(Stop::new(
            Failure::EspTooFull,
            format!("the EFI system partition has {free} bytes free, less than {ESP_MIN_FREE}"),
        ));
    }
    if free >= ESP_BOOT_FREE {
        Ok(SharedRoom::Kernels)
    } else {
        Ok(SharedRoom::LoaderOnly)
    }
}

/// Bytes available to root on the filesystem mounted at `path`.
fn free_bytes(path: &Path) -> Result<u64, Stop> {
    let name = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| Stop::new(Failure::Internal, "mount point holds a NUL byte"))?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `name` is a valid NUL-terminated path and `stats` is writable
    // memory of the right type; statvfs fills it completely on success.
    if unsafe { libc::statvfs(name.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(Stop::new(
            Failure::Internal,
            format!(
                "cannot measure {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ),
        ));
    }
    // SAFETY: statvfs returned 0, so the structure is initialised.
    let stats = unsafe { stats.assume_init() };
    Ok(stats.f_bavail.saturating_mul(stats.f_frsize))
}

/// The arguments of the one mount [`measure`] makes: FAT and nothing else,
/// read-only, and nothing on it run, opened as a device or given its owner's
/// rights. The measure entry holds this very command line to its allowlist
/// before it lets `measure` run (`worker::measure::allowed`).
pub(crate) fn mount_arguments<'a>(device: &'a str, target: &'a str) -> [&'a str; 6] {
    ["-t", "vfat", "-o", "ro,nosuid,nodev,noexec", device, target]
}

/// Mounts `device` read-only at `dir`, measures it and unmounts it again.
pub fn measure(device: &str, dir: &Path) -> Result<u64, Stop> {
    let target = dir
        .to_str()
        .ok_or_else(|| Stop::new(Failure::Internal, "stage path is not UTF-8"))?;
    run::run("mount", &mount_arguments(device, target))?;
    let measured = free_bytes(dir);
    // Unmounted whatever the measurement said; a failure here is reported
    // first, as a mount left behind matters more.
    run::run("umount", &[target])?;
    measured
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_for_kernels_decides_between_boot_and_efi() {
        assert_eq!(decide(ESP_BOOT_FREE).unwrap(), SharedRoom::Kernels);
        assert_eq!(decide(4 * ESP_BOOT_FREE).unwrap(), SharedRoom::Kernels);
        assert_eq!(decide(ESP_BOOT_FREE - 1).unwrap(), SharedRoom::LoaderOnly);
        assert_eq!(decide(ESP_MIN_FREE).unwrap(), SharedRoom::LoaderOnly);
    }

    #[test]
    fn a_partition_too_full_for_a_boot_loader_is_refused() {
        let full = decide(ESP_MIN_FREE - 1).unwrap_err();
        assert_eq!(full.failure, Failure::EspTooFull);
        assert_eq!(decide(0).unwrap_err().failure, Failure::EspTooFull);
    }
}
