//! The screens, one module each, all of one shape.
//!
//! Every screen module offers the same eight functions, and this module only
//! hands each call to the screen the person is on:
//!
//! * `enter(&mut State)` — the person has just arrived (forward or back): put
//!   the light where it starts and work out anything the screen lists;
//! * `head(&State) -> Head` — the mark, title and caption in the panel's head;
//! * `hints(&State) -> Vec<Hint>` — what the legend in the foot says, asked
//!   of the same screen that answers the presses, so the two cannot drift;
//! * `note(&State) -> Option<Note>` — a standing line beside the legend
//!   (a press's refusal, kept in `State::note`, is said before it);
//! * `act(&mut State, Action, Option<Scroll>)` — one press; the wheel gesture
//!   behind it, if it came from one, says which list it was turned over;
//! * `draw(&mut State, &mut Page, Room, lit)` — draw its own controls into
//!   the room's `content`, the part of the panel between its head and its
//!   buttons (less the on-screen keyboard while it is up), and its row of
//!   buttons into the room's `buttons`, which stand at one place for a panel
//!   size; the light is the screen's to lay only while `lit` is true;
//! * `pressed(&mut State, &mut Page)` — the pointer's presses, asked after
//!   drawing, of the spots that drawing wrote down: a click acts on the
//!   control it lands on, at once, as Accept on it would;
//! * `hover(&mut State, id)` — the pointer moved onto the control drawn with
//!   the number `id`: the light goes there, silently, where it can stand.
//!   **Hovering is selecting**, as in LineXinBar (`Shell::point_at`); a
//!   pointer resting still moves nothing, so a light a key or a pad moved
//!   stays where they put it.
//!
//! A screen never plays a sound, raises a question or closes the window
//! itself: it asks the state to (`State::sound`, `State::ask`,
//! `State::leave`, `State::menu`), and the frame does it.
//!
//! What a screen keeps between frames — where its light stands, how far its
//! columns are scrolled — is its own `View`, kept together in [`Views`] on
//! the state. Four more calls are made only of the screens that need them:
//! `settle` (put every column where it is going, for a photograph), `stage`
//! (stand where a named picture shows the screen), `chose` (the row of a
//! menu the screen raised) and `drawn` (the frame has drawn the screen, which
//! Time zone and Keyboard count the entry in force as seen by).

use lxb_app::lxb_render::Align;
use lxb_app::lxb_toolkit::{input::Action, palette::Role, sound::Sound, typography::Text};
use lxb_app::{Page, Scroll};

use super::app::{Screen, State};
use super::button::{self, Button};
use super::field::{self, Edit, FieldId};
use super::legend::{hint, Button as Pictured, Hint};
use super::panel::Note;
use super::text;
use crate::i18n;

pub mod account;
pub mod desktop;
pub mod done;
pub mod installing;
pub mod keyboard;
pub mod language;
pub mod partitions;
pub mod review;
pub mod sheets;
pub mod stopped;
pub mod target;
pub mod timezone;
pub mod welcome;

/// What every screen keeps between frames.
#[derive(Debug, Default, Clone)]
pub struct Views {
    pub welcome: welcome::View,
    pub language: language::View,
    pub timezone: timezone::View,
    pub keyboard: keyboard::View,
    pub target: target::View,
    /// Where to install's Partitions page, opened with Advanced.
    pub partitions: partitions::View,
    pub desktop: desktop::View,
    pub account: account::View,
    pub review: review::View,
    pub installing: installing::View,
    pub done: done::View,
    pub stopped: stopped::View,
}

/// Put every column and slide where it is going, for a photograph.
pub fn settle(state: &mut State) {
    state.views.partitions.settle();
    state.views.language.settle();
    state.views.timezone.settle();
    state.views.keyboard.settle();
    state.views.target.settle();
    state.views.desktop.settle();
    state.views.account.settle();
    state.views.review.settle();
    state.views.installing.settle();
    state.views.stopped.settle();
}

