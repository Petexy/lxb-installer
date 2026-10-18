//! Everything on a screen that is on its way somewhere: the clock it moves
//! by, the light that crosses the page, a value carried on a spring, and how
//! something stepping back into the distance is drawn.
//!
//! Nothing here decides anything and nothing here draws except the light: it
//! turns "the light is on this row now" into "the light is 40% of the way
//! there", so that drawing can ask where something is rather than work it out.
//! Two rules from the design language run through it: easing is never linear
//! (every value goes through a spring or `motion::ease`), and a still frame is
//! still — a spring that has arrived stops, so the window is not redrawn for
//! ever a thousandth of a pixel at a time.
//!
//! **In low-end mode nothing of the installer's own moves.** Where the toolkit
//! counts the session as low-end hardware (`Page::low_end`: the shell's
//! low-end mode, or a window drawn in software) every frame puts everything
//! where it is going (`State::settle`): a page changes at once, as it always
//! did, and so do the light, the columns and the board. A press still dips
//! what it presses (`Pressing`), as the toolkit's own presses dip there, and
//! the toolkit's menus and question still unfold.

use lxb_app::lxb_toolkit::{control, metrics::Metric, motion};
use lxb_app::Page;

/// One frame of the clock at the sixty a second a window is drawn at.
pub const FRAME: f32 = 1.0 / 60.0;

/// How long a frame may count for at most. A window that was not drawn for a
/// second (covered, suspended) must not move everything a second's worth.
const LONGEST: f32 = 0.1;

/// The installer's own clock.
///
/// **Not `Page::seconds` alone.** That is the wallpaper's clock, and in the
/// shell's low-end mode the wallpaper is held still at one instant, so a page
/// timing itself by it would never see a frame go by and every light, scroll
/// and bar would freeze where it stood. A window therefore times itself by the
/// machine's monotonic clock. A picture (`--shot`) is the other way round: it
/// must come out the same every time it is taken, so it times itself by the
/// instant it was given and nothing else.
#[derive(Debug, Clone)]
pub struct Clock {
    shot: bool,
    began: std::time::Instant,
    last: Option<f32>,
    /// Seconds since the clock started (or the instant a picture was asked
    /// for). What blinking and breathing are timed by.
    pub now: f32,
    /// How long the frame being drawn counts for.
    pub dt: f32,
}

impl Clock {
    pub fn new(shot: bool) -> Self {
        Self {
            shot,
            began: std::time::Instant::now(),
            last: None,
            now: 0.0,
            dt: FRAME,
        }
    }

    /// Move on to the frame being drawn now.
    pub fn tick(&mut self, page_seconds: f32) {
        let now = if self.shot {
            page_seconds
        } else {
            self.began.elapsed().as_secs_f32()
        };
        self.dt = match self.last {
            Some(last) => (now - last).clamp(0.0, LONGEST),
            None => FRAME,
        };
        self.last = Some(now);
        self.now = now;
    }

    /// Move on to the frame being drawn now where the instant handed in is
    /// held still: a picture of a low-end window (`App::low_end`), which
    /// the toolkit hands the same instant on every frame, as the window
    /// holds its clock there. A picture counts the sixtieth of a second its
    /// frames are drawn apart, so a press still dips in it as in the window;
    /// a window times itself by the machine's clock either way.
    pub fn tick_held(&mut self, page_seconds: f32) {
        let now = match self.last {
            Some(last) if self.shot => last + FRAME,
            _ => page_seconds,
        };
        self.tick(now);
    }
}

/// How a thing one step further back is drawn: the scale it keeps and the
/// share of its ink that reaches the front. LineXinBar's own recession, its
/// bar's `DEPTH_SHRINK` and `DEPTH_HAZE` — size and ink together, because
/// either alone is something else: a row made small, or a row switched off.
/// A trail's column behind the one in hand recedes by it, and so does a page
/// the next one has come in over.
pub const SHRINK: f32 = 0.86;
pub const HAZE: f32 = 0.72;

/// How much of its way out something on its way out has gone by the time it
/// has given up the last of its ink: a third of it, the bar's
/// `COLUMN_GONE_BY` (the picker's `PICKER_COLUMN_GONE_BY` is the same). The
/// ink goes first and the movement follows, so two lists, or two pages, are
/// never read over one another at half strength each.
pub const GONE_BY: f32 = 0.35;

/// How something `steps` back from the front is drawn: its scale, and the
/// share of its ink that reaches the front.
pub fn receded(steps: f32) -> (f32, f32) {
    let steps = steps.max(0.0);
    (SHRINK.powf(steps), HAZE.powf(steps))
}

/// How much ink something on its way out keeps, `left` of the way still to
/// go: all of it as it sets off, none once [`GONE_BY`] of the way is left.
pub fn departing(left: f32) -> f32 {
    motion::ease(((left - GONE_BY) / (1.0 - GONE_BY)).clamp(0.0, 1.0))
}

