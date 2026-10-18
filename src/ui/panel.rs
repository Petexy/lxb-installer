//! The one pane of glass the installer is drawn in.
//!
//! **The wallpaper fills the whole window all the time and the installer is a
//! panel standing on it**, centred, by design: not a pane
//! filling the window edge to edge the way a store or a music player is, but
//! one sheet of glass the size a dialog of this weight should be.
//!
//! The panel is sized in the design language's reference pixels — about 1280
//! by 860 at the 1080-pixel reference height — and scaled the way everything
//! in the family is, by the window's height: at 4K it is twice as big, at
//! 720p two thirds. It is never more than the window less a margin. A window
//! standing on its side keeps the size its height gives (the family's rule
//! for every program on a portrait display): the panel is as tall as the
//! scale says and as wide as the window allows, and what is inside it re-flows
//! — two columns that no longer fit side by side slide, as the Home menu's do.
//!
//! Inside the panel, top to bottom: a head (a mark, the screen's title and the
//! line under it), the room the screen draws in, the band its buttons stand
//! in, and a foot line that belongs to the button legend and to a note beside
//! it — or, where a narrow panel has no room for both on one line, a note on
//! lines of its own over the buttons.

use lxb_app::lxb_render::{Align, Drawn};
use lxb_app::lxb_toolkit::{
    material::Overlay, metrics::Metric, motion, palette::Role, typography::Text,
};
use lxb_app::Page;

use super::button;
use super::legend::{self, Hint, LaidPair};
use super::motion::{departing, Share, GONE_BY};
use super::text;

/// The panel's size at the reference height, in reference pixels.
pub const WIDTH: f32 = 1280.0;
pub const HEIGHT: f32 = 860.0;

/// The least wallpaper kept showing on every side, in reference pixels.
pub const MARGIN: f32 = 24.0;

/// Where the panel stands in a window `width` by `height` whose scale is
/// `scale` (the toolkit's `scale_for(height)`): centred, the reference size
/// scaled, and never more than the window less the margin.
pub fn pane_rect(width: f32, height: f32, scale: f32) -> [f32; 4] {
    let margin = MARGIN * scale;
    let w = (WIDTH * scale).min(width - 2.0 * margin).max(1.0);
    let h = (HEIGHT * scale).min(height - 2.0 * margin).max(1.0);
    [(width - w) * 0.5, (height - h) * 0.5, w, h]
}

/// What the panel has to make room for on this frame: the words whose
/// length decides how many lines the head and the foot take.
#[derive(Debug, Clone, Copy)]
pub struct Needs<'a> {
    /// The title.
    pub title: &'a str,
    /// Every title a step with chips can have, in the language spoken: the
    /// longest of them decides whether the chips stand beside the title or
    /// under the caption — on every step alike, so the chips do not move
    /// from one step to the next.
    pub titles: &'a [&'a str],
    /// The line under the title.
    pub caption: &'a str,
    /// Where in the flow this is, which takes the right-hand end of the head.
    pub place: Option<Place>,
    /// What the legend says, and whether it pictures a pad.
    pub hints: &'a [Hint],
    pub pad: bool,
    /// The line beside the legend.
    pub note: Option<&'a str>,
}

/// The most lines a caption or a note is given before what is left is cut.
const MOST_LINES: usize = 2;