/// Stand the screen where the picture named `scene` shows it.
pub fn stage(state: &mut State, scene: &str) {
    match scene {
        "language" | "language-search" => language::stage(state, scene),
        "timezone"
        | "timezone-continue"
        | "timezone-places"
        | "timezone-search"
        | "timezone-search-nothing" => timezone::stage(state, scene),
        "keyboard"
        | "keyboard-layouts"
        | "keyboard-continue"
        | "keyboard-none"
        | "keyboard-search"
        | "keyboard-search-nothing" => keyboard::stage(state, scene),
        "target"
        | "target-erase"
        | "target-free"
        | "target-nothing"
        | "target-whole"
        | "target-kinds"
        | "target-options"
        | "target-storage"
        | "target-refused"
        | "target-partitions-in-use"
        | "partitions"
        | "partitions-nvme"
        | "partitions-msdos"
        | "partitions-tiny"
        | "partitions-blocked"
        | "partitions-menu"
        | "partitions-mount"
        | "partitions-table"
        | "partitions-planned"
        | "partitions-options"
        | "partitions-pending"
        | "partitions-missing"
        | "partitions-new"
        | "partitions-new-held"
        | "partitions-new-typed"
        | "partitions-resize"
        | "partitions-label"
        | "partitions-other"
        | "partitions-new-planned"
        | "partitions-continue-refused"
        | "partitions-portrait-slid"
        | "partitions-unmountable"
        | "partitions-measured"
        | "partitions-hibernated"
        | "partitions-hibernated-other"
        | "partitions-reading" => target::stage(state, scene),
        "desktop" | "desktop-options" | "desktop-software" | "desktop-loaders-plan" => {
            desktop::stage(state, scene)
        }
        "account" | "account-administrator" => account::stage(state, scene),
        "review" | "confirm" => review::stage(state),
        "review-partitions"
        | "confirm-partitions"
        | "installing-shrink"
        | "done-shrink"
        | "stopped-shrink"
        | "stopped-hibernated"
        | "stopped-needs-check"
        | "stopped-windows-in-use"
        | "stopped-windows-check"
        | "stopped-windows-unread"
        | "stopped-bitlocker"
        | "stopped-windows-shrunk" => review::stage_with_the_plan(state),
        "confirm-plan-pages" | "confirm-plan-last" => review::stage_with_the_plan_pages(state),
        "installing-details" => installing::stage(state),
        _ => {}
    }
}

/// The row chosen from the menu numbered `id`, which the screen the person
/// is on raised.
pub fn chose(state: &mut State, id: u32, row: usize) {
    if state.screen == Screen::Target {
        target::chose(state, id, row);
    }
}

/// Something typed into the search field (`FieldId::Search`), which belongs
/// to the screen the person is on.
pub fn searched(state: &mut State, edit: &Edit) {
    match state.screen {
        Screen::Language => language::searched(state, edit),
        Screen::Timezone => timezone::searched(state, edit),
        Screen::Keyboard => keyboard::searched(state, edit),
        _ => {}
    }
}

/// The installer's board was put away with Done, its typing into `field`
/// finished: a search's light lands on what it found, and a sheet of
/// Partitions takes a size typed for it and moves on from a field.
pub fn typed(state: &mut State, field: FieldId) {
    if matches!(
        field,
        FieldId::PartLabel | FieldId::MountPath | FieldId::PartSize
    ) {
        return partitions::typed(state, field);
    }
    if field != FieldId::Search {
        return;
    }
    match state.screen {
        Screen::Language => {
            language::land(state);
        }
        Screen::Timezone => {
            timezone::land(state);
        }
        Screen::Keyboard => {
            keyboard::land(state);
        }
        _ => {}
    }
}

/// The legend of a screen with a search: Choose, Search, Back — or, with a
/// pad in hand and the light in the search field, Type and Back.
pub fn search_hints(state: &State, in_field: bool) -> Vec<Hint> {
    if in_field && state.pad {
        return vec![
            hint(i18n::text("hint-type"), Pictured::Accept),
            hint(i18n::text("hint-back"), Pictured::Back),
        ];
    }
    vec![
        hint(i18n::text("hint-choose"), Pictured::Accept),
        hint(i18n::text("hint-search"), Pictured::Options),
        hint(i18n::text("hint-back"), Pictured::Back),
    ]
}

/// Draw a screen's search field in `rect`, saying `query`, or `placeholder`
/// while nothing is typed, the light in it when `lit`.
pub fn search_field(
    state: &mut State,
    page: &mut Page,
    rect: [f32; 4],
    query: &str,
    placeholder: &str,
    lit: bool,
) {
    if lit {
        state.in_field(FieldId::Search);
    }
    let caret = state.typing_into() == Some(FieldId::Search);
    let (pressing, dt, now) = (state.pressing.through(), state.clock.dt, state.clock.now);
    let light = lit.then_some(&mut state.light);
    field::draw(
        page,
        rect,
        FieldId::Search,
        field::Look {
            label: i18n::text("search-field"),
            shown: query,
            placeholder,
            refused: None,
            caret,
        },
        light,
        pressing,
        dt,
        now,
    );
}

