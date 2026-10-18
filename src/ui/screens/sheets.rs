//! Partitions' sheets: New partition, Resize, Label… and Mount point ·
//! Other… — panels laid over the page under its map, which stays in sight
//! above them and draws the plan with the sheet's values in it as they are
//! being chosen, the new tile growing as its size moves
//! (`INSTALLER-HANDOFF.md` §7.0, Unit P, *Sheets* and *The size bar*).
//!
//! **Rows like a column's.** Each row is a capsule saying what it sets over
//! its value, as the installer's fields do: a size with its track, a choice
//! that raises a menu (Position, File system, Mount point), a field typed
//! into (a label, a path). The light walks them with Up and Down — on a
//! landscape panel New partition stands its four choices two by two under
//! its size, and Left and Right cross between the two — and its last row is
//! the page's row of buttons, Cancel where Options stood and the sheet's
//! verb where Continue stands. Back, B or Escape cancels and nothing
//! changes; the verb puts one operation in the plan (`ui::partitions`).
//!
//! **The size bar is held**, as LineXinBar's Settings bars are: A takes hold
//! of it (the row turns to the accent, its value in a capsule of it); Left
//! and Right step, LB (Shift+Tab) ten steps down and RB (Tab) ten up
//! (`sheet::stepped`); Y types a number on a board — on a keyboard its
//! digits go straight in — which Enter keeps, in GiB, with either decimal
//! mark; A keeps it, B puts back what it was. The pointer takes hold of the
//! track, or of the room on the map — the tile it sizes and the unallocated
//! space beside it — and drags its edge or clicks along it; the wheel over
//! the row steps it.
//!
//! **Nothing here can reach a disk**: a sheet changes the plan the page
//! keeps, and only through the plan's own operations.

use lxb_app::lxb_render::{Align, Carry, Press, Spot};
use lxb_app::lxb_toolkit::{
    color::Srgb, input::Action, material::Surface, menu, metrics::Metric, motion, palette::Role,
    sound::Sound, typography::Text,
};
use lxb_app::{Page, Scroll};

use super::partitions as page_of;
use super::Room;
use crate::i18n;
use crate::model::disks::MIB;
use crate::ui::app::{Menu, State};
use crate::ui::button::{self, Button};
use crate::ui::field::{self, FieldId, Look};
use crate::ui::legend::{hint, Button as Pictured, Hint};
use crate::ui::map::{self, Stop};
use crate::ui::motion::Shape;
use crate::ui::partitions::{
    self as planning, Fs, Mount, NewPart, Plan, Planned, Room as SizeRoom,
};
use crate::ui::sheet::{self as arithmetic, PathRefused};
use crate::ui::{glyphs, spots, text};

/// What a sheet is for, and what it holds while it is up.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    New(New),
    Resize(Resize),
    Label(Labelling),
    Other(Other),
}

/// New partition: made in the run of unallocated space `gap` (its start and
/// length) on `disk`, as New Partition #`number`.
#[derive(Debug, Clone, PartialEq)]
pub struct New {
    pub disk: String,
    pub number: u32,
    pub gap: (u64, u64),
    pub size: u64,
    pub position: Position,
    pub fs: Fs,
    pub mount: Option<Mount>,
    pub label: String,
    /// The filesystems the File system row offers here.
    pub filesystems: Vec<Fs>,
}

/// Where in its unallocated space a new partition stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Start,
    End,
}

/// Resize: the partition at `part` of `disk`, starting at `start`, `from`
/// bytes long on the disk now, made `size` long within `room`.
#[derive(Debug, Clone, PartialEq)]
pub struct Resize {
    pub disk: String,
    pub part: String,
    pub start: u64,
    pub from: u64,
    pub size: u64,
    pub room: SizeRoom,
    /// Its filesystem as the plan leaves it, by `map::filesystem`'s name:
    /// what colours its track, as it colours its tile's band.
    pub filesystem: Option<String>,
}

/// Label…: the label of the partition at `part` of `disk`, whose
/// filesystem — as the plan leaves it — is `filesystem`; `was` is the label
/// it has without this sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct Labelling {
    pub disk: String,
    pub part: String,
    pub filesystem: String,
    pub was: String,
    pub label: String,
}

/// Mount point · Other…: a path for the partition at `part` of `disk` — or,
/// asked from New partition's Mount point, for the partition it is making,
/// which comes back with it (`back`).
#[derive(Debug, Clone, PartialEq)]
pub struct Other {
    pub disk: String,
    pub part: String,
    pub typed: String,
    pub back: Option<Box<New>>,
}

/// A row of a sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Size,
    Position,
    Filesystem,
    Mount,
    Label,
    Path,
    Cancel,
    Apply,
}

/// A size held in hand: what it was when it was taken hold of (B puts it
/// back), and a number being typed for it, if one is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub was: u64,
    pub typed: Option<String>,
}

/// A sheet over Partitions.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub kind: Kind,
    /// The row the light is on, among [`Sheet::rows`].
    pub at: usize,
    pub held: Option<Held>,
    /// The verb was pressed and refused: what refuses it is said in bold
    /// until something changes.
    pub refused: bool,
    /// A label's letter was refused as it was typed or pasted: why is said
    /// in the full white of a refusal — its filesystem's limit, for a letter
    /// it does not hold or has no room for, or that no label holds a `/` —
    /// until the label is changed again.
    pub label_refused: Option<arithmetic::LabelRefused>,
    /// Whether it was laid out two by two on the last frame drawn (a
    /// landscape panel's New partition), which Left and Right cross.
    pub wide: bool,
    /// How far it has unfolded, along the toolkit's menus' flight: from
    /// nought as it is raised, at once in low-end; and, once it closes, how
    /// far it is from having folded away (`Folding`).
    out: f32,
    /// Its card as the last frame drew it.
    pub(super) card: Option<Card>,
    /// Its size moved since the map was last drawn: the map draws the new
    /// size at once rather than easing to it, as a size held is drawn.
    pub(super) sized: bool,
}

/// What the light does with a sheet's rows: where each stands, line by
/// line and side by side.
fn places(rows: &[Row], wide: bool) -> Vec<(usize, usize)> {
    let mut line = 0;
    let mut placed = Vec::new();
    for row in rows {
        let place = match (row, wide) {
            (Row::Cancel, _) => (usize::MAX, 0),
            (Row::Apply, _) => (usize::MAX, 1),
            (Row::Position | Row::Mount, true) => (line, 0),
            (Row::Filesystem | Row::Label, true) => {
                let place = (line, 1);
                line += 1;
                place
            }
            _ => {
                let place = (line, 0);
                line += 1;
                place
            }
        };
        placed.push(place);
    }
    // The buttons' line comes after the last of the rows.
    for place in &mut placed {
        if place.0 == usize::MAX {
            place.0 = line;
        }
    }
    placed
}

impl Sheet {
    pub fn new(kind: Kind) -> Self {
        let mut sheet = Self {
            kind,
            at: 0,
            held: None,
            refused: false,
            label_refused: None,
            wide: false,
            out: 0.0,
            card: None,
            sized: false,
        };
        sheet.at = sheet
            .rows()
            .iter()
            .position(|row| sheet.enabled(*row))
            .unwrap_or(0);
        sheet
    }

    /// Put it where it is going: unfolded.
    pub fn settle(&mut self) {
        self.out = 1.0;
    }

    /// Carry its unfolding a frame of `dt` seconds on, along the toolkit's
    /// menus' flight, and answer how far it has travelled (eased).
    pub(super) fn unfolding(&mut self, dt: f32) -> f32 {
        self.out = (self.out + dt / motion::duration::MENU_FLIGHT).min(1.0);
        motion::ease(self.out)
    }