/// Where everything in the panel goes on this frame.
///
/// **What does not fit re-flows rather than shrinks.** A narrow panel — a
/// window standing on its side keeps the size its height gives — has less
/// room for a caption beside the chips and for a note beside the legend, so
/// the caption takes a second line, and the note leaves the legend's line for
/// lines of its own over the buttons. Only past two lines is anything cut.
///
/// **The buttons and the legend never move.** The foot is one line, the
/// legend's, whether a legend is drawn or not and whether a note stands
/// beside it or not, and the band the buttons stand in is a gap above it: so
/// Continue stands at one place for a panel size on every screen, whatever
/// the head says, wherever the light is, whatever the shell's button hints
/// are set to and whatever note there is. (Were it to move, a note too long
/// for the legend's line would grow the foot and push the buttons up, and
/// Continue would jump a few pixels down and up.) A note on lines of its
/// own takes them from the room instead.
#[derive(Debug, Clone, PartialEq)]
pub struct Panel {
    /// The glass itself.
    pub pane: [f32; 4],
    /// The mark, the title and the lines under it.
    pub head: [f32; 4],
    /// What the screen draws its own controls in: under the head, over the
    /// buttons and over a note standing on lines of its own.
    pub room: [f32; 4],
    /// The band the screen's row of buttons stands in.
    pub buttons: [f32; 4],
    /// The legend's line, and the note beside it.
    pub foot: [f32; 4],
    /// Where a note too long for the legend's line stands, on lines of its
    /// own just over the buttons; nothing tall when it is beside the legend.
    pub note_at: [f32; 4],
    /// The title's lines: one, or two where it is too long for one line of
    /// the head — the head grows rather than the title being cut.
    pub title: Vec<String>,
    /// The caption, wrapped to the room the head leaves it.
    pub caption: Vec<String>,
    /// The note's lines: one beside the legend, or up to two over the
    /// buttons.
    pub note: Vec<String>,
    /// Whether the note stands on lines of its own over the buttons.
    pub stacked: bool,
    /// Whether the chips stand on a line of their own over the title, at
    /// the head's top right — the corner they hold beside the title —
    /// because a title would not fit beside them.
    pub chips_above: bool,
}

/// How tall the parts of a panel came out, measured: what [`arrange`] lays
/// out inside it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Tall {
    head: f32,
    legend: f32,
    buttons: f32,
    /// A note on lines of its own; nought where it stands beside the legend
    /// or there is none.
    note: f32,
    gap: f32,
}

/// Where the parts go `inside` the panel's padding, from how tall they are:
/// the head, the room, the buttons, the note's own lines and the foot, in
/// that order. The foot and the buttons are hung off the bottom and depend
/// on nothing but the legend's and a button's height.
fn arrange(inside: [f32; 4], tall: Tall) -> [[f32; 4]; 5] {
    let [x, y, w, h] = inside;
    let gap = tall.gap;
    let head = [x, y, w, tall.head];
    let foot = [x, y + h - tall.legend, w, tall.legend];
    let buttons = [x, foot[1] - gap * 1.2 - tall.buttons, w, tall.buttons];
    let note_at = if tall.note > 0.0 {
        [x, buttons[1] - gap * 0.8 - tall.note, w, tall.note]
    } else {
        [x, buttons[1], w, 0.0]
    };
    let top = head[1] + head[3] + gap * 1.6;
    let bottom = if tall.note > 0.0 {
        note_at[1] - gap * 0.8
    } else {
        buttons[1] - gap * 1.2
    };
    let room = [x, top, w, (bottom - top).max(1.0)];
    [head, room, buttons, note_at, foot]
}

impl Panel {
    /// Lay the panel out for this window and these words, without drawing
    /// anything.
    pub fn lay(page: &mut Page, needs: Needs<'_>) -> Self {
        let scale = page.scaled(1.0);
        let pane = pane_rect(page.width(), page.height(), scale);
        let pad = page.metric(Metric::PanelPadding);
        let gap = page.metric(Metric::Gap);
        let inside = [
            pane[0] + pad,
            pane[1] + pad,
            (pane[2] - 2.0 * pad).max(1.0),
            (pane[3] - 2.0 * pad).max(1.0),
        ];

        let mark = page.metric(Metric::ItemIcon);
        let title_line = page.line(Text::Title);
        let caption_line = page.line(Text::Caption);
        // The chips stand on the title's line only, so the caption runs on
        // under them to the panel's edge.
        let words_w = words_width(inside[2], mark, gap, 0.0);
        let caption = text::wrapped(page.ui(), Text::Caption, needs.caption, words_w, MOST_LINES);
        // Where any step's title would be too long to stand beside the chips
        // — a long headline on a panel standing on its side — titles keep
        // their words whole, and the chips stand on a line of their own over
        // the title on every step, at the head's top: decided by the longest
        // title, not this one, and over the title rather than under the
        // caption, or the chips moved from step to step with how many lines
        // a title and its caption took.
        let dots = needs.place.map_or(0.0, |place| place_width(page, place.of));
        let beside = words_width(inside[2], mark, gap, dots);
        let chips_above = dots > 0.0
            && needs
                .titles
                .iter()
                .chain([&needs.title])
                .any(|title| page.measure(Text::Title, title) > beside);
        let chips_line = if chips_above { chip_line(page) } else { 0.0 };
        // Past the head's whole width, the title takes a second line.
        let title = text::wrapped(page.ui(), Text::Title, needs.title, words_w, MOST_LINES);
        let titles = title_line * title.len().max(1) as f32;
        let head_tall = mark.max(titles + caption_line * caption.len().max(1) as f32 + chips_line);

        let legend_w = legend::width(page, needs.hints, needs.pad);
        let (note, stacked) = match needs.note {
            None => (Vec::new(), false),
            Some(words) => {
                let wide = page.measure(Text::Caption, words);
                if wide + gap * 2.0 + legend_w <= inside[2] {
                    (vec![words.to_string()], false)
                } else {
                    let lines =
                        text::wrapped(page.ui(), Text::Caption, words, inside[2], MOST_LINES);
                    (lines, true)
                }
            }
        };
        let note_tall = if stacked {
            note.len() as f32 * caption_line
        } else {
            0.0
        };
        let [head, room, buttons, note_at, foot] = arrange(
            inside,
            Tall {
                head: head_tall,
                legend: legend::height(page),
                buttons: button::height(page),
                note: note_tall,
                gap,
            },
        );
        Self {
            pane,
            head,
            room,
            buttons,
            foot,
            note_at,
            title,
            caption,
            note,
            stacked,
            chips_above,
        }
    }

