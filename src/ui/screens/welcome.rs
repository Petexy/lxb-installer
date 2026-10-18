//! Welcome: the mark, a greeting that takes turns in twenty languages, and
//! the button that begins.
//!
//! One control and nothing else: the button, the one primary verb, in the
//! corner of the buttons' band where every screen's verb stands, the light on
//! it. Welcome offers no Language button: Language is the very next step,
//! one press away.
//!
//! **The greeting takes turns**, as the earlier installer's did (its
//! `welcome_widget.py`): Welcome to Linexin in the same twenty languages, in
//! its order, with the button saying its Begin Installation in the same
//! language. It starts on the person's own language where it is one of the
//! twenty, and the first change comes some three seconds after arriving; each
//! greeting stays five seconds, goes in 0.4 s (easing in) and the next comes
//! up in 0.6 s (easing out), the button's word 0.15 s after the greeting. The
//! button itself never moves and never goes: it is as wide as the widest of
//! the twenty words, in the corner, lit and pressable while its word changes.
//! On low-end hardware the words change without the fade, the window drawn
//! once for each change.
//!
//! Back here is the only way out of the installer, and it is asked rather
//! than done: the toolkit's dialog, "Stay" first.

use lxb_app::lxb_render::Align;
use lxb_app::lxb_toolkit::{input::Action, metrics::Metric, palette::Role, typography::Text};
use lxb_app::{Page, Scroll};

use super::{Head, Room};
use crate::i18n;
use crate::ui::app::{Machine, State};
use crate::ui::button::{self, Button};
use crate::ui::legend::{hint, Button as Pictured, Hint};
use crate::ui::panel::Note;
use crate::ui::text;

/// One of the greetings Welcome takes turns with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Greeting {
    /// The language's code, by which the person's own is started on.
    pub language: &'static str,
    /// Welcome to Linexin, the name where the language puts it.
    pub welcome: &'static str,
    /// The button's word: the earlier installer's Begin Installation.
    pub begin: &'static str,
}

const fn greeting(language: &'static str, welcome: &'static str, begin: &'static str) -> Greeting {
    Greeting {
        language,
        welcome,
        begin,
    }
}

/// The earlier installer's twenty, in its order, each with the name put
/// into it as the language puts it: after the words in most, before them
/// where the language ends with them — Japanese へようこそ, Hindi में आपका
/// स्वागत है, Korean 에 오신 것을 환영합니다 — and as a suffix Turkish and
/// Finnish join to the name itself (Linexin’e, Linexiniin), where the earlier
/// installer had a bare 'a to stand after its logo. Portuguese takes the
/// article a name takes there (ao Linexin), as the installer's own catalog
/// says it.
pub const GREETINGS: [Greeting; 20] = [
    greeting("en", "Welcome to Linexin", "Begin Installation"),
    greeting("pl", "Witaj w Linexin", "Rozpocznij instalację"),
    greeting("es", "Bienvenido a Linexin", "Iniciar instalación"),
    greeting("fr", "Bienvenue à Linexin", "Commencer l'installation"),
    greeting("de", "Willkommen bei Linexin", "Installation beginnen"),
    greeting("it", "Benvenuto in Linexin", "Inizia l'installazione"),
    greeting("pt", "Bem-vindo ao Linexin", "Iniciar instalação"),
    greeting("ru", "Добро пожаловать в Linexin", "Начать установку"),
    greeting("ja", "Linexinへようこそ", "インストールを開始"),
    greeting("zh", "欢迎来到 Linexin", "开始安装"),
    greeting("ar", "مرحبا بك في Linexin", "بدء التثبيت"),
    greeting("hi", "Linexin में आपका स्वागत है", "इंस्टॉलेशन शुरू करें"),
    greeting("nl", "Welkom bij Linexin", "Installatie beginnen"),
    greeting("sv", "Välkommen till Linexin", "Påbörja installationen"),
    greeting("tr", "Linexin’e hoş geldiniz", "Kuruluma Başla"),
    greeting("ko", "Linexin에 오신 것을 환영합니다", "설치 시작"),
    greeting("el", "Καλώς ήρθατε στο Linexin", "Έναρξη εγκατάστασης"),
    greeting("uk", "Ласкаво просимо до Linexin", "Почати встановлення"),
    greeting("cs", "Vítejte v Linexin", "Zahájit instalaci"),
    greeting("fi", "Tervetuloa Linexiniin", "Aloita asennus"),
];

