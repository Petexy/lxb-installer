//! Review: one headline, a few rows saying in plain words what will be
//! installed where, and Install — which raises the last question (the
//! toolkit's dialog, "Go back" first, listing what is erased) before
//! anything is written.
//!
//! Each row is a step's answer — where, the desktop, the language with the
//! time zone and keyboard, the account, the extras — and pressing it goes
//! back to that step to change it. An answer too long for one line takes a
//! second, and its row grows to hold it: on a panel standing on its side,
//! "Example SATA HDD · 1 TB · in its free space" is worth reading whole.
//!
//! Install is offered only where an installation can be started from:
//! always in a demo (which rehearses), and on the real machine only when it
//! was started from a Linexin live medium. Elsewhere it is outlined, and the
//! foot says why.

use lxb_app::lxb_render::{Align, Press};
use lxb_app::lxb_toolkit::{
    input::Action, metrics::Metric, motion, palette::Role, sound::Sound, typography::Text,
};
use lxb_app::{Page, Scroll};

use super::{partitions, Head, Room};
use crate::i18n;
use crate::model::config::{Bootloader, Target};
use crate::model::disks::{self, Partition};
use crate::model::secret::Secret;
use crate::ui::app::{Screen, State};
use crate::ui::button::Button;
use crate::ui::column::{self, Column, Corners, Pick, Row, Scrolled, Span};
use crate::ui::legend::Hint;
use crate::ui::motion::{Carried, Shape};
use crate::ui::panel::Note;
use crate::ui::{glyphs, spots, text, words};

/// The numbers the rows answer to.
const IDS: spots::Column = spots::Column::nth(0);

/// Where pressing a row goes back to, to change it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Goes {
    Step(Screen),
    /// Where to install's Partitions page: a row of the plan in use.
    Partitions,
}

/// Where the light stands.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Rows,
    #[default]
    Install,
}

/// What this screen keeps between frames.
#[derive(Debug, Default, Clone)]
pub struct View {
    pub focus: Focus,
    pub column: Column,
    /// How far the rows are scrolled: on a panel standing on its side the
    /// answers can take more lines than there is room for.
    scroll: Carried,
}

impl View {
    pub fn settle(&mut self) {
        self.column.settle();
        self.scroll.settle();
    }
}

fn verb(state: &State) -> Button<'static> {
    Button::primary(i18n::text("button-install")).enabled(state.can_install())
}

/// A part of a disk as a sentence names it: by the name it was given, in
/// quotes, or by what it holds.
pub fn part_name(part: &Partition) -> String {
    let label = part.label.trim();
    if label.is_empty() {
        words::content(part.content).to_string()
    } else {
        crate::message!("part-labelled", "label" => label)
    }
}

/// Where Linexin goes: the disk, and the place on it — and where the
/// person's files go when they have a place of their own, kept or erased.
fn where_words(state: &State) -> String {
    let place = place_words(state);
    let draft = &state.judged();
    let Some(home) = &draft.storage.home else {
        return place;
    };
    let part = disks::home_partition(draft, home)
        .map(|(_, part)| part_name(part))
        .unwrap_or_default();
    let files = if home.format {
        crate::message!("review-where-home-erased", "part" => part)
    } else {
        crate::message!("review-where-home-kept", "part" => part)
    };
    format!("{place} · {files}")
}

/// The disk Linexin goes on, and the place on it.
fn place_words(state: &State) -> String {
    let draft = &state.judged();
    let disk = words::disk_name(&draft.disk);
    match &draft.target {
        Target::WholeDisk => crate::message!("review-where-disk", "disk" => disk),
        Target::FreeSpace { .. } => crate::message!("review-where-free", "disk" => disk),
        Target::Partition { path } => {
            let part = draft
                .disk
                .partition(path)
                .map(part_name)
                .unwrap_or_default();
            crate::message!("review-where-part", "disk" => disk, "part" => part)
        }
        // A layout made by hand has no one place: while it is the choice in
        // use the plan's own rows stand where this one would (`listed`).
        Target::Layout { .. } => disk,
    }
}