    /// The room and the buttons together: where the on-screen keyboard rises
    /// into, over whatever stands there.
    pub fn reach(&self) -> [f32; 4] {
        let bottom = self.buttons[1] + self.buttons[3];
        [
            self.room[0],
            self.room[1],
            self.room[2],
            (bottom - self.room[1]).max(1.0),
        ]
    }
}

/// How much a line of chips of its own takes over the title.
fn chip_line(page: &Page) -> f32 {
    page.metric(Metric::Gap) * 0.8 + page.scaled(DOT)
}

/// How wide the title and caption can be in a head `width` wide: what the
/// mark at the left and the chips at the right leave.
fn words_width(width: f32, mark: f32, gap: f32, dots: f32) -> f32 {
    let chips = if dots > 0.0 { dots + gap * 2.0 } else { 0.0 };
    (width - mark - gap * 1.4 - chips).max(0.0)
}

/// Lay the glass down. Done first on every frame, so everything else is drawn
/// on it.
pub fn glass(page: &mut Page, panel: &Panel) {
    page.ui().pane(panel.pane, Overlay::Dialog);
}

/// What a screen's head shows beside its words (laid out by
/// [`Panel::lay`]): its mark.
#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub mark: &'static str,
}

/// Where in the flow the person is: the `place`th of `of` steps that ask
/// something, drawn as a row of small chips at the right of the head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub place: usize,
    pub of: usize,
}

/// How wide the row of chips for `of` steps is, at a given scale.
fn place_width(page: &Page, of: usize) -> f32 {
    if of == 0 {
        return 0.0;
    }
    let dot = page.scaled(DOT);
    let long = page.scaled(LONG);
    let gap = page.scaled(CHIP_GAP);
    long + dot * (of as f32 - 1.0) + gap * (of as f32 - 1.0)
}

/// Where the head's words begin, how wide the title and the caption may be,
/// and where the first of them stands, on a panel laid out for `place`'s row
/// of chips (or none).
struct HeadLines {
    words_x: f32,
    title_w: f32,
    caption_w: f32,
    top: f32,
    title_line: f32,
}

fn head_lines(page: &Page, panel: &Panel, place: Option<Place>) -> HeadLines {
    let head = panel.head;
    let mark = page.metric(Metric::ItemIcon);
    let gap = page.metric(Metric::Gap);
    let title_line = page.line(Text::Title);
    let caption_line = page.line(Text::Caption);
    let dots = place.map_or(0.0, |place| place_width(page, place.of));
    let above = panel.chips_above;
    let title_w = words_width(head[2], mark, gap, if above { 0.0 } else { dots });
    let caption_w = words_width(head[2], mark, gap, 0.0);
    let captions = caption_line * panel.caption.len().max(1) as f32;
    let chips_line = if above { chip_line(page) } else { 0.0 };
    let titles = title_line * panel.title.len().max(1) as f32;
    // The words in the middle of what the chips leave of the head, the
    // chips — where they stand over the title — at its very top.
    let top = head[1] + chips_line + (head[3] - chips_line - titles - captions) * 0.5;
    HeadLines {
        words_x: head[0] + mark + gap * 1.4,
        // A title on two lines stands at the head's whole width, the chips
        // having gone over it.
        title_w: if panel.title.len() > 1 {
            caption_w
        } else {
            title_w
        },
        caption_w,
        top,
        title_line,
    }
}

