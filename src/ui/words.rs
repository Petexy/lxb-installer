//! The installer's identifiers, said in words.
//!
//! The worker and the model never carry a sentence for the face of a panel:
//! a step, a warning, a failure, a refusal and a disk's state are all
//! identifiers, and this is where they become words, from the catalogs, in the
//! language the person chose. Every arm names its message literally — the
//! catalog test can only check an id it can read in the source, and a message
//! looked up by a computed name is one a translation could lose unnoticed.
//!
//! Plain words only (the family's rule: no tech talk in what a person reads).
//! A disk is "Samsung SSD · 1 TB", never `/dev/nvme0n1`; a partition holds
//! "Windows", never `ntfs`; and what went wrong says what can be done about
//! it. The technical detail goes behind Details and into the log.

use crate::i18n;
use crate::model::config::Invalid;
use crate::model::disks::{Blocked, Content, Disk, Partition, Transport};
use crate::model::protocol::{Failure, Step, Warning};

/// A size the way a person reads it on a box: decimal units, one figure
/// after the point below ten, none above — "512 GB", "1 TB", "1.5 TB",
/// "750 MB" — with the decimal mark of the language being spoken.
pub fn size(bytes: u64) -> String {
    const UNITS: [(u64, &str); 4] = [
        (1_000_000_000_000, "tb"),
        (1_000_000_000, "gb"),
        (1_000_000, "mb"),
        (1_000, "kb"),
    ];
    let (scale, unit) = UNITS
        .iter()
        .copied()
        .find(|(scale, _)| bytes >= *scale)
        .unwrap_or((1, "b"));
    let amount = bytes as f64 / scale as f64;
    let written = if amount < 10.0 && scale > 1 {
        let tenths = (amount * 10.0).round() / 10.0;
        if tenths.fract() == 0.0 {
            format!("{tenths:.0}")
        } else {
            format!("{tenths:.1}")
        }
    } else {
        format!("{:.0}", amount.round())
    };
    let amount = i18n::decimal(written);
    match unit {
        "tb" => crate::message!("size-tb", "amount" => amount),
        "gb" => crate::message!("size-gb", "amount" => amount),
        "mb" => crate::message!("size-mb", "amount" => amount),
        "kb" => crate::message!("size-kb", "amount" => amount),
        _ => crate::message!("size-bytes", "amount" => amount),
    }
}

/// What a disk is called on a row: its model and its size, or what kind of
/// disk it is where it reports no model.
pub fn disk_name(disk: &Disk) -> String {
    crate::message!("disk-name", "disk" => disk_called(disk), "size" => size(disk.size))
}

/// What a disk is called in a sentence about something on it: its model, or
/// what kind of disk it is where it reports no model.
pub fn disk_called(disk: &Disk) -> String {
    let model = disk.model.trim();
    if model.is_empty() {
        transport(disk.transport).to_string()
    } else {
        model.to_string()
    }
}

/// What kind of disk a transport makes it, for a disk with no model.
pub fn transport(transport: Transport) -> &'static str {
    match transport {
        Transport::Nvme => i18n::text("disk-kind-nvme"),
        Transport::Sata => i18n::text("disk-kind-sata"),
        Transport::Usb => i18n::text("disk-kind-usb"),
        Transport::Mmc => i18n::text("disk-kind-card"),
        Transport::Virtio => i18n::text("disk-kind-virtual"),
        Transport::Other => i18n::text("disk-kind-other"),
    }
}

/// What a disk holds, in a few words: "Empty", "Windows", "Windows and 2
/// other parts", or "Linux, filling the whole disk" where it was written
/// over the whole disk with no partitions. The parts that hold nothing a
/// person put there (an EFI partition, Microsoft's reserved space, BIOS
/// boot) are not counted as parts a person would recognise.
pub fn disk_holds(disk: &Disk) -> String {
    if let (Some(what), true) = (disk.content, disk.partitions.is_empty()) {
        return crate::message!("disk-holds-whole", "what" => content(what));
    }
    let seen: Vec<&Partition> = disk
        .partitions
        .iter()
        .filter(|part| {
            !matches!(
                part.content,
                Content::Efi | Content::MicrosoftReserved | Content::BiosBoot
            )
        })
        .collect();
    let Some(first) = seen
        .iter()
        .find(|part| part.content == Content::Windows)
        .or(seen.first())
    else {
        return i18n::text("disk-empty").to_string();
    };
    let named = content(first.content).to_string();
    let others = seen.len() - 1;
    if others == 0 {
        named
    } else {
        crate::message!("disk-holds-more", "first" => named, "count" => others as i64)
    }
}

