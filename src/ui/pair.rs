//! Two columns side by side, the second following the first: Where to
//! install's disks and the places on them, read together. (Time zone and
//! Keyboard walk theirs as LineXinBar's file picker does, `trail`.)
//!
//! On a wide panel the two stand side by side and the light crosses between
//! them with Left and Right (or Accept going on, Back coming back). On a
//! narrow one — a window standing on its side — the left column keeps the
//! width its rows need, the right one is laid out past the panel's edge at a
//! width it can use, and the whole view slides over to it when the light goes
//! there and back when it comes out: the toolkit's `layout::beside` and
//! `layout::Slide`, which are LineXinBar's Home menu's own rule between its
//! column and its cards, and the family's rule for every program on a
//! display standing on its side.

use lxb_app::lxb_render::Align;
use lxb_app::lxb_toolkit::{
    input::Action,
    layout::{self, Beside, Slide},
    metrics::Metric,
    typography::Text,
};
use lxb_app::{Page, Scroll};

use super::column::{Column, Corners, Look, Pick, Row};
use super::motion::Light;
use super::spots;
use super::text;

/// How wide the left-hand column is where the right-hand one has room beside
/// it, in reference pixels: room for a disk's name and its note without
/// cutting either, after the mark of the kind of disk it is (a column row's
/// mark and the gap after it).
pub const LEFT: f32 = 440.0 + Metric::ItemIcon.value() * 0.6 + Metric::Gap.value();

/// The narrowest the left-hand column grows to leave the right-hand one a
/// peek, in reference pixels: a disk's row with its mark, its name on two
/// lines at most.
pub const LEFT_LEAST: f32 = 340.0;

/// The narrowest the right-hand column can be and still be read, in
/// reference pixels. Every landscape panel has more; a portrait one does not,
/// and there the view slides.
pub const LEAST: f32 = 460.0;

/// The air between the two columns, in reference pixels.
const GAP: f32 = Metric::PanelPadding.value() * 0.6;

/// How wide the left-hand column is on a panel whose room is `room` wide,
/// both in reference pixels.
///
/// `LEFT` wherever the right-hand column has `LEAST` beside it — every
/// landscape panel, laid out as it always was. Where the view slides (a
/// display standing on its side), the column is narrower by as much as
/// leaves the right-hand one peeking in at the panel's edge by the toolkit's
/// `layout::PEEK` of the room, the cue that Right or Accept leads somewhere,
/// as the cards wait past the edge of LineXinBar's Home menu — but never
/// narrower than `LEFT_LEAST`. At `LEFT` the disks' column filled a 9:16
/// panel and the places' column was laid out wholly past its edge.
pub fn left_width(room: f32) -> f32 {
    if room - LEFT - GAP >= LEAST {
        return LEFT;
    }
    (room - GAP - room * layout::PEEK).clamp(LEFT_LEAST, LEFT)
}

/// The two columns' widths on a panel `room` wide, in reference pixels: the
/// left one's, and where the right one stands and how wide it is
/// (`layout::beside`).
pub fn beside_at(room: f32) -> (f32, Beside) {
    let left = left_width(room);
    (left, layout::beside(room, 0.0, left, GAP, LEAST))
}

/// Which of the two columns the light is in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Side {
    #[default]
    Left,
    Right,
}

/// What a press did to the pair, for the screen to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    /// Nothing moved; nothing is heard.
    Nothing,
    /// The light moved within a column. `left` says it was the left one,
    /// whose choice decides what the right one lists — the screen builds the
    /// right-hand rows afresh and calls `right.start_at_chosen` with them.
    Moved { left: bool },
    /// The light crossed to the other column; `pressed` when it was Accept
    /// that took it (a press is heard rather than a move).
    Crossed { to: Side, pressed: bool },
    /// Accept on a row of the right-hand column: the screen takes its value.
    Chose(usize),
    /// Back from the right-hand column to the left one.
    Backed,
    /// Not the pair's to answer (Back from the left-hand column, Options…).
    Pass,
}