    /// Its rows, the buttons last.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = match &self.kind {
            Kind::New(_) => vec![
                Row::Size,
                Row::Position,
                Row::Filesystem,
                Row::Mount,
                Row::Label,
            ],
            Kind::Resize(_) => vec![Row::Size],
            Kind::Label(_) => vec![Row::Label],
            Kind::Other(_) => vec![Row::Path],
        };
        rows.extend([Row::Cancel, Row::Apply]);
        rows
    }

    /// The row the light is on.
    pub fn row(&self) -> Row {
        self.rows().get(self.at).copied().unwrap_or(Row::Cancel)
    }

    /// Whether the light can stand on `row`: a size whose room leaves it no
    /// choice is fixed, and GRUB's partition has no mount point or label.
    pub fn enabled(&self, row: Row) -> bool {
        match (&self.kind, row) {
            (_, Row::Size) => self.range().is_some_and(|(least, most, _)| least < most),
            (Kind::New(new), Row::Mount | Row::Label) => new.fs != Fs::BiosGrub,
            _ => true,
        }
    }

    /// How small and how large its size may be, and — where the space is
    /// smaller than the partition must be — what it must be at least.
    pub fn range(&self) -> Option<(u64, u64, Option<u64>)> {
        match &self.kind {
            Kind::New(new) => Some(new_range(new)),
            Kind::Resize(resize) => Some((resize.room.least, resize.room.most, None)),
            Kind::Label(_) | Kind::Other(_) => None,
        }
    }

    /// Its size, where it has one.
    pub fn size(&self) -> Option<u64> {
        match &self.kind {
            Kind::New(new) => Some(new.size),
            Kind::Resize(resize) => Some(resize.size),
            Kind::Label(_) | Kind::Other(_) => None,
        }
    }

    /// The size the map and the track show: a number being typed, as far
    /// as it can be read and held to the room, else the size.
    pub fn shown_size(&self) -> Option<u64> {
        let size = self.size()?;
        let typed = self.held.as_ref().and_then(|held| held.typed.as_deref());
        match (typed.and_then(arithmetic::parse_gib), self.range()) {
            (Some(bytes), Some((least, most, _))) => Some(bytes.clamp(least, most)),
            _ => Some(size),
        }
    }

    /// Set its size, held to its range; answers whether it moved.
    pub fn set_size(&mut self, size: u64) -> bool {
        let Some((least, most, _)) = self.range() else {
            return false;
        };
        // A partition resized keeps its start: its end is what lands on a
        // MiB (the same thing, for a start on one).
        let size = match &self.kind {
            Kind::Resize(resize) => {
                let end = (resize.start + size) / MIB * MIB;
                end.saturating_sub(resize.start)
            }
            _ => arithmetic::aligned(size),
        }
        .clamp(least, most);
        let slot = match &mut self.kind {
            Kind::New(new) => &mut new.size,
            Kind::Resize(resize) => &mut resize.size,
            Kind::Label(_) | Kind::Other(_) => return false,
        };
        let moved = *slot != size;
        *slot = size;
        if moved {
            self.refused = false;
            self.sized = true;
        }
        moved
    }

    /// Step its size `steps` steps (`sheet::stepped`); answers whether it
    /// moved.
    pub fn step(&mut self, steps: i32) -> bool {
        let (Some((least, most, _)), Some(size)) = (self.range(), self.size()) else {
            return false;
        };
        self.set_size(arithmetic::stepped(size, least, most, steps))
    }

    /// Move the light `direction` — Up, Down, Left or Right — to the row
    /// standing there that it can stand on. Answers whether it moved.
    fn walk(&mut self, action: Action) -> bool {
        let rows = self.rows();
        let placed = places(&rows, self.wide);
        let (line, side) = placed[self.at];
        let candidates: Vec<usize> = (0..rows.len())
            .filter(|at| *at != self.at && self.enabled(rows[*at]))
            .collect();
        let found = match action {
            Action::Left | Action::Right => candidates.into_iter().find(|at| {
                let (there, other) = placed[*at];
                there == line
                    && if action == Action::Left {
                        other + 1 == side
                    } else {
                        other == side + 1
                    }
            }),
            Action::Up | Action::Down => {
                let lines: Vec<usize> = if action == Action::Up {
                    (0..line).rev().collect()
                } else {
                    (line + 1..=placed.iter().map(|place| place.0).max().unwrap_or(0)).collect()
                };
                lines.into_iter().find_map(|wanted| {
                    let on: Vec<usize> = candidates
                        .iter()
                        .copied()
                        .filter(|at| placed[*at].0 == wanted)
                        .collect();
                    // Down from a row onto the buttons lands on the sheet's
                    // verb, which is what the rows lead to (the buttons
                    // stand at the right, under the second side); elsewhere
                    // the same side where the line has one.
                    let verb = on.iter().copied().find(|at| rows[*at] == Row::Apply);
                    let onto_buttons = !matches!(rows[self.at], Row::Cancel | Row::Apply)
                        && on
                            .iter()
                            .any(|at| matches!(rows[*at], Row::Cancel | Row::Apply));
                    verb.filter(|_| onto_buttons && action == Action::Down)
                        .or_else(|| on.iter().copied().find(|at| placed[*at].1 == side))
                        .or_else(|| on.first().copied())
                })
            }
            _ => None,
        };
        match found {
            Some(at) => {
                self.at = at;
                true
            }
            None => false,
        }
    }

    /// The operation the sheet would put in the plan now, applied to a copy
    /// of `plan`: what the map draws while the sheet is up. A path that
    /// cannot be used changes nothing yet.
    pub fn preview(&self, plan: &Plan) -> Plan {
        let mut plan = plan.clone();
        let size = self.shown_size();
        match &self.kind {
            Kind::New(new) => {
                let mut shown = new.clone();
                shown.size = size.unwrap_or(new.size);
                plan.create(&new.disk, made(&shown));
            }
            Kind::Resize(resize) => {
                let to = size.unwrap_or(resize.size);
                plan.resize(&resize.disk, &resize.part, resize.from, to);
            }
            Kind::Label(labelling) => {
                plan.label(
                    &labelling.disk,
                    &labelling.part,
                    &labelling.was,
                    &labelling.label,
                );
            }
            Kind::Other(other) => {
                let checked = arithmetic::check_path(&other.typed, &plan, &other.part);
                match (&other.back, checked) {
                    (Some(back), Ok(mount)) => {
                        let mut new = (**back).clone();
                        new.fs = mount.suited(new.fs);
                        new.mount = Some(mount);
                        plan.create(&new.disk, made(&new));
                    }
                    (Some(back), Err(_)) => plan.create(&back.disk, made(back)),
                    (None, Ok(mount)) => {
                        plan.mount(&other.disk, &other.part, Some(mount));
                    }
                    (None, Err(_)) => {}
                }
            }
        }
        plan
    }

    /// The partition the sheet is about, by its path in the plan.
    pub fn part(&self) -> String {
        match &self.kind {
            Kind::New(new) => planning::new_path(new.number),
            Kind::Resize(resize) => resize.part.clone(),
            Kind::Label(labelling) => labelling.part.clone(),
            Kind::Other(other) => other.part.clone(),
        }
    }
}

/// The least and the most a new partition can be — its filesystem's and
/// its mount point's least, all of its space — and what it must be at least
/// where the space is smaller than that. GRUB's is 1 MiB exactly.
fn new_range(new: &New) -> (u64, u64, Option<u64>) {
    let most = new.gap.1 / MIB * MIB;
    if new.fs == Fs::BiosGrub {
        let least = new.fs.least();
        return if most >= least {
            (least, least, None)
        } else {
            (most, most, Some(least))
        };
    }
    let least = new
        .fs
        .least()
        .max(new.mount.as_ref().map_or(0, Mount::least));
    if least > most {
        (most, most, Some(least))
    } else {
        (least, most, None)
    }
}

/// `new` as the plan makes it: at the start of its space or at its end.
fn made(new: &New) -> NewPart {
    let start = match new.position {
        Position::Start => new.gap.0,
        Position::End => (new.gap.0 + new.gap.1 - new.size) / MIB * MIB,
    };
    NewPart {
        number: new.number,
        start,
        size: new.size,
        fs: new.fs,
        mount: new.mount.clone(),
        label: new.label.clone(),
    }
}

// -------------------------------------------------------------------------
// Raising one.

/// The disk shown and the plan, for raising a sheet.
fn disk_path(state: &State) -> Option<String> {
    page_of::shown(state).map(|disk| disk.path.clone())
}

/// Raise `kind` over the page, the light on its first row it can stand on:
/// put down there as the sheet unfolds rather than flown across the page
/// from the tile or the menu it was raised from.
fn raise(state: &mut State, kind: Kind) {
    let mut sheet = Sheet::new(kind);
    if state.low_end() {
        sheet.settle();
    }
    state.views.partitions.sheet = Some(sheet);
    state.views.partitions.folding = None;
    state.light.settle();
    // Its tile is kept in view on a map that slides, wherever the pointer
    // rests.
    state.views.partitions.pointed_tile = false;
    state.note = None;
    state.sound(Sound::Press);
}

/// New partition in the run of unallocated space from `start`, `size`
/// long: all of it, at its start, ext4, at `/` where nothing is yet and the
/// space holds it, labelled nothing — GParted's own first answers.
pub fn open_new(state: &mut State, start: u64, size: u64) {
    let Some(disk) = disk_path(state) else {
        return;
    };
    let plan = &state.views.partitions.plan;
    let rooted = plan
        .operations()
        .iter()
        .any(|op| matches!(op, planning::Operation::Mount { at: Mount::Root, .. }))
        || plan.operations().iter().any(|op| {
            matches!(op, planning::Operation::Create { new, .. } if new.mount == Some(Mount::Root))
        });
    let mount = (!rooted && size >= Mount::Root.least()).then_some(Mount::Root);
    let table = page_of::planned(state).and_then(|planned| planned.disk.table);
    let filesystems = planning::new_filesystems(state.firmware(), table);
    let mut new = New {
        disk,
        number: plan.next_number(),
        gap: (start, size),
        size: size / MIB * MIB,
        position: Position::Start,
        fs: Fs::Ext4,
        mount,
        label: String::new(),
        filesystems,
    };
    new.size = new_range(&new).1;
    raise(state, Kind::New(new));
}

/// Resize the partition at `at` of the disk shown, as the plan leaves it.
pub fn open_resize(state: &mut State, at: usize) {
    let Some(disk) = disk_path(state) else {
        return;
    };
    let Some(planned) = page_of::planned(state) else {
        return;
    };
    let (Some(part), Some(marks)) = (planned.disk.partitions.get(at), planned.parts.get(at)) else {
        return;
    };
    let (measures, _) = page_of::measures_of(state);
    let shrinkable = planning::shrinkable(&measures, &marks.was);
    let Some(room) = planning::resize_room(&planned, at, shrinkable) else {
        return;
    };
    let kind = Kind::Resize(Resize {
        disk,
        part: part.path.clone(),
        start: part.start,
        from: marks.was.size,
        size: part.size,
        room,
        filesystem: map::filesystem(part).map(str::to_string),
    });
    raise(state, kind);
}

/// Label… on the partition at `at` of the disk shown, its filesystem as
/// the plan leaves it.
pub fn open_label(state: &mut State, at: usize) {
    let Some(disk) = disk_path(state) else {
        return;
    };
    let Some(planned) = page_of::planned(state) else {
        return;
    };
    let (Some(part), Some(marks)) = (planned.disk.partitions.get(at), planned.parts.get(at)) else {
        return;
    };
    let Some(filesystem) = map::filesystem(part) else {
        return;
    };
    // What it has without this sheet: the disk's own label, or none on a
    // filesystem the plan makes; a label the plan already sets is where
    // the field starts.
    let was = if marks.format.is_some() || marks.new.is_some() {
        String::new()
    } else {
        marks.was.label.trim().to_string()
    };
    let kind = Kind::Label(Labelling {
        disk,
        part: part.path.clone(),
        filesystem: filesystem.to_string(),
        was,
        label: part.label.trim().to_string(),
    });
    raise(state, kind);
}

/// Mount point · Other… for the partition at `part` of the disk shown: the
/// field starting with the path of its own it has, if it has one.
pub fn open_other(state: &mut State, disk: String, part: String) {
    let typed = match state.views.partitions.plan.mount_of(&part) {
        Some(Mount::Other(path)) => path,
        _ => String::new(),
    };
    raise(
        state,
        Kind::Other(Other {
            disk,
            part,
            typed,
            back: None,
        }),
    );
}

/// Close the sheet, changing nothing.
fn cancel(state: &mut State) {
    let Some(sheet) = state.views.partitions.sheet.as_ref() else {
        return;
    };
    // Other… asked from New partition goes back to it as it was.
    let back = match &sheet.kind {
        Kind::Other(Other {
            back: Some(back), ..
        }) => Some((**back).clone()),
        _ => None,
    };
    state.sound(Sound::Back);
    match back {
        Some(back) => {
            state.views.partitions.sheet = None;
            back_to_new(state, back);
        }
        None => close(state),
    }
}

/// Close the sheet that is up onto the page: it folds back into the tile it
/// was about over the page (`draw_folding`; at once in low-end), and the
/// light is put down on the page where it goes rather than flown across it
/// from the sheet's row.
fn close(state: &mut State) {
    let Some(sheet) = state.views.partitions.sheet.take() else {
        return;
    };
    if !state.low_end() {
        state.views.partitions.folding = Some(sheet);
    }
    state.light.settle();
}

/// Draw a sheet that has closed folding back into the tile `anchor` (the
/// one the light is on, as the page drew it), the page under it already
/// the page: a frame's worth further on, along the toolkit's menus' flight
/// back, until it has gone.
pub(super) fn draw_folding(state: &mut State, page: &mut Page, anchor: Option<[f32; 4]>) {
    let Some(mut sheet) = state.views.partitions.folding.take() else {
        return;
    };
    sheet.out = (sheet.out - state.clock.dt / motion::duration::MENU_FLIGHT).max(0.0);
    if sheet.out <= 0.0 || state.low_end() {
        return;
    }
    let Some(mut card) = sheet.card.clone() else {
        return;
    };
    if anchor.is_some() {
        card.anchor = anchor;
    }
    let travelled = motion::ease(sheet.out);
    draw_card(state, page, &sheet, &card, false, travelled, false);
    state.views.partitions.folding = Some(sheet);
}