/// Where the row of chips for `place` stands on this panel: its left end, and
/// the middle of its line — over the title where the chips stand above it,
/// otherwise on the title's own line at the head's right.
pub fn chip_row(page: &Page, panel: &Panel, place: Place) -> [f32; 2] {
    let head = panel.head;
    let lines = head_lines(page, panel, Some(place));
    let dot = page.scaled(DOT);
    let middle = if panel.chips_above {
        head[1] + dot * 0.5
    } else {
        lines.top + lines.title_line * 0.5
    };
    [head[0] + head[2] - place_width(page, place.of), middle]
}

/// Draw the head's words: the mark at the left, and the title beside it with
/// the caption's lines under it — what a page carries with it as it comes in
/// and goes. Where in the flow this is, the row of chips at the right, is
/// [`Chips`]'s, which moves by itself.
pub fn header(page: &mut Page, panel: &Panel, header: Header, place: Option<Place>, fade: f32) {
    let head = panel.head;
    let mark = page.metric(Metric::ItemIcon);
    let caption_line = page.line(Text::Caption);
    let icons = page.icons();
    let HeadLines {
        words_x,
        title_w,
        caption_w,
        top,
        title_line,
    } = head_lines(page, panel, place);
    let titles = title_line * panel.title.len().max(1) as f32;

    let ui = page.ui();
    ui.icon_tinted(
        [head[0], head[1] + (head[3] - mark) * 0.5, mark, mark],
        header.mark,
        icons,
        Role::Text,
        fade,
    );
    for (index, line) in panel.title.iter().enumerate() {
        text::label_faded(
            ui,
            [
                words_x,
                top + index as f32 * title_line,
                title_w,
                title_line,
            ],
            Text::Title,
            line,
            Role::Text,
            fade,
            Align::Left,
        );
    }
    let (caption_ink, caption_alpha) = text::LESSER;
    for (index, line) in panel.caption.iter().enumerate() {
        text::label_faded(
            ui,
            [
                words_x,
                top + titles + index as f32 * caption_line,
                caption_w,
                caption_line,
            ],
            Text::Caption,
            line,
            caption_ink,
            caption_alpha * fade,
            Align::Left,
        );
    }
}

/// The chips' sizes in reference pixels: a short one, the long one, and the
/// gap between two.
const DOT: f32 = 9.0;
const LONG: f32 = 26.0;
const CHIP_GAP: f32 = 7.0;

/// How bright a short chip is for a step behind and for one still to come.
const BEHIND: f32 = 0.55;
const AHEAD: f32 = 0.2;

/// One frame of the row of chips, measured from its left end: each chip's
/// left edge and width and how far behind the long chip it is (one behind,
/// nought still to come), and where the long chip runs from and to.
#[derive(Debug, Clone, PartialEq)]
pub struct ChipsAt {
    pub chips: Vec<([f32; 2], f32)>,
    pub long: [f32; 2],
}

/// The row of `of` chips with the long chip's leading and trailing ends at
/// `lead` and `trail` (in places, a fraction between two of them on the way
/// from one to the next), `dot` and `long` wide, `gap` apart.
///
/// Each chip is as wide as the long chip is near it, so the row keeps its
/// length and its ends stand still as the long chip goes over; the long chip
/// runs from the left edge of the place its rear end is at to the right edge
/// of the place its front end is at, so it stretches over the chips between
/// on its way and closes up again where it lands.
pub fn chips_at(of: usize, lead: f32, trail: f32, dot: f32, long: f32, gap: f32) -> ChipsAt {
    let last = of.saturating_sub(1) as f32;
    let (lead, trail) = (lead.clamp(0.0, last), trail.clamp(0.0, last));
    let middle = (lead + trail) * 0.5;
    let mut x = 0.0;
    let mut edges = Vec::with_capacity(of);
    let mut chips = Vec::with_capacity(of);
    for index in 0..of {
        let near = (1.0 - (index as f32 - middle).abs()).max(0.0);
        let width = dot + (long - dot) * near;
        edges.push([x, x + width]);
        let behind = (middle - index as f32 + 0.5).clamp(0.0, 1.0);
        chips.push(([x, width], behind));
        x += width + gap;
    }
    let edge = |at: f32, side: usize| {
        if edges.is_empty() {
            return 0.0;
        }
        let below = (at.floor() as usize).min(edges.len() - 1);
        let above = (below + 1).min(edges.len() - 1);
        let share = at - below as f32;
        edges[below][side] + (edges[above][side] - edges[below][side]) * share
    };
    ChipsAt {
        long: [edge(lead.min(trail), 0), edge(lead.max(trail), 1)],
        chips,
    }
}

