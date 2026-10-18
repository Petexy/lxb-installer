//! Stopped: the installation did not finish. The headline says what went
//! wrong and the sentence under it what can be done about it, both in plain
//! words; the technical detail goes behind Options, as Details. Where
//! Windows' partition was shrunk before it stopped, a quieter sentence under
//! that says Windows will check its disk the next time it starts, as Done
//! would have.
//!
//! Details is the same scrolled column Installing keeps the worker's lines
//! in, with the failure itself at its head — the step it stopped on, in
//! words, then the worker's own account of it — and every line the worker
//! wrote after that. It opens at the top, not at the end as Installing's
//! does: here the first lines are the ones that matter, and the ones a bug
//! report wants. Close is the one verb, and the legend names Options only
//! when there is something behind it.

use lxb_app::lxb_render::Align;
use lxb_app::lxb_toolkit::{
    input::Action, metrics::Metric, palette::Role, sound::Sound, typography::Text,
};
use lxb_app::{Page, Scroll};

use super::{Head, Room};
use crate::i18n;
use crate::model::protocol::{Failure, Step, Warning};
use crate::ui::app::State;
use crate::ui::button::Button;
use crate::ui::install::Ending;
use crate::ui::legend::{hint, Button as Pictured, Hint};
use crate::ui::log::Log;
use crate::ui::panel::Note;
use crate::ui::{text, words};

/// What this screen keeps between frames.
#[derive(Debug, Clone)]
pub struct View {
    /// Whether Details is open.
    pub details: bool,
    pub log: Log,
}

impl Default for View {
    fn default() -> Self {
        Self {
            details: false,
            log: Log::from_top(),
        }
    }
}

impl View {
    pub fn settle(&mut self) {
        self.log.settle();
    }
}

fn verb() -> Button<'static> {
    Button::primary(i18n::text("button-close"))
}

/// What stopped it, and in which step. An installation that ended without
/// a word from the worker is the installer's own fault, and is said as that
/// — in no step, which says nothing about the disk either way, so the
/// installer's own failure claims nothing about it.
fn failure(state: &State) -> (Failure, Option<Step>) {
    match &state.install.ending {
        Some(Ending::Failed { failure, step, .. }) => (*failure, *step),
        _ => (Failure::Internal, None),
    }
}

/// The headline and the sentence under it, for where it stopped.
fn said(state: &State) -> (&'static str, &'static str) {
    let (failure, step) = failure(state);
    words::failure(failure, step)
}

/// What went as planned before it stopped that the person must still
/// learn of (step Q9): Windows' partition shrunk — the worker
/// says so once the shrink is read back, before any entry changes — which
/// Windows checks the next time it starts, and Linexin leaves alone until
/// it has (`Warning::WindowsWillCheck`), in Done's words. Nothing else Done
/// would say belongs on Stopped.
fn known(state: &State) -> Option<&'static str> {
    state
        .install
        .warnings
        .contains(&Warning::WindowsWillCheck)
        .then(|| words::warning(Warning::WindowsWillCheck))
}

/// [`known`] under what to do, from `top`, quieter, as Done says its
/// warnings: three lines at most. Gives the height it took, nothing where
/// there is nothing to say.
fn draw_known(state: &State, page: &mut Page, content: [f32; 4], top: f32) -> f32 {
    let Some(known) = known(state) else {
        return 0.0;
    };
    let line = page.line(Text::Body);
    let gap = page.metric(Metric::Gap);
    let top = top + gap * 0.5;
    let room = ((content[1] + content[3] - top) / line).floor() as usize;
    let ui = page.ui();
    let (role, alpha) = text::LESSER;
    let ink = ui.tinted(role, alpha);
    let lines = text::wrapped(ui, Text::Body, known, content[2], room.min(3));
    for (index, one) in lines.iter().enumerate() {
        ui.label_tinted(
            [content[0], top + index as f32 * line, content[2], line],
            Text::Body,
            one,
            ink,
            Align::Left,
        );
    }
    if lines.is_empty() {
        0.0
    } else {
        gap * 0.5 + lines.len() as f32 * line
    }
}

