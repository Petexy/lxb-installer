//! Where to install: the disks, then the places on the one the light is on
//! — the whole disk erased, a gap of free space, or a partition replaced —
//! with a bar of the disk's parts over the places, and Options for how
//! Linexin is stored, the spare memory and a separate place for the
//! person's files.
//!
//! Plain words throughout: a disk is "Example SSD · 1 TB" with what it
//! holds under it, a place is "Replace “Data” · 299 GB" (its own name, where
//! it has one) or "Replace Windows · 400 GB", and nothing on the
//! face of the panel names a device, a filesystem or a partition type. A disk
//! that cannot be used is outlined, says why, and the light walks past it.
//!
//! Every place is judged as it is listed, with the options in force, by the
//! same check the worker makes (`disks::validate_storage`): one that would be
//! refused says why under it, in Danger, before anybody chooses it — and
//! Continue on a refused choice is refused, with the reason said again in the
//! foot. Choosing a place that can be used puts the light on Continue.
//!
//! What is chosen before the person chooses anything (`first_place`) never
//! erases what a disk holds: the whole disk only where it holds nothing, the
//! free space on one that holds something where Linexin fits there, and
//! otherwise nothing — Continue then asks for a place, as it refuses any
//! other choice that cannot be installed.
//!
//! The bar is the disk drawn to scale: each part in a plain colour for what
//! it holds, free space faint, and where Linexin would go — the place the
//! light is on, or the one chosen — in the accent, with a key under it.
//!
//! **Advanced**, at the other end of the button band, under the disks,
//! opens Partitions (`partitions`): every disk laid out as GParted lays one
//! out, in GParted's words — a page of this step, under the same step chip,
//! which Back leaves for this one with the light on Advanced. Options and
//! Continue stand where they always stood: Advanced takes the band's free
//! left-hand end, and where that is too narrow for it (a display standing on
//! its side) it stands at the foot of the disks' column instead and slides
//! with it, the places keeping all their height; the light reaching it
//! slides the view over to the disks.

use lxb_app::lxb_render::{Align, Ui};
use lxb_app::lxb_toolkit::{
    color::Srgb, input::Action, metrics::Metric, palette::Role, sound::Sound, typography::Text,
};
use lxb_app::{Page, Scroll};

use super::{partitions, Head, Room};
use crate::i18n;
use crate::model::config::{Filesystem, Home, InstallConfig, Invalid, Target};
use crate::model::disks::{self, Content, Disk, Gap, Partition, MIN_HOME};
use crate::model::firmware::Firmware;
use crate::ui::app::{Menu, State};
use crate::ui::button::{self, Button};
use crate::ui::column::{Column, Corners, Pick, Row};
use crate::ui::legend::Hint;
use crate::ui::pair::{Pair, Side, Went};
use crate::ui::panel::Note;
use crate::ui::{glyphs, spots, text, words};

/// The numbers the two columns answer to.
const IDS: [spots::Column; 2] = [spots::Column::nth(0), spots::Column::nth(1)];

/// How the two columns' rows are rounded: every disk a card, at every size,
/// whether or not its name takes a second line, so that every disk wears the
/// same shape; the places by their words, one shape for all of them
/// (`column::Corners`).
const CORNERS: [Corners; 2] = [Corners::Cards, Corners::Fitted];

/// The number Advanced answers to: a control of the screen's own, apart
/// from the row of Options and Continue, whose numbers are the row's.
const ADVANCED: u32 = spots::ITEMS;

/// The menus this screen raises, by their numbers.
const OPTIONS: u32 = 1;
const STORAGE: u32 = 2;
const SWAP: u32 = 3;
const HOME: u32 = 4;
const HOME_KEEP: u32 = 5;

/// The sizes of spare memory offered, in MiB; nought is none. Partitions'
/// Options offers the same as its swap file.
pub const SWAP_SIZES: [u32; 5] = [0, 2048, 4096, 8192, 16384];

/// Which page of the step is up.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The disks, the places on them, and the buttons.
    #[default]
    Main,
    /// Partitions, opened with Advanced (`partitions`).
    Partitions,
}

/// Where the light stands on this screen.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    #[default]
    Pair,
    /// Advanced, at the left-hand end of the buttons: Left from Options.
    Advanced,
    /// One of the buttons under the columns: Options, then Continue.
    Buttons(usize),
}

/// The index of Continue among the buttons.
const CONTINUE: usize = 1;

/// What this screen keeps between frames.
#[derive(Debug, Default, Clone)]
pub struct View {
    /// Which page of the step is up: its own, or Partitions.
    pub part: Part,
    pub pair: Pair,
    pub focus: Focus,
    /// The partitions the last "Your files" menu offered, in its order after
    /// "With the system", so its answer can be read back.
    homes: Vec<String>,
    /// The partition picked in that menu, waiting to be kept or erased.
    home_pick: Option<String>,
}

impl View {
    pub fn settle(&mut self) {
        self.pair.settle();
    }
}

/// One place Linexin can go on a disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Erase,
    Free(Gap),
    Replace(String),
}

impl Place {
    /// The mark the place's row wears: an eraser for the whole disk, room
    /// with something to be added for free space, two arrows passing for a
    /// part replaced.
    fn mark(&self) -> &'static str {
        match self {
            Place::Erase => glyphs::PLACE_ERASE,
            Place::Free(_) => glyphs::PLACE_FREE,
            Place::Replace(_) => glyphs::PLACE_REPLACE,
        }
    }

    fn target(&self) -> Target {
        match self {
            Place::Erase => Target::WholeDisk,
            Place::Free(gap) => Target::FreeSpace {
                start: gap.start,
                size: gap.size,
            },
            Place::Replace(path) => Target::Partition { path: path.clone() },
        }
    }
}

/// The places on `disk`, in the order the design lists them: the whole disk,
/// each gap of free space, each partition that may be replaced. A disk with
/// no partition table has no gap to offer — all of it is free, and erasing
/// it is how it is used.
pub fn places(disk: &Disk) -> Vec<Place> {
    let mut places = vec![Place::Erase];
    if disk.table.is_some() {
        places.extend(disk.free.iter().copied().map(Place::Free));
    }
    places.extend(
        disk.partitions
            .iter()
            .filter(|part| part.replaceable())
            .map(|part| Place::Replace(part.path.clone())),
    );
    places
}

/// The place on `disk` chosen before the person has chosen any: the whole
/// disk where it holds nothing at all, else the largest gap of free space
/// Linexin can be installed in, else none. On a disk that holds something —
/// another system, somebody's files, also when they were written over the
/// whole disk with no partitions — a place that erases any of it is only
/// ever the person's own choice.
pub fn first_place(draft: &InstallConfig, disk: &Disk, firmware: Firmware) -> Option<Place> {
    if disk.is_blank() {
        return Some(Place::Erase);
    }
    places(disk)
        .into_iter()
        .filter(|place| matches!(place, Place::Free(_)))
        .filter(|place| disks::validate_storage(&with_place(draft, disk, place), firmware).is_ok())
        .max_by_key(|place| match place {
            Place::Free(gap) => gap.size,
            _ => 0,
        })
}

/// Whether two disks are the same disk.
fn same_disk(a: &Disk, b: &Disk) -> bool {
    a.path == b.path && a.serial == b.serial
}

/// The request as it would be with Linexin put at `place` on `disk`: the
/// options kept, and a separate place for files kept where it is — on this
/// disk, or on the one it was already on — unless it cannot survive the
/// choice: on a disk erased whole, or the very partition replaced.
pub fn with_place(draft: &InstallConfig, disk: &Disk, place: &Place) -> InstallConfig {
    let mut config = draft.clone();
    config.disk = disk.clone();
    config.target = place.target();
    if let Some(home) = config.storage.home.as_mut() {
        // The disk the partition is on, said again from where Linexin now
        // goes: none when it is this disk, else that disk as it was shown.
        let holder = home.disk.take().unwrap_or_else(|| draft.disk.clone());
        if !same_disk(&holder, disk) {
            home.disk = Some(holder);
        }
    }
    let lost = match (&config.storage.home, place) {
        (None, _) => false,
        (Some(home), _) if home.disk.is_some() => false,
        (Some(_), Place::Erase) => true,
        (Some(home), Place::Replace(path)) => home.path == *path,
        (Some(_), Place::Free(_)) => false,
    };
    if lost {
        config.storage.home = None;
    }
    config
}

/// The disk the left-hand light is on.
fn lit_disk(state: &State) -> Option<&Disk> {
    state.disks.get(state.views.target.pair.left.at)
}

/// The left-hand rows: each disk by its model and size, what it holds (or
/// why it cannot be used) under it.
fn left_rows(state: &State) -> Vec<Row> {
    // While Partitions' plan is the choice in use, no disk and no place on
    // it is: nothing here is ticked.
    let planned = partitions::in_use(state);
    state
        .disks
        .iter()
        .map(|disk| {
            let note = match &disk.blocked {
                Some(blocked) => words::blocked(blocked).to_string(),
                None => words::disk_holds(disk),
            };
            Row::new(words::disk_name(disk))
                .note(note)
                .mark(glyphs::disk(disk))
                .enabled(disk.blocked.is_none())
                .chosen(disk.blocked.is_none() && same_disk(disk, &state.draft.disk) && !planned)
        })
        .collect()
}

