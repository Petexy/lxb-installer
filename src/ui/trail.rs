//! Two columns walked the way LineXinBar's file picker walks a disk: Time
//! zone's parts of the world and the places in them, Keyboard's layouts and
//! their kinds.
//!
//! These two screens look like the shell's file picker: the second column is
//! not shown until something is pressed in the first. So this is the picker's
//! trail of columns — `ui::build_picker` in the shell, `Ui::file_picker` in
//! the toolkit — drawn with the toolkit's own pieces in the installer's room,
//! and nothing invented:
//!
//! * **a column of the bar's rows**: a mark, and its name beside it. The row
//!   the light is on stands on one line across the room (the picker's
//!   cross), its mark grown on the light's glass with the picker's breathing
//!   bloom behind it, its name large and bold and its note under it; every
//!   other row is a smaller mark and its name alone. The list slides under
//!   the line rather than the light walking down a still list, and dissolves
//!   before the room's ends — no chips, no scroll bar;
//! * **the second column is not drawn at all until a row of the first is
//!   pressed** (Accept, Right, a click on one of its rows), and then
//!   it opens beside it with the light on the row in force. The column at
//!   the front stands on the picker's cross, a line down the room, and the
//!   whole trail slides one step to the left as the second opens, so the
//!   light stays where it is and the rows pass under it. The first column
//!   steps back as the picker's trail does — smaller, hazier, keeping only
//!   the row it was opened from — so the two read as a path rather than as
//!   two lists side by side;
//! * **Back closes it** (or Left, or a click on the row it was opened from):
//!   the second column leaves and the first comes forward again, whole;
//! * **the pointer lights the row it moves onto** in the column at the
//!   front, as the picker's does, and a click presses the row it lands on at
//!   once, as Accept would (the installer's rule for every list);
//! * on a panel too narrow for both — a display standing on its side — the
//!   second column takes the room and the first keeps a strip at the left
//!   with its row's mark in it: the family's rule for every program on such
//!   a display, the Home menu's `layout::PEEK`.
//!
//! The rows are the size the toolkit's own file picker draws them at in a
//! window this tall: it sizes them by its panel, eight tenths of the window's
//! height, and so is the installer's panel ([`SHARE`]). Where to install
//! keeps its two columns side by side (`pair.rs`): a disk and the places on
//! it are read together, and the trail is Time zone's and Keyboard's
//! alone.

use lxb_app::lxb_render::Align;
use lxb_app::lxb_toolkit::{
    input::Action, layout, metrics::Metric, motion, palette::Role, typography::Text,
};
use lxb_app::Page;

use super::column::{self, Row};
use super::motion::{departing, halves, receded, Light, Shape, SHRINK};
use super::{mark, spots, text};

/// The bar's rows against the window's own scale: the toolkit's file picker
/// sizes them by its panel, which is eight tenths of the window's height —
/// as the installer's is (`panel::HEIGHT` at the 1080-pixel reference).
pub const SHARE: f32 = 0.8;

/// How far down the room the rows have the line the lit row stands on falls:
/// the picker's own (`PICKER_CROSS_Y`).
const CROSS: f32 = 0.38;

/// The glass under the lit row's mark, against the mark: the bar's disc.
const DISC: f32 = std::f32::consts::SQRT_2 * 1.04;

/// How far the second column's mark stands from the first's on a panel wide
/// enough for both, in reference pixels: room beside the first column's row,
/// stepped back, for the longest part of the world in any of the catalogs.
/// The toolkit picker's own step at its size comes to the same.
pub const STEP: f32 = 440.0;

/// The least room the second column's words are given beside its mark, in
/// reference pixels, for the two to stand side by side. A panel with less
/// beside the first column gives the second the room and the first a strip.
pub const LEAST: f32 = 330.0;

/// The share of its ink the name of a row the light is not on keeps. The
/// picker's is 0.62, on its own near-black panel; on the installer's paler
/// glass a name that faint is hard to read, which no lesser line may be.
const QUIET: f32 = 0.82;

/// How far the second column comes in from as it opens, and goes back out
/// to as it closes, on a narrow panel, as a share of the step. The picker's
/// new column arrives from a whole step to the right as the trail slides
/// over to it, and so it does on a landscape panel; on a narrow one the
/// first column hardly moves (it is already at the left), and the second
/// comes in over where the first's words were, after they have gone.
const ENTRY: f32 = 0.25;

/// How large the `chosen` mark beside a name is, in reference pixels: a
/// column's row's (`column::CHOSEN`).
const CHOSEN: f32 = 30.0;

/// The least room for a name worth drawing at all, in reference pixels.
const WORDS_LEAST: f32 = 48.0;

/// How near a column's position has to be to where it is going to be put
/// there, in rows (or, for the depth, in columns).
const SETTLED: f32 = 0.001;

/// The two columns' own state: where the light stands in each, whether the
/// second is open, and how far each has rolled and the trail has stepped.
/// The rows are the screen's, handed in every frame.
#[derive(Debug, Default, Clone)]
pub struct Trail {
    /// The row the light is on in each column (or would be, while the light
    /// is elsewhere; or was, in a column that is shut).
    pub at: [usize; 2],
    /// Whether the second column is open: a row of the first was pressed.
    pub open: bool,
    /// The row of the first column the second was last opened from.
    from: usize,
    /// Where each list stands under the line, in rows.
    rolled: [Rolling; 2],
    /// How far the trail has stepped towards the second column, from nought
    /// (the first column alone) to one (the second open).
    depth: Rolling,
    /// Where the disc under the lit row was asked to be on the last frame
    /// the light was here, and whether it stood between two columns' rows,
    /// so that the light rides the disc as the trail steps (`disc_moved`) —
    /// and glides where the rows it stands between changed (the second
    /// column gone at once, `second_drawn`).
    laid: Option<([f32; 4], bool)>,
    /// Whether the pointer put the light where it is, and the column at the
    /// front holds still under it rather than rolling the row to the line:
    /// a list moving under a resting pointer would have the next nudge light
    /// the row that rolled in under it, and walk the light down the list.
    /// Let go of by a press the trail answers (`Trail::act`, a click, a key
    /// bringing the light back, `Trail::let_go`).
    held: bool,
    /// Whether the light has been walked in the second column since it last
    /// opened ([`Trail::walked`]).
    walked: bool,
    /// Whether a frame has drawn the second column since it last opened
    /// ([`Trail::drawn`]): only a step from then on is a walk.
    shown: bool,
}

/// What a press did to the trail, for the screen to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    /// Nothing moved; nothing is heard. Past the foot of a column, or Right
    /// in the second: the screen may take the light on to what is under it.
    Nothing,
    /// The light moved within the column at the front. A move in the first
    /// changes what the second would list; it is shut, so nothing is drawn.
    Moved,
    /// A row of the first column was pressed and the second opened beside it.
    Opened,
    /// Back (or Left) from the second column: it closed.
    Closed,
    /// Accept on a row of the second column: the screen takes its value.
    Chose(usize),
    /// Not the trail's to answer: Back from the first column, Options…
    Pass,
}

/// What a click on the trail landed on (`Trail::pressed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// A row of the column at the front. The light has not been moved: the
    /// screen points at the row (`Trail::point_at`) and presses it as Accept
    /// would — the first column opens it, the second chooses it.
    Row(usize),
    /// The row the second column was opened from: the second column closed.
    Closed,
}

/// How the trail is drawn on this frame.
#[derive(Debug, Clone, Copy)]
pub struct Look<'a> {
    /// What each column is of, over it.
    pub headings: [&'a str; 2],
    /// The mark every row of each column wears.
    pub marks: [&'static str; 2],
    /// Which numbers each column's rows answer to.
    pub ids: [spots::Column; 2],
    /// How far through a press the page is, for the lit row.
    pub pressing: Option<f32>,
    /// How long this frame counts for.
    pub dt: f32,
}

/// The bar's sizes for a window at `scale` (`Page::scaled(1.0)`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizes {
    /// The lit row's mark, and every other row's.
    pub lit: f32,
    pub idle: f32,
    /// From one row's middle to the next one's.
    pub spacing: f32,
    /// The glass under the lit row's mark.
    pub disc: f32,
    /// From a mark's middle to where its words begin, in a column at the front.
    pub words: f32,
    /// What a column's words keep clear of the next column's disc.
    pub clear: f32,
}

/// The bar's sizes at `scale`.
pub fn sizes(scale: f32) -> Sizes {
    let unit = scale * SHARE;
    let lit = Metric::ItemIconFocused.value() * unit;
    Sizes {
        lit,
        idle: Metric::ItemIcon.value() * unit,
        spacing: Metric::ItemSpacing.value() * unit,
        disc: lit * DISC,
        words: lit * DISC * 0.5 + 12.0 * unit,
        clear: 10.0 * unit,
    }
}

/// Where the two columns' marks stand across a room, from its left edge,
/// once the trail has settled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stand {
    /// The first column's mark: alone, and with the second open.
    pub first: [f32; 2],
    /// The second column's mark.
    pub second: f32,
    /// How far to the right of where it stands the second column comes in
    /// from, and goes back out to.
    pub entry: f32,
    /// Whether the two do not fit side by side, and the first keeps only a
    /// strip at the left while the second is open.
    pub narrow: bool,
}

