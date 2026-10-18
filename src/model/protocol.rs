//! The protocol between the unprivileged interface and the root worker.
//!
//! The interface writes one [`Request`] as JSON to the worker's standard input
//! and closes it; the worker answers with one [`Event`] per line on standard
//! output. Nothing here is shown to a person as it is: every [`Step`],
//! [`Warning`] and [`Failure`] is an identifier the interface turns into words
//! from its catalogs, and `detail`/`Log` text is technical and only appears
//! behind Details and in the journal ("no tech talk on the face of a panel").
use super::config::InstallConfig;
use serde::{Deserialize, Serialize};

/// The largest request the worker reads, so a broken writer cannot make it
/// buffer without end. 1 MiB since a request can carry a layout made by
/// hand (Unit Q's Q2): the largest the interface allows — four disks of 128
/// GPT entries, every value at its longest, `/`'s disk carried twice — is
/// some 880 KiB, which the worker's test of it holds within this
/// (`check::tests::the_largest_request_the_interface_allows_is_read_whole`).
/// An easy request is two or three KiB.
pub const MAX_REQUEST: u64 = 1024 * 1024;

/// Where a running worker mirrors its events, so an interface that was
/// closed and opened again can show the installation still in progress. The
/// file holds no secret (events never carry one) and is world-readable.
pub const PROGRESS_FILE: &str = "/run/lxb-installer/progress.jsonl";

/// The worker's whole-run lock. A second worker refuses while it is held.
pub const LOCK_FILE: &str = "/run/lxb-installer/lock";

/// The log the worker writes as it goes; it is copied to the installed
/// system as `/var/log/lxb-installer.log` at the end. Never contains a
/// password.
pub const LOG_FILE: &str = "/run/lxb-installer/install.log";

// Deliberately no Debug: the configuration holds passwords.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub config: InstallConfig,
    /// The device paths the person agreed to have erased or formatted, as
    /// the confirmation dialog listed them. The worker works out the same
    /// list from `config` on its own and refuses unless the two are equal,
    /// so an interface bug cannot destroy something that was never shown.
    pub consent: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Event {
    /// The first event: the steps this run will take, in order, so the
    /// interface can say "step 3 of 11".
    Plan { steps: Vec<Step> },
    /// The worker has started a step.
    Begin { step: Step },
    /// Measured progress inside a step (bytes copied, packages installed).
    /// `total` is never 0.
    Progress { step: Step, done: u64, total: u64 },
    /// One technical line for the Details view and the log.
    Log { line: String },
    /// Something did not work but the installed system is still sound.
    Warning { warning: Warning },
    /// The installation is complete and every installer mount is released.
    Finished,
    /// The installation stopped. `step` is where, when it had begun one.
    /// `detail` is technical text for Details and the journal only.
    Failed {
        step: Option<Step>,
        failure: Failure,
        detail: String,
    },
}

/// The steps of an installation, in the order they run. Not every run takes
/// every step: the worker's [`Event::Plan`] says which.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// Re-checking the live system, the image and the disk before writing.
    Check,
    /// Making kept filesystems smaller, before any partition table is
    /// changed: a layout made by hand that shrinks one (Windows', or an
    /// ext4 kept beside Linexin).
    Resize,
    /// Writing the partition table or the new partitions.
    Partition,
    /// Creating filesystems (and Btrfs subvolumes).
    Format,
    /// Copying the system image onto the new root.
    Copy,
    /// Creating the swap file.
    Swap,
    /// Kernel, fstab, package keyring, removing what only the live system needs.
    System,
    /// Language, keyboard and time zone.
    Region,
    /// The person's account, the computer's name and the root account.
    Accounts,
    /// Setting up the chosen desktop and its login screen.
    Desktop,
    /// Updating the installed system from the internet.
    Updates,
    /// Removing the packages the person deselected.
    Packages,
    /// Installing the chosen Flatpak applications.
    Flatpaks,
    /// Removing microcode and graphics drivers this computer does not use.
    Drivers,
    /// Setting up Btrfs snapshots before updates.
    Snapshots,
    /// Building the start-up image (initramfs).
    Initramfs,
    /// Installing the boot loader.
    Bootloader,
    /// Releasing every installer mount and flushing writes.
    Finish,
}