/// What a partition appears to hold.
pub fn content(content: Content) -> &'static str {
    match content {
        Content::Efi => i18n::text("content-efi"),
        Content::BiosBoot => i18n::text("content-bios-boot"),
        Content::MicrosoftReserved => i18n::text("content-reserved"),
        Content::WindowsRecovery => i18n::text("content-recovery"),
        Content::Windows => i18n::text("content-windows"),
        Content::Linux => i18n::text("content-linux"),
        Content::Swap => i18n::text("content-swap"),
        Content::Data => i18n::text("content-data"),
        Content::Empty => i18n::text("content-empty"),
        Content::Other => i18n::text("content-other"),
    }
}

/// Why a disk cannot be chosen, said under it.
pub fn blocked(blocked: &Blocked) -> &'static str {
    match blocked {
        Blocked::InstallationMedia => i18n::text("blocked-installation-media"),
        Blocked::InUse { .. } => i18n::text("blocked-in-use"),
        Blocked::Swap => i18n::text("blocked-swap"),
        Blocked::Holder => i18n::text("blocked-holder"),
        Blocked::ReadOnly => i18n::text("blocked-read-only"),
        Blocked::TooSmall => i18n::text("blocked-too-small"),
        Blocked::Unknown => i18n::text("blocked-unknown"),
    }
}

/// What the installation is doing, while it does it.
pub fn step(step: Step) -> &'static str {
    match step {
        Step::Check => i18n::text("step-check"),
        Step::Resize => i18n::text("step-resize"),
        Step::Partition => i18n::text("step-partition"),
        Step::Format => i18n::text("step-format"),
        Step::Copy => i18n::text("step-copy"),
        Step::Swap => i18n::text("step-swap"),
        Step::System => i18n::text("step-system"),
        Step::Region => i18n::text("step-region"),
        Step::Accounts => i18n::text("step-accounts"),
        Step::Desktop => i18n::text("step-desktop"),
        Step::Updates => i18n::text("step-updates"),
        Step::Packages => i18n::text("step-packages"),
        Step::Flatpaks => i18n::text("step-flatpaks"),
        Step::Drivers => i18n::text("step-drivers"),
        Step::Snapshots => i18n::text("step-snapshots"),
        Step::Initramfs => i18n::text("step-initramfs"),
        Step::Bootloader => i18n::text("step-bootloader"),
        Step::Finish => i18n::text("step-finish"),
    }
}

/// Something that did not work while the installed system is still sound,
/// said on the finished screen.
pub fn warning(warning: Warning) -> &'static str {
    match warning {
        Warning::UpdatesSkippedOffline => i18n::text("warning-updates-offline"),
        Warning::UpdatesFailed => i18n::text("warning-updates-failed"),
        Warning::FlatpaksFailed => i18n::text("warning-flatpaks-failed"),
        Warning::PackagesKept => i18n::text("warning-packages-kept"),
        Warning::BootEntryNotRegistered => i18n::text("warning-boot-entry"),
        Warning::SnapshotsUnavailable => i18n::text("warning-snapshots"),
        Warning::DriversKept => i18n::text("warning-drivers-kept"),
        Warning::WindowsWillCheck => i18n::text("warning-windows-check"),
    }
}

