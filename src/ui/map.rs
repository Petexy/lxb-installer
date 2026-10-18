//! The disk map of Partitions (Where to install's Advanced page): a disk laid
//! out as GParted lays one out — one row of tiles, each a partition or a run
//! of unallocated space, in disk order — and the technical words the page
//! says about them.
//!
//! **The one place the installer talks like GParted** (by design): device
//! names (`sda3`), filesystem types (`ntfs`, `ext4`, `linux-swap`), flags
//! (`msftdata`, `bios_grub`) and binary units (`931.5 GiB`). None of those
//! are translated, here or in GParted; the few words around them that are
//! plain language ("unallocated", "unknown", "no partition table") come from
//! the catalogs. Where to install keeps its plain words.
//!
//! Everything here is arithmetic over the `Disk`s the interface already
//! holds — it reads them and builds nothing a worker could ever be handed —
//! so it is tested without a renderer: what is a tile, how wide each one is,
//! how many of its words it says at that width, and how a size is written.

use lxb_app::lxb_toolkit::color::Srgb;

use crate::i18n;
use crate::model::disks::{part_type, Disk, Partition, Table, GIB, MIB};

/// The no-break space a size's number and its unit are held together with,
/// so a caption that wraps never leaves "GiB" alone on its line.
const HELD: char = '\u{a0}';

/// One tile of the map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tile {
    /// A partition, by its place in the disk's `partitions`.
    Part(usize),
    /// A run of unallocated space: where it starts and how long it is, in
    /// bytes.
    Free { start: u64, size: u64 },
    /// An msdos extended partition, by its place in the disk's
    /// `partitions`, drawn as a frame round the logical partitions inside it
    /// (`inside`, in disk order), as GParted draws one.
    Extended { at: usize, inside: Vec<Tile> },
    /// What was written straight over a disk with no partition table — a
    /// filesystem, encryption, a RAID member — drawn as one tile across it.
    Whole,
}

/// A stop of the light along the map: every tile, an extended partition's
/// frame first and then the logical partitions inside it — the order Left
/// and Right walk, which is the disk's own order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    Part(usize),
    Free { start: u64, size: u64 },
    Extended(usize),
    Whole,
}

impl Tile {
    /// How much of the disk the tile stands for, in bytes: what its width
    /// is shared out by.
    fn bytes(&self, disk: &Disk) -> u64 {
        match self {
            Tile::Part(at) => disk.partitions.get(*at).map_or(0, |part| part.size),
            Tile::Free { size, .. } => *size,
            Tile::Extended { at, inside } => extended_span(disk, *at, inside).1,
            Tile::Whole => disk.size,
        }
    }

    /// Where on the disk the tile starts, in bytes: what the tiles are put
    /// in order by.
    fn start(&self, disk: &Disk) -> u64 {
        match self {
            Tile::Part(at) | Tile::Extended { at, .. } => {
                disk.partitions.get(*at).map_or(0, |part| part.start)
            }
            Tile::Free { start, .. } => *start,
            Tile::Whole => 0,
        }
    }

    /// The stop of the light this tile is (an extended partition's: its
    /// frame).
    pub fn stop(&self) -> Stop {
        match self {
            Tile::Part(at) => Stop::Part(*at),
            Tile::Free { start, size } => Stop::Free {
                start: *start,
                size: *size,
            },
            Tile::Extended { at, .. } => Stop::Extended(*at),
            Tile::Whole => Stop::Whole,
        }
    }
}

/// Whether `part` is a logical partition of an msdos table: numbered from
/// five, inside the extended partition (the rule `disks::free_gaps` keeps).
fn logical(table: Option<Table>, part: &Partition) -> bool {
    table == Some(Table::Msdos) && part.number > 4
}

/// Where an extended partition starts and how far it reaches: from its own
/// start to its own end or the end of the last logical partition inside it,
/// whichever is further. The kernel shows an extended partition as a stub of
/// 1 KiB, and lsblk reports that; the logical partitions say how far it
/// really goes.
fn extended_span(disk: &Disk, at: usize, inside: &[Tile]) -> (u64, u64) {
    let Some(part) = disk.partitions.get(at) else {
        return (0, 0);
    };
    let end = inside
        .iter()
        .map(|tile| tile.start(disk) + tile.bytes(disk))
        .fold(part.start + part.size, u64::max);
    (part.start, end - part.start)
}

/// How far the extended partition at `at` really reaches, in bytes, with
/// the logical partitions `inside` it: from its start to its own end or the
/// last of theirs, whichever is further.
pub fn extent(disk: &Disk, at: usize, inside: &[Tile]) -> u64 {
    extended_span(disk, at, inside).1
}

/// The tiles of `disk`, in disk order: its partitions and the gaps of free
/// space discovery listed (`Disk::free`, the runs worth listing at all), an
/// msdos extended partition holding its logical ones — and any run of free
/// space that lies inside it, which discovery never lists but a deleted
/// logical partition leaves on a disk as Partitions' plan will leave it
/// (`ui::partitions::apply`). A disk with no partition table is one tile:
/// unallocated from end to end, or what was written over the whole of it.
pub fn tiles(disk: &Disk) -> Vec<Tile> {
    if disk.table.is_none() && disk.partitions.is_empty() {
        return vec![if disk.content.is_some() {
            Tile::Whole
        } else {
            Tile::Free {
                start: 0,
                size: disk.size,
            }
        }];
    }
    let mut top: Vec<Tile> = Vec::new();
    for (at, part) in disk.partitions.iter().enumerate() {
        if logical(disk.table, part) {
            continue;
        }
        if part.is_extended() {
            let mut inside: Vec<Tile> = disk
                .partitions
                .iter()
                .enumerate()
                .filter(|(_, one)| logical(disk.table, one))
                .map(|(index, _)| Tile::Part(index))
                .collect();
            inside.sort_by_key(|tile| tile.start(disk));
            top.push(Tile::Extended { at, inside });
        } else {
            top.push(Tile::Part(at));
        }
    }
    for gap in &disk.free {
        let free = Tile::Free {
            start: gap.start,
            size: gap.size,
        };
        // Inside an extended partition's own reach (its size, which a disk
        // as the plan leaves it gives whole): one of its runs.
        let frame = top.iter_mut().find_map(|tile| match tile {
            Tile::Extended { at, inside } => {
                let part = &disk.partitions[*at];
                (gap.start >= part.start && gap.start < part.start + part.size).then_some(inside)
            }
            _ => None,
        });
        match frame {
            Some(inside) => {
                inside.push(free);
                inside.sort_by_key(|tile| tile.start(disk));
            }
            None => top.push(free),
        }
    }
    top.sort_by_key(|tile| tile.start(disk));
    top
}