/// Where the long chip's two ends are, `through` of the way from `from` to
/// `to`: the leading end there in the first two thirds of the way and the
/// trailing end setting off a third in — the shell's [`GONE_BY`], the share
/// by which what leaves has gone before what comes has arrived.
pub fn ends(from: f32, to: f32, through: f32) -> (f32, f32) {
    let span = 1.0 - GONE_BY;
    let lead = motion::ease((through / span).clamp(0.0, 1.0));
    let trail = motion::ease(((through - GONE_BY) / span).clamp(0.0, 1.0));
    (from + (to - from) * lead, from + (to - from) * trail)
}

/// The row of chips that says which step this is, on its way from one step
/// to the next.
///
/// The long accented chip goes over to its new place as the page changes,
/// its leading end first and its trailing end after it, stretching over the
/// chips between and closing up where it lands, on the light's own spring
/// (`HIGHLIGHT_SPRING`) — it is the light of the row; the chips it passes
/// brighten as steps behind. The row comes up with the first step that has
/// one and goes with the last, where it stood.
#[derive(Debug, Clone, Default)]
pub struct Chips {
    /// The step the long chip is going to (or, going, the one it was on).
    place: Option<Place>,
    /// Where it set off from, in places.
    from: f32,
    through: Share,
    /// Whether the row is up, and how far it has come up or gone.
    showing: bool,
    shown: Share,
    /// Where the row stood on the last frame it was laid out for a step.
    row: Option<[f32; 2]>,
    /// Whether it has been told a step at all: the first frame ever drawn
    /// shows the row where it is, as a light or a column is put where it is
    /// first asked to be, rather than coming up on a window just opened.
    aimed: bool,
}

impl Chips {
    /// The step drawn now is at `place` (or has none): the long chip sets off
    /// for it from where it is, and the row comes up or goes.
    pub fn aim(&mut self, place: Option<Place>) {
        if !std::mem::replace(&mut self.aimed, true) {
            self.place = place;
            self.from = place.map_or(0.0, |place| place.place as f32);
            self.showing = place.is_some();
            self.through.settle();
            self.shown.settle();
            return;
        }
        match (self.showing.then_some(self.place).flatten(), place) {
            (Some(was), Some(now)) if was == now => {}
            (Some(was), Some(now)) if was.of == now.of => {
                self.from = self.middle();
                self.place = Some(now);
                self.through.begin();
            }
            (_, Some(now)) => {
                self.place = Some(now);
                self.from = now.place as f32;
                self.through.settle();
                self.showing = true;
                self.shown.begin();
            }
            (Some(_), None) => {
                self.showing = false;
                self.shown.begin();
            }
            (None, None) => {}
        }
    }

    /// Where the long chip's middle is now, in places.
    fn middle(&self) -> f32 {
        let to = self.place.map_or(0.0, |place| place.place as f32);
        let (lead, trail) = ends(self.from, to, self.through.at());
        (lead + trail) * 0.5
    }

    /// Carry the row one frame's worth on.
    pub fn advance(&mut self, dt: f32) {
        self.through.advance(motion::HIGHLIGHT_SPRING, dt);
        self.shown.advance(motion::HIGHLIGHT_SPRING, dt);
    }

    /// How much of the row is shown: coming up, the whole of its way; going,
    /// gone a third of the way before it stops, as a page's ink is.
    fn presence(&self) -> f32 {
        if self.showing {
            self.shown.at()
        } else {
            departing(1.0 - self.shown.at())
        }
    }

