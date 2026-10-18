//! Time zone: the parts of the world, and the places in the one pressed,
//! walked as LineXinBar's file picker walks a disk (`trail`), starting on
//! the guess the chosen language made, and Continue under them.
//!
//! Everything is said in the person's language — the one chosen on the
//! Language step, whichever of its 65 it is, even where the installer's own
//! words stay English: the parts of the world from the catalogs, the places
//! and their countries as CLDR names them there (Warszawa, Нью-Йорк, 纽约;
//! `model::places`), sorted as that language sorts them. A place the
//! language has no name of its own for keeps the time zone database's. A
//! place inside a country that has several (`Argentina / Buenos Aires`) is
//! named by its city, the country under it.
//!
//! The places are not shown until a part of the world is pressed: the
//! guess puts the light on its part of the world, and Accept (or Right, or a
//! click on it) opens its places beside it, the light on the guessed one;
//! Back closes them again. Choosing a place is the press that answers the
//! question, as choosing a language is: the zone is taken and the screen
//! goes on. Continue keeps the zone in force.
//!
//! **Continue keeps it only once it has been seen**, so that the person
//! installing is aware of what is selected by default. Seen means that the
//! places of the part of the world the zone in force is in, that place marked
//! among them, have stood open on the screen for a dwell, counted by the
//! frames that draw them (`trail::Seen`): from the second frame that drew
//! them — by which the first had gone to the display — to one drawn the dwell
//! later, their opening's own length, 0.45 s, or 0.3 s in low-end mode, where
//! they come in whole at once (`trail::OPENING`); and, where they slide in,
//! only once the frames have carried them all the way in, each frame a tenth
//! of a second of that at most, so that a frame or two drawn across a stall,
//! the places barely begun in them, never count however long it lasted. Only
//! a frame counts them, never a press: every press that reaches a frame is
//! answered at its one instant of the installer's clock and before it is
//! drawn — a click, asked after the drawing, finds them as they stood before
//! it (`State::as_before_drawing`) — so two presses in one frame are no time
//! apart, and a press may have been made at any moment since the frame
//! before. Until it is seen, Continue does not go on — not Accept on it
//! (Enter, Space, South), nor a click on it, nor the legend's Choose:
//!
//! * with those places not open on the zone in force, the press opens them,
//!   as pressing their part of the world would, the light on the line on the
//!   place in force, with the same motion and sound, wherever the first
//!   column's light had got to (and a search narrowing the columns emptied
//!   first, so they are back to show it); the dwell begins with the frames
//!   that draw them — for a key or a pad the frame of the press, for a click
//!   the frame after, which the frame asks after drawing;
//! * with them open on it and the dwell not counted — the press came in the
//!   frame that opened them, while they were coming in, or in the frame
//!   that counts them, before it was drawn — it does nothing, and is not
//!   heard (a key's or a pad's Accept dips Continue, as every Accept dips
//!   what the light is on).
//!
//! The light stays on Continue either way, and the first press after a
//! frame has counted them goes on — and leaves them seen, so Back from
//! Keyboard finds Continue going on at once. A double click on Continue is
//! held the same way: where its first click opened the places,
//! `State::click` lets go of a second within half a second; where they stood
//! open already — on another part of the world, behind a search, on another
//! place — and the first click only rolled them over to the zone in force, a
//! second click is held while the dwell runs, as any press is.
//!
//! The same dwell holds back taking the guess, and no more than that: Accept
//! on the zone in force, or a click on it, while its places are dwelling with
//! the light where their opening put it — Enter on the guessed part of the
//! world and Enter again in one frame — does nothing, so the guess is never
//! taken before it was drawn. A place the person walked the light to once a
//! frame had drawn the places (`Trail::walked`), even while they come in;
//! another part of the world's places, which do not hold the zone in force;
//! and what a search found are not held.
//!
//! What has been seen is the view's ([`View::seen`]). The dwell begins with
//! a frame that draws the places standing open with the zone in force on the
//! line: opened so by any press — Accept, Right or a click on their part of
//! the world, as by Continue — or the light coming onto it in them, walked
//! there from another place or left there as a search over them is emptied;
//! not with places that opened on it, the light stepped off it before a frame
//! drew them, which Continue rolls back onto it. Leaving them open for it
//! counts, the light walked to other places of theirs or not.
//! Shutting them, or a search narrowing the columns over them, before it is
//! counted does not, and it begins again when they stand on it. Taking a
//! place counts at once, in the columns or from what a search found, as it
//! is taken. Opening another part of the world does not count, nor do the
//! columns coming back open on another place as a search is emptied, until
//! the light comes onto the zone in force. Seen stays for the rest of the
//! installation — Back from Keyboard finds it seen and Continue goes on at
//! once — until choosing a language guesses another zone than the one in
//! force (`language::guess_the_rest`), which nobody has seen yet. A guess
//! that comes to the same zone leaves it seen, and so does any language once
//! the person has chosen a place, which no guess changes again. A zone that
//! no part of the world offered holds has nowhere to be shown, and Continue
//! goes on with it.
//!
//! Options (North, the Menu key, F10) brings a search field up over the
//! columns (`search`). Typed into, it shows the places it finds straight
//! away, in one column, whatever part of the world they are in — every
//! place whose city, country or part of the world holds every word typed,
//! in the person's language or in the database's English (Warszawa or
//! Warsaw, Polska or Poland) — and Accept on one takes it. Emptied, the
//! columns are back as they were.

use lxb_app::lxb_toolkit::{input::Action, metrics::Metric, sound::Sound, typography::Text};
use lxb_app::{Page, Scroll};

use super::{Head, Room};
use crate::i18n;
use crate::model::catalogs::Zone;
use crate::ui::app::State;
use crate::ui::button::Button;
use crate::ui::column::{Column, Row};
use crate::ui::field::{self, Edit, FieldId};
use crate::ui::legend::Hint;
use crate::ui::panel::Note;
use crate::ui::search::{Backed, Search};
use crate::ui::spots;
use crate::ui::trail::{self, Click, Look, Seen, Showing, Trail, Went};

/// The numbers the two columns answer to.
const IDS: [spots::Column; 2] = [spots::Column::nth(0), spots::Column::nth(1)];

/// The marks the rows wear: the globe for a part of the world, a clock for
/// a place, whose clock is what the screen sets.
const MARKS: [&str; 2] = ["category-internet", "setting-schedule"];

/// Where the light stands on the time zone screen.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// In the search field over the columns, while it is up.
    Search,
    /// In the columns — or, while a search narrows them, in the one column
    /// of the places it found.
    #[default]
    Trail,
    Continue,
}

/// What this screen keeps between frames.
#[derive(Debug, Default, Clone)]
pub struct View {
    pub trail: Trail,
    pub focus: Focus,
    /// The search over the places.
    pub search: Search,
    /// The one column of the places the search found, walked as the
    /// trail's first column is; the trail itself is kept as it was, for
    /// when the search is emptied.
    pub found: Trail,
    /// Whether the zone in force has been seen: the places of its part of
    /// the world kept open on the screen with it among them for the dwell,
    /// as the frames count it (`trail::Seen`), or a place chosen. Until then
    /// Continue opens them rather than going on ([`onward`]).
    pub seen: Seen,
}

impl View {
    pub fn settle(&mut self) {
        self.trail.settle();
        self.found.settle();
    }
}

/// The widest the search field stands, in reference pixels: as wide as a
/// row of a form reads well, from the first column's heading.
pub const SEARCH_WIDEST: f32 = 760.0;

/// The parts of the world the zones fall in, in the zones' order.
fn regions(zones: &[Zone]) -> Vec<&str> {
    let mut regions: Vec<&str> = Vec::new();
    for zone in zones {
        if regions.last() != Some(&zone.region.as_str()) {
            regions.push(&zone.region);
        }
    }
    regions
}

/// A part of the world, in words. One the catalogs do not know is shown
/// as the database names it rather than not at all.
fn region_words(region: &str) -> String {
    match region {
        "Africa" => i18n::text("zone-region-africa").to_string(),
        "America" => i18n::text("zone-region-america").to_string(),
        "Antarctica" => i18n::text("zone-region-antarctica").to_string(),
        "Arctic" => i18n::text("zone-region-arctic").to_string(),
        "Asia" => i18n::text("zone-region-asia").to_string(),
        "Atlantic" => i18n::text("zone-region-atlantic").to_string(),
        "Australia" => i18n::text("zone-region-australia").to_string(),
        "Europe" => i18n::text("zone-region-europe").to_string(),
        "Indian" => i18n::text("zone-region-indian").to_string(),
        "Pacific" => i18n::text("zone-region-pacific").to_string(),
        "UTC" => i18n::text("zone-region-utc").to_string(),
        other => other.replace('_', " "),
    }
}

/// The left-hand rows: each part of the world, how many places it holds,
/// and the mark on the one the zone in force is in.
fn left_rows(state: &State) -> Vec<Row> {
    let current = state.draft.region.timezone.as_str();
    regions(&state.zones)
        .into_iter()
        .map(|region| {
            let count = state
                .zones
                .iter()
                .filter(|zone| zone.region == region)
                .count();
            let holds_current = state
                .zones
                .iter()
                .any(|zone| zone.region == region && zone.name == current);
            Row::new(region_words(region))
                .note(crate::message!("timezone-places", "count" => count as i64))
                .chosen(holds_current)
        })
        .collect()
}

/// The zones of the part of the world the first column's light is on.
fn places(state: &State) -> Vec<&Zone> {
    let regions = regions(&state.zones);
    let Some(region) = regions.get(state.views.timezone.trail.at[0]) else {
        return Vec::new();
    };
    state
        .zones
        .iter()
        .filter(|zone| zone.region == *region)
        .collect()
}