/// Every stop of the light along `tiles`, in the order Left and Right walk
/// them: an extended partition's frame, then the partitions inside it.
pub fn stops(tiles: &[Tile]) -> Vec<Stop> {
    let mut all = Vec::new();
    for tile in tiles {
        all.push(tile.stop());
        if let Tile::Extended { inside, .. } = tile {
            all.extend(inside.iter().map(Tile::stop));
        }
    }
    all
}

/// How wide each of a row of things is in `room`, `gap` apart, where each is
/// `(bytes, least)`: as wide as its share of the bytes, but never narrower
/// than its least width — the rest shared out by size among the others.
///
/// A 1 MiB `bios_grub` and a 16 MiB `msr` beside a 372 GiB Windows would be a
/// hair wide to scale; held to a least width, they stay tiles a pointer lands
/// on and the light can be seen on. Shared out again after each one is held
/// up (a share can fall under the least once room is given to the small ones),
/// so the widths and the gaps always come to exactly `room`. Where even the
/// least widths do not fit, everything is narrowed alike.
pub fn widths(room: f32, gap: f32, items: &[(u64, f32)]) -> Vec<f32> {
    if items.is_empty() {
        return Vec::new();
    }
    let open = (room - gap * (items.len() - 1) as f32).max(0.0);
    let least: f32 = items.iter().map(|(_, least)| least).sum();
    if least >= open {
        let least = least.max(f32::EPSILON);
        return items.iter().map(|(_, one)| open * one / least).collect();
    }
    let mut held = vec![false; items.len()];
    loop {
        let taken: f32 = items
            .iter()
            .zip(&held)
            .filter(|(_, held)| **held)
            .map(|((_, least), _)| least)
            .sum();
        let left = open - taken;
        let bytes: u64 = items
            .iter()
            .zip(&held)
            .filter(|(_, held)| !**held)
            .map(|((bytes, _), _)| bytes)
            .sum();
        let free = held.iter().filter(|held| !**held).count().max(1);
        let share = |bytes_of: u64| {
            if bytes == 0 {
                left / free as f32
            } else {
                (left as f64 * bytes_of as f64 / bytes as f64) as f32
            }
        };
        let mut more = false;
        for (index, (bytes_of, least)) in items.iter().enumerate() {
            if !held[index] && share(*bytes_of) < *least {
                held[index] = true;
                more = true;
            }
        }
        if !more {
            return items
                .iter()
                .zip(&held)
                .map(|((bytes_of, least), held)| if *held { *least } else { share(*bytes_of) })
                .collect();
        }
    }
}

/// How a map is laid out: the least width of a tile, the gap between two,
/// and how far an extended partition's frame stands out round the tiles
/// inside it — at the side, and at the top where its band is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measures {
    pub least: f32,
    pub gap: f32,
    pub inset: f32,
    pub head: f32,
}

/// One stop laid out on the map: what it is, where it stands, and whether
/// it stands inside an extended partition's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Laid {
    pub stop: Stop,
    pub rect: [f32; 4],
    pub inside: bool,
}

/// The least width of `tile`, each tile's own being `least`: a tile's own,
/// or for an extended partition the least widths of the tiles inside it,
/// their gaps, and its frame.
fn least_of(tile: &Tile, measures: Measures, least: &impl Fn(&Tile) -> f32) -> f32 {
    match tile {
        Tile::Extended { inside, .. } if !inside.is_empty() => {
            inside.iter().map(least).sum::<f32>()
                + (inside.len() - 1) as f32 * measures.gap
                + measures.inset * 2.0
        }
        _ => least(tile),
    }
}

/// Lay `tiles` of `disk` out across `rect`, in the order the light walks
/// them ([`stops`]): each tile as tall as the map, an extended partition's
/// frame round the tiles inside it, which stand inside it under its band.
pub fn lay(disk: &Disk, tiles: &[Tile], rect: [f32; 4], measures: Measures) -> Vec<Laid> {
    lay_by(disk, tiles, rect, measures, |_| measures.least)
}

/// How wide a map of `tiles` is laid out where it slides (a display
/// standing on its side, `room` wide): each tile as wide as `least` says
/// it is comfortable — wide enough to say its words — their gaps, and
/// [`SHARE`] of the room besides, which the tiles share out by size, so the
/// larger partitions are still the wider; never narrower than the room.
pub fn spread(tiles: &[Tile], room: f32, measures: Measures, least: impl Fn(&Tile) -> f32) -> f32 {
    let leasts: f32 = tiles
        .iter()
        .map(|tile| least_of(tile, measures, &least))
        .sum();
    let gaps = measures.gap * tiles.len().saturating_sub(1) as f32;
    (leasts + gaps + room * SHARE).max(room)
}

