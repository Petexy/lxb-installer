//! What the buttons do, drawn rather than spelled out.
//!
//! The shell writes this row in the corner of its start screen and so does
//! every application beside it: a word, and a picture of the button that does
//! it, laid out from the right-hand margin leftwards. Drawn rather than
//! lettered because the same act is South on a pad and Enter on a keyboard,
//! and there is no wording that names both without naming neither. Copied
//! from SongOnSole and DistriBumpy, whose legends are the same row.
//!
//! Three rules come with it, all the shell's:
//!
//! * **A legend naming a button that does nothing is worse than naming none.**
//!   Every hint is asked of the same screen that answers the press, so the
//!   two cannot drift.
//! * **Nothing about moving.** The arrows are the one thing a page does not
//!   have to explain.
//! * **A pair is a control.** Each is a target a pointer can land on, and a
//!   click on one is a press of the button it pictures — which is how a mouse
//!   gets back out of a screen whose Back is only ever drawn here.
//!
//! Which picture is drawn is `Page::pad_in_hand`, which the toolkit watches
//! the way the shell does; and nothing is drawn at all when the shell's
//! Settings > System > Button hints is off.

use lxb_app::lxb_render::{Align, Drawn};
use lxb_app::lxb_toolkit::{input::Action, palette::Role, typography::Text};
use lxb_app::Page;

use super::{spots, text};

/// One thing a button does: the word, and which button it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hint {
    pub label: &'static str,
    pub on: Button,
}

/// The buttons a legend ever names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// South, Enter: acts on what the light is on.
    Accept,
    /// North, the right mouse button: the other answers (Details, Options).
    Options,
    /// East, Escape: the way back.
    Back,
    /// Start, which no key on a keyboard is pictured as — so it is named only
    /// while a pad is in hand. The board's Done.
    Start,
}

impl Button {
    /// What a press of this is, for a pair that was clicked rather than
    /// pressed. A click on a pair has to do exactly what the button it
    /// pictures does, or the row is a picture of something else.
    pub fn action(self) -> Action {
        match self {
            Button::Accept => Action::Accept,
            Button::Options => Action::Menu,
            Button::Back => Action::Back,
            Button::Start => Action::Submit,
        }
    }

    /// The picture of this button, given what the person's hands are on.
    ///
    /// `None` where there is no honest picture of it, which is a reason to
    /// leave the pair out rather than to draw the wrong button.
    pub fn glyph(self, pad: bool) -> Option<&'static str> {
        Some(match (self, pad) {
            (Button::Accept, true) => "pad-south",
            (Button::Accept, false) => "key-enter",
            (Button::Options, true) => "pad-north",
            (Button::Options, false) => "mouse-right",
            (Button::Back, true) => "pad-east",
            (Button::Back, false) => "key-escape",
            (Button::Start, true) => "pad-start",
            (Button::Start, false) => return None,
        })
    }
}

/// Shorthand for one pair.
pub const fn hint(label: &'static str, on: Button) -> Hint {
    Hint { label, on }
}

/// How large a legend is drawn: the shell's own proportions, a picture of a
/// button rather larger than the word beside it, because the picture is the
/// half being read.
pub const GLYPH: f32 = 28.0;
pub const GAP: f32 = 7.0;
pub const STEP: f32 = 22.0;

/// How far the step between pairs closes, at the most, in a row short of
/// room, before any word is cut.
pub const CLOSEST: f32 = 0.5;

/// Whether this session wants a legend at all: the shell's own switch, read
/// once as the toolkit reads it, and on where nothing has been said.
pub fn wanted() -> bool {
    lxb_app::lxb_toolkit::settings::button_hints().unwrap_or(true)
}

/// What drawing one pair of a legend left on the frame: what it says, the
/// button it pictures, where it stands, and the part of the scene it is —
/// what a change of page keeps standing where the next page's legend has the
/// same pair in the same place.
#[derive(Debug, Clone)]
pub struct LaidPair {
    pub said: String,
    pub on: Button,
    pub at: [f32; 4],
    pub drawn: (Drawn, Drawn),
}