/// New partition again, the light on its Mount point row.
fn back_to_new(state: &mut State, new: New) {
    let mut sheet = Sheet::new(Kind::New(new));
    sheet.settle();
    sheet.at = sheet
        .rows()
        .iter()
        .position(|row| *row == Row::Mount)
        .unwrap_or(0);
    state.views.partitions.sheet = Some(sheet);
    state.light.settle();
}

/// The sheet's verb: Add, Resize, Set. What cannot be done is refused in
/// sound, and its reason — already said on the sheet — said in bold.
fn apply(state: &mut State) {
    let Some(sheet) = state.views.partitions.sheet.clone() else {
        return;
    };
    if sheet.range().is_some_and(|(_, _, short)| short.is_some()) {
        return refuse(state);
    }
    match sheet.kind {
        Kind::New(new) => {
            let part = made(&new);
            let start = part.start;
            close(state);
            page_of::replan(state, |plan| plan.create(&new.disk, part));
            page_of::light_on_offset(state, start);
        }
        Kind::Resize(resize) => {
            close(state);
            page_of::replan(state, |plan| {
                plan.resize(&resize.disk, &resize.part, resize.from, resize.size);
            });
            page_of::light_on_offset(state, resize.start);
        }
        Kind::Label(labelling) => {
            close(state);
            page_of::replan(state, |plan| {
                plan.label(
                    &labelling.disk,
                    &labelling.part,
                    &labelling.was,
                    &labelling.label,
                );
            });
        }
        Kind::Other(other) => {
            let plan = &state.views.partitions.plan;
            let mount = match arithmetic::check_path(&other.typed, plan, &other.part) {
                Ok(mount) => mount,
                Err(_) => return refuse(state),
            };
            match other.back {
                Some(back) => {
                    state.views.partitions.sheet = None;
                    let mut new = *back;
                    new.fs = mount.suited(new.fs);
                    new.mount = Some(mount);
                    let (least, most, _) = new_range(&new);
                    new.size = new.size.clamp(least, most);
                    back_to_new(state, new);
                }
                None => {
                    close(state);
                    page_of::replan(state, |plan| {
                        plan.mount(&other.disk, &other.part, Some(mount));
                    });
                }
            }
        }
    }
    state.sound(Sound::Press);
}

fn refuse(state: &mut State) {
    if let Some(sheet) = state.views.partitions.sheet.as_mut() {
        sheet.refused = true;
    }
    state.sound(Sound::Error);
}

// -------------------------------------------------------------------------
// Menus raised from a sheet.

/// What a sheet's row raises, by its number: clear of the page's own menus.
pub const POSITION_MENU: u32 = 21;
pub const FILESYSTEM_MENU: u32 = 22;
pub const MOUNT_MENU: u32 = 23;

/// A filesystem as the File system row and its menu say it: its name, and
/// GRUB's "bios_grub (unformatted)".
fn fs_words(fs: Fs) -> String {
    match fs {
        Fs::BiosGrub => i18n::text("partitions-bios-grub").to_string(),
        fs => fs.name().to_string(),
    }
}

/// A position as its row and menu say it.
fn position_words(position: Position) -> &'static str {
    match position {
        Position::Start => i18n::text("partitions-position-start"),
        Position::End => i18n::text("partitions-position-end"),
    }
}

/// A mount point as its row says it: its path, or none.
fn mount_words(at: Option<&Mount>) -> String {
    at.map_or_else(
        || i18n::text("partitions-none").to_string(),
        |at| at.path().to_string(),
    )
}

/// Raise the menu of the row the light is on.
fn open_menu(state: &mut State) {
    let Some(sheet) = state.views.partitions.sheet.as_ref() else {
        return;
    };
    let Kind::New(new) = &sheet.kind else {
        return;
    };
    let (id, title, commands, marked) = match sheet.row() {
        Row::Position => (
            POSITION_MENU,
            i18n::text("partitions-position"),
            vec![
                position_words(Position::Start).to_string(),
                position_words(Position::End).to_string(),
            ],
            Some(usize::from(new.position == Position::End)),
        ),
        Row::Filesystem => (
            FILESYSTEM_MENU,
            i18n::text("partitions-filesystem"),
            new.filesystems.iter().map(|fs| fs_words(*fs)).collect(),
            new.filesystems.iter().position(|fs| *fs == new.fs),
        ),
        Row::Mount => {
            let (commands, marked) = mount_rows(&Mount::ALL, true, new.mount.as_ref());
            (
                MOUNT_MENU,
                i18n::text("partitions-mount-point"),
                commands,
                Some(marked),
            )
        }
        _ => return,
    };
    state.menu(Menu {
        id,
        title: Some(title.to_string()),
        commands,
        named: Vec::new(),
        shorter: Vec::new(),
        marked,
        on: None,
    });
    page_of::raised_from_sheet(state, id);
}

/// The rows of a Mount point menu — the mount points `offered` by name
/// (`/`, `/home`, `/efi`, `/boot`, swap, or fewer: `partitions::mount_offers`),
/// Other… (or the path of one's own in force) where `other`, none — and the
/// one marked.
pub fn mount_rows(offered: &[Mount], other: bool, now: Option<&Mount>) -> (Vec<String>, usize) {
    let mut commands: Vec<String> = offered.iter().map(|at| at.path().to_string()).collect();
    if other {
        commands.push(match now {
            Some(Mount::Other(path)) => path.clone(),
            _ => i18n::text("partitions-other").to_string(),
        });
    }
    commands.push(mount_words(None));
    let none = commands.len() - 1;
    let marked = match now {
        Some(Mount::Other(_)) if other => offered.len(),
        Some(now) => offered.iter().position(|at| at == now).unwrap_or(none),
        None => none,
    };
    (commands, marked)
}

/// The row of a menu a sheet raised.
pub fn chose(state: &mut State, id: u32, row: usize) {
    let Some(sheet) = state.views.partitions.sheet.as_mut() else {
        return;
    };
    let Kind::New(new) = &mut sheet.kind else {
        return;
    };
    sheet.refused = false;
    match id {
        POSITION_MENU => {
            new.position = if row == 1 {
                Position::End
            } else {
                Position::Start
            };
        }
        FILESYSTEM_MENU => {
            let Some(fs) = new.filesystems.get(row).copied() else {
                return;
            };
            new.fs = fs;
            match fs {
                Fs::BiosGrub => {
                    new.mount = None;
                    new.label.clear();
                }
                Fs::LinuxSwap => new.mount = Some(Mount::Swap),
                // A mount point the filesystem does not suit is let go of,
                // so the two agree (`/` is ext4 or Btrfs alone).
                fs if new
                    .mount
                    .as_ref()
                    .is_some_and(|at| !crate::model::layout::made_suits(at, fs)) =>
                {
                    new.mount = None
                }
                _ => {}
            }
            // A label the new filesystem has no room for is cut to it, and
            // written as it writes labels.
            new.label = arithmetic::label_with("", &new.label, Some(fs.name())).0;
        }
        MOUNT_MENU => match row {
            row if row < Mount::ALL.len() => {
                let mount = Mount::ALL[row].clone();
                new.fs = mount.suited(new.fs);
                new.mount = Some(mount);
            }
            row if row == Mount::ALL.len() => {
                // Other…: a sheet of its own, which comes back here.
                let back = Box::new(new.clone());
                let (disk, part) = (new.disk.clone(), planning::new_path(new.number));
                let typed = match &new.mount {
                    Some(Mount::Other(path)) => path.clone(),
                    _ => String::new(),
                };
                let mut other = Sheet::new(Kind::Other(Other {
                    disk,
                    part,
                    typed,
                    back: Some(back),
                }));
                other.settle();
                state.views.partitions.sheet = Some(other);
                return;
            }
            _ => new.mount = None,
        },
        _ => return,
    }
    let (least, most, _) = new_range(new);
    new.size = new.size.clamp(least, most);
    // The light stays on the row just changed, wherever it now stands.
    let row = match id {
        POSITION_MENU => Row::Position,
        FILESYSTEM_MENU => Row::Filesystem,
        _ => Row::Mount,
    };
    if let Some(at) = sheet.rows().iter().position(|one| *one == row) {
        sheet.at = at;
    }
}

// -------------------------------------------------------------------------
// Presses.

/// Whether a wheel turned over `spot` was turned over the size: its row,
/// its track, or its room on the map.
fn over_the_size(scroll: Option<Scroll>, rows: &[Row]) -> bool {
    let Some(scroll) = scroll else {
        return false;
    };
    let size = rows.iter().position(|row| *row == Row::Size);
    matches!(scroll.spot, Spot::Control(id)
        if id == spots::SHEET_TRACK
            || id == spots::SHEET_ROOM
            || size.is_some_and(|at| id == spots::SHEET + at as u32))
}

/// One press while a sheet is up.
pub fn act(state: &mut State, action: Action, scroll: Option<Scroll>) {
    let Some(sheet) = state.views.partitions.sheet.as_mut() else {
        return;
    };
    let rows = sheet.rows();
    // The wheel over the size steps it, held or not.
    if over_the_size(scroll, &rows) && matches!(action, Action::Up | Action::Down) {
        let steps = if action == Action::Up { 1 } else { -1 };
        if let Some(held) = sheet.held.as_mut() {
            held.typed = None;
        }
        if sheet.step(steps) {
            state.sound(Sound::Move);
        }
        return;
    }
    if sheet.held.is_some() {
        return act_held(state, action);
    }
    match action {
        Action::Back => cancel(state),
        Action::Up | Action::Down | Action::Left | Action::Right => {
            if sheet.walk(action) {
                state.sound(Sound::Move);
            }
        }
        Action::Accept => accept(state),
        Action::Previous | Action::Next | Action::Menu | Action::Submit => {}
    }
}

/// Accept on the row the light is on.
fn accept(state: &mut State) {
    let Some(sheet) = state.views.partitions.sheet.as_mut() else {
        return;
    };
    match sheet.row() {
        Row::Size => {
            if let Some(size) = sheet.size() {
                sheet.held = Some(Held {
                    was: size,
                    typed: None,
                });
                state.sound(Sound::Press);
            }
        }
        Row::Position | Row::Filesystem | Row::Mount => open_menu(state),
        Row::Label | Row::Path => {
            let field = if sheet.row() == Row::Label {
                FieldId::PartLabel
            } else {
                FieldId::MountPath
            };
            if state.pad {
                state.type_with_pad(field);
            } else if sheet.walk(Action::Down) {
                // A keyboard is typing already, and Enter in a field is
                // done with it: on to the next row, the verb last.
                state.sound(Sound::Move);
            }
        }
        Row::Cancel => cancel(state),
        Row::Apply => apply(state),
    }
}