/// A place's name in the person's language: its city, or universal time's
/// own words.
pub fn place_name(zone: &Zone) -> String {
    if zone.city.is_empty() {
        i18n::text("zone-utc").to_string()
    } else {
        zone.shown_city.clone()
    }
}

/// What stands under a place inside a country that has several
/// (`Argentina / Buenos Aires`, `Indiana / Knox`): its country in the
/// person's language, or where none is known what the database names it
/// within. Nothing for any other place.
fn within(zone: &Zone) -> Option<&str> {
    let (within, _) = zone.city.rsplit_once(" / ")?;
    Some(if zone.shown_country.is_empty() {
        within
    } else {
        &zone.shown_country
    })
}

/// The right-hand rows: each place by its city, its country under it where
/// the database names it within one.
fn right_rows(state: &State) -> Vec<Row> {
    let current = state.draft.region.timezone.as_str();
    places(state)
        .into_iter()
        .map(|zone| {
            let row = Row::new(place_name(zone));
            let row = match within(zone) {
                Some(within) => row.note(within),
                None => row,
            };
            row.chosen(zone.name == current)
        })
        .collect()
}

/// Take the place at `index` of the second column and go on.
fn choose(state: &mut State, index: usize) {
    let Some(name) = places(state).get(index).map(|zone| zone.name.clone()) else {
        return;
    };
    take(state, name);
}

/// Take the zone called `name` and go on. It was seen as it was chosen, in
/// the columns or among what a search found.
fn take(state: &mut State, name: String) {
    state.draft.region.timezone = name;
    state.chose_zone = true;
    state.views.timezone.seen.see();
    state.forward();
}

/// How the places of the part of the world the zone in force is in stand
/// on the screen (`trail::Showing`): open, no search narrowing the columns
/// over them, with the light on the zone in force among them — on the line,
/// marked — or on another of them; or not at all.
fn showing(state: &State) -> Showing {
    let view = &state.views.timezone;
    if !view.trail.open || view.search.narrows() {
        return Showing::Not;
    }
    let current = state.draft.region.timezone.as_str();
    let places = places(state);
    if places
        .get(view.trail.at[1])
        .is_some_and(|zone| zone.name == current)
    {
        Showing::Row
    } else if places.iter().any(|zone| zone.name == current) {
        Showing::Column
    } else {
        Showing::Not
    }
}

/// Whether the zone in force has been seen: counted so by a frame drawn
/// before this one's presses (`Seen::seen`), or chosen.
fn seen(state: &State) -> bool {
    state.views.timezone.seen.seen()
}

/// Whether the places have opened on the zone in force, or the light come
/// onto it among them, and no frame has yet counted it seen.
fn dwelling(state: &State) -> bool {
    state.views.timezone.seen.dwelling()
}

/// Whether taking the place at `index` would take the guess before its
/// places were seen: it is the zone in force, its places are still dwelling
/// on it, and the light stands where their opening put it, not walked there
/// by the person (`Trail::walked`) — Enter on the part of the world and
/// Enter again in one frame, or a click on the place as they come in.
fn too_soon(state: &State, index: usize) -> bool {
    dwelling(state)
        && !state.views.timezone.trail.walked()
        && right_rows(state).get(index).is_some_and(|row| row.chosen)
}

/// The places have just opened. With the zone in force on the line among
/// them, marked as chosen, its dwell begins (`Seen::opened`), to run from the
/// frames that draw them. Opened on another part of the world, or on another
/// place — the one the light was on among what a search found, Back
/// emptying the search — the zone in force may be rows away or not there at
/// all, and nothing begins until the light comes onto it.
fn opened(state: &mut State) {
    if showing(state) == Showing::Row {
        state.views.timezone.seen.opened();
    } else {
        state.views.timezone.seen.shut();
    }
}

/// What the frame tells the screen once it has drawn it (`screens::drawn`):
/// how the places of the zone in force stood on it, at this frame's instant
/// and for its length, which begins their dwell (the frame that first draws
/// them open on it, or the light come onto it among them), counts them seen
/// once it has run, or — not there — ends it. Answers whether the dwell is
/// still running, for the window to keep drawing frames to time it.
pub fn drawn(state: &mut State) -> bool {
    let showing = showing(state);
    let (now, dt, low_end) = (state.clock.now, state.clock.dt, state.low_end());
    let view = &mut state.views.timezone;
    // The columns, not what a search found in their place: from now on a
    // step in the places is the person's walk (`Trail::walked`).
    if !view.search.narrows() {
        view.trail.drawn();
    }
    view.seen.drawn(showing, now, dt, low_end);
    view.seen.dwelling()
}

/// Continue: on to Keyboard with the zone in force, which a frame drawn
/// before this press has counted seen — or, while none has, its places
/// opened instead and nothing more, the light left on Continue for the press
/// that goes on. While they stand open on it and are dwelling, the press does
/// nothing at all, and is not heard: it may have come before the person could
/// have seen the zone — in the very frame that opened them, while they were
/// still coming in, or before the frame that counts them was drawn.
fn onward(state: &mut State) {
    if !seen(state) {
        if dwelling(state) && showing(state) != Showing::Not {
            state.views.timezone.focus = Focus::Continue;
            return;
        }
        if show_the_zone_in_force(state) {
            return;
        }
    }
    super::onward(state);
}

/// Open the places of the part of the world the zone in force is in, the
/// light in them on that zone, as pressing that part of the world opens
/// them — whatever row of the first column the light had walked to, and with
/// a search that narrows the columns emptied first, so they are back to show
/// it. The light is on Continue, which was pressed, and the zone's dwell
/// begins. Answers whether they opened: a zone no part of the world offered
/// holds has nowhere to be shown, and counts as seen as it is.
fn show_the_zone_in_force(state: &mut State) -> bool {
    let current = state.draft.region.timezone.as_str();
    let Some(at) = state
        .zones
        .iter()
        .find(|zone| zone.name == current)
        .and_then(|zone| {
            regions(&state.zones)
                .iter()
                .position(|one| *one == zone.region)
        })
    else {
        state.views.timezone.seen.see();
        return false;
    };
    let view = &mut state.views.timezone;
    if view.search.narrows() {
        view.search.back();
    }
    view.focus = Focus::Continue;
    view.trail.at[0] = at;
    let right = right_rows(state);
    if !state.views.timezone.trail.open_on(&right) {
        state.views.timezone.seen.see();
        return false;
    }
    state.views.timezone.seen.opened();
    state.sound(Sound::Press);
    true
}

/// The places the search finds, in the zones' order (each part of the
/// world's places as the person's language sorts them), whatever part of
/// the world they are in: by the city (and the country or state it is
/// named within), the country, and the part of the world, each in the
/// person's language and in the database's own English.
fn found(state: &State) -> Vec<&Zone> {
    let search = &state.views.timezone.search;
    if !search.narrows() {
        return Vec::new();
    }
    state
        .zones
        .iter()
        .filter(|zone| {
            let region = region_words(&zone.region);
            let english = zone.region.replace('_', " ");
            let utc = if zone.city.is_empty() {
                i18n::text("zone-utc")
            } else {
                ""
            };
            search.finds(&[
                &zone.shown_city,
                &zone.city,
                &zone.shown_country,
                &zone.country,
                &region,
                &english,
                utc,
            ])
        })
        .collect()
}

/// The rows of the column of what the search found: each place by its
/// city, and under it its country and its part of the world.
fn found_rows(state: &State) -> Vec<Row> {
    let current = state.draft.region.timezone.as_str();
    found(state)
        .into_iter()
        .map(|zone| {
            let name = place_name(zone);
            let place = if zone.shown_country.is_empty() {
                within(zone).unwrap_or_default()
            } else {
                zone.shown_country.as_str()
            };
            let region = region_words(&zone.region);
            let note = if place.is_empty() {
                region
            } else {
                crate::message!("timezone-found-note", "place" => place, "region" => region)
            };
            Row::new(name).note(note).chosen(zone.name == current)
        })
        .collect()
}

/// The pictures of the time zone screen: as it is arrived at, the parts of
/// the world alone with the light on the guessed one (`timezone`); the same
/// with the light gone down to Continue, the guess's places not yet seen, so
/// that a press of it (`--press accept`) opens them (`timezone-continue`);
/// with the guessed one pressed, its places open beside it (`timezone-places`),
/// so that a narrow panel is seen slid over to them; and the search up,
/// "united" typed in it (`timezone-search`), or "qqqq", which finds nothing
/// (`timezone-search-nothing`).
pub fn stage(state: &mut State, scene: &str) {
    if scene == "timezone-continue" {
        state.views.timezone.focus = Focus::Continue;
    }
    if scene == "timezone-places" {
        let right = right_rows(state);
        state.views.timezone.trail.open_on(&right);
        // Standing open as long after their opening as a settled picture
        // shows them: seen, as the person who opened them has seen them.
        state.views.timezone.seen.see();
    }
    if scene == "timezone-search" {
        to_search(state);
        searched(state, &Edit::Wrote("united".into()));
    }
    if scene == "timezone-search-nothing" {
        to_search(state);
        searched(state, &Edit::Wrote("qqqq".into()));
    }
}

fn verb() -> Button<'static> {
    Button::primary(i18n::text("button-continue"))
}

pub fn enter(state: &mut State) {
    state.views.timezone.search.close();
    let left = left_rows(state);
    let view = &mut state.views.timezone;
    view.focus = Focus::Trail;
    view.trail.start(&left);
    // The places are shut on arriving: a dwell left unfinished by leaving
    // (Back from Continue as they came in) is over, as their shutting ends
    // it, rather than standing until this screen's first frame is drawn.
    view.seen.shut();
}