/// Where the columns stand in a room `width` wide at `scale`.
///
/// On a panel wide enough for both, this is the picker's trail
/// (`picker_column_x` in the shell): the column at the front stands on one
/// line down the room, its cross, whether it is the first alone or the
/// second; opening the second slides the whole trail one [`STEP`] to the
/// left, the first going back to stand against the room's left edge and
/// the second coming in from a step to the right, so the light stays on the
/// cross and the room right of it belongs to the names. The cross is a step
/// in from the left edge, where the picker's is a third of the way across:
/// the step is what the longest part of the world needs stepped back.
///
/// On a panel too narrow for that — a display standing on its side — the
/// first column alone takes the room from its left edge, and opening the
/// second slides the view as the Home menu's does: the second's disc stands
/// just past a strip of the room ([`layout::PEEK`] of it), and the first
/// steps back into the strip, its row's mark in the middle of it.
pub fn stand(width: f32, scale: f32) -> Stand {
    let sizes = sizes(scale);
    let home = sizes.disc * 0.5;
    let cross = home + STEP * scale;
    if width - (cross + sizes.words) >= LEAST * scale {
        return Stand {
            first: [cross, home],
            second: cross,
            entry: STEP * scale,
            narrow: false,
        };
    }
    let strip = width * layout::PEEK;
    Stand {
        first: [home, strip * 0.5],
        second: strip + home,
        entry: STEP * scale * ENTRY,
        narrow: true,
    }
}

/// How far down a room the columns' rows begin, under their headings: a
/// caption's `caption` line and a little air, the page's `gap` its measure.
/// What is said in a column's stead goes there too — a search that found
/// nothing — so it never stands over the heading.
pub fn heading_tall(caption: f32, gap: f32) -> f32 {
    caption + gap * 0.6
}

/// Where the first column's heading starts in a room `width` wide at
/// `scale`, from the room's left edge, with the column alone: the left-hand
/// edge of the glass of its mark — the cross's, a step in, on a panel wide
/// enough for two columns; the room's own edge on a narrow one. What
/// something standing over the trail lines up with (a search field).
pub fn first_edge(width: f32, scale: f32) -> f32 {
    (stand(width, scale).first[0] - sizes(scale).disc * 0.5).max(0.0)
}

/// How much room a row's words have in a room `width` wide at `scale`, once
/// the trail has settled: `level` nought for the first column, one for the
/// second, with the second `open` or not. Up to the room's edge for the
/// column at the front; up to the next column's disc for the one behind.
///
/// Measured where the trail lands, never where it has got to: a name given
/// the room of a column still stepping would have its room change under it
/// half way, the picker's own lesson (`picker_text_max`).
pub fn words_room(width: f32, scale: f32, level: usize, open: bool) -> f32 {
    let sizes = sizes(scale);
    let stand = stand(width, scale);
    let room = match (level, open) {
        (0, false) => width - (stand.first[0] + sizes.words),
        (0, true) => {
            stand.second - sizes.disc * 0.5 - sizes.clear - (stand.first[1] + sizes.words * SHRINK)
        }
        _ => width - (stand.second + sizes.words),
    };
    room.max(0.0)
}

/// How much room a column's heading has, settled: from the left edge of its
/// mark's glass to the next column's, or to the room's edge.
pub fn heading_room(width: f32, scale: f32, level: usize, open: bool) -> f32 {
    let sizes = sizes(scale);
    let stand = stand(width, scale);
    let room = match (level, open) {
        (0, false) => width - (stand.first[0] - sizes.disc * 0.5).max(0.0),
        (0, true) => {
            stand.second
                - sizes.disc * 0.5
                - sizes.clear
                - (stand.first[1] - sizes.disc * 0.5).max(0.0)
        }
        _ => width - (stand.second - sizes.disc * 0.5),
    };
    room.max(0.0)
}

/// Whether a column's heading stays where it is, whole, while the trail
/// steps: over the column at the front, and over one behind it that has
/// room for it (`fits`). Otherwise it goes and comes back with the first
/// column's words (`halves`) — and always over the strip of a `narrow`
/// panel, fit or not: a short one left standing there read as one phrase
/// with the second column's ("Układ Wariant"), and one back at full ink the
/// moment Back was pressed was drawn over the second's on its way out.
fn heading_stays(level: usize, front: usize, narrow: bool, fits: bool) -> bool {
    !(level == 0 && narrow) && (fits || level == front)
}

/// How much of a column whose mark stands at `x` is still inside the room
/// between `left` and `right`: it dissolves over the width of a lit mark as
/// it leaves.
fn inside(x: f32, left: f32, right: f32, band: f32) -> f32 {
    let band = band.max(1.0);
    ((x - left + band) / band).clamp(0.0, 1.0) * ((right - x + band) / band).clamp(0.0, 1.0)
}

/// How much of a row at height `y` shows between `top` and `bottom`: the
/// picker's fade before the panel's ends, so no mark is ever cut in half.
fn upright(y: f32, top: f32, bottom: f32, unit: f32) -> f32 {
    let range = 70.0 * unit;
    ((y - top - 24.0 * unit) / range).clamp(0.0, 1.0)
        * ((bottom - 24.0 * unit - y) / range).clamp(0.0, 1.0)
}

/// The rows of a list `rows` long worth drawing with the one at `position`
/// on the line, `spacing` apart, in a room `tall`.
fn in_view(position: f32, rows: usize, spacing: f32, tall: f32) -> std::ops::Range<usize> {
    if spacing <= 0.0 || !spacing.is_finite() || !position.is_finite() {
        return 0..rows;
    }
    let reach = tall / spacing + 3.0;
    let from = (position - reach).clamp(0.0, rows as f32) as usize;
    let to = ((position + reach).clamp(0.0, rows as f32) as usize + 1).min(rows);
    from..to.max(from)
}

/// A column as it stands on one frame of the step.
#[derive(Debug, Clone, Copy)]
struct Standing {
    /// The scale it keeps, stepped back or not.
    near: f32,
    /// The share of its ink left after the haze and the room's edges.
    clarity: f32,
    /// How much of it is here at all: the second comes in as the trail steps
    /// over to it and dissolves on its way back out.
    present: f32,
    /// How much it is the column in hand, from nought to one.
    active: f32,
    /// How far a column held back until its turn on a narrow panel has come
    /// in (one where nothing is held back).
    gate: f32,
}

/// How a column stands at `depth` of the step, the second `open` or not,
/// on a `narrow` panel or not — before the room's edges take their share of
/// its ink. The second column comes in as the trail steps over to it and
/// dissolves on its way back out; on a narrow panel it comes in only in the
/// second half of the step (`gate`), the whole of it at once, its lit row and
/// every other row with it.
fn standing(level: usize, depth: f32, open: bool, narrow: bool) -> Standing {
    let (near, haze) = receded(depth - level as f32);
    let gate = if level == 1 && narrow {
        halves(depth).1
    } else {
        1.0
    };
    let present = gate
        * match (level, open) {
            (0, _) => 1.0,
            (_, true) => depth,
            (_, false) => departing(depth),
        };
    Standing {
        near,
        clarity: haze,
        present,
        active: 1.0 - (depth - level as f32).abs().min(1.0),
        gate,
    }
}

/// How much of its ink a row of `column` keeps (`level`, with `front` the
/// column at the front): the row a column `stands` on keeps all of the
/// column's, at the front or behind it; the others belong to the column at
/// the front only, as much as it is in hand and has come in.
fn presence(level: usize, front: usize, stands: bool, column: Standing) -> f32 {
    if stands {
        column.present
    } else if level == front {
        column.active * column.gate
    } else if level < front {
        departing(column.active)
    } else {
        column.present
    }
}

/// Where the two columns' marks stand across the room, from its left edge,
/// at `depth` of the step: the first sliding from its place alone to its
/// place behind, the second coming in from `entry` to the right of its own.
fn marks_at(stand: Stand, depth: f32) -> [f32; 2] {
    [
        lerp(stand.first[0], stand.first[1], depth),
        stand.second + (1.0 - depth) * stand.entry,
    ]
}

/// Where the disc stands at `depth` of the step, between the first column's
/// lit row's disc `first` and the second's `second`: on the first's while the
/// first column is in hand, on the second's once the step is made, and across
/// in the middle of the step — carried with the columns and arriving with
/// them, eased so that it rides each row through its own end of the step.
///
/// The toolkit picker's disc goes over to the other column at half way in
/// one frame (`selected` at `active > 0.5`), the whole step between them; a
/// light that glided after it from there arrived a third of a second after
/// the columns had stopped, an empty disc lying over the strip's words.
fn disc_between(first: [f32; 4], second: [f32; 4], depth: f32) -> [f32; 4] {
    let across = motion::ease(depth.clamp(0.0, 1.0));
    [0, 1, 2, 3].map(|side| lerp(first[side], second[side], across))
}

/// How strong the disc is at `depth` of the step, the row of the first column
/// asking for `first` of its strength and the second's for `second`: the one
/// handing over to the other across the whole of the step, never at once.
/// The toolkit picker's disc takes its column's presence the moment it goes
/// over at half way, which is a column at full strength handing over to one
/// at half — the disc went half out in one frame.
fn handed_over(first: f32, second: f32, depth: f32) -> f32 {
    first + (second - first) * motion::ease(depth.clamp(0.0, 1.0))
}