/// The language by its own name, the nearest city and the keyboard.
fn region_words(state: &State) -> String {
    let region = &state.draft.region;
    let language = super::language::current(state)
        .map(|language| language.native.to_string())
        .unwrap_or_else(|| region.locale.clone());
    let city = match state.zones.iter().find(|zone| zone.name == region.timezone) {
        Some(zone) => super::timezone::place_name(zone),
        None => region
            .timezone
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .replace('_', " "),
    };
    let keyboard = &region.keyboard;
    let layout = state
        .layouts
        .iter()
        .find(|one| one.layout == keyboard.layout);
    let keys = layout
        .and_then(|layout| {
            layout
                .variants
                .iter()
                .find(|one| !keyboard.variant.is_empty() && one.variant == keyboard.variant)
                .map(|variant| variant.description.clone())
                .or_else(|| Some(layout.description.clone()))
        })
        .unwrap_or_else(|| keyboard.layout.clone());
    crate::message!("review-region", "language" => language, "city" => city, "keyboard" => keys)
}

/// The account: the full name and the username it signs in with, and the
/// administrator account when the person gave it a password of its own.
fn account_words(state: &State) -> String {
    let account = &state.draft.account;
    let name = account.full_name.trim();
    let words = match (name.is_empty(), account.username.is_empty()) {
        (_, true) => return i18n::text("review-account-none").to_string(),
        (true, false) => account.username.clone(),
        (false, false) => {
            let user = account.username.as_str();
            crate::message!("review-account", "name" => name, "username" => user)
        }
    };
    if account.root_password.is_some() {
        format!("{words} · {}", i18n::text("account-root"))
    } else {
        words
    }
}

/// Everything else that comes with it, in a few words each — judged by
/// what the person has chosen as a request carries it (`State::judged`).
fn extras_words(state: &State) -> String {
    let draft = &state.judged();
    let mut said = Vec::new();
    if draft.software.updates {
        said.push(i18n::text("review-extra-updates").to_string());
    }
    let apps = draft.software.flatpaks.len() as i64;
    if apps > 0 {
        said.push(crate::message!("review-extra-apps", "count" => apps));
    }
    if super::desktop::restore_points(state) {
        said.push(i18n::text("review-extra-snapshots").to_string());
    }
    // A start-up menu the person chose themselves, by the name Options
    // showed it under; the automatic one goes unsaid.
    if draft.bootloader != Bootloader::Automatic {
        said.push(super::desktop::bootloader_name(draft.bootloader).to_string());
    }
    // The swap file of a plan in use has a row of its own.
    if draft.storage.swap_mib > 0 && !partitions::in_use(state) {
        let size =
            crate::message!("size-gb", "amount" => (draft.storage.swap_mib / 1024).to_string());
        said.push(crate::message!("review-extra-swap", "size" => size));
    }
    if said.is_empty() {
        i18n::text("review-extra-none").to_string()
    } else {
        said.join(" · ")
    }
}

/// The rows, and where each goes back to: each step's answer under its
/// name, with the step's own mark. Where Linexin goes is one row — or, while
/// Partitions' plan is the choice in use, the plan's rows in its place, in
/// GParted's words as Partitions speaks: one for each mount point, the swap
/// file, and how many operations will be applied (`partitions::review_rows`),
/// each going back to Partitions.
fn listed(state: &State) -> Vec<(Row, Goes)> {
    let mut rows = Vec::new();
    if partitions::in_use(state) {
        for said in partitions::review_rows(state) {
            let row = Row::new(said.name).note(said.note).mark(said.mark);
            rows.push((row, Goes::Partitions));
        }
    } else {
        let row = Row::new(i18n::text("review-where"))
            .note(where_words(state))
            .mark(glyphs::DISK_TARGET);
        rows.push((row, Goes::Step(Screen::Target)));
    }
    let desktop = super::desktop::desktop_name(state.draft.desktop);
    rows.extend([
        (
            Row::new(i18n::text("review-desktop"))
                .note(desktop)
                .mark("setting-appearance"),
            Goes::Step(Screen::Desktop),
        ),
        (
            Row::new(i18n::text("review-region-name"))
                .note(region_words(state))
                .mark("setting-name-server"),
            Goes::Step(Screen::Language),
        ),
        (
            Row::new(i18n::text("review-account-name"))
                .note(account_words(state))
                .mark("file-home"),
            Goes::Step(Screen::Account),
        ),
        (
            Row::new(i18n::text("review-extras"))
                .note(extras_words(state))
                .mark("setting-info"),
            Goes::Step(Screen::Desktop),
        ),
    ]);
    rows
}

/// The rows, as `listed` has them.
fn rows(state: &State) -> Vec<Row> {
    listed(state).into_iter().map(|(row, _)| row).collect()
}