pub fn head(_state: &State) -> Head {
    Head::new(
        "setting-schedule",
        i18n::text("timezone-title"),
        i18n::text("timezone-caption"),
    )
}

pub fn hints(state: &State) -> Vec<Hint> {
    super::search_hints(state, state.views.timezone.focus == Focus::Search)
}

pub fn note(_state: &State) -> Option<Note> {
    None
}

pub fn act(state: &mut State, action: Action, scroll: Option<Scroll>) {
    if let Some(scroll) = scroll {
        if IDS.iter().any(|ids| Column::owns(scroll.spot, *ids)) {
            state.views.timezone.focus = Focus::Trail;
        }
    }
    match (state.views.timezone.focus, action) {
        (_, Action::Menu) => return to_search(state),
        // Back empties the search, then closes it; with no search up it is
        // the columns' and the screen's, as it always was.
        (_, Action::Back) if state.views.timezone.search.is_open() => return back(state),
        (Focus::Search, Action::Accept) if state.pad => {
            return state.type_with_pad(FieldId::Search)
        }
        // Enter in the field, or Down out of it: the light lands on the
        // first place found.
        (Focus::Search, Action::Accept | Action::Down) => {
            if !land(state) {
                state.sound(Sound::Error);
            }
            return;
        }
        (Focus::Search, Action::Right) => {
            state.views.timezone.focus = Focus::Continue;
            state.sound(Sound::Move);
            return;
        }
        (Focus::Search, _) => return,
        _ => {}
    }
    if state.views.timezone.focus == Focus::Continue {
        match action {
            Action::Up | Action::Left => {
                // Back into the columns, which roll to the light again
                // wherever the pointer held them — or into the field,
                // where a search found nothing to stand on.
                let nowhere = state.views.timezone.search.narrows() && found(state).is_empty();
                let view = &mut state.views.timezone;
                view.focus = if nowhere { Focus::Search } else { Focus::Trail };
                view.trail.let_go();
                view.found.let_go();
                state.sound(Sound::Move);
            }
            Action::Accept => onward(state),
            Action::Back => state.back(),
            _ => {}
        }
        return;
    }
    if state.views.timezone.search.narrows() {
        return act_on_found(state, action);
    }
    let left = left_rows(state);
    let right = right_rows(state);
    let went = state.views.timezone.trail.act(action, &left, &right);
    // Up from the top of the parts of the world, with the search up: into
    // its field.
    if went == Went::Nothing && action == Action::Up && state.views.timezone.search.is_open() {
        state.views.timezone.focus = Focus::Search;
        state.sound(Sound::Move);
        return;
    }
    answer(state, went, action);
}

/// A press in the column of the places the search found: Up and Down walk
/// it, Accept takes the place, and past its ends the light goes to the field
/// or on to Continue.
fn act_on_found(state: &mut State, action: Action) {
    let rows = found_rows(state);
    if action == Action::Accept {
        let at = state.views.timezone.found.at[0];
        if let Some(name) = found(state).get(at).map(|zone| zone.name.clone()) {
            take(state, name);
        }
        return;
    }
    let went = state.views.timezone.found.act(action, &rows, &[]);
    match (went, action) {
        (Went::Nothing, Action::Up) => {
            state.views.timezone.focus = Focus::Search;
            state.sound(Sound::Move);
        }
        _ => answer(state, went, action),
    }
}

/// Options: the search field comes up, the light in it — and with a pad in
/// hand a board to type with at once.
fn to_search(state: &mut State) {
    let view = &mut state.views.timezone;
    view.search.open();
    view.focus = Focus::Search;
    if state.pad {
        state.type_with_pad(FieldId::Search);
    } else {
        state.sound(Sound::Press);
    }
}

