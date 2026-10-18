//! Desktop: how Linexin looks — three cards, Linexin, Kinexin and
//! LineXinBar, each with a picture of itself — and whether the installed
//! system is brought up to date on the way, with Options for the rest of what
//! is installed: software to leave out, apps from Flathub, the start-up
//! manager, unused drivers and restore points.
//!
//! **What needs the internet says so before it is chosen.** A desktop the
//! image does not carry is downloaded, and so are updates and Flathub's apps.
//! Whether the internet can be reached is asked on a thread of its own each
//! time this step is reached (it can take seconds); until it answers, those
//! say they are checking. With no internet they are refused: outlined, their
//! state said in Danger, and a press of one answered with the Error sound and
//! what to do. Online, updates and every app start switched on, as the legacy
//! installer had them — until the person switches one themselves.
//!
//! **Options are pages inside the step**, not menus: a menu holds one choice
//! with a mark, and these are lists of switches — the software of the image
//! grouped by kind, the apps — and a start-up manager whose unusable choices
//! must be seen outlined. Each page is one column with Done under it; Back
//! goes back a page, and from the step itself back to the last step.

use lxb_app::lxb_toolkit::{input::Action, metrics::Metric, palette::Role, sound::Sound};
use lxb_app::{Page, Scroll};

use super::{Head, Room};
use crate::i18n;
use crate::model::catalogs::{self, Package, PackageCategory};
use crate::model::config::{Bootloader, Desktop, Filesystem, Invalid};
use crate::model::disks;
use crate::ui::app::{Machine, State};
use crate::ui::button::{self, Button};
use crate::ui::cards::{self, Card, Cards};
use crate::ui::column::{self, Column, Corners, Look, Pick, Row};
use crate::ui::legend::Hint;
use crate::ui::panel::Note;
use crate::ui::toggle::{self, Toggle};
use crate::ui::{pictures, spots};

/// The "Install updates" row's number.
const UPDATES: u32 = spots::ITEMS;

/// The numbers the Options page's column and a list page's column answer to.
const OPTION_IDS: spots::Column = spots::Column::nth(0);
const LIST_IDS: spots::Column = spots::Column::nth(1);

/// The rows of the Options page, in order.
const REMOVE: usize = 0;
const FLATPAKS: usize = 1;
const BOOTLOADER: usize = 2;
const DRIVERS: usize = 3;
const SNAPSHOTS: usize = 4;

/// The index of Continue among the step's buttons, after Options.
const CONTINUE: usize = 1;

/// The start-up managers offered, in the order they are listed.
const BOOTLOADERS: [Bootloader; 4] = [
    Bootloader::Automatic,
    Bootloader::SystemdBoot,
    Bootloader::Grub,
    Bootloader::Refind,
];

/// The groups of software, in the order they are listed: what a person
/// recognises first, the libraries other programs use last.
const GROUPS: [PackageCategory; 5] = [
    PackageCategory::Applications,
    PackageCategory::Multimedia,
    PackageCategory::Network,
    PackageCategory::Fonts,
    PackageCategory::Libraries,
];

/// Which page of the step is up.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The cards, the updates row, Options and Continue.
    #[default]
    Main,
    Options,
    Remove,
    Flatpaks,
    Bootloader,
}

/// Where the light stands on the step's own page.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    #[default]
    Cards,
    Updates,
    /// Options, then Continue.
    Buttons(usize),
}

/// What this step keeps between frames.
#[derive(Debug, Default, Clone)]
pub struct View {
    pub part: Part,
    pub focus: Focus,
    pub cards: Cards,
    /// Whether the cards were stacked on the last frame, which decides
    /// whether Up and Down walk them or leave them.
    stacked: bool,
    pub options: Column,
    pub list: Column,
    /// On a page of Options: the light is on Done rather than in the column.
    pub on_done: bool,
    /// Whether the person switched updates or an app themselves. Until they
    /// do, the internet's answer decides: on when it can be reached.
    pub chose_updates: bool,
    pub chose_flatpaks: bool,
}

impl View {
    pub fn settle(&mut self) {
        self.options.settle();
        self.list.settle();
    }
}

// -------------------------------------------------------------------------
// The internet, the image and the software.

/// Whether the internet is known to be out of reach. Not knowing yet is not
/// being offline: nothing is refused while the question is out.
pub fn offline(state: &State) -> bool {
    state.online.known() == Some(&false)
}

/// Whether the internet's answer is still awaited, with no earlier one.
fn checking(state: &State) -> bool {
    state.online.known().is_none() && state.online.asking()
}

/// Whether `desktop` has to be downloaded.
fn needs_internet(state: &State, desktop: Desktop) -> bool {
    !state
        .offers
        .iter()
        .any(|offer| offer.desktop == desktop && offer.in_image)
}

/// What follows from the internet's answer: online, updates and every app on
/// (where the person has not switched them); offline, both off, because
/// neither can be had.
pub fn online_known(state: &mut State) {
    match state.online.known() {
        Some(true) => {
            if !state.views.desktop.chose_updates {
                state.draft.software.updates = true;
            }
            if !state.views.desktop.chose_flatpaks {
                state.draft.software.flatpaks = catalogs::flatpaks()
                    .iter()
                    .map(|app| app.id.to_string())
                    .collect();
            }
        }
        Some(false) => {
            state.draft.software.updates = false;
            state.draft.software.flatpaks.clear();
        }
        None => {}
    }
}

/// Fold in whatever the threads have answered since the last frame. Called
/// on every frame, whichever step is up, so an answer that arrives after the
/// person moved on still counts.
pub fn poll(state: &mut State) {
    if state.online.poll() {
        online_known(state);
    }
    if state.packages.poll() {
        if let Some(Err(trouble)) = state.packages.known() {
            eprintln!("lxb-installer: the software could not be listed: {trouble}");
        }
    }
}

