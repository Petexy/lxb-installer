//! A page giving way to the next one: how every step, every page of the
//! Desktop step's Options and the end of an installation come in.
//!
//! **LineXinBar's step into a subcategory, a page at a time.** The shell walks
//! into its Settings pages and out of them by stepping its bar a column at a
//! time (`model.rs`, `Cursor::animate`; `ui.rs`, `receded`, `departing`):
//! going in, the column opened comes in from the right as the one it was
//! opened from slides left and steps back — smaller, hazier; coming back, the
//! column being left goes back out to the right as the one behind it comes
//! forward again. The installer's pages step the same way:
//!
//! * **Continue, or into an Options page** — the new page comes in from a
//!   step to the right while the page left slides a step to the left and
//!   recedes ([`motion::receded`]);
//! * **Back, or out of an Options page** — the page left goes back out a step
//!   to the right while the page returned to comes forward from a step to the
//!   left, out of the recession.
//!
//! **Two pages are never read over one another.** The shell's column comes
//! in a step clear of the one it leaves; a page comes in over the whole of
//! the page going, in the same panel. So their ink crosses as the shell's
//! two sets of columns cross where they do share the screen — its search
//! gathering the bar's row (`ui.rs`, `home_alpha` and `found_alpha`), whose
//! first half is the one leaving and second half the one arriving, "so the
//! rows of the one are never printed over the rows of the other": the page
//! going has given up all of its ink by [`GONE`] of the way ([`going`]), and
//! the words and marks of the page coming in come up only from as far from
//! the end ([`coming`]). The glass of the page coming in comes up over the
//! whole of the way, so the panel is never empty between the two.
//!
//! What a page carries is the whole of it: its head (the mark, the title, the
//! caption) and everything in its room. What does not change from one page to
//! the next does not move at all — Continue, the legend, the panel itself —
//! and the row of chips in the head says which step this is by its own
//! motion (`panel::Chips`). The step rides the shell's own depth spring
//! ([`STEP_SPRING`]), gentler than its rows': a page is a longer journey than
//! a row, which is why the shell steps its columns more gently than its rows.
//!
//! Nothing here draws: it says where each page stands on this frame, and the
//! frame carries what it drew there (`Ui::carry_between`). In low-end mode
//! nothing is begun at all, and a page changes at once, as it always did.

use lxb_app::lxb_render::Pressing;
use lxb_app::lxb_toolkit::motion as toolkit;

use super::motion::{receded, Light, Share};
use super::screens::{Shown, Stood};

/// How far a page travels as it comes in or goes, as a share of the panel's
/// width: the shell's own step into a subcategory (`SUBCOLUMN_STEP`, about 321
/// reference pixels: its bar's cross at 22% of a 16:9 display, less the
/// column's margin) against the installer's panel at the same height (1280
/// reference pixels) — a quarter of it. Measured against the panel rather than
/// fixed in pixels, so a display standing on its side steps a page as far
/// across itself as a landscape one does.
pub const TRAVEL: f32 = 0.25;

/// How stiff the spring a page steps on is, in radians a second: the shell's
/// `DEPTH_EASE_RATE`, the rate its bar steps into a subcategory and out of it
/// at (`model.rs`; its file picker and its transfers step their columns at it
/// too). Gentler than its rows' 19 — the toolkit's `CARD_SPRING` — for the
/// shell's own reason: a spring rate is in units a second, a column is a wider
/// journey than a row, and driven at a row's rate it "would arrive faster than
/// anything else the bar does, which reads as a jump rather than as a move".
/// The toolkit has no spring this gentle, so it is copied here, as the
/// recession is ([`motion::SHRINK`](super::motion::SHRINK)).
pub const STEP_SPRING: f64 = 14.0;

/// Which way the walk went: on (Continue, an Options page opened, the
/// installation followed to its end) or back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    On,
    Back,
}

impl Way {
    /// The way from `from` to `to`, by where each stands in the walk.
    pub fn between(from: Shown, to: Shown) -> Self {
        if to.order() < from.order() {
            Way::Back
        } else {
            Way::On
        }
    }