/// Seconds on Welcome before the first greeting goes.
pub const FIRST: f32 = 3.0;
/// Seconds a greeting stays, from the moment it begins to come up to the
/// moment it begins to go.
pub const HOLD: f32 = 5.0;
/// Seconds a greeting takes to go, easing in.
pub const OUT: f32 = 0.4;
/// Seconds the next takes to come up, easing out.
pub const IN: f32 = 0.6;
/// How long after the greeting the button's word follows it up.
pub const LAG: f32 = 0.15;
/// From one greeting going to the next going.
pub const PERIOD: f32 = OUT + HOLD;

/// What Welcome keeps between frames.
#[derive(Debug, Default, Clone)]
pub struct View {
    /// The clock's time when Welcome was drawn first since it was arrived
    /// at: what the turns are timed from.
    since: Option<f32>,
    /// The greeting it starts on.
    start: usize,
    /// A greeting a picture asked to start on (`--greeting`), kept over
    /// arriving again.
    pinned: Option<usize>,
    /// How many lines the tallest greeting takes at a width and scale: the
    /// room the greeting stands in, the same for all twenty.
    lines: Option<(f32, f32, usize)>,
}

/// Where the turns stand some seconds after arriving: which greeting, how
/// much of its ink is up, and of the button's word, and how long until the
/// next greeting replaces it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Turn {
    pub at: usize,
    pub greeting: f32,
    pub button: f32,
    pub next: f32,
}

/// The turns `elapsed` seconds after arriving, starting on greeting
/// `start`: fading (`fade`), or changing at once where the hardware is
/// low-end, at the moment the next would begin to come up.
pub fn turn(start: usize, elapsed: f32, fade: bool) -> Turn {
    let count = GREETINGS.len();
    let elapsed = elapsed.max(0.0);
    if elapsed < FIRST {
        return Turn {
            at: start % count,
            greeting: 1.0,
            button: 1.0,
            next: FIRST + OUT - elapsed,
        };
    }
    let since = elapsed - FIRST;
    let done = (since / PERIOD).floor();
    let phase = since - done * PERIOD;
    let done = done as usize;
    if phase < OUT {
        let going = if fade {
            1.0 - ease_in(phase / OUT)
        } else {
            1.0
        };
        return Turn {
            at: (start + done) % count,
            greeting: going,
            button: going,
            next: OUT - phase,
        };
    }
    let up = phase - OUT;
    let (greeting, button) = if fade {
        (ease_out(up / IN), ease_out((up - LAG) / IN))
    } else {
        (1.0, 1.0)
    };
    Turn {
        at: (start + done + 1) % count,
        greeting,
        button,
        next: PERIOD - phase + OUT,
    }
}

fn ease_in(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// The greeting a person speaking `locale` (`pl_PL.UTF-8`, `pt-BR`) is
/// greeted with first: the one in their language, else the first.
pub fn first_for(locale: &str) -> usize {
    let language = locale
        .split(['_', '-', '.', '@'])
        .next()
        .unwrap_or_default();
    GREETINGS
        .iter()
        .position(|greeting| greeting.language == language)
        .unwrap_or(0)
}

/// The widest of the twenty button words, which the button is laid out
/// for whichever it says.
fn widest_begin(page: &mut Page) -> &'static str {
    let mut widest = (GREETINGS[0].begin, 0.0f32);
    for greeting in GREETINGS {
        let wide = text::bold_width(page.ui(), Text::Label, greeting.begin);
        if wide > widest.1 {
            widest = (greeting.begin, wide);
        }
    }
    widest.0
}

/// The turns where they stand now: started on the first draw since
/// arriving.
fn now(state: &mut State) -> Turn {
    let clock = state.clock.now;
    let view = &mut state.views.welcome;
    let since = *view.since.get_or_insert(clock);
    turn(view.start, clock - since, !state.low_end())
}

pub fn enter(state: &mut State) {
    let view = &mut state.views.welcome;
    view.since = None;
    view.start = match view.pinned {
        Some(pinned) => pinned,
        None => first_for(&state.draft.region.locale),
    };
}

/// For a picture: start on greeting `at` (from 0), whatever the language.
pub fn pin(state: &mut State, at: usize) {
    state.views.welcome.pinned = Some(at % GREETINGS.len());
    enter(state);
}

