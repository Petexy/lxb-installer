//! The installer's own on-screen keyboard, for somebody holding a pad.
//!
//! The toolkit's only board is private to its file chooser, and the GNOME live
//! session has none a pad can type with, so the installer brings its own,
//! modelled on that private one: a pane of keys rising from the foot of the
//! panel, a light that walks it, and a face of letters, one of digits and
//! symbols, and one of the accented letters the ten languages need (a Pole
//! must be able to type their own name and a password in their own letters).
//! With a keyboard in hand it never appears — the field takes text the
//! ordinary way, which under LineXinBar is also what raises the shell's own
//! board.
//!
//! On a pad:
//!
//! * the D-pad or the stick walks the keys, wrapping round each row;
//! * **South** types the key under the light;
//! * **North** is Shift — once for one capital, twice to hold it;
//! * **the left bumper** rubs out the last character and **the right
//!   bumper** turns to the next face;
//! * **Start** is Done, and **East** closes the board, keeping what was typed.
//!
//! West would have been the natural rub-out, as on a console's own board, but
//! the toolkit's input layer does not report it (`Action::of_button` answers
//! `None` for West), so the left bumper does it and the board has a rub-out
//! key of its own as well.
//!
//! The letters follow the keyboard layout chosen on the keyboard step where
//! that layout is one of the three common arrangements (QWERTY, QWERTZ,
//! AZERTY), so the board under the person's thumb is the keyboard they will
//! have under their fingers.
//!
//! **Under LineXinBar this board is never raised.** The shell has a board of
//! its own, the one every application on it types with, and it answers a
//! field that takes text (`Page::taking_text`, which the toolkit says to the
//! compositor as Wayland's text input) by rising over it while a pad is in
//! hand. There a pad's Accept on a field makes the field take text instead
//! (`State::type_with_pad`), and the shell's board comes; this one stays the
//! board of every other session — the GNOME live session among them, which
//! has none a pad can type with. Which of the two it is, is
//! [`under_linexinbar`].

use std::ffi::OsStr;

use lxb_app::lxb_render::{Align, Press};
use lxb_app::lxb_toolkit::{
    input::Action, material::Surface, motion, palette::Role, typography::Text,
};
use lxb_app::Page;

use super::field::FieldId;
use super::legend::{hint, Button, Hint};
use super::motion::{Carried, Light, Shape};
use super::spots;

/// Whether the installer's window is one of LineXinBar's, whose own board
/// answers a field that takes text: the session's desktop names LineXinBar,
/// and the window is a Wayland one.
///
/// * `desktop` is `XDG_CURRENT_DESKTOP`, a `:`-separated list compared
///   without regard to case, as the shell itself reads it
///   (`apps::current_desktops`). LineXinBar's session sets it
///   (`lxb-session`), and so does its compositor for everything it starts
///   (`confine_to_session`), nested or not — the one name the session gives
///   an application that cannot be inherited from another desktop by
///   mistake. The compositor's own `lxb_shell_v1` is the shell's alone, and
///   an application's toolkit never sees the registry.
/// * `display` and `socket` are `WAYLAND_DISPLAY` and `WAYLAND_SOCKET`: the
///   shell's board answers Wayland's text input only, and a window on X11 —
///   an X server given to it with no Wayland socket — would summon nothing.
pub fn under_linexinbar(
    desktop: Option<&OsStr>,
    display: Option<&OsStr>,
    socket: Option<&OsStr>,
) -> bool {
    let named = desktop.and_then(OsStr::to_str).is_some_and(|desktops| {
        desktops
            .split(':')
            .any(|name| name.trim().eq_ignore_ascii_case("LineXinBar"))
    });
    let wayland = [display, socket]
        .into_iter()
        .flatten()
        .any(|value| !value.is_empty());
    named && wayland
}

/// [`under_linexinbar`], asked of this process's own environment: what a
/// window decides once, as it opens. A picture and a test never ask it.
pub fn under_linexinbar_here() -> bool {
    under_linexinbar(
        std::env::var_os("XDG_CURRENT_DESKTOP").as_deref(),
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
        std::env::var_os("WAYLAND_SOCKET").as_deref(),
    )
}

/// Which face of the board is showing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Face {
    #[default]
    Letters,
    Symbols,
    Accents,
}

