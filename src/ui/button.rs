//! Buttons: the verbs at the foot of a screen.
//!
//! The family's button, as DistriBumpy draws the row on an application's
//! page: a capsule of control glass, the one under the light left bright and
//! every other one washed back into the page, the one primary verb tinted
//! with the accent while the light is elsewhere, and a destructive verb
//! carrying a Danger wash and glow. A button that cannot be pressed now is
//! outlined rather than filled, and the light never stops on it.

use lxb_app::lxb_render::{Align, Drawn, Press};
use lxb_app::lxb_toolkit::{
    control, menu, metrics::Metric, motion, palette::Role, typography::Text,
};
use lxb_app::Page;

use super::motion::{Light, Shape};
use super::spots;
use super::text;

/// How a resting control steps back into the page: washed towards
/// `Role::Glass`, nearly black, so the lit one reads as the one in hand at a
/// glance. DistriBumpy's number, because it is the same rule.
pub const RESTING_WASH: f32 = 0.42;

/// What a button means, which is what decides how it is tinted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The one verb the screen is for (Continue, Install, Restart now).
    Primary,
    /// Any other way on or out (Not now, Details).
    Plain,
    /// A verb that destroys something.
    Danger,
}

/// One button: its word, what it means and whether it can be pressed now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Button<'a> {
    pub label: &'a str,
    pub kind: Kind,
    pub enabled: bool,
    /// Shorter words, said where the row has no room for the whole even
    /// with its buttons narrowed as far as they go (see [`fitted`]).
    pub shorter: Option<&'a str>,
    /// How much of its word's ink is there, from 0 to 1: Welcome's word
    /// fading between its greetings' languages while the button itself
    /// stands, lit and pressable.
    pub ink: f32,
}

impl<'a> Button<'a> {
    pub const fn primary(label: &'a str) -> Self {
        Self {
            label,
            kind: Kind::Primary,
            enabled: true,
            shorter: None,
            ink: 1.0,
        }
    }

    pub const fn plain(label: &'a str) -> Self {
        Self {
            label,
            kind: Kind::Plain,
            enabled: true,
            shorter: None,
            ink: 1.0,
        }
    }

    pub const fn danger(label: &'a str) -> Self {
        Self {
            label,
            kind: Kind::Danger,
            enabled: true,
            shorter: None,
            ink: 1.0,
        }
    }

    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub const fn shorter(mut self, label: &'a str) -> Self {
        self.shorter = Some(label);
        self
    }

    pub const fn inked(mut self, ink: f32) -> Self {
        self.ink = ink;
        self
    }
}

/// `buttons` as they are said in a band `room` wide: whole where their row
/// fits narrowed as far as it goes (see [`widths_in`]), and otherwise each
/// that has shorter words saying those — "Redémarrer" beside "Pas
/// maintenant" on a display standing on its side, where the shell's own
/// "Redémarrer maintenant" would be cut.
pub fn fitted<'a>(page: &mut Page, room: f32, buttons: &[Button<'a>]) -> Vec<Button<'a>> {
    let scale = page.scaled(1.0);
    let gap = page.metric(Metric::Gap);
    let tightest = |page: &mut Page, buttons: &[Button<'_>]| {
        buttons
            .iter()
            .map(|button| {
                tightest_at(
                    scale,
                    text::bold_width(page.ui(), Text::Label, button.label),
                )
            })
            .sum::<f32>()
            + gap * buttons.len().saturating_sub(1) as f32
    };
    if tightest(page, buttons) <= room {
        return buttons.to_vec();
    }
    buttons
        .iter()
        .map(|button| Button {
            label: button.shorter.unwrap_or(button.label),
            ..*button
        })
        .collect()
}

/// How tall a button is: the toolkit's own control height.
pub fn height(page: &Page) -> f32 {
    page.scaled(menu::ROW - 2.0 * control::PADDING)
}

/// How wide a button for `label` is: its word and the row padding either
/// side, and never so narrow that a short verb makes a stub. Measured in the
/// bold face, which every button draws its word in once the light is on it.
pub fn width(page: &mut Page, label: &str) -> f32 {
    let word = text::bold_width(page.ui(), Text::Label, label);
    width_at(page.scaled(1.0), word)
}