/// One press while the size is held.
fn act_held(state: &mut State, action: Action) {
    let Some(sheet) = state.views.partitions.sheet.as_mut() else {
        return;
    };
    let typing = sheet.held.as_ref().is_some_and(|held| held.typed.is_some());
    match action {
        Action::Left | Action::Right | Action::Previous | Action::Next => {
            let steps = match action {
                Action::Left => -1,
                Action::Right => 1,
                Action::Previous => -10,
                _ => 10,
            };
            if let Some(held) = sheet.held.as_mut() {
                held.typed = None;
            }
            if sheet.step(steps) {
                state.sound(Sound::Move);
            }
        }
        Action::Accept | Action::Submit => {
            if typing && !keep_typed(sheet) {
                state.sound(Sound::Error);
                return;
            }
            sheet.held = None;
            state.sound(Sound::Press);
        }
        Action::Back => {
            if let Some(held) = sheet.held.take() {
                sheet.set_size(held.was);
            }
            state.sound(Sound::Back);
        }
        // Y types a number: on the board a pad types with, the installer's
        // or LineXinBar's. A keyboard types its digits straight in.
        Action::Menu if state.pad => {
            if let Some(held) = sheet.held.as_mut() {
                held.typed.get_or_insert_with(String::new);
            }
            state.type_with_pad(FieldId::PartSize);
        }
        Action::Up | Action::Down | Action::Menu => {}
    }
}

/// Keep the number typed for the size held: read in GiB, held to the room.
/// Answers whether there was a number to read.
fn keep_typed(sheet: &mut Sheet) -> bool {
    let Some(typed) = sheet.held.as_ref().and_then(|held| held.typed.clone()) else {
        return true;
    };
    match arithmetic::parse_gib(&typed) {
        Some(bytes) => {
            sheet.set_size(bytes);
            if let Some(held) = sheet.held.as_mut() {
                held.typed = None;
            }
            true
        }
        None => false,
    }
}

/// Something typed into a sheet's field: a label, held to what its
/// filesystem allows; a path; a number for the size held.
pub fn edited(state: &mut State, field: FieldId, edit: &field::Edit) {
    let Some(sheet) = state.views.partitions.sheet.as_mut() else {
        return;
    };
    sheet.refused = false;
    match field {
        FieldId::PartSize => {
            let Some(held) = sheet.held.as_mut() else {
                return;
            };
            let typed = held.typed.get_or_insert_with(String::new);
            match edit {
                field::Edit::Rubbed => {
                    typed.pop();
                }
                field::Edit::Wrote(text) => {
                    for letter in text
                        .chars()
                        .filter(|letter| arithmetic::typed_keeps(*letter))
                    {
                        if typed.chars().count() < arithmetic::TYPED_MOST {
                            typed.push(letter);
                        }
                    }
                }
            }
        }
        FieldId::PartLabel => {
            let (label, filesystem) = match &mut sheet.kind {
                Kind::New(new) => (&mut new.label, new.fs.name().to_string()),
                Kind::Label(labelling) => (&mut labelling.label, labelling.filesystem.clone()),
                _ => return,
            };
            match edit {
                field::Edit::Rubbed => {
                    label.pop();
                    sheet.label_refused = None;
                }
                field::Edit::Wrote(text) => {
                    let (written, refused) = arithmetic::label_with(label, text, Some(&filesystem));
                    *label = written;
                    sheet.label_refused = refused;
                    if refused.is_some() {
                        state.sound(Sound::Error);
                    }
                }
            }
        }
        FieldId::MountPath => {
            let Kind::Other(other) = &mut sheet.kind else {
                return;
            };
            field::apply(&mut other.typed, edit);
        }
        _ => {}
    }
}

/// The board was put away with Done on a sheet's field: a number typed is
/// kept, and the light moves on from a field to the verb.
pub fn typed(state: &mut State, field: FieldId) {
    let Some(sheet) = state.views.partitions.sheet.as_mut() else {
        return;
    };
    match field {
        FieldId::PartSize => {
            if keep_typed(sheet) {
                sheet.held = None;
            }
        }
        FieldId::PartLabel | FieldId::MountPath => {
            if let Some(at) = sheet.rows().iter().position(|row| *row == Row::Apply) {
                sheet.at = at;
            }
        }
        _ => {}
    }
}

/// What the legend says while a sheet is up, the row the light is on
/// deciding: Adjust a size, and once it is held Keep it, Type a number for
/// it with a pad, or Put it back; Choose a row's value; Type into a field
/// with a pad; and Cancel the sheet.
pub fn hints(state: &State) -> Vec<Hint> {
    let Some(sheet) = state.views.partitions.sheet.as_ref() else {
        return Vec::new();
    };
    if sheet.held.is_some() {
        let mut hints = vec![hint(i18n::text("hint-keep"), Pictured::Accept)];
        if state.pad {
            hints.push(hint(i18n::text("hint-type"), Pictured::Options));
        }
        hints.push(hint(i18n::text("hint-put-back"), Pictured::Back));
        return hints;
    }
    let first = match sheet.row() {
        Row::Size => i18n::text("hint-adjust"),
        Row::Label | Row::Path if state.pad => i18n::text("hint-type"),
        _ => i18n::text("hint-choose"),
    };
    vec![
        hint(first, Pictured::Accept),
        hint(i18n::text("hint-cancel"), Pictured::Back),
    ]
}

/// The pointer moved onto a control while a sheet is up: the light goes to
/// the row or button it is on, unless the size is held.
pub fn hover(state: &mut State, id: u32) {
    let Some(sheet) = state.views.partitions.sheet.as_mut() else {
        return;
    };
    if sheet.held.is_some() {
        return;
    }
    let rows = sheet.rows();
    let at = if id == spots::SHEET_TRACK || id == spots::SHEET_ROOM {
        rows.iter().position(|row| *row == Row::Size)
    } else if let Some(button) = super::button_of(id, 2) {
        rows.iter()
            .position(|row| *row == if button == 0 { Row::Cancel } else { Row::Apply })
    } else {
        id.checked_sub(spots::SHEET)
            .map(|at| at as usize)
            .filter(|at| *at < rows.len())
    };
    if let Some(at) = at.filter(|at| sheet.enabled(rows[*at])) {
        sheet.at = at;
    }
}

/// Where a pointer's place along the track or the map's room puts the size,
/// as the last frame drew them.
pub(super) fn size_at(state: &State, x: f32, spot: u32) -> Option<u64> {
    let view = &state.views.partitions;
    let sheet = view.sheet.as_ref()?;
    let drawn = &view.drawn;
    let (least, most, _) = sheet.range()?;
    if spot == spots::SHEET_TRACK {
        let track = drawn.track?;
        let share = ((x - track[0]) / track[2].max(1.0)).clamp(0.0, 1.0) as f64;
        return Some(((most as f64 * share) as u64).clamp(least, most));
    }
    let (rect, from, to) = drawn.room?;
    let share = ((x - rect[0]) / rect[2].max(1.0)).clamp(0.0, 1.0) as f64;
    let at = from + ((to - from) as f64 * share) as u64;
    let size = match &sheet.kind {
        Kind::New(new) if new.position == Position::End => {
            (new.gap.0 + new.gap.1).saturating_sub(at)
        }
        Kind::New(new) => at.saturating_sub(new.gap.0),
        Kind::Resize(resize) => at.saturating_sub(resize.start),
        _ => return None,
    };
    Some(size.clamp(least, most))
}

/// The pointer's presses while a sheet is up: a hand on the track or on
/// the map's room drags the size; a click on a row acts on it as Accept
/// would; on Cancel or the verb, those.
pub fn pressed(state: &mut State, page: &mut Page) {
    for bar in [spots::SHEET_TRACK, spots::SHEET_ROOM] {
        let grabbed = page.pressed(bar);
        if let Some([x, _]) = page.dragging(bar) {
            let size = size_at(state, x, bar);
            if let (Some(size), Some(sheet)) = (size, state.views.partitions.sheet.as_mut()) {
                if let Some(at) = sheet.rows().iter().position(|row| *row == Row::Size) {
                    sheet.at = at;
                }
                if let Some(held) = sheet.held.as_mut() {
                    held.typed = None;
                }
                if sheet.set_size(size) {
                    state.sound(Sound::Move);
                }
            }
            return;
        }
        if grabbed {
            return;
        }
    }
    let Some(sheet) = state.views.partitions.sheet.as_ref() else {
        return;
    };
    let rows = sheet.rows();
    let mut clicked = None;
    for at in 0..rows.len() {
        if page.pressed(spots::SHEET + at as u32) && clicked.is_none() {
            clicked = Some(at);
        }
    }
    let buttons = sheet_buttons(sheet);
    let button = button::pressed(page, &buttons);
    let clicked = clicked.or_else(|| {
        button.and_then(|button| {
            let wanted = if button == 0 { Row::Cancel } else { Row::Apply };
            rows.iter().position(|row| *row == wanted)
        })
    });
    let Some(at) = clicked else {
        return;
    };
    if !sheet.enabled(rows[at]) {
        return;
    }
    let held = sheet.held.is_some();
    if let Some(sheet) = state.views.partitions.sheet.as_mut() {
        sheet.at = at;
    }
    match rows[at] {
        // A click on the size's row takes hold of it, or keeps it.
        Row::Size => accept_or_keep(state, held),
        Row::Label | Row::Path => {
            let field = if rows[at] == Row::Label {
                FieldId::PartLabel
            } else {
                FieldId::MountPath
            };
            state.clicked_field(field);
        }
        _ if held => {}
        _ => accept(state),
    }
    page_of::anchor_sheet(state);
}

fn accept_or_keep(state: &mut State, held: bool) {
    if held {
        act_held(state, Action::Accept);
    } else {
        accept(state);
    }
}

/// The sheet's buttons: Cancel, and its verb.
pub fn sheet_buttons(sheet: &Sheet) -> [Button<'static>; 2] {
    let verb = match sheet.kind {
        Kind::New(_) => "partitions-add",
        Kind::Resize(_) => "partitions-resize-button",
        Kind::Label(_) | Kind::Other(_) => "partitions-set",
    };
    [
        Button::plain(i18n::text("partitions-cancel")),
        Button::primary(i18n::text(verb)),
    ]
}

// -------------------------------------------------------------------------
// Words.

/// The sheet's title: New partition, Resize /dev/sda3, Label of /dev/sda5,
/// Mount point of New Partition #1.
fn title(sheet: &Sheet) -> String {
    match &sheet.kind {
        Kind::New(_) => i18n::text("partitions-new-title").to_string(),
        Kind::Resize(resize) => crate::message!(
            "partitions-resize-title",
            "partition" => planning::part_name(&resize.part)
        ),
        Kind::Label(labelling) => crate::message!(
            "partitions-label-title",
            "partition" => planning::part_name(&labelling.part)
        ),
        Kind::Other(other) => crate::message!(
            "partitions-other-title",
            "partition" => planning::part_name(&other.part)
        ),
    }
}