impl Face {
    /// The face the face key turns to.
    pub fn next(self) -> Self {
        match self {
            Face::Letters => Face::Symbols,
            Face::Symbols => Face::Accents,
            Face::Accents => Face::Letters,
        }
    }

    /// What the face key says while this face is showing: the face it turns
    /// to, the way a phone's board labels it.
    fn key_label(self) -> &'static str {
        match self.next() {
            Face::Letters => crate::i18n::text("board-letters"),
            Face::Symbols => crate::i18n::text("board-symbols"),
            Face::Accents => crate::i18n::text("board-accents"),
        }
    }
}

/// Whether the next letter is a capital.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Shift {
    #[default]
    Off,
    /// For one letter, then off again.
    Once,
    /// Until Shift is pressed again.
    Held,
}

impl Shift {
    fn pressed(self) -> Self {
        match self {
            Shift::Off => Shift::Once,
            Shift::Once => Shift::Held,
            Shift::Held => Shift::Off,
        }
    }
}

/// The three arrangements of letters the board knows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Letters {
    #[default]
    Qwerty,
    Qwertz,
    Azerty,
}

impl Letters {
    /// The arrangement for an XKB layout name; QWERTY for anything else.
    pub fn for_layout(layout: &str) -> Self {
        match layout {
            "de" | "at" | "ch" | "cz" | "sk" | "hu" | "si" | "hr" | "rs" | "ba" | "me" => {
                Letters::Qwertz
            }
            "fr" | "be" => Letters::Azerty,
            _ => Letters::Qwerty,
        }
    }

    fn rows(self) -> [&'static str; 3] {
        match self {
            Letters::Qwerty => ["qwertyuiop", "asdfghjkl'", "zxcvbnm,.-"],
            Letters::Qwertz => ["qwertzuiop", "asdfghjkl'", "yxcvbnm,.-"],
            Letters::Azerty => ["azertyuiop", "qsdfghjklm", "wxcvbn,.'-"],
        }
    }
}

const DIGITS: &str = "1234567890";
const SYMBOLS: [&str; 3] = ["@#$%&*()_+", "!?\"':;/\\=~", "<>[]{}|^`€"];
const ACCENTS: [&str; 4] = ["àáâäãåæçèé", "êëìíîïñòóô", "öõøœùúûüýß", "ąćęłńśźżčš"];

/// One key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A character, typed as a capital while Shift is on.
    Char(char),
    Shift,
    /// Turns to the next face.
    Face,
    Space,
    Rub,
    Done,
}

/// What a press on the board did, for the frame loop to answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stroke {
    /// Nothing; nothing is heard.
    Nothing,
    /// The light moved to another key.
    Moved,
    /// Text to put in the field.
    Typed(String),
    /// Rub out the field's last character.
    Rubbed,
    /// Shift or the face changed.
    Changed,
    /// Done: close the board, the text is finished.
    Done,
    /// Closed with Back: close the board, keeping what was typed.
    Closed,
}

/// The board's state while it is open.
#[derive(Debug, Clone)]
pub struct Board {
    /// The field it types into.
    pub field: FieldId,
    face: Face,
    shift: Shift,
    letters: Letters,
    row: usize,
    column: usize,
    rise: Carried,
    /// Where the board's top was drawn on the last frame, so the light on
    /// its key rises with it (`Light::ride`).
    top: Option<f32>,
    /// Whether the light still stands on the field it came from, waiting for
    /// its key to rise into sight.
    waiting: bool,
    /// Whether a press on the board's keys has been made since it opened:
    /// the press that opened it was on the field, and dips the field.
    pressed: bool,
}

/// How many key widths a row is: ten keys, and the bottom row's five keys
/// sharing the same ten.
const UNITS: f32 = 10.0;

impl Board {
    /// A board typing into `field`, its letters arranged as `layout` (an XKB
    /// layout name) arranges them. Opens on the letters, the light on the
    /// first letter.
    pub fn open(field: FieldId, layout: &str) -> Self {
        let mut rise = Carried::default();
        rise.place(0.0);
        Self {
            field,
            face: Face::Letters,
            shift: Shift::Off,
            letters: Letters::for_layout(layout),
            row: 1,
            column: 0,
            rise,
            top: None,
            waiting: true,
            pressed: false,
        }
    }

    pub fn face(&self) -> Face {
        self.face
    }

    pub fn shift(&self) -> Shift {
        self.shift
    }