    /// Draw the row, standing at `row` on this frame's panel (its left end and
    /// the middle of its line) where the step drawn has one; going, it stays
    /// where it last stood.
    pub fn draw(&mut self, page: &mut Page, row: Option<[f32; 2]>) {
        if row.is_some() {
            self.row = row;
        }
        let (Some(place), Some([left, middle])) = (self.place, self.row) else {
            return;
        };
        let presence = self.presence();
        if presence <= 0.0 {
            return;
        }
        let (lead, trail) = ends(self.from, place.place as f32, self.through.at());
        let dot = page.scaled(DOT);
        let at = chips_at(
            place.of,
            lead,
            trail,
            dot,
            page.scaled(LONG),
            page.scaled(CHIP_GAP),
        );
        let ui = page.ui();
        let [from, to] = at.long;
        for ([x, width], behind) in at.chips {
            // One wholly under the long chip is not drawn at all: its rim
            // would show round the accent's.
            if x >= from - 0.5 && x + width <= to + 0.5 {
                continue;
            }
            let ink = AHEAD + (BEHIND - AHEAD) * behind;
            ui.chip(
                [left + x, middle - dot * 0.5, width, dot],
                ui.tinted(Role::Text, ink * presence),
            );
        }
        // The long chip over them, in the accent: where the person is.
        ui.chip(
            [left + from, middle - dot * 0.5, to - from, dot],
            ui.tinted(Role::Accent, presence),
        );
    }

    /// Put the long chip where it is going and the row up or gone.
    pub fn settle(&mut self) {
        self.through.settle();
        self.shown.settle();
        if let Some(place) = self.place {
            self.from = place.place as f32;
        }
        if !self.showing {
            self.place = None;
        }
    }

    pub fn moving(&self) -> bool {
        self.through.moving() || self.shown.moving()
    }
}

/// A line beside the legend, and what kind of line it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub text: String,
    /// `Role::Danger` for something that was refused — drawn in the full
    /// white of a refusal ([`text::REFUSAL`]), never in red — and
    /// `Role::Text` for anything else, drawn as a lesser line
    /// ([`text::LESSER`]). Never the accent: a sentence in the accent on the
    /// accent's own ground reads as a smudge.
    pub role: Role,
}

/// What drawing the foot left on the frame: each pair of the legend, and the
/// note with where it stood and the part of the scene it is.
#[derive(Debug, Clone, Default)]
pub struct LaidFoot {
    pub pairs: Vec<LaidPair>,
    pub note: Option<LaidNote>,
}

/// The note as it was drawn: its lines, where the first began, and the part
/// of the scene it is.
#[derive(Debug, Clone)]
pub struct LaidNote {
    pub lines: Vec<String>,
    pub at: [f32; 2],
    pub drawn: (Drawn, Drawn),
}

impl LaidNote {
    /// Whether `other` says the same in the same place.
    pub fn same_as(&self, other: &LaidNote) -> bool {
        self.lines == other.lines
            && (self.at[0] - other.at[0]).abs() < 0.5
            && (self.at[1] - other.at[1]).abs() < 0.5
    }
}