/// What a size sheet says over its rows at the right: the least and the
/// most the size can be.
fn range_words(sheet: &Sheet) -> Option<String> {
    let (least, most, _) = sheet.range()?;
    (least < most).then(|| format!("{} – {}", map::size(least), map::size(most)))
}

/// One line said under a sheet's rows: its words, and whether it is a
/// refusal (white, with the warning mark) or a note.
struct Said {
    words: String,
    refusal: bool,
}

/// The lines under a sheet's rows: on Resize what is left of the space
/// after the partition or what of it it takes, and why it can only grow;
/// where the space is smaller than the partition must be, the least it
/// must be; a path refused and why; a label's limit.
fn notes(state: &State, sheet: &Sheet) -> Vec<Said> {
    let mut said = Vec::new();
    if let Some((_, _, Some(least))) = sheet.range() {
        said.push(Said {
            words: crate::message!("partitions-at-least", "size" => map::size(least)),
            refusal: true,
        });
    }
    match &sheet.kind {
        Kind::Resize(resize) => {
            let left = resize.room.most.saturating_sub(resize.size);
            let words = if resize.size > resize.from && left == 0 {
                i18n::text("partitions-takes-all").to_string()
            } else if resize.size > resize.from {
                crate::message!(
                    "partitions-takes",
                    "size" => map::size(resize.size - resize.from)
                )
            } else if left == 0 {
                i18n::text("partitions-leaves-none").to_string()
            } else {
                crate::message!("partitions-leaves", "size" => map::size(left))
            };
            said.push(Said {
                words,
                refusal: false,
            });
        }
        Kind::Other(other) => {
            let checked =
                arithmetic::check_path(&other.typed, &state.views.partitions.plan, &other.part);
            if let (Err(why), false) = (checked, other.typed.trim().is_empty()) {
                said.push(Said {
                    words: why.words(),
                    refusal: true,
                });
            } else if other.typed.trim().is_empty() && sheet.refused {
                said.push(Said {
                    words: PathRefused::NotWhole.words(),
                    refusal: true,
                });
            }
        }
        // A label's limit is said under it, in a refusal's white once a
        // letter past it was refused — or in its place, while a "/" typed
        // is refused, that no label holds one.
        Kind::Label(labelling) => {
            let filesystem = &labelling.filesystem;
            let refused = sheet.label_refused.and_then(|why| why.words(filesystem));
            let limit =
                arithmetic::label_limit(Some(filesystem)).map(|limit| limit.words(filesystem));
            let refusal = refused.is_some();
            if let Some(words) = refused.or(limit) {
                said.push(Said { words, refusal });
            }
        }
        Kind::New(new) => {
            if let Some(words) = sheet.label_refused.and_then(|why| why.words(new.fs.name())) {
                said.push(Said {
                    words,
                    refusal: true,
                });
            }
        }
    }
    said
}

/// The lines under a sheet's rows as [`notes`] says them: each one's words,
/// and whether it is said as a refusal.
#[cfg(test)]
pub(super) fn said_under(state: &State, sheet: &Sheet) -> Vec<(String, bool)> {
    notes(state, sheet)
        .into_iter()
        .map(|said| (said.words, said.refusal))
        .collect()
}

/// The words of a row: what it sets, and its value.
fn row_words(sheet: &Sheet, row: Row) -> (&'static str, String) {
    match (&sheet.kind, row) {
        (Kind::Resize(_), Row::Size) => (i18n::text("partitions-new-size"), String::new()),
        (_, Row::Size) => (i18n::text("partitions-size"), String::new()),
        (Kind::New(new), Row::Position) => (
            i18n::text("partitions-position"),
            position_words(new.position).to_string(),
        ),
        (Kind::New(new), Row::Filesystem) => {
            (i18n::text("partitions-filesystem"), fs_words(new.fs))
        }
        (Kind::New(new), Row::Mount) => (
            i18n::text("partitions-mount-point"),
            mount_words(new.mount.as_ref()),
        ),
        (Kind::New(new), Row::Label) => (i18n::text("partitions-label"), new.label.clone()),
        (Kind::Label(labelling), Row::Label) => {
            (i18n::text("partitions-label"), labelling.label.clone())
        }
        (Kind::Other(other), Row::Path) => {
            (i18n::text("partitions-mount-point"), other.typed.clone())
        }
        _ => ("", String::new()),
    }
}

// -------------------------------------------------------------------------
// Drawing.

/// How tall a sheet's row is, in reference pixels: a field's heading over a
/// value, with air round them, as the installer's fields are — a little
/// shorter than a column's row, so New partition holds its five under the
/// map on a landscape panel.
const ROW: f32 = 66.0;

/// How thick a size's track is, and how large its handle against it.
const TRACK: f32 = 7.0;
const HANDLE: f32 = 2.4;

/// How a row stands while a sheet is drawn.
struct Laid {
    row: Row,
    rect: [f32; 4],
}

/// The colour of the filesystem a size sheet sizes: what its track fills
/// with, as its tile's band is.
fn fs_colour(sheet: &Sheet) -> Srgb {
    match &sheet.kind {
        Kind::New(new) => map::colour(match new.fs {
            Fs::BiosGrub => None,
            fs => Some(fs.name()),
        }),
        Kind::Resize(resize) => map::colour(resize.filesystem.as_deref()),
        _ => map::colour(None),
    }
}

/// Lay a sheet's rows out in `inner` (the card less its padding and its
/// title), `between` apart: two by two under the size where `wide`, one to
/// a line otherwise. Answers each row's place and how tall they came out.
fn lay_rows(rows: &[Row], inner: [f32; 4], tall: f32, between: f32, wide: bool) -> Vec<Laid> {
    let placed = places(rows, wide);
    let half = (inner[2] - between) * 0.5;
    rows.iter()
        .zip(placed)
        .filter(|(row, _)| !matches!(row, Row::Cancel | Row::Apply))
        .map(|(row, (line, side))| {
            let y = inner[1] + line as f32 * (tall + between);
            let pair = wide && !matches!(row, Row::Size);
            let rect = if pair {
                [inner[0] + side as f32 * (half + between), y, half, tall]
            } else {
                [inner[0], y, inner[2], tall]
            };
            Laid { row: *row, rect }
        })
        .collect()
}

/// How many lines a sheet's rows take: two by two where `wide`.
fn lines_of(rows: &[Row], wide: bool) -> usize {
    places(rows, wide)
        .iter()
        .zip(rows)
        .filter(|(_, row)| !matches!(row, Row::Cancel | Row::Apply))
        .map(|(place, _)| place.0 + 1)
        .max()
        .unwrap_or(0)
}