    /// The keys of one row of the face showing, with how many key widths
    /// each takes.
    pub fn row_keys(&self, row: usize) -> Vec<(Key, f32)> {
        let chars = |line: &str| line.chars().map(|c| (Key::Char(c), 1.0)).collect();
        match (self.face, row) {
            (_, 4) => vec![
                (Key::Shift, 1.5),
                (Key::Face, 1.5),
                (Key::Space, 4.0),
                (Key::Rub, 1.5),
                (Key::Done, 1.5),
            ],
            (Face::Letters, 0) | (Face::Symbols, 0) => chars(DIGITS),
            (Face::Letters, n) => chars(self.letters.rows()[n - 1]),
            (Face::Symbols, n) => chars(SYMBOLS[n - 1]),
            (Face::Accents, n) => chars(ACCENTS[n]),
        }
    }

    /// How many rows every face has.
    pub const ROWS: usize = 5;

    /// The key under the light.
    pub fn key(&self) -> Key {
        let keys = self.row_keys(self.row);
        keys[self.column.min(keys.len() - 1)].0
    }

    /// Where along its row a key's middle is, in key widths.
    fn middle_of(&self, row: usize, column: usize) -> f32 {
        let keys = self.row_keys(row);
        let before: f32 = keys.iter().take(column).map(|(_, units)| units).sum();
        before + keys[column.min(keys.len() - 1)].1 * 0.5
    }

    /// The key of `row` whose middle is nearest `middle`.
    fn nearest_in(&self, row: usize, middle: f32) -> usize {
        let keys = self.row_keys(row);
        let mut at = 0.0;
        let mut best = (0, f32::MAX);
        for (index, (_, units)) in keys.iter().enumerate() {
            let distance = (at + units * 0.5 - middle).abs();
            if distance < best.1 {
                best = (index, distance);
            }
            at += units;
        }
        best.0
    }

    /// Answer one press.
    pub fn act(&mut self, action: Action) -> Stroke {
        match action {
            Action::Left | Action::Right => {
                let count = self.row_keys(self.row).len();
                self.column = if action == Action::Left {
                    (self.column + count - 1) % count
                } else {
                    (self.column + 1) % count
                };
                Stroke::Moved
            }
            Action::Up | Action::Down => {
                let to = if action == Action::Up {
                    self.row.checked_sub(1)
                } else {
                    (self.row + 1 < Self::ROWS).then_some(self.row + 1)
                };
                let Some(to) = to else {
                    return Stroke::Nothing;
                };
                let middle = self.middle_of(self.row, self.column);
                self.column = self.nearest_in(to, middle);
                self.row = to;
                Stroke::Moved
            }
            Action::Accept => {
                self.pressed = true;
                self.press(self.key())
            }
            Action::Submit => Stroke::Done,
            Action::Back => Stroke::Closed,
            Action::Menu => self.press(Key::Shift),
            Action::Previous => Stroke::Rubbed,
            Action::Next => self.press(Key::Face),
        }
    }

    /// Press a key: the one under the light, or one a pointer landed on.
    pub fn press(&mut self, key: Key) -> Stroke {
        match key {
            Key::Char(c) => {
                let typed: String = if self.shift == Shift::Off {
                    c.to_string()
                } else {
                    c.to_uppercase().collect()
                };
                if self.shift == Shift::Once {
                    self.shift = Shift::Off;
                }
                Stroke::Typed(typed)
            }
            Key::Shift => {
                self.shift = self.shift.pressed();
                Stroke::Changed
            }
            Key::Face => {
                self.face = self.face.next();
                // The same place on the new face, which has the same rows.
                let middle = self.middle_of(self.row.min(Self::ROWS - 1), self.column);
                self.column = self.nearest_in(self.row, middle);
                Stroke::Changed
            }
            Key::Space => Stroke::Typed(" ".into()),
            Key::Rub => Stroke::Rubbed,
            Key::Done => Stroke::Done,
        }
    }

    /// The legend while the board is open.
    pub fn hints() -> Vec<Hint> {
        vec![
            hint(crate::i18n::text("hint-type"), Button::Accept),
            hint(crate::i18n::text("hint-shift"), Button::Options),
            hint(crate::i18n::text("hint-close"), Button::Back),
            hint(crate::i18n::text("hint-done"), Button::Start),
        ]
    }