impl LaidPair {
    /// Whether `other` says the same of the same button in the same place,
    /// within half a pixel.
    pub fn same_as(&self, other: &LaidPair) -> bool {
        self.said_alike(other)
            && self
                .at
                .iter()
                .zip(other.at)
                .all(|(a, b)| (a - b).abs() < 0.5)
    }

    /// Whether `other` says the same of the same button, wherever it stands:
    /// one moved along the row because a pair beside it says something else.
    pub fn said_alike(&self, other: &LaidPair) -> bool {
        self.said == other.said && self.on == other.on
    }
}

/// Lay a legend out from `right` leftwards, centred on `middle`, and answer
/// where its left-hand end came out — which is where anything else on that
/// line (a note, a refusal) has to stop — and what each pair left on the
/// frame.
///
/// Right to left because the words are different lengths and the row is hung
/// off the margin: built that way, the pair nearest the corner lands exactly
/// on it whatever the words turn out to measure.
///
/// Never further left than `left`: where the whole row would run past it — a
/// panel standing on its side in a long language — the pairs first stand
/// closer together, down to [`CLOSEST`] of their step, and only past that
/// does every word give up the same share of itself, cut with an ellipsis,
/// the pictures staying whole.
pub fn row(
    page: &mut Page,
    right: f32,
    left: f32,
    middle: f32,
    hints: &[Hint],
    pad: bool,
) -> (f32, Vec<LaidPair>) {
    let mut laid = Vec::new();
    if !wanted() {
        return (right, laid);
    }
    let glyph = page.scaled(GLYPH);
    let gap = page.scaled(GAP);
    let label = page.line(Text::Caption);
    let icons = page.icons();
    let natural = width(page, hints, pad);
    let pairs = hints
        .iter()
        .filter(|hint| hint.on.glyph(pad).is_some())
        .count();
    let step = closed_step(page.scaled(STEP), natural, pairs, right - left);
    let natural = natural - (page.scaled(STEP) - step) * pairs.saturating_sub(1) as f32;
    let words: f32 = hints
        .iter()
        .filter(|hint| hint.on.glyph(pad).is_some())
        .map(|hint| page.measure(Text::Caption, hint.label))
        .sum();
    let squeeze = squeeze(natural, words, right - left);

    let mut at = right;
    for (index, hint) in hints.iter().enumerate().rev() {
        let Some(mark) = hint.on.glyph(pad) else {
            continue;
        };
        let word = page.measure(Text::Caption, hint.label) * squeeze;
        let said = if squeeze < 1.0 {
            text::fit(page.ui(), Text::Caption, hint.label, word)
        } else {
            hint.label.to_string()
        };
        let ends = at - glyph - gap;
        let ui = page.ui();
        let from = ui.drawn();
        // The word and the picture beside it are one target: they say the same
        // thing. Grown by half a gap all round, which is comfortably inside
        // the step between pairs, so no two of them ever overlap.
        let target = [
            ends - word - gap * 0.5,
            middle - glyph * 0.5 - gap * 0.5,
            word + gap * 2.0 + glyph,
            glyph + gap,
        ];
        ui.spot(spots::LEGEND + index as u32, target);
        ui.icon_tinted(
            [at - glyph, middle - glyph * 0.5, glyph, glyph],
            mark,
            icons,
            Role::Text,
            0.85,
        );
        let (role, alpha) = super::text::LESSER;
        ui.label_tinted(
            [ends - word, middle - label * 0.5, word, label],
            Text::Caption,
            &said,
            ui.tinted(role, alpha),
            Align::Right,
        );
        laid.push(LaidPair {
            said,
            on: hint.on,
            at: target,
            drawn: (from, ui.drawn()),
        });
        at = ends - word - step;
    }
    (at, laid)
}

/// The step between `pairs` pairs of a row `natural` wide at its full
/// `step`, in `room`: the whole step where the row fits, else as much less
/// as brings it in, down to [`CLOSEST`] of it.
fn closed_step(step: f32, natural: f32, pairs: usize, room: f32) -> f32 {
    if natural <= room || pairs < 2 {
        return step;
    }
    let over = natural - room;
    (step - over / (pairs - 1) as f32).max(step * CLOSEST)
}

