//! You: full name, username, the password twice with how strong it is, the
//! computer's name, and an administrator account that is off unless asked
//! for (with its own password twice when it is on).
//!
//! A form in one column, every field below the one before and never two
//! side by side, in the legacy installer's order: who you are, the password
//! twice, the computer's name, and the administrator switch last, its own two
//! fields under it while it is on. The column stands in the middle of the
//! panel, a field with nothing to say under it close under the one before, so
//! on a landscape panel the whole form shows until the administrator's fields
//! are asked for; where the panel is too short for all of it (the board up,
//! the administrator on) it scrolls to keep the field in hand in view, and
//! says that it goes on past an end — to a pointer with a scroll bar, with a
//! pad in hand by the arrows the family's menus draw there. Each field says
//! what is wrong with it on its own label line, in the model's own words for
//! the refusal (`config::validate_*`, the checks the worker makes again), so
//! the reason stands beside the thing to change. An empty field is not called
//! wrong until Continue has been pressed: a form that greets somebody with
//! five refusals has told them nothing yet.
//!
//! The username follows the full name — its first word, in small letters,
//! with accents folded away and anything a username cannot hold dropped —
//! until the person types one of their own.
//!
//! Typing: with a keyboard in hand the field the light stands in takes text;
//! with a pad, Accept on a field brings a board — under LineXinBar the
//! shell's own, which rises over a field that takes text, and elsewhere the
//! installer's (`State::type_with_pad`).

use lxb_app::lxb_render::Align;
use lxb_app::lxb_toolkit::{
    input::Action, metrics::Metric, palette::Role, sound::Sound, typography::Text,
};
use lxb_app::{Page, Scroll};

use super::{Head, Room};
use crate::i18n;
use crate::model::config::{
    validate_full_name, validate_hostname, validate_password, validate_username, Invalid,
};
use crate::model::secret::Secret;
use crate::ui::app::State;
use crate::ui::button::Button;
use crate::ui::column::{self, keep_span_in_view, Span};
use crate::ui::field::{self, Editable, FieldId, Look};
use crate::ui::legend::{hint, Button as Pictured, Hint};
use crate::ui::motion::Carried;
use crate::ui::panel::Note;
use crate::ui::toggle::{self, Toggle};
use crate::ui::{glyphs, spots, text, words};

/// The administrator switch's number.
const ROOT: u32 = spots::ITEMS;

/// The number of the form's scroll bar.
const BAR: u32 = spots::ITEMS + 1;

/// The number written over what the form has scrolled past its room, which
/// nothing answers to. A click is answered by the last target written under
/// it, and none is taken back: a field cut off at the room's edge would
/// still be clicked where it is not shown — over the screen's caption, or
/// the note under the form — as a column's row cut off is not.
const BEYOND: u32 = spots::ITEMS + 2;

/// How wide the form's fields are at most, in reference pixels: as wide as
/// a row reads well.
const WIDEST: f32 = 760.0;

/// How wide the form's fields are in a room `room` wide at `scale`: as wide
/// as a row reads well, or the whole room. What the catalogs' test holds a
/// field's words to.
pub fn width_at(scale: f32, room: f32) -> f32 {
    room.min(WIDEST * scale)
}

/// One control of the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Field(FieldId),
    /// The administrator account's switch.
    Root,
}

/// Where the light stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Item(Item),
    Continue,
}

impl Default for Focus {
    fn default() -> Self {
        Focus::Item(Item::Field(FieldId::FullName))
    }
}

/// What this screen keeps between frames.
#[derive(Debug, Default, Clone)]
pub struct View {
    pub focus: Focus,
    /// The item the light was on before it went down to Continue, which Up
    /// from Continue goes back to.
    last: Option<Item>,
    /// Whether the administrator account is on.
    pub root: bool,
    /// Whether the person typed a username of their own; until then it
    /// follows the full name.
    pub username_edited: bool,
    /// Whether Continue has been pressed, after which an empty field is
    /// called wrong too.
    pub tried: bool,
    scroll: Carried,
    /// Whether the pointer put the light where it is, which holds the form
    /// still (`Column::point_at` says why) until a direction moves it.
    pointed: bool,
    /// Where the scroll bar was drawn last and how much of the form shows,
    /// so a drag can be measured against it; no height where none was.
    track: [f32; 4],
    run: f32,
    /// How far the room for each item's lines under it is open, by the
    /// item's place in the form with the administrator on (the form without
    /// it is the same list, shorter).
    under: [Carried; ITEMS],
}

/// How many items the form has at most: five fields, the administrator
/// switch and its two fields.
const ITEMS: usize = 8;

impl View {
    pub fn settle(&mut self) {
        self.scroll.settle();
        for under in &mut self.under {
            under.settle();
        }
    }

    /// Whether the pointer that put the light where it is holds the form
    /// still, rather than the form scrolling the item in hand into view.
    pub fn held_by_the_pointer(&self) -> bool {
        self.pointed
    }
}

// -------------------------------------------------------------------------
// The username and the password's strength.

/// A letter as the ASCII a username can hold: accents folded away and the
/// Cyrillic alphabet written out in Latin letters. `None` for anything with
/// no such spelling (a Hindi or Chinese name gives no username, and the
/// person types one).
fn folded(letter: char) -> Option<&'static str> {
    Some(match letter {
        'a'..='z' | '0'..='9' | '_' | '-' => return None,
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ą' | 'ă' => "a",
        'ç' | 'ć' | 'č' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
        'ğ' => "g",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ı' => "i",
        'ł' | 'ľ' | 'ĺ' => "l",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'ŕ' | 'ř' => "r",
        'ś' | 'š' | 'ş' | 'ș' => "s",
        'ß' => "ss",
        'ť' | 'ţ' | 'ț' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'æ' => "ae",
        'œ' => "oe",
        'þ' => "th",
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'е' | 'ё' | 'э' => "e",
        'ж' => "zh",
        'з' => "z",
        'и' => "i",
        'й' | 'ы' => "y",
        'к' => "k",
        'л' => "l",
        'м' => "m",
        'н' => "n",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'у' => "u",
        'ф' => "f",
        'х' => "kh",
        'ц' => "ts",
        'ч' => "ch",
        'ш' => "sh",
        'щ' => "shch",
        'ю' => "yu",
        'я' => "ya",
        _ => "",
    })
}