/// Where a search that found nothing says so over a column of what it
/// found: in `room` under the column's heading, `heading` tall
/// (`trail::heading_tall`), across `field` — the search field — as
/// Language's line stands under its field. Laid over the heading, the two
/// were written over each other ("Places found" through "Nothing found").
pub fn under_heading(room: [f32; 4], heading: f32, field: Option<[f32; 4]>) -> [f32; 4] {
    let [x, w] = field.map_or([room[0], room[2]], |field| [field[0], field[2]]);
    [x, room[1] + heading, w, (room[3] - heading).max(1.0)]
}

/// Say in `rect` that a search found nothing, in a line of its own.
pub fn found_nothing(page: &mut Page, rect: [f32; 4], query: &str) {
    let said = crate::message!("search-nothing", "query" => text::unbroken(query.trim()));
    let line = page.line(Text::Body);
    let ui = page.ui();
    let words = text::fit(ui, Text::Body, &said, rect[2]);
    let (role, alpha) = text::LESSER;
    text::label_faded(
        ui,
        [rect[0], rect[1] + line * 0.5, rect[2], line],
        Text::Body,
        &words,
        role,
        alpha,
        Align::Centre,
    );
}

/// Where a screen draws on this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Room {
    /// Its own controls — columns, cards, rows, fields — under the head and
    /// over the buttons, less the on-screen keyboard while it is up.
    pub content: [f32; 4],
    /// The band its row of buttons stands in, right-aligned: at one place for
    /// a panel size on every screen, whatever the screen, the light, the
    /// legend or a note is doing (`panel::Panel`). Nothing else is drawn in
    /// it but what stands beside the buttons as one line with them.
    pub buttons: [f32; 4],
    /// Where a note standing on lines of its own over the buttons ends, if
    /// one does: what something standing beside the buttons and taller than
    /// them (Keyboard's try-it field) must stay under.
    pub note_ends: Option<f32>,
    /// How tall the clear space over `content` and under it is: from the
    /// head's foot down to it, and from its foot down to whatever stands
    /// next under it — the buttons, a note on lines of its own, the risen
    /// board. Where a list says it goes on past an end, as the family's
    /// menus do with an arrow, outside the end that fades it.
    pub clear: [f32; 2],
    /// Where the on-screen keyboard's top stands while it is up — rising or
    /// risen — over the buttons' band and the foot of `content`.
    pub board_top: Option<f32>,
}

impl Room {
    /// Where something `tall` standing on the line of the buttons goes, at
    /// `x`, `width` wide: its foot on the band's foot, so it reaches up from
    /// the buttons rather than down towards the legend — and never over a
    /// note standing on lines of its own above them.
    pub fn beside_buttons(&self, x: f32, width: f32, tall: f32, gap: f32) -> [f32; 4] {
        let band = self.buttons;
        let mut top = band[1] + band[3] - tall;
        if let Some(ends) = self.note_ends {
            top = top.max(ends + gap * 0.5);
        }
        [x, top, width, tall]
    }

    /// Where `rect` stands while the on-screen keyboard is up: where it is,
    /// or — where the board has risen over it — pushed up ahead of the
    /// board's top, `gap` clear of it, so a field being typed into on the
    /// line of the buttons is never typed into blind under the keys. Never
    /// above the top of `content`.
    pub fn over_board(&self, rect: [f32; 4], gap: f32) -> [f32; 4] {
        let Some(top) = self.board_top else {
            return rect;
        };
        let [x, y, w, h] = rect;
        [x, y.min(top - gap - h).max(self.content[1]), w, h]
    }
}

/// A block at most `widest` wide standing in the middle of `content`, as
/// tall as it: where a screen's rows, cards and fields stand — at a width
/// that suits them rather than stretched across the panel, and in its middle
/// rather than pushed to its left (the rows on Desktop and on its Options
/// step, and "Install updates", centred).
pub fn centred(content: [f32; 4], widest: f32) -> [f32; 4] {
    let width = content[2].min(widest).max(0.0);
    [
        content[0] + (content[2] - width) * 0.5,
        content[1],
        width,
        content[3],
    ]
}

/// What a screen's head says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub mark: &'static str,
    pub title: String,
    pub caption: String,
}

impl Head {
    pub fn new(mark: &'static str, title: &str, caption: &str) -> Self {
        Self {
            mark,
            title: title.to_string(),
            caption: caption.to_string(),
        }
    }
}

