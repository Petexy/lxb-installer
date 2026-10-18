//! Keyboard: the layouts, and the kinds of the one pressed — the plain
//! layout first, then its variants — walked as LineXinBar's file picker
//! walks a disk (`trail`), with a field under them to try it in, and
//! Continue.
//!
//! The kinds are not shown until a layout is pressed: the guess the chosen
//! language made puts the light on its layout, and Accept (or Right, or a
//! click on it) opens its kinds beside it, the light on the one in force;
//! Back closes them again.
//!
//! **Continue keeps the keyboard in force only once it has been seen**, as on
//! Time zone (`timezone` has the whole rule). Seen means that the kinds of
//! the layout in force, the kind in force marked among them, have stood open
//! on the screen for the dwell, counted by the frames that draw them
//! (`trail::Seen`): from the second frame that drew them to one drawn the
//! dwell later — their opening's own length, 0.45 s, or 0.3 s in low-end mode
//! — and, where they slide in, only once the frames have carried them all the
//! way in, so that frames drawn across a stall never count. Only a frame
//! counts them, never a press, which is answered at its frame's one instant
//! before the frame is drawn (a click, asked after the drawing, finds them as
//! they stood before it). Until then Continue — Accept on it (Enter, Space,
//! South), a click on it, the legend's Choose — with those kinds not open
//! opens them instead, as pressing that layout would, wherever the first
//! column's light had got to and with a search's narrowing emptied first, and
//! their dwell begins; with them open and the dwell not counted it does
//! nothing, and is not heard. The light stays on Continue, and the first
//! press after a frame has counted them goes on. Accept on the kind in force
//! (or a click on it) while its kinds are dwelling with the light where their
//! opening put it — Enter on the layout and Enter again in one frame — does
//! nothing either; a kind the person walked the light to once a frame had
//! drawn the kinds, another layout's kinds and what a search found are not
//! held. A frame that draws the kinds standing open with the kind in force on
//! the line begins the dwell however they came to — opened so by the person
//! or by Continue, or the light walked onto it, or left on it as a search
//! over them is emptied — but not one that draws them opened on it with the
//! light stepped off it before, which Continue rolls back onto it; leaving
//! them open for it counts, and shutting them before it is counted does not.
//! Choosing a kind, in the columns or from what a search found, sees it at
//! once; opening another layout does not. It stays seen for the installation,
//! Back from Where to install included, until choosing a language guesses
//! another keyboard than the one in force (`language::guess_the_rest`). Every
//! state of the step has the second column, with a keyboard attached or not
//! (`keyboard-none`, where the light goes from a kind chosen straight on to
//! Continue), so the rule holds in all of them; a layout the list does not
//! offer has no kinds to show, and Continue goes on with it.
//!
//! The field is there only while a keyboard with letter keys is attached
//! (`model::keyboards`): an installation with only a controller has nothing
//! to try a layout on, and never sees it. The list of input devices is read
//! again every second while the step is up, so a keyboard plugged in brings
//! the field and one pulled out takes it away, the light — or the board —
//! with it.
//!
//! Options (North, the Menu key, F10) brings a search field up over the
//! columns (`search`). Typed into, it shows in one column every kind of
//! every layout whose name or whose layout's name holds every word typed
//! ("dvorak" finds the Dvorak kinds of English, the British one and
//! Polish), and Accept on one takes it as choosing it in the columns does;
//! the columns then stand on it. Emptied, they are back as they were.
//!
//! The names are XKB's own, which is what every keyboard setting on a Linux
//! desktop calls them. Choosing a kind takes the layout, the variant and the
//! console keymap that goes with them, and puts the light in the field, so
//! the next thing to do is to type and see. With a keyboard in hand the field
//! takes what is typed while the light stands in it; with a pad in hand
//! Accept on it brings a board — under LineXinBar the shell's own, and
//! elsewhere the installer's, its letters arranged as the chosen layout
//! arranges them.

use std::time::Duration;

use lxb_app::lxb_toolkit::{input::Action, metrics::Metric, sound::Sound, typography::Text};
use lxb_app::{Page, Scroll};

use super::{Head, Room};
use crate::i18n;
use crate::model::catalogs::KeyboardLayout;
use crate::model::keyboards;
use crate::ui::app::{Machine, Screen, State};
use crate::ui::button::{self, Button};
use crate::ui::column::{Column, Row};
use crate::ui::field::{self, Edit, FieldId, Look};
use crate::ui::legend::{hint, Button as Pictured, Hint};
use crate::ui::panel::Note;
use crate::ui::search::{Backed, Search};
use crate::ui::spots;
use crate::ui::text;
use crate::ui::trail::{self, Click, Seen, Showing, Trail, Went};

/// The numbers the two columns answer to.
const IDS: [spots::Column; 2] = [spots::Column::nth(0), spots::Column::nth(1)];

/// The mark the rows of both columns wear: a keyboard.
const MARKS: [&str; 2] = ["setting-typed", "setting-typed"];

/// Where the light stands on the keyboard screen.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// In the search field over the columns, while it is up.
    Search,
    /// In the columns — or, while a search narrows them, in the one column
    /// of the kinds it found.
    #[default]
    Trail,
    Field,
    Continue,
}

/// What this screen keeps between frames.
#[derive(Debug, Clone)]
pub struct View {
    pub trail: Trail,
    pub focus: Focus,
    /// Whether the field stood above Continue on the last frame drawn (a
    /// narrow panel) rather than beside it, which decides where Up and Down
    /// lead from either.
    pub stacked: bool,
    /// Whether a keyboard with letter keys is attached, and so the field
    /// offered. The made-up machine has one; the real one is read on
    /// arriving and every [`READ_EVERY`] while the step is up.
    pub attached: bool,
    /// When the list of input devices was last read, by the installer's
    /// clock.
    read_at: Option<f32>,
    /// Where the field was drawn on the last frame, so a light standing in
    /// it goes up with it as the board pushes it up.
    field_at: Option<[f32; 4]>,
    /// The search over the layouts and their kinds.
    pub search: Search,
    /// The one column of the kinds the search found, walked as the trail's
    /// first column is; the trail itself is kept as it was.
    pub found: Trail,
    /// Whether the keyboard in force has been seen: the kinds of its layout
    /// kept open on the screen with it among them for the dwell, as the
    /// frames count it (`trail::Seen`), or a kind chosen. Until then Continue
    /// opens them rather than going on ([`onward`]).
    pub seen: Seen,
}

impl Default for View {
    fn default() -> Self {
        Self {
            trail: Trail::default(),
            focus: Focus::default(),
            stacked: false,
            attached: true,
            read_at: None,
            field_at: None,
            search: Search::default(),
            found: Trail::default(),
            seen: Seen::Not,
        }
    }
}

/// How often the list of input devices is read while the step is up.
const READ_EVERY: Duration = Duration::from_secs(1);

impl View {
    pub fn settle(&mut self) {
        self.trail.settle();
        self.found.settle();
    }
}

/// Set the keyboard: its layout, its variant and the console keymap that
/// goes with them.
pub fn set(state: &mut State, layout: &str, variant: &str) {
    let console = match state.machine {
        Machine::Demo => crate::ui::demo::console_keymap(layout),
        Machine::Real { .. } => {
            crate::model::catalogs::console_keymap(std::path::Path::new("/"), layout, variant)
        }
    };
    let keyboard = &mut state.draft.region.keyboard;
    keyboard.layout = layout.to_string();
    keyboard.variant = variant.to_string();
    keyboard.console = console;
}

/// The left-hand rows: each layout, how many kinds of it there are, and the
/// mark on the one in force.
fn left_rows(state: &State) -> Vec<Row> {
    let current = &state.draft.region.keyboard.layout;
    state
        .layouts
        .iter()
        .map(|layout| {
            Row::new(layout.description.as_str())
                .note(crate::message!(
                    "keyboard-others",
                    "count" => layout.variants.len() as i64
                ))
                .chosen(&layout.layout == current)
        })
        .collect()
}

/// The layout the first column's light is on.
fn lit_layout(state: &State) -> Option<&KeyboardLayout> {
    state.layouts.get(state.views.keyboard.trail.at[0])
}

/// The second column's rows: the plain layout first, then each of its
/// variants.
fn right_rows(state: &State) -> Vec<Row> {
    let Some(layout) = lit_layout(state) else {
        return Vec::new();
    };
    let keyboard = &state.draft.region.keyboard;
    let ours = keyboard.layout == layout.layout;
    let plain = Row::new(layout.description.as_str())
        .note(i18n::text("keyboard-plain"))
        .chosen(ours && keyboard.variant.is_empty());
    std::iter::once(plain)
        .chain(layout.variants.iter().map(|variant| {
            Row::new(variant.description.as_str())
                .chosen(ours && keyboard.variant == variant.variant)
        }))
        .collect()
}

/// The kinds the search finds, each as its layout's place in the list and
/// the place of the variant among the layout's — `None` for the layout on
/// its own — in the layouts' order: every kind whose own name, or whose
/// layout's name, holds every word typed (XKB's short names too).
fn found(state: &State) -> Vec<(usize, Option<usize>)> {
    let search = &state.views.keyboard.search;
    if !search.narrows() {
        return Vec::new();
    }
    let mut kinds = Vec::new();
    for (at, layout) in state.layouts.iter().enumerate() {
        if search.finds(&[&layout.description, &layout.layout]) {
            kinds.push((at, None));
        }
        for (index, variant) in layout.variants.iter().enumerate() {
            let words = [
                variant.description.as_str(),
                &layout.description,
                &layout.layout,
                &variant.variant,
            ];
            if search.finds(&words) {
                kinds.push((at, Some(index)));
            }
        }
    }
    kinds
}

/// The rows of the column of what the search found: each kind by its name,
/// and under it its layout's — or, for the layout on its own, that it is
/// the standard kind.
fn found_rows(state: &State) -> Vec<Row> {
    let keyboard = &state.draft.region.keyboard;
    found(state)
        .into_iter()
        .map(|(at, variant)| {
            let layout = &state.layouts[at];
            let ours = keyboard.layout == layout.layout;
            match variant.and_then(|index| layout.variants.get(index)) {
                Some(kind) => Row::new(kind.description.as_str())
                    .note(layout.description.as_str())
                    .chosen(ours && keyboard.variant == kind.variant),
                None => Row::new(layout.description.as_str())
                    .note(i18n::text("keyboard-plain"))
                    .chosen(ours && keyboard.variant.is_empty()),
            }
        })
        .collect()
}

/// Where the light goes once a kind is chosen, or from the foot of the
/// columns: into the field to try the keyboard, or — with no keyboard to try
/// it on — on to Continue.
fn after_columns(state: &State) -> Focus {
    if state.views.keyboard.attached {
        Focus::Field
    } else {
        Focus::Continue
    }
}

