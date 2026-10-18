//! **The e2e harness's walks on Partitions, replayed** (`INSTALLER-HANDOFF.md`
//! §7.0 Unit Q, "The campaign, in order", step 0): every key walk a Unit Q
//! scenario presses (`tests/e2e/scenarios.py`, `CAMPAIGN_REPLAYS`), pressed
//! here word for word, so that a walk is proven on the host — and kept in
//! step with the interface after — before any VM, never found out by trial
//! in QEMU.
//!
//! **The keys are the scenarios' own.** Each walk is a string constant below,
//! named after the walk, one token a key as QMP names it (`type:…` for what
//! is typed): Where to install's presses to Partitions (`to_partitions`), the
//! walk, and for a plan Continue on to Desktop (`continue_to_desktop`).
//! `test_unit_q.py`'s `ReplayTests` holds each to what the scenario's walk
//! presses on the disks as `--list-disks` lists its copies, token for token:
//! a walk changed there is a replay changed here.
//!
//! **The machine is the scenario's.** The disks are listed from the lsblk
//! the guest prints for the scenario's copies (`disks::parse_lsblk`, as
//! `--list-disks` lists them), on a live session judged by the scenario's
//! firmware; the measure entry's answer is handed in as it would read the
//! fixtures (each partition's numbers this file's own, where the guest's
//! are not known on the host); a plan goes on through Continue with its
//! scenario's own set alone pretended proven (`layout::PretendProven`, the
//! set `layout::CAMPAIGN` names for it). Nothing here reads this computer:
//! no disk, no firmware, no network, no program — Desktop's question of the
//! internet is a test's own, answered at once.
//!
//! **The toolkit's part is stood in for** (`Hands`), as the frame meets it:
//! a menu opens on the row it is raised with (`Page::menu_on`, `menu_marked`)
//! and its light wraps at both ends; Enter chooses a row, Escape puts it
//! away; a dialog's light starts on its first answer; a sheet stands its
//! rows two by two where the panel is wide enough (`sheets::draw`), and the
//! field the light stands in takes what is typed. The panel is the guest's,
//! 1600×900, and `plan_bios_gpt`'s walk — whose File system row is reached
//! "whichever way the sheet stands its rows" — is pressed on a display
//! standing on its side too.
//!
//! **Each ends where its scenario does**: on the layout the scenario reads
//! back, its exact needs the scenario's set; and then on its question, put
//! up by Install, read and answered page by page as `plan_question` answers
//! it, up to its last page, whose "Erase and install" is never pressed here:
//! it would begin, and the launch looks at this machine, so nothing here
//! starts anything.

use super::*;
use crate::model::disks::{part_type, Facts, Table, GIB, MIB};
use crate::model::layout::{
    Capability, Create, DiskLayout, Fate, Fs, ImageSizes, Measure, Measured, Measures, MountPoint,
    Part, PretendProven, CAMPAIGN,
};
use crate::model::measure::MeasureAnswer;
use crate::ui::screens::sheets::{Kind, Row};
use lxb_app::lxb_toolkit::input::Key as Keyboard;
use lxb_app::lxb_toolkit::metrics::scale_for;
use serde_json::{json, Value};

// -------------------------------------------------------------------------
// The keys.

/// `measure_walk`'s first boot, its four disks attached and dualboot-base's
/// old Linux in use (`to_partitions`, `look_at_the_measured_tiles`).
const MEASURE_WALK_FOUR: &str = "
    down down down down left left ret
    up left left left left right right right down
    left left left left left left right right ret esc
    up left left left left right down
    left left left left left left left left right right ret esc
    up left left left left down
    left left left left left left left left right right ret esc
    up left left left left down
    left left left left left left left left right right right right right ret esc
    up left left left left right right down
    left left left ret esc
    esc
";

/// `measure_walk`'s second boot, without windows-hibernated-base.
const MEASURE_WALK_THREE: &str = "
    down down down left left ret
    up left left left right right down
    left left left left left left right right ret esc
    up left left left down
    left left left left left left left left right right ret esc
    up left left left right down
    left left left ret esc
    esc
";

/// `plan_on_an_empty_disk`.
const PLAN_ON_AN_EMPTY_DISK: &str = "
    down left left ret
    ret ret
    ret ret down down down down down ret
    ret ret up ret esc
    ret down ret up up ret esc
    ret down down ret ret type:1 ret down ret
    ret ret down down down ret esc
    right
    ret ret ret type:28 ret down down down down down ret
    ret up up ret type:E2E-ROOT ret ret
    right
    ret ret ret type:2 ret down down down down down ret
    ret down ret up ret esc
    ret ret up up ret esc
    right
    ret ret ret type:1 ret down down down down down ret
    ret down ret up ret esc
    right
    ret ret ret type:6 ret down down down down down ret
    ret ret up ret type:/var ret ret
    right
    ret ret down down down down down ret
    ret down ret down down ret esc
    ret ret down ret esc
    down ret
";

/// `plan_new_table_over_used`.
const PLAN_NEW_TABLE_OVER_USED: &str = "
    down left left ret
    f10 ret ret
    ret ret down down down down down ret
    ret ret up ret esc
    ret down ret up up ret esc
    ret down down ret ret type:1 ret down ret
    ret ret down down ret esc
    right
    ret ret down down down down down ret
    ret ret up ret esc
    ret down ret up up ret esc
    ret down down ret ret type:2 ret down ret
    ret ret down down down ret esc
    right
    ret ret down down down down down ret
    ret down ret down ret esc
    down ret
";

/// `plan_beside_windows`, which `refuse-manual-changed` walks too.
const PLAN_BESIDE_WINDOWS: &str = "
    down left left ret
    left left left left left left left left left
    ret ret down ret esc
    left left left left left left left left left right right
    ret ret up ret type:/mnt/windows ret ret
    left left left left left left left left left right right right
    ret up ret
    ret ret down down down down down ret
    ret down down ret ret type:28 ret down ret
    right
    ret ret down down down down down ret
    ret down ret up up ret esc
    ret ret down down down ret esc
    right right
    ret down ret down ret esc
    ret down down ret ret type:16 ret down ret
    ret ret down down ret esc
    right
    ret ret ret type:4 ret down down down down down ret
    ret down ret up ret esc
    ret ret up up ret esc
    down ret
";

/// `plan_three_disks`.
const PLAN_THREE_DISKS: &str = "
    down down down left left ret
    up left left left down
    left left left
    ret ret
    ret ret ret type:28 ret down down down down down ret
    right
    ret ret down down down down down ret
    ret down ret up ret esc
    ret ret up up ret esc
    up left left left right down
    ret ret
    ret ret down down down down down ret
    ret down ret up up ret esc
    ret down down ret ret type:1 ret down ret
    ret ret down down ret esc
    right
    ret ret ret type:2 ret down down down down down ret
    ret down ret up up ret esc
    ret ret down down down ret esc
    up left left left right right down
    left left
    ret ret down down ret esc
    down ret
";

/// `plan_bios_gpt`.
const PLAN_BIOS_GPT: &str = "
    down left left ret
    ret ret
    ret ret down down down down up up ret up ret down ret
    right
    ret ret ret type:28 ret down down down down down ret
    right
    ret ret down down down down down ret
    ret ret down down ret esc
    down ret
";

/// `plan_bios_keep_fat`.
const PLAN_BIOS_KEEP_FAT: &str = "
    down left left ret
    ret ret up ret type:/mnt/data ret ret
    right
    ret ret ret type:32 ret down down down down down ret
    right
    ret ret down down down down down ret
    ret down ret up ret esc
    ret ret up up ret esc
    down ret
";

/// `plan_bios_logical`.
const PLAN_BIOS_LOGICAL: &str = "
    down left left ret
    left left left left left left left left left
    ret down ret down ret esc
    ret ret down ret esc
    left left left left left left left left left right right right right
    ret up ret
    left left left left left left left left left right right right
    ret ret up ret esc
    ret ret up up ret esc
    ret down down ret ret type:13 ret down ret
    left left left left left left left left left right right
    ret ret up ret type:/data ret ret
    down ret
";

/// `plan_shrink_windows`, which `refuse-ntfs-hibernated` walks too.
const PLAN_SHRINK_WINDOWS: &str = "
    down left left ret
    left left left left left left left
    ret ret down ret esc
    left left left left left left left right right
    ret esc
    ret down down ret ret type:33.152344 ret down ret
    ret ret up ret type:/windows ret ret
    right
    ret ret ret type:28 ret down down down down down ret
    right
    ret ret down down down down down ret
    ret down ret up up ret esc
    ret ret down down down ret esc
    f10 up ret up ret esc
    down ret
";

/// `plan_shrink_ext4`, which `refuse-manual-busy-shrink` walks too.
const PLAN_SHRINK_EXT4: &str = "
    down down left left ret
    up left left down
    left left left
    ret ret
    ret ret down down down down down ret
    ret ret up ret esc
    ret down ret up up ret esc
    ret down down ret ret type:1 ret down ret
    ret ret down down down ret esc
    right
    ret ret down down down down down ret
    ret down ret down ret esc
    up left left right down
    ret esc
    ret down down ret ret type:16 ret down ret
    ret ret up ret type:/data ret ret
    right
    ret ret down down down down down ret
    ret ret up ret type:/var ret ret
    down ret
";

/// `plan_reinstall_keep_home`.
const PLAN_REINSTALL_KEEP_HOME: &str = "
    down left left ret
    left left left left left left left
    ret ret up ret esc
    left left left left left left left right
    ret down ret down ret esc
    ret ret down ret esc
    left left left left left left left right right
    ret ret up ret esc
    ret ret up up ret esc
    left left left left left left left right right right
    ret ret down down ret esc
    down ret
";

/// `plan_bios_windows_keep_home`.
const PLAN_BIOS_WINDOWS_KEEP_HOME: &str = "
    down left left ret
    left left left left left left right right
    ret ret down down ret esc
    left left left left left left right
    ret down ret down ret esc
    ret ret down ret esc
    left left left left left left
    ret esc
    ret down down ret ret type:28 ret down ret
    right
    ret ret down down down down down ret
    ret down ret up ret esc
    ret ret up up ret esc
    down ret
";

/// A key the harness presses, or text it types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key<'a> {
    Press(Action),
    Type(&'a str),
}