/// What a partition is, in words: what it holds and its size, its label
/// before them where it has one.
fn part_words(part: &Partition) -> String {
    let what = words::content(part.content);
    let size = words::size(part.size);
    if part.label.trim().is_empty() {
        crate::message!("target-part", "what" => what, "size" => size)
    } else {
        crate::message!(
            "target-part-named",
            "label" => part.label.trim(),
            "what" => what,
            "size" => size
        )
    }
}

/// A partition Linexin can take the place of, as a row says it: by the name
/// the person gave it where it has one — "Replace “Data” · 299 GB", with what
/// it holds under it — and otherwise by what it holds, "Replace Windows ·
/// 400 GB". A label is somebody's own word for the part, so it goes in as a
/// value and the quotes round it come from the catalog; the content word is
/// the fallback, because a part labelled "Data" that happens to be formatted
/// the way Windows formats things is not Windows.
fn replace_words(part: Option<&Partition>) -> (String, String) {
    let what = part.map_or("", |part| words::content(part.content));
    let size = words::size(part.map_or(0, |part| part.size));
    let label = part.map_or("", |part| part.label.trim());
    if label.is_empty() {
        return (
            crate::message!("target-replace", "what" => what, "size" => size),
            i18n::text("target-replace-note").to_string(),
        );
    }
    // "Holds Windows" under a part already named “Windows” says it twice.
    let note = if label.to_lowercase() == what.to_lowercase() {
        i18n::text("target-replace-note").to_string()
    } else {
        crate::message!("target-replace-holds", "what" => what)
    };
    (
        crate::message!("target-replace-labelled", "label" => label, "size" => size),
        note,
    )
}

/// The right-hand rows for the disk the light is on, each judged as it
/// would be installed.
fn right_rows(state: &State) -> Vec<Row> {
    let Some(disk) = lit_disk(state) else {
        return Vec::new();
    };
    let firmware = state.firmware();
    let ours = same_disk(disk, &state.draft.disk) && !partitions::in_use(state);
    places(disk)
        .iter()
        .map(|place| {
            let (name, note) = match place {
                Place::Erase => (
                    i18n::text("target-erase").to_string(),
                    i18n::text("target-erase-note").to_string(),
                ),
                Place::Free(gap) => (
                    crate::message!("target-free", "size" => words::size(gap.size)),
                    i18n::text("target-free-note").to_string(),
                ),
                Place::Replace(path) => replace_words(disk.partition(path)),
            };
            let candidate = with_place(&state.draft, disk, place);
            let refusal = disks::validate_storage(&candidate, firmware).err();
            let row = match refusal {
                Some(invalid) => Row::new(name).note(words::invalid(invalid)).refused(true),
                None => Row::new(name).note(note),
            };
            row.mark(place.mark())
                .chosen(ours && state.draft.target == place.target())
        })
        .collect()
}

/// Put Linexin at the place at `index` on the disk the light is on. Taken
/// whatever the check says, so the options can still be changed to make it
/// fit; the light goes on to Continue only where it can be used.
fn choose(state: &mut State, index: usize) {
    let Some(disk) = lit_disk(state).cloned() else {
        return;
    };
    let Some(place) = places(&disk).get(index).cloned() else {
        return;
    };
    state.draft = with_place(&state.draft, &disk, &place);
    // The place is the choice in use now, the one confirmed last; a plan
    // made on Partitions is kept there as a draft.
    partitions::let_go_of(state);
    if disks::validate_storage(&state.draft, state.firmware()).is_ok() {
        state.views.target.focus = Focus::Buttons(CONTINUE);
        state.sound(Sound::Press);
    } else {
        state.sound(Sound::Error);
    }
}

/// For a test: whether a disk and whether a place on the disk the light is
/// on are ticked as the choice in force.
#[cfg(test)]
pub fn ticks(state: &State) -> (bool, bool) {
    let disks = left_rows(state).iter().any(|row| row.chosen);
    let places = right_rows(state).iter().any(|row| row.chosen);
    (disks, places)
}

/// For a test: choose the place at `index` on the disk the light is on, as
/// Accept on it does.
#[cfg(test)]
pub fn choose_place(state: &mut State, index: usize) {
    choose(state, index);
}

/// For a test: Continue pressed on this page.
#[cfg(test)]
pub fn continue_pressed(state: &mut State) {
    onward(state);
}

/// Whether a place has been chosen at all: until one is, the draft holds the
/// stand-in disk, which has no path (`app::draft`).
fn placed(state: &State) -> bool {
    !state.draft.disk.path.is_empty()
}

/// Continue: on, or refused with the reason where the choice cannot be
/// installed — and with nothing chosen yet, asking for a place (or saying
/// again that there is no disk at all, where that is why).
fn onward(state: &mut State) {
    // Partitions' plan in use: on with it, as Continue on Partitions goes.
    if partitions::in_use(state) {
        return partitions::onward_from_where_to_install(state);
    }
    let checked = if placed(state) {
        disks::validate_storage(&state.draft, state.firmware())
    } else {
        Err(Invalid::TargetMissing)
    };
    match checked {
        Ok(()) => state.forward(),
        Err(invalid) => {
            state.sound(Sound::Error);
            let text =
                note(state).map_or_else(|| words::invalid(invalid).to_string(), |note| note.text);
            state.say(text, Role::Danger);
        }
    }
}

// -------------------------------------------------------------------------
// Options.

fn storage_words(filesystem: Filesystem) -> &'static str {
    match filesystem {
        Filesystem::Ext4 => i18n::text("target-storage-standard"),
        Filesystem::Btrfs => i18n::text("target-storage-snapshots"),
    }
}

fn swap_words(mib: u32) -> String {
    if mib == 0 {
        i18n::text("target-swap-off").to_string()
    } else {
        // Whole gigabytes as the menu offers them, not the 2.1 GB the
        // decimal size of 2048 MiB would come to.
        crate::message!("size-gb", "amount" => (mib / 1024).to_string())
    }
}

/// The partitions that could hold the person's files: on the chosen disk,
/// any that may be replaced, big enough, and not where Linexin itself goes
/// (none where the disk is erased whole); then the same on every other disk
/// that can be written to, a disk too small for Linexin included — as the
/// GTK installer offered them.
fn home_candidates(state: &State) -> Vec<(&Disk, &Partition)> {
    let draft = &state.draft;
    let fits = |part: &&Partition| part.replaceable() && part.size >= MIN_HOME;
    let mut found: Vec<(&Disk, &Partition)> = Vec::new();
    if draft.target != Target::WholeDisk {
        found.extend(
            draft
                .disk
                .partitions
                .iter()
                .filter(fits)
                .filter(|part| {
                    !matches!(&draft.target, Target::Partition { path } if *path == part.path)
                })
                .map(|part| (&draft.disk, part)),
        );
    }
    for disk in state
        .disks
        .iter()
        .filter(|disk| !same_disk(disk, &draft.disk) && disks::holds_homes(disk))
    {
        found.extend(disk.partitions.iter().filter(fits).map(|part| (disk, part)));
    }
    found
}

/// A partition offered for the person's files, as a row of the menu says
/// it: by its name, or what it holds where it has none, and its size — and
/// on another disk than Linexin's, that disk named after it. Shorter than a
/// part is said elsewhere, because a menu row is one line of a menu's own
/// width (lxb-toolkit's); what it holds is said in full in the question
/// that follows, whether to keep it.
fn home_part_words(state: &State, disk: &Disk, part: &Partition) -> String {
    let said = crate::message!(
        "target-part",
        "what" => super::review::part_name(part),
        "size" => words::size(part.size)
    );
    if same_disk(disk, &state.draft.disk) {
        said
    } else {
        crate::message!("part-on-disk", "part" => said, "disk" => words::disk_called(disk))
    }
}

/// The value of the Options row for files: with the system, or the part by
/// its name (or what it holds), kept or erased. Short, for the same reason.
fn home_words(state: &State) -> String {
    match &state.draft.storage.home {
        None => i18n::text("target-home-system").to_string(),
        Some(home) => {
            let part = home_part(state).unwrap_or_default();
            if home.format {
                crate::message!("target-home-erased", "part" => part)
            } else {
                crate::message!("target-home-kept", "part" => part)
            }
        }
    }
}

/// The name of the part the person's files go on, where they go on one of
/// their own.
fn home_part(state: &State) -> Option<String> {
    let home = state.draft.storage.home.as_ref()?;
    disks::home_partition(&state.draft, home).map(|(_, part)| super::review::part_name(part))
}

/// The rows of the Options menu, in order.
const STORAGE_ROW: usize = 0;
const SWAP_ROW: usize = 1;
const HOME_ROW: usize = 2;

/// Raise the Options menu: each option with its setting in force, in every
/// language — restore points in shorter words where the display has no room
/// for the whole row. A part's label, where the files go on one, is cut
/// where its row would not fit.
fn open_options(state: &mut State) {
    open_options_on(state, None);
}

/// Raise the Options menu again once a choice inside one of its options is
/// made, the light on that option's row — the one just changed — so its new
/// setting is seen where the person left it, and the next option is a Down
/// away rather than counted again from the top.
fn back_to_options(state: &mut State, row: usize) {
    open_options_on(state, Some(row));
}