/// Back to the step behind the row at `index`, to change it — for a row of
/// Partitions' plan, its page, the light on the map.
fn revisit(state: &mut State, index: usize) {
    let Some((_, goes)) = listed(state).into_iter().nth(index) else {
        return;
    };
    state.sound(Sound::Press);
    match goes {
        Goes::Step(step) => state.go(step),
        Goes::Partitions => {
            state.go(Screen::Target);
            partitions::came_back(state);
            state.views.partitions.focus = partitions::Focus::Map;
        }
    }
}

/// The picture of the review: an account typed in, so every row says
/// something, and the light on Install — for `review-partitions` and
/// `confirm-partitions` with Partitions' whole plan the choice in use
/// (`partitions::stage_plan`), its rows in place of Where.
pub fn stage_with_the_plan(state: &mut State) {
    partitions::stage_plan(state);
    state.views.partitions.in_use = true;
    stage(state);
    enter(state);
}

/// The review for the pictures of the plan's paged question
/// (`confirm-plan-pages`, `confirm-plan-last`): `stage_with_the_plan`'s, the
/// plan also writing a new table over the made-up archive disk
/// (`partitions::stage_plan_pages`).
pub fn stage_with_the_plan_pages(state: &mut State) {
    partitions::stage_plan_pages(state);
    state.views.partitions.in_use = true;
    stage(state);
    enter(state);
}

/// The picture of the review: an account typed in, so every row says
/// something, and the light on Install.
pub fn stage(state: &mut State) {
    state.draft.account.full_name = "Alex Example".into();
    super::account::edited(state, crate::ui::field::FieldId::FullName);
    state.draft.account.password = Secret::new("Example-2026");
    state.repeat = Secret::new("Example-2026");
}

pub fn enter(state: &mut State) {
    let rows = rows(state);
    let can_install = state.can_install();
    let view = &mut state.views.review;
    view.column.start_at_chosen(&rows);
    view.focus = if can_install {
        Focus::Install
    } else {
        Focus::Rows
    };
}

pub fn head(_state: &State) -> Head {
    Head::new(
        "file-page",
        i18n::text("review-title"),
        i18n::text("review-caption"),
    )
}

pub fn hints(_state: &State) -> Vec<Hint> {
    super::choose_and_back()
}

pub fn note(state: &State) -> Option<Note> {
    (!state.can_install()).then(|| Note {
        text: i18n::text("install-unavailable").to_string(),
        role: Role::Text,
    })
}

pub fn act(state: &mut State, action: Action, scroll: Option<Scroll>) {
    if scroll.is_some_and(|scroll| Column::owns(scroll.spot, IDS)) {
        state.views.review.focus = Focus::Rows;
    }
    if action == Action::Back {
        return state.back();
    }
    let rows = rows(state);
    let can_install = state.can_install();
    let view = &mut state.views.review;
    match (view.focus, action) {
        (Focus::Rows, Action::Up) => {
            if view.column.step(&rows, -1) {
                state.sound(Sound::Move);
            }
        }
        (Focus::Rows, Action::Down) => {
            // Past the last row, the light goes down to Install.
            if view.column.step(&rows, 1) {
                state.sound(Sound::Move);
            } else if can_install {
                view.focus = Focus::Install;
                state.sound(Sound::Move);
            }
        }
        (Focus::Rows, Action::Right) if can_install => {
            view.focus = Focus::Install;
            state.sound(Sound::Move);
        }
        (Focus::Rows, Action::Accept) => {
            let at = view.column.at;
            revisit(state, at);
        }
        (Focus::Install, Action::Up | Action::Left) => {
            view.focus = Focus::Rows;
            state.sound(Sound::Move);
        }
        (Focus::Install, Action::Accept) => state.ask_to_install(),
        _ => {}
    }
}

/// The most lines an answer is given under its row's name.
pub const NOTE_LINES: usize = 2;

/// Where each row stands in `view`: its rectangle and its answer's lines,
/// each row as tall as its answer needs and never shorter than a row.
fn lay(page: &mut Page, view: [f32; 4], rows: &[Row]) -> Vec<([f32; 4], Vec<String>)> {
    let pad = page.metric(Metric::RowPadding);
    let gap = page.metric(Metric::Gap);
    let least = page.metric(Metric::RowHeight);
    let name_line = page.line(Text::Body);
    let note_line = page.line(Text::Caption);
    let mark = page.metric(Metric::ItemIcon) * 0.6;
    let room = (view[2] - pad * 2.0 - mark - gap).max(1.0);
    let mut y = view[1];
    rows.iter()
        .map(|row| {
            let note = row.note.as_deref().unwrap_or_default();
            let lines = text::wrapped(page.ui(), Text::Caption, note, room, NOTE_LINES);
            let words = name_line + note_line * lines.len() as f32;
            let tall = least.max(words + (least - name_line - note_line));
            let rect = [view[0], y, view[2], tall];
            y += tall + gap * 0.35;
            (rect, lines)
        })
        .collect()
}

