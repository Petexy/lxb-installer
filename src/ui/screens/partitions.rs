//! Partitions: Where to install's Advanced page — every disk laid out as
//! GParted lays one out, a chip for each disk over the map of the one shown
//! and the details of the tile the light is on under it
//! (`INSTALLER-HANDOFF.md` §7.0, Unit P).
//!
//! **It plans, and only plans.** A tile's menu and a disk's give a partition
//! a mount point, format it, resize it, label it, delete it, make a new one
//! in unallocated space, or write a new partition table over a disk — the
//! larger of those on a sheet laid over the page under its map
//! (`sheets`); Options takes the last operation back, or all of them,
//! lists them, and sets the swap file. What is asked for is
//! a [`Plan`] (`ui::partitions`), kept in this page's view: only once it is
//! the choice in use — which it becomes in the demo, and on a live system
//! only where everything it needs lies within one set QEMU proved
//! (`layout::PROVEN`) — does a request carry it, as a layout
//! (`State::config`); until then nothing this page does can reach a disk.
//! The map, the details and the chips draw the disks as the plan will leave
//! them; under the details, on a landscape panel, the plan's operations are
//! listed as GParted lists them in its bottom pane. The foot says the first
//! thing Continue still needs (`partitions::missing`, the rule set the
//! worker judges a request by, `model::layout`), and Continue is refused
//! with it — and, once nothing is missing, off the demo with the first
//! thing the plan asks that this version cannot carry out ("Shrinking a
//! partition can't be applied in this version yet.").
//!
//! **Technical words, like GParted** — by design, on this one page alone:
//! device names, filesystem types, flags, mount points and binary units
//! (`ui::map`). Its title, the names of the details, the menus' verbs, the
//! pending operations' sentences and the refusals are the catalogs'.
//!
//! **Hands on it.** Left and Right walk the map's tiles in disk order (an
//! extended partition's frame, then what is in it), stopping at either end;
//! Up and Down go between the chips, the map and the buttons; on the chips
//! Left and Right change the disk shown, and the map follows at once (a chip
//! raises no menu — one there would be confusing — and Accept on it goes down
//! onto the map; a disk with no partition table is given one from its
//! unallocated space, any other a new one from Options). LB (Shift+Tab) shows
//! the disk before and RB (Tab) the disk after, from anywhere, and they stop
//! at the first and the last as Left and Right on the chips do: a row of
//! disks is short, and the light jumping from its last chip back to its first
//! reads as a jump rather than a step. A disk that cannot be used — the
//! installer's own medium, one in use — is outlined and the light walks past
//! it; the pointer resting on it puts the reason in the details. A, Enter or
//! a click on a tile raises its menu, growing out of the tile; on the shown
//! disk's chip, the disk's own; Y, F10, the menu key, a right click or the
//! Options button raise Options. A choice inside a menu's row comes back to
//! that menu, the light on the row just changed, as Where to install's
//! Options does. Back leaves for Where to install, the light on Advanced; the
//! plan is kept for when the person comes back.

use lxb_app::lxb_render::{Align, Press};
use lxb_app::lxb_toolkit::{
    color::Srgb,
    input::Action,
    layout::{self, Slide},
    material::Surface,
    metrics::Metric,
    motion,
    palette::Role,
    sound::Sound,
    typography::Text,
};
use lxb_app::{Page, Scroll};

use super::sheets::{self, Sheet};
use super::{Head, Room};
use crate::i18n;
use crate::launch::MeasureEvent;
use crate::model::disks::{device_path_ok, Blocked, Content, Disk, Partition, Table};
use crate::model::layout::{self as rules, Destroy, ImageSizes, Measure, Refusal};
use crate::model::measure::MeasureAnswer;
use crate::model::protocol::Failure;
use crate::ui::app::{Machine, Menu, Question, State};
use crate::ui::button::{self, Button};
use crate::ui::field::{Edit, FieldId};
use crate::ui::legend::{hint, Button as Pictured, Hint};
use crate::ui::map::{self, Laid, Measures, Said, Stop, Tile, Words};
use crate::ui::motion::{Shape, Share};
use crate::ui::partitions::TileRow;
use crate::ui::partitions::{self as planning, Fs, Mount, Operation, Plan, Planned};
use crate::ui::{demo, glyphs, spots, text, words};
use std::sync::mpsc::{Receiver, TryRecvError};

/// Where the light stands on this page.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// On the chip of the disk shown.
    Chips,
    /// On a tile of the map.
    #[default]
    Map,
    /// On a button of the row: Options, then Continue.
    Buttons(usize),
}

/// The buttons of the row, by their places in it.
pub const OPTIONS: usize = 0;
pub const CONTINUE: usize = 1;

/// What this page keeps between frames.
#[derive(Debug, Default, Clone)]
pub struct View {
    /// The disk shown, by its place among the machine's disks: never one
    /// that cannot be used.
    pub disk: usize,
    /// The stop of the map the light is on (`map::stops`).
    pub stop: usize,
    pub focus: Focus,
    /// A disk that cannot be used whose chip the pointer rests on: the
    /// details say why, until the pointer or a key moves on.
    pub pointed: Option<usize>,
    /// For the picture of a disk that cannot be used (`partitions-blocked`):
    /// put the picture's pointer on the first such chip once it is drawn,
    /// which is the first a scene knows where it stands.
    point_at_blocked: bool,
    /// What the page will do: kept while the person goes back to Where to
    /// install and comes again, and never anywhere the worker could read.
    pub plan: Plan,
    /// Whether the plan is the choice in use: the one confirmed last.
    /// Continue on Partitions took it, once nothing was missing (in the
    /// demo: off it Continue is refused, so it never is); a place chosen on
    /// Where to install since lets go of it, the plan kept as a draft. Back
    /// changes nothing about it. See [`in_use`].
    pub in_use: bool,
    /// The plan was let go of because a disk it changes was listed again
    /// and is not as it was ([`disks_listed`]): Where to install's foot says
    /// so until a place or the plan is chosen again.
    pub let_go: bool,
    /// The menu raised last, and what its rows are: how its answer is read
    /// back, and what it grows out of.
    raised: Option<Raised>,
    /// Where the tiles and Options stood on the last frame drawn:
    /// what a menu a click raises grows out of, the click being answered
    /// after the frame is drawn.
    pub(super) drawn: Drawn,
    /// Continue was pressed while something is still missing: what the foot
    /// says is the refusal, said in bold until the plan changes.
    refused: bool,
    /// The sheet laid over the page — New partition, Resize, Label…, Mount
    /// point · Other… — while one is up (`sheets`).
    pub sheet: Option<Sheet>,
    /// How far the map is slid where it is laid out wider than the panel (a
    /// display standing on its side), on the spring the Home menu's cards
    /// ride (`layout::Slide`), and the disk it was slid for: another disk's
    /// map stands where it belongs at once.
    slide: Slide,
    slid_for: Option<usize>,
    /// How far the map stood slid on the last frame drawn: the light lying
    /// on a tile rides with it as it slides.
    slid_by: f32,
    /// The pointer put the light on the tile it is on: the map does not
    /// slide under a pointer resting on it (a click on a tile not wholly in
    /// view brings it in).
    pub(super) pointed_tile: bool,
    /// The map as the last frame drew it — its disk, its room, the plan it
    /// drew and where each stop stood along it (before any slide) — and how
    /// far it has eased from where its stops stood when the plan last
    /// changed (`ease_from`) to where they stand now.
    map_was: Option<MapWas>,
    ease: Share,
    ease_from: Vec<map::Span>,
    /// A sheet that has closed, folding back into its tile over the page
    /// (`sheets::draw_folding`).
    pub(super) folding: Option<Sheet>,
}

/// The map as a frame drew it (`View::map_was`).
#[derive(Debug, Clone)]
struct MapWas {
    disk: usize,
    room: [f32; 2],
    plan: Plan,
    spans: Vec<map::Span>,
}

impl View {
    /// Put everything where it is going, for a photograph: a sheet
    /// unfolded, the map slid where it is going.
    pub fn settle(&mut self) {
        if let Some(sheet) = self.sheet.as_mut() {
            sheet.settle();
        }
        let target = self.slide.target;
        self.slide = Slide {
            target,
            ..Slide::default()
        };
        self.ease.settle();
        self.folding = None;
    }
}

/// Where the page's controls stood on the last frame drawn.
#[derive(Debug, Default, Clone)]
pub(super) struct Drawn {
    pub(super) tiles: Vec<[f32; 4]>,
    /// Where the map was cut to where it slides: a tile not wholly inside
    /// it is a tile partly out of view.
    pub(super) map_view: Option<[f32; 4]>,
    pub(super) options: Option<[f32; 4]>,
    /// A sheet's rows, by their places among its rows (its buttons among
    /// them), where they were drawn.
    pub(super) sheet_rows: Vec<Option<[f32; 4]>>,
    /// A sheet's size's track.
    pub(super) track: Option<[f32; 4]>,
    /// The room a sheet's size takes on the map: where it was drawn, and
    /// the bytes it spans.
    pub(super) room: Option<([f32; 4], u64, u64)>,
}

/// Whether `disk` can be used on Partitions: one that is not blocked — or
/// blocked only for being too small for Linexin, which may hold anything
/// but `/` (`disks::holds_homes`, as Where to install lets one keep a
/// `/home`; the rule set refuses `/` on it).
pub(super) fn usable(disk: &Disk) -> bool {
    crate::model::disks::holds_homes(disk)
}

/// Whether the disk at `index` can be shown: one that can be used.
fn showable(state: &State, index: usize) -> bool {
    state.disks.get(index).is_some_and(usable)
}

/// The disk shown, if any can be.
pub(super) fn shown(state: &State) -> Option<&Disk> {
    let at = state.views.partitions.disk;
    state.disks.get(at).filter(|disk| usable(disk))
}

/// The disk shown as the plan leaves it: what the map, the details and the
/// light walk.
pub(super) fn planned(state: &State) -> Option<Planned> {
    shown(state).map(|disk| planning::apply(&state.views.partitions.plan, disk, state.firmware()))
}

/// The stops of the shown disk's map, in the order the light walks them.
fn stops(state: &State) -> Vec<Stop> {
    planned(state).map_or_else(Vec::new, |planned| map::stops(&map::tiles(&planned.disk)))
}

/// The stop the light is on, with the disk it is a stop of.
fn lit_stop(state: &State) -> Option<(Planned, Stop)> {
    let planned = planned(state)?;
    let stop = *map::stops(&map::tiles(&planned.disk)).get(state.views.partitions.stop)?;
    Some((planned, stop))
}

/// Where on the disk the stop the light is on starts, in bytes.
fn lit_offset(state: &State) -> Option<u64> {
    let (planned, stop) = lit_stop(state)?;
    Some(match stop {
        Stop::Part(at) | Stop::Extended(at) => planned.disk.partitions[at].start,
        Stop::Free { start, .. } => start,
        Stop::Whole => 0,
    })
}

/// Arrive on Partitions from Where to install, showing the disk its light
/// was on (`from`) — or the first that can be shown — with the light on the
/// first tile of its map; on Continue where no disk at all can be shown. The
/// plan made before is kept. On a live system, the first time, the measure
/// entry is asked what the disks hold ([`measure`]).
pub fn open(state: &mut State, from: usize) {
    let disk = if showable(state, from) {
        Some(from)
    } else {
        (0..state.disks.len()).find(|at| showable(state, *at))
    };
    let view = &mut state.views.partitions;
    let plan = std::mem::take(&mut view.plan);
    *view = View {
        disk: disk.unwrap_or(0),
        stop: 0,
        focus: if disk.is_some() {
            Focus::Map
        } else {
            Focus::Buttons(CONTINUE)
        },
        plan,
        in_use: view.in_use,
        let_go: view.let_go,
        ..View::default()
    };
    // What the disks hold is read once, as the page first opens on a live
    // system, and again only once they are listed anew.
    if !state.measuring.asked() {
        measure(state);
    }
}

/// Whether Partitions' plan is the choice in use rather than a place on
/// Where to install: the one confirmed last (`View::in_use`). While it is,
/// Where to install ticks no place, Review says the plan's rows, the last
/// question is the plan's own, page by page (`Question::PlanErase`), and the
/// request carries the plan as a layout with nothing of the place in it
/// (`State::config`). Off the demo it never is: Continue does not put it in
/// use there (`go_on_with_the_plan`).
pub fn in_use(state: &State) -> bool {
    state.views.partitions.in_use
}

/// Let go of the plan as the choice in use, keeping it as a draft: a place
/// was chosen on Where to install.
pub fn let_go_of(state: &mut State) {
    let view = &mut state.views.partitions;
    view.in_use = false;
    view.let_go = false;
}

/// The disks listed again, as `disks`. The plan names partitions by their
/// devices and their places on the disks as they were listed, so it is kept
/// only while every disk it touches is listed again exactly as it was —
/// its partitions, their places, sizes, filesystems and labels, its table;
/// otherwise it is dropped whole (a plan half carried over to a changed
/// disk would be a plan nobody made), it stops being the choice in use, and
/// Where to install's foot says so. A plan that touches no disk is kept.
///
/// Disks listed otherwise than they were at all — whether or not the plan
/// touches them — let go of what was read of them ([`Measuring`]), which
/// was read of the disks as they were, and the measure entry is asked again
/// where it had been asked before; a sheet or a menu up stood on the disks
/// as they were and is put away, and the light goes to the start of the map
/// of the disk shown, by its path where it is still listed. Listed exactly
/// as they were, nothing of the page moves.
///
/// The interface lists the disks as it starts and, on a live system, again
/// before the plan's question (`State::ask_to_install`) and after a
/// measurement names a disk that is not as it was listed ([`poll`]).
pub fn disks_listed(state: &mut State, disks: Vec<Disk>) {
    let touched = |disks: &[Disk], path: &str| disks.iter().find(|disk| disk.path == path).cloned();
    let changed = state
        .views
        .partitions
        .plan
        .operations()
        .iter()
        .map(|op| op.disk().to_string())
        .any(|path| touched(&state.disks, &path) != touched(&disks, &path));
    let anew = state.disks != disks;
    let shown = state
        .disks
        .get(state.views.partitions.disk)
        .map(|disk| disk.path.clone());
    state.disks = disks;
    let view = &mut state.views.partitions;
    if changed {
        view.let_go = view.in_use;
        view.plan.clear();
        view.in_use = false;
        view.sheet = None;
        view.refused = false;
    }
    if !anew {
        return;
    }
    let again = shown.and_then(|path| state.disks.iter().position(|disk| disk.path == path));
    view.disk = again.unwrap_or(view.disk.min(state.disks.len().saturating_sub(1)));
    view.stop = 0;
    view.sheet = None;
    view.raised = None;
    let asked = state.measuring.asked();
    state.measuring.found = Found::Nothing;
    if asked {
        measure(state);
    }
}

/// One of Review's rows for the plan in use: what it is called, what it says
/// under its name, and its mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRow {
    pub name: String,
    pub note: String,
    pub mark: &'static str,
}

/// The mark a mount point's row wears on Review: where Linexin goes for
/// `/`, a home for `/home`, the computer's chip for what it starts from
/// (`/efi`, `/boot`), a drive for swap, a folder for a path of one's own.
fn mount_mark(at: &Mount) -> &'static str {
    match at {
        Mount::Root => glyphs::DISK_TARGET,
        Mount::Home => "file-home",
        Mount::Efi | Mount::Boot => "setting-system",
        Mount::Swap => "file-drive",
        Mount::Other(_) => "file-folder",
    }
}

/// Review's rows for the plan in use, in place of Where's: one for each
/// mount point, in the order Mount point offers them — named by its path,
/// saying under it the partition used there in GParted's words ("New
/// Partition #1 · ext4 · 160.0 GiB on /dev/sda", "/dev/sda1 · fat32 · 100
/// MiB · kept") — the swap file where there is one, and how many operations
/// will be applied.
pub fn review_rows(state: &State) -> Vec<ReviewRow> {
    let plan = &state.views.partitions.plan;
    let mut used: Vec<(Mount, ReviewRow)> = Vec::new();
    for disk in state.disks.iter().filter(|disk| usable(disk)) {
        let planned = planning::apply(plan, disk, state.firmware());
        for (part, marks) in planned.disk.partitions.iter().zip(&planned.parts) {
            let Some(at) = marks.mount.clone() else {
                continue;
            };
            let filesystem = map::filesystem_words(part).to_string();
            let size = map::size(part.size);
            let note = if marks.new.is_some() {
                crate::message!(
                    "review-part-new",
                    "partition" => planning::part_name(&part.path),
                    "filesystem" => filesystem,
                    "size" => size,
                    "disk" => disk.path.as_str()
                )
            } else if marks.format.is_some() {
                crate::message!(
                    "review-part-formatted",
                    "partition" => part.path.as_str(),
                    "filesystem" => filesystem,
                    "size" => size
                )
            } else {
                crate::message!(
                    "review-part-kept",
                    "partition" => part.path.as_str(),
                    "filesystem" => filesystem,
                    "size" => size
                )
            };
            let said = ReviewRow {
                name: at.path().to_string(),
                note,
                mark: mount_mark(&at),
            };
            used.push((at, said));
        }
    }
    used.sort_by(|one, other| one.0.cmp(&other.0));
    let mut rows: Vec<ReviewRow> = used.into_iter().map(|(_, said)| said).collect();
    let swap = state.judged().storage.swap_mib;
    if swap > 0 && !planning::swap_partition(plan) {
        rows.push(ReviewRow {
            name: i18n::text("partitions-swap-file-title").to_string(),
            note: swap_words(swap),
            mark: "file-drive",
        });
    }
    rows.push(ReviewRow {
        name: i18n::text("partitions-title").to_string(),
        note: crate::message!("review-operations", "count" => plan.len() as i64),
        mark: glyphs::DISK_PARTITIONS,
    });
    rows
}

/// What Where to install's foot says of the plan: that it is the choice in
/// use and how many operations it has, or that it was let go of. Nothing
/// otherwise.
pub fn standing(state: &State) -> Option<String> {
    let view = &state.views.partitions;
    if view.let_go {
        return Some(i18n::text("target-partitions-let-go").to_string());
    }
    view.in_use
        .then(|| crate::message!("target-using-partitions", "count" => view.plan.len() as i64))
}

/// Show the disk at `index`, the light at the start of its map. Answers
/// whether it is another disk than the one shown.
fn show(state: &mut State, index: usize) -> bool {
    if !showable(state, index) || state.views.partitions.disk == index {
        return false;
    }
    let view = &mut state.views.partitions;
    view.disk = index;
    view.stop = 0;
    true
}

/// The next disk from the one shown in `direction` (±1) that can be shown,
/// shown — or nothing, at the first or the last.
fn step_disk(state: &mut State, direction: isize) -> bool {
    let mut at = state.views.partitions.disk as isize + direction;
    while at >= 0 && (at as usize) < state.disks.len() {
        if showable(state, at as usize) {
            return show(state, at as usize);
        }
        at += direction;
    }
    false
}

fn focus(state: &mut State, to: Focus) {
    if to == Focus::Map && stops(state).is_empty() {
        return;
    }
    if state.views.partitions.focus != to {
        state.views.partitions.focus = to;
        state.sound(Sound::Move);
    }
}

/// What the details say reading found of a partition the disk holds
/// ([`measured_of`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Read {
    /// The measure entry is still reading the disks: "Reading…".
    Reading,
    /// What it found of it, and whether that lets it be shrunk, kept — an
    /// NTFS judged across every disk of the measurement
    /// (`Measures::shrink_of`).
    Found {
        measure: Measure,
        shrink: Result<rules::Shrinkable, rules::Unshrinkable>,
    },
    /// Reading has ended — answered, refused or failed — with nothing of
    /// this partition, which it would have read: nothing more is known of
    /// it until the disks are listed anew.
    Unread,
}

/// Whether the measure entry reads `part` as Partitions asks it to: an ext4
/// or NTFS that could be shrunk, an EFI system partition that could be kept
/// (`worker::measure` reads these, and answers BitLocker and LUKS unread,
/// which nothing would shrink or keep mounted).
fn read_by_the_measure(part: &Partition) -> bool {
    rules::shrinkable(part) || rules::kept_esp(part)
}

/// Whether reading the disks as they are listed has ended: the demo's, made
/// up, always but in the picture of a live system still reading; a live
/// system's once the measure entry answered, was refused or failed
/// ([`Measuring`]); off the live medium never — nothing is read there.
fn reading_over(state: &State) -> bool {
    match state.machine {
        Machine::Demo => !state.measuring.reading(),
        Machine::Real { .. } => state.measuring.over(),
    }
}

/// What reading has found of a partition as the disk holds it now — named
/// by its path, its filesystem's UUID and its size (`Measures::of`), so a
/// partition changed since is not what was read: what the details say as
/// Used, and why a kept partition is not shrunk. The made-up machine's own
/// readings in the demo ([`measures_of`]); on a live system the measure
/// entry's — "Reading…" while it reads what it would read, and unread
/// once it has ended without it; nothing of what it never reads.
pub(super) fn measured_of(state: &State) -> impl Fn(&Partition) -> Option<Read> {
    let (measures, _) = measures_of(state);
    let (reading, over) = (state.measuring.reading(), reading_over(state));
    move |part| {
        if let (Some(measure), Some(shrink)) = (measures.of(part), measures.shrink_of(part)) {
            return Some(Read::Found {
                measure: *measure,
                shrink,
            });
        }
        if !read_by_the_measure(part) {
            return None;
        }
        if reading {
            Some(Read::Reading)
        } else {
            over.then_some(Read::Unread)
        }
    }
}

/// What reading the disks found, as Partitions judges by it: the made-up
/// machine's own numbers in the demo (none in the picture of a live system
/// still reading, `partitions-reading`); on a live system the measure
/// entry's last answer ([`Measuring`]), none while it reads or where it was
/// refused or failed — so a shrink is not offered there, a kept EFI
/// partition counts as unread, and a folder the image fills cannot be
/// judged; and off the live medium none ever.
pub(super) fn measures_of(state: &State) -> (rules::Measures, ImageSizes) {
    if state.machine == Machine::Demo {
        if state.measuring.reading() {
            return (rules::Measures::default(), ImageSizes::default());
        }
        let reading = state.measuring.demo_reading;
        return (
            demo::measures_as(&state.disks, reading),
            demo::image_sizes(),
        );
    }
    match &state.measuring.found {
        Found::Read { measures, image } => (measures.clone(), image.clone()),
        Found::Nothing | Found::Failed => (rules::Measures::default(), ImageSizes::default()),
    }
}

/// How long a measurement may be out before nothing more is waited for
/// from it ([`Measuring`]), by the interface's own clock (`State::clock`):
/// a minute. On the live medium polkit lets the live user run the measure
/// entry without a question, and the measure entry reads only what each
/// filesystem says of itself (`dumpe2fs -h`, `resize2fs -P`, `ntfsresize
/// --info`), what an EFI partition has free, and how much five folders of
/// the image hold — seconds' work even on a hard disk, with the medium on a
/// USB 2 stick — so an honest answer comes well inside it. One that has not
/// come by then is held up where it is not coming back from soon (a disk
/// that does not answer its reads, a polkit agent gone), and Partitions'
/// "Reading…" and Install under a plan are not held up with it; nor is the
/// person, who may well be at Install by then.
pub const READING_PATIENCE: f32 = 60.0;

/// What reading the disks with privilege has found, for Partitions to judge
/// a plan by: the read-only measure entry's answer (`launch::measure`,
/// through polkit's own read-only action), asked on a live system only —
/// once, as Partitions first opens ([`open`]), and again once the disks are
/// listed anew ([`disks_listed`]) — kept by partition, each measure named by
/// the partition's path, its filesystem's UUID and its size
/// (`Measures::of`), so it is never taken for another's. The demo never
/// asks: its numbers are its own (`demo::measures`).
///
/// While a measurement is out Partitions' details say "Reading…" and a
/// plan in use is judged as not read yet. **Nor is a measurement waited
/// for longer than [`READING_PATIENCE`]** for what it reads: then what it
/// has not answered — all of it — is unread, as though it had failed, and
/// Partitions and Install judge by that, which never offers more than
/// reading would have (no shrink, a kept EFI partition unread, a folder the
/// image fills refused in words). Nothing is killed: the measure entry runs
/// as root, out of the interface's reach, and is only let go of — its
/// answer, should it ever come, is never taken for that of a measurement
/// asked after it.
///
/// **But no installation is started beside a measure entry still running**
/// ([`Measuring::running`]), whatever the choice in use, a place on Where
/// to install as much as a plan: it holds the installer's lock while it
/// reads, and a worker started then would be refused the lock and would say
/// that another installation is running, on Stopped, with no way back. So a
/// measurement let go of is still kept until its measure entry has ended —
/// its thread ends only once that process has — and until then Install says
/// "This computer's disks are still being read. Try again in a moment."
/// (`State::ready`), the person still on Review: each press is answered at
/// once, and the wait is only as long as the measure entry's own, which
/// holds the lock for exactly that long.
///
/// Refused, failed, or not waited for any longer, nothing is measured until
/// the disks are listed anew. Nothing it answers is trusted with a write:
/// the worker measures again itself.
#[derive(Debug, Default)]
pub struct Measuring {
    /// The measurement out now, whose answer arrives here.
    asking: Option<Receiver<MeasureEvent>>,
    /// When the measurement out now was asked, by the interface's clock
    /// (`State::clock`): it is waited for until [`READING_PATIENCE`] after.
    asked_at: f32,
    /// The disks were listed anew while it was out: what it answers is of
    /// disks that are not these any more, so it is let go of and they are
    /// asked about again then — never beside it, for each measurement takes
    /// the installer's lock, and a second would be refused it.
    again: bool,
    /// What the last answer found.
    found: Found,
    /// For the picture of a live system still reading (`partitions-reading`):
    /// the demo, which never measures, drawn as though it were.
    pictured: bool,
    /// Which of its readings the demo's made-up machine is pictured with:
    /// every Windows shut down fully, or — for the pictures of a hibernated
    /// Windows — the old disk's hibernated (`demo::Reading`).
    demo_reading: demo::Reading,
    /// Measurements not waited for any longer ([`READING_PATIENCE`]) whose
    /// measure entry may still be running, holding the installer's lock:
    /// each kept until its thread has ended — which it does only once the
    /// measure entry has — and its answer, should one come, ignored.
    lingering: Vec<Receiver<MeasureEvent>>,
}

/// What the measure entry's last answer found.
#[derive(Debug, Default)]
enum Found {
    /// Nothing asked yet, or what was read let go of with the disks it was
    /// read of.
    #[default]
    Nothing,
    /// Its answer: what it read of each partition it could, and what the
    /// image puts in each folder it fills.
    Read {
        measures: rules::Measures,
        image: ImageSizes,
    },
    /// Refused (another installation holds the lock …), or it could not be
    /// started, was not allowed, or said nothing that could be read.
    Failed,
}

impl Measuring {
    /// Whether a measurement is out: Partitions says "Reading…" and Install
    /// waits.
    pub fn reading(&self) -> bool {
        self.asking.is_some() || self.pictured
    }

    /// Whether a measure entry this interface started may still be running,
    /// holding the installer's lock: one out now, or one let go of whose
    /// thread has not ended ([`Measuring::lingering`], looked at again on
    /// every frame by [`poll`]). No installation is started while one is
    /// (`State::ready`), whatever the choice in use.
    pub fn running(&self) -> bool {
        self.asking.is_some() || !self.lingering.is_empty()
    }

    /// Lets go of each measurement let go of whose thread has ended: it
    /// answered — the answer ignored — or ended without a word.
    fn ended(&mut self) {
        self.lingering
            .retain(|lingering| matches!(lingering.try_recv(), Err(TryRecvError::Empty)));
    }

    /// Whether the measure entry has been asked about the disks as they are
    /// listed: it is out, or it answered (or failed) and nothing has been
    /// let go of since.
    fn asked(&self) -> bool {
        self.asking.is_some() || !matches!(self.found, Found::Nothing)
    }

    /// Whether reading the disks as they are listed has ended: answered,
    /// refused or failed. What it did not find is not going to be found
    /// until they are listed anew.
    fn over(&self) -> bool {
        !self.reading() && !matches!(self.found, Found::Nothing)
    }

    /// For the picture of a live system still reading (`partitions-reading`).
    pub fn picture_reading(&mut self) {
        self.pictured = true;
    }

    /// For the pictures of a hibernated Windows (`partitions-hibernated`,
    /// `partitions-hibernated-other`): the demo's old disk's Windows read
    /// as hibernated (`demo::Reading::Hibernated`).
    pub fn picture_hibernated(&mut self) {
        self.demo_reading = demo::Reading::Hibernated;
    }

    /// The measurement's answer, if it has come since the last look: taken,
    /// the measurement no longer out. A measurement that ended without one
    /// came to a failure.
    fn arrived(&mut self) -> Option<MeasureEvent> {
        let asking = self.asking.as_ref()?;
        let event = match asking.try_recv() {
            Ok(event) => event,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => MeasureEvent::Failed {
                failure: Failure::Internal,
                detail: "the measurement ended without an answer".into(),
            },
        };
        self.asking = None;
        Some(event)
    }
}

/// Ask the measure entry what it reads of the disks listed: on a live
/// system only — the demo has numbers of its own and a machine off the live
/// medium can install nothing — and never two at once: one asked while one
/// is out is asked once that one has answered, its answer let go of
/// ([`Measuring`]). No disk Partitions can show, nothing is asked.
///
/// Every disk listed is sent, not only those Partitions shows: the rule set
/// judges a shrink of an NTFS across every disk of the measurement
/// (`Measures::shrink_of`), for a hibernated Windows on any disk may hold an
/// NTFS on another. The measure entry reads a disk in use partition by
/// partition, as any other; one nothing may be made on (the installation
/// medium, a read-only disk, one not read completely) it does not read at
/// all, and answers its Windows volumes unread. A disk whose device or
/// partitions discovery could not have named (`disks::device_path_ok`) is
/// left out: the measure entry refuses a request naming one.
fn measure(state: &mut State) {
    if !matches!(state.machine, Machine::Real { live: Some(_) }) {
        return;
    }
    if state.measuring.asking.is_some() {
        state.measuring.again = true;
        return;
    }
    if !state.disks.iter().any(usable) {
        return;
    }
    let named = |disk: &&Disk| {
        device_path_ok(&disk.path) && disk.partitions.iter().all(|p| device_path_ok(&p.path))
    };
    let disks: Vec<Disk> = state.disks.iter().filter(named).cloned().collect();
    state.measuring.asking = Some((state.measure)(disks));
    state.measuring.asked_at = state.clock.now;
}

/// Fold in the measure entry's answer if it has arrived. Called on every
/// frame, whichever step is up, so an answer that arrives after the person
/// moved on still counts.
///
/// A measurement of disks listed anew since it was asked is let go of and
/// they are asked about again. A refusal or a failure leaves nothing
/// measured until the disks are listed anew, said in the journal. An answer
/// naming disks that are not as they were listed (`changed`, of which it
/// read nothing) has the disks listed again — read-only, as at the start
/// (`State::list_disks`) — which asks again where they did change, and
/// keeps the answer where the listing still stands.
///
/// A measurement out for [`READING_PATIENCE`] with no answer is not waited
/// for any longer ([`waited_long_enough`]); one let go of so is kept until
/// its measure entry has ended, and looked at again here.
pub fn poll(state: &mut State) {
    state.measuring.ended();
    let Some(event) = state.measuring.arrived() else {
        return waited_long_enough(state);
    };
    if std::mem::take(&mut state.measuring.again) {
        return measure(state);
    }
    match event {
        MeasureEvent::Answer(MeasureAnswer::Measured {
            measures,
            image,
            changed,
        }) => {
            state.measuring.found = Found::Read { measures, image };
            if !changed.is_empty() {
                match (state.list_disks)() {
                    Ok(disks) => disks_listed(state, disks),
                    Err(trouble) => eprintln!(
                        "lxb-installer: the disks could not be listed again after a measurement: {trouble}"
                    ),
                }
            }
        }
        MeasureEvent::Answer(MeasureAnswer::Refused { failure, detail })
        | MeasureEvent::Failed { failure, detail } => {
            eprintln!("lxb-installer: the disks could not be measured ({failure:?}): {detail}");
            state.measuring.found = Found::Failed;
        }
    }
}

