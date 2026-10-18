//! Language: which language the installed system speaks — the 65 system
//! locales by their own names, the English name under each, and their
//! country's flag before both, as tall as the two lines together, as the
//! legacy installer wrote a flag before each name (`flags`) — and, where one
//! of the ten catalogs speaks it, the language the installer itself switches
//! to at once.
//!
//! One scrolled column and Continue under it, the column as wide as its
//! longest name needs and standing in the middle of the panel, as Desktop's
//! Options and Review stand — not stretched from one side of the panel to
//! the other. Options (North, the Menu key, F10) brings a search field up
//! over it (`search`), which narrows it to the languages whose own name or
//! English name has every word typed in it. Choosing a language is the press
//! that answers the question: it is taken, the installer's words switch where
//! a catalog speaks it, the time zone and keyboard are guessed from it
//! (unless the person already chose those), and the screen goes on to the
//! time zone. Continue keeps the language in force and goes on.
//!
//! On a live session the shell the installer runs in follows the language
//! chosen too: where it is one of LineXinBar's ten, it is written into the
//! live account's `~/.config/lxb/shell.toml` as `language`, which LineXinBar
//! reads as the language it was set to. The 3.0 image builds those ten
//! locales for that.

use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};

use lxb_app::lxb_toolkit::{input::Action, metrics::Metric, sound::Sound, typography::Text};
use lxb_app::{Page, Scroll};

use super::{Head, Room};
use crate::i18n;
use crate::model::catalogs::{self, Language};
use crate::ui::app::{Screen, State};
use crate::ui::button::Button;
use crate::ui::column::{self, Column, Corners, Look, Pick, Row};
use crate::ui::field::{self, Edit, FieldId};
use crate::ui::flags;
use crate::ui::legend::Hint;
use crate::ui::panel::Note;
use crate::ui::search::{Backed, Search};
use crate::ui::spots;

/// The numbers the column's rows answer to.
const IDS: spots::Column = spots::Column::nth(0);

/// Where the light stands on the Language screen.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// In the search field over the list, while it is up.
    Search,
    #[default]
    List,
    Continue,
}

/// What the Language screen keeps between frames.
#[derive(Debug, Default, Clone)]
pub struct View {
    pub column: Column,
    pub focus: Focus,
    /// The search over the list.
    pub search: Search,
    /// How wide the column's rows need to be, and the scale it was measured
    /// at: the names never change, so they are measured once a scale.
    widest: Option<(u32, f32)>,
}

impl View {
    pub fn settle(&mut self) {
        self.column.settle();
    }
}

/// The language in force, if it is one of those offered.
pub fn current(state: &State) -> Option<&'static Language> {
    catalogs::languages()
        .iter()
        .find(|language| language.locale == state.draft.region.locale)
}

/// The language a session's `LANG` names, if it is one of those offered:
/// matched on the language and country, whatever the charset is spelt as
/// (`pl_PL.utf8` is `pl_PL.UTF-8`).
pub fn from_environment(lang: &str) -> Option<&'static Language> {
    let name = |locale: &str| {
        locale
            .split(['.', '@'])
            .next()
            .unwrap_or_default()
            .to_string()
    };
    let wanted = name(lang);
    if wanted.is_empty() {
        return None;
    }
    catalogs::languages()
        .iter()
        .find(|language| name(language.locale) == wanted)
}

/// The languages the list shows, by their place among those offered: every
/// one, or those the search finds by their own name or their English one.
fn found(state: &State) -> Vec<usize> {
    let search = &state.views.language.search;
    catalogs::languages()
        .iter()
        .enumerate()
        .filter(|(_, language)| search.finds(&[language.native, language.english]))
        .map(|(at, _)| at)
        .collect()
}

/// The languages the list shows, as rows (`row_of`).
fn rows(state: &State) -> Vec<Row> {
    let languages = catalogs::languages();
    found(state)
        .into_iter()
        .map(|at| &languages[at])
        .map(|language| row_of(language, language.locale == state.draft.region.locale))
        .collect()
}

/// A language's row: its country's flag (`flags`, the white flag for a
/// locale naming none), its own name, and the English one under it where
/// that says something else; `chosen` where it is the language in force.
pub fn row_of(language: &Language, chosen: bool) -> Row {
    let row = Row::new(language.native)
        .flag(flags::of(language.country()))
        .chosen(chosen);
    if language.english == language.native {
        row
    } else {
        row.note(language.english)
    }
}