/// The two halves a step runs in where two things stand in one place, at
/// `through` of the way: how much of the first is still there, and how far
/// the second has come in. Never both at once: the second comes in where the
/// first stood once it has gone — the first column's words and the second
/// column on a narrow panel, the word on a button that stays as its word
/// changes, a pair of the legend that changes.
pub fn halves(through: f32) -> (f32, f32) {
    (
        motion::ease((1.0 - through * 2.0).clamp(0.0, 1.0)),
        motion::ease((through * 2.0 - 1.0).clamp(0.0, 1.0)),
    )
}

/// How near a share of the way has to be to the end, and how slow, to be put
/// there: the shell's own `SETTLED`.
const SHARE_SETTLED: f32 = 0.001;

/// A share of the way from nought to one, carried on one of the toolkit's
/// springs: how far a page has given way to the next, how far the long chip
/// in the head has gone to its new place. Arrived (one) until it is begun.
#[derive(Debug, Clone, Copy)]
pub struct Share {
    at: f32,
    speed: f32,
}

impl Default for Share {
    fn default() -> Self {
        Self {
            at: 1.0,
            speed: 0.0,
        }
    }
}

impl Share {
    /// Set off from nought.
    pub fn begin(&mut self) {
        self.at = 0.0;
        self.speed = 0.0;
    }

    /// Carry it one frame's worth towards one on the spring `rate` (one of
    /// the toolkit's), and answer where it is. Near enough and slow enough,
    /// it is put there and stops.
    pub fn advance(&mut self, rate: f64, dt: f32) -> f32 {
        if !self.moving() {
            return self.at;
        }
        let (at, speed) = motion::spring(self.at as f64, self.speed as f64, 1.0, rate, dt as f64);
        self.at = at as f32;
        self.speed = speed as f32;
        if (1.0 - self.at).abs() < SHARE_SETTLED && self.speed.abs() < SHARE_SETTLED {
            self.settle();
        }
        self.at
    }

    /// Turn round where it stands: what was `at` of the way there is now as
    /// far from the end as it was from the start, and its speed is the same
    /// speed the other way — carried on, it goes on the way it was going for
    /// a moment and then comes back, as a spring whose target is moved does.
    pub fn reverse(&mut self) {
        self.at = 1.0 - self.at;
        self.speed = -self.speed;
    }

    /// How far it has got.
    pub fn at(&self) -> f32 {
        self.at
    }

    /// Put it at the end.
    pub fn settle(&mut self) {
        self.at = 1.0;
        self.speed = 0.0;
    }

    pub fn moving(&self) -> bool {
        self.at != 1.0 || self.speed != 0.0
    }
}

/// The two roundings a light can wear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A row or a button: rounded to half its own height.
    Capsule,
    /// A card or a field: rounded to `Metric::CardRadius`.
    Card,
    /// A key on the board, rounded to a share of its height.
    Key,
}

impl Shape {
    /// The radius of the corners of `rect` in this shape: the one rounding
    /// a light and the control it lies on are both drawn with, so a row
    /// told to be a card (`column::Corners`) and its light are never two
    /// shapes.
    pub fn radius(self, page: &Page, rect: [f32; 4]) -> f32 {
        match self {
            Shape::Capsule => rect[3] * 0.5,
            Shape::Card => page.metric(Metric::CardRadius),
            Shape::Key => rect[3] * 0.28,
        }
    }
}

/// The light that crosses a screen, carried here rather than by the toolkit.
///
/// `Page::glide` draws its own selection as a capsule whatever it is over,
/// which is right for a row and wrong for a card: a capsule laid over a card
/// leaves the card's corners sticking out behind it. So the spring is the
/// toolkit's own `HIGHLIGHT_SPRING` and the drawing is `Ui::lit`, which is told
/// what shape to be — the same answer DistriBumpy and SongOnSole came to.
///
/// **One light for the whole installer.** Every component that can hold it is
/// handed it when it does and lays it down before its own controls, because
/// `Ui::control` reads where the light is to fade the chip under it and a
/// light drawn after a row would be drawn over the row's mark.
#[derive(Debug, Default, Clone, Copy)]
pub struct Light {
    at: Option<[f32; 4]>,
    speed: [f32; 4],
    /// Put it where it is going on the next frame rather than carrying it
    /// there: what a photograph needs, and what a screen arrived at fresh
    /// needs — a light that flew in from the last screen would be a light
    /// crossing a page nobody was looking at.
    snap: bool,
}

