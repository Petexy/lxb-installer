//! Text fields: a name, a password, a computer's name, a place to try a
//! keyboard layout.
//!
//! The toolkit has no field of its own, so this is DistriBumpy's: a control
//! with a heading, the text (or, while it is empty, a hint) and a blinking
//! accent caret. Three sizes tell them apart: the heading the smallest
//! ([`text::HEADING`] of a caption) and white, the hint a caption in the
//! palette's soft ink ([`text::HINT`]), barely there, and what is typed the
//! body's size, in white. It is a capsule, the shape of the rows
//! around it, rather than DistriBumpy's card: a form here is a column of
//! rows, and a field that changed shape among them would read as a different
//! kind of thing. Three rules come with it.
//!
//! * **A field takes text only while the light stands in it.** Taking text
//!   everywhere spent Space on a letter instead of on Accept, and told the
//!   compositor a field had the cursor while the light was somewhere else.
//! * **With a keyboard in hand** the field takes text through
//!   `Page::taking_text` and says where it is with `Page::text_at` — which is
//!   also what summons LineXinBar's own on-screen keyboard where the
//!   installer runs under the shell.
//! * **With a pad in hand** Accept on a field opens the installer's own board
//!   ([`super::board`]), because the GNOME live session has no keyboard a pad
//!   can type with.
//!
//! A password field is masked: one dot a character, never the text, and the
//! value lives in a [`Secret`] that is overwritten when it is let go of.

use lxb_app::lxb_render::{Align, Press};
use lxb_app::lxb_toolkit::{metrics::Metric, motion, palette::Role, typography::Text};
use lxb_app::Page;

use super::button::RESTING_WASH;
use super::glyphs;
use super::motion::{Light, Shape};
use super::spots;
use super::text;
use crate::model::secret::Secret;

/// The longest text a field takes, in characters. The longest any value the
/// installer asks for may be (a password, a full name) is 128.
pub const MOST: usize = 128;

/// Every field the installer has. A screen says which one the light is
/// standing in, and the frame loop routes what is typed there; see
/// `State::edit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldId {
    FullName,
    Username,
    Password,
    Repeat,
    Hostname,
    RootPassword,
    RootRepeat,
    /// The keyboard step's "try it here" field. What is typed in it goes
    /// nowhere.
    KeyboardTest,
    /// The search over the list of the step the person is on: Language,
    /// Time zone or Keyboard (`super::search`).
    Search,
    /// A partition's label, on Partitions' New partition and Label… sheets.
    PartLabel,
    /// A mount point of one's own, on Partitions' Other… sheet.
    MountPath,
    /// A number typed for a size held on Partitions' New partition and
    /// Resize sheets, in GiB.
    PartSize,
}

impl FieldId {
    pub const ALL: [FieldId; 12] = [
        FieldId::FullName,
        FieldId::Username,
        FieldId::Password,
        FieldId::Repeat,
        FieldId::Hostname,
        FieldId::RootPassword,
        FieldId::RootRepeat,
        FieldId::KeyboardTest,
        FieldId::Search,
        FieldId::PartLabel,
        FieldId::MountPath,
        FieldId::PartSize,
    ];

    /// Whether what is typed is shown as dots.
    pub fn masked(self) -> bool {
        matches!(
            self,
            FieldId::Password | FieldId::Repeat | FieldId::RootPassword | FieldId::RootRepeat
        )
    }

