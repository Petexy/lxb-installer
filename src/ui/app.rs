//! The interface's state and its frame: one value holding everything the
//! person has chosen so far, which screen they are on, and what the machine
//! said when it was read.
//!
//! **Every frame is a whole frame.** The page is `driven().plain()`: the
//! toolkit draws the wallpaper and nothing else, and this draws everything on
//! it — the one pane of glass, its head, the screen inside it and the legend
//! in its foot — every time, from the state, so a picture taken with `--shot`
//! and a window are the same drawing. The order inside a frame is the
//! family's: the answer to a question asked last frame, then this frame's
//! presses (each wheel gesture walked beside the directions it turned into),
//! then the drawing, and only then the question or menu those presses asked
//! for and the pointer's presses — a press is asked of the spots this frame's
//! drawing wrote down, never of last frame's, and a menu grows out of a
//! button where this frame drew it.
//!
//! Sounds and questions are not played or raised where a press is answered:
//! a screen answering a press has no page in hand, so it says what should be
//! heard ([`State::sound`]) or asked ([`State::question`]) and the frame does
//! it. That also keeps every screen's `act` a plain function of the state.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use lxb_app::lxb_render::{Carry, Drawn, Pressing, Spot};
use lxb_app::lxb_toolkit::{
    board as shells_board, input::Action, metrics::Metric, palette::Role, sound::Sound,
    typography::Text,
};
use lxb_app::{Hold, Page, Scroll};

use crate::i18n;
use crate::launch::MeasureEvent;
use crate::model::catalogs::{self, DesktopOffer, KeyboardLayout, Package, Zone};
use crate::model::config::Invalid;
use crate::model::config::{
    Account, Bootloader, Desktop, InstallConfig, Region, Software, Storage, Target,
    DEFAULT_HOSTNAME, DEFAULT_SOURCE,
};
use crate::model::disks::{self, Blocked, Disk, Transport};
use crate::model::firmware::Firmware;
use crate::model::layout::{self, Unpinned};
use crate::model::live::LiveSession;
use crate::model::places::Names;
use crate::model::protocol::{Event, Failure, Request, Step};
use crate::model::secret::Secret;

use super::asked::Asked;
use super::board::{Board, Stroke};
use super::button::LaidRow;
use super::change::{self, Change, Placing, Way};
use super::demo::{self, Rehearsal};
use super::field::{self, Edit, FieldId};
use super::install::{Ending, Install};
use super::legend::{self, Hint};
use super::menus;
use super::motion::{halves, Carried, Clock, Light};
use super::panel::{self, LaidFoot, Note, Panel};
use super::progress::Progress;
use super::screens;
use super::text;
use super::trail::Seen;
use super::words;

/// How soon after a click that put another page under the pointer a click
/// is the second half of a double-click, in seconds (`State::click`): a
/// little over the 400 ms GTK and Qt take for a double-click by default, and
/// shorter than anybody takes to read a page that has just come in.
const DOUBLE_CLICK: f32 = 0.5;

/// Every screen of the installer, in the order the person meets them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    Welcome,
    Language,
    Timezone,
    Keyboard,
    Target,
    Desktop,
    Account,
    Review,
    Installing,
    Done,
    Stopped,
}

impl Screen {
    /// The screens that ask something, in order: what Continue walks forward
    /// through and Back walks back through. The three after them are where an
    /// installation is followed, and nothing walks back into the questions
    /// from there.
    pub const ASKING: [Screen; 8] = [
        Screen::Welcome,
        Screen::Language,
        Screen::Timezone,
        Screen::Keyboard,
        Screen::Target,
        Screen::Desktop,
        Screen::Account,
        Screen::Review,
    ];

    /// The screen Continue leads to, if this is one of the questions and not
    /// the last of them.
    pub fn next(self) -> Option<Screen> {
        let at = Self::ASKING.iter().position(|one| *one == self)?;
        Self::ASKING.get(at + 1).copied()
    }

    /// The screen Back leads to, if there is one to go back to.
    pub fn previous(self) -> Option<Screen> {
        let at = Self::ASKING.iter().position(|one| *one == self)?;
        at.checked_sub(1).map(|before| Self::ASKING[before])
    }

    /// Where in the flow this screen is, for the chips in the head: the
    /// questions after Welcome, counted from nought. Welcome has none — it
    /// asks nothing — and nor has anything after the review.
    pub fn place(self) -> Option<panel::Place> {
        let at = Self::ASKING.iter().position(|one| *one == self)?;
        let of = Self::ASKING.len() - 1;
        at.checked_sub(1).map(|place| panel::Place { place, of })
    }
}

/// Which machine the interface is looking at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Machine {
    /// `--demo` and `--shot`: the made-up disks of [`demo::disks`], and a
    /// rehearsal of an installation instead of a worker. **A demo never
    /// starts a worker**, whatever is pressed.
    Demo,
    /// The computer the installer is running on, read without changing it.
    /// `live` is whether it was started from a Linexin live medium, which is
    /// the only place an installation can be started from.
    Real { live: Option<LiveSession> },
}

/// A question the frame is to put to the person with the toolkit's dialog.
///
/// Always asked safe-first (`Page::ask` puts the light on the first answer),
/// so a press of Accept that was meant for the screen behind never destroys
/// anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// Back from Welcome: leave the installer?
    Quit,
    /// Install, pressed on the review: the last word before the disk is
    /// written. The easy path's; asked while Partitions' plan is the choice
    /// in use, it is the plan's own question instead (`raise`).
    Erase,
    /// Install, pressed on the review while Partitions' plan is the choice in
    /// use: everything the plan destroys, in GParted's words, on as many
    /// pages as it takes — `page` the one up, from 0 — and **never "N more"**
    /// (by design, §2). "Go back" and Next on every page but the
    /// last, "Go back" and "Erase and install" on the last, "Go back" lit
    /// first on each (`screens::partitions::plan_pages`).
    PlanErase { page: usize },
    /// Partitions' Options, Pending operations: the whole list of what its
    /// plan will do, in GParted's words, and Close — a question only in that
    /// the dialog is what holds it; nothing hangs on the answer.
    Pending,
}

/// A menu a screen wants the frame to raise: the toolkit's own, growing out
/// of the button the screen drew it from (`State::anchor_menus`). `id` is the
/// screen's own number for it, handed back with the row chosen (see
/// `screens::chose`); `marked` is the row in force, which wears the `chosen`
/// mark.
///
/// **A menu of options says the setting in force on every row**, in every
/// language ("Przechowywanie · Standardowe"): the toolkit's menu grows to
/// its widest row, and the catalogs' test holds every such row, with every
/// setting it can say, to the widest menu a handheld standing on its side
/// allows — and its shorter words, where it has them, to the narrowest
/// display's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    pub id: u32,
    pub title: Option<String>,
    pub commands: Vec<String>,
    /// For each command, the name in it the machine gave (a partition's
    /// label), if any: what is cut where its row does not fit, never the
    /// catalog's words around it (`menus::row_words`). Empty for a menu of
    /// plain words.
    pub named: Vec<Option<String>>,
    /// For each command, shorter words for the same, if it has any: what is
    /// said where the whole does not fit the widest menu this display
    /// allows. Empty where no row has any.
    pub shorter: Vec<Option<String>>,
    pub marked: Option<usize>,
    /// Where the light opens in a menu of acts with nothing marked: the row
    /// a person comes back to (`Page::menu_on`). Its first row where `None`.
    pub on: Option<usize>,
}

/// Everything the interface knows.
pub struct State {
    pub machine: Machine,
    /// What the person has chosen so far, as the request the worker will be
    /// handed. Built from the model's defaults and changed screen by screen.
    pub draft: InstallConfig,
    /// The second typing of each password, and the field nothing is installed
    /// from — kept beside the draft because the request has no place for them.
    pub repeat: Secret,
    pub root_password: Secret,
    pub root_repeat: Secret,
    pub keyboard_test: String,
    /// Every disk of the machine, those that cannot be chosen included.
    pub disks: Vec<Disk>,
    /// Why the disks could not be read, if they could not: technical, for
    /// the log only. The face of the panel says it in plain words.
    pub disks_trouble: Option<String>,
    pub screen: Screen,
    /// The one light of the whole installer.
    pub light: Light,
    pub clock: Clock,
    /// The page drawn on the last frame, and the change of page under way
    /// since it gave way to another (`change`).
    drawn_page: Option<screens::Stood>,
    pub change: Change,
    /// The row of chips in the head that says which step this is.
    chips: panel::Chips,
    /// What the rows of buttons drawn so far on the page being drawn left on
    /// the frame.
    laid_buttons: Vec<LaidRow>,
    /// Whether the toolkit counts this session as low-end hardware
    /// (`Page::low_end`), where nothing of the installer's own moves; and
    /// whether a picture is to be drawn as there (`--shot --low-end`).
    low_end: bool,
    pretend_low_end: bool,
    /// The dip of a press, which the toolkit does not keep for a page that
    /// steers itself.
    pub pressing: Pressing,
    /// The installer's own on-screen keyboard, while it is open.
    pub board: Option<Board>,
    /// Whether a pad types with LineXinBar's own on-screen keyboard rather
    /// than the installer's: decided as the window opens
    /// (`board::under_linexinbar_here`), never for a picture or a test.
    shell_board: bool,
    /// The field a pad is typing into with the shell's board: one Accept
    /// was pressed on with a pad in hand, under LineXinBar. It takes text
    /// while the light stays in it, which is what brings the shell's board
    /// up over it; Enter (which the shell's board sends as it is put away
    /// with Start), Back and the light leaving the field let it go.
    shell_typing: Option<FieldId>,
    /// Whether the field taking text lets go of it for one frame: the shell
    /// offers its board once for a field that takes text, and a field whose
    /// board was put away asks for it again so — a pad's South on it, the
    /// legend's Type, a click or a tap on it.
    summon_again: bool,
    /// Where LineXinBar's board stands over the window on this frame, as the
    /// toolkit knows it (`Page::shells_board`): up over a field taking text
    /// with a controller in the shell's hands, however the field was chosen —
    /// a pad's South, a click, a finger on a handheld's screen — and gone the
    /// moment it is put away. `None` outside LineXinBar.
    board_over: Option<[f32; 4]>,
    /// Whether the board stood over the window on the frame before this one:
    /// what keys arriving now were typed beside. A key arriving where it did
    /// not is a keyboard of the person's own; where it did, the board's.
    board_before: bool,
    /// The field the board went away from while it took text — its own hide
    /// key, B, a key of the person's own — which a click or a tap on it asks
    /// the board back for (`clicked_field`).
    put_away_from: Option<FieldId>,
    /// A picture drawn as under LineXinBar with its board typing into the
    /// lit field (`--shot --shells-board`), which no toolkit tells it of.
    pretend_board: bool,
    /// How much of the window's foot the shell's board takes on this frame,
    /// in the window's pixels: from its top down while it stands over the
    /// window (`board_over`), nothing otherwise, carried on the cards' spring
    /// as it rises and goes down. The screen's room ends over it, as it ends
    /// over the installer's own board, so the field typed into stands in
    /// view: the shell draws its board wherever the field is.
    shell_room: Carried,
    /// Whether `shell_room` is put where it belongs on the next frame rather
    /// than carried there: a picture settled.
    shell_room_at_once: bool,
    /// The field the light stands in on this frame, whatever is in hand —
    /// where `standing_in` is only the one that takes a keyboard's typing.
    field_lit: Option<FieldId>,
    /// A line for the foot of the panel that a press left there (a refusal),
    /// until the screen changes.
    pub note: Option<Note>,
    /// The installation being followed, from the worker or the rehearsal.
    pub install: Install,
    pub progress: Progress,
    pub events: Option<Receiver<Event>>,
    pub rehearsal: Option<Rehearsal>,
    /// What each screen keeps between frames: where its light stands, how
    /// far its columns are scrolled.
    pub views: screens::Views,
    /// The time zones and keyboard layouts offered: the live system's, or
    /// the made-up machine's few.
    pub zones: Vec<Zone>,
    pub layouts: Vec<KeyboardLayout>,
    /// Whether the person chose a time zone or a keyboard themselves. Until
    /// they do, choosing a language guesses both from it; afterwards a
    /// language chosen again leaves their choice alone.
    pub chose_zone: bool,
    pub chose_keyboard: bool,
    /// The three desktops, and whether the image carries each (so it
    /// installs with no internet).
    pub offers: Vec<DesktopOffer>,
    /// Whether the internet can be reached: asked off the frame's thread
    /// each time the Desktop step is reached, because finding out can take
    /// seconds.
    pub online: Asked<bool>,
    /// The software of the image a person may leave out, read off the
    /// frame's thread the first time the Desktop step is reached. The error
    /// is a line for the log.
    pub packages: Asked<Result<Vec<Package>, String>>,
    /// Whether a pad is what the person last reached for, as the page said
    /// at the start of this frame. Kept here because a press is answered
    /// without the page in hand, and read wherever the frame draws what a
    /// pad changes — the legend's pictures, a column's scroll bar.
    pub pad: bool,
    /// Whether the toolkit itself says a pad is in hand, the pad held for
    /// the shell's board (`pad_held`) left out: what tells a pad's South
    /// from Enter.
    pad_pressed: bool,
    /// Whether a pad is taken to be in hand because a pad summoned the
    /// shell's board, whose keys reach the window as a keyboard's: what
    /// the shell's board types must not make the installer answer as to a
    /// keyboard — keyboard pictures in the legend, a field hovered taking
    /// text and the shell's board coming up over it unasked. Let go of by a
    /// pad's own press, and by a key once nothing is being typed with the
    /// shell's board, which is a keyboard of the person's own.
    pad_held: bool,
    /// A picture of the screen with a pad in hand (`--shot --pad`): the
    /// toolkit takes a picture with nothing in hand, which is the keyboard.
    pretend_pad: bool,
    /// A demo window opened as with a pad in hand (`--demo --pad`), which
    /// the pointer does not put down and the first key does: how a window
    /// with no pad to read — the sandbox gives it none — is driven as a pad
    /// drives it, by clicks, and typed into by a board's keys.
    pretend_pad_until_a_key: bool,
    /// Where a picture's pointer rests (`--shot --pointer`), which the
    /// toolkit, with no window, never says; and whether what it rests on
    /// has been hovered yet.
    pretend_pointer: Option<[f32; 2]>,
    pointer_hovered: bool,
    /// The field the light stands in on this frame, if it stands in one:
    /// where text typed on a keyboard goes. Written by the screen as it is
    /// drawn and read on the next frame, when the keys typed since arrive.
    pub standing_in: Option<FieldId>,
    /// The field the light stands in on this frame while a pad is in hand,
    /// which takes a keyboard's typing with nothing said to the compositor
    /// (`Page::taking_keys`): a keyboard reached for types into it from its
    /// first key, and no on-screen keyboard rises over it unasked. Without
    /// it, after the shell's board had typed, the pad stayed held and every
    /// letter of the person's own keyboard was lost — the toolkit turns a
    /// letter into no press — until a key that is a press came and acted:
    /// Space moved the light down a field, Backspace left You.
    quiet_in: Option<FieldId>,
    /// The field being typed in: one typed into, or one a key brought the
    /// light into, for as long as the light stays in it. Hovering does not
    /// take the light out of it — the light is where typing goes, and a
    /// pointer nudged between two keystrokes (a mouse knocked, a handheld's
    /// trackpad brushed) would have sent the rest of a password into the
    /// plain-text field it rested on. A key, a pad or a click still moves
    /// the light, and the light leaving the field lets it go.
    typing_held: Option<FieldId>,
    /// When a click last put another page under the pointer
    /// (`screens::page`), by the installer's clock: a click soon after it is
    /// the second half of a double-click (`State::click`).
    paged_by_click: Option<f32>,
    /// Whether a key or a pad pressed something since the last frame was
    /// drawn, on the screen drawn now.
    keyed: bool,
    question: Option<Question>,
    /// The question whose dialog is up, answered when the toolkit says how.
    answering: Option<Question>,
    /// The lines of what Partitions' plan destroys — the consent its request
    /// carries (`Destroy::line`) — as they were when the first page of its
    /// question was put up (`Question::PlanErase`): what `begin` holds the
    /// request's consent to, so nothing is erased that the pages did not
    /// say. `None` until a first page is up, and again once it is gone.
    pub asked: Option<Vec<String>>,
    /// The pages of the plan's question, made once as its first page is put
    /// up, for the window as it is then, so every page after it goes on
    /// exactly where the one before stopped; and whether its last page has
    /// been put up since (`begin` goes on only once it has).
    plan_pages: Vec<String>,
    plan_read: bool,
    /// How the disks are listed again before the plan's question on a live
    /// system: as at the start, read-only, as the person (`disks::discover`).
    /// A test that stands the made-up machine off the demo hands it disks of
    /// its own here, so no test lists this computer's.
    pub list_disks: fn() -> Result<Vec<Disk>, String>,
    /// What reading the disks with privilege has found, for Partitions to
    /// judge a plan by (`screens::partitions::Measuring`): asked on a live
    /// system only, through [`measure`](Self::measure).
    pub measuring: screens::partitions::Measuring,
    /// How the disks are measured on a live system: the read-only measure
    /// entry through polkit (`launch::measure`, which asks the live guard
    /// first). A test that stands the made-up machine on a live session
    /// hands it a measurement of its own here; in a test build the one it
    /// starts with refuses to be used (`measured_in_a_test`), so no test can
    /// reach for polkit, or the guard's look at this computer, even by
    /// forgetting to.
    pub measure: fn(Vec<Disk>) -> Receiver<MeasureEvent>,
    menu: Option<Menu>,
    /// The menu up now, and the screen that raised it, so its answer goes
    /// back to that screen and nowhere else.
    menu_open: Option<(Screen, u32)>,
    /// Where the screen drew the button its menus grow out of on this frame
    /// — Options — if it drew one.
    ///
    /// The toolkit grows a menu out of wherever the page last said the light
    /// was. Left to that, Options pressed with the pointer (or the right
    /// button, or North) while the light stood on a disk opened its menu
    /// beside the disk, and pressed while the light stood on Options opened
    /// it beside Options: two places for one menu. So the frame tells the
    /// toolkit this rectangle, and nothing else, just before it raises a
    /// menu.
    menu_anchor: Option<[f32; 4]>,
    /// The catalog the installer is to speak from, once this frame's
    /// presses are answered. Switched by the frame rather than by the press,
    /// so a test answering presses never changes the words every other test
    /// reads.
    speak: Option<&'static str>,
    /// The language LineXinBar's live session is to follow, by its own tag,
    /// once this frame's presses are answered. Written by the frame, and
    /// only on a live system, so no test and no picture ever touches the
    /// settings of the account that runs them.
    shell_language: Option<&'static str>,
    sounds: Vec<Sound>,
    leaving: bool,
    /// Restart now was pressed: the frame restarts the computer (a live
    /// session only) and closes the window.
    restarting: bool,
    /// The firmware a test stands the made-up machine on when it stands it
    /// off the demo (`Machine::Real`), where `firmware` would otherwise read
    /// this computer's (`Firmware::detect`) and the test's answer would be a
    /// fact about the computer running it — which the fence refuses in a
    /// test build, so such a test must pretend one.
    pretend_firmware: Option<Firmware>,
}

/// The stand-in disk of a draft with no place chosen in it: on a machine
/// with no disk to offer, or one whose first disk holds something and has no
/// free space Linexin fits in. A value no check passes, so nothing can be
/// installed until the person chooses a real place.
fn no_disk() -> Disk {
    Disk {
        path: String::new(),
        diskseq: None,
        model: String::new(),
        serial: String::new(),
        size: 0,
        major_minor: String::new(),
        logical_sector: 512,
        transport: Transport::Other,
        removable: false,
        rotational: false,
        table: None,
        content: None,
        read_only: true,
        blocked: Some(Blocked::Unknown),
        partitions: Vec::new(),
        free: Vec::new(),
    }
}

/// The request as it stands before anything has been asked: the model's own
/// defaults, the legacy installer's computer name, the live system's image,
/// and on the first disk that could be chosen at all the place Where to
/// install starts with (`target::first_place`) — the whole disk only where it
/// holds nothing, its free space where Linexin fits there, and otherwise no
/// place at all (the stand-in disk) until the person chooses one.
pub fn draft(disks: &[Disk], source: &str, firmware: Firmware) -> InstallConfig {
    let blank = InstallConfig {
        disk: no_disk(),
        target: Target::WholeDisk,
        storage: Storage::default(),
        bootloader: Bootloader::default(),
        desktop: Desktop::default(),
        software: Software::default(),
        region: Region::default(),
        account: Account {
            hostname: DEFAULT_HOSTNAME.into(),
            ..Account::default()
        },
        source: source.into(),
    };
    let Some(disk) = disks.iter().find(|disk| disk.blocked.is_none()) else {
        return blank;
    };
    match screens::target::first_place(&blank, disk, firmware) {
        Some(place) => screens::target::with_place(&blank, disk, &place),
        None => blank,
    }
}

/// The firmware an installation on `machine` is judged against: this
/// computer's, or — for the made-up one — UEFI, which every computer of the
/// last ten years starts with.
fn firmware_of(machine: &Machine) -> Firmware {
    match machine {
        Machine::Demo => Firmware::Uefi,
        Machine::Real { .. } => Firmware::detect(),
    }
}

/// A ring where a picture's pointer rests (`--shot --pointer`), since a
/// picture has no cursor of its own: a white bead in a dark ring, its
/// middle on the point.
fn pointer_ring(page: &mut Page, at: [f32; 2]) {
    let outer = page.scaled(30.0);
    let inner = outer * 0.6;
    let ui = page.ui();
    let ring = [at[0] - outer * 0.5, at[1] - outer * 0.5, outer, outer];
    ui.chip(ring, ui.tinted(Role::Glass, 0.95));
    let bead = [at[0] - inner * 0.5, at[1] - inner * 0.5, inner, inner];
    ui.chip(bead, ui.tinted(Role::Text, 1.0));
}

/// How a live system measures its disks (`State::measure`): the read-only
/// measure entry through polkit.
#[cfg(not(test))]
const MEASURE: fn(Vec<Disk>) -> Receiver<MeasureEvent> = crate::launch::measure;
/// In a test build, a measurement no test may use: a test standing on a live
/// session hands in its own.
#[cfg(test)]
const MEASURE: fn(Vec<Disk>) -> Receiver<MeasureEvent> = measured_in_a_test;

#[cfg(test)]
fn measured_in_a_test(_: Vec<Disk>) -> Receiver<MeasureEvent> {
    panic!("test build: a test standing on a live session hands in its own measurement (State::measure)")
}

/// What `--search` says to a scene with no search up.
const NO_SEARCH: &str =
    "--search goes with a scene whose search is up: language-search, timezone-search or keyboard-search";

impl State {
    /// The interface on a machine: the made-up one, or this one read as it
    /// is. `shot` is whether this state is drawn for a picture, which times
    /// it by the instant it is given rather than by the machine's clock.
    pub fn new(machine: Machine, shot: bool) -> Self {
        let (disks, disks_trouble) = match &machine {
            Machine::Demo => (demo::disks(), None),
            Machine::Real { .. } => match disks::discover() {
                Ok(disks) => (disks, None),
                Err(trouble) => (Vec::new(), Some(trouble)),
            },
        };
        let source = match &machine {
            Machine::Real { live: Some(live) } => live.image.to_string_lossy().into_owned(),
            _ => DEFAULT_SOURCE.to_string(),
        };
        if let Some(trouble) = &disks_trouble {
            eprintln!("lxb-installer: the disks could not be read: {trouble}");
        }
        // The lists people choose from are the live system's own (never the
        // image's or this machine's configuration); the made-up machine has a
        // few of its own, so a picture is the same on every computer.
        let (zones, layouts) = match &machine {
            Machine::Demo => (demo::zones(), demo::layouts()),
            Machine::Real { .. } => {
                let root = std::path::Path::new("/");
                let mut layouts = catalogs::keyboard_layouts(root);
                if layouts.is_empty() {
                    // No keyboard list at all: offer the one layout every
                    // system has rather than an empty column.
                    layouts.push(KeyboardLayout {
                        layout: "us".into(),
                        description: "English (US)".into(),
                        variants: Vec::new(),
                    });
                }
                (catalogs::zones(root), layouts)
            }
        };
        let offers = match &machine {
            Machine::Demo => demo::offers(),
            Machine::Real { .. } => catalogs::desktops(std::path::Path::new(&source)),
        };
        let mut state = Self {
            draft: draft(&disks, &source, firmware_of(&machine)),
            machine,
            repeat: Secret::default(),
            root_password: Secret::default(),
            root_repeat: Secret::default(),
            keyboard_test: String::new(),
            disks,
            disks_trouble,
            screen: Screen::Welcome,
            light: Light::default(),
            clock: Clock::new(shot),
            drawn_page: None,
            change: Change::default(),
            chips: panel::Chips::default(),
            laid_buttons: Vec::new(),
            low_end: false,
            pretend_low_end: false,
            pressing: Pressing::default(),
            board: None,
            shell_board: false,
            shell_typing: None,
            summon_again: false,
            board_over: None,
            board_before: false,
            put_away_from: None,
            pretend_board: false,
            shell_room: Carried::default(),
            shell_room_at_once: false,
            field_lit: None,
            note: None,
            install: Install::default(),
            progress: Progress::default(),
            events: None,
            rehearsal: None,
            views: screens::Views::default(),
            zones,
            layouts,
            chose_zone: false,
            chose_keyboard: false,
            offers,
            online: Asked::default(),
            packages: Asked::default(),
            pad: false,
            pad_pressed: false,
            pad_held: false,
            pretend_pad: false,
            pretend_pad_until_a_key: false,
            pretend_pointer: None,
            pointer_hovered: false,
            standing_in: None,
            quiet_in: None,
            typing_held: None,
            paged_by_click: None,
            keyed: false,
            question: None,
            answering: None,
            asked: None,
            plan_pages: Vec::new(),
            plan_read: false,
            list_disks: disks::discover,
            measuring: screens::partitions::Measuring::default(),
            measure: MEASURE,
            menu: None,
            menu_open: None,
            menu_anchor: None,
            speak: None,
            shell_language: None,
            sounds: Vec::new(),
            leaving: false,
            restarting: false,
            pretend_firmware: None,
        };
        // On a live system the language starts as the one the session was
        // started in, where it is one of those offered; the time zone and
        // keyboard start as its guesses, as they would after choosing it.
        // The made-up machine starts in US English; `--demo` and `--shot`
        // start it in the session's (`start_in`), never a test.
        let lang = match state.machine {
            Machine::Real { .. } => std::env::var("LANG").unwrap_or_default(),
            Machine::Demo => String::new(),
        };
        state.start_in(&lang);
        // The made-up machine has nothing to find out: it is online, and its
        // software is the invented list, known from the start — so a picture
        // of any later step shows what follows from them.
        if state.machine == Machine::Demo {
            state.packages.tell(Ok(demo::packages()));
            state.online.tell(true);
            screens::desktop::online_known(&mut state);
        }
        state.go(Screen::Welcome);
        state
    }

