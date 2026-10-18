//! Done: Linexin is installed. Restart now first — the one primary verb,
//! where the light starts — and Not now beside it, which closes the
//! installer and leaves the live session running. Anything that did not
//! work while the installed system is still sound is said under the
//! headline, one plain sentence each, with what can be done about it.
//!
//! Restart now restarts the computer as the person (logind lets the active
//! session's own user) on a live system only; the made-up machine just
//! closes. The press only asks for it: the frame does it (`State::restart`),
//! so nothing answering presses — no test, no picture — ever restarts
//! anything.

use lxb_app::lxb_render::Align;
use lxb_app::lxb_toolkit::{
    input::Action, metrics::Metric, palette::Role, sound::Sound, typography::Text,
};
use lxb_app::{Page, Scroll};

use super::{Head, Room};
use crate::i18n;
use crate::model::protocol::Warning;
use crate::ui::app::State;
use crate::ui::button::{self, Button};
use crate::ui::legend::{hint, Button as Pictured, Hint};
use crate::ui::panel::Note;
use crate::ui::{text, words};

/// Restart now, then Not now.
const RESTART: usize = 0;
const NOT_NOW: usize = 1;

/// What this screen keeps between frames: which button the light is on.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct View {
    pub at: usize,
}

fn buttons() -> [Button<'static>; 2] {
    [
        Button::primary(i18n::text("button-restart")).shorter(i18n::text("button-restart-short")),
        Button::plain(i18n::text("button-not-now")),
    ]
}

/// Restart now: asked of the frame, which does it.
fn restart(state: &mut State) {
    state.sound(Sound::Press);
    state.restart();
}

/// Not now: the installer closes and the live session goes on.
fn not_now(state: &mut State) {
    super::close(state);
}

pub fn enter(state: &mut State) {
    state.views.done = View { at: RESTART };
}

pub fn head(_state: &State) -> Head {
    Head::new(
        "chosen",
        i18n::text("done-title"),
        i18n::text("done-caption"),
    )
}

pub fn hints(_state: &State) -> Vec<Hint> {
    vec![hint(i18n::text("hint-choose"), Pictured::Accept)]
}

pub fn note(_state: &State) -> Option<Note> {
    None
}

/// Left and Right walk the two buttons and Accept presses the one the light
/// is on; Back does nothing, because there is nothing to go back to.
pub fn act(state: &mut State, action: Action, _scroll: Option<Scroll>) {
    let at = state.views.done.at;
    match action {
        Action::Left | Action::Up if at == NOT_NOW => {
            state.views.done.at = RESTART;
            state.sound(Sound::Move);
        }
        Action::Right | Action::Down if at == RESTART => {
            state.views.done.at = NOT_NOW;
            state.sound(Sound::Move);
        }
        Action::Accept if at == RESTART => restart(state),
        Action::Accept => not_now(state),
        _ => {}
    }
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let Room {
        content,
        buttons: band,
        ..
    } = room;
    let line = page.line(Text::Body);
    let gap = page.metric(Metric::Gap);
    let room = content[3];
    let width = content[2].min(page.scaled(900.0));
    let (lead, warnings) = said(&state.install.warnings);
    // The lead in the ink of the panel's words, each thing that did not work
    // under it as its own paragraph, quieter, behind a bullet that hangs:
    // the lines a long one wraps onto start where its words do, not under
    // the bullet, so each thing reads as one.
    let ui = page.ui();
    let bullet = "• ";
    let hang = ui.measure(Text::Body, bullet);
    let mut y = content[1];
    let most = ((room / line).floor() as usize).max(1);
    let mut left = most;
    for (index, words) in std::iter::once(lead.as_str()).chain(warnings).enumerate() {
        if left == 0 {
            break;
        }
        let ((role, alpha), indent, hung) = if index == 0 {
            ((Role::Text, 1.0), 0.0, 0.0)
        } else {
            (text::LESSER, gap * 1.2, hang)
        };
        let ink = ui.tinted(role, alpha);
        let room = (width - indent - hung).max(1.0);
        let lines = text::wrapped(ui, Text::Body, words, room, left.min(3));
        if hung > 0.0 && !lines.is_empty() {
            ui.label_tinted(
                [content[0] + indent, y, hung + 1.0, line],
                Text::Body,
                bullet,
                ink,
                Align::Left,
            );
        }
        for one in &lines {
            ui.label_tinted(
                [content[0] + indent + hung, y, room, line],
                Text::Body,
                one,
                ink,
                Align::Left,
            );
            y += line;
        }
        left = left.saturating_sub(lines.len());
        y += gap * 0.5;
    }

    let buttons = button::fitted(page, band[2], &buttons());
    let places = button::places(page, band, &buttons);
    let pressing = state.pressing.through();
    let at = lit.then_some(state.views.done.at);
    let laid = button::row(
        page,
        &places,
        &buttons,
        at,
        &mut state.light,
        pressing,
        state.clock.dt,
    );
    state.laid_buttons(laid);
}