/// The username a full name suggests: its first word, in small letters,
/// accents folded, anything else a username cannot hold dropped, starting
/// with a letter, at most 32 characters. Empty where nothing is left.
pub fn derive_username(full_name: &str) -> String {
    let first = full_name.split_whitespace().next().unwrap_or_default();
    let mut name = String::new();
    for letter in first.chars().flat_map(char::to_lowercase) {
        match folded(letter) {
            None => name.push(letter),
            Some(spelt) => name.push_str(spelt),
        }
    }
    let name = name.trim_start_matches(|c: char| !c.is_ascii_lowercase() && c != '_');
    name.chars().take(32).collect()
}

/// How strong a password looks, as the legacy installer rated it: a point
/// each for eight characters or more, a small letter, a capital, a digit and
/// a sign. Advisory only — nothing is refused for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    Weak,
    Fair,
    Good,
    Strong,
}

pub fn strength(password: &str) -> Option<Strength> {
    if password.is_empty() {
        return None;
    }
    let has = |test: fn(char) -> bool| password.chars().any(test);
    let points = [
        password.chars().count() >= 8,
        has(char::is_lowercase),
        has(char::is_uppercase),
        has(|c| c.is_ascii_digit()),
        has(|c| !c.is_alphanumeric() && !c.is_whitespace()),
    ]
    .into_iter()
    .filter(|point| *point)
    .count();
    Some(match points {
        0..=2 => Strength::Weak,
        3 => Strength::Fair,
        4 => Strength::Good,
        _ => Strength::Strong,
    })
}

fn strength_words(strength: Strength) -> &'static str {
    match strength {
        Strength::Weak => i18n::text("strength-weak"),
        Strength::Fair => i18n::text("strength-fair"),
        Strength::Good => i18n::text("strength-good"),
        Strength::Strong => i18n::text("strength-strong"),
    }
}

// -------------------------------------------------------------------------
// The checks.

/// The administrator's password into the request while the account is on,
/// and out of it while it is off.
pub fn sync_root(state: &mut State) {
    state.draft.account.root_password = state
        .views
        .account
        .root
        .then(|| state.root_password.clone());
}

/// What a field holds now changed: the username follows a full name it was
/// not typed over, and the administrator's password is carried into the
/// request.
///
/// Typed in, the field in hand is scrolled into view, wherever the pointer
/// that lit or clicked it held the form: the form rests with the next field's
/// label peeking in at its soft edge (`peeking`), which invites a click on a
/// field whose line of text and caret are past the room's foot. Typed in, the
/// field holds the light from the pointer (`State::typing_held`), so the form
/// moving under a resting pointer walks nothing.
pub fn edited(state: &mut State, field: FieldId) {
    if state.views.account.focus == Focus::Item(Item::Field(field)) {
        state.views.account.pointed = false;
    }
    match field {
        FieldId::FullName if !state.views.account.username_edited => {
            state.draft.account.username = derive_username(&state.draft.account.full_name);
        }
        // Rubbed out to nothing, the username follows the full name again.
        FieldId::Username => {
            state.views.account.username_edited = !state.draft.account.username.is_empty();
        }
        FieldId::RootPassword => sync_root(state),
        _ => {}
    }
}

/// Whether the second typing of a password says something else than the
/// first: only once something is typed in it, or Continue has been pressed.
fn mismatch(first: &Secret, second: &Secret, tried: bool) -> Option<Invalid> {
    let typed = !second.is_empty() || (tried && !first.is_empty());
    (typed && first != second).then_some(Invalid::PasswordMismatch)
}

/// Why what an item holds cannot be installed, if it cannot. With `tried`
/// false an empty field is let be.
fn refusal(state: &State, item: Item, tried: bool) -> Option<Invalid> {
    let account = &state.draft.account;
    let quiet_when_empty = |result: Result<(), Invalid>, empty: Invalid| match result {
        Err(invalid) if invalid == empty && !tried => None,
        other => other.err(),
    };
    match item {
        Item::Root => None,
        Item::Field(FieldId::FullName) => validate_full_name(&account.full_name).err(),
        Item::Field(FieldId::Username) => {
            quiet_when_empty(validate_username(&account.username), Invalid::UsernameEmpty)
        }
        Item::Field(FieldId::Password) => quiet_when_empty(
            validate_password(&account.password, &account.password),
            Invalid::PasswordEmpty,
        ),
        Item::Field(FieldId::Repeat) => mismatch(&account.password, &state.repeat, tried),
        Item::Field(FieldId::Hostname) => validate_hostname(&account.hostname).err(),
        Item::Field(FieldId::RootPassword) if state.views.account.root => {
            if state.root_password.is_empty() {
                tried.then_some(Invalid::RootPasswordEmpty)
            } else {
                validate_password(&state.root_password, &state.root_password).err()
            }
        }
        Item::Field(FieldId::RootRepeat) if state.views.account.root => {
            mismatch(&state.root_password, &state.root_repeat, tried)
        }
        Item::Field(_) => None,
    }
}

/// The whole form judged as Continue judges it: the first item, in the
/// order they are read, that cannot be installed, and why.
pub fn check(state: &State) -> Result<(), (Item, Invalid)> {
    for item in order(state.views.account.root) {
        if let Some(invalid) = refusal(state, item, true) {
            return Err((item, invalid));
        }
    }
    Ok(())
}

/// Continue: on, or refused with the reason — said in the foot, and the
/// light carried to the field it is about.
fn onward(state: &mut State) {
    state.views.account.tried = true;
    match check(state) {
        Ok(()) => state.forward(),
        Err((item, invalid)) => {
            state.sound(Sound::Error);
            state.say(words::invalid(invalid), Role::Danger);
            // Carried there, not pointed at: the form scrolls to it.
            state.views.account.focus = Focus::Item(item);
            state.views.account.pointed = false;
        }
    }
}

// -------------------------------------------------------------------------
// The layout.

/// Every item in the order it is read, top to bottom: the order the form
/// lists them in, and the order Continue judges them in.
fn order(root: bool) -> Vec<Item> {
    let mut items = vec![
        Item::Field(FieldId::FullName),
        Item::Field(FieldId::Username),
        Item::Field(FieldId::Password),
        Item::Field(FieldId::Repeat),
        Item::Field(FieldId::Hostname),
        Item::Root,
    ];
    if root {
        items.push(Item::Field(FieldId::RootPassword));
        items.push(Item::Field(FieldId::RootRepeat));
    }
    items
}

fn verb() -> Button<'static> {
    Button::primary(i18n::text("button-continue"))
}

// -------------------------------------------------------------------------
// The screen.