/// A keyboard is attached now, or no longer: the field offered or taken
/// away, and the light and the board taken out of a field that went.
pub fn attached(state: &mut State, attached: bool) {
    state.views.keyboard.attached = attached;
    if attached {
        return;
    }
    if state.views.keyboard.focus == Focus::Field {
        state.views.keyboard.focus = Focus::Continue;
    }
    if state
        .board
        .as_ref()
        .is_some_and(|board| board.field == FieldId::KeyboardTest)
    {
        state.board = None;
    }
}

/// Read again whether a keyboard is attached, on the real machine, while the
/// step is up and [`READ_EVERY`] has passed since the last reading — and ask
/// for a frame by then, so a still window (low-end mode draws once a second)
/// still follows a keyboard plugged in. The file is a few lines the kernel
/// writes out on asking, read on the frame's own thread.
pub fn poll(state: &mut State, page: &mut Page) {
    if state.screen != Screen::Keyboard || state.machine == Machine::Demo {
        return;
    }
    page.redraw_within(READ_EVERY);
    let now = state.clock.now;
    let due = state
        .views
        .keyboard
        .read_at
        .is_none_or(|at| now - at >= READ_EVERY.as_secs_f32());
    if due {
        state.views.keyboard.read_at = Some(now);
        attached(state, keyboards::attached());
    }
}

/// Take the kind a search found at `index` of its column: the columns then
/// stand on it, its layout's kinds open, as they would after choosing it
/// there, and the search is put away.
fn take_found(state: &mut State, index: usize) {
    let Some((at, variant)) = found(state).get(index).copied() else {
        return;
    };
    state.views.keyboard.search.close();
    state.views.keyboard.trail.at[0] = at;
    let right = right_rows(state);
    let trail = &mut state.views.keyboard.trail;
    trail.open_on(&right);
    trail.at[1] = variant.map_or(0, |index| index + 1);
    trail.settle();
    choose(state, variant.map_or(0, |index| index + 1));
}

/// Take the kind at `index` of the second column, and put the light in the
/// field to try it — or, with no keyboard to try it on, on Continue.
fn choose(state: &mut State, index: usize) {
    let Some(layout) = lit_layout(state) else {
        return;
    };
    let name = layout.layout.clone();
    let variant = match index {
        0 => String::new(),
        _ => match layout.variants.get(index - 1) {
            Some(variant) => variant.variant.clone(),
            None => return,
        },
    };
    set(state, &name, &variant);
    state.chose_keyboard = true;
    state.views.keyboard.seen.see();
    state.views.keyboard.focus = after_columns(state);
    state.sound(Sound::Press);
}

/// How the kinds of the layout in force stand on the screen
/// (`trail::Showing`): open, no search narrowing the columns over them, with
/// the light on the kind in force among them — on the line, marked — or on
/// another of them; or not at all.
fn showing(state: &State) -> Showing {
    let view = &state.views.keyboard;
    let current = state.draft.region.keyboard.layout.as_str();
    if !view.trail.open
        || view.search.narrows()
        || !lit_layout(state).is_some_and(|layout| layout.layout == current)
    {
        return Showing::Not;
    }
    if right_rows(state)
        .get(view.trail.at[1])
        .is_some_and(|row| row.chosen)
    {
        Showing::Row
    } else {
        Showing::Column
    }
}

/// Whether the keyboard in force has been seen: counted so by a frame drawn
/// before this one's presses (`Seen::seen`), or chosen.
fn seen(state: &State) -> bool {
    state.views.keyboard.seen.seen()
}

/// Whether the kinds have opened on the keyboard in force, or the light come
/// onto it among them, and no frame has yet counted it seen.
fn dwelling(state: &State) -> bool {
    state.views.keyboard.seen.dwelling()
}

/// Whether choosing the kind at `index` would take the guess before its
/// kinds were seen: it is the kind in force, its kinds are still dwelling on
/// it, and the light stands where their opening put it, not walked there by
/// the person (`Trail::walked`) — Enter on the layout and Enter again in one
/// frame, or a click on the kind as they come in.
fn too_soon(state: &State, index: usize) -> bool {
    dwelling(state)
        && !state.views.keyboard.trail.walked()
        && right_rows(state).get(index).is_some_and(|row| row.chosen)
}

/// The kinds have just opened. With the kind in force on the line among
/// them, marked as chosen, its dwell begins (`Seen::opened`), to run from the
/// frames that draw them. Opened on another layout, or on another kind — the
/// one the light was on among what a search found, Back emptying the search
/// — the one in force may be out of sight or not there at all, and nothing
/// begins until the light comes onto it.
fn opened(state: &mut State) {
    if showing(state) == Showing::Row {
        state.views.keyboard.seen.opened();
    } else {
        state.views.keyboard.seen.shut();
    }
}

/// What the frame tells the screen once it has drawn it (`screens::drawn`):
/// how the kinds of the keyboard in force stood on it, at this frame's
/// instant and for its length, which begins their dwell (the frame that
/// first draws them open on it, or the light come onto it among them),
/// counts them seen once it has run, or — not there — ends it. Answers
/// whether the dwell is still running, for the window to keep drawing frames
/// to time it.
pub fn drawn(state: &mut State) -> bool {
    let showing = showing(state);
    let (now, dt, low_end) = (state.clock.now, state.clock.dt, state.low_end());
    let view = &mut state.views.keyboard;
    // The columns, not what a search found in their place: from now on a
    // step in the kinds is the person's walk (`Trail::walked`).
    if !view.search.narrows() {
        view.trail.drawn();
    }
    view.seen.drawn(showing, now, dt, low_end);
    view.seen.dwelling()
}

/// Continue: on to Where to install with the keyboard in force, which a
/// frame drawn before this press has counted seen — or, while none has, the
/// kinds of its layout opened instead and nothing more, the light left on
/// Continue for the press that goes on. While they stand open on it and are
/// dwelling, the press does nothing at all, and is not heard: it may have
/// come before the person could have seen the kind — in the very frame that
/// opened them, while they were still coming in, or before the frame that
/// counts them was drawn.
fn onward(state: &mut State) {
    if !seen(state) {
        if dwelling(state) && showing(state) != Showing::Not {
            state.views.keyboard.focus = Focus::Continue;
            return;
        }
        if show_the_keyboard_in_force(state) {
            return;
        }
    }
    super::onward(state);
}

/// Open the kinds of the layout in force, the light in them on the kind in
/// force, as pressing that layout opens them — whatever row of the first
/// column the light had walked to, and with a search that narrows the
/// columns emptied first, so they are back to show it. The light is on
/// Continue, which was pressed, and the kind's dwell begins. Answers whether
/// they opened: a layout the list does not offer has no kinds to show, and
/// counts as seen as it is.
fn show_the_keyboard_in_force(state: &mut State) -> bool {
    let current = state.draft.region.keyboard.layout.as_str();
    let Some(at) = state
        .layouts
        .iter()
        .position(|layout| layout.layout == current)
    else {
        state.views.keyboard.seen.see();
        return false;
    };
    let view = &mut state.views.keyboard;
    if view.search.narrows() {
        view.search.back();
    }
    view.focus = Focus::Continue;
    view.trail.at[0] = at;
    let right = right_rows(state);
    if !state.views.keyboard.trail.open_on(&right) {
        state.views.keyboard.seen.see();
        return false;
    }
    state.views.keyboard.seen.opened();
    state.sound(Sound::Press);
    true
}

/// The pictures of the keyboard screen: Polish chosen from its kinds, and
/// something typed in the field with the light standing in it (`keyboard`);
/// as it is arrived at, the layouts alone with the light on the one in
/// force (`keyboard-layouts`); as it is arrived at in the language of the
/// picture, the keyboard it guessed in force and its kinds not yet seen,
/// the light gone down to Continue, so that a press of it
/// (`--press accept`) opens them (`keyboard-continue`); Polish chosen from
/// its kinds on a computer with no keyboard attached, where there is no field
/// and the light has gone on to Continue (`keyboard-none`); or the search
/// up, "dvorak" typed in it (`keyboard-search`), or "qqqq", which finds
/// nothing (`keyboard-search-nothing`).
pub fn stage(state: &mut State, scene: &str) {
    if scene == "keyboard-continue" {
        state.views.keyboard.focus = Focus::Continue;
        return;
    }
    set(state, "pl", "");
    state.chose_keyboard = true;
    // Chosen, as the person chooses it: seen (`choose`), so that Continue
    // pressed in the picture goes on as it would after that.
    state.views.keyboard.seen.see();
    enter(state);
    if scene == "keyboard-layouts" {
        return;
    }
    if scene == "keyboard-search" {
        to_search(state);
        searched(state, &Edit::Wrote("dvorak".into()));
        return;
    }
    if scene == "keyboard-search-nothing" {
        to_search(state);
        searched(state, &Edit::Wrote("qqqq".into()));
        return;
    }
    state.keyboard_test = "Zażółć gęślą jaźń".into();
    let right = right_rows(state);
    state.views.keyboard.trail.open_on(&right);
    state.views.keyboard.focus = Focus::Field;
    if scene == "keyboard-none" {
        attached(state, false);
    }
}

fn verb() -> Button<'static> {
    Button::primary(i18n::text("button-continue"))
}

pub fn enter(state: &mut State) {
    state.views.keyboard.search.close();
    // Read at once, so the step's first frame has the field or not.
    if let Machine::Real { .. } = state.machine {
        state.views.keyboard.read_at = Some(state.clock.now);
        attached(state, keyboards::attached());
    }
    let left = left_rows(state);
    let view = &mut state.views.keyboard;
    view.focus = Focus::Trail;
    view.trail.start(&left);
    // The kinds are shut on arriving: a dwell left unfinished by leaving
    // (Back from Continue as they came in) is over, as their shutting ends
    // it, rather than standing until this screen's first frame is drawn.
    view.seen.shut();
}

pub fn head(_state: &State) -> Head {
    Head::new(
        "setting-typed",
        i18n::text("keyboard-title"),
        i18n::text("keyboard-caption"),
    )
}

pub fn hints(state: &State) -> Vec<Hint> {
    if state.views.keyboard.focus == Focus::Field && state.pad {
        vec![
            hint(i18n::text("hint-type"), Pictured::Accept),
            hint(i18n::text("hint-back"), Pictured::Back),
        ]
    } else {
        super::search_hints(state, state.views.keyboard.focus == Focus::Search)
    }
}

pub fn note(_state: &State) -> Option<Note> {
    None
}

/// Move the light to `to`, heard as a move. A key bringing it back into the
/// columns has them roll to it again wherever the pointer held them.
fn focus(state: &mut State, to: Focus) {
    if state.views.keyboard.focus != to {
        state.views.keyboard.focus = to;
        if to == Focus::Trail {
            state.views.keyboard.trail.let_go();
        }
        state.sound(Sound::Move);
    }
}