pub fn hover(state: &mut State, id: u32) {
    if let Some(at) = super::button_of(id, buttons().len()) {
        state.views.done.at = at;
    }
}

pub fn pressed(state: &mut State, page: &mut Page) {
    match button::pressed(page, &buttons()) {
        Some(RESTART) => {
            state.views.done.at = RESTART;
            restart(state);
        }
        Some(_) => {
            state.views.done.at = NOT_NOW;
            not_now(state);
        }
        None => {}
    }
}

/// Whether `warning` says something that did not work, rather than
/// something to know of what did: Windows' partition shrunk as planned,
/// which Windows checks the next time it starts, went as planned.
fn went_wrong(warning: Warning) -> bool {
    !matches!(warning, Warning::WindowsWillCheck)
}

/// What Done says of `warnings`: its lead — everything went as planned, or
/// how many things did not (counted, so one is not introduced as "a few")
/// — and each warning under it, those that went wrong first, then what is
/// only to be known (Windows will check its disk), which the lead does not
/// count as anything that went wrong.
fn said(warnings: &[Warning]) -> (String, Vec<&'static str>) {
    let wrong: Vec<Warning> = warnings
        .iter()
        .copied()
        .filter(|w| went_wrong(*w))
        .collect();
    let lead = if wrong.is_empty() {
        i18n::text("done-all-well").to_string()
    } else {
        crate::message!("done-warnings", "count" => wrong.len() as i64)
    };
    let known = warnings.iter().copied().filter(|w| !went_wrong(*w));
    let said = wrong.into_iter().chain(known).map(words::warning).collect();
    (lead, said)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::app::Screen;

    /// **Windows' check is something to know, not something that went
    /// wrong** (Unit Q, Q9): alone, Done says everything went as planned
    /// and then that Windows will check its disk; beside a warning, the lead
    /// counts the warning alone, which is said first.
    #[test]
    fn windows_checking_its_disk_is_said_after_the_lead_but_never_counted_as_gone_wrong() {
        let windows = words::warning(Warning::WindowsWillCheck);
        let entry = words::warning(Warning::BootEntryNotRegistered);
        assert_eq!(said(&[]), (i18n::text("done-all-well").to_string(), vec![]));
        assert_eq!(
            said(&[Warning::WindowsWillCheck]),
            (i18n::text("done-all-well").to_string(), vec![windows])
        );
        let one = crate::message!("done-warnings", "count" => 1i64);
        for warnings in [
            [Warning::WindowsWillCheck, Warning::BootEntryNotRegistered],
            [Warning::BootEntryNotRegistered, Warning::WindowsWillCheck],
        ] {
            assert_eq!(said(&warnings), (one.clone(), vec![entry, windows]));
        }
        let mut state = State::demo(true);
        state.stage("done-shrink").unwrap();
        assert_eq!(state.install.warnings, [Warning::WindowsWillCheck]);
    }

    #[test]
    fn restart_now_is_first_and_only_asks_the_frame() {
        let mut state = State::demo(true);
        state.stage("done").unwrap();
        assert_eq!(state.screen, Screen::Done);
        assert_eq!(state.views.done.at, RESTART);
        act(&mut state, Action::Accept, None);
        assert!(state.restarting(), "Restart now asked for nothing");
    }

    #[test]
    fn hovering_lights_either_button_and_presses_neither() {
        let mut state = State::demo(true);
        state.stage("done").unwrap();
        hover(&mut state, crate::ui::spots::BUTTONS + NOT_NOW as u32);
        assert_eq!(state.views.done.at, NOT_NOW);
        assert!(!state.leaving() && !state.restarting());
        hover(&mut state, crate::ui::spots::BUTTONS + RESTART as u32);
        assert_eq!(state.views.done.at, RESTART);
        assert!(!state.restarting(), "a hover restarted");
    }

    #[test]
    fn not_now_closes_and_back_does_nothing() {
        let mut state = State::demo(true);
        state.stage("done").unwrap();
        act(&mut state, Action::Back, None);
        assert_eq!(state.screen, Screen::Done);
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.done.at, NOT_NOW);
        act(&mut state, Action::Accept, None);
        assert!(!state.restarting(), "Not now restarted");
    }
}