    /// The real interface: this machine, and — if an installation is already
    /// running because the window was closed in the middle of one — straight
    /// back to following it.
    pub fn real() -> Self {
        let live = crate::model::live::detect();
        let mut state = Self::new(Machine::Real { live }, false);
        if let Some(events) = crate::launch::running() {
            state.events = Some(events);
            state.go(Screen::Installing);
        }
        state
    }

    pub fn demo(shot: bool) -> Self {
        Self::new(Machine::Demo, shot)
    }

    /// Start in the language a session was started in (its `LANG`), where
    /// it is one of those offered — else in the one already in force: the
    /// places said in it, and the time zone and keyboard it guesses, as
    /// after choosing it on the Language step.
    pub fn start_in(&mut self, lang: &str) {
        if let Some(language) = screens::language::from_environment(lang) {
            self.draft.region.locale = language.locale.to_string();
        }
        self.name_the_places();
        screens::language::guess_the_rest(self);
    }

    /// For a picture: Welcome starts on the `nth` of its twenty greetings
    /// (from 1), whatever the language.
    pub fn greet_with(&mut self, nth: usize) -> Result<(), String> {
        let count = screens::welcome::GREETINGS.len();
        if !(1..=count).contains(&nth) {
            return Err(format!("--greeting wants 1 to {count}, not {nth}"));
        }
        screens::welcome::pin(self, nth - 1);
        Ok(())
    }

    /// For a picture: the search the scene raised holds `words` in place of
    /// what the scene typed into it.
    pub fn search_for(&mut self, words: &str) -> Result<(), String> {
        let typed = match self.screen {
            Screen::Language => &self.views.language.search,
            Screen::Timezone => &self.views.timezone.search,
            Screen::Keyboard => &self.views.keyboard.search,
            _ => return Err(NO_SEARCH.into()),
        };
        if !typed.is_open() {
            return Err(NO_SEARCH.into());
        }
        for _ in 0..typed.query().chars().count() {
            self.edit(FieldId::Search, &Edit::Rubbed);
        }
        self.edit(FieldId::Search, &Edit::Wrote(words.into()));
        Ok(())
    }

    /// Say Time zone's places and their countries in the language chosen,
    /// sorted as it sorts them (`model::places`), whenever it changes.
    pub fn name_the_places(&mut self) {
        let names = Names::of(&self.draft.region.locale);
        self.name_the_places_with(&names);
    }

    /// The same with names of a test's own.
    pub fn name_the_places_with(&mut self, names: &Names) {
        catalogs::name_zones(&mut self.zones, names);
    }

    /// Draw every frame as with a pad in hand: what `--shot --pad` takes its
    /// picture with.
    pub fn pretend_a_pad(&mut self) {
        self.pretend_pad = true;
        self.pad = true;
    }

    /// Answer a window as with a pad in hand until the first key arrives
    /// (`--demo --pad`): the pointer's clicks do not put the pad down, as
    /// the toolkit has every click do.
    pub fn pretend_a_pad_until_a_key(&mut self) {
        self.pretend_pad_until_a_key = true;
        self.pad = true;
    }

    /// Have a picture drawn as under LineXinBar with its board typing into
    /// the field the light stands in, as a pad's South there summons it
    /// (`--shot --shells-board`): called once the first frame has said
    /// which field that is. Nothing where the light is in none.
    pub fn pretend_the_shells_board(&mut self) {
        self.shell_board = true;
        self.pretend_board = true;
        self.pretend_a_pad();
        // Under LineXinBar the installer's own board is never raised: a
        // scene that raised it for a pad (`partitions-label`) types with the
        // shell's instead, into the same field.
        let field = self.board.take().map(|board| board.field);
        if let Some(field) = field.or(self.field_lit) {
            self.type_with_pad(field);
        }
    }

    /// Draw every frame as low-end hardware is drawn, nothing of the
    /// installer's own moving: what `--shot --low-end` takes its picture with.
    pub fn pretend_low_end(&mut self) {
        self.pretend_low_end = true;
        self.low_end = true;
    }

    /// Whether nothing of the installer's own moves on this frame.
    pub fn low_end(&self) -> bool {
        self.low_end
    }

    /// What a row of buttons the screen drew left on the frame: called by
    /// the screen as it draws one (`button::row`).
    pub fn laid_buttons(&mut self, laid: LaidRow) {
        self.laid_buttons.push(laid);
    }

    /// Draw every frame as with the pointer resting at `at`: what
    /// `--shot --pointer` takes its picture with. What it rests on is
    /// hovered once the first frame has drawn where everything is, so the
    /// settled frame after it shows that lit.
    pub fn pretend_a_pointer(&mut self, at: [f32; 2]) {
        self.pretend_pointer = Some(at);
        self.pointer_hovered = false;
    }

    /// Whether this machine can be installed onto from here at all: a demo
    /// always can (it only rehearses), the real machine only from a live
    /// session.
    pub fn can_install(&self) -> bool {
        match &self.machine {
            Machine::Demo => true,
            Machine::Real { live } => live.is_some(),
        }
    }

    // ---------------------------------------------------------------------
    // What a screen asks of the frame.

    /// Have the frame play `sound`.
    pub fn sound(&mut self, sound: Sound) {
        self.sounds.push(sound);
    }

    /// What the frame has been asked to play and has not played yet: what a
    /// test hears a press make.
    #[cfg(test)]
    pub fn heard(&self) -> &[Sound] {
        &self.sounds
    }

    /// Have the frame put `question` to the person.
    pub fn ask(&mut self, question: Question) {
        self.question = Some(question);
    }

    /// Have the frame close the window.
    pub fn leave(&mut self) {
        self.leaving = true;
    }

    /// Whether the window has been asked to close: what a test can read
    /// where the frame would have closed it.
    pub fn leaving(&self) -> bool {
        self.leaving
    }

    /// Have the frame restart the computer and close the window: Restart
    /// now, once Linexin is installed. Only asked for here and only done by
    /// the frame — never where a press is answered — so no test answering
    /// presses, and no picture, can ever restart anything.
    pub fn restart(&mut self) {
        self.restarting = true;
    }

    /// Whether Restart now has been asked for: what a test can read where
    /// the frame would have restarted.
    pub fn restarting(&self) -> bool {
        self.restarting
    }

    /// Have the frame raise `menu` over the screen.
    pub fn menu(&mut self, menu: Menu) {
        self.menu = Some(menu);
    }

    /// The menu a press asked for, taken where the frame would have raised
    /// it: what a test walking the menus as the harness does reads.
    #[cfg(test)]
    pub fn raised_menu(&mut self) -> Option<Menu> {
        self.menu.take()
    }

    /// The question a press asked, where the frame would have put it.
    #[cfg(test)]
    pub fn question_up(&self) -> Option<Question> {
        self.question
    }

    /// Say where the button this screen's menus grow out of was drawn on
    /// this frame. Called by the screen as it draws its Options button.
    pub fn anchor_menus(&mut self, rect: [f32; 4]) {
        self.menu_anchor = Some(rect);
    }

    /// Where this frame's menus would grow out of, if anything was named.
    pub fn menu_anchor(&self) -> Option<[f32; 4]> {
        self.menu_anchor
    }

    /// Have the installer speak from the catalog `tag` from this frame on.
    pub fn speak(&mut self, tag: &'static str) {
        self.speak = Some(tag);
    }

