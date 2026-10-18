//! The bar the installation is shown by.
//!
//! DistriBumpy's bar: a capsule track of glass, filled with the accent, with
//! a light at its leading edge so it reads as filling rather than as having
//! been filled. What it is on goes on the left above it and how far through
//! the whole on the right — "Step 4 of 12 · 40%", the console rule's "a bar
//! with a number and the thing it is on".
//!
//! The bar follows the installation rather than jumping to it: the worker
//! reports in bursts, and a bar that leapt with every report would look like
//! something going wrong rather than something going.

use lxb_app::lxb_render::Align;
use lxb_app::lxb_toolkit::{metrics::Metric, motion, palette::Role, typography::Text};
use lxb_app::Page;

use super::text;

/// The bar's own position.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Progress {
    through: f32,
    target: f32,
}

impl Progress {
    /// Follow the work a frame further: `target` is how far through it really
    /// is, nought to one.
    pub fn advance(&mut self, target: f32, dt: f32) {
        self.target = target.clamp(0.0, 1.0);
        self.through = motion::approach(self.through, self.target, dt * 1.6);
    }

    /// Where the bar is drawn to.
    pub fn through(&self) -> f32 {
        self.through
    }

    /// Put the bar where the work is, for a photograph.
    pub fn settle(&mut self) {
        self.through = self.target;
    }

    /// Start again from nothing.
    pub fn restart(&mut self) {
        *self = Self::default();
    }

    pub fn moving(&self) -> bool {
        self.through != self.target
    }
}

/// How tall the bar and the words with it are together, `width` wide, the
/// count being at most `widest` ("Step 17 of 17 · 100%").
///
/// **The same on every step**: room for what the work is on is kept for two
/// lines whether it takes one or two, and the words stand on the bar,
/// growing up from it. Sized by the words of each step, the bar moved by
/// most of a line whenever a step that wraps followed one that did not, and
/// back again on the next.
pub fn height(page: &mut Page, width: f32, widest: &str) -> f32 {
    let line = page.line(Text::Caption);
    let gap = page.metric(Metric::Gap);
    let below = if count_below(page, width, widest) {
        gap * 0.5 + line
    } else {
        0.0
    };
    line * ON_LINES as f32 + gap * 0.5 + page.scaled(THICK) + below
}

/// The most lines what the work is on is given over the bar.
const ON_LINES: usize = 2;

/// Whether the count stands under the bar, at its right, rather than beside
/// what the work is on: where the widest it can be would take more than a
/// third of a bar `width` wide — a display standing on its side — and leave
/// the words too little room. Decided by the widest count, not this one, so
/// it does not move from one step to the next.
fn count_below(page: &mut Page, width: f32, widest: &str) -> bool {
    count_below_at(width, page.measure(Text::Caption, widest))
}

/// How much of a bar `width` wide what the work is on may take on a line.
fn on_room(page: &mut Page, width: f32, widest: &str) -> f32 {
    if count_below(page, width, widest) {
        width
    } else {
        (width - page.measure(Text::Caption, widest) - page.metric(Metric::Gap)).max(0.0)
    }
}

/// How thick the bar is, in reference pixels.
const THICK: f32 = 8.0;

/// Draw the bar in `rect` (as tall as [`height`] says): `on` (what the work
/// is on) at the left over it, on one line or two standing on the bar, and
/// `count` ("Step 4 of 12 · 40%") at the right of the line over the bar — or
/// under the bar where `widest`, the longest the count can be, would crowd
/// the words.
pub fn draw(page: &mut Page, rect: [f32; 4], through: f32, on: &str, count: &str, widest: &str) {
    let line = page.line(Text::Caption);
    let gap = page.metric(Metric::Gap);
    let thick = page.scaled(THICK);
    let below = count_below(page, rect[2], widest);
    let room = on_room(page, rect[2], widest);
    let lines = text::wrapped(page.ui(), Text::Caption, on, room, ON_LINES);
    let words = line * ON_LINES as f32;
    let bar = [rect[0], rect[1] + words + gap * 0.5, rect[2], thick];
    let ui = page.ui();
    let first = rect[1] + words - line * lines.len() as f32;
    for (index, said) in lines.iter().enumerate() {
        text::label(
            ui,
            [rect[0], first + index as f32 * line, room, line],
            Text::Caption,
            said,
            Role::Text,
            Align::Left,
        );
    }
    let count_at = if below {
        [rect[0], bar[1] + thick + gap * 0.5, rect[2], line]
    } else {
        let count_w = rect[2] - room - gap;
        [
            rect[0] + rect[2] - count_w,
            rect[1] + words - line,
            count_w,
            line,
        ]
    };
    text::label_faded(
        ui,
        count_at,
        Text::Caption,
        count,
        text::LESSER.0,
        text::LESSER.1,
        Align::Right,
    );
    ui.chip(bar, ui.tinted(Role::Glass, 0.5));
    let through = through.clamp(0.0, 1.0);
    if through > 0.0 {
        let run = (bar[2] * through).max(thick);
        ui.chip([bar[0], bar[1], run, bar[3]], ui.role(Role::Accent));
        // The light a bar carries at its leading edge.
        ui.glow(
            [
                bar[0] + run - thick * 3.0,
                bar[1] - thick,
                thick * 6.0,
                thick * 3.0,
            ],
            Role::Accent,
            0.35,
        );
    }
}

/// Whether the count of a bar `width` wide stands under it, by the widest
/// count's width `widest`: what the catalogs' test lays the words out by.
pub fn count_below_at(width: f32, widest: f32) -> bool {
    widest > width / 3.0
}

/// The whole number of percent a share is, rounded down so 100% means done.
pub fn percent(through: f32) -> u32 {
    (through.clamp(0.0, 1.0) * 100.0).floor() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_catches_up_rather_than_jumping() {
        let mut bar = Progress::default();
        bar.advance(1.0, 1.0 / 60.0);
        assert!(bar.through() > 0.0 && bar.through() < 1.0);
        for _ in 0..120 {
            bar.advance(1.0, 1.0 / 60.0);
        }
        assert_eq!(bar.through(), 1.0);
        assert!(!bar.moving());
    }

    #[test]
    fn a_hundred_percent_means_done() {
        assert_eq!(percent(0.999), 99);
        assert_eq!(percent(1.0), 100);
        assert_eq!(percent(-1.0), 0);
    }
}