/// `walk`'s keys, one token a key as QMP names it, each the keyboard's key
/// it presses, turned into a press as the toolkit turns that key into one
/// (`Action::of_key`): the arrows, Enter (`ret`) to Accept, Escape to Back,
/// Tab to Next, Shift+Tab to Previous, F10 to Menu. Shift+Tab is Tab with
/// Shift held, which lxb-app reads as BackTab (`lxb_input::key_of_logical`,
/// with Shift as the keyboard last reported it).
///
/// **No walk presses Tab or Shift+Tab**: each changes disks by the chips
/// (`show_disk`). An lxb-app from before lxb-toolkit's fix hands every key
/// on as though Shift were not held (it gives `lxb_input::key_of` `false`
/// for it), so in a guest whose installer was built against one Shift+Tab
/// reaches the installer as Tab, Next — and a walk pressing it would look
/// at every disk after the first on the last one. The chips walk the same
/// on a package built against either toolkit.
fn keys(walk: &str) -> Vec<Key<'_>> {
    walk.split_whitespace()
        .map(|token| {
            if let Some(text) = token.strip_prefix("type:") {
                return Key::Type(text);
            }
            let key = match token {
                "down" => Keyboard::Down,
                "up" => Keyboard::Up,
                "left" => Keyboard::Left,
                "right" => Keyboard::Right,
                "ret" => Keyboard::Enter,
                "esc" => Keyboard::Escape,
                "tab" => Keyboard::Tab,
                "shift+tab" => Keyboard::BackTab,
                "f10" => Keyboard::F10,
                other => panic!("no key is called {other:?}"),
            };
            Key::Press(Action::of_key(key).unwrap_or_else(|| panic!("{token} is no press")))
        })
        .collect()
}

/// A walk's keys are pressed as the toolkit reads them ([`keys`]): Tab to
/// Next and Shift+Tab, which a walk could not press while lxb-app handed it
/// on as Tab, to Previous.
#[test]
fn a_walks_keys_are_pressed_as_the_toolkit_reads_them() {
    let pressed = [
        Action::Down,
        Action::Up,
        Action::Left,
        Action::Right,
        Action::Accept,
        Action::Back,
        Action::Next,
        Action::Previous,
        Action::Menu,
    ]
    .map(Key::Press);
    assert_eq!(
        keys("down up left right ret esc tab shift+tab f10 type:28"),
        [&pressed[..], &[Key::Type("28")]].concat()
    );
}

// -------------------------------------------------------------------------
// The disks, as the guest lists the scenarios' copies.

/// A partition as the guest's lsblk prints it (`--json --bytes --paths`):
/// its number, where it starts and how long it is (bytes), its type (as
/// discovery spells it), filesystem, label and filesystem UUID — the
/// fixture's own, from its manifest — and what the measure entry answers
/// of it, where it reads it: an ext4, an NTFS, an EFI system partition.
struct Piece {
    number: u32,
    start: u64,
    size: u64,
    part_type: &'static str,
    fs: &'static str,
    label: &'static str,
    uuid: &'static str,
    read: Option<Measure>,
}

/// A disk a scenario attaches: its serial, size, table (lsblk's `PTTYPE`)
/// and partitions — and, on an msdos table, the size its table gives the
/// extended partition, which the kernel shows as a 1 KiB stub and udev
/// records (`Facts::extended_sizes`), and the partition the table marks
/// active (lsblk's `PARTFLAGS`, `0x80`), as its builder marks it in
/// `tests/e2e/fixtures.py`: Windows on windows-msdos-base, none on
/// msdos-extended-base or bios-base.
struct Fixture {
    serial: &'static str,
    size: u64,
    table: Option<&'static str>,
    parts: &'static [Piece],
    extended: Option<(u32, u64)>,
    active: Option<u32>,
}

/// What the measure entry answers of an ext4 `size` bytes long with `used`
/// in use: read, and its tool's least a little over what it uses.
const fn ext4(size: u64, used: u64) -> Option<Measure> {
    Some(Measure::Read {
        used,
        free: size - used,
        least: Some(used + 128 * MIB),
    })
}

/// A Windows' 100 MiB EFI system partition, read: its boot manager's files.
const ESP_100: Option<Measure> = Some(Measure::Read {
    used: 3 * MIB,
    free: 97 * MIB,
    least: None,
});

/// A blank disk.
const fn blank(serial: &'static str, size: u64) -> Fixture {
    Fixture {
        serial,
        size,
        table: None,
        parts: &[],
        extended: None,
        active: None,
    }
}

const EMPTY_48: Fixture = blank("LXB-E2E-TARGET", 48 * GIB);
const BIOS_GPT_32: Fixture = blank("LXB-E2E-TARGET", 32 * GIB);
const DISK_A: Fixture = blank("LXB-E2E-DISK-A", 32 * GIB);
const DISK_B: Fixture = blank("LXB-E2E-DISK-B", 8 * GIB);

/// dualboot-base: Windows (an NTFS with no hiberfil.sys of its own), 30 GiB
/// free, its recovery partition and an old Linux.
const DUALBOOT: Fixture = Fixture {
    serial: "LXB-E2E-DUALBOOT",
    size: 64 * GIB,
    table: Some("gpt"),
    parts: &[
        Piece {
            number: 1,
            start: MIB,
            size: 100 * MIB,
            part_type: part_type::ESP,
            fs: "vfat",
            label: "SYSTEM",
            uuid: "1234-ABCD",
            read: ESP_100,
        },
        Piece {
            number: 2,
            start: 101 * MIB,
            size: 16 * MIB,
            part_type: part_type::MICROSOFT_RESERVED,
            fs: "",
            label: "",
            uuid: "",
            read: None,
        },
        Piece {
            number: 3,
            start: 117 * MIB,
            size: 6144 * MIB,
            part_type: part_type::BASIC_DATA,
            fs: "ntfs",
            label: "Windows",
            uuid: "34F5EE1202469FF7",
            read: Some(Measure::NoHiberfile {
                used: 40 * MIB,
                free: 6_442_450_432 - 40 * MIB,
            }),
        },
        Piece {
            number: 4,
            start: 36_981 * MIB,
            size: 750 * MIB,
            part_type: part_type::WINDOWS_RECOVERY,
            fs: "",
            label: "",
            uuid: "",
            read: None,
        },
        Piece {
            number: 5,
            start: 37_731 * MIB,
            size: 29_154_607_104,
            part_type: part_type::LINUX,
            fs: "ext4",
            label: "OLD_LINUX",
            uuid: "0ddc0ffe-e000-4000-8000-0000000000aa",
            read: ext4(29_154_607_104, 900 * MIB),
        },
    ],
    extended: None,
    active: None,
};

/// replace-base: dualboot-base with an ext4 "Old Linux" (6) filling its gap.
const REPLACE: Fixture = Fixture {
    serial: "LXB-E2E-DUALBOOT",
    size: 64 * GIB,
    table: Some("gpt"),
    parts: &[
        Piece {
            number: 1,
            start: MIB,
            size: 100 * MIB,
            part_type: part_type::ESP,
            fs: "vfat",
            label: "SYSTEM",
            uuid: "1234-ABCD",
            read: ESP_100,
        },
        Piece {
            number: 2,
            start: 101 * MIB,
            size: 16 * MIB,
            part_type: part_type::MICROSOFT_RESERVED,
            fs: "",
            label: "",
            uuid: "",
            read: None,
        },
        Piece {
            number: 3,
            start: 117 * MIB,
            size: 6144 * MIB,
            part_type: part_type::BASIC_DATA,
            fs: "ntfs",
            label: "Windows",
            uuid: "34F5EE1202469FF7",
            read: Some(Measure::NoHiberfile {
                used: 40 * MIB,
                free: 6_442_450_432 - 40 * MIB,
            }),
        },
        Piece {
            number: 4,
            start: 36_981 * MIB,
            size: 750 * MIB,
            part_type: part_type::WINDOWS_RECOVERY,
            fs: "",
            label: "",
            uuid: "",
            read: None,
        },
        Piece {
            number: 5,
            start: 37_731 * MIB,
            size: 29_154_607_104,
            part_type: part_type::LINUX,
            fs: "ext4",
            label: "OLD_LINUX",
            uuid: "0ddc0ffe-e000-4000-8000-0000000000aa",
            read: ext4(29_154_607_104, 900 * MIB),
        },
        Piece {
            number: 6,
            start: 6261 * MIB,
            size: 30_720 * MIB,
            part_type: part_type::LINUX,
            fs: "ext4",
            label: "Old Linux",
            uuid: "0ddc0ffe-e000-4000-8000-0000000000bb",
            read: ext4(30_720 * MIB, 600 * MIB),
        },
    ],
    extended: None,
    active: None,
};

/// home-base: one ext4 "Home" on a disk too small for Linexin.
const HOME: Fixture = Fixture {
    serial: "LXB-E2E-HOME",
    size: 16 * GIB,
    table: Some("gpt"),
    parts: &[Piece {
        number: 1,
        start: MIB,
        size: 17_177_772_032,
        part_type: part_type::LINUX,
        fs: "ext4",
        label: "Home",
        uuid: "0ddc0ffe-e000-4000-8000-0000000000cc",
        read: ext4(17_177_772_032, 300 * MIB),
    }],
    extended: None,
    active: None,
};

/// bios-base: an msdos disk with a FAT32 DATA and free space after it.
const BIOS_BASE: Fixture = Fixture {
    serial: "LXB-E2E-BIOSBASE",
    size: 40 * GIB,
    table: Some("dos"),
    parts: &[Piece {
        number: 1,
        start: MIB,
        size: 4 * GIB,
        part_type: part_type::MBR_FAT32_LBA,
        fs: "vfat",
        label: "DATA",
        uuid: "1ABB-105E",
        read: None,
    }],
    extended: None,
    active: None,
};

/// msdos-extended-base: an old root, then an extended partition (0f)
/// holding Data (5), a swap partition (6) and an old home (7); free space
/// after it.
const MSDOS_EXTENDED: Fixture = Fixture {
    serial: "LXB-E2E-MSDOSEXT",
    size: 64 * GIB,
    table: Some("dos"),
    parts: &[
        Piece {
            number: 1,
            start: MIB,
            size: 28_672 * MIB,
            part_type: part_type::MBR_LINUX,
            fs: "ext4",
            label: "Old root",
            uuid: "0ddc0ffe-e000-4000-8000-0000000e7d01",
            read: ext4(28_672 * MIB, 800 * MIB),
        },
        Piece {
            number: 2,
            start: 28_673 * MIB,
            size: 1024,
            part_type: part_type::MBR_EXTENDED_LBA,
            fs: "",
            label: "",
            uuid: "",
            read: None,
        },
        Piece {
            number: 5,
            start: 28_674 * MIB,
            size: 8192 * MIB,
            part_type: part_type::MBR_LINUX,
            fs: "ext4",
            label: "Data",
            uuid: "0ddc0ffe-e000-4000-8000-0000000e7d05",
            read: ext4(8192 * MIB, 200 * MIB),
        },
        Piece {
            number: 6,
            start: 36_867 * MIB,
            size: 2048 * MIB,
            part_type: part_type::MBR_SWAP,
            fs: "swap",
            label: "",
            uuid: "0ddc0ffe-e000-4000-8000-0000000e7d06",
            read: None,
        },
        Piece {
            number: 7,
            start: 38_916 * MIB,
            size: 10_240 * MIB,
            part_type: part_type::MBR_LINUX,
            fs: "ext4",
            label: "Old home",
            uuid: "0ddc0ffe-e000-4000-8000-0000000e7d07",
            read: ext4(10_240 * MIB, 300 * MIB),
        },
    ],
    extended: Some((2, (49_156 - 28_673) * MIB)),
    active: None,
};