    /// How wide a one-unit key is on a board `width` wide: as wide as the
    /// board allows, but no wider than a key should be — on a wide panel the
    /// board stands in the middle of it.
    fn unit(page: &Page, width: f32) -> f32 {
        let inner = width - page.scaled(PAD) * 2.0;
        let gap = page.scaled(KEY_GAP);
        ((inner - gap * (UNITS - 1.0)) / UNITS)
            .min(page.scaled(KEY_WIDEST))
            .max(1.0)
    }

    /// How tall a key is on a board `width` wide: its reference height, or
    /// as wide as a one-unit key where the board is narrower than that. A
    /// panel standing on its side keeps the size its height gives, and keys
    /// that kept theirs there would be tall pills nearly twice as high as they
    /// are wide, a board taking half the panel; square keys re-flow it.
    fn key_height(page: &Page, width: f32) -> f32 {
        page.scaled(KEY).min(Self::unit(page, width))
    }

    /// How tall the board is, on a panel `width` wide, when it has risen all
    /// the way.
    pub fn height(page: &Page, width: f32) -> f32 {
        let key = Self::key_height(page, width);
        let gap = page.scaled(KEY_GAP);
        key * Self::ROWS as f32 + gap * (Self::ROWS as f32 - 1.0) + page.scaled(PAD) * 2.0
    }

    /// How far it has risen, nought to one: what the room above it gives up.
    pub fn risen(&self) -> f32 {
        motion::ease(self.rise.at().clamp(0.0, 1.0))
    }

    /// Carry the board a frame further up.
    pub fn advance(&mut self, dt: f32) {
        self.rise.advance(1.0, dt);
    }

    pub fn settle(&mut self) {
        self.rise.place(1.0);
        self.waiting = false;
    }

    /// Whether the light waited on the field on this frame for the key
    /// under it, standing at `key`, to rise into sight in `room`; once it is
    /// wholly in sight, the light is the board's from the next frame. Laid by
    /// the screen on this one, it is not laid twice.
    fn waited_for(&mut self, key: [f32; 4], room: [f32; 4]) -> bool {
        let waited = self.waiting;
        self.waiting &= key[1] + key[3] > room[1] + room[3] + 0.5;
        waited
    }

    /// Whether the board has laid the light on its keys yet: until it has,
    /// the light is where the field it came from put it, and goes where the
    /// field goes.
    pub fn has_light(&self) -> bool {
        self.top.is_some()
    }

    /// Whether the light is waiting on the field for its key to rise into
    /// sight: the screen under the board lays it there meanwhile, on the
    /// field as it stands, which the board's rise may be moving.
    pub fn waiting(&self) -> bool {
        self.waiting
    }

    pub fn moving(&self) -> bool {
        self.rise.moving()
    }

    /// Where every key of the face showing goes, row by row, in a board at
    /// `rect`.
    fn places(&self, page: &Page, rect: [f32; 4]) -> Vec<Vec<(Key, [f32; 4])>> {
        let pad = page.scaled(PAD);
        let gap = page.scaled(KEY_GAP);
        let tall = Self::key_height(page, rect[2]);
        let unit = Self::unit(page, rect[2]);
        let width = unit * UNITS + gap * (UNITS - 1.0);
        let left = rect[0] + (rect[2] - width) * 0.5;
        (0..Self::ROWS)
            .map(|row| {
                let keys = self.row_keys(row);
                let row_width: f32 = keys
                    .iter()
                    .map(|(_, units)| units * unit + gap * (units - 1.0))
                    .sum::<f32>()
                    + gap * (keys.len() as f32 - 1.0);
                let mut x = left + (width - row_width) * 0.5;
                let y = rect[1] + pad + row as f32 * (tall + gap);
                keys.into_iter()
                    .map(|(key, units)| {
                        let w = units * unit + gap * (units - 1.0);
                        let place = [x, y, w, tall];
                        x += w + gap;
                        (key, place)
                    })
                    .collect()
            })
            .collect()
    }