/// The software a person may leave out, where it has been read.
fn packages(state: &State) -> &[Package] {
    match state.packages.known() {
        Some(Ok(packages)) => packages,
        _ => &[],
    }
}

// -------------------------------------------------------------------------
// Words.

/// A desktop's name.
pub fn desktop_name(desktop: Desktop) -> &'static str {
    match desktop {
        Desktop::Linexin => i18n::text("desktop-linexin"),
        Desktop::Kinexin => i18n::text("desktop-kinexin"),
        Desktop::LineXinBar => i18n::text("desktop-linexinbar"),
    }
}

fn desktop_about(desktop: Desktop) -> &'static str {
    match desktop {
        Desktop::Linexin => i18n::text("desktop-about-linexin"),
        Desktop::Kinexin => i18n::text("desktop-about-kinexin"),
        Desktop::LineXinBar => i18n::text("desktop-about-linexinbar"),
    }
}

fn desktop_mark(desktop: Desktop) -> &'static str {
    match desktop {
        Desktop::Linexin => "logo",
        Desktop::Kinexin => "setting-appearance",
        Desktop::LineXinBar => "pad-south",
    }
}

/// A start-up manager's name.
pub fn bootloader_name(bootloader: Bootloader) -> &'static str {
    match bootloader {
        Bootloader::Automatic => i18n::text("bootloader-automatic"),
        Bootloader::SystemdBoot => i18n::text("bootloader-systemd-boot"),
        Bootloader::Grub => i18n::text("bootloader-grub"),
        Bootloader::Refind => i18n::text("bootloader-refind"),
    }
}

fn bootloader_note(bootloader: Bootloader) -> &'static str {
    match bootloader {
        Bootloader::Automatic => i18n::text("bootloader-automatic-note"),
        Bootloader::SystemdBoot => i18n::text("bootloader-systemd-boot-note"),
        Bootloader::Grub => i18n::text("bootloader-grub-note"),
        Bootloader::Refind => i18n::text("bootloader-refind-note"),
    }
}

fn group_name(group: PackageCategory) -> &'static str {
    match group {
        PackageCategory::Applications => i18n::text("package-group-applications"),
        PackageCategory::Multimedia => i18n::text("package-group-multimedia"),
        PackageCategory::Network => i18n::text("package-group-network"),
        PackageCategory::Fonts => i18n::text("package-group-fonts"),
        PackageCategory::Libraries => i18n::text("package-group-libraries"),
    }
}

/// Whether restore points will really be made: asked for, and the system
/// stored the way that keeps them — judged by what the person has chosen as
/// a request carries it (`State::judged`): with Partitions' plan in use, a
/// Btrfs `/` of the plan's, and no separate `/var` beside it.
pub fn restore_points(state: &State) -> bool {
    let config = state.judged();
    config.software.snapshots && config.storage.filesystem == Filesystem::Btrfs
}

// -------------------------------------------------------------------------
// The step's own page.

/// The three cards, in the order the image's offers list them. Pictures are
/// added where they are drawn, so building the cards writes nothing.
fn cards(state: &State) -> Vec<Card> {
    let offline = offline(state);
    state
        .offers
        .iter()
        .map(|offer| {
            let downloaded = !offer.in_image;
            let state_words = if downloaded {
                i18n::text("desktop-needs-internet")
            } else {
                i18n::text("desktop-ready")
            };
            Card {
                name: desktop_name(offer.desktop).to_string(),
                note: desktop_about(offer.desktop).to_string(),
                mark: desktop_mark(offer.desktop),
                picture: None,
                state: Some(state_words.to_string()),
                chosen: state.draft.desktop == offer.desktop,
                enabled: true,
                refused: downloaded && offline,
            }
        })
        .collect()
}

/// Every note the "Install updates" row can say.
const UPDATES_NOTES: [&str; 4] = [
    "desktop-needs-internet",
    "desktop-checking-short",
    "desktop-updates-on",
    "desktop-updates-off",
];

/// The "Install updates" row: what it does, or why it cannot be had — one
/// of [`UPDATES_NOTES`].
fn updates_row(state: &State) -> (&'static str, bool) {
    if offline(state) {
        (i18n::text("desktop-needs-internet"), true)
    } else if checking(state) {
        (i18n::text("desktop-checking-short"), false)
    } else if state.draft.software.updates {
        (i18n::text("desktop-updates-on"), false)
    } else {
        (i18n::text("desktop-updates-off"), false)
    }
}

/// Take the card at `index`, or refuse it with the reason.
fn choose_card(state: &mut State, index: usize) {
    let Some(offer) = state.offers.get(index).copied() else {
        return;
    };
    state.views.desktop.cards.at = index;
    if needs_internet(state, offer.desktop) && offline(state) {
        state.sound(Sound::Error);
        let name = desktop_name(offer.desktop);
        let words = crate::message!("desktop-refused", "desktop" => name);
        state.say(words, Role::Danger);
        return;
    }
    state.draft.desktop = offer.desktop;
    state.views.desktop.focus = Focus::Buttons(CONTINUE);
    state.sound(Sound::Press);
}

/// Switch updates, or refuse with the reason where there is no internet.
fn flip_updates(state: &mut State) {
    if offline(state) {
        state.sound(Sound::Error);
        state.say(i18n::text("updates-refused"), Role::Danger);
        return;
    }
    state.draft.software.updates = !state.draft.software.updates;
    state.views.desktop.chose_updates = true;
    state.sound(Sound::Press);
}

/// Continue: on, unless the desktop chosen needs an internet there is none
/// of.
fn onward(state: &mut State) {
    let desktop = state.draft.desktop;
    if needs_internet(state, desktop) && offline(state) {
        state.sound(Sound::Error);
        let words = crate::message!("desktop-refused", "desktop" => desktop_name(desktop));
        state.say(words, Role::Danger);
        return;
    }
    state.forward();
}

fn buttons() -> [Button<'static>; 2] {
    [
        Button::plain(i18n::text("button-options")),
        Button::primary(i18n::text("button-continue")),
    ]
}