/// The pictures of this screen: a name typed and the username it gave, a
/// password with its strength, and its second typing short of the first, so
/// a refusal stands on its row; or (`account-administrator`) the passwords
/// matching and the administrator account on, the light in its second
/// password at the foot of the form, which has scrolled to keep it in view.
pub fn stage(state: &mut State, scene: &str) {
    state.draft.account.full_name = "Alex Example".into();
    edited(state, FieldId::FullName);
    state.draft.account.password = Secret::new("Example-2026");
    state.repeat = Secret::new("Exam");
    state.views.account.focus = Focus::Item(Item::Field(FieldId::Password));
    if scene == "account-administrator" {
        state.repeat = Secret::new("Example-2026");
        state.views.account.root = true;
        state.root_password = Secret::new("Another-2026");
        sync_root(state);
        state.views.account.focus = Focus::Item(Item::Field(FieldId::RootRepeat));
    }
}

pub fn enter(state: &mut State) {
    let view = &mut state.views.account;
    view.focus = Focus::default();
    view.last = None;
    view.tried = false;
    view.pointed = false;
    // Arriving, what each item says under itself is there at once.
    view.under = Default::default();
}

pub fn head(_state: &State) -> Head {
    Head::new(
        "file-home",
        i18n::text("account-title"),
        i18n::text("account-caption"),
    )
}

pub fn hints(state: &State) -> Vec<Hint> {
    match state.views.account.focus {
        Focus::Item(Item::Field(_)) if state.pad => vec![
            hint(i18n::text("hint-type"), Pictured::Accept),
            hint(i18n::text("hint-back"), Pictured::Back),
        ],
        _ => super::choose_and_back(),
    }
}

pub fn note(_state: &State) -> Option<Note> {
    None
}

fn focus(state: &mut State, to: Focus) {
    let view = &mut state.views.account;
    // A key that takes the light out of the form to Continue leaves the form
    // where the pointer held it — a press that was not about the form does
    // not move it under a resting pointer — and one that brings the light
    // back to an item scrolls to it again, as a column does
    // (`Column::light_is_here`).
    if to != Focus::Continue {
        view.pointed = false;
    }
    if view.focus != to {
        if let (Focus::Item(item), Focus::Continue) = (view.focus, to) {
            view.last = Some(item);
        }
        view.focus = to;
        state.sound(Sound::Move);
    }
}

/// Switch the administrator account, letting go of its passwords when it
/// goes off, as the legacy installer did.
fn flip_root(state: &mut State) {
    let root = !state.views.account.root;
    state.views.account.root = root;
    if !root {
        state.root_password.clear();
        state.root_repeat.clear();
    }
    sync_root(state);
    state.sound(Sound::Press);
}

pub fn act(state: &mut State, action: Action, _scroll: Option<Scroll>) {
    if action == Action::Back {
        return state.back();
    }
    let view = &state.views.account;
    let items = order(view.root);
    let item = match view.focus {
        Focus::Continue => {
            match action {
                Action::Up | Action::Left => {
                    let back = view
                        .last
                        .filter(|item| items.contains(item))
                        .unwrap_or(Item::Field(FieldId::FullName));
                    focus(state, Focus::Item(back));
                }
                Action::Accept => onward(state),
                _ => {}
            }
            return;
        }
        Focus::Item(item) => item,
    };
    let Some(at) = items.iter().position(|one| *one == item) else {
        return focus(state, Focus::default());
    };
    let down = |state: &mut State| match items.get(at + 1) {
        Some(next) => focus(state, Focus::Item(*next)),
        None => focus(state, Focus::Continue),
    };
    match action {
        Action::Up if at > 0 => focus(state, Focus::Item(items[at - 1])),
        Action::Down => down(state),
        // Nothing stands beside an item: across is to Continue, as from
        // every list on its own, and Left from there comes back.
        Action::Right => focus(state, Focus::Continue),
        Action::Accept => match item {
            Item::Root => flip_root(state),
            // A pad has no keys: a board — the shell's under LineXinBar,
            // the installer's elsewhere — and the field about to be typed
            // into comes into view, as one typed into does (`edited`),
            // wherever the pointer that lit it held the form: the board
            // rising over the form's foot would cover it (seen live, the
            // administrator's password clicked half in view). A keyboard is
            // typing already, and Enter in a field is done with it: on to
            // the next.
            Item::Field(id) if state.pad => {
                state.views.account.pointed = false;
                state.type_with_pad(id);
            }
            Item::Field(_) => down(state),
        },
        _ => {}
    }
}

/// What a field shows: its label and placeholder, and what is in it.
fn field_words(state: &State, id: FieldId) -> (&'static str, &'static str, String) {
    let account = &state.draft.account;
    match id {
        FieldId::FullName => (
            i18n::text("field-full-name"),
            i18n::text("field-full-name-placeholder"),
            account.full_name.shown(false),
        ),
        FieldId::Username => (
            i18n::text("field-username"),
            i18n::text("field-username-placeholder"),
            account.username.shown(false),
        ),
        FieldId::Password => (
            i18n::text("field-password"),
            i18n::text("field-password-placeholder"),
            account.password.shown(true),
        ),
        FieldId::Repeat => (
            i18n::text("field-repeat"),
            i18n::text("field-repeat-placeholder"),
            state.repeat.shown(true),
        ),
        FieldId::Hostname => (
            i18n::text("field-hostname"),
            i18n::text("field-hostname-placeholder"),
            account.hostname.shown(false),
        ),
        FieldId::RootPassword => (
            i18n::text("field-root-password"),
            i18n::text("field-root-password-placeholder"),
            state.root_password.shown(true),
        ),
        FieldId::RootRepeat => (
            i18n::text("field-root-repeat"),
            i18n::text("field-repeat-placeholder"),
            state.root_repeat.shown(true),
        ),
        // Not You's: Keyboard's test field and the lists' search.
        FieldId::KeyboardTest
        | FieldId::Search
        | FieldId::PartLabel
        | FieldId::MountPath
        | FieldId::PartSize => ("", "", String::new()),
    }
}

/// The most lines what an item says under itself is given.
const UNDER_LINES: usize = 2;

/// How near an end of the form the scroll must stand for that end's arrow
/// to go, in pixels: under a pixel, where a spring is taken to have arrived.
const ARRIVED: f32 = 0.5;

/// How strong the arrows are: as the pictures of the legend's buttons, which
/// are signs too, not words.
const ARROW_INK: f32 = 0.85;

/// How much of its square the arrow glyph's body is tall (`arrow-down.svg`:
/// 16 of 24), the rest clear: what may reach past the clear space it stands
/// in without anything of it showing there.
const ARROW_BODY: f32 = 16.0 / 24.0;