/// The head names the program: the greeting under it says the welcome.
pub fn head(_state: &State) -> Head {
    Head::new(
        "logo",
        i18n::text("app-name"),
        i18n::text("welcome-caption"),
    )
}

pub fn hints(_state: &State) -> Vec<Hint> {
    vec![
        hint(i18n::text("hint-choose"), Pictured::Accept),
        hint(i18n::text("hint-quit"), Pictured::Back),
    ]
}

/// Said at the door rather than at the review: a machine that cannot be
/// installed onto from here is told so before anybody answers seven
/// questions for nothing.
pub fn note(state: &State) -> Option<Note> {
    if state.disks_trouble.is_some() {
        return Some(Note {
            text: i18n::text("disks-unreadable").to_string(),
            role: Role::Danger,
        });
    }
    match &state.machine {
        Machine::Real { live: None } => Some(Note {
            text: i18n::text("install-unavailable").to_string(),
            role: Role::Text,
        }),
        _ => None,
    }
}

pub fn act(state: &mut State, action: Action, _scroll: Option<Scroll>) {
    super::lone_act(state, action, super::onward);
}

/// How many lines of the greeting's face the tallest of the twenty takes
/// in `room`, broken as [`text::break_lines`] breaks them; `measure` is how
/// wide words are in it.
pub fn lines_for(room: f32, mut measure: impl FnMut(&str) -> f32) -> usize {
    GREETINGS
        .iter()
        .map(|greeting| text::break_lines(greeting.welcome, room, &mut measure).len())
        .max()
        .unwrap_or(1)
        .max(1)
}

/// `words` broken into as few lines as `room` allows, and those lines as
/// even as they can be: broken at the narrowest room that still takes no
/// more of them. A greeting broken as full as each line holds left a lone
/// character under it on a display standing on its side (Linexinへようこ /
/// そ); evened, it reads Linexinへ / ようこそ.
pub fn even_lines(words: &str, room: f32, mut measure: impl FnMut(&str) -> f32) -> Vec<String> {
    let full = text::break_lines(words, room, &mut measure);
    if full.len() < 2 {
        return full;
    }
    let fits = |lines: &[String], within: f32, measure: &mut dyn FnMut(&str) -> f32| {
        lines.len() <= full.len() && lines.iter().all(|line| measure(line) <= within + 0.5)
    };
    let (mut least, mut most) = (0.0f32, room);
    let mut best = full.clone();
    for _ in 0..14 {
        let middle = (least + most) * 0.5;
        let lines = text::break_lines(words, middle, &mut measure);
        if fits(&lines, middle, &mut measure) {
            most = middle;
            best = lines;
        } else {
            least = middle;
        }
    }
    best
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let turn = now(state);
    if state.low_end() {
        // One frame for each change, at the moment it is due.
        page.redraw_within(std::time::Duration::from_secs_f32(turn.next.max(0.0)));
    }
    let greeting = GREETINGS[turn.at];
    let gap = page.metric(Metric::Gap);
    let content = room.content;
    let line = page.line(Text::Display);
    let scale = page.scaled(1.0);
    let lines = match state.views.welcome.lines {
        Some((width, at, lines)) if width == content[2] && at == scale => lines,
        _ => {
            let ui = page.ui();
            let lines = lines_for(content[2], |words| {
                text::bold_width(ui, Text::Display, words)
            });
            state.views.welcome.lines = Some((content[2], scale, lines));
            lines
        }
    };
    let greeting_h = line * lines as f32;

    // The mark and the greeting under it, together in the middle of the
    // room over the buttons; the greeting's room as tall as the tallest of
    // the twenty needs, so the mark never moves as they take turns.
    let above = (room.buttons[1].min(content[1] + content[3]) - gap * 2.0 - content[1]).max(0.0);
    // Whole pixels: a mark this large laid at a fraction of one drew a hair
    // of its material down its right-hand edge.
    let mark = page
        .scaled(220.0)
        .min((above - greeting_h - gap) * 0.85)
        .max(0.0)
        .round();
    let block = mark + gap + greeting_h;
    let top = (content[1] + ((above - block) * 0.5).max(0.0)).round();
    if mark > page.scaled(48.0) {
        let icons = page.icons();
        let left = (content[0] + (content[2] - mark) * 0.5).round();
        page.ui()
            .icon_tinted([left, top, mark, mark], "logo", icons, Role::Text, 0.92);
    }
    let ui = page.ui();
    let broken = even_lines(greeting.welcome, content[2], |words| {
        text::bold_width(ui, Text::Display, words)
    });
    // A greeting of fewer lines than the tallest stands in the middle of
    // the room the tallest takes.
    let short = lines.saturating_sub(broken.len()) as f32;
    let first = top + mark + gap + line * short * 0.5;
    for (index, words) in broken.iter().take(lines).enumerate() {
        text::label_faded(
            ui,
            [content[0], first + line * index as f32, content[2], line],
            Text::Display,
            words,
            Role::Text,
            turn.greeting,
            Align::Centre,
        );
    }

    // The button laid out for the widest word, so it stands still whichever
    // it says, its word fading while the button stays lit.
    let widest = widest_begin(page);
    let places = button::places(page, room.buttons, &[Button::primary(widest)]);
    let verb = [Button::primary(greeting.begin).inked(turn.button)];
    let pressing = state.pressing.through();
    let laid = button::row(
        page,
        &places,
        &verb,
        lit.then_some(0),
        &mut state.light,
        pressing,
        state.clock.dt,
    );
    state.laid_buttons(laid);
}