fn done() -> Button<'static> {
    Button::primary(i18n::text("button-done"))
}

/// Open a page of the step, the light at its first row.
fn open(state: &mut State, part: Part) {
    state.views.desktop.part = part;
    state.views.desktop.on_done = false;
    state.note = None;
    match part {
        Part::Main => {}
        Part::Options => {
            let rows = option_rows(state);
            state.views.desktop.options.start_at_chosen(&rows);
        }
        _ => {
            let rows = list_rows(state);
            state.views.desktop.list.start_at_chosen(&rows);
        }
    }
    state.sound(Sound::Press);
}

/// Back a page: from a list to Options, from Options to the step's own.
fn close(state: &mut State, pressed: bool) {
    let view = &mut state.views.desktop;
    view.on_done = false;
    view.part = match view.part {
        Part::Main | Part::Options => Part::Main,
        Part::Remove | Part::Flatpaks | Part::Bootloader => Part::Options,
    };
    state.note = None;
    state.sound(if pressed { Sound::Press } else { Sound::Back });
}

// -------------------------------------------------------------------------
// The pages of Options.

/// The Options page: each option with its state under it.
fn option_rows(state: &State) -> Vec<Row> {
    let software = &state.draft.software;
    let remove = match state.packages.known() {
        None => Row::new(i18n::text("desktop-option-remove"))
            .note(i18n::text("desktop-remove-looking"))
            .enabled(false),
        Some(Err(_)) => Row::new(i18n::text("desktop-option-remove"))
            .note(i18n::text("desktop-remove-unlisted"))
            .enabled(false),
        Some(Ok(listed)) if listed.is_empty() => Row::new(i18n::text("desktop-option-remove"))
            .note(i18n::text("desktop-remove-none"))
            .enabled(false),
        Some(Ok(_)) => {
            let count = software.remove.len() as i64;
            let note = if count == 0 {
                i18n::text("desktop-remove-nothing").to_string()
            } else {
                crate::message!("desktop-remove-count", "count" => count)
            };
            Row::new(i18n::text("desktop-option-remove")).note(note)
        }
    };
    let flatpaks = Row::new(i18n::text("desktop-option-flatpaks"));
    let flatpaks = if offline(state) {
        flatpaks
            .note(i18n::text("desktop-needs-internet"))
            .refused(true)
    } else if checking(state) {
        flatpaks.note(i18n::text("desktop-checking-short"))
    } else {
        let on = catalogs::flatpaks()
            .iter()
            .filter(|app| software.flatpaks.iter().any(|id| id == app.id))
            .count() as i64;
        let of = catalogs::flatpaks().len() as i64;
        let words = crate::message!("desktop-flatpaks-count", "count" => on, "total" => of);
        flatpaks.note(words)
    };
    let bootloader = Row::new(i18n::text("desktop-option-bootloader"))
        .note(bootloader_name(state.draft.bootloader));
    let drivers = Row::new(i18n::text("desktop-option-drivers"))
        .note(if software.prune_drivers {
            i18n::text("desktop-drivers-on")
        } else {
            i18n::text("desktop-drivers-off")
        })
        .switch(software.prune_drivers);
    // Restore points are kept only by the storage that keeps them, chosen
    // under Where to install (with Partitions' plan in use, a Btrfs / of the
    // plan's); with any other, the switch would do nothing, so it is
    // outlined, shown off, and says where it is set up. Beside a separate
    // /var of the plan's they are switched off, and it says why.
    let snapshots = Row::new(i18n::text("desktop-option-snapshots"));
    let snapshots = if state.judged().storage.filesystem != Filesystem::Btrfs {
        snapshots
            .note(i18n::text("desktop-snapshots-needs"))
            .switch(false)
            .enabled(false)
    } else if super::partitions::separate_var(state) {
        snapshots
            .note(i18n::text("desktop-snapshots-var"))
            .switch(false)
            .enabled(false)
    } else if software.snapshots {
        snapshots
            .note(i18n::text("desktop-snapshots-on"))
            .switch(true)
    } else {
        snapshots
            .note(i18n::text("desktop-snapshots-off"))
            .switch(false)
    };
    vec![remove, flatpaks, bootloader, drivers, snapshots]
}

/// One line of the software page: a group's heading, or a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    Group(PackageCategory),
    Package(usize),
}

/// The software page's lines: each group that has any, its packages under
/// it in the order pacman listed them.
fn software_lines(listed: &[Package]) -> Vec<Line> {
    let mut lines = Vec::new();
    for group in GROUPS {
        let mut members = listed
            .iter()
            .enumerate()
            .filter(|(_, package)| package.category == group)
            .peekable();
        if members.peek().is_some() {
            lines.push(Line::Group(group));
            lines.extend(members.map(|(index, _)| Line::Package(index)));
        }
    }
    lines
}