/// windows-full-base: Windows filling the disk up to its recovery
/// partition, its own hiberfil.sys saying it is not hibernated.
const WINDOWS_FULL: Fixture = Fixture {
    serial: "LXB-E2E-WINFULL",
    size: 64 * GIB,
    table: Some("gpt"),
    parts: &[
        Piece {
            number: 1,
            start: MIB,
            size: 100 * MIB,
            part_type: part_type::ESP,
            fs: "vfat",
            label: "SYSTEM",
            uuid: "1ABE-F011",
            read: ESP_100,
        },
        Piece {
            number: 2,
            start: 101 * MIB,
            size: 16 * MIB,
            part_type: part_type::MICROSOFT_RESERVED,
            fs: "",
            label: "",
            uuid: "",
            read: None,
        },
        Piece {
            number: 3,
            start: 117 * MIB,
            size: 64_668 * MIB,
            part_type: part_type::BASIC_DATA,
            fs: "ntfs",
            label: "Windows",
            uuid: "1ABEF01100000003",
            read: Some(Measure::Read {
                used: 160 * MIB,
                free: 67_809_312_256 - 160 * MIB,
                least: Some(400 * MIB),
            }),
        },
        Piece {
            number: 4,
            start: 64_785 * MIB,
            size: 750 * MIB,
            part_type: part_type::WINDOWS_RECOVERY,
            fs: "",
            label: "",
            uuid: "",
            read: None,
        },
    ],
    extended: None,
    active: None,
};

/// windows-hibernated-base: dualboot-base's layout, every identifier its
/// own, its Windows hibernated.
const WINDOWS_HIBERNATED: Fixture = Fixture {
    serial: "LXB-E2E-WINHIBR",
    size: 64 * GIB,
    table: Some("gpt"),
    parts: &[
        Piece {
            number: 1,
            start: MIB,
            size: 100 * MIB,
            part_type: part_type::ESP,
            fs: "vfat",
            label: "SYSTEM",
            uuid: "1ABE-B1B0",
            read: ESP_100,
        },
        Piece {
            number: 2,
            start: 101 * MIB,
            size: 16 * MIB,
            part_type: part_type::MICROSOFT_RESERVED,
            fs: "",
            label: "",
            uuid: "",
            read: None,
        },
        Piece {
            number: 3,
            start: 117 * MIB,
            size: 6144 * MIB,
            part_type: part_type::BASIC_DATA,
            fs: "ntfs",
            label: "Windows",
            uuid: "1ABEB1B000000003",
            read: Some(Measure::Hibernated),
        },
        Piece {
            number: 4,
            start: 36_981 * MIB,
            size: 750 * MIB,
            part_type: part_type::WINDOWS_RECOVERY,
            fs: "",
            label: "",
            uuid: "",
            read: None,
        },
        Piece {
            number: 5,
            start: 37_731 * MIB,
            size: 29_154_607_104,
            part_type: part_type::LINUX,
            fs: "ext4",
            label: "OLD_LINUX",
            uuid: "0ddc0ffe-e000-4000-8000-000000b1b005",
            read: ext4(29_154_607_104, 900 * MIB),
        },
    ],
    extended: None,
    active: None,
};

/// ext4-data-base: one ext4 "Data", about 45 % full (its manifest's own
/// count of used bytes).
const EXT4_DATA: Fixture = Fixture {
    serial: "LXB-E2E-EXT4DATA",
    size: 24 * GIB,
    table: Some("gpt"),
    parts: &[Piece {
        number: 1,
        start: MIB,
        size: 25_767_706_624,
        part_type: part_type::LINUX,
        fs: "ext4",
        label: "Data",
        uuid: "0ddc0ffe-e000-4000-8000-000000da7a01",
        read: Some(Measure::Read {
            used: 11_595_468_800,
            free: 25_767_706_624 - 11_595_468_800,
            least: Some(2_900_000 * 4096),
        }),
    }],
    extended: None,
    active: None,
};

/// old-linux-base: an old Linux a UEFI started with systemd-boot — its EFI
/// system partition, root, swap and home.
const OLD_LINUX: Fixture = Fixture {
    serial: "LXB-E2E-OLDLINUX",
    size: 48 * GIB,
    table: Some("gpt"),
    parts: &[
        Piece {
            number: 1,
            start: MIB,
            size: GIB,
            part_type: part_type::ESP,
            fs: "vfat",
            label: "EFI",
            uuid: "1ABE-01D1",
            read: Some(Measure::Read {
                used: 24 * MIB,
                free: 1000 * MIB,
                least: None,
            }),
        },
        Piece {
            number: 2,
            start: 1025 * MIB,
            size: 32 * GIB,
            part_type: part_type::LINUX_ROOT_X86_64,
            fs: "ext4",
            label: "Old root",
            uuid: "0ddc0ffe-e000-4000-8000-00000001d102",
            read: ext4(32 * GIB, 900 * MIB),
        },
        Piece {
            number: 3,
            start: 33_793 * MIB,
            size: 4 * GIB,
            part_type: part_type::LINUX_SWAP,
            fs: "swap",
            label: "",
            uuid: "0ddc0ffe-e000-4000-8000-00000001d103",
            read: None,
        },
        Piece {
            number: 4,
            start: 37_889 * MIB,
            size: 11_262 * MIB,
            part_type: part_type::LINUX_HOME,
            fs: "ext4",
            label: "Old home",
            uuid: "0ddc0ffe-e000-4000-8000-00000001d104",
            read: ext4(11_262 * MIB, 300 * MIB),
        },
    ],
    extended: None,
    active: None,
};

/// windows-msdos-base: an older BIOS computer's MBR disk — Windows (its own
/// hiberfil.sys saying it is not hibernated), an old root and its home.
const WINDOWS_MSDOS: Fixture = Fixture {
    serial: "LXB-E2E-WINMSDOS",
    size: 64 * GIB,
    table: Some("dos"),
    parts: &[
        Piece {
            number: 1,
            start: MIB,
            size: 32 * GIB,
            part_type: part_type::MBR_NTFS,
            fs: "ntfs",
            label: "Windows",
            uuid: "1ABE1D0500000001",
            read: Some(Measure::Read {
                used: 72 * MIB,
                free: 34_359_737_856 - 72 * MIB,
                least: Some(300 * MIB),
            }),
        },
        Piece {
            number: 2,
            start: 32_769 * MIB,
            size: 28 * GIB,
            part_type: part_type::MBR_LINUX,
            fs: "ext4",
            label: "Old root",
            uuid: "0ddc0ffe-e000-4000-8000-0000001d0502",
            read: ext4(28 * GIB, 900 * MIB),
        },
        Piece {
            number: 3,
            start: 61_441 * MIB,
            size: 4095 * MIB,
            part_type: part_type::MBR_LINUX,
            fs: "ext4",
            label: "Old home",
            uuid: "0ddc0ffe-e000-4000-8000-0000001d0503",
            read: ext4(4095 * MIB, 300 * MIB),
        },
    ],
    extended: None,
    active: Some(1),
};

/// Where the live session's image is, as a live system's draft names it.
const LIVE_IMAGE: &str = "/run/archiso/airootfs";

/// The live session the guest is: the medium on its optical drive, which
/// discovery leaves out.
fn live_session() -> LiveSession {
    LiveSession {
        image: LIVE_IMAGE.into(),
        medium_uuid: None,
        medium_label: Some("LINEXIN".into()),
        medium_mounted: true,
    }
}

/// A text column lsblk leaves empty (`null`).
fn column(text: &str) -> Value {
    if text.is_empty() {
        Value::Null
    } else {
        Value::from(text)
    }
}

/// What the guest's lsblk prints for `fixtures`, attached in that order
/// (`/dev/vda`, `/dev/vdb`, …, each a virtio disk of its own) beside the
/// installation medium on its optical drive.
fn lsblk(fixtures: &[&Fixture]) -> String {
    let mut devices: Vec<Value> = fixtures
        .iter()
        .enumerate()
        .map(|(at, fixture)| {
            let name = format!("/dev/vd{}", char::from(b'a' + at as u8));
            let minor = 16 * at as u32;
            let children: Vec<Value> = fixture
                .parts
                .iter()
                .map(|piece| {
                    json!({
                        "name": format!("{name}{}", piece.number),
                        "type": "part",
                        "size": piece.size,
                        "model": null,
                        "serial": null,
                        "ro": false,
                        "rm": false,
                        "rota": false,
                        "tran": null,
                        "mountpoints": [null],
                        "maj:min": format!("253:{}", minor + piece.number),
                        "fstype": column(piece.fs),
                        "label": column(piece.label),
                        "uuid": column(piece.uuid),
                        "parttype": column(piece.part_type),
                        "partflags": if fixture.active == Some(piece.number) {
                            Value::from("0x80")
                        } else {
                            Value::Null
                        },
                        "partn": piece.number,
                        "start": piece.start / 512,
                        "pttype": fixture.table,
                        "log-sec": 512,
                        "disk-seq": null,
                    })
                })
                .collect();
            json!({
                "name": name,
                "type": "disk",
                "size": fixture.size,
                "model": null,
                "serial": fixture.serial,
                "ro": false,
                "rm": false,
                "rota": false,
                "tran": "virtio",
                "mountpoints": [null],
                "maj:min": format!("253:{minor}"),
                "fstype": null,
                "label": null,
                "uuid": null,
                "parttype": null,
                "partn": null,
                "start": null,
                "pttype": fixture.table,
                "log-sec": 512,
                "disk-seq": 10 + at,
                "children": children,
            })
        })
        .collect();
    devices.push(json!({
        "name": "/dev/sr0",
        "type": "rom",
        "size": 3_000_000_000_u64,
        "ro": true,
        "rm": true,
        "mountpoints": ["/run/archiso/bootmnt"],
        "maj:min": "11:0",
        "fstype": "iso9660",
        "label": "LINEXIN",
    }));
    json!({ "blockdevices": devices }).to_string()
}