/// Take the language at row `index` of the list and go on to the time zone.
fn choose(state: &mut State, index: usize) {
    let Some(language) = found(state)
        .get(index)
        .and_then(|at| catalogs::languages().get(*at))
    else {
        return;
    };
    state.draft.region.locale = language.locale.to_string();
    // A language no catalog speaks leaves the installer's words as they
    // were, as the legacy installer did, rather than dropping to English.
    if let Some(tag) = language.ui {
        state.speak(tag);
    }
    // And the live session's shell, likewise, where it speaks the language.
    if let Some(tag) = shell_language(language.locale) {
        state.tell_shell(tag);
    }
    // Time zone's places in it, whatever the installer's own words speak.
    state.name_the_places();
    guess_the_rest(state);
    state.sound(Sound::Press);
    if let Some(next) = Screen::Language.next() {
        state.go(next);
    }
}

/// LineXinBar's ten languages, by the tag its `shell.toml` writes for each
/// (LineXinBar's `lxb-desktop`, `i18n::Language::key`).
pub const SHELL_LANGUAGES: [&str; 10] = [
    "de", "en-GB", "en-US", "es", "fr", "hi", "pl", "pt-BR", "ru", "zh-CN",
];

/// The tag LineXinBar's settings take for the language with `locale`, when
/// the shell speaks it: the installer's own catalog for it, which carries the
/// same tag (the English of Australia and Canada is the British catalog,
/// Canada's French the French one). A language neither has a catalog for —
/// European Portuguese and Traditional Chinese among them — has none, and
/// leaves the shell's settings alone.
pub fn shell_language(locale: &str) -> Option<&'static str> {
    catalogs::languages()
        .iter()
        .find(|language| language.locale == locale)?
        .ui
        .filter(|tag| SHELL_LANGUAGES.contains(tag))
}

/// Where LineXinBar keeps the settings of the account running the
/// installer, found as the shell finds it: `$XDG_CONFIG_HOME` when that is
/// an absolute path, else `~/.config`, then `lxb/shell.toml`.
pub fn shell_settings(xdg_config_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let absolute = |dir: &OsStr| Some(PathBuf::from(dir)).filter(|path| path.is_absolute());
    let config = xdg_config_home
        .and_then(absolute)
        .or_else(|| home.and_then(absolute).map(|home| home.join(".config")))?;
    Some(config.join("lxb").join("shell.toml"))
}

/// The key of a `key = value` line, unquoted; `None` for anything else.
fn key_of(line: &str) -> Option<&str> {
    let bare = line.trim();
    if bare.starts_with('#') || bare.starts_with('[') {
        return None;
    }
    let (key, _) = bare.split_once('=')?;
    Some(key.trim().trim_matches(|c| c == '"' || c == '\''))
}

