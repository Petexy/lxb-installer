//! What the storage half of the worker hands the system half: a target root
//! that is partitioned, formatted and mounted, and the facts about it that
//! fstab, the swap file and the boot loader need.
//!
//! CONTRACT (phase 0): storage produces a [`Mounted`]; everything after the
//! copy consumes it. Paths in `Mounted` are absolute on the live system.
use crate::model::config::Filesystem;
use crate::model::disks::Table;
use crate::model::protocol::Failure;
use std::path::PathBuf;

/// Which firmware started the machine moved to `model::firmware`, which the
/// interface and the rule set share; it is read from the machine here, as it
/// was.
pub use crate::model::firmware::Firmware;

impl Firmware {
    /// UEFI when the kernel was started by UEFI firmware.
    pub fn detect() -> Self {
        // In a test build the fence refuses before anything is looked at:
        // no test reads this computer's firmware (see `run::fence`).
        #[cfg(test)]
        crate::worker::run::fence::firmware();
        if std::path::Path::new("/sys/firmware/efi").is_dir() {
            Firmware::Uefi
        } else {
            Firmware::Bios
        }
    }
}

/// Where the EFI system partition is mounted inside the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EspMount {
    /// `/boot`: the ESP holds the kernels (an ESP the installer made, or a
    /// shared one with room). Needed for systemd-boot.
    Boot,
    /// `/efi`: a shared ESP too small for kernels, holding the boot loader
    /// only; the kernels are on a Linux boot partition the installer made,
    /// mounted at `/boot` ([`Mounted::linux_boot`]).
    Efi,
}

impl EspMount {
    pub fn path(self) -> &'static str {
        match self {
            EspMount::Boot => "/boot",
            EspMount::Efi => "/efi",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Esp {
    pub device: String,
    pub uuid: String,
    pub mount: EspMount,
    /// The ESP existed before (dual boot): never formatted, other systems'
    /// files are never touched.
    pub shared: bool,
}

/// The Linux boot partition (XBOOTLDR) the installer made for the kernels.
#[derive(Clone, Debug)]
pub struct LinuxBoot {
    pub device: String,
    pub uuid: String,
}

#[derive(Clone, Debug)]
pub struct Mounted {
    pub firmware: Firmware,
    /// The disk's partition table after partitioning.
    pub table: Table,
    /// The whole disk (`/dev/sda`), where BIOS GRUB is installed.
    pub disk: String,
    /// The target root, mounted with everything below it (ESP, /home, Btrfs
    /// subvolumes). The system half writes only beneath this.
    pub root: PathBuf,
    pub root_device: String,
    pub root_uuid: String,
    pub filesystem: Filesystem,
    /// `Some("@")` on Btrfs: the subvolume mounted as `/`.
    pub root_subvolume: Option<String>,
    /// `None` on BIOS.
    pub esp: Option<Esp>,
    /// The Linux boot partition (FAT, at `/boot`, holding the kernels) made
    /// beside a shared ESP mounted at `/efi`; `None` otherwise.
    pub linux_boot: Option<LinuxBoot>,
    /// A separate /home partition, if any.
    pub home_device: Option<String>,
    /// The swap file's path inside the target (`/swapfile`, `/swap/swapfile`)
    /// once the storage half has made it; `None` for no swap.
    pub swapfile: Option<PathBuf>,
    /// The complete fstab for the target, by UUID.
    pub fstab: String,
    /// Whether the kernel is told `systemd.gpt_auto=0` (`boot::GPT_AUTO_OFF`)
    /// in every boot loader's entries: true for a layout made by hand, so a
    /// partition it leaves unused whose type says swap or home — an old
    /// system's — is never switched on or mounted at the first start by
    /// systemd's GPT auto-discovery; false on the easy path, whose entries
    /// are as they always were.
    pub gpt_auto_off: bool,
    /// Partitions os-prober is to take for this installation's own, beside
    /// the root, home, EFI and Linux boot partitions it takes already: for a
    /// layout made by hand, every partition it formats or makes and a /home
    /// it keeps (`manual::own_devices`); empty on the easy path.
    pub other_devices: Vec<String>,
}

/// Why a worker step stopped: the identifier the interface words, and the
/// technical detail for Details and the log.
#[derive(Clone, Debug)]
pub struct Stop {
    pub failure: Failure,
    pub detail: String,
}

impl Stop {
    pub fn new(failure: Failure, detail: impl Into<String>) -> Self {
        Self {
            failure,
            detail: detail.into(),
        }
    }
}
