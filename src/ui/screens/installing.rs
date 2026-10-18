//! Installing: the bar, the step it is on, and "Step 4 of 12 · 40%" — the
//! console rule's "a bar with a number and the thing it is on" — with the
//! worker's own lines behind Options, as Details.
//!
//! Nothing here can be pressed away: an installation is not stopped halfway
//! from the interface, so Back does nothing and the legend says nothing about
//! it. The frame keeps the machine awake and the bar drawn while this screen
//! is up, and moves to Done or Stopped when the work ends.

use lxb_app::lxb_toolkit::{input::Action, metrics::Metric, palette::Role};
use lxb_app::{Page, Scroll};

use super::{Head, Room};
use crate::i18n;
use crate::ui::app::State;
use crate::ui::legend::{hint, Button as Pictured, Hint};
use crate::ui::log::Log;
use crate::ui::panel::Note;
use crate::ui::progress;
use crate::ui::words;

/// What this screen keeps between frames.
#[derive(Debug, Default, Clone)]
pub struct View {
    /// Whether Details is open.
    pub details: bool,
    pub log: Log,
}

impl View {
    pub fn settle(&mut self) {
        self.log.settle();
    }
}

pub fn enter(state: &mut State) {
    state.views.installing = View::default();
}

/// The picture of Details: the installation under way with its lines open.
pub fn stage(state: &mut State) {
    state.views.installing.details = true;
}

/// What the work is on now, in words.
fn on(state: &State) -> &'static str {
    state
        .install
        .current
        .map_or(i18n::text("installing-waiting"), words::step)
}

/// The step is said once, over the bar; the head says what the person needs
/// to know about the wait.
pub fn head(_state: &State) -> Head {
    Head::new(
        "refresh",
        i18n::text("installing-title"),
        i18n::text("installing-caption"),
    )
}

/// One verb, on Options: Details, or — while they are open — hiding them.
pub fn hints(state: &State) -> Vec<Hint> {
    let label = if state.views.installing.details {
        i18n::text("hint-hide-details")
    } else {
        i18n::text("hint-details")
    };
    vec![hint(label, Pictured::Options)]
}

pub fn note(_state: &State) -> Option<Note> {
    Some(Note {
        text: i18n::text("installing-note").to_string(),
        role: Role::Text,
    })
}

/// Options opens and closes Details; Up and Down (or a wheel) scroll them
/// while they are open. Back does nothing: there is no way out of an
/// installation from here.
pub fn act(state: &mut State, action: Action, _scroll: Option<Scroll>) {
    let view = &mut state.views.installing;
    match action {
        Action::Menu => {
            view.details = !view.details;
            view.log.follow = true;
            state.sound(lxb_app::lxb_toolkit::sound::Sound::Press);
        }
        Action::Up if view.details => view.log.step(-1.0),
        Action::Down if view.details => view.log.step(1.0),
        _ => {}
    }
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, _lit: bool) {
    let content = room.content;
    let (step, steps) = state.install.place();
    let through = state.progress.through();
    let (step, steps, percent) = (step as i64, steps as i64, progress::percent(through) as i64);
    // On one line: the catalog test finds the messages a program asks for by
    // reading the macro's first argument where the macro is called.
    let count =
        crate::message!("installing-count", "step" => step, "steps" => steps, "percent" => percent);
    // The longest the count can be, which decides where it stands.
    let widest =
        crate::message!("installing-count", "step" => steps, "steps" => steps, "percent" => 100);
    let width = content[2].min(page.scaled(960.0));
    let tall = progress::height(page, width, &widest);
    let details = state.views.installing.details;
    // The bar stands in the middle of the room, a little above it — where
    // the eye rests on a panel — and no wider than a bar should be; with
    // Details open it goes up to the top, and the lines fill the rest.
    let y = if details {
        content[1]
    } else {
        content[1] + (content[3] - tall) * 0.42
    };
    let rect = [content[0] + (content[2] - width) * 0.5, y, width, tall];
    progress::draw(page, rect, through, on(state), &count, &widest);
    if details {
        let gap = page.metric(Metric::Gap);
        let top = rect[1] + tall + gap * 1.6;
        let area = [
            content[0],
            top,
            content[2],
            (content[1] + content[3] - top).max(1.0),
        ];
        let dt = state.clock.dt;
        let lines = &state.install.log;
        state.views.installing.log.draw(page, area, &[], lines, dt);
    }
}

pub fn pressed(_state: &mut State, _page: &mut Page) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::app::Screen;

    #[test]
    fn options_opens_details_and_back_leaves_nothing() {
        let mut state = State::demo(true);
        state.stage("installing").unwrap();
        assert!(!state.views.installing.details);
        act(&mut state, Action::Menu, None);
        assert!(state.views.installing.details);
        act(&mut state, Action::Back, None);
        assert_eq!(
            state.screen,
            Screen::Installing,
            "Back left an installation"
        );
        act(&mut state, Action::Menu, None);
        assert!(!state.views.installing.details);
        assert_eq!(hints(&state).len(), 1);
    }
}