/// How the disc under the lit row moved from `last` to `now`, for the light
/// to ride: across, and in size about its middle. Up and down it still
/// glides, as the light follows a list rolling under the line.
fn disc_moved(last: [f32; 4], now: [f32; 4]) -> [f32; 4] {
    let across = (now[0] + now[2] * 0.5) - (last[0] + last[2] * 0.5);
    let wider = now[2] - last[2];
    let taller = now[3] - last[3];
    [across - wider * 0.5, -taller * 0.5, wider, taller]
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

impl Trail {
    /// Arrive at the screen: the light on the row of the first column whose
    /// value is in force (the guess the language made, or what was chosen),
    /// the second column shut, and everything standing where it belongs.
    pub fn start(&mut self, first: &[Row]) {
        self.start_on(chosen_or_first(first));
    }

    /// Stand on row `row` of the first column, the second shut and
    /// everything where it belongs at once: what a search does to the
    /// column of what it found, the light on the first.
    pub fn start_on(&mut self, row: usize) {
        self.at = [row, 0];
        self.open = false;
        self.held = false;
        self.walked = false;
        self.shown = false;
        self.settle();
    }

    /// Open the second column with the light on its row in force, standing
    /// there at once rather than rolling from the top. Answers whether it
    /// opened: a list with nothing in it that can be chosen does not.
    pub fn open_on(&mut self, second: &[Row]) -> bool {
        if !second.iter().any(|row| row.enabled) {
            return false;
        }
        self.at[1] = chosen_or_first(second);
        self.rolled[1].place(self.at[1] as f32);
        self.open = true;
        self.held = false;
        self.walked = false;
        self.shown = false;
        self.from = self.at[0];
        true
    }

    /// Whether the light has been walked in the second column since it last
    /// opened — stepped by a key or a pad, or moved to another row by the
    /// pointer — rather than standing on the row the opening put it on: the
    /// row in force, or the first. A row lit so was found by the person, not
    /// handed to them. Only a step after a frame has drawn the column counts
    /// ([`Trail::drawn`]): steps answered before it, in the frame that opened
    /// it — Enter on a row of the first column, Down, Up, Enter — walked a
    /// column nobody had been shown, and the light they leave on the row in
    /// force is still where the opening put it.
    pub fn walked(&self) -> bool {
        self.walked
    }

    /// A frame has drawn the trail as it stands (`screens::drawn`): the
    /// second column, open, has been drawn since it opened, and from now on a
    /// step in it is the person's walk ([`Trail::walked`]).
    pub fn drawn(&mut self) {
        self.shown |= self.open;
    }

    /// Whether the second column is drawn at all: while it is open, and on
    /// its way out only while the first column's light is still on the row
    /// it was opened from. Its rows are worked out from that row every
    /// frame, so a move in the first column as it left would have it list
    /// another part of the world's places on its way out.
    fn second_drawn(&self) -> bool {
        self.open || (self.depth.at > SETTLED && self.at[0] == self.from)
    }

    /// Answer one press. `first` and `second` are what the columns list now.
    /// A press the trail answers lets go of the pointer's hold, so the list
    /// rolls to the light; one it passes on (past a column's foot, to
    /// Continue) leaves the list where the pointer held it.
    pub fn act(&mut self, action: Action, first: &[Row], second: &[Row]) -> Went {
        let went = self.answer(action, first, second);
        if !matches!(went, Went::Nothing | Went::Pass) {
            self.held = false;
        }
        went
    }

    fn answer(&mut self, action: Action, first: &[Row], second: &[Row]) -> Went {
        match (self.open, action) {
            (false, Action::Up) => self.step(0, -1, first),
            (false, Action::Down) => self.step(0, 1, first),
            (true, Action::Up) => self.step(1, -1, second),
            (true, Action::Down) => self.step(1, 1, second),
            (false, Action::Accept | Action::Right) => {
                if self.open_on(second) {
                    Went::Opened
                } else {
                    Went::Nothing
                }
            }
            (true, Action::Accept) if second.get(self.at[1]).is_some_and(|row| row.enabled) => {
                Went::Chose(self.at[1])
            }
            (true, Action::Back | Action::Left) => {
                self.open = false;
                Went::Closed
            }
            (_, Action::Back | Action::Menu | Action::Submit) => Went::Pass,
            _ => Went::Nothing,
        }
    }

    fn step(&mut self, level: usize, direction: isize, rows: &[Row]) -> Went {
        match column::next_enabled(rows, self.at[level], direction) {
            Some(next) => {
                self.at[level] = next;
                self.walked |= level == 1 && self.shown;
                Went::Moved
            }
            None => Went::Nothing,
        }
    }

    /// Which column is at the front: the second while it is open.
    pub fn front(&self) -> usize {
        usize::from(self.open)
    }

    /// Draw the trail in `room`, each column under its heading, the light on
    /// the front column's row when `light` is handed in — which is how a
    /// screen says the light is here.
    pub fn draw(
        &mut self,
        page: &mut Page,
        room: [f32; 4],
        rows: [&[Row]; 2],
        light: Option<&mut Light>,
        look: Look,
    ) {
        let scale = page.scaled(1.0);
        let unit = scale * SHARE;
        let sizes = sizes(scale);
        let stand = stand(room[2], scale);
        let depth = self
            .depth
            .roll(if self.open { 1.0 } else { 0.0 }, look.dt)
            .clamp(0.0, 1.0);
        let aims = self.aims();
        let positions = [
            self.rolled[0].roll(aims[0], look.dt),
            self.rolled[1].roll(aims[1], look.dt),
        ];
        let caption = page.line(Text::Caption);
        let gap = page.metric(Metric::Gap);
        let headed = heading_tall(caption, gap);
        let band = [
            room[0],
            room[1] + headed,
            room[2],
            (room[3] - headed).max(1.0),
        ];
        let cross = band[1] + band[3] * CROSS;
        let right = room[0] + room[2];
        let xs = marks_at(stand, depth).map(|x| room[0] + x);
        let front = self.front();
        // The bloom breathes on the page's clock, as the toolkit picker's
        // does, not the installer's: in low-end mode the toolkit holds the
        // page's still, and a window redrawn once a second there would show
        // the bloom jumping between two sizes at every redraw.
        let pulse = motion::pulse(page.seconds());
        // On a narrow panel the second column comes in where the first's
        // words stood: they go in the first half of the step and the second
        // column comes in in the second, so the two are never drawn over each
        // other (and back the other way round).
        let (first_words, second_in) = halves(depth);

        // Each column as it stands on this frame (`standing`), and how much
        // of it is still inside the room.
        let column_at = |level: usize| {
            let mut column = standing(level, depth, self.open, stand.narrow);
            column.clarity *= inside(xs[level], room[0], right, sizes.lit);
            column
        };

        // The space between the rows scrolls the column at the front too:
        // laid down before the rows, so a row still wins wherever one is
        // under the pointer.
        page.ui().spot(look.ids[front].viewport(), band);

        let from = page.ui().written();

        // The light first, under the lit row: the disc of glass the bar
        // stands its chosen row on, with the picker's breathing bloom behind
        // it. Carried on the installer's one light, so that it flies to
        // Continue and back as every light on every screen does.
        //
        // The bloom breathes behind the row each column stands on, as much
        // as that column is the one in hand — the picker's own crossfade —
        // and never more than the column is there: a second column going out
        // gives up its rows a third of the way from the end, and its bloom,
        // going as it stopped being in hand, glowed on where its row had been
        // for some 100 ms. On a narrow panel, as much as its words are there
        // (`halves`), going with the first column's and coming back with the
        // second.
        if light.is_some() {
            for level in 0..2 {
                if level == 1 && !self.second_drawn() {
                    continue;
                }
                if rows[level].get(self.at[level]).is_none() {
                    continue;
                }
                let column = column_at(level);
                let strength = if stand.narrow {
                    [first_words, second_in][level]
                } else {
                    column.active.min(column.present)
                };
                let offset = self.at[level] as f32 - positions[level];
                let y = cross + offset * sizes.spacing * column.near;
                let icon = lerp(sizes.idle, sizes.lit, column.active) * column.near;
                let glow = icon * (2.3 + 0.2 * pulse);
                let alpha = strength * upright(y, band[1], band[1] + band[3], unit);
                if alpha > 0.01 {
                    let x = xs[level];
                    page.ui().glow(
                        [x - glow * 0.5, y - glow * 0.5, glow, glow],
                        Role::Accent,
                        (0.34 + 0.26 * pulse) * alpha,
                    );
                }
            }
        }
        // The disc stands on the row it lights, as the picker's does,
        // growing and shrinking with it and carried with it across the room
        // as the trail steps: the first column's row, the second's once the
        // step is made, and across from one to the other in the middle of the
        // step, arriving with the columns (`disc_between`) where the picker's
        // disc jumps the step at half way. The light rides the disc rather
        // than gliding after it, so it is never left standing beside the row
        // it lights. On a row coming in or going out it is as faint as the
        // row, as the picker's disc fades with its row, rather than standing
        // there whole and empty — the two rows' strengths handed over across
        // the whole step. A second column gone at once (`second_drawn`) takes
        // its row with it, and the light glides back to the first's.
        let discs = [0, 1].map(|level| {
            if level == 1 && !self.second_drawn() {
                return None;
            }
            rows[level].get(self.at[level])?;
            let column = column_at(level);
            let offset = self.at[level] as f32 - positions[level];
            let y = cross + offset * sizes.spacing * column.near;
            let icon = lerp(sizes.idle, sizes.lit, column.active) * column.near;
            let disc = icon * DISC;
            let rect = [xs[level] - disc * 0.5, y - disc * 0.5, disc, disc];
            // Never brighter than the row it lights: a row coming in or going
            // out, or near the room's ends.
            let fade = column.present * upright(y, band[1], band[1] + band[3], unit);
            Some((rect, fade))
        });
        let disc = match discs {
            [Some((first, a)), Some((second, b))] => Some((
                disc_between(first, second, depth),
                handed_over(a, b, depth),
                true,
            )),
            [Some((rect, fade)), None] | [None, Some((rect, fade))] => Some((rect, fade, false)),
            [None, None] => None,
        };
        match (light, disc) {
            (Some(light), Some((rect, fade, between))) => {
                if let Some((last, was_between)) = self.laid {
                    if was_between == between {
                        light.ride(disc_moved(last, rect));
                    }
                }
                self.laid = Some((rect, between));
                let sunk = motion::pressed(rect, look.pressing);
                light.lay_faded(page, sunk, Shape::Capsule, look.pressing, look.dt, fade);
            }
            _ => self.laid = None,
        }

        for level in 0..2 {
            // Not drawn at all until a row of the first column is pressed,
            // and gone again once it has left.
            if level == 1 && !self.second_drawn() {
                continue;
            }
            let column = column_at(level);
            let Standing {
                near,
                clarity,
                present,
                active,
                ..
            } = column;
            if present * clarity <= 0.01 {
                continue;
            }
            let x = xs[level];
            let words_x = x + sizes.words * near;
            // The first column's words on a narrow panel keep the room they
            // have alone, and go (`first_words`) rather than being squeezed
            // into the strip. Every room is the one the column lands in: a
            // name on its way in from the right runs under the room's edge
            // for a moment rather than being cut short with an ellipsis.
            let fading = level == 0 && stand.narrow;
            let open = self.open && !fading;
            let words_w = words_room(room[2], scale, level, open);
            let words_ink = if fading { first_words } else { 1.0 };
            let pitch = sizes.spacing * near;
            let list = rows[level];

            // The heading over the column, from the left edge of its mark's
            // glass at the front — where it stays while the column steps
            // back, rather than creeping in as the mark shrinks. One that
            // does not stay (`heading_stays`) fades out whole in the room it
            // had at the front, rather than being cut to a letter or two.
            let heading_x = (x - sizes.disc * 0.5).max(room[0]);
            let words = look.headings[level];
            let ui = page.ui();
            let wide = ui.measure(Text::Caption, words);
            let settled = heading_room(room[2], scale, level, self.open);
            let (heading_w, fade) = if heading_stays(level, front, stand.narrow, wide <= settled) {
                (settled, 1.0)
            } else {
                (heading_room(room[2], scale, level, false), first_words)
            };
            // A lesser line, in the one ink every lesser line is written in.
            let (role, strength) = text::LESSER;
            text::label_faded(
                ui,
                [heading_x, room[1], heading_w, caption],
                Text::Caption,
                words,
                role,
                strength * clarity * present * fade,
                Align::Left,
            );

            for index in in_view(positions[level], list.len(), pitch, band[3]) {
                let row = &list[index];
                let offset = index as f32 - positions[level];
                let y = cross + offset * pitch;
                if y < band[1] - pitch || y > band[1] + band[3] + pitch {
                    continue;
                }
                let distance = offset.abs();
                // The row a column stands on keeps all of its ink whether the
                // column is at the front or behind it; the others belong to
                // the column at the front only.
                let stands = index == self.at[level];
                // The row the pointer lit where it stands is the lit row
                // wherever it is, at the whole of its ink; any other is lit
                // on the line alone.
                let held_here = stands && self.held && level == front;
                let presence = presence(level, front, stands, column);
                let far = if held_here {
                    1.0
                } else {
                    (1.0 - distance / 6.0).clamp(0.0, 1.0)
                };
                let alpha = presence * clarity * far * upright(y, band[1], band[1] + band[3], unit);
                if alpha <= 0.01 {
                    continue;
                }
                let grown = if stands && (distance < 0.5 || held_here) && active > 0.5 {
                    active
                } else {
                    0.0
                };
                let lit_row = grown > 0.0;
                let icon = lerp(sizes.idle, sizes.lit, grown) * near;
                let ink = if row.enabled { 1.0 } else { 0.55 };

                // Where a pointer can land: the row's band, from its glass to
                // the end of its words. The rows of the column at the front,
                // and the row the one behind was opened from — a click on
                // which goes back to it.
                let behind = level < front && stands;
                if row.enabled && (level == front || behind) {
                    let reach = [
                        x - sizes.disc * 0.5 * near,
                        y - pitch * 0.44,
                        (words_x + words_w - (x - sizes.disc * 0.5 * near)).max(icon),
                        pitch * 0.88,
                    ];
                    if let Some(reach) = column::intersection(reach, band) {
                        page.ui().spot(look.ids[level].row(index), reach);
                    }
                }

                // The bar's own two tints for a mark: the lit one quieter
                // against the light's glass, the rest in the accent's pale.
                let (role, strength) = if lit_row {
                    (Role::TextSoft, 0.9)
                } else {
                    (Role::AccentSoft, 0.75)
                };
                let icons = page.icons();
                page.ui().icon_tinted(
                    [x - icon * 0.5, y - icon * 0.5, icon, icon],
                    look.marks[level],
                    icons,
                    role,
                    alpha * strength * ink,
                );
                let tick = if row.chosen {
                    page.scaled(CHOSEN) * near
                } else {
                    0.0
                };
                let name_room =
                    (words_w - if row.chosen { tick + gap * 0.5 } else { 0.0 }).max(0.0);
                // A column stepped back into a strip of a narrow panel keeps
                // its row's mark and nothing beside it: no room for a name is
                // no room for its tick either.
                let alpha = alpha * words_ink;
                if name_room < page.scaled(WORDS_LEAST) || alpha <= 0.01 {
                    continue;
                }
                let (name_end, name_middle) = if lit_row {
                    lit_words(page, [words_x, y], name_room, words_w, row, alpha * ink)
                } else {
                    // The name alone, a little quieter, and quieter still on
                    // the row a column behind was opened from.
                    let line = page.line(Text::Body);
                    let ui = page.ui();
                    let name = text::fit(ui, Text::Body, &row.name, name_room);
                    let quiet = if level < front { 0.85 } else { 1.0 };
                    let tint = ui.tinted(Role::Text, alpha * QUIET * quiet * ink);
                    let line_rect = [words_x, y - line * 0.5, name_room, line];
                    ui.label_tinted(line_rect, Text::Body, &name, tint, Align::Left);
                    (words_x + ui.measure(Text::Body, &name), y)
                };
                if row.chosen {
                    mark::chosen(
                        page.ui(),
                        [name_end + gap * 0.5, name_middle - tick * 0.5, tick, tick],
                        icons,
                        alpha * ink,
                    );
                }
            }
        }
        let to = page.ui().written();
        // Cut to the room's top and bottom, so nothing on its way past an
        // end reaches the head or the buttons (a row has faded out before the
        // headings, `upright`); and to its sides with a little air either
        // side, for the glow the light carries past its disc.
        let air = page.metric(Metric::PanelPadding) * 0.9;
        page.ui().cut_between(
            from,
            to,
            [room[0] - air, room[1], room[2] + air * 2.0, room[3]],
        );
    }

    /// The column and row a spot number is, if it is a row of either: what
    /// the pointer moved onto or clicked, read off the frame it was drawn on.
    pub fn row_of(id: u32, ids: [spots::Column; 2], rows: [&[Row]; 2]) -> Option<(usize, usize)> {
        (0..2).find_map(|level| {
            ids[level]
                .row_of(id, rows[level].len())
                .map(|index| (level, index))
        })
    }

    /// Put the light on row `index` of column `level`, which the pointer
    /// moved onto or clicked, the way the shell's file picker points
    /// (`Picker::point_at`): a row of the column at the front only, and one
    /// that can be chosen. The row a column behind was opened from is the
    /// trail, which a hover leaves alone (a click on it goes back to it,
    /// [`Trail::pressed`]); and a row read off a frame drawn before a step
    /// is not the front's any more. Answers whether the light stands there.
    ///
    /// The list holds still under it (`held`), the row lit where it stands,
    /// as every list in the installer holds under the pointer: the toolkit's
    /// picker rolls the row to the line, and the row that rolls in under the
    /// resting pointer is lit at the next nudge, walking the light down the
    /// list. A press the trail answers rolls it there.
    pub fn point_at(&mut self, level: usize, index: usize, rows: &[Row]) -> bool {
        if level != self.front() || !rows.get(index).is_some_and(|row| row.enabled) {
            return false;
        }
        self.walked |= level == 1 && self.shown && index != self.at[1];
        self.at[level] = index;
        self.held = true;
        true
    }

    /// A key or a pad brought the light back into the trail: the column at
    /// the front rolls its lit row to the line.
    pub fn let_go(&mut self) {
        self.held = false;
    }

    /// What a click landed on, if it landed on the trail. A click on the
    /// row the second column was opened from closes it, as Back does — the
    /// shell's trail, a press on which walks back out to it — and the trail
    /// does that itself. A click on a row of the column at the front is the
    /// screen's to answer: it points at the row as a hover would and presses
    /// it as Accept would, so a click and a press of South reach the same one
    /// place.
    pub fn pressed(
        &mut self,
        page: &mut Page,
        rows: [&[Row]; 2],
        ids: [spots::Column; 2],
    ) -> Option<Click> {
        for id in ids {
            let _ = page.pressed(id.viewport());
        }
        if self.open && page.pressed(ids[0].row(self.at[0])) {
            self.open = false;
            self.held = false;
            return Some(Click::Closed);
        }
        let front = self.front();
        let mut found = None;
        for (index, row) in rows[front].iter().enumerate() {
            if page.pressed(ids[front].row(index)) && row.enabled && found.is_none() {
                found = Some(index);
            }
        }
        found.map(Click::Row)
    }

    /// Put everything where it is going, for a photograph.
    pub fn settle(&mut self) {
        self.laid = None;
        self.depth.place(if self.open { 1.0 } else { 0.0 });
        let aims = self.aims();
        self.rolled[0].place(aims[0]);
        self.rolled[1].place(aims[1]);
    }

    /// Where each list is rolling to: its lit row, or — in the column the
    /// pointer holds — where it already is.
    fn aims(&self) -> [f32; 2] {
        let front = self.front();
        [0, 1].map(|level| {
            if self.held && level == front {
                self.rolled[level].to
            } else {
                self.at[level] as f32
            }
        })
    }

    /// Whether anything in the trail is still moving.
    pub fn moving(&self) -> bool {
        self.depth.moving() || self.rolled.iter().any(Rolling::moving)
    }
}

/// The lit row's words, at `at` (the start of its words, the line it stands
/// on): its name large and bold, on as many lines as [`lit_name`] gives it —
/// a layout's long name on a display standing on its side takes two — and
/// its note under it, the block centred on the line. Answers where the
/// name's last line ends and the middle of that line, for the `chosen` mark.
fn lit_words(
    page: &mut Page,
    at: [f32; 2],
    name_room: f32,
    note_room: f32,
    row: &Row,
    alpha: f32,
) -> (f32, f32) {
    let caption = page.line(Text::Caption);
    let ui = page.ui();
    let (style, names) = lit_name(&row.name, name_room, |style, candidate| {
        text::bold_width(ui, style, candidate)
    });
    let line_tall = ui.line(style);
    let note = row
        .note
        .as_deref()
        .map(|note| text::fit_parts(ui, Text::Caption, note, note_room));
    let tall = line_tall * names.len().max(1) as f32 + note.as_ref().map_or(0.0, |_| caption);
    let top = at[1] - tall * 0.5;
    let tint = ui.tinted(Role::Text, alpha);
    let mut end = (at[0], at[1]);
    for (index, name) in names.iter().enumerate() {
        let line = [at[0], top + index as f32 * line_tall, name_room, line_tall];
        ui.label_weighted(line, style, name, tint, Align::Left, true);
        end = (
            at[0] + text::bold_width(ui, style, name),
            line[1] + line_tall * 0.5,
        );
    }
    if let Some(note) = note {
        // A lesser line, and still white, as every one is.
        let under = [
            at[0],
            top + line_tall * names.len() as f32,
            note_room,
            caption,
        ];
        let (role, strength) = text::LESSER;
        let tint = ui.tinted(role, alpha * strength);
        ui.label_tinted(under, Text::Caption, &note, tint, Align::Left);
    }
    end
}

/// How the lit row says its name in `room`: large and bold, on up to two
/// lines; or where that is not enough — universal time's own words in
/// Russian on a display standing on its side — at a row's own size, still
/// bold, on up to three. Answers the style and the lines; `measure` is how
/// wide words are in a style's bold face.
pub fn lit_name(
    name: &str,
    room: f32,
    mut measure: impl FnMut(Text, &str) -> f32,
) -> (Text, Vec<String>) {
    let lines = text::break_lines(name, room, |candidate| measure(Text::Title, candidate));
    let fits = lines.len() <= 2
        && lines
            .iter()
            .all(|line| measure(Text::Title, line) <= room + 0.5);
    if fits {
        return (Text::Title, lines);
    }
    let lines = text::wrapped_by(name, room, 3, |candidate| measure(Text::Body, candidate));
    (Text::Body, lines)
}

/// How long the second column takes to come in as it opens, in seconds. The
/// trail steps over to it on the light's spring (the toolkit's
/// `HIGHLIGHT_SPRING`, critically damped), and drawn sixty times a second it
/// has come within half a reference pixel of its place across the [`STEP`] —
/// still, to the eye — on its 26th frame, 0.43 s after the press that opened
/// it; this is the frame after that
/// (`the_second_column_comes_in_within_its_opening`). Before it the column is
/// still sliding in: a tenth of a second in it is 62% of the way, at a fifth
/// 92%.
pub const OPENING: f32 = 0.45;

/// The same where nothing of the installer's own moves (low-end mode,
/// `State::low_end`): the column stands whole on the first frame that draws
/// it, so its motion has no length to give, and it is held 0.3 s instead —
/// longer than two quick presses of one button are apart (Enter or South
/// tapped twice comes a tenth to a quarter of a second apart), short enough
/// that a person who reads the line and presses again is not kept waiting.
pub const OPENING_AT_ONCE: f32 = 0.3;

/// How long the second column has to stand on the screen, drawn, before the
/// row in force in it counts as seen ([`Seen`]): [`OPENING`], or in low-end
/// mode [`OPENING_AT_ONCE`].
pub fn dwell(low_end: bool) -> f32 {
    if low_end {
        OPENING_AT_ONCE
    } else {
        OPENING
    }
}

/// How far the frames have to have carried the second column into its place
/// before it counts as come in, in seconds of its own motion: its whole
/// [`OPENING`] — or nothing in low-end mode, where it stands whole on the
/// first frame that draws it.
fn coming_in(low_end: bool) -> f32 {
    if low_end {
        0.0
    } else {
        OPENING
    }
}

/// How a frame showed the row in force in the second column, as the screen
/// tells [`Seen::drawn`] once it has drawn it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Showing {
    /// Not its column: the second column shut, open on another row of the
    /// first (another part of the world, another layout), or a search
    /// narrowing the columns over it.
    Not,
    /// Its column, standing open with the light on another of its rows: the
    /// row in force somewhere in it, perhaps rows away and out of sight.
    Column,
    /// Its column, standing open with the row in force on the line, the
    /// light on it.
    Row,
}