    /// The other way.
    fn turned(self) -> Self {
        match self {
            Way::On => Way::Back,
            Way::Back => Way::On,
        }
    }
}

/// Where a page stands on one frame of a change, which the frame turns into
/// a `Carry` for what it drew.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placing {
    /// How far across it is from where it rests, in travels (one is
    /// [`TRAVEL`] of the panel to the right).
    pub across: f32,
    /// How large it is drawn about its own middle.
    pub scale: f32,
    /// How much of it is left: its glass, and everything on it with it.
    pub fade: f32,
    /// How much of its words and marks is left besides (`Carry::words`).
    pub words: f32,
}

impl Placing {
    /// Where it rests, whole.
    pub const STANDING: Self = Self {
        across: 0.0,
        scale: 1.0,
        fade: 1.0,
        words: 1.0,
    };

    /// This placing, and then `then` from there: a page that was still on its
    /// way in when it was asked to go goes from where it had got to.
    pub fn then(self, then: Placing) -> Self {
        Self {
            across: self.across + then.across,
            scale: self.scale * then.scale,
            fade: self.fade * then.fade,
            words: self.words * then.words,
        }
    }

    /// The same place, with `ink` of it left.
    fn inked(self, ink: Ink) -> Self {
        Self {
            fade: self.fade * ink.glass,
            words: self.words * ink.words,
            ..self
        }
    }

    /// Whether it stands where it rests, whole.
    pub fn standing(self) -> bool {
        self == Self::STANDING
    }
}

/// Where the page coming in stands, `through` of the way through a change
/// that went `way`: how far across, how large, and how hazy for the depth it
/// comes forward out of. How much of its ink has come up is [`coming`]'s.
pub fn arriving(through: f32, way: Way) -> Placing {
    let left = 1.0 - through.clamp(0.0, 1.0);
    match way {
        Way::On => Placing {
            across: left,
            ..Placing::STANDING
        },
        Way::Back => {
            let (scale, haze) = receded(left);
            Placing {
                across: -left,
                scale,
                fade: haze,
                words: 1.0,
            }
        }
    }
}

/// Where the page going stands, `through` of the way through a change that
/// went `way`. How much of its ink is left is [`going`]'s.
pub fn leaving(through: f32, way: Way) -> Placing {
    let through = through.clamp(0.0, 1.0);
    match way {
        Way::On => {
            let (scale, haze) = receded(through);
            Placing {
                across: -through,
                scale,
                fade: haze,
                words: 1.0,
            }
        }
        Way::Back => Placing {
            across: through,
            ..Placing::STANDING
        },
    }
}

/// How much of a page is drawn: its glass (and everything on it), and its
/// words and marks besides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ink {
    pub glass: f32,
    pub words: f32,
}

impl Ink {
    pub const WHOLE: Self = Self {
        glass: 1.0,
        words: 1.0,
    };
    pub const NONE: Self = Self {
        glass: 0.0,
        words: 0.0,
    };
}

/// How little of its words a page may keep and still count as having none to
/// give up.
const NO_WORDS: f32 = 1e-3;

/// How much of the way the page going has given up all of its ink by — and
/// how far from the end the words of the page coming in begin to come up:
/// the shell's own crossing of two sets of columns in one place, its search
/// (`smoothstep(0.0, 0.45, gather)` going, `smoothstep(0.55, 1.0, gather)`
/// coming), so that between the two neither page's words are written.
pub const GONE: f32 = 0.45;

/// What is left of the page going's ink, `through` of the way through the
/// ink's change, from `from` where it began: none once [`GONE`] of the way
/// has gone, its words with its glass.
pub fn going(through: f32, from: Ink) -> Ink {
    let left = 1.0 - toolkit::smoothstep(through / GONE);
    Ink {
        glass: from.glass * left,
        words: from.words * left,
    }
}