pub fn enter(state: &mut State) {
    match state.screen {
        Screen::Welcome => welcome::enter(state),
        Screen::Language => language::enter(state),
        Screen::Timezone => timezone::enter(state),
        Screen::Keyboard => keyboard::enter(state),
        Screen::Target => target::enter(state),
        Screen::Desktop => desktop::enter(state),
        Screen::Account => account::enter(state),
        Screen::Review => review::enter(state),
        Screen::Installing => installing::enter(state),
        Screen::Done => done::enter(state),
        Screen::Stopped => stopped::enter(state),
    }
}

/// Every title a step that shows where it is in the flow can have, in the
/// language spoken: what decides where the head's chips stand
/// (`panel::Needs::titles`).
pub fn step_titles() -> [&'static str; 12] {
    [
        "language-title",
        "timezone-title",
        "keyboard-title",
        "target-title",
        "partitions-title",
        "desktop-title",
        "desktop-options-title",
        "desktop-remove-title",
        "desktop-flatpaks-title",
        "desktop-bootloader-title",
        "account-title",
        "review-title",
    ]
    .map(i18n::text)
}

pub fn head(state: &State) -> Head {
    match state.screen {
        Screen::Welcome => welcome::head(state),
        Screen::Language => language::head(state),
        Screen::Timezone => timezone::head(state),
        Screen::Keyboard => keyboard::head(state),
        Screen::Target => target::head(state),
        Screen::Desktop => desktop::head(state),
        Screen::Account => account::head(state),
        Screen::Review => review::head(state),
        Screen::Installing => installing::head(state),
        Screen::Done => done::head(state),
        Screen::Stopped => stopped::head(state),
    }
}

pub fn hints(state: &State) -> Vec<Hint> {
    match state.screen {
        Screen::Welcome => welcome::hints(state),
        Screen::Language => language::hints(state),
        Screen::Timezone => timezone::hints(state),
        Screen::Keyboard => keyboard::hints(state),
        Screen::Target => target::hints(state),
        Screen::Desktop => desktop::hints(state),
        Screen::Account => account::hints(state),
        Screen::Review => review::hints(state),
        Screen::Installing => installing::hints(state),
        Screen::Done => done::hints(state),
        Screen::Stopped => stopped::hints(state),
    }
}

pub fn note(state: &State) -> Option<Note> {
    match state.screen {
        Screen::Welcome => welcome::note(state),
        Screen::Language => language::note(state),
        Screen::Timezone => timezone::note(state),
        Screen::Keyboard => keyboard::note(state),
        Screen::Target => target::note(state),
        Screen::Desktop => desktop::note(state),
        Screen::Account => account::note(state),
        Screen::Review => review::note(state),
        Screen::Installing => installing::note(state),
        Screen::Done => done::note(state),
        Screen::Stopped => stopped::note(state),
    }
}

pub fn act(state: &mut State, action: Action, scroll: Option<Scroll>) {
    match state.screen {
        Screen::Welcome => welcome::act(state, action, scroll),
        Screen::Language => language::act(state, action, scroll),
        Screen::Timezone => timezone::act(state, action, scroll),
        Screen::Keyboard => keyboard::act(state, action, scroll),
        Screen::Target => target::act(state, action, scroll),
        Screen::Desktop => desktop::act(state, action, scroll),
        Screen::Account => account::act(state, action, scroll),
        Screen::Review => review::act(state, action, scroll),
        Screen::Installing => installing::act(state, action, scroll),
        Screen::Done => done::act(state, action, scroll),
        Screen::Stopped => stopped::act(state, action, scroll),
    }
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    match state.screen {
        Screen::Welcome => welcome::draw(state, page, room, lit),
        Screen::Language => language::draw(state, page, room, lit),
        Screen::Timezone => timezone::draw(state, page, room, lit),
        Screen::Keyboard => keyboard::draw(state, page, room, lit),
        Screen::Target => target::draw(state, page, room, lit),
        Screen::Desktop => desktop::draw(state, page, room, lit),
        Screen::Account => account::draw(state, page, room, lit),
        Screen::Review => review::draw(state, page, room, lit),
        Screen::Installing => installing::draw(state, page, room, lit),
        Screen::Done => done::draw(state, page, room, lit),
        Screen::Stopped => stopped::draw(state, page, room, lit),
    }
}