/// How much of the room a map that slides lays out beyond its tiles'
/// comfortable widths, shared out among them by size.
pub const SHARE: f32 = 0.3;

/// Whether the tiles of a map `room` wide keep their comfortable widths
/// (`least`) side by side in it.
pub fn fits(tiles: &[Tile], room: f32, measures: Measures, least: impl Fn(&Tile) -> f32) -> bool {
    let leasts: f32 = tiles
        .iter()
        .map(|tile| least_of(tile, measures, &least))
        .sum();
    leasts + measures.gap * tiles.len().saturating_sub(1) as f32 <= room
}

/// Where a map `width` wide that slides in a view `room` wide stands slid,
/// `was` being where it was going last: as little moved from there as keeps
/// the tile the light is on — from `lit[0]` to `lit[1]` along the map — in
/// view, with `peek` of what is beside it showing at either side where
/// there is more (the Home menu's rule: the next card always peeks in at the
/// edge). A tile wider than the view less its peeks stands from its left
/// edge. Never slid past either end.
pub fn slide_to(was: f32, lit: [f32; 2], room: f32, width: f32, peek: f32) -> f32 {
    let most = (width - room).max(0.0);
    let mut at = was.clamp(0.0, most);
    let left = if lit[0] <= 0.5 { 0.0 } else { lit[0] - peek };
    let right = if lit[1] >= width - 0.5 {
        width
    } else {
        lit[1] + peek
    };
    if right > at + room {
        at = right - room;
    }
    if left < at {
        at = left;
    }
    at.clamp(0.0, most)
}

/// [`lay`], each tile held to a least width of its own (`least`, asked of
/// every tile, those inside an extended partition's frame among them)
/// rather than the one `measures` says.
pub fn lay_by(
    disk: &Disk,
    tiles: &[Tile],
    rect: [f32; 4],
    measures: Measures,
    least: impl Fn(&Tile) -> f32,
) -> Vec<Laid> {
    let items: Vec<(u64, f32)> = tiles
        .iter()
        .map(|tile| (tile.bytes(disk), least_of(tile, measures, &least)))
        .collect();
    let wide = widths(rect[2], measures.gap, &items);
    let mut laid = Vec::new();
    let mut x = rect[0];
    for (tile, width) in tiles.iter().zip(wide) {
        let at = [x, rect[1], width, rect[3]];
        laid.push(Laid {
            stop: tile.stop(),
            rect: at,
            inside: false,
        });
        if let Tile::Extended { inside, .. } = tile {
            let room = [
                at[0] + measures.inset,
                at[1] + measures.head,
                (at[2] - measures.inset * 2.0).max(1.0),
                (at[3] - measures.head - measures.inset).max(1.0),
            ];
            let items: Vec<(u64, f32)> = inside
                .iter()
                .map(|one| (one.bytes(disk), least(one)))
                .collect();
            let mut x = room[0];
            for (one, width) in inside.iter().zip(widths(room[2], measures.gap, &items)) {
                laid.push(Laid {
                    stop: one.stop(),
                    rect: [x, room[1], width, room[3]],
                    inside: true,
                });
                x += width + measures.gap;
            }
        }
        x += width + measures.gap;
    }
    laid
}

/// Where a stop of the map stands along it: the bytes of the disk it spans,
/// and where it was drawn across, from its left edge to its right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub start: u64,
    pub end: u64,
    pub x: [f32; 2],
}

/// The bytes `stop` of `disk` spans: a partition's, a run's, an extended
/// partition's whole reach, the whole disk.
pub fn bytes_of(disk: &Disk, stop: Stop) -> (u64, u64) {
    match stop {
        Stop::Part(at) => disk
            .partitions
            .get(at)
            .map_or((0, 0), |part| (part.start, part.start + part.size)),
        Stop::Free { start, size } => (start, start + size),
        Stop::Extended(at) => {
            let reach = tiles(disk).iter().find_map(|tile| match tile {
                Tile::Extended { at: here, inside } if *here == at => {
                    Some(extent(disk, at, inside))
                }
                _ => None,
            });
            let start = disk.partitions.get(at).map_or(0, |part| part.start);
            (start, start + reach.unwrap_or(0))
        }
        Stop::Whole => (0, disk.size),
    }
}

/// The spans of a laid map of `disk`, along: what a map that eases from one
/// layout to the next remembers of where it stood.
pub fn spans(disk: &Disk, laid: &[Laid]) -> Vec<Span> {
    laid.iter()
        .map(|one| {
            let (start, end) = bytes_of(disk, one.stop);
            Span {
                start,
                end,
                x: [one.rect[0], one.rect[0] + one.rect[2]],
            }
        })
        .collect()
}

/// `to`'s spans `through` of the way from where they stood in a map laid
/// as `from` to where they stand now: what a map eases by when the plan
/// changes. A tile is the one it was where it starts where one started — a
/// partition shrunk or grown keeps its start, so its right-hand edge goes
/// from its old size to its new one — or else where it ends where one ended
/// — the unallocated space after it, whose left-hand edge goes along with
/// that edge; a run a deleted partition joined keeps the start of the space
/// beside it and widens over where the partition was, the tiles after it
/// closing up. A tile that was nowhere (none started or ended where it
/// does) stands where it stands from the first.
pub fn eased(from: &[Span], to: &[Span], through: f32) -> Vec<[f32; 2]> {
    let t = through.clamp(0.0, 1.0);
    to.iter()
        .map(|span| {
            let was = from
                .iter()
                .find(|one| one.start == span.start)
                .or_else(|| from.iter().find(|one| one.end == span.end));
            match was {
                Some(was) => [
                    was.x[0] + (span.x[0] - was.x[0]) * t,
                    was.x[1] + (span.x[1] - was.x[1]) * t,
                ],
                None => span.x,
            }
        })
        .collect()
}