/// How much of the page coming in's ink has come up, `through` of the way
/// through the ink's change, from `from` where it began. Its glass comes up
/// over the whole of the way, with the spring, so the panel is never empty;
/// its words and marks come up only once the page going has given up its
/// own — over the last [`GONE`] of the way (`after`) — and, where the page
/// going had none left to give up, over the first [`GONE`] of it, as fast as
/// a page going gives its own up: nothing is waited for. Brought up over the
/// whole of the way instead, a page called back after its words had gone
/// was a blank glass for nine frames, where before pages crossed at all its
/// words were back in six.
pub fn coming(through: f32, from: Ink, after: bool) -> Ink {
    let through = through.clamp(0.0, 1.0);
    let up = if after {
        toolkit::smoothstep((through - (1.0 - GONE)) / GONE)
    } else {
        toolkit::smoothstep(through / GONE)
    };
    Ink {
        glass: from.glass + (1.0 - from.glass) * through,
        words: from.words + (1.0 - from.words) * up,
    }
}

/// The page going, while it is still drawn.
#[derive(Debug, Clone, Copy)]
struct Leaving {
    /// Which page it is, as it stood.
    page: Stood,
    /// Where it stood when it began to go: where it rests, or — asked to go
    /// before it had come all the way in — where it had got to.
    began: Placing,
}

/// Where each page's ink stood when the ink's change began: from nothing and
/// whole when a page gives way to another, and from wherever each had got to
/// when a change turns round or a page goes before it had come in.
#[derive(Debug, Clone, Copy)]
struct Inks {
    coming: Ink,
    going: Ink,
    /// Whether the page going had words to give up, which the words of the
    /// page coming in wait for.
    after: bool,
}

/// What the page called back by a change that turned round was drawn with as
/// it went, and takes back as it comes in again ([`Change::begin`]).
#[derive(Debug, Clone, Copy)]
pub struct Called {
    /// Its light, where it had left it on what it lit: the light it comes
    /// back with, rather than one put down afresh — put down afresh, it
    /// jumped onto its row in one frame, at its full size where it had been
    /// dipping.
    pub light: Light,
    /// The press it was dipping under, if a press sent it away.
    pub pressing: Pressing,
}

/// The change of page under way, if one is.
#[derive(Debug, Clone)]
pub struct Change {
    leaving: Option<Leaving>,
    /// The page going's own light, left on what it lit (`Light::kept`).
    light: Light,
    /// The page going's own press: the one that sent it away dips the
    /// control it landed on, on the page it was on, not whatever the light
    /// stands on in the page that comes in.
    pressing: Pressing,
    /// How far the pages have got to where they are going.
    through: Share,
    way: Way,
    /// How far their ink has got: begun with the change, and again when it
    /// turns round, where the pages go on from where they stand.
    ink: Share,
    inks: Inks,
}

impl Default for Change {
    fn default() -> Self {
        Self {
            leaving: None,
            light: Light::default(),
            pressing: Pressing::default(),
            through: Share::default(),
            way: Way::On,
            ink: Share::default(),
            inks: Inks {
                coming: Ink::WHOLE,
                going: Ink::NONE,
                after: false,
            },
        }
    }
}