/// The rows of the list page up now.
fn list_rows(state: &State) -> Vec<Row> {
    let software = &state.draft.software;
    match state.views.desktop.part {
        Part::Remove => {
            let listed = packages(state);
            software_lines(listed)
                .into_iter()
                .map(|line| match line {
                    Line::Group(group) => Row::heading(group_name(group)),
                    Line::Package(index) => {
                        let package = &listed[index];
                        let kept = !software.remove.contains(&package.name);
                        let row = Row::new(package.name.clone()).switch(kept);
                        if package.description.trim().is_empty() {
                            row
                        } else {
                            row.note(package.description.trim())
                        }
                    }
                })
                .collect()
        }
        Part::Flatpaks => {
            let offline = offline(state);
            catalogs::flatpaks()
                .iter()
                .map(|app| {
                    let on = software.flatpaks.iter().any(|id| id == app.id);
                    let row = Row::new(app.name).switch(on && !offline);
                    if offline {
                        row.note(i18n::text("desktop-needs-internet")).refused(true)
                    } else if on {
                        row.note(i18n::text("desktop-flatpak-on"))
                    } else {
                        row.note(i18n::text("desktop-flatpak-off"))
                    }
                })
                .collect()
        }
        Part::Bootloader => {
            // Judged by what the person has chosen as a request carries it:
            // the place's disk — or, with Partitions' plan in use, the plan
            // (the table of its EFI system partition's disk, `layout::shape`).
            let firmware = state.firmware();
            let judged = state.judged();
            BOOTLOADERS
                .iter()
                .map(|bootloader| {
                    let mut candidate = judged.clone();
                    candidate.bootloader = *bootloader;
                    let refused = disks::validate_storage(&candidate, firmware)
                        == Err(Invalid::BootloaderUnavailable);
                    let row = Row::new(bootloader_name(*bootloader))
                        .chosen(state.draft.bootloader == *bootloader);
                    if refused {
                        row.note(i18n::text("bootloader-unavailable"))
                            .enabled(false)
                    } else {
                        row.note(bootloader_note(*bootloader))
                    }
                })
                .collect()
        }
        Part::Main | Part::Options => Vec::new(),
    }
}

/// Accept on the row at `index` of the Options page.
fn press_option(state: &mut State, index: usize) {
    match index {
        REMOVE => {
            if packages(state).is_empty() {
                state.sound(Sound::Error);
            } else {
                open(state, Part::Remove);
            }
        }
        FLATPAKS => {
            if offline(state) {
                state.sound(Sound::Error);
                state.say(i18n::text("flatpaks-refused"), Role::Danger);
            } else {
                open(state, Part::Flatpaks);
            }
        }
        BOOTLOADER => open(state, Part::Bootloader),
        DRIVERS => {
            state.draft.software.prune_drivers = !state.draft.software.prune_drivers;
            state.sound(Sound::Press);
        }
        SNAPSHOTS => {
            state.draft.software.snapshots = !state.draft.software.snapshots;
            state.sound(Sound::Press);
        }
        _ => {}
    }
}

/// Accept on the row at `index` of a list page.
fn press_line(state: &mut State, index: usize) {
    match state.views.desktop.part {
        Part::Remove => {
            let listed = packages(state);
            let Some(Line::Package(at)) = software_lines(listed).get(index).copied() else {
                return;
            };
            let name = listed[at].name.clone();
            let remove = &mut state.draft.software.remove;
            match remove.iter().position(|one| *one == name) {
                Some(found) => {
                    remove.remove(found);
                }
                None => remove.push(name),
            }
            state.sound(Sound::Press);
        }
        Part::Flatpaks => {
            if offline(state) {
                state.sound(Sound::Error);
                state.say(i18n::text("flatpaks-refused"), Role::Danger);
                return;
            }
            let Some(app) = catalogs::flatpaks().get(index) else {
                return;
            };
            let chosen = &mut state.draft.software.flatpaks;
            match chosen.iter().position(|id| id == app.id) {
                Some(found) => {
                    chosen.remove(found);
                }
                None => chosen.push(app.id.to_string()),
            }
            state.views.desktop.chose_flatpaks = true;
            state.sound(Sound::Press);
        }
        Part::Bootloader => {
            let rows = list_rows(state);
            if !rows.get(index).is_some_and(|row| row.enabled) {
                return;
            }
            state.draft.bootloader = BOOTLOADERS[index];
            close(state, true);
        }
        Part::Main | Part::Options => {}
    }
}

/// The rows of whichever page of Options is up.
fn page_rows(state: &State) -> Vec<Row> {
    if state.views.desktop.part == Part::Options {
        option_rows(state)
    } else {
        list_rows(state)
    }
}

// -------------------------------------------------------------------------
// The step.

/// The pictures of this step: its own page, the light on the first card;
/// or the Options page; or the software page; or — `desktop-loaders-plan` —
/// the start-up managers with Partitions' plan on the old msdos disk the
/// choice in use (`partitions::stage_plan_on_msdos`), which systemd-boot
/// cannot start: it is outlined, though the easy place would have it.
pub fn stage(state: &mut State, scene: &str) {
    match scene {
        "desktop-loaders-plan" => {
            super::partitions::stage_plan_on_msdos(state);
            state.views.partitions.in_use = true;
            state.views.desktop.part = Part::Bootloader;
            let rows = list_rows(state);
            state.views.desktop.list.start_at_chosen(&rows);
        }
        "desktop-options" => {
            state.views.desktop.part = Part::Options;
            let rows = option_rows(state);
            state.views.desktop.options.start_at_chosen(&rows);
        }
        "desktop-software" => {
            state.views.desktop.part = Part::Remove;
            let rows = list_rows(state);
            state.views.desktop.list.start_at_chosen(&rows);
            // One package switched off, so the picture shows both states.
            if let Some(first) = packages(state).first() {
                state.draft.software.remove.push(first.name.clone());
            }
        }
        _ => {}
    }
}

pub fn enter(state: &mut State) {
    // The internet is asked again on every arrival — a cable plugged in
    // since is worth knowing about — and the software only the first time.
    if let Machine::Real { .. } = state.machine {
        state.online.ask(catalogs::online);
        if state.packages.known().is_none() {
            state.packages.ask(catalogs::removable_packages);
        }
    }
    let at = state
        .offers
        .iter()
        .position(|offer| offer.desktop == state.draft.desktop)
        .unwrap_or(0);
    let view = &mut state.views.desktop;
    view.part = Part::Main;
    view.focus = Focus::Cards;
    view.cards.at = at;
    view.on_done = false;
}