    /// The catalog the installer is about to switch to, if a press asked it
    /// to: what a test can read where the frame would have switched.
    pub fn speaking_next(&self) -> Option<&'static str> {
        self.speak
    }

    /// Have the live session's shell follow the language `tag` (one of
    /// LineXinBar's), from this frame on.
    pub fn tell_shell(&mut self, tag: &'static str) {
        self.shell_language = Some(tag);
    }

    /// The language the live session's shell is about to be told, if a
    /// press asked for it: what a test can read where the frame would have
    /// written it.
    pub fn telling_shell_next(&self) -> Option<&'static str> {
        self.shell_language
    }

    /// The firmware an installation is judged against (`firmware_of`).
    pub fn firmware(&self) -> Firmware {
        self.pretend_firmware
            .unwrap_or_else(|| firmware_of(&self.machine))
    }

    /// For a test standing the made-up machine off the demo: judge it as
    /// started by `firmware`, without reading this computer's.
    #[cfg(test)]
    pub fn pretend_firmware(&mut self, firmware: Firmware) {
        self.pretend_firmware = Some(firmware);
    }

    /// Say something in the foot of the panel until the screen changes.
    pub fn say(&mut self, text: impl Into<String>, role: Role) {
        self.note = Some(Note {
            text: text.into(),
            role,
        });
    }

    // ---------------------------------------------------------------------
    // Navigation.

    /// Arrive at `screen`: the light is put down there rather than flown in
    /// from the last screen, a note from the last screen is let go of, and
    /// the screen sets itself up.
    pub fn go(&mut self, screen: Screen) {
        self.screen = screen;
        self.note = None;
        self.board = None;
        self.shell_typing = None;
        self.summon_again = false;
        self.put_away_from = None;
        self.standing_in = None;
        self.quiet_in = None;
        self.field_lit = None;
        self.typing_held = None;
        self.keyed = false;
        self.light.settle();
        screens::enter(self);
    }

    /// Continue: on to the next question, with a press heard.
    pub fn forward(&mut self) {
        if let Some(next) = self.screen.next() {
            self.sound(Sound::Press);
            self.go(next);
        }
    }

    /// Back: to the last question, or — from Welcome — the question whether
    /// to leave. Nothing walks back out of an installation.
    pub fn back(&mut self) {
        if self.screen == Screen::Welcome {
            self.ask(Question::Quit);
        } else if let Some(previous) = self.screen.previous() {
            self.sound(Sound::Back);
            self.go(previous);
            // Back to the page Continue was pressed on: Partitions, while
            // its plan is the choice in use.
            if previous == Screen::Target && screens::partitions::in_use(self) {
                screens::partitions::came_back(self);
            }
        }
    }

    /// Install, pressed on the review: refused with the reason where the
    /// request cannot be installed, otherwise the last question before
    /// anything is written — the plan's paged one while Partitions' plan is
    /// the choice in use. On a live system the plan's disks are listed again
    /// first (`list_again`), so its question never says what a disk no longer
    /// holds.
    pub fn ask_to_install(&mut self) {
        let plan = screens::partitions::in_use(self);
        let checked = if plan && self.live() {
            self.list_again().and_then(|()| self.ready())
        } else {
            self.ready()
        };
        if let Err(words) = checked {
            self.sound(Sound::Error);
            self.say(words, Role::Danger);
            return;
        }
        self.ask(if plan {
            Question::PlanErase { page: 0 }
        } else {
            Question::Erase
        });
    }

    /// Whether this is a live session of the Linexin medium: the only
    /// machine an installation is started on.
    fn live(&self) -> bool {
        matches!(self.machine, Machine::Real { live: Some(_) })
    }

    /// Before the plan's question on a live system: the disks listed again
    /// as at the start — read-only, as the person (`list_disks`) — and handed
    /// to Partitions, which drops the plan where a disk it changes is not as
    /// it was (`partitions::disks_listed`). Refused in words where the disks
    /// cannot be read ("The disks of this computer could not be read.") or
    /// the plan was dropped ("A disk changed, so your partitions were
    /// cleared. Choose again.").
    fn list_again(&mut self) -> Result<(), String> {
        let disks = (self.list_disks)().map_err(|trouble| {
            eprintln!("lxb-installer: the disks could not be read again: {trouble}");
            i18n::text("disks-unreadable").to_string()
        })?;
        screens::partitions::disks_listed(self, disks);
        if !screens::partitions::in_use(self) {
            return Err(i18n::text("target-partitions-let-go").to_string());
        }
        Ok(())
    }

    /// Whether the request can be installed as it stands, and if not, why —
    /// in words. Off the demo: the region, the account and its password,
    /// then the place (`disks::validate_storage` of the draft) — or, while
    /// Partitions' plan is the choice in use, the plan (`plan_ready`) — and
    /// then no measure entry still running (`no_measure_entry`).
    pub fn ready(&self) -> Result<(), String> {
        if !self.can_install() {
            return Err(i18n::text("install-unavailable").to_string());
        }
        // A demo rehearses whatever it is given: there is nothing for the
        // checks to protect, and a picture of the review should not need a
        // typed-in account to show its Install button.
        if self.machine == Machine::Demo {
            return Ok(());
        }
        let checked = crate::model::config::validate_region(&self.draft.region)
            .and_then(|()| crate::model::config::validate_account(&self.draft.account))
            .and_then(|()| {
                crate::model::config::validate_password(&self.draft.account.password, &self.repeat)
            })
            .and_then(|()| screens::account::check(self).map_err(|(_, invalid)| invalid));
        checked.map_err(|invalid| words::invalid(invalid).to_string())?;
        if screens::partitions::in_use(self) {
            self.plan_ready()?;
        } else {
            disks::validate_storage(&self.draft, self.firmware())
                .map_err(|invalid| words::invalid(invalid).to_string())?;
        }
        self.no_measure_entry()
    }

    /// A plan under Partitions' judgement waits while its measurement is out
    /// (`partitions::Measuring`): what it reads judges the plan — "This
    /// computer's disks are still being read. Try again in a moment." — for
    /// no longer than `partitions::READING_PATIENCE`, after which what it
    /// had not answered is unread and the plan is judged by that.
    fn not_reading(&self) -> Result<(), String> {
        if self.measuring.reading() {
            return Err(i18n::text("install-reading").to_string());
        }
        Ok(())
    }

    /// **No installation is started beside a measure entry still running**
    /// (`partitions::Measuring::running`), whatever the choice in use: the
    /// measure entry holds the installer's lock while it reads, and a worker
    /// started now would be refused it and say, on Stopped with no way
    /// back, that another installation is running. Install says instead
    /// "This computer's disks are still being read. Try again in a moment.",
    /// the person still on Review — also once the measurement is no longer
    /// waited for, for as long as its measure entry still runs. `begin`
    /// asks this again, through `ready`, at the last moment.
    fn no_measure_entry(&self) -> Result<(), String> {
        if self.measuring.running() {
            return Err(i18n::text("install-reading").to_string());
        }
        Ok(())
    }

    /// [`ready`](Self::ready) for Partitions' plan in use, off the demo:
    /// everything [`plan_judged`](Self::plan_judged) holds it to, which
    /// takes a plan only where all it needs lies within one set of
    /// `layout::PROVEN`.
    fn plan_ready(&self) -> Result<(), String> {
        self.plan_judged()
    }

    /// Partitions' plan judged as Install judges it: the configuration it
    /// makes ([`config`](Self::config)), its easy fields its pins
    /// (`layout::pinned`; a swap file too large for `/` said as Where to
    /// install says it), the rule set's shape and its measurements
    /// (`partitions::missing`, in Partitions' words), everything it needs
    /// within one set QEMU proved (`layout::unproven`: a mix one scenario
    /// that passed ran whole, `layout::PROVEN` — said, where it is not, as
    /// Partitions says it, `partitions::unproven_words`), and no measurement
    /// still out.
    fn plan_judged(&self) -> Result<(), String> {
        let config = self.config()?;
        layout::pinned(&config).map_err(|unpinned| {
            let invalid = match unpinned {
                Unpinned::SwapTooLarge => Invalid::SwapTooLarge,
                _ => Invalid::LayoutRefused,
            };
            words::invalid(invalid).to_string()
        })?;
        if let Some(words) = screens::partitions::missing_words(self) {
            return Err(words);
        }
        let Target::Layout { disks: laid } = &config.target else {
            return Err(words::invalid(Invalid::LayoutRefused).to_string());
        };
        if let Some(unproven) = layout::unproven(laid, self.firmware()) {
            return Err(crate::ui::partitions::unproven_words(&unproven).to_string());
        }
        self.not_reading()
    }

    /// What the person has chosen, as a request carries it. **With no plan
    /// of Partitions' in use, the draft itself**, byte for byte. With a plan
    /// in use, the draft with its place replaced by the plan: `target` the
    /// plan as a layout (`partitions::layout`), `disk` and `storage` its pins
    /// (`layout::pins`: the disk that holds `/`, `/`'s filesystem, no separate
    /// `/home`, the swap file Options set or none beside a swap partition) —
    /// so the easy draft's first place, which Where to install started with,
    /// never stands under a plan — and restore points switched off beside a
    /// separate `/var` (Unit Q's decision 8), whose snapshots would not hold
    /// the package database they restore. Refused, in Partitions' words,
    /// where the plan cannot be laid out or has no `/` to pin.
    pub fn config(&self) -> Result<InstallConfig, String> {
        if !screens::partitions::in_use(self) {
            return Ok(self.draft.clone());
        }
        let plan = &self.views.partitions.plan;
        let words = |refusal| crate::ui::partitions::refusal_words(&refusal, plan);
        let laid = crate::ui::partitions::layout(plan, &self.disks).map_err(words)?;
        let pins = layout::pins(&laid, self.draft.storage.swap_mib).map_err(words)?;
        let mut config = self.draft.clone();
        config.disk = pins.disk;
        config.storage = Storage {
            filesystem: pins.filesystem,
            swap_mib: pins.swap_mib,
            home: None,
        };
        config.software.snapshots &= !layout::separate_var(&laid);
        config.target = Target::Layout { disks: laid };
        Ok(config)
    }

    /// What Desktop and Review judge what they say by: [`config`](Self::config)
    /// — the plan's while one is in use, so its start-up managers and restore
    /// points are the plan's — or the draft where the plan in use has no `/`
    /// to pin, which Continue on Partitions and on Where to install refuse
    /// before either is reached again.
    pub fn judged(&self) -> InstallConfig {
        self.config().unwrap_or_else(|_| self.draft.clone())
    }

    /// The request the worker would be handed: what the person has chosen
    /// ([`config`](Self::config)) and the consent list of what it erases —
    /// for Partitions' plan, every line of what it destroys, which its paged
    /// question shows whole. **The one place the interface builds a
    /// request**, and building one hands it to nothing: only `begin` does,
    /// past [`ready`](Self::ready) and, under a plan, past the last page of
    /// its question with its lines unchanged.
    pub fn request(&self) -> Result<Request, String> {
        let config = self.config()?;
        Ok(Request {
            consent: disks::consent(&config),
            config,
        })
    }

    /// The person said yes to `answered`, the question whose dialog they
    /// answered: start the installation — the rehearsal in a demo, the
    /// worker through polkit on a live system — and follow it.
    ///
    /// Only where what that question said is the very consent the request
    /// carries now ([`consent_read`](Self::consent_read)), in the demo as on
    /// a live system: the easy question only while no plan of Partitions' is
    /// in use; the plan's only while its plan still is, once its last page
    /// was put up and the lines its first page kept (`asked`) are still the
    /// request's consent. So a plan let go of between its question and the
    /// answer never lets "Erase and install" on its last page start the
    /// draft's place, whose own question was never put up. The demo's
    /// rehearsal needs no request: it says what a worker would say, from a
    /// script of its own (`demo::Rehearsal`). Only a live system builds a
    /// request and asks for a worker, and only through `launch::start`,
    /// which asks the live guard first.
    fn begin(&mut self, answered: Question) {
        // Checked again here, at the last moment, because the state may
        // have changed between the question and its answer.
        if self.ready().is_err() || !self.consent_read(answered) {
            self.sound(Sound::Error);
            return;
        }
        match self.machine {
            Machine::Demo => {
                self.install = Install::default();
                self.progress.restart();
                self.rehearsal = Some(Rehearsal::start(self.clock.now, &self.judged()));
            }
            Machine::Real { .. } => {
                let Ok(request) = self.request() else {
                    self.sound(Sound::Error);
                    return;
                };
                self.install = Install::default();
                self.progress.restart();
                self.events = Some(crate::launch::start(request));
            }
        }
        self.go(Screen::Installing);
    }

    /// Whether answering `answered` showed the person everything the
    /// request's consent holds now — decided by which question it was, never
    /// by whether a plan is in use alone, which may have changed since it
    /// was put up:
    /// - the easy question (`Erase`) says the draft's consent whole, so it
    ///   was read while no plan of Partitions' is in use; once one is, the
    ///   request is the plan's, which that question never said;
    /// - the plan's (`PlanErase`) says its plan's, so it was read only while
    ///   that plan is still in use, its last page was put up, and the lines
    ///   kept when its first was (`asked`) are the request's consent now,
    ///   line for line; once the plan is let go of, the request is the
    ///   draft's place, which no page of it said;
    /// - no other question begins anything.
    fn consent_read(&self, answered: Question) -> bool {
        let plan = screens::partitions::in_use(self);
        match answered {
            Question::Erase => !plan,
            Question::PlanErase { .. } => {
                let consent = self.request().ok().map(|request| request.consent);
                plan && self.plan_read && self.asked.is_some() && consent == self.asked
            }
            Question::Quit | Question::Pending => false,
        }
    }

    /// The question asked about the request now, in words: its title, the
    /// sentence under it, and its answers, the safe one first — two, or for
    /// the list of pending operations one, Close. The plan's question is
    /// said here whole, as one page with the last page's answers; the frame
    /// puts it up page by page ([`put_up_plan_page`](Self::put_up_plan_page)).
    pub fn question_words(&self, question: Question) -> (String, String, Vec<String>) {
        match question {
            Question::Quit => (
                i18n::text("quit-title").to_string(),
                i18n::text("quit-body").to_string(),
                vec![
                    i18n::text("quit-stay").to_string(),
                    i18n::text("quit-leave").to_string(),
                ],
            ),
            Question::Pending => (
                i18n::text("partitions-pending-title").to_string(),
                screens::partitions::pending_list(self, usize::MAX, f32::MAX, |_| 1.0),
                vec![i18n::text("button-close").to_string()],
            ),
            Question::Erase if screens::partitions::in_use(self) => {
                self.question_words(Question::PlanErase { page: 0 })
            }
            Question::Erase => {
                let (title, go) = self.erase_title_and_go();
                (
                    title,
                    self.erase_words(),
                    vec![i18n::text("erase-back").to_string(), go],
                )
            }
            Question::PlanErase { .. } => {
                let (title, go) = self.erase_title_and_go();
                let pages = screens::partitions::plan_pages(self, usize::MAX, f32::MAX, |_| 1.0);
                (
                    title,
                    pages.join("\n"),
                    vec![i18n::text("erase-back").to_string(), go],
                )
            }
        }
    }

    /// The last question's title and its answer that goes on: to erase and
    /// install — or, where nothing is erased (free space, a plan that only
    /// makes partitions in unallocated space), to install, rather than
    /// threatening an erasure that will not happen.
    fn erase_title_and_go(&self) -> (String, String) {
        let (title, go) = if self.erases() {
            (i18n::text("erase-title"), i18n::text("erase-confirm"))
        } else {
            (i18n::text("install-title"), i18n::text("button-install"))
        };
        (title.to_string(), go.to_string())
    }

    /// The question the frame puts up for `question`: itself — but the last
    /// question asked while Partitions' plan is the choice in use is the
    /// plan's own, page by page from its first, never the easy place's.
    fn put_as(&self, question: Question) -> Question {
        match question {
            Question::Erase if screens::partitions::in_use(self) => Question::PlanErase { page: 0 },
            other => other,
        }
    }

    /// The question the frame puts up, in words fitted to its dialog: `most`
    /// lines of its body `room` wide, as `measure` measures a line — the
    /// list of pending operations as much of it as they hold, the plan's
    /// question its page; anything else as [`question_words`](Self::question_words).
    fn dialog(
        &mut self,
        question: Question,
        most: usize,
        room: f32,
        measure: impl FnMut(&str) -> f32,
    ) -> (String, String, Vec<String>) {
        match question {
            Question::Pending => (
                i18n::text("partitions-pending-title").to_string(),
                screens::partitions::pending_list(self, most, room, measure),
                vec![i18n::text("button-close").to_string()],
            ),
            Question::PlanErase { page } => self.put_up_plan_page(page, most, room, measure),
            Question::Quit | Question::Erase => self.question_words(question),
        }
    }

    /// Put up page `page` of the plan's question (from 0; one past the last
    /// is the last), as `most` lines of a dialog `room` wide hold it
    /// (`measure`): its title, its page and its answers — "Go back" and
    /// Next, or on the last page "Go back" and "Erase and install". As its
    /// first page is put up, its pages are made for the window as it is
    /// then (`partitions::plan_pages`), and the lines of what the plan
    /// destroys are kept (`asked`); once its last page is up, it has been
    /// read whole (`begin` goes on only then).
    pub fn put_up_plan_page(
        &mut self,
        page: usize,
        most: usize,
        room: f32,
        measure: impl FnMut(&str) -> f32,
    ) -> (String, String, Vec<String>) {
        if page == 0 || self.plan_pages.is_empty() {
            self.plan_pages = screens::partitions::plan_pages(self, most, room, measure);
            self.plan_read = false;
            self.asked = (page == 0).then(|| {
                let destroys = screens::partitions::destroys(self);
                destroys.iter().map(layout::Destroy::line).collect()
            });
        }
        let last = self.plan_pages.len().saturating_sub(1);
        let at = page.min(last);
        if at == last {
            self.plan_read = true;
        }
        let body = self.plan_pages.get(at).cloned().unwrap_or_default();
        let (title, go) = self.erase_title_and_go();
        let on = if at < last {
            i18n::text("button-next").to_string()
        } else {
            go
        };
        (title, body, vec![i18n::text("erase-back").to_string(), on])
    }

    /// Whether the page of the plan's question up now is not its last.
    fn more_pages_after(&self, page: usize) -> bool {
        page < self.plan_pages.len().saturating_sub(1)
    }

    /// The plan's question let go of: its pages, what they said and
    /// whether they were read are put up afresh the next time.
    fn forget_the_plan_question(&mut self) {
        self.asked = None;
        self.plan_pages.clear();
        self.plan_read = false;
    }

    /// What the last question says will happen, in plain words: one
    /// sentence for each thing the consent list holds — the very list the
    /// worker checks against what it is about to erase — and then whether
    /// it can be undone. Every name in it is held together on one line
    /// (`text::unbroken`): the dialog keeps the lines it is given, and a
    /// disk's name broken after its model read as two things.
    fn erase_words(&self) -> String {
        let draft = &self.draft;
        let disk = text::unbroken(&words::disk_name(&draft.disk));
        let consent = disks::consent(draft);
        let mut said = Vec::new();
        if matches!(draft.target, Target::FreeSpace { .. }) {
            said.push(crate::message!("erase-target-free", "disk" => disk.clone()));
        }
        for path in &consent {
            let home = draft
                .storage
                .home
                .as_ref()
                .is_some_and(|home| home.path == *path);
            let sentence = if *path == draft.disk.path {
                crate::message!("erase-target-disk", "disk" => disk.clone())
            } else if home {
                // Wherever it is: on another disk, that disk is named too.
                let part = draft
                    .storage
                    .home
                    .as_ref()
                    .and_then(|home| disks::home_partition(draft, home))
                    .map(|(on, part)| {
                        let name = text::unbroken(&screens::review::part_name(part));
                        if on.path == draft.disk.path {
                            name
                        } else {
                            crate::message!(
                                "part-on-disk",
                                "part" => name,
                                "disk" => text::unbroken(&words::disk_called(on))
                            )
                        }
                    })
                    .unwrap_or_default();
                crate::message!("erase-home", "part" => part)
            } else {
                // The partition with its own size and the disk by its name
                // alone: "“Old Linux” · 32 GB on Virtual disk", never the
                // disk's size where it would read as the partition's.
                let (part, size) = draft
                    .disk
                    .partition(path)
                    .map(|part| (screens::review::part_name(part), words::size(part.size)))
                    .unwrap_or_default();
                crate::message!(
                    "erase-target-part",
                    "part" => text::unbroken(&part),
                    "size" => text::unbroken(&size),
                    "disk" => text::unbroken(&words::disk_called(&draft.disk))
                )
            };
            said.push(sentence);
        }
        said.push(if consent.is_empty() {
            i18n::text("erase-nothing").to_string()
        } else {
            i18n::text("erase-final").to_string()
        });
        said.join(" ")
    }

    /// Whether installing destroys anything: what the consent list holds —
    /// or, while Partitions' plan is the choice in use, what the plan
    /// deletes, formats, shrinks or writes a new table over
    /// (`partitions::destroys`, its request's consent).
    fn erases(&self) -> bool {
        if screens::partitions::in_use(self) {
            !screens::partitions::destroys(self).is_empty()
        } else {
            !disks::consent(&self.draft).is_empty()
        }
    }

    /// Shorter words for the title of `question`, where it has any: the verb
    /// alone, the body and the answers saying the rest.
    fn shorter_title(&self, question: Question) -> Option<String> {
        match question {
            Question::Erase | Question::PlanErase { .. } if self.erases() => {
                Some(i18n::text("erase-title-short").to_string())
            }
            Question::Pending => Some(i18n::text("partitions-pending-short").to_string()),
            _ => None,
        }
    }

    /// Shorter words for the second answer of `question`, where it has any:
    /// the verb alone, the title over it saying the rest — the erase answer,
    /// on the plan's question only on its last page (Next is short).
    fn shorter_answer(&self, question: Question) -> Option<String> {
        match question {
            Question::Erase if self.erases() => Some(i18n::text("erase-confirm-short").to_string()),
            Question::PlanErase { page } if self.erases() && !self.more_pages_after(page) => {
                Some(i18n::text("erase-confirm-short").to_string())
            }
            _ => None,
        }
    }

    fn answer(&mut self, question: Question, answer: usize) {
        match (question, answer) {
            (Question::Quit, 1) => self.leave(),
            (Question::Erase, 1) => self.begin(question),
            // Next, to the page after; on the last page, erase and install
            // what the plan's pages said, if that is still what would be.
            (Question::PlanErase { page }, 1) if self.more_pages_after(page) => {
                self.ask(Question::PlanErase { page: page + 1 });
            }
            (Question::PlanErase { .. }, 1) => {
                self.begin(question);
                self.forget_the_plan_question();
            }
            (Question::PlanErase { .. }, _) => self.forget_the_plan_question(),
            _ => {}
        }
    }

    // ---------------------------------------------------------------------
    // Text.

    /// The field text is going into now: the installer's board's, the one a
    /// pad is typing into with the shell's board, or the one the light stands
    /// in with a keyboard in hand, or none.
    pub fn typing_into(&self) -> Option<FieldId> {
        self.board
            .as_ref()
            .map(|board| board.field)
            .or(self.shell_typing)
            .or(self.standing_in)
    }

    /// Have every window from now on type with LineXinBar's own board when
    /// a pad is in hand (`board::under_linexinbar`), or with the
    /// installer's. Asked once, by the window, as it opens.
    pub fn type_with_the_shells_board(&mut self, shell: bool) {
        self.shell_board = shell;
    }

    /// Whether a pad types with LineXinBar's own board.
    pub fn shells_board(&self) -> bool {
        self.shell_board
    }

    /// The field a pad is typing into with the shell's board, if one is.
    pub fn typing_with_the_shell(&self) -> Option<FieldId> {
        self.shell_typing
    }

    /// Say that the light stands in `field` on this frame. Called by the
    /// screen as it draws the field with the light in it: with a keyboard in
    /// hand the field takes what is typed (`standing_in`); with a pad, it
    /// takes it only once Accept has been pressed on it.
    pub fn in_field(&mut self, field: FieldId) {
        self.field_lit = Some(field);
        if self.pad {
            self.quiet_in = Some(field);
        } else {
            self.standing_in = Some(field);
        }
    }

    /// Let go of the pad held for the shell's board (`pad_held`) where this
    /// frame says the hands are elsewhere: a pad's own press (`pressed`),
    /// or a key (`keyed`) while nothing is being typed with the shell's
    /// board — none summoned, or none standing over the window before the
    /// key came (put away, or never raised where the shell saw a keyboard):
    /// a keyboard of the person's own, a letter included, which the field
    /// the light stands in takes quietly (`quiet_in`). The key that arrives
    /// with the board going — Start's Enter, the person's own key the board
    /// passes on as it goes — is still the board's. A click does not: the
    /// shell itself goes on counting a controller in hand through a click,
    /// and would raise its board over a field that took text.
    fn hold_the_pad(&mut self, pressed: bool, keyed: bool) {
        let typing = self.shell_typing.is_some() && self.board_before;
        if self.pad_held && (pressed || (keyed && !typing)) {
            self.pad_held = false;
        }
        self.pad = self.pad_pressed || self.pad_held;
    }

    /// As a frame begins: what the toolkit says is in hand
    /// (`Page::pad_in_hand`), with the pad held for the shell's board.
    fn reach(&mut self, pad_in_hand: bool) {
        self.pad_pressed = pad_in_hand || self.pretend_pad || self.pretend_pad_until_a_key;
        self.pad = self.pad_pressed || self.pad_held;
    }

    /// As a frame begins: where the toolkit says LineXinBar's board stands
    /// over the window (`Page::shells_board`) — for a picture pretending it,
    /// over the field a pad types into with it — on a window `width` ×
    /// `height`. A board risen lets You's form scroll the field in hand above
    /// it, wherever the pointer that lit the field held the form; a board put
    /// away while a field takes text leaves the field to ask for it again.
    fn told(&mut self, board: Option<[f32; 4]>, width: f32, height: f32) {
        self.board_before = self.board_over.is_some();
        self.board_over = if self.pretend_board {
            self.shell_typing.map(|_| shells_board::rect(width, height))
        } else {
            board
        };
        match (self.board_before, self.board_over.is_some()) {
            (false, true) => {
                self.put_away_from = None;
                screens::account::shells_board_rose(self);
            }
            (true, false) => self.put_away_from = self.typing_into(),
            _ => {}
        }
    }

    /// What arrived on this frame — typing (`typed`), presses of any kind
    /// (`pressed`) — and what the toolkit says is in hand after it.
    fn arrived(&mut self, pad_in_hand: bool, typed: bool, pressed: bool) {
        // A window pretending a pad (`--demo --pad`) has none to read: what
        // arrives as a press or as typing is a key, which puts it down.
        let keyed = typed || pressed;
        if self.pretend_pad_until_a_key && keyed {
            self.pretend_pad_until_a_key = false;
            self.pad_pressed = pad_in_hand || self.pretend_pad;
        }
        // A pad's own press is a press arriving with a pad the last thing
        // reached for — not a pad merely not put down yet. The toolkit says
        // a pad is in hand until the first key, and the shell's board, which
        // South summons, holds the pad and types nothing for a while: taken
        // for a press, that let the hold go on the very next frame, and with
        // it went everything the hold is for.
        self.hold_the_pad(pad_in_hand && pressed, keyed);
    }

    /// Accept on `field` with a pad in hand, which has no keys: a board to
    /// type with. Under LineXinBar the shell's own — the field takes text,
    /// and the shell raises its board over it, as it does for every
    /// application on it — and, pressed on a field already taking text
    /// (its board put away, or a click's field with none raised), the
    /// shell's board again. Anywhere else, the installer's own.
    pub fn type_with_pad(&mut self, field: FieldId) {
        if !self.shell_board {
            self.open_board(field);
            return;
        }
        if self.typing_into() == Some(field) {
            self.summon_again = true;
        }
        self.shell_typing = Some(field);
        self.pad_held = true;
        self.sound(Sound::Press);
    }

    /// A click or a tap on `field`, which puts the light in it. On the field
    /// whose board was put away while it took text, it asks the shell for
    /// its board again, as a pad's South does there: the field lets go of
    /// its text for one frame, and the shell offers its board to a field
    /// taking it anew. A click on a field the board never rose over — a
    /// keyboard in the shell's hands — asks nothing.
    pub fn clicked_field(&mut self, field: FieldId) {
        if self.board_over.is_none()
            && self.put_away_from == Some(field)
            && self.typing_into() == Some(field)
        {
            self.summon_again = true;
        }
    }

    /// Text typed on a keyboard: into the field it is going into, and with
    /// none, dropped rather than left to arrive later somewhere else. Typed
    /// into the field the light stands in, it holds the light there.
    pub fn type_keys(&mut self, typed: Vec<Edit>) {
        let Some(field) = self.typing_into().or(self.quiet_in) else {
            return;
        };
        if !typed.is_empty() && self.board.is_none() {
            self.typing_held = Some(field);
        }
        for edit in typed {
            self.edit(field, &edit);
        }
    }

    /// Once the screen has said which field the light stands in: one a key
    /// or a pad brought it into, or left it in, is held from the pointer,
    /// and a held field the light has left is let go.
    fn hold_typing(&mut self) {
        if std::mem::take(&mut self.keyed) {
            self.typing_held = self.standing_in;
        } else if self.typing_held != self.standing_in {
            self.typing_held = None;
        }
    }

    /// Once the screen has said which field the light stands in: the field
    /// the shell's board was typing into, if the light has left it, lets go
    /// of its text, and the shell puts its board away.
    fn let_go_of_a_field_left(&mut self) {
        if self.shell_typing.is_some() && self.shell_typing != self.field_lit {
            self.shell_typing = None;
        }
    }

    /// Apply one edit to the value behind a field.
    pub fn edit(&mut self, field: FieldId, edit: &Edit) {
        let account = &mut self.draft.account;
        match field {
            FieldId::FullName => field::apply(&mut account.full_name, edit),
            FieldId::Username => field::apply(&mut account.username, edit),
            FieldId::Password => field::apply(&mut account.password, edit),
            FieldId::Repeat => field::apply(&mut self.repeat, edit),
            FieldId::Hostname => field::apply(&mut account.hostname, edit),
            FieldId::RootPassword => field::apply(&mut self.root_password, edit),
            FieldId::RootRepeat => field::apply(&mut self.root_repeat, edit),
            FieldId::KeyboardTest => field::apply(&mut self.keyboard_test, edit),
            FieldId::Search => return screens::searched(self, edit),
            FieldId::PartLabel | FieldId::MountPath | FieldId::PartSize => {
                return screens::partitions::edited(self, field, edit)
            }
        }
        screens::account::edited(self, field);
    }

    /// Open the board over the screen, typing into `field`. The light stays
    /// on the field until the board's first letter has risen into sight and
    /// then goes over to it (`Board::draw`), rather than flying from the
    /// field down to a key still under the foot of the buttons, or being put
    /// on that key while nothing of it shows.
    pub fn open_board(&mut self, field: FieldId) {
        self.board = Some(Board::open(field, &self.draft.region.keyboard.layout));
        self.sound(Sound::Press);
    }

    fn stroke(&mut self, stroke: Stroke) {
        let Some(field) = self.typing_into() else {
            return;
        };
        match stroke {
            Stroke::Nothing => {}
            Stroke::Moved => self.sound(Sound::Move),
            Stroke::Changed => self.sound(Sound::Key),
            Stroke::Typed(text) => {
                self.edit(field, &Edit::Wrote(text));
                self.sound(Sound::Key);
            }
            Stroke::Rubbed => {
                self.edit(field, &Edit::Rubbed);
                self.sound(Sound::Key);
            }
            Stroke::Done => {
                self.board = None;
                self.sound(Sound::Press);
                screens::typed(self, field);
            }
            Stroke::Closed => {
                self.board = None;
                self.sound(Sound::Back);
            }
        }
    }

    // ---------------------------------------------------------------------
    // The installation.

    /// Fold in whatever the worker or the rehearsal has said since the last
    /// frame, and move to Done or Stopped when it ends.
    fn follow(&mut self) {
        let mut said = Vec::new();
        if let Some(rehearsal) = self.rehearsal.as_mut() {
            said.extend(rehearsal.poll(self.clock.now));
        }
        if let Some(events) = &self.events {
            loop {
                match events.try_recv() {
                    Ok(event) => said.push(event),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // The worker went without a last word. Said once, as a
                        // failure, rather than an installation followed for
                        // ever. Its last word may have come in this same look,
                        // just before the end of its channel, and not be
                        // folded in yet: the worker writes Finished and exits
                        // at once, and a frame can be a long time apart from
                        // the next (the first QEMU installation, which
                        // finished, was shown as stopped for that reason).
                        self.events = None;
                        let ended = self.install.ending.is_some()
                            || said.iter().any(crate::launch::terminal);
                        if !ended {
                            said.push(Event::Failed {
                                step: None,
                                failure: Failure::Internal,
                                detail: "the installation stopped without saying why".into(),
                            });
                        }
                        break;
                    }
                }
            }
        }
        for event in said {
            self.install.take(event);
        }
        self.progress.advance(self.install.through(), self.clock.dt);
        if self.screen == Screen::Installing {
            match self.install.ending {
                Some(Ending::Finished) => {
                    self.rehearsal = None;
                    self.events = None;
                    self.sound(Sound::Notify);
                    self.go(Screen::Done);
                }
                Some(Ending::Failed { .. }) => {
                    self.rehearsal = None;
                    self.events = None;
                    self.sound(Sound::Error);
                    self.go(Screen::Stopped);
                }
                None => {}
            }
        }
    }

    // ---------------------------------------------------------------------
    // The frame.

    /// One press, from a pad, a key, a wheel or a pair of the legend.
    pub fn act(&mut self, action: Action, scroll: Option<Scroll>) {
        self.keyed = true;
        if let Some(board) = self.board.as_mut() {
            let stroke = board.act(action);
            self.stroke(stroke);
            return;
        }
        // While the shell's board is up it holds the pad, and only its keys
        // reach the window; a press of the pad's own reaching it means the
        // board was put away (Back on it).
        if let Some(field) = self.shell_typing {
            match action {
                // South on the field whose board was put away: the board
                // again.
                Action::Accept if self.pad_pressed => return self.type_with_pad(field),
                // Enter — what the shell's board sends as Start puts it away:
                // the typing is done, and the screen answers Enter as it
                // answers it from a keyboard (on to the next field, the light
                // down to what a search found), the pad still held.
                Action::Accept => {
                    self.shell_typing = None;
                    let pad = std::mem::replace(&mut self.pad, false);
                    screens::act(self, action, scroll);
                    self.pad = pad;
                    return;
                }
                // Back or Start of the pad's: the typing is done, and the
                // press is the screen's.
                Action::Back | Action::Submit => self.shell_typing = None,
                _ => {}
            }
        }
        screens::act(self, action, scroll);
    }

    /// The pointer moved onto the control drawn with the number `id`: the
    /// light goes there — a key while the board is up, which has the light,
    /// else the screen's control, unless a field is being typed in, from a
    /// keyboard (`typing_held`) or with the shell's board (`shell_typing`,
    /// or the board standing over the window, however its field was chosen).
    /// The shell's board is no part of the page: the pointer merely crossing
    /// the page on its way, as the installer's board keeps it, must not take
    /// the light out of the field and with it the board away. A click still
    /// moves the light.
    pub fn hover(&mut self, id: u32) {
        match self.board.as_mut() {
            Some(board) => {
                board.point_at(id);
            }
            None if self.typing_held.is_some()
                || self.shell_typing.is_some()
                || self.board_over.is_some() => {}
            None => screens::hover(self, id),
        }
    }

    /// What the legend says now: the board's while it is open, the screen's
    /// otherwise.
    pub fn hints(&self) -> Vec<Hint> {
        if self.board.is_some() {
            Board::hints()
        } else {
            screens::hints(self)
        }
    }

    /// The line in the foot of the panel: a refusal first, then what the
    /// screen has to say, then — in a demo — that nothing is real.
    pub fn foot_note(&self) -> Option<Note> {
        self.note
            .clone()
            .or_else(|| screens::note(self))
            .or_else(|| {
                (self.machine == Machine::Demo).then(|| Note {
                    text: i18n::text("demo-note").to_string(),
                    role: Role::Text,
                })
            })
    }

    /// What the session is kept from while this screen is up.
    ///
    /// An installation is work a sleep would stop halfway, and nobody
    /// touching the machine while it runs is nobody having left: the session
    /// is kept from sleeping, and its screen from dimming and going dark,
    /// until it ends. Only sleep was held once, and the screen dimmed some two
    /// minutes into every installation in QEMU, the bar and its step out of
    /// sight while the work went on.
    pub fn kept_awake(&self) -> Hold {
        if self.screen == Screen::Installing {
            Hold::SCREEN_AND_SLEEP
        } else {
            Hold::NOTHING
        }
    }

    /// Draw one whole frame.
    pub fn frame(&mut self, page: &mut Page) {
        if page.low_end() {
            self.clock.tick_held(page.seconds());
        } else {
            self.clock.tick(page.seconds());
        }
        self.low_end = page.low_end() || self.pretend_low_end;
        let dt = self.clock.dt;
        self.pressing.advance(dt);
        self.change.leaving_pressing().advance(dt);
        self.reach(page.pad_in_hand());
        self.told(page.shells_board(), page.width(), page.height());
        self.follow();
        screens::desktop::poll(self);
        screens::partitions::poll(self);
        screens::keyboard::poll(self, page);
        // A still window (low-end mode draws once a second) still takes in
        // a measurement's answer as it comes.
        if self.measuring.reading() {
            page.redraw_within(Duration::from_millis(250));
        }

        // In low-end mode (where a still window is drawn once a second) an
        // installation's bar is drawn four times a second.
        let working = self.screen == Screen::Installing;
        page.keep_awake(self.kept_awake());
        if working {
            page.redraw_within(Duration::from_millis(250));
        }

        if let Some(answer) = page.answered() {
            if let Some(question) = self.answering.take() {
                self.answer(question, answer);
            }
        }
        if let Some(row) = page.chose() {
            if let Some((screen, id)) = self.menu_open.take() {
                if screen == self.screen {
                    screens::chose(self, id, row);
                }
            }
        }
        // Hovering is selecting, as in LineXinBar: where the pointer moved
        // onto a control since the last frame, the light goes there,
        // silently. Read off the frame it moved over, before this frame's
        // presses; a pointer resting still is told nothing, so a light a key
        // or a pad moved stays where they put it.
        if let Some(id) = page.hovered() {
            self.hover(id);
        }
        let typed: Vec<Edit> = page
            .typed()
            .map(|one| match one {
                lxb_app::Typed::Wrote(text) => Edit::Wrote(text),
                lxb_app::Typed::Rubbed => Edit::Rubbed,
            })
            .collect();
        let actions: Vec<Action> = page.actions().collect();
        let scrolls: Vec<Scroll> = page.scrolls().collect();
        self.arrived(page.pad_in_hand(), !typed.is_empty(), !actions.is_empty());
        self.type_keys(typed);
        self.answer_presses(actions, &scrolls);
        if let Some(tag) = self.speak.take() {
            i18n::set_language(tag);
        }
        if let Some(tag) = self.shell_language.take() {
            // Only a live session's shell follows the installer: a demo, a
            // picture and the installer opened on an installed system leave
            // the account's settings alone.
            if let Machine::Real { live: Some(_) } = &self.machine {
                screens::language::tell_the_live_shell(tag);
            }
        }

        // Drawn before a menu the presses asked for is raised, so the menu
        // grows out of its button where this frame put it. The toolkit draws
        // its menus and dialogs after the page, so one raised now is on this
        // frame all the same. What the guesses were seen as before it is
        // kept for the pointer's presses, answered after it.
        let seen = self.seen();
        self.draw(page);
        // A menu or a question standing over the page takes the pointer: the
        // toolkit tells a page nothing while one is up, so nothing of the
        // page is hovered, and the ring, which the page would draw stepped
        // back with it, is not drawn either.
        let covered = self.menu.is_some()
            || self.menu_open.is_some()
            || self.question.is_some()
            || self.answering.is_some();
        if let Some(at) = self.pretend_pointer.filter(|_| !covered) {
            if !std::mem::replace(&mut self.pointer_hovered, true) {
                if let Spot::Control(id) = page.at(at[0], at[1]) {
                    self.hover(id);
                }
            }
            pointer_ring(page, at);
        }
        let raised = self.raise(page);
        if !raised {
            self.as_before_drawing(seen, |state| state.pressed(page));
            // A pointer can raise a question too; it is asked on this frame,
            // and its dialog is drawn from the next.
            self.raise(page);
        }

        if self.restarting {
            self.restarting = false;
            if self.reboot() {
                page.quit();
            }
        }
        for sound in self.sounds.drain(..) {
            page.play(sound);
        }
        if self.leaving {
            page.quit();
        }
    }

    /// Answer the presses that reached this frame — a key's, a pad's, a
    /// wheel's — in the order they came. All of them are answered at the one
    /// instant of this frame's clock and before the frame is drawn, so a
    /// press that changes what is on the screen is not seen by the next one
    /// in the same frame: two of them are no time apart, and any of them may
    /// have come in at any moment since the frame before (what Time zone's
    /// and Keyboard's Continue, and taking the guess in their second column,
    /// are held back for until a frame has counted it seen: `trail::Seen`).
    fn answer_presses(&mut self, actions: Vec<Action>, scrolls: &[Scroll]) {
        for (index, action) in actions.into_iter().enumerate() {
            if action == Action::Accept {
                self.pressing.press();
            }
            let scroll = scrolls.iter().find(|scroll| scroll.covers(index)).copied();
            self.act(action, scroll);
            if self.question.is_some() || self.menu.is_some() {
                // Presses already queued for this frame did not pass through
                // the dialog, which did not exist when they arrived. Nothing
                // behind a question just raised should move.
                break;
            }
        }
    }

    /// Time zone's and Keyboard's [`Seen`] as they stand: kept as a frame is
    /// drawn, for the pointer's presses answered after it
    /// ([`State::as_before_drawing`]).
    fn seen(&self) -> [Seen; 2] {
        [self.views.timezone.seen, self.views.keyboard.seen]
    }

    /// Answer the pointer's presses with `press`, finding Time zone's and
    /// Keyboard's guess seen only where it was before this frame was drawn
    /// (`before`, [`State::seen`]). They are asked after the drawing, of the
    /// spots it wrote, but came in, as a key does, at any moment since the
    /// frame before; and the frame that has just counted a guess seen
    /// (`trail::Seen`) has not reached the display. A guess this frame
    /// counted stands for them as it stood before it — dwelling, so a press
    /// of Continue or on the guess does nothing — and is seen again after
    /// them, whatever they did to its dwell: the frame did keep it on the
    /// screen for it, and only a language chosen forgets it.
    fn as_before_drawing(&mut self, before: [Seen; 2], press: impl FnOnce(&mut Self)) {
        fn seens(state: &mut State) -> [&mut Seen; 2] {
            [
                &mut state.views.timezone.seen,
                &mut state.views.keyboard.seen,
            ]
        }
        let mut counted = [false; 2];
        for ((seen, before), counted) in seens(self).into_iter().zip(before).zip(&mut counted) {
            if seen.seen() && !before.seen() {
                *seen = before;
                *counted = true;
            }
        }
        press(self);
        for (seen, counted) in seens(self).into_iter().zip(counted) {
            if counted {
                seen.see();
            }
        }
    }

    /// One frame as [`State::frame`] answers it, with no window to draw in:
    /// the clock moved on to `now` (the instant itself, as a picture's clock
    /// takes it, low-end or not), the presses that reached the frame
    /// answered together (`answer_presses`), the screen drawn as far as it
    /// counts what was on it (`screens::drawn`, which `State::draw` calls
    /// after drawing it), and then the pointer's `clicks`, which the frame
    /// asks after drawing, each through `State::click` — finding the guesses
    /// seen as they were before the drawing (`as_before_drawing`).
    #[cfg(test)]
    pub fn frame_at(&mut self, now: f32, presses: &[Action], clicks: &[fn(&mut State)]) {
        self.clock.tick(now);
        self.answer_presses(presses.to_vec(), &[]);
        let seen = self.seen();
        let _ = screens::drawn(self);
        self.as_before_drawing(seen, |state| {
            for click in clicks {
                state.click(*click);
            }
        });
    }

    /// Frames with nothing pressed in them, a sixtieth of a second apart, up
    /// to (and not at) `to`: the screen kept on the display as a window
    /// keeps it.
    #[cfg(test)]
    pub fn frames_until(&mut self, to: f32) {
        let mut now = self.clock.now;
        while now + super::motion::FRAME < to {
            now += super::motion::FRAME;
            self.frame_at(now, &[], &[]);
        }
    }

    /// `presses`, each in a frame of its own `apart` seconds after the one
    /// before, the frames between them drawn: keys pressed as a person, or
    /// the e2e harness, presses them one at a time.
    #[cfg(test)]
    pub fn press_apart(&mut self, presses: &[Action], apart: f32) {
        for press in presses {
            let at = self.clock.now + apart;
            self.frames_until(at);
            self.frame_at(at, &[*press], &[]);
        }
    }

    /// Restart the computer, as the person, on a live system only — logind
    /// lets the active session's own user do it without a question. The made-up
    /// machine just closes. Answers whether the window can close; where the
    /// computer could not be asked, it says so and stays.
    fn reboot(&mut self) -> bool {
        let Machine::Real { live: Some(_) } = &self.machine else {
            return true;
        };
        match std::process::Command::new("/usr/bin/systemctl")
            .arg("reboot")
            .stdin(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => true,
            Err(trouble) => {
                eprintln!("lxb-installer: the computer could not be restarted: {trouble}");
                self.sound(Sound::Error);
                self.say(i18n::text("done-restart-failed"), Role::Danger);
                false
            }
        }
    }

    /// Put a question a press asked for to the person, or raise the menu it
    /// asked for. Answers whether either was raised.
    fn raise(&mut self, page: &mut Page) -> bool {
        let scale = page.scaled(1.0);
        if let Some(menu) = self.menu.take() {
            // Fitted to the widest menu the window allows: only a name the
            // machine gave is ever cut, and the toolkit would cut anything
            // else with an ellipsis of its own.
            let room = menus::MenuRoom::of(page.width(), scale);
            let ui = page.ui();
            let commands: Vec<String> = menu
                .commands
                .iter()
                .enumerate()
                .map(|(at, whole)| {
                    let room = if menu.marked == Some(at) {
                        room.marked
                    } else {
                        room.label
                    };
                    let said = match menu.shorter.get(at).and_then(Option::as_deref) {
                        Some(shorter) if text::bold_width(ui, Text::Body, whole) > room => shorter,
                        _ => whole,
                    };
                    let named = menu.named.get(at).and_then(Option::as_deref);
                    menus::row_words(ui, said, named, room)
                })
                .collect();
            let title = menu
                .title
                .as_deref()
                .map(|title| text::fit_bold(ui, Text::Title, title, room.title));
            // Out of the button the screen named, whatever the light is on,
            // and never over it — for the menu as wide as these rows grow it.
            if let Some(button) = self.menu_anchor {
                let ui = page.ui();
                let width = menus::grown_width(ui, &commands, menu.marked, title.as_deref(), scale);
                let display = [page.width(), page.height()];
                let rows = commands.len();
                let titled = title.is_some();
                page.light_at(menus::clear_of(button, rows, titled, width, display));
            }
            let commands: Vec<&str> = commands.iter().map(String::as_str).collect();
            match (menu.marked, menu.on) {
                (None, Some(row)) => page.menu_on(title.as_deref(), &commands, row),
                (marked, _) => {
                    page.menu_marked(title.as_deref(), &commands, marked.unwrap_or(usize::MAX))
                }
            }
            self.menu_open = Some((self.screen, menu.id));
            return true;
        }
        let Some(question) = self.question.take() else {
            return false;
        };
        let question = self.put_as(question);
        // The dialog's title is one line at the dialog's width, and its
        // answers a row hung off its right-hand edge: where either would run
        // past it — a display standing on its side — the one that has
        // shorter words says those. The list of pending operations and the
        // plan's question are as much as the window holds once each line is
        // broken where the dialog draws it.
        let room = menus::dialog_room(page.width(), scale);
        let most = menus::dialog_lines(page.height(), scale);
        let (title, body, mut answers) = {
            let ui = page.ui();
            self.dialog(question, most, room, |line| ui.measure(Text::Caption, line))
        };
        let ui = page.ui();
        let title = match self.shorter_title(question) {
            Some(shorter) if ui.measure(Text::Title, &title) > room => shorter,
            _ => title,
        };
        let title = text::fit(ui, Text::Title, &title, room);
        let body = menus::body_words(ui, &body, room);
        let whole: Vec<&str> = answers.iter().map(String::as_str).collect();
        if menus::answers_width(ui, &whole, scale) > room {
            if let (Some(shorter), Some(second)) =
                (self.shorter_answer(question), answers.get_mut(1))
            {
                *second = shorter;
            }
        }
        let answers: Vec<&str> = answers.iter().map(String::as_str).collect();
        page.ask(&title, &body, &answers);
        self.answering = Some(question);
        true
    }

    /// Draw the panel and everything in it: the page drawn now and — while a
    /// change of page is under way — the page it is coming in over
    /// (`change`), each where it stands on this frame, with what is the same
    /// on both (Continue, the legend) standing still and drawn once.
    fn draw(&mut self, page: &mut Page) {
        if self.low_end {
            self.settle();
        }
        self.notice_a_change();
        let dt = self.clock.dt;
        self.change.advance(dt);
        let place = self.screen.place();
        self.chips.aim(place);
        if self.low_end {
            self.chips.settle();
        }
        self.chips.advance(dt);

        let head = screens::head(self);
        let hints = self.hints();
        let pad = self.pad;
        let note = self.foot_note();
        let laid = Self::lay(page, &head, place, &hints, note.as_ref(), pad);
        panel::glass(page, &laid);
        let stage = Stage::of(page, &laid);

        // The page going first, under the one coming in.
        let going = self.draw_leaving(page, &stage, &hints, note.as_ref(), laid.stacked);

        let arriving = self.change.arriving();
        let from = page.ui().drawn();
        self.laid_buttons.clear();
        panel::header(page, &laid, panel::Header { mark: head.mark }, place, 1.0);

        // The board rises from the foot of the buttons' band over whatever
        // stands there, and the screen's own controls give up what it takes;
        // the buttons stay where they stand, under it, while it is up. What
        // the screen draws is cut off where the board's top is: a word under
        // a pane is still drawn over it (words go down after a layer's
        // glass), and Continue's showed through the board's last row.
        let reach = laid.reach();
        let mut content = laid.room;
        let mut under_board = None;
        if let Some(board) = self.board.as_mut() {
            board.advance(self.clock.dt);
            let taken = Board::height(page, reach[2]) * board.risen();
            let top = reach[1] + reach[3] - taken;
            content[3] = content[3]
                .min(top - page.metric(Metric::Gap) - content[1])
                .max(1.0);
            under_board = Some(top);
        } else if let Some(top) = self.shells_board_top(page) {
            content[3] = content[3]
                .min(top - page.metric(Metric::Gap) - content[1])
                .max(1.0);
            under_board = Some(top);
        }
        // The screen's light is the board's once the board has it: until its
        // first letter has risen into sight the light stays on the field.
        let lit = self.board.as_ref().is_none_or(Board::waiting);
        self.standing_in = None;
        self.quiet_in = None;
        self.field_lit = None;
        self.menu_anchor = None;
        let room = room_of(&laid, content, under_board);
        let written = page.ui().written();
        screens::draw(self, page, room, lit);
        if screens::drawn(self) {
            // The second column of Time zone or Keyboard is being counted on
            // the screen, which only frames do (`trail::Seen`): the next is
            // asked for at once, a low-end window, which draws a still page
            // once a second, included.
            page.redraw_within(Duration::from_millis(16));
        }
        self.hold_typing();
        self.let_go_of_a_field_left();
        if let Some(top) = under_board {
            let to = page.ui().written();
            let above = [0.0, 0.0, page.width(), top.max(0.0)];
            page.ui().cut_between(written, to, above);
        }
        let to = page.ui().drawn();
        let rows = std::mem::take(&mut self.laid_buttons);
        if !arriving.standing() {
            stage.carry_page(page, (from, to), &rows, arriving, true);
        }
        let through = self.change.through();
        if let Some(going) = &going {
            stage.hand_over_buttons(page, through, (&going.rows, going.at), (&rows, arriving));
        }
        if self.change.moving() {
            stage.soften_sides(page);
        }

        // Keys typed between this frame and the next are text while a field
        // is being typed into, and presses otherwise. Asked for again, the
        // shell's board needs a frame on which the field takes nothing.
        let again = std::mem::take(&mut self.summon_again);
        page.taking_text(self.typing_into().is_some() && !again);
        page.taking_keys(self.quiet_in.is_some());
        if again {
            page.redraw_within(Duration::from_millis(30));
        }
        if let Some(mut board) = self.board.take() {
            let pressing = self.pressing.through();
            let from = page.ui().drawn();
            board.draw(page, reach, &mut self.light, pressing, self.clock.dt);
            // Cut to the room and the buttons' band, so that a board on its
            // way up comes out of their foot rather than being drawn below
            // the panel's rim, under the legend.
            let to = page.ui().drawn();
            page.ui().carry_between(from, to, Carry::standing(reach));
            self.board = Some(board);
        }

        // A note on lines of its own stands over the buttons, which the
        // board rises over: under the board it is not drawn, rather than
        // drawn over its keys.
        let role = note.map_or(Role::Text, |note| note.role);
        let covered =
            under_board.is_some_and(|top| laid.stacked && laid.note_at[1] + laid.note_at[3] > top);
        let foot = if covered {
            let mut uncovered = laid.clone();
            uncovered.note.clear();
            panel::foot(page, &uncovered, &hints, pad, role)
        } else {
            panel::foot(page, &laid, &hints, pad, role)
        };
        // The legend and the note stand still; where the page going said
        // something else there, what changed crosses where it stands.
        if let Some(old) = going.as_ref().and_then(|going| going.foot.as_ref()) {
            stage.hand_over_foot(page, through, old, &foot);
        }

        // Where in the walk this is, on its own way there.
        let row = place.map(|place| panel::chip_row(page, &laid, place));
        self.chips.draw(page, row);
        if self.low_end {
            self.settle();
        }
    }

    /// Where the top of the shell's board stands over the window on this
    /// frame, if it takes any of it: while the toolkit says it stands there
    /// (`board_over`), risen to where the shell settles it
    /// (`lxb_toolkit::board::rect`, for a window that fills the display, as
    /// every window under LineXinBar does), and on its way down from the
    /// moment it is put away. The shell raises its board over a field that
    /// takes text with a controller in its hands, however the field was
    /// chosen, and never moves it out of the field's way, so the screen's
    /// room ends over it as it ends over the installer's own: Keyboard's test
    /// field rises ahead of it, and You's form scrolls the field typed into
    /// above it (seen live: both were typed into blind under its keys, and a
    /// field clicked or tapped stood under it, the band left reserved over
    /// Continue after the board's own hide key).
    fn shells_board_top(&mut self, page: &Page) -> Option<f32> {
        self.shells_board_top_at(page.height())
    }

    /// `shells_board_top` on a window `height` tall.
    fn shells_board_top_at(&mut self, height: f32) -> Option<f32> {
        let wanted = self
            .board_over
            .map_or(0.0, |board| (height - board[1]).max(0.0));
        let taken = if self.low_end || std::mem::take(&mut self.shell_room_at_once) {
            self.shell_room.place(wanted);
            wanted
        } else {
            self.shell_room.advance(wanted, self.clock.dt)
        };
        (taken >= 1.0).then_some(height - taken)
    }

    /// Lay the panel out for a page whose head, legend and note are these.
    fn lay(
        page: &mut Page,
        head: &screens::Head,
        place: Option<panel::Place>,
        hints: &[Hint],
        note: Option<&Note>,
        pad: bool,
    ) -> Panel {
        let titles = screens::step_titles();
        Panel::lay(
            page,
            panel::Needs {
                title: &head.title,
                titles: &titles,
                caption: &head.caption,
                place,
                hints,
                pad,
                note: note.map(|note| note.text.as_str()),
            },
        )
    }

    /// Begin a change of page where the page drawn now is not the one drawn
    /// on the last frame, however it changed — a key, a click, a menu's row,
    /// the installation ending — and not in low-end mode, where a page
    /// changes at once.
    fn notice_a_change(&mut self) {
        let stood = screens::stood(self);
        let Some(was) = self.drawn_page.replace(stood) else {
            return;
        };
        let shown = stood.page;
        if was.page == shown {
            return;
        }
        if self.low_end {
            // The page a press sent away is gone at once, and its dip with
            // it: nothing on the page that takes its place was pressed.
            self.change.settle();
            self.pressing = Pressing::default();
            return;
        }
        // A press under way goes with the page it was pressed on: Accept on
        // Continue dips Continue as the page leaves, and nothing on the page
        // coming in, where it had dipped the row the light came in on.
        let pressing = std::mem::take(&mut self.pressing);
        let way = Way::between(was.page, shown);
        match self.change.begin(was, shown, self.light, pressing, way) {
            // Called back while it went, a page takes back the light and the
            // press it was drawn with, and the page it gives way to keeps
            // the installer's.
            Some(called) => {
                self.light = called.light;
                self.pressing = called.pressing;
            }
            // Otherwise the installer's own light goes straight to the page
            // coming in, which carries it in; the page going keeps a light of
            // its own.
            None => self.light.settle(),
        }
    }

    /// Draw the page going, where it stands on this frame of the change, as
    /// it was drawn when it was the page: its own head and room and its own
    /// light, in the panel laid out for it, nothing of it pressable. Its
    /// legend and note are drawn too where the page coming in says something
    /// else there (`hints`, `note`, `stacked`), fading where they stand.
    /// Answers what its buttons left on the frame.
    fn draw_leaving(
        &mut self,
        page: &mut Page,
        stage: &Stage,
        hints: &[Hint],
        note: Option<&Note>,
        stacked: bool,
    ) -> Option<Going> {
        let (stood, at) = self.change.leaving()?;
        let was = screens::stand_on(self, stood);
        let shown = stood.page;
        let kept = (
            self.standing_in,
            self.quiet_in,
            self.field_lit,
            self.menu_anchor,
        );
        std::mem::swap(&mut self.light, self.change.leaving_light());
        std::mem::swap(&mut self.pressing, self.change.leaving_pressing());

        let head = screens::head(self);
        let place = shown.screen.place();
        let its_hints = self.hints();
        let its_note = self.foot_note();
        let laid = Self::lay(page, &head, place, &its_hints, its_note.as_ref(), self.pad);
        let from = page.ui().drawn();
        self.laid_buttons.clear();
        panel::header(page, &laid, panel::Header { mark: head.mark }, place, 1.0);
        screens::draw(self, page, room_of(&laid, laid.room, None), true);
        let to = page.ui().drawn();
        let rows = std::mem::take(&mut self.laid_buttons);

        std::mem::swap(&mut self.light, self.change.leaving_light());
        std::mem::swap(&mut self.pressing, self.change.leaving_pressing());
        (
            self.standing_in,
            self.quiet_in,
            self.field_lit,
            self.menu_anchor,
        ) = kept;
        screens::stand_back(self, was);
        stage.carry_page(page, (from, to), &rows, at, false);

        // Its legend and note only where the page coming in says something
        // else there; what is the same stands still, drawn once.
        let changed = its_hints != hints || its_note.as_ref() != note || laid.stacked != stacked;
        let foot = changed.then(|| {
            let role = its_note.as_ref().map_or(Role::Text, |note| note.role);
            panel::foot(page, &laid, &its_hints, self.pad, role)
        });
        Some(Going { rows, at, foot })
    }

    /// The pointer's presses, asked of the spots this frame's drawing wrote.
    fn pressed(&mut self, page: &mut Page) {
        self.click(|state| state.pressed_here(page));
    }

    /// Answer the pointer with `press`, unless this is the second click of a
    /// double-click whose first put another page under the pointer — another
    /// screen, an Options page, a trail's second column, a pair slid across
    /// (`screens::page`) — which is let go of: the toolkit drops a press
    /// nobody asked about when the frame ends.
    ///
    /// A click acts at once, on the control under it, as Accept on it would
    /// (lxb-toolkit's way). So the second click of a double-click would act
    /// on whatever the first had brought in under the resting pointer:
    /// double-clicking a part of the world on a landscape panel would choose
    /// the place that slid in where it stood and go on to Keyboard, and
    /// double-clicking Continue — which stands at one place on every screen —
    /// would go past the next question. Keys, and clicks that change nothing
    /// under the pointer (the board's keys, a switch, a field), are not held
    /// back here: Accept held down is pressed once (only the directions
    /// repeat), so a key pressed twice was meant twice. Time zone's and
    /// Keyboard's Continue hold back any press, a key's or a click's, that
    /// comes before the column it showed has stood on the screen for its
    /// dwell (`trail::Seen`): a double click on it too, where its first click
    /// only rolled an open column over and so changed no page here.
    pub fn click(&mut self, press: impl FnOnce(&mut Self)) {
        let now = self.clock.now;
        if self
            .paged_by_click
            .is_some_and(|at| (0.0..DOUBLE_CLICK).contains(&(now - at)))
        {
            return;
        }
        let before = screens::page(self);
        press(self);
        if screens::page(self) != before {
            self.paged_by_click = Some(now);
        }
    }

    /// A pair of the legend pressed by the pointer or a finger: the button
    /// it pictures, pressed. Where it pictures a pad's, it is that pad's
    /// press, though the toolkit counts the hand on the pointer — so the
    /// legend's Type on a field whose shell's board was put away asks for
    /// the board again (`act`), where Enter, which a keyboard's picture is,
    /// would be done with the field.
    pub fn press_pair(&mut self, button: legend::Button) {
        let was = self.pad_pressed;
        self.pad_pressed |= self.pad;
        self.act(button.action(), None);
        self.pad_pressed = was;
    }

    fn pressed_here(&mut self, page: &mut Page) {
        let hints = self.hints();
        let pad = self.pad;
        if let Some(button) = legend::pressed(page, &hints, pad) {
            self.press_pair(button);
            return;
        }
        if let Some(mut board) = self.board.take() {
            let stroke = board.pressed(page);
            self.board = Some(board);
            if let Some(stroke) = stroke {
                self.stroke(stroke);
            }
            return;
        }
        screens::pressed(self, page);
    }

    /// Put everything where it is going, for a photograph — and on every
    /// frame in low-end mode, where nothing of the installer's own moves: the
    /// light on its control, a page that is changing changed, the long chip
    /// in its place, the board risen, the bar at the work.
    pub fn settle(&mut self) {
        self.shell_room_at_once = true;
        self.light.settle();
        self.change.settle();
        self.chips.settle();
        self.progress.settle();
        screens::settle(self);
        if let Some(board) = self.board.as_mut() {
            board.settle();
        }
    }

    // ---------------------------------------------------------------------
    // Pictures.

    /// Put a demo where a named picture shows it. See `SCENES`.
    pub fn stage(&mut self, scene: &str) -> Result<(), String> {
        let screen = match scene {
            "welcome" => Screen::Welcome,
            "language" | "language-search" => Screen::Language,
            "timezone"
            | "timezone-continue"
            | "timezone-places"
            | "timezone-search"
            | "timezone-search-nothing" => Screen::Timezone,
            "keyboard"
            | "keyboard-layouts"
            | "keyboard-continue"
            | "keyboard-none"
            | "keyboard-search"
            | "keyboard-search-nothing" => Screen::Keyboard,
            "target"
            | "target-erase"
            | "target-free"
            | "target-nothing"
            | "target-whole"
            | "target-kinds"
            | "target-options"
            | "target-storage"
            | "target-refused"
            | "target-partitions-in-use"
            | "partitions"
            | "partitions-nvme"
            | "partitions-msdos"
            | "partitions-tiny"
            | "partitions-blocked"
            | "partitions-menu"
            | "partitions-mount"
            | "partitions-table"
            | "partitions-planned"
            | "partitions-options"
            | "partitions-pending"
            | "partitions-missing"
            | "partitions-new"
            | "partitions-new-held"
            | "partitions-new-typed"
            | "partitions-resize"
            | "partitions-label"
            | "partitions-other"
            | "partitions-new-planned"
            | "partitions-continue-refused"
            | "partitions-portrait-slid"
            | "partitions-unmountable"
            | "partitions-measured"
            | "partitions-hibernated"
            | "partitions-hibernated-other"
            | "partitions-reading" => Screen::Target,
            "desktop" | "desktop-options" | "desktop-software" | "desktop-loaders-plan" => {
                Screen::Desktop
            }
            "account" | "account-empty" | "account-administrator" | "board" => Screen::Account,
            "review" | "confirm" | "review-partitions" | "confirm-partitions"
            | "confirm-plan-pages" | "confirm-plan-last" => Screen::Review,
            "installing"
            | "installing-details"
            | "done"
            | "stopped"
            | "stopped-details"
            | "installing-shrink"
            | "done-shrink"
            | "stopped-shrink"
            | "stopped-hibernated"
            | "stopped-needs-check"
            | "stopped-windows-in-use"
            | "stopped-windows-check"
            | "stopped-windows-unread"
            | "stopped-bitlocker"
            | "stopped-windows-shrunk" => Screen::Installing,
            other => {
                return Err(format!(
                    "no scene called {other}; the scenes are: {}",
                    SCENES.join(" ")
                ))
            }
        };
        self.go(screen);
        // A scene past Time zone, or past Keyboard, is reached as the person
        // reaches it: through that step's Continue, which goes on only with
        // what it holds seen (`trail::Seen`) — so that Back to it from the
        // picture finds it seen, as Back finds it after the walk.
        if !matches!(
            screen,
            Screen::Welcome | Screen::Language | Screen::Timezone
        ) {
            self.views.timezone.seen.see();
        }
        if !matches!(
            screen,
            Screen::Welcome | Screen::Language | Screen::Timezone | Screen::Keyboard
        ) {
            self.views.keyboard.seen.see();
        }
        screens::stage(self, scene);
        match scene {
            "board" => {
                self.draft.account.full_name = "Alex Example".into();
                screens::account::edited(self, FieldId::FullName);
                self.open_board(FieldId::FullName);
                self.sounds.clear();
            }
            "confirm" | "confirm-partitions" => self.ask(Question::Erase),
            // The plan's paged question on its first page, and on its last
            // (one past it is the last).
            "confirm-plan-pages" => self.ask(Question::PlanErase { page: 0 }),
            "confirm-plan-last" => self.ask(Question::PlanErase { page: usize::MAX }),
            "installing" | "installing-details" => self.rehearse_until(7.0),
            "done" => {
                self.rehearse_until(60.0);
                // One thing that did not work, so the picture shows how the
                // finished screen says it.
                self.install.take(Event::Warning {
                    warning: crate::model::protocol::Warning::BootEntryNotRegistered,
                });
                self.follow();
            }
            "stopped" | "stopped-details" => {
                self.rehearse_until(5.0);
                self.install.take(Event::Failed {
                    step: Some(Step::Copy),
                    failure: Failure::DiskChanged,
                    detail: "rehearsal: a made-up failure".into(),
                });
                self.follow();
                // Stopped is arrived at only once the failure is followed,
                // and arriving closes Details: they are opened after.
                if scene == "stopped-details" {
                    screens::stopped::stage(self);
                }
            }
            // The plan that keeps Windows smaller (`review::
            // stage_with_the_plan`), rehearsed: into its shrink, to Done
            // telling Windows will check its disk, and stopped where the
            // worker stops a shrink — in Resize, or in Check before anything
            // is written, for each cause it reads there: Windows hibernated,
            // the partition to be shrunk asking for a check, and another
            // Windows partition in use, marked for Windows' check, not read,
            // or a BitLocker volume.
            "installing-shrink" => self.rehearse_until(1.8),
            // Stopped once Windows' partition was shrunk and Windows told
            // to check its disk: the disk read back wrong in Partition.
            "stopped-windows-shrunk" => {
                self.rehearse_until(3.0);
                self.install.take(Event::Failed {
                    step: Some(Step::Partition),
                    failure: Failure::DiskChanged,
                    detail: "rehearsal: a made-up failure".into(),
                });
                self.follow();
            }
            "done-shrink" => {
                self.rehearse_until(60.0);
                self.follow();
            }
            "stopped-shrink"
            | "stopped-hibernated"
            | "stopped-needs-check"
            | "stopped-windows-in-use"
            | "stopped-windows-check"
            | "stopped-windows-unread"
            | "stopped-bitlocker" => {
                let (seconds, step, failure) = match scene {
                    "stopped-shrink" => (1.8, Step::Resize, Failure::ShrinkRefused),
                    "stopped-hibernated" => (0.5, Step::Check, Failure::WindowsHibernated),
                    "stopped-windows-in-use" => (0.5, Step::Check, Failure::WindowsInUse),
                    "stopped-windows-check" => (0.5, Step::Check, Failure::WindowsNeedsCheck),
                    "stopped-windows-unread" => (0.5, Step::Check, Failure::WindowsUnread),
                    "stopped-bitlocker" => (0.5, Step::Check, Failure::BitLocker),
                    _ => (0.5, Step::Check, Failure::FilesystemNeedsCheck),
                };
                self.rehearse_until(seconds);
                self.install.take(Event::Failed {
                    step: Some(step),
                    failure,
                    detail: "rehearsal: a made-up failure".into(),
                });
                self.follow();
            }
            _ => {}
        }
        self.sounds.clear();
        Ok(())
    }

    /// Fold in what a rehearsal would have said `seconds` after it began.
    fn rehearse_until(&mut self, seconds: f32) {
        let mut rehearsal = Rehearsal::start(0.0, &self.judged());
        for event in rehearsal.poll(seconds) {
            self.install.take(event);
        }
        self.progress.advance(self.install.through(), 0.0);
        self.progress.settle();
    }
}