/// One row: its glass in `shape`, its mark, its name and its answer's
/// lines.
fn draw_row(
    page: &mut Page,
    rect: [f32; 4],
    row: &Row,
    lines: &[String],
    press: Press,
    shape: Shape,
) {
    let pad = page.metric(Metric::RowPadding);
    let gap = page.metric(Metric::Gap);
    let name_line = page.line(Text::Body);
    let note_line = page.line(Text::Caption);
    let mark = page.metric(Metric::ItemIcon) * 0.6;
    let icons = page.icons();
    let radius = shape.radius(page, rect);
    let ui = page.ui();
    let sunk = ui.control(rect, radius, press, rect[2], 1.0);
    let [x, y, w, h] = sunk;
    // White, every word of it, lit or not: these rows are what the screen
    // is for. The answers are lesser lines, white too, a little quieter
    // than the names.
    let ink = 1.0;
    if let Some(name) = row.mark {
        ui.icon_tinted(
            [x + pad, y + (h - mark) * 0.5, mark, mark],
            name,
            icons,
            Role::Text,
            ink,
        );
    }
    let left = x + pad + mark + gap;
    let room = (x + w - pad - left).max(0.0);
    let top = y + (h - name_line - note_line * lines.len() as f32) * 0.5;
    text::label_faded(
        ui,
        [left, top, room, name_line],
        Text::Body,
        &row.name,
        Role::Text,
        ink,
        Align::Left,
    );
    let (soft, soft_alpha) = text::LESSER;
    for (index, line) in lines.iter().enumerate() {
        text::label_faded(
            ui,
            [
                left,
                top + name_line + index as f32 * note_line,
                room,
                note_line,
            ],
            Text::Caption,
            line,
            soft,
            soft_alpha,
            Align::Left,
        );
    }
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let rows = rows(state);
    // In the middle of the panel, as every other step's rows are.
    let view = super::centred(room.content, page.scaled(960.0));
    let focus = state.views.review.focus;
    state
        .views
        .review
        .column
        .light_is_here(lit && focus == Focus::Rows);
    let at = state.views.review.column.at;
    let pressing = state.pressing.through();
    let dt = state.clock.dt;
    // Laid out from the top first, to know how tall everything is, then
    // scrolled to keep the row in hand in view with a little of the next.
    let flat = lay(page, view, &rows);
    let total = flat
        .last()
        .map_or(0.0, |(rect, _)| rect[1] + rect[3] - view[1]);
    let most = (total - view[3]).max(0.0);
    let scroll = &mut state.views.review.scroll;
    let mut wanted = scroll.wanted();
    if focus == Focus::Rows && !state.views.review.column.pointed() {
        if let Some((rect, _)) = flat.get(at) {
            let context = page.metric(Metric::RowHeight) * 0.5;
            let top = rect[1] - view[1];
            let bottom = top + rect[3];
            if top - context < wanted {
                wanted = top - context;
            }
            if bottom + context > wanted + view[3] {
                wanted = bottom + context - view[3];
            }
        }
    }
    let top = scroll.advance(wanted.clamp(0.0, most), dt);
    let laid: Vec<([f32; 4], Vec<String>)> = flat
        .into_iter()
        .map(|(rect, lines)| ([rect[0], rect[1] - top, rect[2], rect[3]], lines))
        .collect();
    // One list, one shape, as a column's (`column::Corners`): capsules
    // while every answer is one line, and every row a card, and the light
    // with them, once any answer has taken a second — a list of capsules
    // and cards side by side read as two kinds of thing.
    let spans: Vec<Span> = laid
        .iter()
        .map(|(rect, _)| Span {
            top: rect[1],
            tall: rect[3],
        })
        .collect();
    let row_h = page.metric(Metric::RowHeight);
    let shape = column::shape(Corners::Fitted, &rows, &spans, row_h);
    if lit && focus == Focus::Rows {
        if let Some((rect, _)) = laid.get(at) {
            let sunk = motion::pressed(*rect, pressing);
            state.light.lay(page, sunk, shape, pressing, dt);
        }
    }
    let from = page.ui().written();
    for (index, (row, (rect, lines))) in rows.iter().zip(&laid).enumerate() {
        let here = lit && focus == Focus::Rows && index == at;
        let press = match pressing {
            Some(through) if here => Press::Going(through),
            _ => Press::from(here),
        };
        page.ui().spot(IDS.row(index), *rect);
        draw_row(page, *rect, row, lines, press, shape);
    }
    let to = page.ui().written();
    // Kept above the button row however tall the answers make the rows, and
    // faded away at an end that really continues.
    let air = page.metric(Metric::RowHeight) * 0.5;
    page.ui().cut_between(
        from,
        to,
        [view[0] - air, view[1], view[2] + air * 2.0, view[3]],
    );
    let band = page.metric(Metric::RowHeight) * 0.55;
    page.ui().soft_edges(
        view,
        band,
        (top / band).min(1.0),
        ((most - top) / band).min(1.0),
    );
    // Where the answers take more lines than the room has (a display
    // standing on its side), a pointer is shown how much more there is as
    // in every column: a scroll bar just beside the rows — in the panel's
    // padding where they take the whole room, as You's form keeps its own —
    // that a hand walks the light along.
    let (rail, channel) = column::bar_room(page);
    let track = [view[0] + view[2] + channel, view[1], rail, view[3]];
    let scrolled = Scrolled {
        top,
        content: total,
        shown: view[3],
    };
    let pad = state.pad;
    state
        .views
        .review
        .column
        .draw_bar(page, IDS, track, scrolled, pad);
    let verb = verb(state);
    super::lone_button(
        state,
        page,
        room.buttons,
        verb,
        lit && focus == Focus::Install,
    );
}