fn open_options_on(state: &mut State, on: Option<usize>) {
    let filesystem = state.draft.storage.filesystem;
    let storage = crate::message!("target-option-storage", "value" => storage_words(filesystem));
    let shorter = (filesystem == Filesystem::Btrfs).then(|| {
        let short = i18n::text("target-storage-snapshots-short");
        crate::message!("target-option-storage", "value" => short)
    });
    let spare = swap_words(state.draft.storage.swap_mib);
    let swap = crate::message!("target-option-swap", "value" => spare);
    let home = crate::message!("target-option-home", "value" => home_words(state));
    state.menu(Menu {
        id: OPTIONS,
        title: Some(i18n::text("target-options").to_string()),
        commands: vec![storage, swap, home],
        named: vec![None, None, home_part(state)],
        shorter: vec![shorter],
        marked: None,
        on,
    });
}

/// The menu of how Linexin is stored, the one in force marked.
fn open_storage(state: &mut State) {
    let marked = match state.draft.storage.filesystem {
        Filesystem::Ext4 => 0,
        Filesystem::Btrfs => 1,
    };
    state.menu(Menu {
        id: STORAGE,
        title: Some(i18n::text("target-storage").to_string()),
        commands: vec![
            storage_words(Filesystem::Ext4).to_string(),
            storage_words(Filesystem::Btrfs).to_string(),
        ],
        named: Vec::new(),
        shorter: Vec::new(),
        marked: Some(marked),
        on: None,
    });
}

/// The menu of spare memory sizes, the one in force marked.
fn open_swap(state: &mut State) {
    let marked = SWAP_SIZES
        .iter()
        .position(|mib| *mib == state.draft.storage.swap_mib);
    state.menu(Menu {
        id: SWAP,
        title: Some(i18n::text("target-swap").to_string()),
        commands: SWAP_SIZES.iter().map(|mib| swap_words(*mib)).collect(),
        named: Vec::new(),
        shorter: Vec::new(),
        marked,
        on: None,
    });
}

/// The menu of where the person's files are kept: with the system, or on
/// one of the chosen disk's other partitions, or on another disk's.
fn open_home(state: &mut State) {
    let candidates: Vec<(String, String)> = home_candidates(state)
        .into_iter()
        .map(|(disk, part)| (part.path.clone(), home_part_words(state, disk, part)))
        .collect();
    let marked = match state.draft.storage.home.as_ref() {
        None => Some(0),
        Some(home) => candidates
            .iter()
            .position(|(path, _)| *path == home.path)
            .map(|at| at + 1),
    };
    let mut commands = vec![i18n::text("target-home-system").to_string()];
    commands.extend(candidates.iter().map(|(_, words)| words.clone()));
    state.views.target.homes = candidates.into_iter().map(|(path, _)| path).collect();
    state.menu(Menu {
        id: HOME,
        title: Some(i18n::text("target-home").to_string()),
        commands,
        named: Vec::new(),
        shorter: Vec::new(),
        marked,
        on: None,
    });
}

/// The disk holding the partition at `path`, when it is not the chosen
/// disk: the separate place for files carries it.
fn home_disk(state: &State, path: &str) -> Option<Disk> {
    if state.draft.disk.partition(path).is_some() {
        return None;
    }
    state
        .disks
        .iter()
        .find(|disk| disk.partition(path).is_some())
        .cloned()
}

/// The question whether the partition picked for files keeps what is on it,
/// asked under the part's own words: its name, what it holds and its size.
fn open_home_keep(state: &mut State, path: String) {
    let title = home_candidates(state)
        .into_iter()
        .find(|(_, part)| part.path == path)
        .map(|(_, part)| part_words(part))
        .unwrap_or_default();
    let marked = state
        .draft
        .storage
        .home
        .as_ref()
        .filter(|home| home.path == path)
        .map(|home| usize::from(home.format));
    state.views.target.home_pick = Some(path);
    state.menu(Menu {
        id: HOME_KEEP,
        title: Some(title),
        commands: vec![
            i18n::text("target-home-keep").to_string(),
            i18n::text("target-home-erase").to_string(),
        ],
        named: Vec::new(),
        shorter: Vec::new(),
        marked,
        on: None,
    });
}

/// The row of a menu this screen raised. A choice inside an option goes back
/// to the Options menu, so its new state is seen beside the others, with the
/// light on the option just changed. Partitions' menus are its own.
pub fn chose(state: &mut State, id: u32, row: usize) {
    if state.views.target.part == Part::Partitions {
        return partitions::chose(state, id, row);
    }
    match (id, row) {
        (OPTIONS, STORAGE_ROW) => open_storage(state),
        (OPTIONS, SWAP_ROW) => open_swap(state),
        (OPTIONS, HOME_ROW) => open_home(state),
        (STORAGE, _) => {
            state.draft.storage.filesystem = if row == 1 {
                Filesystem::Btrfs
            } else {
                Filesystem::Ext4
            };
            back_to_options(state, STORAGE_ROW);
        }
        (SWAP, _) => {
            if let Some(mib) = SWAP_SIZES.get(row) {
                state.draft.storage.swap_mib = *mib;
            }
            back_to_options(state, SWAP_ROW);
        }
        (HOME, 0) => {
            state.draft.storage.home = None;
            back_to_options(state, HOME_ROW);
        }
        (HOME, _) => {
            if let Some(path) = state.views.target.homes.get(row - 1).cloned() {
                open_home_keep(state, path);
            }
        }
        (HOME_KEEP, _) => {
            if let Some(path) = state.views.target.home_pick.take() {
                let disk = home_disk(state, &path);
                state.draft.storage.home = Some(Home {
                    path,
                    format: row == 1,
                    disk,
                });
            }
            back_to_options(state, HOME_ROW);
        }
        _ => {}
    }
}

// -------------------------------------------------------------------------
// The screen.

fn buttons() -> [Button<'static>; 2] {
    [
        Button::plain(i18n::text("button-options")),
        Button::primary(i18n::text("button-continue")),
    ]
}

/// The pictures of this screen. `target`: the disk with Windows on it, its
/// free space chosen and the light on it, so the bar shows where Linexin
/// goes beside what is kept. The rest show what is chosen before the person
/// chooses anything, the screen as it opens: `target-erase` on the made-up
/// machine as it is (its empty disk first), `target-free` with the Windows
/// disk first, `target-nothing` with the Windows disk first and too little
/// room left on it, `target-whole` with Linux written over the whole of the
/// empty disk, `target-kinds` with an SSD and a memory card beside its
/// disks, so every kind of disk wears its mark in one picture;
/// `target-options` is `target` with Options pressed, its menu
/// growing out of the Options button while the light stays on the place —
/// with the storage that keeps restore points in force, the longest row it
/// says — and `target-storage` the menu of how Linexin is stored opened from
/// it, that storage marked, on the row whose mark takes some of its room;
/// `target-refused`
/// is `target-nothing` with the light on the free space too small for
/// Linexin, its refusal under it. `target-partitions-in-use` is `target`
/// with Partitions' whole plan (`partitions::stage_plan`) the choice in use,
/// back from Partitions with the light on Advanced: no place ticked, the
/// foot saying it. The scenes of Partitions open it from
/// `target`, with the two disks the made-up machine shows only there
/// (`partitions::stage`).
pub fn stage(state: &mut State, scene: &str) {
    if scene.starts_with("partitions") {
        state.disks.extend(crate::ui::demo::partition_disks());
        stage_free_space(state);
        open_partitions(state);
        partitions::stage(state, scene);
        return;
    }
    match scene {
        "target-erase" => {}
        "target-free" => open_on_windows(state, false),
        "target-nothing" => open_on_windows(state, true),
        "target-refused" => {
            open_on_windows(state, true);
            light_on_free_space(state);
        }
        "target-partitions-in-use" => {
            stage_free_space(state);
            partitions::stage_plan(state);
            state.views.partitions.in_use = true;
            enter(state);
            to_advanced(state);
        }
        "target-whole" => open_written_over(state),
        "target-kinds" => {
            state.disks.extend(crate::ui::demo::other_kinds());
            reopen(state);
        }
        "target-options" => {
            stage_free_space(state);
            state.draft.storage.filesystem = Filesystem::Btrfs;
            open_options(state);
        }
        "target-storage" => {
            stage_free_space(state);
            state.draft.storage.filesystem = Filesystem::Btrfs;
            open_storage(state);
        }
        _ => stage_free_space(state),
    }
}

/// The made-up machine with Linux written over the whole of its empty disk,
/// no partitions, and Where to install opened on it afresh.
fn open_written_over(state: &mut State) {
    if let Some(disk) = state.disks.iter_mut().find(|disk| disk.is_blank()) {
        disk.content = Some(Content::Linux);
        disk.free.clear();
    }
    reopen(state);
}

/// Where to install opened afresh on the disks as they now are.
fn reopen(state: &mut State) {
    let source = state.draft.source.to_string_lossy().into_owned();
    let fresh = crate::ui::app::draft(&state.disks, &source, state.firmware());
    state.draft.disk = fresh.disk;
    state.draft.target = fresh.target;
    enter(state);
}

/// The made-up machine without its empty disk, Where to install opened on
/// it afresh; `full`: Windows grown over all but 20 GB of its gap, too
/// little for Linexin.
fn open_on_windows(state: &mut State, full: bool) {
    state
        .disks
        .retain(|disk| disk.blocked.is_some() || !disk.partitions.is_empty());
    if full {
        if let Some(disk) = state.disks.iter_mut().find(|disk| disk.blocked.is_none()) {
            let keep = 20_000 * disks::MIB;
            if let Some(gap) = disk.free.first_mut().filter(|gap| gap.size > keep) {
                let grown = gap.size - keep;
                if let Some(before) = disk
                    .partitions
                    .iter_mut()
                    .find(|part| part.start + part.size == gap.start)
                {
                    before.size += grown;
                }
                gap.start += grown;
                gap.size = keep;
            }
        }
    }
    reopen(state);
}

