//! The worker's own lines, behind Details: what an installation did, in the
//! technical words it did it in.
//!
//! Never on the face of a panel — the bar and its step say what is happening
//! in plain words — but there for whoever wants to see, and for a bug
//! report. One line each, newest at the bottom, cut where a line is wider
//! than the panel (the whole of it is in the log file) — but for the
//! failure's own lines at the head of Stopped's, which say where it stopped
//! in the person's language and are worth reading whole: those wrap, up to
//! [`HEAD_LINES`] each. The view follows the end as lines arrive, unless the
//! person has scrolled up to read something, and follows again once they
//! scroll back down to the end.

use lxb_app::lxb_render::Align;
use lxb_app::lxb_toolkit::{material::Surface, metrics::Metric, palette::Role, typography::Text};
use lxb_app::Page;

use super::motion::Carried;
use super::text;

/// How many lines one press of Up or Down scrolls.
const STEP: f32 = 3.0;

/// The most lines one of the failure's own lines at the head is given
/// before what is left is cut.
pub const HEAD_LINES: usize = 3;

/// How wide the lines are in a view `width` wide, at the design language's
/// `scale`: the view less its padding either side.
pub fn words_room_at(scale: f32, width: f32) -> f32 {
    (width - Metric::RowPadding.value() * scale * 0.6 * 2.0).max(1.0)
}

/// The view's own state: how far it is scrolled, and whether it follows the
/// end.
#[derive(Debug, Clone)]
pub struct Log {
    scroll: Carried,
    /// Following the newest line. Off once the person scrolls up.
    pub follow: bool,
    /// How tall a line and the view were on the last frame, which a press
    /// is measured in.
    line: f32,
    most: f32,
}

impl Default for Log {
    fn default() -> Self {
        Self {
            scroll: Carried::default(),
            follow: true,
            line: 0.0,
            most: 0.0,
        }
    }
}

impl Log {
    /// A view that opens at its first line and stays there until scrolled:
    /// for a record whose first lines are the ones that matter, as a
    /// failure's are.
    pub fn from_top() -> Self {
        Self {
            follow: false,
            ..Self::default()
        }
    }

    /// Scroll `direction` (−1 up, +1 down) by a few lines. Up lets go of the
    /// end; reaching the end again takes hold of it.
    pub fn step(&mut self, direction: f32) {
        let line = if self.line > 0.0 { self.line } else { 1.0 };
        let from = if self.follow {
            self.most
        } else {
            self.scroll.wanted()
        };
        let to = (from + direction * STEP * line).clamp(0.0, self.most);
        self.follow = to >= self.most - 0.5;
        let dt = 0.0;
        // Told where to go now; carried there over the frames that follow.
        self.scroll.advance(to, dt);
    }

    /// Put the scroll where it is going, for a photograph.
    pub fn settle(&mut self) {
        self.scroll.settle();
    }

    /// Draw `lines` in `rect`, with `first` — why it stopped, where it did
    /// — over them in the panel's full ink, so what went wrong is the first
    /// thing read. Not in Danger: the toolkit's red on this darker pane is
    /// too dim to read a technical line in, and the headline above has
    /// already said that something went wrong.
    pub fn draw(
        &mut self,
        page: &mut Page,
        rect: [f32; 4],
        first: &[String],
        lines: &[String],
        dt: f32,
    ) {
        let pad = page.metric(Metric::RowPadding) * 0.6;
        let line = page.line(Text::Caption);
        let inside = [
            rect[0] + pad,
            rect[1] + pad * 0.5,
            words_room_at(page.scaled(1.0), rect[2]),
            (rect[3] - pad).max(1.0),
        ];
        let first: Vec<String> = first
            .iter()
            .flat_map(|one| text::wrapped(page.ui(), Text::Caption, one, inside[2], HEAD_LINES))
            .collect();
        let count = first.len() + lines.len();
        let total = count as f32 * line;
        let most = (total - inside[3]).max(0.0);
        self.line = line;
        self.most = most;
        let wanted = if self.follow {
            most
        } else {
            self.scroll.wanted().min(most)
        };
        let top = self.scroll.advance(wanted, dt).clamp(0.0, most);

        let ui = page.ui();
        // A darker pane under the lines, so they read as a record set apart
        // from the panel rather than as more of its words.
        ui.card(rect, Surface::Control, Role::Glass, 0.55);
        let from = ui.written();
        let shown_from = (top / line).floor().max(0.0) as usize;
        let shown_to = (((top + inside[3]) / line).ceil() as usize + 1).min(count);
        for index in shown_from..shown_to {
            let y = inside[1] + index as f32 * line - top;
            let at = [inside[0], y, inside[2], line];
            let (words, (role, alpha)) = match index.checked_sub(first.len()) {
                None => (&first[index], (Role::Text, 1.0)),
                Some(later) => (&lines[later], text::LESSER),
            };
            text::label_faded(ui, at, Text::Caption, words, role, alpha, Align::Left);
        }
        let to = ui.written();
        ui.cut_between(from, to, inside);
        let band = line * 1.5;
        ui.soft_edges(
            inside,
            band,
            (top / band).min(1.0),
            ((most - top) / band).min(1.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolling_up_lets_go_of_the_end_and_coming_back_down_takes_hold_again() {
        let mut log = Log {
            line: 10.0,
            most: 100.0,
            ..Log::default()
        };
        assert!(log.follow);
        log.step(-1.0);
        assert!(!log.follow, "still following after scrolling up");
        assert_eq!(log.scroll.wanted(), 70.0);
        log.step(1.0);
        assert!(log.follow, "back at the end, yet not following");
        log.step(1.0);
        assert!(log.follow);
    }
}