/// A measurement out for [`READING_PATIENCE`] by the interface's clock with
/// no answer: what it has not answered — all of it — is unread, as though
/// it had failed, and nothing more is asked until the disks are listed anew
/// (the disks listed anew while it was out among them: a second measure
/// entry asked now would only be refused the installer's lock this one may
/// still hold). It is let go of, never killed — it runs as root, out of the
/// interface's reach — and kept among the lingering until its measure entry
/// has ended, so that no installation is started beside it; its answer,
/// should it come, counts for nothing.
fn waited_long_enough(state: &mut State) {
    let now = state.clock.now;
    let measuring = &mut state.measuring;
    if measuring.asking.is_none() || now - measuring.asked_at < READING_PATIENCE {
        return;
    }
    eprintln!(
        "lxb-installer: the disks' measurement has not answered in {READING_PATIENCE} s; \
         what it would have read is taken as unread"
    );
    measuring.lingering.extend(measuring.asking.take());
    measuring.again = false;
    measuring.found = Found::Failed;
}

/// The first thing Continue still needs, judged by the rule set the worker
/// judges a request by (`partitions::missing`, `model::layout`).
pub fn missing(state: &State) -> Option<Refusal> {
    let (measures, images) = measures_of(state);
    let judge = planning::Judge {
        firmware: state.firmware(),
        bootloader: state.draft.bootloader,
        software: &state.draft.software,
        measures: &measures,
        images: &images,
    };
    planning::missing(&state.views.partitions.plan, &state.disks, &judge)
}

/// [`missing`] in words, as the foot says it: a number reading has not
/// found — how much of a partition is used, how much Linexin puts in a
/// folder — "isn't known yet" while it might still come, and "couldn't be
/// read" once reading has ended without it.
pub fn missing_words(state: &State) -> Option<String> {
    let refusal = missing(state)?;
    let plan = &state.views.partitions.plan;
    let unread = reading_over(state);
    Some(match &refusal {
        Refusal::NotRead { part } if unread => crate::message!(
            "partitions-refused-unread",
            "partition" => planning::which_name(part, plan)
        ),
        Refusal::ImageUnknown { at } if unread => {
            crate::message!("partitions-refused-image-unread", "path" => at.path())
        }
        _ => planning::refusal_words(&refusal, plan),
    })
}

/// Continue: refused in words, in white, as every refusal is said, with the
/// first thing still missing — the very words the foot already says beside
/// the buttons, which then turn bold rather than being said a second time a
/// line under themselves. Once nothing is: **in the demo** the plan becomes
/// the choice in use and the person goes on to Desktop, and from there to
/// Review and the demo's rehearsal, which runs no worker; **on a live
/// system** only a plan whose every need lies within one set QEMU proved
/// does (`layout::unproven`: a mix one scenario of the campaign that passed
/// ran whole, `layout::PROVEN`); **off the live medium** never. Refused,
/// the foot's own line says why (`not_yet`).
fn onward(state: &mut State) {
    if missing(state).is_some() {
        state.sound(Sound::Error);
        state.views.partitions.refused = true;
        return;
    }
    go_on_with_the_plan(state);
}

/// On with the plan as the choice in use, nothing being missing (§7.0,
/// Unit Q, "The gate"): in the demo; on a live system only where everything
/// the plan needs lies within one set of `PROVEN` ([`all_proven`]); off the
/// live medium never.
/// Refused, with the first thing the plan asks that no scenario proved, or
/// "Partitions can't be applied in this version yet." (`not_yet`).
fn go_on_with_the_plan(state: &mut State) {
    let goes_on = match &state.machine {
        Machine::Demo => true,
        Machine::Real { live: Some(_) } => all_proven(state),
        Machine::Real { live: None } => false,
    };
    if !goes_on {
        return not_yet(state);
    }
    let view = &mut state.views.partitions;
    view.in_use = true;
    view.let_go = false;
    state.forward();
}

/// Whether the plan lays out and everything it needs lies within one set
/// QEMU proved: a mix one scenario that passed ran whole
/// (`layout::unproven`).
fn all_proven(state: &State) -> bool {
    planning::layout(&state.views.partitions.plan, &state.disks)
        .is_ok_and(|laid| rules::unproven(&laid, state.firmware()).is_none())
}

/// Continue refused off the demo, however complete the plan, in the foot's
/// own line: the first thing the plan asks of the worker that no scenario
/// of this version proved in QEMU (`model::layout::PROVEN`), in that
/// thing's words — "Shrinking a partition can't be applied in this version
/// yet." were no shrink proven; of what no set of this version holds, each
/// is said in Partitions' own sentence — or, each thing it asks proven but
/// never all together by one scenario, or all of it proven (off the live
/// medium, where nothing is installed), "Partitions can't be applied in
/// this version yet." (no new words: `planning::unproven_words`).
fn not_yet(state: &mut State) {
    state.sound(Sound::Error);
    let unproven = planning::layout(&state.views.partitions.plan, &state.disks)
        .ok()
        .and_then(|laid| rules::unproven(&laid, state.firmware()));
    let words = unproven.as_ref().map_or_else(
        || i18n::text("partitions-not-yet"),
        planning::unproven_words,
    );
    state.say(words, Role::Danger);
}

/// Continue on Where to install while the plan is the choice in use: what
/// Continue on Partitions does, the first thing missing said in the foot
/// (Where to install has no line of its own for it).
pub fn onward_from_where_to_install(state: &mut State) {
    if let Some(words) = missing_words(state) {
        state.sound(Sound::Error);
        state.say(words, Role::Danger);
        return;
    }
    go_on_with_the_plan(state);
}

/// Arrive on Partitions coming back from the step after Where to install
/// with the plan in use: the page the person went on from, the light on
/// Continue, where they left it.
pub fn came_back(state: &mut State) {
    let from = state.views.target.pair.left.at;
    let disk = state.views.partitions.disk;
    state.views.target.part = super::target::Part::Partitions;
    open(state, if showable(state, disk) { disk } else { from });
    state.views.partitions.focus = Focus::Buttons(CONTINUE);
}

/// Change the plan with `change`. The light stays on what it was on — or,
/// where that is gone, on what took its place (the unallocated space a
/// deleted partition became, the one run a new table leaves) — and a
/// refusal said before is let go of: it was about the plan as it was. A
/// partition the plan made where it has no room any more goes with it
/// (`Plan::prune`).
pub(super) fn replan(state: &mut State, change: impl FnOnce(&mut Plan)) {
    let offset = lit_offset(state);
    change(&mut state.views.partitions.plan);
    let disks = state.disks.clone();
    let firmware = state.firmware();
    state.views.partitions.plan.prune(&disks, firmware);
    state.note = None;
    state.views.partitions.refused = false;
    if let (Some(offset), Some(planned)) = (offset, planned(state)) {
        if let Some(at) = planning::stop_at(&planned, offset) {
            state.views.partitions.stop = at;
        }
    }
    let count = stops(state).len();
    let view = &mut state.views.partitions;
    view.stop = view.stop.min(count.saturating_sub(1));
}

/// Put the light on the map's tile that starts at `offset` (`stop_at`): a
/// partition a sheet just made or resized.
pub(super) fn light_on_offset(state: &mut State, offset: u64) {
    if let Some(at) = planned(state).and_then(|planned| planning::stop_at(&planned, offset)) {
        let view = &mut state.views.partitions;
        view.stop = at;
        view.focus = Focus::Map;
    }
}

/// Something typed into one of a sheet's fields (`sheets::edited`).
pub fn edited(state: &mut State, field: FieldId, edit: &Edit) {
    sheets::edited(state, field, edit);
}

/// The board was put away with Done on one of a sheet's fields.
pub fn typed(state: &mut State, field: FieldId) {
    sheets::typed(state, field);
}

pub fn head(state: &State) -> Head {
    Head {
        mark: glyphs::DISK_PARTITIONS,
        title: i18n::text("partitions-title").to_string(),
        caption: shown(state).map(map::disk_line).unwrap_or_default(),
    }
}

/// Whether Accept on what the light is on does anything: a tile with a menu
/// (unallocated space where no partition can be made has none), the shown
/// disk's chip, a button.
fn acts(state: &State) -> bool {
    match state.views.partitions.focus {
        Focus::Map => lit_stop(state).is_some_and(|(planned, stop)| {
            let (measures, _) = measures_of(state);
            !planning::tile_rows(&state.views.partitions.plan, &planned, stop, &measures).is_empty()
        }),
        Focus::Chips => shown(state).is_some(),
        Focus::Buttons(_) => true,
    }
}

/// Choose where Accept does something, Options and Back always: a legend
/// naming a button that does nothing is worse than naming none.
pub fn hints(state: &State) -> Vec<Hint> {
    if state.views.partitions.sheet.is_some() {
        return sheets::hints(state);
    }
    let mut hints = Vec::new();
    if acts(state) {
        hints.push(hint(i18n::text("hint-choose"), Pictured::Accept));
    }
    hints.push(hint(i18n::text("hint-options"), Pictured::Options));
    hints.push(hint(i18n::text("hint-back"), Pictured::Back));
    hints
}

pub fn act(state: &mut State, action: Action, scroll: Option<Scroll>) {
    // A key or a pad moves the light: the map slides after it again.
    state.views.partitions.pointed_tile = false;
    if state.views.partitions.sheet.is_some() {
        return sheets::act(state, action, scroll);
    }
    state.views.partitions.pointed = None;
    match action {
        Action::Back => return super::target::close_partitions(state),
        Action::Previous | Action::Next => {
            let direction = if action == Action::Next { 1 } else { -1 };
            if step_disk(state, direction) {
                state.sound(Sound::Move);
            }
            return;
        }
        Action::Menu => return open_options(state, None),
        _ => {}
    }
    let view = &state.views.partitions;
    match view.focus {
        Focus::Chips => match action {
            Action::Left | Action::Right => {
                let direction = if action == Action::Right { 1 } else { -1 };
                if step_disk(state, direction) {
                    state.sound(Sound::Move);
                }
            }
            // A chip raises no menu (one there would be confusing): Accept
            // goes down onto its map, as Down does.
            Action::Down | Action::Accept => focus(state, Focus::Map),
            _ => {}
        },
        Focus::Map => {
            let (stop, count) = (view.stop, stops(state).len());
            match action {
                Action::Left if stop > 0 => {
                    state.views.partitions.stop -= 1;
                    state.sound(Sound::Move);
                }
                Action::Right if stop + 1 < count => {
                    state.views.partitions.stop += 1;
                    state.sound(Sound::Move);
                }
                Action::Up => focus(state, Focus::Chips),
                Action::Down => focus(state, Focus::Buttons(CONTINUE)),
                // Unallocated space where no partition can be made has no
                // menu: Accept on it does nothing, and says nothing it would
                // do (the details say why).
                Action::Accept => open_tile(state, None),
                _ => {}
            }
        }
        Focus::Buttons(at) => match action {
            Action::Left if at > OPTIONS => focus(state, Focus::Buttons(at - 1)),
            Action::Right if at < CONTINUE => focus(state, Focus::Buttons(at + 1)),
            Action::Up => focus(state, Focus::Map),
            Action::Accept if at == CONTINUE => onward(state),
            Action::Accept => open_options(state, None),
            _ => {}
        },
    }
}

// -------------------------------------------------------------------------
// Menus.

/// The menus this page raises, by their numbers: clear of Where to
/// install's own, which the same screen answers.
const TILE_MENU: u32 = 11;
const MOUNT_MENU: u32 = 12;
const FORMAT_MENU: u32 = 13;
// 14 is left unused (a disk chip raises no menu): a table is made from the
// unallocated space or Options.
const TABLE_MENU: u32 = 15;
const OPTIONS_MENU: u32 = 16;
const SWAP_MENU: u32 = 17;
const SPACE_MENU: u32 = 18;

/// A row of Options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionRow {
    UndoLast,
    Clear,
    /// Pending operations (N)…: the whole list.
    Pending,
    /// Swap file · 4 GiB ›, offered while no partition is used as swap.
    Swap,
    /// Create partition table, over the shown disk's partitions: on a disk
    /// that has a table (one that has none is given it from its unallocated
    /// space), is not blocked, and has nothing written over the whole of it
    /// (`partitions::table_offered`).
    Table,
}

/// A menu this page raised, and what its rows are.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Raised {
    Tile {
        disk: String,
        part: String,
        rows: Vec<TileRow>,
    },
    Mount {
        disk: String,
        part: String,
        /// The mount points offered by name (`partitions::mount_offers`),
        /// and whether Other… comes after them.
        offered: Vec<Mount>,
        other: bool,
    },
    Format {
        disk: String,
        part: String,
        /// Whether keep is offered: not on a partition the plan makes.
        keep: bool,
        /// The filesystems offered (`partitions::format_offers`).
        offered: Vec<Fs>,
    },
    Table {
        disk: String,
        tables: Vec<Table>,
        /// Raised from the unallocated space of a disk with no table, rather
        /// than from Options.
        from_tile: bool,
    },
    Options {
        rows: Vec<OptionRow>,
    },
    Swap,
    /// A menu a sheet's row raised (`sheets::POSITION_MENU` and the rest).
    Sheet(u32),
    /// The menu of a run of unallocated space: New….
    Space {
        start: u64,
        size: u64,
    },
}

/// What a menu grows out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grows {
    Tile,
    Options,
    /// The row of a sheet the light is on.
    Sheet,
}

impl Raised {
    fn id(&self) -> u32 {
        match self {
            Raised::Tile { .. } => TILE_MENU,
            Raised::Mount { .. } => MOUNT_MENU,
            Raised::Format { .. } => FORMAT_MENU,
            Raised::Table { .. } => TABLE_MENU,
            Raised::Options { .. } => OPTIONS_MENU,
            Raised::Swap => SWAP_MENU,
            Raised::Sheet(id) => *id,
            Raised::Space { .. } => SPACE_MENU,
        }
    }

    /// A tile's menus grow out of the tile, and Options' out of its
    /// button, however it was asked for; the partition table's out of
    /// whichever of the two raised it.
    fn grows(&self) -> Grows {
        match self {
            Raised::Tile { .. }
            | Raised::Mount { .. }
            | Raised::Format { .. }
            | Raised::Space { .. }
            | Raised::Table {
                from_tile: true, ..
            } => Grows::Tile,
            Raised::Table { .. } | Raised::Options { .. } | Raised::Swap => Grows::Options,
            Raised::Sheet(_) => Grows::Sheet,
        }
    }
}

/// Remember that a sheet raised the menu numbered `id`: its answer is the
/// sheet's, and it grows out of the sheet's row.
pub(super) fn raised_from_sheet(state: &mut State, id: u32) {
    state.views.partitions.raised = Some(Raised::Sheet(id));
}

/// Have the frame raise a menu of `commands` — with, for any of them,
/// shorter words said where the whole does not fit — and remember what it
/// is.
fn raise(
    state: &mut State,
    raised: Raised,
    title: String,
    commands: impl Into<Rows>,
    marked: Option<usize>,
    on: Option<usize>,
) {
    let Rows { commands, shorter } = commands.into();
    state.menu(Menu {
        id: raised.id(),
        title: Some(title),
        commands,
        named: Vec::new(),
        shorter,
        marked,
        on,
    });
    state.views.partitions.raised = Some(raised);
}

/// A menu's rows, and shorter words for any of them.
struct Rows {
    commands: Vec<String>,
    shorter: Vec<Option<String>>,
}

impl From<Vec<String>> for Rows {
    fn from(commands: Vec<String>) -> Self {
        Rows {
            commands,
            shorter: Vec::new(),
        }
    }
}

impl From<(Vec<String>, Vec<Option<String>>)> for Rows {
    fn from((commands, shorter): (Vec<String>, Vec<Option<String>>)) -> Self {
        Rows { commands, shorter }
    }
}

/// Name what the menu just asked for grows out of, where it stood on the
/// last frame drawn: the tile the light is on, or Options.
fn anchor(state: &mut State) {
    let view = &state.views.partitions;
    let drawn = &view.drawn;
    let rect = match view.raised.as_ref().map(Raised::grows) {
        Some(Grows::Sheet) => view
            .sheet
            .as_ref()
            .and_then(|sheet| drawn.sheet_rows.get(sheet.at).copied().flatten()),
        Some(Grows::Tile) => drawn
            .tiles
            .get(view.stop)
            .and_then(|rect| inside_cut(*rect, drawn.map_view))
            .map(where_its_name_is),
        Some(Grows::Options) | None => None,
    }
    .or(drawn.options);
    if let Some(rect) = rect {
        state.anchor_menus(rect);
    }
}

/// The start of a tile, where its name stands, for a menu to grow out of:
/// no wider than one and a half times its height. The toolkit sets a menu
/// beside what it grows out of, or against the window's edge where there is
/// no room beside it — so a menu grown out of a tile as wide as the disk (an
/// empty disk's unallocated space) would stand in the window's corner, far
/// from what was pressed.
fn where_its_name_is(tile: [f32; 4]) -> [f32; 4] {
    let [x, y, width, height] = tile;
    [x, y, width.min(height * 1.5), height]
}

/// Where a menu a sheet raised grows out of: its row, as the last frame
/// drew it.
pub(super) fn anchor_sheet(state: &mut State) {
    anchor(state);
}

/// A mount point as a menu row says it: its path, or none.
fn mount_words(at: Option<Mount>) -> String {
    at.map_or_else(
        || i18n::text("partitions-none").to_string(),
        |at| at.path().to_string(),
    )
}

/// A format as a menu row says it: the filesystem, or keep.
fn format_words(fs: Option<Fs>) -> String {
    fs.map_or_else(
        || i18n::text("partitions-keep").to_string(),
        |fs| fs.name().to_string(),
    )
}

/// The swap file as Options says it: its size in whole GiB, or none.
fn swap_words(mib: u32) -> String {
    if mib == 0 {
        i18n::text("partitions-none").to_string()
    } else {
        format!("{}\u{a0}GiB", mib / 1024)
    }
}

/// Raise the menu of the tile the light is on, its title the partition's
/// device (or "New Partition #N"): on unallocated space New…; on a
/// partition Mount point · none, Format as · keep, Resize…, Label…, Delete,
/// and Undo changes to it once the plan does anything to it — what cannot
/// be done to it left out (`partitions::tile_rows`). Nothing on a tile with
/// no rows. `on`: the row the light comes back to after a choice inside it.
fn open_tile(state: &mut State, on: Option<TileRow>) {
    let Some(disk) = shown(state).map(|disk| disk.path.clone()) else {
        return;
    };
    let Some((planned, stop)) = lit_stop(state) else {
        return;
    };
    let (measures, _) = measures_of(state);
    let plan = &state.views.partitions.plan;
    let rows = planning::tile_rows(plan, &planned, stop, &measures);
    if rows.is_empty() {
        return;
    }
    let part = match stop {
        Stop::Part(at) | Stop::Extended(at) => planned.disk.partitions[at].path.clone(),
        Stop::Free { .. } if rows == [TileRow::Table] => {
            // A disk with no partition table: the table to make, right
            // where the person pressed.
            return open_table(state, disk, true);
        }
        Stop::Free { start, size } => {
            // Unallocated space: New…, which raises its sheet.
            let title = i18n::text("partitions-unallocated").to_string();
            let commands = vec![i18n::text("partitions-new").to_string()];
            return raise(
                state,
                Raised::Space { start, size },
                title,
                commands,
                None,
                on.map(|_| 0),
            );
        }
        Stop::Whole => return,
    };
    let commands: Vec<String> = rows
        .iter()
        .map(|row| match row {
            TileRow::New => i18n::text("partitions-new").to_string(),
            TileRow::Table => i18n::text("partitions-create-table").to_string(),
            TileRow::Mount => crate::message!(
                "partitions-menu-mount",
                "value" => mount_words(plan.mount_of(&part))
            ),
            TileRow::Format => crate::message!(
                "partitions-menu-format",
                "value" => format_words(plan.format_of(&part))
            ),
            TileRow::Resize => i18n::text("partitions-resize").to_string(),
            TileRow::Label => i18n::text("partitions-relabel").to_string(),
            TileRow::Delete => i18n::text("partitions-delete").to_string(),
            TileRow::Undo => {
                crate::message!("partitions-undo-part", "partition" => part.as_str())
            }
        })
        .collect();
    // Where Undo changes to a long device does not fit the menu (a display
    // standing on its side), it says Undo changes: the title names it.
    let shorter = rows
        .iter()
        .map(|row| (*row == TileRow::Undo).then(|| i18n::text("partitions-undo").to_string()))
        .collect();
    let on = on.and_then(|row| rows.iter().position(|one| *one == row));
    let title = planning::part_name(&part);
    raise(
        state,
        Raised::Tile { disk, part, rows },
        title,
        (commands, shorter),
        None,
        on,
    );
}

/// The mount points a partition can be given, the one in force marked:
/// `/`, `/home`, `/efi`, `/boot`, swap, Other… (or the path of its own in
/// force), none — fewer where the partition cannot be used everywhere
/// (`partitions::mount_offers`): a kept EFI system partition `/efi` and
/// `/boot` alone, `/` only to what can be ext4 or Btrfs.
fn open_mount(state: &mut State, disk: String, part: String) {
    let now = state.views.partitions.plan.mount_of(&part);
    let marks = planned(state)
        .and_then(|planned| planned.index_of(&part).map(|at| planned.parts[at].clone()));
    let (offered, other) = marks.as_ref().map_or_else(
        || (Mount::ALL.to_vec(), true),
        |marks| (planning::mount_offers(marks), planning::offers_other(marks)),
    );
    let (commands, marked) = sheets::mount_rows(&offered, other, now.as_ref());
    let title = i18n::text("partitions-mount-point").to_string();
    raise(
        state,
        Raised::Mount {
            disk,
            part,
            offered,
            other,
        },
        title,
        commands,
        Some(marked),
        None,
    );
}

/// What a partition can be formatted as, the one in force marked: keep,
/// ext4, btrfs, xfs, fat32, linux-swap — at `/` ext4 and btrfs alone
/// (`partitions::format_offers`) — and for a partition the plan makes,
/// which holds nothing to keep, the filesystems alone.
fn open_format(state: &mut State, disk: String, part: String) {
    let plan = &state.views.partitions.plan;
    let now = plan.format_of(&part);
    let keep = plan.created(&part).is_none();
    let offered = planning::format_offers(plan.mount_of(&part).as_ref());
    let mut commands = Vec::new();
    if keep {
        commands.push(format_words(None));
    }
    commands.extend(offered.iter().map(|fs| fs.name().to_string()));
    let marked = now
        .and_then(|now| offered.iter().position(|fs| *fs == now))
        .map_or(0, |at| at + usize::from(keep));
    let title = i18n::text("partitions-format-as").to_string();
    raise(
        state,
        Raised::Format {
            disk,
            part,
            keep,
            offered,
        },
        title,
        commands,
        Some(marked),
        None,
    );
}

/// The partition tables the disk may be given on this machine, each a row
/// saying all of it ("Create gpt partition table"): gpt, and msdos only
/// where a BIOS starts it. The title is the disk's device, whose table it
/// is; none is marked, since writing the table the disk has already still
/// deletes every partition on it. `from_tile`: raised from the unallocated
/// space of a disk that has none, else from Options.
fn open_table(state: &mut State, disk: String, from_tile: bool) {
    let tables = planning::tables(state.firmware());
    let commands: Vec<String> = tables
        .iter()
        .map(|table| {
            crate::message!(
                "partitions-create-table-kind",
                "table" => planning::table_name(*table)
            )
        })
        .collect();
    let title = disk.clone();
    raise(
        state,
        Raised::Table {
            disk,
            tables,
            from_tile,
        },
        title,
        commands,
        None,
        None,
    );
}

/// The rows Options offers now: Undo last operation, Clear all operations
/// and Pending operations (N)… while there are any, Create partition table
/// where the shown disk has one to write over (`table`), and the swap file
/// while no partition is used as swap.
pub fn option_rows(plan: &Plan, table: bool) -> Vec<OptionRow> {
    let mut rows = Vec::new();
    if !plan.is_empty() {
        rows.extend([OptionRow::UndoLast, OptionRow::Clear, OptionRow::Pending]);
    }
    if table {
        rows.push(OptionRow::Table);
    }
    if !planning::swap_partition(plan) {
        rows.push(OptionRow::Swap);
    }
    rows
}

/// Whether Options offers a new partition table over the shown disk: one
/// that is not blocked, has a table as the plan leaves it and nothing
/// written over the whole of it (`partitions::table_offered`; the details
/// say why not, [`held_back`]). A disk with none is given one from its
/// unallocated space instead.
fn table_offered(state: &State) -> bool {
    planned(state).is_some_and(|planned| planning::table_offered(&planned))
}

/// Raise Options, the light on `on` where a choice inside it brought the
/// person back, else on its first row.
fn open_options(state: &mut State, on: Option<OptionRow>) {
    let table = table_offered(state);
    let plan = &state.views.partitions.plan;
    let rows = option_rows(plan, table);
    if rows.is_empty() {
        return;
    }
    let count = plan.len() as i64;
    let swap = state.draft.storage.swap_mib;
    let commands: Vec<String> = rows
        .iter()
        .map(|row| match row {
            OptionRow::UndoLast => i18n::text("partitions-undo-last").to_string(),
            OptionRow::Clear => i18n::text("partitions-clear").to_string(),
            OptionRow::Pending => crate::message!("partitions-pending-list", "count" => count),
            OptionRow::Swap => {
                crate::message!("partitions-swap-file", "value" => swap_words(swap))
            }
            OptionRow::Table => i18n::text("partitions-create-table").to_string(),
        })
        .collect();
    let on = on.and_then(|row| rows.iter().position(|one| *one == row));
    let title = i18n::text("target-options").to_string();
    raise(state, Raised::Options { rows }, title, commands, None, on);
}

/// The sizes of swap file Where to install offers as spare memory, in GiB,
/// the one in force marked: the same choice, read and set in the same place.
fn open_swap(state: &mut State) {
    let sizes = super::target::SWAP_SIZES;
    let marked = sizes
        .iter()
        .position(|mib| *mib == state.draft.storage.swap_mib);
    let commands: Vec<String> = sizes.iter().map(|mib| swap_words(*mib)).collect();
    let title = i18n::text("partitions-swap-file-title").to_string();
    raise(state, Raised::Swap, title, commands, marked, None);
}

/// The row of a menu this page raised.
pub fn chose(state: &mut State, id: u32, row: usize) {
    let Some(raised) = state.views.partitions.raised.take() else {
        return;
    };
    if raised.id() != id {
        return;
    }
    let at = |state: &State, part: &str| planned(state).and_then(|one| one.index_of(part));
    match raised {
        Raised::Tile { disk, part, rows } => match rows.get(row) {
            Some(TileRow::Mount) => open_mount(state, disk, part),
            Some(TileRow::Format) => open_format(state, disk, part),
            Some(TileRow::Resize) => {
                if let Some(at) = at(state, &part) {
                    sheets::open_resize(state, at);
                }
            }
            Some(TileRow::Label) => {
                if let Some(at) = at(state, &part) {
                    sheets::open_label(state, at);
                }
            }
            Some(TileRow::Delete) => replan(state, |plan| plan.delete(&disk, &part)),
            Some(TileRow::Undo) => replan(state, |plan| {
                plan.undo_part(&part);
            }),
            Some(TileRow::Table) => open_table(state, disk, true),
            Some(TileRow::New) | None => {}
        },
        Raised::Mount {
            disk,
            part,
            offered,
            other,
        } => {
            // The presets, then Other… (a sheet of its own), then none.
            if other && row == offered.len() {
                return sheets::open_other(state, disk, part);
            }
            let at = offered.get(row).cloned();
            replan(state, |plan| {
                plan.mount(&disk, &part, at);
            });
            open_tile(state, Some(TileRow::Mount));
        }
        Raised::Format {
            disk,
            part,
            keep,
            offered,
        } => {
            let fs = row
                .checked_sub(usize::from(keep))
                .and_then(|at| offered.get(at))
                .copied();
            replan(state, |plan| {
                plan.format(&disk, &part, fs);
            });
            open_tile(state, Some(TileRow::Format));
        }
        Raised::Sheet(id) => sheets::chose(state, id, row),
        Raised::Space { start, size } => sheets::open_new(state, start, size),
        Raised::Table { disk, tables, .. } => {
            if let Some(table) = tables.get(row).copied() {
                replan(state, |plan| plan.table(&disk, table));
            }
        }
        Raised::Options { rows } => match rows.get(row) {
            Some(OptionRow::UndoLast) => replan(state, |plan| {
                plan.undo_last();
            }),
            Some(OptionRow::Clear) => replan(state, Plan::clear),
            Some(OptionRow::Pending) => state.ask(Question::Pending),
            Some(OptionRow::Swap) => open_swap(state),
            Some(OptionRow::Table) => {
                if let Some(disk) = shown(state).map(|disk| disk.path.clone()) {
                    open_table(state, disk, false);
                }
            }
            None => {}
        },
        Raised::Swap => {
            if let Some(mib) = super::target::SWAP_SIZES.get(row) {
                state.draft.storage.swap_mib = *mib;
            }
            open_options(state, Some(OptionRow::Swap));
        }
    }
}

/// Every pending operation, in GParted's words, the oldest first, one to a
/// line: the body of Options' Pending operations (`Question::Pending`) —
/// each broken into the lines it takes in `room` by `measure`, the lines
/// after its first standing in from its bullet (a hanging indent: in Polish
/// on a display standing on its side "• Ustaw punkt montowania" ran on under
/// its bullet), and as many of the latest as `most` lines hold, the oldest
/// left out with a line first saying how many.
pub fn pending_list(
    state: &State,
    most: usize,
    room: f32,
    measure: impl FnMut(&str) -> f32,
) -> String {
    let sentences: Vec<String> = state
        .views
        .partitions
        .plan
        .operations()
        .iter()
        .map(|op| planning::sentence(op, &state.disks))
        .collect();
    if sentences.is_empty() {
        return i18n::text("partitions-pending-none").to_string();
    }
    bulleted(&sentences, most, room, measure)
}

/// What the plan destroys, as its request's consent carries it: the rule
/// set's own list (`layout::destroys`) of the plan as a layout — disk by
/// disk, by their paths: a new table with every partition it erases, then
/// each partition deleted, shrunk or formatted, by number. What it makes in
/// unallocated space, mounts or labels destroys nothing and is not in it.
/// Nothing for a plan that cannot be laid out.
pub fn destroys(state: &State) -> Vec<Destroy> {
    planning::layout(&state.views.partitions.plan, &state.disks)
        .map(|laid| rules::destroys(&laid))
        .unwrap_or_default()
}

/// Whether the plan in use mounts a partition of its own at `/var`, beside
/// which restore points are switched off (Unit Q's decision 8): Desktop
/// says so under them.
pub fn separate_var(state: &State) -> bool {
    in_use(state)
        && planning::layout(&state.views.partitions.plan, &state.disks)
            .is_ok_and(|laid| rules::separate_var(&laid))
}

/// One thing the plan destroys as its question says it, in GParted's words
/// as Partitions speaks: its sentence first — "Create gpt partition table
/// on /dev/sdb", "Delete /dev/sda5 (ntfs, 278.7 GiB)", "Shrink /dev/sda3
/// from 372.5 GiB to 300.0 GiB", "Format /dev/sda4 as fat32" — then what
/// goes with it, a line each: every partition a new table erases, and a
/// formatted partition's new size where its entry is resized. A partition
/// that is an EFI system partition, or Windows' own, is named so
/// ("… · EFI system partition", "… · Windows"). Sizes and devices are the
/// partitions' as they were reviewed (`disks`).
pub fn destroy_rows(destroy: &Destroy, disks: &[Disk]) -> Vec<String> {
    let reviewed = |path: &str| disks.iter().find_map(|disk| disk.partition(path));
    let named = |sentence: String, path: &str| match reviewed(path).map(|part| part.content) {
        Some(Content::Efi) => name_after(
            sentence,
            |what| crate::message!("erase-plan-esp", "what" => what.to_string()),
        ),
        Some(Content::Windows | Content::WindowsRecovery | Content::MicrosoftReserved) => {
            name_after(
                sentence,
                |what| crate::message!("erase-plan-windows", "what" => what.to_string()),
            )
        }
        _ => sentence,
    };
    let filesystem = |path: &str| {
        reviewed(path).map_or_else(
            || i18n::text("partitions-unknown").to_string(),
            |part| map::filesystem_words(part).to_string(),
        )
    };
    let resized = |path: &str, from: u64, to: u64| {
        let shrunk = to < from;
        let (from, to, partition) = (map::size(from), map::size(to), path.to_string());
        if shrunk {
            crate::message!(
                "partitions-op-shrink",
                "partition" => partition,
                "from" => from,
                "to" => to
            )
        } else {
            crate::message!(
                "partitions-op-grow",
                "partition" => partition,
                "from" => from,
                "to" => to
            )
        }
    };
    match destroy {
        Destroy::Table {
            disk,
            table,
            erases,
        } => {
            let mut rows = vec![crate::message!(
                "partitions-op-table",
                "table" => planning::table_name(*table),
                "disk" => disk.as_str()
            )];
            for path in erases {
                let size = reviewed(path).map_or_else(String::new, |part| map::size(part.size));
                let erased = crate::message!(
                    "erase-plan-erased",
                    "partition" => path.as_str(),
                    "filesystem" => filesystem(path),
                    "size" => size
                );
                rows.push(named(erased, path));
            }
            rows
        }
        Destroy::Delete { part, size, .. } => {
            let sentence = crate::message!(
                "partitions-op-delete",
                "partition" => part.as_str(),
                "filesystem" => filesystem(part),
                "size" => map::size(*size)
            );
            vec![named(sentence, part)]
        }
        Destroy::Shrink { part, from, to, .. } => vec![named(resized(part, *from, *to), part)],
        Destroy::Format {
            part, size, to, fs, ..
        } => {
            let sentence = crate::message!(
                "partitions-op-format",
                "partition" => part.as_str(),
                "filesystem" => fs.name()
            );
            let mut rows = vec![named(sentence, part)];
            if to != size {
                rows.push(resized(part, *size, *to));
            }
            rows
        }
    }
}