/// How large an arrow's square is at most, in reference pixels: the
/// legend's pictures' size, which a clear space taller than a panel ever
/// leaves would otherwise outgrow.
const ARROW_LARGEST: f32 = 24.0;

/// How much of the room's foot the arrow under the form is given, where the
/// clear spaces over and under the room are `clear` and an arrow's body is
/// at most `largest` tall: enough for it to stand in as much space as the
/// arrow over the form does, and nothing where it already has that.
fn foot_for_arrow(clear: [f32; 2], largest: f32) -> f32 {
    let [over, under] = clear;
    (over.min(largest) - under).max(0.0)
}

/// One item of the form as it is laid out on this frame: how tall its
/// control is, the lines it says under itself and their ink, and how far
/// the room for those lines is open (0 to 1).
struct Laid {
    control: f32,
    under: Vec<String>,
    ink: (Role, f32),
    open: f32,
}

/// Where each item of the form stands from its top, given how tall each
/// one's control is and how far the room for its lines under it is open,
/// `between` apart; and how tall the whole form is. An item with nothing to
/// say under itself is its control alone.
fn stack(items: &[(f32, f32)], between: f32) -> (Vec<Span>, f32) {
    let mut spans = Vec::with_capacity(items.len());
    let mut y = 0.0;
    for (control, open) in items {
        let tall = control + open;
        spans.push(Span { top: y, tall });
        y += tall + between;
    }
    (spans, (y - between).max(0.0))
}

/// Where the form starts to show, `view` tall, moved on from `wanted`
/// where that leaves less than `peek` of the next item's control showing at
/// the room's foot — the fold in the gap between two items, or in the line
/// said under one — as far as it takes for `peek` of it to show, while the
/// item in hand stays wholly in view and the form is not scrolled past its
/// end (`most`). Ending in a gap, a form that goes on looked finished: the
/// administrator switch under Computer name was there and not seen.
fn peeking(
    wanted: f32,
    spans: &[Span],
    controls: &[f32],
    in_hand: usize,
    peek: f32,
    view: f32,
    most: f32,
) -> f32 {
    let foot = wanted + view;
    let next = spans
        .iter()
        .zip(controls)
        .find(|(span, control)| span.top + **control > foot + ARRIVED);
    let Some((next, _)) = next else {
        return wanted;
    };
    let shown = foot - next.top;
    if shown >= peek {
        return wanted;
    }
    let moved = (wanted + peek - shown).min(most);
    match spans.get(in_hand) {
        Some(hand) if hand.top >= moved => moved.max(wanted),
        _ => wanted,
    }
}

/// How much of the next item's control is kept showing at the room's foot
/// where the form goes on: its label line, inside the soft edge.
const PEEK: f32 = 0.4;

/// The administrator account's switch, on or off, wearing the shell's
/// shield where the form has the room for its marks (`marked`).
fn root_toggle(root: bool) -> Toggle<'static> {
    let note = if root {
        i18n::text("account-root-on")
    } else {
        i18n::text("account-root-off")
    };
    Toggle {
        name: i18n::text("account-root"),
        note: Some(note),
        on: root,
        enabled: true,
        refused: false,
        mark: Some(glyphs::ADMINISTRATOR),
    }
}

/// What a field says under itself, and in which ink: why it cannot be
/// used, as a refusal, or else how strong a password in it looks.
fn said(state: &State, item: Item, id: FieldId) -> Option<(&'static str, (Role, f32))> {
    let refused = refusal(state, item, state.views.account.tried).map(words::invalid);
    match (refused, aside(state, id)) {
        (Some(refusal), _) => Some((refusal, text::REFUSAL)),
        (None, Some(strength)) => Some((strength, text::LESSER)),
        (None, None) => None,
    }
}