/// Whether the row in force — what a screen keeps unless the person chooses
/// another: Time zone's place, Keyboard's kind — has been seen in the second
/// column: Continue never goes on with a value the person was not shown, by
/// design. Kept in the screen's view; the screen moves it on as the column
/// opens on that row ([`Seen::opened`]) or shuts ([`Seen::shut`]) and as a
/// row is chosen ([`Seen::see`]), and the frame as it draws the screen
/// ([`Seen::drawn`]).
///
/// **Seen is the column standing on the screen, open on the row, for the
/// dwell**, counted by the frames that draw it — every one of them, from the
/// first, drawing it so ([`Showing`]); one that does not ends it. The dwell
/// begins with the first frame that draws the column open with the light on
/// the row — opened on it, or the light come onto it in its column standing
/// open — and the row is seen once:
///
/// * a frame has been drawn [`dwell`] after the *second* frame that drew it,
///   by the installer's clock. Not the first: a frame is begun only once the
///   one before it has gone to the display, so the second is the first
///   instant the column is known to be on the screen — the first may have
///   taken any time to draw, the display showing what stood before;
/// * and, where the column moves in (not in low-end mode), the frames that
///   drew it have carried it its whole [`OPENING`], as `Trail::draw` carries
///   the trail's depth — each by its own length, a tenth of a second at most
///   however long it really took — and a frame has been drawn after the one
///   that brought it in. A frame or two drawn across a stall, the column
///   barely begun in them (on a display standing on its side, not a name of
///   it drawn yet), never count, however long the stall.
///
/// **A press finds the row seen only where a frame before it counted it so.**
/// Every press that reaches a frame is answered at that frame's one instant
/// of the installer's clock, before the frame is drawn, and may have come in
/// at any moment since the frame before: two presses in one frame — Enter
/// twice, Enter and a click, the toolkit draining a queue that built up while
/// a slow frame was drawn — are no time apart, and a press answered a dwell
/// after the column opened may have been made before the column ever reached
/// the screen. The pointer's presses are answered after the frame is drawn,
/// for they are asked of the spots its drawing wrote, and find the row as it
/// stood before it was drawn (`State::as_before_drawing`): one the frame has
/// just counted seen is not, to them. While a dwell runs the window is asked
/// for frames (`screens::drawn`), so that one comes to count it.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Seen {
    /// Not seen: the column has not stood open on the row in force, or it
    /// went (or a search covered it) before it had stood there long enough.
    #[default]
    Not,
    /// The column has just opened on it, and no frame has drawn it yet.
    Opened,
    /// Every frame since the column opened on it, or the light came onto it,
    /// has drawn the column open on it.
    Drawn {
        /// The instant, by the installer's clock, of the second of those
        /// frames, by which the first had reached the screen — none while
        /// only the first has been drawn. The dwell runs from here.
        since: Option<f32>,
        /// How far those frames have carried the column in, in seconds of
        /// its own motion: the sum of their lengths as the installer's clock
        /// counts them (`Clock::dt`, which `Trail::draw` carries the trail's
        /// depth by).
        came: f32,
    },
    /// Seen, until the row in force changes under the person
    /// ([`Seen::forget`]).
    Yes,
}