/// `shell.toml` with `language = "<tag>"`, everything else kept: a
/// `language` line among the top-level keys (before the first table) is
/// replaced where it stands, and any second one dropped; with none, the line
/// goes in front of the first setting, which keeps it a top-level key and
/// after the comments that open the file. A file that is not there is the
/// one line.
pub fn with_shell_language(existing: Option<&str>, tag: &str) -> String {
    let line = format!("language = \"{tag}\"");
    let Some(existing) = existing else {
        return format!("{line}\n");
    };
    let lines: Vec<&str> = existing.split_inclusive('\n').collect();
    let top_level = lines
        .iter()
        .position(|piece| piece.trim_start().starts_with('['))
        .unwrap_or(lines.len());
    let replacing = lines[..top_level]
        .iter()
        .any(|piece| key_of(piece) == Some("language"));
    let mut out = String::with_capacity(existing.len() + line.len() + 1);
    let mut placed = false;
    for (index, piece) in lines.iter().enumerate() {
        let top = index < top_level;
        if top && replacing && key_of(piece) == Some("language") {
            if !placed {
                let ending = &piece[piece.trim_end_matches(['\n', '\r']).len()..];
                out.push_str(&line);
                out.push_str(if ending.is_empty() { "\n" } else { ending });
                placed = true;
            }
            continue;
        }
        let bare = piece.trim();
        if !replacing && !placed && !bare.is_empty() && !bare.starts_with('#') {
            out.push_str(&line);
            out.push('\n');
            placed = true;
        }
        out.push_str(piece);
    }
    if !placed {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Writes `language = "<tag>"` into the settings file at `path`, merged
/// with what it says ([`with_shell_language`]), and atomically: into a file
/// beside it first, flushed to the disk, then renamed over it, so the shell
/// reading it never sees half a file. Answers whether anything changed. A
/// file that cannot be read (not text, not ours to read) is left alone.
pub fn write_shell_language(path: &Path, tag: &str) -> std::io::Result<bool> {
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(trouble) if trouble.kind() == std::io::ErrorKind::NotFound => None,
        Err(trouble) => return Err(trouble),
    };
    let merged = with_shell_language(existing.as_deref(), tag);
    if existing.as_deref() == Some(merged.as_str()) {
        return Ok(false);
    }
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(std::io::Error::other("the settings file has no directory"));
    };
    std::fs::create_dir_all(dir)?;
    let mut temporary = name.to_os_string();
    temporary.push(format!(".lxb-installer-{}", std::process::id()));
    let temporary = dir.join(temporary);
    let written = (|| {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(merged.as_bytes())?;
        if let Ok(meta) = std::fs::metadata(path) {
            file.set_permissions(meta.permissions())?;
        }
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written.map(|()| true)
}

/// Has the live session's shell follow `tag`: written into the settings of
/// the account the installer runs as, the live user. Only ever called by the
/// frame, on a live system; whatever goes wrong is said on the error stream
/// and changes nothing else.
pub fn tell_the_live_shell(tag: &str) {
    let xdg = std::env::var_os("XDG_CONFIG_HOME");
    let home = std::env::var_os("HOME");
    let Some(path) = shell_settings(xdg.as_deref(), home.as_deref()) else {
        eprintln!("lxb-installer: the live session's shell has no settings folder to follow {tag}");
        return;
    };
    match write_shell_language(&path, tag) {
        Ok(true) => eprintln!(
            "lxb-installer: the live session's shell follows {tag} ({})",
            path.display()
        ),
        Ok(false) => {}
        Err(trouble) => eprintln!(
            "lxb-installer: the live session's shell keeps its language: {}: {trouble}",
            path.display()
        ),
    }
}

/// The time zone and keyboard a person speaking the language in force most
/// likely wants — each only where the person has not chosen one themselves.
///
/// The keyboard guess is checked against the layouts offered: the layout and
/// variant together, else the layout alone, else `us`; a guess the list does
/// not have would be a keyboard the installed system cannot set.
///
/// A guess that changes what is in force puts it where nobody has seen it:
/// Time zone's or Keyboard's Continue then shows it before it goes on again
/// (`timezone::View::seen`, `keyboard::View::seen`). One that comes to what
/// was in force already leaves it seen.
pub fn guess_the_rest(state: &mut State) {
    let locale = state.draft.region.locale.clone();
    if !state.chose_zone {
        if let Some(zone) = catalogs::guess_zone(&locale, &state.zones) {
            if zone != state.draft.region.timezone {
                state.views.timezone.seen.forget();
            }
            state.draft.region.timezone = zone;
        }
    }
    if !state.chose_keyboard {
        let (layout, variant) = catalogs::guess_keyboard(&locale);
        let (layout, variant) = match state.layouts.iter().find(|one| one.layout == layout) {
            Some(known) if known.variants.iter().any(|one| one.variant == variant) => {
                (layout, variant)
            }
            Some(_) => (layout, String::new()),
            None => ("us".to_string(), String::new()),
        };
        let keyboard = &state.draft.region.keyboard;
        if keyboard.layout != layout || keyboard.variant != variant {
            state.views.keyboard.seen.forget();
        }
        super::keyboard::set(state, &layout, &variant);
    }
}

/// The pictures of the Language screen: the column standing on the
/// language in force, which `enter` has already done (`language`); and the
/// search up, something typed in it, the light in it (`language-search`).
pub fn stage(state: &mut State, scene: &str) {
    if scene == "language-search" {
        to_search(state);
        searched(state, &Edit::Wrote("port".into()));
    }
}

fn verb() -> Button<'static> {
    Button::primary(i18n::text("button-continue"))
}

pub fn enter(state: &mut State) {
    state.views.language.search.close();
    let rows = rows(state);
    let view = &mut state.views.language;
    view.focus = Focus::List;
    view.column.start_at_chosen(&rows);
}

pub fn head(_state: &State) -> Head {
    Head::new(
        "setting-name-server",
        i18n::text("language-title"),
        i18n::text("language-caption"),
    )
}

pub fn hints(state: &State) -> Vec<Hint> {
    super::search_hints(state, state.views.language.focus == Focus::Search)
}

pub fn note(_state: &State) -> Option<Note> {
    None
}

pub fn act(state: &mut State, action: Action, scroll: Option<Scroll>) {
    // A wheel turned over the list moves the list, wherever the light was.
    if scroll.is_some_and(|scroll| Column::owns(scroll.spot, IDS)) {
        state.views.language.focus = Focus::List;
    }
    let rows = rows(state);
    let pad = state.pad;
    let view = &mut state.views.language;
    match (view.focus, action) {
        (_, Action::Menu) => to_search(state),
        (_, Action::Back) => back(state),
        (Focus::Search, Action::Accept) if pad => state.type_with_pad(FieldId::Search),
        // Enter in the field, or Down out of it: the light lands on the
        // first language found.
        (Focus::Search, Action::Accept | Action::Down) => {
            if !land(state) {
                state.sound(Sound::Error);
            }
        }
        (Focus::Search, Action::Right) => {
            view.focus = Focus::Continue;
            state.sound(Sound::Move);
        }
        (Focus::List, Action::Up) => {
            if view.column.step(&rows, -1) {
                state.sound(Sound::Move);
            } else if view.search.is_open() {
                // Over the first row, the search field.
                view.focus = Focus::Search;
                state.sound(Sound::Move);
            }
        }
        (Focus::List, Action::Down) => {
            // Past the last language, the light goes down to Continue.
            if !view.column.step(&rows, 1) {
                view.focus = Focus::Continue;
            }
            state.sound(Sound::Move);
        }
        (Focus::List, Action::Right) => {
            view.focus = Focus::Continue;
            state.sound(Sound::Move);
        }
        (Focus::List, Action::Accept) => {
            let at = view.column.at;
            choose(state, at);
        }
        (Focus::Continue, Action::Up | Action::Left) => {
            // Back to the list, or to the field where the search found
            // nothing to stand on.
            view.focus = if rows.is_empty() && view.search.is_open() {
                Focus::Search
            } else {
                Focus::List
            };
            state.sound(Sound::Move);
        }
        (Focus::Continue, Action::Accept) => super::onward(state),
        _ => {}
    }
}

/// Options: the search field comes up, the light in it — and with a pad in
/// hand a board to type with at once.
fn to_search(state: &mut State) {
    let view = &mut state.views.language;
    view.search.open();
    view.focus = Focus::Search;
    if state.pad {
        state.type_with_pad(FieldId::Search);
    } else {
        state.sound(Sound::Press);
    }
}

/// The light lands on the first language the search found, if it found
/// any. Answers whether it did.
pub fn land(state: &mut State) -> bool {
    let rows = rows(state);
    if rows.is_empty() {
        return false;
    }
    let view = &mut state.views.language;
    view.focus = Focus::List;
    view.column.start_at_first(&rows);
    state.sound(Sound::Move);
    true
}

/// Back: what is typed in the search, then the search field, then the step
/// before. The light stays on the language it was on as the list grows
/// whole again.
fn back(state: &mut State) {
    let lit = match state.views.language.focus {
        Focus::List => found(state).get(state.views.language.column.at).copied(),
        _ => None,
    };
    let backed = state.views.language.search.back();
    if backed == Backed::Nothing {
        return state.back();
    }
    let rows = rows(state);
    let view = &mut state.views.language;
    match lit {
        // The whole list is every language, so its place is its row.
        Some(at) if !view.search.narrows() => {
            view.column.select(&rows, at);
        }
        _ => view.column.start_at_chosen(&rows),
    }
    // The field gone, the light comes out of it onto the list.
    if view.focus == Focus::Search && !view.search.is_open() {
        view.focus = Focus::List;
    }
    state.sound(Sound::Back);
}

/// Something typed into the search: the list narrows to what it finds, the
/// light — in the field — over the first of them.
pub fn searched(state: &mut State, edit: &Edit) {
    if !state.views.language.search.edit(edit) {
        return;
    }
    let rows = rows(state);
    let view = &mut state.views.language;
    view.column.start_at_first(&rows);
    if rows.is_empty() && view.focus == Focus::List {
        view.focus = Focus::Search;
    }
}

/// How wide the rows of the list need to be at `scale`, `measure` saying
/// how wide words are in a style: the longest name, or the longest English
/// name under one, of every language offered — not only those a search
/// shows, so the column keeps its width as it narrows — with a row's padding
/// either side, the flag at its start and the `chosen` mark at its end, as
/// every row may come to wear it, and a little air between the words and the
/// mark. The search field over the list is as wide.
pub fn rows_width_at(scale: f32, mut measure: impl FnMut(Text, &str) -> f32) -> f32 {
    let words = catalogs::languages()
        .iter()
        .fold(0.0_f32, |widest, language| {
            let name = measure(Text::Body, language.native);
            let note = measure(Text::Caption, language.english);
            widest.max(name).max(note)
        });
    // What a row keeps for itself besides its words, read off the column's
    // own arithmetic at any width it is wider than.
    let wide = 10_000.0 * scale;
    let every_part = Row::new("").flag(flags::WHITE).chosen(true);
    let kept = wide - column::words_room_at(scale, wide, &every_part);
    words + kept + Metric::Gap.value() * scale * 2.0
}

/// Where the column stands in `content`: as wide as its rows need and its
/// scroll bar's channel beside them, in the middle (`column::centred`).
fn column_view(state: &mut State, page: &mut Page, content: [f32; 4]) -> [f32; 4] {
    let scale = page.scaled(1.0).to_bits();
    let rows_w = match state.views.language.widest {
        Some((at, width)) if at == scale => width,
        _ => {
            let width = rows_width_at(page.scaled(1.0), |style, words| page.measure(style, words));
            state.views.language.widest = Some((scale, width));
            width
        }
    };
    let (rail, channel) = column::bar_room(page);
    column::centred(page, content, rows_w + rail + channel)
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let rows = rows(state);
    let mut view = column_view(state, page, room.content);
    // The search field over the list, as wide as its rows, while it is up.
    if state.views.language.search.is_open() {
        let tall = field::height(page);
        let gap = page.metric(Metric::Gap);
        let (rail, channel) = column::bar_room(page);
        let rect = [view[0], view[1], (view[2] - rail - channel).max(1.0), tall];
        view[1] += tall + gap;
        view[3] = (view[3] - tall - gap).max(1.0);
        let query = state.views.language.search.query().to_string();
        let in_field = lit && state.views.language.focus == Focus::Search;
        let placeholder = i18n::text("search-language-placeholder");
        super::search_field(state, page, rect, &query, placeholder, in_field);
        if rows.is_empty() {
            super::found_nothing(page, [rect[0], view[1], rect[2], view[3]], &query);
        }
    }
    let look = Look {
        ids: IDS,
        pressing: state.pressing.through(),
        dt: state.clock.dt,
        pad: state.pad,
        bounds: room.content,
        corners: Corners::Fitted,
    };
    let in_list = lit && state.views.language.focus == Focus::List && !rows.is_empty();
    let light = in_list.then_some(&mut state.light);
    state
        .views
        .language
        .column
        .draw(page, view, &rows, light, look);
    let on_button = lit && state.views.language.focus == Focus::Continue;
    super::lone_button(state, page, room.buttons, verb(), on_button);
}

/// Put the light where the pointer is: a language, or Continue. Answers
/// whether it stands there.
fn point(state: &mut State, id: u32) -> bool {
    let rows = rows(state);
    let view = &mut state.views.language;
    if let Some(index) = IDS.row_of(id, rows.len()) {
        let landed = view.column.point_at(&rows, index);
        if landed {
            view.focus = Focus::List;
        }
        return landed;
    }
    if super::button_of(id, 1).is_some() {
        view.focus = Focus::Continue;
        return true;
    }
    if id == FieldId::Search.spot() && view.search.is_open() {
        view.focus = Focus::Search;
        return true;
    }
    false
}

pub fn hover(state: &mut State, id: u32) {
    point(state, id);
}

pub fn pressed(state: &mut State, page: &mut Page) {
    if state.views.language.search.is_open() && page.pressed(FieldId::Search.spot()) {
        // A click puts the light in the field, where a keyboard types; the
        // installer raises no board of its own for a pointer. On a field
        // whose shell's board was put away it asks for that board again.
        state.views.language.focus = Focus::Search;
        state.clicked_field(FieldId::Search);
        return;
    }
    let rows = rows(state);
    match state.views.language.column.pressed(page, &rows, IDS) {
        Some(Pick::Moved(_)) => {
            state.views.language.focus = Focus::List;
            state.sound(Sound::Move);
            return;
        }
        Some(Pick::Clicked(index)) => {
            if point(state, IDS.row(index)) {
                act(state, Action::Accept, None);
            }
            return;
        }
        None => {}
    }
    if super::lone_pressed(page, verb()) {
        super::onward(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(locale: &str) -> usize {
        catalogs::languages()
            .iter()
            .position(|language| language.locale == locale)
            .unwrap()
    }

    #[test]
    fn the_column_opens_on_the_language_in_force() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        assert_eq!(state.views.language.column.at, at("en_US.UTF-8"));
    }

    #[test]
    fn choosing_a_language_guesses_the_zone_and_keyboard_and_goes_on() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        choose(&mut state, at("pl_PL.UTF-8"));
        assert_eq!(state.draft.region.locale, "pl_PL.UTF-8");
        assert_eq!(state.draft.region.timezone, "Europe/Warsaw");
        assert_eq!(state.draft.region.keyboard.layout, "pl");
        assert_eq!(state.draft.region.keyboard.variant, "");
        assert_eq!(state.draft.region.keyboard.console, "pl2");
        assert_eq!(state.speaking_next(), Some("pl"));
        assert_eq!(state.screen, Screen::Timezone);
    }

    #[test]
    fn a_zone_or_keyboard_the_person_chose_is_left_alone() {
        let mut state = State::demo(true);
        state.draft.region.timezone = "Asia/Tokyo".into();
        state.chose_zone = true;
        crate::ui::screens::keyboard::set(&mut state, "us", "dvorak");
        state.chose_keyboard = true;
        state.go(Screen::Language);
        choose(&mut state, at("fr_FR.UTF-8"));
        assert_eq!(state.draft.region.timezone, "Asia/Tokyo");
        assert_eq!(state.draft.region.keyboard.variant, "dvorak");
    }

    #[test]
    fn a_language_no_catalog_speaks_leaves_the_installers_words_alone() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        choose(&mut state, at("it_IT.UTF-8"));
        assert_eq!(state.speaking_next(), None);
        assert_eq!(state.draft.region.locale, "it_IT.UTF-8");
    }

    /// Time zone's places follow the language chosen, any of those offered
    /// and not only the ten the installer speaks.
    #[test]
    fn choosing_a_language_names_the_places_in_it() {
        let city = |state: &State, name: &str| {
            let zone = state.zones.iter().find(|zone| zone.name == name).unwrap();
            zone.shown_city.clone()
        };
        let mut state = State::demo(true);
        assert_eq!(city(&state, "Europe/Warsaw"), "Warsaw");
        for (locale, warsaw) in [
            ("pl_PL.UTF-8", "Warszawa"),
            ("it_IT.UTF-8", "Varsavia"),
            ("ru_RU.UTF-8", "Варшава"),
            ("en_GB.UTF-8", "Warsaw"),
        ] {
            state.go(Screen::Language);
            choose(&mut state, at(locale));
            assert_eq!(city(&state, "Europe/Warsaw"), warsaw, "{locale}");
        }
    }

    #[test]
    fn a_keyboard_guess_the_list_lacks_falls_back_to_us() {
        // The demo has no Czech layout, so a Czech speaker is offered `us`.
        let mut state = State::demo(true);
        state.go(Screen::Language);
        choose(&mut state, at("cs_CZ.UTF-8"));
        assert_eq!(state.draft.region.keyboard.layout, "us");
    }

    #[test]
    fn hovering_lights_a_language_and_continue_and_chooses_nothing() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        let polish = at("pl_PL.UTF-8");
        hover(&mut state, IDS.row(polish));
        let view = &state.views.language;
        assert_eq!((view.focus, view.column.at), (Focus::List, polish));
        assert_eq!(state.draft.region.locale, "en_US.UTF-8", "a hover chose");
        hover(&mut state, crate::ui::spots::BUTTONS);
        assert_eq!(state.views.language.focus, Focus::Continue);
        assert_eq!(state.screen, Screen::Language);
    }

    /// The names of the rows the list shows.
    fn shown(state: &State) -> Vec<String> {
        rows(state).into_iter().map(|row| row.name).collect()
    }

    fn typed(state: &mut State, text: &str) {
        state.edit(FieldId::Search, &Edit::Wrote(text.into()));
    }

    /// Options brings the search up with the light in it; every letter
    /// narrows the list, by a language's own name or its English one; Enter
    /// lands the light on the first found and Accept chooses it.
    #[test]
    fn a_search_finds_a_language_by_either_name_and_accept_chooses_it() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        act(&mut state, Action::Menu, None);
        let view = &state.views.language;
        assert!(view.search.is_open());
        assert_eq!(view.focus, Focus::Search);
        assert!(state.board.is_none(), "a keyboard in hand got a board");
        typed(&mut state, "pol");
        assert_eq!(shown(&state), ["Polski (Polska)"]);
        typed(&mut state, "ish");
        assert_eq!(shown(&state), ["Polski (Polska)"], "by its English name");
        for _ in 0.."polish".len() {
            state.edit(FieldId::Search, &Edit::Rubbed);
        }
        typed(&mut state, "deutsch");
        assert_eq!(shown(&state), ["Deutsch (Deutschland)"]);
        // Enter in the field: the light on the first found.
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.language.focus, Focus::List);
        assert_eq!(state.views.language.column.at, 0);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.draft.region.locale, "de_DE.UTF-8");
        assert_eq!(state.screen, Screen::Timezone);
        // Coming back finds the search put away and the list whole.
        state.back();
        assert!(!state.views.language.search.is_open());
        assert_eq!(rows(&state).len(), catalogs::languages().len());
    }

    /// Back empties the search, the light staying on the language it was
    /// on as the list grows whole; then it closes the field; then it goes
    /// back to Welcome.
    #[test]
    fn back_empties_the_search_then_closes_it_then_goes_back() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        act(&mut state, Action::Menu, None);
        typed(&mut state, "nor");
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Down, None);
        let lit = rows(&state)[state.views.language.column.at].name.clone();
        assert!(lit.starts_with("Norsk"), "{lit}");
        act(&mut state, Action::Back, None);
        let view = &state.views.language;
        assert!(view.search.is_open() && view.search.query().is_empty());
        assert_eq!(rows(&state)[view.column.at].name, lit, "the light moved");
        act(&mut state, Action::Back, None);
        assert!(!state.views.language.search.is_open());
        assert_eq!(state.screen, Screen::Language);
        act(&mut state, Action::Back, None);
        assert_eq!(state.screen, Screen::Welcome);
    }

    /// A search that finds nothing keeps the light in its field: there is
    /// no row for it to land on.
    #[test]
    fn a_search_that_finds_nothing_keeps_the_light_in_its_field() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        act(&mut state, Action::Menu, None);
        typed(&mut state, "qqq");
        assert!(rows(&state).is_empty());
        act(&mut state, Action::Down, None);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.language.focus, Focus::Search);
        assert_eq!(state.screen, Screen::Language, "Accept chose nothing");
        // From Continue, Up comes back to the field.
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.language.focus, Focus::Continue);
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.language.focus, Focus::Search);
    }

    /// With a pad, Options brings a board up at once: the installer's, or
    /// under LineXinBar the shell's. Its Done lands the light on the first
    /// language found.
    #[test]
    fn with_a_pad_the_search_comes_up_with_a_board_to_type_with() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        state.pad = true;
        act(&mut state, Action::Menu, None);
        assert_eq!(
            state.board.as_ref().map(|board| board.field),
            Some(FieldId::Search)
        );
        state.type_keys(vec![Edit::Wrote("francais".into())]);
        state.act(Action::Submit, None);
        assert!(state.board.is_none());
        assert_eq!(state.views.language.focus, Focus::List);
        assert_eq!(state.views.language.column.at, 0);
        let found = shown(&state);
        assert!(!found.is_empty(), "accents were not folded");
        assert!(
            found.iter().all(|name| name.starts_with("Français")),
            "{found:?}"
        );

        let mut state = State::demo(true);
        state.type_with_the_shells_board(true);
        state.go(Screen::Language);
        state.pad = true;
        act(&mut state, Action::Menu, None);
        assert!(
            state.board.is_none(),
            "the installer's board under the shell"
        );
        assert_eq!(state.typing_with_the_shell(), Some(FieldId::Search));
        // Up from the first row goes back into the field.
        let mut state = State::demo(true);
        state.go(Screen::Language);
        act(&mut state, Action::Menu, None);
        typed(&mut state, "es");
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.language.focus, Focus::List);
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.language.focus, Focus::Search);
    }

    /// Under LineXinBar, while the shell's board types into the search, the
    /// pointer passing over the list leaves the light in the field, as a
    /// field typed into from a keyboard is left: had it lit a row, the field
    /// would let go of its text and the shell put its board away mid-word
    /// (seen live). Once the board is done, hovering lights rows
    /// again.
    #[test]
    fn the_pointer_crossing_the_list_leaves_the_shells_board_typing() {
        let mut state = State::demo(true);
        state.type_with_the_shells_board(true);
        state.go(Screen::Language);
        state.pad = true;
        act(&mut state, Action::Menu, None);
        assert_eq!(state.typing_with_the_shell(), Some(FieldId::Search));
        state.hover(IDS.row(3));
        assert_eq!(
            state.views.language.focus,
            Focus::Search,
            "the pointer passing took the light out of the field"
        );
        assert_eq!(state.typing_with_the_shell(), Some(FieldId::Search));
        // The board's Start: Enter, the light down to what was found, and
        // the pointer lights a row again.
        typed(&mut state, "en");
        state.act(Action::Accept, None);
        assert_eq!(state.typing_with_the_shell(), None);
        assert_eq!(state.views.language.focus, Focus::List);
        assert_eq!(state.views.language.column.at, 0);
        state.hover(IDS.row(1));
        assert_eq!(state.views.language.column.at, 1, "hovering stayed held");
    }

    #[test]
    fn the_light_goes_down_from_the_last_language_to_continue_and_back() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        let last = catalogs::languages().len() - 1;
        state.views.language.column.at = last;
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.language.focus, Focus::Continue);
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.language.focus, Focus::List);
        assert_eq!(state.views.language.column.at, last);
    }

    #[test]
    fn choosing_a_language_the_shell_speaks_tells_the_live_shell() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        choose(&mut state, at("pl_PL.UTF-8"));
        assert_eq!(state.telling_shell_next(), Some("pl"));
        let mut state = State::demo(true);
        state.go(Screen::Language);
        choose(&mut state, at("en_AU.UTF-8"));
        assert_eq!(state.telling_shell_next(), Some("en-GB"));
        // Another language leaves the shell's settings alone.
        for locale in ["it_IT.UTF-8", "pt_PT.UTF-8"] {
            let mut state = State::demo(true);
            state.go(Screen::Language);
            choose(&mut state, at(locale));
            assert_eq!(state.telling_shell_next(), None, "{locale}");
        }
    }

    #[test]
    fn the_shells_ten_languages_are_reached_from_their_locales() {
        for (locale, tag) in [
            ("de_DE.UTF-8", "de"),
            ("en_GB.UTF-8", "en-GB"),
            ("en_US.UTF-8", "en-US"),
            ("es_ES.UTF-8", "es"),
            ("fr_FR.UTF-8", "fr"),
            ("hi_IN.UTF-8", "hi"),
            ("pl_PL.UTF-8", "pl"),
            ("pt_BR.UTF-8", "pt-BR"),
            ("ru_RU.UTF-8", "ru"),
            ("zh_CN.UTF-8", "zh-CN"),
        ] {
            assert_eq!(shell_language(locale), Some(tag), "{locale}");
        }
        assert_eq!(shell_language("fr_CA.UTF-8"), Some("fr"));
        assert_eq!(shell_language("zh_TW.UTF-8"), None);
        assert_eq!(shell_language("not_A.LOCALE"), None);
        // Every catalog the installer switches to is one the shell has.
        for language in catalogs::languages() {
            if let Some(tag) = language.ui {
                assert!(SHELL_LANGUAGES.contains(&tag), "{tag}");
            }
        }
    }

    #[test]
    fn the_shells_settings_are_found_as_the_shell_finds_them() {
        let os = |text: &'static str| Some(OsStr::new(text));
        assert_eq!(
            shell_settings(os("/home/example/.cfg"), os("/home/example")),
            Some(PathBuf::from("/home/example/.cfg/lxb/shell.toml"))
        );
        // A relative XDG_CONFIG_HOME is ignored, as the specification says.
        assert_eq!(
            shell_settings(os("cfg"), os("/home/example")),
            Some(PathBuf::from("/home/example/.config/lxb/shell.toml"))
        );
        assert_eq!(
            shell_settings(None, os("/home/example")),
            Some(PathBuf::from("/home/example/.config/lxb/shell.toml"))
        );
        assert_eq!(shell_settings(None, os("")), None);
        assert_eq!(shell_settings(None, None), None);
    }

    #[test]
    fn the_language_line_is_merged_into_the_shells_settings() {
        // No file: the one line.
        assert_eq!(with_shell_language(None, "pl"), "language = \"pl\"\n");
        // A file shaped like the shell's own: a comment block, top-level
        // keys, a table. The language line is replaced where it stands.
        let file = "\
# LineXinBar's own settings.
#
# language: chosen under Settings > Language.

language = \"en-GB\"
clock = \"24-hour\"
accent = \"teal\"

[displays.example]
scale = 1.25
language = \"not-the-shells\"
";
        assert_eq!(
            with_shell_language(Some(file), "de"),
            file.replace("language = \"en-GB\"", "language = \"de\"")
        );
        // Without one, it goes in front of the first setting, a top-level
        // key, after the comments; a key of the same name in a table is the
        // table's.
        let without = file.replace("language = \"en-GB\"\n", "");
        assert_eq!(
            with_shell_language(Some(&without), "ru"),
            without.replace("clock = ", "language = \"ru\"\nclock = ")
        );
        // A second language line at the top is dropped; spacing and quoting
        // of the key do not hide it; line endings stay as they were.
        assert_eq!(
            with_shell_language(Some("language=\"fr\"\r\n\"language\" = \"es\"\r\n"), "hi"),
            "language = \"hi\"\r\n"
        );
        // Only comments, or only a table: the line before the table, or at
        // the end, kept apart from a last line with no ending.
        assert_eq!(
            with_shell_language(Some("# nothing yet"), "pl"),
            "# nothing yet\nlanguage = \"pl\"\n"
        );
        assert_eq!(
            with_shell_language(Some("[displays.example]\nscale = 2\n"), "pl"),
            "language = \"pl\"\n[displays.example]\nscale = 2\n"
        );
        assert_eq!(with_shell_language(Some(""), "pl"), "language = \"pl\"\n");
    }

    #[test]
    fn the_shells_settings_are_written_whole_and_beside_nothing() {
        let dir = std::env::temp_dir().join(format!(
            "lxb-installer-shell-settings-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("lxb").join("shell.toml");
        // A folder that is not there yet is made.
        assert!(write_shell_language(&path, "pl").unwrap());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "language = \"pl\"\n"
        );
        std::fs::write(&path, "# mine\naccent = \"teal\"\n").unwrap();
        assert!(write_shell_language(&path, "de").unwrap());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "# mine\nlanguage = \"de\"\naccent = \"teal\"\n"
        );
        // Saying it again changes nothing, and nothing is left beside it.
        assert!(!write_shell_language(&path, "de").unwrap());
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["shell.toml"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sessions_lang_is_matched_whatever_its_charset_is_spelt_as() {
        assert_eq!(
            from_environment("pl_PL.utf8").map(|language| language.locale),
            Some("pl_PL.UTF-8")
        );
        assert!(from_environment("C").is_none());
        assert!(from_environment("").is_none());
    }
}
