//! A scrolled column of rows: languages, time zones, layouts, disks.
//!
//! The toolkit has no list, and its flow does not scroll, so this is the
//! family's own column as DistriBumpy draws its listing:
//!
//! * **one column, never pages** — the family's rule for a long list;
//! * scrolled on the toolkit's card spring, keeping the row in the light in
//!   view with a little of the next one showing;
//! * cut at its own edges with `Ui::cut_between`, so a row half out of view is
//!   half drawn rather than drawn whole over the head or the legend, and faded
//!   away at an end that really continues with `Ui::soft_edges`;
//! * a scroll bar beside it for a pointer, and none for a pad — whose light is
//!   already saying where in the list it is — with its channel kept either
//!   way, so the rows do not re-flow when somebody puts the pad down;
//! * two lines a row (`ui.row`'s shape: a name, and its state under it), a
//!   mark before the name where the row has one — or a language's flag, as
//!   tall as the two lines together (`super::flags`) — and the `chosen`
//!   mark at the end of the row whose value is in force — and **a row grows
//!   to its words**: a name too long for its line takes a second, and a note
//!   too long for its line (a refusal in a long language, a narrow panel) a
//!   second and a third, and the row is as tall as they need, rather than
//!   the words being cut;
//! * **one column, one shape** ([`Corners`]): a one-line row is a capsule
//!   and a grown one a card, but a column wears one of the two on every row
//!   — cards, all of them, once any row has grown — and the light on any of
//!   them the same, rather than a card among capsules wherever a name
//!   happened to wrap;
//! * a row that cannot be chosen is outlined rather than filled, says why
//!   under its name, and the light walks past it;
//! * **hovering is selecting**, as in LineXinBar: the light goes to the row a
//!   pointer moves onto, and a click acts on the row it lands on at once —
//!   and a row reached by the pointer is not scrolled into view, which would
//!   move the very row being pointed at out from under the pointer (the
//!   shell's friends list keeps the same rule); the next direction pressed
//!   brings it into view, and so does a key that brings the light back to
//!   the column after it left.

use lxb_app::lxb_render::{Align, Press, Spot};
use lxb_app::lxb_toolkit::{metrics::Metric, motion, palette::Role, typography::Text};
use lxb_app::Page;

use super::motion::{Carried, Light, Shape};
use super::spots;
use super::text;

/// One row of a column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Row {
    /// The name, in Body: a language's own name, a city, a disk.
    pub name: String,
    /// The line under it, in Caption: where it is, what it holds, or — for a
    /// row that cannot be chosen — why not.
    pub note: Option<String>,
    /// A mark before the name.
    pub mark: Option<&'static str>,
    /// A flag before the name (`super::flags`), as tall as the name and the
    /// line under it together: a language's country's.
    pub flag: Option<&'static str>,
    /// The value in force wears the `chosen` mark at the end of its row.
    pub chosen: bool,
    /// Whether the row can be chosen at all.
    pub enabled: bool,
    /// Whether the note is a refusal, said in the full white of a refusal
    /// rather than quietly.
    pub refused: bool,
    /// Whether the row steps back into the page while the light is
    /// elsewhere, as a resting button does. For a row standing alone among
    /// buttons: left bright beside them it reads as lit whether it is or
    /// not. A column's rows do not, because the light walking them already
    /// says which one is in hand.
    pub rests: bool,
    /// A switch at the end of the row, and whether it is on: a row of a
    /// list of things each kept or left out (software, apps), which Accept
    /// flips.
    pub switch: Option<bool>,
    /// A heading over the rows after it (a group of software): its name in
    /// the quiet small type of a column's own heading, on no glass, and never
    /// a place the light stops — so it is built not `enabled`.
    pub heading: bool,
}

impl Row {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            note: None,
            mark: None,
            flag: None,
            chosen: false,
            enabled: true,
            refused: false,
            rests: false,
            switch: None,
            heading: false,
        }
    }

    /// A heading over the rows after it; see [`Row::heading`].
    pub fn heading(name: impl Into<String>) -> Self {
        Self {
            heading: true,
            enabled: false,
            ..Self::new(name)
        }
    }

    /// A switch at the end of the row, on or off.
    pub fn switch(mut self, on: bool) -> Self {
        self.switch = Some(on);
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn mark(mut self, mark: &'static str) -> Self {
        self.mark = Some(mark);
        self
    }

    /// The flag before the name; see [`Row::flag`].
    pub fn flag(mut self, flag: &'static str) -> Self {
        self.flag = Some(flag);
        self
    }

    pub fn chosen(mut self, chosen: bool) -> Self {
        self.chosen = chosen;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The note says why the row's value cannot be used.
    pub fn refused(mut self, refused: bool) -> Self {
        self.refused = refused;
        self
    }

    pub fn rests(mut self) -> Self {
        self.rests = true;
        self
    }
}

/// What the pointer did to a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// A hand on the scroll bar carried the light to this row. Sounds like a
    /// move.
    Moved(usize),
    /// A click landed on this row. The light has not been moved: the screen
    /// points at the row as a hover would and acts on it as Accept would, so
    /// a click and a press of South reach the same one place.
    Clicked(usize),
}