/// How strong the password in a password field looks, for the line under
/// it.
fn aside(state: &State, id: FieldId) -> Option<&'static str> {
    let password = match id {
        FieldId::Password => state.draft.account.password.expose(),
        FieldId::RootPassword => state.root_password.expose(),
        _ => return None,
    };
    strength(password).map(strength_words)
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let gap = page.metric(Metric::Gap);
    let buttons_at = room.buttons;
    let mut clear = room.clear;
    let mut room = room.content;
    let root = state.views.account.root;
    let items = order(root);
    // In the middle of the panel. A form taller than the panel is a list,
    // and a pointer is shown how much of it there is, as in every column: by
    // a scroll bar just beside it — in the panel's padding where the form
    // takes the whole room, rather than taking a column's channel from
    // fields that are already as narrow as their words allow.
    let mut form = super::centred(room, page.scaled(WIDEST));
    let width = form[2];
    let (rail, channel) = column::bar_room(page);
    let tall = field::height(page);
    // Each item is its control and, under it, a line for its state — what
    // is wrong with it, or how strong a password looks — so the reason
    // stands where the person is looking, whole, in every language: a
    // reason too long for its line takes a second, and the item grows.
    let line = page.line(Text::Caption);
    let pad = page.metric(Metric::RowPadding);
    let root_switch = root_toggle(root);
    let mut laid: Vec<Laid> = Vec::with_capacity(items.len());
    for item in &items {
        let (control, said) = match *item {
            Item::Root => (toggle::height(page, width, &root_switch), None),
            Item::Field(id) => (tall, said(state, *item, id)),
        };
        let (lines, ink) = match (said, *item) {
            (Some((words, ink)), Item::Field(id)) => {
                // Under the field's words, beside where its mark stands.
                let room = field::words_room_at(page.scaled(1.0), width, id);
                (
                    text::wrapped(page.ui(), Text::Caption, words, room, UNDER_LINES),
                    ink,
                )
            }
            _ => (Vec::new(), text::LESSER),
        };
        laid.push(Laid {
            control,
            under: lines,
            ink,
            open: 0.0,
        });
    }
    // An item is as tall as its control and the lines it says under itself,
    // and no taller: a field with nothing to say stands close under the one
    // before, so more of the form fits the room — on a landscape panel the
    // whole of it, until the administrator's own fields are asked for. A
    // line arriving or going (a password's strength as it is typed, a
    // refusal) opens or closes its room on a spring, so the fields under it
    // move over rather than jump.
    let dt = state.clock.dt;
    let mut opened = Vec::with_capacity(laid.len());
    for (index, one) in laid.iter_mut().enumerate() {
        let wanted = if one.under.is_empty() {
            0.0
        } else {
            gap * 0.2 + line * one.under.len() as f32
        };
        let open = state.views.account.under[index.min(ITEMS - 1)]
            .advance(wanted, dt)
            .max(0.0);
        one.open = if wanted > 0.0 {
            (open / wanted).min(1.0)
        } else {
            0.0
        };
        opened.push((one.control, open));
    }
    let (spans, content) = stack(&opened, gap * 0.6);
    // With a pad, where the form goes on past the room, the arrow under it
    // needs a space to stand in: under a note stacked over the buttons (a
    // demo, a long refusal, on a display standing on its side) the clear
    // space there was 0.8 of a gap and the arrow a speck beside the one over
    // the form. The room gives up as much of its foot as the arrow over it
    // has, so the two are one size.
    if state.pad && content > room[3] {
        let largest = page.scaled(ARROW_LARGEST) * ARROW_BODY;
        let given = foot_for_arrow(clear, largest).min(room[3] * 0.25);
        room[3] -= given;
        form[3] -= given;
        clear[1] += given;
    }

    // The form scrolls to keep the item in hand in view, with a little of
    // the next showing, where the panel is too short for all of it — and
    // rests where the next item below peeks in at the soft edge, so a form
    // that goes on is seen to (`peeking`).
    let focus = state.views.account.focus;
    let held = match focus {
        Focus::Item(item) => Some(item),
        Focus::Continue => state.views.account.last,
    };
    let in_hand = held
        .and_then(|item| items.iter().position(|one| *one == item))
        .unwrap_or(items.len().saturating_sub(1));
    let pointed = state.views.account.pointed;
    let scroll = &mut state.views.account.scroll;
    let most = (content - room[3]).max(0.0);
    let wanted = match spans.get(in_hand) {
        _ if pointed => scroll.wanted().clamp(0.0, most),
        Some(span) => {
            let kept = keep_span_in_view(scroll.wanted(), *span, tall * 0.75, content, room[3]);
            let controls: Vec<f32> = opened.iter().map(|(control, _)| *control).collect();
            peeking(kept, &spans, &controls, in_hand, tall * PEEK, room[3], most)
        }
        None => 0.0,
    };
    let top = scroll.advance(wanted, dt);

    let pressing = state.pressing.through();
    let now = state.clock.now;
    let air = tall * 0.5;
    // How far a control drawn reaches past the room's top and past its foot.
    let mut past = [0.0f32; 2];
    let from = page.ui().written();
    for ((item, span), one) in items.iter().zip(&spans).zip(&laid) {
        let Laid {
            control,
            under: lines,
            ink: (role, alpha),
            open,
        } = one;
        let rect = [form[0], room[1] + span.top - top, width, *control];
        let here = lit && focus == Focus::Item(*item);
        // The field the light stands in is said to be lit even while it is
        // scrolled wholly out of view. The room ending over LineXinBar's
        // board as it rises leaves a field low on the form out of view for a
        // few frames, until the form has scrolled it back; a field said to
        // be left lets go of its text, and the shell put its board away 134
        // ms after raising it (seen live, the administrator's password).
        if let (true, Item::Field(id)) = (here, *item) {
            state.in_field(id);
        }
        // Wholly out of view, it is not drawn at all — and so cannot be
        // clicked where it cannot be seen.
        if rect[1] + span.tall <= room[1] || rect[1] >= room[1] + room[3] {
            continue;
        }
        past[0] = past[0].max(room[1] - rect[1]);
        past[1] = past[1].max(rect[1] + control - (room[1] + room[3]));
        let under_top = rect[1] + control + gap * 0.2;
        let under_w = match *item {
            Item::Field(id) => field::words_room_at(page.scaled(1.0), rect[2], id),
            Item::Root => (rect[2] - pad * 2.0).max(0.0),
        };
        for (index, words) in lines.iter().enumerate() {
            let under = [
                rect[0] + rect[2] - pad - under_w,
                under_top + index as f32 * line,
                under_w,
                line,
            ];
            // Coming in as its room opens, so a line arriving is not
            // written over the field under it before there is room for it.
            let alpha = alpha * open * open;
            let ui = page.ui();
            text::label_faded(ui, under, Text::Caption, words, *role, alpha, Align::Left);
        }
        match *item {
            Item::Root => {
                let light = here.then_some(&mut state.light);
                toggle::draw(page, rect, root_switch, light, ROOT, pressing, dt);
            }
            Item::Field(id) => {
                let (label, placeholder, shown) = field_words(state, id);
                let caret = state.typing_into() == Some(id);
                let light = here.then_some(&mut state.light);
                field::draw(
                    page,
                    rect,
                    id,
                    Look {
                        label,
                        shown: &shown,
                        placeholder,
                        refused: None,
                        caret,
                    },
                    light,
                    pressing,
                    dt,
                    now,
                );
            }
        }
    }
    let to = page.ui().written();
    page.ui().cut_between(
        from,
        to,
        [room[0] - air, room[1], room[2] + air * 2.0, room[3]],
    );
    // What is cut off answers no click. Continue, the legend and the board
    // are written after it, and still answer theirs.
    let [above, below] = past;
    if above > 0.0 {
        page.ui()
            .spot(BEYOND, [form[0], room[1] - above, width, above]);
    }
    if below > 0.0 {
        page.ui()
            .spot(BEYOND, [form[0], room[1] + room[3], width, below]);
    }
    // Faded away at an end that really continues, as a column is.
    let band = tall * 0.55;
    page.ui().soft_edges(
        form,
        band,
        (top / band).min(1.0),
        ((most - top) / band).min(1.0),
    );
    // With a pad there is no bar, and the light says where it is but not
    // that the form goes on past the room — the administrator's fields below
    // the fold, the fields above once scrolled: the arrows the family's
    // menus draw where a list carries on past what is drawn say so, at an
    // end that really continues and only there. In the clear space just
    // past that end, as the menus keep theirs in a strip of their own (drawn
    // inside the room, the soft edge faded them with the form), and as large
    // as that space lets the arrow itself be: the menus' own size is too
    // small to see from a sofa.
    if most > 0.0 && state.pad {
        let icons = page.icons();
        let largest = page.scaled(ARROW_LARGEST);
        let [over, under] = clear;
        for (showing, name, space, from) in [
            (top > ARRIVED, "arrow-up", over, room[1] - over),
            (most - top > ARRIVED, "arrow-down", under, room[1] + room[3]),
        ] {
            let arrow = (space / ARROW_BODY).min(largest);
            if showing && arrow > 0.0 {
                let rect = [
                    form[0] + (width - arrow) * 0.5,
                    from + (space - arrow) * 0.5,
                    arrow,
                    arrow,
                ];
                page.ui()
                    .icon_tinted(rect, name, icons, Role::Text, ARROW_INK);
            }
        }
    }
    // The bar, for a pointer only: a pad's light already says where it is.
    let track = [form[0] + width + channel, room[1], rail, room[3]];
    let shown = most > 0.0 && !state.pad;
    state.views.account.track = if shown { track } else { [0.0; 4] };
    state.views.account.run = if shown { room[3] / content } else { 0.0 };
    if shown {
        let held = page.dragging(BAR).is_some();
        let reach = gap * 0.6;
        let ui = page.ui();
        ui.scroll_bar(track, top / content, room[3] / content, held);
        ui.spot(
            BAR,
            [track[0] - reach, track[1], track[2] + reach * 2.0, track[3]],
        );
    }

    super::lone_button(
        state,
        page,
        buttons_at,
        verb(),
        lit && focus == Focus::Continue,
    );
}