/// `fixtures` as `--list-disks` lists them in the guest: its lsblk read by
/// discovery (`disks::parse_lsblk`), no swap but the live system's, nothing
/// held, the extended partitions' sizes as udev recorded them.
fn listed(fixtures: &[&Fixture]) -> Vec<Disk> {
    let extended_sizes = fixtures
        .iter()
        .enumerate()
        .filter_map(|(at, fixture)| {
            let (number, size) = fixture.extended?;
            Some((
                format!("/dev/vd{}{number}", char::from(b'a' + at as u8)),
                size,
            ))
        })
        .collect();
    let facts = Facts {
        swaps: Some("Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority\n".into()),
        live: Some(live_session()),
        medium_devices: vec!["11:0".into()],
        extended_sizes,
        ..Facts::default()
    };
    disks::parse_lsblk(&lsblk(fixtures), &facts).expect("the guest's lsblk")
}

/// What the measure entry answers of `disks` (`fixtures`, as listed): each
/// partition it reads, as its fixture says — but what `instead` says of the
/// partition of the disk with that serial and number (one in use) — and
/// what the image puts in the folders it fills.
fn answered(
    disks: &[Disk],
    fixtures: &[&Fixture],
    instead: &[(&str, u32, Measure)],
) -> MeasureAnswer {
    let mut parts = Vec::new();
    for (disk, fixture) in disks.iter().zip(fixtures) {
        assert_eq!(disk.serial, fixture.serial);
        for (part, piece) in disk.partitions.iter().zip(fixture.parts) {
            let changed = instead
                .iter()
                .find(|(serial, number, _)| *serial == fixture.serial && *number == piece.number)
                .map(|(_, _, measure)| *measure);
            if let Some(measure) = changed.or(piece.read) {
                parts.push(Measured {
                    path: part.path.clone(),
                    fs: part.fs_type.clone(),
                    uuid: part.uuid.clone(),
                    size: part.size,
                    measure,
                });
            }
        }
    }
    MeasureAnswer::Measured {
        measures: Measures { parts },
        image: image(),
        changed: Vec::new(),
    }
}

/// What the image puts in each folder it fills, a test's own figures: the
/// guest's are phase A's to say (`layout: the image puts N bytes at
/// /var`). 512 MiB at `/var` keeps every plan's `/var` above its floor —
/// `manual-uefi-empty`'s 6 GiB one stands up to 0.8 GiB
/// (`EMPTY_VAR_IMAGE_MOST`).
fn image() -> ImageSizes {
    let paths = [
        ("/var", 512 * MIB),
        ("/opt", 64 * MIB),
        ("/srv", 0),
        ("/root", MIB),
        ("/tmp", 0),
    ];
    ImageSizes {
        paths: paths
            .into_iter()
            .map(|(path, size)| (path.to_string(), size))
            .collect(),
    }
}

thread_local! {
    /// The disks this thread's replay lists, again too (`listed_again`).
    static LISTED: std::cell::RefCell<Vec<Disk>> = const { std::cell::RefCell::new(Vec::new()) };
    /// What its measure entry answers (`measured_as_handed`).
    static ANSWER: std::cell::RefCell<Option<MeasureAnswer>> =
        const { std::cell::RefCell::new(None) };
    /// The disks each measurement was asked about, by their paths.
    static ASKED: std::cell::RefCell<Vec<Vec<String>>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The disks listed again before the plan's question: as they were.
fn listed_again() -> Result<Vec<Disk>, String> {
    Ok(LISTED.with(|listed| listed.borrow().clone()))
}

/// A measurement of the replay's own: what the measure entry answers, there
/// before the walk's first press as `wait_for_measure` waits for it.
fn measured_as_handed(disks: Vec<Disk>) -> Receiver<MeasureEvent> {
    ASKED.with(|asked| {
        let paths = disks.iter().map(|disk| disk.path.clone()).collect();
        asked.borrow_mut().push(paths);
    });
    let (tx, rx) = std::sync::mpsc::channel();
    if let Some(answer) = ANSWER.with(|answer| answer.borrow().clone()) {
        tx.send(MeasureEvent::Answer(answer)).unwrap();
    }
    rx
}

// -------------------------------------------------------------------------
// The panel, and the toolkit's part.

/// The guest's own panel: the window fills QEMU's 1600×900 display.
const GUEST: [f32; 2] = [1600.0, 900.0];

/// A display standing on its side (1080×1920), where New partition stands
/// its rows one under another.
const STANDING: [f32; 2] = [1080.0, 1920.0];

/// Whether New partition stands its rows two by two on a window `width` ×
/// `height`, as `sheets::draw` decides it: the room the panel gives the
/// page (`Panel::lay`, the pane less its padding) at least 900 reference
/// pixels wide.
fn sheet_stands_wide([width, height]: [f32; 2]) -> bool {
    let scale = scale_for(height);
    let pane = panel::pane_rect(width, height, scale);
    pane[2] - Metric::PanelPadding.value() * scale * 2.0 >= 900.0 * scale
}

/// A menu the toolkit raised, and what the page said of the stop the light
/// was on as it rose (`partitions::said_of_the_lit_stop`).
struct Raised {
    menu: Menu,
    said: Vec<String>,
}

/// The installer on a live session, and the toolkit's part stood in for:
/// the menu and the dialog it holds up, the field the frame says the light
/// stands in, how a sheet stands its rows on this panel.
struct Hands {
    state: State,
    panel: [f32; 2],
    /// The menu up now, and the row its light is on.
    menu: Option<(Menu, usize)>,
    /// The dialog up now: its answers, and the one its light is on.
    dialog: Option<(Vec<String>, usize)>,
    /// Every menu raised, in turn.
    raised: Vec<Raised>,
    /// Each page of the plan's question as it was put up.
    pages: Vec<String>,
    /// Whether New partition stood its rows two by two, frame by frame.
    new_sheets: Vec<bool>,
}

impl Hands {
    /// Where to install on a live session of the Linexin medium a
    /// `firmware` starts, its disks `disks`, its measure entry answering
    /// `answer`, drawn on a window `panel` — as `walk_to_target` leaves it.
    fn on_where_to_install(
        disks: Vec<Disk>,
        firmware: Firmware,
        answer: MeasureAnswer,
        panel: [f32; 2],
    ) -> Hands {
        LISTED.with(|listed| *listed.borrow_mut() = disks.clone());
        ANSWER.with(|handed| *handed.borrow_mut() = Some(answer));
        ASKED.with(|asked| asked.borrow_mut().clear());
        let mut state = State::demo(true);
        state.machine = Machine::Real {
            live: Some(live_session()),
        };
        state.pretend_firmware(firmware);
        let fresh = draft(&disks, LIVE_IMAGE, firmware);
        state.draft.disk = fresh.disk;
        state.draft.target = fresh.target;
        state.draft.storage = fresh.storage;
        state.draft.source = fresh.source;
        state.disks = disks;
        state.list_disks = listed_again;
        state.measure = measured_as_handed;
        // Desktop asks the internet as it comes: a question of the test's
        // own, out already, so its own (which the fence refuses in a test
        // build) is never sent.
        state.online.ask(|| false);
        state.go(Screen::Target);
        Hands {
            state,
            panel,
            menu: None,
            dialog: None,
            raised: Vec::new(),
            pages: Vec::new(),
            new_sheets: Vec::new(),
        }
    }

    /// `walk`'s keys, each in a frame of its own.
    fn walk(&mut self, walk: &str) {
        for key in keys(walk) {
            self.press(key);
        }
    }

    /// One key, in a frame of its own: the measure entry's answer folded in
    /// as the frame begins (`partitions::poll`); the key to the menu or the
    /// dialog up, else to the screen — or, typed, to the field the light
    /// stands in, which must take it (a letter no field takes is no press,
    /// and Y is Options); then the page drawn as far as a press depends on
    /// it, and a menu or a question it asked for raised.
    fn press(&mut self, key: Key) {
        screens::partitions::poll(&mut self.state);
        match key {
            Key::Type(text) => {
                let field = self.state.typing_into();
                assert!(
                    field.is_some() && self.menu.is_none() && self.dialog.is_none(),
                    "{text:?} typed where no field takes it"
                );
                self.state.type_keys(vec![Edit::Wrote(text.into())]);
            }
            Key::Press(action) if self.menu.is_some() => self.in_the_menu(action),
            Key::Press(action) if self.dialog.is_some() => self.in_the_dialog(action),
            Key::Press(action) => {
                if action == Action::Accept {
                    self.state.pressing.press();
                }
                self.state.act(action, None);
            }
        }
        self.drawn();
        self.raise();
    }

    /// A key while a menu is up, as the toolkit takes it: Up and Down move
    /// its light, round either end (`Menu::step`); Enter chooses the row,
    /// which the next frame hands the screen that raised it; Escape puts it
    /// away, nothing chosen. No walk presses anything else there.
    fn in_the_menu(&mut self, action: Action) {
        let Some((menu, at)) = self.menu.as_mut() else {
            return;
        };
        let rows = menu.commands.len();
        match action {
            Action::Up => *at = (*at + rows - 1) % rows,
            Action::Down => *at = (*at + 1) % rows,
            Action::Accept => {
                let row = *at;
                self.menu = None;
                if let Some((screen, id)) = self.state.menu_open.take() {
                    if screen == self.state.screen {
                        screens::chose(&mut self.state, id, row);
                    }
                }
            }
            Action::Back => self.menu = None,
            other => panic!("{other:?} pressed with {:?} up", menu.title),
        }
    }

    /// A key while a dialog is up: Left and Right move its light along its
    /// answers, round either end; Enter answers the one lit.
    fn in_the_dialog(&mut self, action: Action) {
        let Some((answers, at)) = self.dialog.as_mut() else {
            return;
        };
        let count = answers.len();
        match action {
            Action::Left => *at = (*at + count - 1) % count,
            Action::Right => *at = (*at + 1) % count,
            Action::Accept => {
                let answer = *at;
                self.dialog = None;
                if let Some(question) = self.state.answering.take() {
                    self.state.answer(question, answer);
                }
            }
            other => panic!("{other:?} pressed with a question up"),
        }
    }

    /// What drawing the page tells the frame that a press depends on: a
    /// sheet stands its rows two by two on a panel wide enough
    /// (`sheets::draw`), and the field the light stands in — a size held,
    /// a label's or a path's row lit, where nothing stands over the page —
    /// takes what is typed (`State::in_field`).
    fn drawn(&mut self) {
        let wide = sheet_stands_wide(self.panel);
        let covered = self.menu.is_some()
            || self.dialog.is_some()
            || self.state.menu.is_some()
            || self.state.question.is_some();
        let state = &mut self.state;
        state.standing_in = None;
        state.quiet_in = None;
        state.field_lit = None;
        let on_partitions = state.screen == Screen::Target
            && state.views.target.part == screens::target::Part::Partitions;
        let mut lit = None;
        if let Some(sheet) = state
            .views
            .partitions
            .sheet
            .as_mut()
            .filter(|_| on_partitions)
        {
            sheet.wide = matches!(sheet.kind, Kind::New(_)) && wide;
            if matches!(sheet.kind, Kind::New(_)) {
                self.new_sheets.push(sheet.wide);
            }
            lit = if sheet.held.is_some() {
                Some(FieldId::PartSize)
            } else {
                match sheet.row() {
                    Row::Label if sheet.enabled(Row::Label) => Some(FieldId::PartLabel),
                    Row::Path => Some(FieldId::MountPath),
                    _ => None,
                }
            };
        }
        if let Some(field) = lit.filter(|_| !covered) {
            state.in_field(field);
        }
        state.hold_typing();
        state.let_go_of_a_field_left();
    }

    /// Raise what the presses asked for, as the frame does: a menu, the
    /// light on the row it is raised with — the row it comes back to
    /// (`menu_on`), else the one marked (`menu_marked`), else its first —
    /// or the question, as the frame puts it up on the guest's panel (a
    /// line of the dialog measured at half its face's size a letter: what
    /// a page holds is no part of a walk, which answers every page alike).
    fn raise(&mut self) {
        if let Some(menu) = self.state.raised_menu() {
            let rows = menu.commands.len();
            let at = match (menu.marked, menu.on) {
                (None, Some(row)) | (Some(row), _) if row < rows => row,
                _ => 0,
            };
            self.state.menu_open = Some((self.state.screen, menu.id));
            self.raised.push(Raised {
                menu: menu.clone(),
                said: screens::partitions::said_of_the_lit_stop(&self.state),
            });
            self.menu = Some((menu, at));
            return;
        }
        let Some(question) = self.state.question.take() else {
            return;
        };
        let question = self.state.put_as(question);
        let [width, height] = self.panel;
        let scale = scale_for(height);
        let most = menus::dialog_lines(height, scale);
        let room = menus::dialog_room(width, scale);
        let letter = Text::Caption.size() * scale * 0.5;
        let (_, body, answers) = self.state.dialog(question, most, room, |line| {
            line.chars().count() as f32 * letter
        });
        self.pages.push(body);
        self.state.answering = Some(question);
        self.dialog = Some((answers, 0));
    }

    /// Continue went on to Desktop with the plan in use.
    fn on_desktop_with_the_plan(&self) {
        let state = &self.state;
        assert_eq!(
            state.screen,
            Screen::Desktop,
            "Continue on Partitions did not go on: {:?}; the plan still needs {:?}",
            state.note.as_ref().map(|note| &note.text),
            screens::partitions::missing(state),
        );
        assert!(screens::partitions::in_use(state), "the plan is not in use");
    }

    /// The layout the request carries.
    fn layout(&self) -> Vec<DiskLayout> {
        let request = self.state.request().expect("a request is built");
        let Target::Layout { disks } = request.config.target else {
            panic!("the request does not carry the plan");
        };
        disks
    }

    /// The plan's question, as the scenario meets it: Review, an account
    /// typed in, the plan judged whole, then the question Install puts up,
    /// answered as `plan_question` answers it: on every page Right from Go
    /// back and Enter, Next to the page after, up to the last, whose "Erase
    /// and install" is never pressed here (it would begin, and the launch
    /// looks at this machine). Answers what its pages said.
    fn answer_the_question(&mut self) -> Vec<String> {
        let state = &mut self.state;
        screens::review::stage(state);
        state.go(Screen::Review);
        assert_eq!(state.plan_judged(), Ok(()));
        let consent = state.request().expect("a request").consent;
        state.ask_to_install();
        assert_eq!(
            state.question,
            Some(Question::PlanErase { page: 0 }),
            "Install did not put the plan's question up"
        );
        self.raise();
        assert_eq!(self.state.asked.as_ref(), Some(&consent));
        let go = if consent.is_empty() {
            i18n::text("button-install")
        } else {
            i18n::text("erase-confirm")
        };
        let mut last = Vec::new();
        while let Some((answers, _)) = &self.dialog {
            last = answers.clone();
            assert!(self.pages.len() <= 16, "the question went on past 16 pages");
            if last[1] == go {
                break;
            }
            self.walk("right ret");
        }
        assert_eq!(last[1], go, "the last page answered was not the last");
        let state = &self.state;
        assert!(state.events.is_none(), "a worker was asked for");
        assert!(state.rehearsal.is_none());
        assert_eq!(state.screen, Screen::Review);
        std::mem::take(&mut self.pages)
    }
}

// -------------------------------------------------------------------------
// What the scenarios read back.

/// `text` as the harness compares words (`scenarios.plain`): lower case,
/// everything but letters, digits and `|` a space, one at a time.
fn plain(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '|' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether `said` holds `words` as the harness finds them (`found_words`):
/// whole words, compared plain — a device's path reads "dev vda1".
fn reads(said: &[String], words: &str) -> bool {
    let text = format!(" {} ", plain(&said.join("\n")));
    text.contains(&format!(" {} ", plain(words)))
}

/// Only the scenario's own set pretended proven (`layout::CAMPAIGN`).
fn its_own_set(scenario: &str) -> PretendProven {
    let at = CAMPAIGN
        .iter()
        .position(|(name, _)| *name == scenario)
        .unwrap_or_else(|| panic!("{scenario} is none of the campaign's"));
    PretendProven::these(std::slice::from_ref(&CAMPAIGN[at].1))
}

/// The set `layout::CAMPAIGN` names for `scenario`.
fn set_of(scenario: &str) -> Vec<Capability> {
    let found = CAMPAIGN.iter().find(|(name, _)| *name == scenario);
    found.expect("one of the campaign's").1.to_vec()
}

/// The disk of `disks` with `serial`.
fn by_serial<'a>(disks: &'a [Disk], serial: &str) -> &'a Disk {
    let found = disks.iter().find(|disk| disk.serial == serial);
    found.unwrap_or_else(|| panic!("no disk {serial}"))
}