/// What the page going left on the frame: its rows of buttons, where it
/// stood, and its legend and note where they said something else than the
/// page coming in's.
struct Going {
    rows: Vec<LaidRow>,
    at: Placing,
    foot: Option<LaidFoot>,
}

/// Where pages are carried on this frame: cut to the pane at its sides and
/// half its padding inside it at its top and foot, so nothing on its way in
/// or out is drawn past the glass; a step of [`change::TRAVEL`] of its width;
/// scaled about its middle.
struct Stage {
    within: [f32; 4],
    travel: f32,
    about: [f32; 2],
    /// The pane, and its padding: the margin at each side where nothing that
    /// stands still is drawn, which a page in motion fades out across.
    pane: [f32; 4],
    margin: f32,
}

impl Stage {
    fn of(page: &Page, laid: &Panel) -> Self {
        let pane = laid.pane;
        let margin = page.metric(Metric::PanelPadding);
        let inset = margin * 0.5;
        Self {
            within: [
                pane[0],
                pane[1] + inset,
                pane[2].max(1.0),
                (pane[3] - inset * 2.0).max(1.0),
            ],
            travel: pane[2] * change::TRAVEL,
            about: [pane[0] + pane[2] * 0.5, pane[1] + pane[3] * 0.5],
            pane,
            margin,
        }
    }

    /// Fade what is on its way in or out across the margin at each side of
    /// the pane, into the glass, rather than cutting it off at the pane's
    /// edge: LineXinBar's columns run on off the display, where a page here
    /// has the edge of its glass to stop at. Only the pages move there —
    /// the head, the room, the buttons and the legend all stand inside the
    /// padding — so what stands still is untouched.
    fn soften_sides(&self, page: &mut Page) {
        page.ui().soft_side_edges(self.pane, self.margin, 1.0, 1.0);
    }

    /// A page standing `at`.
    fn carry(&self, at: Placing, pressable: bool) -> Carry {
        Carry {
            scale: at.scale,
            about: self.about,
            by: [at.across * self.travel, 0.0],
            fade: at.fade,
            words: at.words,
            within: self.within,
            pressable,
        }
    }