pub fn head(state: &State) -> Head {
    let mark = "setting-appearance";
    match state.views.desktop.part {
        Part::Main => Head::new(
            mark,
            i18n::text("desktop-title"),
            i18n::text("desktop-caption"),
        ),
        Part::Options => Head::new(
            mark,
            i18n::text("desktop-options-title"),
            i18n::text("desktop-options-caption"),
        ),
        Part::Remove => Head::new(
            mark,
            i18n::text("desktop-remove-title"),
            i18n::text("desktop-remove-caption"),
        ),
        Part::Flatpaks => Head::new(
            mark,
            i18n::text("desktop-flatpaks-title"),
            i18n::text("desktop-flatpaks-caption"),
        ),
        Part::Bootloader => Head::new(
            mark,
            i18n::text("desktop-bootloader-title"),
            i18n::text("desktop-bootloader-caption"),
        ),
    }
}

pub fn hints(state: &State) -> Vec<Hint> {
    if state.views.desktop.part == Part::Main {
        super::choose_options_and_back()
    } else {
        super::choose_and_back()
    }
}

/// Said in the foot of the step's own page while the internet is being
/// asked, and once it is known to be out of reach.
pub fn note(state: &State) -> Option<Note> {
    if state.views.desktop.part != Part::Main {
        return None;
    }
    if checking(state) {
        Some(Note {
            text: i18n::text("desktop-checking").to_string(),
            role: Role::Text,
        })
    } else if offline(state) {
        Some(Note {
            text: i18n::text("desktop-offline").to_string(),
            role: Role::Text,
        })
    } else {
        None
    }
}

fn focus(state: &mut State, to: Focus) {
    if state.views.desktop.focus != to {
        state.views.desktop.focus = to;
        state.sound(Sound::Move);
    }
}

pub fn act(state: &mut State, action: Action, scroll: Option<Scroll>) {
    if state.views.desktop.part == Part::Main {
        act_main(state, action);
    } else {
        act_page(state, action, scroll);
    }
}

fn act_main(state: &mut State, action: Action) {
    if action == Action::Menu {
        return open(state, Part::Options);
    }
    if action == Action::Back {
        return state.back();
    }
    let view = &state.views.desktop;
    match view.focus {
        Focus::Cards => {
            let cards = cards(state);
            let stacked = view.stacked;
            let mut row = view.cards;
            match row.act(action, &cards, stacked) {
                Some(true) => {
                    state.views.desktop.cards = row;
                    state.sound(Sound::Move);
                }
                // Past the last card of a stack: the updates row.
                Some(false) if stacked && action == Action::Down => {
                    focus(state, Focus::Updates);
                }
                Some(_) => {}
                None => match action {
                    Action::Down => focus(state, Focus::Updates),
                    Action::Accept => choose_card(state, row.at),
                    _ => {}
                },
            }
        }
        Focus::Updates => match action {
            Action::Up => focus(state, Focus::Cards),
            Action::Down | Action::Right => focus(state, Focus::Buttons(CONTINUE)),
            Action::Accept => flip_updates(state),
            _ => {}
        },
        Focus::Buttons(at) => match action {
            Action::Left if at > 0 => focus(state, Focus::Buttons(at - 1)),
            Action::Right if at < CONTINUE => focus(state, Focus::Buttons(at + 1)),
            Action::Up => focus(state, Focus::Updates),
            Action::Accept if at == CONTINUE => onward(state),
            Action::Accept => open(state, Part::Options),
            _ => {}
        },
    }
}

fn act_page(state: &mut State, action: Action, scroll: Option<Scroll>) {
    let options = state.views.desktop.part == Part::Options;
    let ids = if options { OPTION_IDS } else { LIST_IDS };
    if scroll.is_some_and(|scroll| Column::owns(scroll.spot, ids)) {
        state.views.desktop.on_done = false;
    }
    if action == Action::Back {
        return close(state, false);
    }
    if state.views.desktop.on_done {
        match action {
            Action::Up | Action::Left => {
                state.views.desktop.on_done = false;
                state.sound(Sound::Move);
            }
            Action::Accept => close(state, true),
            _ => {}
        }
        return;
    }
    let rows = page_rows(state);
    let view = &mut state.views.desktop;
    let column = if options {
        &mut view.options
    } else {
        &mut view.list
    };
    match action {
        Action::Up => {
            if column.step(&rows, -1) {
                state.sound(Sound::Move);
            }
        }
        Action::Down => {
            // Past the last row, the light goes down to Done.
            if !column.step(&rows, 1) {
                view.on_done = true;
            }
            state.sound(Sound::Move);
        }
        Action::Right => {
            view.on_done = true;
            state.sound(Sound::Move);
        }
        Action::Accept => {
            let at = column.at;
            if options {
                press_option(state, at);
            } else {
                press_line(state, at);
            }
        }
        _ => {}
    }
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    if state.views.desktop.part == Part::Main {
        draw_main(state, page, room, lit);
    } else {
        draw_page(state, page, room, lit);
    }
}

fn draw_main(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let Room {
        content,
        buttons: band,
        ..
    } = room;
    let gap = page.metric(Metric::Gap);
    // The updates row stands under the cards, in the middle of the panel as
    // they are and as wide as a row reads well, and as tall as the longest
    // of its notes needs — whichever it says now: sized by the note in
    // force, the row grew or shrank when the internet check came back with a
    // note that wrapped differently, and the three cards over it all changed
    // size at once.
    let (note, refused) = updates_row(state);
    let updates = Toggle {
        name: i18n::text("desktop-updates"),
        note: Some(note),
        on: state.draft.software.updates && !refused,
        enabled: true,
        refused,
        mark: None,
    };
    let [left, _, width, _] = super::centred(content, page.scaled(760.0));
    let tall = UPDATES_NOTES
        .iter()
        .map(|id| {
            let note = Some(i18n::text(id));
            toggle::height(page, width, &Toggle { note, ..updates })
        })
        .fold(0.0f32, f32::max);
    let room = [
        content[0],
        content[1],
        content[2],
        (content[3] - tall - gap * 1.4).max(1.0),
    ];
    let mut cards = cards(state);
    for (card, offer) in cards.iter_mut().zip(&state.offers) {
        card.picture = pictures::path(offer.desktop);
    }
    let places = cards::places(page, room, cards.len());
    state.views.desktop.stacked = places.len() > 1 && places[0][1] != places[1][1];
    let focus = state.views.desktop.focus;
    let pressing = state.pressing.through();
    let dt = state.clock.dt;
    let row = state.views.desktop.cards;
    let light = (lit && focus == Focus::Cards).then_some(&mut state.light);
    row.draw(page, &places, &cards, light, pressing, dt);

    let bottom = places
        .iter()
        .map(|place| place[1] + place[3])
        .fold(room[1], f32::max);
    let rect = [left, bottom + gap * 1.4, width, tall];
    let light = (lit && focus == Focus::Updates).then_some(&mut state.light);
    toggle::draw(page, rect, updates, light, UPDATES, pressing, dt);

    let buttons = buttons();
    let places = button::places(page, band, &buttons);
    let lit_button = match focus {
        Focus::Buttons(at) if lit => Some(at),
        _ => None,
    };
    let laid = button::row(
        page,
        &places,
        &buttons,
        lit_button,
        &mut state.light,
        pressing,
        dt,
    );
    state.laid_buttons(laid);
}