/// What becomes of `disk` as the scenario reads it back: a new `table` or
/// none, each of its partitions' fates by number (every one of them said),
/// and the partitions made.
fn laid(
    disk: &Disk,
    table: Option<Table>,
    fates: &[(u32, Fate)],
    create: Vec<Create>,
) -> DiskLayout {
    let parts = disk
        .partitions
        .iter()
        .map(|was| {
            let found = fates.iter().find(|(number, _)| *number == was.number);
            let (_, fate) = found.unwrap_or_else(|| panic!("no fate for {}", was.path));
            Part {
                path: was.path.clone(),
                number: was.number,
                start: was.start,
                size: was.size,
                fate: fate.clone(),
            }
        })
        .collect();
    DiskLayout {
        disk: disk.clone(),
        table,
        parts,
        create,
    }
}

/// A partition made.
fn made(start: u64, size: u64, fs: Fs, label: &str, mount: Option<MountPoint>) -> Create {
    Create {
        start,
        size,
        fs,
        label: label.into(),
        mount,
    }
}

/// A path of one's own.
fn path(at: &str) -> Option<MountPoint> {
    Some(MountPoint::Other(at.into()))
}

/// Kept as it is, at `mount`.
fn keep(mount: Option<MountPoint>) -> Fate {
    Fate::Keep { mount }
}

/// Formatted as `fs`, its entry `size` long, at `mount`.
fn format(fs: Fs, size: u64, mount: Option<MountPoint>) -> Fate {
    Fate::Format {
        fs,
        label: String::new(),
        size,
        mount,
    }
}

/// A plan's walk pressed from Where to install on `fixtures`, a `firmware`
/// starting them, their measure entry answering as the fixtures say, on a
/// window `panel`, with `scenario`'s own set alone pretended proven: the
/// replay on Desktop, the plan in use. New partition stood its rows as the
/// panel decides, whenever it was up.
fn replayed(
    scenario: &str,
    fixtures: &[&Fixture],
    firmware: Firmware,
    panel: [f32; 2],
    walk: &str,
) -> (Hands, PretendProven) {
    let proven = its_own_set(scenario);
    let disks = listed(fixtures);
    let answer = answered(&disks, fixtures, &[]);
    let mut hands = Hands::on_where_to_install(disks.clone(), firmware, answer, panel);
    hands.walk(walk);
    hands.on_desktop_with_the_plan();
    let asked: Vec<String> = disks.iter().map(|disk| disk.path.clone()).collect();
    assert_eq!(ASKED.with(|each| each.borrow().clone()), [asked]);
    let wide = sheet_stands_wide(panel);
    assert!(
        hands.new_sheets.iter().all(|stood| *stood == wide),
        "New partition stood its rows otherwise than the panel decides"
    );
    (hands, proven)
}

/// Whether a menu titled `title` was raised offering Resize…: what
/// `resize_offered` reads before Resize… is pressed.
fn resize_was_offered(hands: &Hands, title: &str) -> bool {
    let resize = i18n::text("partitions-resize");
    hands.raised.iter().any(|raised| {
        raised.menu.title.as_deref() == Some(title)
            && raised.menu.commands.iter().any(|row| row == resize)
    })
}

// -------------------------------------------------------------------------
// The replays.

/// The disks are listed as the harness's walks take them: the stops of each
/// disk's map where its walk counts them (`scenarios.map_stops`), the disks
/// Partitions shows (`showable`) — so the keys pressed here are the keys
/// the scenarios press on the guest's listing — and New partition stands
/// two by two on the guest's panel, one under another on a display
/// standing on its side.
#[test]
fn the_harness_walks_on_the_disks_as_the_guest_lists_them() {
    use crate::ui::map;
    let stops = |fixture: &Fixture| {
        let disks = listed(&[fixture]);
        map::stops(&map::tiles(&disks[0])).len()
    };
    for (fixture, count) in [
        (&EMPTY_48, 1),
        (&DUALBOOT, 6),
        (&REPLACE, 6),
        (&HOME, 1),
        (&BIOS_BASE, 2),
        (&MSDOS_EXTENDED, 6),
        (&WINDOWS_FULL, 4),
        (&WINDOWS_HIBERNATED, 6),
        (&EXT4_DATA, 1),
        (&OLD_LINUX, 4),
        (&WINDOWS_MSDOS, 3),
    ] {
        assert_eq!(stops(fixture), count, "{}", fixture.serial);
    }
    let three = listed(&[&DISK_A, &DISK_B, &HOME]);
    let blocked: Vec<Option<Blocked>> = three.iter().map(|disk| disk.blocked.clone()).collect();
    assert_eq!(
        blocked,
        [None, Some(Blocked::TooSmall), Some(Blocked::TooSmall)]
    );
    assert!(
        three.iter().all(disks::holds_homes),
        "Partitions shows all three"
    );
    let paths: Vec<&str> = three.iter().map(|disk| disk.path.as_str()).collect();
    assert_eq!(paths, ["/dev/vda", "/dev/vdb", "/dev/vdc"]);
    let extended = listed(&[&MSDOS_EXTENDED]);
    assert_eq!(extended[0].free.len(), 1, "the space after the frame");
    assert_eq!(extended[0].free[0].start, 49_156 * MIB);
    assert!(sheet_stands_wide(GUEST));
    assert!(!sheet_stands_wide(STANDING));
}