    /// Something that stays where it is and only fades.
    fn in_place(&self, fade: f32, pressable: bool) -> Carry {
        Carry {
            fade,
            pressable,
            ..Carry::standing(self.within)
        }
    }

    /// Carry everything a page drew between two marks to where it stands —
    /// its head and its room — but its buttons, which never slide
    /// ([`Stage::hand_over_buttons`]).
    fn carry_page(
        &self,
        page: &mut Page,
        (from, to): (Drawn, Drawn),
        rows: &[LaidRow],
        at: Placing,
        pressable: bool,
    ) {
        let carry = self.carry(at, pressable);
        let mut start = from;
        for (part_from, part_to) in rows.iter().flat_map(LaidRow::parts) {
            page.ui().carry_between(start, part_from, carry);
            start = part_to;
        }
        page.ui().carry_between(start, to, carry);
    }

    /// The buttons of the page going and of the page coming in, `through` of
    /// the way through the change. A button standing in the same place on
    /// both — Continue, from one step to the next — stands still, drawn once,
    /// by the page coming in; and so does the light on it, where it is on it
    /// on both. One of the same meaning in the same place with another word —
    /// Continue becoming Done, Get started becoming Continue — keeps its
    /// glass and changes only its word, the one gone before the other comes
    /// ([`halves`]), never two words in one capsule. Every other button, and
    /// a light that moved, fades where it stands, going or coming: buttons
    /// never slide.
    ///
    /// A lit button's glass is its light — the toolkit leaves the chip under
    /// the light bare — so where the button comes in lit and its light comes
    /// up with the page (it was elsewhere on the page going), the glass going
    /// stays until that light is up: Back from Language to Welcome put Get
    /// started's light in where Continue stood unlit, and for two frames the
    /// place was a bare word. Going lit, the other way round, the glass
    /// coming in comes up as the light goes.
    fn hand_over_buttons(
        &self,
        page: &mut Page,
        through: f32,
        (going, going_at): (&[LaidRow], Placing),
        (coming, coming_at): (&[LaidRow], Placing),
    ) {
        let (word_out, word_in) = halves(through);
        let lit_stays = going.iter().filter_map(LaidRow::lit).any(|old| {
            coming
                .iter()
                .filter_map(LaidRow::lit)
                .any(|new| new.same_place(old))
        });
        let mut fade = |(from, to): (Drawn, Drawn), carry: Carry| {
            page.ui().carry_between(from, to, carry);
        };
        for row in going {
            for button in &row.buttons {
                let (from, to) = button.glass;
                let (over, there) = handed_over(coming, button);
                let held = match there {
                    Some(there) if there.lit && !button.lit => 1.0 - coming_at.fade,
                    _ => 0.0,
                };
                match over {
                    Over::Stays => {
                        fade((from, button.word), self.in_place(held, false));
                        fade((button.word, to), self.in_place(0.0, false));
                    }
                    Over::Word => {
                        fade((from, button.word), self.in_place(held, false));
                        fade((button.word, to), self.in_place(word_out, false));
                    }
                    Over::Fades => fade((from, to), self.in_place(going_at.fade, false)),
                }
            }
            if let Some(light) = row.light {
                let gone = if lit_stays { 0.0 } else { going_at.fade };
                fade(light, self.in_place(gone, false));
            }
        }
        for row in coming {
            for button in &row.buttons {
                let (from, to) = button.glass;
                let (over, there) = handed_over(going, button);
                // And the other way round: a button going lit, its light
                // going with the page, has the glass coming in come up as
                // that light goes, rather than covering it at once.
                if there.is_some_and(|there| there.lit && !button.lit) {
                    fade(
                        (from, button.word),
                        self.in_place(1.0 - going_at.fade, true),
                    );
                }
                match over {
                    Over::Stays => {}
                    Over::Word => fade((button.word, to), self.in_place(word_in, true)),
                    Over::Fades => fade((from, to), self.in_place(coming_at.fade, true)),
                }
            }
            if let (Some(light), false) = (row.light, lit_stays) {
                fade(light, self.in_place(coming_at.fade, true));
            }
        }
    }

    /// The legend and the note of the page going and of the page coming in,
    /// `through` of the way through the change: a pair of the legend that
    /// says the same of the same button in the same place on both stands
    /// still — Back in the corner, from one step to the next — and so does a
    /// note that says the same where it said it; a pair that says the same
    /// but stands a little along the row, because a pair beside it says
    /// something longer or shorter (Choose beside Leave, then beside Back),
    /// goes along to its new place; everything else crosses where it stands,
    /// the old gone before the new comes ([`halves`]), so two words are
    /// never written over each other. Crossed by halves, a pair moved by a
    /// few pixels went out and came back, and the whole legend was blank for
    /// a frame.
    fn hand_over_foot(&self, page: &mut Page, through: f32, going: &LaidFoot, coming: &LaidFoot) {
        let (out, into) = halves(through);
        for pair in &going.pairs {
            let goes_along = coming.pairs.iter().any(|other| other.said_alike(pair));
            let (from, to) = pair.drawn;
            let fade = if goes_along { 0.0 } else { out };
            page.ui()
                .carry_between(from, to, self.in_place(fade, false));
        }
        for pair in &coming.pairs {
            let (from, to) = pair.drawn;
            match going.pairs.iter().find(|other| other.said_alike(pair)) {
                Some(old) if old.same_as(pair) => {}
                Some(old) => {
                    // In the middle of the step, where what crosses by
                    // halves is all but gone on both sides: going along
                    // sooner, it ran into a pair still fading where it went
                    // (Choose into Options, Desktop to its Options page on a
                    // display standing on its side).
                    let along = lxb_app::lxb_toolkit::motion::ease(
                        ((through - ALONG_FROM) / (1.0 - 2.0 * ALONG_FROM)).clamp(0.0, 1.0),
                    );
                    let left = 1.0 - along;
                    let carry = Carry {
                        by: [
                            (old.at[0] - pair.at[0]) * left,
                            (old.at[1] - pair.at[1]) * left,
                        ],
                        ..Carry::standing(self.within)
                    };
                    page.ui().carry_between(from, to, carry);
                }
                None => page.ui().carry_between(from, to, self.in_place(into, true)),
            }
        }
        let note_stays = matches!(
            (&going.note, &coming.note),
            (Some(old), Some(new)) if old.same_as(new)
        );
        if let Some(old) = &going.note {
            let (from, to) = old.drawn;
            let fade = if note_stays { 0.0 } else { out };
            page.ui()
                .carry_between(from, to, self.in_place(fade, false));
        }
        if let (Some(new), false) = (&coming.note, note_stays) {
            let (from, to) = new.drawn;
            page.ui().carry_between(from, to, self.in_place(into, true));
        }
    }
}

/// Where in a change a legend pair that says the same goes along to its new
/// place: from this share of the way to as far from the end, where a pair
/// crossing by halves beside it has under 4% of its ink, going or coming.
const ALONG_FROM: f32 = 0.4;

/// How a button of one page is handed over to the other's
/// (`Stage::hand_over_buttons`).
enum Over {
    /// The same button in the same place: it stands still.
    Stays,
    /// One of the same meaning in the same place with another word: its
    /// glass stands still and its word changes.
    Word,
    /// Nothing like it there: it fades where it stands.
    Fades,
}

/// How `button` is handed over to a page whose rows of buttons are `rows`,
/// and the button there it is handed over to, if one is.
fn handed_over<'a>(
    rows: &'a [LaidRow],
    button: &super::button::Laid,
) -> (Over, Option<&'a super::button::Laid>) {
    let mut others = rows.iter().flat_map(|row| &row.buttons);
    if let Some(same) = others.clone().find(|other| other.same_as(button)) {
        (Over::Stays, Some(same))
    } else if let Some(there) = others.find(|other| other.same_place(button)) {
        (Over::Word, Some(there))
    } else {
        (Over::Fades, None)
    }
}

/// Where a screen draws on a panel laid out as `laid`: its controls in
/// `content` (the room, less the board while it is up, whose top is
/// `under_board`), its buttons in the panel's band.
fn room_of(laid: &Panel, content: [f32; 4], under_board: Option<f32>) -> screens::Room {
    let under = [
        Some(laid.buttons[1]),
        laid.stacked.then_some(laid.note_at[1]),
        under_board,
    ]
    .into_iter()
    .flatten()
    .fold(f32::INFINITY, f32::min);
    screens::Room {
        content,
        buttons: laid.buttons,
        note_ends: laid.stacked.then(|| laid.note_at[1] + laid.note_at[3]),
        clear: [
            (content[1] - (laid.head[1] + laid.head[3])).max(0.0),
            (under - (content[1] + content[3])).max(0.0),
        ],
        board_top: under_board,
    }
}

/// The pictures `--shot --scene` can take.
pub const SCENES: [&str; 79] = [
    "welcome",
    "language",
    "language-search",
    "timezone",
    "timezone-continue",
    "timezone-places",
    "timezone-search",
    "timezone-search-nothing",
    "keyboard",
    "keyboard-layouts",
    "keyboard-continue",
    "keyboard-none",
    "keyboard-search",
    "keyboard-search-nothing",
    "target",
    "target-erase",
    "target-free",
    "target-nothing",
    "target-whole",
    "target-kinds",
    "target-options",
    "target-storage",
    "target-refused",
    "target-partitions-in-use",
    "partitions",
    "partitions-nvme",
    "partitions-msdos",
    "partitions-tiny",
    "partitions-blocked",
    "partitions-menu",
    "partitions-mount",
    "partitions-table",
    "partitions-planned",
    "partitions-options",
    "partitions-pending",
    "partitions-missing",
    "partitions-new",
    "partitions-new-held",
    "partitions-new-typed",
    "partitions-resize",
    "partitions-label",
    "partitions-other",
    "partitions-new-planned",
    "partitions-continue-refused",
    "partitions-portrait-slid",
    "partitions-unmountable",
    "partitions-measured",
    "partitions-hibernated",
    "partitions-hibernated-other",
    "partitions-reading",
    "desktop",
    "desktop-options",
    "desktop-software",
    "desktop-loaders-plan",
    "account",
    "account-empty",
    "account-administrator",
    "review",
    "confirm",
    "review-partitions",
    "confirm-partitions",
    "confirm-plan-pages",
    "confirm-plan-last",
    "installing",
    "installing-details",
    "installing-shrink",
    "done",
    "done-shrink",
    "stopped",
    "stopped-details",
    "stopped-shrink",
    "stopped-hibernated",
    "stopped-needs-check",
    "stopped-windows-in-use",
    "stopped-windows-check",
    "stopped-windows-unread",
    "stopped-bitlocker",
    "stopped-windows-shrunk",
    "board",
];