impl Light {
    /// Carry the light a frame's worth towards `target` and answer where it
    /// is, without drawing it.
    pub fn glide(&mut self, target: [f32; 4], dt: f32) -> [f32; 4] {
        let from = match self.at {
            Some(from) if !self.snap => from,
            _ => {
                self.snap = false;
                self.speed = [0.0; 4];
                self.at = Some(target);
                return target;
            }
        };
        let mut next = [0.0; 4];
        for (index, slot) in next.iter_mut().enumerate() {
            let (at, speed) = motion::spring(
                from[index] as f64,
                self.speed[index] as f64,
                target[index] as f64,
                motion::HIGHLIGHT_SPRING,
                dt as f64,
            );
            *slot = at as f32;
            self.speed[index] = speed as f32;
        }
        self.at = Some(next);
        next
    }

    /// Carry the light towards `target` and lay it down there in `shape`,
    /// dipped by `pressed` (how far through a press the page is). Answers
    /// where it was drawn.
    ///
    /// Also says to the page where the light is, which is where a menu grows
    /// out of: `Page` only records that for the controls it draws itself.
    pub fn lay(
        &mut self,
        page: &mut Page,
        target: [f32; 4],
        shape: Shape,
        pressed: Option<f32>,
        dt: f32,
    ) -> [f32; 4] {
        self.lay_faded(page, target, shape, pressed, dt, 1.0)
    }

    /// The same, at `fade` of its strength: a light on a row that is only
    /// coming in, which is no brighter than the row.
    pub fn lay_faded(
        &mut self,
        page: &mut Page,
        target: [f32; 4],
        shape: Shape,
        pressed: Option<f32>,
        dt: f32,
        fade: f32,
    ) -> [f32; 4] {
        let at = self.glide(target, dt);
        let radius = shape.radius(page, at);
        // A press dims the light a little as the control under it sinks, the
        // way the toolkit's own flow light does.
        let strength = (1.0 - pressed.unwrap_or(0.0) * 0.35) * fade.clamp(0.0, 1.0);
        page.light_at(at);
        page.ui()
            .lit(at, radius, control::LIT_ROLE, at[2], strength.max(0.0));
        at
    }

    /// Move the light with what it lies on: the control under it moved by
    /// `by` (x, y, width, height) since the last frame, and a light resting
    /// on it stays on it rather than gliding after it. A light still on its
    /// way to it is carried along by as much, and arrives all the same.
    pub fn ride(&mut self, by: [f32; 4]) {
        if let Some(at) = self.at.as_mut() {
            for (slot, by) in at.iter_mut().zip(by) {
                *slot += by;
            }
        }
    }

    /// The light as it stands, kept where it was rather than put down afresh:
    /// what a page on its way out is drawn with, its light left on the row it
    /// lit while the installer's own goes to the page coming in.
    pub fn kept(&self) -> Self {
        Self {
            snap: false,
            speed: [0.0; 4],
            ..*self
        }
    }

    /// Have the next frame put the light where it belongs rather than carry
    /// it there.
    pub fn settle(&mut self) {
        self.snap = true;
        self.speed = [0.0; 4];
    }

    /// Whether the light is still on its way, and so wants another frame.
    pub fn moving(&self) -> bool {
        self.speed.iter().any(|speed| speed.abs() > 0.5)
    }
}

/// When a spring is near enough where it is going to stop being carried.
///
/// A spring never quite arrives. Half a point is well under a pixel on any
/// display this runs on, and twenty points a second is a third of a pixel a
/// frame; stopping there is invisible.
const ARRIVED: f32 = 0.5;
const STILL: f64 = 20.0;

/// One number carried towards where it belongs on the toolkit's card spring:
/// how far a column is scrolled, how far the board has risen.
#[derive(Debug, Default, Clone, Copy)]
pub struct Carried {
    at: f32,
    speed: f64,
    wanted: f32,
    placed: bool,
}

impl Carried {
    /// Where it is now.
    pub fn at(&self) -> f32 {
        self.at
    }

    /// Where it was last told to go.
    pub fn wanted(&self) -> f32 {
        self.wanted
    }

    /// Carry it one frame's worth towards `to`. The first time it is ever
    /// told where to go it is put there: a column opened scrolled half way
    /// down opens there rather than scrolling from the top.
    pub fn advance(&mut self, to: f32, dt: f32) -> f32 {
        self.wanted = to;
        if !self.placed {
            self.placed = true;
            self.at = to;
            self.speed = 0.0;
            return to;
        }
        let (at, speed) = motion::spring(
            self.at as f64,
            self.speed,
            to as f64,
            motion::CARD_SPRING,
            dt as f64,
        );
        self.at = at as f32;
        self.speed = speed;
        if (self.at - to).abs() < ARRIVED && self.speed.abs() < STILL {
            self.at = to;
            self.speed = 0.0;
        }
        self.at
    }