/// One column's own state: which row the light is on, and how far it is
/// scrolled. The rows themselves are the screen's, handed in every frame.
#[derive(Debug, Default, Clone)]
pub struct Column {
    /// The row the light is on (or would be, while the light is elsewhere).
    pub at: usize,
    scroll: Carried,
    /// Where the bar was drawn last, so a drag can be measured against it.
    track: [f32; 4],
    run: f32,
    /// Where each row stands, as last laid out, and what it was laid out
    /// for. See [`Laid`].
    laid: Laid,
    /// Whether the light was put where it is by the pointer, which holds
    /// the scroll still until a direction moves the light again.
    pointed: bool,
    /// Whether the light was elsewhere when the column was last drawn.
    away: bool,
}

/// Where each row of a column stands, kept from one frame to the next with
/// what it was worked out from.
///
/// How tall a row is depends on how many lines its name and note break
/// into, and breaking them asks the measurer about every word — for every
/// row of a list of hundreds of cities, on every frame while anything
/// moves, on a handheld. The rows rarely change between two frames, so they
/// are laid out again only when they, the width or the size of the type do.
#[derive(Debug, Default, Clone)]
struct Laid {
    /// A fingerprint of the rows, the width and the scale they were laid out
    /// at.
    key: u64,
    spans: Vec<Span>,
}

/// How a column is drawn this frame.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// Which numbers its rows answer to.
    pub ids: spots::Column,
    /// How far through a press the page is, for the row in the light.
    pub pressing: Option<f32>,
    /// How long this frame counts for.
    pub dt: f32,
    /// Whether a pad is in hand, in which case no bar is drawn.
    pub pad: bool,
    /// The part of the screen the column may show in: the panel's room.
    /// Wider than the column on most screens; narrower where the column is
    /// sliding out of view beside another, and then only what is inside
    /// takes a click or is faded at its ends.
    pub bounds: [f32; 4],
    /// How its rows are rounded: by the column's words, or cards whatever
    /// they say.
    pub corners: Corners,
}

/// How the rows of a column are rounded.
///
/// A row by itself is a capsule while it is one line tall and a card once
/// it has grown ([`radius`]), so that a tall row is not drawn as a pill. A
/// column rounding each of its rows so by its own height would mix the
/// two: on a display standing on its side "Example NVMe SSD · 512 GB" takes
/// a second line and would be a card among the other disks' capsules. So
/// every row of a column wears one shape, and the light on any of them that
/// shape too ([`shape`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Corners {
    /// Capsules while every row of the column is one line tall, and cards,
    /// every row of it, once any has grown — a place whose words wrap in a
    /// long language on a narrow display makes the whole column cards, not
    /// that row alone. Which of the two is decided by all the column's rows,
    /// not the ones in view, so the rows do not change shape as the column
    /// scrolls.
    #[default]
    Fitted,
    /// Cards, every row, however tall its words make it: lit or not, chosen
    /// or not, outlined (a row that cannot be chosen) or not, at every
    /// display size. Where to install's disks, by design.
    Cards,
}

/// Whether a row `tall` high has grown past the one line of a row of the
/// design language, `row_h` tall: a little past, so a row a hair taller for
/// the rounding of its words is still one line.
fn grown(tall: f32, row_h: f32) -> bool {
    tall > row_h * 1.05
}

/// The one shape every row of a column wears, and the light on it: a card
/// where the column asks for cards ([`Corners::Cards`]) or any of its
/// `rows` but a heading — which is words on no glass — was laid out at
/// `spans` taller than one line of a row `row_h` tall; else a capsule.
pub fn shape(corners: Corners, rows: &[Row], spans: &[Span], row_h: f32) -> Shape {
    let any_grown = rows
        .iter()
        .zip(spans)
        .any(|(row, span)| !row.heading && grown(span.tall, row_h));
    if corners == Corners::Cards || any_grown {
        Shape::Card
    } else {
        Shape::Capsule
    }
}

/// Where one thing of a list stands, from the list's top: its top and how
/// tall it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub top: f32,
    pub tall: f32,
}

/// Where a list starts to show, `view` tall, with `span` kept in view and
/// up to `context` of its neighbours showing, in a list `content` tall: the
/// scroll is moved only as far as it has to be from where it `was`.
pub fn keep_span_in_view(was: f32, span: Span, context: f32, content: f32, view: f32) -> f32 {
    let most = (content - view).max(0.0);
    let context = context.min(((view - span.tall) * 0.5).max(0.0));
    let bottom = span.top + span.tall;
    let mut wanted = was;
    if span.top - context < wanted {
        wanted = span.top - context;
    }
    if bottom + context > wanted + view {
        wanted = bottom + context - view;
    }
    // Taller than the view: its top, where its words begin.
    if span.top < wanted {
        wanted = span.top;
    }
    wanted.clamp(0.0, most)
}

/// The scroll bar's width and the channel between it and the rows: what a
/// column keeps at its right-hand side whether a bar is drawn or not.
pub fn bar_room(page: &mut Page) -> (f32, f32) {
    (page.ui().scroll_bar_width(), page.metric(Metric::Gap) * 0.6)
}

/// Where a column at most `widest` wide stands in `room`: in its middle, as
/// [`super::screens::centred`] stands a block — measured by its rows, not by
/// the channel kept for its scroll bar, so rows with no bar beside them
/// stand exactly in the middle and a bar, where one is drawn, beside them —
/// and never past the room's right-hand edge.
pub fn centred(page: &mut Page, room: [f32; 4], widest: f32) -> [f32; 4] {
    let (rail, channel) = bar_room(page);
    centred_at(room, widest, rail + channel)
}