/// LineXinBar's board rose over the window: the field in hand comes into
/// view above it, wherever the pointer that lit or clicked it held the form
/// — the board rising over the form's foot would cover it, however it was
/// summoned (a pad's South, a click, a tap).
pub fn shells_board_rose(state: &mut State) {
    if let Focus::Item(Item::Field(_)) = state.views.account.focus {
        state.views.account.pointed = false;
    }
}

pub fn hover(state: &mut State, id: u32) {
    let items = order(state.views.account.root);
    let to = if id == ROOT {
        Focus::Item(Item::Root)
    } else if super::button_of(id, 1).is_some() {
        Focus::Continue
    } else {
        match items
            .iter()
            .find(|item| matches!(item, Item::Field(field) if field.spot() == id))
        {
            Some(item) => Focus::Item(*item),
            None => return,
        }
    };
    let view = &mut state.views.account;
    if let (Focus::Item(item), Focus::Continue) = (view.focus, to) {
        view.last = Some(item);
    }
    view.focus = to;
    view.pointed = true;
}

pub fn pressed(state: &mut State, page: &mut Page) {
    let items = order(state.views.account.root);
    // A hand on the bar: the light follows it, and with the light the form,
    // as in a column. Taken first: while the bar is held it owns the pointer
    // wherever the pointer has wandered.
    let grabbed = page.pressed(BAR);
    if let Some([_, y]) = page.dragging(BAR) {
        let View { track, run, .. } = state.views.account;
        if track[3] > 0.0 {
            let at = column::row_under_bar(y, track, run, items.len());
            focus(state, Focus::Item(items[at]));
        }
        return;
    }
    if grabbed {
        return;
    }
    for item in items {
        if let Item::Field(id) = item {
            if page.pressed(id.spot()) {
                // A click puts the light in the field, where a keyboard
                // types; the installer raises no board of its own for a
                // pointer, though LineXinBar raises its own over a field
                // taking text while it has a controller in hand (and the form
                // then scrolls the field above it, `shells_board_rose`). It
                // points as a hover does: a field half out of view is not
                // scrolled out from under the pointer that clicked it.
                focus(state, Focus::Item(item));
                state.views.account.pointed = true;
                state.clicked_field(id);
                return;
            }
        }
    }
    if page.pressed(ROOT) {
        state.views.account.focus = Focus::Item(Item::Root);
        state.views.account.pointed = true;
        flip_root(state);
        return;
    }
    if super::lone_pressed(page, verb()) {
        state.views.account.focus = Focus::Continue;
        onward(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::DEFAULT_HOSTNAME;
    use crate::ui::app::Screen;
    use crate::ui::field::Edit;
    use crate::ui::motion::FRAME;

    fn on_account() -> State {
        let mut state = State::demo(true);
        state.go(Screen::Account);
        state
    }

    fn type_into(state: &mut State, field: FieldId, text: &str) {
        state.edit(field, &Edit::Wrote(text.into()));
    }

    /// A form that can be installed: a name, its username, a password typed
    /// twice and the default computer name.
    fn filled() -> State {
        let mut state = on_account();
        type_into(&mut state, FieldId::FullName, "Zoë O'Brien");
        type_into(&mut state, FieldId::Password, "correct horse");
        type_into(&mut state, FieldId::Repeat, "correct horse");
        state
    }

    #[test]
    fn the_username_follows_the_first_word_of_the_full_name() {
        assert_eq!(derive_username("Zoë O'Brien"), "zoe");
        assert_eq!(derive_username("  Łukasz Żółć"), "lukasz");
        assert_eq!(derive_username("Jürgen-Groß"), "jurgen-gross");
        assert_eq!(derive_username("Анна Каренина"), "anna");
        assert_eq!(derive_username("3D Artist"), "d");
        assert_eq!(derive_username("O'Neil"), "oneil");
        assert_eq!(derive_username("李雷"), "");
        assert_eq!(derive_username(&"a".repeat(40)).len(), 32);
        for name in ["Zoë O'Brien", "Анна", "Jürgen-Groß"] {
            assert!(validate_username(&derive_username(name)).is_ok(), "{name}");
        }
    }

    #[test]
    fn hovering_lights_a_field_the_switch_and_continue_and_types_nowhere_else() {
        let mut state = on_account();
        hover(&mut state, FieldId::Hostname.spot());
        assert_eq!(
            state.views.account.focus,
            Focus::Item(Item::Field(FieldId::Hostname))
        );
        assert!(state.views.account.pointed);
        hover(&mut state, ROOT);
        assert_eq!(state.views.account.focus, Focus::Item(Item::Root));
        assert!(!state.views.account.root, "a hover switched it");
        hover(&mut state, spots::BUTTONS);
        assert_eq!(state.views.account.focus, Focus::Continue);
        assert_eq!(state.views.account.last, Some(Item::Root));
        // The administrator's own fields are not there until it is on.
        hover(&mut state, FieldId::RootPassword.spot());
        assert_eq!(state.views.account.focus, Focus::Continue);
        // A direction lets the form scroll to the light again.
        act(&mut state, Action::Up, None);
        assert!(!state.views.account.pointed);
        assert_eq!(state.screen, Screen::Account);
    }

    /// A field the pointer lit, holding the form still, comes into view
    /// once a pad's Accept brings a board to type into it with.
    #[test]
    fn a_board_summoned_brings_a_pointed_field_into_view() {
        let mut state = on_account();
        hover(&mut state, FieldId::Hostname.spot());
        assert!(state.views.account.pointed);
        state.pad = true;
        act(&mut state, Action::Accept, None);
        assert!(state.board.is_some(), "no board was raised");
        assert!(
            !state.views.account.pointed,
            "the form held the field out of view"
        );
    }

    #[test]
    fn a_field_with_nothing_to_say_stands_close_under_the_one_before() {
        let (spans, content) = stack(&[(75.0, 0.0), (75.0, 30.0), (75.0, 0.0)], 8.0);
        assert_eq!(spans[1].top, 83.0, "a line was kept under a silent field");
        assert_eq!(spans[1].tall, 105.0);
        assert_eq!(spans[2].top, 196.0, "the line said did not push on");
        assert_eq!(content, 271.0);
        assert_eq!(stack(&[], 8.0), (Vec::new(), 0.0));
    }

    #[test]
    fn a_form_that_goes_on_rests_with_the_next_item_peeking_in() {
        // Five items 75 tall, 10 apart: tops 0, 85, 170, 255, 340.
        let (spans, content) = stack(&[(75.0, 0.0); 5], 10.0);
        let controls = [75.0; 5];
        let view = 245.0;
        let most = content - view;
        // The fold at 245, in the gap after the third: the fourth peeks in
        // 30 of it, the first scrolled 40 out of view at the top.
        assert_eq!(peeking(0.0, &spans, &controls, 1, 30.0, view, most), 40.0);
        // Not with the light on the first, which would be cut.
        assert_eq!(peeking(0.0, &spans, &controls, 0, 30.0, view, most), 0.0);
        // Enough of the next showing already: left where it was.
        assert_eq!(peeking(0.0, &spans, &controls, 1, 30.0, 290.0, most), 0.0);
        // Nothing below: left where it was.
        assert_eq!(peeking(most, &spans, &controls, 4, 30.0, view, most), most);
    }

    #[test]
    fn the_arrow_under_the_form_is_given_the_room_the_one_over_it_has() {
        // A note stacked over the buttons leaves 8 under the room, 30 over.
        assert_eq!(foot_for_arrow([30.0, 8.0], 16.0), 8.0);
        assert_eq!(foot_for_arrow([12.0, 8.0], 16.0), 4.0);
        // Room enough under it already: nothing is taken.
        assert_eq!(foot_for_arrow([30.0, 20.0], 16.0), 0.0);
        assert_eq!(foot_for_arrow([10.0, 12.0], 16.0), 0.0);
    }

    #[test]
    fn a_line_said_under_a_field_opens_its_room_and_arriving_it_is_there() {
        let mut state = on_account();
        let under = &mut state.views.account.under[2];
        assert_eq!(under.advance(30.0, FRAME), 30.0, "arriving, it opened");
        let opening = under.advance(0.0, FRAME);
        assert!(opening > 0.0 && opening < 30.0, "it closed at once");
        enter(&mut state);
        let under = &mut state.views.account.under[2];
        assert_eq!(under.advance(0.0, FRAME), 0.0, "arriving again, it closed");
    }

    #[test]
    fn a_key_to_continue_leaves_the_form_where_the_pointer_held_it() {
        let mut state = on_account();
        hover(&mut state, FieldId::Hostname.spot());
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.account.focus, Focus::Continue);
        assert!(
            state.views.account.pointed,
            "a key to Continue scrolled the form under the pointer"
        );
        // Brought back by a key, the light's field is scrolled to again.
        act(&mut state, Action::Left, None);
        assert_eq!(
            state.views.account.focus,
            Focus::Item(Item::Field(FieldId::Hostname))
        );
        assert!(!state.views.account.pointed);
        // Arriving on the screen again holds nothing.
        hover(&mut state, FieldId::Username.spot());
        enter(&mut state);
        assert!(!state.views.account.pointed);
    }

    #[test]
    fn a_field_the_pointer_lit_is_scrolled_into_view_once_it_is_typed_in() {
        let mut state = on_account();
        flip_root(&mut state);
        let field = FieldId::RootRepeat;
        // Lit by the pointer half under the soft edge, as a hover or a click
        // (`pressed`) leaves it: the form holds still.
        hover(&mut state, field.spot());
        assert!(state.views.account.pointed);
        // Another field changing (the username following the full name)
        // does not let go: only the field in hand being typed in does.
        state.edit(FieldId::FullName, &Edit::Wrote("Ola".into()));
        assert!(state.views.account.pointed);
        // The frame says the light stands in it; a key is typed there.
        state.standing_in = Some(field);
        state.type_keys(vec![Edit::Wrote("x".into())]);
        assert!(
            !state.views.account.pointed,
            "typed into below the room's foot, out of sight"
        );
        // And the form scrolling under a resting pointer walks nothing: a
        // field typed in is not left by hovering.
        state.hover(FieldId::Hostname.spot());
        assert_eq!(state.views.account.focus, Focus::Item(Item::Field(field)));
    }

    #[test]
    fn a_refusal_carries_the_light_to_its_field_and_scrolls_to_it() {
        let mut state = on_account();
        // Continue reached by the pointer and pressed with the form empty.
        let (refused, _) = check(&state).unwrap_err();
        hover(&mut state, spots::BUTTONS);
        assert!(state.views.account.pointed);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.account.focus, Focus::Item(refused));
        assert!(
            !state.views.account.pointed,
            "the field refused stayed wherever the form was scrolled"
        );
    }

    #[test]
    fn a_username_typed_by_hand_is_no_longer_followed() {
        let mut state = on_account();
        type_into(&mut state, FieldId::FullName, "Alex");
        assert_eq!(state.draft.account.username, "alex");
        type_into(&mut state, FieldId::Username, "x");
        type_into(&mut state, FieldId::FullName, "ander");
        assert_eq!(state.draft.account.username, "alexx");
        // Rubbed out to nothing, it follows again.
        for _ in 0..5 {
            state.edit(FieldId::Username, &Edit::Rubbed);
        }
        type_into(&mut state, FieldId::FullName, " Smith");
        assert_eq!(state.draft.account.username, "alexander");
    }

    #[test]
    fn strength_is_rated_as_the_legacy_installer_rated_it() {
        assert_eq!(strength(""), None);
        assert_eq!(strength("abc"), Some(Strength::Weak));
        assert_eq!(strength("abcdefgh1"), Some(Strength::Fair));
        assert_eq!(strength("Abcdefgh1"), Some(Strength::Good));
        assert_eq!(strength("Abcdefgh1!"), Some(Strength::Strong));
        assert_eq!(strength("Zażółć gęślą"), Some(Strength::Fair));
    }

    #[test]
    fn an_empty_form_is_not_called_wrong_until_continue_is_pressed() {
        let mut state = on_account();
        assert_eq!(state.draft.account.hostname, DEFAULT_HOSTNAME);
        for item in order(false) {
            assert_eq!(refusal(&state, item, false), None, "{item:?}");
        }
        state.views.account.focus = Focus::Continue;
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Account, "an empty form went on");
        assert!(state.note.is_some());
        assert_eq!(
            state.views.account.focus,
            Focus::Item(Item::Field(FieldId::Username)),
            "the light was not carried to what is wrong"
        );
    }

    #[test]
    fn a_second_typing_that_differs_is_said_on_its_own_row() {
        let mut state = filled();
        state.edit(FieldId::Repeat, &Edit::Rubbed);
        assert_eq!(
            refusal(&state, Item::Field(FieldId::Repeat), false),
            Some(Invalid::PasswordMismatch)
        );
        assert!(check(&state).is_err());
    }

    #[test]
    fn a_filled_form_goes_on() {
        let mut state = filled();
        assert!(check(&state).is_ok());
        onward(&mut state);
        assert_eq!(state.screen, Screen::Review);
    }

    #[test]
    fn the_administrator_account_asks_for_its_own_password_twice() {
        let mut state = filled();
        flip_root(&mut state);
        assert!(state.draft.account.root_password.is_some());
        assert_eq!(
            check(&state),
            Err((
                Item::Field(FieldId::RootPassword),
                Invalid::RootPasswordEmpty
            ))
        );
        type_into(&mut state, FieldId::RootPassword, "another one");
        assert_eq!(
            check(&state),
            Err((Item::Field(FieldId::RootRepeat), Invalid::PasswordMismatch))
        );
        type_into(&mut state, FieldId::RootRepeat, "another one");
        assert!(check(&state).is_ok());
        assert_eq!(
            state
                .draft
                .account
                .root_password
                .as_ref()
                .map(Secret::expose),
            Some("another one")
        );
        flip_root(&mut state);
        assert!(state.draft.account.root_password.is_none());
        assert!(state.root_password.is_empty(), "its password was kept");
    }

    #[test]
    fn with_a_pad_accept_on_a_field_opens_the_board_and_with_a_keyboard_goes_on() {
        let mut state = on_account();
        state.pad = true;
        act(&mut state, Action::Accept, None);
        assert_eq!(state.typing_into(), Some(FieldId::FullName));
        let mut state = on_account();
        state.pad = false;
        act(&mut state, Action::Accept, None);
        assert!(state.board.is_none());
        assert_eq!(
            state.views.account.focus,
            Focus::Item(Item::Field(FieldId::Username))
        );
    }

    /// What the harness types goes into the field the light is on.
    fn type_here(state: &mut State, edit: Edit) {
        let Focus::Item(Item::Field(field)) = state.views.account.focus else {
            panic!("typed with the light on {:?}", state.views.account.focus);
        };
        state.edit(field, &edit);
    }

    /// The harness's walks through You with a keyboard
    /// (`tests/e2e/scenarios.py`, `walk_from_account` and
    /// `walk_from_account_with_administrator`): Enter down the one column,
    /// each field where its comment says, and on to Review, where Install
    /// asks the last question.
    #[test]
    fn the_harness_walks_down_you_to_review() {
        for administrator in [false, true] {
            let mut state = on_account();
            let at = |state: &State| state.views.account.focus;
            let field = |id| Focus::Item(Item::Field(id));
            type_here(&mut state, Edit::Wrote("Test Person".into()));
            act(&mut state, Action::Accept, None); // on to the username
            assert_eq!(at(&state), field(FieldId::Username));
            assert_eq!(state.draft.account.username, "test", "it followed the name");
            for _ in 0..4 {
                type_here(&mut state, Edit::Rubbed);
            }
            type_here(&mut state, Edit::Wrote("tester".into()));
            act(&mut state, Action::Accept, None); // on to the password
            assert_eq!(at(&state), field(FieldId::Password));
            type_here(&mut state, Edit::Wrote("E2e-0123456789".into()));
            act(&mut state, Action::Accept, None); // on to its second typing
            assert_eq!(at(&state), field(FieldId::Repeat));
            type_here(&mut state, Edit::Wrote("E2e-0123456789".into()));
            act(&mut state, Action::Accept, None); // on to the computer's name
            assert_eq!(at(&state), field(FieldId::Hostname));
            act(&mut state, Action::Accept, None); // on to the switch
            assert_eq!(at(&state), Focus::Item(Item::Root));
            if administrator {
                act(&mut state, Action::Accept, None); // on: its two fields
                assert!(state.views.account.root);
                act(&mut state, Action::Down, None); // its password
                assert_eq!(at(&state), field(FieldId::RootPassword));
                type_here(&mut state, Edit::Wrote("E2e-root-0123".into()));
                act(&mut state, Action::Accept, None); // its second typing
                assert_eq!(at(&state), field(FieldId::RootRepeat));
                type_here(&mut state, Edit::Wrote("E2e-root-0123".into()));
                act(&mut state, Action::Accept, None); // past the last field
            } else {
                act(&mut state, Action::Down, None); // Continue
            }
            assert_eq!(at(&state), Focus::Continue);
            act(&mut state, Action::Accept, None);
            assert_eq!(state.screen, Screen::Review, "{:?}", state.note);
            let account = &state.draft.account;
            assert_eq!(
                (account.full_name.as_str(), account.username.as_str()),
                ("Test Person", "tester")
            );
            assert_eq!(account.hostname, DEFAULT_HOSTNAME);
            assert_eq!(account.root_password.is_some(), administrator);
            // Install, where Review's light starts: the last question.
            state.act(Action::Accept, None);
            assert_eq!(state.question_up(), Some(crate::ui::app::Question::Erase));
        }
    }

    #[test]
    fn one_column_walks_down_in_reading_order_to_continue() {
        let mut state = on_account();
        // Nothing stands beside a field: across is to Continue, and back
        // across is to the field the light left.
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.account.focus, Focus::Continue);
        act(&mut state, Action::Left, None);
        assert_eq!(
            state.views.account.focus,
            Focus::Item(Item::Field(FieldId::FullName))
        );
        let mut walked = vec![state.views.account.focus];
        for _ in 0..6 {
            act(&mut state, Action::Down, None);
            walked.push(state.views.account.focus);
        }
        let mut read: Vec<Focus> = order(false).into_iter().map(Focus::Item).collect();
        read.push(Focus::Continue);
        assert_eq!(walked, read);
        assert_eq!(
            order(false).last(),
            Some(&Item::Root),
            "the administrator switch is not last"
        );
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.account.focus, Focus::Item(Item::Root));
        // Switched on, its own two fields stand under it.
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Down, None);
        assert_eq!(
            state.views.account.focus,
            Focus::Item(Item::Field(FieldId::RootPassword))
        );
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.account.focus, Focus::Continue);
    }
}