/// A step that went wrong without making the installed system unusable —
/// or, Windows' check after its partition was shrunk, something the person
/// should know of a step that went as planned. Done says each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Warning {
    /// There was no internet connection, so updates were skipped.
    UpdatesSkippedOffline,
    /// Updating failed; the system is as it was on the installation media.
    UpdatesFailed,
    /// One or more Flatpak applications could not be installed.
    FlatpaksFailed,
    /// Some deselected packages are needed by others and were kept.
    PackagesKept,
    /// The firmware did not accept a new boot entry; the disk still boots
    /// through the fallback loader.
    BootEntryNotRegistered,
    /// Snapshots could not be set up.
    SnapshotsUnavailable,
    /// Unused drivers could not be removed; they are harmless.
    DriversKept,
    /// A Windows partition was made smaller. Windows' own mark for a check
    /// of its disk is left set (no `ntfsfix`), so Windows checks it the next
    /// time it starts, and until then Linexin leaves it unmounted (`ntfs3`
    /// refuses it, `nofail` keeps the start going).
    WindowsWillCheck,
}

/// Why the worker stopped. Each one is something the interface can say in
/// ordinary words, with what the person can do about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Failure {
    /// The request could not be read or did not validate.
    InvalidRequest,
    /// The consent list does not match what the request would erase.
    ConsentMismatch,
    /// The worker is not running as root.
    NotRoot,
    /// This is not a Linexin live session.
    NotLiveSession,
    /// Secure Boot is on; the boot chain this installs is not signed.
    SecureBoot,
    /// Another installation is running.
    AlreadyRunning,
    /// A program the installation needs is missing from the live system.
    MissingTool,
    /// The system image is missing, not read-only, or incomplete.
    ImageInvalid,
    /// The chosen disk changed (unplugged, repartitioned) since it was reviewed.
    DiskChanged,
    /// The chosen disk or a partition on it is in use.
    DiskBusy,
    /// There is not enough room for the system.
    NotEnoughSpace,
    /// The EFI system partition that would be shared is too full.
    EspTooFull,
    /// A kept filesystem could not be made smaller as the layout asks:
    /// reading it again said it may not be, its tool refused or failed, or
    /// what it left is not inside its new entry.
    ShrinkRefused,
    /// Windows is hibernated, or was shut down by Fast Startup, somewhere on
    /// this computer, so no Windows partition is made smaller.
    WindowsHibernated,
    /// A filesystem to be made smaller asks to be checked first: errors its
    /// check found and left, or Windows' own mark for a check.
    FilesystemNeedsCheck,
    /// Another Windows partition on this computer than the one to be made
    /// smaller is in use in the live session (mounted, or held), so whether
    /// a Windows keeps it as it left it cannot be read, and no Windows
    /// partition is made smaller.
    WindowsInUse,
    /// Another Windows partition is marked for Windows' own check, so whether
    /// a Windows keeps it cannot be read, and no Windows partition is made
    /// smaller.
    WindowsNeedsCheck,
    /// Another Windows partition could not be read — its tool or its sign of
    /// hibernation failed, it did not answer in time, or it is on a disk
    /// nothing is read of (the installation medium, a read-only disk) — so
    /// no Windows partition is made smaller.
    WindowsUnread,
    /// A BitLocker volume is on this computer: whether the Windows it
    /// belongs to is hibernated cannot be read, so no Windows partition is
    /// made smaller. (Travels as `bitlocker`, the product's one word.)
    #[serde(rename = "bitlocker")]
    BitLocker,
    /// The chosen desktop or option needs the internet and there is none.
    NeedsInternet,
    /// A command failed; `detail` says which.
    CommandFailed,
    /// Anything else; `detail` says what.
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_round_trip_as_one_line_each() {
        let events = [
            Event::Plan {
                steps: vec![Step::Check, Step::Copy, Step::Finish],
            },
            Event::Progress {
                step: Step::Copy,
                done: 5,
                total: 10,
            },
            Event::Failed {
                step: Some(Step::Partition),
                failure: Failure::DiskBusy,
                detail: "sda1 is mounted".into(),
            },
        ];
        for event in events {
            let line = serde_json::to_string(&event).unwrap();
            assert!(!line.contains('\n'));
            assert_eq!(serde_json::from_str::<Event>(&line).unwrap(), event);
        }
    }

    /// **The shrink's identifiers travel as their names, and back** (Unit Q,
    /// Q9): the step, the seven ways a shrink stops and Done's warning, each
    /// written as the kebab-case name an interface of the same version reads
    /// and read back as itself — the worker says them, the interface turns
    /// them into words.
    #[test]
    fn the_shrinks_step_failures_and_warning_round_trip_by_their_names() {
        let said = [
            (
                Event::Begin { step: Step::Resize },
                r#"{"event":"begin","step":"resize"}"#,
            ),
            (
                Event::Progress {
                    step: Step::Resize,
                    done: 1,
                    total: 3,
                },
                r#"{"event":"progress","step":"resize","done":1,"total":3}"#,
            ),
            (
                Event::Warning {
                    warning: Warning::WindowsWillCheck,
                },
                r#"{"event":"warning","warning":"windows-will-check"}"#,
            ),
            (
                Event::Failed {
                    step: Some(Step::Resize),
                    failure: Failure::ShrinkRefused,
                    detail: "resize2fs ended 1".into(),
                },
                r#"{"event":"failed","step":"resize","failure":"shrink-refused","detail":"resize2fs ended 1"}"#,
            ),
            (
                Event::Failed {
                    step: Some(Step::Check),
                    failure: Failure::WindowsHibernated,
                    detail: String::new(),
                },
                r#"{"event":"failed","step":"check","failure":"windows-hibernated","detail":""}"#,
            ),
            (
                Event::Failed {
                    step: None,
                    failure: Failure::FilesystemNeedsCheck,
                    detail: String::new(),
                },
                r#"{"event":"failed","step":null,"failure":"filesystem-needs-check","detail":""}"#,
            ),
            (
                Event::Plan {
                    steps: vec![Step::Check, Step::Resize, Step::Partition],
                },
                r#"{"event":"plan","steps":["check","resize","partition"]}"#,
            ),
            // Each cause the worker reads in another Windows partition.
            (
                Event::Failed {
                    step: Some(Step::Check),
                    failure: Failure::WindowsInUse,
                    detail: String::new(),
                },
                r#"{"event":"failed","step":"check","failure":"windows-in-use","detail":""}"#,
            ),
            (
                Event::Failed {
                    step: Some(Step::Check),
                    failure: Failure::WindowsNeedsCheck,
                    detail: String::new(),
                },
                r#"{"event":"failed","step":"check","failure":"windows-needs-check","detail":""}"#,
            ),
            (
                Event::Failed {
                    step: Some(Step::Check),
                    failure: Failure::WindowsUnread,
                    detail: String::new(),
                },
                r#"{"event":"failed","step":"check","failure":"windows-unread","detail":""}"#,
            ),
            (
                Event::Failed {
                    step: Some(Step::Check),
                    failure: Failure::BitLocker,
                    detail: String::new(),
                },
                r#"{"event":"failed","step":"check","failure":"bitlocker","detail":""}"#,
            ),
        ];
        for (event, line) in said {
            assert_eq!(serde_json::to_string(&event).unwrap(), line);
            assert_eq!(serde_json::from_str::<Event>(line).unwrap(), event);
        }
    }

    /// Every request of the easy path the storage tests' fixtures make —
    /// each fixture disk at 512 and 4096 bytes a sector, with every place on
    /// it as the target and every other choice at its default — and three
    /// more that set every other field: Btrfs, a swap file, a separate
    /// /home formatted on the disk or kept on another, each boot loader's
    /// and desktop's kind, software, a region and an account with an
    /// administrator. Each with the consent list `disks::consent` makes for
    /// it, as the interface hands it over.
    fn easy_requests() -> Vec<(String, Request)> {
        use crate::model::config::{
            Account, Bootloader, Desktop, Filesystem, Home, Keyboard, Region, Software, Storage,
            Target,
        };
        use crate::model::disks;
        use crate::model::secret::Secret;
        use crate::worker::partition::fixtures::{all, config, disk_gpt_plain, targets};
        let request = |config: InstallConfig| Request {
            consent: disks::consent(&config),
            config,
        };
        let mut every = Vec::new();
        for sector in [512, 4096] {
            for (name, disk) in all(sector) {
                for (place, target) in targets(&disk) {
                    let made = request(config(disk.clone(), target));
                    every.push((format!("{name}-{sector}-{place}"), made));
                }
            }
        }
        let (_, windows) = all(512).remove(1);
        let gap = windows.free[0];
        let mut chosen = config(
            windows.clone(),
            Target::FreeSpace {
                start: gap.start,
                size: gap.size,
            },
        );
        chosen.disk.partitions[2].label = "Windows".into();
        chosen.disk.partitions[2].uuid = "0123456789ABCDEF".into();
        chosen.storage = Storage {
            filesystem: Filesystem::Btrfs,
            swap_mib: 8192,
            home: Some(Home {
                path: chosen.disk.partitions[2].path.clone(),
                format: true,
                disk: None,
            }),
        };
        chosen.bootloader = Bootloader::Refind;
        chosen.desktop = Desktop::LineXinBar;
        chosen.software = Software {
            updates: true,
            flatpaks: vec!["org.example.App".into(), "org.example.Other".into()],
            remove: vec!["firefox".into(), "gnome-maps".into()],
            prune_drivers: false,
            snapshots: false,
        };
        chosen.region = Region {
            locale: "pl_PL.UTF-8".into(),
            timezone: "Europe/Warsaw".into(),
            keyboard: Keyboard {
                layout: "pl".into(),
                variant: "dvorak".into(),
                console: "pl2".into(),
            },
        };
        chosen.account = Account {
            full_name: "Zoë Example".into(),
            username: "zoe".into(),
            password: Secret::new("Example-2026"),
            hostname: "zoe-pc".into(),
            root_password: Some(Secret::new("Root-2026")),
        };
        every.push(("every-choice".into(), request(chosen.clone())));
        let mut other = disk_gpt_plain(512);
        other.path = "/dev/vdy".into();
        other.partitions[0].path = "/dev/vdy1".into();
        let mut elsewhere = chosen;
        elsewhere.storage.home = Some(Home {
            path: "/dev/vdy1".into(),
            format: false,
            disk: Some(other),
        });
        elsewhere.bootloader = Bootloader::SystemdBoot;
        elsewhere.desktop = Desktop::Kinexin;
        every.push(("home-elsewhere".into(), request(elsewhere)));
        let mut whole = config(windows, Target::WholeDisk);
        whole.bootloader = Bootloader::Grub;
        every.push(("grub".into(), request(whole)));
        every
    }

    /// FNV-1a over 64 bits: enough to tell any change of a request's bytes.
    fn fnv(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        })
    }

    /// Each easy request as it was written before a request could carry a
    /// layout (`Target::Layout`, Unit Q's Q2): its name, its length in bytes
    /// and the FNV-1a of its JSON.
    const EASY_REQUESTS: &str = include_str!("easy-requests.golden");

    /// **An easy request's JSON is byte for byte what it was** before a
    /// request could carry a hand-made layout (§7.0, Unit Q, "The request"
    /// 1): a worker reads it as it always did, and its consent list is the
    /// one it always was. Where one differs, its JSON is printed whole.
    #[test]
    fn every_easy_request_is_written_byte_for_byte_as_before() {
        let mut said = Vec::new();
        let mut written = std::collections::BTreeMap::new();
        for (name, request) in easy_requests() {
            let json = serde_json::to_string(&request).unwrap();
            said.push(format!(
                "{name} {} {:016x}",
                json.len(),
                fnv(json.as_bytes())
            ));
            written.insert(name, json);
        }
        let golden: Vec<&str> = EASY_REQUESTS.lines().collect();
        let changed: Vec<String> = said
            .iter()
            .filter(|line| !golden.contains(&line.as_str()))
            .map(|line| {
                let name = line.split(' ').next().unwrap_or_default();
                format!("{line}\n{}", written[name])
            })
            .collect();
        assert!(
            changed.is_empty() && said.len() == golden.len(),
            "{} of {} easy requests are written differently:\n{}\n\nevery one now:\n{}",
            changed.len(),
            said.len(),
            changed.join("\n"),
            said.join("\n")
        );
    }
}