/// The frame has drawn the screen the person is on (not a page on its way
/// out): Time zone and Keyboard count how long the entry in force has stood
/// on it in the second column (`trail::Seen`). Answers whether that is still
/// being counted — only frames count it, so the window is to keep drawing
/// them until it is done.
pub fn drawn(state: &mut State) -> bool {
    match state.screen {
        Screen::Timezone => timezone::drawn(state),
        Screen::Keyboard => keyboard::drawn(state),
        _ => false,
    }
}

pub fn hover(state: &mut State, id: u32) {
    match state.screen {
        Screen::Welcome => welcome::hover(state, id),
        Screen::Language => language::hover(state, id),
        Screen::Timezone => timezone::hover(state, id),
        Screen::Keyboard => keyboard::hover(state, id),
        Screen::Target => target::hover(state, id),
        Screen::Desktop => desktop::hover(state, id),
        Screen::Account => account::hover(state, id),
        Screen::Review => review::hover(state, id),
        Screen::Done => done::hover(state, id),
        // One button, always lit, or nothing to press at all.
        Screen::Installing | Screen::Stopped => {}
    }
}

/// Which page stands under the pointer: the screen, and within it the step's
/// Options pages on Desktop, a trail's second column on Time zone and
/// Keyboard, or on Where to install its Partitions page or the column a pair
/// that slides is slid over to. A click that changes it has put other
/// controls under a resting pointer (`State::click`).
///
/// Whether Time zone's or Keyboard's entry in force has been seen is not
/// told apart. Their Continue holds back a second click itself, as it holds
/// back any press, while the column its first click showed — rolled over to
/// the entry in force, open before and after — has not stood on the screen
/// for its dwell (`trail::Seen`). Told apart here, it held back every click
/// for half a second after a Keyboard kind was chosen by a click, which
/// sees it.
pub fn page(state: &State) -> (Screen, u8) {
    let views = &state.views;
    let within = match state.screen {
        Screen::Desktop => views.desktop.part as u8,
        Screen::Timezone => u8::from(views.timezone.trail.open),
        Screen::Keyboard => u8::from(views.keyboard.trail.open),
        // A sheet over Partitions puts other controls under the pointer.
        Screen::Target if views.target.part == target::Part::Partitions => {
            3 + u8::from(views.partitions.sheet.is_some())
        }
        Screen::Target => views.target.pair.slid_to().map_or(0, |side| 1 + side as u8),
        _ => 0,
    };
    (state.screen, within)
}

/// Which page of the installer is drawn: the screen, on Desktop which of
/// its Options pages, and on Where to install whether its Partitions page is
/// up. What a change of page moves between (`change::Change`) — a trail's
/// second column and a pair slid across move by themselves, so they are not
/// told apart here, as [`page`] tells them apart for a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    pub screen: Screen,
    pub part: desktop::Part,
    pub partitions: bool,
}

impl Shown {
    /// Where this page stands in the walk through the installer: the screens
    /// in the order they are met, the installation's own after the questions,
    /// on Desktop its Options page one step in from the step's own and a
    /// list of Options one further, and Partitions one step in from Where to
    /// install. A page further on comes in from the right; one further back
    /// from the left.
    pub fn order(self) -> (usize, u8) {
        const WALK: [Screen; 11] = [
            Screen::Welcome,
            Screen::Language,
            Screen::Timezone,
            Screen::Keyboard,
            Screen::Target,
            Screen::Desktop,
            Screen::Account,
            Screen::Review,
            Screen::Installing,
            Screen::Done,
            Screen::Stopped,
        ];
        let at = WALK.iter().position(|one| *one == self.screen).unwrap_or(0);
        let depth = match self.part {
            desktop::Part::Main => u8::from(self.partitions),
            desktop::Part::Options => 1,
            desktop::Part::Remove | desktop::Part::Flatpaks | desktop::Part::Bootloader => 2,
        };
        (at, depth)
    }
}

/// The page drawn now (see [`Shown`]).
pub fn shown(state: &State) -> Shown {
    Shown {
        screen: state.screen,
        part: match state.screen {
            Screen::Desktop => state.views.desktop.part,
            _ => desktop::Part::Main,
        },
        partitions: state.screen == Screen::Target
            && state.views.target.part == target::Part::Partitions,
    }
}

/// A page as it was drawn: which page it was ([`Shown`]), and what of where
/// its light stood the press that leaves it may change before the change's
/// first frame draws it going — Desktop's light on Done, which closing a page
/// of Options puts back on the column. The page going is drawn as it stood
/// ([`stand_on`]), not as the press left the state: drawn as the press left
/// it, its light glided from Done up into the list as the page went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stood {
    pub page: Shown,
    /// Desktop's page and its light on Done, and Where to install's page,
    /// kept whatever the screen so that standing back puts them back as they
    /// were.
    part: desktop::Part,
    on_done: bool,
    target: target::Part,
}