/// One control, always lit: nothing for the pointer to move the light to.
pub fn hover(_state: &mut State, _id: u32) {}

pub fn pressed(state: &mut State, page: &mut Page) {
    if button::pressed(page, &[Button::primary("")]).is_some() {
        super::onward(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::app::Screen;

    #[test]
    fn the_button_goes_on_into_the_flow() {
        let mut state = State::demo(true);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Language);
    }

    /// Welcome offers no Language button: no direction leaves the button, and
    /// Back asks whether to leave.
    #[test]
    fn nothing_but_the_button_stands_on_welcome() {
        let mut state = State::demo(true);
        for direction in [Action::Left, Action::Up, Action::Right, Action::Down] {
            act(&mut state, direction, None);
            assert_eq!(state.screen, Screen::Welcome, "{direction:?}");
        }
        hover(&mut state, crate::ui::spots::ITEMS);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Language, "the light left the button");
        let mut state = State::demo(true);
        act(&mut state, Action::Back, None);
        assert_eq!(state.question_up(), Some(crate::ui::app::Question::Quit));
    }

    /// The earlier installer's twenty, in its order, each with the name in
    /// it once, and its button's word.
    #[test]
    fn the_twenty_greetings_are_the_earlier_installers() {
        let languages: Vec<&str> = GREETINGS.iter().map(|one| one.language).collect();
        assert_eq!(
            languages,
            [
                "en", "pl", "es", "fr", "de", "it", "pt", "ru", "ja", "zh", "ar", "hi", "nl", "sv",
                "tr", "ko", "el", "uk", "cs", "fi"
            ]
        );
        for greeting in GREETINGS {
            let named = greeting.welcome.matches("Linexin").count();
            assert_eq!(named, 1, "{greeting:?}");
            assert!(!greeting.begin.is_empty());
        }
        // The name where the language puts it: before the words that end
        // the sentence, joined to a suffix where the language joins one.
        for language in ["ja", "hi", "ko", "tr"] {
            let at = first_for(language);
            assert!(GREETINGS[at].welcome.starts_with("Linexin"), "{language}");
        }
        assert_eq!(GREETINGS[first_for("tr")].welcome, "Linexin’e hoş geldiniz");
    }

    #[test]
    fn the_first_greeting_is_in_the_persons_language() {
        assert_eq!(first_for("en_US.UTF-8"), 0);
        assert_eq!(first_for("pl_PL.UTF-8"), 1);
        assert_eq!(first_for("pt-BR"), 6);
        assert_eq!(first_for("zh_CN.UTF-8"), 9);
        assert_eq!(first_for("it_IT.UTF-8"), 5);
        assert_eq!(first_for("da_DK.UTF-8"), 0, "not one of the twenty");
        let mut state = State::demo(true);
        state.draft.region.locale = "uk_UA.UTF-8".into();
        enter(&mut state);
        assert_eq!(state.views.welcome.start, 17);
        pin(&mut state, 3);
        assert_eq!(state.views.welcome.start, 3);
    }

    /// The earlier installer's rhythm: the first change three seconds in,
    /// each greeting going in 0.4 s and the next coming up in 0.6 s, the
    /// button's word 0.15 s behind it, five seconds each.
    #[test]
    fn the_greetings_take_turns_in_the_earlier_installers_rhythm() {
        let at = |seconds: f32| turn(0, seconds, true);
        let still = Turn {
            at: 0,
            greeting: 1.0,
            button: 1.0,
            next: FIRST + OUT,
        };
        assert_eq!(at(0.0), still);
        assert_eq!(at(2.9).at, 0);
        // Going: the same greeting, its ink easing away, the button's with
        // it.
        let going = at(FIRST + 0.2);
        assert_eq!(going.at, 0);
        assert!(going.greeting > 0.8 && going.greeting < 1.0, "{going:?}");
        assert_eq!(going.greeting, going.button);
        assert!(at(FIRST + OUT - 0.01).greeting < 0.1);
        // Coming up: the next, the button's word behind it.
        let coming = at(FIRST + OUT + 0.1);
        assert_eq!(coming.at, 1);
        assert!(coming.greeting > 0.3 && coming.button == 0.0, "{coming:?}");
        let up = at(FIRST + OUT + IN + LAG);
        assert_eq!((up.greeting, up.button), (1.0, 1.0));
        // Five seconds from coming up to going.
        assert_eq!(at(FIRST + PERIOD - 0.01).at, 1);
        assert_eq!(at(FIRST + PERIOD - 0.01).greeting, 1.0);
        assert!(at(FIRST + PERIOD + 0.1).greeting < 1.0);
        assert_eq!(at(FIRST + PERIOD * 2.0 + OUT + 1.0).at, 3);
        // Round the twenty and back to the first.
        assert_eq!(at(FIRST + PERIOD * 19.0 + OUT + 1.0).at, 0);
        assert_eq!(turn(18, FIRST + PERIOD + OUT + 1.0, true).at, 0);
    }

    /// On low-end hardware the words change at once, at the moment the
    /// next would begin to come up, and the window is asked for that one
    /// frame and no other.
    #[test]
    fn low_end_hardware_changes_the_words_at_once() {
        for seconds in [
            0.0,
            FIRST + 0.1,
            FIRST + OUT - 0.01,
            FIRST + OUT + 0.05,
            20.0,
        ] {
            let still = turn(0, seconds, false);
            assert_eq!((still.greeting, still.button), (1.0, 1.0), "{seconds}");
            assert_eq!(still.at, turn(0, seconds, true).at, "{seconds}");
        }
        let before = turn(0, FIRST + 0.1, false);
        assert_eq!(before.at, 0);
        assert!((before.next - (OUT - 0.1)).abs() < 1e-4, "{before:?}");
        let after = turn(0, FIRST + OUT + 1.0, false);
        assert_eq!(after.at, 1);
        assert!((after.next - (PERIOD - 1.0)).abs() < 1e-4, "{after:?}");
    }

    /// Ten points a Latin letter, twenty a kana, as wide as they are drawn
    /// near enough.
    fn even(words: &str) -> f32 {
        words
            .chars()
            .map(|letter| if letter.is_ascii() { 10.0 } else { 20.0 })
            .sum()
    }

    /// A greeting on more than one line is broken evenly, never leaving a
    /// lone character under the rest.
    #[test]
    fn a_greeting_on_two_lines_is_broken_evenly() {
        assert_eq!(
            even_lines("Linexinへようこそ", 160.0, even),
            ["Linexinへ", "ようこそ"]
        );
        assert_eq!(
            text::break_lines("Linexinへようこそ", 160.0, even),
            ["Linexinへようこ", "そ"],
            "broken full, as before"
        );
        assert_eq!(
            even_lines("Welcome to Linexin", 400.0, even),
            ["Welcome to Linexin"]
        );
        let russian = "Добро пожаловать в Linexin";
        let lines = even_lines(russian, 260.0, even);
        assert_eq!(lines, ["Добро", "пожаловать", "в Linexin"]);
        assert_eq!(lines.len(), text::break_lines(russian, 260.0, even).len());
    }

    /// Pressing the button mid-turn, its word faded away, still begins.
    #[test]
    fn the_button_is_pressed_whatever_its_word_is_doing() {
        let mut state = State::demo(true);
        state.clock.tick(10.0);
        let _ = now(&mut state);
        state.clock.tick(10.0 + FIRST + OUT - 0.01);
        assert!(now(&mut state).button < 0.1);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Language);
    }
}