/// How many of its words a tile says at its width: all of them (its name,
/// its filesystem, its size and its label), then its name and size, then its
/// name, then — where even its name does not fit but its size does, as for
/// a run of unallocated space on a narrow map, whose name is long and whose
/// dashed band already says what it is — its size alone, then none. The
/// details under the map always say everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Said {
    Nothing,
    Size,
    Name,
    NameAndSize,
    All,
}

/// How wide a tile's words are and how tall their lines: its name, in a
/// row's larger type; its size and filesystem in the smaller one, and its
/// size in whole units (`size_short`) for a tile that says its size alone;
/// whether it has a third small line (a label, which is the person's own
/// word, so it is cut to fit rather than leaving the rest unsaid, or what a
/// run of unallocated space was); the two line heights; and the mount point
/// the plan gives it, if any, which stands under the name, larger, as wide
/// as `mount` and `mount_line` tall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Words {
    pub name: f32,
    pub size: f32,
    pub short: f32,
    pub filesystem: f32,
    pub labelled: bool,
    pub name_line: f32,
    pub line: f32,
    pub mount: Option<f32>,
    pub mount_line: f32,
}

/// How many of its words a tile whose inside is `room` (width, height) says,
/// and whether its mount point stands under its name: wherever the name is
/// said and the mount point fits beside the lines said with it — given up
/// before the name is, never before the filesystem, size and label (which
/// the details say too, where the mount point is the plan itself).
pub fn said(room: [f32; 2], words: Words) -> (Said, bool) {
    let [wide, tall] = room;
    let fits = |width: f32| width <= wide + 0.5;
    let fits_tall = |height: f32| height <= tall + 0.5;
    let lines = if words.labelled { 3.0 } else { 2.0 };
    for mount in [true, false] {
        let mounted = mount && words.mount.is_some_and(fits);
        if mount && !mounted {
            continue;
        }
        let head = words.name_line + if mounted { words.mount_line } else { 0.0 };
        let all = fits(words.name) && fits(words.size) && fits(words.filesystem);
        if all && fits_tall(head + words.line * lines) {
            return (Said::All, mounted);
        }
        if fits(words.name) && fits(words.size) && fits_tall(head + words.line) {
            return (Said::NameAndSize, mounted);
        }
        if fits(words.name) && fits_tall(head) {
            return (Said::Name, mounted);
        }
    }
    if fits(words.short) && fits_tall(words.line) {
        (Said::Size, false)
    } else {
        (Said::Nothing, false)
    }
}

/// A size as GParted writes one, in binary units: whole KiB under a MiB,
/// whole MiB under a GiB, then GiB to one decimal ("931.5 GiB") and TiB from
/// a TiB on — with the decimal mark of the language spoken, and the number
/// and its unit held together on one line. Units are never translated.
pub fn size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const TIB: u64 = 1024 * GIB;
    let (number, unit) = if bytes < MIB {
        (format!("{}", (bytes as f64 / KIB as f64).round()), "KiB")
    } else if bytes < GIB {
        (format!("{}", (bytes as f64 / MIB as f64).round()), "MiB")
    } else if bytes < TIB {
        (format!("{:.1}", bytes as f64 / GIB as f64), "GiB")
    } else {
        (format!("{:.1}", bytes as f64 / TIB as f64), "TiB")
    };
    format!("{}{HELD}{unit}", i18n::decimal(number))
}

/// [`size`] in whole units — "279 GiB" for 279.4 GiB — for a tile too narrow
/// for the decimal, which says its size alone (`Said::Size`); the details
/// give it whole. Under a GiB it is [`size`] itself.
pub fn size_short(bytes: u64) -> String {
    const TIB: u64 = 1024 * GIB;
    let (number, unit) = if bytes < GIB {
        return size(bytes);
    } else if bytes < TIB {
        (bytes as f64 / GIB as f64, "GiB")
    } else {
        (bytes as f64 / TIB as f64, "TiB")
    };
    format!("{}{HELD}{unit}", number.round())
}

/// A place or a length on the disk in whole MiB, as the details give an
/// unallocated run's start, end and size.
pub fn mib(bytes: u64) -> String {
    format!("{}{HELD}MiB", bytes / MIB)
}

/// A disk's or a partition's device name without `/dev/`: `sda`, `sda3`,
/// `nvme0n1p2`.
pub fn device(path: &str) -> &str {
    path.strip_prefix("/dev/").unwrap_or(path)
}

/// The filesystem a partition holds, by the name GParted gives it: `fat32`
/// for what lsblk calls `vfat`, `linux-swap` for `swap`, and every other as
/// lsblk names it. `None` where it holds none that could be read — said as
/// "unknown" — and `extended` for an extended partition.
pub fn filesystem(part: &Partition) -> Option<&str> {
    if part.is_extended() {
        return Some("extended");
    }
    match part.fs_type.as_str() {
        "" => None,
        "vfat" => Some("fat32"),
        "swap" => Some("linux-swap"),
        other => Some(other),
    }
}

/// [`filesystem`], with "unknown" from the catalogs where there is none.
pub fn filesystem_words(part: &Partition) -> &str {
    filesystem(part).unwrap_or_else(|| i18n::text("partitions-unknown"))
}