/// `sentence` with what it is about named after it by `named`, a catalog's
/// "{ $what } · EFI system partition": the name held together and to the
/// mark before it, so that a line broken there breaks before the mark and
/// never ends on a lone "·" with the name on the next.
pub fn name_after(sentence: String, named: impl Fn(&str) -> String) -> String {
    const HERE: &str = "\u{1}";
    let framed = named(HERE);
    match framed.split_once(HERE) {
        Some((before, after)) => {
            let name = after.trim_start();
            let gap = &after[..after.len() - name.len()];
            format!("{before}{sentence}{gap}{}", text::unbroken(name))
        }
        None => framed,
    }
}

/// The pages of the plan's last question, as `most` lines of a dialog
/// `room` wide hold them (`measure`): each with the line saying what
/// follows ("These operations change what is on your disks:"), as many of
/// the things the plan destroys as fit — each a bullet, what goes with it
/// under it ([`destroy_rows`]) — and, where there is more than one page,
/// which page it is of how many; the last with "This cannot be undone."
/// under them. **Every line of the consent is said, and nothing is ever
/// left out with a count** (by design): a thing that fits a page
/// starts on a page it fits on whole, and one longer than a page — a new
/// table erasing many partitions — goes on over the next, its sentence said
/// again at the top. Where the plan destroys nothing, one page saying so.
pub fn plan_pages(
    state: &State,
    most: usize,
    room: f32,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<String> {
    let destroyed = destroys(state);
    if destroyed.is_empty() {
        return vec![i18n::text("erase-plan-nothing").to_string()];
    }
    let head = text::break_lines(i18n::text("erase-plan"), room, &mut measure);
    let tail = text::break_lines(i18n::text("erase-final"), room, &mut measure);
    let bullet = "•\u{a0}";
    let dash = "–\u{a0}";
    let indent = hanging(bullet, &mut measure);
    let under = format!("{indent}{dash}");
    let under_indent = format!("{indent}{}", hanging(dash, &mut measure));
    let items: Vec<Vec<Vec<String>>> = destroyed
        .iter()
        .map(|destroy| {
            destroy_rows(destroy, &state.disks)
                .iter()
                .enumerate()
                .map(|(at, row)| {
                    let (lead, hang) = if at == 0 {
                        (bullet, indent.as_str())
                    } else {
                        (under.as_str(), under_indent.as_str())
                    };
                    listed_lines(row, lead, hang, room, &mut measure)
                })
                .collect()
        })
        .collect();
    let pages = paginate(&items, head.len(), tail.len(), most);
    let count = pages.len();
    pages
        .into_iter()
        .enumerate()
        .map(|(at, body)| {
            let mut lines = head.clone();
            lines.extend(body);
            if at + 1 == count {
                lines.extend(tail.iter().cloned());
            }
            if count > 1 {
                lines.push(crate::message!(
                    "erase-plan-page",
                    "page" => (at + 1) as i64,
                    "pages" => count as i64
                ));
            }
            lines.join("\n")
        })
        .collect()
}

/// The body lines of each page of the plan's question, `items` being the
/// things it destroys, each a list of rows broken into lines, and each page
/// holding `head` lines over them and the last `tail` lines under: one page
/// where all of it fits `most` lines; otherwise pages of `most` less the
/// head and the line saying which page it is. A thing starts on a fresh
/// page where it would fit one whole but not what is left of this one (the
/// last thing with the tail under it); a thing longer than a page breaks
/// between its rows, its first row (its sentence) said again at the top of
/// the page it goes on over. Every page holds at least one row, however few
/// lines the window has.
fn paginate(items: &[Vec<Vec<String>>], head: usize, tail: usize, most: usize) -> Vec<Vec<String>> {
    let lines = |rows: &[Vec<String>]| rows.iter().map(Vec::len).sum::<usize>();
    let all: usize = items.iter().map(|item| lines(item)).sum();
    if head + all + tail <= most {
        return vec![items.iter().flatten().flatten().cloned().collect()];
    }
    let room = most.saturating_sub(head + 1).max(1);
    let mut pages: Vec<Vec<String>> = vec![Vec::new()];
    for (index, item) in items.iter().enumerate() {
        let last_item = index + 1 == items.len();
        let whole = lines(item) + if last_item { tail } else { 0 };
        let page = pages.last().map_or(0, Vec::len);
        if page > 0 && page + whole > room && whole <= room {
            pages.push(Vec::new());
        }
        for (at, row) in item.iter().enumerate() {
            let last_row = last_item && at + 1 == item.len();
            let needs = row.len() + if last_row { tail } else { 0 };
            let page = pages.last().map_or(0, Vec::len);
            if page > 0 && page + needs > room {
                pages.push(Vec::new());
                if at > 0 {
                    if let Some(page) = pages.last_mut() {
                        page.extend(item[0].iter().cloned());
                    }
                }
            }
            if let Some(page) = pages.last_mut() {
                page.extend(row.iter().cloned());
            }
        }
    }
    pages
}

/// No-break spaces as wide as `lead` (a bullet and the space after it), as
/// near as whole ones come: the dialog writes what it is given, and a line
/// it is given whole it does not break again — so a line that hangs under
/// its bullet stays where it was put.
fn hanging(lead: &str, measure: &mut impl FnMut(&str) -> f32) -> String {
    // Whole no-break spaces, then the narrow ones (U+202F) that come
    // nearest the rest; both hold, so the line never breaks in them.
    let space = measure("\u{a0}").max(0.01);
    let narrow = measure("\u{202f}").max(0.01);
    let wide = measure(lead);
    let whole = (wide / space).floor() as usize;
    let rest = ((wide - whole as f32 * space) / narrow).round() as usize;
    let indent = format!("{}{}", "\u{a0}".repeat(whole), "\u{202f}".repeat(rest));
    if indent.is_empty() {
        "\u{a0}".to_string()
    } else {
        indent
    }
}

/// `sentence` broken into the lines it takes in `room`, the first after
/// `lead` and the rest after `hang`.
fn listed_lines(
    sentence: &str,
    lead: &str,
    hang: &str,
    room: f32,
    measure: &mut impl FnMut(&str) -> f32,
) -> Vec<String> {
    let inside = (room - measure(hang)).max(1.0);
    text::break_lines(sentence, inside, &mut *measure)
        .into_iter()
        .enumerate()
        .map(|(at, line)| {
            let lead = if at == 0 { lead } else { hang };
            format!("{lead}{line}")
        })
        .collect()
}

/// `sentences`, a bullet to each, broken into the lines they take in
/// `room` by `measure`, the lines after a sentence's first standing in from
/// its bullet — and as many of the latest as `most` lines hold, the oldest
/// left out with a line first saying how many.
fn bulleted(
    sentences: &[String],
    most: usize,
    room: f32,
    mut measure: impl FnMut(&str) -> f32,
) -> String {
    let bullet = "•\u{a0}";
    let indent = hanging(bullet, &mut measure);
    let said: Vec<Vec<String>> = sentences
        .iter()
        .map(|sentence| listed_lines(sentence, bullet, &indent, room, &mut measure))
        .collect();
    if said.is_empty() {
        return String::new();
    }
    let taken: Vec<usize> = said.iter().map(|item| item.len().max(1)).collect();
    let mut from = said.len();
    let mut used = 0;
    while from > 0 {
        let next = taken[from - 1];
        // Room is kept for the line saying how many are left out, unless
        // this one is the last to go in.
        let kept = if from > 1 { 1 } else { 0 };
        if used + next + kept > most {
            break;
        }
        used += next;
        from -= 1;
    }
    let from = from.min(said.len() - 1);
    let mut lines = Vec::new();
    if from > 0 {
        lines.push(crate::message!("partitions-pending-earlier", "count" => from as i64));
    }
    lines.extend(said[from..].iter().flatten().cloned());
    lines.join("\n")
}

/// The stage of a picture of this page, with the two disks the made-up
/// machine shows only here beside its own (`demo::partition_disks`).
///
/// Step 1's: `partitions` the Windows disk, the light on Windows itself, as
/// the design draws it; `partitions-nvme` the empty NVMe drive, one tile of
/// unallocated space; `partitions-msdos` the old disk with an msdos table,
/// the light on a logical partition inside the extended one's frame;
/// `partitions-tiny` the Linux disk, the light on its 1 MiB `bios_grub`;
/// `partitions-blocked` the Windows disk with the pointer resting on the
/// installer's own stick.
///
/// Step 2's, on the Windows disk: `partitions-menu` the menu of its Data
/// partition; `partitions-mount` its Mount point; `partitions-table` the
/// empty NVMe disk's unallocated space offering the table it has not got;
/// `partitions-planned` a plan in
/// force — the recovery partition deleted, Data formatted ext4 at `/`, the
/// EFI partition kept at `/efi` — the light on Data, the foot saying what is
/// still missing (a `/boot` beside the small EFI partition);
/// `partitions-options` that plan with Options up; `partitions-pending` its
/// whole list; `partitions-missing` Data at `/` but kept, Continue pressed,
/// the foot and the refusal saying it must be formatted.
///
/// Step 3's sheets, on the Windows disk: `partitions-new` New partition
/// over its 279.4 GiB of unallocated space, 120 GiB at its start, ext4 at
/// `/`; `partitions-new-held` its size held and ten steps up, the new tile
/// grown on the map; `partitions-new-typed` a number being typed for it;
/// `partitions-resize` Windows shrunk to 250 GiB; `partitions-label` Data's
/// label being changed to Photos — with a pad, on the installer's board;
/// `partitions-other` a mount point of one's own for Data, `/usr/local`,
/// refused; `partitions-new-planned` New Partition #1 added (ext4,
/// 120 GiB, `/`) with the EFI partition at `/efi`, the light on its tile and
/// the pending operations naming it.
///
/// Step 4's: `partitions-continue-refused` the whole plan of [`stage_plan`],
/// nothing missing, Continue pressed as off the demo — the refusal a live
/// system says, drawn by the very function that says it there
/// ([`not_yet`]): the demo itself would go on to Desktop with it;
/// `partitions-portrait-slid` the Windows disk with the light on its last
/// tile, Data, which a display standing on its side slides the map to.
///
/// Unit Q's: `partitions-unmountable` the Linux disk with the light on its
/// encrypted partition, which is given no mount point — its details say
/// why, under them; `partitions-measured` its home ext4, read — what it
/// uses in its details — and Resize raised over it, from the least its tool
/// names, above what it uses and a margin, shrunk to 120 GiB;
/// `partitions-hibernated` the old msdos disk with the light on its Windows,
/// read as hibernated (the made-up machine pictured so,
/// `demo::Reading::Hibernated`) — no Used, and under the details why it is
/// not shrunk and what to do; `partitions-hibernated-other` the same
/// machine with the light on the Windows disk's own Windows, read and
/// signed not hibernated yet held back by the old disk's, its Used shown
/// and the same why under the details; `partitions-reading` the Windows
/// disk as a live system shows it while the measure entry is still
/// reading, "Reading…" as Windows' Used.
pub fn stage(state: &mut State, scene: &str) {
    let (disk, stop) = match scene {
        "partitions-nvme" | "partitions-table" => ("/dev/nvme0n1", None),
        "partitions-msdos" => ("/dev/sdc", Some("/dev/sdc5")),
        "partitions-hibernated" => ("/dev/sdc", Some("/dev/sdc1")),
        "partitions-tiny" => ("/dev/sdd", Some("/dev/sdd1")),
        "partitions-unmountable" => ("/dev/sdd", Some("/dev/sdd5")),
        "partitions-measured" => ("/dev/sdd", Some("/dev/sdd4")),
        "partitions"
        | "partitions-blocked"
        | "partitions-resize"
        | "partitions-reading"
        | "partitions-hibernated-other" => ("/dev/sda", Some("/dev/sda3")),
        "partitions-new" | "partitions-new-held" | "partitions-new-typed" => ("/dev/sda", None),
        "partitions-continue-refused" => ("/dev/sda", Some("/dev/sda4")),
        _ => ("/dev/sda", Some("/dev/sda5")),
    };
    let Some(at) = state.disks.iter().position(|one| one.path == disk) else {
        return;
    };
    open(state, at);
    let plan = &mut state.views.partitions.plan;
    match scene {
        "partitions-planned" | "partitions-options" | "partitions-pending" => {
            plan.delete(disk, "/dev/sda4");
            plan.format(disk, "/dev/sda5", Some(Fs::Ext4));
            plan.mount(disk, "/dev/sda5", Some(Mount::Root));
            plan.mount(disk, "/dev/sda1", Some(Mount::Efi));
        }
        "partitions-missing" => {
            plan.mount(disk, "/dev/sda5", Some(Mount::Root));
        }
        "partitions-new-planned" => {
            let gap = state.disks[at].free[0];
            plan.create(
                disk,
                planning::NewPart {
                    number: plan.next_number(),
                    start: gap.start,
                    size: 120 * crate::model::disks::GIB,
                    fs: Fs::Ext4,
                    mount: Some(Mount::Root),
                    label: String::new(),
                },
            );
            plan.mount(disk, "/dev/sda1", Some(Mount::Efi));
        }
        _ => {}
    }
    if scene == "partitions-continue-refused" {
        stage_plan(state);
    }
    if let (Some(path), Some(planned)) = (stop, planned(state)) {
        let walk = map::stops(&map::tiles(&planned.disk));
        if let Some(index) = walk.iter().position(|stop| match stop {
            Stop::Part(part) => planned.disk.partitions[*part].path == path,
            _ => false,
        }) {
            state.views.partitions.stop = index;
        }
    }
    if scene.starts_with("partitions-new") {
        // The light on the unallocated space, or on what was made in it.
        let gap = state.disks[at].free[0];
        if let Some(planned) = planned(state) {
            if let Some(index) = planning::stop_at(&planned, gap.start) {
                state.views.partitions.stop = index;
            }
        }
    }
    state.views.partitions.point_at_blocked = scene == "partitions-blocked";
    if scene == "partitions-reading" {
        state.measuring.picture_reading();
    }
    if matches!(
        scene,
        "partitions-hibernated" | "partitions-hibernated-other"
    ) {
        state.measuring.picture_hibernated();
    }
    stage_sheet(state, scene, at);
    match scene {
        "partitions-menu" => open_tile(state, None),
        "partitions-mount" => {
            open_tile(state, None);
            let part = "/dev/sda5".to_string();
            open_mount(state, disk.to_string(), part);
        }
        "partitions-table" => open_tile(state, None),
        "partitions-options" => open_options(state, None),
        "partitions-pending" => state.ask(Question::Pending),
        "partitions-missing" => {
            state.views.partitions.focus = Focus::Buttons(CONTINUE);
            onward(state);
        }
        "partitions-continue-refused" => {
            state.views.partitions.focus = Focus::Buttons(CONTINUE);
            not_yet(state);
        }
        _ => {}
    }
}

/// The whole plan step 4's pictures show, on the made-up machine's Windows
/// disk — nothing missing, on its UEFI: Windows shrunk to 300 GiB; New
/// Partition #1 made at the start of the space that leaves, 160 GiB of ext4
/// at `/`; the EFI partition kept at `/efi`; the recovery partition
/// formatted FAT32 for `/boot` (the EFI partition is too small for the
/// kernels); and Data deleted. Six operations, three of which destroy
/// something: the shrink, the format, the delete.
pub fn stage_plan(state: &mut State) {
    use crate::model::disks::GIB;
    const DISK: &str = "/dev/sda";
    let Some(disk) = state.disks.iter().find(|one| one.path == DISK).cloned() else {
        return;
    };
    let Some(windows) = disk.partition("/dev/sda3").cloned() else {
        return;
    };
    let firmware = state.firmware();
    let plan = &mut state.views.partitions.plan;
    *plan = Plan::default();
    plan.resize(DISK, &windows.path, windows.size, 300 * GIB);
    let after = windows.start + 300 * GIB;
    let Some(room) = planning::apply(plan, &disk, firmware)
        .disk
        .free
        .iter()
        .find(|run| run.start >= after)
        .copied()
    else {
        return;
    };
    plan.create(
        DISK,
        planning::NewPart {
            number: plan.next_number(),
            start: room.start,
            size: 160 * GIB,
            fs: Fs::Ext4,
            mount: Some(Mount::Root),
            label: String::new(),
        },
    );
    plan.mount(DISK, "/dev/sda1", Some(Mount::Efi));
    plan.format(DISK, "/dev/sda4", Some(Fs::Fat32));
    plan.mount(DISK, "/dev/sda4", Some(Mount::Boot));
    plan.delete(DISK, "/dev/sda5");
}

/// The plan the pictures of the plan's paged question show
/// (`confirm-plan-pages`, `confirm-plan-last`): [`stage_plan`]'s on the
/// Windows disk, and a new gpt over the made-up archive disk
/// (`demo::archive_disk`, added for them alone), which erases its forty
/// partitions — more than one page of the question holds on any display.
pub fn stage_plan_pages(state: &mut State) {
    if !state.disks.iter().any(|disk| disk.path == "/dev/sde") {
        state.disks.push(demo::archive_disk());
    }
    stage_plan(state);
    state.views.partitions.plan.table("/dev/sde", Table::Gpt);
}

/// The plan `desktop-loaders-plan` shows, on the made-up machine's old
/// msdos disk (`demo::partition_disks`, added where they are not listed
/// yet): its logical partitions deleted from the last and the extended one
/// after them, and in their room a FAT32 EFI system partition alone at
/// `/boot` and ext4 at `/`, Windows kept — an EFI system partition on an
/// msdos table, which systemd-boot cannot start, so Desktop outlines it.
pub fn stage_plan_on_msdos(state: &mut State) {
    use crate::model::disks::GIB;
    const DISK: &str = "/dev/sdc";
    if !state.disks.iter().any(|disk| disk.path == DISK) {
        state.disks.extend(demo::partition_disks());
    }
    let Some(disk) = state.disks.iter().find(|one| one.path == DISK).cloned() else {
        return;
    };
    let firmware = state.firmware();
    let plan = &mut state.views.partitions.plan;
    *plan = Plan::default();
    for part in ["/dev/sdc7", "/dev/sdc6", "/dev/sdc5", "/dev/sdc2"] {
        plan.delete(DISK, part);
    }
    let Some(room) = planning::apply(plan, &disk, firmware)
        .disk
        .free
        .first()
        .copied()
    else {
        return;
    };
    for (start, size, fs, mount) in [
        (room.start, GIB, Fs::Fat32, Mount::Boot),
        (room.start + GIB, room.size - GIB, Fs::Ext4, Mount::Root),
    ] {
        let number = plan.next_number();
        plan.create(
            DISK,
            planning::NewPart {
                number,
                start,
                size,
                fs,
                mount: Some(mount),
                label: String::new(),
            },
        );
    }
}

/// Raise the sheet a picture of one shows (see [`stage`]).
fn stage_sheet(state: &mut State, scene: &str, at: usize) {
    use crate::model::disks::GIB;
    let gap = state.disks[at].free.first().copied();
    match scene {
        "partitions-new" | "partitions-new-held" | "partitions-new-typed" => {
            let Some(gap) = gap else {
                return;
            };
            sheets::open_new(state, gap.start, gap.size);
            let Some(sheet) = state.views.partitions.sheet.as_mut() else {
                return;
            };
            sheet.set_size(120 * GIB);
            if scene != "partitions-new" {
                sheet.held = Some(sheets::Held {
                    was: 120 * GIB,
                    typed: None,
                });
            }
            if scene == "partitions-new-held" {
                sheet.step(10);
            }
            if scene == "partitions-new-typed" {
                if let Some(held) = sheet.held.as_mut() {
                    held.typed = Some(i18n::decimal("200.5".to_string()));
                }
            }
        }
        "partitions-resize" => {
            if let Some(index) = planned(state).and_then(|one| one.index_of("/dev/sda3")) {
                sheets::open_resize(state, index);
            }
            if let Some(sheet) = state.views.partitions.sheet.as_mut() {
                sheet.set_size(250 * GIB);
            }
        }
        "partitions-measured" => {
            if let Some(index) = planned(state).and_then(|one| one.index_of("/dev/sdd4")) {
                sheets::open_resize(state, index);
            }
            if let Some(sheet) = state.views.partitions.sheet.as_mut() {
                sheet.set_size(120 * GIB);
            }
        }
        "partitions-label" => {
            // A label is set as a filesystem is made: Data formatted ext4.
            let plan = &mut state.views.partitions.plan;
            plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
            if let Some(index) = planned(state).and_then(|one| one.index_of("/dev/sda5")) {
                sheets::open_label(state, index);
            }
            if let Some(sheet) = state.views.partitions.sheet.as_mut() {
                if let sheets::Kind::Label(labelling) = &mut sheet.kind {
                    labelling.label = "Photos".into();
                }
            }
            // With a pad in hand, typed on the installer's board, as South
            // on the field opens it (under LineXinBar the picture types
            // with the shell's instead, `--shells-board`).
            if state.pad {
                state.open_board(FieldId::PartLabel);
            }
        }
        "partitions-other" => {
            sheets::open_other(state, "/dev/sda".into(), "/dev/sda5".into());
            if let Some(sheet) = state.views.partitions.sheet.as_mut() {
                if let sheets::Kind::Other(other) = &mut sheet.kind {
                    other.typed = "/usr/local".into();
                }
            }
        }
        _ => {}
    }
    if let Some(sheet) = state.views.partitions.sheet.as_mut() {
        sheet.settle();
    }
}

// -------------------------------------------------------------------------
// Drawing.

/// How tall a disk's chip is, in reference pixels.
const CHIP: f32 = 48.0;

/// The least width of a tile, in reference pixels: one a pointer lands on
/// and the light reads on, however small the partition — GParted's 1 MiB
/// slivers made into something a hand can reach.
const LEAST: f32 = 44.0;

/// How tall the band along a tile's top is, in reference pixels.
const BAND: f32 = 7.0;

/// How much of its glass a tile at rest keeps: what steps it back into the
/// page beside the lit one.
const RESTING: f32 = 0.62;

fn buttons() -> [Button<'static>; 2] {
    [
        Button::plain(i18n::text("button-options")),
        Button::primary(i18n::text("button-continue")),
    ]
}

/// One chip as it is drawn: the disk's place among the machine's disks,
/// where it stands, its words, and how many operations the plan has on it.
pub(super) struct Chip {
    pub(super) index: usize,
    rect: [f32; 4],
    words: String,
    pub(super) blocked: bool,
    /// The kind of disk it is (`glyphs::disk`), where there is room for it.
    mark: Option<&'static str>,
    planned: usize,
}

/// How far a chip's words and mark stand in from its ends.
fn chip_pad(page: &Page) -> f32 {
    page.scaled(14.0)
}

/// How large a chip's mark is, and the air after it.
fn chip_mark(page: &Page) -> (f32, f32) {
    (page.scaled(26.0), page.metric(Metric::Gap) * 0.45)
}

/// The type a chip's words are in: a caption's, a step under a button's,
/// so that on a landscape panel five disks still say their sizes on one line.
const CHIP_WORDS: Text = Text::Caption;

/// How wide and tall the count of a disk's pending operations is drawn on
/// its chip, after its words: a small capsule holding the number.
fn badge_size(page: &mut Page, count: usize) -> [f32; 2] {
    let tall = page.line(Text::Caption) * 0.86;
    let words = text::bold_width(page.ui(), CHIP_WORDS, &count.to_string());
    [(words + tall * 0.7).max(tall), tall]
}

/// How wide a chip saying `words` is, with its mark or without, and with
/// the count of its pending operations where it has any: its words in the
/// bold face, which the disk shown and a lit chip say them in.
fn chip_width(page: &mut Page, words: &str, marked: bool, planned: usize) -> f32 {
    let (mark, after) = chip_mark(page);
    let mark = if marked { mark + after } else { 0.0 };
    let badge = if planned > 0 {
        badge_size(page, planned)[0] + after
    } else {
        0.0
    };
    chip_pad(page) * 2.0 + mark + text::bold_width(page.ui(), CHIP_WORDS, words) + badge
}

/// The chips of every disk across `room`, from its top, on one line where
/// they can be: each the disk's mark, its device and its size; where those
/// do not all go on one line, the mark and the device alone (the head says
/// the size of the disk shown); then the device alone, as a display standing
/// on its side has room for; and where even that does not go on one line,
/// on as many as it needs. `planned` is how many operations the plan has on
/// each disk. Answers the chips and how tall they came out.
pub(super) fn lay_chips(
    page: &mut Page,
    disks: &[Disk],
    planned: &[usize],
    room: [f32; 4],
) -> (Vec<Chip>, f32) {
    let tall = page.scaled(CHIP);
    let between = page.metric(Metric::Gap) * 0.6;
    let whole: Vec<String> = disks.iter().map(map::chip_words).collect();
    let short: Vec<String> = disks
        .iter()
        .map(|disk| map::device(&disk.path).to_string())
        .collect();
    let count = |at: usize| planned.get(at).copied().unwrap_or(0);
    let row = |page: &mut Page, said: &[String], marked: bool| {
        said.iter()
            .enumerate()
            .map(|(at, words)| chip_width(page, words, marked, count(at)))
            .sum::<f32>()
            + between * said.len().saturating_sub(1) as f32
    };
    let (said, marked) = if row(page, &whole, true) <= room[2] {
        (whole, true)
    } else if row(page, &short, true) <= room[2] {
        (short, true)
    } else {
        (short, false)
    };
    let mut chips = Vec::new();
    let (mut x, mut y) = (room[0], room[1]);
    for (index, (disk, words)) in disks.iter().zip(said).enumerate() {
        let width = chip_width(page, &words, marked, count(index)).min(room[2]);
        if x > room[0] && x + width > room[0] + room[2] + 0.5 {
            x = room[0];
            y += tall + between;
        }
        chips.push(Chip {
            index,
            rect: [x, y, width, tall],
            words,
            blocked: !usable(disk),
            mark: marked.then(|| glyphs::disk(disk)),
            planned: count(index),
        });
        x += width + between;
    }
    let height = if chips.is_empty() {
        0.0
    } else {
        y + tall - room[1]
    };
    (chips, height)
}

/// Draw one chip: a capsule of control glass with the disk's mark and words;
/// the disk shown tinted with the accent while the light is elsewhere, as
/// the primary verb is; a disk that cannot be used outlined, its words
/// quieter, as a row that cannot be chosen is. A disk the plan does anything
/// on says how many operations, in a small capsule of the accent's soft
/// tint after its words — quiet, and read at a glance from any disk.
pub(super) fn draw_chip(
    page: &mut Page,
    chip: &Chip,
    shown: bool,
    lit: bool,
    pressing: Option<f32>,
) {
    let radius = chip.rect[3] * 0.5;
    let pad = chip_pad(page);
    let (mark, after) = chip_mark(page);
    let badge = (chip.planned > 0).then(|| badge_size(page, chip.planned));
    let icons = page.icons();
    let ui = page.ui();
    let (sunk, ink) = if chip.blocked {
        ui.control_out(chip.rect, radius, 1.0);
        (chip.rect, 0.55)
    } else {
        let press = match pressing {
            Some(through) if lit => Press::Going(through),
            _ => Press::from(lit),
        };
        let sunk = ui.control(chip.rect, radius, press, chip.rect[2], 1.0);
        if !lit {
            ui.chip(sunk, ui.tinted(Role::Glass, button::RESTING_WASH));
            if shown {
                ui.chip(sunk, ui.tinted(Role::Accent, 0.22));
            }
        }
        (sunk, if lit || shown { 1.0 } else { 0.86 })
    };
    // The words are laid in the chip as it was measured, not in its glass
    // sunk under a press, so a chip measured to its words never cuts them.
    let [x, _, w, _] = chip.rect;
    let [_, y, _, h] = sunk;
    let mut left = x + pad;
    if let Some(name) = chip.mark {
        ui.icon_tinted(
            [left, y + (h - mark) * 0.5, mark, mark],
            name,
            icons,
            Role::Text,
            ink,
        );
        left += mark + after;
    }
    let badged = badge.map_or(0.0, |[wide, _]| wide + after);
    let room = (x + w - pad - badged - left).max(1.0) + 1.0;
    let bold = lit || shown;
    let cut = if bold {
        text::fit_bold(ui, CHIP_WORDS, &chip.words, room)
    } else {
        text::fit(ui, CHIP_WORDS, &chip.words, room)
    };
    let tint = ui.tinted(Role::Text, ink);
    ui.label_weighted(
        [left, y, room, h],
        CHIP_WORDS,
        &cut,
        tint,
        Align::Left,
        bold,
    );
    if let Some([wide, tall]) = badge {
        let at = [x + w - pad - wide, y + (h - tall) * 0.5, wide, tall];
        ui.chip(at, ui.tinted(Role::AccentSoft, 0.92));
        let digits = ui.tinted(Role::AccentDeep, 1.0);
        ui.label_weighted(
            at,
            CHIP_WORDS,
            &chip.planned.to_string(),
            digits,
            Align::Centre,
            true,
        );
    }
}

/// How the map is measured on this page, at the page's scale.
pub(super) fn measures(page: &Page) -> Measures {
    let inset = page.scaled(8.0);
    Measures {
        least: page.scaled(LEAST),
        gap: page.scaled(4.0),
        inset,
        head: band_top(page) + page.scaled(BAND) + inset,
    }
}

/// The widest a tile's comfortable width is held to, in reference pixels:
/// a label is the person's own word and is cut to fit.
const COMFORTABLE: f32 = 230.0;

/// How wide a tile is comfortable — wide enough to say all of its words on
/// their lines, the "format" tag after its name and its mount point's
/// capsule among them (`draw_tile`), its label aside — and never narrower
/// than a tile's least width: what each tile of a map that slides is laid
/// out at, at the least.
fn comfortable(page: &mut Page, words: &TileWords) -> f32 {
    let pad = tile_pad(page);
    let line = page.line(Text::Caption);
    let mount_line = page.line(MOUNT_WORDS);
    let gap = page.metric(Metric::Gap);
    let (least, most) = (page.scaled(LEAST), page.scaled(COMFORTABLE));
    let ui = page.ui();
    let mut name = ui.measure(Text::Body, &words.name);
    if words.formatted && words.band.is_some() {
        let tag = i18n::text("partitions-tile-format");
        name += gap * 0.45 + text::bold_width(ui, Text::Caption, tag) + line * 0.82 * 0.8;
    }
    let mount = words.mount.as_deref().map_or(0.0, |at| {
        ui.measure(MOUNT_WORDS, at) + mount_line * MOUNT_PAD
    });
    let widest = name
        .max(ui.measure(Text::Caption, &words.size))
        .max(ui.measure(Text::Caption, &words.filesystem))
        .max(mount);
    (widest + pad * 2.0 + 1.0).clamp(least, most)
}

/// A map laid out to be drawn: its tiles where they stand on this frame,
/// and — where it slides — the room it is cut to.
pub(super) struct Mapped {
    pub(super) laid: Vec<Laid>,
    pub(super) cut: Option<[f32; 4]>,
    /// How far it moved under the light since the last frame drawn, along.
    pub(super) moved: f32,
    /// Whether its tiles are easing to new widths on this frame.
    pub(super) easing: bool,
}

/// Lay the map of `planned` out in `rect`. On a panel wide enough — every
/// landscape one — exactly as it always was (`map::lay`): every tile at
/// least its least width, the rest shared by size, the words thinning as a
/// tile narrows. Where the panel is narrow enough for Where to install's
/// columns to slide (a display standing on its side) and the tiles would
/// not keep their words side by side in it, the map is laid out wider than
/// the panel — each tile at least as wide as it is comfortable
/// ([`comfortable`]), a share of the room more by size (`map::spread`) —
/// and slides with the light like the Home menu's cards: the tile `keep`
/// (by its place in the laid map) always in view, the next one peeking in
/// at the edge (`map::slide_to`), on the cards' spring — at once in low-end,
/// and at once onto another disk. A tile the pointer lit is not slid under
/// it.
pub(super) fn lay_map(
    state: &mut State,
    page: &mut Page,
    planned: &Planned,
    rect: [f32; 4],
    keep: Option<usize>,
    plan: &Plan,
    at_once: bool,
) -> Mapped {
    let tiles = map::tiles(&planned.disk);
    let measures = measures(page);
    let scale = page.scaled(1.0).max(f32::EPSILON);
    let narrow = crate::ui::pair::beside_at(rect[2] / scale).1.slides();
    let leasts: Vec<(Stop, f32)> = if narrow {
        map::stops(&tiles)
            .into_iter()
            .map(|stop| (stop, comfortable(page, &tile_words(planned, stop))))
            .collect()
    } else {
        Vec::new()
    };
    let least = |tile: &Tile| {
        let stop = tile.stop();
        leasts
            .iter()
            .find(|(one, _)| *one == stop)
            .map_or(measures.least, |(_, least)| *least)
    };
    let (low_end, dt) = (state.low_end(), state.clock.dt);
    let view = &mut state.views.partitions;
    if !narrow || map::fits(&tiles, rect[2], measures, least) {
        view.slide = Slide::default();
        view.slid_by = 0.0;
        let laid = map::lay(&planned.disk, &tiles, rect, measures);
        let laid = ease_map(view, planned, rect, plan, laid, at_once || low_end, dt);
        return Mapped {
            laid,
            cut: None,
            moved: 0.0,
            easing: view.ease.moving(),
        };
    }
    let width = map::spread(&tiles, rect[2], measures, least);
    let along = [0.0, rect[1], width, rect[3]];
    let laid = map::lay_by(&planned.disk, &tiles, along, measures, least);
    let target_laid = laid.clone();
    let laid = ease_map(view, planned, rect, plan, laid, at_once || low_end, dt);
    if view.slid_for != Some(view.disk) || low_end {
        let target = view.slide.target;
        view.slide = Slide {
            target,
            ..Slide::default()
        };
        view.slid_for = Some(view.disk);
    }
    let was = view.slide.target;
    // Slid to where the lit tile is going, not where it is on its way.
    let target = match keep.and_then(|at| target_laid.get(at)) {
        Some(one) if !view.pointed_tile => {
            let lit = [one.rect[0], one.rect[0] + one.rect[2]];
            map::slide_to(was, lit, rect[2], width, rect[2] * layout::PEEK)
        }
        _ => was.clamp(0.0, (width - rect[2]).max(0.0)),
    };
    let pan = view.slide.follow(target, dt);
    let moved = view.slid_by - pan;
    view.slid_by = pan;
    let laid = laid
        .into_iter()
        .map(|mut one| {
            one.rect[0] += rect[0] - pan;
            one
        })
        .collect();
    Mapped {
        laid,
        cut: Some(rect),
        moved,
        easing: view.ease.moving(),
    }
}

/// Ease the map `laid` (along its own reach, before any slide) from where
/// its stops stood when the plan last changed to where they stand now: a
/// change to the plan drawn on the same disk in the same room — an
/// operation added or taken back, a sheet's choice — eases on the cards'
/// spring (`map::eased`), and a change of mind on the way eases on from
/// where the map had got to. Drawn at once: another disk, another room, the
/// first frame, a size moving on a sheet (`at_once`), low-end.
fn ease_map(
    view: &mut View,
    planned: &Planned,
    rect: [f32; 4],
    plan: &Plan,
    mut laid: Vec<Laid>,
    at_once: bool,
    dt: f32,
) -> Vec<Laid> {
    let now = map::spans(&planned.disk, &laid);
    let room = [rect[0], rect[2]];
    let same = view
        .map_was
        .as_ref()
        .filter(|was| was.disk == view.disk && was.room == room);
    match same {
        Some(was) if was.plan != *plan && !at_once => {
            view.ease_from = was.spans.clone();
            view.ease.begin();
        }
        Some(_) if !at_once => {}
        _ => view.ease.settle(),
    }
    let through = motion::ease(view.ease.advance(motion::CARD_SPRING, dt).clamp(0.0, 1.0));
    if view.ease.moving() {
        for (one, x) in laid
            .iter_mut()
            .zip(map::eased(&view.ease_from, &now, through))
        {
            one.rect[0] = x[0];
            one.rect[2] = (x[1] - x[0]).max(0.0);
        }
    }
    view.map_was = Some(MapWas {
        disk: view.disk,
        room,
        plan: plan.clone(),
        spans: map::spans(&planned.disk, &laid),
    });
    laid
}

/// The part of `rect` inside `cut`, along: where a tile of a map that
/// slides can be pointed at. `None` where none of it is.
pub(super) fn inside_cut(rect: [f32; 4], cut: Option<[f32; 4]>) -> Option<[f32; 4]> {
    let Some(cut) = cut else {
        return Some(rect);
    };
    let left = rect[0].max(cut[0]);
    let right = (rect[0] + rect[2]).min(cut[0] + cut[2]);
    (right - left > 0.5).then_some([left, rect[1], right - left, rect[3]])
}

/// Whether `rect` stands wholly inside `cut`, along.
fn wholly_in(rect: [f32; 4], cut: [f32; 4]) -> bool {
    rect[0] >= cut[0] - 0.5 && rect[0] + rect[2] <= cut[0] + cut[2] + 0.5
}

/// How far a tile's band stands in from its top and its sides.
fn band_top(page: &Page) -> f32 {
    page.scaled(7.0)
}

/// How far a tile's words stand in from its sides.
fn tile_pad(page: &Page) -> f32 {
    page.scaled(12.0)
}

/// The type a mount point is said in on its tile: larger than the tile's
/// other words, bold, white on a capsule of the accent — what the plan is
/// about.
const MOUNT_WORDS: Text = Text::Title;

/// How much wider than its words a mount point's capsule is, in its lines.
const MOUNT_PAD: f32 = 0.6;

/// How tall the map is with room for all of a tile's words: its band, its
/// name, its mount point where the plan gives the disk any, its filesystem,
/// its size and its label.
pub(super) fn map_height(page: &Page, mounted: bool) -> f32 {
    band_top(page)
        + page.scaled(BAND)
        + tile_pad(page) * 1.5
        + page.line(Text::Body)
        + if mounted { page.line(MOUNT_WORDS) } else { 0.0 }
        + page.line(Text::Caption) * 3.0
}

/// The least height of the map: a tile's band and its name.
pub(super) fn map_least(page: &Page) -> f32 {
    band_top(page) + page.scaled(BAND) + tile_pad(page) * 1.5 + page.line(Text::Body)
}

/// What a stop of the map says of itself on its tile, and the colour of its
/// band: its name, its filesystem (none for free space), its size and its
/// third line — its label, or the partitions a run of unallocated space was
/// — and, where the plan gives it one, its mount point and whether it is
/// formatted; `None` for its band where it is unallocated, whose band is
/// dashed.
pub(super) struct TileWords {
    name: String,
    filesystem: String,
    size: String,
    /// The size in whole units, for a tile that says its size alone and has
    /// no room for the decimal.
    short: Option<String>,
    label: String,
    band: Option<Srgb>,
    mount: Option<String>,
    formatted: bool,
}

pub(super) fn tile_words(planned: &Planned, stop: Stop) -> TileWords {
    let disk = &planned.disk;
    match stop {
        Stop::Part(at) | Stop::Extended(at) => {
            let part = &disk.partitions[at];
            let marks = &planned.parts[at];
            let size = match stop {
                Stop::Extended(_) => extended_bytes(disk, at),
                _ => part.size,
            };
            // A partition the plan makes is "new", as GParted's map says,
            // and needs no "format" beside it.
            let name = match marks.new {
                Some(_) => i18n::text("partitions-tile-new").to_string(),
                None => map::device(&part.path).to_string(),
            };
            TileWords {
                name,
                filesystem: map::filesystem_words(part).to_string(),
                size: map::size(size),
                short: Some(map::size_short(size)),
                label: part.label.trim().to_string(),
                band: Some(map::colour(map::filesystem(part))),
                mount: marks.mount.as_ref().map(|at| at.path().to_string()),
                formatted: marks.format.is_some() && marks.new.is_none(),
            }
        }
        Stop::Free { start, size } => {
            let deleted = planned.deleted_in(start, size);
            let label = if deleted.is_empty() || planned.table.is_some() {
                String::new()
            } else {
                let names: Vec<&str> = deleted.iter().map(|part| map::device(&part.path)).collect();
                crate::message!(
                    "partitions-tile-deleted",
                    "partitions" => names.join(", "),
                    "count" => names.len() as i64
                )
            };
            TileWords {
                name: i18n::text("partitions-unallocated").to_string(),
                filesystem: String::new(),
                size: map::size(size),
                short: Some(map::size_short(size)),
                label,
                band: None,
                mount: None,
                formatted: false,
            }
        }
        Stop::Whole => TileWords {
            name: map::device(&disk.path).to_string(),
            filesystem: i18n::text("partitions-unknown").to_string(),
            size: map::size(disk.size),
            short: Some(map::size_short(disk.size)),
            label: String::new(),
            band: Some(map::colour(None)),
            mount: None,
            formatted: false,
        },
    }
}

/// How far the extended partition at `at` really reaches, in bytes
/// (`map::extent`).
fn extended_bytes(disk: &Disk, at: usize) -> u64 {
    map::tiles(disk)
        .iter()
        .find_map(|tile| match tile {
            Tile::Extended { at: here, inside } if *here == at => {
                Some(map::extent(disk, at, inside))
            }
            _ => None,
        })
        .unwrap_or_else(|| disk.partitions[at].size)
}

/// How many dashes of about `dash` long, `space` apart, fill a band `long`
/// from end to end, and how long each is then.
fn dashes(long: f32, dash: f32, space: f32) -> (usize, f32) {
    let count = ((long + space) / (dash + space)).round().max(1.0) as usize;
    let each = (long - space * (count - 1) as f32) / count as f32;
    (count, each.max(1.0))
}

/// Draw one tile: the shell's glass, its band along its top in its
/// filesystem's colour (dashed and faint for unallocated space, on glass
/// washed back into the page), and as many of its words as its width says
/// (`map::said`): a pending format a small "format" capsule in the new
/// filesystem's colour after the name, the mount point under the name,
/// larger and in the accent's soft tint. An extended partition's frame says
/// nothing of itself: the tiles inside it stand under its band, and the
/// details say the rest. `kept`: the tile the details describe while the
/// light is elsewhere (on a chip or a button), which keeps its glass, as
/// the shown disk's chip keeps its tint, so the map still says which one
/// they are about.
pub(super) fn draw_tile(
    page: &mut Page,
    laid: &Laid,
    words: &TileWords,
    (lit, kept): (bool, bool),
    pressing: Option<f32>,
) {
    let rect = laid.rect;
    let radius = page
        .metric(Metric::CardRadius)
        .min(rect[2] * 0.5)
        .min(rect[3] * 0.5);
    let inset = band_top(page).min(rect[2] * 0.2);
    let band_tall = page.scaled(BAND);
    let pad = tile_pad(page).min(rect[2] * 0.25);
    let body = page.line(Text::Body);
    let line = page.line(Text::Caption);
    let mount_line = page.line(MOUNT_WORDS);
    let gap = page.metric(Metric::Gap);
    let ui = page.ui();
    let press = match pressing {
        Some(through) if lit => Press::Going(through),
        _ => Press::from(lit),
    };
    // A tile at rest steps back into the page, as a resting button does,
    // so the one under the light reads as the one in hand at a glance —
    // on a map of tiles of every width the light alone did not: a lit 1 MiB
    // sliver beside a resting partition read as two of a kind. Unallocated
    // space steps back further, with a faint edge round it: room, rather
    // than something in it. Faded glass rather than the buttons' wash,
    // which is a capsule, and a tile is not.
    let free = words.band.is_none();
    let glass = match (lit || kept, free) {
        (true, false) => 1.0,
        (true, true) if lit => 1.0,
        (true, true) => RESTING,
        (false, false) => RESTING,
        (false, true) => RESTING * 0.6,
    };
    let sunk = ui.control(rect, radius, press, rect[2], glass);
    if free && !lit {
        ui.control_out(sunk, radius, 0.6);
    }
    let band = [
        sunk[0] + inset,
        sunk[1] + inset,
        (sunk[2] - inset * 2.0).max(1.0),
        band_tall,
    ];
    match words.band {
        Some(colour) => ui.chip(band, colour.a(0.95)),
        None => {
            // A dashed band: room that is there to be used, as the free
            // space's mark draws it — whole dashes from end to end, never
            // one cut to a dot at the end.
            let space = band_tall * 1.2;
            let (count, dash) = dashes(band[2], band_tall * 2.2, space);
            let tint = ui.tinted(Role::Text, 0.35);
            for index in 0..count {
                let x = band[0] + index as f32 * (dash + space);
                ui.chip([x, band[1], dash, band[3]], tint);
            }
        }
    }
    if matches!(laid.stop, Stop::Extended(_)) {
        return;
    }
    let room = [
        (sunk[2] - pad * 2.0).max(0.0),
        (sunk[3] - inset - band_tall - pad * 1.5).max(0.0),
    ];
    let short = words.short.as_deref().unwrap_or(&words.size);
    let measured = Words {
        name: ui.measure(Text::Body, &words.name),
        size: ui.measure(Text::Caption, &words.size),
        short: ui.measure(Text::Caption, short),
        filesystem: ui.measure(Text::Caption, &words.filesystem),
        labelled: !words.label.is_empty(),
        name_line: body,
        line,
        mount: words
            .mount
            .as_deref()
            .map(|at| ui.measure(MOUNT_WORDS, at) + mount_line * MOUNT_PAD),
        mount_line,
    };
    let (mut said, mounted) = map::said(room, measured);
    let (mut room, mut x) = (room, sunk[0] + pad);
    if said == Said::Nothing {
        // Too narrow for its name, a tile may still say its size, standing
        // closer to its edges: a run of unallocated space on a display
        // standing on its side said nothing at all.
        let tight = [(sunk[2] - pad).max(0.0), room[1]];
        if map::said(tight, measured).0 == Said::Size {
            (said, room, x) = (Said::Size, tight, sunk[0] + pad * 0.5);
        }
    }
    let ink = if lit { 1.0 } else { 0.9 };
    let mut y = band[1] + band_tall + pad * 0.5;
    if said >= Said::Name {
        text::label_faded(
            ui,
            [x, y, room[0], body],
            Text::Body,
            &words.name,
            Role::Text,
            ink,
            Align::Left,
        );
        // A pending format: "format" in a small capsule of the new
        // filesystem's colour, after the name, where it fits beside it.
        if let (true, Some(colour)) = (words.formatted, words.band) {
            let tag = i18n::text("partitions-tile-format");
            let tall = line * 0.82;
            let wide = text::bold_width(ui, Text::Caption, tag) + tall * 0.8;
            let at = x + measured.name + gap * 0.45;
            if at + wide <= x + room[0] + 0.5 {
                let pill = [at, y + (body - tall) * 0.55, wide, tall];
                ui.chip(pill, colour.a(0.92));
                let deep = ui.tinted(Role::AccentDeep, 1.0);
                ui.label_weighted(pill, Text::Caption, tag, deep, Align::Centre, true);
            }
        }
        y += body;
        if let (true, Some(at)) = (mounted, words.mount.as_deref()) {
            // The mount point in a capsule of the accent, white on it: the
            // plan itself, read from across the map, on a lit tile as on one
            // at rest.
            let tall = mount_line * 0.86;
            let wide = (ui.measure(MOUNT_WORDS, at) + mount_line * MOUNT_PAD).min(room[0]);
            let pill = [x, y + (mount_line - tall) * 0.5, wide, tall];
            ui.chip(pill, ui.tinted(Role::Accent, 1.0));
            let white = ui.tinted(Role::Text, 1.0);
            ui.label_tinted(pill, MOUNT_WORDS, at, white, Align::Centre);
            y += mount_line;
        }
    }
    let lines: Vec<&str> = match said {
        Said::All => [words.filesystem.as_str(), &words.size, &words.label]
            .into_iter()
            .filter(|one| !one.is_empty())
            .collect(),
        Said::NameAndSize => vec![words.size.as_str()],
        // Its size alone, in whole units where the decimal does not fit.
        Said::Size if measured.size <= room[0] + 0.5 => vec![words.size.as_str()],
        Said::Size => vec![short],
        Said::Name | Said::Nothing => Vec::new(),
    };
    let (role, alpha) = text::LESSER;
    for one in lines {
        // Half a pixel more than it was measured against (`map::said`), so
        // a size that fits is never cut by the measure's rounding.
        text::label_faded(
            ui,
            [x, y, room[0] + 0.5, line],
            Text::Caption,
            one,
            role,
            alpha * ink,
            Align::Left,
        );
        y += line;
    }
    // A tile with no room to say where it is mounted still shows that it
    // is: a bead of the accent at its foot, under its words — or, on a map
    // too short for that, at the right-hand end of its name's line (the
    // details say where).
    if words.mount.is_some() && !mounted {
        let bead = line * 0.34;
        let foot = sunk[1] + sunk[3] - pad.max(inset) - bead;
        let at = if foot >= y {
            [sunk[0] + (sunk[2] - bead) * 0.5, foot, bead, bead]
        } else {
            let name_y = band[1] + band_tall + pad * 0.5;
            [
                sunk[0] + sunk[2] - pad - bead,
                name_y + (body - bead) * 0.5,
                bead,
                bead,
            ]
        };
        ui.chip(at, ui.tinted(Role::Accent, 1.0));
    }
}

/// What a line of the details names, GParted's columns and its information
/// window's: each said from the catalogs by its own literal message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    Partition,
    Filesystem,
    Label,
    Size,
    Used,
    MountPoint,
    Flags,
    Pending,
    Start,
    End,
}

