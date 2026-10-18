//! A row of cards: the desktop step's three choices.
//!
//! Each card is a pane of control glass with a picture at its head (a mark
//! where there is no picture), a name, and one line about it. The light on a
//! card is card-shaped — `Ui::lit` at `Metric::CardRadius` — because a capsule
//! laid over a card leaves its corners sticking out. The choice in force wears
//! the `chosen` mark in its corner, and a card that cannot be chosen now (a
//! desktop that needs the internet, with none) is outlined and says why.
//!
//! Where the panel is too narrow for the cards to stand side by side at a
//! width they can be read at, they re-flow into a stack of wide, short cards,
//! picture at the left — the family's rule that a part too wide on its own
//! re-flows to the window's shape rather than shrinking.

use lxb_app::lxb_render::{Align, Fit, Press};
use lxb_app::lxb_toolkit::{
    input::Action, material::Surface, metrics::Metric, motion, palette::Role, typography::Text,
};
use lxb_app::Page;

use super::motion::{Light, Shape};
use super::spots;
use super::text;

/// The narrowest a card standing beside others can be, in reference pixels.
/// Narrower, and the row becomes a stack.
pub const LEAST: f32 = 300.0;

/// How tall a card in a stack is, in reference pixels.
pub const STACKED: f32 = 150.0;

/// One card.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub name: String,
    pub note: String,
    /// The mark drawn where there is no picture.
    pub mark: &'static str,
    /// A picture to show instead of the mark, if there is one on disk.
    pub picture: Option<std::path::PathBuf>,
    /// The state the card is in, on a line of its own under the note: "No
    /// internet needed", "Needs the internet".
    pub state: Option<String>,
    pub chosen: bool,
    pub enabled: bool,
    /// The card cannot be chosen now (a desktop that needs the internet,
    /// with none): outlined, its state said as a refusal — but the light still
    /// stops on it, so a press is answered with the Error sound and the
    /// reason instead of nothing at all.
    pub refused: bool,
}

/// Which card the light is on.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Cards {
    pub at: usize,
}

/// What a card in a stack keeps beside its picture's frame and its words:
/// its padding, the gap after the frame and the `chosen` mark at its end.
fn frame_beside(pad: f32, gap: f32, chosen: f32) -> f32 {
    pad * 0.5 + gap * 1.4 + pad + chosen + gap
}

/// The most room a card `width` by `height` leaves its words, at `scale`:
/// standing in a row, under its picture; in a stack, beside its picture's
/// frame narrowed as far as it goes. What the catalogs' test measures a
/// card's words against.
pub fn words_room_at(scale: f32, width: f32, height: f32, stacked: bool) -> f32 {
    let pad = Metric::RowPadding.value() * scale;
    let gap = Metric::Gap.value() * scale;
    let chosen = CHOSEN * scale;
    if stacked {
        let side = height - pad;
        (width - frame_beside(pad, gap, chosen) - side * NARROWEST).max(0.0)
    } else {
        (width - pad * 2.0 - chosen - gap).max(0.0)
    }
}

/// How wide the picture's frame in a stack of cards is, as a share of its
/// height: as wide as a screen is where the words beside it leave room, and
/// never narrower than three quarters of its height.
const WIDEST: f32 = 1.6;
const NARROWEST: f32 = 0.75;

/// How wide the picture's frame is on every card of a stack `width` wide
/// whose cards are `height` tall, the widest of their words being `words`:
/// what the words leave, within [`WIDEST`] and [`NARROWEST`].
///
/// **One width for the whole stack**, the narrowest any card needs: sized
/// card by card, the three cards' words began at three different places,
/// and a frame sized to leave its words exactly their width cut them by a
/// hair's breadth of rounding — so a little more than that is left.
pub fn stacked_frame(scale: f32, width: f32, height: f32, words: f32) -> f32 {
    let pad = Metric::RowPadding.value() * scale;
    let gap = Metric::Gap.value() * scale;
    let side = height - pad;
    let spare = width - frame_beside(pad, gap, CHOSEN * scale) - words - scale;
    spare.clamp(side * NARROWEST, side * WIDEST)
}

/// How large the `chosen` mark in a card's corner is, in reference pixels.
const CHOSEN: f32 = 34.0;

/// Whether `count` cards stand side by side in `width`, or stack.
pub fn side_by_side(width: f32, count: usize, gap: f32, least: f32) -> bool {
    count > 0 && (width - gap * (count as f32 - 1.0)) / count as f32 >= least
}