/// [`centred`] with the bar's room given: `kept` is what the column keeps
/// at its right-hand side beside its rows.
pub fn centred_at(room: [f32; 4], widest: f32, kept: f32) -> [f32; 4] {
    let width = room[2].min(widest).max(0.0);
    let spare = room[2] - width;
    let left = ((spare + kept) * 0.5).min(spare).max(0.0);
    [room[0] + left, room[1], width, room[3]]
}

/// The most lines a row's note is given before what is left is cut: four,
/// which is what the longest refusal under a place on a disk takes, in
/// German, Polish and Russian, on a display standing on its side, beside the
/// place's mark.
pub const NOTE_LINES: usize = 4;

/// The most lines a row's name is given: a second where it does not fit its
/// line ("Replace Windows · 400 GB" in a column on a display standing on its
/// side), before what is left is cut — which only a name the machine gave,
/// a disk's model or a city, ever needs.
pub const NAME_LINES: usize = 2;

/// How much room a row `width` wide leaves its words: the row's padding,
/// and the mark or the flag at its start, its switch and its `chosen` mark
/// at its end where it has them.
pub fn words_room(page: &Page, width: f32, row: &Row) -> f32 {
    words_room_at(page.scaled(1.0), width, row)
}

/// [`words_room`] at the design language's `scale`, with no page: what the
/// catalogs' test measures every row's words against.
pub fn words_room_at(scale: f32, width: f32, row: &Row) -> f32 {
    let pad = Metric::RowPadding.value() * scale;
    let gap = Metric::Gap.value() * scale;
    let mut room = width - pad * 2.0;
    if row.mark.is_some() {
        room -= Metric::ItemIcon.value() * scale * 0.6 + gap;
    }
    if row.flag.is_some() {
        room -= super::flags::width_at(scale) + gap;
    }
    if row.switch.is_some() {
        room -= super::toggle::track_width_at(scale) + gap;
    }
    if row.chosen {
        room -= CHOSEN * scale + gap;
    }
    room.max(0.0)
}

/// How wide the rows of a column drawn in a view `width` wide are: the
/// view less the scroll bar's channel, kept whether a bar is drawn or not.
pub fn body_width_at(scale: f32, width: f32) -> f32 {
    (width - SCROLL_BAR * scale - Metric::Gap.value() * scale * 0.6).max(1.0)
}

/// `Ui::scroll_bar_width` at the reference height, for [`body_width_at`],
/// which has no page to ask.
const SCROLL_BAR: f32 = 8.0;

/// The lines a row's note is drawn on in a row `width` wide: one where it
/// fits, else as many as it wraps to, up to `most`.
pub fn note_lines(page: &mut Page, width: f32, row: &Row, most: usize) -> Vec<String> {
    let Some(note) = row.note.as_deref() else {
        return Vec::new();
    };
    let room = words_room(page, width, row);
    text::wrapped(page.ui(), Text::Caption, note, room, most.max(1))
}

/// The lines a row's name is drawn on in a row `width` wide: one where it
/// fits, else two, the second cut where even that is not enough.
pub fn name_lines(page: &mut Page, width: f32, row: &Row) -> Vec<String> {
    let room = words_room(page, width, row);
    text::wrapped(page.ui(), Text::Body, &row.name, room, NAME_LINES)
}

/// How tall a row `width` wide is: a row of the design language, and a line
/// more for each line its name and its note take past the first.
pub fn row_height(page: &mut Page, width: f32, row: &Row) -> f32 {
    let row_h = page.metric(Metric::RowHeight);
    if row.heading {
        return row_h;
    }
    let names = name_lines(page, width, row).len();
    let notes = note_lines(page, width, row, NOTE_LINES).len();
    row_h
        + names.saturating_sub(1) as f32 * page.line(Text::Body)
        + notes.saturating_sub(1) as f32 * page.line(Text::Caption)
}

/// A row's corners, where it stands alone rather than in a column (a
/// switch among a form's fields): a capsule's while it is one line tall, a
/// card's once it has grown, so a tall row is not drawn as a pill — and the
/// light on it is the same shape. A column's rows are rounded all alike
/// instead ([`Corners`]).
pub fn radius(page: &Page, rect: [f32; 4]) -> (f32, Shape) {
    let shape = if grown(rect[3], page.metric(Metric::RowHeight)) {
        Shape::Card
    } else {
        Shape::Capsule
    };
    (shape.radius(page, rect), shape)
}

/// How large the `chosen` mark at the end of a row is, in reference pixels.
const CHOSEN: f32 = 30.0;

/// The next row from `from` in `direction` that can be chosen, or `None`.
pub fn next_enabled(rows: &[Row], from: usize, direction: isize) -> Option<usize> {
    let mut at = from as isize + direction;
    while at >= 0 && (at as usize) < rows.len() {
        if rows[at as usize].enabled {
            return Some(at as usize);
        }
        at += direction;
    }
    None
}

/// The row that can be chosen nearest to `at`, preferring the one after.
pub fn nearest_enabled(rows: &[Row], at: usize) -> Option<usize> {
    if rows.get(at).is_some_and(|row| row.enabled) {
        return Some(at);
    }
    let at = at.min(rows.len().saturating_sub(1));
    (0..rows.len()).find_map(|distance| {
        [at + distance, at.wrapping_sub(distance)]
            .into_iter()
            .find(|index| rows.get(*index).is_some_and(|row| row.enabled))
    })
}