/// The light on the first disk's free space, chosen or not.
fn light_on_free_space(state: &mut State) {
    let Some(disk) = lit_disk(state) else {
        return;
    };
    if let Some(at) = places(disk)
        .iter()
        .position(|place| matches!(place, Place::Free(_)))
    {
        let pair = &mut state.views.target.pair;
        pair.side = Side::Right;
        pair.right.at = at;
    }
}

/// `target`: the free space on the Windows disk chosen, the light on it.
fn stage_free_space(state: &mut State) {
    let Some(at) = state
        .disks
        .iter()
        .position(|disk| disk.blocked.is_none() && !disk.partitions.is_empty())
    else {
        return;
    };
    let disk = state.disks[at].clone();
    let listed = places(&disk);
    let Some(index) = listed
        .iter()
        .position(|place| matches!(place, Place::Free(_)))
    else {
        return;
    };
    state.draft = with_place(&state.draft, &disk, &listed[index]);
    let pair = &mut state.views.target.pair;
    pair.left.at = at;
    pair.side = Side::Right;
    pair.right.at = index;
}

/// Advanced: Partitions, showing the disk the light is on among the disks.
pub fn open_partitions(state: &mut State) {
    let from = state.views.target.pair.left.at;
    state.views.target.part = Part::Partitions;
    state.note = None;
    partitions::open(state, from);
    state.sound(Sound::Press);
}

/// Back from Partitions: this page again, the light on Advanced, where the
/// person left for it.
pub fn close_partitions(state: &mut State) {
    state.views.target.part = Part::Main;
    to_advanced(state);
    state.note = None;
    state.sound(Sound::Back);
}

pub fn enter(state: &mut State) {
    // The light opens on the disk in force, or the first that can be used —
    // while Partitions' plan is the choice in use, on the first disk it
    // changes.
    let left = left_rows(state);
    let planned = partitions::in_use(state).then(|| {
        let plan = &state.views.partitions.plan;
        state
            .disks
            .iter()
            .position(|disk| disk.blocked.is_none() && plan.on_disk(&disk.path) > 0)
    });
    let view = &mut state.views.target;
    view.part = Part::Main;
    view.focus = Focus::Pair;
    view.pair.side = Side::Left;
    view.pair.left.start_at_chosen(&left);
    if let Some(at) = planned.flatten() {
        view.pair.left.at = at;
    }
    let right = right_rows(state);
    state.views.target.pair.right.start_at_chosen(&right);
}

pub fn head(state: &State) -> Head {
    if state.views.target.part == Part::Partitions {
        return partitions::head(state);
    }
    Head::new(
        glyphs::DISK_TARGET,
        i18n::text("target-title"),
        i18n::text("target-caption"),
    )
}

pub fn hints(state: &State) -> Vec<Hint> {
    if state.views.target.part == Part::Partitions {
        return partitions::hints(state);
    }
    super::choose_options_and_back()
}

/// Said in the foot: while Partitions' plan is the choice in use, "Using
/// your partitions · N operations" (or that it was let go of, its disks
/// changed); else only where there is no disk at all to choose. Partitions
/// says what its plan still needs on its own line.
pub fn note(state: &State) -> Option<Note> {
    if state.views.target.part == Part::Partitions {
        return None;
    }
    if let Some(text) = partitions::standing(state) {
        return Some(Note {
            text,
            role: Role::Text,
        });
    }
    state
        .disks
        .iter()
        .all(|disk| disk.blocked.is_some())
        .then(|| Note {
            text: i18n::text("target-no-disk").to_string(),
            role: Role::Danger,
        })
}

fn focus(state: &mut State, to: Focus) {
    if state.views.target.focus != to {
        state.views.target.focus = to;
        state.sound(Sound::Move);
    }
    if to == Focus::Advanced {
        to_advanced(state);
    }
}

/// The light on Advanced, which stands under the disks: the pair's own light
/// goes back to the disks too, so Up from it lands on them — and on a
/// display standing on its side, where Advanced stands at the foot of the
/// disks' column, the view slides over to it.
fn to_advanced(state: &mut State) {
    let view = &mut state.views.target;
    view.focus = Focus::Advanced;
    view.pair.side = Side::Left;
}

pub fn act(state: &mut State, action: Action, scroll: Option<Scroll>) {
    if state.views.target.part == Part::Partitions {
        return partitions::act(state, action, scroll);
    }
    if let Some(scroll) = scroll {
        if IDS.iter().any(|ids| Column::owns(scroll.spot, *ids)) {
            state.views.target.focus = Focus::Pair;
            state.views.target.pair.aim(scroll, IDS);
        }
    }
    if action == Action::Menu {
        return open_options(state);
    }
    if state.views.target.focus == Focus::Advanced {
        match action {
            Action::Right => focus(state, Focus::Buttons(0)),
            Action::Up => focus(state, Focus::Pair),
            Action::Accept => open_partitions(state),
            Action::Back => state.back(),
            _ => {}
        }
        return;
    }
    if let Focus::Buttons(at) = state.views.target.focus {
        match action {
            Action::Left if at > 0 => focus(state, Focus::Buttons(at - 1)),
            Action::Left => focus(state, Focus::Advanced),
            Action::Right if at < CONTINUE => focus(state, Focus::Buttons(at + 1)),
            Action::Up => focus(state, Focus::Pair),
            Action::Accept if at == CONTINUE => onward(state),
            Action::Accept => open_options(state),
            Action::Back => state.back(),
            _ => {}
        }
        return;
    }
    let left = left_rows(state);
    let right = right_rows(state);
    let went = state.views.target.pair.act(action, &left, &right);
    match went {
        Went::Nothing => {
            if matches!(action, Action::Down | Action::Right) {
                focus(state, Focus::Buttons(CONTINUE));
            }
        }
        Went::Moved { left } => {
            if left {
                let right = right_rows(state);
                state.views.target.pair.right.start_at_chosen(&right);
            }
            state.sound(Sound::Move);
        }
        Went::Crossed { pressed, .. } => {
            state.sound(if pressed { Sound::Press } else { Sound::Move });
        }
        Went::Chose(index) => choose(state, index),
        Went::Backed => state.sound(Sound::Back),
        Went::Pass => {
            if action == Action::Back {
                state.back();
            }
        }
    }
}

/// How tall the bar is, in reference pixels.
const BAR: f32 = 16.0;

/// How tall the band over the places is: the bar and its key.
fn band_height(page: &Page) -> f32 {
    page.scaled(BAR) + page.metric(Metric::Gap) * 1.6 + page.line(Text::Caption)
}

/// The plain colour of what a part holds.
fn content_colour(content: Content) -> Srgb {
    match content {
        Content::Windows => Srgb(0x5aa9e6),
        Content::Linux => Srgb(0xe2a54a),
        Content::Swap => Srgb(0xb48ad6),
        Content::Data => Srgb(0x4fb7a3),
        Content::Efi
        | Content::BiosBoot
        | Content::MicrosoftReserved
        | Content::WindowsRecovery => Srgb(0x9aa3b5),
        Content::Empty | Content::Other => Srgb(0x8c8c8c),
    }
}

/// What one stretch of the bar is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stretch {
    Part(Content),
    Free,
    Linexin,
}

impl Stretch {
    fn colour(self, ui: &Ui) -> [f32; 4] {
        match self {
            Stretch::Linexin => ui.tinted(Role::Accent, 1.0),
            Stretch::Free => ui.tinted(Role::Text, 0.16),
            Stretch::Part(content) => content_colour(content).a(0.92),
        }
    }

    fn words(self) -> &'static str {
        match self {
            Stretch::Linexin => i18n::text("target-key-linexin"),
            Stretch::Free => i18n::text("target-key-free"),
            Stretch::Part(content) => words::content(content),
        }
    }
}

/// Every stretch of `disk` the bar draws, in disk order: its partitions and
/// its free gaps, the one Linexin would take (`at`) marked as Linexin — or
/// one stretch of Linexin across the whole disk where it is erased, and one
/// of what it holds where that was written over the whole disk.
fn stretches(disk: &Disk, at: Option<&Place>) -> Vec<(u64, u64, Stretch)> {
    if matches!(at, Some(Place::Erase)) {
        return vec![(0, disk.size, Stretch::Linexin)];
    }
    if let (Some(content), true) = (disk.content, disk.partitions.is_empty()) {
        return vec![(0, disk.size, Stretch::Part(content))];
    }
    let mut all: Vec<(u64, u64, Stretch)> = disk
        .partitions
        .iter()
        .filter(|part| !part.is_extended())
        .map(|part| {
            let here = matches!(at, Some(Place::Replace(path)) if *path == part.path);
            let kind = if here {
                Stretch::Linexin
            } else {
                Stretch::Part(part.content)
            };
            (part.start, part.size, kind)
        })
        .chain(disk.free.iter().map(|free| {
            let here = matches!(at, Some(Place::Free(chosen)) if chosen == free);
            let kind = if here {
                Stretch::Linexin
            } else {
                Stretch::Free
            };
            (free.start, free.size, kind)
        }))
        .collect();
    all.sort_by_key(|(start, _, _)| *start);
    all
}