/// Draw the page with a sheet over it: the chips and the map along the top
/// as the page draws them, the map drawing the plan with the sheet's values
/// in it — the tile the sheet is about lit as the details' tile is, a grip
/// on the edge a size moves — and under it the sheet: a darker card with
/// its title, its rows and what it has to say under them, its buttons where
/// Options and Continue stand. Where the room is short (a board risen) the
/// map gives up its height first, then the chips, then the rows furthest
/// from the light.
pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let Room {
        content,
        buttons: band,
        ..
    } = room;
    let gap = page.metric(Metric::Gap);
    let pressing = state.pressing.through();
    let dt = state.clock.dt;
    let low_end = state.low_end();
    let Some(mut sheet) = state.views.partitions.sheet.take() else {
        return;
    };
    if low_end {
        sheet.settle();
    }

    // The card's measures.
    let mut pad = page.metric(Metric::RowPadding) * 0.6;
    let mut tall = page.scaled(ROW);
    let between = gap * 0.5;
    // Its title, beside the size's range where both fit on its line, else
    // alone, wrapped onto two lines at most ("Point de montage de Nouvelle
    // partition #2" on a display standing on its side).
    let title_words = title(&sheet);
    let (title_lines, range) = {
        let range = range_words(&sheet);
        let room = content[2] - pad * 2.0;
        let ui = page.ui();
        let range_w = range
            .as_deref()
            .map_or(0.0, |range| ui.measure(Text::Caption, range) + gap);
        if text::bold_width(ui, Text::Body, &title_words) + range_w <= room {
            (vec![title_words.clone()], range)
        } else {
            let lines = text::wrapped_by(&title_words, room, 2, |line| {
                text::bold_width(ui, Text::Body, line)
            });
            (lines, None)
        }
    };
    let title_more = page.line(Text::Body) * title_lines.len().saturating_sub(1) as f32;
    let mut title_h = page.line(Text::Body) * 1.25 + title_more;
    let note_line = page.line(Text::Caption);
    // What it says under its rows, each wrapped onto two lines at most
    // beside the warning mark a refusal wears: on a display standing on its
    // side "Its used space can't be read…" is wider than the card.
    let said: Vec<(Vec<String>, bool)> = {
        let mark = note_line * 0.8 + gap * 0.4;
        let room = content[2] - pad * 2.0 - mark;
        let ui = page.ui();
        notes(state, &sheet)
            .into_iter()
            .map(|one| {
                let lines = text::wrapped_by(&one.words, room, 2, |line| {
                    if one.refusal {
                        text::bold_width(ui, Text::Caption, line)
                    } else {
                        ui.measure(Text::Caption, line)
                    }
                });
                (lines, one.refusal)
            })
            .collect()
    };
    let said_lines: usize = said.iter().map(|(lines, _)| lines.len()).sum();
    let notes_h = if said.is_empty() {
        0.0
    } else {
        between + note_line * said_lines as f32
    };
    sheet.wide = matches!(sheet.kind, Kind::New(_)) && content[2] >= page.scaled(900.0);
    let rows = sheet.rows();
    let all_lines = lines_of(&rows, sheet.wide);
    let card_for = |lines: usize, tall: f32, pad: f32, title_h: f32| {
        pad * 2.0
            + title_h
            + lines as f32 * tall
            + lines.saturating_sub(1) as f32 * between
            + notes_h
    };

    // The chips, the map and the card share the room: the map at its own
    // height where it fits, else as short as its name; then without the
    // chips (the head names the disk); then with its rows closer together,
    // down to a field's own height; then with fewer of them, those round the
    // light (a display standing on its side with a board risen).
    let counts: Vec<usize> = state
        .disks
        .iter()
        .map(|disk| state.views.partitions.plan.on_disk(&disk.path))
        .collect();
    let (chips, chips_tall) = page_of::lay_chips(page, &state.disks, &counts, content);
    let preview = sheet.preview(&state.views.partitions.plan);
    let firmware = state.firmware();
    let planned = page_of::shown(state).map(|disk| planning::apply(&preview, disk, firmware));
    let mounted = planned
        .as_ref()
        .is_some_and(|planned| planned.parts.iter().any(|marks| marks.mount.is_some()));
    let mut air = gap * 1.1;
    let least = page_of::map_least(page);
    let mut chips_shown = true;
    let mut lines = all_lines;
    let left = |chips: f32, lines: usize, tall: f32, air: f32, pad: f32, title_h: f32| {
        content[3] - chips - air * 2.0 - card_for(lines, tall, pad, title_h)
    };
    if left(chips_tall + air, lines, tall, air, pad, title_h) < least {
        // Short of room: no chips, and everything closer.
        chips_shown = false;
        air = gap * 0.6;
        pad *= 0.7;
        title_h = page.line(Text::Body) * 1.1 + title_more;
    }
    let left =
        |chips: f32, lines: usize, tall: f32, air: f32| left(chips, lines, tall, air, pad, title_h);
    let card_for = |lines: usize, tall: f32| card_for(lines, tall, pad, title_h);
    let chips_room = if chips_shown { chips_tall + air } else { 0.0 };
    let shortest = page.metric(Metric::RowHeight) * 0.74;
    let over = least - left(chips_room, lines, tall, air);
    if over > 0.0 {
        tall = (tall - over / lines.max(1) as f32).max(shortest);
    }
    while lines > 1 && left(chips_room, lines, tall, air) < least {
        lines -= 1;
    }
    let map_tall = left(chips_room, lines, tall, air)
        .min(page_of::map_height(page, mounted))
        .max(least);
    let card_h = card_for(lines, tall);
    // The sheet stands at the foot of the room, over the buttons it ends in;
    // what is left between it and the map says the details of the tile it
    // is about, where they fit — on Resize, how much of it is used.
    let card_top = (content[1] + content[3] - card_h).max(content[1] + chips_room + map_tall + air);

    if chips_shown {
        for chip in &chips {
            let shown = chip.index == state.views.partitions.disk && !chip.blocked;
            page_of::draw_chip(page, chip, shown, false, None);
        }
    }

    // The map, the sheet's values in it.
    let map_rect = [content[0], content[1] + chips_room, content[2], map_tall];
    let part = sheet.part();
    let held = sheet.held.is_some();
    // A size in hand takes a number typed for it: a keyboard's digits go
    // straight in, and a board a pad summons (Y) types into it — the field
    // the light stands in while it is held, which is no field drawn.
    if held && lit {
        state.in_field(FieldId::PartSize);
    }
    let mut room_span = None;
    let mut anchor = None;
    if let Some(planned) = &planned {
        let ours = planned.index_of(&part);
        // The tile it is about kept in view where the map slides.
        let keep = ours.and_then(|at| {
            map::stops(&map::tiles(&planned.disk))
                .iter()
                .position(|stop| *stop == Stop::Part(at))
        });
        // A size moving — held, dragged, stepped by the wheel — is drawn at
        // once; anything else the sheet changes eases there.
        let at_once = held || std::mem::take(&mut sheet.sized);
        let mapped = page_of::lay_map(state, page, planned, map_rect, keep, &preview, at_once);
        let from = page.ui().written();
        let mut span: Option<[f32; 4]> = None;
        let byte_span = room_bytes(&sheet, planned);
        for one in &mapped.laid {
            let words = page_of::tile_words(planned, one.stop);
            let here = matches!(one.stop, Stop::Part(at) if Some(at) == ours);
            page_of::draw_tile(page, one, &words, (false, here), None);
            if let Some((from, to)) = byte_span {
                let (start, end) = stop_bytes(planned, one.stop);
                if start >= from && end <= to && end > start {
                    span = Some(match span {
                        Some(was) => union(was, one.rect),
                        None => one.rect,
                    });
                }
            }
            if here {
                anchor = page_of::inside_cut(one.rect, mapped.cut);
                let radius = page
                    .metric(Metric::CardRadius)
                    .min(one.rect[2] * 0.5)
                    .min(one.rect[3] * 0.5);
                if held {
                    // The tile in hand wears the held row's ring.
                    ring(page.ui(), one.rect, radius);
                }
                if let Some(edge) = grip(&sheet, one.rect, held, page) {
                    let ui = page.ui();
                    ui.chip(edge, ui.tinted(Role::Text, if held { 1.0 } else { 0.85 }));
                }
            }
        }
        if let Some(cut) = mapped.cut {
            let to = page.ui().written();
            page.ui()
                .cut_between(from, to, [cut[0], cut[1] - gap, cut[2], cut[3] + gap * 2.0]);
        }
        if let (Some(rect), Some((from, to))) = (span, byte_span) {
            if sheet.enabled(Row::Size) {
                if let Some(seen) = page_of::inside_cut(rect, mapped.cut) {
                    page.ui().spot(spots::SHEET_ROOM, seen);
                }
                let _ = page.dragging(spots::SHEET_ROOM);
                room_span = Some((rect, from, to));
            }
        }
    }

    // The details of the sheet's tile, as the plan with the sheet's values
    // in it leaves it, between the map and the card: all of them where they
    // fit; where they do not (a display standing on its side), as many of
    // the lines the sheet is about as fit (`telling`), standing in the
    // middle of the room — so that room is never an empty band between the
    // map and the card.
    if let Some(planned) = &planned {
        if let Some(at) = planned.index_of(&part) {
            let columns = page_of::details(state, planned, Stop::Part(at));
            let lines = page_of::details_lines(page, content[2], &columns);
            let line = page.line(Text::Body);
            let top = map_rect[1] + map_rect[3] + air;
            let room = card_top - air - top;
            if lines as f32 * line <= room {
                page_of::draw_details(page, [content[0], top, content[2], room], &columns);
            } else {
                // A value said on more than one line (what is pending) is
                // left out rather than cut.
                let fits = (room / line).floor().max(0.0) as usize;
                let mut take = fits;
                let shown = loop {
                    let shown = telling(&sheet, &columns, take);
                    if take == 0 || page_of::details_lines(page, content[2], &shown) <= fits {
                        break shown;
                    }
                    take -= 1;
                };
                let lines = page_of::details_lines(page, content[2], &shown);
                if lines > 0 {
                    let tall = lines as f32 * line;
                    let y = top + (room - tall) * 0.5;
                    page_of::draw_details(page, [content[0], y, content[2], tall], &shown);
                }
            }
        }
    }

    // The card, growing out of the tile it is about.
    let laid_card = Card {
        rect: [content[0], card_top, content[2], card_h],
        pad,
        tall,
        between,
        title_lines,
        title_h,
        title_more,
        range,
        said,
        lines,
        anchor,
    };
    let travelled = sheet.unfolding(dt);
    let (mut drawn_rows, track) = draw_card(state, page, &sheet, &laid_card, lit, travelled, true);
    sheet.card = Some(laid_card);

    // Cancel and the verb, where Options and Continue stand.
    let buttons = sheet_buttons(&sheet);
    let places = button::places(page, band, &buttons);
    let lit_button = match sheet.row() {
        Row::Cancel if lit => Some(0),
        Row::Apply if lit => Some(1),
        _ => None,
    };
    let laid_buttons = button::row(
        page,
        &places,
        &buttons,
        lit_button,
        &mut state.light,
        pressing,
        dt,
    );
    state.laid_buttons(laid_buttons);
    if let Some(at) = rows.iter().position(|row| *row == Row::Cancel) {
        drawn_rows[at] = Some(places[0]);
    }
    if let Some(at) = rows.iter().position(|row| *row == Row::Apply) {
        drawn_rows[at] = Some(places[1]);
    }
    let view = &mut state.views.partitions;
    view.drawn.sheet_rows = drawn_rows;
    view.drawn.track = track;
    view.drawn.room = room_span;
    view.drawn.options = Some(places[0]);
    view.sheet = Some(sheet);
    page_of::anchor_sheet(state);
}

/// A sheet's card as a frame laid it out: everything drawing it needs —
/// kept on the sheet, so that a sheet closing is drawn folding away as it
/// was last drawn.
#[derive(Debug, Clone)]
pub(super) struct Card {
    rect: [f32; 4],
    pad: f32,
    tall: f32,
    between: f32,
    title_lines: Vec<String>,
    title_h: f32,
    title_more: f32,
    range: Option<String>,
    said: Vec<(Vec<String>, bool)>,
    /// How many lines of rows it shows.
    lines: usize,
    /// What it grows out of and folds back into: the tile it is about, as
    /// the map drew it on that frame.
    anchor: Option<[f32; 4]>,
}