    /// Put it where it was last told to go without carrying it there.
    pub fn settle(&mut self) {
        self.at = self.wanted;
        self.speed = 0.0;
    }

    /// Put it somewhere at once and forget where it was going.
    pub fn place(&mut self, to: f32) {
        self.at = to;
        self.wanted = to;
        self.speed = 0.0;
        self.placed = true;
    }

    pub fn moving(&self) -> bool {
        self.at != self.wanted || self.speed != 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_light_crosses_and_then_is_put_down_for_a_photograph() {
        let mut light = Light::default();
        assert_eq!(
            light.glide([0.0, 0.0, 10.0, 10.0], FRAME),
            [0.0, 0.0, 10.0, 10.0],
            "the first place a light is asked for is where it starts"
        );
        let on_its_way = light.glide([100.0, 0.0, 10.0, 10.0], FRAME);
        assert!(on_its_way[0] > 0.0 && on_its_way[0] < 100.0);
        assert!(light.moving());
        light.settle();
        assert_eq!(
            light.glide([100.0, 0.0, 10.0, 10.0], FRAME),
            [100.0, 0.0, 10.0, 10.0]
        );
        assert!(!light.moving());
    }

    #[test]
    fn a_carried_value_arrives_and_then_stops() {
        let mut carried = Carried::default();
        assert_eq!(carried.advance(240.0, FRAME), 240.0, "placed at first");
        let moving = carried.advance(0.0, FRAME);
        assert!(moving > 0.0 && moving < 240.0, "jumped: {moving}");
        for _ in 0..120 {
            carried.advance(0.0, FRAME);
        }
        assert_eq!(carried.at(), 0.0);
        assert!(!carried.moving(), "an arrived value still wants frames");
    }

    #[test]
    fn a_share_of_the_way_arrives_and_then_is_still() {
        let mut share = Share::default();
        assert!(!share.moving(), "a share nobody began is at its end");
        share.begin();
        assert_eq!(share.at(), 0.0);
        let mut frames = 0;
        while share.moving() {
            share.advance(motion::CARD_SPRING, FRAME);
            frames += 1;
            assert!(frames < 120, "never arrived: {}", share.at());
        }
        assert_eq!(share.at(), 1.0, "stopped short of the end");
        assert!(frames > 10, "arrived in {frames} frames, which is a jump");
        // Arrived, it is still: another frame moves nothing.
        assert_eq!(share.advance(motion::CARD_SPRING, FRAME), 1.0);
        assert!(!share.moving());
    }

    #[test]
    fn what_leaves_gives_up_its_ink_first_and_what_recedes_shrinks() {
        assert_eq!(departing(1.0), 1.0, "faint as it set off");
        assert_eq!(departing(GONE_BY), 0.0, "still there a third from the end");
        assert!(departing(0.7) > 0.0 && departing(0.7) < 1.0);
        assert_eq!(receded(0.0), (1.0, 1.0));
        assert_eq!(receded(1.0), (SHRINK, HAZE));
        assert_eq!(receded(-1.0), (1.0, 1.0), "a step forward grew it");
    }

    #[test]
    fn a_kept_light_stays_where_it_was() {
        let mut light = Light::default();
        light.glide([10.0, 20.0, 30.0, 40.0], FRAME);
        light.settle();
        let mut kept = light.kept();
        assert_eq!(
            kept.glide([10.0, 20.0, 30.0, 40.0], FRAME),
            [10.0, 20.0, 30.0, 40.0]
        );
        // The installer's own light, settled, goes straight to the page
        // coming in; the kept one would have glided there.
        assert_eq!(
            light.glide([500.0, 20.0, 30.0, 40.0], FRAME),
            [500.0, 20.0, 30.0, 40.0]
        );
        assert!(kept.glide([500.0, 20.0, 30.0, 40.0], FRAME)[0] < 500.0);
    }

    #[test]
    fn a_picture_is_timed_by_the_instant_it_was_given() {
        let mut clock = Clock::new(true);
        clock.tick(6.0);
        assert_eq!(clock.now, 6.0);
        assert_eq!(clock.dt, FRAME, "the first frame counts as one frame");
        clock.tick(6.0);
        assert_eq!(
            clock.dt, 0.0,
            "the second frame of a picture is the same instant"
        );
        clock.tick(60.0);
        assert!(clock.dt <= LONGEST, "a long gap moved everything at once");
    }

    #[test]
    fn a_picture_of_a_still_clock_counts_its_frames() {
        let mut clock = Clock::new(true);
        clock.tick_held(24.0);
        assert_eq!((clock.now, clock.dt), (24.0, FRAME));
        clock.tick_held(24.0);
        assert_eq!(clock.now, 24.0 + FRAME, "a frame went by at one instant");
        assert!((clock.dt - FRAME).abs() < 1e-5, "it counted {}", clock.dt);
    }
}