/// [`width`] at the design language's `scale`, for a word `word` wide.
pub fn width_at(scale: f32, word: f32) -> f32 {
    (word + Metric::RowPadding.value() * scale * 2.0).max(LEAST * scale)
}

/// The narrowest a button is, in reference pixels.
const LEAST: f32 = 200.0;

/// How much of its padding a button keeps, at the least, in a row short of
/// room.
const TIGHTEST: f32 = 0.5;

/// The narrowest a button for a word `word` wide is drawn at `scale`, in a
/// row short of room: its word and [`TIGHTEST`] of its padding either side.
/// What the catalogs' test holds a row of buttons to.
pub fn tightest_at(scale: f32, word: f32) -> f32 {
    word + Metric::RowPadding.value() * scale * TIGHTEST * 2.0
}

/// How wide each of a row of buttons whose words are `words` wide is, in a
/// band `room` wide, `gap` apart, at `scale`.
///
/// Each as [`width_at`] says where they fit. Where they do not — a display
/// standing on its side in a long language, "Uruchom ponownie teraz" beside
/// "Później" — they first give up the least width a short verb is kept to,
/// then as much of their padding as they have to, down to [`TIGHTEST`] of
/// it; only past that do they share the band equally, each word cut to its
/// share.
fn widths_in(scale: f32, words: &[f32], room: f32, gap: f32) -> Vec<f32> {
    if words.is_empty() {
        return Vec::new();
    }
    let gaps = gap * (words.len() - 1) as f32;
    let natural: Vec<f32> = words.iter().map(|word| width_at(scale, *word)).collect();
    if natural.iter().sum::<f32>() + gaps <= room {
        return natural;
    }
    let pad = Metric::RowPadding.value() * scale;
    let spare = room - gaps - words.iter().sum::<f32>();
    let kept = (spare / (pad * 2.0 * words.len() as f32)).min(1.0);
    if kept >= TIGHTEST {
        return words.iter().map(|word| word + pad * 2.0 * kept).collect();
    }
    let share = (room - gaps) / words.len() as f32;
    vec![share.max(1.0); words.len()]
}