impl Detail {
    /// Every name a line of the details can have: what the catalogs' test
    /// holds to the room the details get.
    pub const ALL: [Detail; 10] = [
        Detail::Partition,
        Detail::Filesystem,
        Detail::Label,
        Detail::Size,
        Detail::Used,
        Detail::MountPoint,
        Detail::Flags,
        Detail::Pending,
        Detail::Start,
        Detail::End,
    ];

    pub fn words(self) -> &'static str {
        match self {
            Detail::Partition => i18n::text("partitions-partition"),
            Detail::Filesystem => i18n::text("partitions-filesystem"),
            Detail::Label => i18n::text("partitions-label"),
            Detail::Size => i18n::text("partitions-size"),
            Detail::Used => i18n::text("partitions-used"),
            Detail::MountPoint => i18n::text("partitions-mount-point"),
            Detail::Flags => i18n::text("partitions-flags"),
            Detail::Pending => i18n::text("partitions-pending"),
            Detail::Start => i18n::text("partitions-start"),
            Detail::End => i18n::text("partitions-end"),
        }
    }
}

/// One line of the details: what it names, and its value.
pub(super) type Pair = (Detail, String);

/// The details of what the light is on, as the plan leaves it — or, where
/// the pointer rests on a disk that cannot be used, why it cannot — in two
/// columns: the left one what the partition is, the right one what is or
/// will be done with it: its mount point, and what is pending, in short
/// (`Format as ext4`, the mount point having its own row); for a run of
/// unallocated space a
/// deletion made, the deletions.
pub(super) fn details(state: &State, planned: &Planned, stop: Stop) -> [Vec<Pair>; 2] {
    let dash = || "—".to_string();
    let or_dash = |said: Vec<String>| {
        if said.is_empty() {
            dash()
        } else {
            said.join(" · ")
        }
    };
    let measured = measured_of(state);
    let disk = &planned.disk;
    match stop {
        Stop::Part(at) | Stop::Extended(at) => {
            let part = &disk.partitions[at];
            let marks = &planned.parts[at];
            let size = match stop {
                Stop::Extended(_) => extended_bytes(disk, at),
                _ => part.size,
            };
            let mut left = vec![
                (Detail::Partition, planning::part_name(&part.path)),
                (Detail::Filesystem, map::filesystem_words(part).to_string()),
                (Detail::Size, map::size(size)),
            ];
            // What the partition holds now, read as the disk holds it — or
            // that it is being read; a partition formatted afresh, or made,
            // will hold nothing.
            if marks.format.is_none() && marks.new.is_none() {
                match measured(&marks.was) {
                    Some(Read::Found { measure, .. }) => {
                        if let Some(used) = measure.used() {
                            left.push((Detail::Used, map::size(used)));
                        }
                    }
                    Some(Read::Reading) => {
                        let reading = i18n::text("partitions-reading").to_string();
                        left.push((Detail::Used, reading));
                    }
                    _ => {}
                }
            }
            let label = part.label.trim();
            let flags = map::flags(part);
            let right = vec![
                (
                    Detail::Label,
                    if label.is_empty() {
                        dash()
                    } else {
                        label.to_string()
                    },
                ),
                (
                    Detail::Flags,
                    if flags.is_empty() {
                        dash()
                    } else {
                        flags.join(", ")
                    },
                ),
                (
                    Detail::MountPoint,
                    marks
                        .mount
                        .as_ref()
                        .map_or_else(dash, |at| at.path().into()),
                ),
                (
                    Detail::Pending,
                    or_dash(planning::pending_words(marks, part)),
                ),
            ];
            [left, right]
        }
        Stop::Free { start, size } => {
            let mut right = vec![
                (Detail::Start, map::mib(start)),
                (Detail::End, map::mib(start + size)),
            ];
            let deleted: Vec<String> = if planned.table.is_some() {
                let table = planned.table.map(planning::table_name).unwrap_or_default();
                vec![crate::message!(
                    "partitions-op-table",
                    "table" => table,
                    "disk" => disk.path.as_str()
                )]
            } else {
                planned
                    .deleted_in(start, size)
                    .iter()
                    .map(|part| {
                        crate::message!("partitions-do-delete", "partition" => part.path.as_str())
                    })
                    .collect()
            };
            if !deleted.is_empty() {
                right.push((Detail::Pending, deleted.join(" · ")));
            }
            [
                vec![
                    (
                        Detail::Partition,
                        i18n::text("partitions-unallocated").to_string(),
                    ),
                    (Detail::Size, map::mib(size)),
                ],
                right,
            ]
        }
        Stop::Whole => [
            vec![
                (Detail::Partition, disk.path.clone()),
                (
                    Detail::Filesystem,
                    i18n::text("partitions-unknown").to_string(),
                ),
                (Detail::Size, map::size(disk.size)),
            ],
            vec![(Detail::MountPoint, dash()), (Detail::Pending, dash())],
        ],
    }
}

/// How the details stand in a room `room` wide whose names are at most
/// `names` wide and values at most `values` wide, `gap` between a name and
/// its value and twice that between two columns: in two columns where each
/// column's name and value fit side by side, and otherwise in one.
pub fn details_columns(room: f32, names: f32, values: f32, gap: f32) -> usize {
    let column = (room - gap * 2.0) * 0.5;
    if names + gap + values <= column {
        2
    } else {
        1
    }
}

/// How wide the widest name and the widest value of the details are. What
/// is pending is left out of the values: it is said in parts, and as many
/// of them as fit (`text::fit_parts`) — the mount point beside it says the
/// one it gives up.
fn measured(page: &mut Page, columns: &[Vec<Pair>; 2]) -> (f32, f32) {
    let ui = page.ui();
    let names = columns
        .iter()
        .flatten()
        .map(|(name, _)| ui.measure(Text::Caption, name.words()))
        .fold(0.0f32, f32::max);
    let values = columns
        .iter()
        .flatten()
        .filter(|(name, _)| *name != Detail::Pending)
        .map(|(_, value)| ui.measure(Text::Body, value))
        .fold(0.0f32, f32::max);
    (names, values)
}

/// The columns the details are laid in across a room `room` wide: where
/// each starts, its rows, and how wide a column, a name and a value are.
struct Columns<'a> {
    laid: Vec<(f32, Vec<&'a Pair>)>,
    name_w: f32,
    value_w: f32,
}

fn columns_in<'a>(page: &mut Page, room: [f32; 4], columns: &'a [Vec<Pair>; 2]) -> Columns<'a> {
    let gap = page.metric(Metric::Gap);
    let (names, values) = measured(page, columns);
    // Lines in one column only (a sheet's few on a display standing on its
    // side) stand in one.
    let two = !columns[1].is_empty() && details_columns(room[2], names, values, gap) == 2;
    let laid: Vec<(f32, Vec<&Pair>)> = if two {
        let width = (room[2] - gap * 2.0) * 0.5;
        vec![
            (room[0], columns[0].iter().collect()),
            (room[0] + width + gap * 2.0, columns[1].iter().collect()),
        ]
    } else {
        vec![(room[0], columns.iter().flatten().collect())]
    };
    let column_w = if laid.len() == 2 {
        (room[2] - gap * 2.0) * 0.5
    } else {
        room[2]
    };
    let name_w = names.min(column_w * 0.5);
    Columns {
        laid,
        name_w,
        value_w: column_w - name_w - gap,
    }
}