/// `measure-writes-nothing`'s look at Partitions (`measure_walk`), in both
/// its boots: the measure entry asked about every disk listed; each tile
/// looked at saying what the scenario reads under it, its menu offering
/// Resize… where the scenario expects it and nowhere else; Back to Where to
/// install, the light on Advanced, nothing planned.
#[test]
fn the_harness_walks_partitions_to_read_the_measure() {
    let look = |hands: &Hands, title: &str, says: &[&str], resize: bool| {
        let raised = hands
            .raised
            .iter()
            .find(|raised| raised.menu.title.as_deref() == Some(title))
            .unwrap_or_else(|| panic!("{title}'s menu was never raised"));
        for words in says {
            assert!(
                reads(&raised.said, words),
                "{title}: {words:?} not in {:?}",
                raised.said
            );
        }
        let offered = raised
            .menu
            .commands
            .iter()
            .any(|row| row == i18n::text("partitions-resize"));
        assert_eq!(offered, resize, "{title}: {:?}", raised.menu.commands);
    };
    let back = |hands: &Hands| {
        let state = &hands.state;
        assert_eq!(state.screen, Screen::Target);
        assert_eq!(state.views.target.part, screens::target::Part::Main);
        assert_eq!(state.views.target.focus, screens::target::Focus::Advanced);
        assert!(state.views.partitions.plan.is_empty());
    };
    let held = "shut it down with fast startup off";
    let in_windows = "use disk management in windows";

    // Four disks, dualboot-base's old Linux mounted once they were listed.
    let fixtures = [&DUALBOOT, &WINDOWS_HIBERNATED, &EXT4_DATA, &WINDOWS_FULL];
    let disks = listed(&fixtures);
    let answer = answered(
        &disks,
        &fixtures,
        &[("LXB-E2E-DUALBOOT", 5, Measure::InUse)],
    );
    let mut hands = Hands::on_where_to_install(disks.clone(), Firmware::Uefi, answer, GUEST);
    hands.walk(MEASURE_WALK_FOUR);
    let asked: Vec<String> = disks.iter().map(|disk| disk.path.clone()).collect();
    assert_eq!(ASKED.with(|each| each.borrow().clone()), [asked]);
    let titles: Vec<_> = hands
        .raised
        .iter()
        .map(|raised| raised.menu.title.clone())
        .collect();
    let titled = |path: &str| Some(path.to_string());
    assert_eq!(
        titles,
        [
            titled("/dev/vdd3"),
            titled("/dev/vdb3"),
            titled("/dev/vda3"),
            titled("/dev/vda5"),
            titled("/dev/vdc1")
        ]
    );
    look(&hands, "/dev/vdd3", &[held, "/dev/vdb3"], false);
    look(&hands, "/dev/vdb3", &[held, "/dev/vdb3"], false);
    look(&hands, "/dev/vda3", &[in_windows], false);
    look(&hands, "/dev/vda5", &["in use"], false);
    look(&hands, "/dev/vdc1", &[], true);
    back(&hands);

    // Three, without windows-hibernated-base: Windows offered Resize….
    let fixtures = [&DUALBOOT, &EXT4_DATA, &WINDOWS_FULL];
    let disks = listed(&fixtures);
    let answer = answered(&disks, &fixtures, &[]);
    let mut hands = Hands::on_where_to_install(disks, Firmware::Uefi, answer, GUEST);
    hands.walk(MEASURE_WALK_THREE);
    let titles: Vec<_> = hands
        .raised
        .iter()
        .map(|raised| raised.menu.title.clone())
        .collect();
    assert_eq!(
        titles,
        [
            titled("/dev/vdc3"),
            titled("/dev/vda3"),
            titled("/dev/vdb1")
        ]
    );
    look(&hands, "/dev/vdc3", &[], true);
    look(&hands, "/dev/vda3", &[in_windows], false);
    look(&hands, "/dev/vdb1", &[], true);
    back(&hands);
}

/// **Shift+Tab goes to the disk before** on Partitions, as Tab goes on to
/// the next, from the map and from the chips alike, and neither past the
/// end: `measure_walk`'s four disks, the keys pressed as the toolkit reads
/// them ([`keys`]). Until lxb-toolkit's fix lxb-app handed Shift+Tab on as
/// Tab, and in the guest it went on to the next disk
/// (`measure-writes-nothing`, 1017-011703).
#[test]
fn shift_tab_on_partitions_goes_to_the_disk_before() {
    use screens::partitions::Focus;
    let fixtures = [&DUALBOOT, &WINDOWS_HIBERNATED, &EXT4_DATA, &WINDOWS_FULL];
    let disks = listed(&fixtures);
    let answer = answered(&disks, &fixtures, &[]);
    let mut hands = Hands::on_where_to_install(disks, Firmware::Uefi, answer, GUEST);
    hands.walk("down down down down left left ret");
    assert_eq!(hands.state.screen, Screen::Target);
    assert_eq!(
        hands.state.views.target.part,
        screens::target::Part::Partitions
    );
    let shown = |hands: &Hands| {
        let view = &hands.state.views.partitions;
        (hands.state.disks[view.disk].path.clone(), view.focus)
    };
    let on = |path: &str, focus: Focus| (path.to_string(), focus);

    // The first disk, by the chips, and down onto its map.
    hands.walk("up left left left left down");
    assert_eq!(shown(&hands), on("/dev/vda", Focus::Map));
    for (walk, path) in [
        ("tab", "/dev/vdb"),
        ("tab", "/dev/vdc"),
        ("shift+tab", "/dev/vdb"),
        ("shift+tab", "/dev/vda"),
        ("shift+tab", "/dev/vda"),
        ("tab tab tab", "/dev/vdd"),
        ("tab", "/dev/vdd"),
        ("shift+tab", "/dev/vdc"),
    ] {
        hands.walk(walk);
        assert_eq!(shown(&hands), on(path, Focus::Map), "after {walk}");
    }
    hands.walk("up shift+tab");
    assert_eq!(
        shown(&hands),
        on("/dev/vdb", Focus::Chips),
        "from the chips"
    );
    hands.walk("tab");
    assert_eq!(
        shown(&hands),
        on("/dev/vdc", Focus::Chips),
        "from the chips"
    );
    assert!(hands.state.views.partitions.plan.is_empty());
}

/// **Shift+Tab on a held size takes ten steps off**, as LB does, and Tab
/// puts ten on, as RB does: New partition's size on `plan_on_an_empty_disk`'s
/// blank disk, a new gpt on it, the size taken hold of and the keys pressed
/// as the toolkit reads them ([`keys`]). Until lxb-toolkit's fix lxb-app
/// handed Shift+Tab on as Tab, which put ten steps on.
#[test]
fn shift_tab_on_a_held_size_takes_ten_steps_off_as_lb_does() {
    let disks = listed(&[&EMPTY_48]);
    let answer = answered(&disks, &[&EMPTY_48], &[]);
    let mut hands = Hands::on_where_to_install(disks, Firmware::Uefi, answer, GUEST);
    // Partitions, a new gpt from the disk's menu, New… on all of it.
    hands.walk("down left left ret ret ret ret ret");
    let sheet = |hands: &Hands| {
        hands
            .state
            .views
            .partitions
            .sheet
            .as_ref()
            .expect("New partition's sheet")
            .clone()
    };
    assert!(matches!(sheet(&hands).kind, Kind::New(_)));
    assert_eq!(sheet(&hands).row(), Row::Size);
    let size = |hands: &Hands| sheet(hands).size().expect("a size");
    let whole = size(&hands);
    hands.walk("ret");
    assert!(
        sheet(&hands).held.is_some(),
        "the size was not taken hold of"
    );

    hands.walk("shift+tab");
    let off = size(&hands);
    assert!(off < whole, "Shift+Tab took nothing off: {off} of {whole}");
    assert!(sheet(&hands).held.is_some(), "Shift+Tab let go of the size");
    // LB, from the same place: B puts the size back, A takes hold again.
    hands.walk("esc ret");
    assert_eq!(size(&hands), whole);
    hands.press(Key::Press(Action::Previous));
    assert_eq!(size(&hands), off, "Shift+Tab is not LB");

    // Tab, ten steps on as RB, the last snapping to all of it.
    hands.walk("shift+tab");
    let further = size(&hands);
    assert!(further < off, "a second Shift+Tab took nothing off");
    hands.walk("tab");
    assert_eq!(size(&hands), off, "Tab is not ten steps on");
    hands.press(Key::Press(Action::Next));
    assert_eq!(size(&hands), whole, "RB is not ten steps on");
    assert!(sheet(&hands).held.is_some());
}

/// **Shift+Tab on the installer's own board rubs a letter out**, as LB
/// does, and Tab turns the board to its next face, as RB does: the board a
/// pad raises over a field anywhere but under LineXinBar, a keyboard's keys
/// pressed on it as the toolkit reads them ([`keys`]). Until lxb-toolkit's
/// fix lxb-app handed Shift+Tab on as Tab, which turned the board's face.
#[test]
fn shift_tab_on_the_installers_board_rubs_out_as_lb_does() {
    use crate::ui::board::Face;
    let mut state = State::demo(true);
    state.go(Screen::Account);
    state.edit(FieldId::Username, &Edit::Wrote("linexin".into()));
    state.type_with_pad(FieldId::Username);
    let face = |state: &State| state.board.as_ref().map(Board::face);
    assert_eq!(face(&state), Some(Face::Letters), "the board did not open");
    let press = |state: &mut State, walk: &str| {
        for key in keys(walk) {
            let Key::Press(action) = key else {
                panic!("{walk:?} types");
            };
            state.act(action, None);
        }
    };

    press(&mut state, "shift+tab");
    assert_eq!(state.draft.account.username, "linexi");
    assert_eq!(
        face(&state),
        Some(Face::Letters),
        "Shift+Tab turned the face"
    );
    state.act(Action::Previous, None);
    assert_eq!(state.draft.account.username, "linex", "LB is not Shift+Tab");

    press(&mut state, "tab");
    assert_eq!(
        face(&state),
        Some(Face::Symbols),
        "Tab did not turn the face"
    );
    state.act(Action::Next, None);
    assert_eq!(face(&state), Some(Face::Accents), "RB is not Tab");
    assert_eq!(state.draft.account.username, "linex", "Tab rubbed out");
}