/// The head of Details: the step it stopped on, in words — its name held
/// together where the line wraps — then the worker's own account of what
/// went wrong, a line of it at a time.
fn failure_lines(state: &State) -> Vec<String> {
    let Some(Ending::Failed { step, detail, .. }) = &state.install.ending else {
        return Vec::new();
    };
    let named = |step| text::unbroken(words::step(step));
    let step = step.map(|step| crate::message!("stopped-step", "step" => named(step)));
    step.into_iter()
        .chain(
            detail
                .lines()
                .map(str::trim_end)
                .filter(|line| !line.trim().is_empty())
                .map(str::to_string),
        )
        .collect()
}

/// Whether Details would show anything. With nothing behind it, Options is
/// not offered at all rather than opening an empty pane.
fn has_details(state: &State) -> bool {
    !state.install.log.is_empty() || !failure_lines(state).is_empty()
}

pub fn enter(state: &mut State) {
    state.views.stopped = View::default();
}

/// The picture of Details: the failure with its lines open.
pub fn stage(state: &mut State) {
    state.views.stopped.details = true;
}

pub fn head(state: &State) -> Head {
    let (headline, _) = said(state);
    Head::new("setting-info", i18n::text("stopped-title"), headline)
}

/// Close, on Accept; and Details on Options when there are any — or, while
/// they are open, hiding them.
pub fn hints(state: &State) -> Vec<Hint> {
    let mut hints = vec![hint(i18n::text("hint-choose"), Pictured::Accept)];
    if has_details(state) {
        let label = if state.views.stopped.details {
            i18n::text("hint-hide-details")
        } else {
            i18n::text("hint-details")
        };
        hints.push(hint(label, Pictured::Options));
    }
    hints
}

pub fn note(_state: &State) -> Option<Note> {
    None
}

/// Accept closes the installer; Options opens and closes Details, and Up and
/// Down (or a wheel) scroll them while they are open. Back does nothing:
/// there is nothing to go back to.
pub fn act(state: &mut State, action: Action, _scroll: Option<Scroll>) {
    match action {
        Action::Accept => super::close(state),
        Action::Menu if has_details(state) => {
            let view = &mut state.views.stopped;
            view.details = !view.details;
            state.sound(Sound::Press);
        }
        Action::Up if state.views.stopped.details => state.views.stopped.log.step(-1.0),
        Action::Down if state.views.stopped.details => state.views.stopped.log.step(1.0),
        _ => {}
    }
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let content = room.content;
    let (_, what_to_do) = said(state);
    let said = super::words(page, content, what_to_do, Role::Text);
    let said = said + draw_known(state, page, content, content[1] + said);
    if state.views.stopped.details && has_details(state) {
        // Between the sentence and Close: the pane keeps clear of both, so
        // opening it moves nothing the person was already reading.
        let gap = page.metric(Metric::Gap);
        let top = content[1] + said + gap * 1.2;
        let bottom = content[1] + content[3];
        let area = [content[0], top, content[2], (bottom - top).max(1.0)];
        let first = failure_lines(state);
        let dt = state.clock.dt;
        let lines = &state.install.log;
        state.views.stopped.log.draw(page, area, &first, lines, dt);
    }
    super::lone_button(state, page, room.buttons, verb(), lit);
}