impl Change {
    /// The page `from` gives way to `to`, the one now drawn, the walk having
    /// gone `way`; `light` is the light as it stood on `from`, and `pressing`
    /// the press under way there — Accept on Continue sends a page away
    /// dipping the button pressed, which goes with it.
    ///
    /// **Asked back to the page that is going, the change turns round where
    /// it stands** — Continue and then Back at once, an easy slip. Each page
    /// goes back from where it is, at the speed it had, as the shell's step
    /// does when the way it is going changes before it has arrived (the
    /// spring keeps its place and its speed and is given the other end):
    /// the two pages stand in the same places going one way as the other
    /// (`arriving` and `leaving` mirror each other). Their ink goes on from
    /// what each had: the page called back comes up from what it had kept,
    /// never back down to nothing first, and the page it gives way to gives
    /// up what it had brought in — the words of the one only once the
    /// other's have gone, and straight away where the other had written
    /// none yet ([`coming`]). Letting the page that was going go at once and
    /// bringing it back in from nothing blinked it out for a frame; mending
    /// its ink along the way of the turned spring, which first carries on the
    /// way it was going, left it without any for three.
    ///
    /// Otherwise a page still coming in goes from where it had got to, and
    /// one that was going is let go of.
    ///
    /// Answers, where the change turned round, what the page called back was
    /// drawn with as it went, which it takes back.
    pub fn begin(
        &mut self,
        from: Stood,
        to: Shown,
        light: Light,
        pressing: Pressing,
        way: Way,
    ) -> Option<Called> {
        let turning = self.moving()
            && self
                .leaving
                .is_some_and(|going| going.page.page == to && going.began.standing());
        if turning {
            let coming = self.coming_ink();
            let going = self.going_ink();
            self.through.reverse();
            self.way = self.way.turned();
            self.inks = Inks {
                coming: going,
                going: coming,
                after: coming.words > NO_WORDS,
            };
            self.ink.begin();
            self.leaving = Some(Leaving {
                page: from,
                began: Placing::STANDING,
            });
            let called = Called {
                light: std::mem::replace(&mut self.light, light.kept()),
                pressing: std::mem::replace(&mut self.pressing, pressing),
            };
            return Some(called);
        }
        let (began, kept) = if self.moving() {
            (arriving(self.through.at(), self.way), self.coming_ink())
        } else {
            (Placing::STANDING, Ink::WHOLE)
        };
        self.leaving = Some(Leaving { page: from, began });
        self.light = light.kept();
        self.pressing = pressing;
        self.way = way;
        self.inks = Inks {
            coming: Ink::NONE,
            going: kept,
            after: kept.words > NO_WORDS,
        };
        self.through.begin();
        self.ink.begin();
        None
    }

    /// Carry the change one frame's worth on, and let the page going go once
    /// it has arrived.
    pub fn advance(&mut self, dt: f32) {
        self.through.advance(STEP_SPRING, dt);
        self.ink.advance(STEP_SPRING, dt);
        if !self.moving() {
            self.leaving = None;
            self.pressing = Pressing::default();
        }
    }

    /// How much of the page drawn now's ink has come up.
    fn coming_ink(&self) -> Ink {
        coming(self.ink.at(), self.inks.coming, self.inks.after)
    }

    /// How much of the page going's ink is left.
    fn going_ink(&self) -> Ink {
        going(self.ink.at(), self.inks.going)
    }

    /// Where the page drawn now stands.
    pub fn arriving(&self) -> Placing {
        if !self.moving() {
            return Placing::STANDING;
        }
        arriving(self.through.at(), self.way).inked(self.coming_ink())
    }

    /// The page going and where it stands, while it is still drawn.
    pub fn leaving(&self) -> Option<(Stood, Placing)> {
        let (through, way) = (self.through.at(), self.way);
        let ink = self.going_ink();
        self.leaving.map(|going| {
            let at = going.began.then(leaving(through, way)).inked(ink);
            (going.page, at)
        })
    }

    /// The page going's own light, which it is drawn with.
    pub fn leaving_light(&mut self) -> &mut Light {
        &mut self.light
    }

    /// The page going's own press, which it is drawn dipping under.
    pub fn leaving_pressing(&mut self) -> &mut Pressing {
        &mut self.pressing
    }

    /// How far through the change the pages stand, from nought to one (one
    /// with none).
    pub fn through(&self) -> f32 {
        self.through.at()
    }

    /// Put the change at its end: the page going gone, the one drawn now
    /// where it rests.
    pub fn settle(&mut self) {
        self.through.settle();
        self.ink.settle();
        self.leaving = None;
        self.pressing = Pressing::default();
    }