fn draw_page(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    let Room {
        content,
        buttons: band,
        ..
    } = room;
    let rows = page_rows(state);
    // In the middle of the panel, as the step's own cards and row are.
    let view_rect = column::centred(page, content, page.scaled(900.0));
    let options = state.views.desktop.part == Part::Options;
    let look = Look {
        ids: if options { OPTION_IDS } else { LIST_IDS },
        pressing: state.pressing.through(),
        dt: state.clock.dt,
        pad: state.pad,
        bounds: content,
        corners: Corners::Fitted,
    };
    let on_done = state.views.desktop.on_done;
    let light = (lit && !on_done).then_some(&mut state.light);
    let view = &mut state.views.desktop;
    let column = if options {
        &mut view.options
    } else {
        &mut view.list
    };
    column.draw(page, view_rect, &rows, light, look);
    super::lone_button(state, page, band, done(), lit && on_done);
}

/// Put the light where the pointer is, on the page that is up: a card, the
/// updates row or a button of the step's own page, or a row or Done of a
/// page of Options. Answers whether it stands there.
fn point(state: &mut State, id: u32) -> bool {
    if state.views.desktop.part == Part::Main {
        let cards = cards(state);
        let view = &mut state.views.desktop;
        let to = if let Some(index) = Cards::card_of(id, cards.len()) {
            if !view.cards.point_at(&cards, index) {
                return false;
            }
            Focus::Cards
        } else if id == UPDATES {
            Focus::Updates
        } else if let Some(at) = super::button_of(id, buttons().len()) {
            Focus::Buttons(at)
        } else {
            return false;
        };
        view.focus = to;
        return true;
    }
    let rows = page_rows(state);
    let options = state.views.desktop.part == Part::Options;
    let view = &mut state.views.desktop;
    let (column, ids) = if options {
        (&mut view.options, OPTION_IDS)
    } else {
        (&mut view.list, LIST_IDS)
    };
    if let Some(index) = ids.row_of(id, rows.len()) {
        let landed = column.point_at(&rows, index);
        if landed {
            view.on_done = false;
        }
        return landed;
    }
    if super::button_of(id, 1).is_some() {
        view.on_done = true;
        return true;
    }
    false
}

pub fn hover(state: &mut State, id: u32) {
    point(state, id);
}

pub fn pressed(state: &mut State, page: &mut Page) {
    if state.views.desktop.part == Part::Main {
        pressed_main(state, page);
    } else {
        pressed_page(state, page);
    }
}

fn pressed_main(state: &mut State, page: &mut Page) {
    let cards = cards(state);
    if let Some(index) = Cards::clicked(page, &cards) {
        if point(state, spots::CARDS + index as u32) {
            act(state, Action::Accept, None);
        }
        return;
    }
    if page.pressed(UPDATES) {
        state.views.desktop.focus = Focus::Updates;
        flip_updates(state);
        return;
    }
    match button::pressed(page, &buttons()) {
        Some(CONTINUE) => {
            state.views.desktop.focus = Focus::Buttons(CONTINUE);
            onward(state);
        }
        Some(at) => {
            state.views.desktop.focus = Focus::Buttons(at);
            open(state, Part::Options);
        }
        None => {}
    }
}