impl Seen {
    /// The column has just opened on the row in force: the dwell is to begin
    /// with the frame that draws it. A row seen already stays seen.
    pub fn opened(&mut self) {
        if *self != Seen::Yes {
            *self = Seen::Opened;
        }
    }

    /// The column has shut, or opened on another row: what had begun for
    /// the row in force is over, and begins again when it opens on it.
    pub fn shut(&mut self) {
        if *self != Seen::Yes {
            *self = Seen::Not;
        }
    }

    /// A frame has drawn the screen at `now`, `dt` long by the installer's
    /// clock, the row in force `showing` as it showed it, in low-end mode or
    /// not: the first frame to draw its column open on it, the light on it,
    /// begins the dwell — the frame after the column opened on it, or the one
    /// that shows the light come onto it in the column standing open — the
    /// second sets the instant it runs from, and one drawn the dwell after
    /// that, the column come in by the frames before it, counts the row seen.
    /// A frame without its column ends what had begun; the light on another
    /// row of it does not, nor does it begin anything — not even in the first
    /// frame after the column opened on the row, the light stepped off it
    /// before that frame was drawn, where the row may stand out of sight:
    /// nothing has begun then, and Continue rolls the column back onto it.
    pub fn drawn(&mut self, showing: Showing, now: f32, dt: f32, low_end: bool) {
        let (dwell, coming_in) = (dwell(low_end), coming_in(low_end));
        *self = match (*self, showing) {
            (Seen::Yes, _) => Seen::Yes,
            (Seen::Opened | Seen::Drawn { .. }, Showing::Not) => Seen::Not,
            (Seen::Opened | Seen::Not, Showing::Row) => Seen::Drawn {
                since: None,
                came: dt,
            },
            // Opened on it, and the light stepped off it before a frame drew
            // the column: the row may be out of sight, and nothing begins
            // until a frame shows it on the line.
            (Seen::Opened, Showing::Column) | (Seen::Not, _) => Seen::Not,
            (Seen::Drawn { since: None, came }, _) => Seen::Drawn {
                since: Some(now),
                came: came + dt,
            },
            (
                Seen::Drawn {
                    since: Some(at),
                    came,
                },
                _,
            ) if came >= coming_in && now - at >= dwell => Seen::Yes,
            (Seen::Drawn { since, came }, _) => Seen::Drawn {
                since,
                came: came + dt,
            },
        };
    }