pub fn act(state: &mut State, action: Action, scroll: Option<Scroll>) {
    if let Some(scroll) = scroll {
        if IDS.iter().any(|ids| Column::owns(scroll.spot, *ids)) {
            state.views.keyboard.focus = Focus::Trail;
        }
    }
    let stacked = state.views.keyboard.stacked;
    let field = state.views.keyboard.attached;
    let searching = state.views.keyboard.search.is_open();
    match (state.views.keyboard.focus, action) {
        // Options: the search, the light in it — from the test field too
        // with a keyboard in hand, whose legend offers it there as
        // everywhere else on the step. A pad's legend in the field offers
        // Type and Back alone, and North there is left alone (on the
        // installer's board over it North is Shift).
        (Focus::Search | Focus::Trail | Focus::Continue, Action::Menu) => return to_search(state),
        (Focus::Field, Action::Menu) if !state.pad => return to_search(state),
        // Back empties the search, then closes it; with no search up it is
        // the columns' and the screen's, as it always was.
        (Focus::Search | Focus::Trail | Focus::Continue, Action::Back) if searching => {
            return back(state)
        }
        (Focus::Search, Action::Accept) if state.pad => {
            return state.type_with_pad(FieldId::Search)
        }
        // Enter in the field, or Down out of it: the light lands on the
        // first kind found.
        (Focus::Search, Action::Accept | Action::Down) => {
            if !land(state) {
                state.sound(Sound::Error);
            }
            return;
        }
        (Focus::Search, Action::Right) => return focus(state, Focus::Continue),
        (Focus::Search, _) => return,
        (Focus::Trail, _) if state.views.keyboard.search.narrows() => {
            return act_on_found(state, action)
        }
        (Focus::Trail, _) => {}
        (_, Action::Back) => return state.back(),
        (Focus::Field, Action::Up) => return focus(state, into_columns(state)),
        (Focus::Field, Action::Right) => return focus(state, Focus::Continue),
        (Focus::Field, Action::Down) if stacked => return focus(state, Focus::Continue),
        (Focus::Field, Action::Accept) => {
            // A pad has no keys: a board, the shell's under LineXinBar. A
            // keyboard is typing already, and Enter in the field is done
            // with it.
            if state.pad {
                state.type_with_pad(FieldId::KeyboardTest);
            } else {
                focus(state, Focus::Continue);
            }
            return;
        }
        (Focus::Continue, Action::Left) => {
            let to = if field {
                Focus::Field
            } else {
                into_columns(state)
            };
            return focus(state, to);
        }
        (Focus::Continue, Action::Up) => {
            let to = if stacked && field {
                Focus::Field
            } else {
                into_columns(state)
            };
            return focus(state, to);
        }
        (Focus::Continue, Action::Accept) => return onward(state),
        _ => return,
    }
    let left = left_rows(state);
    let right = right_rows(state);
    let went = state.views.keyboard.trail.act(action, &left, &right);
    // Up from the top of the layouts, with the search up: into its field.
    if went == Went::Nothing && action == Action::Up && searching {
        return focus(state, Focus::Search);
    }
    answer(state, went, action);
}

/// Where the light goes back up or across into the columns: the columns —
/// or the search field, where a search found nothing for the light to stand
/// on, as Language and Time zone send it.
fn into_columns(state: &State) -> Focus {
    if state.views.keyboard.search.narrows() && found(state).is_empty() {
        Focus::Search
    } else {
        Focus::Trail
    }
}

/// A press in the column of the kinds the search found: Up and Down walk
/// it, Accept takes the kind, and past its ends the light goes to the field
/// over it or on, as from the columns.
fn act_on_found(state: &mut State, action: Action) {
    if action == Action::Accept {
        let at = state.views.keyboard.found.at[0];
        return take_found(state, at);
    }
    let rows = found_rows(state);
    let went = state.views.keyboard.found.act(action, &rows, &[]);
    match (went, action) {
        (Went::Nothing, Action::Up) => focus(state, Focus::Search),
        _ => answer(state, went, action),
    }
}

/// Options: the search field comes up, the light in it — and with a pad in
/// hand a board to type with at once.
fn to_search(state: &mut State) {
    let view = &mut state.views.keyboard;
    view.search.open();
    view.focus = Focus::Search;
    if state.pad {
        state.type_with_pad(FieldId::Search);
    } else {
        state.sound(Sound::Press);
    }
}

/// The light lands on the first kind the search found — or, with nothing
/// typed in it, back in the columns. Answers whether it did.
pub fn land(state: &mut State) -> bool {
    let narrows = state.views.keyboard.search.narrows();
    if narrows && found(state).is_empty() {
        return false;
    }
    let view = &mut state.views.keyboard;
    view.focus = Focus::Trail;
    if narrows {
        view.found.start_on(0);
    } else {
        view.trail.let_go();
    }
    state.sound(Sound::Move);
    true
}

/// Back while the search is up: what is typed in it goes, then the field.
/// The kind the light was on among those found is where the columns stand
/// when they come back: its layout, its kinds open on it.
fn back(state: &mut State) {
    let lit = match state.views.keyboard.focus {
        Focus::Trail if state.views.keyboard.search.narrows() => {
            found(state).get(state.views.keyboard.found.at[0]).copied()
        }
        _ => None,
    };
    let backed = state.views.keyboard.search.back();
    debug_assert_ne!(backed, Backed::Nothing);
    if let Some((at, variant)) = lit {
        state.views.keyboard.trail.at[0] = at;
        let right = right_rows(state);
        let trail = &mut state.views.keyboard.trail;
        if trail.open_on(&right) {
            trail.at[1] = variant.map_or(0, |index| index + 1);
        }
        trail.settle();
        opened(state);
    }
    let view = &mut state.views.keyboard;
    if view.focus == Focus::Search && !view.search.is_open() {
        view.focus = Focus::Trail;
    }
    state.sound(Sound::Back);
}

/// Something typed into the search: the kinds it finds, the light — in the
/// field — over the first of them.
pub fn searched(state: &mut State, edit: &Edit) {
    if !state.views.keyboard.search.edit(edit) {
        return;
    }
    let none = found(state).is_empty();
    let view = &mut state.views.keyboard;
    view.found.start_on(0);
    if view.focus == Focus::Trail && view.search.narrows() && none {
        view.focus = Focus::Search;
    }
}

/// Answer what a press — a key's or a click's — did to the columns.
fn answer(state: &mut State, went: Went, action: Action) {
    match went {
        Went::Nothing => {
            // Past the foot of a column, or on past the kinds: down to the
            // field, or to Continue with no field.
            if matches!(action, Action::Down | Action::Right) {
                focus(state, after_columns(state));
            }
        }
        Went::Moved => state.sound(Sound::Move),
        Went::Opened => {
            opened(state);
            state.sound(Sound::Press);
        }
        // Accept on the guess while the kinds that opened on it have not
        // stood on the screen for the dwell, the light where their opening
        // put it — Enter on the layout and Enter again in one frame — does
        // nothing: it would be taken never drawn. A kind the person walked
        // the light to is theirs to take at once.
        Went::Chose(index) if too_soon(state, index) => {}
        Went::Chose(index) => choose(state, index),
        Went::Closed => {
            state.views.keyboard.seen.shut();
            state.sound(Sound::Back);
        }
        Went::Pass => {
            if action == Action::Back {
                state.back();
            }
        }
    }
}

/// The narrowest the field stands beside Continue, in reference pixels: on a
/// band too narrow for it, it stands over Continue the width of the room
/// instead.
const BESIDE_LEAST: f32 = 420.0;

/// The widest the field stands beside Continue, in reference pixels. What
/// the catalogs' test holds its words to, with the narrowest room.
pub const WIDEST: f32 = 760.0;