/// The pair's own state: its two columns, where the light is and how far the
/// view has slid.
#[derive(Debug, Default, Clone)]
pub struct Pair {
    pub left: Column,
    pub right: Column,
    pub side: Side,
    slide: Slide,
    /// Whether a pad is in hand, which draws the columns without their
    /// scroll bars: told by the screen before it draws, from `State::pad`.
    pub pad: bool,
    /// Whether the pair slid between its columns on the last frame drawn
    /// (a narrow panel), where the light crossing over moves the view.
    slides: bool,
}

/// Where the pointer put the light in a pair (`Pair::point_at`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pointed {
    /// The left-hand column's light moved to another row, so the screen
    /// lists the right-hand column afresh.
    pub left_moved: bool,
}

/// The band a screen kept over the right-hand column and the room it kept at
/// the foot of the left-hand one, where the slide has put them, and what to
/// cut what it draws there to (`None` where nothing slides and nothing needs
/// cutting).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reserved {
    pub band: [f32; 4],
    pub foot: [f32; 4],
    pub cut: Option<[f32; 4]>,
}

/// Where the two columns go in `room`, before any slide: how wide the left
/// one is, and where the right one stands and how wide it is.
pub fn beside(page: &Page, room: [f32; 4]) -> (f32, Beside) {
    let scale = page.scaled(1.0).max(f32::EPSILON);
    let (left, laid) = beside_at(room[2] / scale);
    let scaled = |value: f32| value * scale;
    (
        scaled(left),
        Beside {
            page_x: scaled(laid.page_x),
            page_w: scaled(laid.page_w),
            reach: scaled(laid.reach),
        },
    )
}

impl Pair {
    /// Answer one press. `left_rows` and `right_rows` are what the two
    /// columns list now.
    pub fn act(&mut self, action: Action, left_rows: &[Row], right_rows: &[Row]) -> Went {
        let right_open = right_rows.iter().any(|row| row.enabled);
        match (self.side, action) {
            (Side::Left, Action::Up) => self.moved(true, -1, left_rows),
            (Side::Left, Action::Down) => self.moved(true, 1, left_rows),
            (Side::Right, Action::Up) => self.moved(false, -1, right_rows),
            (Side::Right, Action::Down) => self.moved(false, 1, right_rows),
            (Side::Left, Action::Right | Action::Accept) if right_open => {
                self.side = Side::Right;
                self.right.fix(right_rows);
                Went::Crossed {
                    to: Side::Right,
                    pressed: action == Action::Accept,
                }
            }
            (Side::Right, Action::Left) => {
                self.side = Side::Left;
                Went::Crossed {
                    to: Side::Left,
                    pressed: false,
                }
            }
            (Side::Right, Action::Back) => {
                self.side = Side::Left;
                Went::Backed
            }
            (Side::Right, Action::Accept)
                if right_rows.get(self.right.at).is_some_and(|row| row.enabled) =>
            {
                Went::Chose(self.right.at)
            }
            (_, Action::Back | Action::Menu | Action::Submit) => Went::Pass,
            _ => Went::Nothing,
        }
    }

    /// The column a pair that slides (a narrow panel, on the last frame
    /// drawn) is slid over to, or `None` where the two stand side by side:
    /// what crossing over puts under a resting pointer.
    pub fn slid_to(&self) -> Option<Side> {
        self.slides.then_some(self.side)
    }

    fn moved(&mut self, left: bool, direction: isize, rows: &[Row]) -> Went {
        let column = if left {
            &mut self.left
        } else {
            &mut self.right
        };
        if column.step(rows, direction) {
            Went::Moved { left }
        } else {
            Went::Nothing
        }
    }