impl Column {
    /// The distance from one row's top to the next one's.
    pub fn pitch(page: &Page) -> f32 {
        page.metric(Metric::RowHeight) + page.metric(Metric::Gap) * 0.35
    }

    /// Walk the light `direction` rows (±1), past any that cannot be chosen.
    /// Answers whether it moved, which is whether a Move is heard.
    pub fn step(&mut self, rows: &[Row], direction: isize) -> bool {
        match next_enabled(rows, self.at, direction) {
            Some(next) => {
                self.at = next;
                self.pointed = false;
                true
            }
            None => false,
        }
    }

    /// Put the light on `index` if that row can be chosen.
    pub fn select(&mut self, rows: &[Row], index: usize) -> bool {
        if rows.get(index).is_some_and(|row| row.enabled) {
            self.at = index;
            self.pointed = false;
            true
        } else {
            false
        }
    }

    /// Put the light on `index`, a row the pointer moved onto or clicked, if
    /// that row can be chosen — without scrolling to it: the pointer is on a
    /// row it can see, and a list that moved to show more of it would move
    /// it out from under the pointer, which on a long list walks the light
    /// down the list on its own. Answers whether the light is on that row.
    pub fn point_at(&mut self, rows: &[Row], index: usize) -> bool {
        if !rows.get(index).is_some_and(|row| row.enabled) {
            return false;
        }
        self.at = index;
        self.pointed = true;
        self.away = false;
        true
    }

    /// Keep the light on a row that exists and can be chosen, after the rows
    /// have changed under it. Answers whether there is any such row.
    pub fn fix(&mut self, rows: &[Row]) -> bool {
        match nearest_enabled(rows, self.at) {
            Some(at) => {
                self.at = at;
                true
            }
            None => false,
        }
    }

    /// Start at the row whose value is in force, or the first that can be
    /// chosen, and stand the scroll there at once.
    pub fn start_at_chosen(&mut self, rows: &[Row]) {
        self.at = rows
            .iter()
            .position(|row| row.chosen && row.enabled)
            .or_else(|| nearest_enabled(rows, 0))
            .unwrap_or(0);
        self.scroll = Carried::default();
        self.pointed = false;
    }

    /// Start at the first row that can be chosen, the scroll at the top at
    /// once: a list a search has just narrowed, whose rows are other rows.
    pub fn start_at_first(&mut self, rows: &[Row]) {
        self.at = nearest_enabled(rows, 0).unwrap_or(0);
        self.scroll = Carried::default();
        self.pointed = false;
    }

    /// Whether the pointer put the light where it is, so a list scrolled
    /// by its screen holds still too.
    pub fn pointed(&self) -> bool {
        self.pointed
    }

    /// Say whether the light is in the column on this frame. While it is
    /// away the scroll still holds where the pointer left it — a key that
    /// takes the light to Continue does not move the list under a resting
    /// pointer — but a light coming back other than by the pointer (a key
    /// with no step inside the column: Continue, then Left) scrolls to its
    /// row again. Said by drawing the column, or by a screen that scrolls
    /// the column itself.
    pub fn light_is_here(&mut self, here: bool) {
        if !here {
            self.away = true;
        } else if std::mem::take(&mut self.away) {
            self.pointed = false;
        }
    }

    /// Put the scroll where it is going, for a photograph.
    pub fn settle(&mut self) {
        self.scroll.settle();
    }

    /// Whether the column is still scrolling, and so wants another frame.
    pub fn moving(&self) -> bool {
        self.scroll.moving()
    }