    /// Draw the board risen into the foot of `room`, the light on its key.
    ///
    /// It rises out of the foot of `room` as the shell's board rises out of
    /// the display's edge: what is still below that foot is not drawn (the
    /// frame cuts the board to `room`), and the light on its key rises with
    /// its key rather than gliding after it — left to glide, it hung below
    /// the board's last row, outside the panel, while the keys came up.
    ///
    /// **The light is never nowhere.** The board opens on its first letter,
    /// on the second row, which is still under the foot of `room` for the
    /// first frames of the rise: put on it at once, the light was cut off
    /// with it and nothing on the screen was lit for four frames. It stays on
    /// the field it came from — the screen still lays it there, while
    /// [`Board::waiting`] — until that key has risen wholly into sight, and
    /// from the next frame the board lays it, going over to the key and
    /// riding it from there.
    pub fn draw(
        &mut self,
        page: &mut Page,
        room: [f32; 4],
        light: &mut Light,
        pressing: Option<f32>,
        dt: f32,
    ) {
        // The press that opened the board landed on the field, and dips the
        // field; none of the keys was pressed by it.
        let pressing = pressing.filter(|_| self.pressed);
        let tall = Self::height(page, room[2]);
        let risen = self.risen();
        let rect = [room[0], room[1] + room[3] - tall * risen, room[2], tall];
        let icons = page.icons();
        page.ui().card(rect, Surface::Panel, Role::Glass, 0.55);
        let places = self.places(page, rect);
        if let Some((_, lit)) = places.get(self.row).and_then(|row| row.get(self.column)) {
            if !self.waited_for(*lit, room) {
                if let Some(top) = self.top.replace(rect[1]) {
                    light.ride([0.0, rect[1] - top, 0.0, 0.0]);
                }
                light.lay(
                    page,
                    motion::pressed(*lit, pressing),
                    Shape::Key,
                    pressing,
                    dt,
                );
            }
        }
        let mut number = 0u32;
        for (row, keys) in places.iter().enumerate() {
            for (column, (key, place)) in keys.iter().enumerate() {
                let here = row == self.row && column == self.column;
                let press = match pressing {
                    Some(through) if here => Press::Going(through),
                    _ => Press::from(here),
                };
                let ui = page.ui();
                ui.spot(spots::BOARD + number, *place);
                number += 1;
                let radius = place[3] * 0.28;
                let sunk = ui.control(*place, radius, press, place[2], 1.0);
                // A key holding the board in a state — Shift on — is drawn
                // pressed into the panel, darker, rather than brighter: the
                // light is what is bright.
                let held = matches!(key, Key::Shift) && self.shift != Shift::Off;
                if held {
                    let strength = if self.shift == Shift::Held { 0.6 } else { 0.35 };
                    ui.chip(sunk, ui.tinted(Role::Glass, strength));
                }
                let ink = ui.tinted(Role::Text, if here { 1.0 } else { 0.85 });
                let mark = |name: &'static str, ui: &mut lxb_app::lxb_render::Ui| {
                    let size = sunk[3] * 0.5;
                    ui.icon_tinted(
                        [
                            sunk[0] + (sunk[2] - size) * 0.5,
                            sunk[1] + (sunk[3] - size) * 0.5,
                            size,
                            size,
                        ],
                        name,
                        icons,
                        Role::Text,
                        if here { 1.0 } else { 0.85 },
                    );
                };
                match key {
                    Key::Char(c) => {
                        let shown: String = if self.shift == Shift::Off {
                            c.to_string()
                        } else {
                            c.to_uppercase().collect()
                        };
                        ui.label_tinted(sunk, Text::Body, &shown, ink, Align::Centre);
                    }
                    Key::Shift => mark("arrow-up", ui),
                    Key::Space => mark("key-space", ui),
                    Key::Rub => mark("arrow-left", ui),
                    Key::Face => {
                        let room = sunk[2] - sunk[3] * 0.3;
                        let cut = super::text::fit(ui, Text::Label, self.face.key_label(), room);
                        ui.label_tinted(sunk, Text::Label, &cut, ink, Align::Centre)
                    }
                    Key::Done => {
                        // The word where it fits, and the `chosen` mark where
                        // it does not: a narrow board in a long language would
                        // otherwise run Done out over the key's edge, and an
                        // ellipsis on the one key that finishes the text is no
                        // label at all. Bold is measured as a little wider
                        // than the regular face the measurer knows.
                        let word = crate::i18n::text("hint-done");
                        let room = sunk[2] - sunk[3] * 0.3;
                        if ui.measure(Text::Label, word) * 1.08 <= room {
                            ui.label_weighted(sunk, Text::Label, word, ink, Align::Centre, true);
                        } else {
                            mark("chosen", ui);
                        }
                    }
                }
            }
        }
    }

    /// Put the light on the key drawn with the number `id`, where the
    /// pointer moved onto it. Answers whether it is one of the board's keys.
    pub fn point_at(&mut self, id: u32) -> bool {
        let Some(mut number) = id.checked_sub(spots::BOARD) else {
            return false;
        };
        for row in 0..Self::ROWS {
            let keys = self.row_keys(row).len() as u32;
            if number < keys {
                self.row = row;
                self.column = number as usize;
                return true;
            }
            number -= keys;
        }
        false
    }

    /// The key a click landed on, if it landed on the board: the light goes
    /// there and the key is pressed at once — a keyboard is not a list, and a
    /// key that needed two clicks would be a key nobody could type with.
    pub fn pressed(&mut self, page: &mut Page) -> Option<Stroke> {
        let mut number = 0u32;
        let mut found = None;
        for row in 0..Self::ROWS {
            for column in 0..self.row_keys(row).len() {
                if page.pressed(spots::BOARD + number) && found.is_none() {
                    found = Some((row, column));
                }
                number += 1;
            }
        }
        let (row, column) = found?;
        self.row = row;
        self.column = column;
        Some(self.press(self.key()))
    }
}