/// Why the installation stopped, and what the person can do about it: a
/// headline and the sentence under it. `step` is where it stopped. Before
/// the plan and in Check the worker has only looked, so a disk that changed
/// or is busy, or an internet that went away, is said with "nothing was
/// erased" — which is not true once Partition has begun, where the same
/// failures can come from a disk already half rewritten. Nor once Resize
/// has (a layout made by hand that shrinks a kept filesystem, before any
/// table is changed): every part of the disk is still there, but the one
/// being shrunk may have changed.
pub fn failure(failure: Failure, step: Option<Step>) -> (&'static str, &'static str) {
    let untouched = matches!(step, None | Some(Step::Check));
    let what_to_do = |untouched_words: &'static str, words: &'static str| {
        i18n::text(if untouched { untouched_words } else { words })
    };
    match failure {
        Failure::InvalidRequest => (
            i18n::text("failure-invalid-request"),
            i18n::text("failure-invalid-request-do"),
        ),
        Failure::ConsentMismatch => (
            i18n::text("failure-consent-mismatch"),
            i18n::text("failure-consent-mismatch-do"),
        ),
        Failure::NotRoot => (
            i18n::text("failure-not-root"),
            i18n::text("failure-not-root-do"),
        ),
        Failure::NotLiveSession => (
            i18n::text("failure-not-live"),
            i18n::text("failure-not-live-do"),
        ),
        Failure::SecureBoot => (
            i18n::text("failure-secure-boot"),
            i18n::text("failure-secure-boot-do"),
        ),
        Failure::AlreadyRunning => (
            i18n::text("failure-already-running"),
            i18n::text("failure-already-running-do"),
        ),
        Failure::MissingTool => (
            i18n::text("failure-missing-tool"),
            i18n::text("failure-missing-tool-do"),
        ),
        Failure::ImageInvalid => (
            i18n::text("failure-image-invalid"),
            i18n::text("failure-image-invalid-do"),
        ),
        Failure::DiskChanged => (
            i18n::text("failure-disk-changed"),
            what_to_do(
                "failure-disk-changed-untouched-do",
                "failure-disk-changed-do",
            ),
        ),
        Failure::DiskBusy => (
            i18n::text("failure-disk-busy"),
            what_to_do("failure-disk-busy-untouched-do", "failure-disk-busy-do"),
        ),
        Failure::NotEnoughSpace => (
            i18n::text("failure-no-space"),
            i18n::text("failure-no-space-do"),
        ),
        Failure::EspTooFull => (
            i18n::text("failure-esp-full"),
            i18n::text("failure-esp-full-do"),
        ),
        Failure::ShrinkRefused => (
            i18n::text("failure-shrink-refused"),
            what_to_do(
                "failure-shrink-refused-untouched-do",
                "failure-shrink-refused-do",
            ),
        ),
        // Only ever said before anything is written: the worker reads the
        // sign of hibernation in Check.
        Failure::WindowsHibernated => (
            i18n::text("failure-windows-hibernated"),
            i18n::text("failure-windows-hibernated-do"),
        ),
        Failure::FilesystemNeedsCheck => (
            i18n::text("failure-needs-check"),
            what_to_do("failure-needs-check-untouched-do", "failure-needs-check-do"),
        ),
        // Like Windows' hibernation, each only ever said before anything is
        // written: what the worker reads again in Check of the Windows
        // partitions beside the one to be shrunk.
        Failure::WindowsInUse => (
            i18n::text("failure-windows-in-use"),
            i18n::text("failure-windows-in-use-do"),
        ),
        Failure::WindowsNeedsCheck => (
            i18n::text("failure-windows-needs-check"),
            i18n::text("failure-windows-needs-check-do"),
        ),
        Failure::WindowsUnread => (
            i18n::text("failure-windows-unread"),
            i18n::text("failure-windows-unread-do"),
        ),
        Failure::BitLocker => (
            i18n::text("failure-bitlocker"),
            i18n::text("failure-bitlocker-do"),
        ),
        Failure::NeedsInternet => (
            i18n::text("failure-needs-internet"),
            what_to_do(
                "failure-needs-internet-untouched-do",
                "failure-needs-internet-do",
            ),
        ),
        Failure::CommandFailed => (
            i18n::text("failure-command"),
            i18n::text("failure-command-do"),
        ),
        Failure::Internal => (
            i18n::text("failure-internal"),
            i18n::text("failure-internal-do"),
        ),
    }
}