pub fn pressed(state: &mut State, page: &mut Page) {
    if super::lone_pressed(page, verb()) {
        super::close(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::protocol::{Event, Step};
    use crate::ui::app::Screen;

    /// **Stopped says Windows will check its disk once Windows' partition
    /// was shrunk** (Unit Q, Q9): stopped in Partition after the
    /// worker said so, Stopped says it under what to do, in Done's words;
    /// stopped before it was said — in Resize, or in Check — or with nothing
    /// of Windows shrunk, it says nothing of it, nor anything else Done
    /// would.
    #[test]
    fn windows_checking_its_disk_is_said_once_its_partition_was_shrunk() {
        let staged = |scene: &str| {
            let mut state = State::demo(true);
            state.stage(scene).unwrap();
            assert_eq!(state.screen, crate::ui::app::Screen::Stopped, "{scene}");
            state
        };
        let shrunk = staged("stopped-windows-shrunk");
        assert_eq!(
            known(&shrunk),
            Some(words::warning(Warning::WindowsWillCheck))
        );
        assert!(matches!(
            shrunk.install.ending,
            Some(Ending::Failed {
                step: Some(Step::Partition),
                ..
            })
        ));
        for scene in ["stopped-shrink", "stopped-hibernated", "stopped"] {
            assert_eq!(known(&staged(scene)), None, "{scene}");
        }
        let mut other = staged("stopped");
        other.install.take(Event::Warning {
            warning: Warning::BootEntryNotRegistered,
        });
        assert_eq!(known(&other), None);
    }

    #[test]
    fn options_opens_details_with_the_failure_first_and_close_still_closes() {
        let mut state = State::demo(true);
        state.stage("stopped").unwrap();
        assert_eq!(state.screen, Screen::Stopped);
        assert!(!state.views.stopped.details);
        assert_eq!(
            hints(&state).iter().map(|one| one.on).collect::<Vec<_>>(),
            [Pictured::Accept, Pictured::Options]
        );

        let first = failure_lines(&state);
        assert_eq!(
            first[0].replace('\u{a0}', " "),
            crate::message!("stopped-step", "step" => words::step(Step::Copy)),
            "Details did not open on where it stopped"
        );
        assert_eq!(first[1], "rehearsal: a made-up failure");

        act(&mut state, Action::Menu, None);
        assert!(state.views.stopped.details);
        assert_eq!(hints(&state)[1].label, i18n::text("hint-hide-details"));
        assert!(
            !state.views.stopped.log.follow,
            "Details opened at the end instead of on the failure"
        );
        act(&mut state, Action::Back, None);
        assert_eq!(state.screen, Screen::Stopped, "Back went somewhere");
        act(&mut state, Action::Menu, None);
        assert!(!state.views.stopped.details);
        assert_eq!(hints(&state)[1].label, i18n::text("hint-details"));
        act(&mut state, Action::Accept, None);
        assert!(state.leaving(), "Close did not close");
    }

    #[test]
    fn with_nothing_to_show_options_is_neither_named_nor_answered() {
        let mut state = State::demo(true);
        state.go(Screen::Stopped);
        state.install.log.clear();
        state.install.ending = None;
        assert!(!has_details(&state));
        assert_eq!(hints(&state).len(), 1, "Options named over nothing");
        act(&mut state, Action::Menu, None);
        assert!(!state.views.stopped.details);

        // A failure with no lines of the worker's still has its own account.
        state.install.take(Event::Failed {
            step: None,
            failure: Failure::DiskBusy,
            detail: "\n  example detail  \n\n".into(),
        });
        assert_eq!(failure_lines(&state), ["  example detail"]);
        assert_eq!(hints(&state).len(), 2);
    }

    /// "Nothing was erased" only where the worker had only looked: a disk
    /// that changed is refused in Check before anything is written, but the
    /// same failure in Format comes from a disk already repartitioned.
    #[test]
    fn nothing_erased_is_said_only_before_anything_was_written() {
        let mut state = State::demo(true);
        let mut stopped_in = |step: Option<Step>, failure: Failure| {
            state.install.ending = Some(Ending::Failed {
                step,
                failure,
                detail: "example detail".into(),
            });
            said(&state).1
        };
        assert_eq!(
            stopped_in(Some(Step::Check), Failure::DiskChanged),
            i18n::text("failure-disk-changed-untouched-do")
        );
        assert_eq!(
            stopped_in(None, Failure::DiskBusy),
            i18n::text("failure-disk-busy-untouched-do")
        );
        assert_eq!(
            stopped_in(Some(Step::Check), Failure::NeedsInternet),
            i18n::text("failure-needs-internet-untouched-do")
        );
        assert_eq!(
            stopped_in(Some(Step::Format), Failure::DiskChanged),
            i18n::text("failure-disk-changed-do")
        );
        assert_eq!(
            stopped_in(Some(Step::Partition), Failure::DiskBusy),
            i18n::text("failure-disk-busy-do")
        );
        assert_eq!(
            stopped_in(Some(Step::Desktop), Failure::NeedsInternet),
            i18n::text("failure-needs-internet-do")
        );
        assert_ne!(
            i18n::text("failure-disk-changed-do"),
            i18n::text("failure-disk-changed-untouched-do")
        );
    }
}