/// How much of itself each word of a legend keeps in `room`: all of it where
/// the row's `natural` width fits, else the share that brings the words —
/// `words` wide together — into what the pictures and gaps leave.
fn squeeze(natural: f32, words: f32, room: f32) -> f32 {
    if natural <= room || words <= 0.0 {
        return 1.0;
    }
    let fixed = natural - words;
    ((room - fixed) / words).clamp(0.0, 1.0)
}

/// How wide [`row`] would draw these hints, without drawing them: what the
/// panel asks before it decides whether a note fits beside the legend.
pub fn width(page: &mut Page, hints: &[Hint], pad: bool) -> f32 {
    if !wanted() {
        return 0.0;
    }
    let glyph = page.scaled(GLYPH);
    let gap = page.scaled(GAP);
    let step = page.scaled(STEP);
    let mut total = 0.0;
    let mut pairs = 0;
    for hint in hints {
        if hint.on.glyph(pad).is_none() {
            continue;
        }
        total += glyph + gap + page.measure(Text::Caption, hint.label);
        pairs += 1;
    }
    if pairs > 1 {
        total += step * (pairs - 1) as f32;
    }
    total
}

/// Which button a click on the row landed on, if it landed on it at all.
///
/// Asked of the same hints the row was drawn from, so a pair that was not
/// drawn — Start, on a keyboard — cannot be pressed.
pub fn pressed(page: &mut Page, hints: &[Hint], pad: bool) -> Option<Button> {
    let mut found = None;
    for (index, hint) in hints.iter().enumerate() {
        // Every pair's press is taken whether or not it is the one, so a
        // stale click cannot linger to be read on a later frame.
        if page.pressed(spots::LEGEND + index as u32)
            && hint.on.glyph(pad).is_some()
            && found.is_none()
        {
            found = Some(hint.on);
        }
    }
    found
}

/// How tall a row of this is, which is what the foot of the panel keeps for
/// it.
pub fn height(page: &Page) -> f32 {
    page.scaled(GLYPH).max(page.line(Text::Caption))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_button_pictured_is_a_mark_the_toolkit_has() {
        for button in [Button::Accept, Button::Options, Button::Back, Button::Start] {
            for pad in [true, false] {
                if let Some(glyph) = button.glyph(pad) {
                    assert!(
                        lxb_app::lxb_toolkit::assets::glyph(glyph).is_some(),
                        "a legend asks for a mark that is not in the toolkit: {glyph}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_legend_that_fits_keeps_its_words_and_one_that_does_not_shares_the_cut() {
        assert_eq!(squeeze(400.0, 200.0, 471.0), 1.0);
        let share = squeeze(519.0, 300.0, 471.0);
        assert!((219.0 + 300.0 * share - 471.0).abs() < 0.01, "{share}");
        assert_eq!(squeeze(519.0, 300.0, 100.0), 0.0, "past the pictures");
    }

    #[test]
    fn a_legend_short_of_room_closes_up_before_it_cuts_a_word() {
        assert_eq!(closed_step(22.0, 400.0, 3, 471.0), 22.0);
        let step = closed_step(22.0, 481.0, 4, 471.0);
        assert!((481.0 - (22.0 - step) * 3.0 - 471.0).abs() < 0.01, "{step}");
        assert_eq!(closed_step(22.0, 600.0, 4, 471.0), 11.0, "closed past half");
    }

    #[test]
    fn start_is_named_only_where_it_can_be_pictured() {
        assert!(Button::Start.glyph(true).is_some());
        assert!(Button::Start.glyph(false).is_none());
    }

    #[test]
    fn a_click_on_a_pair_is_the_press_it_pictures() {
        assert_eq!(Button::Accept.action(), Action::Accept);
        assert_eq!(Button::Back.action(), Action::Back);
        assert_eq!(Button::Options.action(), Action::Menu);
        assert_eq!(Button::Start.action(), Action::Submit);
    }
}