    /// Draw the column in `view`, laying the light on its row first when
    /// `light` is handed in — which is how a screen says the light is here.
    pub fn draw(
        &mut self,
        page: &mut Page,
        view: [f32; 4],
        rows: &[Row],
        light: Option<&mut Light>,
        look: Look,
    ) {
        self.light_is_here(light.is_some());
        let row_h = page.metric(Metric::RowHeight);
        let pitch = Self::pitch(page);
        let (rail, channel) = bar_room(page);
        // The bar's channel is taken whether or not a bar is drawn: what a
        // page reserves must not depend on what the person last had in their
        // hands, or the rows would re-flow the moment a pad was put down.
        let body = [
            view[0],
            view[1],
            (view[2] - rail - channel).max(1.0),
            view[3],
        ];
        let track = [view[0] + view[2] - rail, view[1], rail, view[3]];
        // Each row as tall as its words need, a little space between them.
        let between = pitch - row_h;
        self.lay(page, body[2], rows, between);
        let spans = std::mem::take(&mut self.laid.spans);
        let content = spans
            .last()
            .map_or(0.0, |last| last.top + last.tall)
            .max(0.0);
        let most = (content - view[3]).max(0.0);

        let wanted = match spans.get(self.at) {
            Some(span) if !self.pointed => {
                keep_span_in_view(self.scroll.wanted(), *span, pitch * 0.75, content, view[3])
            }
            _ => self.scroll.wanted().clamp(0.0, most),
        };
        let top = self.scroll.advance(wanted, look.dt).clamp(0.0, most);
        // One shape for every row and the light, decided by all of them.
        let shape = shape(look.corners, rows, &spans, row_h);

        let seen = intersection(view, look.bounds);
        // The space between rows scrolls too: laid down before the rows, so a
        // row still wins wherever one is under the pointer.
        if let Some(behind) = intersection(body, look.bounds) {
            page.ui().spot(look.ids.viewport(), behind);
        }

        let from = page.ui().written();
        let row_at = |index: usize| {
            let span = spans[index];
            [body[0], view[1] + span.top - top, body[2], span.tall]
        };
        // Only the row the light is really on is drawn lit. A column the
        // light is not in draws its own row at rest: drawn lit, `Ui::control`
        // lays a light of its own under it (the real one not having been laid
        // yet this frame), and the page shows two.
        let here = light.is_some();
        if let Some(light) = light {
            if rows.get(self.at).is_some() {
                let rect = row_at(self.at);
                let sunk = motion::pressed(rect, look.pressing);
                light.lay(page, sunk, shape, look.pressing, look.dt);
            }
        }
        for (index, row) in rows.iter().enumerate() {
            let span = spans[index];
            // Only what shows, and a row either side of it.
            if span.top + span.tall + pitch < top || span.top > top + view[3] + pitch {
                continue;
            }
            let rect = row_at(index);
            let lit = here && index == self.at;
            draw_row(page, rect, row, lit, look.pressing, shape);
            if row.enabled {
                if let Some(target) = seen.and_then(|seen| pressable(rect, seen)) {
                    page.ui().spot(look.ids.row(index), target);
                }
            }
        }
        let to = page.ui().written();
        // Cut to the column's own top and bottom. Not to its sides: the light
        // carries a glow past its row's edge, and a glow squared off against
        // thin air is a worse fault than the one the cut is for.
        let air = row_h * 0.5;
        page.ui().cut_between(
            from,
            to,
            [view[0] - air, view[1], view[2] + air * 2.0, view[3]],
        );

        let band = row_h * 0.55;
        if let Some(faded) = intersection(body, look.bounds) {
            page.ui().soft_edges(
                faded,
                band,
                (top / band).min(1.0),
                ((most - top) / band).min(1.0),
            );
        }

        self.laid.spans = spans;
        // Within half a point: the track's right edge is the view's, reached
        // by adding the bar's width back on, and that sum came out a hair
        // short of the bounds often enough to lose the bar on wide panels.
        let bar_in_view =
            intersection(track, look.bounds).is_some_and(|seen| seen[2] >= track[2] - 0.5);
        let scrolled = Scrolled {
            top,
            content,
            shown: view[3],
        };
        self.draw_bar(page, look.ids, track, scrolled, look.pad || !bar_in_view);
    }

    /// Draw the column's scroll bar at `track`, where the list goes on past
    /// what is shown and no pad is in hand (`hidden`), and note where, so a
    /// hand on it walks the light along (`Column::pressed`). Also for a
    /// screen that lays its rows out itself, as Review does.
    pub fn draw_bar(
        &mut self,
        page: &mut Page,
        ids: spots::Column,
        track: [f32; 4],
        scrolled: Scrolled,
        hidden: bool,
    ) {
        let Scrolled {
            top,
            content,
            shown,
        } = scrolled;
        self.track = [0.0; 4];
        self.run = 0.0;
        if hidden || content <= shown {
            return;
        }
        let held = page.dragging(ids.bar()).is_some();
        let reach = page.metric(Metric::Gap) * 0.6;
        let ui = page.ui();
        ui.scroll_bar(track, top / content, shown / content, held);
        ui.spot(
            ids.bar(),
            [track[0] - reach, track[1], track[2] + reach * 2.0, track[3]],
        );
        self.track = track;
        self.run = shown / content;
    }

    /// Lay `rows` out `width` wide, `between` apart, where they were not
    /// laid out for exactly these on an earlier frame.
    fn lay(&mut self, page: &mut Page, width: f32, rows: &[Row], between: f32) {
        use std::hash::{Hash, Hasher};
        let mut print = std::collections::hash_map::DefaultHasher::new();
        width.to_bits().hash(&mut print);
        page.scaled(1.0).to_bits().hash(&mut print);
        between.to_bits().hash(&mut print);
        rows.hash(&mut print);
        let key = print.finish();
        if key == self.laid.key && self.laid.spans.len() == rows.len() {
            return;
        }
        let mut spans = Vec::with_capacity(rows.len());
        let mut y = 0.0;
        for row in rows {
            let tall = row_height(page, width, row);
            spans.push(Span { top: y, tall });
            y += tall + between;
        }
        self.laid = Laid { key, spans };
    }

    /// What the pointer did to this column, if it landed on it: a hand on
    /// the bar, or a click on a row (which leaves the light where it is for
    /// the screen to move).
    pub fn pressed(&mut self, page: &mut Page, rows: &[Row], ids: spots::Column) -> Option<Pick> {
        // A hand on the bar: the light follows it, and with the light the
        // list. Taken before anything else, because while a bar is held it
        // owns the pointer wherever the pointer has wandered.
        let grabbed = page.pressed(ids.bar());
        if let Some([_, y]) = page.dragging(ids.bar()) {
            if self.track[3] > 0.0 && !rows.is_empty() {
                let wanted = row_under_bar(y, self.track, self.run, rows.len());
                if let Some(at) = nearest_enabled(rows, wanted) {
                    let moved = at != self.at;
                    self.at = at;
                    self.pointed = false;
                    return moved.then_some(Pick::Moved(at));
                }
            }
            return None;
        }
        if grabbed {
            return None;
        }
        let _ = page.pressed(ids.viewport());
        let mut found = None;
        for (index, row) in rows.iter().enumerate() {
            if page.pressed(ids.row(index)) && row.enabled && found.is_none() {
                found = Some(index);
            }
        }
        found.map(Pick::Clicked)
    }