    /// Put the light on row `index` of the column on `side`, which the
    /// pointer moved onto (`hovered`) or clicked. `None` where it cannot
    /// stand there: a row that cannot be chosen — or, for a hover, the other
    /// column of a pair that slides, since crossing over slides the view
    /// and everything under the pointer with it; a click crosses.
    pub fn point_at(
        &mut self,
        side: Side,
        index: usize,
        left_rows: &[Row],
        right_rows: &[Row],
        hovered: bool,
    ) -> Option<Pointed> {
        if hovered && self.slides && side != self.side {
            return None;
        }
        let was = self.left.at;
        let landed = match side {
            Side::Left => self.left.point_at(left_rows, index),
            Side::Right => self.right.point_at(right_rows, index),
        };
        if !landed {
            return None;
        }
        self.side = side;
        Some(Pointed {
            left_moved: side == Side::Left && self.left.at != was,
        })
    }

    /// The side and row a spot number is, if it is a row of either column.
    pub fn row_of(
        id: u32,
        ids: [spots::Column; 2],
        left_rows: &[Row],
        right_rows: &[Row],
    ) -> Option<(Side, usize)> {
        if let Some(index) = ids[0].row_of(id, left_rows.len()) {
            return Some((Side::Left, index));
        }
        ids[1]
            .row_of(id, right_rows.len())
            .map(|index| (Side::Right, index))
    }

    /// Point the light at the column a wheel was turned over, before the
    /// directions it turned into are acted on.
    pub fn aim(&mut self, scroll: Scroll, ids: [spots::Column; 2]) {
        if Column::owns(scroll.spot, ids[0]) {
            self.side = Side::Left;
        } else if Column::owns(scroll.spot, ids[1]) {
            self.side = Side::Right;
        }
    }