/// A key's height, the widest a one-unit key gets, the gap between keys and
/// the board's own padding, in reference pixels.
const KEY: f32 = 62.0;
const KEY_WIDEST: f32 = 92.0;
const KEY_GAP: f32 = 8.0;
const PAD: f32 = 16.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// LineXinBar is told from its session's own name, in any case and
    /// anywhere in the list, and only on Wayland; every other desktop, and
    /// none, keeps the installer's own board.
    #[test]
    fn linexinbar_is_known_by_the_desktop_its_session_names_and_wayland() {
        let os = |text: &'static str| Some(OsStr::new(text));
        let wayland = os("wayland-0");
        assert!(under_linexinbar(os("LineXinBar"), wayland, None));
        assert!(under_linexinbar(os("linexinbar"), wayland, None));
        assert!(under_linexinbar(os("KDE: LineXinBar"), wayland, None));
        assert!(under_linexinbar(os("LineXinBar"), None, os("3"),));
        // A Wayland socket given as a whole path, as a nested session's is.
        assert!(under_linexinbar(
            os("LineXinBar"),
            os("/run/user/1000/lxb-0"),
            None
        ));
        for other in ["GNOME", "KDE", "ubuntu:GNOME", "LineXinBarNot", "", "LXB"] {
            assert!(!under_linexinbar(os(other), wayland, None), "{other}");
        }
        assert!(!under_linexinbar(None, wayland, None));
        // Named, but on X11: the shell's board answers no X11 window.
        assert!(!under_linexinbar(os("LineXinBar"), None, None));
        assert!(!under_linexinbar(os("LineXinBar"), os(""), None));
    }

    fn typed(board: &mut Board, action: Action) -> String {
        match board.act(action) {
            Stroke::Typed(text) => text,
            other => panic!("{other:?} instead of typing"),
        }
    }

    #[test]
    fn every_face_has_the_same_rows_and_every_row_the_same_width() {
        let mut board = Board::open(FieldId::FullName, "us");
        for _ in 0..3 {
            for row in 0..Board::ROWS {
                let units: f32 = board.row_keys(row).iter().map(|(_, units)| units).sum();
                assert_eq!(units, UNITS, "{:?} row {row}", board.face());
            }
            board.press(Key::Face);
        }
        assert_eq!(board.face(), Face::Letters, "three turns came back round");
    }

    #[test]
    fn hovering_a_key_lights_it_and_types_nothing() {
        let mut board = Board::open(FieldId::FullName, "us");
        let first_row = board.row_keys(0).len() as u32;
        assert!(board.point_at(spots::BOARD + first_row + 2));
        assert_eq!((board.row, board.column), (1, 2));
        assert_eq!(board.key(), Key::Char('e'));
        assert!(!board.point_at(spots::BOARD + 1000));
        assert!(!board.point_at(spots::BUTTONS));
        assert_eq!((board.row, board.column), (1, 2));
    }

    #[test]
    fn shift_once_is_one_capital_and_shift_held_is_all_of_them() {
        let mut board = Board::open(FieldId::FullName, "us");
        assert_eq!(typed(&mut board, Action::Accept), "q");
        board.act(Action::Menu);
        assert_eq!(typed(&mut board, Action::Accept), "Q");
        assert_eq!(typed(&mut board, Action::Accept), "q", "Shift stayed on");
        board.act(Action::Menu);
        board.act(Action::Menu);
        assert_eq!(board.shift(), Shift::Held);
        assert_eq!(typed(&mut board, Action::Accept), "Q");
        assert_eq!(typed(&mut board, Action::Accept), "Q");
    }

    #[test]
    fn accented_letters_have_capitals_too() {
        let mut board = Board::open(FieldId::Password, "pl");
        board.press(Key::Face);
        board.press(Key::Face);
        assert_eq!(board.face(), Face::Accents);
        assert_eq!(board.press(Key::Char('ł')), Stroke::Typed("ł".into()));
        board.press(Key::Shift);
        assert_eq!(board.press(Key::Char('ż')), Stroke::Typed("Ż".into()));
        assert_eq!(
            board.press(Key::Char('ß')),
            Stroke::Typed("ß".into()),
            "ß stays ß"
        );
    }

    #[test]
    fn the_light_wraps_round_a_row_and_keeps_its_place_between_rows() {
        let mut board = Board::open(FieldId::FullName, "us");
        board.act(Action::Left);
        assert_eq!(board.key(), Key::Char('p'), "Left from the first key wraps");
        board.act(Action::Down);
        board.act(Action::Down);
        board.act(Action::Down);
        assert_eq!(board.key(), Key::Done, "the right-hand end leads to Done");
        board.act(Action::Up);
        assert_eq!(board.key(), Key::Char('-'));
        assert_eq!(board.act(Action::Down), Stroke::Moved);
        assert_eq!(
            board.act(Action::Down),
            Stroke::Nothing,
            "fell off the bottom"
        );
        for _ in 0..4 {
            board.act(Action::Up);
        }
        assert_eq!(board.act(Action::Up), Stroke::Nothing, "fell off the top");
    }

    #[test]
    fn the_pad_buttons_do_what_the_legend_says() {
        let mut board = Board::open(FieldId::Username, "us");
        assert_eq!(board.act(Action::Submit), Stroke::Done);
        assert_eq!(board.act(Action::Back), Stroke::Closed);
        assert_eq!(board.act(Action::Previous), Stroke::Rubbed);
        assert_eq!(board.act(Action::Next), Stroke::Changed);
        assert_eq!(board.face(), Face::Symbols);
        assert_eq!(board.press(Key::Space), Stroke::Typed(" ".into()));
    }

    #[test]
    fn the_letters_follow_the_layout_the_person_chose() {
        let german = Board::open(FieldId::FullName, "de");
        assert_eq!(german.row_keys(1)[5].0, Key::Char('z'));
        let french = Board::open(FieldId::FullName, "fr");
        assert_eq!(french.row_keys(1)[0].0, Key::Char('a'));
        let other = Board::open(FieldId::FullName, "jp");
        assert_eq!(other.row_keys(1)[5].0, Key::Char('y'));
    }

    #[test]
    fn the_light_stays_on_the_field_until_its_key_has_risen_into_sight() {
        let mut board = Board::open(FieldId::KeyboardTest, "us");
        assert!(
            board.waiting(),
            "the light left the field as the board opened"
        );
        let room = [0.0, 0.0, 800.0, 600.0];
        // The first letter still under the room's foot, then partly in it:
        // the screen keeps the light on the field.
        for top in [640.0, 590.0] {
            assert!(board.waited_for([10.0, top, 40.0, 40.0], room));
            assert!(board.waiting(), "the light went to a key out of sight");
        }
        // Wholly in sight: the screen lays it this once more, and the board
        // has it from the next frame.
        assert!(board.waited_for([10.0, 550.0, 40.0, 40.0], room));
        assert!(!board.waiting());
        assert!(
            !board.waited_for([10.0, 640.0, 40.0, 40.0], room),
            "it came back"
        );
        // The press that opened the board was the field's; the board's own
        // keys are dipped only by a press of the board's.
        assert!(!board.pressed, "the press that opened it pressed a key");
        board.act(Action::Accept);
        assert!(board.pressed, "a key pressed did not dip");
        // A picture puts the board risen and the light on its key at once.
        let mut settled = Board::open(FieldId::FullName, "us");
        settled.settle();
        assert!(!settled.waiting());
    }
}