impl From<Shown> for Stood {
    /// A page standing with its light where a page's light starts.
    fn from(page: Shown) -> Self {
        Self {
            page,
            part: page.part,
            on_done: false,
            target: if page.partitions {
                target::Part::Partitions
            } else {
                target::Part::Main
            },
        }
    }
}

/// The page drawn now, as it stands (see [`Stood`]).
pub fn stood(state: &State) -> Stood {
    Stood {
        page: shown(state),
        part: state.views.desktop.part,
        on_done: state.views.desktop.on_done,
        target: state.views.target.part,
    }
}

/// Stand the state on `page` as it stood, for as long as it takes to draw
/// it: the screen, and Desktop's Options page and its light on Done. Answers
/// what it stood on before, for [`stand_back`] to put back.
pub fn stand_on(state: &mut State, page: Stood) -> Stood {
    let was = stood(state);
    state.screen = page.page.screen;
    if page.page.screen == Screen::Desktop {
        state.views.desktop.part = page.page.part;
        state.views.desktop.on_done = page.on_done;
    }
    if page.page.screen == Screen::Target {
        state.views.target.part = page.target;
    }
    was
}

/// Put back what [`stand_on`] stood the state on.
pub fn stand_back(state: &mut State, was: Stood) {
    state.screen = was.page.screen;
    state.views.desktop.part = was.part;
    state.views.desktop.on_done = was.on_done;
    state.views.target.part = was.target;
}

/// The button of a row a spot number is, if it is one of `count` buttons.
pub fn button_of(id: u32, count: usize) -> Option<usize> {
    let index = id.checked_sub(super::spots::BUTTONS)? as usize;
    (index < count).then_some(index)
}

pub fn pressed(state: &mut State, page: &mut Page) {
    match state.screen {
        Screen::Welcome => welcome::pressed(state, page),
        Screen::Language => language::pressed(state, page),
        Screen::Timezone => timezone::pressed(state, page),
        Screen::Keyboard => keyboard::pressed(state, page),
        Screen::Target => target::pressed(state, page),
        Screen::Desktop => desktop::pressed(state, page),
        Screen::Account => account::pressed(state, page),
        Screen::Review => review::pressed(state, page),
        Screen::Installing => installing::pressed(state, page),
        Screen::Done => done::pressed(state, page),
        Screen::Stopped => stopped::pressed(state, page),
    }
}

// -------------------------------------------------------------------------
// What the placeholder screens share. Each of them is a head and one verb at
// the foot of the room; the screens built for real replace these with their
// own controls and keep the seven-function shape above.

/// The legend of a screen whose one verb is Accept and whose way out is
/// Back: "Choose" and "Back".
pub fn choose_and_back() -> Vec<Hint> {
    vec![
        hint(i18n::text("hint-choose"), Pictured::Accept),
        hint(i18n::text("hint-back"), Pictured::Back),
    ]
}

/// The answer of a placeholder to a press: Accept is its one verb, Back goes
/// back, and nothing else moves (there is nothing else on it to move to).
pub fn lone_act(state: &mut State, action: Action, verb: impl FnOnce(&mut State)) {
    match action {
        Action::Accept => verb(state),
        Action::Back => state.back(),
        _ => {}
    }
}

/// Draw one button in the corner of the button band, the light on it while
/// the light is the screen's.
pub fn lone_button(
    state: &mut State,
    page: &mut Page,
    band: [f32; 4],
    verb: Button<'_>,
    lit: bool,
) {
    let buttons = [verb];
    let places = button::places(page, band, &buttons);
    let pressing = state.pressing.through();
    let lit_index = (lit && verb.enabled).then_some(0);
    let laid = button::row(
        page,
        &places,
        &buttons,
        lit_index,
        &mut state.light,
        pressing,
        state.clock.dt,
    );
    state.laid_buttons(laid);
}

/// Whether a click landed on the lone button. Answered as Accept would be,
/// with the press heard by whoever answers it.
pub fn lone_pressed(page: &mut Page, verb: Button<'_>) -> bool {
    button::pressed(page, &[verb]).is_some()
}