/// The kinds of stretch on a bar, in the order its key names them: Linexin
/// first, then by how much of the disk each kind takes, largest first.
fn key(laid: &[(u64, u64, Stretch)]) -> Vec<Stretch> {
    let mut kinds: Vec<(Stretch, u64)> = Vec::new();
    for (_, length, kind) in laid {
        match kinds
            .iter_mut()
            .find(|(one, _)| one.words() == kind.words())
        {
            Some((_, total)) => *total += length,
            None => kinds.push((*kind, *length)),
        }
    }
    kinds.sort_by_key(|(kind, total)| (*kind != Stretch::Linexin, std::cmp::Reverse(*total)));
    kinds.into_iter().map(|(kind, _)| kind).collect()
}

/// The bar of `disk` in `band`: its parts and gaps to scale, where Linexin
/// would go (`at`) in the accent, and a key of what the colours are under it.
fn draw_bar(page: &mut Page, band: [f32; 4], disk: &Disk, at: Option<&Place>) {
    let tall = page.scaled(BAR);
    let gap = page.metric(Metric::Gap);
    let line = page.line(Text::Caption);
    let least = page.scaled(4.0);
    let bar = [band[0], band[1] + gap * 0.3, band[2], tall];
    let size = disk.size.max(1) as f64;
    let laid = stretches(disk, at);

    let ui = page.ui();
    // The track is the disk itself: what no stretch covers (alignment, the
    // table's own room) shows as the glass it stands on.
    ui.chip(bar, ui.tinted(Role::Glass, 0.55));
    // What is on the track is a picture of the disk, and the dots of the key
    // are marks beside its words: they go with the page's words, not with
    // the glass, as the page comes in or goes (`Ui::words_between`).
    let pictured = ui.drawn();
    for (start, length, kind) in &laid {
        let x0 = bar[0] + (*start as f64 / size * bar[2] as f64) as f32;
        let x1 = bar[0] + ((*start + *length) as f64 / size * bar[2] as f64) as f32;
        let width = (x1 - x0).max(least).min(bar[0] + bar[2] - x0);
        // A hair between neighbours, so two parts of one colour still read
        // as two — where the stretch is wide enough to spare it.
        let hair = if width > 6.0 { 1.0 } else { 0.0 };
        let fill = kind.colour(ui);
        ui.chip(
            [x0 + hair, bar[1], (width - hair * 2.0).max(1.0), tall],
            fill,
        );
    }

    // The key: one dot and word for each kind of stretch on the bar, as many
    // as fit on the line — Linexin first, then the rest by how much of the
    // disk they take, so a narrow panel loses the slivers (the start-up
    // files, the space Windows keeps) and never what the bar is about.
    let kinds = key(&laid);
    let dot = line * 0.42;
    let mut x = band[0];
    let y = bar[1] + tall + gap * 0.8;
    for kind in kinds {
        let label = kind.words();
        let wide = ui.measure(Text::Caption, label);
        if x + dot + gap * 0.4 + wide > band[0] + band[2] {
            break;
        }
        let fill = kind.colour(ui);
        ui.chip([x, y + (line - dot) * 0.5, dot, dot], fill);
        text::label_faded(
            ui,
            [x + dot + gap * 0.4, y, wide + 1.0, line],
            Text::Caption,
            label,
            text::LESSER.0,
            text::LESSER.1,
            Align::Left,
        );
        x += dot + gap * 0.4 + wide + gap * 1.2;
    }
    let to = ui.drawn();
    ui.words_between(pictured, to);
}

fn advanced() -> Button<'static> {
    Button::plain(i18n::text("button-advanced"))
}

/// Whether a button `width` wide stands at the left-hand end of a band that
/// starts at `band_x`, `gap` clear of the first of the row's own buttons,
/// which starts at `row_x`.
pub fn stands_beside(band_x: f32, width: f32, row_x: f32, gap: f32) -> bool {
    band_x + width + gap <= row_x + 0.5
}

/// Where Advanced stands: at the left-hand end of the band, under the
/// disks, its foot on the band's foot like every button's — Options and
/// Continue not moved by a pixel — answered as its rectangle; or, where the
/// band has no room left of Options for it (a display standing on its side),
/// at the foot of the disks' column, sliding with it — answered as how much
/// of that column's foot it keeps (`Pair::draw`'s `foot`).
fn place_advanced(page: &mut Page, band: [f32; 4], row_x: f32) -> Result<[f32; 4], f32> {
    let gap = page.metric(Metric::Gap);
    let tall = button::height(page);
    let width = button::width(page, advanced().label);
    if stands_beside(band[0], width, row_x, gap) {
        Ok([band[0], band[1] + band[3] - tall, width, tall])
    } else {
        Err(tall + gap * 0.25)
    }
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    if state.views.target.part == Part::Partitions {
        return partitions::draw(state, page, room, lit);
    }
    let Room {
        content,
        buttons: band,
        ..
    } = room;
    let buttons = buttons();
    let places_at = button::places(page, band, &buttons);
    let beside = place_advanced(page, band, places_at[0][0]);
    let left = left_rows(state);
    let right = right_rows(state);
    let focus = state.views.target.focus;
    let pressing = state.pressing.through();
    let dt = state.clock.dt;
    let disk = lit_disk(state).cloned();
    let heading = disk.as_ref().map(words::disk_name).unwrap_or_default();
    let in_pair = lit && focus == Focus::Pair;
    state.views.target.pair.pad = state.pad;
    let light = in_pair.then_some(&mut state.light);
    let reserved = state.views.target.pair.draw(
        page,
        content,
        [i18n::text("target-heading-disks"), &heading],
        &left,
        &right,
        light,
        IDS,
        CORNERS,
        pressing,
        dt,
        band_height(page),
        beside.err().unwrap_or(0.0),
    );

    // The bar shows where Linexin would go: the place the light is on while
    // it is among the places, else the place chosen on this disk.
    if let Some(disk) = &disk {
        let listed = places(disk);
        let pair = &state.views.target.pair;
        let at = if in_pair && pair.side == Side::Right {
            listed.get(pair.right.at).cloned()
        } else if same_disk(disk, &state.draft.disk) && !partitions::in_use(state) {
            listed
                .into_iter()
                .find(|place| place.target() == state.draft.target)
        } else {
            None
        };
        let from = page.ui().written();
        draw_bar(page, reserved.band, disk, at.as_ref());
        if let Some(cut) = reserved.cut {
            let to = page.ui().written();
            page.ui().cut_between(from, to, cut);
        }
    }

    // Every menu of this screen grows out of Options, however it was asked
    // for: the button, the right button, North or the menu key.
    state.anchor_menus(places_at[0]);
    // Advanced: in the band, or at the foot of the disks' column, sliding
    // and cut with it.
    let advanced_at = beside.unwrap_or_else(|kept| {
        let [x, y, w, _] = reserved.foot;
        let tall = button::height(page);
        let width = button::width(page, advanced().label).min(w);
        [x, y + kept - tall, width, tall]
    });
    let lit_advanced = (lit && focus == Focus::Advanced).then_some(0);
    let from = page.ui().written();
    let laid = button::row_numbered(
        page,
        &[advanced_at],
        &[advanced()],
        lit_advanced,
        &mut state.light,
        pressing,
        dt,
        ADVANCED,
    );
    state.laid_buttons(laid);
    if let (Err(_), Some(cut)) = (beside, reserved.cut) {
        let to = page.ui().written();
        page.ui().cut_between(from, to, cut);
    }
    let lit_button = match focus {
        Focus::Buttons(at) if lit => Some(at),
        _ => None,
    };
    let laid = button::row(
        page,
        &places_at,
        &buttons,
        lit_button,
        &mut state.light,
        pressing,
        dt,
    );
    state.laid_buttons(laid);
}

/// Put the light on row `index` of the column on `side`, where the pointer
/// moved onto it (`hovered`) or clicked it — a disk shows its places, as
/// Up and Down to it would. Answers whether it stands there.
fn point(state: &mut State, side: Side, index: usize, hovered: bool) -> bool {
    let left = left_rows(state);
    let right = right_rows(state);
    let pair = &mut state.views.target.pair;
    let Some(went) = pair.point_at(side, index, &left, &right, hovered) else {
        return false;
    };
    state.views.target.focus = Focus::Pair;
    if went.left_moved {
        let right = right_rows(state);
        state.views.target.pair.right.start_at_chosen(&right);
    }
    true
}

pub fn hover(state: &mut State, id: u32) {
    if state.views.target.part == Part::Partitions {
        return partitions::hover(state, id);
    }
    if id == ADVANCED {
        // Not while the view is slid over to the places, where Advanced has
        // slid out of the panel with the disks.
        if state.views.target.pair.slid_to() != Some(Side::Right) {
            to_advanced(state);
        }
        return;
    }
    if let Some(at) = super::button_of(id, buttons().len()) {
        state.views.target.focus = Focus::Buttons(at);
        return;
    }
    let (left, right) = (left_rows(state), right_rows(state));
    if let Some((side, index)) = Pair::row_of(id, IDS, &left, &right) {
        point(state, side, index, true);
    }
}