    /// Whether a change is under way, and so wants another frame.
    pub fn moving(&self) -> bool {
        self.through.moving() || self.ink.moving()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::motion::{FRAME, HAZE, SHRINK};
    use crate::ui::screens::desktop::Part;
    use crate::ui::Screen;

    fn page(screen: Screen) -> Shown {
        Shown {
            screen,
            part: Part::Main,
            partitions: false,
        }
    }

    #[test]
    fn continue_brings_the_next_page_from_the_right_and_back_from_the_left() {
        let (timezone, keyboard) = (page(Screen::Timezone), page(Screen::Keyboard));
        assert_eq!(Way::between(timezone, keyboard), Way::On);
        assert_eq!(Way::between(keyboard, timezone), Way::Back);
        assert!(arriving(0.0, Way::On).across > 0.0, "came in from the left");
        assert!(
            arriving(0.0, Way::Back).across < 0.0,
            "came back from the right"
        );
        assert!(leaving(0.5, Way::On).across < 0.0, "went on to the right");
        assert!(
            leaving(0.5, Way::Back).across > 0.0,
            "went back to the left"
        );
        for way in [Way::On, Way::Back] {
            assert!(arriving(1.0, way).standing(), "{way:?} did not arrive");
            assert_eq!(leaving(0.0, way), Placing::STANDING, "{way:?} jumped");
        }
        assert_eq!(coming(0.0, Ink::NONE, true), Ink::NONE, "came in whole");
        assert_eq!(coming(1.0, Ink::NONE, true), Ink::WHOLE, "never came in");
        assert_eq!(going(0.0, Ink::WHOLE), Ink::WHOLE, "faded as it set off");
        assert_eq!(going(1.0, Ink::WHOLE), Ink::NONE, "never went");
        // Going on, the page left steps back as the bar's column does; going
        // back, the page returned to comes forward out of that same depth.
        let back = leaving(1.0, Way::On);
        assert!((back.scale - SHRINK).abs() < 1e-6);
        assert!((back.fade - HAZE).abs() < 1e-6);
        let returned = arriving(0.0, Way::Back);
        assert!((returned.scale - SHRINK).abs() < 1e-6);
        assert!((arriving(0.5, Way::Back).fade - HAZE.sqrt()).abs() < 1e-5);
    }

    #[test]
    fn the_page_going_clears_before_the_words_of_the_page_coming_in_come_up() {
        for step in 0..=100 {
            let through = step as f32 / 100.0;
            let (old, new) = (going(through, Ink::WHOLE), coming(through, Ink::NONE, true));
            // The first half is the page going's, the second the words of
            // the page coming in: never two pages' words at once.
            assert!(
                old.words.min(new.words) < 1e-6,
                "at {through}: words {} over words {}",
                old.words,
                new.words
            );
            if through >= GONE {
                assert!(old.glass < 1e-6, "at {through} the page going is there");
            }
            // The glass of the page coming in comes up the whole way, so the
            // panel is never empty between the two.
            assert!(old.glass + new.glass > 0.3, "at {through}: an empty panel");
            assert!((new.glass - through).abs() < 1e-6);
        }
        // With no words left on the page going, nothing is waited for.
        let alone = coming(0.25, Ink::NONE, false);
        assert!(alone.words > 0.1, "its words waited for nothing");
    }

    #[test]
    fn a_change_arrives_and_then_its_frames_are_still() {
        let mut change = Change::default();
        assert!(!change.moving(), "a change nobody began is under way");
        assert!(change.arriving().standing());
        change.begin(
            page(Screen::Language).into(),
            page(Screen::Timezone),
            Light::default(),
            Pressing::default(),
            Way::On,
        );
        let mut frames = 0;
        let mut last = change.arriving();
        while change.moving() {
            change.advance(FRAME);
            frames += 1;
            let now = change.arriving();
            assert!(now.across <= last.across, "the page came in and went back");
            last = now;
            assert!(frames < 90, "still changing after a second and a half");
        }
        assert!(frames > 12, "{frames} frames is a jump, not a change");
        assert!(change.arriving().standing());
        assert!(change.leaving().is_none(), "the page left is still drawn");
        // A settled change is still: frame after frame draws the same.
        for _ in 0..30 {
            change.advance(FRAME);
            assert!(!change.moving());
            assert!(change.arriving().standing());
        }
    }

    #[test]
    fn a_page_asked_to_go_on_its_way_in_goes_from_where_it_had_got_to() {
        let mut change = Change::default();
        change.begin(
            page(Screen::Language).into(),
            page(Screen::Timezone),
            Light::default(),
            Pressing::default(),
            Way::On,
        );
        for _ in 0..4 {
            change.advance(FRAME);
        }
        let halfway = change.arriving();
        assert!(halfway.across > 0.0 && halfway.fade < 1.0);
        // Continue again before the page had come in: it begins to go from
        // where it stood, not from where it would have rested.
        change.begin(
            page(Screen::Timezone).into(),
            page(Screen::Keyboard),
            Light::default(),
            Pressing::default(),
            Way::On,
        );
        let (going, at) = change.leaving().expect("the page going");
        assert_eq!(going.page, page(Screen::Timezone));
        assert_eq!(at, halfway, "the page going jumped as it set off");
        assert_eq!(change.arriving().fade, 0.0, "the new page came in whole");
    }

    #[test]
    fn back_at_once_after_continue_turns_the_change_round_where_it_stands() {
        let (timezone, keyboard) = (page(Screen::Timezone), page(Screen::Keyboard));
        for frames in [1, 3, 6, 12] {
            let mut change = Change::default();
            change.begin(
                timezone.into(),
                keyboard,
                Light::default(),
                Pressing::default(),
                Way::On,
            );
            for _ in 0..frames {
                change.advance(FRAME);
            }
            let (_, was_going) = change.leaving().expect("Time zone going");
            let was_coming = change.arriving();
            // Back, before Keyboard had come in: Time zone is asked for again.
            change.begin(
                keyboard.into(),
                timezone,
                Light::default(),
                Pressing::default(),
                Way::Back,
            );
            let (going, now_going) = change.leaving().expect("Keyboard going");
            assert_eq!(going.page, keyboard, "after {frames} frames");
            let now_coming = change.arriving();
            for (was, now, which) in [
                (was_going, now_coming, "Time zone"),
                (was_coming, now_going, "Keyboard"),
            ] {
                assert!(
                    (was.across - now.across).abs() < 1e-5
                        && (was.scale - now.scale).abs() < 1e-5
                        && (was.fade - now.fade).abs() < 1e-5,
                    "{which} jumped as the change turned after {frames} frames: \
                     {was:?} to {now:?}"
                );
            }
            // And it goes on from there, smoothly, to Time zone standing.
            let mut last = (now_coming, now_going);
            let mut count = 0;
            while change.moving() {
                change.advance(FRAME);
                count += 1;
                let coming = change.arriving();
                let going = change.leaving().map_or(Placing::STANDING, |(_, at)| at);
                assert!((coming.across - last.0.across).abs() < 0.2, "a jump");
                assert!((coming.fade - last.0.fade).abs() < 0.3, "a blink");
                if change.moving() {
                    assert!((going.fade - last.1.fade).abs() < 0.3, "a blink");
                }
                last = (coming, going);
                assert!(count < 90, "never came back");
            }
            assert!(change.arriving().standing());
            assert!(change.leaving().is_none());
        }
    }

    /// How strongly a page's words are written where it stands.
    fn written(at: Placing) -> f32 {
        at.fade * at.words
    }

    #[test]
    fn two_pages_never_write_at_once_frame_by_frame() {
        let (timezone, keyboard) = (page(Screen::Timezone), page(Screen::Keyboard));
        for (from, to, way) in [
            (timezone, keyboard, Way::On),
            (keyboard, timezone, Way::Back),
        ] {
            let mut change = Change::default();
            change.begin(from.into(), to, Light::default(), Pressing::default(), way);
            let mut frames = 0;
            while change.moving() {
                let coming = change.arriving();
                let (_, going) = change.leaving().expect("the page going");
                assert!(
                    written(coming).min(written(going)) < 0.02,
                    "{way:?}, frame {frames}: words at {} over words at {}",
                    written(going),
                    written(coming)
                );
                assert!(
                    coming.fade + going.fade > 0.3,
                    "{way:?}, frame {frames}: an empty panel"
                );
                change.advance(FRAME);
                frames += 1;
            }
        }
    }

    #[test]
    fn a_page_called_back_never_goes_without_its_ink() {
        let (language, timezone) = (page(Screen::Language), page(Screen::Timezone));
        for frames in 1..=24 {
            let mut change = Change::default();
            change.begin(
                language.into(),
                timezone,
                Light::default(),
                Pressing::default(),
                Way::On,
            );
            for _ in 0..frames {
                change.advance(FRAME);
            }
            let (_, was) = change.leaving().expect("Language going");
            change.begin(
                timezone.into(),
                language,
                Light::default(),
                Pressing::default(),
                Way::Back,
            );
            let mut after = 0;
            while change.moving() {
                let back = change.arriving();
                let (_, going) = change.leaving().expect("Time zone going");
                // Called back, Language comes up from what it had kept and
                // is never put out first, whatever the turned spring does.
                assert!(
                    back.fade >= was.fade * 0.9,
                    "turned after {frames}: Language at {} {after} frames on, \
                     from {}",
                    back.fade,
                    was.fade
                );
                assert!(
                    written(back).min(written(going)) < 0.02,
                    "turned after {frames}, {after} frames on: words at {} over {}",
                    written(going),
                    written(back)
                );
                change.advance(FRAME);
                after += 1;
                assert!(after < 120, "turned after {frames}: never came back");
            }
            assert!(change.arriving().standing());
        }
    }

    #[test]
    fn a_page_called_back_writes_its_words_again_as_soon_as_the_other_has_none() {
        let (language, timezone) = (page(Screen::Language), page(Screen::Timezone));
        // How strongly a page's words must be written to be read.
        const READ: f32 = 0.3;
        for frames in 1..=24 {
            let mut change = Change::default();
            change.begin(
                language.into(),
                timezone,
                Light::default(),
                Pressing::default(),
                Way::On,
            );
            for _ in 0..frames {
                change.advance(FRAME);
            }
            let written_there = written(change.arriving());
            change.begin(
                timezone.into(),
                language,
                Light::default(),
                Pressing::default(),
                Way::Back,
            );
            let mut after = 0;
            while written(change.arriving()) < READ {
                change.advance(FRAME);
                after += 1;
            }
            // Where Time zone had written nothing yet, nothing is waited for:
            // Language's words are back as soon as a page going would have
            // given its own up — not a blank glass for a tenth of a second
            // and more, which was later than before pages crossed at all.
            if written_there < 1e-3 {
                assert!(
                    after <= 6,
                    "turned after {frames}: Language's words read {after} frames on"
                );
            } else {
                assert!(
                    after <= 14,
                    "turned after {frames}, Time zone written: Language's words \
                     read {after} frames on"
                );
            }
        }
    }

    #[test]
    fn a_page_called_back_comes_back_with_its_own_light() {
        let (language, timezone) = (page(Screen::Language), page(Screen::Timezone));
        let row = [40.0, 300.0, 600.0, 48.0];
        let mut on_language = Light::default();
        on_language.glide(row, FRAME);
        let mut change = Change::default();
        let fresh = change.begin(
            language.into(),
            timezone,
            on_language,
            Pressing::default(),
            Way::On,
        );
        assert!(
            fresh.is_none(),
            "a change nobody turned gave something back"
        );
        // Language goes, drawing its light where it left it.
        change.leaving_light().glide(row, FRAME);
        change.advance(FRAME);
        let mut on_timezone = Light::default();
        on_timezone.glide([200.0, 500.0, 400.0, 64.0], FRAME);
        // Back: Time zone goes with the installer's light, and Language
        // comes back with its own, still on its row — not one put down
        // afresh wherever it is asked to go.
        let called = change
            .begin(
                timezone.into(),
                language,
                on_timezone,
                Pressing::default(),
                Way::Back,
            )
            .expect("the change turned round");
        let mut light = called.light;
        let next = light.glide([40.0, 360.0, 600.0, 48.0], FRAME);
        assert!(
            (next[1] - 300.0).abs() < 30.0,
            "Language's light jumped to {next:?} as it was called back"
        );
        let going = change
            .leaving_light()
            .glide([200.0, 500.0, 400.0, 64.0], FRAME);
        assert_eq!(
            going,
            [200.0, 500.0, 400.0, 64.0],
            "Time zone's light moved"
        );
    }

    #[test]
    fn settling_ends_the_change_where_everything_rests() {
        let mut change = Change::default();
        change.begin(
            page(Screen::Review).into(),
            page(Screen::Installing),
            Light::default(),
            Pressing::default(),
            Way::On,
        );
        change.advance(FRAME);
        change.settle();
        assert!(!change.moving());
        assert!(change.leaving().is_none());
        assert!(change.arriving().standing());
    }
}