/// Where the field and Continue stand: Continue in the corner of the button
/// band, where every screen's verb stands, and the field beside it on the
/// same line, its foot on the band's foot — or at the foot of the content over it where
/// the panel is too narrow for a field worth typing in beside it, its label
/// and what it says while empty whole. Answers the two and whether they
/// stack.
///
/// While the on-screen keyboard is up, the field beside Continue goes up
/// ahead of it (`Room::over_board`): the board rises out of the buttons'
/// band, and the field typed into stood under its keys. Stacked, it stands
/// at the content's foot, which ends over the board already.
fn foot(page: &mut Page, room: Room) -> ([f32; 4], [f32; 4], bool) {
    let Room {
        content,
        buttons: band,
        ..
    } = room;
    let gap = page.metric(Metric::Gap);
    let tall = field::height(page);
    let button_w = button::width(page, verb().label).min(band[2]);
    let button = [band[0] + band[2] - button_w, band[1], button_w, band[3]];
    let beside = band[2] - button_w - gap * 2.0;
    let pad = page.metric(Metric::RowPadding);
    // The field's heading, written smaller than a caption, and its hint, a
    // caption, as `field::draw` writes them.
    let words = (page.measure(Text::Caption, i18n::text("keyboard-test")) * text::HEADING)
        .max(page.measure(Text::Caption, i18n::text("keyboard-test-placeholder")));
    if beside >= page.scaled(BESIDE_LEAST).max(words + pad * 2.0) {
        let field_w = beside.min(page.scaled(WIDEST));
        let field = room.beside_buttons(band[0], field_w, tall, gap);
        (room.over_board(field, gap), button, false)
    } else {
        let bottom = content[1] + content[3];
        ([content[0], bottom - tall, content[2], tall], button, true)
    }
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let gap = page.metric(Metric::Gap);
    let content = room.content;
    let (field_rect, button_rect, stacked) = foot(page, room);
    state.views.keyboard.stacked = stacked;
    // The columns stand over the field, or — with no field — take the room
    // down to its foot. Continue stays where it is either way. Never past
    // the room's foot: a field beside Continue stands under a note on lines
    // of its own, and the columns would run on under the note to it.
    let attached = state.views.keyboard.attached;
    let foot = content[1] + content[3];
    let bottom = if attached {
        (field_rect[1] - gap * 1.6).min(foot)
    } else {
        foot
    };
    let mut room = [
        content[0],
        content[1],
        content[2],
        (bottom - content[1]).max(1.0),
    ];

    let focus = state.views.keyboard.focus;
    let pressing = state.pressing.through();
    let dt = state.clock.dt;
    // The search field over the columns, lined up with the first column's
    // heading, while it is up.
    let mut searched_in = None;
    if state.views.keyboard.search.is_open() {
        let tall = field::height(page);
        let edge = trail::first_edge(room[2], page.scaled(1.0));
        let rect = [
            room[0] + edge,
            room[1],
            (room[2] - edge).min(page.scaled(super::timezone::SEARCH_WIDEST)),
            tall,
        ];
        room[1] += tall + gap * 1.5;
        room[3] = (room[3] - tall - gap * 1.5).max(1.0);
        let query = state.views.keyboard.search.query().to_string();
        let in_field = lit && focus == Focus::Search;
        let placeholder = i18n::text("search-keyboard-placeholder");
        super::search_field(state, page, rect, &query, placeholder, in_field);
        searched_in = Some(rect);
    }
    if state.views.keyboard.search.narrows() {
        // What the search found, in one column, whatever its layout.
        let rows = found_rows(state);
        if rows.is_empty() {
            let query = state.views.keyboard.search.query().to_string();
            let heading = trail::heading_tall(page.line(Text::Caption), gap);
            let under = super::under_heading(room, heading, searched_in);
            super::found_nothing(page, under, &query);
        }
        let look = trail::Look {
            headings: [i18n::text("keyboard-heading-found"), ""],
            marks: MARKS,
            ids: IDS,
            pressing,
            dt,
        };
        let here = lit && focus == Focus::Trail && !rows.is_empty();
        let light = here.then_some(&mut state.light);
        state
            .views
            .keyboard
            .found
            .draw(page, room, [&rows, &[]], light, look);
    } else {
        let left = left_rows(state);
        let right = right_rows(state);
        let look = trail::Look {
            headings: [
                i18n::text("keyboard-heading-layout"),
                i18n::text("keyboard-heading-variant"),
            ],
            marks: MARKS,
            ids: IDS,
            pressing,
            dt,
        };
        let light = (lit && focus == Focus::Trail).then_some(&mut state.light);
        state
            .views
            .keyboard
            .trail
            .draw(page, room, [&left, &right], light, look);
    }

    // The field takes text from a keyboard only while the light stands in
    // it; a pad types through a board instead.
    if attached {
        let in_field = lit && focus == Focus::Field;
        if in_field {
            state.in_field(FieldId::KeyboardTest);
        }
        // Pushed up by the board rising, the field takes the light it holds
        // along, rather than leaving it to glide after it from where the
        // field was — under the keys — and so does the frame the board takes
        // the light over, which it does from where the field is now.
        let was = state.views.keyboard.field_at.replace(field_rect);
        let holds =
            focus == Focus::Field && state.board.as_ref().is_some_and(|board| !board.has_light());
        if let (true, Some(was)) = (holds, was) {
            state.light.ride([0.0, field_rect[1] - was[1], 0.0, 0.0]);
        }
        let caret = state.typing_into() == Some(FieldId::KeyboardTest);
        let shown = state.keyboard_test.clone();
        let light = in_field.then_some(&mut state.light);
        field::draw(
            page,
            field_rect,
            FieldId::KeyboardTest,
            Look {
                label: i18n::text("keyboard-test"),
                shown: &shown,
                placeholder: i18n::text("keyboard-test-placeholder"),
                refused: None,
                caret,
            },
            light,
            pressing,
            dt,
            state.clock.now,
        );
    }

    let buttons = [verb()];
    let lit_button = (lit && focus == Focus::Continue).then_some(0);
    let laid = button::row(
        page,
        &[button_rect],
        &buttons,
        lit_button,
        &mut state.light,
        pressing,
        dt,
    );
    state.laid_buttons(laid);
}

/// Put the light on row `index` of column `level`, which the pointer moved
/// onto or clicked. Answers whether it stands there.
fn point(state: &mut State, level: usize, index: usize) -> bool {
    let landed = if state.views.keyboard.search.narrows() {
        let rows = found_rows(state);
        state.views.keyboard.found.point_at(level, index, &rows)
    } else {
        let rows = [left_rows(state), right_rows(state)];
        state
            .views
            .keyboard
            .trail
            .point_at(level, index, &rows[level])
    };
    if landed {
        state.views.keyboard.focus = Focus::Trail;
    }
    landed
}

/// The two columns as they are drawn: the trail's, or the one of what the
/// search found and nothing beside it.
fn columns(state: &State) -> [Vec<Row>; 2] {
    if state.views.keyboard.search.narrows() {
        [found_rows(state), Vec::new()]
    } else {
        [left_rows(state), right_rows(state)]
    }
}

pub fn hover(state: &mut State, id: u32) {
    if id == FieldId::KeyboardTest.spot() {
        // Read off the last frame: a keyboard pulled out since has taken
        // the field away, and the light does not go into a field not there.
        if state.views.keyboard.attached {
            state.views.keyboard.focus = Focus::Field;
        }
        return;
    }
    if super::button_of(id, 1).is_some() {
        state.views.keyboard.focus = Focus::Continue;
        return;
    }
    if id == FieldId::Search.spot() && state.views.keyboard.search.is_open() {
        state.views.keyboard.focus = Focus::Search;
        return;
    }
    let [left, right] = columns(state);
    if let Some((level, index)) = Trail::row_of(id, IDS, [&left, &right]) {
        point(state, level, index);
    }
}