/// Lay a row of buttons out along the bottom of `band`, right-aligned, the
/// last one in the corner. Answers one rectangle per button, in order.
/// Where the row is wider than the band the buttons narrow first and are
/// cut last (see [`widths_in`]).
pub fn places(page: &mut Page, band: [f32; 4], buttons: &[Button<'_>]) -> Vec<[f32; 4]> {
    let gap = page.metric(Metric::Gap);
    let tall = height(page);
    let top = band[1] + band[3] - tall;
    let words: Vec<f32> = buttons
        .iter()
        .map(|button| text::bold_width(page.ui(), Text::Label, button.label))
        .collect();
    let widths = widths_in(page.scaled(1.0), &words, band[2], gap);
    let total = widths.iter().sum::<f32>() + gap * buttons.len().saturating_sub(1) as f32;
    let mut x = band[0] + band[2] - total;
    widths
        .into_iter()
        .map(|w| {
            let rect = [x, top, w, tall];
            x += w + gap;
            rect
        })
        .collect()
}

/// The next button from `from` in `direction` (±1) that can be pressed, or
/// `None` where there is none that way.
pub fn step(buttons: &[Button<'_>], from: usize, direction: isize) -> Option<usize> {
    let mut at = from as isize + direction;
    while at >= 0 && (at as usize) < buttons.len() {
        if buttons[at as usize].enabled {
            return Some(at as usize);
        }
        at += direction;
    }
    None
}

/// The last button that can be pressed: where the light lands when it comes
/// down into a row of them, because the primary verb stands in the corner.
pub fn last_enabled(buttons: &[Button<'_>]) -> Option<usize> {
    buttons.iter().rposition(|button| button.enabled)
}

/// What drawing a row of buttons left on the frame, in the order it was
/// drawn: the light laid on the lit one first, then each button — what a
/// change of page keeps standing where the next page has the same button in
/// the same place (Continue), and fades where it has not.
#[derive(Debug, Clone, Default)]
pub struct LaidRow {
    /// The part of the scene the light on the lit button is.
    pub light: Option<(Drawn, Drawn)>,
    pub buttons: Vec<Laid>,
}

impl LaidRow {
    /// Every part of the scene the row is, in the order it was drawn.
    pub fn parts(&self) -> Vec<(Drawn, Drawn)> {
        self.light
            .into_iter()
            .chain(self.buttons.iter().map(|button| button.glass))
            .collect()
    }

    /// The button the light was laid on.
    pub fn lit(&self) -> Option<&Laid> {
        self.light
            .and(self.buttons.iter().find(|button| button.lit))
    }
}

/// One button of a [`LaidRow`]: the button as it was drawn, the part of the
/// scene that is the whole of it — its glass, then its word — and where its
/// word begins.
#[derive(Debug, Clone)]
pub struct Laid {
    pub label: String,
    pub kind: Kind,
    pub enabled: bool,
    pub rect: [f32; 4],
    pub lit: bool,
    pub glass: (Drawn, Drawn),
    pub word: Drawn,
}

impl Laid {
    /// Whether `other` is the same button standing in the same place: the
    /// same word, meaning and state, within half a pixel.
    pub fn same_as(&self, other: &Laid) -> bool {
        self.label == other.label && self.same_place(other)
    }

    /// Whether `other` is a button of the same meaning and state standing in
    /// the same place, whatever its word: one whose glass stands still while
    /// its word changes (Continue becoming Done).
    pub fn same_place(&self, other: &Laid) -> bool {
        self.kind == other.kind
            && self.enabled == other.enabled
            && self
                .rect
                .iter()
                .zip(other.rect)
                .all(|(a, b)| (a - b).abs() < 0.5)
    }
}

/// Draw a row of buttons at `places`, with the light on `lit` if it is in
/// this row. `pressing` is how far through a press the page is. Answers what
/// the row left on the frame (see [`LaidRow`]).
pub fn row(
    page: &mut Page,
    places: &[[f32; 4]],
    buttons: &[Button<'_>],
    lit: Option<usize>,
    light: &mut Light,
    pressing: Option<f32>,
    dt: f32,
) -> LaidRow {
    row_numbered(
        page,
        places,
        buttons,
        lit,
        light,
        pressing,
        dt,
        spots::BUTTONS,
    )
}

/// [`row`], its buttons drawn with the numbers from `first` on rather than
/// the row's own (`spots::BUTTONS`): a button standing apart from the
/// screen's row — Where to install's Advanced, at the other end of the band
/// from Options and Continue — which must answer to a number of its own.
#[allow(clippy::too_many_arguments)]
pub fn row_numbered(
    page: &mut Page,
    places: &[[f32; 4]],
    buttons: &[Button<'_>],
    lit: Option<usize>,
    light: &mut Light,
    pressing: Option<f32>,
    dt: f32,
    first: u32,
) -> LaidRow {
    let lit = lit.filter(|index| *index < places.len().min(buttons.len()));
    let light_at = lit.map(|index| {
        let from = page.ui().drawn();
        let sunk = motion::pressed(places[index], pressing);
        light.lay(page, sunk, Shape::Capsule, pressing, dt);
        (from, page.ui().drawn())
    });
    let mut laid = Vec::with_capacity(buttons.len());
    for (index, (button, rect)) in buttons.iter().zip(places).enumerate() {
        let is_lit = lit == Some(index);
        let press = match pressing {
            Some(through) if is_lit => Press::Going(through),
            _ => Press::from(is_lit),
        };
        let from = page.ui().drawn();
        if button.enabled {
            page.ui().spot(first + index as u32, *rect);
        }
        let word = draw(page, *rect, *button, press);
        laid.push(Laid {
            label: button.label.to_string(),
            kind: button.kind,
            enabled: button.enabled,
            rect: *rect,
            lit: is_lit,
            glass: (from, page.ui().drawn()),
            word,
        });
    }
    LaidRow {
        light: light_at,
        buttons: laid,
    }
}

/// One button, drawn resting, lit or dipped as `press` says. Answers where
/// its word begins on the frame: everything drawn before is its glass.
pub fn draw(page: &mut Page, rect: [f32; 4], button: Button<'_>, press: Press) -> Drawn {
    let lit = press.lit();
    let radius = rect[3] * 0.5;
    let ui = page.ui();
    if !button.enabled {
        ui.control_out(rect, radius, 1.0);
        let word = ui.drawn();
        let cut = text::fit(ui, Text::Label, button.label, rect[2] - rect[3] * 0.6);
        let ink = ui.tinted(Role::Text, 0.55);
        ui.label_tinted(rect, Text::Label, &cut, ink, Align::Centre);
        return word;
    }
    if button.kind == Kind::Danger && !lit {
        let sunk = motion::pressed(rect, press.through());
        ui.glow(motion::scaled_about_centre(sunk, 1.16), Role::Danger, 0.25);
    }
    let sunk = ui.control(rect, radius, press, rect[2], 1.0);
    if !lit {
        ui.chip(sunk, ui.tinted(Role::Glass, RESTING_WASH));
    }
    // Semantic colour goes over the neutral glass: the control surface is
    // substantial and hides a wash laid under it. The primary's accent is
    // only there while the light is elsewhere — under the light it would be
    // the same colour twice over.
    match button.kind {
        Kind::Primary if !lit => ui.chip(sunk, ui.tinted(Role::Accent, 0.22)),
        Kind::Danger => ui.chip(sunk, ui.tinted(Role::Danger, 0.18)),
        _ => {}
    }
    let word = ui.drawn();
    let bold = button.kind == Kind::Primary || lit;
    let room = sunk[2] - sunk[3] * 0.6;
    let cut = if bold {
        text::fit_bold(ui, Text::Label, button.label, room)
    } else {
        text::fit(ui, Text::Label, button.label, room)
    };
    let ink = ui.tinted(Role::Text, if lit { 1.0 } else { 0.88 } * button.ink);
    if button.ink > 0.0 {
        ui.label_weighted(sunk, Text::Label, &cut, ink, Align::Centre, bold);
    }
    word
}

/// Which button of a row a click landed on. Every button's press is taken
/// whether or not it is the one, so a stale click cannot linger.
pub fn pressed(page: &mut Page, buttons: &[Button<'_>]) -> Option<usize> {
    pressed_numbered(page, buttons, spots::BUTTONS)
}

/// [`pressed`], for buttons drawn with the numbers from `first` on
/// ([`row_numbered`]).
pub fn pressed_numbered(page: &mut Page, buttons: &[Button<'_>], first: u32) -> Option<usize> {
    let mut found = None;
    for (index, button) in buttons.iter().enumerate() {
        if page.pressed(first + index as u32) && button.enabled && found.is_none() {
            found = Some(index);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_short_of_room_narrows_its_buttons_before_it_cuts_a_word() {
        let gap = 14.0;
        // Room enough: each as wide as it is anywhere, the short verb kept
        // to the least width.
        assert_eq!(widths_in(1.0, &[100.0, 250.0], 800.0, gap), [200.0, 298.0]);
        // Short of room: the least width goes, then some of the padding —
        // and every word still fits inside its button.
        let tight = widths_in(1.0, &[150.0, 250.0], 470.0, gap);
        assert!(
            tight[0] >= 150.0 + 24.0 && tight[1] >= 250.0 + 24.0,
            "{tight:?}"
        );
        assert!(tight.iter().sum::<f32>() + gap <= 470.01, "{tight:?}");
        // Past half the padding: shared equally, the words cut.
        let shared = widths_in(1.0, &[220.0, 270.0], 470.0, gap);
        assert_eq!(shared[0], shared[1]);
    }

    #[test]
    fn the_light_steps_over_a_button_that_cannot_be_pressed() {
        let buttons = [
            Button::plain("Back"),
            Button::plain("Details").enabled(false),
            Button::primary("Install"),
        ];
        assert_eq!(step(&buttons, 0, 1), Some(2));
        assert_eq!(step(&buttons, 2, -1), Some(0));
        assert_eq!(step(&buttons, 2, 1), None);
        assert_eq!(last_enabled(&buttons), Some(2));
        let none = [Button::primary("Install").enabled(false)];
        assert_eq!(last_enabled(&none), None);
    }
}