/// Put the light where the pointer is: a row, or Install. Answers whether
/// it stands there.
fn point(state: &mut State, id: u32) -> bool {
    let rows = rows(state);
    let can_install = state.can_install();
    let view = &mut state.views.review;
    if let Some(index) = IDS.row_of(id, rows.len()) {
        let landed = view.column.point_at(&rows, index);
        if landed {
            view.focus = Focus::Rows;
        }
        return landed;
    }
    if can_install && super::button_of(id, 1).is_some() {
        view.focus = Focus::Install;
        return true;
    }
    false
}

pub fn hover(state: &mut State, id: u32) {
    point(state, id);
}

pub fn pressed(state: &mut State, page: &mut Page) {
    let rows = rows(state);
    match state.views.review.column.pressed(page, &rows, IDS) {
        // A hand on the bar walked the light, and the rows scroll after it.
        Some(Pick::Moved(_)) => {
            state.views.review.focus = Focus::Rows;
            state.sound(Sound::Move);
            return;
        }
        // Back to the row's step, as Accept on it.
        Some(Pick::Clicked(index)) => {
            if point(state, IDS.row(index)) {
                act(state, Action::Accept, None);
            }
            return;
        }
        None => {}
    }
    if super::lone_pressed(page, verb(state)) {
        state.views.review.focus = Focus::Install;
        state.ask_to_install();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::Filesystem;
    use crate::ui::app::{Machine, Question};

    fn on_review() -> State {
        let mut state = State::demo(true);
        state.go(Screen::Review);
        stage(&mut state);
        state
    }

    #[test]
    fn hovering_lights_a_row_and_install_and_a_click_on_a_row_goes_to_its_step() {
        let mut state = on_review();
        hover(&mut state, IDS.row(3));
        let view = &state.views.review;
        assert_eq!((view.focus, view.column.at), (Focus::Rows, 3));
        assert!(view.column.pointed());
        assert_eq!(state.screen, Screen::Review, "a hover went back");
        hover(&mut state, spots::BUTTONS);
        assert_eq!(state.views.review.focus, Focus::Install);
        assert!(point(&mut state, IDS.row(1)));
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Desktop);
    }

    /// The place in force is said as Time zone says it: in the language
    /// chosen.
    #[test]
    fn the_place_is_said_in_the_language_chosen() {
        let mut state = on_review();
        state.draft.region.timezone = "Europe/Warsaw".into();
        assert!(region_words(&state).contains("Warsaw"));
        state.draft.region.locale = "pl_PL.UTF-8".into();
        state.name_the_places();
        let words = region_words(&state);
        assert!(words.contains("Warszawa"), "{words}");
    }

    #[test]
    fn every_row_says_something_in_plain_words() {
        let state = on_review();
        for row in rows(&state) {
            let note = row.note.unwrap_or_default();
            assert!(!note.is_empty(), "{} says nothing", row.name);
            assert!(!note.contains("/dev"), "{note}");
            assert!(!note.contains('{'), "{note}");
        }
        assert!(account_words(&state).contains("alex"));
        assert!(extras_words(&state).contains(i18n::text("review-extra-updates")));
    }

    #[test]
    fn a_chosen_start_up_menu_and_an_administrator_are_said() {
        let mut state = on_review();
        let admin = i18n::text("account-root");
        let grub = i18n::text("bootloader-grub");
        assert!(!account_words(&state).contains(admin));
        assert!(!extras_words(&state).contains(grub));
        state.draft.account.root_password = Some(Secret::new("another secret"));
        state.draft.bootloader = Bootloader::Grub;
        assert!(account_words(&state).ends_with(&format!(" · {admin}")));
        assert!(extras_words(&state).contains(grub));
    }

    #[test]
    fn restore_points_are_counted_only_with_the_storage_that_keeps_them() {
        let mut state = on_review();
        let snapshots = i18n::text("review-extra-snapshots");
        assert!(!extras_words(&state).contains(snapshots));
        state.draft.storage.filesystem = Filesystem::Btrfs;
        assert!(extras_words(&state).contains(snapshots));
    }

    #[test]
    fn where_the_files_go_is_said_beside_where_linexin_goes() {
        let mut state = on_review();
        assert_eq!(where_words(&state), place_words(&state));
        // The empty disk erased, the files on the Windows disk's data part.
        let sata = state.disks[1].clone();
        let data = sata.partitions.iter().find(|p| p.label == "Data").unwrap();
        for format in [false, true] {
            state.draft.storage.home = Some(crate::model::config::Home {
                path: data.path.clone(),
                format,
                disk: Some(sata.clone()),
            });
            let said = where_words(&state);
            assert!(said.starts_with(&place_words(&state)), "{said}");
            assert!(said.contains("“Data”"), "{said}");
            let state_of_it = if format { "”, erased" } else { "”, kept" };
            assert!(said.ends_with(state_of_it), "{said}");
        }
    }

    /// With Partitions' plan the choice in use, its rows stand where Where
    /// stood — each mount point, the swap file, the operations — the extras
    /// leaving the swap file to its own row; each goes back to Partitions,
    /// and the rows after them to their steps as before.
    #[test]
    fn the_plans_rows_stand_in_place_of_where_and_go_back_to_partitions() {
        let mut state = State::demo(true);
        state.go(Screen::Review);
        stage_with_the_plan(&mut state);
        let names: Vec<String> = rows(&state).into_iter().map(|row| row.name).collect();
        let t = |id| i18n::text(id).to_string();
        let expected = [
            "/".to_string(),
            "/efi".into(),
            "/boot".into(),
            t("partitions-swap-file-title"),
            t("partitions-title"),
            t("review-desktop"),
            t("review-region-name"),
            t("review-account-name"),
            t("review-extras"),
        ];
        assert_eq!(names, expected);
        assert!(!extras_words(&state).contains("spare memory"));
        let view = &mut state.views.review;
        view.focus = Focus::Rows;
        view.column.at = 1;
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Target);
        assert_eq!(
            state.views.target.part,
            super::super::target::Part::Partitions
        );
        assert_eq!(state.views.partitions.focus, partitions::Focus::Map);
        state.go(Screen::Review);
        state.views.review.focus = Focus::Rows;
        state.views.review.column.at = 5;
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Desktop);
    }

    #[test]
    fn a_row_goes_back_to_its_step() {
        let mut state = on_review();
        state.views.review.focus = Focus::Rows;
        state.views.review.column.at = 3;
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Account);
    }

    #[test]
    fn install_raises_the_question_with_go_back_first() {
        let mut state = on_review();
        assert_eq!(state.views.review.focus, Focus::Install);
        act(&mut state, Action::Accept, None);
        let (_, body, answers) = state.question_words(Question::Erase);
        let body = body.replace('\u{a0}', " ");
        assert_eq!(answers[0], i18n::text("erase-back"));
        assert!(
            body.contains(&words::disk_name(&state.draft.disk)),
            "{body}"
        );
    }

    #[test]
    fn where_install_is_impossible_install_is_outlined_and_the_light_stays_on_the_rows() {
        // The made-up machine, told it is a computer not started from a
        // live medium: nothing of this computer is read.
        let mut state = State::demo(true);
        state.machine = Machine::Real { live: None };
        state.go(Screen::Review);
        assert!(!verb(&state).enabled);
        assert_eq!(state.views.review.focus, Focus::Rows);
        assert!(note(&state).is_some());
    }
}