pub fn pressed(state: &mut State, page: &mut Page) {
    if state.views.target.part == Part::Partitions {
        return partitions::pressed(state, page);
    }
    let left = left_rows(state);
    let right = right_rows(state);
    let advanced_pressed = button::pressed_numbered(page, &[advanced()], ADVANCED).is_some();
    match state.views.target.pair.pressed(page, &left, &right, IDS) {
        Some((side, Pick::Moved(_))) => {
            state.views.target.focus = Focus::Pair;
            if side == Side::Left {
                let right = right_rows(state);
                state.views.target.pair.right.start_at_chosen(&right);
            }
            state.sound(Sound::Move);
            return;
        }
        Some((side, Pick::Clicked(index))) => {
            // A disk opens its places, a place is chosen: what Accept on it
            // does.
            if point(state, side, index, false) {
                act(state, Action::Accept, None);
            }
            return;
        }
        None => {}
    }
    if advanced_pressed {
        to_advanced(state);
        return open_partitions(state);
    }
    match button::pressed(page, &buttons()) {
        Some(CONTINUE) => {
            state.views.target.focus = Focus::Buttons(CONTINUE);
            onward(state);
        }
        Some(at) => {
            state.views.target.focus = Focus::Buttons(at);
            open_options(state);
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::DEFAULT_SOURCE;
    use crate::model::disks::MIB;
    use crate::ui::app::Screen;

    fn on_target() -> State {
        let mut state = State::demo(true);
        state.go(Screen::Target);
        state
    }

    /// Where to install on the made-up machine with `disks` in place of its
    /// own, the draft starting as it would on them.
    fn on_target_with(disks: Vec<Disk>) -> State {
        let mut state = State::demo(true);
        let fresh = crate::ui::app::draft(&disks, DEFAULT_SOURCE, state.firmware());
        state.draft.disk = fresh.disk;
        state.draft.target = fresh.target;
        state.disks = disks;
        state.go(Screen::Target);
        state
    }

    /// The made-up Windows disk alone, with its gap of free space `free`
    /// bytes long (none at all for nought).
    fn windows_alone(free: u64) -> Vec<Disk> {
        let mut disks = crate::ui::demo::disks();
        let mut windows = disks.remove(1);
        windows.free[0].size = free;
        windows.free.retain(|gap| gap.size > 0);
        vec![windows]
    }

    #[test]
    fn every_disk_is_a_card_whatever_its_name_says_and_the_places_one_shape() {
        use crate::ui::column::{self, Span};
        use crate::ui::motion::Shape;
        let state = on_target();
        // Every row one line tall, as on a wide display.
        let one_line = |count: usize| -> Vec<Span> {
            (0..count)
                .map(|index| Span {
                    top: index as f32 * 100.0,
                    tall: 90.0,
                })
                .collect()
        };
        let disks = left_rows(&state);
        assert!(
            disks.iter().any(|disk| !disk.enabled),
            "the made-up machine's installer stick, outlined, is among them"
        );
        let shape =
            |at: usize, rows: &[Row], spans: &[Span]| column::shape(CORNERS[at], rows, spans, 90.0);
        assert_eq!(shape(0, &disks, &one_line(disks.len())), Shape::Card);
        // The places by their words: capsules on one line each, cards all
        // of them once one has grown. (On the Windows disk, which has
        // several.)
        let places = right_rows(&on_target_with(windows_alone(286_102 * MIB)));
        assert!(places.len() > 2, "{places:?}");
        let mut spans = one_line(places.len());
        assert_eq!(shape(1, &places, &spans), Shape::Capsule);
        spans[1].tall = 130.0;
        assert_eq!(shape(1, &places, &spans), Shape::Card);
    }

    #[test]
    fn an_empty_disk_starts_erased_with_the_light_on_it() {
        let mut state = on_target();
        assert!(left_rows(&state)[0].chosen);
        assert_eq!(state.views.target.pair.left.at, 0);
        let right = right_rows(&state);
        assert!(right[0].chosen, "Erase this disk");
        assert_eq!(state.views.target.pair.right.at, 0);
        // Continue as it stands goes on.
        state.views.target.focus = Focus::Buttons(CONTINUE);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Desktop);
    }

    #[test]
    fn a_disk_that_holds_something_starts_on_its_free_space_never_erased() {
        let disks = windows_alone(286_102 * MIB);
        let mut state = on_target_with(disks);
        let right = right_rows(&state);
        assert!(
            !right[0].chosen,
            "Erase this disk was chosen for the person"
        );
        assert!(right[1].chosen, "{right:?}");
        assert!(left_rows(&state)[0].chosen);
        // The light starts on the free space, and taking it goes on as before.
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.target.pair.right.at, 1);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.target.focus, Focus::Buttons(CONTINUE));
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Desktop);
        assert!(matches!(state.draft.target, Target::FreeSpace { .. }));
    }

    #[test]
    fn a_disk_that_holds_something_without_room_starts_with_nothing_chosen() {
        for free in [20_000 * MIB, 0] {
            let mut state = on_target_with(windows_alone(free));
            assert!(!placed(&state));
            assert!(left_rows(&state).iter().all(|row| !row.chosen));
            assert!(right_rows(&state).iter().all(|row| !row.chosen));
            // The light still opens on the disk, and on the first place when
            // it crosses — lit, not chosen.
            assert_eq!(state.views.target.pair.left.at, 0);
            act(&mut state, Action::Accept, None);
            assert_eq!(state.views.target.pair.right.at, 0);
            assert!(!placed(&state));
            // Continue with nothing chosen goes nowhere and asks for a place.
            state.views.target.focus = Focus::Buttons(CONTINUE);
            act(&mut state, Action::Accept, None);
            assert_eq!(state.screen, Screen::Target);
            assert_eq!(
                state.note.as_ref().map(|note| note.text.as_str()),
                Some(words::invalid(Invalid::TargetMissing))
            );
            // A place the person chooses is taken, and Continue goes on.
            state.views.target.focus = Focus::Pair;
            let windows = places(&state.disks[0])
                .iter()
                .position(|place| matches!(place, Place::Replace(_)))
                .unwrap();
            choose(&mut state, windows);
            assert!(placed(&state));
            assert_eq!(state.views.target.focus, Focus::Buttons(CONTINUE));
            act(&mut state, Action::Accept, None);
            assert_eq!(state.screen, Screen::Desktop);
        }
    }

    /// The made-up empty disk with Linux written over the whole of it, no
    /// partition table.
    fn written_over_whole() -> Disk {
        let mut disk = crate::ui::demo::disks().remove(0);
        disk.content = Some(Content::Linux);
        disk.free.clear();
        disk
    }

    #[test]
    fn a_disk_written_over_whole_holds_something_and_is_never_erased_unasked() {
        let disk = written_over_whole();
        assert_eq!(
            first_place(&State::demo(true).draft, &disk, Firmware::Uefi),
            None
        );
        let mut state = on_target_with(vec![disk.clone()]);
        assert!(!placed(&state));
        // Said by what it holds, never "Empty".
        let left = left_rows(&state);
        let note = left[0].note.clone().unwrap_or_default();
        assert!(note.contains(words::content(Content::Linux)), "{note}");
        assert_ne!(note, i18n::text("disk-empty"));
        // Drawn as what it holds across the whole bar, with no free space.
        assert_eq!(
            stretches(&disk, None),
            vec![(0, disk.size, Stretch::Part(Content::Linux))]
        );
        // Erasing it is the one place, lit and not chosen.
        assert_eq!(places(&disk), vec![Place::Erase]);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.target.pair.right.at, 0);
        assert!(right_rows(&state).iter().all(|row| !row.chosen));
        state.views.target.focus = Focus::Buttons(CONTINUE);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Target, "went on with nothing chosen");
        // The person chooses to erase it: taken, and the consent names it.
        state.views.target.focus = Focus::Pair;
        choose(&mut state, 0);
        assert_eq!(state.draft.target, Target::WholeDisk);
        assert_eq!(disks::consent(&state.draft), vec![disk.path.clone()]);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Desktop);
    }

    #[test]
    fn the_first_place_is_the_largest_gap_linexin_fits_in() {
        let state = on_target();
        let mut disk = state.disks[windows_disk(&state)].clone();
        // Its one gap split in two: 10 GB, a sliver, and the rest.
        let whole = disk.free[0];
        let small = Gap {
            start: whole.start,
            size: 10_000 * MIB,
        };
        let big = Gap {
            start: whole.start + 10_001 * MIB,
            size: whole.size - 10_001 * MIB,
        };
        disk.free = vec![small, big];
        assert_eq!(
            first_place(&state.draft, &disk, Firmware::Uefi),
            Some(Place::Free(big))
        );
        // The big one gone, the small one is too small to be chosen.
        disk.free.retain(|gap| *gap == small);
        assert_eq!(first_place(&state.draft, &disk, Firmware::Uefi), None);
    }

    #[test]
    fn with_no_disk_at_all_continue_says_so_again() {
        let mut stick = crate::ui::demo::disks().remove(2);
        stick.blocked = Some(disks::Blocked::InstallationMedia);
        let mut state = on_target_with(vec![stick]);
        state.views.target.focus = Focus::Buttons(CONTINUE);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Target);
        assert_eq!(
            state.note.as_ref().map(|note| note.text.as_str()),
            Some(i18n::text("target-no-disk"))
        );
    }

    /// The made-up disk with Windows on it.
    fn windows_disk(state: &State) -> usize {
        state
            .disks
            .iter()
            .position(|disk| {
                disk.partitions
                    .iter()
                    .any(|p| p.content == Content::Windows)
            })
            .unwrap()
    }

    /// On the Windows disk, its free space chosen.
    fn on_free_space() -> State {
        let mut state = on_target();
        state.views.target.pair.left.at = windows_disk(&state);
        choose(&mut state, 1);
        state
    }

    #[test]
    fn an_empty_disk_offers_only_to_be_erased() {
        let state = on_target();
        assert_eq!(places(&state.disks[0]), vec![Place::Erase]);
    }

    #[test]
    fn a_windows_disk_offers_its_free_space_and_every_part_that_may_be_replaced() {
        let state = on_target();
        let disk = &state.disks[windows_disk(&state)];
        let listed = places(disk);
        assert_eq!(listed[0], Place::Erase);
        assert!(matches!(listed[1], Place::Free(_)));
        let replaced: Vec<&str> = listed
            .iter()
            .filter_map(|place| match place {
                Place::Replace(path) => Some(path.as_str()),
                _ => None,
            })
            .collect();
        // Windows and the data part; never the start-up files, the space
        // Windows keeps or its recovery tools.
        assert_eq!(replaced.len(), 2, "{replaced:?}");
        for path in replaced {
            assert!(disk.partition(path).unwrap().replaceable());
        }
    }

    #[test]
    fn the_light_walks_past_the_disk_that_cannot_be_used() {
        let mut state = on_target();
        let rows = left_rows(&state);
        assert!(!rows[2].enabled, "the stick the installer runs from");
        state.views.target.pair.left.at = 1;
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.target.pair.left.at, 1);
    }

    #[test]
    fn choosing_free_space_takes_it_and_goes_to_continue() {
        let mut state = on_target();
        state.views.target.pair.left.at = windows_disk(&state);
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Accept, None);
        assert!(matches!(state.draft.target, Target::FreeSpace { .. }));
        assert_eq!(state.draft.disk.serial, "DEMO-SATA-0002");
        assert_eq!(state.views.target.focus, Focus::Buttons(CONTINUE));
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Desktop);
    }

    #[test]
    fn a_place_too_small_for_the_spare_memory_says_so_and_continue_is_refused() {
        let mut state = on_target();
        let at = windows_disk(&state);
        state.views.target.pair.left.at = at;
        // Spare memory more than a quarter of the room is refused: here, half
        // of the 300 GB gap.
        let gap = state.disks[at].free[0].size;
        state.draft.storage.swap_mib = (gap / MIB / 2) as u32;
        let rows = right_rows(&state);
        assert!(rows[1].refused, "{:?}", rows[1]);
        assert_eq!(
            rows[1].note.as_deref(),
            Some(words::invalid(Invalid::SwapTooLarge))
        );
        choose(&mut state, 1);
        assert_eq!(state.views.target.focus, Focus::Pair, "the light moved on");
        onward(&mut state);
        assert_eq!(state.screen, Screen::Target, "a refused choice went on");
        assert!(state.note.is_some(), "the refusal was not said");
    }

    #[test]
    fn options_change_the_storage_the_swap_and_the_place_for_files() {
        let mut state = on_free_space();
        chose(&mut state, OPTIONS, 0);
        chose(&mut state, STORAGE, 1);
        assert_eq!(state.draft.storage.filesystem, Filesystem::Btrfs);
        chose(&mut state, OPTIONS, 1);
        chose(&mut state, SWAP, 0);
        assert_eq!(state.draft.storage.swap_mib, 0);
        chose(&mut state, OPTIONS, 2);
        assert_eq!(
            state.views.target.homes.len(),
            2,
            "Windows and the data part"
        );
        chose(&mut state, HOME, 2);
        chose(&mut state, HOME_KEEP, 1);
        let home = state.draft.storage.home.clone().expect("a place for files");
        assert!(home.format);
        assert!(disks::validate_storage(&state.draft, state.firmware()).is_ok());
    }

    /// The toolkit's menu as the harness walks it: it opens with the light
    /// on the row marked as in force (lxb-toolkit's way), on the row
    /// it was raised on where none is (Options come back to, `Page::menu_on`),
    /// or on its first row; `downs` presses of Down move the light, and Enter
    /// chooses.
    fn walk_menu(state: &mut State, downs: usize) -> u32 {
        let menu = state.raised_menu().expect("no menu raised");
        let row = menu.marked.or(menu.on).unwrap_or(0) + downs;
        assert!(row < menu.commands.len(), "past the foot of {menu:?}");
        chose(state, menu.id, row);
        menu.id
    }

    /// The harness's presses through Where to install's Options
    /// (`tests/e2e/scenarios.py`, `set_storage_options`, after Enter, Enter
    /// on the empty disk): each reaches what its comment names, and Escape
    /// leaves the light on Continue.
    #[test]
    fn the_harness_presses_through_options_reach_what_they_name() {
        for keep in [true, false] {
            let mut state = on_target();
            act(&mut state, Action::Accept, None); // across to Erase this disk
            act(&mut state, Action::Accept, None); // chosen again
            assert_eq!(state.draft.target, Target::WholeDisk);
            assert_eq!(state.views.target.focus, Focus::Buttons(CONTINUE));
            act(&mut state, Action::Menu, None); // F10: Options
            assert_eq!(walk_menu(&mut state, 0), OPTIONS); // Storage
            assert_eq!(walk_menu(&mut state, 1), STORAGE); // With restore points
            assert_eq!(state.draft.storage.filesystem, Filesystem::Btrfs);
            let back = state.raised_menu().expect("Options again");
            assert_eq!((back.id, back.on), (OPTIONS, Some(STORAGE_ROW)));
            state.menu(back);
            assert_eq!(walk_menu(&mut state, 1), OPTIONS); // Spare memory
            let swap = state.draft.storage.swap_mib;
            assert_eq!(swap, 4096, "the default the harness expects");
            assert_eq!(walk_menu(&mut state, 0), SWAP); // as it was
            assert_eq!(state.draft.storage.swap_mib, swap);
            // Back on Spare memory, the option just changed: one Down on.
            assert_eq!(walk_menu(&mut state, 1), OPTIONS); // Your files
            assert_eq!(walk_menu(&mut state, 1), HOME); // the other disk's part
            let part = state.views.target.homes[0].clone();
            let downs = usize::from(!keep);
            assert_eq!(walk_menu(&mut state, downs), HOME_KEEP); // keep / erase
            let home = state.draft.storage.home.clone().expect("a place for files");
            assert_eq!((home.path, home.format), (part, !keep));
            let options = state.raised_menu().expect("Options again");
            assert_eq!(options.id, OPTIONS);
            assert_eq!(options.on, Some(HOME_ROW), "back on Your files");
            assert_eq!(options.marked, None, "an act is never ticked");
            // Escape closes it in the toolkit; the light is where it was.
            assert_eq!(state.views.target.focus, Focus::Buttons(CONTINUE));
        }
    }

    /// The harness's look at Options on the way to an installation that
    /// keeps every default (`tests/e2e/scenarios.py`,
    /// `look_at_storage_options`): Storage and Spare memory are opened and
    /// each is chosen as it was, so the installation is the one it would
    /// have been, and Escape leaves the light on Continue.
    #[test]
    fn the_harness_looks_through_options_and_changes_nothing() {
        let mut state = on_target();
        act(&mut state, Action::Accept, None); // across to Erase this disk
        act(&mut state, Action::Accept, None); // chosen again
        let before = state.draft.storage.clone();
        act(&mut state, Action::Menu, None); // F10: Options
        assert_eq!(walk_menu(&mut state, 0), OPTIONS); // Storage
        assert_eq!(walk_menu(&mut state, 0), STORAGE); // as it was
        assert_eq!(walk_menu(&mut state, 1), OPTIONS); // Spare memory
        assert_eq!(walk_menu(&mut state, 0), SWAP); // as it was
        assert_eq!(state.draft.storage, before);
        assert_eq!(state.raised_menu().map(|menu| menu.id), Some(OPTIONS));
        assert_eq!(state.views.target.focus, Focus::Buttons(CONTINUE));
    }

    #[test]
    fn a_place_for_files_that_cannot_survive_the_choice_is_let_go_of() {
        let mut state = on_free_space();
        let data = state.draft.disk.partitions[4].path.clone();
        state.draft.storage.home = Some(Home {
            path: data.clone(),
            format: true,
            disk: None,
        });
        let index = places(&state.draft.disk)
            .iter()
            .position(|place| *place == Place::Replace(data.clone()))
            .unwrap();
        choose(&mut state, index);
        assert!(state.draft.storage.home.is_none(), "replaced, yet kept");
        state.draft.storage.home = Some(Home {
            path: data,
            format: true,
            disk: None,
        });
        choose(&mut state, 0);
        assert!(state.draft.storage.home.is_none(), "erased, yet kept");
    }

    #[test]
    fn files_may_be_kept_on_another_disk_when_linexin_erases_its_own() {
        // The made-up machine as it opens: its empty disk erased.
        let mut state = on_target();
        assert_eq!(state.draft.target, Target::WholeDisk);
        let sata = state.disks[windows_disk(&state)].clone();
        chose(&mut state, OPTIONS, 2);
        // Windows and the data part, on the other disk, that disk named;
        // never the start-up files, the space Windows keeps, its recovery
        // tools, or the stick the installer runs from.
        let homes = state.views.target.homes.clone();
        assert_eq!(homes.len(), 2, "{homes:?}");
        assert!(homes.iter().all(|path| sata.partition(path).is_some()));
        for (disk, part) in home_candidates(&state) {
            let row = home_part_words(&state, disk, part);
            assert!(row.contains(&words::disk_called(&sata)), "{row}");
            assert!(!row.contains("/dev"), "{row}");
        }
        // The data part: kept is refused (it is not Linux's), erased is not.
        let data = sata.partitions.iter().find(|p| p.label == "Data").unwrap();
        let at = homes.iter().position(|path| *path == data.path).unwrap();
        chose(&mut state, HOME, at + 1);
        chose(&mut state, HOME_KEEP, 0);
        let home = state.draft.storage.home.clone().expect("a place for files");
        assert_eq!(
            home.disk.as_ref().map(|d| d.serial.as_str()),
            Some("DEMO-SATA-0002")
        );
        assert_eq!(
            disks::validate_storage(&state.draft, state.firmware()),
            Err(Invalid::HomeInvalid)
        );
        chose(&mut state, OPTIONS, 2);
        chose(&mut state, HOME, at + 1);
        chose(&mut state, HOME_KEEP, 1);
        assert!(disks::validate_storage(&state.draft, state.firmware()).is_ok());
        assert_eq!(
            disks::consent(&state.draft),
            vec![state.draft.disk.path.clone(), data.path.clone()]
        );
        let (_, body, _) = state.question_words(crate::ui::app::Question::Erase);
        let body = body.replace('\u{a0}', " ");
        assert!(body.contains(&words::disk_called(&sata)), "{body}");
        assert!(body.contains("“Data”"), "{body}");
        // Said in Options by its name, short enough for a menu row.
        assert!(
            home_words(&state).contains("“Data”"),
            "{}",
            home_words(&state)
        );
    }

    #[test]
    fn a_place_for_files_stays_on_its_disk_wherever_linexin_goes() {
        let mut state = on_target();
        let nvme = state.disks[0].clone();
        let at = windows_disk(&state);
        let sata = state.disks[at].clone();
        let data = sata.partitions.iter().find(|p| p.label == "Data").unwrap();
        state.draft.storage.home = Some(Home {
            path: data.path.clone(),
            format: true,
            disk: Some(sata.clone()),
        });
        // Linexin into the other disk's free space: the files' disk is now
        // Linexin's, and the place for them is kept.
        let free = with_place(&state.draft, &sata, &Place::Free(sata.free[0]));
        let home = free.storage.home.clone().expect("kept");
        assert_eq!((home.path.as_str(), home.disk), (data.path.as_str(), None));
        // Back onto the empty disk: on another disk again, carried with it.
        let back = with_place(&free, &nvme, &Place::Erase);
        let home = back.storage.home.clone().expect("kept");
        assert_eq!(home.disk.map(|d| d.serial), Some(sata.serial.clone()));
        // The disk holding it erased, or the part itself replaced: let go of.
        assert!(with_place(&state.draft, &sata, &Place::Erase)
            .storage
            .home
            .is_none());
        assert!(
            with_place(&state.draft, &sata, &Place::Replace(data.path.clone()))
                .storage
                .home
                .is_none()
        );
    }

    #[test]
    fn keeping_files_on_a_windows_part_is_refused_in_words() {
        let mut state = on_free_space();
        state.draft.storage.home = Some(Home {
            path: state.draft.disk.partitions[4].path.clone(),
            format: false,
            disk: None,
        });
        let rows = right_rows(&state);
        assert!(rows[1].refused);
        assert_eq!(
            rows[1].note.as_deref(),
            Some(words::invalid(Invalid::HomeInvalid))
        );
    }

    #[test]
    fn the_bar_marks_where_linexin_goes() {
        let state = on_free_space();
        let disk = &state.draft.disk;
        let gap = Place::Free(disk.free[0]);
        let laid = stretches(disk, Some(&gap));
        assert_eq!(
            laid.iter()
                .filter(|(_, _, kind)| *kind == Stretch::Linexin)
                .count(),
            1
        );
        assert!(
            laid.windows(2).all(|two| two[0].0 <= two[1].0),
            "out of order"
        );
        assert_eq!(stretches(disk, Some(&Place::Erase)).len(), 1);
        let named = key(&laid);
        assert_eq!(named[0], Stretch::Linexin, "{named:?}");
        assert_eq!(named[1], Stretch::Part(Content::Windows), "{named:?}");
    }

    #[test]
    fn a_part_with_a_name_is_called_by_it_and_one_without_by_what_it_holds() {
        let state = on_target();
        let disk = &state.disks[windows_disk(&state)];
        let data = disk.partitions.iter().find(|p| p.label == "Data").unwrap();
        let (name, note) = replace_words(Some(data));
        assert!(name.contains("“Data”"), "{name}");
        assert!(name.contains(&words::size(data.size)), "{name}");
        assert!(note.contains(words::content(data.content)), "{note}");
        let mut bare = data.clone();
        bare.label = "  ".into();
        let (name, _) = replace_words(Some(&bare));
        assert!(!name.contains('“'), "{name}");
        assert!(name.contains(words::content(bare.content)), "{name}");
    }

    #[test]
    fn hovering_a_disk_shows_its_places_and_a_click_on_one_chooses_it() {
        let mut state = on_target();
        let was = state.draft.target.clone();
        let windows = windows_disk(&state);
        hover(&mut state, IDS[0].row(windows));
        assert_eq!(state.views.target.pair.left.at, windows);
        let right = right_rows(&state);
        assert!(right.len() > 1, "{right:?}");
        hover(&mut state, IDS[1].row(0));
        let pair = &state.views.target.pair;
        assert_eq!((pair.side, pair.right.at), (Side::Right, 0));
        assert_eq!(state.draft.target, was, "a hover chose a place");
        hover(&mut state, spots::BUTTONS);
        assert_eq!(state.views.target.focus, Focus::Buttons(0));
        hover(&mut state, spots::BUTTONS + CONTINUE as u32);
        assert_eq!(state.views.target.focus, Focus::Buttons(CONTINUE));
        // A click on the erase place chooses it at once.
        assert!(point(&mut state, Side::Right, 0, false));
        act(&mut state, Action::Accept, None);
        assert_eq!(state.draft.target, Target::WholeDisk);
        assert_eq!(state.draft.disk.path, state.disks[windows].path);
    }

    #[test]
    fn left_from_options_reaches_advanced_which_opens_partitions() {
        let mut state = on_target();
        state.views.target.focus = Focus::Buttons(CONTINUE);
        act(&mut state, Action::Left, None);
        assert_eq!(state.views.target.focus, Focus::Buttons(0), "Options");
        state.views.target.pair.side = Side::Right;
        act(&mut state, Action::Left, None);
        assert_eq!(state.views.target.focus, Focus::Advanced);
        // It stands under the disks: the pair's light goes back to them, so
        // Up lands there and a display on its side slides over to them.
        assert_eq!(state.views.target.pair.side, Side::Left);
        act(&mut state, Action::Left, None);
        assert_eq!(state.views.target.focus, Focus::Advanced, "past the end");
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.target.focus, Focus::Buttons(0));
        act(&mut state, Action::Left, None);
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.target.focus, Focus::Pair);
        // Accept on it opens Partitions, a page of this step: the same
        // screen, the same chip in the head, nothing chosen changed.
        state.views.target.focus = Focus::Advanced;
        let target = state.draft.target.clone();
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Target);
        assert_eq!(state.views.target.part, Part::Partitions);
        assert_eq!(state.draft.target, target);
        assert_eq!(head(&state).title, i18n::text("partitions-title"));
        // Back: here again, the light on Advanced; Back again leaves the step.
        act(&mut state, Action::Back, None);
        assert_eq!(state.views.target.part, Part::Main);
        assert_eq!(state.views.target.focus, Focus::Advanced);
        assert_eq!(head(&state).title, i18n::text("target-title"));
        act(&mut state, Action::Back, None);
        assert_eq!(state.screen, Screen::Keyboard);
    }

    #[test]
    fn the_pointer_lights_advanced_and_arriving_again_is_on_the_steps_own_page() {
        let mut state = on_target();
        hover(&mut state, ADVANCED);
        assert_eq!(state.views.target.focus, Focus::Advanced);
        hover(&mut state, spots::BUTTONS);
        assert_eq!(state.views.target.focus, Focus::Buttons(0));
        open_partitions(&mut state);
        // Coming to the step again, from Keyboard or back from Desktop, is
        // coming to its own page.
        state.go(Screen::Desktop);
        state.go(Screen::Target);
        assert_eq!(state.views.target.part, Part::Main);
    }

    #[test]
    fn the_legend_and_the_options_stay_as_they_were() {
        let mut state = on_target();
        let before = hints(&state);
        state.views.target.focus = Focus::Advanced;
        assert_eq!(hints(&state), before, "the legend changed for Advanced");
        act(&mut state, Action::Menu, None);
        assert_eq!(state.raised_menu().map(|menu| menu.id), Some(OPTIONS));
    }

    /// Advanced stands beside Options where the band has room left of it,
    /// and over the band where it has not — a display standing on its side.
    #[test]
    fn advanced_stands_beside_the_row_where_there_is_room_for_it() {
        assert!(stands_beside(100.0, 200.0, 400.0, 14.0));
        assert!(stands_beside(100.0, 200.0, 314.0, 14.0));
        assert!(!stands_beside(100.0, 200.0, 300.0, 14.0));
    }

    #[test]
    fn no_face_of_the_panel_names_a_device() {
        let mut state = on_target();
        for at in 0..state.disks.len() {
            state.views.target.pair.left.at = at;
            for row in left_rows(&state).iter().chain(right_rows(&state).iter()) {
                let all = format!("{} {}", row.name, row.note.clone().unwrap_or_default());
                assert!(!all.contains("/dev"), "{all}");
                assert!(!all.to_lowercase().contains("ntfs"), "{all}");
            }
        }
    }
}