/// Draw `sheet`'s card as `laid_card` lays it — its glass, its title and
/// range, its rows (the light on the one it is on where `lit`), what it says
/// under them — `travelled` of the way unfolded (`opening`) or folded away.
/// Answers where its rows and its size's track were drawn.
fn draw_card(
    state: &mut State,
    page: &mut Page,
    sheet: &Sheet,
    laid_card: &Card,
    lit: bool,
    travelled: f32,
    opening: bool,
) -> (Vec<Option<[f32; 4]>>, Option<[f32; 4]>) {
    let Card {
        rect: card,
        pad,
        tall,
        between,
        ref title_lines,
        title_h,
        title_more,
        ref range,
        ref said,
        lines,
        anchor,
    } = *laid_card;
    let gap = page.metric(Metric::Gap);
    let note_line = page.line(Text::Caption);
    let pressing = state.pressing.through();
    let dt = state.clock.dt;
    let held = sheet.held.is_some();
    let rows = sheet.rows();
    let drawn_from = page.ui().drawn();
    page.ui().card(card, Surface::Control, Role::Glass, 0.55);
    let inner = [
        card[0] + pad,
        card[1] + pad,
        (card[2] - pad * 2.0).max(1.0),
        (card[3] - pad * 2.0).max(1.0),
    ];
    // Its title, and the size's range at the right.
    {
        let line = page.line(Text::Body);
        let first = title_h - title_more;
        let ui = page.ui();
        let range_w = range
            .as_deref()
            .map_or(0.0, |range| ui.measure(Text::Caption, range) + gap);
        let room = (inner[2] - range_w).max(1.0);
        let ink = ui.tinted(Role::Text, 1.0);
        for (index, words) in title_lines.iter().enumerate() {
            let cut = text::fit_bold(ui, Text::Body, words, room + 0.5);
            let (y, tall) = if index == 0 {
                (inner[1], first)
            } else {
                (
                    inner[1] + first + (index - 1) as f32 * line - (first - line) * 0.5,
                    line,
                )
            };
            ui.label_weighted(
                [inner[0], y, room + 0.5, tall],
                Text::Body,
                &cut,
                ink,
                Align::Left,
                true,
            );
        }
        if let Some(range) = &range {
            let (role, alpha) = text::LESSER;
            text::label_faded(
                ui,
                [inner[0], inner[1], inner[2], first],
                Text::Caption,
                range,
                role,
                alpha * 0.8,
                Align::Right,
            );
        }
    }

    // Its rows: those that fit, round the light.
    let shown_rows = window(&rows, sheet.at, sheet.wide, lines);
    let rows_room = [
        inner[0],
        inner[1] + title_h,
        inner[2],
        lines as f32 * tall + lines.saturating_sub(1) as f32 * between,
    ];
    let laid = lay_rows(&shown_rows, rows_room, tall, between, sheet.wide);
    // Rows left out above or under say so, as a list that goes on past its
    // end does: an arrow at the card's edge.
    {
        let fields = |list: &[Row]| -> Vec<Row> {
            list.iter()
                .copied()
                .filter(|row| !matches!(row, Row::Cancel | Row::Apply))
                .collect()
        };
        let (every, shown) = (fields(&rows), fields(&shown_rows));
        let size = note_line * 0.62;
        let icons = page.icons();
        let ui = page.ui();
        let middle = card[0] + (card[2] - size) * 0.5;
        if shown.first() != every.first() {
            let at = [middle, card[1] + (pad - size) * 0.5, size, size];
            ui.icon_tinted(at, "arrow-up", icons, Role::Text, 0.75);
        }
        if shown.last() != every.last() {
            let at = [
                middle,
                card[1] + card[3] - pad + (pad - size) * 0.5,
                size,
                size,
            ];
            ui.icon_tinted(at, "arrow-down", icons, Role::Text, 0.75);
        }
    }
    let mut drawn_rows = vec![None; rows.len()];
    let colour = fs_colour(sheet);
    let mut track = None;
    for one in &laid {
        let Some(at) = rows.iter().position(|row| *row == one.row) else {
            continue;
        };
        let here = lit && sheet.at == at;
        let enabled = sheet.enabled(one.row);
        drawn_rows[at] = Some(one.rect);
        match one.row {
            Row::Label | Row::Path => {
                let id = if one.row == Row::Label {
                    FieldId::PartLabel
                } else {
                    FieldId::MountPath
                };
                if here {
                    state.in_field(id);
                }
                let (heading, value) = row_words(sheet, one.row);
                let placeholder = if one.row == Row::Label {
                    i18n::text("partitions-none")
                } else {
                    i18n::text("partitions-path-placeholder")
                };
                let caret = state.typing_into() == Some(id);
                if enabled {
                    let now = state.clock.now;
                    let light = here.then_some(&mut state.light);
                    field::draw(
                        page,
                        one.rect,
                        id,
                        Look {
                            label: heading,
                            shown: &value,
                            placeholder,
                            refused: None,
                            caret,
                        },
                        light,
                        pressing,
                        dt,
                        now,
                    );
                } else {
                    draw_choice(page, one.rect, heading, "—", false, false, false);
                }
                page.ui().spot(spots::SHEET + at as u32, one.rect);
            }
            Row::Size => {
                if here {
                    let sunk = motion::pressed(one.rect, pressing);
                    state.light.lay(page, sunk, Shape::Capsule, pressing, dt);
                }
                let at_track = draw_size(page, one.rect, sheet, here, held, colour, pressing);
                if enabled {
                    page.ui().spot(spots::SHEET + at as u32, one.rect);
                    if let Some(at_track) = at_track {
                        // A hand takes hold of the track anywhere across the
                        // row's height.
                        let grab = [at_track[0], one.rect[1], at_track[2], one.rect[3]];
                        page.ui().spot(spots::SHEET_TRACK, grab);
                        let _ = page.dragging(spots::SHEET_TRACK);
                        track = Some(at_track);
                    }
                }
            }
            row => {
                if here {
                    let sunk = motion::pressed(one.rect, pressing);
                    state.light.lay(page, sunk, Shape::Capsule, pressing, dt);
                }
                let (heading, value) = row_words(sheet, row);
                let press = pressing.filter(|_| here);
                if let Some(bead) = (row == Row::Filesystem).then_some(colour) {
                    draw_choice_bead(page, one.rect, heading, &value, here, enabled, press, bead);
                } else {
                    draw_choice(page, one.rect, heading, &value, here, enabled, true);
                }
                if enabled {
                    page.ui().spot(spots::SHEET + at as u32, one.rect);
                }
            }
        }
    }

    // What it has to say under its rows.
    if !said.is_empty() {
        let mut y = rows_room[1] + rows_room[3] + between;
        let (mark, after) = (note_line * 0.8, gap * 0.4);
        let icons = page.icons();
        let ui = page.ui();
        for (lines, refusal) in said {
            let (role, alpha) = if *refusal {
                text::REFUSAL
            } else {
                text::LESSER
            };
            // A refusal's lines stand clear of its mark; a note's line up
            // with the rows.
            let x = if *refusal {
                ui.icon_tinted(
                    [inner[0], y + (note_line - mark) * 0.5, mark, mark],
                    glyphs::WARNING,
                    icons,
                    role,
                    alpha,
                );
                inner[0] + mark + after
            } else {
                inner[0]
            };
            let tint = ui.tinted(role, if *refusal { alpha } else { alpha * 0.85 });
            for line in lines {
                let cut = text::fit(ui, Text::Caption, line, inner[0] + inner[2] - x + 0.5);
                ui.label_weighted(
                    [x, y, inner[0] + inner[2] - x + 0.5, note_line],
                    Text::Caption,
                    &cut,
                    tint,
                    Align::Left,
                    *refusal && sheet.refused,
                );
                y += note_line;
            }
        }
    }
    // Unfolding or folding: grown out of the tile it is about, and folded
    // back into it, as the toolkit's menus grow out of what raised them —
    // the glass first and its words after it coming in, both going together
    // — or, with no tile to grow out of, risen from a little lower.
    if travelled < 1.0 {
        let shape = menu::CONTEXT;
        let glass = shape.shown(travelled, opening);
        // Folding, its words go before its glass, as a page's ink goes
        // first: read faintly over the page coming back under it, they were
        // two pages' words at once.
        let words = if opening {
            shape.content_shown(travelled, opening)
        } else {
            crate::ui::motion::departing(travelled)
        };
        let whole = [0.0, 0.0, page.width(), page.height()];
        let carry = match anchor {
            Some(anchor) => {
                let grown = menu::growing(anchor, card, travelled);
                Carry {
                    scale: grown[2] / card[2].max(1.0),
                    about: [card[0], card[1]],
                    by: [grown[0] - card[0], grown[1] - card[1]],
                    fade: glass,
                    words,
                    // A card folding away is pressed no more.
                    pressable: opening,
                    ..Carry::standing(whole)
                }
            }
            None => Carry {
                by: [0.0, (1.0 - travelled) * gap * 1.5],
                fade: glass,
                words,
                pressable: opening,
                ..Carry::standing(whole)
            },
        };
        let drawn_to = page.ui().drawn();
        page.ui().carry_between(drawn_from, drawn_to, carry);
    }
    (drawn_rows, track)
}

/// The lines of a sheet's tile's details most worth its room where not all
/// of them fit, `most` of them at most, in one column and in the details'
/// own order: what the sheet is about first — Resize the used space (what
/// it cannot go under) and what is pending, Label… the label and its
/// filesystem (which decides how long it may be), Mount point · Other… the
/// mount point and the filesystem, New partition what is pending — then
/// what is pending, the mount point and the size. The tile on the map says
/// its name, filesystem and size itself.
pub(super) fn telling(
    sheet: &Sheet,
    columns: &[Vec<page_of::Pair>; 2],
    most: usize,
) -> [Vec<page_of::Pair>; 2] {
    use page_of::Detail;
    let first: &[Detail] = match &sheet.kind {
        Kind::Resize(_) => &[Detail::Used, Detail::Pending],
        Kind::Label(_) => &[Detail::Label, Detail::Filesystem],
        Kind::Other(_) => &[Detail::MountPoint, Detail::Filesystem],
        Kind::New(_) => &[Detail::Pending],
    };
    let then = [Detail::Pending, Detail::MountPoint, Detail::Size];
    let every: Vec<page_of::Pair> = columns.iter().flatten().cloned().collect();
    let mut wanted: Vec<Detail> = Vec::new();
    for detail in first.iter().chain(&then) {
        let there = every.iter().any(|(name, _)| name == detail);
        if there && !wanted.contains(detail) && wanted.len() < most {
            wanted.push(*detail);
        }
    }
    let shown = every
        .into_iter()
        .filter(|(name, _)| wanted.contains(name))
        .collect();
    [shown, Vec::new()]
}

/// The rows of `rows` a card with room for `lines` lines shows: all where
/// they fit, else the lines round the one the light is on.
fn window(rows: &[Row], at: usize, wide: bool, lines: usize) -> Vec<Row> {
    let placed = places(rows, wide);
    let total = lines_of(rows, wide);
    if lines >= total {
        return rows.to_vec();
    }
    let lit_line = placed
        .get(at)
        .map(|place| place.0)
        .filter(|line| *line < total)
        .unwrap_or(total - 1);
    let first = lit_line.saturating_sub(lines - 1).min(total - lines);
    rows.iter()
        .zip(&placed)
        .filter(|(row, place)| {
            matches!(row, Row::Cancel | Row::Apply) || (place.0 >= first && place.0 < first + lines)
        })
        .map(|(row, _)| *row)
        .collect()
}

/// Where on the disk a stop of `planned` starts and ends, in bytes.
fn stop_bytes(planned: &Planned, stop: Stop) -> (u64, u64) {
    match stop {
        Stop::Part(at) | Stop::Extended(at) => {
            let part = &planned.disk.partitions[at];
            (part.start, part.start + part.size)
        }
        Stop::Free { start, size } => (start, start + size),
        Stop::Whole => (0, planned.disk.size),
    }
}

/// The bytes a size sheet's room spans on the disk: the new partition's
/// unallocated space, or the resized partition from its start to the most
/// it can reach.
fn room_bytes(sheet: &Sheet, _planned: &Planned) -> Option<(u64, u64)> {
    match &sheet.kind {
        Kind::New(new) => Some((new.gap.0, new.gap.0 + new.gap.1)),
        Kind::Resize(resize) => Some((resize.start, resize.start + resize.room.most)),
        _ => None,
    }
}

fn union(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x = a[0].min(b[0]);
    let y = a[1].min(b[1]);
    let right = (a[0] + a[2]).max(b[0] + b[2]);
    let bottom = (a[1] + a[3]).max(b[1] + b[3]);
    [x, y, right - x, bottom - y]
}

/// The grip on the edge of the tile a size sheet sizes, in `tile`: the end
/// the size moves — its right-hand end, or for a new partition at the end
/// of its space its left-hand one. None where the size is fixed.
fn grip(sheet: &Sheet, tile: [f32; 4], held: bool, page: &Page) -> Option<[f32; 4]> {
    if !sheet.enabled(Row::Size) {
        return None;
    }
    let wide = page.scaled(if held { 6.0 } else { 5.0 });
    let high = (tile[3] * if held { 0.46 } else { 0.36 })
        .max(page.scaled(18.0))
        .min(tile[3]);
    let inset = page.scaled(5.0);
    let x = match &sheet.kind {
        Kind::New(new) if new.position == Position::End => tile[0] + inset,
        Kind::New(_) | Kind::Resize(_) => tile[0] + tile[2] - inset - wide,
        _ => return None,
    };
    Some([
        x,
        tile[1] + (tile[3] - high) * 0.5 + page.scaled(5.0),
        wide,
        high,
    ])
}