/// Where each of `count` cards goes in `room`: side by side and as tall as
/// the room allows (but no taller than a card should be), or stacked.
pub fn places(page: &Page, room: [f32; 4], count: usize) -> Vec<[f32; 4]> {
    let gap = page.metric(Metric::Gap) * 1.4;
    if count == 0 {
        return Vec::new();
    }
    if side_by_side(room[2], count, gap, page.scaled(LEAST)) {
        let width = (room[2] - gap * (count as f32 - 1.0)) / count as f32;
        let tall = room[3].min(width * 1.15);
        (0..count)
            .map(|index| [room[0] + index as f32 * (width + gap), room[1], width, tall])
            .collect()
    } else {
        let tall = page
            .scaled(STACKED)
            .min((room[3] - gap * (count as f32 - 1.0)) / count as f32);
        (0..count)
            .map(|index| {
                [
                    room[0],
                    room[1] + index as f32 * (tall + gap),
                    room[2],
                    tall,
                ]
            })
            .collect()
    }
}

impl Cards {
    /// Answer one direction. Cards side by side move with Left and Right,
    /// stacked ones with Up and Down; the other two directions are not the
    /// row's (they go to whatever is above or below it), which is `None`.
    /// `Some(moved)` otherwise.
    pub fn act(&mut self, action: Action, cards: &[Card], stacked: bool) -> Option<bool> {
        let direction = match (action, stacked) {
            (Action::Left, false) | (Action::Up, true) => -1,
            (Action::Right, false) | (Action::Down, true) => 1,
            _ => return None,
        };
        let mut at = self.at as isize + direction;
        while at >= 0 && (at as usize) < cards.len() {
            if cards[at as usize].enabled {
                self.at = at as usize;
                return Some(true);
            }
            at += direction;
        }
        Some(false)
    }

    /// Draw the cards at `places`, the light on the one it is on when `light`
    /// is handed in.
    ///
    /// In three passes, as DistriBumpy draws its cards: the cards' glass,
    /// then the light over it, then what is on the cards. Every quad of a
    /// layer is drawn in the order it was asked for, so a light laid before
    /// the glass would be buried under it, and one laid after the pictures
    /// would cover them.
    pub fn draw(
        &self,
        page: &mut Page,
        places: &[[f32; 4]],
        cards: &[Card],
        light: Option<&mut Light>,
        pressing: Option<f32>,
        dt: f32,
    ) {
        let stacked = places.len() > 1 && places[0][1] != places[1][1];
        let lit = light.is_some();
        let press_of = |index: usize| {
            let here = lit && index == self.at;
            match pressing {
                Some(through) if here => Press::Going(through),
                _ => Press::from(here),
            }
        };
        for (index, (card, rect)) in cards.iter().zip(places).enumerate() {
            if card.enabled {
                page.ui().spot(spots::CARDS + index as u32, *rect);
            }
            draw_glass(page, *rect, card, press_of(index));
        }
        if let Some(light) = light {
            if let Some(rect) = places.get(self.at) {
                let sunk = motion::pressed(*rect, pressing);
                light.lay(page, sunk, Shape::Card, pressing, dt);
            }
        }
        let frame = stacked.then(|| {
            let ui = page.ui();
            let words = cards
                .iter()
                .map(|card| words_width(ui, card))
                .fold(0.0f32, f32::max);
            let [_, _, width, height] = places[0];
            stacked_frame(page.scaled(1.0), width, height, words)
        });
        for (index, (card, rect)) in cards.iter().zip(places).enumerate() {
            draw_face(page, *rect, card, press_of(index), frame);
        }
    }

    /// Put the light on card `index`, where the pointer moved onto it or
    /// clicked it, if that card can be chosen. Answers whether it is there.
    pub fn point_at(&mut self, cards: &[Card], index: usize) -> bool {
        let landed = cards.get(index).is_some_and(|card| card.enabled);
        if landed {
            self.at = index;
        }
        landed
    }

    /// The card a spot number is, if it is one of `count` cards.
    pub fn card_of(id: u32, count: usize) -> Option<usize> {
        let index = id.checked_sub(spots::CARDS)? as usize;
        (index < count).then_some(index)
    }

    /// Which card a click landed on. Every card's press is taken, so a
    /// stale click cannot linger.
    pub fn clicked(page: &mut Page, cards: &[Card]) -> Option<usize> {
        let mut found = None;
        for (index, card) in cards.iter().enumerate() {
            if page.pressed(spots::CARDS + index as u32) && card.enabled && found.is_none() {
                found = Some(index);
            }
        }
        found
    }
}

/// A card's own glass: filled where it can be chosen, outlined where not.
fn draw_glass(page: &mut Page, rect: [f32; 4], card: &Card, press: Press) {
    let radius = page.metric(Metric::CardRadius);
    let sunk = motion::pressed(rect, press.through());
    let ui = page.ui();
    if card.enabled && !card.refused {
        ui.card(
            sunk,
            Surface::Control,
            Role::Glass,
            if press.lit() { 0.30 } else { 0.42 },
        );
    } else {
        ui.control_out(sunk, radius, 1.0);
    }
}

/// How wide a card's words are: the widest of its name, its note and its
/// state.
fn words_width(ui: &mut lxb_app::lxb_render::Ui, card: &Card) -> f32 {
    super::text::bold_width(ui, Text::Title, &card.name)
        .max(ui.measure(Text::Caption, &card.note))
        .max(
            card.state
                .as_deref()
                .map_or(0.0, |state| ui.measure(Text::Caption, state)),
        )
}