/// Draw the foot: the legend in the corner, and the note — laid out by
/// [`Panel::lay`] — beside it, or over the buttons on lines of its own, in
/// `role`. Answers what each part left on the frame.
pub fn foot(page: &mut Page, panel: &Panel, hints: &[Hint], pad: bool, role: Role) -> LaidFoot {
    let (role, alpha) = text::note_ink(role);
    let foot = panel.foot;
    let gap = page.metric(Metric::Gap);
    let line = page.line(Text::Caption);
    let middle = foot[1] + foot[3] * 0.5;
    if panel.stacked {
        let at = panel.note_at;
        let ui = page.ui();
        let from = ui.drawn();
        for (index, words) in panel.note.iter().enumerate() {
            text::label_faded(
                ui,
                [at[0], at[1] + index as f32 * line, at[2], line],
                Text::Caption,
                words,
                role,
                alpha,
                Align::Left,
            );
        }
        let note = (!panel.note.is_empty()).then(|| LaidNote {
            lines: panel.note.clone(),
            at: [at[0], at[1]],
            drawn: (from, ui.drawn()),
        });
        let (_, pairs) = legend::row(page, foot[0] + foot[2], foot[0], middle, hints, pad);
        return LaidFoot { pairs, note };
    }
    let (left, pairs) = legend::row(page, foot[0] + foot[2], foot[0], middle, hints, pad);
    let Some(words) = panel.note.first() else {
        return LaidFoot { pairs, note: None };
    };
    let width = (left - gap - foot[0]).max(0.0);
    let at = [foot[0], middle - line * 0.5];
    let ui = page.ui();
    let from = ui.drawn();
    text::label_faded(
        ui,
        [at[0], at[1], width, line],
        Text::Caption,
        words,
        role,
        alpha,
        Align::Left,
    );
    let note = Some(LaidNote {
        lines: vec![words.clone()],
        at,
        drawn: (from, ui.drawn()),
    });
    LaidFoot { pairs, note }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lxb_app::lxb_toolkit::metrics::scale_for;

    fn at(width: f32, height: f32) -> [f32; 4] {
        pane_rect(width, height, scale_for(height))
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn at_the_reference_height_the_panel_is_its_reference_size_centred() {
        let pane = at(1920.0, 1080.0);
        assert!(close(pane[2], WIDTH) && close(pane[3], HEIGHT), "{pane:?}");
        assert!(close(pane[0], (1920.0 - WIDTH) * 0.5));
        assert!(close(pane[1], (1080.0 - HEIGHT) * 0.5));
    }

    #[test]
    fn it_grows_and_shrinks_with_the_height() {
        let big = at(3840.0, 2160.0);
        assert!(close(big[2], WIDTH * 2.0) && close(big[3], HEIGHT * 2.0));
        let small = at(1280.0, 720.0);
        assert!(close(small[2], WIDTH * 720.0 / 1080.0));
        assert!(close(small[3], HEIGHT * 720.0 / 1080.0));
    }

    #[test]
    fn standing_on_its_side_it_keeps_its_height_and_takes_the_width_there_is() {
        let scale = scale_for(1920.0);
        let pane = at(1080.0, 1920.0);
        assert!(
            close(pane[3], HEIGHT * scale),
            "the height was shrunk to fit"
        );
        assert!(close(pane[2], 1080.0 - 2.0 * MARGIN * scale));
        assert!(close(pane[0], MARGIN * scale), "not centred");
    }

    #[test]
    fn the_chips_take_room_from_the_title_and_not_from_the_caption() {
        let title = words_width(1000.0, 60.0, 20.0, 120.0);
        let caption = words_width(1000.0, 60.0, 20.0, 0.0);
        assert!(close(caption - title, 120.0 + 2.0 * 20.0));
        assert_eq!(
            words_width(50.0, 60.0, 20.0, 0.0),
            0.0,
            "a sliver went negative"
        );
    }

    /// A panel's inside at the reference height, and the parts' heights
    /// there: a head of one caption line, the legend, a button, no note.
    fn plain() -> ([f32; 4], Tall) {
        (
            [44.0, 44.0, 1192.0, 772.0],
            Tall {
                head: 72.0,
                legend: 28.0,
                buttons: 54.0,
                note: 0.0,
                gap: 14.0,
            },
        )
    }

    #[test]
    fn the_buttons_and_the_legend_stand_where_they_stand_whatever_else_there_is() {
        let (inside, tall) = plain();
        let [_, _, buttons, _, foot] = arrange(inside, tall);
        let others = [
            Tall {
                head: 100.0,
                ..tall
            },
            Tall { note: 55.0, ..tall },
            Tall {
                head: 100.0,
                note: 27.5,
                ..tall
            },
        ];
        for other in others {
            let [_, room, moved, note_at, still] = arrange(inside, other);
            assert_eq!((moved, still), (buttons, foot), "{other:?}");
            // What the head and a note take comes out of the room.
            assert!(room[1] + room[3] <= note_at[1] && note_at[1] + note_at[3] <= buttons[1]);
        }
        // Hung off the bottom of the inside: the legend's line last, the
        // buttons a gap over it.
        assert!(close(foot[1] + foot[3], inside[1] + inside[3]));
        assert!(buttons[1] + buttons[3] < foot[1]);
    }

    #[test]
    fn a_note_on_lines_of_its_own_stands_over_the_buttons_not_in_the_foot() {
        let (inside, tall) = plain();
        let [_, room, buttons, note_at, _] = arrange(inside, Tall { note: 55.0, ..tall });
        assert!(close(note_at[3], 55.0));
        assert!(note_at[1] + note_at[3] < buttons[1]);
        assert!(room[1] + room[3] < note_at[1]);
        let [_, roomier, _, none, _] = arrange(inside, tall);
        assert_eq!(none[3], 0.0);
        assert!(roomier[3] > room[3], "a note that is not there took room");
    }

    #[test]
    fn the_row_of_chips_keeps_its_length_as_the_long_chip_goes_over() {
        let (dot, long, gap) = (9.0, 26.0, 7.0);
        let whole = long + (dot + gap) * 6.0;
        let rest = chips_at(7, 2.0, 2.0, dot, long, gap);
        assert_eq!(rest.chips[2].0, [2.0 * (dot + gap), long]);
        assert_eq!(rest.long, [2.0 * (dot + gap), 2.0 * (dot + gap) + long]);
        assert_eq!(rest.chips[1].1, 1.0, "a step behind is not behind");
        assert_eq!(rest.chips[3].1, 0.0, "a step to come is behind");
        for (lead, trail) in [(2.0, 2.0), (2.7, 2.1), (3.0, 2.5), (2.4, 2.4), (0.0, 6.0)] {
            let at = chips_at(7, lead, trail, dot, long, gap);
            let [x, width] = at.chips[6].0;
            assert!(
                (x + width - whole).abs() < 1e-3 && at.chips[0].0[0] == 0.0,
                "the row's ends moved at {lead}, {trail}"
            );
        }
        // On its way from the third step to the fourth it stretches from
        // where it was to where it is going.
        let between = chips_at(7, 3.0, 2.0, dot, long, gap);
        assert_eq!(between.long[0], between.chips[2].0[0]);
        let [x, width] = between.chips[3].0;
        assert_eq!(between.long[1], x + width);
    }

    #[test]
    fn the_long_chips_leading_end_goes_first_and_its_trailing_end_follows() {
        assert_eq!(ends(2.0, 3.0, 0.0), (2.0, 2.0));
        assert_eq!(ends(2.0, 3.0, 1.0), (3.0, 3.0));
        let (lead, trail) = ends(2.0, 3.0, 0.5);
        assert!(lead > trail + 0.5, "it slid over without stretching");
        let (lead, trail) = ends(3.0, 1.0, 0.5);
        assert!(
            lead < trail - 1.0,
            "going back, the leading end is the left"
        );
    }

    #[test]
    fn the_row_of_chips_comes_up_goes_over_and_is_then_still() {
        use crate::ui::motion::FRAME;
        let settle = |chips: &mut Chips| {
            let mut frames = 0;
            while chips.moving() {
                chips.advance(FRAME);
                frames += 1;
                assert!(frames < 90, "still moving after a second and a half");
            }
            frames
        };
        let mut first = Chips::default();
        first.aim(Some(Place { place: 3, of: 7 }));
        assert!(!first.moving(), "the first frame ever drawn moved the row");
        assert_eq!((first.presence(), first.middle()), (1.0, 3.0));

        let mut chips = Chips::default();
        chips.aim(None);
        assert!(!chips.moving(), "a row nobody showed moved");
        chips.aim(Some(Place { place: 0, of: 7 }));
        assert!(chips.moving(), "the row came up at once");
        assert_eq!(chips.presence(), 0.0);
        assert!(settle(&mut chips) > 5);
        assert_eq!(chips.presence(), 1.0);
        chips.aim(Some(Place { place: 1, of: 7 }));
        chips.advance(FRAME);
        let going = chips.middle();
        assert!(going > 0.0 && going < 1.0, "the long chip jumped: {going}");
        settle(&mut chips);
        assert_eq!(chips.middle(), 1.0);
        // Still once there: the same place asked for again moves nothing.
        chips.aim(Some(Place { place: 1, of: 7 }));
        assert!(!chips.moving());
        chips.aim(None);
        assert!(chips.moving());
        settle(&mut chips);
        assert_eq!(chips.presence(), 0.0, "the row stayed when its steps ended");
    }

    #[test]
    fn it_never_runs_past_the_window() {
        for (width, height) in [
            (800.0, 480.0),
            (1366.0, 768.0),
            (640.0, 1136.0),
            (2560.0, 1080.0),
        ] {
            let pane = at(width, height);
            assert!(
                pane[0] >= 0.0 && pane[1] >= 0.0,
                "{width}x{height}: {pane:?}"
            );
            assert!(pane[0] + pane[2] <= width && pane[1] + pane[3] <= height);
        }
    }
}