/// A few lines of words at the top of `content`, wrapped to its width and
/// stopped short of its foot. Answers how tall they came out.
pub fn words(page: &mut Page, content: [f32; 4], words: &str, role: Role) -> f32 {
    let line = page.line(Text::Body);
    let lines = ((content[3] / line).floor() as usize).max(1);
    let ui = page.ui();
    let wrapped = text::wrapped(ui, Text::Body, words, content[2], lines.min(6));
    for (index, one) in wrapped.iter().enumerate() {
        ui.label(
            [
                content[0],
                content[1] + index as f32 * line,
                content[2],
                line,
            ],
            Text::Body,
            one,
            role,
            Align::Left,
        );
    }
    wrapped.len() as f32 * line
}

/// The legend of a screen whose verbs are Accept, Options and Back.
pub fn choose_options_and_back() -> Vec<Hint> {
    vec![
        hint(i18n::text("hint-choose"), Pictured::Accept),
        hint(i18n::text("hint-options"), Pictured::Options),
        hint(i18n::text("hint-back"), Pictured::Back),
    ]
}

/// One lone row — a name, its state under it — that acts as one control:
/// the light laid on it first when `lit`, then the row, then its spot.
pub fn lone_row(
    state: &mut State,
    page: &mut Page,
    rect: [f32; 4],
    row: &super::column::Row,
    lit: bool,
    spot: u32,
) {
    let pressing = state.pressing.through();
    // Alone, it is rounded by its own height, and its light with it.
    let (_, shape) = super::column::radius(page, rect);
    if lit {
        let sunk = lxb_app::lxb_toolkit::motion::pressed(rect, pressing);
        state.light.lay(page, sunk, shape, pressing, state.clock.dt);
    }
    super::column::draw_row(page, rect, row, lit, pressing.filter(|_| lit), shape);
    if row.enabled {
        page.ui().spot(spot, rect);
    }
}

/// The press of Continue on a placeholder question: on to the next screen.
pub fn onward(state: &mut State) {
    state.forward();
}