    /// Whether a wheel turned over `spot` was turned over this column.
    pub fn owns(spot: Spot, ids: spots::Column) -> bool {
        matches!(spot, Spot::Control(id) if ids.owns(id))
    }
}

/// Where two rectangles overlap, if they do.
pub fn intersection(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    let w = (a[0] + a[2]).min(b[0] + b[2]) - x;
    let h = (a[1] + a[3]).min(b[1] + b[3]) - y;
    (w > 0.0 && h > 0.0).then_some([x, y, w, h])
}

/// How far a list is scrolled, for its scroll bar: `top` of its `content`
/// scrolled out of view, `shown` of it in view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scrolled {
    pub top: f32,
    pub content: f32,
    pub shown: f32,
}

/// Which of `count` rows a hand at `y` on a scroll bar drawn at `track`, its
/// thumb `run` of it long, stands for: the thumb at the top of the track the
/// first, at its foot the last. A column's bar and You's form's both read a
/// hand so.
pub fn row_under_bar(y: f32, track: [f32; 4], run: f32, count: usize) -> usize {
    let usable = (track[3] * (1.0 - run)).max(1.0);
    let share = ((y - track[1] - track[3] * run * 0.5) / usable).clamp(0.0, 1.0);
    (share * count.saturating_sub(1) as f32).round() as usize
}

/// How much of a row a pointer can land on, where enough of it is showing to
/// be worth landing on at all.
fn pressable(rect: [f32; 4], view: [f32; 4]) -> Option<[f32; 4]> {
    let seen = intersection(rect, view)?;
    (seen[3] > rect[3] * 0.6 && seen[2] > rect[2] * 0.4).then_some(seen)
}