    /// Whether the row in force has been seen: counted so by a frame drawn
    /// before this one's presses, or chosen.
    pub fn seen(&self) -> bool {
        *self == Seen::Yes
    }

    /// Whether the column has opened on the row in force (or the light come
    /// onto it there) and the frames have not yet counted it seen.
    pub fn dwelling(&self) -> bool {
        matches!(self, Seen::Opened | Seen::Drawn { .. })
    }

    /// Seen at once: the row was chosen, as nobody does without seeing it,
    /// or there is no column to show it in.
    pub fn see(&mut self) {
        *self = Seen::Yes;
    }

    /// Not seen: the row in force changed under the person, which nobody has
    /// been shown yet.
    pub fn forget(&mut self) {
        *self = Seen::Not;
    }
}

/// The row whose value is in force, or the first that can be chosen.
fn chosen_or_first(rows: &[Row]) -> usize {
    rows.iter()
        .position(|row| row.chosen && row.enabled)
        .or_else(|| column::nearest_enabled(rows, 0))
        .unwrap_or(0)
}

/// One number carried on the toolkit's highlight spring, as the picker
/// carries its lists and its depth: a list's position under the line, the
/// trail's step.
#[derive(Debug, Default, Clone, Copy)]
struct Rolling {
    at: f32,
    speed: f32,
    to: f32,
}

impl Rolling {
    /// Put it somewhere at once.
    fn place(&mut self, to: f32) {
        self.at = to;
        self.to = to;
        self.speed = 0.0;
    }

    /// Carry it one frame's worth towards `to`, and answer where it is.
    fn roll(&mut self, to: f32, dt: f32) -> f32 {
        self.to = to;
        let (at, speed) = motion::spring(
            self.at as f64,
            self.speed as f64,
            to as f64,
            motion::HIGHLIGHT_SPRING,
            dt as f64,
        );
        self.at = at as f32;
        self.speed = speed as f32;
        if (self.at - to).abs() < SETTLED && self.speed.abs() < SETTLED {
            self.place(to);
        }
        self.at
    }