fn pressed_page(state: &mut State, page: &mut Page) {
    let rows = page_rows(state);
    let options = state.views.desktop.part == Part::Options;
    let view = &mut state.views.desktop;
    let (column, ids) = if options {
        (&mut view.options, OPTION_IDS)
    } else {
        (&mut view.list, LIST_IDS)
    };
    match column.pressed(page, &rows, ids) {
        Some(Pick::Moved(_)) => {
            state.views.desktop.on_done = false;
            state.sound(Sound::Move);
            return;
        }
        Some(Pick::Clicked(index)) => {
            if point(state, ids.row(index)) {
                act(state, Action::Accept, None);
            }
            return;
        }
        None => {}
    }
    if super::lone_pressed(page, done()) {
        close(state, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::catalogs::DesktopOffer;
    use crate::ui::app::Screen;

    fn on_desktop() -> State {
        let mut state = State::demo(true);
        state.go(Screen::Desktop);
        state
    }

    /// The made-up machine with its internet unplugged.
    fn offline_desktop() -> State {
        let mut state = on_desktop();
        state.online.tell(false);
        online_known(&mut state);
        state
    }

    #[test]
    fn online_updates_and_every_app_start_switched_on() {
        let state = on_desktop();
        assert!(state.draft.software.updates);
        assert_eq!(
            state.draft.software.flatpaks.len(),
            catalogs::flatpaks().len()
        );
        assert_eq!(state.draft.desktop, Desktop::Linexin);
    }

    #[test]
    fn the_updates_row_is_sized_by_every_note_it_can_say() {
        let notes = UPDATES_NOTES.map(i18n::text);
        let mut state = on_desktop();
        for updates in [true, false] {
            state.draft.software.updates = updates;
            assert!(notes.contains(&updates_row(&state).0));
        }
        assert!(notes.contains(&updates_row(&offline_desktop()).0));
    }

    #[test]
    fn offline_a_downloaded_desktop_is_refused_with_the_error_and_a_note() {
        let mut state = offline_desktop();
        let rows = cards(&state);
        // The made-up medium is the 3.0 stick: GNOME and LineXinBar on it,
        // Kinexin downloaded.
        assert!(!rows[0].refused, "Linexin is on the USB stick");
        assert!(rows[1].refused, "Kinexin is downloaded");
        assert!(!rows[2].refused, "LineXinBar is on the USB stick");
        choose_card(&mut state, 1);
        assert_eq!(
            state.draft.desktop,
            Desktop::Linexin,
            "a refused card was taken"
        );
        assert!(state.note.is_some(), "the refusal was not said");
        assert!(!state.draft.software.updates && state.draft.software.flatpaks.is_empty());
        flip_updates(&mut state);
        assert!(
            !state.draft.software.updates,
            "updates were switched on offline"
        );
        state.note = None;
        choose_card(&mut state, 2);
        assert_eq!(
            state.draft.desktop,
            Desktop::LineXinBar,
            "offline, from the stick"
        );
        assert!(state.note.is_none(), "LineXinBar was refused offline");
    }

    #[test]
    fn online_a_downloaded_desktop_is_taken_and_the_light_goes_to_continue() {
        let mut state = on_desktop();
        act(&mut state, Action::Right, None);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.draft.desktop, Desktop::Kinexin);
        assert_eq!(state.views.desktop.focus, Focus::Buttons(CONTINUE));
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Account);
    }

    #[test]
    fn a_desktop_chosen_online_cannot_be_continued_with_once_offline() {
        let mut state = on_desktop();
        state.draft.desktop = Desktop::Kinexin;
        state.online.tell(false);
        online_known(&mut state);
        onward(&mut state);
        assert_eq!(state.screen, Screen::Desktop);
        assert!(state.note.is_some());

        // One the stick carries goes on.
        state.note = None;
        state.draft.desktop = Desktop::LineXinBar;
        onward(&mut state);
        assert_eq!(state.screen, Screen::Account);
    }

    #[test]
    fn switching_updates_yourself_is_kept_when_the_internet_answers_again() {
        let mut state = on_desktop();
        state.views.desktop.focus = Focus::Updates;
        act(&mut state, Action::Accept, None);
        assert!(!state.draft.software.updates);
        state.online.tell(true);
        online_known(&mut state);
        assert!(
            !state.draft.software.updates,
            "the person's choice was undone"
        );
    }

    #[test]
    fn options_open_as_a_page_and_back_comes_out_a_page_at_a_time() {
        let mut state = on_desktop();
        act(&mut state, Action::Menu, None);
        assert_eq!(state.views.desktop.part, Part::Options);
        state.views.desktop.options.at = FLATPAKS;
        act(&mut state, Action::Accept, None);
        assert_eq!(state.views.desktop.part, Part::Flatpaks);
        act(&mut state, Action::Accept, None);
        assert_eq!(
            state.draft.software.flatpaks.len(),
            catalogs::flatpaks().len() - 1,
            "the first app was not switched off"
        );
        act(&mut state, Action::Back, None);
        assert_eq!(state.views.desktop.part, Part::Options);
        act(&mut state, Action::Back, None);
        assert_eq!(state.views.desktop.part, Part::Main);
        assert_eq!(state.screen, Screen::Desktop);
    }

    #[test]
    fn hovering_lights_a_card_the_updates_row_a_button_and_an_options_row() {
        let mut state = on_desktop();
        let desktop = state.draft.desktop;
        hover(&mut state, spots::CARDS + 2);
        let view = &state.views.desktop;
        assert_eq!((view.focus, view.cards.at), (Focus::Cards, 2));
        assert_eq!(state.draft.desktop, desktop, "a hover chose a desktop");
        hover(&mut state, UPDATES);
        assert_eq!(state.views.desktop.focus, Focus::Updates);
        assert!(state.draft.software.updates, "a hover switched updates");
        hover(&mut state, spots::BUTTONS + CONTINUE as u32);
        assert_eq!(state.views.desktop.focus, Focus::Buttons(CONTINUE));
        act(&mut state, Action::Menu, None);
        hover(&mut state, OPTION_IDS.row(BOOTLOADER));
        assert_eq!(state.views.desktop.options.at, BOOTLOADER);
        assert_eq!(
            state.views.desktop.part,
            Part::Options,
            "a hover opened a page"
        );
        hover(&mut state, spots::BUTTONS);
        assert!(state.views.desktop.on_done);
        hover(&mut state, OPTION_IDS.row(REMOVE));
        assert!(!state.views.desktop.on_done);
    }

    #[test]
    fn the_software_page_is_grouped_and_a_switch_leaves_a_package_out() {
        let mut state = on_desktop();
        state.views.desktop.part = Part::Remove;
        let rows = list_rows(&state);
        assert!(
            rows[0].heading && !rows[0].enabled,
            "a group heads the list"
        );
        let headings = rows.iter().filter(|row| row.heading).count();
        assert_eq!(headings, GROUPS.len());
        assert_eq!(rows.len(), packages(&state).len() + headings);
        state.views.desktop.list.start_at_chosen(&rows);
        let at = state.views.desktop.list.at;
        assert_eq!(at, 1, "the light started on a heading");
        press_line(&mut state, at);
        assert_eq!(state.draft.software.remove, vec![rows[1].name.clone()]);
        assert_eq!(list_rows(&state)[1].switch, Some(false));
        press_line(&mut state, at);
        assert!(state.draft.software.remove.is_empty());
    }

    #[test]
    fn a_start_up_manager_this_computer_cannot_use_is_outlined() {
        let mut state = on_desktop();
        state.views.desktop.part = Part::Bootloader;
        // The empty disk erased whole, on UEFI: every manager can be used.
        assert!(list_rows(&state).iter().all(|row| row.enabled));
        // Free space on a disk with an old DOS table, which systemd-boot
        // cannot start from: it alone is outlined.
        let mut disk = state.disks[1].clone();
        disk.table = Some(crate::model::disks::Table::Msdos);
        disk.partitions.truncate(1);
        let gap = disk.free[0];
        state.draft.disk = disk;
        state.draft.target = crate::model::config::Target::FreeSpace {
            start: gap.start,
            size: gap.size,
        };
        let refused: Vec<String> = list_rows(&state)
            .into_iter()
            .filter(|row| !row.enabled)
            .map(|row| row.name)
            .collect();
        assert_eq!(
            refused,
            [bootloader_name(Bootloader::SystemdBoot).to_string()]
        );
        press_line(&mut state, 1);
        assert_eq!(
            state.draft.bootloader,
            Bootloader::Automatic,
            "a refused manager was taken"
        );
        press_line(&mut state, 2);
        assert_eq!(state.draft.bootloader, Bootloader::Grub);
        assert_eq!(state.views.desktop.part, Part::Options);
    }

    #[test]
    fn restore_points_say_they_need_the_storage_that_keeps_them() {
        let mut state = on_desktop();
        state.draft.storage.filesystem = Filesystem::Ext4;
        assert!(!restore_points(&state));
        let rows = option_rows(&state);
        assert!(!rows[SNAPSHOTS].enabled, "a switch that does nothing");
        assert_eq!(
            rows[SNAPSHOTS].note.as_deref(),
            Some(i18n::text("desktop-snapshots-needs"))
        );
        state.draft.storage.filesystem = Filesystem::Btrfs;
        assert!(restore_points(&state));
        assert!(option_rows(&state)[SNAPSHOTS].enabled);
    }

    /// With Partitions' plan the choice in use, the start-up managers are
    /// judged by the plan, not by the place the draft still holds: the
    /// draft's empty disk erased whole lets every one start Linexin, the
    /// plan's EFI system partition on an msdos table not systemd-boot — and
    /// let go of, every one again.
    #[test]
    fn with_the_plan_in_use_the_start_up_managers_are_judged_by_the_plan() {
        let mut state = on_desktop();
        state.views.desktop.part = Part::Bootloader;
        let refused = |state: &State| -> Vec<String> {
            list_rows(state)
                .into_iter()
                .filter(|row| !row.enabled)
                .map(|row| row.name)
                .collect()
        };
        assert!(refused(&state).is_empty(), "the place's");
        super::super::partitions::stage_plan_on_msdos(&mut state);
        assert!(refused(&state).is_empty(), "a plan not in use");
        state.views.partitions.in_use = true;
        assert_eq!(super::super::partitions::missing(&state), None);
        assert_eq!(
            refused(&state),
            [bootloader_name(Bootloader::SystemdBoot).to_string()]
        );
        press_line(&mut state, 1);
        assert_eq!(state.draft.bootloader, Bootloader::Automatic);
        super::super::partitions::let_go_of(&mut state);
        assert!(refused(&state).is_empty(), "the place's again");
    }

    /// Restore points with Partitions' plan in use are the plan's: kept by
    /// a Btrfs `/` of the plan's, needing one where `/` is ext4, and switched
    /// off — outlined, saying why — beside a separate `/var`.
    #[test]
    fn with_the_plan_in_use_restore_points_follow_its_root_and_its_var() {
        use crate::ui::partitions::{Fs, Mount, NewPart};
        let mut state = on_desktop();
        state.draft.storage.filesystem = Filesystem::Btrfs;
        assert!(restore_points(&state), "the place's Btrfs");
        super::super::partitions::stage_plan_on_msdos(&mut state);
        state.views.partitions.in_use = true;
        assert!(!restore_points(&state), "the plan's / is ext4");
        assert_eq!(
            option_rows(&state)[SNAPSHOTS].note.as_deref(),
            Some(i18n::text("desktop-snapshots-needs"))
        );
        // The plan's / made Btrfs: kept.
        let root = state.views.partitions.plan.operations()[5].clone();
        let crate::ui::partitions::Operation::Create { disk, new, .. } = root else {
            panic!("the plan's / is made last");
        };
        let plan = &mut state.views.partitions.plan;
        plan.undo_last();
        let half = new.size / 2 / crate::model::disks::MIB * crate::model::disks::MIB;
        plan.create(
            &disk,
            NewPart {
                fs: Fs::Btrfs,
                size: half,
                ..new.clone()
            },
        );
        assert!(restore_points(&state));
        assert!(option_rows(&state)[SNAPSHOTS].enabled);
        // And a /var of its own beside it: switched off, saying why.
        let number = state.views.partitions.plan.next_number();
        state.views.partitions.plan.create(
            &disk,
            NewPart {
                number,
                start: new.start + half,
                size: new.size - half,
                fs: Fs::Ext4,
                mount: Some(Mount::Other("/var".into())),
                label: String::new(),
            },
        );
        assert!(!restore_points(&state));
        let row = &option_rows(&state)[SNAPSHOTS];
        assert!(!row.enabled);
        assert_eq!(
            row.note.as_deref(),
            Some(i18n::text("desktop-snapshots-var"))
        );
        assert!(state.draft.software.snapshots, "the person's own choice");
    }

    #[test]
    fn a_desktop_the_image_carries_needs_no_internet() {
        let mut state = on_desktop();
        state.offers = vec![DesktopOffer {
            desktop: Desktop::LineXinBar,
            in_image: true,
        }];
        assert!(!needs_internet(&state, Desktop::LineXinBar));
        assert!(needs_internet(&state, Desktop::Kinexin));
    }
}