    /// The mark at the field's start, as Review's rows wear theirs: the
    /// shell's name card, at sign, key and the installer's own key with a
    /// tick for the same password again, and a display with a name on it.
    /// The search field wears the shell's magnifier, as the shell's own
    /// searches do. The keyboard's test field has none: it asks nothing;
    /// nor do Partitions' sheets' fields, which stand among rows that wear
    /// none.
    pub fn mark(self) -> Option<&'static str> {
        match self {
            FieldId::FullName => Some(glyphs::FULL_NAME),
            FieldId::Username => Some(glyphs::USERNAME),
            FieldId::Password | FieldId::RootPassword => Some(glyphs::PASSWORD),
            FieldId::Repeat | FieldId::RootRepeat => Some(glyphs::PASSWORD_AGAIN),
            FieldId::Hostname => Some(glyphs::COMPUTER),
            FieldId::Search => Some(glyphs::SEARCH),
            FieldId::KeyboardTest | FieldId::PartLabel | FieldId::MountPath | FieldId::PartSize => {
                None
            }
        }
    }

    /// The number a click on this field comes back as.
    pub fn spot(self) -> u32 {
        spots::FIELDS + Self::ALL.iter().position(|one| *one == self).unwrap_or(0) as u32
    }
}

/// One thing typed into a field, from the keyboard or from the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    Wrote(String),
    Rubbed,
}

/// A value a field can edit: plain text, or a password.
pub trait Editable {
    fn write(&mut self, text: &str);
    fn rub(&mut self);
    fn length(&self) -> usize;
    /// What the field shows: the text, or a dot a character when masked.
    fn shown(&self, masked: bool) -> String;
}

impl Editable for String {
    fn write(&mut self, text: &str) {
        self.push_str(text);
    }
    fn rub(&mut self) {
        self.pop();
    }
    fn length(&self) -> usize {
        self.chars().count()
    }
    fn shown(&self, masked: bool) -> String {
        if masked {
            "•".repeat(self.length())
        } else {
            self.clone()
        }
    }
}

impl Editable for Secret {
    fn write(&mut self, text: &str) {
        self.push(text);
    }
    fn rub(&mut self) {
        self.pop();
    }
    fn length(&self) -> usize {
        self.chars()
    }
    fn shown(&self, _masked: bool) -> String {
        // A secret is shown masked whatever is asked: there is no field it
        // should ever be printed in.
        "•".repeat(self.chars())
    }
}

/// Apply one edit to a value, keeping it to [`MOST`] characters and leaving
/// out anything that is not printable (a newline from a paste, a tab).
pub fn apply(value: &mut dyn Editable, edit: &Edit) {
    match edit {
        Edit::Rubbed => value.rub(),
        Edit::Wrote(text) => {
            let room = MOST.saturating_sub(value.length());
            let kept: String = text
                .chars()
                .filter(|c| !c.is_control())
                .take(room)
                .collect();
            if !kept.is_empty() {
                value.write(&kept);
            }
        }
    }
}

/// What a field shows this frame.
#[derive(Debug, Clone, Copy)]
pub struct Look<'a> {
    /// What the field is for, on its first line.
    pub label: &'a str,
    /// What is in it, already masked where it should be.
    pub shown: &'a str,
    /// What it says while it is empty.
    pub placeholder: &'a str,
    /// Why what is in it cannot be used, if it cannot: said on the label's
    /// line, at the right, as a refusal.
    pub refused: Option<&'a str>,
    /// Whether the caret is drawn: the light is standing in it and something
    /// can type into it now.
    pub caret: bool,
}

/// How tall a field is: a row of the design language.
pub fn height(page: &Page) -> f32 {
    page.metric(Metric::RowHeight)
}

/// How much of a field `width` wide its words have, at `scale`: the row's
/// padding at each end, and its mark and the gap after it where it wears
/// one — every field but the keyboard's test field, at any width. You's
/// fields wear theirs on a display standing on its side too, as Review's
/// rows do there: every heading and hint of every language is held to fit
/// beside one at the narrowest form (`fit.rs`).
pub fn words_room_at(scale: f32, width: f32, id: FieldId) -> f32 {
    let pad = Metric::RowPadding.value() * scale;
    let mark = match id.mark() {
        Some(_) => mark_size_at(scale) + Metric::Gap.value() * scale,
        None => 0.0,
    };
    (width - pad * 2.0 - mark).max(0.0)
}