/// The press of Close on a screen that ends the installer.
pub fn close(state: &mut State) {
    state.sound(Sound::Press);
    state.leave();
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop::Part;

    /// A search that found nothing says so under the found column's
    /// heading, never over it ("Places found" and "Nothing found" were
    /// written through each other), and across the search field, as
    /// Language's line stands under its field.
    #[test]
    fn nothing_found_stands_under_the_found_columns_heading() {
        let room = [100.0, 200.0, 800.0, 400.0];
        for (caption, gap, line) in [(18.0, 16.0, 28.0), (12.0, 10.0, 19.0), (27.0, 24.0, 42.0)] {
            let heading = crate::ui::trail::heading_tall(caption, gap);
            let field = [300.0, 120.0, 600.0, 56.0];
            let under = under_heading(room, heading, Some(field));
            // `found_nothing` writes its line from half a line down its room.
            assert!(
                under[1] + line * 0.5 > room[1] + caption,
                "the line touches the heading at a {caption} caption"
            );
            assert_eq!([under[0], under[2]], [field[0], field[2]]);
            assert!(under[1] + under[3] <= room[1] + room[3] + 1.0);
            let alone = under_heading(room, heading, None);
            assert_eq!([alone[0], alone[2]], [room[0], room[2]]);
        }
    }

    #[test]
    fn a_field_on_the_line_of_the_buttons_goes_up_ahead_of_the_board() {
        let mut room = Room {
            content: [100.0, 200.0, 800.0, 500.0],
            buttons: [100.0, 720.0, 800.0, 48.0],
            note_ends: None,
            clear: [0.0; 2],
            board_top: None,
        };
        let field = [100.0, 712.0, 400.0, 56.0];
        assert_eq!(room.over_board(field, 10.0), field, "moved with no board");
        // The board's top still under the field: it stands where it is.
        room.board_top = Some(790.0);
        assert_eq!(room.over_board(field, 10.0), field);
        // Risen over it, the board pushes it up, clear of its top.
        room.board_top = Some(600.0);
        let over = room.over_board(field, 10.0);
        assert_eq!(over, [100.0, 534.0, 400.0, 56.0]);
        assert!(over[1] + over[3] < 600.0, "typed into under the keys");
        // Never past the top of the room.
        room.board_top = Some(150.0);
        assert_eq!(room.over_board(field, 10.0)[1], 200.0);
    }

    #[test]
    fn a_page_further_on_is_further_on() {
        let page = |screen, part| Shown {
            screen,
            part,
            partitions: false,
        };
        let main = page(Screen::Desktop, Part::Main);
        let options = page(Screen::Desktop, Part::Options);
        let list = page(Screen::Desktop, Part::Flatpaks);
        assert!(main.order() < options.order() && options.order() < list.order());
        assert!(page(Screen::Target, Part::Main).order() < main.order());
        assert!(list.order() < page(Screen::Account, Part::Main).order());
        assert!(
            page(Screen::Review, Part::Main).order() < page(Screen::Installing, Part::Main).order()
        );
        for ending in [Screen::Done, Screen::Stopped] {
            assert!(
                page(Screen::Installing, Part::Main).order() < page(ending, Part::Main).order()
            );
        }
    }

    #[test]
    fn standing_on_a_page_to_draw_it_is_put_back() {
        let mut state = State::demo(true);
        state.go(Screen::Desktop);
        state.views.desktop.part = Part::Remove;
        let here = stood(&state);
        let was = stand_on(
            &mut state,
            Stood::from(Shown {
                screen: Screen::Desktop,
                part: Part::Options,
                partitions: false,
            }),
        );
        assert_eq!(state.views.desktop.part, Part::Options);
        stand_back(&mut state, was);
        assert_eq!(stood(&state), here);
        let was = stand_on(
            &mut state,
            Stood::from(Shown {
                screen: Screen::Target,
                part: Part::Main,
                partitions: false,
            }),
        );
        assert_eq!(state.screen, Screen::Target);
        stand_back(&mut state, was);
        assert_eq!(stood(&state), here, "Desktop lost its Options page");
    }

    #[test]
    fn a_page_of_options_closed_from_done_goes_with_its_light_on_done() {
        let mut state = State::demo(true);
        state.go(Screen::Desktop);
        state.views.desktop.part = Part::Flatpaks;
        state.views.desktop.on_done = true;
        // As the frame drew it, the light on Done; then Done is pressed.
        let drawn = stood(&state);
        state.act(lxb_app::lxb_toolkit::input::Action::Accept, None);
        assert_eq!(state.views.desktop.part, Part::Options);
        assert!(!state.views.desktop.on_done, "Options opened on Done");
        // The page going is drawn as it stood, Done lit.
        let was = stand_on(&mut state, drawn);
        assert_eq!(state.views.desktop.part, Part::Flatpaks);
        assert!(
            state.views.desktop.on_done,
            "drawn going with the light moved"
        );
        stand_back(&mut state, was);
        assert_eq!(state.views.desktop.part, Part::Options);
        assert!(!state.views.desktop.on_done);
    }

    #[test]
    fn partitions_is_a_page_one_step_in_from_where_to_install() {
        let mut state = State::demo(true);
        state.go(Screen::Target);
        let main = shown(&state);
        let under = page(&state);
        target::open_partitions(&mut state);
        let partitions = shown(&state);
        // Another page — it comes in as a page of Options does — under the
        // same step's chip, and another page under the pointer.
        assert_ne!(main, partitions);
        assert!(main.order() < partitions.order());
        assert!(partitions.order() < shown_of(Screen::Desktop).order());
        assert_ne!(page(&state), under);
        assert_eq!(state.screen.place(), Screen::Target.place());
        // Drawn going and put back as it stood.
        let here = stood(&state);
        let was = stand_on(&mut state, Stood::from(main));
        assert_eq!(state.views.target.part, target::Part::Main);
        stand_back(&mut state, was);
        assert_eq!(stood(&state), here);
    }

    fn shown_of(screen: Screen) -> Shown {
        Shown {
            screen,
            part: Part::Main,
            partitions: false,
        }
    }

    #[test]
    fn a_page_is_told_apart_by_whatever_a_click_puts_under_the_pointer() {
        let mut state = State::demo(true);
        state.go(Screen::Desktop);
        let parts = [
            Part::Main,
            Part::Options,
            Part::Remove,
            Part::Flatpaks,
            Part::Bootloader,
        ];
        let pages: Vec<_> = parts
            .iter()
            .map(|part| {
                state.views.desktop.part = *part;
                page(&state)
            })
            .collect();
        for (at, one) in pages.iter().enumerate() {
            assert!(!pages[..at].contains(one), "{:?}", parts[at]);
        }
        for screen in [Screen::Timezone, Screen::Keyboard] {
            state.go(screen);
            let shut = page(&state);
            match screen {
                Screen::Timezone => state.views.timezone.trail.open = true,
                _ => state.views.keyboard.trail.open = true,
            }
            assert_ne!(page(&state), shut, "{screen:?}");
        }
        // The same page on another screen is another page.
        state.go(Screen::Account);
        let account = page(&state);
        state.go(Screen::Review);
        assert_ne!(page(&state), account);
    }
}