/// The ring a held control wears: a firm line of the soft ink just outside
/// its glass, which nothing else on the page wears.
fn ring(ui: &mut lxb_app::lxb_render::Ui, rect: [f32; 4], radius: f32) {
    let step = ui.s(1.1);
    for one in 0..3 {
        let out = step * one as f32;
        ui.control_out(
            [
                rect[0] - out,
                rect[1] - out,
                rect[2] + out * 2.0,
                rect[3] + out * 2.0,
            ],
            radius + out,
            4.5,
        );
    }
}

/// Draw a choice's row: a capsule saying what it sets in a field's heading
/// over its value, and the chevron of a row that opens a list at its end.
#[allow(clippy::too_many_arguments)]
fn draw_choice(
    page: &mut Page,
    rect: [f32; 4],
    heading: &str,
    value: &str,
    lit: bool,
    enabled: bool,
    opens: bool,
) {
    draw_choice_with(page, rect, heading, value, lit, enabled, opens, None, None);
}

#[allow(clippy::too_many_arguments)]
fn draw_choice_bead(
    page: &mut Page,
    rect: [f32; 4],
    heading: &str,
    value: &str,
    lit: bool,
    enabled: bool,
    pressing: Option<f32>,
    bead: Srgb,
) {
    draw_choice_with(
        page,
        rect,
        heading,
        value,
        lit,
        enabled,
        true,
        Some(bead),
        pressing,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_choice_with(
    page: &mut Page,
    rect: [f32; 4],
    heading: &str,
    value: &str,
    lit: bool,
    enabled: bool,
    opens: bool,
    bead: Option<Srgb>,
    pressing: Option<f32>,
) {
    let pad = page.metric(Metric::RowPadding);
    let gap = page.metric(Metric::Gap);
    let heading_line = page.line(Text::Caption) * text::HEADING;
    let value_line = page.line(Text::Body);
    let chevron = page.line(Text::Body) * 0.62;
    let ui = page.ui();
    let radius = rect[3] * 0.5;
    let sunk = if enabled {
        let press = match pressing {
            Some(through) if lit => Press::Going(through),
            _ => Press::from(lit),
        };
        let sunk = ui.control(rect, radius, press, rect[2], 1.0);
        if !lit {
            ui.chip(sunk, ui.tinted(Role::Glass, button::RESTING_WASH));
        }
        sunk
    } else {
        ui.control_out(rect, radius, 1.0);
        rect
    };
    let [x, y, w, h] = sunk;
    let ink = if !enabled {
        0.55
    } else if lit {
        1.0
    } else {
        0.88
    };
    let mut right = x + w - pad;
    if opens && enabled {
        // The chevron of a row that opens a list, as Where to install's
        // own rows say it.
        let tint = ui.tinted(Role::Text, ink * 0.75);
        ui.label_weighted(
            [right - chevron, y, chevron, h],
            Text::Title,
            "›",
            tint,
            Align::Right,
            true,
        );
        right -= chevron + gap * 0.5;
    }
    let top = y + (h - heading_line - value_line) * 0.5;
    let (role, alpha) = text::LESSER;
    text::label_scaled(
        ui,
        [x + pad, top, (right - x - pad).max(0.0), heading_line],
        Text::Caption,
        heading,
        role,
        alpha * ink,
        Align::Left,
        text::HEADING,
    );
    let mut left = x + pad;
    if let Some(colour) = bead {
        let size = value_line * 0.36;
        ui.chip(
            [
                left,
                top + heading_line + (value_line - size) * 0.5,
                size,
                size,
            ],
            colour.a(0.95),
        );
        left += size + gap * 0.45;
    }
    text::label_faded(
        ui,
        [
            left,
            top + heading_line,
            (right - left).max(0.0),
            value_line,
        ],
        Text::Body,
        value,
        Role::Text,
        ink,
        Align::Left,
    );
}

/// Draw the size's row: what it sets over its value at the left and its
/// track filling the rest — the groove, the part filled in its
/// filesystem's colour, a tick where the least it may be stands, and the
/// handle. Held, the row turns to the accent, its value stands in a capsule
/// of the accent between two arrows (or says the number being typed, with
/// a caret), and the handle grows. Answers where the track came out, where
/// it can be taken hold of.
fn draw_size(
    page: &mut Page,
    rect: [f32; 4],
    sheet: &Sheet,
    lit: bool,
    held: bool,
    colour: Srgb,
    pressing: Option<f32>,
) -> Option<[f32; 4]> {
    let (least, most, short) = sheet.range()?;
    let size = sheet.shown_size()?;
    let enabled = least < most;
    let pad = page.metric(Metric::RowPadding);
    let gap = page.metric(Metric::Gap);
    let heading_line = page.line(Text::Caption) * text::HEADING;
    let value_line = page.line(Text::Body);
    let track_h = page.scaled(TRACK);
    let now = page.seconds();
    let typed = sheet
        .held
        .as_ref()
        .and_then(|held| held.typed.as_deref())
        .map(|typed| format!("{typed}\u{a0}GiB"));
    let value = typed.clone().unwrap_or_else(|| map::size(size));
    let (heading, _) = row_words(sheet, Row::Size);
    let ui = page.ui();
    // The value's column is as wide as the widest size the room can say,
    // so the track does not move as the value changes.
    let widest = text::bold_width(ui, Text::Body, &map::size(most))
        .max(text::bold_width(ui, Text::Body, "8888.8\u{a0}GiB"))
        .max(ui.measure(Text::Caption, heading) * text::HEADING);
    let arrows = value_line * 0.5;
    let column = widest + value_line * 0.8 + arrows * 2.0;
    let radius = rect[3] * 0.5;
    let press = match pressing {
        Some(through) if lit => Press::Going(through),
        _ => Press::from(lit),
    };
    let sunk = if enabled {
        let sunk = ui.control(rect, radius, press, rect[2], 1.0);
        if held {
            ring(ui, sunk, radius);
        } else if !lit {
            ui.chip(sunk, ui.tinted(Role::Glass, button::RESTING_WASH));
        }
        sunk
    } else {
        ui.control_out(rect, radius, 1.0);
        rect
    };
    let [x, y, w, h] = sunk;
    let ink = if !enabled {
        0.7
    } else if lit || held {
        1.0
    } else {
        0.88
    };
    let top = y + (h - heading_line - value_line) * 0.5;
    let (role, alpha) = text::LESSER;
    text::label_scaled(
        ui,
        [x + pad, top, column, heading_line],
        Text::Caption,
        heading,
        role,
        alpha * ink,
        Align::Left,
        text::HEADING,
    );
    let value_at = [x + pad, top + heading_line, column, value_line];
    if held {
        // The value in hand: white, the accent's deepest ink on it — the one
        // value on the page drawn the other way round — between the two
        // chevrons Left and Right move it by.
        let words = text::bold_width(ui, Text::Body, &value);
        let pill_w = words + value_line * 0.8;
        let pill = [
            value_at[0] + arrows * 1.1,
            value_at[1] + value_line * 0.04,
            pill_w,
            value_line * 0.92,
        ];
        let white = ui.tinted(Role::Text, 1.0);
        for (x, mark) in [
            (value_at[0] - arrows * 0.2, "‹"),
            (pill[0] + pill[2] + arrows * 0.2, "›"),
        ] {
            ui.label_weighted(
                [x, pill[1] - value_line * 0.06, arrows, pill[3]],
                Text::Title,
                mark,
                white,
                Align::Centre,
                true,
            );
        }
        ui.chip(pill, white);
        let deep = ui.tinted(Role::AccentDeep, 1.0);
        match sheet.held.as_ref().and_then(|held| held.typed.as_deref()) {
            // A number being typed: its digits, then its unit quieter, so
            // what is typed reads apart from what is not.
            Some(number) => {
                let digits = text::bold_width(ui, Text::Body, number);
                let left = pill[0] + (pill[2] - words) * 0.5;
                ui.label_weighted(
                    [left, pill[1], digits + 1.0, pill[3]],
                    Text::Body,
                    number,
                    deep,
                    Align::Left,
                    true,
                );
                let quiet = ui.tinted(Role::AccentDeep, 0.55);
                let unit = &value[number.len()..];
                ui.label_weighted(
                    [left + digits, pill[1], words - digits + 1.0, pill[3]],
                    Text::Body,
                    unit,
                    quiet,
                    Align::Left,
                    true,
                );
            }
            None => ui.label_weighted(pill, Text::Body, &value, deep, Align::Centre, true),
        }
        if typed.is_some() && now.fract() < 0.55 {
            // The caret after the number, before its unit.
            let number = sheet
                .held
                .as_ref()
                .and_then(|held| held.typed.as_deref())
                .unwrap_or("");
            let unit =
                text::bold_width(ui, Text::Body, &value) - text::bold_width(ui, Text::Body, number);
            let at = pill[0] + (pill[2] + words) * 0.5 - unit + ui.s(1.0);
            let thick = ui.s(2.0).max(1.0);
            ui.chip([at, pill[1] + pill[3] * 0.2, thick, pill[3] * 0.6], deep);
        }
    } else {
        let tint = if short.is_some() {
            ui.tinted(text::REFUSAL.0, text::REFUSAL.1)
        } else {
            ui.tinted(Role::Text, ink)
        };
        ui.label_weighted(value_at, Text::Body, &value, tint, Align::Left, lit);
    }
    // The track.
    let track_x = x + pad + column + gap;
    let track_w = (x + w - pad - track_x).max(1.0);
    let middle = y + h * 0.5;
    let track = [track_x, middle - track_h * 0.5, track_w, track_h];
    let groove = ui.tinted(Role::Text, 0.2);
    ui.chip(track, groove);
    let share = |bytes: u64| (bytes as f64 / most.max(1) as f64).clamp(0.0, 1.0) as f32;
    let filled = (track_w * share(size)).max(track_h);
    ui.chip(
        [track[0], track[1], filled, track_h],
        colour.a(if enabled { 0.95 } else { 0.5 }),
    );
    if enabled && least > 0 {
        // Where the least stands: under it the size cannot go.
        let tick = [
            track[0] + track_w * share(least) - ui.s(1.0),
            track[1] - track_h * 0.6,
            ui.s(2.0).max(1.0),
            track_h * 2.2,
        ];
        ui.chip(tick, ui.tinted(Role::Text, 0.55));
    }
    if enabled {
        let handle = track_h * HANDLE * if held { 1.3 } else { 1.0 };
        let at =
            (track[0] + filled).clamp(track[0] + handle * 0.5, track[0] + track_w - handle * 0.5);
        let knob = [at - handle * 0.5, middle - handle * 0.5, handle, handle];
        if held {
            let halo = handle * 0.36;
            ui.chip(
                [
                    knob[0] - halo,
                    knob[1] - halo,
                    knob[2] + halo * 2.0,
                    knob[3] + halo * 2.0,
                ],
                ui.tinted(Role::Text, 0.3),
            );
        }
        ui.chip(knob, ui.tinted(Role::Text, 1.0));
    }
    Some(track)
}