/// How large the mark at a field's start is, at `scale`: a column's row's,
/// which is Review's.
fn mark_size_at(scale: f32) -> f32 {
    Metric::ItemIcon.value() * scale * 0.6
}

/// Draw a field in `rect`, the light standing in it when `light` is handed
/// in. `now` is the installer's clock, which the caret blinks by.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    page: &mut Page,
    rect: [f32; 4],
    id: FieldId,
    look: Look<'_>,
    light: Option<&mut Light>,
    pressing: Option<f32>,
    dt: f32,
    now: f32,
) {
    let standing_in = light.is_some();
    let radius = rect[3] * 0.5;
    let pad = page.metric(Metric::RowPadding);
    let gap = page.metric(Metric::Gap);
    let label_line = page.line(Text::Caption) * text::HEADING;
    let value_line = page.line(Text::Body);

    // The light first, then the surface, which `Ui::control` fades where the
    // light has arrived over it — the order every control here keeps.
    page.ui().spot(id.spot(), rect);
    let surface = motion::pressed(rect, pressing.filter(|_| standing_in));
    let press = match pressing {
        Some(through) if standing_in => Press::Going(through),
        _ => Press::from(standing_in),
    };
    if let Some(light) = light {
        light.lay(page, surface, Shape::Capsule, pressing, dt);
        // Where the field is, told to the compositor along with the fact that
        // it has the cursor, so an on-screen keyboard can stand clear of it.
        page.text_at(rect);
    }
    let surface = page.ui().control(rect, radius, press, rect[2], 1.0);
    if !standing_in {
        // A field that is not the one in hand steps back into the page, as a
        // resting button does: left bright, it reads as lit whether it is or
        // not.
        let ui = page.ui();
        ui.chip(surface, ui.tinted(Role::Glass, RESTING_WASH));
    }

    let [x, y, w, h] = surface;
    let top = y + (h - label_line - value_line) * 0.5;
    let scale = page.scaled(1.0);
    let room = words_room_at(scale, w, id);
    let left = x + w - pad - room;
    let icons = page.icons();
    let ui = page.ui();
    if let Some(mark) = id.mark() {
        let size = mark_size_at(scale);
        ui.icon_tinted(
            [x + pad, y + (h - size) * 0.5, size, size],
            mark,
            icons,
            Role::Text,
            1.0,
        );
    }
    let (label_role, label_alpha) = text::LESSER;
    // The heading, and a refusal on its line, a little smaller than what is
    // typed under them, and smaller than the hint: the form's words, not the
    // person's.
    let refused_w = look
        .refused
        .map(|words| (ui.measure(Text::Caption, words) * text::HEADING).min(room * 0.6))
        .unwrap_or(0.0);
    text::label_scaled(
        ui,
        [left, top, (room - refused_w - gap).max(0.0), label_line],
        Text::Caption,
        look.label,
        label_role,
        label_alpha,
        Align::Left,
        text::HEADING,
    );
    if let Some(words) = look.refused {
        let (role, alpha) = text::REFUSAL;
        text::label_scaled(
            ui,
            [x + w - pad - refused_w, top, refused_w, label_line],
            Text::Caption,
            words,
            role,
            alpha,
            Align::Right,
            text::HEADING,
        );
    }
    let value = [left, top + label_line, room, value_line];
    if look.shown.is_empty() {
        // The hint, in the soft ink (`text::HINT`) and a caption's size:
        // barely there, so that only what is typed — white, the body's
        // size — reads as written. In white with every other lesser line, an
        // empty form would read as one already filled in.
        let (hint_role, hint_alpha) = text::HINT;
        text::label_faded(
            ui,
            value,
            Text::Caption,
            look.placeholder,
            hint_role,
            hint_alpha,
            Align::Left,
        );
    } else {
        // The end of what was typed is what matters while typing, so a value
        // too long for the field is shown from its tail.
        let shown = tail_that_fits(ui, look.shown, room - ui.s(8.0));
        ui.label(value, Text::Body, &shown, Role::Text, Align::Left);
    }
    // A bar that blinks, so an empty field still says a key would go here —
    // and only while something can type into it, because a caret blinking in
    // a field nothing would reach is a lie. It goes with what is typed, not
    // with the field's glass (`Ui::words_between`): a page coming in brought
    // it up ahead of the words, a caret alone in an empty-looking field.
    if look.caret && now.fract() < 0.55 {
        let shown = if look.shown.is_empty() {
            String::new()
        } else {
            tail_that_fits(ui, look.shown, room - ui.s(8.0))
        };
        let width = ui.measure(Text::Body, &shown);
        let thick = ui.s(2.0).max(1.0);
        let tall = value_line * 0.72;
        let from = ui.drawn();
        ui.rule(
            [
                value[0] + width + ui.s(3.0),
                value[1] + (value_line - tall) * 0.5,
                thick,
                tall,
            ],
            Role::Accent,
        );
        let to = ui.drawn();
        ui.words_between(from, to);
    }
}