/// The lines a value of the details is said on in `room`: one, cut where it
/// does not fit — but what is pending, which is said in parts ("Format as
/// ext4 · Mount at /"), where the whole does not fit on one line, a part to
/// a line, each wrapped onto two where even it does not: on a display
/// standing on its side, "Форматировать в linux-swap" is wider than the
/// value's room, and cut it said nothing.
pub fn value_lines(
    room: f32,
    detail: Detail,
    value: &str,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<String> {
    if detail != Detail::Pending || measure(value) <= room {
        return vec![text::cut_to(value, room, measure)];
    }
    value
        .split(" · ")
        .flat_map(|part| text::wrapped_by(part, room, 2, &mut measure))
        .collect()
}

/// How many lines the details take in a room `room` wide: those of the
/// longer column side by side, or all of them in one — a value said on
/// more than one line counted with all of them.
pub(super) fn details_lines(page: &mut Page, room: f32, columns: &[Vec<Pair>; 2]) -> usize {
    let laid = columns_in(page, [0.0, 0.0, room, 0.0], columns);
    let ui = page.ui();
    laid.laid
        .iter()
        .map(|(_, pairs)| {
            pairs
                .iter()
                .map(|(name, value)| {
                    value_lines(laid.value_w, *name, value, |line| {
                        ui.measure(Text::Body, line)
                    })
                    .len()
                })
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0)
}

/// The details `columns` made to fit `room` tall, `width` wide, over the
/// reason said under them (`why_tall`): where the map could not give up its
/// least height for them — a display standing on its side, a partition
/// with much to say and a reason under it — the rows that say nothing, a
/// dash, make way, the last first, so nothing is drawn over the foot.
/// Answers the rows left and the height they and the reason take.
fn make_way(
    page: &mut Page,
    width: f32,
    mut columns: [Vec<Pair>; 2],
    why_tall: f32,
    room: f32,
) -> ([Vec<Pair>; 2], f32) {
    let line = page.line(Text::Body);
    loop {
        let tall = line * details_lines(page, width, &columns).max(2) as f32 + why_tall;
        if tall <= room + 0.5 {
            return (columns, tall);
        }
        let dash = columns.iter().enumerate().rev().find_map(|(side, pairs)| {
            pairs
                .iter()
                .rposition(|(_, value)| value == "—")
                .map(|at| (side, at))
        });
        match dash {
            Some((side, at)) => {
                columns[side].remove(at);
            }
            None => return (columns, tall),
        }
    }
}

/// Draw the details in `room`: two columns of a name and its value, or one
/// where two would cut a value ([`details_columns`]), what is pending on
/// more than one line where it needs them ([`value_lines`]). Lines past the
/// room's foot are not drawn.
pub(super) fn draw_details(page: &mut Page, room: [f32; 4], columns: &[Vec<Pair>; 2]) {
    let gap = page.metric(Metric::Gap);
    let line = page.line(Text::Body);
    let Columns {
        laid,
        name_w,
        value_w,
    } = columns_in(page, room, columns);
    let ui = page.ui();
    let (role, alpha) = text::LESSER;
    let foot = room[1] + room[3] + 0.5;
    for (x, pairs) in laid {
        let mut y = room[1];
        for (name, value) in pairs {
            if y + line > foot {
                break;
            }
            text::label_faded(
                ui,
                [x, y, name_w, line],
                Text::Caption,
                name.words(),
                role,
                alpha,
                Align::Left,
            );
            let said = value_lines(value_w, *name, value, |one| ui.measure(Text::Body, one));
            for one in said {
                if y + line > foot {
                    break;
                }
                ui.label(
                    [x + name_w + gap, y, value_w, line],
                    Text::Body,
                    &one,
                    Role::Text,
                    Align::Left,
                );
                y += line;
            }
        }
    }
}

/// What the details say under themselves of why the tile at `stop` of
/// `planned` offers less than it might, as GParted does: unallocated space
/// no partition can be made in (something written over the whole disk, a
/// table to make first, four primary partitions already, the inside of an
/// extended partition, or beside one kept), a kept filesystem the installer
/// cannot mount, a logical partition that cannot go before the ones after it
/// (`partitions::held_back`), a kept ext4 or NTFS that reading says cannot
/// be shrunk ([`unshrunk`]; `measured`, what reading found:
/// [`measured_of`]) — and, under any tile of a disk something is written
/// over as a whole that holds nothing back of its own, that no table or
/// partition can be made on it, which is why Options offers no new table
/// there (`partitions::whole_held_back`). Under every tile of a disk whose
/// logical sector the rule set lays nothing out in, first and alone, that
/// nothing can be changed on it (`partitions::sectors_held_back`): no tile
/// of it offers anything, so nothing of its own is held back.
pub(super) fn held_back(
    planned: &Planned,
    stop: Stop,
    measured: &impl Fn(&Partition) -> Option<Read>,
) -> Option<String> {
    if let Some(why) = planning::sectors_held_back(planned) {
        return Some(why);
    }
    match stop {
        Stop::Free { start, .. } => planning::can_create(&planned.disk, start)
            .err()
            .map(|why| why.words().to_string()),
        Stop::Part(at) => {
            planning::held_back(planned, at).or_else(|| unshrunk(planned, at, measured))
        }
        Stop::Extended(_) | Stop::Whole => None,
    }
    .or_else(|| planning::whole_held_back(planned))
}

/// Why the kept ext4 or NTFS at `at` of `planned` is not offered to be
/// shrunk, as reading said (`partitions::unshrunk_words`): of it — Windows
/// hibernated, a check due, in use, encrypted, unreadable, an NTFS with no
/// hibernation file of its own — or, for an NTFS, of another
/// Windows volume of the measurement: hibernated, unread, BitLocker; or,
/// reading over with nothing of it, unreadable. Nothing while it is read,
/// once it is read and may be shrunk, off the live medium (where nothing is
/// read), and for a partition the plan formats or makes, whose entry alone
/// is resized.
fn unshrunk(
    planned: &Planned,
    at: usize,
    measured: &impl Fn(&Partition) -> Option<Read>,
) -> Option<String> {
    let (part, marks) = (planned.disk.partitions.get(at)?, planned.parts.get(at)?);
    if marks.format.is_some() || marks.new.is_some() || !rules::shrinkable(&marks.was) {
        return None;
    }
    let why = match measured(&marks.was)? {
        Read::Reading | Read::Found { shrink: Ok(_), .. } => return None,
        Read::Found {
            shrink: Err(why), ..
        } => why,
        Read::Unread => rules::Unshrinkable::Itself(Measure::Unreadable),
    };
    Some(planning::unshrunk_words(&part.path, &why))
}

/// What a tile says of why it offers less ([`held_back`]), broken onto the
/// lines it takes in a room `room` wide beside the warning mark — two at
/// most.
fn why_lines(page: &mut Page, words: &str, room: f32) -> Vec<String> {
    let (mark, after) = warning_mark(page);
    text::wrapped(page.ui(), Text::Body, words, room - mark - after, 2)
}

/// Draw why a tile offers less ([`held_back`]) in `room`: the warning mark
/// and the words, in white, as every refusal is said.
fn draw_why(page: &mut Page, room: [f32; 4], said: &[String]) {
    let line = page.line(Text::Body);
    let (mark, after) = warning_mark(page);
    let icons = page.icons();
    let ui = page.ui();
    let (role, alpha) = text::REFUSAL;
    ui.icon_tinted(
        [room[0], room[1] + (line - mark) * 0.5, mark, mark],
        glyphs::WARNING,
        icons,
        role,
        alpha,
    );
    for (index, one) in said.iter().enumerate() {
        text::label_faded(
            ui,
            [
                room[0] + mark + after,
                room[1] + index as f32 * line,
                room[2] - mark - after,
                line,
            ],
            Text::Body,
            one,
            role,
            alpha,
            Align::Left,
        );
    }
}

/// Why the disk the pointer rests on cannot be used: the disk as the head
/// says one, and the reason in plain words, as Where to install says it —
/// with, for one in use, where it is mounted, which this page may say. Each
/// wraps rather than being cut, on two lines at most, as the head's caption
/// does: on a display standing on its side the disk's line is longer than
/// the room.
fn draw_blocked(page: &mut Page, room: [f32; 4], disk: &Disk) {
    let line = page.line(Text::Body);
    let mut reason = disk
        .blocked
        .as_ref()
        .map(words::blocked)
        .unwrap_or_default()
        .to_string();
    if let Some(Blocked::InUse { mountpoints }) = &disk.blocked {
        if !mountpoints.is_empty() {
            reason = format!("{reason} · {}", mountpoints.join(", "));
        }
    }
    let ui = page.ui();
    let said = text::wrapped(ui, Text::Body, &map::disk_line(disk), room[2], 2);
    let why = text::wrapped(ui, Text::Body, &reason, room[2], 2);
    let (role, alpha) = text::REFUSAL;
    for (index, one) in said.iter().chain(&why).enumerate() {
        let y = room[1] + index as f32 * line;
        if y + line > room[1] + room[3] + 0.5 {
            break;
        }
        text::label_faded(
            ui,
            [room[0], y, room[2], line],
            Text::Body,
            one,
            role,
            alpha,
            Align::Left,
        );
    }
}

/// The colour of the dot before a pending operation in the list under the
/// details, which ties it to the map: a format the new filesystem's band, a
/// mount point the accent's soft tint, a deletion the faint white of
/// unallocated space's dashes, a new table the panel's white.
fn operation_dot(page: &mut Page, op: &Operation) -> [f32; 4] {
    let ui = page.ui();
    match op {
        Operation::Format { fs, .. } => map::colour(Some(fs.name())).a(0.95),
        Operation::Mount { .. } => ui.tinted(Role::AccentSoft, 1.0),
        Operation::Delete { .. } => ui.tinted(Role::Text, 0.4),
        Operation::Table { .. } => ui.tinted(Role::Text, 0.9),
        Operation::Create { new, .. } => match new.fs {
            Fs::BiosGrub => map::colour(None).a(0.95),
            fs => map::colour(Some(fs.name())).a(0.95),
        },
        Operation::Resize { .. } => ui.tinted(Role::Text, 0.7),
        Operation::Label { .. } => ui.tinted(Role::AccentSoft, 1.0),
    }
}

/// The least room the list of pending operations is drawn in: its heading
/// and two of its lines — less, and it is left out.
fn pending_least(page: &Page) -> f32 {
    page.metric(Metric::RowPadding) * 0.6 * 2.0 + page.line(Text::Caption) * 3.0
}

/// GParted's bottom pane: the pending operations in a darker pane under the
/// details, as Installing's Details are, so they read as a record apart from
/// the page — its heading and how many, then one operation to a line in
/// GParted's words, the most recent last; where they do not all fit, the
/// oldest are left out and a line says how many; with none, says so.
fn draw_pending(page: &mut Page, rect: [f32; 4], state: &State) {
    let pad = page.metric(Metric::RowPadding) * 0.6;
    let line = page.line(Text::Caption);
    let gap = page.metric(Metric::Gap);
    let dot = line * 0.3;
    let ops = state.views.partitions.plan.operations();
    let dots: Vec<[f32; 4]> = ops.iter().map(|op| operation_dot(page, op)).collect();
    let ui = page.ui();
    ui.card(rect, Surface::Control, Role::Glass, 0.55);
    let inside = [
        rect[0] + pad * 1.4,
        rect[1] + pad * 0.35,
        (rect[2] - pad * 2.8).max(1.0),
        (rect[3] - pad * 0.7).max(1.0),
    ];
    let (role, alpha) = text::LESSER;
    if ops.is_empty() {
        // An empty list says so in its middle, quietly: a place kept for
        // what will be done, not a pane left over.
        let middle = inside[1] + (inside[3] - line) * 0.5;
        text::label_faded(
            ui,
            [inside[0], middle, inside[2], line],
            Text::Caption,
            i18n::text("partitions-pending-none"),
            role,
            alpha * 0.6,
            Align::Centre,
        );
        return;
    }
    let heading = i18n::text("partitions-pending-title");
    text::label_faded(
        ui,
        [inside[0], inside[1], inside[2], line],
        Text::Caption,
        heading,
        role,
        alpha * 0.75,
        Align::Left,
    );
    let count = ops.len().to_string();
    text::label_faded(
        ui,
        [inside[0], inside[1], inside[2], line],
        Text::Caption,
        &count,
        role,
        alpha * 0.75,
        Align::Right,
    );
    let room = ((inside[3] - line) / line).floor().max(0.0) as usize;
    if room == 0 {
        return;
    }
    let (earlier, from) = if ops.len() > room {
        let from = ops.len() - (room - 1);
        (Some(from), from)
    } else {
        (None, 0)
    };
    let mut y = inside[1] + line;
    if let Some(count) = earlier {
        let said = crate::message!("partitions-pending-earlier", "count" => count as i64);
        text::label_faded(
            ui,
            [inside[0] + dot + gap * 0.5, y, inside[2], line],
            Text::Caption,
            &said,
            role,
            alpha * 0.6,
            Align::Left,
        );
        y += line;
    }
    for (op, colour) in ops.iter().zip(&dots).skip(from) {
        let said = planning::sentence(op, &state.disks);
        ui.chip([inside[0], y + (line - dot) * 0.5, dot, dot], *colour);
        let x = inside[0] + dot + gap * 0.5;
        text::label_faded(
            ui,
            [x, y, inside[2] - dot - gap * 0.5, line],
            Text::Caption,
            &said,
            Role::Text,
            1.0,
            Align::Left,
        );
        y += line;
    }
}

/// Where what Continue still needs is said: on the buttons' line, left of
/// Options, where it fits there on one line; otherwise — a display standing
/// on its side — over the buttons, across the room, on up to two lines.
/// Each the warning mark and the words, in white.
struct Wanting {
    lines: Vec<String>,
    /// The first line's rectangle, the mark at its left.
    at: [f32; 4],
    /// Whether it stands over the buttons, taking its height from the room.
    over: bool,
}

/// How large the warning mark is beside a line of the body's type, and the
/// air after it.
fn warning_mark(page: &Page) -> (f32, f32) {
    (page.line(Text::Body) * 0.8, page.metric(Metric::Gap) * 0.4)
}

fn lay_wanting(
    page: &mut Page,
    words: &str,
    content: [f32; 4],
    band: [f32; 4],
    options: [f32; 4],
) -> Wanting {
    let gap = page.metric(Metric::Gap);
    let line = page.line(Text::Body);
    let (mark, after) = warning_mark(page);
    let beside = options[0] - gap - band[0] - mark - after;
    let ui = page.ui();
    // Measured bold, as a refusal says them, so they stand beside the
    // buttons the same before and after Continue is pressed.
    if text::bold_width(ui, Text::Body, words) <= beside {
        let y = options[1] + (options[3] - line) * 0.5;
        return Wanting {
            lines: vec![words.to_string()],
            at: [band[0], y, beside + mark + after, line],
            over: false,
        };
    }
    let room = content[2] - mark - after;
    let lines = text::wrapped_by(words, room, 2, |candidate| {
        text::bold_width(ui, Text::Body, candidate)
    });
    let tall = line * lines.len() as f32;
    let y = content[1] + content[3] - tall;
    Wanting {
        lines,
        at: [content[0], y, content[2], line],
        over: true,
    }
}

fn draw_wanting(page: &mut Page, wanting: &Wanting, refused: bool) {
    let line = page.line(Text::Body);
    let (mark, after) = warning_mark(page);
    let icons = page.icons();
    let ui = page.ui();
    let [x, y, w, _] = wanting.at;
    let (role, alpha) = text::REFUSAL;
    ui.icon_tinted(
        [x, y + (line - mark) * 0.5, mark, mark],
        glyphs::WARNING,
        icons,
        role,
        alpha,
    );
    let tint = ui.tinted(role, alpha);
    for (index, one) in wanting.lines.iter().enumerate() {
        let at = [
            x + mark + after,
            y + index as f32 * line,
            w - mark - after,
            line,
        ];
        ui.label_weighted(at, Text::Body, one, tint, Align::Left, refused);
    }
}

pub fn draw(state: &mut State, page: &mut Page, room: Room, lit: bool) {
    if state.views.partitions.sheet.is_some() {
        return sheets::draw(state, page, room, lit);
    }
    let Room {
        mut content,
        buttons: band,
        ..
    } = room;
    let gap = page.metric(Metric::Gap);
    let pressing = state.pressing.through();
    let dt = state.clock.dt;
    let buttons = buttons();
    let places = button::places(page, band, &buttons);

    // What Continue still needs, first: over the buttons it takes its
    // height from the room everything else is laid in.
    let wanting =
        missing_words(state).map(|words| lay_wanting(page, &words, content, band, places[OPTIONS]));
    if let Some(wanting) = wanting.as_ref().filter(|wanting| wanting.over) {
        content[3] = (wanting.at[1] - gap - content[1]).max(1.0);
    }

    // The chips, along the top, each saying how many operations the plan
    // has on its disk.
    let counts: Vec<usize> = state
        .disks
        .iter()
        .map(|disk| state.views.partitions.plan.on_disk(&disk.path))
        .collect();
    let (chips, chips_tall) = lay_chips(page, &state.disks, &counts, content);
    if std::mem::take(&mut state.views.partitions.point_at_blocked) {
        if let Some(chip) = chips.iter().find(|chip| chip.blocked) {
            let [x, y, w, h] = chip.rect;
            state.pretend_a_pointer([x + w * 0.5, y + h * 0.5]);
        }
    }
    // The reason a disk cannot be used is said while the pointer rests on
    // its chip: moved off it onto nothing, which tells no control it was
    // moved onto, the details are the light's again.
    if let (Some(at), Some([x, y])) = (state.views.partitions.pointed, page.pointer()) {
        let on = chips
            .iter()
            .find(|chip| chip.index == at)
            .is_some_and(|chip| {
                let [left, top, w, h] = chip.rect;
                (left..=left + w).contains(&x) && (top..=top + h).contains(&y)
            });
        if !on {
            state.views.partitions.pointed = None;
        }
    }
    let (disk_at, stop_at, focus, pointed) = {
        let view = &state.views.partitions;
        (view.disk, view.stop, view.focus, view.pointed)
    };
    for chip in &chips {
        let here = lit && focus == Focus::Chips && chip.index == disk_at;
        if here {
            let sunk = motion::pressed(chip.rect, pressing);
            state.light.lay(page, sunk, Shape::Capsule, pressing, dt);
        }
        draw_chip(
            page,
            chip,
            chip.index == disk_at && !chip.blocked,
            here,
            pressing.filter(|_| here),
        );
        page.ui().spot(spots::CHIPS + chip.index as u32, chip.rect);
    }

    // The details take what they need under the map, and the map what is
    // left of its own height.
    let planned = planned(state);
    let walk = planned
        .as_ref()
        .map_or_else(Vec::new, |planned| map::stops(&map::tiles(&planned.disk)));
    let stop = walk.get(stop_at.min(walk.len().saturating_sub(1))).copied();
    let pointed = pointed
        .and_then(|at| state.disks.get(at))
        .filter(|disk| !usable(disk))
        .cloned();
    let columns = match (&planned, stop) {
        (Some(planned), Some(stop)) => details(state, planned, stop),
        _ => [Vec::new(), Vec::new()],
    };
    let lines = details_lines(page, content[2], &columns);
    // A tile that offers less than it might says why under its details, as
    // GParted does ([`held_back`]).
    let measured = measured_of(state);
    let no_new = match (&planned, stop) {
        (Some(planned), Some(stop)) => {
            held_back(planned, stop, &measured).map(|why| why_lines(page, &why, content[2]))
        }
        _ => None,
    };
    let why_tall = no_new
        .as_ref()
        .map_or(0.0, |said| page.line(Text::Body) * said.len() as f32);
    let details_tall = page.line(Text::Body) * lines.max(2) as f32 + why_tall;
    let mounted = planned
        .as_ref()
        .is_some_and(|planned| planned.parts.iter().any(|marks| marks.mount.is_some()));
    // Where the details leave the map less than its least height (a display
    // standing on its side, a partition with much to say), the air round the
    // map is halved before anything is cut. On a panel narrow enough for the
    // map to slide, so that its tiles keep their words, it keeps the height
    // of a tile's name and size before the air does: the air goes down to
    // a hair for it; and its details keep one height as the light walks it
    // — the room the most any tile's details say takes — rather than
    // growing and shrinking under the light from a partition with much to
    // say to the unallocated space beside it. A reason under the details
    // takes its room only under the tile it is said of: no empty band is kept
    // for another tile's while the light is elsewhere, which the map, its
    // tiles' sizes among it, would give up.
    let narrow = crate::ui::pair::beside_at(content[2] / page.scaled(1.0))
        .1
        .slides();
    let details_room = match (&planned, narrow) {
        (Some(planned), true) => {
            let mut most = lines.max(2);
            for one in &walk {
                let columns = details(state, planned, *one);
                most = most.max(details_lines(page, content[2], &columns).max(2));
            }
            page.line(Text::Body) * most as f32 + why_tall
        }
        _ => details_tall,
    };
    let left_for_map = |air: f32| content[3] - chips_tall - air * 2.0 - details_room;
    let wanted = if narrow {
        map_least(page) + page.line(Text::Caption)
    } else {
        map_least(page)
    };
    let airs: &[f32] = if narrow {
        &[1.4, 0.7, 0.3]
    } else {
        &[1.4, 0.7]
    };
    let air = airs
        .iter()
        .map(|share| gap * share)
        .find(|air| left_for_map(*air) >= wanted)
        .unwrap_or(gap * airs[airs.len() - 1]);
    let top = content[1] + chips_tall + air;
    let spare = left_for_map(air);
    let map_tall = spare.min(map_height(page, mounted)).max(map_least(page));
    let map_rect = [content[0], top, content[2], map_tall];

    let mut tiles = Vec::new();
    let mut map_view = None;
    if let Some(planned) = &planned {
        let plan = state.views.partitions.plan.clone();
        let mapped = lay_map(state, page, planned, map_rect, Some(stop_at), &plan, false);
        if focus == Focus::Map && mapped.moved != 0.0 {
            // The light lying on a tile goes along with the map as it slides,
            // rather than gliding after it.
            state.light.ride([mapped.moved, 0.0, 0.0, 0.0]);
        }
        if focus == Focus::Map && mapped.easing {
            // And it lies on its tile as the tile eases to its new width.
            state.light.settle();
        }
        let from = page.ui().written();
        for (index, one) in mapped.laid.iter().enumerate() {
            let here = lit && focus == Focus::Map && index == stop_at;
            let sunk = motion::pressed(one.rect, pressing.filter(|_| here));
            if here {
                state.light.lay(page, sunk, Shape::Card, pressing, dt);
            }
            let words = tile_words(planned, one.stop);
            let kept = focus != Focus::Map && index == stop_at;
            draw_tile(page, one, &words, (here, kept), pressing.filter(|_| here));
            if let Some(seen) = inside_cut(one.rect, mapped.cut) {
                page.ui().spot(spots::TILES + index as u32, seen);
            }
            tiles.push(one.rect);
        }
        // A map that slides goes behind the room's edges, as Where to
        // install's columns do.
        if let Some(cut) = mapped.cut {
            let to = page.ui().written();
            page.ui()
                .cut_between(from, to, [cut[0], cut[1] - gap, cut[2], cut[3] + gap * 2.0]);
        }
        map_view = mapped.cut;
    }

    let details_top = map_rect[1] + map_rect[3] + air;
    let (columns, details_tall) = match &pointed {
        None => make_way(
            page,
            content[2],
            columns,
            why_tall,
            content[1] + content[3] - details_top,
        ),
        Some(_) => (columns, details_tall),
    };
    let details_rect = [
        content[0],
        details_top,
        content[2],
        (content[1] + content[3] - details_top).max(1.0),
    ];
    match &pointed {
        Some(disk) => draw_blocked(page, details_rect, disk),
        None => {
            draw_details(page, details_rect, &columns);
            if let Some(said) = &no_new {
                let top = details_top + details_tall - why_tall;
                draw_why(page, [content[0], top, content[2], why_tall], said);
            }
        }
    }

    // Under the details, on a landscape panel with room for it, the pending
    // operations, as GParted lists them in its bottom pane.
    let pane_top = details_top + details_tall + gap * 0.8;
    let pane = [
        content[0],
        pane_top,
        content[2],
        content[1] + content[3] - pane_top,
    ];
    let landscape = page.width() > page.height();
    if landscape && pointed.is_none() && pane[3] >= pending_least(page) {
        draw_pending(page, pane, state);
    }

    // A sheet that has just closed, folding back into the tile the light is
    // on.
    let folding_into = tiles
        .get(stop_at)
        .and_then(|rect| inside_cut(*rect, map_view));
    sheets::draw_folding(state, page, folding_into);

    if let Some(wanting) = &wanting {
        draw_wanting(page, wanting, state.views.partitions.refused);
    }

    // Options and Continue, where Where to install has them: Continue does
    // not move by a pixel from that page to this one.
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
    state.views.partitions.drawn = Drawn {
        tiles,
        map_view,
        options: Some(places[OPTIONS]),
        ..Drawn::default()
    };
    anchor(state);
}

/// The disk a chip's number is, if it is one of the machine's disks.
fn chip_of(state: &State, id: u32) -> Option<usize> {
    let index = id.checked_sub(spots::CHIPS)? as usize;
    (index < state.disks.len() && id < spots::CHIPS + 0x100).then_some(index)
}

/// The stop a tile's number is, if it is one of the shown disk's.
fn tile_of(state: &State, id: u32) -> Option<usize> {
    let index = id.checked_sub(spots::TILES)? as usize;
    (index < stops(state).len()).then_some(index)
}

pub fn hover(state: &mut State, id: u32) {
    if state.views.partitions.sheet.is_some() {
        return sheets::hover(state, id);
    }
    let view = &mut state.views.partitions;
    view.pointed = None;
    if let Some(at) = super::button_of(id, buttons().len()) {
        view.focus = Focus::Buttons(at);
        return;
    }
    if let Some(index) = chip_of(state, id) {
        if showable(state, index) {
            show(state, index);
            state.views.partitions.focus = Focus::Chips;
        } else {
            state.views.partitions.pointed = Some(index);
        }
        return;
    }
    if let Some(index) = tile_of(state, id) {
        let view = &mut state.views.partitions;
        view.focus = Focus::Map;
        view.stop = index;
        view.pointed_tile = true;
    }
}

/// A click: on a chip, that disk shown (a chip raises no menu); on a tile, the light
/// there and its menu up — what Accept on them does; on Options, Options; on
/// Continue, Continue. A menu a click raises grows out of what was clicked,
/// where it stood on the frame just drawn.
pub fn pressed(state: &mut State, page: &mut Page) {
    if state.views.partitions.sheet.is_some() {
        return sheets::pressed(state, page);
    }
    // Every spot's press is taken whether or not it is the one, so a stale
    // click cannot linger.
    let mut chip = None;
    for index in 0..state.disks.len() {
        if page.pressed(spots::CHIPS + index as u32) && chip.is_none() {
            chip = Some(index);
        }
    }
    let mut tile = None;
    for index in 0..stops(state).len() {
        if page.pressed(spots::TILES + index as u32) && tile.is_none() {
            tile = Some(index);
        }
    }
    let button = button::pressed(page, &buttons());
    if let Some(index) = chip {
        state.views.partitions.pointed = None;
        if showable(state, index) {
            let moved = show(state, index);
            state.views.partitions.focus = Focus::Chips;
            if moved {
                state.sound(Sound::Move);
            }
        }
        return;
    }
    if let Some(index) = tile {
        let view = &mut state.views.partitions;
        view.pointed = None;
        view.focus = Focus::Map;
        view.stop = index;
        // A tile partly out of view on a map that slides: the click brings
        // it in, as a click on the column peeking in at the edge of Where to
        // install crosses to it; the next opens its menu.
        let partly = match (view.drawn.tiles.get(index), view.drawn.map_view) {
            (Some(rect), Some(cut)) => !wholly_in(*rect, cut),
            _ => false,
        };
        if partly {
            view.pointed_tile = false;
            state.sound(Sound::Move);
            return;
        }
        open_tile(state, None);
        anchor(state);
        return;
    }
    match button {
        Some(OPTIONS) => {
            state.views.partitions.focus = Focus::Buttons(OPTIONS);
            open_options(state, None);
            anchor(state);
        }
        Some(_) => {
            state.views.partitions.focus = Focus::Buttons(CONTINUE);
            onward(state);
        }
        None => {}
    }
}

/// What the page says of the stop the light is on, as it draws it there:
/// the values of its details ([`details`]) and why it offers less
/// ([`held_back`]), where it does — the words the e2e harness reads off the
/// screen under a tile (`tests/e2e/scenarios.py`'s `look_at_tile`), for the
/// replays of its walks (`ui::app::walks`). Test builds only.
#[cfg(test)]
pub(crate) fn said_of_the_lit_stop(state: &State) -> Vec<String> {
    let Some((planned, stop)) = lit_stop(state) else {
        return Vec::new();
    };
    let [left, right] = details(state, &planned, stop);
    let measured = measured_of(state);
    left.into_iter()
        .chain(right)
        .map(|(_, value)| value)
        .chain(held_back(&planned, stop, &measured))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::disks::{GIB, MIB};
    use crate::model::firmware::Firmware;
    use crate::model::live::LiveSession;
    use crate::ui::app::Screen;
    use crate::ui::screens::target;
    use std::cell::RefCell;
    use std::sync::mpsc;

    /// Partitions on the made-up machine with the two disks it shows only
    /// there, opened from Where to install as Advanced opens it.
    fn on_partitions() -> State {
        let mut state = State::demo(true);
        state.disks.extend(demo::partition_disks());
        state.go(Screen::Target);
        target::open_partitions(&mut state);
        state
    }

    fn at_disk(state: &mut State, path: &str) {
        let at = state.disks.iter().position(|d| d.path == path).unwrap();
        assert!(show(state, at) || state.views.partitions.disk == at);
    }

    /// Put the light on the tile of the partition at `path`.
    fn at_part(state: &mut State, path: &str) {
        let planned = planned(state).unwrap();
        let walk = map::stops(&map::tiles(&planned.disk));
        let index = walk
            .iter()
            .position(|stop| match stop {
                Stop::Part(at) => planned.disk.partitions[*at].path == path,
                _ => false,
            })
            .unwrap_or_else(|| panic!("no tile of {path}"));
        state.views.partitions.focus = Focus::Map;
        state.views.partitions.stop = index;
    }

    /// Choose row `row` of the menu the page raised last, as the frame
    /// answers a menu: the menu taken, then its row handed back.
    /// Ten points a letter.
    fn even(text: &str) -> f32 {
        text.chars().count() as f32 * 10.0
    }

    fn choose(state: &mut State, row: usize) -> Menu {
        let menu = state.raised_menu().expect("a menu");
        target::chose(state, menu.id, row);
        menu
    }

    #[test]
    fn it_opens_on_the_disk_the_light_was_on_its_first_tile_lit() {
        let mut state = State::demo(true);
        state.go(Screen::Target);
        state.views.target.pair.left.at = 1;
        target::open_partitions(&mut state);
        assert_eq!(state.views.target.part, target::Part::Partitions);
        assert_eq!(state.views.partitions.disk, 1);
        assert_eq!(state.views.partitions.stop, 0);
        assert_eq!(state.views.partitions.focus, Focus::Map);
        // The head names the disk shown, as GParted would.
        let head = head(&state);
        assert_eq!(head.caption, map::disk_line(&state.disks[1]));
        assert!(head.caption.starts_with("/dev/sda · "), "{}", head.caption);
    }

    #[test]
    fn left_and_right_walk_every_tile_and_stop_at_the_ends() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sdc");
        let count = stops(&state).len();
        assert_eq!(count, 6, "sdc1, the frame, three logical, free space");
        for expected in 1..count {
            act(&mut state, Action::Right, None);
            assert_eq!(state.views.partitions.stop, expected);
        }
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.partitions.stop, count - 1, "past the end");
        for expected in (0..count - 1).rev() {
            act(&mut state, Action::Left, None);
            assert_eq!(state.views.partitions.stop, expected);
        }
        act(&mut state, Action::Left, None);
        assert_eq!(state.views.partitions.stop, 0, "past the start");
    }

    #[test]
    fn up_reaches_the_chips_and_down_the_buttons() {
        let mut state = on_partitions();
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.partitions.focus, Focus::Chips);
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.partitions.focus, Focus::Chips);
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.partitions.focus, Focus::Map);
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.partitions.focus, Focus::Buttons(CONTINUE));
        act(&mut state, Action::Down, None);
        assert_eq!(state.views.partitions.focus, Focus::Buttons(CONTINUE));
        // Left to Options and back; past either end, nothing.
        act(&mut state, Action::Left, None);
        assert_eq!(state.views.partitions.focus, Focus::Buttons(OPTIONS));
        act(&mut state, Action::Left, None);
        assert_eq!(state.views.partitions.focus, Focus::Buttons(OPTIONS));
        act(&mut state, Action::Right, None);
        act(&mut state, Action::Right, None);
        assert_eq!(state.views.partitions.focus, Focus::Buttons(CONTINUE));
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.partitions.focus, Focus::Map);
    }

    #[test]
    fn on_the_chips_left_and_right_change_the_disk_and_skip_the_blocked_one() {
        let mut state = on_partitions();
        // nvme0n1, sda, sdb (the stick), sdc, sdd.
        at_disk(&mut state, "/dev/sda");
        state.views.partitions.stop = 2;
        act(&mut state, Action::Up, None);
        act(&mut state, Action::Right, None);
        let shown_path = |state: &State| shown(state).unwrap().path.clone();
        assert_eq!(shown_path(&state), "/dev/sdc", "the stick was not skipped");
        assert_eq!(state.views.partitions.stop, 0, "the map did not follow");
        act(&mut state, Action::Right, None);
        assert_eq!(shown_path(&state), "/dev/sdd");
        act(&mut state, Action::Right, None);
        assert_eq!(shown_path(&state), "/dev/sdd", "past the last");
        act(&mut state, Action::Left, None);
        act(&mut state, Action::Left, None);
        assert_eq!(shown_path(&state), "/dev/sda");
        act(&mut state, Action::Left, None);
        assert_eq!(shown_path(&state), "/dev/nvme0n1");
        act(&mut state, Action::Left, None);
        assert_eq!(shown_path(&state), "/dev/nvme0n1", "past the first");
        assert_eq!(state.views.partitions.focus, Focus::Chips);
    }

    #[test]
    fn lb_and_rb_change_the_disk_from_anywhere_and_stop_at_the_ends() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        for focus in [Focus::Map, Focus::Buttons(CONTINUE), Focus::Chips] {
            state.views.partitions.focus = focus;
            at_disk(&mut state, "/dev/sda");
            act(&mut state, Action::Next, None);
            assert_eq!(shown(&state).unwrap().path, "/dev/sdc", "{focus:?}");
            assert_eq!(state.views.partitions.focus, focus, "the light moved");
            act(&mut state, Action::Previous, None);
            assert_eq!(shown(&state).unwrap().path, "/dev/sda");
        }
        at_disk(&mut state, "/dev/sdd");
        act(&mut state, Action::Next, None);
        assert_eq!(shown(&state).unwrap().path, "/dev/sdd", "wrapped");
    }

    #[test]
    fn a_partitions_menu_offers_what_can_be_done_to_it_titled_by_its_device() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        at_part(&mut state, "/dev/sda5");
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().expect("the tile's menu");
        assert_eq!(menu.title.as_deref(), Some("/dev/sda5"));
        // Kept, it takes no label (a label is set only as a filesystem is
        // made); a Windows' data volume, read with no hibernation file of
        // its own, it is never shrunk.
        assert_eq!(
            menu.commands,
            ["Mount point · none", "Format as · keep", "Delete"]
        );
        assert_eq!((menu.marked, menu.on), (None, None));
        // Windows' own, read and signed not hibernated, only shrinks.
        at_part(&mut state, "/dev/sda3");
        act(&mut state, Action::Accept, None);
        assert_eq!(
            state.raised_menu().unwrap().commands,
            [
                "Mount point · none",
                "Format as · keep",
                "Resize…",
                "Delete"
            ]
        );
        // Microsoft's reserved space: only Delete.
        at_part(&mut state, "/dev/sda2");
        act(&mut state, Action::Accept, None);
        assert_eq!(state.raised_menu().unwrap().commands, ["Delete"]);
        // Unallocated space: New…, titled as GParted names the space.
        state.views.partitions.stop = 3;
        assert!(matches!(stops(&state)[3], Stop::Free { .. }));
        act(&mut state, Action::Accept, None);
        let space = state.raised_menu().expect("New…");
        assert_eq!(space.title.as_deref(), Some("unallocated"));
        assert_eq!(space.commands, ["New…"]);
        assert_eq!(hints(&state).len(), 3);
        // On a disk with no partition table: the table to make, and the
        // legend offers Choose.
        at_disk(&mut state, "/dev/nvme0n1");
        assert_eq!(hints(&state).len(), 3);
        act(&mut state, Action::Accept, None);
        let tables = state.raised_menu().expect("the tables");
        assert_eq!(tables.commands, ["Create gpt partition table"]);
        assert!(state.note.is_none());
        // A tile with a menu: Choose, Options, Back.
        at_disk(&mut state, "/dev/sda");
        at_part(&mut state, "/dev/sda5");
        assert_eq!(hints(&state).len(), 3);
    }

    #[test]
    fn a_mount_point_chosen_comes_back_to_the_tiles_menu_on_its_row() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        at_part(&mut state, "/dev/sda5");
        act(&mut state, Action::Accept, None);
        let mount = choose(&mut state, 0);
        assert_eq!(mount.id, TILE_MENU);
        let menu = state.raised_menu().expect("Mount point");
        assert_eq!(
            menu.commands,
            ["/", "/home", "/efi", "/boot", "swap", "Other…", "none"]
        );
        assert_eq!(menu.marked, Some(6), "none is in force");
        target::chose(&mut state, menu.id, 0);
        assert_eq!(
            state.views.partitions.plan.mount_of("/dev/sda5"),
            Some(Mount::Root)
        );
        // Back to the tile's menu, the light on the row just changed, which
        // says its new mount point; Undo has come.
        let back = state.raised_menu().expect("the tile's menu again");
        assert_eq!(back.id, TILE_MENU);
        assert_eq!(back.on, Some(0));
        assert_eq!(back.commands[0], "Mount point · /");
        assert_eq!(back.commands.last().unwrap(), "Undo changes to /dev/sda5");
        // Format as: keep marked, ext4 chosen, back on its row — at / only
        // ext4 and btrfs are offered.
        target::chose(&mut state, back.id, 1);
        let format = state.raised_menu().expect("Format as");
        assert_eq!(format.commands, ["keep", "ext4", "btrfs"]);
        assert_eq!(format.marked, Some(0));
        target::chose(&mut state, format.id, 1);
        let back = state.raised_menu().unwrap();
        assert_eq!(back.on, Some(1));
        assert_eq!(back.commands[1], "Format as · ext4");
        assert_eq!(state.views.partitions.plan.len(), 2);
        // The light stayed on sda5.
        let (planned, stop) = lit_stop(&state).unwrap();
        assert!(matches!(stop, Stop::Part(at) if planned.disk.partitions[at].path == "/dev/sda5"));
        // Undo changes to it: both go.
        let undo = back.commands.len() - 1;
        target::chose(&mut state, back.id, undo);
        assert!(state.views.partitions.plan.is_empty());
    }

    #[test]
    fn deleting_leaves_the_light_on_the_unallocated_space_it_became() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        at_part(&mut state, "/dev/sda4");
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().unwrap();
        let delete = menu
            .commands
            .iter()
            .position(|row| row == "Delete")
            .unwrap();
        target::chose(&mut state, menu.id, delete);
        assert!(state.raised_menu().is_none(), "Delete closes the menu");
        let (planned, stop) = lit_stop(&state).unwrap();
        let Stop::Free { start, size } = stop else {
            panic!("{stop:?}")
        };
        assert_eq!(planned.deleted_in(start, size).len(), 1);
        assert_eq!(stops(&state).len(), 5, "sda4 merged with the gap before it");
        // Undo last brings it back, the light on what it was on.
        act(&mut state, Action::Menu, None);
        let options = state.raised_menu().unwrap();
        assert_eq!(options.commands[0], "Undo last operation");
        target::chose(&mut state, options.id, 0);
        assert!(state.views.partitions.plan.is_empty());
        assert_eq!(stops(&state).len(), 6);
    }

    #[test]
    fn a_table_is_made_from_unallocated_space_or_options_never_from_a_chip() {
        let mut state = on_partitions();
        // A chip raises no menu: Accept on it goes down onto its map.
        at_disk(&mut state, "/dev/sda");
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.partitions.focus, Focus::Chips);
        assert_eq!(hints(&state).len(), 3, "Choose on the chip");
        act(&mut state, Action::Accept, None);
        assert!(state.raised_menu().is_none());
        assert_eq!(state.views.partitions.focus, Focus::Map);
        // A disk with no table: its unallocated space offers the table,
        // each a whole row, the menu named by the disk.
        at_disk(&mut state, "/dev/nvme0n1");
        assert!(matches!(
            lit_stop(&state).map(|(_, stop)| stop),
            Some(Stop::Free { .. })
        ));
        assert!(acts(&state), "Choose on the unallocated space");
        let options = option_rows(&state.views.partitions.plan, table_offered(&state));
        assert!(!options.contains(&OptionRow::Table), "none to write over");
        act(&mut state, Action::Accept, None);
        let tables = state.raised_menu().expect("the tables");
        assert_eq!(tables.title.as_deref(), Some("/dev/nvme0n1"));
        // The made-up machine starts with UEFI.
        assert_eq!(tables.commands, ["Create gpt partition table"]);
        assert_eq!(tables.marked, None);
        target::chose(&mut state, tables.id, 0);
        assert_eq!(state.views.partitions.plan.on_disk("/dev/nvme0n1"), 1);
        let (planned, stop) = lit_stop(&state).unwrap();
        let (measures, _) = measures_of(&state);
        assert_eq!(
            planning::tile_rows(&state.views.partitions.plan, &planned, stop, &measures),
            [TileRow::New],
            "the space now takes a partition"
        );
        // A disk with a table: Options writes a new one over it.
        at_disk(&mut state, "/dev/sda");
        act(&mut state, Action::Menu, None);
        let options = state.raised_menu().unwrap();
        let row = options
            .commands
            .iter()
            .position(|row| row == "Create partition table")
            .expect("Options offers a new table");
        target::chose(&mut state, options.id, row);
        let tables = state.raised_menu().expect("the tables");
        assert_eq!(tables.title.as_deref(), Some("/dev/sda"));
        target::chose(&mut state, tables.id, 0);
        assert_eq!(stops(&state).len(), 1, "one run of unallocated space");
        assert_eq!(state.views.partitions.stop, 0);
        assert_eq!(state.views.partitions.plan.on_disk("/dev/sda"), 1);
        assert_eq!(
            planning::tables(crate::model::firmware::Firmware::Bios).len(),
            2
        );
    }

    #[test]
    fn options_offers_what_can_be_done_and_the_swap_file_while_no_swap_partition_is_used() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sdd");
        state.draft.storage.swap_mib = 4096;
        // Nothing planned: a new table over the disk, and the swap file.
        act(&mut state, Action::Menu, None);
        let menu = state.raised_menu().unwrap();
        assert_eq!(menu.title.as_deref(), Some("Options"));
        assert_eq!(
            menu.commands,
            ["Create partition table", "Swap file · 4\u{a0}GiB"]
        );
        // Its sizes are Where to install's, the one in force marked; a size
        // chosen sets that page's spare memory, and Options comes back on
        // its row.
        target::chose(&mut state, menu.id, 1);
        let swap = state.raised_menu().unwrap();
        assert_eq!(swap.commands.len(), target::SWAP_SIZES.len());
        assert_eq!(swap.marked, Some(2));
        target::chose(&mut state, swap.id, 3);
        assert_eq!(state.draft.storage.swap_mib, 8192);
        let back = state.raised_menu().unwrap();
        assert_eq!(back.on, Some(1));
        // A swap partition used: no swap file; the plan's own rows.
        state
            .views
            .partitions
            .plan
            .mount("/dev/sdd", "/dev/sdd3", Some(Mount::Swap));
        assert_eq!(
            option_rows(&state.views.partitions.plan, true),
            [
                OptionRow::UndoLast,
                OptionRow::Clear,
                OptionRow::Pending,
                OptionRow::Table
            ]
        );
        act(&mut state, Action::Menu, None);
        let menu = state.raised_menu().unwrap();
        assert_eq!(menu.commands[2], "Pending operations (1)…");
        // The whole list, in the dialog.
        target::chose(&mut state, menu.id, 2);
        assert_eq!(state.question_up(), Some(Question::Pending));
        assert_eq!(
            pending_list(&state, 30, 10_000.0, even),
            "•\u{a0}Set mount point of /dev/sdd3 to swap"
        );
        // Clear all.
        act(&mut state, Action::Menu, None);
        let menu = state.raised_menu().unwrap();
        target::chose(&mut state, menu.id, 1);
        assert!(state.views.partitions.plan.is_empty());
    }

    #[test]
    fn options_offers_no_table_over_what_fills_a_whole_disk_and_the_details_say_why() {
        // The rule set writes no new table over what is written on the whole
        // of a disk, so Options offers none where such a disk has a table too
        // (step Q1). The Linux SSD with something written over the whole of
        // it beside its gpt (a hybrid ISO image, an old signature never
        // wiped):
        let mut state = on_partitions();
        let at = state
            .disks
            .iter()
            .position(|d| d.path == "/dev/sdd")
            .unwrap();
        state.disks[at].content = Some(Content::Other);
        at_disk(&mut state, "/dev/sdd");
        state.draft.storage.swap_mib = 4096;
        act(&mut state, Action::Menu, None);
        let menu = state.raised_menu().unwrap();
        assert_eq!(menu.title.as_deref(), Some("Options"));
        assert_eq!(menu.commands, ["Swap file · 4\u{a0}GiB"]);
        // Under every tile of it the details say why, where the tile holds
        // nothing back of its own (its swap and LUKS cannot be mounted).
        let whole = "Something fills the whole disk: no table or partition can be made on it";
        let written = planned(&state).unwrap();
        let measured = measured_of(&state);
        for stop in map::stops(&map::tiles(&written.disk)) {
            let why = held_back(&written, stop, &measured).expect("why, under every tile");
            let path = match stop {
                Stop::Part(at) => written.disk.partitions[at].path.as_str(),
                _ => panic!("the disk offers no free space: {stop:?}"),
            };
            if ["/dev/sdd3", "/dev/sdd5"].contains(&path) {
                assert!(why.contains("can't be mounted"), "{path}: {why}");
            } else {
                assert_eq!(why, whole, "{path}");
            }
        }
        // With nothing over it, the table is offered and nothing said.
        state.disks[at].content = None;
        act(&mut state, Action::Menu, None);
        let menu = state.raised_menu().unwrap();
        assert_eq!(menu.commands[0], "Create partition table");
        let clean = planned(&state).unwrap();
        let root = clean.index_of("/dev/sdd2").unwrap();
        let why = held_back(&clean, Stop::Part(root), &measured_of(&state));
        assert_eq!(why, None);
    }

    /// A disk whose logical sector is larger than 4096 bytes is refused
    /// whole by the rule set (step Q1), so Options offers it no new table,
    /// no tile of it raises a menu, and under every tile the details say
    /// nothing can be changed on it.
    #[test]
    fn a_disk_of_sectors_the_rules_refuse_is_offered_nothing_and_the_details_say_so() {
        let mut state = on_partitions();
        let at = state
            .disks
            .iter()
            .position(|d| d.path == "/dev/sdd")
            .unwrap();
        state.disks[at].logical_sector = 8192;
        at_disk(&mut state, "/dev/sdd");
        state.draft.storage.swap_mib = 4096;
        act(&mut state, Action::Menu, None);
        let menu = state.raised_menu().unwrap();
        assert_eq!(menu.title.as_deref(), Some("Options"));
        assert_eq!(menu.commands, ["Swap file · 4\u{a0}GiB"]);
        let nothing = "Nothing can be changed on /dev/sdd";
        let wide = planned(&state).unwrap();
        let measured = measured_of(&state);
        for (index, stop) in map::stops(&map::tiles(&wide.disk)).into_iter().enumerate() {
            let why = held_back(&wide, stop, &measured);
            assert_eq!(why.as_deref(), Some(nothing), "{stop:?}");
            if let Stop::Part(at) = stop {
                at_part(&mut state, &wide.disk.partitions[at].path.clone());
                act(&mut state, Action::Accept, None);
                assert!(state.raised_menu().is_none(), "a menu on tile {index}");
            }
        }
        assert!(state.views.partitions.plan.is_empty());
        // At 4096 bytes a sector, the table is offered and nothing said.
        state.disks[at].logical_sector = 4096;
        act(&mut state, Action::Menu, None);
        let menu = state.raised_menu().unwrap();
        assert_eq!(menu.commands[0], "Create partition table");
        let fits = planned(&state).unwrap();
        let root = fits.index_of("/dev/sdd2").unwrap();
        assert_eq!(
            held_back(&fits, Stop::Part(root), &measured_of(&state)),
            None
        );
    }

    #[test]
    fn a_long_list_leaves_out_its_oldest_and_says_how_many() {
        let mut state = on_partitions();
        let plan = &mut state.views.partitions.plan;
        for at in 1..=5 {
            plan.mount("/dev/sda", &format!("/dev/sda{at}"), Some(Mount::Home));
        }
        for at in 1..=4 {
            plan.format("/dev/sdd", &format!("/dev/sdd{at}"), Some(Fs::Xfs));
        }
        // Nine operations, a line each: all of them where nine lines fit.
        let all = pending_list(&state, 9, 10_000.0, even);
        assert_eq!(all.lines().count(), 9);
        assert!(all.lines().all(|line| line.starts_with("•\u{a0}")));
        // Six lines: the five latest, and a line saying four more came first.
        let six = pending_list(&state, 6, 10_000.0, even);
        let lines: Vec<&str> = six.lines().collect();
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0], "4 earlier operations");
        assert!(lines[5].ends_with("/dev/sdd4 as xfs"), "{}", lines[5]);
        // Each taking two lines: as many as fit, never none; the second
        // stands in from the bullet, as far as the bullet and its space.
        let two = pending_list(&state, 6, 200.0, even);
        assert_eq!(two.lines().count(), 5, "{two}");
        let lines: Vec<&str> = two.lines().collect();
        assert!(lines[1].starts_with("•\u{a0}"), "{two}");
        assert!(
            lines[2].starts_with("\u{a0}\u{a0}") && !lines[2].contains('•'),
            "{two}"
        );
        assert_eq!(
            pending_list(&state, 1, 200.0, even).lines().count(),
            3,
            "the latest, always, and how many came first"
        );
        assert_eq!(
            pending_list(&on_partitions(), 9, 10_000.0, even),
            "No pending operations"
        );
    }

    #[test]
    fn the_plan_is_kept_while_where_to_install_is_up_and_comes_back_with_advanced() {
        let mut state = on_partitions();
        state.views.partitions.plan.delete("/dev/sda", "/dev/sda4");
        act(&mut state, Action::Back, None);
        target::open_partitions(&mut state);
        assert_eq!(state.views.partitions.plan.len(), 1);
    }

    #[test]
    fn continue_says_what_is_missing_then_goes_on_in_the_demo_only() {
        let mut state = on_partitions();
        // The request as the worker would be handed it (it has no Debug and
        // no equality of its own: it holds passwords).
        let request = |state: &State| serde_json::to_string(&state.draft).unwrap();
        let draft = request(&state);
        state.views.partitions.focus = Focus::Buttons(CONTINUE);
        assert_eq!(missing(&state), Some(Refusal::Root));
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Target);
        assert_eq!(state.views.target.part, target::Part::Partitions);
        // Refused with what the foot says beside the buttons, which turns
        // bold, rather than with the same words a second time under it.
        assert!(state.views.partitions.refused);
        assert!(state.note.is_none());
        assert!(!in_use(&state));
        // A plan with nothing missing.
        replan(&mut state, |plan| {
            plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
            plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
            plan.mount("/dev/sda", "/dev/sda1", Some(Mount::Efi));
            plan.format("/dev/sda", "/dev/sda4", Some(Fs::Fat32));
            plan.mount("/dev/sda", "/dev/sda4", Some(Mount::Boot));
        });
        assert_eq!(missing(&state), None);
        assert!(!state.views.partitions.refused, "a change lets go of it");
        // Off the demo and off the live medium: refused always, and the plan
        // never in use — said in Partitions' own sentence where everything
        // it needs is proven (this plan's, an old root formatted on a kept
        // gpt under UEFI, lies within two sets that passed), and with the
        // first capability it needs that is not proven where something is
        // not (pretended proven by nothing, as were no set written).
        let off_the_medium = || {
            let mut live = on_partitions();
            live.machine = Machine::Real { live: None };
            live.pretend_firmware(crate::model::firmware::Firmware::Uefi);
            live.views.partitions.plan = state.views.partitions.plan.clone();
            live.views.partitions.focus = Focus::Buttons(CONTINUE);
            assert_eq!(missing(&live), None);
            act(&mut live, Action::Accept, None);
            let note = live.note.clone().expect("refused in words");
            assert_eq!(note.role, Role::Danger);
            assert_eq!((live.screen, in_use(&live)), (Screen::Target, false));
            assert!(request(&live) == draft, "the request changed");
            note.text
        };
        assert_eq!(off_the_medium(), i18n::text("partitions-not-yet"));
        {
            let _none = rules::PretendProven::these(&[]);
            assert_eq!(
                off_the_medium(),
                planning::capability_words(rules::Capability::ChangeEntries)
            );
        }
        // In the demo it goes on, the plan the choice in use — and the
        // request the worker would read is not touched by it.
        assert_eq!(state.machine, Machine::Demo);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Desktop);
        assert!(in_use(&state));
        assert!(request(&state) == draft, "the request changed");
    }

    /// Where to install's ticks: whether a disk and a place are marked as
    /// the choice in force.
    fn ticked(state: &mut State) -> (bool, bool) {
        target::ticks(state)
    }

    /// The choice in use is the one confirmed last: Continue on Partitions
    /// takes the plan (Where to install ticking nothing and saying so in its
    /// foot), Back changes nothing about it, a place chosen afterwards takes
    /// the place and keeps the plan as a draft, and Continue takes the plan
    /// again.
    #[test]
    fn the_choice_in_use_is_the_one_confirmed_last() {
        let mut state = on_partitions();
        stage_plan(&mut state);
        assert!(!in_use(&state));
        assert_eq!(ticked(&mut state), (true, true), "the place in force");
        state.views.partitions.focus = Focus::Buttons(CONTINUE);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.screen, Screen::Desktop);
        assert!(in_use(&state));
        // Back from Desktop: the page Continue was pressed on, its light on
        // Continue.
        state.back();
        assert_eq!(state.screen, Screen::Target);
        assert_eq!(state.views.target.part, target::Part::Partitions);
        assert_eq!(state.views.partitions.focus, Focus::Buttons(CONTINUE));
        assert!(in_use(&state));
        // Back from Partitions changes nothing about it.
        act(&mut state, Action::Back, None);
        assert_eq!(state.views.target.part, target::Part::Main);
        assert!(in_use(&state));
        assert_eq!(ticked(&mut state), (false, false), "a place was ticked");
        let note = target::note(&state).expect("the foot says so");
        assert_eq!(note.text, "Using your partitions · 6 operations");
        assert_eq!(note.role, Role::Text);
        // Arrived at afresh, Where to install opens on the disk the plan
        // changes.
        state.go(Screen::Target);
        let sda = state.disks.iter().position(|disk| disk.path == "/dev/sda");
        assert_eq!(Some(state.views.target.pair.left.at), sda);
        // A place chosen: the place in use, the plan kept as a draft.
        target::choose_place(&mut state, 0);
        assert!(!in_use(&state));
        assert_eq!(ticked(&mut state), (true, true));
        assert_eq!(state.views.partitions.plan.len(), 6);
        assert!(target::note(&state).is_none());
        // Shown again on Advanced, and taken again by Continue.
        target::open_partitions(&mut state);
        assert_eq!(state.views.partitions.plan.len(), 6);
        state.views.partitions.focus = Focus::Buttons(CONTINUE);
        act(&mut state, Action::Accept, None);
        assert!(in_use(&state));
        assert_eq!(state.screen, Screen::Desktop);
    }

    /// Continue on Where to install while the plan is in use goes on with
    /// the plan as Partitions' does — refused with what is missing where
    /// something is, said in the foot.
    #[test]
    fn continue_on_where_to_install_goes_on_with_the_plan_in_use() {
        let mut state = on_partitions();
        stage_plan(&mut state);
        state.views.partitions.in_use = true;
        target::close_partitions(&mut state);
        target::continue_pressed(&mut state);
        assert_eq!(state.screen, Screen::Desktop);
        state.back();
        target::close_partitions(&mut state);
        // The /boot taken away: the EFI partition at /efi wants one beside
        // it.
        replan(&mut state, |plan| {
            plan.mount("/dev/sda", "/dev/sda4", None);
        });
        assert!(
            in_use(&state),
            "a change to the plan in use keeps it in use"
        );
        target::continue_pressed(&mut state);
        assert_eq!(state.screen, Screen::Target);
        let note = state.note.clone().expect("refused in words");
        let plan = &state.views.partitions.plan;
        assert_eq!(note.text, planning::refusal_words(&Refusal::Boot, plan));
    }

    /// The disks listed again: the plan is kept while every disk it changes
    /// is listed as it was — another disk changing does not matter — and
    /// dropped whole otherwise, no longer in use, Where to install's foot
    /// saying so until a place is chosen.
    #[test]
    fn the_plan_is_dropped_when_a_disk_it_changes_is_listed_changed() {
        let mut state = on_partitions();
        stage_plan(&mut state);
        state.views.partitions.in_use = true;
        let disks = state.disks.clone();
        disks_listed(&mut state, disks.clone());
        assert_eq!(state.views.partitions.plan.len(), 6);
        assert!(in_use(&state));
        let mut other = disks.clone();
        let sdd = other
            .iter_mut()
            .find(|disk| disk.path == "/dev/sdd")
            .unwrap();
        sdd.partitions[1].label = "changed".into();
        disks_listed(&mut state, other);
        assert_eq!(state.views.partitions.plan.len(), 6, "another disk changed");
        let mut changed = disks.clone();
        let sda = changed
            .iter_mut()
            .find(|disk| disk.path == "/dev/sda")
            .unwrap();
        sda.partitions[4].size -= MIB;
        disks_listed(&mut state, changed);
        assert!(state.views.partitions.plan.is_empty());
        assert!(!in_use(&state));
        target::close_partitions(&mut state);
        let note = target::note(&state).expect("the foot says so");
        assert_eq!(note.text, i18n::text("target-partitions-let-go"));
        assert_eq!(ticked(&mut state), (true, true), "the place is back");
        target::choose_place(&mut state, 0);
        assert!(target::note(&state).is_none());
        // A draft plan let go of says nothing: it was not in use.
        stage_plan(&mut state);
        disks_listed(&mut state, Vec::new());
        assert!(state.views.partitions.plan.is_empty());
        assert!(!state.views.partitions.let_go);
    }

    /// Review's rows for the plan: one for each mount point, in GParted's
    /// words, the swap file, the operations — and what the last question
    /// says the plan destroys.
    #[test]
    fn review_says_the_plans_mount_points_its_swap_file_and_operations() {
        let mut state = on_partitions();
        stage_plan(&mut state);
        let rows: Vec<(String, String)> = review_rows(&state)
            .into_iter()
            .map(|row| (row.name, row.note.replace('\u{a0}', " ")))
            .collect();
        let expected: Vec<(String, String)> = [
            ("/", "New Partition #1 · ext4 · 160.0 GiB on /dev/sda"),
            ("/efi", "/dev/sda1 · fat32 · 100 MiB · kept"),
            ("/boot", "/dev/sda4 · fat32 · 750 MiB · formatted"),
            ("Swap file", "4 GiB"),
            ("Partitions", "6 operations will be applied"),
        ]
        .iter()
        .map(|(name, note)| (name.to_string(), note.to_string()))
        .collect();
        assert_eq!(rows, expected);
        let said: Vec<Vec<String>> = destroys(&state)
            .iter()
            .map(|destroy| destroy_rows(destroy, &state.disks))
            .collect();
        assert_eq!(
            said,
            [
                ["Shrink /dev/sda3 from 372.5\u{a0}GiB to 300.0\u{a0}GiB ·\u{a0}Windows"],
                ["Format /dev/sda4 as fat32 ·\u{a0}Windows"],
                ["Delete /dev/sda5 (ntfs, 278.7\u{a0}GiB) ·\u{a0}Windows"],
            ]
        );
        // No swap file, no row for it.
        state.draft.storage.swap_mib = 0;
        let names: Vec<String> = review_rows(&state)
            .into_iter()
            .map(|row| row.name)
            .collect();
        assert_eq!(names, ["/", "/efi", "/boot", "Partitions"]);
        // A swap partition puts the swap file away, and says its own row.
        state.draft.storage.swap_mib = 4096;
        replan(&mut state, |plan| {
            plan.format("/dev/sdd", "/dev/sdd3", Some(Fs::LinuxSwap));
            plan.mount("/dev/sdd", "/dev/sdd3", Some(Mount::Swap));
        });
        let names: Vec<String> = review_rows(&state)
            .into_iter()
            .map(|row| row.name)
            .collect();
        assert_eq!(names, ["/", "/efi", "/boot", "swap", "Partitions"]);
    }

    #[test]
    fn a_change_to_the_plan_lets_go_of_the_refusal_it_answered() {
        let mut state = on_partitions();
        state.views.partitions.focus = Focus::Buttons(CONTINUE);
        act(&mut state, Action::Accept, None);
        assert!(state.views.partitions.refused);
        state.say("refused", Role::Danger);
        replan(&mut state, |plan| plan.delete("/dev/nvme0n1", "/dev/x"));
        assert!(state.note.is_none());
        assert!(!state.views.partitions.refused);
    }

    #[test]
    fn back_returns_to_where_to_install_with_the_light_on_advanced() {
        for focus in [Focus::Map, Focus::Chips, Focus::Buttons(CONTINUE)] {
            let mut state = on_partitions();
            state.views.partitions.focus = Focus::Buttons(CONTINUE);
            act(&mut state, Action::Accept, None);
            state.views.partitions.focus = focus;
            act(&mut state, Action::Back, None);
            assert_eq!(state.screen, Screen::Target);
            assert_eq!(state.views.target.part, target::Part::Main);
            assert_eq!(state.views.target.focus, target::Focus::Advanced);
            assert!(state.note.is_none(), "the refusal stayed behind");
        }
    }

    #[test]
    fn the_pointer_lights_chips_tiles_and_the_buttons_and_rests_on_a_blocked_chip() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        hover(&mut state, spots::TILES + 3);
        assert_eq!(state.views.partitions.focus, Focus::Map);
        assert_eq!(state.views.partitions.stop, 3);
        let sdc = state
            .disks
            .iter()
            .position(|d| d.path == "/dev/sdc")
            .unwrap();
        hover(&mut state, spots::CHIPS + sdc as u32);
        assert_eq!(state.views.partitions.focus, Focus::Chips);
        assert_eq!(state.views.partitions.disk, sdc);
        assert_eq!(state.views.partitions.stop, 0);
        // The stick: the light stays, and the details say why.
        let stick = state
            .disks
            .iter()
            .position(|d| d.blocked.is_some())
            .unwrap();
        hover(&mut state, spots::CHIPS + stick as u32);
        assert_eq!(state.views.partitions.disk, sdc, "a blocked disk was shown");
        assert_eq!(state.views.partitions.pointed, Some(stick));
        hover(&mut state, spots::BUTTONS);
        assert_eq!(state.views.partitions.focus, Focus::Buttons(OPTIONS));
        assert_eq!(state.views.partitions.pointed, None);
        hover(&mut state, spots::BUTTONS + 1);
        assert_eq!(state.views.partitions.focus, Focus::Buttons(CONTINUE));
        // A key moves on from the reason too.
        hover(&mut state, spots::CHIPS + stick as u32);
        act(&mut state, Action::Up, None);
        assert_eq!(state.views.partitions.pointed, None);
        // A number past the tiles of the disk shown is no tile.
        let count = stops(&state).len() as u32;
        state.views.partitions.focus = Focus::Buttons(CONTINUE);
        hover(&mut state, spots::TILES + count);
        assert_eq!(state.views.partitions.focus, Focus::Buttons(CONTINUE));
    }

    #[test]
    fn a_menu_grows_out_of_its_tile_or_options() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        state.views.partitions.drawn = Drawn {
            tiles: (0..6)
                .map(|at| [at as f32 * 100.0, 300.0, 90.0, 120.0])
                .collect(),
            options: Some([900.0, 700.0, 150.0, 54.0]),
            ..Drawn::default()
        };
        at_part(&mut state, "/dev/sda5");
        act(&mut state, Action::Accept, None);
        anchor(&mut state);
        assert_eq!(state.menu_anchor(), Some([500.0, 300.0, 90.0, 120.0]));
        // A tile as wide as the disk: the menu grows out of its start.
        assert_eq!(
            where_its_name_is([20.0, 300.0, 1200.0, 120.0]),
            [20.0, 300.0, 180.0, 120.0]
        );
        // A chip raises nothing: Accept on it goes down onto the map.
        act(&mut state, Action::Up, None);
        act(&mut state, Action::Accept, None);
        assert_eq!(state.raised_menu().map(|menu| menu.id), Some(TILE_MENU));
        assert_eq!(state.views.partitions.focus, Focus::Map);
        act(&mut state, Action::Menu, None);
        anchor(&mut state);
        assert_eq!(state.menu_anchor(), Some([900.0, 700.0, 150.0, 54.0]));
        // The table Options raises grows out of Options too.
        let options = state.raised_menu().unwrap();
        let row = options
            .commands
            .iter()
            .position(|row| row == "Create partition table")
            .unwrap();
        target::chose(&mut state, options.id, row);
        anchor(&mut state);
        assert_eq!(state.menu_anchor(), Some([900.0, 700.0, 150.0, 54.0]));
    }

    #[test]
    fn the_details_say_what_gparted_says_and_what_is_pending() {
        let mut state = on_partitions();
        let sda = state
            .disks
            .iter()
            .position(|d| d.path == "/dev/sda")
            .unwrap();
        let disk = state.disks[sda].clone();
        let planned = planning::apply(
            &Plan::default(),
            &disk,
            crate::model::firmware::Firmware::Uefi,
        );
        let [left, right] = details(&state, &planned, Stop::Part(2));
        let said: Vec<(Detail, &str)> = left
            .iter()
            .chain(&right)
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        assert!(said.contains(&(Detail::Partition, "/dev/sda3")));
        assert!(said.contains(&(Detail::Filesystem, "ntfs")));
        assert!(said.contains(&(Detail::Label, "Windows")));
        assert!(said.contains(&(Detail::Flags, "msftdata")));
        assert!(said.contains(&(Detail::MountPoint, "—")));
        assert!(said.contains(&(Detail::Pending, "—")));
        assert!(said.iter().any(|(name, _)| *name == Detail::Used));
        // Unallocated space: where it starts and ends, and its size, in MiB.
        let gap = disk.free[0];
        let [left, right] = details(
            &state,
            &planned,
            Stop::Free {
                start: gap.start,
                size: gap.size,
            },
        );
        assert_eq!(right[0].1, map::mib(gap.start));
        assert_eq!(right[1].1, map::mib(gap.start + gap.size));
        assert_eq!(left[1].1, map::mib(gap.size));
        // Planned: Data formatted ext4 at /, its old label and use gone.
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        plan.delete("/dev/sda", "/dev/sda4");
        let planned = planning::apply(&plan, &disk, crate::model::firmware::Firmware::Uefi);
        let data = planned
            .disk
            .partitions
            .iter()
            .position(|p| p.path == "/dev/sda5")
            .unwrap();
        let [left, right] = details(&state, &planned, Stop::Part(data));
        assert!(left.contains(&(Detail::Filesystem, "ext4".into())));
        assert!(left.iter().all(|(name, _)| *name != Detail::Used));
        assert!(right.contains(&(Detail::Label, "—".into())));
        assert!(right.contains(&(Detail::MountPoint, "/".into())));
        assert!(right.contains(&(Detail::Pending, "Format as ext4".into())));
        // The space sda4 left says so.
        let free = planned.disk.free[0];
        let [_, right] = details(
            &state,
            &planned,
            Stop::Free {
                start: free.start,
                size: free.size,
            },
        );
        assert!(right.contains(&(Detail::Pending, "Delete /dev/sda4".into())));
        // On a real machine, Used waits for the functional phase.
        state.machine = Machine::Real { live: None };
        let planned = planning::apply(
            &Plan::default(),
            &disk,
            crate::model::firmware::Firmware::Uefi,
        );
        let [left, _] = details(&state, &planned, Stop::Part(2));
        assert!(left.iter().all(|(name, _)| *name != Detail::Used));
    }

    #[test]
    fn a_tile_says_its_mount_point_its_format_and_what_its_space_was() {
        let disk = demo::disks().remove(1);
        let mut plan = Plan::default();
        plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
        plan.delete("/dev/sda", "/dev/sda4");
        let planned = planning::apply(&plan, &disk, crate::model::firmware::Firmware::Uefi);
        // sda4 gone, sda5 is the fourth partition left.
        assert_eq!(planned.disk.partitions[3].path, "/dev/sda5");
        let data = tile_words(&planned, Stop::Part(3));
        assert_eq!(data.mount.as_deref(), Some("/"));
        assert!(data.formatted);
        assert_eq!(data.filesystem, "ext4");
        assert!(data.label.is_empty());
        assert_eq!(data.band, Some(map::colour(Some("ext4"))));
        let free = planned.disk.free[0];
        let space = tile_words(
            &planned,
            Stop::Free {
                start: free.start,
                size: free.size,
            },
        );
        assert_eq!(space.label, "sda4 deleted");
    }

    #[test]
    fn a_dashed_band_is_whole_dashes_from_end_to_end() {
        for long in [20.0f32, 97.0, 300.0, 1234.5] {
            let (count, dash) = dashes(long, 15.0, 8.0);
            let used = count as f32 * dash + (count - 1) as f32 * 8.0;
            assert!((used - long).abs() < 0.01, "{long}: {used}");
            assert!(dash > 9.0 && dash < 25.0, "{long}: {dash}");
        }
    }

    #[test]
    fn two_columns_of_details_only_where_each_holds_its_names_and_values() {
        assert_eq!(details_columns(1000.0, 150.0, 200.0, 14.0), 2);
        assert_eq!(details_columns(500.0, 150.0, 200.0, 14.0), 1);
    }

    #[test]
    fn every_scene_of_the_page_stands_where_it_says() {
        for (scene, disk, part, menu) in [
            ("partitions", "/dev/sda", Some("/dev/sda3"), None),
            ("partitions-nvme", "/dev/nvme0n1", None, None),
            ("partitions-msdos", "/dev/sdc", Some("/dev/sdc5"), None),
            ("partitions-tiny", "/dev/sdd", Some("/dev/sdd1"), None),
            (
                "partitions-unmountable",
                "/dev/sdd",
                Some("/dev/sdd5"),
                None,
            ),
            ("partitions-measured", "/dev/sdd", Some("/dev/sdd4"), None),
            ("partitions-hibernated", "/dev/sdc", Some("/dev/sdc1"), None),
            ("partitions-reading", "/dev/sda", Some("/dev/sda3"), None),
            ("partitions-blocked", "/dev/sda", Some("/dev/sda3"), None),
            (
                "partitions-menu",
                "/dev/sda",
                Some("/dev/sda5"),
                Some(TILE_MENU),
            ),
            (
                "partitions-mount",
                "/dev/sda",
                Some("/dev/sda5"),
                Some(MOUNT_MENU),
            ),
            ("partitions-table", "/dev/nvme0n1", None, Some(TABLE_MENU)),
            ("partitions-planned", "/dev/sda", Some("/dev/sda5"), None),
            (
                "partitions-options",
                "/dev/sda",
                Some("/dev/sda5"),
                Some(OPTIONS_MENU),
            ),
            ("partitions-pending", "/dev/sda", Some("/dev/sda5"), None),
            ("partitions-missing", "/dev/sda", Some("/dev/sda5"), None),
        ] {
            let mut state = State::demo(true);
            state.stage(scene).unwrap();
            assert_eq!(state.screen, Screen::Target, "{scene}");
            assert_eq!(state.views.target.part, target::Part::Partitions);
            let shown_disk = shown(&state).unwrap().clone();
            assert_eq!(shown_disk.path, disk, "{scene}");
            let (planned, stop) = lit_stop(&state).unwrap();
            match (part, stop) {
                (Some(path), Stop::Part(at)) => {
                    assert_eq!(planned.disk.partitions[at].path, path, "{scene}")
                }
                (None, Stop::Free { .. }) => {}
                other => panic!("{scene}: {other:?}"),
            }
            assert_eq!(state.raised_menu().map(|menu| menu.id), menu, "{scene}");
        }
        // Read: Resize from the least the home ext4's tool names; read as
        // hibernated, why it is not shrunk and what to do — and the same
        // under another Windows, read and signed not hibernated, which the
        // hibernated one holds back; still reading, "Reading…".
        let mut measured = State::demo(true);
        measured.stage("partitions-measured").unwrap();
        match &sheet(&measured).kind {
            sheets::Kind::Resize(resize) => assert_eq!(resize.room.least, 110 * GIB),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            said_of(&measured, "/dev/sdd4").0,
            Some(map::size(99_123 * MIB))
        );
        let mut hibernated = State::demo(true);
        hibernated.stage("partitions-hibernated").unwrap();
        let shut_down_fully = "Windows on /dev/sdc1: shut it down with Fast Startup off";
        assert_eq!(
            said_of(&hibernated, "/dev/sdc1"),
            (None, Some(shut_down_fully.to_string()))
        );
        let mut other = State::demo(true);
        other.stage("partitions-hibernated-other").unwrap();
        let planned = planned(&other).unwrap();
        let at = planned.index_of("/dev/sda3").unwrap();
        let Some(Measure::Read { used, .. }) = demo::measured(&planned.disk.partitions[at]) else {
            panic!("the demo reads the Windows disk's Windows");
        };
        assert_eq!(
            said_of(&other, "/dev/sda3"),
            (Some(map::size(used)), Some(shut_down_fully.to_string()))
        );
        let (measures, _) = measures_of(&other);
        let plan = &other.views.partitions.plan;
        let rows = planning::tile_rows(plan, &planned, Stop::Part(at), &measures);
        assert!(!rows.contains(&TileRow::Resize), "{rows:?}");
        let mut reading = State::demo(true);
        reading.stage("partitions-reading").unwrap();
        assert_eq!(
            said_of(&reading, "/dev/sda3").0.as_deref(),
            Some("Reading…")
        );
        let mut tiny = State::demo(true);
        tiny.stage("partitions-tiny").unwrap();
        let disk = shown(&tiny).unwrap();
        assert_eq!(disk.partitions[0].size, crate::model::disks::MIB);
        // The plan in force: the foot names what is still missing.
        let mut planned = State::demo(true);
        planned.stage("partitions-planned").unwrap();
        assert_eq!(planned.views.partitions.plan.len(), 4);
        assert_eq!(missing(&planned), Some(Refusal::Boot));
        let mut pending = State::demo(true);
        pending.stage("partitions-pending").unwrap();
        assert_eq!(pending.question_up(), Some(Question::Pending));
        let mut wanting = State::demo(true);
        wanting.stage("partitions-missing").unwrap();
        assert!(wanting.views.partitions.refused);
        assert_eq!(
            missing(&wanting),
            Some(Refusal::MustFormat {
                part: rules::Which::Part("/dev/sda5".into()),
                at: Mount::Root
            })
        );
    }

    /// The light on the Windows disk's unallocated space, and New partition
    /// raised over it from its menu.
    fn new_sheet() -> State {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        state.views.partitions.stop = 3;
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().expect("New…");
        target::chose(&mut state, menu.id, 0);
        assert!(state.views.partitions.sheet.is_some(), "no sheet");
        state
    }

    fn sheet(state: &State) -> &Sheet {
        state.views.partitions.sheet.as_ref().expect("a sheet")
    }

    fn new_of(state: &State) -> &sheets::New {
        match &sheet(state).kind {
            sheets::Kind::New(new) => new,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_filesystem_that_does_not_suit_the_mount_point_lets_go_of_it() {
        let mut state = new_sheet();
        assert_eq!(new_of(&state).mount, Some(Mount::Root));
        // xfs is no /: the mount point goes, and the File system row says it.
        let filesystems = new_of(&state).filesystems.clone();
        let xfs = filesystems.iter().position(|fs| *fs == Fs::Xfs).unwrap();
        sheets::chose(&mut state, sheets::FILESYSTEM_MENU, xfs);
        assert_eq!(new_of(&state).fs, Fs::Xfs);
        assert_eq!(new_of(&state).mount, None);
        // Choosing / again makes it ext4, so the two agree.
        sheets::chose(&mut state, sheets::MOUNT_MENU, 0);
        assert_eq!(
            (new_of(&state).fs, new_of(&state).mount.clone()),
            (Fs::Ext4, Some(Mount::Root))
        );
        // A label typed for FAT32 is written in capitals.
        let fat = filesystems.iter().position(|fs| *fs == Fs::Fat32).unwrap();
        sheets::chose(&mut state, sheets::FILESYSTEM_MENU, fat);
        assert_eq!(new_of(&state).mount, None, "fat32 is no /");
        sheets::edited(&mut state, FieldId::PartLabel, &Edit::Wrote("boot".into()));
        assert_eq!(new_of(&state).label, "BOOT");
    }

    #[test]
    fn the_efi_partition_kept_is_offered_efi_and_boot_alone() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        at_part(&mut state, "/dev/sda1");
        act(&mut state, Action::Accept, None);
        let menu = choose(&mut state, 0);
        assert_eq!(menu.id, TILE_MENU);
        let mount = state.raised_menu().expect("Mount point");
        assert_eq!(mount.commands, ["/efi", "/boot", "none"]);
        assert_eq!(mount.marked, Some(2));
        target::chose(&mut state, mount.id, 1);
        assert_eq!(
            state.views.partitions.plan.mount_of("/dev/sda1"),
            Some(Mount::Boot)
        );
        // Windows, kept, is offered everything, / among it.
        at_part(&mut state, "/dev/sda3");
        act(&mut state, Action::Accept, None);
        choose(&mut state, 0);
        let mount = state.raised_menu().expect("Mount point");
        assert_eq!(
            mount.commands,
            ["/", "/home", "/efi", "/boot", "swap", "Other…", "none"]
        );
    }

    #[test]
    fn a_disk_too_small_for_linexin_is_shown_and_may_hold_anything_but_root() {
        let mut state = on_partitions();
        let mut small = demo::partition_disks().remove(1);
        small.path = "/dev/sde".into();
        small.size = 16 * GIB;
        small.partitions.truncate(1);
        small.partitions[0].path = "/dev/sde1".into();
        small.partitions[0].size = 15 * GIB;
        small.partitions[0].fs_type = "ext4".into();
        small.partitions[0].content = crate::model::disks::Content::Linux;
        small.blocked = Some(Blocked::TooSmall);
        state.disks.push(small);
        at_disk(&mut state, "/dev/sde");
        assert_eq!(shown(&state).unwrap().path, "/dev/sde", "it is shown");
        // / on it is refused for its size; a /home of it is not.
        replan(&mut state, |plan| {
            plan.format("/dev/sde", "/dev/sde1", Some(Fs::Ext4));
            plan.mount("/dev/sde", "/dev/sde1", Some(Mount::Root));
        });
        assert_eq!(
            missing(&state),
            Some(Refusal::TooSmall {
                at: Mount::Root,
                least: crate::model::disks::MIN_INSTALL
            })
        );
        replan(&mut state, |plan| {
            plan.mount("/dev/sde", "/dev/sde1", Some(Mount::Home));
            plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
            plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
            plan.format("/dev/sda", "/dev/sda4", Some(Fs::Fat32));
            plan.mount("/dev/sda", "/dev/sda4", Some(Mount::Boot));
        });
        assert_eq!(missing(&state), None);
        // The installer's own stick stays out of reach.
        let stick = state.disks.iter().position(|disk| disk.path == "/dev/sdb");
        assert!(!showable(&state, stick.unwrap()));
    }

    #[test]
    fn new_partition_starts_with_all_of_the_space_at_root_and_add_makes_it() {
        let mut state = new_sheet();
        let gap = state.disks[1].free[0];
        let new = new_of(&state);
        assert_eq!(new.gap, (gap.start, gap.size));
        assert_eq!(new.size, gap.size / MIB * MIB, "all of the room");
        assert_eq!((new.fs, new.mount.clone()), (Fs::Ext4, Some(Mount::Root)));
        assert_eq!(sheet(&state).row(), sheets::Row::Size);
        // Nothing is in the plan while the sheet is up, but the map draws
        // it: the new tile where the space was.
        assert!(state.views.partitions.plan.is_empty());
        let preview = sheet(&state).preview(&state.views.partitions.plan);
        assert_eq!(preview.len(), 1);
        // The light walks the rows down to Add.
        for _ in 0..8 {
            act(&mut state, Action::Down, None);
        }
        assert_eq!(sheet(&state).row(), sheets::Row::Apply);
        act(&mut state, Action::Accept, None);
        assert!(state.views.partitions.sheet.is_none());
        let plan = &state.views.partitions.plan;
        assert_eq!(plan.len(), 1);
        assert_eq!(
            planning::sentence(&plan.operations()[0], &state.disks),
            "Create Primary Partition #1 (ext4, 279.4\u{a0}GiB, /) on /dev/sda"
        );
        // The light on what it made, whose own menu has no keep.
        let (planned, stop) = lit_stop(&state).unwrap();
        let Stop::Part(at) = stop else {
            panic!("{stop:?}")
        };
        assert_eq!(planned.parts[at].new, Some(1));
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().expect("its menu");
        assert_eq!(menu.title.as_deref(), Some("New Partition #1"));
        assert_eq!(
            menu.commands,
            [
                "Mount point · /",
                "Format as · ext4",
                "Resize…",
                "Label…",
                "Delete"
            ]
        );
        // At /, made as ext4 or btrfs alone.
        target::chose(&mut state, menu.id, 1);
        let format = state.raised_menu().unwrap();
        assert_eq!(format.commands, ["ext4", "btrfs"]);
        assert_eq!(format.marked, Some(0));
    }

    #[test]
    fn the_size_is_held_stepped_ten_at_a_time_and_put_back() {
        let mut state = new_sheet();
        let gap = state.disks[1].free[0].size;
        let size = |state: &State| sheet(state).size().unwrap();
        // Not held: Left and Right move nothing.
        act(&mut state, Action::Left, None);
        assert_eq!(size(&state), gap);
        act(&mut state, Action::Accept, None);
        assert!(sheet(&state).held.is_some());
        act(&mut state, Action::Left, None);
        assert_eq!(size(&state), 276 * GIB, "a whole step under all of it");
        act(&mut state, Action::Previous, None);
        assert_eq!(size(&state), 236 * GIB, "ten steps");
        act(&mut state, Action::Next, None);
        act(&mut state, Action::Right, None);
        assert_eq!(size(&state), gap, "the last step snaps to all of it");
        // B puts back what it was when taken hold of.
        act(&mut state, Action::Previous, None);
        act(&mut state, Action::Back, None);
        assert!(sheet(&state).held.is_none());
        assert_eq!(size(&state), gap);
        // A keeps it.
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Previous, None);
        act(&mut state, Action::Accept, None);
        assert!(sheet(&state).held.is_none());
        assert_eq!(size(&state), 240 * GIB, "from between two steps");
        // Back with nothing held cancels the sheet, and nothing changed.
        act(&mut state, Action::Back, None);
        assert!(state.views.partitions.sheet.is_none());
        assert!(state.views.partitions.plan.is_empty());
    }

    #[test]
    fn a_number_typed_for_a_held_size_is_kept_in_gib_with_either_mark() {
        let mut state = new_sheet();
        act(&mut state, Action::Accept, None);
        state.edit(FieldId::PartSize, &Edit::Wrote("1x00,5".into()));
        assert_eq!(
            sheet(&state).held.as_ref().unwrap().typed.as_deref(),
            Some("100,5"),
            "only digits and a mark are taken"
        );
        // The map draws it as it is typed.
        assert_eq!(sheet(&state).shown_size(), Some(100 * GIB + 512 * MIB));
        act(&mut state, Action::Accept, None);
        assert!(sheet(&state).held.is_none());
        assert_eq!(sheet(&state).size(), Some(100 * GIB + 512 * MIB));
        // Past the room it is held to the room; a mark alone is refused.
        act(&mut state, Action::Accept, None);
        state.edit(FieldId::PartSize, &Edit::Wrote("9999".into()));
        act(&mut state, Action::Accept, None);
        assert_eq!(sheet(&state).size(), Some(state.disks[1].free[0].size));
        act(&mut state, Action::Accept, None);
        state.edit(FieldId::PartSize, &Edit::Wrote(".".into()));
        act(&mut state, Action::Accept, None);
        assert!(sheet(&state).held.is_some(), "nothing to read: still held");
        state.edit(FieldId::PartSize, &Edit::Rubbed);
        state.edit(FieldId::PartSize, &Edit::Wrote("50".into()));
        typed(&mut state, FieldId::PartSize);
        assert!(sheet(&state).held.is_none(), "the board's Done keeps it");
        assert_eq!(sheet(&state).size(), Some(50 * GIB));
    }

    #[test]
    fn on_a_landscape_panel_the_light_crosses_new_partitions_rows_two_by_two() {
        let mut state = new_sheet();
        state.views.partitions.sheet.as_mut().unwrap().wide = true;
        let row = |state: &State| sheet(state).row();
        act(&mut state, Action::Down, None);
        assert_eq!(row(&state), sheets::Row::Position);
        act(&mut state, Action::Right, None);
        assert_eq!(row(&state), sheets::Row::Filesystem);
        act(&mut state, Action::Down, None);
        assert_eq!(row(&state), sheets::Row::Label);
        act(&mut state, Action::Left, None);
        assert_eq!(row(&state), sheets::Row::Mount);
        // Down from a row lands on the verb, at the right as the buttons
        // stand; Left from it is Cancel.
        act(&mut state, Action::Down, None);
        assert_eq!(row(&state), sheets::Row::Apply);
        act(&mut state, Action::Left, None);
        assert_eq!(row(&state), sheets::Row::Cancel);
        act(&mut state, Action::Right, None);
        act(&mut state, Action::Up, None);
        assert_eq!(row(&state), sheets::Row::Label);
        act(&mut state, Action::Up, None);
        act(&mut state, Action::Up, None);
        assert_eq!(row(&state), sheets::Row::Size);
    }

    #[test]
    fn a_mount_point_chosen_suits_its_filesystem_and_swap_and_grub_follow() {
        let mut state = new_sheet();
        let choose_in = |state: &mut State, row: sheets::Row, at: usize| {
            let rows = sheet(state).rows();
            state.views.partitions.sheet.as_mut().unwrap().at =
                rows.iter().position(|one| *one == row).unwrap();
            act(state, Action::Accept, None);
            let menu = state.raised_menu().expect("the row's menu");
            target::chose(state, menu.id, at);
        };
        // /efi: FAT32.
        choose_in(&mut state, sheets::Row::Mount, 2);
        assert_eq!(
            (new_of(&state).fs, new_of(&state).mount.clone()),
            (Fs::Fat32, Some(Mount::Efi))
        );
        assert_eq!(sheet(&state).row(), sheets::Row::Mount, "back on its row");
        // linux-swap: swap.
        choose_in(&mut state, sheets::Row::Filesystem, 4);
        assert_eq!(new_of(&state).mount, Some(Mount::Swap));
        // ext4 again: swap is left behind.
        choose_in(&mut state, sheets::Row::Filesystem, 0);
        assert_eq!(new_of(&state).mount, None);
        // At the end of the space.
        choose_in(&mut state, sheets::Row::Position, 1);
        assert_eq!(new_of(&state).position, sheets::Position::End);
        // bios_grub is offered on a BIOS machine's gpt disk only, and is
        // 1 MiB with nothing to mount or label: not on the made-up UEFI one.
        assert_eq!(new_of(&state).filesystems.len(), 5);
    }

    #[test]
    fn grub_on_a_bios_machines_gpt_disk_is_one_mib_and_fixed() {
        let mut state = new_sheet();
        if let Some(sheets::Kind::New(new)) = state
            .views
            .partitions
            .sheet
            .as_mut()
            .map(|sheet| &mut sheet.kind)
        {
            new.filesystems.push(Fs::BiosGrub);
        }
        let rows = sheet(&state).rows();
        state.views.partitions.sheet.as_mut().unwrap().at = rows
            .iter()
            .position(|row| *row == sheets::Row::Filesystem)
            .unwrap();
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().unwrap();
        assert_eq!(menu.commands.last().unwrap(), "bios_grub (unformatted)");
        target::chose(&mut state, menu.id, 5);
        let sheet = sheet(&state);
        assert_eq!(sheet.size(), Some(MIB));
        assert!(!sheet.enabled(sheets::Row::Size), "the bar is fixed");
        assert!(!sheet.enabled(sheets::Row::Mount));
        assert!(!sheet.enabled(sheets::Row::Label));
    }

    #[test]
    fn resize_shrinks_windows_from_its_end_and_says_what_it_leaves() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        at_part(&mut state, "/dev/sda3");
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().unwrap();
        let resize = menu
            .commands
            .iter()
            .position(|row| row == "Resize…")
            .unwrap();
        target::chose(&mut state, menu.id, resize);
        let room = match &sheet(&state).kind {
            sheets::Kind::Resize(resize) => resize.room,
            other => panic!("{other:?}"),
        };
        // The demo knows what is used: Windows shrinks, and only shrinks.
        assert!(room.least < room.most);
        assert_eq!(room.most, shown(&state).unwrap().partitions[2].size);
        state
            .views
            .partitions
            .sheet
            .as_mut()
            .unwrap()
            .set_size(250 * GIB);
        let rows = sheet(&state).rows();
        state.views.partitions.sheet.as_mut().unwrap().at = rows
            .iter()
            .position(|row| *row == sheets::Row::Apply)
            .unwrap();
        act(&mut state, Action::Accept, None);
        let plan = &state.views.partitions.plan;
        assert_eq!(
            planning::sentence(&plan.operations()[0], &state.disks),
            "Shrink /dev/sda3 from 372.5\u{a0}GiB to 250.0\u{a0}GiB"
        );
        // Drawn at its new width, the space it freed after it.
        let planned = planned(&state).unwrap();
        assert_eq!(planned.disk.partitions[2].size, 250 * GIB);
        let (_, stop) = lit_stop(&state).unwrap();
        assert_eq!(stop, Stop::Part(2), "the light stayed on it");
    }

    /// The map eases to a changed plan on the cards' spring — where it was
    /// on the first frame, where the plan puts it once settled, and then
    /// still — and draws a plan at once where a size is moving on a sheet,
    /// on low-end hardware (both `at_once`), and on another disk.
    #[test]
    fn the_map_eases_to_a_changed_plan_and_draws_a_moving_size_at_once() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        let disk = shown(&state).unwrap().clone();
        let rect = [0.0, 0.0, 1200.0, 120.0];
        let m = Measures {
            least: 44.0,
            gap: 4.0,
            inset: 8.0,
            head: 22.0,
        };
        let frame = |state: &mut State, plan: &Plan, at_once: bool| -> Vec<[f32; 2]> {
            let planned = planning::apply(plan, &disk, crate::model::firmware::Firmware::Uefi);
            let laid = map::lay(&planned.disk, &map::tiles(&planned.disk), rect, m);
            let view = &mut state.views.partitions;
            ease_map(
                view,
                &planned,
                rect,
                plan,
                laid,
                at_once,
                crate::ui::motion::FRAME,
            )
            .iter()
            .map(|one| [one.rect[0], one.rect[0] + one.rect[2]])
            .collect()
        };
        let windows = disk.partitions[2].clone();
        let mut shrunk = Plan::default();
        shrunk.resize(&disk.path, &windows.path, windows.size, 250 * GIB);
        let before = frame(&mut state, &Plan::default(), false);
        let settled = {
            let planned = planning::apply(&shrunk, &disk, crate::model::firmware::Firmware::Uefi);
            let laid = map::lay(&planned.disk, &map::tiles(&planned.disk), rect, m);
            laid.iter()
                .map(|one| [one.rect[0], one.rect[0] + one.rect[2]])
                .collect::<Vec<_>>()
        };
        let first = frame(&mut state, &shrunk, false);
        assert!(
            (first[2][1] - before[2][1]).abs() < (settled[2][1] - before[2][1]).abs() * 0.2,
            "Windows jumped: {} → {} of {}",
            before[2][1],
            first[2][1],
            settled[2][1]
        );
        let mut frames = 1;
        let mut now = first;
        while now != settled {
            now = frame(&mut state, &shrunk, false);
            frames += 1;
            assert!(frames < 120, "never settled");
        }
        assert!(frames > 10, "eased in {frames} frames");
        assert_eq!(frame(&mut state, &shrunk, false), settled, "still");
        // A size moving, or low-end: at once.
        assert_eq!(frame(&mut state, &Plan::default(), true), before);
        assert_eq!(frame(&mut state, &shrunk, true), settled);
        // Another disk: at once.
        state.views.partitions.disk += 1;
        let other = frame(&mut state, &Plan::default(), false);
        assert_eq!(other, before);
    }

    /// A sheet unfolds along the toolkit's menus' flight, at once in
    /// low-end; closed, it folds away over the page — not in low-end.
    #[test]
    fn a_sheet_unfolds_and_folds_away_and_at_once_in_low_end() {
        let mut state = new_sheet();
        let sheet = state.views.partitions.sheet.as_mut().unwrap();
        let flight = lxb_app::lxb_toolkit::motion::duration::MENU_FLIGHT;
        let mut travelled = Vec::new();
        for _ in 0..((flight * 60.0).ceil() as usize + 1) {
            travelled.push(sheet.unfolding(crate::ui::motion::FRAME));
        }
        assert!(travelled[0] > 0.0 && travelled[0] < 0.2, "{travelled:?}");
        assert!(travelled.windows(2).all(|two| two[1] >= two[0]));
        assert_eq!(travelled.last(), Some(&1.0));
        act(&mut state, Action::Back, None);
        assert!(state.views.partitions.sheet.is_none());
        assert!(state.views.partitions.folding.is_some(), "it folds away");
        state.views.partitions.settle();
        assert!(state.views.partitions.folding.is_none());
        let mut state = new_sheet();
        state.pretend_low_end();
        act(&mut state, Action::Back, None);
        assert!(
            state.views.partitions.folding.is_none(),
            "folded in low-end"
        );
    }

    /// Where a sheet leaves room for only some of its tile's details (a
    /// display standing on its side), it says the lines it is about, in one
    /// column and in the details' own order.
    #[test]
    fn a_sheet_short_of_room_says_the_details_it_is_about() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        let planned = planned(&state).unwrap();
        let windows = planned.index_of("/dev/sda3").unwrap();
        sheets::open_resize(&mut state, windows);
        let columns = details(&state, &planned, Stop::Part(windows));
        let names = |shown: &[Vec<Pair>; 2]| -> Vec<Detail> {
            assert!(shown[1].is_empty(), "one column");
            shown[0].iter().map(|(name, _)| *name).collect()
        };
        let resize = sheet(&state).clone();
        assert_eq!(
            names(&sheets::telling(&resize, &columns, 1)),
            [Detail::Used]
        );
        assert_eq!(
            names(&sheets::telling(&resize, &columns, 2)),
            [Detail::Used, Detail::Pending]
        );
        assert_eq!(names(&sheets::telling(&resize, &columns, 0)), []);
        sheets::open_label(&mut state, planned.index_of("/dev/sda5").unwrap());
        let label = sheet(&state).clone();
        assert_eq!(
            names(&sheets::telling(&label, &columns, 2)),
            [Detail::Filesystem, Detail::Label]
        );
    }

    thread_local! {
        /// The disks each measurement asked about, by path, in the order
        /// they were asked about (`measured_here`).
        static ASKED: RefCell<Vec<Vec<String>>> = const { RefCell::new(Vec::new()) };
        /// The end the measurement out now is answered from.
        static ANSWERS: RefCell<Option<mpsc::Sender<MeasureEvent>>> = const { RefCell::new(None) };
        /// What the disks are listed as when they are listed again
        /// (`listed_here`).
        static LISTED: RefCell<Vec<Disk>> = const { RefCell::new(Vec::new()) };
    }

    /// A measurement of the test's own, in place of polkit and the measure
    /// entry: it writes down the disks it was asked about, and stays out
    /// until the test answers it ([`answer`]).
    fn measured_here(disks: Vec<Disk>) -> Receiver<MeasureEvent> {
        let (tx, rx) = mpsc::channel();
        let paths = disks.into_iter().map(|disk| disk.path).collect();
        ASKED.with(|asked| asked.borrow_mut().push(paths));
        ANSWERS.with(|answers| *answers.borrow_mut() = Some(tx));
        rx
    }

    /// The disks listed again, as the test has set them ([`LISTED`]).
    fn listed_here() -> Result<Vec<Disk>, String> {
        Ok(LISTED.with(|listed| listed.borrow().clone()))
    }

    fn asked() -> Vec<Vec<String>> {
        ASKED.with(|asked| asked.borrow().clone())
    }

    /// Answer the measurement out with `event`, and fold it in as the frame
    /// does.
    fn answer(state: &mut State, event: MeasureEvent) {
        let answers = ANSWERS.with(|answers| answers.borrow_mut().take());
        answers.expect("a measurement out").send(event).unwrap();
        poll(state);
    }

    /// An answer of the measure entry: each of `parts` read as its measure,
    /// named by the partition as the disks hold it now, the image's sizes,
    /// and the disks it names as not as listed.
    fn read_as(state: &State, parts: &[(&str, Measure)], changed: &[&str]) -> MeasureEvent {
        let parts = parts
            .iter()
            .map(|(path, measure)| {
                let part = state
                    .disks
                    .iter()
                    .flat_map(|disk| &disk.partitions)
                    .find(|part| part.path == *path)
                    .unwrap_or_else(|| panic!("no {path}"));
                rules::Measured {
                    path: part.path.clone(),
                    fs: part.fs_type.clone(),
                    uuid: part.uuid.clone(),
                    size: part.size,
                    measure: *measure,
                }
            })
            .collect();
        MeasureEvent::Answer(MeasureAnswer::Measured {
            measures: rules::Measures { parts },
            image: demo::image_sizes(),
            changed: changed.iter().map(|path| path.to_string()).collect(),
        })
    }

    /// Read, `used` of it in use, its tool naming no least of its own.
    fn read_with(used: u64) -> Measure {
        Measure::Read {
            used,
            free: 0,
            least: None,
        }
    }

    /// Partitions as a live system shows it, as far as the interface can
    /// tell — the made-up machine with the two disks it shows only there, a
    /// live session it was never started from, UEFI without reading this
    /// computer, an account typed in — its measurements the test's own
    /// (`measured_here`) and the disks it lists again too (`listed_here`):
    /// no test asks polkit, looks for the live medium or lists this
    /// computer's disks. Where to install is up; Partitions not opened yet.
    fn on_a_live_system() -> State {
        ASKED.with(|asked| asked.borrow_mut().clear());
        ANSWERS.with(|answers| *answers.borrow_mut() = None);
        let mut state = State::demo(true);
        state.disks.extend(demo::partition_disks());
        LISTED.with(|listed| *listed.borrow_mut() = state.disks.clone());
        crate::ui::screens::review::stage(&mut state);
        state.machine = Machine::Real {
            live: Some(LiveSession {
                image: "/run/archiso/airootfs".into(),
                medium_uuid: None,
                medium_label: Some("LINEXIN".into()),
                medium_mounted: true,
            }),
        };
        state.pretend_firmware(Firmware::Uefi);
        state.measure = measured_here;
        state.list_disks = listed_here;
        state.go(Screen::Target);
        state
    }

    /// Every disk listed, by path: what a measurement is asked about — the
    /// installation medium (`/dev/sdb`) too, which Partitions never shows
    /// and the measure entry never reads, but whose Windows volumes it
    /// answers unread (the rule set judges a shrink of an NTFS across every
    /// disk of the measurement).
    fn listed_disks() -> Vec<String> {
        [
            "/dev/nvme0n1",
            "/dev/sda",
            "/dev/sdb",
            "/dev/sdc",
            "/dev/sdd",
        ]
        .map(String::from)
        .to_vec()
    }

    /// On a live system the measure entry is asked once, as Partitions
    /// first opens, about every disk listed; Partitions opened again
    /// asks nothing more. While it reads, Install waits whatever the choice
    /// in use — a place chosen on Where to install too (Unit Q, Q5; were a
    /// place to go on to its question, its worker would be refused the lock
    /// the measure entry holds): "This computer's disks are still being
    /// read", no question put up. Once it has answered, Install goes on to
    /// its question. Off the live medium, and before Partitions opens,
    /// nothing is asked.
    #[test]
    fn a_live_system_measures_once_as_partitions_opens_and_install_waits_while_it_reads() {
        let reading = i18n::text("install-reading").to_string();
        let mut state = on_a_live_system();
        assert_eq!(state.ready(), Ok(()), "the draft's place, nothing read");
        assert!(asked().is_empty(), "measured before Partitions opened");
        target::open_partitions(&mut state);
        assert_eq!(asked(), [listed_disks()]);
        assert!(state.measuring.reading());
        assert!(!in_use(&state), "the draft's place is the choice");
        assert_eq!(state.ready(), Err(reading.clone()), "a place waits too");
        state.go(Screen::Review);
        state.ask_to_install();
        assert_eq!(state.question_up(), None, "a question while it reads");
        // Opened again while it reads, and after: asked once.
        state.go(Screen::Target);
        target::open_partitions(&mut state);
        assert_eq!(asked().len(), 1);
        let event = read_as(&state, &[("/dev/sda3", read_with(200 * GIB))], &[]);
        answer(&mut state, event);
        assert!(!state.measuring.reading());
        assert_eq!(state.ready(), Ok(()));
        act(&mut state, Action::Back, None);
        target::open_partitions(&mut state);
        assert_eq!(asked().len(), 1);
        state.go(Screen::Review);
        state.ask_to_install();
        assert_eq!(state.question_up(), Some(crate::ui::app::Question::Erase));
        // Off the live medium, nothing at all.
        let mut off = on_a_live_system();
        off.machine = Machine::Real { live: None };
        target::open_partitions(&mut off);
        assert!(asked().is_empty());
        assert!(!off.measuring.reading());
    }

    /// **No worker is started beside a measure entry still running, even
    /// once it is no longer waited for** (Unit Q, Q5): on a live
    /// system with a place chosen on Where to install, Partitions opened and
    /// its measurement never answering, Install says "This computer's disks
    /// are still being read. Try again in a moment." and puts up no
    /// question — never "Another installation is already running", which a
    /// worker refused the installer's lock would end on, on Stopped — both
    /// while it reads and once `READING_PATIENCE` has passed and what it
    /// had not read is taken as unread, for as long as its measure entry
    /// runs; the person stays on Review. Its measure entry ended — its
    /// thread done without a word, or with an answer that comes too late to
    /// count — Install goes on to its question. (`begin` asks `ready` again
    /// before any worker is asked for; a test never reaches it on a live
    /// system, which would look for the live medium.)
    #[test]
    fn a_worker_is_never_started_beside_a_measure_entry_still_running() {
        let reading = i18n::text("install-reading").to_string();
        let running = i18n::text("failure-already-running").to_string();
        for ends_with_an_answer in [false, true] {
            let mut state = on_a_live_system();
            state.clock.tick(1000.0);
            target::open_partitions(&mut state);
            let entry = ANSWERS.with(|answers| answers.borrow_mut().take()).unwrap();
            state.go(Screen::Review);
            let install = |state: &mut State| {
                state.note = None;
                state.ask_to_install();
                let said = state.note.as_ref().map(|note| note.text.clone());
                (state.question_up(), said)
            };
            assert_eq!(install(&mut state), (None, Some(reading.clone())));
            // No longer waited for: unread, but its measure entry still runs.
            state.clock.tick(1000.0 + READING_PATIENCE);
            poll(&mut state);
            assert!(!state.measuring.reading());
            assert!(state.measuring.running());
            for _ in 0..3 {
                poll(&mut state);
                let (question, said) = install(&mut state);
                assert_eq!((question, said.clone()), (None, Some(reading.clone())));
                assert_ne!(said, Some(running.clone()));
                assert_eq!(state.screen, Screen::Review);
            }
            // Its measure entry ends.
            if ends_with_an_answer {
                let event = read_as(&state, &[("/dev/sda3", read_with(200 * GIB))], &[]);
                entry.send(event).expect("still kept until it ends");
            } else {
                drop(entry);
            }
            poll(&mut state);
            assert!(!state.measuring.running());
            assert_eq!(measures_of(&state), Default::default(), "a late answer");
            assert_eq!(state.ready(), Ok(()));
            assert_eq!(install(&mut state).0, Some(crate::ui::app::Question::Erase));
        }
    }

    /// A measurement is waited for no longer than `READING_PATIENCE` by the
    /// interface's clock (here the test's own, as a picture's is): until
    /// then Partitions says "Reading…" and a plan in use is judged as not
    /// read yet; from then what it has not answered — all of it — is
    /// unread: the details say Windows can't be read, Resize is not offered,
    /// and Install judges the plan by "couldn't be read", never offering
    /// more. The measurement is only let go of: its answer, when it comes,
    /// goes nowhere, and counts neither now nor for the measurement asked
    /// once the disks are listed anew. With no plan in use Install waits
    /// all the same while the measure entry runs, out or let go of (Unit Q,
    /// Q5), and goes on once it has ended.
    #[test]
    fn a_measurement_not_answered_in_time_is_unread_and_its_late_answer_dropped() {
        let windows = "/dev/sda3";
        let esp_at_boot = |plan: &mut Plan| {
            plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
            plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
            plan.mount("/dev/sda", "/dev/sda1", Some(Mount::Boot));
        };
        let said = |id: &str| {
            let mut args = crate::i18n::FluentArgs::new();
            args.set("partition", "/dev/sda1");
            crate::i18n::format(id, &args)
        };
        let asked_at = 1000.0;
        let mut state = on_a_live_system();
        state.clock.tick(asked_at);
        target::open_partitions(&mut state);
        at_disk(&mut state, "/dev/sda");
        let late = ANSWERS.with(|answers| answers.borrow().clone()).unwrap();
        let reading = i18n::text("install-reading").to_string();
        assert!(state.measuring.reading());
        assert_eq!(state.ready(), Err(reading.clone()), "a place waits too");
        replan(&mut state, esp_at_boot);
        state.views.partitions.in_use = true;
        let not_yet = said("partitions-refused-not-read");
        // Just short of it: still read.
        state.clock.tick(asked_at + READING_PATIENCE - 0.5);
        poll(&mut state);
        assert!(state.measuring.reading());
        assert_eq!(
            said_of(&state, windows),
            (Some(i18n::text("partitions-reading").to_string()), None)
        );
        assert_eq!(state.ready(), Err(not_yet.clone()));
        // At it: unread, and judged so.
        state.clock.tick(asked_at + READING_PATIENCE);
        poll(&mut state);
        assert!(!state.measuring.reading());
        assert_eq!(measures_of(&state), Default::default());
        let unreadable =
            planning::unshrunk_words(windows, &rules::Unshrinkable::Itself(Measure::Unreadable));
        assert_eq!(said_of(&state, windows), (None, Some(unreadable)));
        assert!(!menu_of(&mut state, windows).contains(&"Resize…".to_string()));
        let unread = said("partitions-refused-unread");
        assert_eq!(missing_words(&state), Some(unread.clone()));
        assert_eq!(state.ready(), Err(unread.clone()));
        // A place chosen meanwhile waits while the measure entry still runs.
        state.views.partitions.in_use = false;
        assert_eq!(state.ready(), Err(reading.clone()));
        state.views.partitions.in_use = true;
        // Its answer, coming late, changes nothing but that its measure
        // entry has ended.
        let event = read_as(&state, &[(windows, read_with(200 * GIB))], &[]);
        assert!(late.send(event.clone()).is_ok(), "kept until it ends");
        poll(&mut state);
        assert!(!state.measuring.running());
        assert_eq!(state.ready(), Err(unread));
        state.views.partitions.in_use = false;
        assert_eq!(state.ready(), Ok(()));
        state.views.partitions.in_use = true;
        target::open_partitions(&mut state);
        assert_eq!(asked().len(), 1, "not asked again for the same disks");
        // The disks listed anew: asked again; the old answer never counts
        // for it, the new one does.
        let mut changed = state.disks.clone();
        changed[1].partitions[4].label = "changed".into();
        disks_listed(&mut state, changed);
        assert_eq!(asked().len(), 2);
        assert!(state.measuring.reading());
        assert!(late.send(event).is_err());
        poll(&mut state);
        assert!(state.measuring.reading());
        let esp = Measure::Read {
            used: 31 * MIB,
            free: 69 * MIB,
            least: None,
        };
        let event = read_as(&state, &[("/dev/sda1", esp)], &[]);
        answer(&mut state, event);
        assert!(!state.measuring.reading());
        assert_ne!(
            missing_words(&state),
            Some(said("partitions-refused-unread"))
        );
        // A place chosen again waits while a measurement is out.
        state.views.partitions.in_use = false;
        let reordered = state.disks.iter().rev().cloned().collect();
        disks_listed(&mut state, reordered);
        assert!(state.measuring.reading());
        assert_eq!(state.ready(), Err(reading));
    }

    /// The disks listed again exactly as they were change nothing; listed
    /// otherwise — any disk, whether or not the plan touches it — what was
    /// read of them goes, and they are read again: once the measurement out
    /// has answered where one is out (never two at once), its answer let go
    /// of. An answer naming a disk that is not as listed has the disks
    /// listed again, which keeps it where they still stand as listed and
    /// asks again where they do not. The disk shown stays shown.
    #[test]
    fn the_disks_listed_anew_let_go_of_what_was_read_and_are_read_again() {
        let mut state = on_a_live_system();
        target::open_partitions(&mut state);
        let event = read_as(&state, &[("/dev/sda3", read_with(200 * GIB))], &[]);
        answer(&mut state, event);
        let disks = state.disks.clone();
        let windows = disks[1].partitions[2].clone();
        let known = |state: &State| measures_of(state).0.of(&windows).is_some();
        assert!(known(&state));
        disks_listed(&mut state, disks.clone());
        assert_eq!((asked().len(), known(&state)), (1, true), "as they were");
        // Another disk changed: read again.
        let mut other = disks.clone();
        other[4].partitions[1].label = "changed".into();
        at_disk(&mut state, "/dev/sdd");
        disks_listed(&mut state, other.clone());
        assert_eq!(asked().len(), 2);
        assert!(state.measuring.reading() && !known(&state));
        assert_eq!(state.disks[state.views.partitions.disk].path, "/dev/sdd");
        // Listed anew again while that one is out: asked once it answers,
        // what it answers let go of.
        let mut third = other.clone();
        third[3].partitions[2].label = "changed".into();
        disks_listed(&mut state, third.clone());
        assert_eq!(asked().len(), 2, "two at once");
        let event = read_as(&state, &[("/dev/sda3", read_with(200 * GIB))], &[]);
        answer(&mut state, event);
        assert_eq!(asked().len(), 3);
        assert!(state.measuring.reading() && !known(&state));
        // An answer naming a disk not as listed, listed again as it stands.
        LISTED.with(|listed| *listed.borrow_mut() = third.clone());
        let event = read_as(
            &state,
            &[("/dev/sda3", read_with(200 * GIB))],
            &["/dev/sdc"],
        );
        answer(&mut state, event);
        assert_eq!(asked().len(), 3);
        assert!(!state.measuring.reading() && known(&state));
        // Listed anew once more, the last change undone: read again.
        disks_listed(&mut state, other.clone());
        assert_eq!(asked().len(), 4);
        // Its answer names a disk not as listed — the stick, taken out
        // since — and the disks listed again stand otherwise: read again,
        // the answer let go of.
        let mut fourth = other;
        fourth.remove(2);
        LISTED.with(|listed| *listed.borrow_mut() = fourth);
        let event = read_as(
            &state,
            &[("/dev/sda3", read_with(200 * GIB))],
            &["/dev/sdb"],
        );
        answer(&mut state, event);
        assert_eq!(asked().len(), 5);
        assert!(state.measuring.reading() && !known(&state));
        assert!(!state.disks.iter().any(|disk| disk.path == "/dev/sdb"));
        // The disks as they now stand, asked about — the stick no longer
        // among them; its answer kept, the listing still standing.
        let event = read_as(&state, &[], &["/dev/sdb"]);
        answer(&mut state, event);
        assert_eq!(asked().len(), 5);
        let mut standing = listed_disks();
        standing.retain(|path| path != "/dev/sdb");
        assert_eq!(asked()[4], standing);
        assert!(!state.measuring.reading());
        assert_eq!(state.disks[state.views.partitions.disk].path, "/dev/sdd");
    }

    /// Refused, failed, or ended without a word, a measurement leaves
    /// nothing measured — Install no longer waits — and nothing is asked
    /// again until the disks are listed anew.
    #[test]
    fn a_measurement_refused_or_failed_reads_nothing_until_the_disks_are_listed_anew() {
        let refused = MeasureEvent::Answer(MeasureAnswer::Refused {
            failure: Failure::AlreadyRunning,
            detail: "the lock is held".into(),
        });
        let failed = MeasureEvent::Failed {
            failure: Failure::Internal,
            detail: "pkexec: dismissed".into(),
        };
        for event in [Some(refused), Some(failed), None] {
            let mut state = on_a_live_system();
            target::open_partitions(&mut state);
            match event.clone() {
                Some(event) => answer(&mut state, event),
                None => {
                    ANSWERS.with(|answers| answers.borrow_mut().take());
                    poll(&mut state);
                }
            }
            assert!(!state.measuring.reading(), "{event:?}");
            assert_eq!(measures_of(&state), Default::default(), "{event:?}");
            assert_eq!(state.ready(), Ok(()), "{event:?}");
            target::open_partitions(&mut state);
            assert_eq!(asked().len(), 1, "{event:?}");
            let mut changed = state.disks.clone();
            changed[1].partitions[4].size -= MIB;
            disks_listed(&mut state, changed);
            assert_eq!(asked().len(), 2, "{event:?}");
        }
    }

    /// What the details say of the partition at `path` on the disk shown —
    /// Used, if anything — and what they say under themselves of it.
    fn said_of(state: &State, path: &str) -> (Option<String>, Option<String>) {
        let planned = planned(state).unwrap();
        let at = planned.index_of(path).unwrap();
        let [left, _] = details(state, &planned, Stop::Part(at));
        let used = left
            .into_iter()
            .find(|(name, _)| *name == Detail::Used)
            .map(|(_, value)| value);
        let why = held_back(&planned, Stop::Part(at), &measured_of(state));
        (used, why)
    }

    /// The rows of the menu of the partition at `path` on the disk shown.
    fn menu_of(state: &mut State, path: &str) -> Vec<String> {
        at_part(state, path);
        act(state, Action::Accept, None);
        state
            .raised_menu()
            .map_or_else(Vec::new, |menu| menu.commands)
    }

    /// Partitions on a live system, the Windows disk shown, the measurement
    /// asked as it opened answered with `event` (or still out).
    fn live_measured(event: Option<fn(&State) -> MeasureEvent>) -> State {
        let mut state = on_a_live_system();
        target::open_partitions(&mut state);
        at_disk(&mut state, "/dev/sda");
        if let Some(event) = event {
            let event = event(&state);
            answer(&mut state, event);
        }
        state
    }

    /// A kept partition's menu, its details and what is said under them
    /// follow what reading it found, on a live system: while it is read,
    /// "Reading…" as Used and no Resize; read, what it uses and Resize from
    /// the larger of that and a margin and its tool's own least, to its own
    /// size; read as what cannot shrink — Windows hibernated, a check due,
    /// in use — no Used and no Resize, and why, under the details; a measure
    /// of the same path that is not this partition (another UUID), or none
    /// of it in an answer, or a measurement refused, is unread: no Resize,
    /// "can't be read". Formatted afresh it is resized by its entry alone,
    /// whatever was read.
    #[test]
    fn a_kept_partition_is_offered_and_told_what_its_measurement_allows() {
        let windows = "/dev/sda3";
        let resize = "Resize…".to_string();
        // Being read.
        let mut state = live_measured(None);
        assert_eq!(
            said_of(&state, windows),
            (Some(i18n::text("partitions-reading").to_string()), None)
        );
        assert_eq!(said_of(&state, "/dev/sda1").0.as_deref(), Some("Reading…"));
        assert_eq!(said_of(&state, "/dev/sda2"), (None, None), "never read");
        assert!(!menu_of(&mut state, windows).contains(&resize));
        // Read: Resize from the tool's least, above what is used and a
        // margin, to its own size.
        fn read(state: &State) -> MeasureEvent {
            let least = Measure::Read {
                used: 200 * GIB,
                free: 172 * GIB,
                least: Some(250 * GIB + 1),
            };
            read_as(state, &[("/dev/sda3", least)], &[])
        }
        let mut state = live_measured(Some(read));
        assert_eq!(said_of(&state, windows), (Some(map::size(200 * GIB)), None));
        at_part(&mut state, windows);
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().unwrap();
        let at = menu.commands.iter().position(|row| *row == resize);
        target::chose(&mut state, menu.id, at.expect("Resize…"));
        let room = match &sheet(&state).kind {
            sheets::Kind::Resize(resize) => resize.room,
            other => panic!("{other:?}"),
        };
        assert_eq!(room.least, 250 * GIB + MIB, "the tool's least, on a MiB");
        assert_eq!(room.most, shown(&state).unwrap().partitions[2].size);
        // Read as what cannot shrink: why, under the details.
        fn hibernated(state: &State) -> MeasureEvent {
            read_as(state, &[("/dev/sda3", Measure::Hibernated)], &[])
        }
        fn checked(state: &State) -> MeasureEvent {
            read_as(state, &[("/dev/sda3", Measure::NeedsCheck)], &[])
        }
        fn in_use(state: &State) -> MeasureEvent {
            read_as(state, &[("/dev/sda3", Measure::InUse)], &[])
        }
        for (event, why) in [
            (
                hibernated as fn(&State) -> MeasureEvent,
                Measure::Hibernated,
            ),
            (checked, Measure::NeedsCheck),
            (in_use, Measure::InUse),
        ] {
            let mut state = live_measured(Some(event));
            let words = planning::unshrunk_words(windows, &rules::Unshrinkable::Itself(why));
            assert_eq!(said_of(&state, windows), (None, Some(words)), "{why:?}");
            assert!(!menu_of(&mut state, windows).contains(&resize), "{why:?}");
        }
        // Unread: another partition at its path, none of it, or refused.
        fn another(state: &State) -> MeasureEvent {
            let MeasureEvent::Answer(MeasureAnswer::Measured {
                mut measures,
                image,
                changed,
            }) = read(state)
            else {
                unreachable!()
            };
            measures.parts[0].uuid = "ANOTHER-UUID".into();
            MeasureEvent::Answer(MeasureAnswer::Measured {
                measures,
                image,
                changed,
            })
        }
        fn none(state: &State) -> MeasureEvent {
            read_as(state, &[], &[])
        }
        fn refused(_: &State) -> MeasureEvent {
            MeasureEvent::Answer(MeasureAnswer::Refused {
                failure: Failure::AlreadyRunning,
                detail: "the lock is held".into(),
            })
        }
        let unreadable =
            planning::unshrunk_words(windows, &rules::Unshrinkable::Itself(Measure::Unreadable));
        for event in [another as fn(&State) -> MeasureEvent, none, refused] {
            let mut state = live_measured(Some(event));
            assert_eq!(said_of(&state, windows), (None, Some(unreadable.clone())));
            assert!(!menu_of(&mut state, windows).contains(&resize));
            // Formatted afresh: its entry alone, whatever was read.
            replan(&mut state, |plan| {
                plan.format("/dev/sda", windows, Some(Fs::Ext4));
            });
            assert_eq!(said_of(&state, windows), (None, None));
            assert!(menu_of(&mut state, windows).contains(&resize));
        }
    }

    /// What Continue still needs of a number reading has not found — how
    /// much of a kept EFI partition at `/boot` is free, how much Linexin
    /// puts in a `/var` of its own — "isn't known yet" while it is read,
    /// and "couldn't be read" once reading has ended without it; read, the
    /// rule set judges by it.
    #[test]
    fn a_number_not_read_is_awaited_while_it_is_read_and_said_unread_after() {
        let esp_at_boot = |plan: &mut Plan| {
            plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
            plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
            plan.mount("/dev/sda", "/dev/sda1", Some(Mount::Boot));
        };
        let var_of_its_own = |plan: &mut Plan| {
            plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
            plan.mount("/dev/sda", "/dev/sda5", Some(Mount::Root));
            plan.mount("/dev/sda", "/dev/sda1", Some(Mount::Efi));
            plan.format("/dev/sda", "/dev/sda4", Some(Fs::Fat32));
            plan.mount("/dev/sda", "/dev/sda4", Some(Mount::Boot));
            plan.format("/dev/sdd", "/dev/sdd4", Some(Fs::Ext4));
            plan.mount("/dev/sdd", "/dev/sdd4", Some(Mount::Other("/var".into())));
        };
        let esp = ("partition", "/dev/sda1");
        let var = ("path", "/var");
        let said = |id: &str, (name, value): (&str, &str)| {
            let mut args = crate::i18n::FluentArgs::new();
            args.set(name, value);
            crate::i18n::format(id, &args)
        };
        fn refused(_: &State) -> MeasureEvent {
            MeasureEvent::Failed {
                failure: Failure::Internal,
                detail: "pkexec: dismissed".into(),
            }
        }
        fn read(state: &State) -> MeasureEvent {
            let esp = Measure::Read {
                used: 31 * MIB,
                free: 69 * MIB,
                least: None,
            };
            read_as(state, &[("/dev/sda1", esp)], &[])
        }
        for (plan, unknown, unread, args) in [
            (
                esp_at_boot as fn(&mut Plan),
                "partitions-refused-not-read",
                "partitions-refused-unread",
                esp,
            ),
            (
                var_of_its_own,
                "partitions-refused-image-unknown",
                "partitions-refused-image-unread",
                var,
            ),
        ] {
            let mut state = live_measured(None);
            replan(&mut state, plan);
            assert_eq!(missing_words(&state), Some(said(unknown, args)));
            let mut state = live_measured(Some(refused));
            replan(&mut state, plan);
            assert_eq!(missing_words(&state), Some(said(unread, args)));
        }
        // Read: the EFI partition too full for the kernels; the image's
        // `/var` with room.
        let mut state = live_measured(Some(read));
        replan(&mut state, esp_at_boot);
        let full = Refusal::EspFull {
            part: rules::Which::Part("/dev/sda1".into()),
            least: rules::ESP_BOOT_FREE,
        };
        assert_eq!(missing(&state), Some(full));
        let mut state = live_measured(Some(read));
        replan(&mut state, var_of_its_own);
        assert_eq!(missing(&state), None);
    }

    #[test]
    fn off_the_demo_a_kept_partition_is_not_resized_until_it_is_read() {
        let mut state = on_partitions();
        state.machine = Machine::Real { live: None };
        state.pretend_firmware(crate::model::firmware::Firmware::Uefi);
        at_disk(&mut state, "/dev/sda");
        // Windows, unread: no Resize (a kept partition only shrinks, and
        // only once what it uses is read; growing it is no longer offered).
        for part in ["/dev/sda3", "/dev/sda5"] {
            at_part(&mut state, part);
            act(&mut state, Action::Accept, None);
            let menu = state.raised_menu().unwrap();
            assert!(!menu.commands.iter().any(|row| row == "Resize…"), "{part}");
            assert!(!menu.commands.iter().any(|row| row == "Label…"), "{part}");
        }
        // Formatted, its entry is resized, read or not.
        replan(&mut state, |plan| {
            plan.format("/dev/sda", "/dev/sda3", Some(Fs::Ext4));
        });
        at_part(&mut state, "/dev/sda3");
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().unwrap();
        assert!(menu.commands.iter().any(|row| row == "Resize…"));
        assert!(menu.commands.iter().any(|row| row == "Label…"));
    }

    #[test]
    fn a_label_is_held_to_what_its_filesystem_holds() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        replan(&mut state, |plan| {
            plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        });
        at_part(&mut state, "/dev/sda5");
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().unwrap();
        let label = menu
            .commands
            .iter()
            .position(|row| row == "Label…")
            .unwrap();
        target::chose(&mut state, menu.id, label);
        state.edit(
            FieldId::PartLabel,
            &Edit::Wrote("Photographs-and-films".into()),
        );
        let sheet_now = sheet(&state);
        assert_eq!(
            sheet_now.label_refused,
            Some(crate::ui::sheet::LabelRefused::Limit),
            "said in white"
        );
        match &sheet_now.kind {
            sheets::Kind::Label(labelling) => {
                assert_eq!(labelling.label, "Photographs-and-", "16 bytes");
                assert_eq!(labelling.filesystem, "ext4");
            }
            other => panic!("{other:?}"),
        }
        state.edit(FieldId::PartLabel, &Edit::Rubbed);
        assert_eq!(sheet(&state).label_refused, None);
        // Set: the label operation, after the format.
        typed(&mut state, FieldId::PartLabel);
        assert_eq!(sheet(&state).row(), sheets::Row::Apply);
        act(&mut state, Action::Accept, None);
        assert_eq!(
            state.views.partitions.plan.label_of("/dev/sda5"),
            Some("Photographs-and")
        );
    }

    /// A "/" typed or pasted into a label — Label… and New partition's alike
    /// — is left out, as no label holds one, and the sheet says so for what
    /// it is, in a refusal's white: "A label can't hold “/”", in place of the
    /// label's limit, until the label is changed again.
    #[test]
    fn a_slash_typed_or_pasted_into_a_label_says_why_it_was_refused() {
        use crate::ui::sheet::LabelRefused;
        let slash = ("A label can't hold “/”".to_string(), true);
        let limit = ("ext4 labels hold 16 bytes at most".to_string(), false);
        let label_of = |state: &State| match &sheet(state).kind {
            sheets::Kind::Label(labelling) => labelling.label.clone(),
            sheets::Kind::New(new) => new.label.clone(),
            other => panic!("{other:?}"),
        };
        let said = |state: &State| sheets::said_under(state, sheet(state));
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        replan(&mut state, |plan| {
            plan.format("/dev/sda", "/dev/sda5", Some(Fs::Ext4));
        });
        let planned = planned(&state).unwrap();
        sheets::open_label(&mut state, planned.index_of("/dev/sda5").unwrap());
        assert_eq!(said(&state), vec![limit.clone()]);
        // Typed alone.
        state.edit(FieldId::PartLabel, &Edit::Wrote("/".into()));
        assert_eq!(label_of(&state), "");
        assert_eq!(sheet(&state).label_refused, Some(LabelRefused::Slash));
        assert_eq!(said(&state), vec![slash.clone()]);
        // A letter it takes, and the limit is a note again.
        state.edit(FieldId::PartLabel, &Edit::Wrote("data".into()));
        assert_eq!(label_of(&state), "data");
        assert_eq!(said(&state), vec![limit.clone()]);
        // Pasted inside a word: what comes before it is taken, nothing after.
        state.edit(FieldId::PartLabel, &Edit::Wrote("a/devops".into()));
        assert_eq!(label_of(&state), "dataa");
        assert_eq!(said(&state), vec![slash.clone()]);
        state.edit(FieldId::PartLabel, &Edit::Rubbed);
        assert_eq!(said(&state), vec![limit]);
        // New partition's label says the same.
        let gap = planned.disk.free[0];
        sheets::open_new(&mut state, gap.start, gap.size);
        state.edit(FieldId::PartLabel, &Edit::Wrote("a/devops".into()));
        assert_eq!(label_of(&state), "a");
        assert!(said(&state).contains(&slash), "{:?}", said(&state));
    }

    #[test]
    fn a_mount_point_of_ones_own_is_refused_until_it_is_a_path_that_can_be() {
        let mut state = on_partitions();
        at_disk(&mut state, "/dev/sda");
        at_part(&mut state, "/dev/sda5");
        act(&mut state, Action::Accept, None);
        choose(&mut state, 0);
        let mount = state.raised_menu().unwrap();
        target::chose(&mut state, mount.id, 5);
        assert!(matches!(
            sheet(&state).kind,
            sheets::Kind::Other(sheets::Other { back: None, .. })
        ));
        state.edit(FieldId::MountPath, &Edit::Wrote("/usr/local".into()));
        let rows = sheet(&state).rows();
        state.views.partitions.sheet.as_mut().unwrap().at = rows
            .iter()
            .position(|row| *row == sheets::Row::Apply)
            .unwrap();
        act(&mut state, Action::Accept, None);
        assert!(sheet(&state).refused, "refused, the reason in bold");
        assert!(state.views.partitions.plan.is_empty());
        for _ in 0.."/usr/local".len() {
            state.edit(FieldId::MountPath, &Edit::Rubbed);
        }
        state.edit(FieldId::MountPath, &Edit::Wrote("/data//photos/".into()));
        assert!(!sheet(&state).refused, "typing lets go of it");
        act(&mut state, Action::Accept, None);
        assert!(state.views.partitions.sheet.is_none());
        assert_eq!(
            state.views.partitions.plan.mount_of("/dev/sda5"),
            Some(Mount::Other("/data/photos".into()))
        );
        // Its menu marks it by its path.
        act(&mut state, Action::Accept, None);
        choose(&mut state, 0);
        let menu = state.raised_menu().unwrap();
        assert_eq!(menu.commands[5], "/data/photos");
        assert_eq!(menu.marked, Some(5));
    }

    #[test]
    fn other_from_new_partition_comes_back_to_it() {
        let mut state = new_sheet();
        let rows = sheet(&state).rows();
        state.views.partitions.sheet.as_mut().unwrap().at = rows
            .iter()
            .position(|row| *row == sheets::Row::Mount)
            .unwrap();
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().unwrap();
        target::chose(&mut state, menu.id, 5);
        assert!(matches!(
            sheet(&state).kind,
            sheets::Kind::Other(sheets::Other { back: Some(_), .. })
        ));
        // Cancelled: New partition as it was.
        act(&mut state, Action::Back, None);
        assert_eq!(new_of(&state).mount, Some(Mount::Root));
        assert_eq!(sheet(&state).row(), sheets::Row::Mount);
        // Set: New partition with it, and still nothing in the plan.
        act(&mut state, Action::Accept, None);
        let menu = state.raised_menu().unwrap();
        target::chose(&mut state, menu.id, 5);
        state.edit(FieldId::MountPath, &Edit::Wrote("/srv".into()));
        let rows = sheet(&state).rows();
        state.views.partitions.sheet.as_mut().unwrap().at = rows
            .iter()
            .position(|row| *row == sheets::Row::Apply)
            .unwrap();
        act(&mut state, Action::Accept, None);
        assert_eq!(new_of(&state).mount, Some(Mount::Other("/srv".into())));
        assert!(state.views.partitions.plan.is_empty());
    }

    #[test]
    fn a_sheets_legend_says_what_each_press_does_there() {
        let mut state = new_sheet();
        let said = |state: &State| -> Vec<&'static str> {
            hints(state).iter().map(|hint| hint.label).collect()
        };
        assert_eq!(said(&state), ["Adjust", "Cancel"]);
        act(&mut state, Action::Accept, None);
        assert_eq!(said(&state), ["Keep", "Put back"]);
        state.pad = true;
        assert_eq!(said(&state), ["Keep", "Type", "Put back"]);
        state.pad = false;
        act(&mut state, Action::Accept, None);
        act(&mut state, Action::Down, None);
        assert_eq!(said(&state), ["Choose", "Cancel"]);
        // The pointer, while the size is held, moves no light.
        act(&mut state, Action::Up, None);
        act(&mut state, Action::Accept, None);
        hover(&mut state, spots::SHEET + 2);
        assert_eq!(sheet(&state).row(), sheets::Row::Size);
        act(&mut state, Action::Accept, None);
        hover(&mut state, spots::SHEET + 2);
        assert_eq!(sheet(&state).row(), sheets::Row::Filesystem);
        hover(&mut state, spots::BUTTONS + 1);
        assert_eq!(sheet(&state).row(), sheets::Row::Apply);
    }

    #[test]
    fn the_pointer_puts_the_size_where_it_is_along_the_maps_room() {
        let mut state = new_sheet();
        let gap = state.disks[1].free[0];
        state.views.partitions.drawn.room = Some((
            [100.0, 300.0, 400.0, 150.0],
            gap.start,
            gap.start + gap.size,
        ));
        state.views.partitions.drawn.track = Some([600.0, 500.0, 500.0, 7.0]);
        let size = |x, spot| sheets_size_at(&state, x, spot);
        assert_eq!(
            size(300.0, spots::SHEET_ROOM),
            Some(gap.size / 2 / MIB * MIB)
        );
        assert_eq!(size(90.0, spots::SHEET_ROOM), Some(Mount::Root.least()));
        assert_eq!(size(600.0, spots::SHEET_ROOM), Some(gap.size / MIB * MIB));
        assert_eq!(
            size(850.0, spots::SHEET_TRACK),
            Some(gap.size / 2 / MIB * MIB)
        );
    }

    fn sheets_size_at(state: &State, x: f32, spot: u32) -> Option<u64> {
        sheets::size_at(state, x, spot).map(|bytes| bytes / MIB * MIB)
    }

    #[test]
    fn every_sheet_scene_stands_where_it_says() {
        for (scene, kind, held) in [
            ("partitions-new", "new", false),
            ("partitions-new-held", "new", true),
            ("partitions-new-typed", "new", true),
            ("partitions-resize", "resize", false),
            ("partitions-label", "label", false),
            ("partitions-other", "other", false),
        ] {
            let mut state = State::demo(true);
            state.stage(scene).unwrap();
            let sheet = state.views.partitions.sheet.as_ref().expect(scene);
            let is = match sheet.kind {
                sheets::Kind::New(_) => "new",
                sheets::Kind::Resize(_) => "resize",
                sheets::Kind::Label(_) => "label",
                sheets::Kind::Other(_) => "other",
            };
            assert_eq!(is, kind, "{scene}");
            assert_eq!(sheet.held.is_some(), held, "{scene}");
        }
        let mut held = State::demo(true);
        held.stage("partitions-new-held").unwrap();
        assert_eq!(
            held.views.partitions.sheet.as_ref().unwrap().size(),
            Some(160 * GIB)
        );
        let mut typed = State::demo(true);
        typed.stage("partitions-new-typed").unwrap();
        let sheet = typed.views.partitions.sheet.as_ref().unwrap();
        assert_eq!(sheet.shown_size(), Some(200 * GIB + 512 * MIB));
        let mut planned_scene = State::demo(true);
        planned_scene.stage("partitions-new-planned").unwrap();
        assert!(planned_scene.views.partitions.sheet.is_none());
        assert_eq!(planned_scene.views.partitions.plan.len(), 2);
        let (planned, stop) = lit_stop(&planned_scene).unwrap();
        assert!(matches!(stop, Stop::Part(at) if planned.parts[at].new == Some(1)));
        // With a pad, the label is typed on the installer's board.
        let mut pad = State::demo(true);
        pad.pretend_a_pad();
        pad.stage("partitions-label").unwrap();
        assert_eq!(pad.typing_into(), Some(FieldId::PartLabel));
    }
}