/// The flags GParted shows for a partition, read from its type: what parted
/// reports for each type the installer knows. None for most.
pub fn flags(part: &Partition) -> Vec<&'static str> {
    match part.part_type.as_str() {
        part_type::ESP => vec!["boot", "esp"],
        part_type::MBR_ESP => vec!["esp"],
        part_type::BIOS_BOOT => vec!["bios_grub"],
        part_type::MICROSOFT_RESERVED => vec!["msftres"],
        part_type::BASIC_DATA => vec!["msftdata"],
        part_type::WINDOWS_RECOVERY => vec!["hidden", "diag"],
        part_type::MBR_WINDOWS_RECOVERY => vec!["diag"],
        part_type::LINUX_SWAP => vec!["swap"],
        part_type::LINUX_HOME => vec!["linux-home"],
        part_type::XBOOTLDR | part_type::MBR_XBOOTLDR => vec!["bls_boot"],
        part_type::MBR_FAT32_LBA | part_type::MBR_FAT16_LBA | part_type::MBR_EXTENDED_LBA => {
            vec!["lba"]
        }
        _ => Vec::new(),
    }
}

/// The colour of the band along a tile's top: GParted's colour for its
/// filesystem, re-tinted for the installer's violet glass — the darker ones
/// lifted (ext4's slate, exFAT's sea green, swap's brick, which sank into
/// the violet), the brighter ones softened a little so they do not shout,
/// every one keeping the hue a GParted user knows it by.
pub fn colour(filesystem: Option<&str>) -> Srgb {
    match filesystem {
        // GParted #4B6983, #7590AE, #9DB8D2.
        Some("ext4") => Srgb(0x7fa8e2),
        Some("ext3") => Srgb(0x98b8e0),
        Some("ext2") => Srgb(0xb4cce6),
        // GParted #FF9955.
        Some("btrfs") => Srgb(0xff9a5c),
        // GParted #EED680.
        Some("xfs") => Srgb(0xeed680),
        // GParted #42E5AC.
        Some("ntfs") => Srgb(0x4fe0b0),
        // GParted #18D918 and #00FF00.
        Some("fat32") => Srgb(0x3dd651),
        Some("fat16") => Srgb(0x5ef05e),
        // GParted #2E8B57.
        Some("exfat") => Srgb(0x45ad75),
        // GParted #C1665A.
        Some("linux-swap") => Srgb(0xdb7a6c),
        // GParted #7DFCFE.
        Some("extended") => Srgb(0x8eeef2),
        // GParted #CC9966 (LVM) and #625B81 (LUKS).
        Some("LVM2_member") => Srgb(0xd4a373),
        Some("crypto_LUKS") | Some("BitLocker") => Srgb(0x8f86b8),
        // GParted's grey for the rest it knows (iso9660, f2fs…).
        Some(_) => Srgb(0xb7b7c2),
        // GParted's black, lifted to a dark grey so it stands off the glass.
        None => Srgb(0x3c3647),
    }
}

/// How a disk is said in the head of Partitions: its device, its model, its
/// size and its partition table — `/dev/sda · Example SATA HDD · 931.5 GiB ·
/// gpt`. A disk with no model leaves it out; one with no table says so.
pub fn disk_line(disk: &Disk) -> String {
    let table = match disk.table {
        Some(Table::Gpt) => "gpt",
        Some(Table::Msdos) => "msdos",
        None => i18n::text("partitions-no-table"),
    };
    let model = disk.model.trim();
    let size = size(disk.size);
    let parts: Vec<&str> = [disk.path.as_str(), model, &size, table]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();
    parts.join(" · ")
}