/// The longest tail of `shown` that fits in `room`, with an ellipsis before
/// it where the head was left out.
fn tail_that_fits(ui: &mut lxb_app::lxb_render::Ui, shown: &str, room: f32) -> String {
    if ui.measure(Text::Body, shown) <= room {
        return shown.to_owned();
    }
    let letters: Vec<char> = shown.chars().collect();
    let mut keep = letters.len();
    while keep > 0 {
        let candidate: String = std::iter::once('…')
            .chain(letters[letters.len() - keep..].iter().copied())
            .collect();
        if ui.measure(Text::Body, &candidate) <= room {
            return candidate;
        }
        keep -= 1;
    }
    String::from("…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_typed_is_kept_to_the_longest_a_value_may_be() {
        let mut name = String::new();
        apply(&mut name, &Edit::Wrote("a".repeat(MOST + 10)));
        assert_eq!(name.chars().count(), MOST);
        apply(&mut name, &Edit::Wrote("more".into()));
        assert_eq!(name.chars().count(), MOST, "a full field took more");
        apply(&mut name, &Edit::Rubbed);
        assert_eq!(name.chars().count(), MOST - 1);
    }

    #[test]
    fn nothing_unprintable_reaches_a_value() {
        let mut name = String::new();
        apply(&mut name, &Edit::Wrote("Zoë\n\tO'Brien".into()));
        assert_eq!(name, "ZoëO'Brien");
    }

    #[test]
    fn a_password_is_shown_as_dots_whatever_is_asked() {
        let mut password = Secret::default();
        apply(&mut password, &Edit::Wrote("zażółć".into()));
        assert_eq!(password.shown(false), "••••••");
        assert_eq!(password.expose(), "zażółć");
        apply(&mut password, &Edit::Rubbed);
        assert_eq!(password.expose(), "zażół");
        assert_eq!("plain".to_string().shown(true), "•••••");
    }

    #[test]
    fn every_field_wears_its_mark_however_narrow_but_the_keyboards_test() {
        for width in [300.0, 470.0, 900.0] {
            assert_eq!(FieldId::Search.mark(), Some(glyphs::SEARCH));
            assert_eq!(FieldId::FullName.mark(), Some(glyphs::FULL_NAME));
            assert_eq!(FieldId::KeyboardTest.mark(), None);
            // The mark and the gap after it come out of the words' room.
            let pad = Metric::RowPadding.value();
            assert!(words_room_at(1.0, width, FieldId::FullName) < width - pad * 2.0);
            assert_eq!(
                words_room_at(1.0, width, FieldId::KeyboardTest),
                width - pad * 2.0
            );
        }
    }

    #[test]
    fn every_field_answers_to_a_number_of_its_own() {
        let mut seen = std::collections::BTreeSet::new();
        for id in FieldId::ALL {
            assert!(seen.insert(id.spot()), "{id:?} shares a number");
        }
        assert!(FieldId::Password.masked() && !FieldId::FullName.masked());
    }
}