/// `manual-uefi-empty` (`plan_on_an_empty_disk`): a new gpt on its blank
/// 48 GiB disk; the 1 GiB FAT32 EFI system partition alone at /boot; / of
/// 28 GiB labelled E2E-ROOT; 2 GiB swap; 1 GiB swap with no mount point
/// (the gpt_auto canary); 6 GiB ext4 /var; xfs /home on the rest. Its
/// question names the new gpt table over the disk.
#[test]
fn the_harness_walks_partitions_on_an_empty_disk() {
    let scenario = "manual-uefi-empty";
    let (mut hands, _proven) = replayed(
        scenario,
        &[&EMPTY_48],
        Firmware::Uefi,
        GUEST,
        PLAN_ON_AN_EMPTY_DISK,
    );
    let laid_out = hands.layout();
    let disk = &hands.state.disks[0];
    let end = 48 * GIB - MIB;
    let at = |mib: u64| MIB + mib * MIB;
    let want = laid(
        disk,
        Some(Table::Gpt),
        &[],
        vec![
            made(at(0), GIB, Fs::Fat32, "", Some(MountPoint::Boot)),
            made(
                at(1024),
                28 * GIB,
                Fs::Ext4,
                "E2E-ROOT",
                Some(MountPoint::Root),
            ),
            made(
                at(29_696),
                2 * GIB,
                Fs::LinuxSwap,
                "",
                Some(MountPoint::Swap),
            ),
            made(at(31_744), GIB, Fs::LinuxSwap, "", None),
            made(at(32_768), 6 * GIB, Fs::Ext4, "", path("/var")),
            made(
                at(38_912),
                end - at(38_912),
                Fs::Xfs,
                "",
                Some(MountPoint::Home),
            ),
        ],
    );
    assert_eq!(laid_out, [want]);
    assert_eq!(layout::needs(&laid_out, Firmware::Uefi), set_of(scenario));
    let pages = hands.answer_the_question();
    assert!(reads(&pages, "create gpt partition table"), "{pages:?}");
    assert!(reads(&pages, "/dev/vda"), "{pages:?}");
}

/// `manual-uefi-new-table-over-used` (`plan_new_table_over_used`): a new
/// gpt from Options over everything on dualboot-base; a 1 GiB EFI system
/// partition at /efi, a 2 GiB FAT32 /boot beside it (XBOOTLDR), btrfs / on
/// the rest. Its question names the new table and every partition it
/// erases.
#[test]
fn the_harness_walks_partitions_to_a_new_table_over_a_used_disk() {
    let scenario = "manual-uefi-new-table-over-used";
    let (mut hands, _proven) = replayed(
        scenario,
        &[&DUALBOOT],
        Firmware::Uefi,
        GUEST,
        PLAN_NEW_TABLE_OVER_USED,
    );
    let laid_out = hands.layout();
    let disk = hands.state.disks[0].clone();
    let end = 64 * GIB - MIB;
    let want = laid(
        &disk,
        Some(Table::Gpt),
        &[
            (1, Fate::Delete),
            (2, Fate::Delete),
            (3, Fate::Delete),
            (4, Fate::Delete),
            (5, Fate::Delete),
        ],
        vec![
            made(MIB, GIB, Fs::Fat32, "", Some(MountPoint::Efi)),
            made(MIB + GIB, 2 * GIB, Fs::Fat32, "", Some(MountPoint::Boot)),
            made(
                MIB + 3 * GIB,
                end - (MIB + 3 * GIB),
                Fs::Btrfs,
                "",
                Some(MountPoint::Root),
            ),
        ],
    );
    assert_eq!(laid_out, [want]);
    assert_eq!(layout::needs(&laid_out, Firmware::Uefi), set_of(scenario));
    let pages = hands.answer_the_question();
    assert!(reads(&pages, "create gpt partition table"), "{pages:?}");
    for part in &disk.partitions {
        assert!(reads(&pages, &part.path), "{} not in {pages:?}", part.path);
    }
}

/// `manual-uefi-beside-windows` and `refuse-manual-changed`
/// (`plan_beside_windows`), on replace-base: the EFI system partition (1)
/// kept at /efi; Windows (3) kept at /mnt/windows; Old Linux (6) deleted,
/// / of 28 GiB and a 2 GiB FAT32 /boot in its span; the old Linux (5)
/// formatted ext4 at /home, its entry made 16 GiB, and a 4 GiB swap in the
/// tail it gave up. Its question names what it deletes (6) and what it
/// formats (5).
#[test]
fn the_harness_walks_partitions_beside_windows() {
    let scenario = "manual-uefi-beside-windows";
    let (mut hands, _proven) = replayed(
        scenario,
        &[&REPLACE],
        Firmware::Uefi,
        GUEST,
        PLAN_BESIDE_WINDOWS,
    );
    let laid_out = hands.layout();
    let disk = &hands.state.disks[0];
    let want = laid(
        disk,
        None,
        &[
            (1, keep(Some(MountPoint::Efi))),
            (2, keep(None)),
            (3, keep(path("/mnt/windows"))),
            (4, keep(None)),
            (5, format(Fs::Ext4, 16 * GIB, Some(MountPoint::Home))),
            (6, Fate::Delete),
        ],
        vec![
            made(6261 * MIB, 28 * GIB, Fs::Ext4, "", Some(MountPoint::Root)),
            made(
                6261 * MIB + 28 * GIB,
                2 * GIB,
                Fs::Fat32,
                "",
                Some(MountPoint::Boot),
            ),
            made(
                37_731 * MIB + 16 * GIB,
                4 * GIB,
                Fs::LinuxSwap,
                "",
                Some(MountPoint::Swap),
            ),
        ],
    );
    assert_eq!(laid_out, [want]);
    assert_eq!(layout::needs(&laid_out, Firmware::Uefi), set_of(scenario));
    let pages = hands.answer_the_question();
    assert!(
        reads(&pages, "/dev/vda6") && reads(&pages, "/dev/vda5"),
        "{pages:?}"
    );
}

/// `manual-uefi-three-disks` (`plan_three_disks`): on A (blank) gpt, / of
/// 28 GiB and swap on the rest; on B (blank, too small for Linexin) gpt, a
/// 1 GiB EFI system partition at /efi and a 2 GiB FAT32 /boot; on C
/// (home-base) its Home kept at /home. Its question names the new tables
/// over A and B, and nothing of C.
#[test]
fn the_harness_walks_partitions_over_three_disks() {
    let scenario = "manual-uefi-three-disks";
    let fixtures = [&DISK_A, &DISK_B, &HOME];
    let (mut hands, _proven) =
        replayed(scenario, &fixtures, Firmware::Uefi, GUEST, PLAN_THREE_DISKS);
    let laid_out = hands.layout();
    let disks = hands.state.disks.clone();
    let a = by_serial(&disks, "LXB-E2E-DISK-A");
    let b = by_serial(&disks, "LXB-E2E-DISK-B");
    let c = by_serial(&disks, "LXB-E2E-HOME");
    let want = [
        laid(
            a,
            Some(Table::Gpt),
            &[],
            vec![
                made(MIB, 28 * GIB, Fs::Ext4, "", Some(MountPoint::Root)),
                made(
                    MIB + 28 * GIB,
                    4 * GIB - 2 * MIB,
                    Fs::LinuxSwap,
                    "",
                    Some(MountPoint::Swap),
                ),
            ],
        ),
        laid(
            b,
            Some(Table::Gpt),
            &[],
            vec![
                made(MIB, GIB, Fs::Fat32, "", Some(MountPoint::Efi)),
                made(MIB + GIB, 2 * GIB, Fs::Fat32, "", Some(MountPoint::Boot)),
            ],
        ),
        laid(c, None, &[(1, keep(Some(MountPoint::Home)))], Vec::new()),
    ];
    assert_eq!(laid_out, want);
    assert_eq!(layout::needs(&laid_out, Firmware::Uefi), set_of(scenario));
    let pages = hands.answer_the_question();
    assert!(
        reads(&pages, "/dev/vda") && reads(&pages, "/dev/vdb"),
        "{pages:?}"
    );
    assert!(
        !reads(&pages, "/dev/vdc") && !reads(&pages, "/dev/vdc1"),
        "{pages:?}"
    );
}

/// `manual-bios-gpt` (`plan_bios_gpt`), on the guest's panel and on a
/// display standing on its side: a new gpt on the blank 32 GiB disk a BIOS
/// starts; GRUB's 1 MiB bios_grub partition from New partition's File
/// system row — reached by Down four times and Up twice, the sheet two by
/// two on the one and one under another on the other; / of 28 GiB; ext4
/// /home on the rest. Its question names the new gpt table over the disk.
#[test]
fn the_harness_walks_partitions_to_a_bios_grub_partition() {
    let scenario = "manual-bios-gpt";
    for panel in [GUEST, STANDING] {
        let (mut hands, _proven) = replayed(
            scenario,
            &[&BIOS_GPT_32],
            Firmware::Bios,
            panel,
            PLAN_BIOS_GPT,
        );
        assert!(!hands.new_sheets.is_empty());
        let laid_out = hands.layout();
        let disk = &hands.state.disks[0];
        let end = 32 * GIB - MIB;
        let want = laid(
            disk,
            Some(Table::Gpt),
            &[],
            vec![
                made(MIB, MIB, Fs::BiosGrub, "", None),
                made(2 * MIB, 28 * GIB, Fs::Ext4, "", Some(MountPoint::Root)),
                made(
                    2 * MIB + 28 * GIB,
                    end - (2 * MIB + 28 * GIB),
                    Fs::Ext4,
                    "",
                    Some(MountPoint::Home),
                ),
            ],
        );
        assert_eq!(laid_out, [want], "{panel:?}");
        assert_eq!(layout::needs(&laid_out, Firmware::Bios), set_of(scenario));
        let pages = hands.answer_the_question();
        assert!(reads(&pages, "create gpt partition table"), "{pages:?}");
        assert!(reads(&pages, "/dev/vda"), "{pages:?}");
    }
}