/// The light lands on the first place the search found — or, with nothing
/// typed in it, back in the columns. Answers whether it did.
pub fn land(state: &mut State) -> bool {
    let narrows = state.views.timezone.search.narrows();
    if narrows && found(state).is_empty() {
        return false;
    }
    let view = &mut state.views.timezone;
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
/// The place the light was on among those found is where the columns stand
/// when they come back: its part of the world, its places open on it.
fn back(state: &mut State) {
    let lit = match state.views.timezone.focus {
        Focus::Trail if state.views.timezone.search.narrows() => found(state)
            .get(state.views.timezone.found.at[0])
            .map(|zone| (zone.region.clone(), zone.name.clone())),
        _ => None,
    };
    let backed = state.views.timezone.search.back();
    debug_assert_ne!(backed, Backed::Nothing);
    if let Some((region, name)) = lit {
        if let Some(at) = regions(&state.zones).iter().position(|one| *one == region) {
            state.views.timezone.trail.at[0] = at;
            let right = right_rows(state);
            let index = places(state).iter().position(|zone| zone.name == name);
            let trail = &mut state.views.timezone.trail;
            if trail.open_on(&right) {
                if let Some(index) = index {
                    trail.at[1] = index;
                }
            }
            trail.settle();
            opened(state);
        }
    }
    let view = &mut state.views.timezone;
    if view.focus == Focus::Search && !view.search.is_open() {
        view.focus = Focus::Trail;
    }
    state.sound(Sound::Back);
}

/// Something typed into the search: the places it finds, the light — in
/// the field — over the first of them.
pub fn searched(state: &mut State, edit: &Edit) {
    if !state.views.timezone.search.edit(edit) {
        return;
    }
    let none = found(state).is_empty();
    let view = &mut state.views.timezone;
    view.found.start_on(0);
    if view.focus == Focus::Trail && view.search.narrows() && none {
        view.focus = Focus::Search;
    }
}

/// Answer what a press — a key's or a click's — did to the columns.
fn answer(state: &mut State, went: Went, action: Action) {
    match went {
        Went::Nothing => {
            // Past the foot of a column, or on past the places: down to
            // Continue.
            if matches!(action, Action::Down | Action::Right) {
                state.views.timezone.focus = Focus::Continue;
                state.sound(Sound::Move);
            }
        }
        Went::Moved => state.sound(Sound::Move),
        Went::Opened => {
            opened(state);
            state.sound(Sound::Press);
        }
        // Accept on the guess while the places that opened on it have not
        // stood on the screen for the dwell, the light where their opening
        // put it — Enter on the part of the world and Enter again in one
        // frame — does nothing: it would be taken never drawn. A place the
        // person walked the light to is theirs to take at once.
        Went::Chose(index) if too_soon(state, index) => {}
        Went::Chose(index) => choose(state, index),
        Went::Closed => {
            state.views.timezone.seen.shut();
            state.sound(Sound::Back);
        }
        Went::Pass => {
            if action == Action::Back {
                state.back();
            }
        }
    }
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let mut content = room.content;
    // The search field over the columns, lined up with the first column's
    // heading, while it is up.
    let mut searched_in = None;
    if state.views.timezone.search.is_open() {
        let tall = field::height(page);
        let gap = page.metric(Metric::Gap);
        let edge = trail::first_edge(content[2], page.scaled(1.0));
        let rect = [
            content[0] + edge,
            content[1],
            (content[2] - edge).min(page.scaled(SEARCH_WIDEST)),
            tall,
        ];
        content[1] += tall + gap * 1.5;
        content[3] = (content[3] - tall - gap * 1.5).max(1.0);
        let query = state.views.timezone.search.query().to_string();
        let in_field = lit && state.views.timezone.focus == Focus::Search;
        let placeholder = i18n::text("search-timezone-placeholder");
        super::search_field(state, page, rect, &query, placeholder, in_field);
        searched_in = Some(rect);
    }
    let in_trail = lit && state.views.timezone.focus == Focus::Trail;
    let pressing = state.pressing.through();
    let dt = state.clock.dt;
    if state.views.timezone.search.narrows() {
        // What the search found, in one column, whatever part of the world.
        let rows = found_rows(state);
        if rows.is_empty() {
            let query = state.views.timezone.search.query().to_string();
            let heading = trail::heading_tall(page.line(Text::Caption), page.metric(Metric::Gap));
            let under = super::under_heading(content, heading, searched_in);
            super::found_nothing(page, under, &query);
        }
        let look = Look {
            headings: [i18n::text("timezone-heading-found"), ""],
            marks: [MARKS[1], MARKS[1]],
            ids: IDS,
            pressing,
            dt,
        };
        let light = (in_trail && !rows.is_empty()).then_some(&mut state.light);
        state
            .views
            .timezone
            .found
            .draw(page, content, [&rows, &[]], light, look);
    } else {
        let left = left_rows(state);
        let right = right_rows(state);
        let look = Look {
            headings: [
                i18n::text("timezone-heading-region"),
                i18n::text("timezone-heading-city"),
            ],
            marks: MARKS,
            ids: IDS,
            pressing,
            dt,
        };
        let light = in_trail.then_some(&mut state.light);
        state
            .views
            .timezone
            .trail
            .draw(page, content, [&left, &right], light, look);
    }
    let on_button = lit && state.views.timezone.focus == Focus::Continue;
    super::lone_button(state, page, room.buttons, verb(), on_button);
}

/// Put the light on row `index` of column `level`, which the pointer moved
/// onto or clicked. Answers whether it stands there.
fn point(state: &mut State, level: usize, index: usize) -> bool {
    let landed = if state.views.timezone.search.narrows() {
        let rows = found_rows(state);
        state.views.timezone.found.point_at(level, index, &rows)
    } else {
        let rows = [left_rows(state), right_rows(state)];
        state
            .views
            .timezone
            .trail
            .point_at(level, index, &rows[level])
    };
    if landed {
        state.views.timezone.focus = Focus::Trail;
    }
    landed
}

/// The two columns as they are drawn: the trail's, or the one of what the
/// search found and nothing beside it.
fn columns(state: &State) -> [Vec<Row>; 2] {
    if state.views.timezone.search.narrows() {
        [found_rows(state), Vec::new()]
    } else {
        [left_rows(state), right_rows(state)]
    }
}

pub fn hover(state: &mut State, id: u32) {
    if super::button_of(id, 1).is_some() {
        state.views.timezone.focus = Focus::Continue;
        return;
    }
    if id == FieldId::Search.spot() && state.views.timezone.search.is_open() {
        state.views.timezone.focus = Focus::Search;
        return;
    }
    let [left, right] = columns(state);
    if let Some((level, index)) = Trail::row_of(id, IDS, [&left, &right]) {
        point(state, level, index);
    }
}

pub fn pressed(state: &mut State, page: &mut Page) {
    if state.views.timezone.search.is_open() && page.pressed(FieldId::Search.spot()) {
        // A click puts the light in the field, where a keyboard types; on a
        // field whose shell's board was put away it asks for that board
        // again.
        state.views.timezone.focus = Focus::Search;
        state.clicked_field(FieldId::Search);
        return;
    }
    let [left, right] = columns(state);
    let view = &mut state.views.timezone;
    let trail = if view.search.narrows() {
        &mut view.found
    } else {
        &mut view.trail
    };
    let front = trail.front();
    match trail.pressed(page, [&left, &right], IDS) {
        // A part of the world opens, a place is taken: what Accept on it
        // does, wherever the light was.
        Some(Click::Row(index)) => {
            if point(state, front, index) {
                act(state, Action::Accept, None);
            }
            return;
        }
        Some(Click::Closed) => {
            state.views.timezone.focus = Focus::Trail;
            return answer(state, Went::Closed, Action::Back);
        }
        None => {}
    }
    if super::lone_pressed(page, verb()) {
        onward(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::app::Screen;
    use crate::ui::motion::FRAME;

    fn on_timezone() -> State {
        let mut state = State::demo(true);
        state.go(Screen::Timezone);
        state
    }

    fn region_at(state: &State, region: &str) -> usize {
        regions(&state.zones)
            .iter()
            .position(|one| *one == region)
            .unwrap()
    }

    #[test]
    fn it_opens_on_the_part_of_the_world_in_force_with_its_places_shut() {
        let state = on_timezone();
        assert_eq!(state.draft.region.timezone, "America/New_York");
        let left = left_rows(&state);
        let trail = &state.views.timezone.trail;
        assert!(left[trail.at[0]].chosen);
        assert!(
            !trail.open,
            "the places were shown before anything was pressed"
        );
    }

    #[test]
    fn pressing_a_part_of_the_world_opens_its_places_on_the_one_in_force() {
        let mut state = on_timezone();
        act(&mut state, Action::Accept, None);
        let trail = &state.views.timezone.trail;
        assert!(trail.open);
        let right = right_rows(&state);
        assert_eq!(right[trail.at[1]].name, "New York");
        assert!(right[trail.at[1]].chosen);
        assert_eq!(state.screen, Screen::Timezone, "opening chose nothing");
    }

    #[test]
    fn right_opens_the_places_as_accept_does() {
        let mut state = on_timezone();
        act(&mut state, Action::Right, None);
        assert!(state.views.timezone.trail.open);
        assert_eq!(state.views.timezone.focus, Focus::Trail);
    }

    #[test]
    fn back_closes_the_places_and_then_goes_back() {
        let mut state = on_timezone();
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Back, None);
        assert!(!state.views.timezone.trail.open);
        assert_eq!(
            state.screen,
            Screen::Timezone,
            "Back left a screen it only had to close"
        );
        act(&mut state, Action::Back, None);
        assert_eq!(state.screen, Screen::Language);
    }

    #[test]
    fn left_closes_the_places_too() {
        let mut state = on_timezone();
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Left, None);
        assert!(!state.views.timezone.trail.open);
        assert_eq!(state.screen, Screen::Timezone);
    }

    #[test]
    fn a_city_inside_a_country_is_named_by_the_city_with_the_country_under_it() {
        let state = on_timezone();
        let right = right_rows(&state);
        let buenos_aires = right
            .iter()
            .find(|row| row.name == "Buenos Aires")
            .expect("the demo has Buenos Aires");
        assert_eq!(buenos_aires.note.as_deref(), Some("Argentina"));
    }

    #[test]
    fn moving_through_the_parts_of_the_world_keeps_their_places_shut() {
        let mut state = on_timezone();
        act(&mut state, Action::Down, None);
        assert!(!state.views.timezone.trail.open);
        let right = right_rows(&state);
        assert!(right.iter().any(|row| row.name == "Dubai"), "{right:?}");
    }

    #[test]
    fn choosing_a_place_takes_it_and_goes_on() {
        let mut state = on_timezone();
        // Europe pressed, and down from its first place to the second.
        state.views.timezone.trail.at[0] = region_at(&state, "Europe");
        act(&mut state, Action::Accept, None);
        assert!(state.views.timezone.trail.open);
        state.views.timezone.trail.at[1] = 0;
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.draft.region.timezone, "Europe/Lisbon");
        assert!(state.chose_zone);
        assert_eq!(state.screen, Screen::Keyboard);
    }

    /// How far apart the e2e harness presses its keys at the closest
    /// (`tests/e2e/scenarios.py`: `press` waits 0.6 s after each key, and two
    /// seconds more after the last of a call).
    const HARNESS_APART: f32 = 0.6;

    /// The harness walks this screen with two presses of Enter
    /// (`tests/e2e/scenarios.py`, `walk_to_target`, and `live_lxb` in one
    /// call): the part of the world the guess lit, then the place it lit.
    /// They keep the guess: by the second the places have stood on the
    /// screen longer than their dwell.
    #[test]
    fn two_presses_keep_the_guessed_zone_and_go_on() {
        const { assert!(HARNESS_APART > trail::OPENING && HARNESS_APART > trail::OPENING_AT_ONCE) };
        let mut state = on_timezone();
        state.press_apart(&[Action::Accept, Action::Accept], HARNESS_APART);
        assert_eq!(state.draft.region.timezone, "America/New_York");
        assert_eq!(state.screen, Screen::Keyboard);

        // And in Polish (`walk_to_target_in_polish`): Europe, then Warsaw.
        let mut state = State::demo(true);
        state.draft.region.timezone = "Europe/Warsaw".into();
        state.go(Screen::Timezone);
        assert_eq!(
            state.views.timezone.trail.at[0],
            region_at(&state, "Europe")
        );
        state.press_apart(&[Action::Accept, Action::Accept], HARNESS_APART);
        assert_eq!(state.draft.region.timezone, "Europe/Warsaw");
        assert_eq!(state.screen, Screen::Keyboard);
    }

    #[test]
    fn coming_back_finds_the_places_shut_so_one_back_leaves_again() {
        let mut state = on_timezone();
        state.press_apart(&[Action::Accept, Action::Accept], HARNESS_APART);
        assert_eq!(state.screen, Screen::Keyboard);
        state.back();
        assert_eq!(state.screen, Screen::Timezone);
        assert!(!state.views.timezone.trail.open);
        act(&mut state, Action::Back, None);
        assert_eq!(state.screen, Screen::Language);
    }

    #[test]
    fn hovering_a_part_of_the_world_lights_it_and_a_click_opens_it() {
        let mut state = on_timezone();
        let europe = region_at(&state, "Europe");
        state.views.timezone.focus = Focus::Continue;
        hover(&mut state, IDS[0].row(europe));
        assert_eq!(state.views.timezone.trail.at[0], europe);
        assert_eq!(state.views.timezone.focus, Focus::Trail);
        assert!(
            !state.views.timezone.trail.open,
            "a hover opened the places"
        );
        // A row of a column not drawn yet, read off an older frame.
        hover(&mut state, IDS[1].row(0));
        assert!(!state.views.timezone.trail.open);
        hover(&mut state, spots::BUTTONS);
        assert_eq!(state.views.timezone.focus, Focus::Continue);
        // A click on a part of the world opens its places at once, wherever
        // the light was: what `pressed` does with `Click::Row`.
        let asia = region_at(&state, "Asia");
        assert!(point(&mut state, 0, asia));
        act(&mut state, Action::Accept, None);
        let trail = &state.views.timezone.trail;
        assert!(trail.open);
        assert_eq!(trail.at[0], asia);
        assert_eq!(state.screen, Screen::Timezone, "opening chose a place");
    }

    #[test]
    fn hovering_a_place_lights_it_and_a_click_takes_it() {
        let mut state = on_timezone();
        state.views.timezone.trail.at[0] = region_at(&state, "Europe");
        act(&mut state, Action::Accept, None);
        let right = right_rows(&state);
        let lisbon = right.iter().position(|row| row.name == "Lisbon").unwrap();
        hover(&mut state, IDS[1].row(lisbon));
        assert_eq!(state.views.timezone.trail.at[1], lisbon);
        // The part of the world it was opened from: a hover leaves it be.
        let (asia, europe) = (region_at(&state, "Asia"), region_at(&state, "Europe"));
        hover(&mut state, IDS[0].row(asia));
        hover(&mut state, IDS[0].row(europe));
        let trail = &state.views.timezone.trail;
        assert!(trail.open, "a hover on the trail closed the places");
        assert_eq!(trail.at, [europe, lisbon]);
        assert_eq!(
            state.draft.region.timezone, "America/New_York",
            "a hover chose"
        );
        assert!(point(&mut state, 1, lisbon));
        act(&mut state, Action::Accept, None);
        assert_eq!(state.draft.region.timezone, "Europe/Lisbon");
        assert_eq!(state.screen, Screen::Keyboard);
    }

    /// A click on row `index` of the column at the front, at `now`: what
    /// `pressed` does with `Click::Row`, through the frame's `State::click`.
    fn click_row(state: &mut State, now: f32, index: usize) {
        state.clock.tick(now);
        state.click(|state| {
            let front = state.views.timezone.trail.front();
            if point(state, front, index) {
                act(state, Action::Accept, None);
            }
        });
    }

    #[test]
    fn a_double_click_on_a_part_of_the_world_opens_it_and_chooses_nothing() {
        let mut state = on_timezone();
        let asia = region_at(&state, "Asia");
        // The first click opens the places; on a landscape panel they stand
        // where the parts of the world stood, so the second lands on one.
        click_row(&mut state, 10.0, asia);
        assert!(state.views.timezone.trail.open);
        let guessed = state.views.timezone.trail.at[1];
        click_row(&mut state, 10.2, guessed + 3);
        assert_eq!(state.screen, Screen::Timezone, "went on with a place");
        assert_eq!(state.draft.region.timezone, "America/New_York");
        assert_eq!(state.views.timezone.trail.at[1], guessed);
        // A click on a place once they have come in takes it.
        click_row(&mut state, 11.0, guessed + 3);
        assert_eq!(state.screen, Screen::Keyboard);
        assert_ne!(state.draft.region.timezone, "America/New_York");
    }

    #[test]
    fn universal_time_has_words_of_its_own() {
        let mut state = on_timezone();
        state.views.timezone.trail.at[0] = region_at(&state, "UTC");
        let right = right_rows(&state);
        assert_eq!(right.len(), 1);
        assert!(!right[0].name.is_empty() && right[0].name != "UTC");
    }

    #[test]
    fn past_the_places_the_light_goes_down_to_continue() {
        let mut state = on_timezone();
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.timezone.focus, Focus::Continue);
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.timezone.focus, Focus::Trail);
        assert!(
            state.views.timezone.trail.open,
            "the places closed under the light"
        );
    }

    fn typed(state: &mut State, text: &str) {
        state.edit(FieldId::Search, &Edit::Wrote(text.into()));
    }

    fn found_names(state: &State) -> Vec<String> {
        found_rows(state).into_iter().map(|row| row.name).collect()
    }

    /// A search shows the places it finds straight away, in one column,
    /// from every part of the world: by the country, by the city, by the
    /// part of the world in the person's language or the database's own.
    #[test]
    fn a_search_finds_places_across_the_parts_of_the_world() {
        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        assert_eq!(state.views.timezone.focus, Focus::Search);
        typed(&mut state, "united");
        let places = found_names(&state);
        for place in ["Chicago", "New York", "Honolulu", "Dubai"] {
            assert!(places.contains(&place.to_string()), "{place}: {places:?}");
        }
        assert!(!places.contains(&"Warsaw".to_string()));
        let regions: std::collections::BTreeSet<&str> = found(&state)
            .iter()
            .map(|zone| zone.region.as_str())
            .collect();
        assert!(regions.len() >= 3, "{regions:?}");
        // Under each, its country and its part of the world.
        let dubai = found_rows(&state)
            .into_iter()
            .find(|row| row.name == "Dubai")
            .unwrap();
        assert!(dubai.note.unwrap().contains("United Arab Emirates"));

        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "warsz");
        assert!(
            found_names(&state).is_empty(),
            "another language's spelling"
        );
        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "pol");
        assert_eq!(found_names(&state), ["Warsaw"], "by its country");
        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "pacific");
        assert_eq!(found_names(&state), ["Auckland", "Honolulu"]);
        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "buenos");
        let rows = found_rows(&state);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].note.as_deref().unwrap().starts_with("Argentina"));
    }

    /// Names of a test's own, written as `data/places/names.txt` is: the demo's
    /// places in Europe and two more, in an order of the table's own (not
    /// the alphabet's), and three countries.
    const POLISH: &str = "\
[zones]
America/Argentina/Buenos_Aires
America/New_York
Europe/Berlin
Europe/Lisbon
Europe/London
Europe/Madrid
Europe/Moscow
Europe/Paris
Europe/Rome
Europe/Vienna
Europe/Warsaw
[en]
0
1
AR\tArgentina
PL\tPoland
US\tUnited States
[pl_PL]
10\tWarszawa
9\tWiedeń
2
3\tLizbona
4\tLondyn
5\tMadryt
6\tMoskwa
7\tParyż
8\tRzym
0
1\tNowy Jork
AR\tArgentyna
PL\tPolska
US\tStany Zjednoczone
";

    fn in_polish() -> State {
        let mut state = on_timezone();
        state.draft.region.locale = "pl_PL.UTF-8".into();
        state.name_the_places_with(&crate::model::places::Names::read(POLISH, "pl_PL.UTF-8"));
        state
    }

    /// In the language chosen the places are said and sorted as it says and
    /// sorts them, a place inside a country with that country under it.
    #[test]
    fn the_places_are_said_and_sorted_in_the_language_chosen() {
        let mut state = in_polish();
        state.views.timezone.trail.at[0] = region_at(&state, "Europe");
        let names: Vec<String> = right_rows(&state).into_iter().map(|row| row.name).collect();
        assert_eq!(
            names,
            [
                "Warszawa", "Wiedeń", "Berlin", "Lizbona", "Londyn", "Madryt", "Moskwa", "Paryż",
                "Rzym"
            ]
        );
        state.views.timezone.trail.at[0] = region_at(&state, "America");
        let rows = right_rows(&state);
        let new_york = rows.iter().find(|row| row.name == "Nowy Jork").unwrap();
        assert_eq!(new_york.note, None);
        let buenos_aires = rows.iter().find(|row| row.name == "Buenos Aires").unwrap();
        assert_eq!(buenos_aires.note.as_deref(), Some("Argentyna"));
        // A place the names do not know keeps the database's city, after
        // those they do.
        assert_eq!(rows.last().map(|row| row.name.as_str()), Some("Toronto"));
        // Choosing one takes the zone, whatever it is called.
        state.views.timezone.trail.at[0] = region_at(&state, "Europe");
        act(&mut state, Action::Accept, None);
        state.views.timezone.trail.at[1] = 0;
        act(&mut state, Action::Accept, None);
        assert_eq!(state.draft.region.timezone, "Europe/Warsaw");
    }

    /// The search finds a place by its name and its country in the
    /// language chosen and in the database's English, and says its country
    /// in the language chosen under it.
    #[test]
    fn a_search_finds_a_place_in_the_language_chosen_and_in_english() {
        for query in ["warszawa", "Warsaw", "polska", "poland", "warsz"] {
            let mut state = in_polish();
            act(&mut state, Action::Menu, None);
            typed(&mut state, query);
            assert_eq!(found_names(&state), ["Warszawa"], "{query}");
            let row = &found_rows(&state)[0];
            assert!(
                row.note.as_deref().unwrap().starts_with("Polska"),
                "{row:?}"
            );
        }
        let mut state = in_polish();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "nowy jork");
        assert_eq!(found_names(&state), ["Nowy Jork"]);
        let mut state = in_polish();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "stany");
        let found = found_names(&state);
        assert!(found.contains(&"Nowy Jork".to_string()), "{found:?}");
        assert!(found.contains(&"Chicago".to_string()), "{found:?}");
    }

    /// Enter lands the light on the first place found, and Accept takes
    /// it and goes on, as choosing a place in the columns does.
    #[test]
    fn accept_on_a_place_found_takes_it_and_goes_on() {
        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "lisbon");
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.timezone.focus, Focus::Trail);
        assert_eq!(state.views.timezone.found.at[0], 0);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.draft.region.timezone, "Europe/Lisbon");
        assert!(state.chose_zone);
        assert_eq!(state.screen, Screen::Keyboard);
        // Coming back: the search is put away, the columns as they were.
        state.back();
        assert!(!state.views.timezone.search.is_open());
    }

    /// Back empties the search — the columns come back standing on the
    /// place the light was on, its part of the world open on it — then
    /// closes it, then is the columns' and the screen's own again.
    #[test]
    fn back_empties_the_search_then_closes_it_and_the_columns_stand_on_the_place() {
        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "united");
        act(&mut state, Action::Down, None);
        let lit = found(&state)[state.views.timezone.found.at[0]].name.clone();
        act(&mut state, Action::Back, None);
        let view = &state.views.timezone;
        assert!(view.search.is_open() && !view.search.narrows());
        assert!(view.trail.open, "the place's part of the world is not open");
        assert_eq!(places(&state)[view.trail.at[1]].name, lit);
        act(&mut state, Action::Back, None);
        assert!(!state.views.timezone.search.is_open());
        act(&mut state, Action::Back, None);
        assert!(!state.views.timezone.trail.open, "Back closed the places");
        act(&mut state, Action::Back, None);
        assert_eq!(state.screen, Screen::Language);
    }

    /// Up from the first place found, or from the top of the parts of the
    /// world while the search is up, goes into the field; a search finding
    /// nothing keeps the light there.
    #[test]
    fn the_light_goes_up_into_the_search_field_and_stays_where_nothing_is_found() {
        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "a");
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.timezone.focus, Focus::Trail);
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.timezone.focus, Focus::Search);
        typed(&mut state, "qqq");
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.timezone.focus, Focus::Search);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Timezone, "nothing found was taken");
        // A pad brings a board up with the field.
        let mut state = on_timezone();
        state.pad = true;
        act(&mut state, Action::Menu, None);
        assert_eq!(
            state.board.as_ref().map(|board| board.field),
            Some(FieldId::Search)
        );
    }

    #[test]
    fn past_the_foot_of_the_parts_of_the_world_the_light_goes_to_continue() {
        let mut state = on_timezone();
        let last = regions(&state.zones).len() - 1;
        state.views.timezone.trail.at[0] = last;
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.timezone.focus, Focus::Continue);
        assert!(!state.views.timezone.trail.open);
    }

    /// Arrived at Time zone, the guessed zone's places not seen, and the
    /// light gone down to Continue, as a key or the pointer takes it there.
    fn on_continue() -> State {
        let mut state = on_timezone();
        assert_eq!(state.views.timezone.seen, Seen::Not, "seen before shown");
        state.views.timezone.focus = Focus::Continue;
        state
    }

    /// What Continue's first press leaves while the zone in force had not
    /// been seen: its places open beside its part of the world, the zone on
    /// the line and marked, heard as the opening is, the light still on
    /// Continue and the screen not left, nothing chosen — and the zone's
    /// dwell begun, not over.
    fn showing(state: &State, zone: &str, how: &str) {
        let view = &state.views.timezone;
        assert_eq!(state.screen, Screen::Timezone, "{how} went on unseen");
        assert!(view.trail.open, "{how} did not open the places");
        assert_eq!(view.focus, Focus::Continue, "{how} took the light off");
        assert_eq!(places(state)[view.trail.at[1]].name, zone, "{how}");
        assert!(right_rows(state)[view.trail.at[1]].chosen, "{how}");
        assert_eq!(state.draft.region.timezone, zone, "{how} chose a zone");
        assert!(!state.chose_zone, "{how} chose a zone");
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

    /// Every way of pressing Continue: Accept on it (Enter, South), the
    /// legend's Choose pressed by the pointer, and a click on the button —
    /// what `pressed` does with it.
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

    /// The second press, once the places have stood on the screen for their
    /// dwell, goes on.
    #[test]
    fn continue_first_shows_the_places_of_the_zone_in_force_then_goes_on() {
        for (how, presses, clicks) in PRESSES {
            let mut state = on_continue();
            state.frame_at(10.0, presses, clicks);
            showing(&state, "America/New_York", how);
            let later = 10.0 + trail::OPENING + 0.1;
            state.frames_until(later);
            assert_eq!(state.views.timezone.seen, Seen::Yes, "{how}");
            state.frame_at(later, presses, clicks);
            assert_eq!(state.screen, Screen::Keyboard, "{how}, pressed again");
            assert_eq!(state.draft.region.timezone, "America/New_York");
        }
        // Start goes nowhere from here, as it never did.
        let mut state = on_continue();
        state.act(Action::Submit, None);
        assert_eq!(state.screen, Screen::Timezone);
        assert!(!state.views.timezone.trail.open);
    }

    /// Two presses of Continue that reach one frame — Enter twice, South
    /// twice, Enter and a click on Continue or on the legend's Choose — are
    /// answered at one instant, before the frame draws the places the first
    /// opened: the second does nothing, and is not heard. So it is in
    /// low-end mode, where the places come in at once.
    #[test]
    fn two_presses_of_continue_in_one_frame_only_show_the_zone_in_force() {
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
        for low_end in [false, true] {
            for (how, pad, presses, clicks) in both {
                let how = format!("{how}, low-end {low_end}");
                let mut state = on_continue();
                if low_end {
                    state.pretend_low_end();
                }
                state.pad = pad;
                state.frame_at(10.0, presses, clicks);
                showing(&state, "America/New_York", &how);
                let heard = state.heard().iter().filter(|one| **one == Sound::Press);
                assert_eq!(heard.count(), 1, "{how}: the second press was heard");
            }
        }
    }

    /// A key 50 ms after a click on Continue — the places three frames into
    /// coming in — does nothing.
    #[test]
    fn a_key_soon_after_a_click_on_continue_waits_for_the_places() {
        let mut state = on_continue();
        state.frame_at(10.0, &[], &[onward]);
        showing(&state, "America/New_York", "a click");
        state.frames_until(10.05);
        state.frame_at(10.05, &[Action::Accept], &[]);
        showing(&state, "America/New_York", "Enter 50 ms after a click");
    }

    /// The places are counted seen by the frames that draw them, never by a
    /// press: from the second frame that drew them (by which the first had
    /// reached the screen), the dwell later. A press in the very frame drawn
    /// the dwell after it does nothing, and is not heard — it is answered
    /// before that frame counts them, and may have come in before — and one
    /// in the frame after goes on. The dwell is the places' opening, and in
    /// low-end mode, where they come in at once, 0.3 s.
    #[test]
    fn a_press_before_a_frame_has_counted_the_dwell_is_let_go_and_one_after_goes_on() {
        for low_end in [false, true] {
            let mut state = on_continue();
            if low_end {
                state.pretend_low_end();
            }
            state.frame_at(10.0, &[Action::Accept], &[]);
            let heard = state.heard().len();
            let counted = 10.0 + FRAME + trail::dwell(low_end) + 0.001;
            state.frames_until(counted - 0.01);
            state.frame_at(counted, &[Action::Accept], &[]);
            assert_eq!(state.screen, Screen::Timezone, "low-end {low_end}");
            assert_eq!(state.views.timezone.focus, Focus::Continue);
            assert_eq!(state.heard().len(), heard, "low-end {low_end}: heard");
            // Counted by that frame, after its press.
            assert_eq!(state.views.timezone.seen, Seen::Yes, "low-end {low_end}");
            state.frame_at(counted + FRAME, &[Action::Accept], &[]);
            assert_eq!(state.screen, Screen::Keyboard, "low-end {low_end}");
        }
    }

    /// Frames drawn far apart — the window stalled, or slow to draw the
    /// places' first frame — never let Continue go on sooner than frames
    /// drawn as often as the window draws them. Pressed in a frame a dwell
    /// after the one that opened them, nothing between, Continue does
    /// nothing: they were drawn once, a sixtieth of the way in, and that
    /// frame may not even have reached the screen. Pressed again in every
    /// frame, each half a second after the last, it goes on only once frames
    /// have carried the places in and kept them there: at the eighth press in
    /// all (the fourth in low-end mode, where they stand whole at once).
    #[test]
    fn frames_far_apart_do_not_let_continue_go_on_sooner() {
        for (low_end, pressed) in [(false, 8), (true, 4)] {
            let mut state = on_continue();
            if low_end {
                state.pretend_low_end();
            }
            state.frame_at(10.0, &[Action::Accept], &[]);
            let late = 10.0 + trail::dwell(low_end) + 0.01;
            state.frame_at(late, &[Action::Accept], &[]);
            showing(&state, "America/New_York", "a dwell later, nothing between");
            let heard = state.heard().iter().filter(|one| **one == Sound::Press);
            assert_eq!(heard.count(), 1, "low-end {low_end}: the press was heard");
            let mut presses = 2;
            while state.screen == Screen::Timezone {
                state.frame_at(state.clock.now + 0.5, &[Action::Accept], &[]);
                presses += 1;
                assert!(presses <= pressed, "low-end {low_end}: still held");
            }
            assert_eq!(presses, pressed, "low-end {low_end}");
            assert_eq!(state.draft.region.timezone, "America/New_York");
        }
    }

    /// Enter on the guessed part of the world and Enter again in one frame:
    /// the places open on the guess and nothing is taken — the second press
    /// came before they were drawn — nor by a press or a click on a place
    /// while they come in; once they have stood there for the dwell, Enter
    /// takes the guess. Only the places of the zone in force wait so: Europe
    /// opened and a place in it taken in one frame is a choice of the
    /// person's own, as it was.
    #[test]
    fn the_guessed_place_is_not_taken_before_its_places_have_been_seen() {
        for low_end in [false, true] {
            let mut state = on_timezone();
            if low_end {
                state.pretend_low_end();
            }
            state.frame_at(10.0, &[Action::Accept, Action::Accept], &[]);
            let view = &state.views.timezone;
            assert!(view.trail.open);
            assert_eq!(places(&state)[view.trail.at[1]].name, "America/New_York");
            assert_eq!(state.screen, Screen::Timezone, "low-end {low_end}");
            assert!(!state.chose_zone);
            state.frames_until(10.2);
            state.frame_at(10.2, &[Action::Accept], &[]);
            let at = state.views.timezone.trail.at[1];
            click_row(&mut state, 10.25, at);
            assert!(!state.chose_zone, "low-end {low_end}: taken coming in");
            let after = 10.0 + trail::dwell(low_end) + 0.1;
            state.frames_until(after);
            state.frame_at(after, &[Action::Accept], &[]);
            assert_eq!(state.screen, Screen::Keyboard);
            assert!(state.chose_zone);
            assert_eq!(state.draft.region.timezone, "America/New_York");
        }
        let mut state = on_timezone();
        state.views.timezone.trail.at[0] = region_at(&state, "Europe");
        state.frame_at(10.0, &[Action::Accept, Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Keyboard);
        assert!(state.draft.region.timezone.starts_with("Europe/"));
    }

    /// Back from Continue while the places it showed were still coming in,
    /// and Time zone arrived at again later: what had begun is over. Presses
    /// that come in the frame that arrives — Enter on the language, Down past
    /// the parts of the world to Continue, Enter — find the places shut and
    /// unseen, and Continue shows them.
    #[test]
    fn a_dwell_left_unfinished_is_over_when_the_screen_is_arrived_at_again() {
        let mut state = on_continue();
        state.frame_at(10.0, &[Action::Accept], &[]);
        showing(&state, "America/New_York", "Continue");
        state.frames_until(10.1);
        state.frame_at(10.1, &[Action::Back], &[]);
        assert_eq!(state.screen, Screen::Language);
        state.frames_until(12.0);
        let downs = regions(&state.zones).len() - region_at(&state, "America");
        let mut presses = vec![Action::Accept];
        presses.extend(std::iter::repeat_n(Action::Down, downs));
        presses.push(Action::Accept);
        state.frame_at(12.0, &presses, &[]);
        showing(&state, "America/New_York", "Continue on arriving again");
    }

    /// A way the columns may stand, the zone in force not seen, when the
    /// light goes to Continue, and its name.
    type Before = (&'static str, fn(&mut State));

    /// A double click on Continue — the button, or the legend's Choose — shows
    /// the places of the zone in force and goes no further, however the
    /// columns stood: shut, the first click opens them (and `State::click`
    /// lets the second go); open on another part of the world, on a search
    /// narrowing them or on another place Back brought out of a search, it
    /// rolls them over to the zone in force, the column open before and
    /// after, and the second click comes within the places' dwell. A click
    /// once they have stood there goes on.
    #[test]
    fn a_double_click_on_continue_shows_the_places_and_goes_no_further() {
        let befores: [Before; 4] = [
            ("the places shut", |_| {}),
            ("Europe's places open", |state| {
                let europe = region_at(state, "Europe");
                state.views.timezone.trail.at[0] = europe;
                act(state, Action::Accept, None);
            }),
            ("a search narrowing over Europe's places", |state| {
                let europe = region_at(state, "Europe");
                state.views.timezone.trail.at[0] = europe;
                act(state, Action::Accept, None);
                act(state, Action::Menu, None);
                typed(state, "united");
                act(state, Action::Right, None);
            }),
            ("Chicago open after Back from a search", |state| {
                act(state, Action::Menu, None);
                typed(state, "chicago");
                act(state, Action::Down, None);
                act(state, Action::Back, None);
                let at = state.views.timezone.trail.at[1];
                assert_eq!(places(state)[at].name, "America/Chicago");
            }),
        ];
        let clicks: [Clicked; 2] = [
            ("Continue", onward),
            ("the legend's Choose", choose_in_the_legend),
        ];
        for (before, stand) in befores {
            for (on, click) in clicks {
                let how = format!("a double click on {on}, {before}");
                let mut state = on_timezone();
                stand(&mut state);
                let view = &state.views.timezone;
                assert_eq!(view.trail.open, !before.ends_with("shut"), "{how}");
                assert_eq!(view.seen, Seen::Not, "{how}: seen already");
                state.views.timezone.focus = Focus::Continue;
                state.frame_at(10.0, &[], &[click]);
                state.frames_until(10.2);
                state.frame_at(10.2, &[], &[click]);
                showing(&state, "America/New_York", &how);
                state.frames_until(11.0);
                state.frame_at(11.0, &[], &[click]);
                assert_eq!(state.screen, Screen::Keyboard, "{how}, then a click");
                assert_eq!(state.draft.region.timezone, "America/New_York");
            }
        }
    }

    /// The light walked off the guessed part of the world, or another part
    /// of the world opened, before Continue: Continue shows the zone in force
    /// all the same, its own part of the world back in the first column.
    #[test]
    fn continue_shows_the_zone_in_force_wherever_the_columns_stood() {
        let america = region_at(&on_timezone(), "America");
        let mut state = on_timezone();
        act(&mut state, Action::Down, None);
        assert_ne!(state.views.timezone.trail.at[0], america);
        state.views.timezone.focus = Focus::Continue;
        act(&mut state, Action::Accept, None);
        showing(&state, "America/New_York", "from another part of the world");
        assert_eq!(state.views.timezone.trail.at[0], america);

        // Europe opened, which does not hold New York: not seen yet.
        let mut state = on_timezone();
        state.views.timezone.trail.at[0] = region_at(&state, "Europe");
        act(&mut state, Action::Accept, None);
        assert!(state.views.timezone.trail.open);
        assert_eq!(state.views.timezone.seen, Seen::Not, "another part");
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.timezone.focus, Focus::Continue);
        state.frame_at(10.0, &[Action::Accept], &[]);
        showing(&state, "America/New_York", "from Europe's places");
        assert_eq!(state.views.timezone.trail.at[0], america);
        state.frames_until(11.0);
        state.frame_at(11.0, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Keyboard);
    }

    /// Opened by the person and left on the screen for their dwell, the
    /// places of the zone in force are seen: Continue goes on at its first
    /// press, even once they are shut again. Shut before the dwell was over,
    /// they were not: Continue's first press shows them.
    #[test]
    fn places_opened_by_the_person_and_left_there_let_continue_go_on_at_once() {
        let mut state = on_timezone();
        state.frame_at(10.0, &[Action::Accept], &[]);
        let seen = state.views.timezone.seen;
        assert!(matches!(seen, Seen::Drawn { since: None, .. }), "{seen:?}");
        state.frames_until(10.5);
        state.frame_at(10.5, &[Action::Right], &[]);
        assert_eq!(state.views.timezone.seen, Seen::Yes);
        assert_eq!(state.views.timezone.focus, Focus::Continue);
        state.frame_at(10.6, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Keyboard);

        let mut state = on_timezone();
        state.frame_at(10.0, &[Action::Right], &[]);
        state.frames_until(10.5);
        state.frame_at(10.5, &[Action::Back], &[]);
        assert!(!state.views.timezone.trail.open);
        state.views.timezone.focus = Focus::Continue;
        state.frame_at(10.6, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Keyboard, "shut again, still seen");

        let mut state = on_timezone();
        state.frame_at(10.0, &[Action::Right], &[]);
        state.frames_until(10.1);
        state.frame_at(10.1, &[Action::Back], &[]);
        assert_eq!(state.views.timezone.seen, Seen::Not, "shut at once");
        state.views.timezone.focus = Focus::Continue;
        state.frame_at(10.2, &[Action::Accept], &[]);
        showing(&state, "America/New_York", "after the places were shut");
    }

    /// A place taken from what a search found was seen as it was taken:
    /// back on Time zone, Continue goes on at once.
    #[test]
    fn a_place_taken_from_a_search_is_seen() {
        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "lisbon");
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Keyboard);
        state.back();
        assert_eq!(state.views.timezone.seen, Seen::Yes);
        state.views.timezone.focus = Focus::Continue;
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Keyboard);
        assert_eq!(state.draft.region.timezone, "Europe/Lisbon");
    }

    /// Continue while a search narrows the columns: the search is emptied,
    /// its field left up, and the columns come back open on the zone in
    /// force. The columns coming back open on another place as Back empties
    /// a search did not show it.
    #[test]
    fn continue_while_a_search_narrows_empties_it_to_show_the_zone_in_force() {
        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "united");
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.timezone.focus, Focus::Continue);
        state.frame_at(10.0, &[Action::Accept], &[]);
        let search = &state.views.timezone.search;
        assert!(search.is_open() && !search.narrows(), "{search:?}");
        showing(&state, "America/New_York", "Continue over a search");
        state.frames_until(11.0);
        state.frame_at(11.0, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Keyboard);

        let mut state = on_timezone();
        act(&mut state, Action::Menu, None);
        typed(&mut state, "chicago");
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Back, None);
        let view = &state.views.timezone;
        assert!(view.trail.open);
        assert_eq!(places(&state)[view.trail.at[1]].name, "America/Chicago");
        assert_eq!(view.seen, Seen::Not, "New York was rows away");
    }

    /// Seen is kept for the installation: Back from Keyboard finds Continue
    /// going on at once.
    #[test]
    fn seen_survives_back_from_keyboard() {
        let mut state = on_continue();
        state.press_apart(&[Action::Accept, Action::Accept], 1.0);
        assert_eq!(state.screen, Screen::Keyboard);
        state.act(Action::Back, None);
        assert_eq!(state.screen, Screen::Timezone);
        assert!(!state.views.timezone.trail.open, "came back to them open");
        state.views.timezone.focus = Focus::Continue;
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Keyboard);
    }

    /// Continue that goes on at the first frame it can — pressed in every
    /// frame, sixty a second, or in frames half a second apart — leaves the
    /// zone seen: Back from Keyboard finds Continue going on at once.
    #[test]
    fn seen_survives_back_when_continue_went_on_at_the_first_frame_it_could() {
        for apart in [FRAME, 0.5] {
            for low_end in [false, true] {
                let how = format!("{apart} s apart, low-end {low_end}");
                let mut state = on_continue();
                if low_end {
                    state.pretend_low_end();
                }
                state.frame_at(10.0, &[Action::Accept], &[]);
                while state.screen == Screen::Timezone {
                    state.frame_at(state.clock.now + apart, &[Action::Accept], &[]);
                }
                assert_eq!(state.screen, Screen::Keyboard, "{how}");
                assert_eq!(state.views.timezone.seen, Seen::Yes, "{how}");
                state.frame_at(state.clock.now + FRAME, &[Action::Back], &[]);
                assert_eq!(state.screen, Screen::Timezone, "{how}");
                state.views.timezone.focus = Focus::Continue;
                state.frame_at(state.clock.now + FRAME, &[Action::Accept], &[]);
                assert_eq!(state.screen, Screen::Keyboard, "{how}, came back");
            }
        }
    }

    /// The places standing open with the zone in force on the line count
    /// however the light came onto it, not only by their opening: walked
    /// there from another place Back brought out of a search, or left there
    /// as a search over them is emptied. Kept so for the dwell, Continue goes
    /// on at its first press; and Enter on the place the light was walked to
    /// takes it at once.
    #[test]
    fn the_zone_in_force_on_the_line_is_seen_however_the_light_came_to_it() {
        // Back from a search on Chicago: New York rows away, nothing begins
        // however long the places stand there.
        let walked_to_it = |state: &mut State| {
            act(state, Action::Menu, None);
            typed(state, "chicago");
            act(state, Action::Down, None);
            state.frame_at(10.0, &[Action::Back], &[]);
            state.frames_until(12.0);
            assert_eq!(state.views.timezone.seen, Seen::Not, "Chicago lit");
            let view = &state.views.timezone;
            let new_york = places(state)
                .iter()
                .position(|zone| zone.name == "America/New_York")
                .unwrap();
            let step = if new_york > view.trail.at[1] {
                Action::Down
            } else {
                Action::Up
            };
            let steps = new_york.abs_diff(view.trail.at[1]);
            state.press_apart(&vec![step; steps], 0.3);
            assert_eq!(super::showing(state), Showing::Row);
        };
        let mut state = on_timezone();
        walked_to_it(&mut state);
        state.frames_until(state.clock.now + 3.0);
        assert_eq!(state.views.timezone.seen, Seen::Yes);
        let heard = state.heard().len();
        state.press_apart(&[Action::Right, Action::Accept], 0.2);
        assert_eq!(state.screen, Screen::Keyboard);
        let pressed = state.heard()[heard..]
            .iter()
            .filter(|one| **one == Sound::Press);
        assert_eq!(pressed.count(), 1, "the places shown again");

        // Walked onto it, Enter takes it at once.
        let mut state = on_timezone();
        walked_to_it(&mut state);
        state.frame_at(state.clock.now + FRAME, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Keyboard);
        assert!(state.chose_zone);
        assert_eq!(state.draft.region.timezone, "America/New_York");

        // The places opened on it, a search typed over them at once and
        // emptied again: they stand on it, and count from there.
        let mut state = on_timezone();
        state.frame_at(10.0, &[Action::Accept], &[]);
        state.frames_until(10.2);
        state.frame_at(10.2, &[Action::Menu], &[]);
        typed(&mut state, "n");
        state.frames_until(11.0);
        assert_eq!(state.views.timezone.seen, Seen::Not, "the search over it");
        state.press_apart(&[Action::Back, Action::Back], 0.3);
        assert!(!state.views.timezone.search.is_open());
        state.frames_until(15.0);
        assert_eq!(state.views.timezone.seen, Seen::Yes);
        state.frame_at(15.0, &[Action::Right], &[]);
        state.frames_until(15.2);
        state.frame_at(15.2, &[Action::Accept], &[]);
        assert_eq!(state.screen, Screen::Keyboard);
    }

    /// While the places of the zone in force come in, a place the person
    /// walked the light to is theirs to take: Enter on the part of the
    /// world, Down, Enter takes the place after New York, a quarter of a
    /// second later or in one frame, and so does a click on it. Only the
    /// guess, the light where the opening put it, waits.
    #[test]
    fn a_place_the_light_was_walked_to_is_taken_while_the_places_come_in() {
        let next_after = |state: &State| {
            let places = places(state);
            let new_york = places
                .iter()
                .position(|zone| zone.name == "America/New_York")
                .unwrap();
            places[new_york + 1].name.clone()
        };
        for low_end in [false, true] {
            let how = format!("low-end {low_end}");
            let mut state = on_timezone();
            if low_end {
                state.pretend_low_end();
            }
            state.frame_at(10.0, &[Action::Accept], &[]);
            let after = next_after(&state);
            state.frame_at(10.12, &[Action::Down], &[]);
            state.frame_at(10.25, &[Action::Accept], &[]);
            assert_eq!(state.screen, Screen::Keyboard, "{how}");
            assert_eq!(state.draft.region.timezone, after, "{how}");

            let mut state = on_timezone();
            if low_end {
                state.pretend_low_end();
            }
            state.frame_at(10.0, &[Action::Accept, Action::Down, Action::Accept], &[]);
            assert_eq!(state.draft.region.timezone, after, "{how}, one frame");

            let mut state = on_timezone();
            if low_end {
                state.pretend_low_end();
            }
            state.frame_at(10.0, &[Action::Accept], &[]);
            let at = state.views.timezone.trail.at[1] + 1;
            click_row(&mut state, 10.25, at);
            assert_eq!(state.draft.region.timezone, after, "{how}, a click");
        }
    }

    /// Enter on the guessed part of the world, Down, Up and Enter, all in one
    /// frame: the light is back on the guess before any frame drew the
    /// places, which is no walk of the person's (`Trail::walked`), and
    /// nothing is taken. Down and Up once a frame has drawn them are: Enter
    /// then takes the guess.
    #[test]
    fn steps_before_a_frame_drew_the_places_are_no_walk_to_the_guess() {
        for low_end in [false, true] {
            let how = format!("low-end {low_end}");
            let mut state = on_timezone();
            if low_end {
                state.pretend_low_end();
            }
            let presses = [Action::Accept, Action::Down, Action::Up, Action::Accept];
            state.frame_at(10.0, &presses, &[]);
            assert_eq!(state.screen, Screen::Timezone, "{how}: taken never drawn");
            assert!(!state.chose_zone, "{how}");
            let view = &state.views.timezone;
            assert_eq!(places(&state)[view.trail.at[1]].name, "America/New_York");
            assert!(!view.trail.walked(), "{how}");
            state.frame_at(10.0 + FRAME, &presses[1..], &[]);
            assert_eq!(state.screen, Screen::Keyboard, "{how}, walked");
            assert!(state.chose_zone, "{how}");
            assert_eq!(state.draft.region.timezone, "America/New_York");
        }
    }

    /// Enter on the guessed part of the world and Down three times in one
    /// frame: the places open on the guess and the light steps off it before
    /// a frame drew them, the guess perhaps out of sight. Nothing begins,
    /// however long they stand there so: Continue then rolls them back onto
    /// the guess — nothing holds it — and goes on once they have stood there
    /// for the dwell.
    #[test]
    fn places_first_drawn_off_the_guess_do_not_count_it_seen() {
        for low_end in [false, true] {
            let how = format!("low-end {low_end}");
            let mut state = on_timezone();
            if low_end {
                state.pretend_low_end();
            }
            let presses = [Action::Accept, Action::Down, Action::Down, Action::Down];
            state.frame_at(10.0, &presses, &[]);
            assert_eq!(super::showing(&state), Showing::Column, "{how}");
            state.frames_until(12.0);
            assert_eq!(state.views.timezone.seen, Seen::Not, "{how}");
            state.frame_at(12.0, &[Action::Right], &[]);
            assert_eq!(state.views.timezone.focus, Focus::Continue, "{how}");
            state.frame_at(12.1, &[Action::Accept], &[]);
            showing(&state, "America/New_York", &how);
            state.frames_until(13.0);
            state.frame_at(13.0, &[Action::Accept], &[]);
            assert_eq!(state.screen, Screen::Keyboard, "{how}");
            assert_eq!(state.draft.region.timezone, "America/New_York");
        }
    }

    /// Continue clicked in every frame once a key has opened the places on
    /// it — the button, or the legend's Choose. A click is answered after its
    /// frame is drawn, but came in before that frame reached the display: the
    /// one in the frame that counts the places seen does nothing, and only a
    /// click in a frame after one that counted them goes on.
    #[test]
    fn a_click_in_the_frame_that_counts_the_places_seen_does_not_go_on() {
        let clicks: [Clicked; 2] = [
            ("Continue", onward),
            ("the legend's Choose", choose_in_the_legend),
        ];
        for low_end in [false, true] {
            for (on, click) in clicks {
                let how = format!("a click on {on}, low-end {low_end}");
                let mut state = on_continue();
                if low_end {
                    state.pretend_low_end();
                }
                state.frame_at(10.0, &[Action::Accept], &[]);
                showing(&state, "America/New_York", &how);
                while state.screen == Screen::Timezone {
                    assert!(state.clock.now < 12.0, "{how}: never went on");
                    let before = state.views.timezone.seen;
                    state.frame_at(state.clock.now + FRAME, &[], &[click]);
                    if state.screen != Screen::Timezone {
                        assert_eq!(before, Seen::Yes, "{how}: went on as it was counted");
                    }
                }
                assert_eq!(state.draft.region.timezone, "America/New_York");
            }
        }
    }

    /// Choosing a language guesses the zone again (`guess_the_rest`, which
    /// is what choosing one does): one that guesses another zone puts it out
    /// of sight, so Continue shows it first again; one that guesses the same
    /// zone, or any once a place was chosen, leaves it seen.
    #[test]
    fn a_language_guessing_another_zone_has_it_seen_again() {
        let guessed = |state: &mut State, locale: &str| {
            state.draft.region.locale = locale.into();
            crate::ui::screens::language::guess_the_rest(state);
        };
        let mut state = on_continue();
        state.press_apart(&[Action::Accept, Action::Accept], 1.0);
        assert_eq!(state.views.timezone.seen, Seen::Yes);
        state.go(Screen::Language);
        guessed(&mut state, "en_US.UTF-8");
        assert_eq!(state.draft.region.timezone, "America/New_York");
        assert_eq!(state.views.timezone.seen, Seen::Yes, "the same zone");
        guessed(&mut state, "pl_PL.UTF-8");
        assert_eq!(state.draft.region.timezone, "Europe/Warsaw");
        assert_eq!(state.views.timezone.seen, Seen::Not, "Warsaw unshown");
        state.go(Screen::Timezone);
        state.views.timezone.focus = Focus::Continue;
        act(&mut state, Action::Accept, None);
        showing(&state, "Europe/Warsaw", "after Polski");
        assert_eq!(
            state.views.timezone.trail.at[0],
            region_at(&state, "Europe")
        );

        // A place chosen is never guessed over, and stays seen.
        let mut state = on_timezone();
        state.views.timezone.trail.at[0] = region_at(&state, "Europe");
        act(&mut state, Action::Accept, None);
        state.views.timezone.trail.at[1] = 0;
        act(&mut state, Action::Accept, None);
        assert!(state.chose_zone);
        assert_eq!(state.views.timezone.seen, Seen::Yes);
        let chosen = state.draft.region.timezone.clone();
        guessed(&mut state, "en_US.UTF-8");
        assert_eq!(state.draft.region.timezone, chosen);
        assert_eq!(state.views.timezone.seen, Seen::Yes);
    }
}