/// Why a choice cannot be installed, said beside it.
pub fn invalid(invalid: Invalid) -> &'static str {
    match invalid {
        Invalid::UsernameEmpty => i18n::text("invalid-username-empty"),
        Invalid::UsernameShape => i18n::text("invalid-username-shape"),
        Invalid::UsernameReserved => i18n::text("invalid-username-reserved"),
        Invalid::FullNameShape => i18n::text("invalid-full-name"),
        Invalid::HostnameShape => i18n::text("invalid-hostname"),
        Invalid::PasswordEmpty => i18n::text("invalid-password-empty"),
        Invalid::PasswordMismatch => i18n::text("invalid-password-mismatch"),
        Invalid::PasswordShape => i18n::text("invalid-password-shape"),
        Invalid::RootPasswordEmpty => i18n::text("invalid-root-password-empty"),
        Invalid::LocaleUnknown => i18n::text("invalid-locale"),
        Invalid::TimezoneUnknown => i18n::text("invalid-timezone"),
        Invalid::KeyboardUnknown => i18n::text("invalid-keyboard"),
        Invalid::DiskBlocked => i18n::text("invalid-disk-blocked"),
        Invalid::TargetMissing => i18n::text("invalid-target-missing"),
        Invalid::TargetTooSmall => i18n::text("invalid-target-small"),
        Invalid::TableFull => i18n::text("table-full"),
        Invalid::TargetIsEsp => i18n::text("invalid-target-esp"),
        Invalid::TargetProtected => i18n::text("target-protected"),
        Invalid::HomeInvalid => i18n::text("invalid-home"),
        Invalid::SwapTooLarge => i18n::text("invalid-swap"),
        Invalid::BootloaderUnavailable => i18n::text("invalid-bootloader"),
        Invalid::SourceInvalid => i18n::text("invalid-source"),
        Invalid::LayoutRefused => i18n::text("invalid-layout"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These read the catalogs in whatever language the process speaks, which
    // in a test is the environment's; the numbers in them are what is held,
    // not the words around them.

    #[test]
    fn sizes_are_written_as_a_box_writes_them() {
        assert!(
            size(512_110_190_592).starts_with("512"),
            "{}",
            size(512_110_190_592)
        );
        assert!(size(1_000_204_886_016).starts_with('1'));
        assert!(
            !size(1_000_204_886_016).contains('0'),
            "{}",
            size(1_000_204_886_016)
        );
        let one_and_a_half = size(1_500_000_000_000);
        assert!(one_and_a_half.starts_with("1.5") || one_and_a_half.starts_with("1,5"));
        assert!(size(786_432_000).starts_with("786"));
        assert!(size(64_023_313_408).starts_with("64"));
    }

    #[test]
    fn every_identifier_has_words() {
        for step in [
            Step::Check,
            Step::Resize,
            Step::Copy,
            Step::Bootloader,
            Step::Finish,
        ] {
            assert!(
                !super::step(step).starts_with("step-"),
                "{step:?} has no words"
            );
        }
        for step in [None, Some(Step::Resize), Some(Step::Format)] {
            for failure in [
                Failure::DiskChanged,
                Failure::DiskBusy,
                Failure::NeedsInternet,
                Failure::ShrinkRefused,
                Failure::WindowsHibernated,
                Failure::FilesystemNeedsCheck,
                Failure::WindowsInUse,
                Failure::WindowsNeedsCheck,
                Failure::WindowsUnread,
                Failure::BitLocker,
                Failure::MissingTool,
            ] {
                let (headline, what_to_do) = super::failure(failure, step);
                assert!(!headline.starts_with("failure-") && !what_to_do.starts_with("failure-"));
            }
        }
        assert!(!super::warning(Warning::UpdatesFailed).starts_with("warning-"));
        assert!(!super::warning(Warning::WindowsWillCheck).starts_with("warning-"));
        assert!(!super::invalid(Invalid::PasswordMismatch).starts_with("invalid-"));
        assert!(!super::blocked(&Blocked::InstallationMedia).starts_with("blocked-"));
    }
}