/// What is on a card: its picture or mark, its name and note, and the
/// `chosen` mark. `frame` is how wide the picture's frame is where the card
/// stands in a stack (see [`stacked_frame`]), `None` where it stands in a
/// row.
fn draw_face(page: &mut Page, rect: [f32; 4], card: &Card, press: Press, frame: Option<f32>) {
    let stacked = frame.is_some();
    let pad = page.metric(Metric::RowPadding);
    let gap = page.metric(Metric::Gap);
    let radius = page.metric(Metric::CardRadius);
    let name_line = page.line(Text::Title);
    let note_line = page.line(Text::Caption);
    let chosen = page.scaled(CHOSEN);
    let icons = page.icons();
    let sunk = motion::pressed(rect, press.through());
    let [x, y, w, h] = sunk;
    let ink = if card.enabled && !card.refused {
        1.0
    } else {
        0.55
    };
    let lines = if card.state.is_some() { 2.0 } else { 1.0 };

    let ui = page.ui();
    // The picture's frame: the top of a card standing in a row, the left of
    // one in a stack — there as wide as the stack's words leave it.
    let frame = if let Some(frame_w) = frame {
        [x + pad * 0.5, y + pad * 0.5, frame_w, h - pad]
    } else {
        [x + pad * 0.6, y + pad * 0.6, w - pad * 1.2, h * 0.58]
    };
    ui.card(frame, Surface::Control, Role::Accent, 0.22 * ink);
    let shown = card
        .picture
        .as_ref()
        .is_some_and(|path| ui.picture(frame, radius * 0.8, path, Fit::Cover, ink));
    if !shown {
        let mark = frame[2].min(frame[3]) * 0.46;
        ui.icon_tinted(
            [
                frame[0] + (frame[2] - mark) * 0.5,
                frame[1] + (frame[3] - mark) * 0.5,
                mark,
                mark,
            ],
            card.mark,
            icons,
            Role::Text,
            0.9 * ink,
        );
    }
    let (words_x, words_w, top) = if stacked {
        let left = frame[0] + frame[2] + gap * 1.4;
        (
            left,
            x + w - pad - left - chosen - gap,
            y + (h - name_line - note_line * lines) * 0.5,
        )
    } else {
        (
            x + pad,
            w - pad * 2.0 - chosen - gap,
            frame[1]
                + frame[3]
                + (y + h - frame[1] - frame[3] - name_line - note_line * lines) * 0.5,
        )
    };
    let words_w = words_w.max(0.0);
    text::label_faded(
        ui,
        [words_x, top, words_w, name_line],
        Text::Title,
        &card.name,
        Role::Text,
        ink,
        Align::Left,
    );
    let (soft, soft_alpha) = text::LESSER;
    text::label_faded(
        ui,
        [words_x, top + name_line, words_w, note_line],
        Text::Caption,
        &card.note,
        soft,
        soft_alpha * if card.enabled { 1.0 } else { 0.85 },
        Align::Left,
    );
    if let Some(state) = &card.state {
        let (role, alpha) = if card.refused {
            text::REFUSAL
        } else {
            (soft, soft_alpha)
        };
        text::label_faded(
            ui,
            [words_x, top + name_line + note_line, words_w, note_line],
            Text::Caption,
            state,
            role,
            alpha,
            Align::Left,
        );
    }
    if card.chosen {
        super::mark::chosen(
            ui,
            [
                x + w - pad - chosen,
                top + (name_line - chosen) * 0.5,
                chosen,
                chosen,
            ],
            icons,
            ink,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(enabled: bool) -> Card {
        Card {
            name: "A desktop".into(),
            note: "A note".into(),
            mark: "logo",
            picture: None,
            state: None,
            chosen: false,
            enabled,
            refused: false,
        }
    }

    #[test]
    fn three_cards_stand_side_by_side_on_a_wide_panel_and_stack_on_a_narrow_one() {
        assert!(side_by_side(1190.0, 3, 20.0, LEAST));
        assert!(!side_by_side(700.0, 3, 20.0, LEAST));
        assert!(!side_by_side(1000.0, 0, 20.0, LEAST));
    }

    #[test]
    fn the_light_steps_over_a_card_that_cannot_be_chosen() {
        let cards = [card(true), card(false), card(true)];
        let mut row = Cards::default();
        assert_eq!(row.act(Action::Right, &cards, false), Some(true));
        assert_eq!(row.at, 2);
        assert_eq!(row.act(Action::Right, &cards, false), Some(false));
        assert_eq!(
            row.act(Action::Down, &cards, false),
            None,
            "Down is not the row's"
        );
        assert_eq!(
            row.act(Action::Up, &cards, true),
            Some(true),
            "stacked, Up is"
        );
        assert_eq!(row.at, 0);
    }
}