pub fn pressed(state: &mut State, page: &mut Page) {
    if state.views.keyboard.attached && page.pressed(FieldId::KeyboardTest.spot()) {
        // A click puts the light in the field, where a keyboard types; the
        // installer raises no board of its own for a pointer. On a field
        // whose shell's board was put away it asks for that board again.
        focus(state, Focus::Field);
        state.clicked_field(FieldId::KeyboardTest);
        return;
    }
    if state.views.keyboard.search.is_open() && page.pressed(FieldId::Search.spot()) {
        // A click puts the light in the search field, where a keyboard
        // types.
        focus(state, Focus::Search);
        state.clicked_field(FieldId::Search);
        return;
    }
    let [left, right] = columns(state);
    let view = &mut state.views.keyboard;
    let trail = if view.search.narrows() {
        &mut view.found
    } else {
        &mut view.trail
    };
    let front = trail.front();
    match trail.pressed(page, [&left, &right], IDS) {
        // A layout opens, a kind is chosen: what Accept on it does, wherever
        // the light was.
        Some(Click::Row(index)) => {
            if point(state, front, index) {
                act(state, Action::Accept, None);
            }
            return;
        }
        Some(Click::Closed) => {
            state.views.keyboard.focus = Focus::Trail;
            return answer(state, Went::Closed, Action::Back);
        }
        None => {}
    }
    if button::pressed(page, &[verb()]).is_some() {
        onward(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::motion::FRAME;

    fn on_keyboard() -> State {
        let mut state = State::demo(true);
        state.go(Screen::Keyboard);
        state
    }

    fn layout_at(state: &State, layout: &str) -> usize {
        state
            .layouts
            .iter()
            .position(|one| one.layout == layout)
            .unwrap()
    }

    #[test]
    fn it_opens_on_the_layout_in_force_with_its_kinds_shut() {
        let state = on_keyboard();
        let trail = &state.views.keyboard.trail;
        assert_eq!(trail.at[0], layout_at(&state, "us"));
        assert!(
            !trail.open,
            "the kinds were shown before anything was pressed"
        );
    }

    #[test]
    fn pressing_a_layout_opens_its_kinds_with_the_plain_layout_first() {
        let mut state = on_keyboard();
        act(&mut state, Action::Accept, None);
        let trail = &state.views.keyboard.trail;
        assert!(trail.open);
        let right = right_rows(&state);
        assert_eq!(right[0].name, "English (US)");
        assert!(right[0].chosen);
        assert_eq!(trail.at[1], 0);
        assert_eq!(right.len(), 4, "the plain layout and its three variants");
        assert_eq!(
            state.views.keyboard.focus,
            Focus::Trail,
            "opening chose nothing"
        );
    }

    #[test]
    fn back_closes_the_kinds_and_then_goes_back() {
        let mut state = on_keyboard();
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Back, None);
        assert!(!state.views.keyboard.trail.open);
        assert_eq!(state.screen, Screen::Keyboard);
        act(&mut state, Action::Back, None);
        assert_eq!(state.screen, Screen::Timezone);
    }

    #[test]
    fn choosing_a_variant_sets_it_with_its_console_keymap_and_goes_to_the_field() {
        let mut state = on_keyboard();
        state.views.keyboard.trail.at[0] = layout_at(&state, "de");
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Accept, None);
        let keyboard = &state.draft.region.keyboard;
        assert_eq!(
            (keyboard.layout.as_str(), keyboard.variant.as_str()),
            ("de", "nodeadkeys")
        );
        assert_eq!(keyboard.console, "de");
        assert!(state.chose_keyboard);
        assert_eq!(state.views.keyboard.focus, Focus::Field);
        assert!(
            state.views.keyboard.trail.open,
            "the kinds closed on the choice"
        );
        assert_eq!(state.screen, Screen::Keyboard, "it waits to be tried");
    }

    /// How far apart the e2e harness presses its keys at the closest
    /// (`tests/e2e/scenarios.py`: `press` waits 0.6 s after each key, and two
    /// seconds more after the last of a call).
    const HARNESS_APART: f32 = 0.6;

    /// The harness walks this screen with Enter, Enter, Right, Enter
    /// (`tests/e2e/scenarios.py`, `walk_to_target`; `live_lxb` in one call):
    /// the layout the guess lit, its kind in force — the kinds having stood
    /// on the screen longer than their dwell by then — across from the field
    /// to Continue, Continue.
    #[test]
    fn the_harness_presses_keep_the_guessed_layout_and_go_on() {
        const { assert!(HARNESS_APART > trail::OPENING && HARNESS_APART > trail::OPENING_AT_ONCE) };
        for (layout, pad) in [("us", false), ("pl", false), ("us", true)] {
            let mut state = State::demo(true);
            set(&mut state, layout, "");
            state.pad = pad;
            state.go(Screen::Keyboard);
            state.press_apart(&[Action::Accept, Action::Accept], HARNESS_APART);
            assert_eq!(state.views.keyboard.focus, Focus::Field);
            state.press_apart(&[Action::Right], HARNESS_APART);
            assert_eq!(state.views.keyboard.focus, Focus::Continue);
            state.press_apart(&[Action::Accept], HARNESS_APART);
            let keyboard = &state.draft.region.keyboard;
            assert_eq!(
                (keyboard.layout.as_str(), keyboard.variant.as_str()),
                (layout, "")
            );
            assert_eq!(state.screen, Screen::Target, "{layout}");
        }
    }

    #[test]
    fn with_a_pad_accept_in_the_field_raises_the_board_on_the_chosen_layout() {
        let mut state = on_keyboard();
        state.views.keyboard.focus = Focus::Field;
        state.pad = true;
        act(&mut state, Action::Accept, None);
        let board = state.board.as_ref().expect("the board");
        assert_eq!(board.field, FieldId::KeyboardTest);
    }

    /// Enter in the field is done with it: the light goes on to Continue,
    /// whose first press shows the kinds of the layout in force (not seen
    /// yet) and whose second goes on.
    #[test]
    fn with_a_keyboard_accept_in_the_field_goes_on_to_continue() {
        let mut state = on_keyboard();
        state.views.keyboard.focus = Focus::Field;
        act(&mut state, Action::Accept, None);
        assert!(state.board.is_none());
        assert_eq!(state.views.keyboard.focus, Focus::Continue);
        state.frame_at(10.0, &[Action::Accept], &[]);
        showing(&state, ("us", ""), "Continue from the field");
        state.frames_until(11.0);
        state.frame_at(11.0, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Target);
    }

    #[test]
    fn hovering_lights_a_layout_its_kinds_the_field_and_continue() {
        let mut state = on_keyboard();
        let german = layout_at(&state, "de");
        hover(&mut state, IDS[0].row(german));
        assert_eq!(state.views.keyboard.trail.at[0], german);
        assert_eq!(right_rows(&state)[0].name, "German");
        assert!(!state.views.keyboard.trail.open, "a hover opened the kinds");
        // A click opens them, as Accept does.
        assert!(point(&mut state, 0, german));
        act(&mut state, Action::Accept, None);
        assert!(state.views.keyboard.trail.open);
        hover(&mut state, IDS[1].row(1));
        assert_eq!(state.views.keyboard.trail.at[1], 1);
        assert_eq!(state.draft.region.keyboard.layout, "us", "a hover chose");
        hover(&mut state, FieldId::KeyboardTest.spot());
        assert_eq!(state.views.keyboard.focus, Focus::Field);
        assert!(state.board.is_none(), "a hover raised the board");
        hover(&mut state, spots::BUTTONS);
        assert_eq!(state.views.keyboard.focus, Focus::Continue);
        // A click on a kind takes it, wherever the light was.
        assert!(point(&mut state, 1, 1));
        act(&mut state, Action::Accept, None);
        let keyboard = &state.draft.region.keyboard;
        assert_eq!(
            (keyboard.layout.as_str(), keyboard.variant.as_str()),
            ("de", "nodeadkeys")
        );
        assert_eq!(state.views.keyboard.focus, Focus::Field);
    }

    #[test]
    fn with_no_keyboard_attached_the_light_goes_past_the_field_to_continue() {
        let mut state = on_keyboard();
        attached(&mut state, false);
        state.views.keyboard.trail.at[0] = layout_at(&state, "de");
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.draft.region.keyboard.layout, "de");
        assert_eq!(
            state.views.keyboard.focus,
            Focus::Continue,
            "a kind chosen put the light in a field that is not there"
        );
        act(&mut state, Action::Left, None);
        assert_eq!(state.views.keyboard.focus, Focus::Trail);
        // A hover read off a frame that still had the field goes nowhere.
        hover(&mut state, FieldId::KeyboardTest.spot());
        assert_eq!(state.views.keyboard.focus, Focus::Trail);
        act(&mut state, Action::Back, None);
        assert!(!state.views.keyboard.trail.open);
        let last = state.layouts.len() - 1;
        state.views.keyboard.trail.at[0] = last;
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.keyboard.focus, Focus::Continue);
        state.views.keyboard.stacked = true;
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.keyboard.focus, Focus::Trail);
    }

    #[test]
    fn a_keyboard_pulled_out_takes_the_light_and_the_board_out_of_its_field() {
        let mut state = on_keyboard();
        state.pad = true;
        state.views.keyboard.focus = Focus::Field;
        act(&mut state, Action::Accept, None);
        assert!(state.board.is_some());
        attached(&mut state, false);
        assert!(state.board.is_none(), "the board kept typing into no field");
        assert_eq!(state.views.keyboard.focus, Focus::Continue);
        // Plugged in again, the field is back and the light stays put.
        attached(&mut state, true);
        assert!(state.views.keyboard.attached);
        assert_eq!(state.views.keyboard.focus, Focus::Continue);
        act(&mut state, Action::Left, None);
        assert_eq!(state.views.keyboard.focus, Focus::Field);
    }

    #[test]
    fn the_made_up_machine_has_a_keyboard_and_is_never_read() {
        let mut state = on_keyboard();
        assert!(state.views.keyboard.attached);
        state.views.keyboard.attached = false;
        enter(&mut state);
        assert!(
            !state.views.keyboard.attached,
            "a demo read this machine's keyboards"
        );
    }

    fn typed(state: &mut State, text: &str) {
        state.edit(FieldId::Search, &Edit::Wrote(text.into()));
    }

    fn found_names(state: &State) -> Vec<String> {
        found_rows(state).into_iter().map(|row| row.name).collect()
    }

    /// A search finds every kind of every layout by its own name or its
    /// layout's, in one column; Accept takes one as choosing it in the
    /// columns does, and the columns then stand on it.
    #[test]
    fn a_search_finds_kinds_of_every_layout_and_accept_takes_one() {
        let mut state = on_keyboard();
        act(&mut state, Action::Menu, None);
        assert_eq!(state.views.keyboard.focus, Focus::Search);
        typed(&mut state, "dvorak");
        assert_eq!(
            found_names(&state),
            [
                "English (UK, Dvorak)",
                "English (Dvorak)",
                "Polish (Dvorak)"
            ]
        );
        // By the layout's name: the layout itself first, then its kinds.
        let mut state = on_keyboard();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "german");
        assert_eq!(found_names(&state)[0], "German");
        assert_eq!(found_names(&state).len(), 3);
        // Enter lands on the first; Down to the second; Accept takes it.
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.keyboard.focus, Focus::Trail);
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Accept, None);
        let keyboard = &state.draft.region.keyboard;
        assert_eq!(
            (keyboard.layout.as_str(), keyboard.variant.as_str()),
            ("de", "nodeadkeys")
        );
        assert!(state.chose_keyboard);
        let view = &state.views.keyboard;
        assert!(!view.search.is_open(), "the search stayed up");
        assert_eq!(view.focus, Focus::Field, "on to the field to try it");
        assert!(view.trail.open, "the columns do not stand on it");
        assert_eq!(view.trail.at, [layout_at(&state, "de"), 1]);
    }

    /// Back empties the search, the columns standing on the kind the light
    /// was on; then it closes it; then it is the columns' own again.
    #[test]
    fn back_empties_the_search_then_closes_it() {
        let mut state = on_keyboard();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "dvorak");
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Back, None);
        let view = &state.views.keyboard;
        assert!(view.search.is_open() && !view.search.narrows());
        assert_eq!(view.trail.at[0], layout_at(&state, "us"));
        assert!(view.trail.open);
        assert_eq!(
            right_rows(&state)[view.trail.at[1]].name,
            "English (Dvorak)"
        );
        act(&mut state, Action::Back, None);
        assert!(!state.views.keyboard.search.is_open());
        act(&mut state, Action::Back, None);
        assert!(!state.views.keyboard.trail.open);
        act(&mut state, Action::Back, None);
        assert_eq!(state.screen, Screen::Timezone);
    }

    /// Options summons the search wherever the legend offers it: from the
    /// columns, Continue and — with a keyboard — the test field, whose
    /// legend says "Search" too. With a pad in the field the legend offers
    /// Type and Back alone, and North there does nothing. A pad brings a
    /// board up with the search field; nothing found keeps the light in it.
    #[test]
    fn the_search_is_summoned_from_the_columns_and_finds_nothing_quietly() {
        let mut state = on_keyboard();
        state.views.keyboard.focus = Focus::Field;
        let offered = |state: &State| {
            hints(state)
                .iter()
                .any(|hint| hint.label == i18n::text("hint-search"))
        };
        assert!(offered(&state), "the legend does not offer the search");
        act(&mut state, Action::Menu, None);
        assert!(state.views.keyboard.search.is_open());
        assert_eq!(state.views.keyboard.focus, Focus::Search);
        let mut state = on_keyboard();
        state.views.keyboard.focus = Focus::Field;
        state.pad = true;
        assert!(!offered(&state), "a pad's legend offers the search");
        act(&mut state, Action::Menu, None);
        assert!(!state.views.keyboard.search.is_open());
        assert!(state.board.is_none());
        let mut state = on_keyboard();
        state.pad = true;
        act(&mut state, Action::Menu, None);
        assert_eq!(
            state.board.as_ref().map(|board| board.field),
            Some(FieldId::Search)
        );
        let mut state = on_keyboard();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "klingon");
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.keyboard.focus, Focus::Search);
        assert_eq!(state.draft.region.keyboard.layout, "us");
    }

    /// Where a search found nothing, the light coming back from Continue or
    /// up from the test field goes into the search field, never onto the
    /// empty column — with or without a keyboard attached.
    #[test]
    fn with_nothing_found_the_light_comes_back_to_the_search_field() {
        for attached in [true, false] {
            let mut state = on_keyboard();
            state.views.keyboard.attached = attached;
            act(&mut state, Action::Menu, None);
            typed(&mut state, "klingon");
            for (from, press) in [
                (Focus::Continue, Action::Left),
                (Focus::Continue, Action::Up),
                (Focus::Field, Action::Up),
            ] {
                if from == Focus::Field && !attached {
                    continue;
                }
                state.views.keyboard.focus = from;
                act(&mut state, press, None);
                let to = state.views.keyboard.focus;
                let expected = if attached && press == Action::Left {
                    Focus::Field
                } else {
                    Focus::Search
                };
                assert_eq!(to, expected, "{from:?} {press:?}, attached {attached}");
            }
            // Something found: back onto the column, as before.
            let mut state = on_keyboard();
            state.views.keyboard.attached = attached;
            act(&mut state, Action::Menu, None);
            typed(&mut state, "dvorak");
            state.views.keyboard.focus = Focus::Continue;
            act(&mut state, Action::Up, None);
            assert_eq!(state.views.keyboard.focus, Focus::Trail);
        }
    }

    #[test]
    fn past_the_foot_of_the_layouts_the_light_goes_down_to_the_field() {
        let mut state = on_keyboard();
        let last = state.layouts.len() - 1;
        state.views.keyboard.trail.at[0] = last;
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.keyboard.focus, Focus::Field);
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.keyboard.focus, Focus::Trail);
    }

    #[test]
    fn the_pictures_show_the_kinds_a_layout_was_chosen_from() {
        let mut state = State::demo(true);
        state.go(Screen::Keyboard);
        stage(&mut state, "keyboard");
        assert!(state.views.keyboard.trail.open);
        assert_eq!(state.views.keyboard.focus, Focus::Field);
        stage(&mut state, "keyboard-layouts");
        assert!(!state.views.keyboard.trail.open);
        assert_eq!(state.views.keyboard.focus, Focus::Trail);
        let mut state = State::demo(true);
        state.go(Screen::Keyboard);
        stage(&mut state, "keyboard-none");
        assert!(state.views.keyboard.trail.open);
        assert!(!state.views.keyboard.attached);
        assert_eq!(state.views.keyboard.focus, Focus::Continue);
        // As it is arrived at, the guess in force and unseen, the light on
        // Continue.
        let mut state = State::demo(true);
        state.go(Screen::Keyboard);
        stage(&mut state, "keyboard-continue");
        let view = &state.views.keyboard;
        assert!(!view.trail.open);
        assert_eq!(view.seen, Seen::Not);
        assert_eq!(view.focus, Focus::Continue);
        assert_eq!(state.draft.region.keyboard.layout, "us");
    }

    /// Arrived at Keyboard, the guessed keyboard's kinds not seen, and the
    /// light gone over to Continue, with a keyboard attached or not.
    fn on_continue(attached: bool) -> State {
        let mut state = on_keyboard();
        assert_eq!(state.views.keyboard.seen, Seen::Not, "seen before shown");
        super::attached(&mut state, attached);
        state.views.keyboard.focus = Focus::Continue;
        state
    }

    /// What Continue's first press leaves while the keyboard in force had
    /// not been seen: its layout's kinds open, the kind in force on the line
    /// and marked, heard as the opening is, the light still on Continue and
    /// the screen not left, nothing chosen — and the kind's dwell begun, not
    /// over.
    fn showing(state: &State, (layout, variant): (&str, &str), how: &str) {
        let view = &state.views.keyboard;
        assert_eq!(state.screen, Screen::Keyboard, "{how} went on unseen");
        assert!(view.trail.open, "{how} did not open the kinds");
        assert_eq!(view.focus, Focus::Continue, "{how} took the light off");
        assert_eq!(view.trail.at[0], layout_at(state, layout), "{how}");
        assert!(right_rows(state)[view.trail.at[1]].chosen, "{how}");
        let keyboard = &state.draft.region.keyboard;
        assert_eq!(
            (keyboard.layout.as_str(), keyboard.variant.as_str()),
            (layout, variant),
            "{how} chose a keyboard"
        );
        assert!(!state.chose_keyboard, "{how} chose a keyboard");
        assert!(dwelling(state), "{how}: {:?}", view.seen);
        assert_eq!(state.heard().last(), Some(&Sound::Press), "{how}");
    }

    /// The legend's Choose pressed by the pointer: what the frame does with
    /// it after drawing (`State::pressed_here`).
    fn choose_in_the_legend(state: &mut State) {
        state.press_pair(crate::ui::legend::Button::Accept);
    }

    /// A way of pressing Continue in one frame, and its name: the presses
    /// that reach the frame, and the clicks it asks after drawing.
    type Press = (&'static str, &'static [Action], &'static [fn(&mut State)]);

    /// Every way of pressing Continue — Accept on it (Enter, South), the
    /// legend's Choose pressed by the pointer, a click on the button.
    const PRESSES: [Press; 3] = [
        ("Accept", &[Action::Accept], &[]),
        ("the legend's Choose", &[], &[choose_in_the_legend]),
        ("a click", &[], &[onward]),
    ];

    /// Two presses of Continue in one frame, and their name: whether a pad
    /// is in hand, the presses that reach the frame, and the clicks it asks
    /// after drawing.
    type Twice = (
        &'static str,
        bool,
        &'static [Action],
        &'static [fn(&mut State)],
    );

    /// A click on Continue — the button, or the legend's Choose — and its
    /// name.
    type Clicked = (&'static str, fn(&mut State));

    /// With a keyboard attached and without one (`keyboard-none`), which
    /// has the same second column; the second press, once the kinds have
    /// stood on the screen for their dwell, goes on.
    #[test]
    fn continue_first_shows_the_kinds_of_the_keyboard_in_force_then_goes_on() {
        for attached in [true, false] {
            for (how, presses, clicks) in PRESSES {
                let how = format!("{how}, a keyboard attached {attached}");
                let mut state = on_continue(attached);
                state.frame_at(10.0, presses, clicks);
                showing(&state, ("us", ""), &how);
                assert_eq!(state.views.keyboard.trail.at[1], 0, "{how}");
                let later = 10.0 + trail::OPENING + 0.1;
                state.frames_until(later);
                assert_eq!(state.views.keyboard.seen, Seen::Yes, "{how}");
                state.frame_at(later, presses, clicks);
                assert_eq!(state.screen, Screen::Target, "{how}, pressed again");
            }
        }
        // Start goes nowhere from here, as it never did.
        let mut state = on_continue(true);
        state.act(Action::Submit, None);
        assert_eq!(state.screen, Screen::Keyboard);
        assert!(!state.views.keyboard.trail.open);
    }

    /// Two presses of Continue that reach one frame — Enter twice, South
    /// twice, Enter and a click on Continue or on the legend's Choose — are
    /// answered at one instant, before the frame draws the kinds the first
    /// opened: the second does nothing, and is not heard. So it is in
    /// low-end mode, and with no keyboard attached.
    #[test]
    fn two_presses_of_continue_in_one_frame_only_show_the_keyboard_in_force() {
        let both: [Twice; 4] = [
            ("Enter twice", false, &[Action::Accept, Action::Accept], &[]),
            ("South twice", true, &[Action::Accept, Action::Accept], &[]),
            (
                "Enter and a click on Continue",
                false,
                &[Action::Accept],
                &[onward],
            ),
            (
                "Enter and the legend's Choose",
                false,
                &[Action::Accept],
                &[choose_in_the_legend],
            ),
        ];
        for attached in [true, false] {
            for low_end in [false, true] {
                for (how, pad, presses, clicks) in both {
                    let how = format!("{how}, low-end {low_end}, attached {attached}");
                    let mut state = on_continue(attached);
                    if low_end {
                        state.pretend_low_end();
                    }
                    state.pad = pad;
                    state.frame_at(10.0, presses, clicks);
                    showing(&state, ("us", ""), &how);
                    let heard = state.heard().iter().filter(|one| **one == Sound::Press);
                    assert_eq!(heard.count(), 1, "{how}: the second press was heard");
                }
            }
        }
    }

    /// A key 50 ms after a click on Continue — the kinds three frames into
    /// coming in — does nothing.
    #[test]
    fn a_key_soon_after_a_click_on_continue_waits_for_the_kinds() {
        let mut state = on_continue(true);
        state.frame_at(10.0, &[], &[onward]);
        showing(&state, ("us", ""), "a click");
        state.frames_until(10.05);
        state.frame_at(10.05, &[Action::Accept], &[]);
        showing(&state, ("us", ""), "Enter 50 ms after a click");
    }

    /// The kinds are counted seen by the frames that draw them, never by a
    /// press: from the second frame that drew them, the dwell later. A press
    /// in the very frame drawn the dwell after it does nothing, and is not
    /// heard; one in the frame after goes on — 0.3 s in low-end mode.
    #[test]
    fn a_press_before_a_frame_has_counted_the_dwell_is_let_go_and_one_after_goes_on() {
        for low_end in [false, true] {
            let mut state = on_continue(true);
            if low_end {
                state.pretend_low_end();
            }
            state.frame_at(10.0, &[Action::Accept], &[]);
            let heard = state.heard().len();
            let counted = 10.0 + FRAME + trail::dwell(low_end) + 0.001;
            state.frames_until(counted - 0.01);
            state.frame_at(counted, &[Action::Accept], &[]);
            assert_eq!(state.screen, Screen::Keyboard, "low-end {low_end}");
            assert_eq!(state.views.keyboard.focus, Focus::Continue);
            assert_eq!(state.heard().len(), heard, "low-end {low_end}: heard");
            // Counted by that frame, after its press.
            assert_eq!(state.views.keyboard.seen, Seen::Yes, "low-end {low_end}");
            state.frame_at(counted + FRAME, &[Action::Accept], &[]);
            assert_eq!(state.screen, Screen::Target, "low-end {low_end}");
        }
    }

    /// Frames drawn far apart never let Continue go on sooner than frames
    /// drawn as often as the window draws them: pressed in a frame a dwell
    /// after the one that opened the kinds, nothing between, it does
    /// nothing; pressed again in every frame, each half a second after the
    /// last, it goes on at the eighth press in all (the fourth in low-end
    /// mode), with a keyboard attached or not.
    #[test]
    fn frames_far_apart_do_not_let_continue_go_on_sooner() {
        for attached in [true, false] {
            for (low_end, pressed) in [(false, 8), (true, 4)] {
                let how = format!("low-end {low_end}, attached {attached}");
                let mut state = on_continue(attached);
                if low_end {
                    state.pretend_low_end();
                }
                state.frame_at(10.0, &[Action::Accept], &[]);
                let late = 10.0 + trail::dwell(low_end) + 0.01;
                state.frame_at(late, &[Action::Accept], &[]);
                showing(&state, ("us", ""), &how);
                let heard = state.heard().iter().filter(|one| **one == Sound::Press);
                assert_eq!(heard.count(), 1, "{how}: the press was heard");
                let mut presses = 2;
                while state.screen == Screen::Keyboard {
                    state.frame_at(state.clock.now + 0.5, &[Action::Accept], &[]);
                    presses += 1;
                    assert!(presses <= pressed, "{how}: still held");
                }
                assert_eq!(presses, pressed, "{how}");
                assert_eq!(state.screen, Screen::Target, "{how}");
            }
        }
    }

    /// Enter on the guessed layout and Enter again in one frame: the kinds
    /// open on the one in force and nothing is chosen — the second press
    /// came before they were drawn — nor while they come in; once they have
    /// stood there for the dwell, Enter chooses it. Only the kinds of the
    /// keyboard in force wait so: German opened and a kind of it chosen in
    /// one frame is a choice of the person's own, as it was.
    #[test]
    fn the_guessed_kind_is_not_chosen_before_its_kinds_have_been_seen() {
        for low_end in [false, true] {
            let mut state = on_keyboard();
            if low_end {
                state.pretend_low_end();
            }
            state.frame_at(10.0, &[Action::Accept, Action::Accept], &[]);
            let view = &state.views.keyboard;
            assert!(view.trail.open);
            assert_eq!(view.trail.at, [layout_at(&state, "us"), 0]);
            assert_eq!(view.focus, Focus::Trail, "low-end {low_end}");
            assert!(!state.chose_keyboard);
            state.frames_until(10.2);
            state.frame_at(10.2, &[Action::Accept], &[]);
            assert!(!state.chose_keyboard, "low-end {low_end}: chosen coming in");
            let after = 10.0 + trail::dwell(low_end) + 0.1;
            state.frames_until(after);
            state.frame_at(after, &[Action::Accept], &[]);
            assert!(state.chose_keyboard);
            assert_eq!(state.views.keyboard.focus, Focus::Field);
        }
        let mut state = on_keyboard();
        state.views.keyboard.trail.at[0] = layout_at(&state, "de");
        state.frame_at(10.0, &[Action::Accept, Action::Accept], &[]);
        assert!(state.chose_keyboard);
        assert_eq!(state.draft.region.keyboard.layout, "de");
    }

    /// A kind chosen by a click puts nothing else under the pointer, and
    /// holds back no click after it: another kind clicked a tenth of a second
    /// later is chosen (`screens::page` once told whether the keyboard in
    /// force had been seen, and choosing a kind, which sees it, let every
    /// click in the next half second go).
    #[test]
    fn a_kind_chosen_by_a_click_holds_back_no_click_after_it() {
        fn click_kind(state: &mut State, index: usize) {
            if point(state, 1, index) {
                act(state, Action::Accept, None);
            }
        }
        let mut state = on_keyboard();
        state.views.keyboard.trail.at[0] = layout_at(&state, "de");
        state.frame_at(10.0, &[Action::Accept], &[]);
        state.frames_until(11.0);
        state.frame_at(11.0, &[], &[|state| click_kind(state, 1)]);
        assert_eq!(state.draft.region.keyboard.variant, "nodeadkeys");
        state.frames_until(11.1);
        state.frame_at(11.1, &[], &[|state| click_kind(state, 0)]);
        assert_eq!(state.draft.region.keyboard.variant, "");
        assert_eq!(state.draft.region.keyboard.layout, "de");
    }

    /// Back from Continue while the kinds it showed were still coming in,
    /// and Keyboard arrived at again later: what had begun is over. Presses
    /// that come in the frame that arrives — Time zone's Continue, Down past
    /// the layouts to the field, Right to Continue, Enter — find the kinds
    /// shut and unseen, and Continue shows them.
    #[test]
    fn a_dwell_left_unfinished_is_over_when_the_screen_is_arrived_at_again() {
        let mut state = on_continue(true);
        state.views.timezone.seen.see();
        state.frame_at(10.0, &[Action::Accept], &[]);
        showing(&state, ("us", ""), "Continue");
        state.frames_until(10.1);
        state.frame_at(10.1, &[Action::Back], &[]);
        assert_eq!(state.screen, Screen::Timezone);
        state.frames_until(12.0);
        state.views.timezone.focus = super::super::timezone::Focus::Continue;
        let downs = state.layouts.len() - layout_at(&state, "us");
        let mut presses = vec![Action::Accept];
        presses.extend(std::iter::repeat_n(Action::Down, downs));
        presses.extend([Action::Right, Action::Accept]);
        state.frame_at(12.0, &presses, &[]);
        showing(&state, ("us", ""), "Continue on arriving again");
    }

    /// A way the columns may stand, the keyboard in force not seen, when the
    /// light goes to Continue, and its name.
    type Before = (&'static str, fn(&mut State));

    /// A double click on Continue — the button, or the legend's Choose —
    /// shows the kinds of the keyboard in force and goes no further, however
    /// the columns stood, with a keyboard attached or not: shut, the first
    /// click opens them (and `State::click` lets the second go); open on
    /// another layout's kinds, on a search narrowing them or on another kind
    /// Back brought out of a search, it rolls them over to the kind in force,
    /// the column open before and after, and the second click comes within
    /// the kinds' dwell. A click once they have stood there goes on.
    #[test]
    fn a_double_click_on_continue_shows_the_kinds_and_goes_no_further() {
        let befores: [Before; 4] = [
            ("the kinds shut", |_| {}),
            ("German's kinds open", |state| {
                let german = layout_at(state, "de");
                state.views.keyboard.trail.at[0] = german;
                act(state, Action::Accept, None);
            }),
            ("a search narrowing over German's kinds", |state| {
                let german = layout_at(state, "de");
                state.views.keyboard.trail.at[0] = german;
                act(state, Action::Accept, None);
                act(state, Action::Menu, None);
                typed(state, "german");
                act(state, Action::Right, None);
            }),
            ("a Dvorak kind open after Back from a search", |state| {
                act(state, Action::Menu, None);
                typed(state, "dvorak");
                act(state, Action::Down, None);
                act(state, Action::Down, None);
                act(state, Action::Back, None);
                let trail = &state.views.keyboard.trail;
                assert_eq!(trail.at[0], layout_at(state, "us"));
                assert_ne!(trail.at[1], 0);
            }),
        ];
        let clicks: [Clicked; 2] = [
            ("Continue", onward),
            ("the legend's Choose", choose_in_the_legend),
        ];
        for attached in [true, false] {
            for (before, stand) in befores {
                for (on, click) in clicks {
                    let how = format!("a double click on {on}, {before}, attached {attached}");
                    let mut state = on_continue(attached);
                    state.views.keyboard.focus = Focus::Trail;
                    stand(&mut state);
                    let view = &state.views.keyboard;
                    assert_eq!(view.trail.open, !before.ends_with("shut"), "{how}");
                    assert_eq!(view.seen, Seen::Not, "{how}: seen already");
                    state.views.keyboard.focus = Focus::Continue;
                    state.frame_at(10.0, &[], &[click]);
                    state.frames_until(10.2);
                    state.frame_at(10.2, &[], &[click]);
                    showing(&state, ("us", ""), &how);
                    assert_eq!(state.views.keyboard.trail.at[1], 0, "{how}");
                    state.frames_until(11.0);
                    state.frame_at(11.0, &[], &[click]);
                    assert_eq!(state.screen, Screen::Target, "{how}, then a click");
                }
            }
        }
    }

    /// Another layout's kinds opened before Continue do not show the one in
    /// force: Continue shows it, its layout back in the first column.
    #[test]
    fn continue_shows_the_kinds_in_force_wherever_the_columns_stood() {
        let mut state = on_keyboard();
        state.views.keyboard.trail.at[0] = layout_at(&state, "de");
        act(&mut state, Action::Accept, None);
        assert!(state.views.keyboard.trail.open);
        assert_eq!(state.views.keyboard.seen, Seen::Not, "another layout");
        state.views.keyboard.focus = Focus::Continue;
        state.frame_at(10.0, &[Action::Accept], &[]);
        showing(&state, ("us", ""), "from German's kinds");
        state.frames_until(11.0);
        state.frame_at(11.0, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Target);
    }

    /// Opened by the person and left on the screen for their dwell, the
    /// kinds of the keyboard in force are seen — shut again or not — and a
    /// kind chosen is seen: Continue goes on at its first press. Shut before
    /// the dwell was over, they were not: Continue's first press shows them.
    #[test]
    fn kinds_opened_or_chosen_by_the_person_let_continue_go_on_at_once() {
        let mut state = on_keyboard();
        state.frame_at(10.0, &[Action::Right], &[]);
        let seen = state.views.keyboard.seen;
        assert!(matches!(seen, Seen::Drawn { since: None, .. }), "{seen:?}");
        state.frames_until(10.5);
        assert_eq!(state.views.keyboard.seen, Seen::Yes);
        state.frame_at(10.5, &[Action::Back], &[]);
        assert!(!state.views.keyboard.trail.open);
        state.views.keyboard.focus = Focus::Continue;
        state.frame_at(10.6, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Target, "shut again, still seen");

        let mut state = on_keyboard();
        state.frame_at(10.0, &[Action::Right], &[]);
        state.frames_until(10.1);
        state.frame_at(10.1, &[Action::Back], &[]);
        assert_eq!(state.views.keyboard.seen, Seen::Not, "shut at once");
        state.views.keyboard.focus = Focus::Continue;
        state.frame_at(10.2, &[Action::Accept], &[]);
        showing(&state, ("us", ""), "after the kinds were shut");

        // German's second kind chosen, with no keyboard attached: the light
        // goes on to Continue, which goes on.
        let mut state = on_continue(false);
        state.views.keyboard.focus = Focus::Trail;
        state.views.keyboard.trail.at[0] = layout_at(&state, "de");
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.keyboard.focus, Focus::Continue);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Target);
        assert_eq!(state.draft.region.keyboard.variant, "nodeadkeys");
    }

    /// A kind taken from what a search found was seen as it was taken; and
    /// Continue while a search narrows the columns empties it, its field
    /// left up, the columns back open on the keyboard in force.
    #[test]
    fn a_search_and_continue() {
        let mut state = on_keyboard();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "dvorak");
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.keyboard.seen, Seen::Yes);
        assert_eq!(state.views.keyboard.focus, Focus::Field);
        act(&mut state, Action::Right, None);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Target);

        let mut state = on_keyboard();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "german");
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.keyboard.focus, Focus::Continue);
        state.frame_at(10.0, &[Action::Accept], &[]);
        let search = &state.views.keyboard.search;
        assert!(search.is_open() && !search.narrows(), "{search:?}");
        showing(&state, ("us", ""), "Continue over a search");
        state.frames_until(11.0);
        state.frame_at(11.0, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Target);

        // The columns coming back open on another kind as Back empties a
        // search did not show the one in force.
        let mut state = on_keyboard();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "dvorak");
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Back, None);
        let view = &state.views.keyboard;
        assert!(view.trail.open);
        assert_eq!(view.trail.at[0], layout_at(&state, "us"));
        assert_ne!(view.trail.at[1], 0);
        assert_eq!(view.seen, Seen::Not, "the standard kind was a row away");
    }

    /// Seen is kept for the installation: Back from Where to install finds
    /// Continue going on at once.
    #[test]
    fn seen_survives_back_from_where_to_install() {
        let mut state = on_continue(true);
        state.press_apart(&[Action::Accept, Action::Accept], 1.0);
        assert_eq!(state.screen, Screen::Target);
        state.back();
        assert_eq!(state.screen, Screen::Keyboard);
        assert!(!state.views.keyboard.trail.open, "came back to them open");
        state.views.keyboard.focus = Focus::Continue;
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Target);
    }

    /// Continue that goes on at the first frame it can — pressed in every
    /// frame, sixty a second, or in frames half a second apart — leaves the
    /// keyboard seen: Back from Where to install finds Continue going on at
    /// once.
    #[test]
    fn seen_survives_back_when_continue_went_on_at_the_first_frame_it_could() {
        for apart in [FRAME, 0.5] {
            for low_end in [false, true] {
                let how = format!("{apart} s apart, low-end {low_end}");
                let mut state = on_continue(true);
                if low_end {
                    state.pretend_low_end();
                }
                state.frame_at(10.0, &[Action::Accept], &[]);
                while state.screen == Screen::Keyboard {
                    state.frame_at(state.clock.now + apart, &[Action::Accept], &[]);
                }
                assert_eq!(state.screen, Screen::Target, "{how}");
                assert_eq!(state.views.keyboard.seen, Seen::Yes, "{how}");
                state.back();
                assert_eq!(state.screen, Screen::Keyboard, "{how}");
                state.views.keyboard.focus = Focus::Continue;
                state.frame_at(state.clock.now + FRAME, &[Action::Accept], &[]);
                assert_eq!(state.screen, Screen::Target, "{how}, came back");
            }
        }
    }

    /// The kinds standing open with the kind in force on the line count
    /// however the light came onto it, not only by their opening: walked
    /// there from a Dvorak kind Back brought out of a search, or left there as
    /// a search over them is emptied. Kept so for the dwell, Continue goes on
    /// at its first press; and Enter on the kind the light was walked to
    /// chooses it at once.
    #[test]
    fn the_kind_in_force_on_the_line_is_seen_however_the_light_came_to_it() {
        let walked_to_it = |state: &mut State| {
            act(state, Action::Menu, None);
            typed(state, "dvorak");
            act(state, Action::Down, None);
            act(state, Action::Down, None);
            state.frame_at(10.0, &[Action::Back], &[]);
            state.frames_until(12.0);
            assert_eq!(state.views.keyboard.seen, Seen::Not, "a Dvorak kind lit");
            let steps = state.views.keyboard.trail.at[1];
            assert_ne!(steps, 0);
            state.press_apart(&vec![Action::Up; steps], 0.3);
            assert_eq!(super::showing(state), Showing::Row);
        };
        let mut state = on_keyboard();
        walked_to_it(&mut state);
        state.frames_until(state.clock.now + 3.0);
        assert_eq!(state.views.keyboard.seen, Seen::Yes);
        state.views.keyboard.focus = Focus::Continue;
        let heard = state.heard().len();
        state.press_apart(&[Action::Accept], 0.2);
        assert_eq!(state.screen, Screen::Target);
        let pressed = state.heard()[heard..]
            .iter()
            .filter(|one| **one == Sound::Press);
        assert_eq!(pressed.count(), 1, "the kinds shown again");

        // Walked onto it, Enter chooses it at once.
        let mut state = on_keyboard();
        walked_to_it(&mut state);
        state.frame_at(state.clock.now + FRAME, &[Action::Accept], &[]);
        assert!(state.chose_keyboard);
        assert_eq!(state.views.keyboard.focus, Focus::Field);
        assert_eq!(state.draft.region.keyboard.variant, "");

        // The kinds opened on it, a search typed over them at once and
        // emptied again: they stand on it, and count from there.
        let mut state = on_keyboard();
        state.frame_at(10.0, &[Action::Accept], &[]);
        state.frames_until(10.2);
        state.frame_at(10.2, &[Action::Menu], &[]);
        typed(&mut state, "n");
        state.frames_until(11.0);
        assert_eq!(state.views.keyboard.seen, Seen::Not, "the search over it");
        state.press_apart(&[Action::Back, Action::Back], 0.3);
        assert!(!state.views.keyboard.search.is_open());
        state.frames_until(15.0);
        assert_eq!(state.views.keyboard.seen, Seen::Yes);
        state.views.keyboard.focus = Focus::Continue;
        state.frame_at(15.2, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Target);
    }

    /// While the kinds of the keyboard in force come in, a kind the person
    /// walked the light to is theirs to choose: Enter on the layout, Down,
    /// Enter chooses the first variant, a quarter of a second later or in one
    /// frame, and so does a click on it. Only the guess, the light where the
    /// opening put it, waits.
    #[test]
    fn a_kind_the_light_was_walked_to_is_chosen_while_the_kinds_come_in() {
        for low_end in [false, true] {
            let how = format!("low-end {low_end}");
            let on = |state: &State| {
                let us = &state.layouts[layout_at(state, "us")];
                us.variants[0].variant.clone()
            };
            let mut state = on_keyboard();
            if low_end {
                state.pretend_low_end();
            }
            let variant = on(&state);
            state.frame_at(10.0, &[Action::Accept], &[]);
            state.frame_at(10.12, &[Action::Down], &[]);
            state.frame_at(10.25, &[Action::Accept], &[]);
            assert!(state.chose_keyboard, "{how}");
            assert_eq!(state.draft.region.keyboard.variant, variant, "{how}");

            let mut state = on_keyboard();
            if low_end {
                state.pretend_low_end();
            }
            state.frame_at(10.0, &[Action::Accept, Action::Down, Action::Accept], &[]);
            assert_eq!(
                state.draft.region.keyboard.variant, variant,
                "{how}, one frame"
            );

            let mut state = on_keyboard();
            if low_end {
                state.pretend_low_end();
            }
            state.frame_at(10.0, &[Action::Accept], &[]);
            state.clock.tick(10.25);
            state.click(|state| {
                if point(state, 1, 1) {
                    act(state, Action::Accept, None);
                }
            });
            assert_eq!(
                state.draft.region.keyboard.variant, variant,
                "{how}, a click"
            );
        }
    }

    /// Enter on the layout in force, Down, Up and Enter, all in one frame:
    /// the light is back on the kind in force before any frame drew the
    /// kinds, which is no walk of the person's (`Trail::walked`), and nothing
    /// is chosen. Down and Up once a frame has drawn them are: Enter then
    /// chooses it.
    #[test]
    fn steps_before_a_frame_drew_the_kinds_are_no_walk_to_the_guess() {
        for low_end in [false, true] {
            let how = format!("low-end {low_end}");
            let mut state = on_keyboard();
            if low_end {
                state.pretend_low_end();
            }
            let presses = [Action::Accept, Action::Down, Action::Up, Action::Accept];
            state.frame_at(10.0, &presses, &[]);
            assert!(!state.chose_keyboard, "{how}: chosen never drawn");
            let view = &state.views.keyboard;
            assert!(view.trail.open, "{how}");
            assert!(right_rows(&state)[view.trail.at[1]].chosen, "{how}");
            assert!(!view.trail.walked(), "{how}");
            state.frame_at(10.0 + FRAME, &presses[1..], &[]);
            assert!(state.chose_keyboard, "{how}, walked");
            assert_eq!(state.draft.region.keyboard.variant, "", "{how}");
        }
    }

    /// Enter on the layout in force and Down three times in one frame: the
    /// kinds open on the one in force and the light steps off it before a
    /// frame drew them, the kind in force perhaps out of sight. Nothing
    /// begins, however long they stand there so: Continue then rolls them
    /// back onto it — nothing holds it — and goes on once they have stood
    /// there for the dwell.
    #[test]
    fn kinds_first_drawn_off_the_guess_do_not_count_it_seen() {
        for low_end in [false, true] {
            let how = format!("low-end {low_end}");
            let mut state = on_continue(true);
            state.views.keyboard.focus = Focus::Trail;
            if low_end {
                state.pretend_low_end();
            }
            let presses = [Action::Accept, Action::Down, Action::Down, Action::Down];
            state.frame_at(10.0, &presses, &[]);
            assert_eq!(super::showing(&state), Showing::Column, "{how}");
            state.frames_until(12.0);
            assert_eq!(state.views.keyboard.seen, Seen::Not, "{how}");
            state.views.keyboard.focus = Focus::Continue;
            state.frame_at(12.0, &[Action::Accept], &[]);
            showing(&state, ("us", ""), &how);
            state.frames_until(13.0);
            state.frame_at(13.0, &[Action::Accept], &[]);
            assert_eq!(state.screen, Screen::Target, "{how}");
            assert_eq!(state.draft.region.keyboard.variant, "", "{how}");
        }
    }

    /// Continue clicked in every frame once a key has opened the kinds on the
    /// one in force — the button, or the legend's Choose. A click is answered
    /// after its frame is drawn, but came in before that frame reached the
    /// display: the one in the frame that counts the kinds seen does nothing,
    /// and only a click in a frame after one that counted them goes on.
    #[test]
    fn a_click_in_the_frame_that_counts_the_kinds_seen_does_not_go_on() {
        let clicks: [Clicked; 2] = [
            ("Continue", onward),
            ("the legend's Choose", choose_in_the_legend),
        ];
        for low_end in [false, true] {
            for (on, click) in clicks {
                let how = format!("a click on {on}, low-end {low_end}");
                let mut state = on_continue(true);
                if low_end {
                    state.pretend_low_end();
                }
                state.frame_at(10.0, &[Action::Accept], &[]);
                showing(&state, ("us", ""), &how);
                while state.screen == Screen::Keyboard {
                    assert!(state.clock.now < 12.0, "{how}: never went on");
                    let before = state.views.keyboard.seen;
                    state.frame_at(state.clock.now + FRAME, &[], &[click]);
                    if state.screen != Screen::Keyboard {
                        assert_eq!(before, Seen::Yes, "{how}: went on as it was counted");
                    }
                }
                assert_eq!(state.screen, Screen::Target, "{how}");
            }
        }
    }

    /// Choosing a language guesses the keyboard again (`guess_the_rest`,
    /// which is what choosing one does): one that guesses another keyboard
    /// puts it out of sight, so Continue shows it first again; one that
    /// guesses the same keyboard — Canada's English, which moves the zone to
    /// Toronto and keeps the US keyboard — leaves it seen, the zone alone
    /// unseen again.
    #[test]
    fn a_language_guessing_another_keyboard_has_it_seen_again() {
        let guessed = |state: &mut State, locale: &str| {
            state.draft.region.locale = locale.into();
            crate::ui::screens::language::guess_the_rest(state);
        };
        let mut state = State::demo(true);
        state.views.timezone.seen.see();
        state.go(Screen::Keyboard);
        state.press_apart(&[Action::Right], 1.0);
        state.frames_until(state.clock.now + 1.0);
        assert_eq!(state.views.keyboard.seen, Seen::Yes);
        state.go(Screen::Language);
        guessed(&mut state, "en_CA.UTF-8");
        assert_ne!(state.draft.region.timezone, "America/New_York");
        assert_eq!(state.views.timezone.seen, Seen::Not, "the zone moved");
        assert_eq!(state.draft.region.keyboard.layout, "us");
        assert_eq!(state.views.keyboard.seen, Seen::Yes, "the same keyboard");
        guessed(&mut state, "de_DE.UTF-8");
        assert_eq!(state.draft.region.keyboard.layout, "de");
        assert_eq!(state.views.keyboard.seen, Seen::Not, "German unshown");
        state.go(Screen::Keyboard);
        state.views.keyboard.focus = Focus::Continue;
        state.press_apart(&[Action::Accept], 1.0);
        showing(&state, ("de", ""), "after Deutsch");
        state.press_apart(&[Action::Accept], 1.0);
        assert_eq!(state.screen, Screen::Target);
        // A keyboard chosen is never guessed over, and stays seen.
        state.back();
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Accept, None);
        assert!(state.chose_keyboard);
        guessed(&mut state, "pl_PL.UTF-8");
        assert_eq!(state.draft.region.keyboard.layout, "de");
        assert_eq!(state.views.keyboard.seen, Seen::Yes);
    }
}