/// How a disk's chip says it: its device name and its size,
/// `sda · 931.5 GiB`.
pub fn chip_words(disk: &Disk) -> String {
    format!("{} · {}", device(&disk.path), size(disk.size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::Content;
    use crate::ui::demo;

    fn sata() -> Disk {
        demo::disks().remove(1)
    }

    #[test]
    fn a_disk_is_its_partitions_and_free_space_in_disk_order() {
        let disk = sata();
        let laid = tiles(&disk);
        let names: Vec<String> = laid
            .iter()
            .map(|tile| match tile {
                Tile::Part(at) => device(&disk.partitions[*at].path).to_string(),
                Tile::Free { .. } => "free".into(),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(names, ["sda1", "sda2", "sda3", "free", "sda4", "sda5"]);
        // The light walks every one of them.
        assert_eq!(stops(&laid).len(), 6);
    }

    #[test]
    fn a_disk_with_no_table_is_one_tile_of_unallocated_space() {
        let nvme = demo::disks().remove(0);
        assert_eq!(
            tiles(&nvme),
            vec![Tile::Free {
                start: 0,
                size: nvme.size
            }]
        );
        let mut written = nvme;
        written.content = Some(Content::Linux);
        written.free.clear();
        assert_eq!(tiles(&written), vec![Tile::Whole]);
    }

    #[test]
    fn an_extended_partition_holds_its_logical_ones_and_the_light_walks_both() {
        let disk = demo::partition_disks().remove(0);
        assert_eq!(disk.table, Some(Table::Msdos));
        let laid = tiles(&disk);
        let extended = laid
            .iter()
            .find_map(|tile| match tile {
                Tile::Extended { at, inside } => Some((*at, inside.clone())),
                _ => None,
            })
            .expect("an extended partition");
        assert!(extended.1.len() >= 2, "{extended:?}");
        for tile in &extended.1 {
            let Tile::Part(at) = tile else {
                panic!("{tile:?}")
            };
            assert!(disk.partitions[*at].number > 4);
        }
        // The frame first, then what is in it, in disk order.
        let walk = stops(&laid);
        let frame = walk
            .iter()
            .position(|stop| *stop == Stop::Extended(extended.0))
            .unwrap();
        assert_eq!(
            walk[frame + 1],
            Stop::Part(match extended.1[0] {
                Tile::Part(at) => at,
                _ => unreachable!(),
            })
        );
        // Every partition and every gap is a stop, none twice.
        assert_eq!(walk.len(), disk.partitions.len() + disk.free.len());
    }

    #[test]
    fn an_extended_partition_reported_as_a_stub_reaches_its_last_logical_one() {
        let mut disk = demo::partition_disks().remove(0);
        let at = disk
            .partitions
            .iter()
            .position(|p| p.is_extended())
            .unwrap();
        let whole = disk.partitions[at].size;
        disk.partitions[at].size = 1024;
        let laid = tiles(&disk);
        let inside = laid
            .iter()
            .find_map(|tile| match tile {
                Tile::Extended { inside, .. } => Some(inside.clone()),
                _ => None,
            })
            .unwrap();
        let (_, span) = extended_span(&disk, at, &inside);
        assert!(span > whole / 2, "{span} of {whole}");
    }

    #[test]
    fn every_tile_is_at_least_its_least_width_and_the_rest_is_shared_by_size() {
        let items = [
            (MIB, 40.0),
            (16 * MIB, 40.0),
            (372 * GIB, 40.0),
            (279 * GIB, 40.0),
            (750 * MIB, 40.0),
            (278 * GIB, 40.0),
        ];
        for room in [400.0f32, 900.0, 1600.0] {
            let wide = widths(room, 4.0, &items);
            let total: f32 = wide.iter().sum::<f32>() + 4.0 * 5.0;
            assert!((total - room).abs() < 0.01, "{room}: {total}");
            for (width, (_, least)) in wide.iter().zip(items) {
                assert!(*width >= least - 0.01, "{room}: {wide:?}");
            }
            // The three large ones share the rest by their sizes.
            let ratio = wide[2] / wide[5];
            assert!((ratio - 372.0 / 278.0).abs() < 0.01, "{room}: {wide:?}");
            assert!(wide[3] > wide[5] && wide[2] > wide[3]);
        }
    }

    /// Measures of a map at scale one, as the page's are.
    fn measures() -> Measures {
        Measures {
            least: 44.0,
            gap: 4.0,
            inset: 8.0,
            head: 22.0,
        }
    }

    /// Where a map slides (a display standing on its side), each tile is
    /// held to a comfortable width of its own, the map is laid out wider than
    /// the room — those widths, their gaps and a share of the room shared by
    /// size — and an msdos frame holds its logical tiles at theirs.
    #[test]
    fn a_map_that_slides_holds_every_tile_to_its_own_least_width() {
        let room = 471.5;
        for disk in [sata(), demo::partition_disks().remove(0)] {
            let tiles = tiles(&disk);
            let least = |tile: &Tile| match tile.stop() {
                Stop::Free { .. } => 150.0,
                _ => 110.0,
            };
            assert!(!fits(&tiles, room, measures(), least));
            assert!(fits(&tiles, 4000.0, measures(), least));
            let width = spread(&tiles, room, measures(), least);
            assert!(width > room * 1.5, "{}: {width}", disk.path);
            let laid = lay_by(&disk, &tiles, [0.0, 0.0, width, 100.0], measures(), least);
            for one in &laid {
                let wanted = match one.stop {
                    Stop::Free { .. } => 150.0,
                    Stop::Extended(_) => continue,
                    _ => 110.0,
                };
                assert!(one.rect[2] >= wanted - 0.01, "{one:?}");
            }
            let last = laid
                .iter()
                .map(|one| one.rect[0] + one.rect[2])
                .fold(0.0f32, f32::max);
            assert!((last - width).abs() < 0.01, "{last} against {width}");
        }
        // Where the comfortable widths fit, the room is the map's width.
        let few = vec![Tile::Free {
            start: 0,
            size: GIB,
        }];
        assert_eq!(
            spread(&few, room, measures(), |_| 100.0),
            room.max(100.0 + room * SHARE)
        );
    }

    /// A change to the plan eases each tile from where the tile it was
    /// stood: a partition shrunk keeps its start and narrows from its right,
    /// the unallocated space after it keeping its end and widening to the
    /// left with that edge — never over it — and a run made by deleting a
    /// partition beside free space keeps the free space's start and widens
    /// over where the partition was.
    #[test]
    fn tiles_ease_from_where_the_tiles_they_were_stood() {
        use crate::ui::partitions::{apply, Plan};
        let disk = sata();
        let rect = [0.0, 0.0, 1200.0, 100.0];
        let laid = |plan: &Plan| {
            let planned = apply(plan, &disk, crate::model::firmware::Firmware::Uefi);
            let tiles = tiles(&planned.disk);
            spans(&planned.disk, &lay(&planned.disk, &tiles, rect, measures()))
        };
        let windows = disk.partitions[2].clone();
        let before = laid(&Plan::default());
        let mut shrunk = Plan::default();
        shrunk.resize(&disk.path, &windows.path, windows.size, 250 * GIB);
        let after = laid(&shrunk);
        // Nothing moves on the first frame, and the last is the new layout.
        let first = eased(&before, &after, 0.0);
        let last = eased(&before, &after, 1.0);
        let ours = after
            .iter()
            .position(|span| span.start == windows.start)
            .unwrap();
        let old = before
            .iter()
            .position(|span| span.start == windows.start)
            .unwrap();
        assert_eq!(first[ours], before[old].x);
        for (x, span) in last.iter().zip(&after) {
            assert_eq!(*x, span.x);
        }
        // Half way, Windows' right-hand edge is half way between its old
        // size and its new one, and the space after it starts beyond it.
        let half = eased(&before, &after, 0.5);
        let edge = before[old].x[1] + (after[ours].x[1] - before[old].x[1]) * 0.5;
        assert!((half[ours][1] - edge).abs() < 0.01);
        assert!(half[ours + 1][0] > half[ours][1], "{half:?}");
        assert!(after[ours].x[1] < before[old].x[1], "it narrowed");
        // A tile that was nowhere stands where it stands.
        let lone = [Span {
            start: 7,
            end: 9,
            x: [1.0, 2.0],
        }];
        assert_eq!(eased(&before, &lone, 0.0), [[1.0, 2.0]]);
    }

    /// The map slides as little as keeps the lit tile in view with a peek of
    /// its neighbours at either side — never past either end, the first and
    /// last tiles at the ends — whichever tile the light walks to, either way.
    #[test]
    fn a_map_that_slides_keeps_the_lit_tile_in_view_and_its_neighbours_peeking() {
        let disk = sata();
        let tiles = tiles(&disk);
        let least = |_: &Tile| 120.0;
        let room = 471.5;
        let peek = room * 0.12;
        let width = spread(&tiles, room, measures(), least);
        let laid = lay_by(&disk, &tiles, [0.0, 0.0, width, 100.0], measures(), least);
        let edges: Vec<[f32; 2]> = laid
            .iter()
            .map(|one| [one.rect[0], one.rect[0] + one.rect[2]])
            .collect();
        let mut at = 0.0;
        let walk: Vec<usize> = (0..edges.len()).chain((0..edges.len()).rev()).collect();
        for index in walk {
            let lit = edges[index];
            let was = at;
            at = slide_to(at, lit, room, width, peek);
            assert!((0.0..=width - room + 0.01).contains(&at), "{index}: {at}");
            assert!(
                lit[0] >= at - 0.01 && lit[1] <= at + room + 0.01,
                "{index} out of view"
            );
            if index > 0 {
                assert!(lit[0] - at >= peek - 0.01, "{index}: no peek at the left");
            }
            if index + 1 < edges.len() {
                assert!(
                    at + room - lit[1] >= peek - 0.01,
                    "{index}: no peek at the right"
                );
            }
            // Already in view with its peeks: nothing moves.
            assert_eq!(slide_to(at, lit, room, width, peek), at);
            let _ = was;
        }
        // The ends: the first tile with nothing slid, the last at the end.
        assert_eq!(slide_to(200.0, edges[0], room, width, peek), 0.0);
        let end = slide_to(0.0, edges[edges.len() - 1], room, width, peek);
        assert!((end - (width - room)).abs() < 0.01);
        // A tile wider than the room less its peeks stands from its left.
        let wide = [100.0, 100.0 + room];
        assert!((slide_to(0.0, wide, room, 2000.0, peek) - (100.0 - peek)).abs() < 0.01);
    }

    #[test]
    fn a_share_held_up_to_the_least_takes_its_room_from_the_others() {
        // The middle one is under its least width only once the first is
        // held up to its own.
        let items = [(1, 100.0), (30, 100.0), (100, 10.0)];
        let wide = widths(400.0, 0.0, &items);
        assert_eq!(wide[0], 100.0);
        assert_eq!(wide[1], 100.0);
        assert!((wide[2] - 200.0).abs() < 0.01, "{wide:?}");
    }

    #[test]
    fn where_even_the_least_widths_do_not_fit_everything_narrows_alike() {
        let items = [(1, 40.0), (1, 40.0), (1000, 40.0)];
        let wide = widths(90.0, 0.0, &items);
        assert!(wide.iter().all(|width| (width - 30.0).abs() < 0.01));
        assert!(widths(100.0, 2.0, &[]).is_empty());
    }

    #[test]
    fn the_map_comes_to_its_own_width_with_an_extended_frame_round_its_logical_ones() {
        let measures = Measures {
            least: 30.0,
            gap: 3.0,
            inset: 5.0,
            head: 12.0,
        };
        for disk in demo::disks()
            .into_iter()
            .chain(demo::partition_disks())
            .filter(|disk| disk.blocked.is_none())
        {
            let laid_tiles = tiles(&disk);
            for width in [300.0f32, 800.0, 1500.0] {
                let rect = [10.0, 20.0, width, 100.0];
                let laid = lay(&disk, &laid_tiles, rect, measures);
                assert_eq!(laid.len(), stops(&laid_tiles).len());
                let outer: Vec<&Laid> = laid.iter().filter(|one| !one.inside).collect();
                let first = outer.first().unwrap().rect;
                let last = outer.last().unwrap().rect;
                assert!((first[0] - rect[0]).abs() < 0.01);
                assert!(
                    (last[0] + last[2] - (rect[0] + rect[2])).abs() < 0.05,
                    "{}: {width}",
                    disk.path
                );
                for one in &laid {
                    assert!(one.rect[2] >= measures.least - 0.01, "{one:?}");
                }
                // What is inside a frame stands inside it.
                if let Some(frame) = laid
                    .iter()
                    .find(|one| matches!(one.stop, Stop::Extended(_)))
                {
                    for one in laid.iter().filter(|one| one.inside) {
                        assert!(one.rect[0] >= frame.rect[0] + measures.inset - 0.01);
                        assert!(
                            one.rect[0] + one.rect[2]
                                <= frame.rect[0] + frame.rect[2] - measures.inset + 0.05
                        );
                        assert!(one.rect[1] >= frame.rect[1] + measures.head - 0.01);
                    }
                }
            }
        }
    }

    #[test]
    fn a_tiles_words_thin_out_as_it_narrows() {
        let words = Words {
            name: 50.0,
            size: 90.0,
            short: 70.0,
            filesystem: 40.0,
            labelled: true,
            name_line: 32.0,
            line: 27.0,
            mount: None,
            mount_line: 40.0,
        };
        let said = |room, words| said(room, words).0;
        let tall = 32.0 + 27.0 * 3.0;
        assert_eq!(said([120.0, tall], words), Said::All);
        // Its size no longer fits: its name alone.
        assert_eq!(said([85.0, tall], words), Said::Name);
        // A filesystem wider than its size thins first too.
        let wide_fs = Words {
            filesystem: 110.0,
            ..words
        };
        assert_eq!(said([100.0, tall], wide_fs), Said::NameAndSize);
        assert_eq!(said([60.0, tall], words), Said::Name);
        assert_eq!(said([30.0, tall], words), Said::Nothing);
        // And as it shortens: a logical partition inside its frame.
        assert_eq!(said([120.0, 32.0 + 27.0 * 2.0], words), Said::NameAndSize);
        let bare = Words {
            labelled: false,
            ..words
        };
        assert_eq!(said([120.0, 32.0 + 27.0 * 2.0], bare), Said::All);
        assert_eq!(said([120.0, 40.0], words), Said::Name);
        // Never more words for a narrower tile.
        let mut last = Said::All;
        for width in (0..200).rev() {
            let now = said([width as f32, tall], words);
            assert!(now <= last, "{width}: {now:?} after {last:?}");
            last = now;
        }
    }

    #[test]
    fn unallocated_space_too_narrow_for_its_name_still_says_its_size() {
        // "unallocated" is wider than its size: on a narrow map the size
        // stands alone, in whole units where the decimal does not fit.
        let words = Words {
            name: 150.0,
            size: 120.0,
            short: 95.0,
            filesystem: 0.0,
            labelled: false,
            name_line: 32.0,
            line: 27.0,
            mount: None,
            mount_line: 40.0,
        };
        assert_eq!(said([130.0, 200.0], words).0, Said::Size);
        assert_eq!(said([100.0, 200.0], words).0, Said::Size);
        assert_eq!(said([90.0, 200.0], words).0, Said::Nothing);
        assert_eq!(size_short(286_852 * MIB), "280\u{a0}GiB");
        assert_eq!(size_short(750 * MIB), size(750 * MIB));
    }

    #[test]
    fn a_mount_point_stands_under_the_name_and_goes_before_the_name_does() {
        let words = Words {
            name: 50.0,
            size: 90.0,
            short: 70.0,
            filesystem: 40.0,
            labelled: false,
            name_line: 32.0,
            line: 27.0,
            mount: Some(30.0),
            mount_line: 40.0,
        };
        let tall = 32.0 + 40.0 + 27.0 * 2.0;
        assert_eq!(said([120.0, tall], words), (Said::All, true));
        // Shorter: the mount point is kept, the filesystem given up.
        assert_eq!(
            said([120.0, 32.0 + 40.0 + 27.0], words),
            (Said::NameAndSize, true)
        );
        assert_eq!(said([120.0, 32.0 + 40.0], words), (Said::Name, true));
        // Shorter still: the name alone.
        assert_eq!(said([120.0, 40.0], words), (Said::Name, false));
        // A mount point wider than the tile is left to the details.
        let wide = Words {
            mount: Some(200.0),
            ..words
        };
        assert_eq!(said([120.0, tall], wide), (Said::All, false));
    }

    #[test]
    fn a_size_is_written_in_binary_units_as_gparted_writes_it() {
        assert_eq!(size(MIB), "1\u{a0}MiB");
        assert_eq!(size(16 * MIB), "16\u{a0}MiB");
        assert_eq!(size(750 * MIB), "750\u{a0}MiB");
        assert_eq!(size(1024), "1\u{a0}KiB");
        assert_eq!(size(GIB), "1.0\u{a0}GiB");
        assert_eq!(size(1_000_204_886_016), "931.5\u{a0}GiB");
        assert_eq!(size(512_110_190_592), "476.9\u{a0}GiB");
        assert_eq!(size(61_035 * MIB), "59.6\u{a0}GiB");
        assert_eq!(size(2 * 1024 * GIB), "2.0\u{a0}TiB");
        assert_eq!(mib(286_102 * MIB), "286102\u{a0}MiB");
    }

    #[test]
    fn a_partitions_technical_words_are_gparteds() {
        let disk = sata();
        let fs: Vec<Option<&str>> = disk.partitions.iter().map(filesystem).collect();
        assert_eq!(
            fs,
            [
                Some("fat32"),
                None,
                Some("ntfs"),
                Some("ntfs"),
                Some("ntfs")
            ]
        );
        assert_eq!(flags(&disk.partitions[0]), ["boot", "esp"]);
        assert_eq!(flags(&disk.partitions[1]), ["msftres"]);
        assert_eq!(flags(&disk.partitions[2]), ["msftdata"]);
        assert_eq!(device(&disk.partitions[2].path), "sda3");
        assert_eq!(chip_words(&disk), "sda · 931.5\u{a0}GiB");
        assert_eq!(
            disk_line(&disk),
            "/dev/sda · Example SATA HDD · 931.5\u{a0}GiB · gpt"
        );
        let linux = demo::partition_disks().remove(1);
        assert_eq!(flags(&linux.partitions[0]), ["bios_grub"]);
        assert_eq!(linux.partitions[0].size, MIB);
        assert_eq!(filesystem(&linux.partitions[2]), Some("linux-swap"));
    }

    #[test]
    fn every_filesystem_band_stands_off_the_glass() {
        // Every band is told apart from the violet glass by its lightness;
        // unknown, the darkest, by a step still.
        let lightness = |colour: Srgb| {
            let [r, g, b] = [colour.0 >> 16, (colour.0 >> 8) & 0xff, colour.0 & 0xff];
            (r * 299 + g * 587 + b * 114) / 1000
        };
        for name in [
            "ext4",
            "btrfs",
            "xfs",
            "ntfs",
            "fat32",
            "exfat",
            "linux-swap",
            "extended",
        ] {
            assert!(lightness(colour(Some(name))) > 120, "{name}");
        }
        assert!(lightness(colour(None)) > 40);
    }
}