    /// Draw the pair in `room`, each column under its heading, the light in
    /// whichever holds it when `light` is handed in.
    ///
    /// `reserve` keeps that much of the right-hand column's top, under its
    /// heading, for the screen to draw in (the disk's bar over its places),
    /// and `foot` that much of the left-hand column's foot (Where to
    /// install's Advanced, on a display standing on its side, under the disks
    /// and sliding with them); each answered as a rectangle where the slide
    /// has put it, with the room the pair was cut to, so what is drawn there
    /// can be cut the same way. `corners` says how each column's rows are
    /// rounded (Where to install's disks are cards whatever their names
    /// say).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        page: &mut Page,
        room: [f32; 4],
        headings: [&str; 2],
        left_rows: &[Row],
        right_rows: &[Row],
        light: Option<&mut Light>,
        ids: [spots::Column; 2],
        corners: [Corners; 2],
        pressing: Option<f32>,
        dt: f32,
        reserve: f32,
        foot: f32,
    ) -> Reserved {
        let (left_w, laid) = beside(page, room);
        self.slides = laid.slides();
        let pan = self.slide.follow(laid.target(self.side == Side::Right), dt);
        let pan = pan.clamp(0.0, laid.reach.max(0.0));
        let heading = page.line(Text::Caption);
        let gap = page.metric(Metric::Gap);
        let top = room[1] + heading + gap * 0.6;
        let tall = (room[3] - heading - gap * 0.6).max(1.0);
        let left_x = room[0] - pan;
        let right_x = room[0] + laid.page_x - pan;
        let foot = foot.clamp(0.0, tall * 0.5);
        let left_view = [left_x, top, left_w, (tall - foot).max(1.0)];
        let reserve = reserve.clamp(0.0, tall * 0.5);
        let right_view = [
            right_x,
            top + reserve,
            laid.page_w,
            (tall - reserve).max(1.0),
        ];
        let pad = self.pad;

        let from = page.ui().written();
        for (x, width, words) in [
            (left_x, left_w, headings[0]),
            (right_x, laid.page_w, headings[1]),
        ] {
            // A heading belongs to a column in view. Over the strip of a
            // column peeking in at the edge it would be a letter or two cut
            // in half, so it fades out as its words leave the room.
            let ui = page.ui();
            let wide = ui.measure(Text::Caption, words).min(width).max(1.0);
            let seen = (room[0] + room[2]).min(x + wide) - room[0].max(x);
            let shown = (seen / wide).clamp(0.0, 1.0);
            let alpha = ((shown - 0.6) / 0.4).clamp(0.0, 1.0);
            text::label_faded(
                ui,
                [x, room[1], width, heading],
                Text::Caption,
                words,
                text::LESSER.0,
                text::LESSER.1 * alpha,
                Align::Left,
            );
        }
        let (left_light, right_light) = match (light, self.side) {
            (Some(light), Side::Left) => (Some(light), None),
            (Some(light), Side::Right) => (None, Some(light)),
            (None, _) => (None, None),
        };
        let look = |side: usize| Look {
            ids: ids[side],
            pressing,
            dt,
            pad,
            bounds: room,
            corners: corners[side],
        };
        self.left
            .draw(page, left_view, left_rows, left_light, look(0));
        self.right
            .draw(page, right_view, right_rows, right_light, look(1));
        let to = page.ui().written();
        // Everything slid out past the panel's room is cut away, so a column
        // sliding out of view goes behind the edge rather than over it.
        let air = page.metric(Metric::RowHeight) * 0.5;
        let cut = [room[0], room[1] - air, room[2], room[3] + air * 2.0];
        if laid.slides() {
            page.ui().cut_between(from, to, cut);
        }
        Reserved {
            band: [right_x, top, laid.page_w, reserve],
            foot: [left_x, top + tall - foot, left_w, foot],
            cut: laid.slides().then_some(cut),
        }
    }

    /// What the pointer did to either column: a hand on a bar, which takes
    /// the light into that column, or a click on a row, which the screen
    /// answers (`Pick::Clicked`).
    pub fn pressed(
        &mut self,
        page: &mut Page,
        left_rows: &[Row],
        right_rows: &[Row],
        ids: [spots::Column; 2],
    ) -> Option<(Side, Pick)> {
        for (side, column, rows, ids) in [
            (Side::Left, &mut self.left, left_rows, ids[0]),
            (Side::Right, &mut self.right, right_rows, ids[1]),
        ] {
            if let Some(pick) = column.pressed(page, rows, ids) {
                if let Pick::Moved(_) = pick {
                    self.side = side;
                }
                return Some((side, pick));
            }
        }
        None
    }

    /// Put everything where it is going, for a photograph.
    pub fn settle(&mut self) {
        self.left.settle();
        self.right.settle();
        self.slide.place_again();
    }

    /// Whether anything in the pair is still moving.
    pub fn moving(&self) -> bool {
        self.left.moving() || self.right.moving() || self.slide.moving()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(count: usize) -> Vec<Row> {
        (0..count)
            .map(|index| Row::new(format!("{index}")))
            .collect()
    }

    #[test]
    fn the_light_goes_on_into_the_second_column_and_back_out_of_it() {
        let (left, right) = (rows(3), rows(4));
        let mut pair = Pair::default();
        assert_eq!(
            pair.act(Action::Down, &left, &right),
            Went::Moved { left: true }
        );
        assert_eq!(
            pair.act(Action::Accept, &left, &right),
            Went::Crossed {
                to: Side::Right,
                pressed: true
            }
        );
        assert_eq!(
            pair.act(Action::Down, &left, &right),
            Went::Moved { left: false }
        );
        assert_eq!(pair.act(Action::Accept, &left, &right), Went::Chose(1));
        assert_eq!(pair.act(Action::Back, &left, &right), Went::Backed);
        assert_eq!(pair.side, Side::Left);
        assert_eq!(pair.act(Action::Back, &left, &right), Went::Pass);
    }

    #[test]
    fn an_empty_second_column_cannot_be_entered() {
        let left = rows(2);
        let none: Vec<Row> = vec![Row::new("blocked").enabled(false)];
        let mut pair = Pair::default();
        assert_eq!(pair.act(Action::Right, &left, &none), Went::Nothing);
        assert_eq!(pair.side, Side::Left);
    }

    #[test]
    fn a_wide_panel_keeps_both_columns_in_view_and_a_narrow_one_slides() {
        let (left, wide) = beside_at(1200.0);
        assert!(!wide.slides());
        assert_eq!(left, LEFT, "a landscape panel's column changed");
        let (_, narrow) = beside_at(700.0);
        assert!(narrow.slides());
        assert!(narrow.page_w >= LEAST.min(700.0 * (1.0 - layout::PEEK)));
    }

    /// On a display standing on its side the places' column peeks in at the
    /// panel's edge while the disks' column has the light — the toolkit's
    /// `PEEK` of the room — and slides fully into view when the light goes
    /// there. The rooms are those of a 9:16 and a 10:16 panel (1080×1920,
    /// 720×1280 and 1440×2560 alike; 800×1280 and 1200×1920).
    #[test]
    fn on_a_display_standing_on_its_side_the_places_peek_in() {
        for room in [471.5, 539.0] {
            let (left, laid) = beside_at(room);
            assert!(laid.slides(), "{room}");
            let peek = room - laid.page_x;
            assert!(
                (peek - room * layout::PEEK).abs() < 0.01,
                "{room}: the places peek in by {peek}"
            );
            assert!((LEFT_LEAST..LEFT).contains(&left), "{room}: {left}");
            // Slid over, the places' column is in view with a strip of the
            // disks' at the left.
            assert!(laid.page_x + laid.page_w - laid.reach <= room + 0.01);
            assert!(laid.page_x - laid.reach > 0.0);
        }
        // Too narrow even for that: the column keeps its least width.
        assert_eq!(left_width(300.0), LEFT_LEAST);
    }

    #[test]
    fn hovering_lights_a_row_of_either_column_where_both_stand_still() {
        let (left, right) = (rows(3), rows(4));
        let mut pair = Pair::default();
        let went = pair.point_at(Side::Left, 2, &left, &right, true);
        assert_eq!(went, Some(Pointed { left_moved: true }));
        assert_eq!(pair.left.at, 2);
        let went = pair.point_at(Side::Right, 3, &left, &right, true);
        assert_eq!(went, Some(Pointed { left_moved: false }));
        assert_eq!((pair.side, pair.right.at), (Side::Right, 3));
        let blocked = vec![Row::new("blocked").enabled(false)];
        assert_eq!(pair.point_at(Side::Left, 0, &blocked, &right, true), None);
        assert_eq!(
            pair.side,
            Side::Right,
            "a row that cannot be chosen took the light"
        );
    }

    #[test]
    fn where_the_pair_slides_a_hover_stays_in_its_column_and_a_click_crosses() {
        let (left, right) = (rows(3), rows(4));
        let mut pair = Pair {
            slides: true,
            ..Pair::default()
        };
        assert_eq!(pair.point_at(Side::Right, 1, &left, &right, true), None);
        assert_eq!(pair.side, Side::Left);
        assert!(pair.point_at(Side::Left, 1, &left, &right, true).is_some());
        assert!(pair
            .point_at(Side::Right, 1, &left, &right, false)
            .is_some());
        assert_eq!((pair.side, pair.right.at), (Side::Right, 1));
    }

    #[test]
    fn only_a_pair_that_slides_says_which_column_it_is_slid_to() {
        let (left, right) = (rows(3), rows(4));
        let mut wide = Pair::default();
        wide.act(Action::Accept, &left, &right);
        assert_eq!((wide.side, wide.slid_to()), (Side::Right, None));
        let mut narrow = Pair {
            slides: true,
            ..Pair::default()
        };
        assert_eq!(narrow.slid_to(), Some(Side::Left));
        narrow.act(Action::Accept, &left, &right);
        assert_eq!(narrow.slid_to(), Some(Side::Right));
    }
}