/// The e2e harness's walks on Partitions, replayed (the QEMU campaign of
/// Unit Q, its step 0).
#[cfg(test)]
mod walks;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::MIB;
    use crate::ui::motion::FRAME;

    #[test]
    fn a_field_being_typed_in_keeps_the_light_from_the_pointer() {
        use screens::account::{Focus, Item};
        let field = |id| Focus::Item(Item::Field(id));
        let lit = |state: &State| state.views.account.focus;
        // What drawing the form tells the frame: the field the light stands
        // in, which takes the keys.
        let draw = |state: &mut State| {
            state.standing_in = match state.views.account.focus {
                Focus::Item(Item::Field(id)) => Some(id),
                _ => None,
            };
            state.hold_typing();
        };
        let mut state = State::demo(true);
        state.go(Screen::Account);
        draw(&mut state);
        // Nothing typed or pressed yet: hovering is selecting.
        state.hover(FieldId::Username.spot());
        assert_eq!(lit(&state), field(FieldId::Username));
        state.hover(FieldId::FullName.spot());
        draw(&mut state);

        // Typed into, the field keeps the light, and the keys, under a
        // pointer nudged onto another field.
        state.type_keys(vec![Edit::Wrote("Alex".into())]);
        draw(&mut state);
        state.hover(FieldId::Hostname.spot());
        draw(&mut state);
        assert_eq!(lit(&state), field(FieldId::FullName));
        state.type_keys(vec![Edit::Wrote(" Example".into())]);
        assert_eq!(state.draft.account.full_name, "Alex Example");

        // A key moves the light on, and a field it brings the light into is
        // held before a letter is typed: the rest of a password never goes
        // into the plain-text name the pointer rests on.
        for _ in 0..2 {
            state.act(Action::Down, None);
            draw(&mut state);
        }
        assert_eq!(lit(&state), field(FieldId::Password));
        state.hover(FieldId::FullName.spot());
        draw(&mut state);
        assert_eq!(lit(&state), field(FieldId::Password));
        state.type_keys(vec![Edit::Wrote("secret".into())]);
        assert_eq!(state.draft.account.full_name, "Alex Example");

        // Out of the fields, the pointer lights what it moves onto again.
        for _ in 0..3 {
            state.act(Action::Down, None);
            draw(&mut state);
        }
        assert_eq!(lit(&state), Focus::Item(Item::Root));
        state.hover(FieldId::FullName.spot());
        assert_eq!(lit(&state), field(FieldId::FullName));
    }

    /// What drawing You tells the frame, without drawing it: the field the
    /// light stands in, and whether it takes a keyboard's typing.
    fn draw_you(state: &mut State) {
        use screens::account::{Focus, Item};
        state.standing_in = None;
        state.quiet_in = None;
        state.field_lit = None;
        if let Focus::Item(Item::Field(id)) = state.views.account.focus {
            state.in_field(id);
        }
        state.hold_typing();
        state.let_go_of_a_field_left();
    }

    /// A frame's start on a 1280×800 window, the toolkit saying whether
    /// LineXinBar's board stands over it (`Page::shells_board`).
    fn told(state: &mut State, up: bool) {
        let (width, height) = (1280.0, 800.0);
        let board = up.then(|| shells_board::rect(width, height));
        state.told(board, width, height);
    }

    /// Under LineXinBar a pad's Accept on a field never raises the
    /// installer's board: the field takes text, which is what brings the
    /// shell's own board up over it. What it types goes into the field;
    /// Enter, which the shell's board sends as Start puts it away, is
    /// answered as Enter from a keyboard — on to the next field.
    #[test]
    fn under_linexinbar_a_pad_types_with_the_shells_board() {
        use screens::account::{Focus, Item};
        let mut state = State::demo(true);
        state.type_with_the_shells_board(true);
        state.go(Screen::Account);
        state.pad = true;
        draw_you(&mut state);
        assert_eq!(
            state.typing_into(),
            None,
            "a field the light only passed took text, and the shell's board \
             would have come up over it"
        );
        state.act(Action::Accept, None);
        assert!(state.board.is_none(), "the installer's own board came up");
        assert_eq!(state.typing_with_the_shell(), Some(FieldId::FullName));
        draw_you(&mut state);
        assert_eq!(state.typing_into(), Some(FieldId::FullName));
        // The shell's board rises, and types: keys, which the toolkit counts
        // as a keyboard in hand — and the pad is held, so nothing is
        // answered as to a keyboard.
        told(&mut state, true);
        told(&mut state, true);
        state.pad_pressed = false;
        state.hold_the_pad(false, true);
        assert!(state.pad, "the shell's keys put the pad down");
        state.type_keys(vec![Edit::Wrote("Ola".into())]);
        draw_you(&mut state);
        assert_eq!(state.draft.account.full_name, "Ola");
        // Start on it: Enter, the board away. On to the username, as Enter
        // from a keyboard goes — where the username, the pad still held,
        // takes no text, and so brings no board up unasked.
        told(&mut state, false);
        state.act(Action::Accept, None);
        assert_eq!(state.typing_with_the_shell(), None);
        assert_eq!(
            state.views.account.focus,
            Focus::Item(Item::Field(FieldId::Username))
        );
        assert!(state.board.is_none());
        draw_you(&mut state);
        assert_eq!(state.typing_into(), None, "the next field took text");
        assert_eq!(
            state.quiet_in,
            Some(FieldId::Username),
            "the next field takes no keyboard's letters"
        );
        // Letters of a keyboard's own, now nothing is typed with the shell's
        // board: the field the light stands in takes them quietly, so the
        // toolkit hands them over as typing — Space a space, never Accept —
        // and the first of them lets the pad go. A pad's press would have
        // let go of it too.
        let before = state.draft.account.username.clone();
        state.arrived(false, true, false);
        state.type_keys(vec![Edit::Wrote("J".into())]);
        assert!(!state.pad, "a keyboard's letter left the pad held");
        draw_you(&mut state);
        assert_eq!(state.typing_into(), Some(FieldId::Username));
        state.arrived(false, true, false);
        state.type_keys(vec![Edit::Wrote("ohn Smith".into())]);
        assert_eq!(state.draft.account.username, format!("{before}John Smith"));
        assert_eq!(
            state.views.account.focus,
            Focus::Item(Item::Field(FieldId::Username))
        );
    }

    /// With a pad in hand, the field the light stands in takes a keyboard's
    /// typing quietly — outside LineXinBar too — and only says it takes
    /// text, which raises an on-screen keyboard, once a keyboard is in hand.
    /// A field the light is not in takes nothing.
    #[test]
    fn with_a_pad_in_hand_the_lit_field_takes_a_keyboard_quietly() {
        let mut state = State::demo(true);
        state.go(Screen::Account);
        state.pad = true;
        draw_you(&mut state);
        assert_eq!(state.typing_into(), None);
        assert_eq!(state.quiet_in, Some(FieldId::FullName));
        state.type_keys(vec![Edit::Wrote("A".into())]);
        assert_eq!(state.draft.account.full_name, "A");
        state.pad = false;
        draw_you(&mut state);
        assert_eq!(state.quiet_in, None);
        assert_eq!(state.typing_into(), Some(FieldId::FullName));
        // Off every field, nothing takes anything.
        state.go(Screen::Desktop);
        state.pad = true;
        state.type_keys(vec![Edit::Wrote("B".into())]);
        assert_eq!(state.draft.account.full_name, "A");
    }

    /// Back on the shell's board puts it away and reaches nothing here; the
    /// pad's next press does. South asks for the board again — the field
    /// lets go of its text for one frame, so the shell, which offers its
    /// board once a field, offers it again — and Back is done with the field
    /// and goes back as Back does.
    #[test]
    fn the_shells_board_put_away_is_asked_for_again_or_left() {
        let mut state = State::demo(true);
        state.type_with_the_shells_board(true);
        state.go(Screen::Account);
        state.pad = true;
        state.pad_pressed = true;
        draw_you(&mut state);
        state.act(Action::Accept, None);
        draw_you(&mut state);
        assert!(!state.summon_again);
        state.act(Action::Accept, None);
        assert!(state.summon_again, "South did not ask for the board again");
        assert_eq!(state.typing_with_the_shell(), Some(FieldId::FullName));
        state.act(Action::Back, None);
        assert_eq!(state.typing_with_the_shell(), None);
        assert_eq!(state.screen, Screen::Desktop, "Back did not go back");
    }

    /// The light leaving the field — a direction of the pad's once the
    /// board is away, a click elsewhere — lets go of its text.
    #[test]
    fn a_held_size_takes_a_number_from_a_keyboard_or_the_shells_board() {
        use crate::model::disks::GIB;
        // What drawing Partitions' sheet tells the frame while its size is
        // held: the field a number for it goes into.
        let draw = |state: &mut State| {
            state.standing_in = None;
            state.quiet_in = None;
            state.field_lit = None;
            let held = state
                .views
                .partitions
                .sheet
                .as_ref()
                .is_some_and(|sheet| sheet.held.is_some());
            if held {
                state.in_field(FieldId::PartSize);
            }
            state.hold_typing();
            state.let_go_of_a_field_left();
        };
        let size = |state: &State| {
            state
                .views
                .partitions
                .sheet
                .as_ref()
                .and_then(|sheet| sheet.size())
        };
        // A keyboard: its digits straight in, Enter keeps them.
        let mut state = State::demo(true);
        state.stage("partitions-new").unwrap();
        state.act(Action::Accept, None);
        draw(&mut state);
        assert_eq!(state.typing_into(), Some(FieldId::PartSize));
        state.type_keys(vec![Edit::Wrote("64".into())]);
        state.act(Action::Accept, None);
        assert_eq!(size(&state), Some(64 * GIB));
        // A pad under LineXinBar: Y raises the shell's board, which stays
        // while the size is held, and its Enter keeps what it typed.
        let mut state = State::demo(true);
        state.stage("partitions-new").unwrap();
        state.type_with_the_shells_board(true);
        state.pad = true;
        state.act(Action::Accept, None);
        draw(&mut state);
        state.act(Action::Menu, None);
        draw(&mut state);
        assert_eq!(state.typing_with_the_shell(), Some(FieldId::PartSize));
        state.type_keys(vec![Edit::Wrote("32,5".into())]);
        draw(&mut state);
        assert_eq!(state.typing_with_the_shell(), Some(FieldId::PartSize));
        state.act(Action::Accept, None);
        assert_eq!(
            size(&state),
            Some(32 * GIB + 512 * crate::model::disks::MIB)
        );
        assert_eq!(state.typing_with_the_shell(), None);
    }

    #[test]
    fn the_light_leaving_the_field_lets_the_shells_board_go() {
        let mut state = State::demo(true);
        state.type_with_the_shells_board(true);
        state.go(Screen::Account);
        state.pad = true;
        draw_you(&mut state);
        state.act(Action::Accept, None);
        draw_you(&mut state);
        state.act(Action::Down, None);
        draw_you(&mut state);
        assert_eq!(state.typing_with_the_shell(), None);
        assert_eq!(state.typing_into(), None, "the username took text");
    }

    /// Frame by frame, with what a real pad gives the toolkit: it says a pad
    /// is in hand from South until the first key, whatever frames go by in
    /// between, and the shell's board South summons holds the pad and types
    /// nothing for a while. The hold lasts until the board has typed and
    /// been put away: the legend keeps the pad's pictures, Enter from the
    /// board's Start leaves the next field taking nothing, and the pad's own
    /// South there makes it take text after a frame without — which is what
    /// brings the shell's board up over it again.
    #[test]
    fn a_real_pad_is_held_while_the_shells_board_is_up() {
        use screens::account::{Focus, Item};
        // A frame: the toolkit's word on the hands as it begins, what
        // arrived, the presses, and You drawn.
        fn frame(
            state: &mut State,
            pad_in_hand: bool,
            board: bool,
            typed: &str,
            presses: &[Action],
        ) {
            state.reach(pad_in_hand);
            told(state, board);
            state.arrived(pad_in_hand, !typed.is_empty(), !presses.is_empty());
            if !typed.is_empty() {
                state.type_keys(vec![Edit::Wrote(typed.into())]);
            }
            for action in presses {
                state.act(*action, None);
            }
            draw_you(state);
        }
        let mut state = State::demo(true);
        state.type_with_the_shells_board(true);
        state.go(Screen::Account);
        frame(&mut state, true, false, "", &[]);
        assert_eq!(state.typing_into(), None);
        // South on Full name: it takes text, and the shell's board rises.
        frame(&mut state, true, false, "", &[Action::Accept]);
        assert_eq!(state.typing_into(), Some(FieldId::FullName));
        // Frames go by with the board up, a pad still the last thing the
        // toolkit saw reached for.
        frame(&mut state, true, true, "", &[]);
        frame(&mut state, true, true, "", &[]);
        assert!(
            state.pad_held,
            "the pad was let go of before the board typed"
        );
        // The board types: a keyboard's keys, to the toolkit.
        frame(&mut state, false, true, "Ola", &[]);
        assert!(state.pad, "the shell's board's keys put the pad down");
        assert_eq!(state.draft.account.full_name, "Ola");
        // Its Start: Enter, and the board letting go of the keyboard after
        // it, in one pass. On to Username, which takes nothing while a pad
        // is in hand.
        frame(&mut state, false, false, "", &[Action::Accept]);
        assert_eq!(
            state.views.account.focus,
            Focus::Item(Item::Field(FieldId::Username))
        );
        assert!(state.pad, "Enter from the shell's board put the pad down");
        assert_eq!(state.typing_into(), None, "Username took text unasked");
        // The pad's own South there: Username takes text, so the shell
        // raises its board over it.
        frame(&mut state, true, false, "", &[Action::Accept]);
        assert_eq!(state.typing_with_the_shell(), Some(FieldId::Username));
        assert_eq!(state.typing_into(), Some(FieldId::Username));
    }

    /// The screen's room ends over the shell's board for as long as the
    /// toolkit says it stands over the window — risen on the cards' spring,
    /// put there at once on low-end hardware — and is given back from the
    /// frame the toolkit says it went: its own hide key, B, Start, a key of
    /// the person's own. Seen live: the band stayed reserved over Continue
    /// after the board's hide key until the pad's next press.
    #[test]
    fn the_room_ends_over_the_shells_board_while_the_toolkit_says_it_stands() {
        let (width, height) = (1280.0, 800.0);
        let top = shells_board::rect(width, height)[1];
        let mut state = State::demo(true);
        state.type_with_the_shells_board(true);
        state.go(Screen::Account);
        state.pad = true;
        state.clock.dt = 1.0 / 60.0;
        draw_you(&mut state);
        told(&mut state, false);
        assert_eq!(state.shells_board_top_at(height), None);
        state.act(Action::Accept, None);
        assert_eq!(state.typing_with_the_shell(), Some(FieldId::FullName));
        // Nothing taken until the toolkit has heard the field take text.
        assert_eq!(state.shells_board_top_at(height), None);
        // Rising: part of the way on the first frame, there in the end.
        told(&mut state, true);
        let first = state.shells_board_top_at(height).expect("no room taken");
        assert!(first > top && first < height, "{first}");
        for _ in 0..120 {
            state.shells_board_top_at(height);
        }
        assert_eq!(state.shells_board_top_at(height), Some(top));
        // Put away with its own hide key: the field still takes text, and
        // the room is given back from that frame on, not the pad's next.
        told(&mut state, false);
        assert_eq!(state.typing_with_the_shell(), Some(FieldId::FullName));
        let going = state.shells_board_top_at(height).expect("gone at once");
        assert!(going > top, "the room stayed where the board was: {going}");
        for _ in 0..120 {
            state.shells_board_top_at(height);
        }
        assert_eq!(state.shells_board_top_at(height), None);
        // On low-end hardware it is there at once, and gone at once.
        state.pretend_low_end();
        told(&mut state, true);
        assert_eq!(state.shells_board_top_at(height), Some(top));
        told(&mut state, false);
        assert_eq!(state.shells_board_top_at(height), None);
    }

    /// A field chosen by a click or a tap — the toolkit puts the pad down
    /// for both — takes text the keyboard's way, and under LineXinBar with a
    /// controller in the shell's hands the shell's board rises over it all
    /// the same. Whatever chose the field, the toolkit's word decides: the
    /// room ends over the board, and You's form lets go of the pointer's
    /// hold to scroll the field above it — the administrator's password,
    /// low on the form, stood under the board.
    #[test]
    fn a_field_clicked_or_tapped_stands_above_the_shells_board() {
        use screens::account::{Focus, Item};
        let height = 800.0;
        let top = shells_board::rect(1280.0, height)[1];
        for chosen_by in ["a pad's South", "a click", "a tap"] {
            let mut state = State::demo(true);
            state.type_with_the_shells_board(true);
            state.stage("account-administrator").expect("You");
            state.pretend_low_end();
            let field = FieldId::RootPassword;
            state.views.account.focus = Focus::Item(Item::Field(field));
            if chosen_by == "a pad's South" {
                state.pad = true;
                draw_you(&mut state);
                state.act(Action::Accept, None);
            } else {
                // The pointer or the finger lit it and holds the form.
                state.pad = false;
                state.hover(field.spot());
                state.clicked_field(field);
            }
            draw_you(&mut state);
            assert_eq!(state.typing_into(), Some(field), "{chosen_by}");
            told(&mut state, true);
            assert_eq!(
                state.shells_board_top_at(height),
                Some(top),
                "no room over the board for a field chosen by {chosen_by}"
            );
            assert!(
                !state.views.account.held_by_the_pointer(),
                "the form held {chosen_by}'s field under the board"
            );
        }
    }

    /// Put away while a field takes text, the shell's board is asked for
    /// again by the legend's Type however it is pressed — the pad's South,
    /// a click or a tap on the pair, which the toolkit counts as the pointer
    /// in hand — and by a click or a tap on the field itself. A field the
    /// board never rose over asks nothing of a click.
    #[test]
    fn the_shells_board_put_away_is_asked_for_again_by_pad_click_or_tap() {
        for how in ["South", "the pair clicked", "the pair tapped"] {
            let mut state = State::demo(true);
            state.type_with_the_shells_board(true);
            state.go(Screen::Account);
            state.pad = true;
            state.pad_pressed = true;
            draw_you(&mut state);
            state.act(Action::Accept, None);
            draw_you(&mut state);
            told(&mut state, true);
            told(&mut state, false);
            assert!(!state.summon_again);
            if how == "South" {
                state.act(Action::Accept, None);
            } else {
                state.pad_pressed = false;
                state.press_pair(legend::Button::Accept);
            }
            assert!(state.summon_again, "{how} did not ask for the board again");
            assert_eq!(state.typing_with_the_shell(), Some(FieldId::FullName));
        }

        let mut state = State::demo(true);
        state.type_with_the_shells_board(true);
        state.go(Screen::Account);
        state.pad = false;
        draw_you(&mut state);
        state.clicked_field(FieldId::FullName);
        assert!(
            !state.summon_again,
            "a click on a field with no board asked"
        );
        told(&mut state, true);
        draw_you(&mut state);
        told(&mut state, false);
        draw_you(&mut state);
        state.clicked_field(FieldId::FullName);
        assert!(
            state.summon_again,
            "a click did not ask for the board again"
        );
    }

    /// A key arriving once the board is known to be away is a keyboard of
    /// the person's own: it lets the pad go, and the legend turns to the
    /// keyboard's pictures. The key that arrives with the board going is
    /// still the board's.
    #[test]
    fn a_key_after_the_shells_board_went_is_the_persons_own() {
        let mut state = State::demo(true);
        state.type_with_the_shells_board(true);
        state.go(Screen::Account);
        state.pad = true;
        state.pad_pressed = true;
        draw_you(&mut state);
        state.act(Action::Accept, None);
        state.pad_pressed = false;
        told(&mut state, true);
        told(&mut state, true);
        state.arrived(false, true, false);
        assert!(state.pad, "the board's own key let the pad go");
        told(&mut state, false);
        state.arrived(false, true, false);
        assert!(state.pad, "the key the board went with let the pad go");
        told(&mut state, false);
        state.arrived(false, true, false);
        assert!(!state.pad, "a keyboard's key after the board kept the pad");
    }

    /// Anywhere but LineXinBar — the GNOME live session — a pad's Accept on
    /// a field raises the installer's own board, as it always did, on You
    /// and on Keyboard's test field alike.
    #[test]
    fn elsewhere_a_pad_types_with_the_installers_own_board() {
        let mut state = State::demo(true);
        assert!(
            !state.shells_board(),
            "a picture or a test asked the session"
        );
        state.go(Screen::Account);
        state.pad = true;
        state.act(Action::Accept, None);
        assert_eq!(
            state.board.as_ref().map(|board| board.field),
            Some(FieldId::FullName)
        );
        assert_eq!(state.typing_with_the_shell(), None);

        for shell in [false, true] {
            let mut state = State::demo(true);
            state.type_with_the_shells_board(shell);
            state.go(Screen::Keyboard);
            state.views.keyboard.focus = screens::keyboard::Focus::Field;
            state.pad = true;
            state.act(Action::Accept, None);
            assert_eq!(state.board.is_some(), !shell, "shell {shell}");
            let shells = state.typing_with_the_shell();
            assert_eq!(shells, shell.then_some(FieldId::KeyboardTest));
        }
    }

    #[test]
    fn an_installation_keeps_the_screen_lit_as_well_as_the_machine_awake() {
        let mut state = State::demo(true);
        state.stage("review").expect("review");
        assert_eq!(state.kept_awake(), Hold::NOTHING, "nothing held before");
        state.stage("installing").expect("installing");
        assert_eq!(state.kept_awake(), Hold::SCREEN_AND_SLEEP);
        state.stage("done").expect("done");
        assert_eq!(state.kept_awake(), Hold::NOTHING, "let go once it is done");
    }

    #[test]
    fn a_page_changes_with_motion_and_at_once_on_low_end_hardware() {
        for low_end in [false, true] {
            let mut state = State::demo(true);
            if low_end {
                state.pretend_low_end();
            }
            state.go(Screen::Language);
            // The frame draws Language, then a press goes on to Time zone.
            state.notice_a_change();
            assert!(!state.change.moving(), "the first page drawn came in");
            state.act(Action::Accept, None);
            assert_eq!(state.screen, Screen::Timezone);
            state.notice_a_change();
            assert_eq!(
                state.change.moving(),
                !low_end,
                "low-end {low_end}: a step changed {}",
                if low_end { "with motion" } else { "at once" }
            );
            let going = state.change.leaving().map(|(page, _)| page.page.screen);
            assert_eq!(going, (!low_end).then_some(Screen::Language));
            // The same page drawn again begins nothing.
            state.change.settle();
            state.notice_a_change();
            assert!(!state.change.moving(), "a page that stayed changed");
        }
    }

    #[test]
    fn on_low_end_hardware_a_press_that_changes_the_page_dips_nothing_on_the_next() {
        let mut state = State::demo(true);
        state.pretend_low_end();
        state.go(Screen::Language);
        state.notice_a_change();
        state.pressing.press();
        state.act(Action::Accept, None);
        assert_eq!(state.screen, Screen::Timezone);
        state.notice_a_change();
        assert_eq!(state.pressing.through(), None, "Time zone's row dipped");
    }

    #[test]
    fn a_press_dips_the_button_it_landed_on_and_goes_with_its_page() {
        let mut state = State::demo(true);
        state.go(Screen::Welcome);
        state.notice_a_change();
        // Accept, as the frame answers it: the press begins, then the page
        // it was pressed on goes.
        state.pressing.press();
        state.act(Action::Accept, None);
        assert_eq!(state.screen, Screen::Language);
        state.notice_a_change();
        assert_eq!(
            state.pressing.through(),
            None,
            "the press dipped the row the light came in on, on Language"
        );
        assert_eq!(
            state.change.leaving_pressing().through(),
            Some(0.0),
            "Get started, pressed, did not dip as Welcome went"
        );
        // Back at once: Welcome is called back with its press still under
        // way, and Language, which nothing pressed, goes without one.
        state.change.leaving_pressing().advance(FRAME);
        state.change.advance(FRAME);
        state.act(Action::Back, None);
        assert_eq!(state.screen, Screen::Welcome);
        state.notice_a_change();
        assert!(
            state.pressing.through().is_some(),
            "Welcome's press was lost"
        );
        assert_eq!(state.change.leaving_pressing().through(), None);
        // And once the change is over, nothing is left dipping.
        state.change.settle();
        assert_eq!(state.change.leaving_pressing().through(), None);
    }

    /// What the frame does with the change and the chips each frame, without
    /// drawing anything (`State::draw`): notice the page, carry both on.
    fn a_frame_of_motion(state: &mut State) {
        state.notice_a_change();
        state.change.advance(FRAME);
        state.chips.aim(state.screen.place());
        state.chips.advance(FRAME);
    }

    #[test]
    fn a_step_settles_within_a_second_as_the_shells_own_step_does_and_is_then_still() {
        let mut state = State::demo(true);
        state.go(Screen::Timezone);
        a_frame_of_motion(&mut state);
        state.settle();
        for press in [Action::Accept, Action::Back] {
            match press {
                Action::Accept => state.forward(),
                _ => state.back(),
            }
            let mut frames = 0;
            loop {
                a_frame_of_motion(&mut state);
                frames += 1;
                if !state.change.moving() && !state.chips.moving() {
                    break;
                }
                // The shell's step into a subcategory, on the same spring
                // and the same `SETTLED`, takes 52 frames.
                assert!(frames < 60, "{press:?}: still moving after a second");
            }
            assert!(frames > 8, "{press:?}: settled in {frames} frames, a jump");
            // Still: frame after frame begins nothing and moves nothing.
            for _ in 0..120 {
                a_frame_of_motion(&mut state);
                assert!(!state.change.moving() && !state.chips.moving());
                assert!(state.change.arriving().standing());
            }
        }
    }

    #[test]
    fn continue_walks_the_questions_and_stops_at_the_review() {
        let mut at = Screen::Welcome;
        let mut seen = vec![at];
        while let Some(next) = at.next() {
            at = next;
            seen.push(at);
        }
        assert_eq!(seen, Screen::ASKING);
        assert_eq!(Screen::Installing.next(), None, "an installation walked on");
    }

    #[test]
    fn back_walks_back_and_never_out_of_an_installation() {
        assert_eq!(Screen::Language.previous(), Some(Screen::Welcome));
        assert_eq!(Screen::Review.previous(), Some(Screen::Account));
        assert_eq!(Screen::Welcome.previous(), None);
        for after in [Screen::Installing, Screen::Done, Screen::Stopped] {
            assert_eq!(after.previous(), None, "{after:?} walked back");
        }
    }

    #[test]
    fn the_chips_count_the_questions_after_welcome() {
        assert_eq!(Screen::Welcome.place(), None);
        assert_eq!(
            Screen::Language.place(),
            Some(panel::Place { place: 0, of: 7 })
        );
        assert_eq!(
            Screen::Review.place(),
            Some(panel::Place { place: 6, of: 7 })
        );
        assert_eq!(Screen::Done.place(), None);
    }

    #[test]
    fn a_draft_starts_on_the_first_disk_that_can_be_chosen_erased_where_it_holds_nothing() {
        let disks = demo::disks();
        let draft = draft(&disks, DEFAULT_SOURCE, Firmware::Uefi);
        assert_eq!(draft.disk.serial, "DEMO-NVME-0001");
        assert_eq!(draft.target, Target::WholeDisk);
        assert_eq!(draft.account.hostname, DEFAULT_HOSTNAME);
        assert_eq!(draft.source, std::path::PathBuf::from(DEFAULT_SOURCE));
        assert_eq!(draft.storage, Storage::default());
    }

    #[test]
    fn a_replaced_part_is_named_with_its_own_size_never_the_disks() {
        let mut state = State::demo(true);
        let disk = demo::disks().into_iter().nth(1).unwrap();
        let part = disk
            .partitions
            .iter()
            .find(|part| part.content == crate::model::disks::Content::Windows)
            .unwrap()
            .clone();
        state.draft.disk = disk.clone();
        state.draft.target = Target::Partition {
            path: part.path.clone(),
        };
        let said = state.erase_words().replace('\u{a0}', " ");
        assert!(said.contains(&words::size(part.size)), "{said}");
        assert!(said.contains(&words::disk_called(&disk)), "{said}");
        assert!(!said.contains(&words::size(disk.size)), "{said}");
    }

    #[test]
    fn a_disk_that_holds_something_starts_on_its_free_space_never_erased() {
        // The Windows disk first: its 300 GB gap, never the whole disk.
        let disks: Vec<Disk> = demo::disks().into_iter().skip(1).collect();
        let gap = disks[0].free[0];
        for firmware in [Firmware::Uefi, Firmware::Bios] {
            let draft = draft(&disks, DEFAULT_SOURCE, firmware);
            assert_eq!(draft.disk.serial, "DEMO-SATA-0002");
            assert_eq!(
                draft.target,
                Target::FreeSpace {
                    start: gap.start,
                    size: gap.size
                },
                "{firmware:?}"
            );
            assert!(disks::validate_storage(&draft, firmware).is_ok());
        }
    }

    #[test]
    fn a_disk_that_holds_something_without_room_starts_with_nothing_chosen() {
        // The Windows disk with its gap shrunk to 20 GB, less than Linexin
        // needs, and with no gap at all: nothing on it is chosen, so nothing
        // can be installed until the person chooses.
        let mut disks: Vec<Disk> = demo::disks().into_iter().skip(1).collect();
        disks[0].free[0].size = 20_000 * MIB;
        for free in [disks[0].free.clone(), Vec::new()] {
            disks[0].free = free;
            let draft = draft(&disks, DEFAULT_SOURCE, Firmware::Uefi);
            assert!(draft.disk.path.is_empty(), "{:?}", draft.target);
            assert!(disks::validate_storage(&draft, Firmware::Uefi).is_err());
        }
    }

    #[test]
    fn with_nothing_to_choose_the_draft_cannot_be_installed() {
        let mut stick = demo::disks().remove(2);
        stick.blocked = Some(Blocked::InstallationMedia);
        let draft = draft(&[stick], DEFAULT_SOURCE, Firmware::Uefi);
        assert!(draft.disk.blocked.is_some());
        assert!(draft.disk.path.is_empty());
    }

    #[test]
    fn a_double_click_on_continue_answers_one_question() {
        let mut state = State::demo(true);
        state.go(Screen::Language);
        let next = Screen::Language.next().unwrap();
        state.clock.tick(10.0);
        // Continue clicked: on to the next question, whose Continue stands
        // where the pointer rests.
        state.click(State::forward);
        assert_eq!(state.screen, next);
        // The second half of the double-click.
        state.clock.tick(10.25);
        state.click(State::forward);
        assert_eq!(state.screen, next, "a double-click went past a question");
        // A click once the page has been there a while is a click.
        state.clock.tick(10.6);
        state.click(State::forward);
        assert_eq!(state.screen, next.next().unwrap());
        // A click that changes nothing under the pointer holds nothing back:
        // the board's keys, a switch and a field take clicks in a row.
        state.clock.tick(20.0);
        state.click(|state| state.say("one", Role::Text));
        state.clock.tick(20.1);
        state.click(State::forward);
        assert_eq!(state.screen, next.next().unwrap().next().unwrap());
    }

    #[test]
    fn a_demo_rehearses_and_ends_on_done() {
        let mut state = State::demo(true);
        state.go(Screen::Review);
        state.ask_to_install();
        assert_eq!(state.question, Some(Question::Erase));
        state.question = None;
        state.answer(Question::Erase, 1);
        assert_eq!(state.screen, Screen::Installing);
        assert!(state.events.is_none(), "a demo started a worker");
        for second in 0..60 {
            state.clock.tick(second as f32);
            state.follow();
        }
        assert_eq!(state.screen, Screen::Done);
    }

    /// `--demo` and every `--shot` are `Machine::Demo`, whose `begin` makes a
    /// rehearsal and never calls `launch::start` — the one place a worker is
    /// started. Walked here from every scene with every press, every
    /// question answered with its dangerous answer and `begin` called
    /// outright as well: the demo installs (rehearses) and no worker's
    /// events are ever followed.
    #[test]
    fn a_demo_never_starts_a_worker_whatever_is_pressed() {
        let presses = [
            Action::Accept,
            Action::Down,
            Action::Accept,
            Action::Right,
            Action::Accept,
            Action::Menu,
            Action::Accept,
            Action::Up,
            Action::Left,
            Action::Accept,
            Action::Back,
        ];
        let mut rehearsed = 0;
        for scene in SCENES {
            let mut state = State::demo(true);
            state.stage(scene).unwrap();
            let mut now = 0.0;
            for action in presses.iter().cycle().take(60) {
                if let Some(question) = state.question.take() {
                    let question = put_up(&mut state, question);
                    state.answer(question, 1);
                }
                state.menu = None;
                state.act(*action, None);
                now += 0.5;
                state.clock.tick(now);
                state.follow();
                assert!(state.events.is_none(), "{scene}: a demo started a worker");
                if state.rehearsal.is_some() {
                    rehearsed += 1;
                }
            }
            // `begin` goes on for the last question as the frame would put
            // it up — under Partitions' plan the plan's, and only once it
            // has been read to its last page.
            if screens::partitions::in_use(&state) {
                read_the_plan_question(&mut state);
            }
            state.begin(state.put_as(Question::Erase));
            assert!(state.events.is_none(), "{scene}: begin started a worker");
            assert!(state.rehearsal.is_some(), "{scene}: begin did not rehearse");
        }
        assert!(rehearsed > 0, "no press ever got as far as an installation");
    }

    /// `--demo` and every `--shot` are `Machine::Demo`, whose numbers are
    /// its own (`demo::measures`): the measure entry is never asked —
    /// `State::measure` is never called — whatever is pressed, from every
    /// scene, every question answered with its dangerous answer, Partitions
    /// opened outright, the disks listed anew and the frame's look for an
    /// answer taken after every press.
    #[test]
    fn a_demo_never_measures_whatever_is_pressed() {
        MEASURED.with(|count| count.set(0));
        let presses = [
            Action::Accept,
            Action::Down,
            Action::Accept,
            Action::Right,
            Action::Accept,
            Action::Menu,
            Action::Accept,
            Action::Up,
            Action::Left,
            Action::Accept,
            Action::Back,
        ];
        let mut opened = 0;
        for scene in SCENES {
            let mut state = State::demo(true);
            state.measure = measured_and_counted;
            state.stage(scene).unwrap();
            for action in presses.iter().cycle().take(60) {
                if let Some(question) = state.question.take() {
                    let question = put_up(&mut state, question);
                    state.answer(question, 1);
                }
                state.menu = None;
                state.act(*action, None);
                screens::partitions::poll(&mut state);
                if state.views.target.part == screens::target::Part::Partitions {
                    opened += 1;
                }
            }
            state.go(Screen::Target);
            screens::target::open_partitions(&mut state);
            let mut fewer = state.disks.clone();
            fewer.pop();
            screens::partitions::disks_listed(&mut state, fewer);
            screens::target::open_partitions(&mut state);
            screens::partitions::poll(&mut state);
            assert_eq!(
                MEASURED.with(|count| count.get()),
                0,
                "{scene}: a demo measured"
            );
        }
        assert!(opened > 0, "no press ever opened Partitions");
    }

    /// A dialog of thirty lines with ten points to a letter, 600 wide: what
    /// a test puts a question up on, with no window.
    fn dialog_of_a_test(
        state: &mut State,
        question: Question,
        most: usize,
    ) -> (String, String, Vec<String>) {
        state.dialog(question, most, 600.0, |line| {
            line.chars().count() as f32 * 10.0
        })
    }

    /// `question` put up as the frame puts it up (`raise`), on a test's
    /// dialog of thirty lines: the question it becomes, which is the one
    /// answered.
    fn put_up(state: &mut State, question: Question) -> Question {
        let question = state.put_as(question);
        dialog_of_a_test(state, question, 30);
        question
    }

    /// The plan's question put up page by page, from its first to its last,
    /// as Next walks it, on a test's dialog of `most` lines; its pages.
    fn read_the_plan_question_in(state: &mut State, most: usize) -> Vec<String> {
        let mut pages = Vec::new();
        let mut page = 0;
        loop {
            let (_, body, _) = dialog_of_a_test(state, Question::PlanErase { page }, most);
            pages.push(body);
            if !state.more_pages_after(page) {
                return pages;
            }
            page += 1;
        }
    }

    fn read_the_plan_question(state: &mut State) -> Vec<String> {
        read_the_plan_question_in(state, 30)
    }

    /// A state following a worker whose channel holds `events` and has
    /// ended, as it is when the worker has said them and exited between two
    /// frames.
    fn following(events: Vec<Event>) -> State {
        let mut state = State::demo(true);
        state.go(Screen::Installing);
        let (tx, rx) = std::sync::mpsc::channel();
        for event in events {
            tx.send(event).unwrap();
        }
        drop(tx);
        state.events = Some(rx);
        state
    }

    #[test]
    fn a_last_word_and_the_end_of_the_worker_in_one_frame_is_done() {
        let mut state = following(vec![
            Event::Plan {
                steps: vec![Step::Copy, Step::Finish],
            },
            Event::Begin { step: Step::Finish },
            Event::Finished,
        ]);
        state.follow();
        assert_eq!(state.screen, Screen::Done);
        assert!(state.install.finished());
        assert!(state.events.is_none());
    }

    #[test]
    fn however_an_installation_ends_the_screen_and_the_machine_are_let_go() {
        let failed = Event::Failed {
            step: Some(Step::Copy),
            failure: Failure::Internal,
            detail: "the disk went away".into(),
        };
        for (events, ended) in [
            (
                vec![
                    Event::Plan {
                        steps: vec![Step::Copy, Step::Finish],
                    },
                    Event::Begin { step: Step::Finish },
                    Event::Finished,
                ],
                Screen::Done,
            ),
            (
                vec![Event::Begin { step: Step::Copy }, failed],
                Screen::Stopped,
            ),
            // The worker gone without a last word stops it too.
            (vec![Event::Begin { step: Step::Copy }], Screen::Stopped),
        ] {
            let mut state = following(events);
            assert_eq!(state.kept_awake(), Hold::SCREEN_AND_SLEEP, "not held");
            // The frame follows the worker before it says what it holds.
            state.follow();
            assert_eq!(state.screen, ended);
            assert_eq!(state.kept_awake(), Hold::NOTHING, "still held on {ended:?}");
        }
    }

    #[test]
    fn a_worker_gone_without_a_last_word_has_stopped() {
        let mut state = following(vec![Event::Begin { step: Step::Copy }]);
        state.follow();
        assert_eq!(state.screen, Screen::Stopped);
        assert!(matches!(
            state.install.ending,
            Some(Ending::Failed {
                failure: Failure::Internal,
                ..
            })
        ));
    }

    #[test]
    fn the_safe_answer_is_always_the_first() {
        let mut state = State::demo(true);
        let (_, _, quit) = state.question_words(Question::Quit);
        assert_eq!(quit[0], i18n::text_in(i18n::language(), "quit-stay"));
        let (_, _, erase) = state.question_words(Question::Erase);
        assert_eq!(erase[0], i18n::text_in(i18n::language(), "erase-back"));
        screens::partitions::stage_plan(&mut state);
        state.views.partitions.in_use = true;
        for page in [0, 1, usize::MAX] {
            let (_, _, answers) = dialog_of_a_test(&mut state, Question::PlanErase { page }, 6);
            assert_eq!(answers[0], i18n::text("erase-back"), "page {page}");
        }
    }

    /// A live system, as far as the interface can tell — the made-up
    /// machine with a live session it was never started from, judged as
    /// UEFI without reading this computer — with Partitions' whole plan the
    /// choice in use and an account typed in. The disks it lists again
    /// before the plan's question are the made-up machine's own, unchanged
    /// (`list_disks`): no test lists this computer's; and what it measures
    /// is the test's own (`measure`, a measurement that answers nothing).
    /// `launch::start` and `launch::measure` ask their own guard
    /// (`/run/archiso/airootfs`) and, in a test build, would run a polkit
    /// that cannot exist; none of it is ever reached here.
    fn live_with_the_plan_in_use() -> State {
        let mut state = State::demo(true);
        screens::partitions::stage_plan(&mut state);
        state.views.partitions.in_use = true;
        screens::review::stage(&mut state);
        state.machine = Machine::Real {
            live: Some(LiveSession {
                image: "/run/archiso/airootfs".into(),
                medium_uuid: None,
                medium_label: Some("LINEXIN".into()),
                medium_mounted: true,
            }),
        };
        state.pretend_firmware(Firmware::Uefi);
        state.list_disks = the_made_up_disks;
        state.measure = measured_and_counted;
        state
    }

    thread_local! {
        /// How many measurements this thread's tests have asked for
        /// (`measured_and_counted`).
        static MEASURED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    /// A measurement of a test's own: it counts that it was asked for, and
    /// its end is let go of at once, so it comes to nothing read.
    fn measured_and_counted(_: Vec<Disk>) -> Receiver<MeasureEvent> {
        MEASURED.with(|count| count.set(count.get() + 1));
        std::sync::mpsc::channel().1
    }

    /// A plan on the made-up machine's Windows disk that asks for nothing a
    /// live system would have to read first (no shrink): Data deleted and
    /// ext4 at `/` made in its place, the recovery partition formatted FAT32
    /// for `/boot`, the EFI partition kept at `/efi` — so what refuses it on
    /// a live system is what this version cannot carry out.
    fn plan_with_nothing_to_read(state: &mut State) {
        use crate::ui::partitions::{apply, Fs, Mount, NewPart, Plan};
        let disk = state.disks[1].clone();
        let firmware = state.firmware();
        let plan = &mut state.views.partitions.plan;
        *plan = Plan::default();
        plan.delete(&disk.path, "/dev/sda5");
        let room = apply(plan, &disk, firmware)
            .disk
            .free
            .last()
            .copied()
            .unwrap();
        plan.create(
            &disk.path,
            NewPart {
                number: 1,
                start: room.start,
                size: room.size,
                fs: Fs::Ext4,
                mount: Some(Mount::Root),
                label: String::new(),
            },
        );
        plan.mount(&disk.path, "/dev/sda1", Some(Mount::Efi));
        plan.format(&disk.path, "/dev/sda4", Some(Fs::Fat32));
        plan.mount(&disk.path, "/dev/sda4", Some(Mount::Boot));
    }

    /// The made-up machine's disks, as a live system would list them again
    /// with nothing changed.
    fn the_made_up_disks() -> Result<Vec<Disk>, String> {
        Ok(demo::disks())
    }

    /// The made-up machine's disks with the Windows disk's logical sector
    /// 4096 bytes rather than 512, as a disk of 4096-byte sectors would be
    /// listed; everything on it is whole MiB, so it holds the same.
    fn the_made_up_disks_on_large_sectors() -> Result<Vec<Disk>, String> {
        let mut disks = demo::disks();
        let sda = disks.iter_mut().find(|disk| disk.path == "/dev/sda");
        sda.expect("the Windows disk").logical_sector = 4096;
        Ok(disks)
    }

    /// The made-up machine's disks with the Windows disk's Data partition
    /// a MiB smaller, as if resized since they were first listed.
    fn the_made_up_disks_changed() -> Result<Vec<Disk>, String> {
        let mut disks = demo::disks();
        let sda = disks.iter_mut().find(|disk| disk.path == "/dev/sda");
        sda.expect("the Windows disk").partitions[4].size -= MIB;
        Ok(disks)
    }

    /// Disks that cannot be read again.
    fn no_disks_to_read() -> Result<Vec<Disk>, String> {
        Err("a test reads no disks".into())
    }

    /// Off the demo a test is given its firmware: without one, `firmware`
    /// would read this computer's, which the fence refuses in a test build
    /// (`run::fence::firmware`), so no test's answer is a fact about the
    /// computer running it.
    #[test]
    fn off_the_demo_a_test_is_given_its_firmware() {
        let mut state = State::demo(true);
        assert_eq!(state.firmware(), Firmware::Uefi, "the made-up machine's");
        state.machine = Machine::Real { live: None };
        let read = std::panic::AssertUnwindSafe(|| state.firmware());
        let words =
            crate::worker::run::fence::refusal(read).expect("this computer's firmware was read");
        assert!(words.contains("test build:"), "{words}");
        state.pretend_firmware(Firmware::Bios);
        assert_eq!(state.firmware(), Firmware::Bios);
    }

    /// The safety boundary's defence in depth (Unit P, Q2): with Partitions'
    /// plan the choice in use on a live system a request is built, the plan
    /// as a layout, and handed to nothing while what it needs is not proven.
    /// This plan (Data deleted and `/` made in its place, the recovery
    /// partition formatted for `/boot`) lies within a set that passed
    /// (`manual-uefi-beside-windows`'), so `layout::PROVEN` takes it.
    /// Pretended proven by nothing, as were no set written, Install is
    /// refused in words with the first thing the plan asks, read to its last
    /// page the question's Erase goes nowhere, `begin` called outright asks
    /// for no worker — and the worker refuses the very request it would be
    /// handed. In the demo the plan's question is asked, and the rehearsal
    /// runs once it has been read to its last page.
    #[test]
    fn a_plan_request_is_refused_until_its_capabilities_are_proven() {
        let mut state = live_with_the_plan_in_use();
        // The whole plan shrinks Windows, which nothing has read off the
        // demo: refused for that first.
        let unread = state.ready().expect_err("refused");
        let missing = screens::partitions::missing_words(&state);
        assert_eq!(Some(unread), missing);
        plan_with_nothing_to_read(&mut state);
        assert_eq!(screens::partitions::missing(&state), None);
        assert!(state.can_install(), "a live session can install");
        let request = state.request().expect("a request is built");
        let Target::Layout { disks: laid } = &request.config.target else {
            panic!("the request does not carry the plan");
        };
        let needs = layout::needs(laid, Firmware::Uefi);
        assert!(!needs.is_empty());
        // What passed takes it.
        assert_eq!(layout::unproven(laid, Firmware::Uefi), None);
        assert_eq!(state.ready(), Ok(()));
        // Were no set written, it would be refused, here and in the worker.
        let none = layout::PretendProven::these(&[]);
        let first = layout::unproven(laid, Firmware::Uefi).expect("something unproven");
        assert_eq!(first, layout::Unproven::Never(needs[0]));
        let refused = crate::ui::partitions::unproven_words(&first);
        assert_eq!(refused, i18n::text("partitions-not-yet-keep-table"));
        assert_eq!(state.ready().err().as_deref(), Some(refused));
        state.go(Screen::Review);
        state.ask_to_install();
        assert_eq!(state.question, None, "the last question was asked");
        let said = state.note.as_ref().map(|note| note.text.as_str());
        assert_eq!(said, Some(refused));
        // Its pages read whole and Erase pressed on the last, or `begin`
        // called outright: no worker.
        let pages = read_the_plan_question(&mut state).len();
        state.answer(Question::PlanErase { page: pages - 1 }, 1);
        assert!(state.events.is_none(), "a worker was asked for");
        read_the_plan_question(&mut state);
        state.begin(Question::PlanErase { page: pages - 1 });
        assert!(state.events.is_none(), "a worker was asked for");
        assert!(state.rehearsal.is_none());
        assert_eq!(state.screen, Screen::Review);
        // The worker refuses the request it would have been handed, read
        // from the bytes the interface writes.
        let bytes = serde_json::to_vec(&request).unwrap();
        let read = crate::worker::system::check::read_request(bytes.as_slice())
            .unwrap_or_else(|stop| panic!("{:?}: {}", stop.failure, stop.detail));
        let stop = crate::worker::system::check::validate(&read, Firmware::Uefi)
            .expect_err("the worker took a layout");
        assert_eq!(stop.failure, Failure::InvalidRequest);
        assert_eq!(stop.detail, first.line());
        drop(none);
        // Let go of, the place is the choice again, its request the draft's.
        screens::partitions::let_go_of(&mut state);
        let easy = state.request().expect("the place's request");
        assert_eq!(easy.config.target, state.draft.target);
        assert_eq!(easy.consent, disks::consent(&state.draft));
        // In the demo, the plan in use: its question is asked, and Erase on
        // its last page rehearses, with no worker.
        let mut state = State::demo(true);
        screens::partitions::stage_plan(&mut state);
        state.views.partitions.in_use = true;
        state.go(Screen::Review);
        state.ask_to_install();
        assert_eq!(state.question, Some(Question::PlanErase { page: 0 }));
        state.question = None;
        state.begin(Question::PlanErase { page: 0 });
        assert!(
            state.rehearsal.is_none(),
            "rehearsed before the pages were read"
        );
        let pages = read_the_plan_question(&mut state).len();
        state.answer(Question::PlanErase { page: pages - 1 }, 1);
        assert_eq!(state.screen, Screen::Installing);
        assert!(state.events.is_none(), "the demo asked for a worker");
        assert!(state.rehearsal.is_some(), "the demo did not rehearse");
    }

    /// **A proven plan on a disk of 4096-byte sectors stays where it is**
    /// (decision 20: QEMU gave every scenario's disks 512-byte sectors). The
    /// plan that goes on by `manual-uefi-beside-windows`' set
    /// ([`plan_with_nothing_to_read`]), made the same on the made-up Windows
    /// disk listed with 4096-byte sectors, needs that set and `LargeSectors`,
    /// which no set holds: on a live system Continue on Where to install
    /// keeps it, saying "Partitions can't be applied in this version yet.",
    /// and in use it is refused in the same words by `plan_judged` and by
    /// Install (`ready`); and the worker refuses the very bytes the interface
    /// would write for it. Given `LargeSectors` beside every set
    /// (`layout::PretendProven`), it goes on and is ready: nothing but the
    /// disk's sectors refuses it.
    #[test]
    fn a_proven_plan_on_a_disk_of_large_sectors_stays_where_it_is() {
        use crate::model::layout::{Capability, PretendProven};
        use crate::worker::system::check;
        use Capability::{ChangeEntries, DeleteEntries, Gpt, KeepTable, LargeSectors, Uefi};
        let mut state = live_with_the_plan_in_use();
        state.disks = the_made_up_disks_on_large_sectors().unwrap();
        state.list_disks = the_made_up_disks_on_large_sectors;
        plan_with_nothing_to_read(&mut state);
        assert_eq!(state.disks[1].path, "/dev/sda");
        assert_eq!(state.disks[1].logical_sector, 4096);
        assert_eq!(screens::partitions::missing(&state), None);
        let request = state.request().expect("a request is built");
        let Target::Layout { disks: laid } = &request.config.target else {
            panic!("the request does not carry the plan");
        };
        assert_eq!(
            layout::needs(laid, Firmware::Uefi),
            [
                KeepTable,
                DeleteEntries,
                ChangeEntries,
                Uefi,
                Gpt,
                LargeSectors
            ]
        );
        assert_eq!(
            layout::unproven(laid, Firmware::Uefi),
            Some(layout::Unproven::Never(LargeSectors))
        );
        let onward = |state: &mut State| {
            screens::partitions::let_go_of(state);
            state.screen = Screen::Target;
            state.note = None;
            screens::partitions::onward_from_where_to_install(state);
            let said = state.note.as_ref().map(|note| note.text.clone());
            (screens::partitions::in_use(state), state.screen, said)
        };
        let words = i18n::text("partitions-not-yet").to_string();
        assert_eq!(
            onward(&mut state),
            (false, Screen::Target, Some(words.clone()))
        );
        state.views.partitions.in_use = true;
        assert_eq!(state.plan_judged(), Err(words.clone()));
        assert_eq!(state.ready(), Err(words));
        let bytes = serde_json::to_vec(&request).unwrap();
        let read = check::read_request(bytes.as_slice())
            .unwrap_or_else(|stop| panic!("{:?}: {}", stop.failure, stop.detail));
        let stop = check::validate(&read, Firmware::Uefi).expect_err("taken by the worker");
        assert_eq!(stop.detail, layout::Unproven::Never(LargeSectors).line());
        let given: Vec<&'static [Capability]> = layout::PROVEN
            .iter()
            .map(|set| {
                let mut more = set.to_vec();
                more.push(LargeSectors);
                more.sort();
                &*more.leak()
            })
            .collect();
        let _given = PretendProven::these(given.leak());
        let (in_use, screen, said) = onward(&mut state);
        assert!(in_use, "{said:?}");
        assert_eq!(screen, Screen::Target.next().unwrap());
        assert_eq!(state.plan_judged(), Ok(()));
        assert_eq!(state.ready(), Ok(()));
        if let Err(stop) = check::validate(&read, Firmware::Uefi) {
            panic!("{:?}: {}", stop.failure, stop.detail);
        }
    }

    /// **A plan QEMU has proven goes on, and its request is exactly what its
    /// question said** (Unit Q, Q5): on a live system the plan, nothing
    /// missing, goes on from Where to install where everything it needs lies
    /// within one set of `layout::PROVEN` — as this one's does,
    /// `manual-uefi-beside-windows`' — and, pretended otherwise
    /// (`layout::PretendProven`), never with nothing proven (as were no set
    /// written), never with one of its needs left out, never with them proven
    /// only apart by two sets (the foot then says "Partitions can't be
    /// applied in this version yet.", decision 20), and never off the live
    /// medium; in use, it is judged whole (`plan_judged`, and `ready`) — and
    /// refused by sets that prove its two only apart; Install puts its
    /// question up, which, read to the last page as the frame puts it up,
    /// keeps exactly the request's consent, so that "Erase and install" there
    /// would begin (not pressed here: the launch looks at this machine); and
    /// the worker takes the very bytes it would be handed.
    #[test]
    fn a_proven_plan_goes_on_and_its_request_is_what_its_question_said() {
        use crate::model::layout::{Capability, PretendProven};
        use Capability::{ChangeEntries, DeleteEntries, Gpt, KeepTable, Uefi};
        const NEEDS: &[Capability] = &[KeepTable, DeleteEntries, ChangeEntries, Uefi, Gpt];
        let mut state = live_with_the_plan_in_use();
        plan_with_nothing_to_read(&mut state);
        let request = state.request().expect("a request is built");
        let Target::Layout { disks: laid } = &request.config.target else {
            panic!("the request does not carry the plan");
        };
        assert_eq!(layout::needs(laid, Firmware::Uefi), NEEDS);
        assert_eq!(layout::unproven(laid, Firmware::Uefi), None);
        let not_yet = |capability| crate::ui::partitions::capability_words(capability);
        // Continue on Where to install, the plan not yet the choice in use.
        let onward = |state: &mut State| {
            screens::partitions::let_go_of(state);
            state.screen = Screen::Target;
            state.note = None;
            screens::partitions::onward_from_where_to_install(state);
            let said = state.note.as_ref().map(|note| note.text.clone());
            (screens::partitions::in_use(state), state.screen, said)
        };
        // With nothing proven, or one capability left out, it stays.
        {
            let _none = PretendProven::these(&[]);
            let refused = onward(&mut state);
            let first = not_yet(Capability::KeepTable).to_string();
            assert_eq!(refused, (false, Screen::Target, Some(first)));
        }
        {
            let _half = PretendProven::these(&[&[KeepTable, DeleteEntries, Uefi, Gpt]]);
            let refused = onward(&mut state);
            let words = not_yet(Capability::ChangeEntries).to_string();
            assert_eq!(refused, (false, Screen::Target, Some(words)));
        }
        // Each proven, but by two scenarios and never by one: a mix no
        // scenario ran whole, said in Partitions' own sentence.
        {
            let _apart = PretendProven::these(&[
                &[KeepTable, DeleteEntries, Uefi, Gpt],
                &[ChangeEntries, Uefi, Gpt],
            ]);
            let refused = onward(&mut state);
            let words = i18n::text("partitions-not-yet").to_string();
            assert_eq!(refused, (false, Screen::Target, Some(words)));
        }
        // Off the live medium, never, proven as it is.
        {
            let mut off = live_with_the_plan_in_use();
            plan_with_nothing_to_read(&mut off);
            off.machine = Machine::Real { live: None };
            let (in_use, screen, _) = onward(&mut off);
            assert_eq!((in_use, screen), (false, Screen::Target));
        }
        // By what passed, it goes on.
        let (in_use, screen, said) = onward(&mut state);
        assert!(in_use, "{said:?}");
        assert_eq!(screen, Screen::Target.next().unwrap());
        // Judged whole, and ready to install.
        assert_eq!(state.plan_judged(), Ok(()));
        assert_eq!(state.ready(), Ok(()));
        // In use, judged again by sets that prove its two only apart:
        // refused, in Partitions' own sentence.
        {
            let _apart = PretendProven::these(&[
                &[KeepTable, DeleteEntries, Uefi, Gpt],
                &[ChangeEntries, Uefi, Gpt],
            ]);
            let words = i18n::text("partitions-not-yet").to_string();
            assert_eq!(state.plan_judged(), Err(words.clone()));
            assert_eq!(state.ready(), Err(words));
        }
        // The request is the one built before it went on, Install puts its
        // question up, and the question keeps exactly its consent.
        let request = state.request().expect("a request is built");
        assert_eq!(request.consent, disks::consent(&request.config));
        state.go(Screen::Review);
        state.ask_to_install();
        assert_eq!(
            state.question.take(),
            Some(Question::PlanErase { page: 0 }),
            "Install did not put the plan's question up"
        );
        let pages = read_the_plan_question(&mut state).len();
        assert_eq!(state.asked.as_ref(), Some(&request.consent));
        // What `begin` asks before it starts the worker holds: "Erase and
        // install" on the last page would begin. It is not pressed — that
        // would ask `launch::start`, which looks at this machine.
        assert!(state.consent_read(Question::PlanErase { page: pages - 1 }));
        assert_eq!(state.ready(), Ok(()));
        assert!(state.events.is_none(), "a worker was asked for");
        assert_eq!(state.screen, Screen::Review);
        // The worker takes the very bytes the interface would write.
        let bytes = serde_json::to_vec(&request).unwrap();
        let read = crate::worker::system::check::read_request(bytes.as_slice())
            .unwrap_or_else(|stop| panic!("{:?}: {}", stop.failure, stop.detail));
        assert_eq!(disks::consent(&read.config), request.consent);
        if let Err(stop) = crate::worker::system::check::validate(&read, Firmware::Uefi) {
            panic!("{:?}: {}", stop.failure, stop.detail);
        }
    }

    /// With nothing proven — as were no set of the campaign written,
    /// pretended so here (`layout::PretendProven`) — every plan is refused
    /// at Install (Unit Q, Q5): `plan_judged` says the first capability the
    /// plan needs, and `ready` the same.
    #[test]
    fn with_nothing_proven_a_plan_is_refused_at_install() {
        let _none = layout::PretendProven::these(&[]);
        let mut state = live_with_the_plan_in_use();
        plan_with_nothing_to_read(&mut state);
        let words = crate::ui::partitions::capability_words(layout::Capability::KeepTable);
        assert_eq!(state.plan_judged().err().as_deref(), Some(words));
        assert_eq!(state.ready().err().as_deref(), Some(words));
    }

    /// With Partitions' plan in use the request carries the plan and nothing
    /// of the place the draft still holds — however far from the plan it is
    /// (another disk, erased whole, a /home kept on another, Btrfs): its
    /// target the layout, its disk the one that holds the plan's `/`, no
    /// separate /home, `/`'s filesystem, the swap file Options set (the plan
    /// uses no swap partition); everything else the draft's. Its consent is
    /// every line of what the plan destroys, and the draft's disk is named
    /// nowhere in it.
    #[test]
    fn a_plan_request_carries_no_easy_place() {
        use crate::model::config::{Filesystem, Home};
        let mut state = live_with_the_plan_in_use();
        let nvme = state.disks[0].clone();
        let sda = state.disks[1].clone();
        state.draft.disk = nvme.clone();
        state.draft.target = Target::WholeDisk;
        state.draft.storage = Storage {
            filesystem: Filesystem::Btrfs,
            swap_mib: 8192,
            home: Some(Home {
                path: sda.partitions[4].path.clone(),
                format: false,
                disk: Some(sda.clone()),
            }),
        };
        let request = state.request().expect("a request is built");
        let config = &request.config;
        let Target::Layout { disks: laid } = &config.target else {
            panic!("the request does not carry the plan");
        };
        assert_eq!(laid.len(), 1);
        assert_eq!(config.disk, sda, "the disk that holds /");
        assert_eq!(config.storage.home, None);
        assert_eq!(config.storage.filesystem, Filesystem::Ext4);
        assert_eq!(config.storage.swap_mib, 8192);
        assert_eq!(layout::pinned(config), Ok(()));
        let lines: Vec<String> = layout::destroys(laid)
            .iter()
            .map(layout::Destroy::line)
            .collect();
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert_eq!(request.consent, lines);
        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains(&nvme.path), "the draft's disk is in it");
        assert!(!json.contains("whole-disk"), "the draft's place is in it");
        assert_eq!(config.region, state.draft.region);
        assert!(config.account == state.draft.account, "the account");
        assert_eq!(config.desktop, state.draft.desktop);
        assert_eq!(config.bootloader, state.draft.bootloader);
        assert_eq!(config.software, state.draft.software);
        assert_eq!(config.source, state.draft.source);
    }

    /// With no plan of Partitions' in use, what the person has chosen is the
    /// draft byte for byte — whatever its place, with a plan made and kept
    /// as a draft beside it, on the demo or off it.
    #[test]
    fn with_no_plan_in_use_the_configuration_is_the_draft_byte_for_byte() {
        let mut state = State::demo(true);
        let same = |state: &State| {
            let config = state.config().expect("the draft");
            assert_eq!(
                serde_json::to_string(&config).unwrap(),
                serde_json::to_string(&state.draft).unwrap()
            );
            let request = state.request().unwrap();
            assert_eq!(
                serde_json::to_string(&request.config).unwrap(),
                serde_json::to_string(&state.draft).unwrap()
            );
            assert_eq!(request.consent, disks::consent(&state.draft));
        };
        same(&state);
        for disk in state.disks.clone() {
            for place in screens::target::places(&disk) {
                state.draft = screens::target::with_place(&state.draft, &disk, &place);
                same(&state);
            }
        }
        screens::partitions::stage_plan(&mut state);
        assert!(!screens::partitions::in_use(&state));
        same(&state);
        let mut live = live_with_the_plan_in_use();
        screens::partitions::let_go_of(&mut live);
        same(&live);
    }

    /// The made-up machine's disks with the two Partitions shows beside
    /// them, in the order lsblk lists a machine's disks — by their numbers,
    /// the SATA disks (8:…) before the NVMe drive (259:0) — which is not the
    /// order of their paths; as a live system would list them again with
    /// nothing changed.
    fn listed_by_their_numbers() -> Result<Vec<Disk>, String> {
        let mut disks = demo::disks();
        disks.extend(demo::partition_disks());
        disks.sort_by_key(|disk| {
            let (major, minor) = disk.major_minor.split_once(':').unwrap_or_default();
            (major.parse::<u32>().ok(), minor.parse::<u32>().ok())
        });
        Ok(disks)
    }

    /// **A plan over three disks is asked about, and handed to the worker,
    /// in path order** (Unit Q, Q6), whatever order its disks are listed
    /// and touched in: on a live system whose disks are listed as lsblk
    /// lists them (the NVMe drive after the SATA disks), with every
    /// capability the plan needs pretended proven (`layout::PretendProven`),
    /// a plan made on the Linux SSD first (its home kept at `/home`, its
    /// swap partition made anew), then on Windows' disk (Data deleted, `/`
    /// made in its place), then on the NVMe drive (a new gpt, the EFI system
    /// partition at `/efi` and `/boot` beside it) — its layout listing them
    /// as the machine does — goes on from Where to install and is judged
    /// whole, and Install puts its question up; its consent,
    /// the paged question's lines and the lines its first page kept
    /// (`asked`) are one list, the NVMe drive's table, then Windows' Data,
    /// then the swap partition, each said on the pages in that order and
    /// the kept home nowhere; and the worker takes the very bytes the
    /// interface would write — their consent the one it builds itself — and
    /// refuses them with the same lines in the order the disks were touched
    /// (`ConsentMismatch`). With `PROVEN` itself the plan is refused: no one
    /// scenario ran all it does together.
    #[test]
    fn a_plan_over_three_disks_is_asked_and_handed_over_in_path_order() {
        use crate::model::disks::{Table, GIB};
        use crate::model::layout::{Capability, PretendProven};
        use crate::ui::partitions::{apply, Fs, Mount, NewPart, Plan};
        use crate::worker::system::check;
        use Capability::{
            ChangeEntries, DeleteEntries, FreshTable, Gpt, KeepTable, KeptHome, SeveralDisks, Uefi,
        };
        const PROVEN: &[Capability] = &[
            FreshTable,
            KeepTable,
            DeleteEntries,
            ChangeEntries,
            KeptHome,
            SeveralDisks,
            Uefi,
            Gpt,
        ];
        let mut state = live_with_the_plan_in_use();
        state.disks = listed_by_their_numbers().unwrap();
        state.list_disks = listed_by_their_numbers;
        let firmware = state.firmware();
        let disk = |path: &str| {
            let found = state.disks.iter().find(|disk| disk.path == path);
            found.expect("a made-up disk").clone()
        };
        let (nvme, sda, sdd) = (disk("/dev/nvme0n1"), disk("/dev/sda"), disk("/dev/sdd"));
        let plan = &mut state.views.partitions.plan;
        *plan = Plan::default();
        plan.mount(&sdd.path, "/dev/sdd4", Some(Mount::Home));
        plan.format(&sdd.path, "/dev/sdd3", Some(Fs::LinuxSwap));
        plan.mount(&sdd.path, "/dev/sdd3", Some(Mount::Swap));
        plan.delete(&sda.path, "/dev/sda5");
        let room = apply(plan, &sda, firmware).disk.free.last().copied();
        let room = room.expect("Data's room");
        let number = plan.next_number();
        plan.create(
            &sda.path,
            NewPart {
                number,
                start: room.start,
                size: room.size,
                fs: Fs::Ext4,
                mount: Some(Mount::Root),
                label: String::new(),
            },
        );
        plan.table(&nvme.path, Table::Gpt);
        let free = apply(plan, &nvme, firmware).disk.free[0];
        for (start, size, mount) in [
            (free.start, GIB, Mount::Efi),
            (free.start + GIB, 2 * GIB, Mount::Boot),
        ] {
            let number = plan.next_number();
            plan.create(
                &nvme.path,
                NewPart {
                    number,
                    start,
                    size,
                    fs: Fs::Fat32,
                    mount: Some(mount),
                    label: String::new(),
                },
            );
        }
        assert_eq!(screens::partitions::missing(&state), None);
        let request = state.request().expect("a request is built");
        let Target::Layout { disks: laid } = &request.config.target else {
            panic!("the request does not carry the plan");
        };
        let listed: Vec<&str> = laid.iter().map(|one| one.disk.path.as_str()).collect();
        assert_eq!(listed, ["/dev/sda", "/dev/sdd", "/dev/nvme0n1"]);
        assert_eq!(layout::needs(laid, Firmware::Uefi), PROVEN);
        let named: Vec<&str> = request
            .consent
            .iter()
            .map(|line| line.split_whitespace().take(2).last().unwrap_or_default())
            .collect();
        assert_eq!(named, ["/dev/nvme0n1", "/dev/sda5", "/dev/sdd3"]);
        // What passed holds no one set of all it needs.
        assert_eq!(
            layout::unproven(laid, Firmware::Uefi),
            Some(layout::Unproven::Together(PROVEN.to_vec()))
        );
        // Through Where to install, every capability pretended proven; then
        // judged whole, and Install puts its question up.
        let _proven = PretendProven::these(&[PROVEN]);
        screens::partitions::let_go_of(&mut state);
        state.screen = Screen::Target;
        state.note = None;
        screens::partitions::onward_from_where_to_install(&mut state);
        assert!(screens::partitions::in_use(&state), "{:?}", state.note);
        assert_eq!(state.plan_judged(), Ok(()));
        assert_eq!(state.ready(), Ok(()));
        // The question: its pages say each line in path order, the kept
        // home nowhere; the lines its first page kept are the consent.
        state.go(Screen::Review);
        state.ask_to_install();
        assert_eq!(
            state.question.take(),
            Some(Question::PlanErase { page: 0 }),
            "Install did not put the plan's question up"
        );
        let pages = read_the_plan_question(&mut state);
        let request = state.request().expect("a request is built");
        assert_eq!(state.asked.as_ref(), Some(&request.consent));
        let said = pages.join("\n");
        let at = |text: &str| {
            said.find(text)
                .unwrap_or_else(|| panic!("{text} is not said in {said}"))
        };
        assert!(at("/dev/nvme0n1") < at("/dev/sda5"), "{said}");
        assert!(at("/dev/sda5") < at("/dev/sdd3"), "{said}");
        assert!(!said.contains("/dev/sdd4"), "{said}");
        // The worker: the very bytes, taken (its own consent the same
        // lines); the lines in the order the disks were touched, or listed,
        // refused.
        let bytes = serde_json::to_vec(&request).unwrap();
        let read = check::read_request(bytes.as_slice())
            .unwrap_or_else(|stop| panic!("{:?}: {}", stop.failure, stop.detail));
        assert_eq!(disks::consent(&read.config), request.consent);
        if let Err(stop) = check::validate(&read, Firmware::Uefi) {
            panic!("{:?}: {}", stop.failure, stop.detail);
        }
        for order in [[2, 1, 0], [1, 2, 0]] {
            let mut other = read.clone();
            other.consent = order.map(|at| read.consent[at].clone()).to_vec();
            let refused =
                check::validate(&other, Firmware::Uefi).expect_err("taken in another order");
            assert_eq!(refused.failure, Failure::ConsentMismatch, "{order:?}");
        }
    }

    /// **Logical partitions are asked about, and handed over, by the numbers
    /// they were reviewed with** (Unit Q, Q7): on a live system a BIOS
    /// starts, with what the plan needs pretended proven, a plan on the old
    /// hard disk's msdos table — its Linux, a logical partition, formatted
    /// as `/`; its shared FAT32, the last logical partition, deleted; its
    /// swap partition, the logical one between, formatted afresh and grown
    /// over where the FAT32 stood — has a consent naming each by its path
    /// and the number it was reviewed with (5, 6, 7) and the place it was
    /// reviewed at; the question says them in that order by those paths;
    /// and the worker takes the very bytes, each number the reviewed
    /// partition's, and refuses them with a logical partition named by the
    /// number it would have were the one before it gone. (With `PROVEN`
    /// itself the plan is refused: its `/` on a logical partition is what no
    /// scenario ran.)
    #[test]
    fn logical_partitions_are_asked_about_and_handed_over_by_their_reviewed_numbers() {
        use crate::model::disks::MIB;
        use crate::model::layout::{Capability, PretendProven};
        use crate::ui::partitions::{Fs, Mount, Plan};
        use crate::worker::system::check;
        use Capability::{
            Bios, ChangeEntries, DeleteLogical, LargerLogical, Logical, Msdos, RootOnLogical,
        };
        // Its `/` on a logical partition, which no scenario runs, among it.
        const PROVEN: &[Capability] = &[
            DeleteLogical,
            ChangeEntries,
            LargerLogical,
            Bios,
            Msdos,
            Logical,
            RootOnLogical,
        ];
        let mut state = live_with_the_plan_in_use();
        state.pretend_firmware(Firmware::Bios);
        state.disks = listed_by_their_numbers().unwrap();
        state.list_disks = listed_by_their_numbers;
        let old = state
            .disks
            .iter()
            .find(|disk| disk.path == "/dev/sdc")
            .expect("the old hard disk")
            .clone();
        let six = old.partition("/dev/sdc6").expect("its swap").clone();
        let plan = &mut state.views.partitions.plan;
        *plan = Plan::default();
        plan.format(&old.path, "/dev/sdc5", Some(Fs::Ext4));
        plan.mount(&old.path, "/dev/sdc5", Some(Mount::Root));
        plan.delete(&old.path, "/dev/sdc7");
        plan.format(&old.path, "/dev/sdc6", Some(Fs::LinuxSwap));
        plan.mount(&old.path, "/dev/sdc6", Some(Mount::Swap));
        let grown = (102_402 - 71_683) * MIB;
        assert!(plan.resize(&old.path, "/dev/sdc6", six.size, grown));
        assert_eq!(screens::partitions::missing(&state), None);
        let request = state.request().expect("a request is built");
        let Target::Layout { disks: laid } = &request.config.target else {
            panic!("the request does not carry the plan");
        };
        assert_eq!(layout::needs(laid, Firmware::Bios), PROVEN);
        let mib = |n: u64| n * MIB;
        assert_eq!(
            request.consent,
            [
                format!(
                    "format /dev/sdc5 5 start={} size={} to={} ext4",
                    mib(40_962),
                    mib(30_720),
                    mib(30_720)
                ),
                format!(
                    "format /dev/sdc6 6 start={} size={} to={grown} linux-swap",
                    mib(71_683),
                    mib(2_048)
                ),
                format!(
                    "delete /dev/sdc7 7 start={} size={} vfat",
                    mib(73_732),
                    mib(28_670)
                ),
            ]
        );
        for line in &request.consent {
            let words: Vec<&str> = line.split_whitespace().collect();
            let reviewed = old.partition(words[1]).expect("a reviewed partition");
            assert_eq!(words[2], reviewed.number.to_string(), "{line}");
            assert_eq!(words[3], format!("start={}", reviewed.start), "{line}");
        }
        // What passed holds no `/` on a logical partition.
        assert_eq!(
            layout::unproven(laid, Firmware::Bios),
            Some(layout::Unproven::Never(RootOnLogical))
        );
        // What it needs pretended proven, it is judged whole. (Not gone on
        // with from Where to install: that would arrive at Desktop, which on
        // a live system asks the internet on a thread of its own, a question
        // no test is handed the answer to.)
        let _proven = PretendProven::these(&[PROVEN]);
        assert!(screens::partitions::in_use(&state));
        assert_eq!(state.plan_judged(), Ok(()));
        // The question says them by their reviewed paths, in that order.
        state.go(Screen::Review);
        state.ask_to_install();
        let pages = read_the_plan_question(&mut state);
        let request = state.request().expect("a request is built");
        assert_eq!(state.asked.as_ref(), Some(&request.consent));
        let said = pages.join("\n").replace('\u{a0}', " ");
        let at = |text: &str| {
            said.find(text)
                .unwrap_or_else(|| panic!("{text} is not said in {said}"))
        };
        assert!(at("Format /dev/sdc5 as ext4") < at("Format /dev/sdc6 as linux-swap"));
        assert!(at("Format /dev/sdc6 as linux-swap") < at("Delete /dev/sdc7 (fat32"));
        // The worker: the very bytes, every number the reviewed one, taken;
        // sdc7 named by the number it would have once sdc6 were gone,
        // refused.
        let bytes = serde_json::to_vec(&request).unwrap();
        let read = check::read_request(bytes.as_slice())
            .unwrap_or_else(|stop| panic!("{:?}: {}", stop.failure, stop.detail));
        assert_eq!(disks::consent(&read.config), request.consent);
        if let Err(stop) = check::validate(&read, Firmware::Bios) {
            panic!("{:?}: {}", stop.failure, stop.detail);
        }
        let mut renumbered = read.clone();
        renumbered.consent[2] = renumbered.consent[2].replace("/dev/sdc7 7 ", "/dev/sdc6 6 ");
        let refused = check::validate(&renumbered, Firmware::Bios)
            .expect_err("a partition named by another number");
        assert_eq!(refused.failure, Failure::ConsentMismatch);
    }

    /// The plan's question lists exactly the request's consent: the lines
    /// its first page kept are the consent `request` builds, every thing the
    /// plan destroys is said on its pages in the rule set's order — each in
    /// GParted's words, a partition that is Windows' named so — every page
    /// but the last answering Next and the last "Erase and install", and no
    /// page ever says how many it left out. Where the plan destroys nothing,
    /// one page asks to install and says nothing is erased. Erase asked
    /// while the plan is in use is the plan's question.
    #[test]
    fn the_question_lists_exactly_the_request_consent() {
        use crate::ui::partitions::{Fs, Mount, NewPart, Plan};
        let mut state = State::demo(true);
        screens::partitions::stage_plan(&mut state);
        state.views.partitions.in_use = true;
        assert_eq!(
            state.put_as(Question::Erase),
            Question::PlanErase { page: 0 }
        );
        for most in [30, 9, 5] {
            let pages = read_the_plan_question_in(&mut state, most);
            let consent = state.request().unwrap().consent;
            assert_eq!(state.asked.as_ref(), Some(&consent), "{most} lines");
            let said = pages.join("\n").replace('\u{a0}', " ");
            let mut from = 0;
            for line in [
                "• Shrink /dev/sda3 from 372.5 GiB to 300.0 GiB · Windows",
                "• Format /dev/sda4 as fat32 · Windows",
                "• Delete /dev/sda5 (ntfs, 278.7 GiB) · Windows",
            ] {
                let at = said[from..]
                    .find(line)
                    .unwrap_or_else(|| panic!("{line} not in {said}"));
                from += at + line.len();
            }
            let earlier = i18n::text("partitions-pending-earlier");
            assert!(!said.contains(&earlier[..earlier.len().min(8)]), "{said}");
            assert!(!said.contains("Create"), "{said}");
            assert!(pages.last().unwrap().contains(i18n::text("erase-final")));
            let (title, _, answers) =
                dialog_of_a_test(&mut state, Question::PlanErase { page: 0 }, most);
            assert_eq!(title, i18n::text("erase-title"));
            let next = if pages.len() > 1 {
                i18n::text("button-next")
            } else {
                i18n::text("erase-confirm")
            };
            assert_eq!(answers, [i18n::text("erase-back"), next]);
            let last = pages.len() - 1;
            let (_, _, answers) =
                dialog_of_a_test(&mut state, Question::PlanErase { page: last }, most);
            assert_eq!(answers[1], i18n::text("erase-confirm"));
        }
        // Made in unallocated space and mounted: nothing destroyed.
        let disk = state.disks[1].clone();
        let gap = disk.free[0];
        let plan = &mut state.views.partitions.plan;
        *plan = Plan::default();
        plan.create(
            &disk.path,
            NewPart {
                number: 1,
                start: gap.start,
                size: gap.size,
                fs: Fs::Ext4,
                mount: Some(Mount::Root),
                label: String::new(),
            },
        );
        let (title, body, answers) =
            dialog_of_a_test(&mut state, Question::PlanErase { page: 0 }, 30);
        assert_eq!(title, i18n::text("install-title"));
        assert_eq!(answers[1], i18n::text("button-install"));
        assert_eq!(body, i18n::text("erase-plan-nothing"));
        assert_eq!(state.asked, Some(Vec::new()));
        assert_eq!(state.request().unwrap().consent, Vec::<String>::new());
        assert_eq!(state.shorter_title(Question::PlanErase { page: 0 }), None);
    }

    /// `begin`, under Partitions' plan, goes on only once the plan's
    /// question has been read to its last page and the lines its first page
    /// kept are still the request's consent: not after its first page of
    /// several, not once the plan changed after its first page was put up,
    /// not where its last page was put up without its first.
    #[test]
    fn begin_refuses_a_changed_consent_or_an_unread_last_page() {
        let mut state = State::demo(true);
        screens::partitions::stage_plan(&mut state);
        state.views.partitions.in_use = true;
        // Four lines a page: more pages than one.
        let pages = read_the_plan_question_in(&mut state, 4).len();
        assert!(pages > 1, "{pages}");
        let last = Question::PlanErase { page: pages - 1 };
        dialog_of_a_test(&mut state, Question::PlanErase { page: 0 }, 4);
        state.begin(last);
        assert!(state.rehearsal.is_none(), "its last page was not read");
        // Read whole, and the plan changed since: refused.
        read_the_plan_question_in(&mut state, 4);
        state.views.partitions.plan.undo_last();
        state.begin(last);
        assert!(state.rehearsal.is_none(), "the consent changed");
        // The last page alone, never the first: refused.
        state.forget_the_plan_question();
        dialog_of_a_test(&mut state, Question::PlanErase { page: usize::MAX }, 4);
        state.begin(last);
        assert!(state.rehearsal.is_none(), "its first page was never put up");
        // Read whole, unchanged: it goes on.
        read_the_plan_question_in(&mut state, 4);
        state.begin(last);
        assert!(state.rehearsal.is_some());
        assert!(state.events.is_none());
    }

    /// What `begin` may start is decided by the question answered, never by
    /// whether a plan is in use alone, which may change between a question
    /// and its answer (step Q2). Partitions' plan read to the last page
    /// of its question and then let go of — as a re-listing that finds a
    /// disk changed lets it go — and "Erase and install" pressed on that
    /// page: nothing begins, though the draft's place behind it could be
    /// installed, for that place's own question was never put up. The easy
    /// question put up with no plan in use and answered once one is:
    /// nothing begins, for the request is then the plan's, which it never
    /// said. Each question still begins what it said. In the demo, whose
    /// rehearsal stands where a live system would ask for a worker behind
    /// the same check (`consent_read`).
    #[test]
    fn begin_goes_on_only_for_what_the_question_answered_said() {
        let mut state = State::demo(true);
        screens::partitions::stage_plan(&mut state);
        state.views.partitions.in_use = true;
        state.go(Screen::Review);
        state.ask_to_install();
        assert_eq!(state.question.take(), Some(Question::PlanErase { page: 0 }));
        let pages = read_the_plan_question(&mut state).len();
        screens::partitions::let_go_of(&mut state);
        assert!(state.ready().is_ok(), "the draft's place is refused");
        assert!(!state.request().unwrap().consent.is_empty());
        state.answer(Question::PlanErase { page: pages - 1 }, 1);
        assert!(state.rehearsal.is_none(), "the draft's place began");
        assert!(state.events.is_none(), "a worker was asked for");
        assert_eq!(state.screen, Screen::Review);
        assert_eq!(state.asked, None, "the plan's question is still held");
        // The easy question, put up with no plan in use and answered once
        // the plan is in use again.
        state.ask_to_install();
        let question = state.question.take().expect("a question");
        let question = put_up(&mut state, question);
        assert_eq!(question, Question::Erase);
        state.views.partitions.in_use = true;
        state.answer(question, 1);
        assert!(state.rehearsal.is_none(), "the plan began");
        assert_eq!(state.screen, Screen::Review);
        // With no plan in use, the easy question begins the draft's place.
        screens::partitions::let_go_of(&mut state);
        state.answer(Question::Erase, 1);
        assert_eq!(state.screen, Screen::Installing);
        assert!(state.rehearsal.is_some());
        // With the plan in use, its question read whole begins the plan.
        let mut state = State::demo(true);
        screens::partitions::stage_plan(&mut state);
        state.views.partitions.in_use = true;
        state.go(Screen::Review);
        let pages = read_the_plan_question(&mut state).len();
        state.answer(Question::PlanErase { page: pages - 1 }, 1);
        assert_eq!(state.screen, Screen::Installing);
        assert!(state.rehearsal.is_some());
        assert!(state.events.is_none());
    }

    /// On a live system, before the plan's question, the disks are listed
    /// again (`list_disks`, read-only, a test's own here): unchanged, the
    /// plan stands and Install goes on to what refuses it next; a disk the
    /// plan changes listed differently drops the plan, and Install says so
    /// rather than asking anything; disks that cannot be read are said so,
    /// the plan kept. The demo lists nothing again, and nor does a live
    /// system with no plan in use.
    #[test]
    fn before_the_plans_question_a_live_system_lists_its_disks_again() {
        let mut state = live_with_the_plan_in_use();
        state.go(Screen::Review);
        state.ask_to_install();
        assert!(screens::partitions::in_use(&state), "unchanged, dropped");
        let said = state.note.as_ref().map(|note| note.text.clone());
        assert_eq!(said.as_deref(), state.ready().err().as_deref());
        state.list_disks = no_disks_to_read;
        state.ask_to_install();
        assert!(screens::partitions::in_use(&state));
        let said = state.note.as_ref().map(|note| note.text.as_str());
        assert_eq!(said, Some(i18n::text("disks-unreadable")));
        state.list_disks = the_made_up_disks_changed;
        state.ask_to_install();
        assert_eq!(state.question, None);
        assert!(!screens::partitions::in_use(&state), "the plan stood");
        assert!(state.views.partitions.plan.is_empty());
        let said = state.note.as_ref().map(|note| note.text.as_str());
        assert_eq!(said, Some(i18n::text("target-partitions-let-go")));
        // With no plan in use, nothing is listed again.
        state.list_disks = no_disks_to_read;
        state.ask_to_install();
        let said = state.note.as_ref().map(|note| note.text.as_str());
        assert_ne!(said, Some(i18n::text("disks-unreadable")));
        // Nor in the demo, the plan in use.
        let mut demo = State::demo(true);
        screens::partitions::stage_plan(&mut demo);
        demo.views.partitions.in_use = true;
        demo.list_disks = no_disks_to_read;
        demo.go(Screen::Review);
        demo.ask_to_install();
        assert_eq!(demo.question, Some(Question::PlanErase { page: 0 }));
    }

    #[test]
    fn the_last_question_names_everything_the_consent_list_erases() {
        let mut state = State::demo(true);
        let disk = state.disks[1].clone();
        let gap = disk.free[0];
        let data = disk.partitions[4].clone();
        state.draft.disk = disk;
        state.draft.target = Target::FreeSpace {
            start: gap.start,
            size: gap.size,
        };
        let (title, body, _) = state.question_words(Question::Erase);
        assert_eq!(title, i18n::text("install-title"), "nothing is erased");
        assert!(body.ends_with(i18n::text("erase-nothing")), "{body}");
        state.draft.storage.home = Some(crate::model::config::Home {
            path: data.path.clone(),
            format: true,
            disk: None,
        });
        assert_eq!(disks::consent(&state.draft), vec![data.path.clone()]);
        let (title, body, _) = state.question_words(Question::Erase);
        assert_eq!(title, i18n::text("erase-title"));
        assert!(body.contains("“Data”"), "{body}");
        assert!(body.ends_with(i18n::text("erase-final")), "{body}");
        assert!(!body.contains("/dev"), "{body}");
    }

    /// How far apart the harness presses its keys at the closest: `press`
    /// waits 0.6 s after each key, and two seconds more after the last of a
    /// call. Replayed so, each in a frame of its own, the second column of
    /// Time zone and Keyboard has stood on the screen for its dwell
    /// (`trail::OPENING`) before the press after the one that opened it.
    const HARNESS_APART: f32 = 0.6;

    /// The harness's walk from Welcome to Where to install with a keyboard
    /// (`tests/e2e/scenarios.py`, `walk_to_target` and `live_lxb`): Get
    /// started, the language already chosen, Enter, Enter on Time zone (the
    /// guessed part of the world opened, its guessed place chosen), Enter,
    /// Enter, Right, Enter on Keyboard (the guessed layout opened, its plain
    /// kind chosen, the field, Continue); then back out the way `live_lxb`
    /// leaves: Escape four times to Welcome, and once more for the question
    /// whether to leave.
    #[test]
    fn the_harness_walks_from_welcome_to_where_to_install_and_out() {
        use Action::{Accept, Back, Right};
        let mut state = State::demo(true);
        let walk = [
            Accept, Accept, Accept, Accept, Accept, Accept, Right, Accept,
        ];
        state.press_apart(&walk, HARNESS_APART);
        assert_eq!(state.screen, Screen::Target);
        let region = &state.draft.region;
        assert_eq!(region.locale, "en_US.UTF-8");
        assert_eq!(region.timezone, "America/New_York");
        assert_eq!(
            (
                region.keyboard.layout.as_str(),
                region.keyboard.variant.as_str()
            ),
            ("us", "")
        );
        for _ in 0..4 {
            state.act(Back, None);
        }
        assert_eq!(state.screen, Screen::Welcome);
        assert_eq!(state.question_up(), None);
        state.act(Back, None);
        assert_eq!(state.question_up(), Some(Question::Quit));
    }

    /// The same walk with Polski chosen (`walk_to_target_in_polish`): the
    /// harness counts its Downs from the rows it names (`LANGUAGE_ROWS`),
    /// and the zone and keyboard then guessed — Europe/Warsaw, Polish — are
    /// what the same presses keep.
    #[test]
    fn the_harness_walks_to_where_to_install_in_polish() {
        use Action::{Accept, Down, Right};
        let row = |locale: &str| {
            crate::model::catalogs::languages()
                .iter()
                .position(|language| language.locale == locale)
                .unwrap()
        };
        assert_eq!((row("en_US.UTF-8"), row("pl_PL.UTF-8")), (11, 26));
        let mut state = State::demo(true);
        state.act(Accept, None); // Get started
        for _ in 0..(26 - 11) {
            state.act(Down, None);
        }
        state.act(Accept, None); // Polski, and on
        assert_eq!(state.screen, Screen::Timezone);
        state.press_apart(
            &[Accept, Accept, Accept, Accept, Right, Accept],
            HARNESS_APART,
        );
        assert_eq!(state.screen, Screen::Target);
        let region = &state.draft.region;
        assert_eq!(region.locale, "pl_PL.UTF-8");
        assert_eq!(region.timezone, "Europe/Warsaw");
        assert_eq!(
            (
                region.keyboard.layout.as_str(),
                region.keyboard.variant.as_str()
            ),
            ("pl", "")
        );
    }

    /// A scene is staged as the walk to it would have left Time zone and
    /// Keyboard (by the rule that their Continue goes on only with what they
    /// hold seen): Polish chosen from its kinds (`keyboard`, `keyboard-none`)
    /// is seen, as choosing sees it; the guess's places standing open,
    /// settled (`timezone-places`), are seen; and a scene past either step
    /// came through its Continue. So Continue pressed in such a picture, or
    /// after Back to the step from one, goes on at once, as it does in the
    /// installer.
    #[test]
    fn a_staged_scene_has_seen_what_the_walk_to_it_saw() {
        use crate::ui::screens::{keyboard, timezone};
        use Action::{Accept, Right};
        let mut state = State::demo(true);
        state.stage("keyboard-none").unwrap();
        assert_eq!(state.views.keyboard.focus, keyboard::Focus::Continue);
        state.press_apart(&[Accept], 0.1);
        assert_eq!(state.screen, Screen::Target, "keyboard-none");
        let mut state = State::demo(true);
        state.stage("keyboard").unwrap();
        state.press_apart(&[Right, Accept], 0.1);
        assert_eq!(state.screen, Screen::Target, "keyboard");
        let mut state = State::demo(true);
        state.stage("timezone-places").unwrap();
        state.press_apart(&[Right, Accept], 0.1);
        assert_eq!(state.screen, Screen::Keyboard, "timezone-places");
        // As arrived at, nothing seen: Continue shows the guess first.
        for (scene, step) in [
            ("timezone-continue", Screen::Timezone),
            ("keyboard-continue", Screen::Keyboard),
        ] {
            let mut state = State::demo(true);
            state.stage(scene).unwrap();
            state.press_apart(&[Accept], 0.1);
            assert_eq!(state.screen, step, "{scene}");
        }
        for scene in ["target", "desktop", "review"] {
            let mut state = State::demo(true);
            state.stage(scene).unwrap();
            while state.screen != Screen::Keyboard {
                state.back();
            }
            state.views.keyboard.focus = keyboard::Focus::Continue;
            state.press_apart(&[Accept], 0.1);
            assert_eq!(state.screen, Screen::Target, "{scene}: Keyboard");
            state.back();
            state.back();
            assert_eq!(state.screen, Screen::Timezone, "{scene}");
            state.views.timezone.focus = timezone::Focus::Continue;
            state.press_apart(&[Accept], 0.1);
            assert_eq!(state.screen, Screen::Keyboard, "{scene}: Time zone");
        }
    }

    /// **The pictures of a plan that keeps Windows smaller show what the
    /// worker would say** (Unit Q, Q9): its rehearsal on the Resize step,
    /// shrinking what stays on the disk; Done, saying Windows will check its
    /// disk; Stopped where a shrink stops — in Resize, with the words for a
    /// disk whose parts are all still there, and in Check, with the words
    /// for nothing changed, each cause the worker reads there its own.
    #[test]
    fn the_shrinks_pictures_show_what_the_worker_would_say() {
        let staged = |scene: &str| {
            let mut state = State::demo(true);
            state.stage(scene).unwrap();
            assert!(screens::partitions::in_use(&state), "{scene}: no plan");
            state
        };
        let installing = staged("installing-shrink");
        assert_eq!(installing.screen, Screen::Installing);
        assert_eq!(installing.install.current, Some(Step::Resize));
        assert_eq!(installing.install.steps[..2], [Step::Check, Step::Resize]);
        let done = staged("done-shrink");
        assert_eq!(done.screen, Screen::Done);
        assert_eq!(
            done.install.warnings,
            [crate::model::protocol::Warning::WindowsWillCheck]
        );
        for (scene, step, failure, said_to_do) in [
            (
                "stopped-shrink",
                Step::Resize,
                Failure::ShrinkRefused,
                "failure-shrink-refused-do",
            ),
            (
                "stopped-hibernated",
                Step::Check,
                Failure::WindowsHibernated,
                "failure-windows-hibernated-do",
            ),
            (
                "stopped-needs-check",
                Step::Check,
                Failure::FilesystemNeedsCheck,
                "failure-needs-check-untouched-do",
            ),
            (
                "stopped-windows-in-use",
                Step::Check,
                Failure::WindowsInUse,
                "failure-windows-in-use-do",
            ),
            (
                "stopped-windows-check",
                Step::Check,
                Failure::WindowsNeedsCheck,
                "failure-windows-needs-check-do",
            ),
            (
                "stopped-windows-unread",
                Step::Check,
                Failure::WindowsUnread,
                "failure-windows-unread-do",
            ),
            (
                "stopped-bitlocker",
                Step::Check,
                Failure::BitLocker,
                "failure-bitlocker-do",
            ),
        ] {
            let stopped = staged(scene);
            assert_eq!(stopped.screen, Screen::Stopped, "{scene}");
            let Some(Ending::Failed {
                step: at,
                failure: said,
                ..
            }) = &stopped.install.ending
            else {
                panic!("{scene}: not stopped");
            };
            assert_eq!((*at, *said), (Some(step), failure), "{scene}");
            let (_, what_to_do) = crate::ui::words::failure(failure, Some(step));
            assert_eq!(what_to_do, i18n::text(said_to_do), "{scene}");
        }
    }

    #[test]
    fn every_scene_can_be_staged() {
        for scene in SCENES {
            let mut state = State::demo(true);
            state.stage(scene).unwrap();
        }
        assert!(State::demo(true).stage("nowhere").is_err());
        let mut done = State::demo(true);
        done.stage("done").unwrap();
        assert_eq!(done.screen, Screen::Done);
        let mut stopped = State::demo(true);
        stopped.stage("stopped").unwrap();
        assert_eq!(stopped.screen, Screen::Stopped);
    }

    #[test]
    fn every_step_with_chips_has_its_title_among_those_that_place_them() {
        let titles = screens::step_titles();
        for screen in Screen::ASKING {
            if screen.place().is_none() {
                continue;
            }
            let mut state = State::demo(true);
            state.go(screen);
            let title = screens::head(&state).title;
            assert!(
                titles.contains(&title.as_str()),
                "{screen:?}'s title is not weighed where the chips stand"
            );
        }
    }

    #[test]
    fn the_options_picture_raises_options_with_the_light_left_on_the_place() {
        let mut state = State::demo(true);
        state.stage("target-options").unwrap();
        let menu = state.menu.as_ref().expect("the Options menu");
        assert_eq!(menu.commands.len(), 3);
        assert_eq!(
            state.views.target.focus,
            screens::target::Focus::Pair,
            "the light went to Options: the picture would not show the menu \
             growing out of Options rather than out of the light"
        );
        // Nothing is named until the screen is drawn, so a menu can never
        // grow out of where a button stood on another frame.
        assert_eq!(state.menu_anchor(), None);
    }
}