/// One row: its chip (or its outline) in `shape`, its mark or its flag, its
/// name and its note's lines under it, and its `chosen` mark. The note takes
/// as many lines as it wraps to and `rect` has room for, up to
/// [`NOTE_LINES`].
pub fn draw_row(
    page: &mut Page,
    rect: [f32; 4],
    row: &Row,
    lit: bool,
    pressing: Option<f32>,
    shape: Shape,
) {
    let pad = page.metric(Metric::RowPadding);
    let gap = page.metric(Metric::Gap);
    let name_line = page.line(Text::Body);
    let note_line = page.line(Text::Caption);
    let mark_size = page.metric(Metric::ItemIcon) * 0.6;
    let flag_w = super::flags::width_at(page.scaled(1.0));
    let chosen_size = page.scaled(CHOSEN);
    let icons = page.icons();
    let press = match pressing {
        Some(through) if lit => Press::Going(through),
        _ => Press::from(lit),
    };
    let radius = shape.radius(page, rect);
    let names = if row.heading {
        Vec::new()
    } else {
        name_lines(page, rect[2], row)
    };
    let names_tall = name_line * names.len().max(1) as f32;
    let room_for = (((rect[3] - page.metric(Metric::RowHeight) - (names_tall - name_line))
        / note_line)
        .round()
        .max(0.0) as usize)
        + 1;
    let lines = note_lines(page, rect[2], row, room_for.min(NOTE_LINES));
    let ui = page.ui();
    if row.heading {
        // Stood on the foot of its row, so it reads as belonging to the rows
        // under it rather than floating between two groups.
        let line = note_line * 1.1;
        text::label_faded(
            ui,
            [
                rect[0] + pad * 0.5,
                rect[1] + rect[3] - line * 1.2,
                rect[2] - pad,
                line,
            ],
            Text::Caption,
            &row.name,
            text::LESSER.0,
            text::LESSER.1,
            Align::Left,
        );
        return;
    }
    let sunk = if row.enabled {
        ui.control(rect, radius, press, rect[2], 1.0)
    } else {
        ui.control_out(rect, radius, 1.0);
        rect
    };
    if row.rests && row.enabled && !lit {
        ui.chip(sunk, ui.tinted(Role::Glass, super::button::RESTING_WASH));
    }
    let [x, y, w, h] = sunk;
    let ink = if !row.enabled {
        0.55
    } else if lit {
        1.0
    } else {
        0.86
    };
    let mut left = x + pad;
    if let Some(mark) = row.mark {
        ui.icon_tinted(
            [left, y + (h - mark_size) * 0.5, mark_size, mark_size],
            mark,
            icons,
            Role::Text,
            ink,
        );
        left += mark_size + gap;
    }
    // A flag is as tall as the name and the line under it together, standing
    // in the middle of the row as they do — as tall on a row whose name says
    // all there is on one line, so the flags of a column stand as one. In
    // its own colours on a row at rest too, as the `chosen` mark is: the
    // words' quieter ink let the row's violet into it, and Germany's black
    // read as plum.
    let mut flag_coming = false;
    if let Some(flag) = row.flag {
        let flag_h = name_line + note_line;
        let rect = [left, y + (h - flag_h) * 0.5, flag_w, flag_h];
        let alpha = if row.enabled { 1.0 } else { 0.55 };
        flag_coming = super::flags::draw(ui, rect, flag, alpha);
        left += flag_w + gap;
    }
    let mut right = x + w - pad;
    if let Some(on) = row.switch {
        let track = super::toggle::switch(ui, sunk, pad, on, ink, icons);
        right = track[0] - gap;
    }
    if row.chosen {
        super::mark::chosen(
            ui,
            [
                right - chosen_size,
                y + (h - chosen_size) * 0.5,
                chosen_size,
                chosen_size,
            ],
            icons,
            if row.enabled { 1.0 } else { 0.55 },
        );
        right -= chosen_size + gap;
    }
    let room = (right - left).max(0.0);
    let top = y + (h - names_tall - note_line * lines.len() as f32) * 0.5;
    for (index, name) in names.iter().enumerate() {
        text::label_faded(
            ui,
            [left, top + index as f32 * name_line, room, name_line],
            Text::Body,
            name,
            Role::Text,
            ink,
            Align::Left,
        );
    }
    let top = top + names_tall - name_line;
    let quiet = if row.enabled { 1.0 } else { 0.85 };
    let (role, alpha) = if row.refused {
        text::REFUSAL
    } else {
        text::LESSER
    };
    for (index, line) in lines.iter().enumerate() {
        text::label_faded(
            ui,
            [
                left,
                top + name_line + index as f32 * note_line,
                room,
                note_line,
            ],
            Text::Caption,
            line,
            role,
            alpha * quiet,
            Align::Left,
        );
    }
    // A flag still being decoded comes on a frame of its own.
    if flag_coming {
        page.redraw_within(std::time::Duration::from_millis(16));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(enabled: &[bool]) -> Vec<Row> {
        enabled
            .iter()
            .enumerate()
            .map(|(index, on)| Row::new(format!("row {index}")).enabled(*on))
            .collect()
    }

    #[test]
    fn the_light_walks_past_rows_that_cannot_be_chosen() {
        let list = rows(&[true, false, false, true, false]);
        let mut column = Column::default();
        assert!(column.step(&list, 1));
        assert_eq!(column.at, 3);
        assert!(!column.step(&list, 1), "walked onto a disabled last row");
        assert_eq!(column.at, 3);
        assert!(column.step(&list, -1));
        assert_eq!(column.at, 0);
        assert!(!column.select(&list, 2));
    }

    #[test]
    fn a_light_left_on_a_row_that_went_away_finds_the_nearest_one() {
        let list = rows(&[false, true, false, true]);
        let mut column = Column {
            at: 9,
            ..Column::default()
        };
        assert!(column.fix(&list));
        assert_eq!(column.at, 3);
        column.at = 2;
        assert!(column.fix(&list));
        assert_eq!(column.at, 3, "the one after is preferred");
        assert!(!Column::default().fix(&rows(&[false, false])));
    }

    #[test]
    fn a_row_pointed_at_holds_the_scroll_until_a_key_or_the_light_leaving() {
        let list = rows(&[true, true, true]);
        let mut column = Column::default();
        assert!(column.point_at(&list, 2));
        assert!(column.pointed());
        assert!(column.step(&list, -1));
        assert!(!column.pointed(), "a direction did not scroll to the light");
        assert!(column.point_at(&list, 2));
        // The light goes elsewhere: the scroll still holds.
        column.light_is_here(false);
        assert!(column.pointed());
        // A key brings it back without a step inside the column (Continue,
        // then Left): it scrolls to its row.
        column.light_is_here(true);
        assert!(!column.pointed());
        assert_eq!(column.at, 2);
        // Brought back by the pointer, it holds.
        column.light_is_here(false);
        assert!(column.point_at(&list, 1));
        column.light_is_here(true);
        assert!(column.pointed());
    }

    #[test]
    fn a_column_starts_on_the_value_in_force() {
        let mut list = rows(&[true, true, true]);
        list[2].chosen = true;
        let mut column = Column::default();
        column.start_at_chosen(&list);
        assert_eq!(column.at, 2);
    }

    #[test]
    fn a_tall_row_is_kept_wholly_in_view_as_a_short_one_is() {
        // Rows of 90, 145 (a note on three lines) and 90, 10 apart, in a
        // view of 200.
        let spans = [
            Span {
                top: 0.0,
                tall: 90.0,
            },
            Span {
                top: 100.0,
                tall: 145.0,
            },
            Span {
                top: 255.0,
                tall: 90.0,
            },
        ];
        let content = 345.0;
        let at_tall = keep_span_in_view(0.0, spans[1], 75.0, content, 200.0);
        assert!(
            at_tall <= 100.0 && 245.0 <= at_tall + 200.0,
            "the tall row is cut: {at_tall}"
        );
        assert_eq!(
            keep_span_in_view(at_tall, spans[1], 75.0, content, 200.0),
            at_tall,
            "a row in view moved the list"
        );
        let at_last = keep_span_in_view(0.0, spans[2], 75.0, content, 200.0);
        assert_eq!(at_last, content - 200.0, "past the end");
        // A row taller than the view keeps its top in view.
        let huge = Span {
            top: 100.0,
            tall: 400.0,
        };
        assert!(keep_span_in_view(0.0, huge, 75.0, 600.0, 200.0) <= 100.0);
    }

    /// Where a list of `count` rows `pitch` apart starts to show, `view`
    /// tall, with the row `at` kept in view: a list whose rows are all one
    /// height.
    fn keep_in_view(was: f32, at: usize, count: usize, pitch: f32, row: f32, view: f32) -> f32 {
        let content = (count as f32 * pitch - (pitch - row)).max(0.0);
        let span = Span {
            top: at as f32 * pitch,
            tall: row,
        };
        keep_span_in_view(was, span, pitch * 0.75, content, view)
    }

    #[test]
    fn the_scroll_moves_only_as_far_as_it_has_to() {
        // Rows 100 apart, 90 tall, a view of 400: four and a bit rows.
        let (pitch, row, view) = (100.0, 90.0, 400.0);
        assert_eq!(keep_in_view(0.0, 0, 50, pitch, row, view), 0.0);
        assert_eq!(
            keep_in_view(0.0, 2, 50, pitch, row, view),
            0.0,
            "in view already"
        );
        let down = keep_in_view(0.0, 4, 50, pitch, row, view);
        assert!(down > 0.0, "the row at the foot was left out of view");
        assert!(
            4.0 * pitch + row <= down + view,
            "the row is not wholly in view"
        );
        assert_eq!(
            keep_in_view(down, 4, 50, pitch, row, view),
            down,
            "a row in view moved the list"
        );
        let most = 50.0 * pitch - (pitch - row) - view;
        assert_eq!(
            keep_in_view(0.0, 49, 50, pitch, row, view),
            most,
            "past the end"
        );
        assert_eq!(keep_in_view(900.0, 0, 50, pitch, row, view), 0.0);
        assert_eq!(
            keep_in_view(0.0, 0, 2, pitch, row, view),
            0.0,
            "a short list scrolled"
        );
    }

    #[test]
    fn a_centred_column_stands_in_the_middle_by_its_rows() {
        let room = [100.0, 50.0, 1000.0, 400.0];
        let kept = 20.0;
        let view = centred_at(room, 600.0, kept);
        assert_eq!(view[2], 600.0, "stretched across the room");
        let rows_middle = view[0] + (view[2] - kept) * 0.5;
        assert_eq!(rows_middle, room[0] + room[2] * 0.5, "not in the middle");
        // Narrower than it would be, it fills the room and keeps inside it.
        let narrow = centred_at([100.0, 50.0, 500.0, 400.0], 600.0, kept);
        assert_eq!((narrow[0], narrow[2]), (100.0, 500.0));
        // Only a little narrower: the bar's room is not pushed past the edge.
        let close = centred_at([0.0, 0.0, 610.0, 400.0], 600.0, kept);
        assert_eq!(close[0] + close[2], 610.0);
    }

    /// Rows laid out one under another, each as tall as `talls` says, 10
    /// apart.
    fn laid(talls: &[f32]) -> (Vec<Row>, Vec<Span>) {
        let rows = rows(&vec![true; talls.len()]);
        let mut top = 0.0;
        let spans = talls
            .iter()
            .map(|tall| {
                let span = Span { top, tall: *tall };
                top += tall + 10.0;
                span
            })
            .collect();
        (rows, spans)
    }

    /// The height of one line of a row in these tests.
    const ONE_LINE: f32 = 90.0;

    #[test]
    fn a_column_with_one_grown_row_draws_every_row_as_a_card() {
        // A name on two lines among names on one: the whole column is cards,
        // and so the light on any row of it.
        let (list, spans) = laid(&[90.0, 128.0, 90.0, 90.0]);
        assert_eq!(shape(Corners::Fitted, &list, &spans, ONE_LINE), Shape::Card);
        // The grown row out of view at the foot of a long list counts as
        // much: the rows do not change shape as the list scrolls.
        let mut talls = vec![90.0; 40];
        talls[39] = 150.0;
        let (list, spans) = laid(&talls);
        assert_eq!(shape(Corners::Fitted, &list, &spans, ONE_LINE), Shape::Card);
    }

    #[test]
    fn a_column_of_one_line_rows_stays_capsules() {
        // A hair over one line for the rounding of its words is still one.
        let (list, spans) = laid(&[90.0, 90.0, 92.0]);
        assert_eq!(
            shape(Corners::Fitted, &list, &spans, ONE_LINE),
            Shape::Capsule
        );
        // A heading is words on no glass: it is not a row that grew.
        let (mut list, mut spans) = laid(&[90.0, 90.0]);
        list.insert(0, Row::heading("Games"));
        spans.insert(
            0,
            Span {
                top: -100.0,
                tall: 140.0,
            },
        );
        assert_eq!(
            shape(Corners::Fitted, &list, &spans, ONE_LINE),
            Shape::Capsule
        );
    }

    #[test]
    fn a_column_of_cards_is_cards_however_short_its_rows() {
        let (mut list, spans) = laid(&[90.0, 90.0, 90.0]);
        // Lit or not, chosen or not, outlined or not.
        list[1].chosen = true;
        list[2].enabled = false;
        assert_eq!(shape(Corners::Cards, &list, &spans, ONE_LINE), Shape::Card);
        assert_eq!(shape(Corners::Cards, &[], &[], ONE_LINE), Shape::Card);
    }

    #[test]
    fn a_hand_on_the_bar_stands_for_the_row_as_far_down_the_list() {
        let track = [0.0, 100.0, 8.0, 400.0];
        let run = 0.5;
        assert_eq!(row_under_bar(0.0, track, run, 6), 0, "above the track");
        assert_eq!(
            row_under_bar(200.0, track, run, 6),
            0,
            "the thumb at its top"
        );
        assert_eq!(
            row_under_bar(400.0, track, run, 6),
            5,
            "the thumb at its foot"
        );
        assert_eq!(row_under_bar(900.0, track, run, 6), 5, "below the track");
        assert_eq!(row_under_bar(300.0, track, run, 7), 3, "half way down");
    }
}