/// `manual-bios-msdos-keep-fat` (`plan_bios_keep_fat`), on bios-base under
/// a BIOS: DATA (1) kept at /mnt/data; two new primaries after it, / of
/// 32 GiB and swap on the rest. Its question says nothing on the disks is
/// erased, and names nothing of DATA.
#[test]
fn the_harness_walks_partitions_beside_a_kept_fat() {
    let scenario = "manual-bios-msdos-keep-fat";
    let (mut hands, _proven) = replayed(
        scenario,
        &[&BIOS_BASE],
        Firmware::Bios,
        GUEST,
        PLAN_BIOS_KEEP_FAT,
    );
    let laid_out = hands.layout();
    let disk = &hands.state.disks[0];
    let root = 4097 * MIB;
    let want = laid(
        disk,
        None,
        &[(1, keep(path("/mnt/data")))],
        vec![
            made(root, 32 * GIB, Fs::Ext4, "", Some(MountPoint::Root)),
            made(
                root + 32 * GIB,
                40 * GIB - (root + 32 * GIB),
                Fs::LinuxSwap,
                "",
                Some(MountPoint::Swap),
            ),
        ],
    );
    assert_eq!(laid_out, [want]);
    assert_eq!(layout::needs(&laid_out, Firmware::Bios), set_of(scenario));
    let pages = hands.answer_the_question();
    assert!(reads(&pages, i18n::text("erase-plan-nothing")), "{pages:?}");
    assert!(!reads(&pages, "/dev/vda1"), "{pages:?}");
}

/// `manual-bios-msdos-logical` (`plan_bios_logical`), on msdos-extended-base
/// under a BIOS: the old root (1) formatted ext4 at /; the last logical
/// partition (7) deleted; the swap partition (6) formatted as swap, its
/// entry grown over 7's place to the extended partition's end (13 GiB
/// typed, held to 12289 MiB); the data partition (5) kept at /data. Its
/// question names 1, 6 and 7 by their reviewed numbers, and nothing of 5.
#[test]
fn the_harness_walks_partitions_among_logical_partitions() {
    let scenario = "manual-bios-msdos-logical";
    let (mut hands, _proven) = replayed(
        scenario,
        &[&MSDOS_EXTENDED],
        Firmware::Bios,
        GUEST,
        PLAN_BIOS_LOGICAL,
    );
    let laid_out = hands.layout();
    let disk = &hands.state.disks[0];
    let want = laid(
        disk,
        None,
        &[
            (1, format(Fs::Ext4, 28_672 * MIB, Some(MountPoint::Root))),
            (2, keep(None)),
            (5, keep(path("/data"))),
            (
                6,
                format(Fs::LinuxSwap, 12_289 * MIB, Some(MountPoint::Swap)),
            ),
            (7, Fate::Delete),
        ],
        Vec::new(),
    );
    assert_eq!(laid_out, [want]);
    assert_eq!(layout::needs(&laid_out, Firmware::Bios), set_of(scenario));
    let pages = hands.answer_the_question();
    for named in ["/dev/vda1", "/dev/vda6", "/dev/vda7"] {
        assert!(reads(&pages, named), "{named} not in {pages:?}");
    }
    assert!(!reads(&pages, "/dev/vda5"), "{pages:?}");
}

/// `manual-uefi-shrink-ntfs` and `refuse-ntfs-hibernated`
/// (`plan_shrink_windows`), on windows-full-base: the EFI system partition
/// (1) kept at /efi; Windows (3) shrunk by 30 GiB, to 33948 MiB — its
/// Resize… seen on its menu before it is pressed — and kept at /windows;
/// / of 28 GiB and a 2 GiB FAT32 /boot in the space it gave up; Options'
/// swap file 2 GiB. Its question says Windows is shrunk.
#[test]
fn the_harness_walks_partitions_to_shrink_windows() {
    let scenario = "manual-uefi-shrink-ntfs";
    let (mut hands, _proven) = replayed(
        scenario,
        &[&WINDOWS_FULL],
        Firmware::Uefi,
        GUEST,
        PLAN_SHRINK_WINDOWS,
    );
    assert!(resize_was_offered(&hands, "/dev/vda3"));
    let laid_out = hands.layout();
    let disk = &hands.state.disks[0];
    let gave_up = (117 + 33_948) * MIB;
    let want = laid(
        disk,
        None,
        &[
            (1, keep(Some(MountPoint::Efi))),
            (2, keep(None)),
            (
                3,
                Fate::Shrink {
                    to: 33_948 * MIB,
                    mount: path("/windows"),
                },
            ),
            (4, keep(None)),
        ],
        vec![
            made(gave_up, 28 * GIB, Fs::Ext4, "", Some(MountPoint::Root)),
            made(
                gave_up + 28 * GIB,
                2 * GIB,
                Fs::Fat32,
                "",
                Some(MountPoint::Boot),
            ),
        ],
    );
    assert_eq!(laid_out, [want]);
    assert_eq!(layout::needs(&laid_out, Firmware::Uefi), set_of(scenario));
    let request = hands.state.request().expect("a request");
    assert_eq!(request.config.storage.swap_mib, 2048, "Options' swap file");
    let pages = hands.answer_the_question();
    assert!(reads(&pages, "shrink /dev/vda3 from"), "{pages:?}");
}

/// `manual-uefi-shrink-ext4` and `refuse-manual-busy-shrink`
/// (`plan_shrink_ext4`): on A (blank) gpt, the 1 GiB EFI system partition
/// alone at /boot, btrfs / on the rest; on B (ext4-data-base) Data shrunk
/// to 16 GiB — its Resize… seen first — and kept at /data, an ext4 /var
/// made in the space it gave up. Its question names the new gpt table over
/// A and the shrink of Data.
#[test]
fn the_harness_walks_partitions_to_shrink_an_ext4() {
    let scenario = "manual-uefi-shrink-ext4";
    let fixtures = [&DISK_A, &EXT4_DATA];
    let (mut hands, _proven) =
        replayed(scenario, &fixtures, Firmware::Uefi, GUEST, PLAN_SHRINK_EXT4);
    assert!(resize_was_offered(&hands, "/dev/vdb1"));
    let laid_out = hands.layout();
    let disks = hands.state.disks.clone();
    let a = by_serial(&disks, "LXB-E2E-DISK-A");
    let b = by_serial(&disks, "LXB-E2E-EXT4DATA");
    let want = [
        laid(
            a,
            Some(Table::Gpt),
            &[],
            vec![
                made(MIB, GIB, Fs::Fat32, "", Some(MountPoint::Boot)),
                made(
                    MIB + GIB,
                    31 * GIB - 2 * MIB,
                    Fs::Btrfs,
                    "",
                    Some(MountPoint::Root),
                ),
            ],
        ),
        laid(
            b,
            None,
            &[(
                1,
                Fate::Shrink {
                    to: 16 * GIB,
                    mount: path("/data"),
                },
            )],
            vec![made(MIB + 16 * GIB, 8190 * MIB, Fs::Ext4, "", path("/var"))],
        ),
    ];
    assert_eq!(laid_out, want);
    assert_eq!(layout::needs(&laid_out, Firmware::Uefi), set_of(scenario));
    let pages = hands.answer_the_question();
    assert!(
        reads(&pages, "create gpt partition table") && reads(&pages, "/dev/vda"),
        "{pages:?}"
    );
    assert!(reads(&pages, "shrink /dev/vdb1 from"), "{pages:?}");
}

/// `manual-uefi-reinstall-keep-home` (`plan_reinstall_keep_home`), on
/// old-linux-base: the EFI system partition (1) kept at /boot alone; the
/// old root (2) formatted ext4 at /; the old swap (3) formatted as swap;
/// the old home (4) kept at /home — nothing made, resized or deleted. Its
/// question names the two formats and nothing of what is kept.
#[test]
fn the_harness_walks_partitions_to_reinstall_keeping_home() {
    let scenario = "manual-uefi-reinstall-keep-home";
    let (mut hands, _proven) = replayed(
        scenario,
        &[&OLD_LINUX],
        Firmware::Uefi,
        GUEST,
        PLAN_REINSTALL_KEEP_HOME,
    );
    let laid_out = hands.layout();
    let disk = &hands.state.disks[0];
    let want = laid(
        disk,
        None,
        &[
            (1, keep(Some(MountPoint::Boot))),
            (2, format(Fs::Ext4, 32 * GIB, Some(MountPoint::Root))),
            (3, format(Fs::LinuxSwap, 4 * GIB, Some(MountPoint::Swap))),
            (4, keep(Some(MountPoint::Home))),
        ],
        Vec::new(),
    );
    assert_eq!(laid_out, [want]);
    assert_eq!(layout::needs(&laid_out, Firmware::Uefi), set_of(scenario));
    let pages = hands.answer_the_question();
    assert!(
        reads(&pages, "/dev/vda2") && reads(&pages, "/dev/vda3"),
        "{pages:?}"
    );
    assert!(
        !reads(&pages, "/dev/vda1") && !reads(&pages, "/dev/vda4"),
        "{pages:?}"
    );
}

/// `manual-bios-msdos-windows-keep-home` (`plan_bios_windows_keep_home`),
/// under a BIOS, on windows-msdos-base, the computer's one disk: walked from
/// its last partition back to its first, the old home (3) kept at /home,
/// the old root (2) formatted ext4 at /, Windows (1) shrunk by 4 GiB — its
/// Resize… seen first — and a swap partition, the fourth primary, made in
/// all it gave up; no table written. Its question says Windows is shrunk
/// and the root formatted, and nothing of the home nor of a new table.
#[test]
fn the_harness_walks_partitions_beside_windows_on_msdos() {
    let scenario = "manual-bios-msdos-windows-keep-home";
    let (mut hands, _proven) = replayed(
        scenario,
        &[&WINDOWS_MSDOS],
        Firmware::Bios,
        GUEST,
        PLAN_BIOS_WINDOWS_KEEP_HOME,
    );
    assert!(resize_was_offered(&hands, "/dev/vda1"));
    let laid_out = hands.layout();
    let disk = &hands.state.disks[0];
    let want = laid(
        disk,
        None,
        &[
            (
                1,
                Fate::Shrink {
                    to: 28 * GIB,
                    mount: None,
                },
            ),
            (2, format(Fs::Ext4, 28 * GIB, Some(MountPoint::Root))),
            (3, keep(Some(MountPoint::Home))),
        ],
        vec![made(
            MIB + 28 * GIB,
            4 * GIB,
            Fs::LinuxSwap,
            "",
            Some(MountPoint::Swap),
        )],
    );
    assert_eq!(laid_out, [want]);
    assert_eq!(layout::needs(&laid_out, Firmware::Bios), set_of(scenario));
    let pages = hands.answer_the_question();
    for words in ["shrink /dev/vda1 from", "/dev/vda2"] {
        assert!(reads(&pages, words), "{words:?} not in {pages:?}");
    }
    assert!(
        !reads(&pages, "/dev/vda3") && !reads(&pages, "partition table"),
        "{pages:?}"
    );
}