    fn moving(&self) -> bool {
        self.at != self.to || self.speed != 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(count: usize) -> Vec<Row> {
        (0..count)
            .map(|index| Row::new(format!("{index}")))
            .collect()
    }

    fn chosen(count: usize, at: usize) -> Vec<Row> {
        let mut list = rows(count);
        list[at].chosen = true;
        list
    }

    #[test]
    fn it_starts_on_the_value_in_force_with_the_second_column_shut() {
        let mut trail = Trail::default();
        trail.start(&chosen(5, 3));
        assert_eq!(trail.at[0], 3);
        assert!(!trail.open);
        assert_eq!(trail.front(), 0);
    }

    #[test]
    fn accept_or_right_opens_the_second_column_on_its_row_in_force() {
        for press in [Action::Accept, Action::Right] {
            let (first, second) = (rows(3), chosen(6, 4));
            let mut trail = Trail::default();
            trail.start(&first);
            assert_eq!(trail.act(press, &first, &second), Went::Opened);
            assert!(trail.open);
            assert_eq!(trail.at[1], 4, "{press:?} opened it on another row");
            assert_eq!(trail.front(), 1);
        }
    }

    #[test]
    fn moving_in_the_first_column_never_opens_the_second() {
        let (first, second) = (rows(3), rows(4));
        let mut trail = Trail::default();
        trail.start(&first);
        assert_eq!(trail.act(Action::Down, &first, &second), Went::Moved);
        assert_eq!(trail.act(Action::Down, &first, &second), Went::Moved);
        assert_eq!(
            trail.act(Action::Down, &first, &second),
            Went::Nothing,
            "past the foot"
        );
        assert_eq!(trail.act(Action::Left, &first, &second), Went::Nothing);
        assert!(!trail.open);
        assert_eq!(trail.at[0], 2);
    }

    #[test]
    fn back_or_left_closes_the_second_column_and_back_again_passes() {
        for press in [Action::Back, Action::Left] {
            let (first, second) = (rows(3), rows(4));
            let mut trail = Trail::default();
            trail.start(&first);
            trail.act(Action::Accept, &first, &second);
            assert_eq!(trail.act(press, &first, &second), Went::Closed);
            assert!(!trail.open);
        }
        let (first, second) = (rows(3), rows(4));
        let mut trail = Trail::default();
        trail.start(&first);
        assert_eq!(trail.act(Action::Back, &first, &second), Went::Pass);
    }

    #[test]
    fn accept_in_the_second_column_chooses_its_row() {
        let (first, second) = (rows(3), rows(4));
        let mut trail = Trail::default();
        trail.start(&first);
        trail.act(Action::Accept, &first, &second);
        assert_eq!(trail.act(Action::Down, &first, &second), Went::Moved);
        assert_eq!(trail.act(Action::Accept, &first, &second), Went::Chose(1));
        assert_eq!(
            trail.act(Action::Right, &first, &second),
            Went::Nothing,
            "Right past the second column is the screen's to answer"
        );
    }

    /// A column on its way out lists the places of the row it was opened
    /// from or nothing: never the next part of the world's.
    #[test]
    fn a_closing_column_goes_at_once_when_the_first_column_moves() {
        let (first, second) = (rows(3), rows(4));
        let mut trail = Trail::default();
        trail.start(&first);
        assert!(!trail.second_drawn(), "drawn before anything was pressed");
        trail.act(Action::Accept, &first, &second);
        trail.settle();
        assert!(trail.second_drawn());
        trail.act(Action::Back, &first, &second);
        assert!(trail.second_drawn(), "it left without going out");
        trail.act(Action::Down, &first, &second);
        assert!(
            !trail.second_drawn(),
            "it listed another row's on its way out"
        );
        trail.act(Action::Up, &first, &second);
        trail.settle();
        assert!(!trail.second_drawn(), "drawn once it had gone");
    }

    /// A row the pointer lit is lit where it stands: the column holds still
    /// under the resting pointer until a press the trail answers, or a key
    /// bringing the light back, rolls it to the line. Rolling it at once had
    /// the next nudge light the row that came in under the pointer, and walk
    /// the light down the list.
    #[test]
    fn a_column_holds_still_under_the_pointer_until_a_press_rolls_it() {
        let (first, second) = (rows(9), rows(4));
        let mut trail = Trail::default();
        trail.start(&first);
        assert_eq!(trail.aims(), [0.0, 0.0]);
        assert!(trail.point_at(0, 2, &first));
        assert_eq!(trail.at[0], 2);
        assert_eq!(trail.aims()[0], 0.0, "the list rolled under the pointer");
        trail.settle();
        assert_eq!(trail.rolled[0].at, 0.0, "a picture rolled it");
        // A key the trail answers rolls it to the light.
        assert_eq!(trail.act(Action::Down, &first, &second), Went::Moved);
        assert_eq!(trail.aims()[0], 3.0);
        // A key past the foot is not the trail's to answer: the light goes
        // on to Continue and the list stays where the pointer held it.
        trail.settle();
        assert!(trail.point_at(0, 8, &first));
        assert_eq!(trail.act(Action::Down, &first, &second), Went::Nothing);
        assert_eq!(trail.aims()[0], 3.0, "a key past the foot moved the list");
        // A key bringing the light back lets go.
        trail.let_go();
        assert_eq!(trail.aims()[0], 8.0);
        // Opening lets go too, and the second column opens on its line.
        trail.settle();
        assert!(trail.point_at(0, 1, &first));
        assert_eq!(trail.act(Action::Accept, &first, &second), Went::Opened);
        assert_eq!(trail.aims(), [1.0, 0.0]);
    }

    /// The pointer lights a row of the column at the front, as the shell's
    /// file picker points; the row the second column was opened from, and a
    /// row of the first read off a frame drawn before it stepped back, are
    /// left alone, and so is a row that cannot be chosen.
    #[test]
    fn the_pointer_lights_a_row_of_the_column_at_the_front_only() {
        let (first, mut second) = (rows(5), rows(4));
        second[2].enabled = false;
        let ids = [spots::Column::nth(0), spots::Column::nth(1)];
        let mut trail = Trail::default();
        trail.start(&first);
        assert_eq!(
            Trail::row_of(ids[0].row(3), ids, [&first, &second]),
            Some((0, 3))
        );
        assert_eq!(
            Trail::row_of(ids[1].row(1), ids, [&first, &second]),
            Some((1, 1))
        );
        assert_eq!(
            Trail::row_of(ids[0].viewport(), ids, [&first, &second]),
            None
        );
        assert!(trail.point_at(0, 3, &first));
        assert_eq!(trail.at[0], 3);
        assert!(!trail.open, "a hover opened the second column");
        assert!(
            !trail.point_at(1, 1, &second),
            "a shut column's row was lit"
        );
        trail.act(Action::Accept, &first, &second);
        assert!(trail.point_at(1, 3, &second));
        assert_eq!(trail.at[1], 3);
        assert!(
            !trail.point_at(1, 2, &second),
            "a row that cannot be chosen"
        );
        assert!(
            !trail.point_at(0, 1, &first),
            "a row of the column behind was lit"
        );
        assert!(!trail.point_at(0, 3, &first), "the trail's own row was lit");
        assert_eq!(trail.at, [3, 3]);
        assert!(trail.open);
    }

    #[test]
    fn a_second_column_with_nothing_to_choose_does_not_open() {
        let first = rows(2);
        let none = vec![Row::new("blocked").enabled(false)];
        let mut trail = Trail::default();
        trail.start(&first);
        assert_eq!(trail.act(Action::Accept, &first, &none), Went::Nothing);
        assert!(!trail.open);
    }

    #[test]
    fn the_columns_roll_and_step_and_then_stand_still() {
        let (first, second) = (rows(9), rows(9));
        let mut trail = Trail::default();
        trail.start(&first);
        assert!(
            !trail.moving(),
            "a column arrived at rolls from where it is"
        );
        trail.at[0] = 6;
        trail.open_on(&second);
        trail.rolled[0].roll(6.0, 1.0 / 60.0);
        let depth = trail.depth.roll(1.0, 1.0 / 60.0);
        assert!(depth > 0.0 && depth < 1.0, "the trail jumped: {depth}");
        assert!(trail.moving());
        for _ in 0..240 {
            trail.rolled[0].roll(6.0, 1.0 / 60.0);
            trail.rolled[1].roll(trail.at[1] as f32, 1.0 / 60.0);
            trail.depth.roll(1.0, 1.0 / 60.0);
        }
        assert!(!trail.moving(), "a settled trail still wants frames");
        trail.open = false;
        trail.settle();
        assert_eq!(trail.depth.at, 0.0);
        assert!(!trail.moving());
    }

    /// Every landscape panel stands the two columns side by side, the one
    /// at the front on the picker's cross and the trail a whole step apart
    /// as it slides; a display standing on its side gives the second the
    /// room and keeps the first's mark in a strip at the left.
    #[test]
    fn a_wide_panel_keeps_both_columns_and_a_narrow_one_keeps_a_strip() {
        let wide = stand(1192.0, 1.0);
        assert!(!wide.narrow);
        assert_eq!(
            wide.first[0], wide.second,
            "the column at the front left the cross"
        );
        assert_eq!(wide.first[0] - wide.first[1], STEP, "the trail stretched");
        assert_eq!(wide.entry, STEP, "the second column came in from nearer");
        assert_eq!(wide.first[1], sizes(1.0).disc * 0.5);
        assert!(words_room(1192.0, 1.0, 0, false) >= LEAST);
        assert!(words_room(1192.0, 1.0, 1, true) >= LEAST);
        assert!(
            words_room(1192.0, 1.0, 0, true) > 280.0,
            "no room for a part of the world"
        );

        let narrow_width = 471.0;
        let narrow = stand(narrow_width, 1.0);
        assert!(narrow.narrow);
        let strip = narrow_width * layout::PEEK;
        let sizes = sizes(1.0);
        let mark = sizes.idle * SHRINK * 0.5;
        assert!(
            narrow.first[1] - mark >= 0.0 && narrow.first[1] + mark <= strip,
            "the first column's mark is not in its strip"
        );
        assert!(
            narrow.second - sizes.disc * 0.5 >= strip,
            "the second column's glass runs into the strip"
        );
        assert_eq!(
            words_room(narrow_width, 1.0, 0, true),
            0.0,
            "a name was given room in the strip"
        );
        assert!(words_room(narrow_width, 1.0, 1, true) > 260.0);
    }

    /// On a narrow panel the first column's words and heading, and the
    /// second column, are never drawn at once, opening or closing: the
    /// headings stood over each other for three frames after Back.
    #[test]
    fn a_narrow_panels_step_never_draws_both_columns_words_at_once() {
        for frame in 0..=120 {
            let depth = frame as f32 / 120.0;
            let (first, second) = halves(depth);
            assert!(
                first * second == 0.0,
                "both at a depth of {depth}: {first} and {second}"
            );
        }
        assert_eq!(halves(0.0), (1.0, 0.0));
        assert_eq!(halves(1.0), (0.0, 1.0));
        for front in 0..2 {
            for fits in [false, true] {
                assert!(
                    !heading_stays(0, front, true, fits),
                    "the first column's heading stood over the strip"
                );
            }
        }
        // On a landscape panel a heading with room for it stays put, and
        // the one at the front always does.
        assert!(heading_stays(0, 1, false, true));
        assert!(!heading_stays(0, 1, false, false));
        assert!(heading_stays(0, 0, false, false));
        assert!(heading_stays(1, 1, true, true));
    }

    /// The disc under the lit row stays on the row through every frame of
    /// the step, opening and closing, on a landscape panel and on a display
    /// standing on its side: it stood on the cross while the columns slid
    /// past it, beside the row it lit for some 200 ms of every step. In the
    /// middle of the step it goes across from one column's row to the
    /// other's with the columns — never the whole step in one frame (the
    /// toolkit picker's hand-over), and never left behind them: the light is
    /// where the disc is on every frame, and on the row of the column in
    /// hand at each end of the step.
    #[test]
    fn the_disc_rides_the_row_it_lights_and_goes_across_with_the_step() {
        let dt = 1.0 / 60.0;
        for width in [1192.0, 471.0] {
            let stand = stand(width, 1.0);
            let sizes = sizes(1.0);
            let disc_at = |depth: f32, level: usize| {
                let column = standing(level, depth, true, stand.narrow);
                let x = marks_at(stand, depth)[level];
                let disc = lerp(sizes.idle, sizes.lit, column.active) * column.near * DISC;
                [x - disc * 0.5, 300.0 - disc * 0.5, disc, disc]
            };
            // How far apart the two columns' rows are at half way: what the
            // light used to jump in one frame.
            let gap = (disc_at(0.5, 1)[0] - disc_at(0.5, 0)[0]).abs();
            let mut light = Light::default();
            let mut depth = Rolling::default();
            let mut last: Option<[f32; 4]> = None;
            let mut was = None;
            for (frame, to) in (0..120).map(|frame| (frame, if frame < 60 { 1.0 } else { 0.0 })) {
                let depth = depth.roll(to, dt).clamp(0.0, 1.0);
                let rect = disc_between(disc_at(depth, 0), disc_at(depth, 1), depth);
                if let Some(last) = last {
                    light.ride(disc_moved(last, rect));
                }
                last = Some(rect);
                let at = light.glide(rect, dt);
                for (side, (got, wanted)) in at.iter().zip(rect).enumerate() {
                    assert!(
                        (got - wanted).abs() < 0.5,
                        "{width} wide, frame {frame}: the light at {at:?}, the disc at {rect:?} ({side})"
                    );
                }
                if let Some(was) = was {
                    let moved: f32 = at[0] - was;
                    assert!(
                        moved.abs() < gap * 0.25,
                        "{width} wide, frame {frame}: the light jumped {moved} of {gap}"
                    );
                }
                was = Some(at[0]);
            }
        }
        // At each end of the step the disc is the row's own.
        let (first, second) = ([10.0, 20.0, 30.0, 30.0], [400.0, 20.0, 40.0, 40.0]);
        assert_eq!(disc_between(first, second, 0.0), first);
        assert_eq!(disc_between(first, second, 1.0), second);
        let middle = disc_between(first, second, 0.5);
        assert!(middle[0] > first[0] && middle[0] < second[0]);
    }

    #[test]
    fn the_discs_strength_is_handed_over_across_the_step() {
        assert_eq!(handed_over(1.0, 0.0, 0.0), 1.0);
        assert_eq!(handed_over(1.0, 0.25, 1.0), 0.25);
        // Either side of half way, where the picker's disc changes columns,
        // the strength hardly moves.
        let before = handed_over(1.0, 0.5, 0.49);
        let after = handed_over(1.0, 0.5, 0.51);
        assert!((before - after).abs() < 0.05, "{before} then {after}");
    }

    /// On a display standing on its side the second column comes in whole:
    /// its other rows were drawn at a depth of 0.62 on the first frame it
    /// showed, while its lit row was held back to the second half.
    #[test]
    fn a_column_coming_in_brings_its_other_rows_no_sooner_than_its_lit_one() {
        for narrow in [true, false] {
            for (open, front) in [(true, 1), (false, 0)] {
                for frame in 0..=120 {
                    let depth = frame as f32 / 120.0;
                    let column = standing(1, depth, open, narrow);
                    let lit = presence(1, front, true, column);
                    let other = presence(1, front, false, column);
                    assert!(
                        other <= lit + 1e-6,
                        "narrow {narrow}, open {open}, depth {depth}: {other} before {lit}"
                    );
                }
            }
        }
        assert_eq!(standing(1, 0.5, true, true).present, 0.0);
        assert_eq!(
            presence(1, 1, false, standing(1, 0.62, true, true)),
            standing(1, 0.62, true, true).present
        );
    }

    #[test]
    fn the_rows_are_the_toolkit_pickers_size_and_the_disc_is_the_bars() {
        let sizes = sizes(1.0);
        assert_eq!(sizes.spacing, Metric::ItemSpacing.value() * SHARE);
        assert!(sizes.disc > sizes.lit && sizes.lit > sizes.idle);
        assert!((sizes.disc / sizes.lit - std::f32::consts::SQRT_2 * 1.04).abs() < 1e-4);
    }

    /// [`OPENING`] is the step's own length: the trail's depth, carried as
    /// `Trail::draw` carries it sixty times a second, is still more than half
    /// a reference pixel short of the whole [`STEP`] at 0.4 s and within it
    /// by [`OPENING`].
    #[test]
    fn the_second_column_comes_in_within_its_opening() {
        use crate::ui::motion::FRAME;
        let half_a_pixel = 0.5 / STEP;
        let mut depth = Rolling::default();
        let frames = (OPENING / FRAME).round() as usize;
        assert_eq!(frames, 27);
        for frame in 1..=frames {
            let at = depth.roll(1.0, FRAME);
            if frame == 24 {
                assert!(1.0 - at > half_a_pixel, "still at 0.4 s: {at}");
            }
            if frame == frames {
                assert!(1.0 - at < half_a_pixel, "not there at {OPENING} s: {at}");
            }
        }
        assert_eq!(dwell(false), OPENING);
        assert_eq!(dwell(true), OPENING_AT_ONCE);
    }

    /// The row in force counts as seen once a frame drawn the dwell after
    /// the second frame that drew it has been drawn, the column come in by
    /// then; a frame without its column ends the dwell, the light on another
    /// of its rows keeps it going but never begins one, and a choice sees it
    /// at once.
    #[test]
    fn a_row_is_seen_once_frames_have_kept_it_on_the_screen_for_the_dwell() {
        use crate::ui::motion::FRAME;
        let mut seen = Seen::default();
        assert!(!seen.seen() && !seen.dwelling());
        // Opened, and not yet drawn: not seen, however late a press asks.
        seen.opened();
        assert!(!seen.seen() && seen.dwelling());
        // Drawn sixty times a second from 10.0: the dwell runs from the
        // second frame, and the frame that counts it is the one drawn
        // OPENING after that — not one a hundredth of a second before.
        let mut now = 10.0;
        seen.drawn(Showing::Row, now, FRAME, false);
        assert!(matches!(seen, Seen::Drawn { since: None, .. }), "{seen:?}");
        now += FRAME;
        seen.drawn(Showing::Row, now, FRAME, false);
        let since = now;
        assert!(matches!(seen, Seen::Drawn { since: Some(at), .. } if at == since));
        while now + FRAME < since + OPENING - 0.01 {
            now += FRAME;
            seen.drawn(Showing::Row, now, FRAME, false);
        }
        assert!(seen.dwelling() && !seen.seen(), "{now}: {seen:?}");
        // The light walked to another place in the column: still going.
        seen.drawn(Showing::Column, since + OPENING + 0.001, FRAME, false);
        assert_eq!(seen, Seen::Yes);
        // Seen stays seen, opened again or shut, until forgotten.
        seen.opened();
        seen.shut();
        seen.drawn(Showing::Not, 11.0, FRAME, false);
        assert_eq!(seen, Seen::Yes);
        seen.forget();
        assert_eq!(seen, Seen::Not);
        // A frame drawn without the column, or the column shutting, before
        // the dwell has run: nothing was seen, and it begins again.
        seen.opened();
        seen.drawn(Showing::Row, 20.0, FRAME, false);
        seen.drawn(Showing::Not, 20.1, FRAME, false);
        assert_eq!(seen, Seen::Not);
        seen.opened();
        seen.shut();
        assert_eq!(seen, Seen::Not);
        // The column standing open with the light on another of its rows
        // begins nothing — not even as the first frame after it opened on
        // the row, the light stepped off it before that frame — and the
        // light coming onto the row in force does.
        seen.opened();
        seen.drawn(Showing::Column, 29.0, FRAME, false);
        assert_eq!(seen, Seen::Not, "opened, and stepped off before drawn");
        seen.drawn(Showing::Column, 30.0, FRAME, false);
        assert_eq!(seen, Seen::Not);
        seen.drawn(Showing::Row, 30.0 + FRAME, FRAME, false);
        assert!(seen.dwelling(), "{seen:?}");
        // In low-end mode the column is whole at once: 0.3 s from the
        // second frame.
        let mut seen = Seen::default();
        seen.opened();
        seen.drawn(Showing::Row, 40.0, FRAME, true);
        seen.drawn(Showing::Row, 40.0 + FRAME, FRAME, true);
        seen.drawn(Showing::Row, 40.0 + FRAME + 0.29, FRAME, true);
        assert!(seen.dwelling(), "{seen:?}");
        seen.drawn(Showing::Row, 40.0 + FRAME + 0.31, FRAME, true);
        assert_eq!(seen, Seen::Yes);
        // A choice is seen at once.
        let mut seen = Seen::default();
        seen.see();
        assert!(seen.seen() && !seen.dwelling());
    }

    /// Frames drawn far apart — a stall, a slow first frame — never count the
    /// row seen sooner than frames drawn as often as the window draws them:
    /// the dwell runs from the second frame whatever the first took, and the
    /// column has to be carried in by frames, each a tenth of a second of its
    /// motion at most. Half a second apart, the column is seen on the
    /// seventh frame (on the third in low-end mode, where it is whole on the
    /// first), and not on one of the six before it however long they stood.
    #[test]
    fn frames_far_apart_count_the_row_seen_no_sooner() {
        for (low_end, counted) in [(false, 7), (true, 3)] {
            let mut seen = Seen::default();
            seen.opened();
            let mut dt = crate::ui::motion::FRAME;
            for frame in 1..=counted {
                seen.drawn(Showing::Row, 10.0 + 0.5 * frame as f32, dt, low_end);
                dt = 0.1;
                if frame < counted {
                    assert!(seen.dwelling(), "low-end {low_end}, frame {frame}");
                }
            }
            assert_eq!(seen, Seen::Yes, "low-end {low_end}");
        }
    }

    /// The trail says whether the light has walked in the second column since
    /// it opened: a step, or the pointer moving it to another row, once a
    /// frame has drawn the column; not before that, nor the pointer on the
    /// row it was on, nor anything in the first column, and opening it again
    /// starts over.
    #[test]
    fn the_trail_tells_whether_the_light_walked_in_the_second_column() {
        let (first, second) = (rows(3), chosen(6, 2));
        let mut trail = Trail::default();
        trail.start(&first);
        trail.act(Action::Down, &first, &second);
        trail.drawn();
        assert!(!trail.walked(), "a step in the first column");
        trail.act(Action::Accept, &first, &second);
        assert!(trail.open && !trail.walked());
        trail.act(Action::Down, &first, &second);
        assert!(trail.point_at(1, 4, &second));
        trail.act(Action::Up, &first, &second);
        assert!(!trail.walked(), "steps before a frame drew the column");
        trail.drawn();
        assert!(trail.point_at(1, 3, &second));
        assert!(!trail.walked(), "the pointer on the row it was on");
        trail.act(Action::Down, &first, &second);
        assert!(trail.walked());
        trail.act(Action::Back, &first, &second);
        trail.act(Action::Accept, &first, &second);
        assert!(!trail.walked(), "opened again");
        trail.drawn();
        assert!(trail.point_at(1, 4, &second));
        assert!(trail.walked(), "the pointer on another row");
    }
}
