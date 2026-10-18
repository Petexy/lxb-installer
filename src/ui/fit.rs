//! Every catalog's words, measured against the room they are drawn in.
//!
//! **No text runs out of its button, row, card, menu or field, in any of the
//! ten languages.** The drawing keeps that promise on its own — a one-line
//! label is cut with an ellipsis where it does not fit, a note wraps and its
//! row grows, a menu grows to its widest row — but an ellipsis in the
//! installer's own sentence is a sentence nobody can finish, so this holds
//! the catalogs to the room they get on the narrowest panel the installer is
//! laid out for — a 16:9 display standing on its side (1080×1920, 720×1280,
//! 1440×2560), narrower than a handheld's 10:16 (800×1280) — and across the
//! full width of every landscape display. A translation that would be cut
//! there fails here, naming the language, the place and the words. Where a
//! shorter form stands in for the whole on the narrowest panel (Done's
//! Restart, the erase question's title), the whole is held to a handheld's.
//!
//! **No GPU.** The words are shaped by the very shaper the toolkit's renderer
//! measures with (cosmic-text through glyphon), in the toolkit's own faces,
//! at the reference height's sizes; everything the installer lays out scales
//! with the height, so the reference sizes stand for every height. The rooms
//! are the drawing code's own arithmetic at scale one (`*_at`), with the
//! panel's own width for the window (`panel::pane_rect`). What a person, a
//! disk or a file named — a disk's model, a partition's label, a city — is
//! not the catalogs' and is fitted where it is drawn.

use glyphon::cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};
use lxb_app::lxb_toolkit::{
    metrics::{scale_for, Metric},
    typography::Text,
};

use super::column::{self, Row};
use super::field::{self, FieldId};
use super::screens::{account, keyboard, language, partitions, review, target, timezone, welcome};
use super::{
    button, cards, glyphs, legend, log, menus, pair, panel, progress, text, toggle, trail,
};
use crate::i18n::{self, FluentArgs};

/// Every catalog the installer carries, by its tag.
const TAGS: [&str; 10] = [
    "en-GB", "en-US", "de", "es", "fr", "hi", "pl", "pt-BR", "ru", "zh-CN",
];

/// The toolkit's renderer's measurer, with no window and no GPU.
struct Shaper {
    fonts: FontSystem,
}

impl Shaper {
    fn new() -> Self {
        Self {
            fonts: lxb_app::lxb_render::faces::shipped_faces(),
        }
    }

    /// How wide `string` is in `text` at the reference height, in the bold
    /// face or the regular one — shaped as `Ui::measure` shapes it.
    fn width(&mut self, text: Text, bold: bool, string: &str) -> f32 {
        let size = text.size();
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size * Text::LINE));
        buffer.set_size(None, None);
        let weight = if bold { Weight::BOLD } else { Weight::NORMAL };
        let attrs = Attrs::new().family(Family::Name("Roboto")).weight(weight);
        buffer.set_text(string, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);
        buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0f32, f32::max)
    }

    /// How `string` breaks into lines in `room`, as the drawing breaks it
    /// (`text::break_lines`: at a space, and between the characters of a
    /// script written without spaces), and how wide its widest line is.
    fn lines(&mut self, text: Text, string: &str, room: f32) -> Broken {
        let bold = matches!(text, Text::Display | Text::Title);
        let lines =
            super::text::break_lines(string, room, |candidate| self.width(text, bold, candidate));
        let widest = lines
            .iter()
            .map(|line| self.width(text, bold, line))
            .fold(0.0f32, f32::max);
        Broken {
            lines: lines.len(),
            widest,
            room,
        }
    }
}

/// A sentence broken into lines: how many, and how wide the widest is in
/// the room it was broken to. A line wider than its room is a word, or a
/// run of letters with nowhere to break, that is cut where it is drawn.
#[derive(Debug, Clone, Copy)]
struct Broken {
    lines: usize,
    widest: f32,
    room: f32,
}

/// How wide the inside of the panel is, in reference pixels, on a window
/// `width` by `height`.
fn inside(width: f32, height: f32) -> f32 {
    let scale = scale_for(height);
    panel::pane_rect(width, height, scale)[2] / scale - Metric::PanelPadding.value() * 2.0
}

/// A handheld standing on its side (800×1280): where the whole of what has
/// a shorter form for the narrowest panel still has to fit.
fn narrow() -> f32 {
    inside(800.0, 1280.0)
}

/// The full width of a landscape panel (1920×1080, 1280×800).
fn wide() -> f32 {
    inside(1920.0, 1080.0)
}

/// A 16:9 display standing on its side (1080×1920, 720×1280, 1440×2560): the
/// narrowest panel of all, narrower than a handheld's 10:16, and the width
/// nearly every check is made at.
fn narrowest() -> f32 {
    inside(1080.0, 1920.0)
}

/// How wide the right-hand column of a pair is on a panel `room` wide, as
/// `pair::beside` lays it out.
fn right_column(room: f32) -> f32 {
    pair::beside_at(room).1.page_w
}

/// A catalog's words with nothing in them.
fn text(tag: &str, id: &'static str) -> String {
    i18n::text_in(tag, id).to_string()
}

/// A value handed to a sentence: a count, which picks the noun's form, or
/// words.
#[derive(Debug, Clone)]
enum Value {
    Count(i64),
    Words(String),
}

/// A catalog's sentence with values in it.
fn sentence(tag: &str, id: &str, values: &[(&'static str, Value)]) -> String {
    let mut args = FluentArgs::new();
    for (name, value) in values {
        match value {
            Value::Count(count) => args.set(*name, *count),
            Value::Words(words) => args.set(*name, words.clone()),
        }
    }
    i18n::format_in(tag, id, &args)
}

/// A size in whole gigabytes, as a catalog writes it.
fn gb(tag: &str, amount: &str) -> String {
    sentence(tag, "size-gb", &[("amount", Value::Words(amount.into()))])
}

/// What did not fit, gathered over every catalog so one run names them all.
#[derive(Default)]
struct Misfits(Vec<String>);

impl Misfits {
    fn check(&mut self, tag: &str, place: &str, wide: f32, room: f32, words: &str) {
        if wide > room + 0.5 {
            let said = format!("{tag:6} {place}: {wide:.0} in {room:.0} — {words}");
            self.0.push(said);
        }
    }

    fn lines(&mut self, tag: &str, place: &str, broken: Broken, most: usize, words: &str) {
        let Broken {
            lines,
            widest,
            room,
        } = broken;
        if lines > most {
            let said = format!("{tag:6} {place}: {lines} lines, {most} at most — {words}");
            self.0.push(said);
        }
        if widest > room + 0.5 {
            let said = format!("{tag:6} {place}: a line {widest:.0} in {room:.0} — {words}");
            self.0.push(said);
        }
    }

    fn assert_none(self, what: &str) {
        assert!(
            self.0.is_empty(),
            "{what} that would be cut:\n{}",
            self.0.join("\n")
        );
    }
}

#[test]
fn the_toolkits_menus_hold_their_rows_and_titles() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    // As wide as the toolkit lets a menu grow — the window less its inset —
    // on the narrowest display, a 16:9 one standing on its side, and on a
    // handheld standing on its side.
    let room = menus::MenuRoom::of(1080.0 / scale_for(1920.0), 1.0);
    let handheld = menus::MenuRoom::of(800.0 / scale_for(1280.0), 1.0);
    for tag in TAGS {
        // Every Options row says its option's setting, in every language
        // (`app::Menu`), so every row is held with every setting it can say
        // — a part's label written as the ellipsis it is cut to at the most:
        // whole on a handheld, and with restore points in their shorter
        // words on the narrowest display.
        let value =
            |id: &'static str, said: String| sentence(tag, id, &[("value", Value::Words(said))]);
        let mut every = vec![
            value(
                "target-option-storage",
                text(tag, "target-storage-standard"),
            ),
            value("target-option-swap", text(tag, "target-swap-off")),
            value("target-option-home", text(tag, "target-home-system")),
        ];
        for amount in ["2", "4", "8", "16"] {
            every.push(value("target-option-swap", gb(tag, amount)));
        }
        for id in ["target-home-kept", "target-home-erased"] {
            let part = sentence(tag, id, &[("part", Value::Words("…".into()))]);
            every.push(value("target-option-home", part));
        }
        let snapshots = |id| value("target-option-storage", text(tag, id));
        let whole = snapshots("target-storage-snapshots");
        let wide = shaper.width(Text::Body, true, &whole);
        misfits.check(tag, "Options row, handheld", wide, handheld.label, &whole);
        every.push(snapshots("target-storage-snapshots-short"));
        for words in every {
            let wide = shaper.width(Text::Body, true, &words);
            misfits.check(tag, "Options row", wide, room.label, &words);
        }
        // The menus' titles: Options, and each option's own menu.
        for id in [
            "target-options",
            "target-storage",
            "target-swap",
            "target-home",
        ] {
            let words = text(tag, id);
            let wide = shaper.width(Text::Title, true, &words);
            misfits.check(tag, "menu title", wide, room.title, &words);
        }
        // The alternatives, any of which can be the one marked.
        let mut alternatives = vec![
            text(tag, "target-storage-standard"),
            text(tag, "target-storage-snapshots"),
            text(tag, "target-swap-off"),
            text(tag, "target-home-system"),
            text(tag, "target-home-keep"),
            text(tag, "target-home-erase"),
        ];
        for amount in ["2", "4", "8", "16"] {
            alternatives.push(gb(tag, amount));
        }
        for words in alternatives {
            let wide = shaper.width(Text::Body, true, &words);
            misfits.check(tag, "menu row, marked", wide, room.marked, &words);
        }
    }
    misfits.assert_none("Menu words");
}

#[test]
fn the_last_question_holds_its_title_and_answers() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let pad = Metric::RowPadding.value() * 2.0;
    let gap = Metric::Gap.value();
    let scale = scale_for(1920.0);
    let narrow_room = menus::dialog_room(1080.0, scale) / scale;
    let handheld = scale_for(1280.0);
    let handheld_room = menus::dialog_room(800.0, handheld) / handheld;
    let wide_room = menus::dialog_room(1920.0, 1.0);
    for tag in TAGS {
        // The erase question's whole title on a handheld standing on its
        // side, and its short one where even that has no room for it.
        let words = text(tag, "erase-title");
        let wide = shaper.width(Text::Title, true, &words);
        misfits.check(tag, "dialog title", wide, handheld_room, &words);
        for id in ["quit-title", "erase-title-short", "install-title"] {
            let words = text(tag, id);
            let wide = shaper.width(Text::Title, true, &words);
            misfits.check(tag, "dialog title", wide, narrow_room, &words);
        }
        let mut answers = |ids: [&'static str; 2]| {
            let words: Vec<String> = ids.iter().map(|id| text(tag, id)).collect();
            let wide: f32 = words
                .iter()
                .map(|words| shaper.width(Text::Label, false, words) + pad)
                .sum::<f32>()
                + gap;
            (wide, words.join(" / "))
        };
        // On a display standing on its side the erase answer is its short
        // form; on a landscape one the whole.
        let mut said = Vec::new();
        for ids in [
            ["quit-stay", "quit-leave"],
            ["erase-back", "erase-confirm-short"],
            ["erase-back", "button-install"],
            ["erase-back", "button-next"],
        ] {
            said.push((answers(ids), narrow_room));
        }
        said.push((answers(["erase-back", "erase-confirm"]), wide_room));
        for ((wide, words), room) in said {
            misfits.check(tag, "dialog answers", wide, room, &words);
        }
    }
    misfits.assert_none("The last question's words");
}

#[test]
fn every_row_of_buttons_fits_its_band() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let gap = Metric::Gap.value();
    for tag in TAGS {
        // Done's row says the short Restart where the whole does not fit
        // (`button::fitted`): the whole is held to a handheld standing on its
        // side, the short one to every panel.
        let whole = ["button-restart", "button-not-now"];
        let mut wide = gap;
        for id in whole {
            let word = shaper.width(Text::Label, true, &text(tag, id));
            wide += button::tightest_at(1.0, word);
        }
        let words: Vec<String> = whole.iter().map(|id| text(tag, id)).collect();
        misfits.check(tag, "buttons", wide, narrow(), &words.join(" / "));
        for ids in [
            &["button-options", "button-continue"][..],
            &["button-restart-short", "button-not-now"],
            &["button-install"],
            &["button-done"],
            &["button-close"],
            // Partitions' sheets: Cancel and the sheet's verb, where Options
            // and Continue stand.
            &["partitions-cancel", "partitions-add"],
            &["partitions-cancel", "partitions-resize-button"],
            &["partitions-cancel", "partitions-set"],
        ] {
            // As narrow as a row short of room draws them
            // (`button::tightest_at`).
            let mut wide = gap * (ids.len() - 1) as f32;
            for id in ids {
                let word = shaper.width(Text::Label, true, &text(tag, id));
                wide += button::tightest_at(1.0, word);
            }
            let words: Vec<String> = ids.iter().map(|id| text(tag, id)).collect();
            misfits.check(tag, "buttons", wide, narrowest(), &words.join(" / "));
        }
    }
    misfits.assert_none("Buttons");
}

#[test]
fn every_legend_fits_the_foot() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    for tag in TAGS {
        for ids in [
            &["hint-choose", "hint-options", "hint-back"][..],
            &["hint-choose", "hint-search", "hint-back"],
            &["hint-choose", "hint-quit"],
            &["hint-type", "hint-back"],
            &["hint-type", "hint-shift", "hint-close", "hint-done"],
            &["hint-choose", "hint-details"],
            &["hint-choose", "hint-hide-details"],
            &["hint-choose", "hint-back"],
            &["hint-options", "hint-back"],
            &["hint-back"],
            // Partitions' sheets: a size, held or not, a row, a field.
            &["hint-adjust", "hint-cancel"],
            &["hint-keep", "hint-type", "hint-put-back"],
            &["hint-choose", "hint-cancel"],
            &["hint-type", "hint-cancel"],
        ] {
            // With its pairs as close as a row short of room draws them.
            let pairs = ids.len() as f32;
            let step = legend::STEP * legend::CLOSEST;
            let mut wide = (legend::GLYPH + legend::GAP) * pairs + step * (pairs - 1.0);
            for id in ids {
                wide += shaper.width(Text::Caption, false, &text(tag, id));
            }
            let said: Vec<String> = ids.iter().map(|id| text(tag, id)).collect();
            misfits.check(tag, "legend", wide, narrowest(), &said.join(" / "));
        }
    }
    misfits.assert_none("Legends");
}

/// The headlines a stopped installation is said in, and what it is waiting
/// on before it starts.
const HEADLINES: [&str; 23] = [
    "failure-invalid-request",
    "failure-consent-mismatch",
    "failure-not-root",
    "failure-not-live",
    "failure-secure-boot",
    "failure-already-running",
    "failure-missing-tool",
    "failure-image-invalid",
    "failure-disk-changed",
    "failure-disk-busy",
    "failure-no-space",
    "failure-esp-full",
    "failure-shrink-refused",
    "failure-windows-hibernated",
    "failure-needs-check",
    "failure-windows-in-use",
    "failure-windows-needs-check",
    "failure-windows-unread",
    "failure-bitlocker",
    "failure-needs-internet",
    "failure-command",
    "failure-internal",
    "installing-waiting",
];

/// Every reason a choice cannot be installed, in the model's own words: said
/// under a place, under a field and in the foot.
const INVALID: [&str; 23] = [
    "invalid-username-empty",
    "invalid-username-shape",
    "invalid-username-reserved",
    "invalid-full-name",
    "invalid-hostname",
    "invalid-password-empty",
    "invalid-password-mismatch",
    "invalid-password-shape",
    "invalid-root-password-empty",
    "invalid-locale",
    "invalid-timezone",
    "invalid-keyboard",
    "invalid-disk-blocked",
    "invalid-target-missing",
    "invalid-target-small",
    "table-full",
    "invalid-target-esp",
    "target-protected",
    "invalid-home",
    "invalid-swap",
    "invalid-bootloader",
    "invalid-source",
    "invalid-layout",
];

/// Every reason a field of You can say under itself.
const UNDER_A_FIELD: [&str; 9] = [
    "invalid-username-empty",
    "invalid-username-shape",
    "invalid-username-reserved",
    "invalid-full-name",
    "invalid-hostname",
    "invalid-password-empty",
    "invalid-password-mismatch",
    "invalid-password-shape",
    "invalid-root-password-empty",
];

#[test]
fn every_head_and_note_takes_the_lines_it_is_given() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let words = narrowest() - Metric::ItemIcon.value() - Metric::Gap.value() * 1.4;
    for tag in TAGS {
        for id in [
            "app-name",
            "language-title",
            "timezone-title",
            "keyboard-title",
            "target-title",
            "desktop-title",
            "desktop-options-title",
            "desktop-remove-title",
            "desktop-flatpaks-title",
            "desktop-bootloader-title",
            "account-title",
            "review-title",
            "partitions-title",
            "installing-title",
            "done-title",
            "stopped-title",
        ] {
            let said = text(tag, id);
            let lines = shaper.lines(Text::Title, &said, words);
            misfits.lines(tag, "title", lines, 2, &said);
        }
        // A caption, and the headline a stop is said in where a caption
        // stands.
        for id in [
            "welcome-caption",
            "language-caption",
            "timezone-caption",
            "keyboard-caption",
            "target-caption",
            "desktop-caption",
            "desktop-options-caption",
            "desktop-remove-caption",
            "desktop-flatpaks-caption",
            "desktop-bootloader-caption",
            "account-caption",
            "review-caption",
            "installing-caption",
            "done-caption",
        ]
        .into_iter()
        .chain(HEADLINES)
        {
            let said = text(tag, id);
            let lines = shaper.lines(Text::Caption, &said, words);
            misfits.lines(tag, "caption", lines, 2, &said);
        }
        // Partitions' head says the disk shown in GParted's words, and for
        // a disk with no partition table says so in the catalog's: the
        // longest the made-up machine has.
        let nvme = format!(
            "/dev/nvme0n1 · Example NVMe SSD · 476.9\u{a0}GiB · {}",
            text(tag, "partitions-no-table")
        );
        let lines = shaper.lines(Text::Caption, &nvme, words);
        misfits.lines(tag, "caption", lines, 2, &nvme);
        // A note in the foot: beside the legend, or on two lines over the
        // buttons.
        let mut notes: Vec<String> = [
            "demo-note",
            "install-unavailable",
            "disks-unreadable",
            "install-reading",
            "installing-note",
            "target-no-disk",
            "desktop-checking",
            "desktop-offline",
            "updates-refused",
            "flatpaks-refused",
            "done-restart-failed",
            "partitions-not-yet",
            "partitions-not-yet-fresh-table",
            "partitions-not-yet-keep-table",
            "partitions-not-yet-change-entries",
            "partitions-not-yet-kept-home",
            "partitions-not-yet-several-disks",
            "partitions-not-yet-bios",
            "partitions-not-yet-shrink",
        ]
        .into_iter()
        .chain(INVALID)
        .map(|id| text(tag, id))
        .collect();
        for name in ["Linexin", "Kinexin", "LineXinBar"] {
            let value = [("desktop", Value::Words(name.into()))];
            notes.push(sentence(tag, "desktop-refused", &value));
        }
        for said in notes {
            let lines = shaper.lines(Text::Caption, &said, narrowest());
            misfits.lines(tag, "note", lines, 2, &said);
        }
    }
    misfits.assert_none("Heads and notes");
}

/// A row of a column as the screens build one.
fn row(name: &str, note: &str, chosen: bool, switch: bool) -> Row {
    let row = Row::new(name).note(note).chosen(chosen);
    if switch {
        row.switch(true)
    } else {
        row
    }
}

/// Hold `row` to a column `width` wide: its name on no more than a column's
/// lines for a name, its note on no more than its lines for a note.
fn hold(shaper: &mut Shaper, misfits: &mut Misfits, tag: &str, place: &str, width: f32, row: &Row) {
    let room = column::words_room_at(1.0, column::body_width_at(1.0, width), row);
    let lines = shaper.lines(Text::Body, &row.name, room);
    misfits.lines(tag, place, lines, column::NAME_LINES, &row.name);
    if let Some(note) = &row.note {
        let lines = shaper.lines(Text::Caption, note, room);
        misfits.lines(tag, place, lines, column::NOTE_LINES, note);
    }
}

#[test]
fn every_row_of_every_column_holds_its_words() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let right = right_column(narrowest());
    let page = narrowest().min(900.0);
    for tag in TAGS {
        let t = |id: &'static str| text(tag, id);
        // The left-hand column of a pair, at its narrowest (a panel
        // standing on its side, where it leaves the right-hand one a peek):
        // the disks, each judged. (Time zone's and Keyboard's columns are
        // the trail's, held below.)
        let mut left = Vec::new();
        for id in [
            "disk-empty",
            "blocked-installation-media",
            "blocked-in-use",
            "blocked-swap",
            "blocked-holder",
            "blocked-read-only",
            "blocked-too-small",
            "blocked-unknown",
        ] {
            left.push(row("", &t(id), false, false).mark(glyphs::DISK_SSD));
        }
        for row in &left {
            hold(
                &mut shaper,
                &mut misfits,
                tag,
                "left column",
                pair::left_width(narrowest()),
                row,
            );
        }
        // The right-hand column on a panel standing on its side: the places
        // on a disk, each judged.
        let free = sentence(
            tag,
            "target-free",
            &[("size", Value::Words(gb(tag, "300")))],
        );
        let replace = sentence(
            tag,
            "target-replace",
            &[
                ("what", Value::Words(t("content-windows"))),
                ("size", Value::Words(gb(tag, "400"))),
            ],
        );
        let mut places = vec![
            row(&t("target-erase"), &t("target-erase-note"), true, false).mark(glyphs::PLACE_ERASE),
            row(&free, &t("target-free-note"), true, false).mark(glyphs::PLACE_FREE),
            row(&replace, &t("target-replace-note"), true, false).mark(glyphs::PLACE_REPLACE),
        ];
        for id in INVALID {
            places.push(row(&t("target-erase"), &t(id), true, false).mark(glyphs::PLACE_ERASE));
        }
        for row in &places {
            hold(&mut shaper, &mut misfits, tag, "right column", right, row);
        }
        // The pages of Desktop's Options, as wide as they are drawn.
        let mut options = Vec::new();
        for (name, notes) in [
            (
                "desktop-option-remove",
                &[
                    "desktop-remove-looking",
                    "desktop-remove-unlisted",
                    "desktop-remove-none",
                    "desktop-remove-nothing",
                ][..],
            ),
            (
                "desktop-option-flatpaks",
                &["desktop-needs-internet", "desktop-checking-short"],
            ),
            (
                "desktop-option-drivers",
                &["desktop-drivers-on", "desktop-drivers-off"],
            ),
            (
                "desktop-option-snapshots",
                &[
                    "desktop-snapshots-on",
                    "desktop-snapshots-off",
                    "desktop-snapshots-needs",
                    "desktop-snapshots-var",
                ],
            ),
        ] {
            for note in notes {
                options.push(row(&t(name), &t(note), false, true));
            }
        }
        let removed = sentence(tag, "desktop-remove-count", &[("count", Value::Count(12))]);
        options.push(row(&t("desktop-option-remove"), &removed, false, false));
        let apps = sentence(
            tag,
            "desktop-flatpaks-count",
            &[("count", Value::Count(7)), ("total", Value::Count(7))],
        );
        options.push(row(&t("desktop-option-flatpaks"), &apps, false, false));
        let manager = t("bootloader-automatic");
        options.push(row(&t("desktop-option-bootloader"), &manager, false, false));
        for (name, note) in [
            ("bootloader-automatic", "bootloader-automatic-note"),
            ("bootloader-systemd-boot", "bootloader-systemd-boot-note"),
            ("bootloader-grub", "bootloader-grub-note"),
            ("bootloader-refind", "bootloader-refind-note"),
            ("bootloader-systemd-boot", "bootloader-unavailable"),
        ] {
            options.push(row(&t(name), &t(note), true, false));
        }
        for note in ["desktop-flatpak-on", "desktop-flatpak-off"] {
            options.push(row("Flatseal", &t(note), false, true));
        }
        for row in &options {
            hold(&mut shaper, &mut misfits, tag, "Options page", page, row);
        }
    }
    misfits.assert_none("Rows");
}

/// Language's rows, each with its country's flag at its start. On a landscape
/// panel, where the column is as wide as its rows need
/// (`language::rows_width_at`), a language's own name and the English one
/// under it each say all of themselves on one line beside the flag and the
/// `chosen` mark; on the narrowest panel, where the column may have to be
/// narrower, the name takes no more than a name's lines and the English no
/// more than a note's, no word cut. Measured in the toolkit's own faces: a
/// script they lack (Thai, Georgian, Arabic …) is measured here by its empty
/// boxes, and on the live system by the machine's fonts, which are what the
/// column is measured with there — so its rows are whole there whatever the
/// boxes say here.
#[test]
fn every_language_holds_its_names_beside_its_flag() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let rows_w = language::rows_width_at(1.0, |style, words| shaper.width(style, false, words));
    // The column's view: its rows and the channel kept for its scroll bar.
    let view_w = rows_w + (rows_w - column::body_width_at(1.0, rows_w));
    for (width, height) in [(1920.0, 1080.0), (1280.0, 800.0)] {
        assert!(
            view_w <= inside(width, height),
            "the flagged column is narrower than its rows on {width}×{height}"
        );
    }
    for language in crate::model::catalogs::languages() {
        let row = language::row_of(language, true);
        assert!(row.flag.is_some(), "{} wears no flag", language.locale);
        let tag = language.locale;
        let room = column::words_room_at(1.0, rows_w, &row);
        let name = shaper.width(Text::Body, false, &row.name);
        misfits.check(tag, "language name", name, room, &row.name);
        if let Some(note) = &row.note {
            let wide = shaper.width(Text::Caption, false, note);
            misfits.check(tag, "language note", wide, room, note);
        }
        let narrow = narrowest().min(view_w);
        hold(&mut shaper, &mut misfits, tag, "language row", narrow, &row);
    }
    misfits.assert_none("Languages beside their flags");
}

/// Hold the lit row's name to `room` as `trail::lit_name` says it: on two
/// lines of the large face, or three of a row's, every line whole.
fn hold_lit(
    shaper: &mut Shaper,
    misfits: &mut Misfits,
    tag: &str,
    place: &str,
    room: f32,
    name: &str,
) {
    let (style, _) = trail::lit_name(name, room, |style, candidate| {
        shaper.width(style, true, candidate)
    });
    let most = if style == Text::Title { 2 } else { 3 };
    let lines =
        super::text::break_lines(name, room, |candidate| shaper.width(style, true, candidate));
    let widest = lines
        .iter()
        .map(|line| shaper.width(style, true, line))
        .fold(0.0f32, f32::max);
    let broken = Broken {
        lines: lines.len(),
        widest,
        room,
    };
    misfits.lines(tag, place, broken, most, name);
}

/// Time zone's and Keyboard's columns, walked as the shell's file picker
/// walks a disk (`trail`): a row the light is not on says its name on one
/// line; the lit one says it larger (`hold_lit`), and its note under it on
/// one; a column stepped back says its row's name in the room up to the next
/// column, on every panel that has room for it beside the second.
#[test]
fn the_trail_holds_its_words() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let gap = Metric::Gap.value();
    // Room for the `chosen` mark after the name of the row in force.
    let tick = 30.0 + gap * 0.5;
    for tag in TAGS {
        let t = |id: &'static str| text(tag, id);
        let mut parts = vec![t("zone-region-utc")];
        for id in [
            "zone-region-africa",
            "zone-region-america",
            "zone-region-antarctica",
            "zone-region-arctic",
            "zone-region-asia",
            "zone-region-atlantic",
            "zone-region-australia",
            "zone-region-europe",
            "zone-region-indian",
            "zone-region-pacific",
        ] {
            parts.push(t(id));
        }
        for width in [narrowest(), wide()] {
            let front = trail::words_room(width, 1.0, 0, false) - tick;
            for part in &parts {
                hold_lit(
                    &mut shaper,
                    &mut misfits,
                    tag,
                    "the parts of the world, lit",
                    front,
                    part,
                );
                let measured = shaper.width(Text::Body, false, part);
                misfits.check(tag, "the parts of the world", measured, front, part);
            }
        }
        // Stepped back beside the places, on a panel that has room for its
        // name there at all (on a display standing on its side it keeps its
        // mark alone).
        let behind = trail::words_room(wide(), 1.0, 0, true) - tick;
        for part in &parts {
            let measured = shaper.width(Text::Body, false, part);
            misfits.check(
                tag,
                "a part of the world stepped back",
                measured,
                behind,
                part,
            );
        }
        // The notes under the lit row of the first column: how many places,
        // how many kinds.
        let front = trail::words_room(narrowest(), 1.0, 0, false);
        let mut notes = Vec::new();
        for count in [1, 2, 5, 22, 148] {
            notes.push(sentence(
                tag,
                "timezone-places",
                &[("count", Value::Count(count))],
            ));
        }
        for count in [0, 1, 2, 5, 22] {
            notes.push(sentence(
                tag,
                "keyboard-others",
                &[("count", Value::Count(count))],
            ));
        }
        // Under a place a search found: a short country and every part of
        // the world (every country: `the_places_hold_their_names_in_every_language`).
        for part in &parts {
            notes.push(sentence(
                tag,
                "timezone-found-note",
                &[
                    ("place", Value::Words("Peru".into())),
                    ("region", Value::Words(part.clone())),
                ],
            ));
        }
        for note in &notes {
            let measured = shaper.width(Text::Caption, false, note);
            misfits.check(tag, "a note in the first column", measured, front, note);
        }
        // The second column on the narrowest panel: universal time's own
        // words for its one place, and the plain layout's note.
        let second = trail::words_room(narrowest(), 1.0, 1, true);
        let utc = t("zone-utc");
        hold_lit(
            &mut shaper,
            &mut misfits,
            tag,
            "universal time, lit",
            second - tick,
            &utc,
        );
        let plain = t("keyboard-plain");
        let measured = shaper.width(Text::Caption, false, &plain);
        misfits.check(tag, "a note in the second column", measured, second, &plain);
        // The headings: the first column's alone and, on a landscape panel,
        // beside the second; the second's on the narrowest.
        for (id, room) in [
            (
                "timezone-heading-region",
                trail::heading_room(narrowest(), 1.0, 0, false),
            ),
            (
                "keyboard-heading-layout",
                trail::heading_room(narrowest(), 1.0, 0, false),
            ),
            (
                "timezone-heading-region",
                trail::heading_room(wide(), 1.0, 0, true),
            ),
            (
                "keyboard-heading-layout",
                trail::heading_room(wide(), 1.0, 0, true),
            ),
            (
                "timezone-heading-city",
                trail::heading_room(narrowest(), 1.0, 1, true),
            ),
            (
                "keyboard-heading-variant",
                trail::heading_room(narrowest(), 1.0, 1, true),
            ),
            // Over the one column of what a search found.
            (
                "timezone-heading-found",
                trail::heading_room(narrowest(), 1.0, 0, false),
            ),
            (
                "keyboard-heading-found",
                trail::heading_room(narrowest(), 1.0, 0, false),
            ),
        ] {
            let said = t(id);
            let measured = shaper.width(Text::Caption, false, &said);
            misfits.check(tag, "column heading", measured, room, &said);
        }
    }
    misfits.assert_none("The trail's words");
}

/// The locale each catalog stands for, whose places Time zone then says.
fn locale_of(tag: &str) -> &'static str {
    match tag {
        "en-GB" => "en_GB.UTF-8",
        "en-US" => "en_US.UTF-8",
        "de" => "de_DE.UTF-8",
        "es" => "es_ES.UTF-8",
        "fr" => "fr_FR.UTF-8",
        "hi" => "hi_IN.UTF-8",
        "pl" => "pl_PL.UTF-8",
        "pt-BR" => "pt_BR.UTF-8",
        "ru" => "ru_RU.UTF-8",
        _ => "zh_CN.UTF-8",
    }
}

/// Time zone's places and their countries as CLDR names them in the
/// language of each catalog (`model::places`): the installer's own table
/// now, not a disk's model, so held here too. Every place's name, lit,
/// stands whole — on two lines of the large face or three of a row's, a
/// word joined by hyphens parted after one where it must be — in the
/// second column and in the one column of what a search found, on every
/// panel. Unlit, a row says its name on one line, and on a landscape panel
/// the whole of it; so does the note under a place, its country (with its
/// part of the world, under one a search found). On a display standing on
/// its side the longest of those (Нью-Сейлем, Северная Дакота; Republika
/// Zielonego Przylądka) are cut with an ellipsis until the light is on them.
#[test]
fn the_places_hold_their_names_in_every_language() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let gap = Metric::Gap.value();
    // Room for the `chosen` mark after the name of the place in force.
    let tick = 30.0 + gap * 0.5;
    let carried = crate::model::places::carried();
    for tag in TAGS {
        let names = crate::model::places::Names::of(locale_of(tag));
        let name = |zone: &str| {
            names
                .city(zone)
                .map(str::to_string)
                .unwrap_or_else(|| zone.rsplit('/').next().unwrap().replace('_', " "))
        };
        let country = |code: &str| names.country(code).unwrap_or(code).to_string();
        for width in [narrowest(), wide()] {
            let second = trail::words_room(width, 1.0, 1, true) - tick;
            let found = trail::words_room(width, 1.0, 0, false) - tick;
            for (zone, _) in &carried {
                let name = name(zone);
                hold_lit(
                    &mut shaper,
                    &mut misfits,
                    tag,
                    "a place, lit",
                    second,
                    &name,
                );
                hold_lit(
                    &mut shaper,
                    &mut misfits,
                    tag,
                    "a place found, lit",
                    found,
                    &name,
                );
            }
        }
        let second = trail::words_room(wide(), 1.0, 1, true);
        let found = trail::words_room(wide(), 1.0, 0, false);
        for (zone, code) in &carried {
            let name = name(zone);
            let measured = shaper.width(Text::Body, false, &name);
            misfits.check(tag, "a place", measured, second - tick, &name);
            misfits.check(tag, "a place found", measured, found - tick, &name);
            let country = country(code);
            let measured = shaper.width(Text::Caption, false, &country);
            misfits.check(tag, "a place's country", measured, second, &country);
            // Under a place a search found, with its own part of the world.
            let id = match zone.split('/').next().unwrap() {
                "Africa" => "zone-region-africa",
                "America" => "zone-region-america",
                "Antarctica" => "zone-region-antarctica",
                "Arctic" => "zone-region-arctic",
                "Asia" => "zone-region-asia",
                "Atlantic" => "zone-region-atlantic",
                "Australia" => "zone-region-australia",
                "Europe" => "zone-region-europe",
                "Indian" => "zone-region-indian",
                _ => "zone-region-pacific",
            };
            let note = sentence(
                tag,
                "timezone-found-note",
                &[
                    ("place", Value::Words(country.clone())),
                    ("region", Value::Words(text(tag, id))),
                ],
            );
            // Where the whole does not fit, its country alone stands there
            // (`text::fit_parts`), never cut.
            let shown = super::text::cut_parts(&note, found, |candidate| {
                shaper.width(Text::Caption, false, candidate)
            });
            let measured = shaper.width(Text::Caption, false, &shown);
            let wide = if shown.ends_with('…') {
                f32::INFINITY
            } else {
                measured
            };
            misfits.check(tag, "under a place found", wide, found, &note);
        }
    }
    misfits.assert_none("The places' names");
}

#[test]
fn every_switch_field_and_card_holds_its_words() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    // You's fields and its administrator switch, as wide as its form; and
    // Keyboard's test field, which stands beside Continue only where its
    // words fit there, at most `keyboard::WIDEST` wide, and otherwise over
    // it, as wide as the room.
    let form_w = account::width_at(1.0, narrowest());
    // Language's search field, as wide as its column's rows, which are as
    // wide as the longest language needs; Time zone's and Keyboard's, over
    // the trail, lined up with its first column's heading and at most
    // `timezone::SEARCH_WIDEST` wide — each with its words beside the
    // magnifier a search field wears at any width.
    let languages_w =
        language::rows_width_at(1.0, |style, words| shaper.width(style, false, words))
            .min(narrowest());
    let language_room = field::words_room_at(1.0, languages_w, FieldId::Search);
    let trail_w = [narrowest(), wide()]
        .map(|width| (width - trail::first_edge(width, 1.0)).min(timezone::SEARCH_WIDEST))
        .into_iter()
        .fold(f32::INFINITY, f32::min);
    let trail_room = field::words_room_at(1.0, trail_w, FieldId::Search);
    let test_room = narrowest().min(keyboard::WIDEST) - Metric::RowPadding.value() * 2.0;
    for tag in TAGS {
        let t = |id: &'static str| text(tag, id);
        // Switch rows: their name and their note each on no more than a
        // switch row's lines for it.
        for (room, name, notes) in [
            (
                toggle::words_room_at(1.0, narrowest().min(760.0), false),
                "desktop-updates",
                &[
                    "desktop-updates-on",
                    "desktop-updates-off",
                    "desktop-needs-internet",
                    "desktop-checking-short",
                ][..],
            ),
            (
                toggle::words_room_at(1.0, form_w, true),
                "account-root",
                &["account-root-on", "account-root-off"],
            ),
            (
                toggle::words_room_at(1.0, account::width_at(1.0, wide()), true),
                "account-root",
                &["account-root-on", "account-root-off"],
            ),
        ] {
            let said = t(name);
            let lines = shaper.lines(Text::Body, &said, room);
            misfits.lines(tag, "switch row", lines, toggle::NAME_LINES, &said);
            for note in notes {
                let said = t(note);
                let lines = shaper.lines(Text::Caption, &said, room);
                misfits.lines(tag, "switch row", lines, toggle::NOTE_LINES, &said);
            }
        }
        // Fields: the label and the placeholder each on its line beside
        // the field's mark, on a landscape panel and on the narrowest alike
        // — and what a field says under itself on two lines at most, under
        // its words.
        let landscape = account::width_at(1.0, wide());
        for width in [form_w, landscape] {
            for (id, label, placeholder) in [
                (
                    FieldId::FullName,
                    "field-full-name",
                    Some("field-full-name-placeholder"),
                ),
                (
                    FieldId::Username,
                    "field-username",
                    Some("field-username-placeholder"),
                ),
                (
                    FieldId::Password,
                    "field-password",
                    Some("field-password-placeholder"),
                ),
                (
                    FieldId::Repeat,
                    "field-repeat",
                    Some("field-repeat-placeholder"),
                ),
                (
                    FieldId::Hostname,
                    "field-hostname",
                    Some("field-hostname-placeholder"),
                ),
                (
                    FieldId::RootPassword,
                    "field-root-password",
                    Some("field-root-password-placeholder"),
                ),
                (FieldId::RootRepeat, "field-root-repeat", None),
            ] {
                let room = field::words_room_at(1.0, width, id);
                // A step under a caption, as `field::draw` writes a heading.
                let said = t(label);
                let measured = shaper.width(Text::Caption, false, &said) * text::HEADING;
                misfits.check(tag, "field label", measured, room, &said);
                if let Some(placeholder) = placeholder {
                    // At a caption's size, as `field::draw` writes it.
                    let said = t(placeholder);
                    let measured = shaper.width(Text::Caption, false, &said);
                    misfits.check(tag, "field placeholder", measured, room, &said);
                }
            }
            let room = field::words_room_at(1.0, width, FieldId::Password);
            for id in UNDER_A_FIELD.into_iter().chain([
                "strength-weak",
                "strength-fair",
                "strength-good",
                "strength-strong",
            ]) {
                let said = t(id);
                let lines = shaper.lines(Text::Caption, &said, room);
                misfits.lines(tag, "under a field", lines, 2, &said);
            }
        }
        // Keyboard's test field, which has no mark.
        let said = t("keyboard-test");
        let measured = shaper.width(Text::Caption, false, &said) * text::HEADING;
        misfits.check(tag, "field label", measured, test_room, &said);
        let said = t("keyboard-test-placeholder");
        let measured = shaper.width(Text::Caption, false, &said);
        misfits.check(tag, "field placeholder", measured, test_room, &said);
        // The search fields: Language's, and Time zone's and Keyboard's.
        for room in [language_room, trail_room] {
            let said = t("search-field");
            let measured = shaper.width(Text::Caption, false, &said) * text::HEADING;
            misfits.check(tag, "field label", measured, room, &said);
        }
        for (id, room) in [
            ("search-language-placeholder", language_room),
            ("search-timezone-placeholder", trail_room),
            ("search-keyboard-placeholder", trail_room),
        ] {
            let said = t(id);
            let measured = shaper.width(Text::Caption, false, &said);
            misfits.check(tag, "field placeholder", measured, room, &said);
        }
        // Cards: three side by side on a landscape panel, a stack of short
        // ones on a panel standing on its side.
        let gap = Metric::Gap.value() * 1.4;
        let side_by_side = (wide() - gap * 2.0) / 3.0;
        let rooms = [
            (cards::words_room_at(1.0, side_by_side, 0.0, false), "card"),
            (
                cards::words_room_at(1.0, narrowest(), cards::STACKED, true),
                "card in a stack",
            ),
        ];
        for (room, place) in rooms {
            for id in ["desktop-linexin", "desktop-kinexin", "desktop-linexinbar"] {
                let said = t(id);
                let wide = shaper.width(Text::Title, true, &said);
                misfits.check(tag, place, wide, room, &said);
            }
            for id in [
                "desktop-about-linexin",
                "desktop-about-kinexin",
                "desktop-about-linexinbar",
                "desktop-ready",
                "desktop-needs-internet",
            ] {
                let said = t(id);
                let wide = shaper.width(Text::Caption, false, &said);
                misfits.check(tag, place, wide, room, &said);
            }
        }
        // The headings over a pair's columns.
        let headings = [("target-heading-disks", pair::left_width(narrowest()))];
        for (id, room) in headings {
            let said = t(id);
            let wide = shaper.width(Text::Caption, false, &said);
            misfits.check(tag, "column heading", wide, room, &said);
        }
    }
    misfits.assert_none("Switches, fields and cards");
}

/// How wide `words` are in `style`'s bold face. A script none of the
/// toolkit's own faces draws — Japanese kana, Korean, Arabic, which the
/// machine's fonts draw (the live medium carries noto-fonts and
/// noto-fonts-cjk) — is held to a whole em a character, wider than any of
/// those faces draws one, rather than to the empty box the toolkit's faces
/// would measure: the test must not ask which fonts this machine has.
fn bold_width_anywhere(shaper: &mut Shaper, style: Text, words: &str) -> f32 {
    let drawn = |shaper: &mut Shaper, words: &str| {
        lxb_app::lxb_render::faces::drawn(&mut shaper.fonts, words, true)
    };
    if drawn(shaper, words) {
        return shaper.width(style, true, words);
    }
    let mut wide = 0.0;
    for letter in words.chars() {
        let one = letter.to_string();
        wide += if letter == ' ' || drawn(shaper, &one) {
            shaper.width(style, true, &one)
        } else {
            style.size()
        };
    }
    wide
}

/// Welcome's twenty greetings (`welcome::GREETINGS`), which are no
/// catalog's but the installer's own: each on one line of the large face
/// across the content of a landscape panel and on three at most on one
/// standing on its side, every line whole — the room they stand in is as
/// tall as the tallest needs, so the mark never moves — and the button, as
/// wide as the widest of its twenty words, inside the band.
#[test]
fn welcome_holds_its_twenty_greetings() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    // On a landscape panel one line each; on one standing on its side,
    // which has height to spare, three at most (Добро / пожаловать в /
    // Linexin).
    for (room, most) in [
        (narrowest(), 3),
        (narrow(), 3),
        (inside(1280.0, 800.0), 1),
        (wide(), 1),
    ] {
        let mut measure = |words: &str| bold_width_anywhere(&mut shaper, Text::Display, words);
        let lines = welcome::lines_for(room, &mut measure);
        for greeting in welcome::GREETINGS {
            let broken = super::text::break_lines(greeting.welcome, room, &mut measure);
            let widest = broken
                .iter()
                .map(|line| measure(line))
                .fold(0.0f32, f32::max);
            let broken = Broken {
                lines: broken.len(),
                widest,
                room,
            };
            misfits.lines(
                greeting.language,
                "greeting",
                broken,
                most,
                greeting.welcome,
            );
        }
        let place = format!("the greeting's lines at {room:.0}");
        let all = Broken {
            lines,
            widest: 0.0,
            room,
        };
        misfits.lines("all", &place, all, most, "the tallest greeting");
    }
    let widest = welcome::GREETINGS
        .iter()
        .map(|greeting| bold_width_anywhere(&mut shaper, Text::Label, greeting.begin))
        .fold(0.0f32, f32::max);
    let button = button::width_at(1.0, widest);
    misfits.check(
        "all",
        "Welcome's button",
        button,
        narrowest(),
        "the widest word",
    );
    misfits.assert_none("Welcome");
}

/// The steps an installation is said to be on.
const STEPS: [&str; 19] = [
    "installing-waiting",
    "step-check",
    "step-resize",
    "step-partition",
    "step-format",
    "step-copy",
    "step-swap",
    "step-system",
    "step-region",
    "step-accounts",
    "step-desktop",
    "step-updates",
    "step-packages",
    "step-flatpaks",
    "step-drivers",
    "step-snapshots",
    "step-initramfs",
    "step-bootloader",
    "step-finish",
];

#[test]
fn the_installation_says_where_it_is_on_two_lines_at_most() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    for bar in [narrowest(), narrow(), wide().min(960.0)] {
        held_to_a_bar(&mut shaper, &mut misfits, bar);
    }
    misfits.assert_none("Steps");
}

/// Hold every step's words to a bar `bar` wide: beside the widest count the
/// language writes, or across the bar where the count stands under it.
fn held_to_a_bar(shaper: &mut Shaper, misfits: &mut Misfits, bar: f32) {
    for tag in TAGS {
        let values = [
            ("step", Value::Count(17)),
            ("steps", Value::Count(17)),
            ("percent", Value::Count(100)),
        ];
        let count = sentence(tag, "installing-count", &values);
        let count_w = shaper.width(Text::Caption, false, &count);
        misfits.check(tag, "count", count_w, bar, &count);
        let room = if progress::count_below_at(bar, count_w) {
            bar
        } else {
            bar - count_w - Metric::Gap.value()
        };
        for id in STEPS {
            let said = text(tag, id);
            let lines = shaper.lines(Text::Caption, &said, room);
            misfits.lines(tag, "step over the bar", lines, 2, &said);
        }
    }
}

#[test]
fn the_end_of_an_installation_says_everything_it_has_to() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let gap = Metric::Gap.value();
    // Done: the lead across the content, each thing that did not work behind
    // a hanging bullet, three lines each at most.
    let width = narrowest().min(900.0);
    for tag in TAGS {
        let lead = text(tag, "done-all-well");
        let lines = shaper.lines(Text::Body, &lead, width);
        misfits.lines(tag, "Done", lines, 3, &lead);
        let some = sentence(tag, "done-warnings", &[("count", Value::Count(3))]);
        let lines = shaper.lines(Text::Body, &some, width);
        misfits.lines(tag, "Done", lines, 3, &some);
        let hung = width - gap * 1.2 - shaper.width(Text::Body, false, "• ");
        for id in [
            "warning-updates-offline",
            "warning-updates-failed",
            "warning-flatpaks-failed",
            "warning-packages-kept",
            "warning-boot-entry",
            "warning-snapshots",
            "warning-drivers-kept",
            "warning-windows-check",
        ] {
            let said = text(tag, id);
            let lines = shaper.lines(Text::Body, &said, hung);
            misfits.lines(tag, "Done", lines, 3, &said);
        }
        // Stopped: what to do about it, across the content, six lines at
        // most.
        for id in [
            "failure-invalid-request-do",
            "failure-consent-mismatch-do",
            "failure-not-root-do",
            "failure-not-live-do",
            "failure-secure-boot-do",
            "failure-already-running-do",
            "failure-missing-tool-do",
            "failure-image-invalid-do",
            "failure-disk-changed-do",
            "failure-disk-changed-untouched-do",
            "failure-disk-busy-do",
            "failure-disk-busy-untouched-do",
            "failure-no-space-do",
            "failure-esp-full-do",
            "failure-shrink-refused-do",
            "failure-shrink-refused-untouched-do",
            "failure-windows-hibernated-do",
            "failure-needs-check-do",
            "failure-needs-check-untouched-do",
            "failure-windows-in-use-do",
            "failure-windows-needs-check-do",
            "failure-windows-unread-do",
            "failure-bitlocker-do",
            "failure-needs-internet-do",
            "failure-needs-internet-untouched-do",
            "failure-command-do",
            "failure-internal-do",
        ] {
            let said = text(tag, id);
            let lines = shaper.lines(Text::Body, &said, narrowest());
            misfits.lines(tag, "Stopped", lines, 6, &said);
        }
        // Stopped once Windows' partition was shrunk: Windows' check under
        // what to do, three lines at most.
        let said = text(tag, "warning-windows-check");
        let lines = shaper.lines(Text::Body, &said, narrowest());
        misfits.lines(tag, "Stopped", lines, 3, &said);
        // Stopped's Details: where it stopped, at the head of the lines,
        // wrapped across the pane.
        let room = log::words_room_at(1.0, narrowest());
        for id in STEPS {
            let step = Value::Words(super::text::unbroken(&text(tag, id)));
            let said = sentence(tag, "stopped-step", &[("step", step)]);
            let lines = shaper.lines(Text::Caption, &said, room);
            misfits.lines(tag, "Details", lines, log::HEAD_LINES, &said);
        }
    }
    misfits.assert_none("The end's words");
}

/// Where to install's Advanced stands at the left-hand end of the button
/// band beside Options and Continue on every landscape panel, in every
/// language, as wide as a button is anywhere — and where a display standing
/// on its side has no room for it there, stands whole over the band.
#[test]
fn advanced_stands_beside_options_and_continue_on_every_landscape_panel() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let gap = Metric::Gap.value();
    for tag in TAGS {
        let mut width = |id: &'static str| {
            button::width_at(1.0, shaper.width(Text::Label, true, &text(tag, id)))
        };
        let advanced = width("button-advanced");
        let row = width("button-options") + gap + width("button-continue");
        for (place, band) in [
            ("advanced beside, 1920×1080", wide()),
            ("advanced beside, 1280×800", inside(1280.0, 800.0)),
        ] {
            if !target::stands_beside(0.0, advanced, band - row, gap) {
                let words = text(tag, "button-advanced");
                misfits.check(tag, place, advanced + gap + row, band, &words);
            }
        }
        let words = text(tag, "button-advanced");
        misfits.check(tag, "advanced over the band", advanced, narrowest(), &words);
    }
    misfits.assert_none("Advanced");
}

/// The names in Partitions' details, by their messages.
const DETAILS: [&str; 10] = [
    "partitions-partition",
    "partitions-filesystem",
    "partitions-label",
    "partitions-size",
    "partitions-used",
    "partitions-mount-point",
    "partitions-flags",
    "partitions-pending",
    "partitions-start",
    "partitions-end",
];

/// Partitions' details hold their names and their values in every language:
/// in two columns on every landscape panel and in one on the narrowest, a
/// name in no more than half its column (where the drawing gives it) and
/// the widest value the page says beside it whole — a device of a disk with
/// many partitions, an unallocated run's place in MiB, the catalogs' words
/// for unallocated space and an unreadable filesystem.
#[test]
fn the_details_of_partitions_hold_their_names_and_values() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let gap = Metric::Gap.value();
    for tag in TAGS {
        assert_eq!(DETAILS.len(), partitions::Detail::ALL.len());
        let names = DETAILS
            .iter()
            .map(|id| shaper.width(Text::Caption, false, &text(tag, id)))
            .fold(0.0f32, f32::max);
        let mut values: Vec<String> = [
            "/dev/nvme0n1p12",
            "1953514\u{a0}MiB",
            "linux-swap",
            "hidden, diag",
            "931.5\u{a0}GiB",
        ]
        .map(String::from)
        .to_vec();
        values.push(text(tag, "partitions-unallocated"));
        values.push(text(tag, "partitions-unknown"));
        values.push(text(tag, "partitions-reading"));
        values.push(sentence(
            tag,
            "partitions-new-name",
            &[("number", Value::Count(12))],
        ));
        let widest = values
            .iter()
            .map(|value| (shaper.width(Text::Body, false, value), value.clone()))
            .fold(
                (0.0f32, String::new()),
                |a, b| if b.0 > a.0 { b } else { a },
            );
        for (place, room) in [
            ("details, 1920×1080", wide()),
            ("details, 1280×800", inside(1280.0, 800.0)),
            ("details, narrowest", narrowest()),
        ] {
            let columns = partitions::details_columns(room, names, widest.0, gap);
            let column = if columns == 2 {
                (room - gap * 2.0) * 0.5
            } else {
                room
            };
            if place != "details, narrowest" && columns != 2 {
                misfits.check(tag, place, names + gap + widest.0, column, &widest.1);
            }
            let name_room = names.min(column * 0.5);
            misfits.check(tag, place, names, name_room, "the widest name");
            misfits.check(tag, place, widest.0, column - name_room - gap, &widest.1);
            // What is pending is said in parts, as many as fit: each part
            // whole, with the longest values it takes.
            for part in [
                sentence(
                    tag,
                    "partitions-do-format",
                    &[("filesystem", Value::Words("linux-swap".into()))],
                ),
                sentence(
                    tag,
                    "partitions-do-mount",
                    &[("path", Value::Words("/home".into()))],
                ),
                sentence(
                    tag,
                    "partitions-do-delete",
                    &[("partition", Value::Words("/dev/nvme0n1p12".into()))],
                ),
                text(tag, "partitions-do-create"),
                sentence(
                    tag,
                    "partitions-do-shrink",
                    &[("size", Value::Words("1023.9\u{a0}GiB".into()))],
                ),
                sentence(
                    tag,
                    "partitions-do-grow",
                    &[("size", Value::Words("1023.9\u{a0}GiB".into()))],
                ),
            ] {
                // On one line each on a landscape panel; on the narrowest,
                // wrapped onto two at most (`partitions::value_lines`).
                let room = column - name_room - gap;
                if place == "details, narrowest" {
                    let lines = shaper.lines(Text::Body, &part, room);
                    misfits.lines(tag, place, lines, 2, &part);
                } else {
                    let wide = shaper.width(Text::Body, false, &part);
                    misfits.check(tag, place, wide, room, &part);
                }
            }
        }
    }
    misfits.assert_none("Partitions' details");
}

/// Partitions' menus hold their rows and titles in every language, at the
/// widest the toolkit lets a menu grow on the narrowest display: a tile's
/// rows with every value they can say (a mount point, none, a filesystem,
/// keep) and the longest device a disk with many partitions has; a disk's;
/// Options' with a count of two figures and the largest swap file; each
/// menu's title; and every alternative, any of which can wear the mark.
#[test]
fn partitions_menus_hold_their_rows_and_titles() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let room = menus::MenuRoom::of(1080.0 / scale_for(1920.0), 1.0);
    let handheld = menus::MenuRoom::of(800.0 / scale_for(1280.0), 1.0);
    let device = "/dev/nvme0n1p12";
    for tag in TAGS {
        let value =
            |id: &str, said: &str| sentence(tag, id, &[("value", Value::Words(said.to_string()))]);
        let none = text(tag, "partitions-none");
        let keep = text(tag, "partitions-keep");
        let mut rows = Vec::new();
        for said in ["/home", "/boot", "swap", none.as_str()] {
            rows.push(value("partitions-menu-mount", said));
        }
        for said in ["linux-swap", "btrfs", keep.as_str()] {
            rows.push(value("partitions-menu-format", said));
        }
        for said in ["16\u{a0}GiB", none.as_str()] {
            rows.push(value("partitions-swap-file", said));
        }
        // Undo changes to a device says Undo changes where the whole does
        // not fit (the title names the device): the whole is held to a
        // handheld's menu with a usual device, the shorter to every one.
        let undo = sentence(
            tag,
            "partitions-undo-part",
            &[("partition", Value::Words("/dev/sda12".into()))],
        );
        let wide = shaper.width(Text::Body, true, &undo);
        misfits.check(
            tag,
            "Partitions menu row, handheld",
            wide,
            handheld.label,
            &undo,
        );
        rows.push(text(tag, "partitions-undo"));
        rows.push(sentence(
            tag,
            "partitions-pending-list",
            &[("count", Value::Count(99))],
        ));
        // The longer of the two tables a disk may be given.
        rows.push(sentence(
            tag,
            "partitions-create-table-kind",
            &[("table", Value::Words("msdos".into()))],
        ));
        for id in [
            "partitions-delete",
            "partitions-create-table",
            "partitions-undo-last",
            "partitions-clear",
        ] {
            rows.push(text(tag, id));
        }
        for words in rows {
            let wide = shaper.width(Text::Body, true, &words);
            misfits.check(tag, "Partitions menu row", wide, room.label, &words);
        }
        for words in [
            text(tag, "partitions-mount-point"),
            text(tag, "partitions-format-as"),
            text(tag, "partitions-table"),
            text(tag, "partitions-swap-file-title"),
            device.to_string(),
        ] {
            let wide = shaper.width(Text::Title, true, &words);
            misfits.check(tag, "Partitions menu title", wide, room.title, &words);
        }
        for words in [none, keep] {
            let wide = shaper.width(Text::Body, true, &words);
            misfits.check(
                tag,
                "Partitions menu row, marked",
                wide,
                room.marked,
                &words,
            );
        }
    }
    misfits.assert_none("Partitions' menus");
}

/// The pending operations, in GParted's words, with the longest values
/// they take — a device of a disk with many partitions, linux-swap, a size
/// of four figures — hold their rooms in every language: one line each in
/// the list under the details on a landscape panel (its padding and dot
/// taken off), the line saying how many earlier ones are left out, its
/// heading and its empty line; and two lines at most each in the dialog
/// that lists them all on the narrowest display. Step 3's sentences (a new
/// partition, a size changed, a label) are held now too.
#[test]
fn the_pending_operations_hold_their_rooms() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let pad = Metric::RowPadding.value() * 0.6;
    let caption = Text::Caption.size() * Text::LINE;
    let dot = caption * 0.3;
    let pane = inside(1280.0, 800.0).min(wide()) - pad * 2.8 - dot - Metric::Gap.value() * 0.5;
    let scale = scale_for(1920.0);
    let dialog = menus::dialog_room(1080.0, scale) / scale;
    let handheld = scale_for(1280.0);
    let handheld_dialog = menus::dialog_room(800.0, handheld) / handheld;
    let words = |said: &str| Value::Words(said.to_string());
    for tag in TAGS {
        let operations = [
            sentence(
                tag,
                "partitions-op-delete",
                &[
                    ("partition", words("/dev/nvme0n1p12")),
                    ("filesystem", words("linux-swap")),
                    ("size", words("1023.9\u{a0}GiB")),
                ],
            ),
            sentence(
                tag,
                "partitions-op-format",
                &[
                    ("partition", words("/dev/nvme0n1p12")),
                    ("filesystem", words("linux-swap")),
                ],
            ),
            sentence(
                tag,
                "partitions-op-mount",
                &[
                    ("partition", words("/dev/nvme0n1p12")),
                    ("path", words("/home")),
                ],
            ),
            sentence(
                tag,
                "partitions-op-table",
                &[("table", words("msdos")), ("disk", words("/dev/nvme0n1"))],
            ),
            sentence(
                tag,
                "partitions-op-create",
                &[
                    ("number", Value::Count(12)),
                    ("filesystem", words("linux-swap")),
                    ("size", words("1023.9\u{a0}GiB")),
                    ("disk", words("/dev/nvme0n1")),
                ],
            ),
            sentence(
                tag,
                "partitions-op-shrink",
                &[
                    ("partition", words("/dev/nvme0n1p12")),
                    ("from", words("1023.9\u{a0}GiB")),
                    ("to", words("1023.9\u{a0}GiB")),
                ],
            ),
            sentence(
                tag,
                "partitions-op-grow",
                &[
                    ("partition", words("/dev/nvme0n1p12")),
                    ("from", words("1023.9\u{a0}GiB")),
                    ("to", words("1023.9\u{a0}GiB")),
                ],
            ),
            sentence(
                tag,
                "partitions-op-label",
                &[
                    ("label", words("LINEXIN_DATA")),
                    ("partition", words("/dev/nvme0n1p12")),
                ],
            ),
            sentence(
                tag,
                "partitions-op-create-at",
                &[
                    ("number", Value::Count(12)),
                    ("filesystem", words("linux-swap")),
                    ("size", words("1023.9\u{a0}GiB")),
                    ("path", words("/home")),
                    ("disk", words("/dev/nvme0n1")),
                ],
            ),
            sentence(
                tag,
                "partitions-op-label-clear",
                &[("partition", words("/dev/nvme0n1p12"))],
            ),
            sentence(
                tag,
                "partitions-op-mount",
                &[
                    (
                        "partition",
                        words(&sentence(
                            tag,
                            "partitions-new-name",
                            &[("number", Value::Count(12))],
                        )),
                    ),
                    ("path", words("/srv/media")),
                ],
            ),
        ];
        for said in &operations {
            let wide = shaper.width(Text::Caption, false, said);
            misfits.check(tag, "pending list", wide, pane, said);
            // In the dialog on the narrowest display: three lines at most,
            // with the longest device; the dialog takes as many whole
            // operations as its window holds (`menus::dialog_lines`).
            let listed = format!("•\u{a0}{said}");
            let lines = shaper.lines(Text::Caption, &listed, dialog);
            misfits.lines(tag, "pending dialog", lines, 3, &listed);
        }
        for said in [
            sentence(
                tag,
                "partitions-pending-earlier",
                &[("count", Value::Count(98))],
            ),
            text(tag, "partitions-pending-title"),
            text(tag, "partitions-pending-none"),
        ] {
            let wide = shaper.width(Text::Caption, false, &said);
            misfits.check(tag, "pending list", wide, pane, &said);
        }
        // Its title whole on a handheld standing on its side, and short on
        // the narrowest display where the whole has no room.
        let title = text(tag, "partitions-pending-title");
        let wide = shaper.width(Text::Title, true, &title);
        misfits.check(tag, "pending dialog title", wide, handheld_dialog, &title);
        let short = text(tag, "partitions-pending-short");
        let wide = shaper.width(Text::Title, true, &short);
        misfits.check(tag, "pending dialog title", wide, dialog, &short);
    }
    misfits.assert_none("Pending operations");
}

/// What Continue still needs, in every language, with the longest values
/// it takes: on one line beside Options and Continue on every landscape
/// panel, as the drawing puts it there where it fits — said bold, as a
/// refusal says it, so pressing Continue moves nothing — and on two lines at
/// most across the narrowest display, where it stands over the buttons.
#[test]
fn what_continue_needs_stands_beside_the_buttons_or_over_them() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let gap = Metric::Gap.value();
    let body = Text::Body.size() * Text::LINE;
    let marked = body * 0.8 + gap * 0.4;
    let words = |said: &str| Value::Words(said.to_string());
    for tag in TAGS {
        let mut said = vec![
            text(tag, "partitions-missing-root"),
            text(tag, "partitions-missing-efi"),
            text(tag, "partitions-missing-boot"),
            sentence(tag, "partitions-needs-linux", &[("path", words("/home"))]),
        ];
        said.push(sentence(
            tag,
            "partitions-missing-bios-grub",
            &[("disk", words("/dev/nvme0n1"))],
        ));
        said.push(sentence(
            tag,
            "partitions-too-small",
            &[("path", words("/boot")), ("size", words("600\u{a0}MiB"))],
        ));
        said.push(sentence(
            tag,
            "partitions-must-format",
            &[("partition", words("/dev/sda12")), ("path", words("/home"))],
        ));
        said.push(sentence(
            tag,
            "partitions-needs",
            &[
                ("path", words("/boot")),
                ("filesystem", words("linux-swap")),
            ],
        ));
        said.push(sentence(
            tag,
            "partitions-twice",
            &[("path", words("/home"))],
        ));
        said.push(sentence(
            tag,
            "partitions-blocked",
            &[("disk", words("/dev/nvme0n1"))],
        ));
        said.push(sentence(
            tag,
            "partitions-primaries",
            &[("disk", words("/dev/nvme0n1"))],
        ));
        said.push(sentence(
            tag,
            "partitions-needs-root",
            &[("path", words("/"))],
        ));
        // The rule set's refusals (`partitions::refusal_words`), at their
        // longest values: a partition the plan makes is named "New Partition
        // #12", a device /dev/nvme0n1p12, a size 1023.9 GiB.
        let new = sentence(tag, "partitions-new-name", &[("number", Value::Count(12))]);
        for part in ["/dev/nvme0n1p12", new.as_str()] {
            for id in [
                "partitions-refused-unaligned",
                "partitions-refused-outside",
                "partitions-refused-overlap",
                "partitions-refused-logical",
                "partitions-refused-shrink",
                "partitions-refused-grow",
                "partitions-refused-bios-grub",
                "partitions-refused-label",
                "partitions-refused-not-read",
                "partitions-refused-unread",
                "partitions-refused-hibernated",
                "partitions-refused-needs-check",
                "partitions-refused-encrypted",
                "partitions-refused-in-use",
                "partitions-refused-unreadable",
                "partitions-refused-no-hiberfile",
            ] {
                said.push(sentence(tag, id, &[("partition", words(part))]));
            }
            for (id, size) in [
                ("partitions-refused-shrink-below", "1023.9\u{a0}GiB"),
                ("partitions-refused-esp-full", "600\u{a0}MiB"),
            ] {
                said.push(sentence(
                    tag,
                    id,
                    &[("partition", words(part)), ("size", words(size))],
                ));
            }
            said.push(sentence(
                tag,
                "partitions-refused-fs-small",
                &[
                    ("partition", words(part)),
                    ("filesystem", words("linux-swap")),
                ],
            ));

            said.push(sentence(
                tag,
                "partitions-refused-unmountable",
                &[
                    ("partition", words(part)),
                    ("filesystem", words("linux_raid_member")),
                ],
            ));
        }
        // Another Windows partition holding every one back, named by its
        // device, which is always one the disk has
        // (`partitions::unshrinkable_words`).
        for id in [
            "partitions-refused-windows-unread",
            "partitions-refused-bitlocker",
        ] {
            said.push(sentence(tag, id, &[("other", words("/dev/nvme0n1p12"))]));
        }
        // Where a kept partition is mounted, a path of one's own.
        said.push(sentence(
            tag,
            "partitions-refused-inside-kept",
            &[("path", words("/mnt/windows"))],
        ));
        for id in [
            "partitions-refused-mismatch",
            "partitions-refused-beside-extended",
            "partitions-refused-entries",
            "partitions-refused-grub-gap",
        ] {
            said.push(sentence(tag, id, &[("disk", words("/dev/nvme0n1"))]));
        }
        for id in [
            "partitions-refused-boot-disk",
            "partitions-refused-image-unknown",
            "partitions-refused-image-unread",
        ] {
            said.push(sentence(tag, id, &[("path", words("/srv/media"))]));
        }
        for id in [
            "partitions-refused-restore-points",
            "partitions-refused-efi-bios",
        ] {
            said.push(text(tag, id));
        }
        said.push(sentence(
            tag,
            "partitions-refused-bootloader",
            &[("loader", words("systemd-boot"))],
        ));
        let mut width = |id: &'static str| {
            button::width_at(1.0, shaper.width(Text::Label, true, &text(tag, id)))
        };
        let row = width("button-options") + gap + width("button-continue");
        for words in &said {
            let bold = shaper.width(Text::Body, true, words);
            for (place, band) in [
                ("beside the buttons, 1920×1080", wide()),
                ("beside the buttons, 1280×800", inside(1280.0, 800.0)),
            ] {
                misfits.check(tag, place, bold, band - row - gap - marked, words);
            }
            // Broken as the drawing breaks it, measured bold.
            let room = narrowest() - marked;
            let broken =
                text::break_lines(words, room, |line| shaper.width(Text::Body, true, line));
            let widest = broken
                .iter()
                .map(|line| shaper.width(Text::Body, true, line))
                .fold(0.0f32, f32::max);
            let lines = Broken {
                lines: broken.len(),
                widest,
                room,
            };
            misfits.lines(tag, "over the buttons, narrowest", lines, 2, words);
        }
    }
    misfits.assert_none("What Continue needs");
}

#[test]
fn the_measurer_measures_as_the_toolkit_does() {
    // The bold face at a size is the Title's width scaled: `text::bold_width`
    // rests on it, and so does every bold room here.
    let mut shaper = Shaper::new();
    for words in ["Przechowywanie · Standardowe", "Продолжить", "继续"] {
        let label = shaper.width(Text::Label, true, words);
        let title =
            shaper.width(Text::Title, true, words) * Text::Label.size() / Text::Title.size();
        assert!(
            (label - title).abs() < 0.01,
            "{words}: {label} against {title}"
        );
    }
    // 800×1280 leaves the reference pixels the layout was first drawn for,
    // and a 16:9 display on its side less.
    assert!((narrow() - 539.0).abs() < 1.0, "{}", narrow());
    assert!((narrowest() - 471.5).abs() < 1.0, "{}", narrowest());
    assert!((wide() - 1192.0).abs() < 0.01, "{}", wide());
}

/// Partitions' sheets hold their words in every language, at the widest
/// values they take: each row's heading and value in its capsule — one to a
/// line on the narrowest display, two by two on a landscape panel — beside a
/// choice's chevron; each title on its line (beside the size's range on a
/// landscape panel); what a sheet says under its rows on two lines at most
/// on the narrowest and one on a landscape panel; why unallocated space
/// offers no New… under the details, likewise; the menus a sheet and a tile
/// raise, at the widest the narrowest display lets a menu grow.
#[test]
fn partitions_sheets_hold_their_words() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let gap = Metric::Gap.value();
    let pad = Metric::RowPadding.value();
    let card_pad = pad * 0.6;
    let body = Text::Body.size() * Text::LINE;
    let caption = Text::Caption.size() * Text::LINE;
    let chevron = body * 0.62 + gap * 0.5;
    let mark = caption * 0.8 + gap * 0.4;
    let menu = menus::MenuRoom::of(1080.0 / scale_for(1920.0), 1.0);
    let words = |said: &str| Value::Words(said.to_string());
    for tag in TAGS {
        let new_name = sentence(tag, "partitions-new-name", &[("number", Value::Count(12))]);
        // The rows: one to a line on the narrowest, two by two on a
        // landscape panel.
        let single = narrowest() - card_pad * 2.0;
        let half = (wide().min(inside(1280.0, 800.0)) - card_pad * 2.0 - gap * 0.5) * 0.5;
        let values = [
            text(tag, "partitions-position-start"),
            text(tag, "partitions-position-end"),
            text(tag, "partitions-bios-grub"),
            "linux-swap".to_string(),
            "/srv/media".to_string(),
            text(tag, "partitions-none"),
        ];
        let headings = [
            text(tag, "partitions-position"),
            text(tag, "partitions-filesystem"),
            text(tag, "partitions-mount-point"),
            text(tag, "partitions-label"),
        ];
        for (place, row) in [("sheet row, narrowest", single), ("sheet row, half", half)] {
            let room = row - pad * 2.0 - chevron;
            for value in &values {
                // A filesystem's value wears its colour's bead before it.
                let wide = shaper.width(Text::Body, false, value) + body * 0.36 + gap * 0.45;
                misfits.check(tag, place, wide, room, value);
            }
            for heading in &headings {
                let wide = shaper.width(Text::Caption, false, heading) * crate::ui::text::HEADING;
                misfits.check(tag, place, wide, room, heading);
            }
            // The fields' hints, in a field's own room.
            let hint = text(tag, "partitions-path-placeholder");
            let wide = shaper.width(Text::Caption, false, &hint);
            misfits.check(tag, place, wide, row - pad * 2.0, &hint);
        }
        // The titles: alone on the narrowest, beside the size's range on a
        // landscape panel.
        let range = shaper.width(Text::Caption, false, "1023.9\u{a0}GiB – 1023.9\u{a0}GiB") + gap;
        let landscape = inside(1280.0, 800.0).min(wide()) - card_pad * 2.0;
        let mut titles = vec![text(tag, "partitions-new-title")];
        for id in [
            "partitions-resize-title",
            "partitions-label-title",
            "partitions-other-title",
        ] {
            for partition in ["/dev/nvme0n1p12", new_name.as_str()] {
                titles.push(sentence(tag, id, &[("partition", words(partition))]));
            }
        }
        for title in &titles {
            // On two lines at most on the narrowest, as the drawing wraps it;
            // on one beside the size's range on a landscape panel.
            let broken = crate::ui::text::break_lines(title, single, |line| {
                shaper.width(Text::Body, true, line)
            });
            let widest = broken
                .iter()
                .map(|line| shaper.width(Text::Body, true, line))
                .fold(0.0f32, f32::max);
            let lines = Broken {
                lines: broken.len(),
                widest,
                room: single,
            };
            misfits.lines(tag, "sheet title, narrowest", lines, 2, title);
            let bold = shaper.width(Text::Body, true, title);
            misfits.check(
                tag,
                "sheet title beside its range",
                bold,
                landscape - range,
                title,
            );
        }
        // What a sheet says under its rows.
        let notes = [
            sentence(
                tag,
                "partitions-leaves",
                &[("size", words("1023.9\u{a0}GiB"))],
            ),
            text(tag, "partitions-leaves-none"),
            sentence(
                tag,
                "partitions-takes",
                &[("size", words("1023.9\u{a0}GiB"))],
            ),
            text(tag, "partitions-takes-all"),
            sentence(
                tag,
                "partitions-at-least",
                &[("size", words("23.3\u{a0}GiB"))],
            ),
            text(tag, "partitions-path-whole"),
            sentence(
                tag,
                "partitions-path-twice",
                &[("path", words("/srv/media"))],
            ),
            sentence(
                tag,
                "partitions-path-system",
                &[("path", words("/swapfile"))],
            ),
            text(tag, "partitions-path-long"),
            text(tag, "partitions-path-spaces"),
            sentence(tag, "partitions-path-under", &[("path", words("/home"))]),
            sentence(
                tag,
                "partitions-label-bytes",
                &[("filesystem", words("btrfs")), ("count", Value::Count(255))],
            ),
            sentence(
                tag,
                "partitions-label-bytes",
                &[("filesystem", words("fat32")), ("count", Value::Count(11))],
            ),
            sentence(
                tag,
                "partitions-label-chars",
                &[("filesystem", words("exfat")), ("count", Value::Count(128))],
            ),
            text(tag, "partitions-label-slash"),
        ];
        for note in &notes {
            // Measured bold, as a refused one is said.
            let room = single - mark;
            let broken = crate::ui::text::break_lines(note, room, |line| {
                shaper.width(Text::Caption, true, line)
            });
            let widest = broken
                .iter()
                .map(|line| shaper.width(Text::Caption, true, line))
                .fold(0.0f32, f32::max);
            let lines = Broken {
                lines: broken.len(),
                widest,
                room,
            };
            misfits.lines(tag, "sheet note, narrowest", lines, 2, note);
            let bold = shaper.width(Text::Caption, true, note);
            misfits.check(tag, "sheet note, landscape", bold, landscape - mark, note);
        }
        // Why unallocated space offers no New…, under the details — and why
        // a disk written over as a whole offers no table or partition.
        let whys = [
            text(tag, "partitions-why-whole"),
            text(tag, "partitions-why-table"),
            text(tag, "partitions-why-primaries"),
            text(tag, "partitions-why-logical"),
            text(tag, "partitions-why-beside-extended"),
            sentence(
                tag,
                "partitions-why-unmountable",
                &[("filesystem", words("linux_raid_member"))],
            ),
            text(tag, "partitions-why-unformatted"),
            text(tag, "partitions-why-logical-delete"),
            // Under every tile of a disk whose sector the rules refuse.
            sentence(
                tag,
                "partitions-blocked",
                &[("disk", words("/dev/nvme0n1"))],
            ),
        ];
        // Why a kept partition is not shrunk, as reading said, under the
        // details too (`partitions::unshrunk_words`), with the longest
        // device: of it, and of another Windows partition holding it back,
        // with what to do.
        let device = || words("/dev/nvme0n1p12");
        let whys = whys
            .into_iter()
            .chain(
                [
                    "partitions-refused-hibernated",
                    "partitions-refused-needs-check",
                    "partitions-refused-encrypted",
                    "partitions-refused-in-use",
                    "partitions-refused-unreadable",
                ]
                .map(|id| sentence(tag, id, &[("partition", device())])),
            )
            .chain([
                text(tag, "partitions-why-no-hiberfile"),
                sentence(tag, "partitions-why-hibernated", &[("other", device())]),
                sentence(tag, "partitions-why-windows-unread", &[("other", device())]),
                sentence(tag, "partitions-why-bitlocker", &[("other", device())]),
            ]);
        for why in whys {
            let room = narrowest() - body * 0.8 - gap * 0.4;
            let lines = shaper.lines(Text::Body, &why, room);
            misfits.lines(tag, "why no New…, narrowest", lines, 2, &why);
            let wide_why = shaper.width(Text::Body, false, &why);
            let landscape = inside(1280.0, 800.0) - body * 0.8 - gap * 0.4;
            misfits.check(tag, "why no New…, landscape", wide_why, landscape, &why);
        }
        // The menus a tile and a sheet raise.
        let mut rows = vec![
            text(tag, "partitions-new"),
            text(tag, "partitions-resize"),
            text(tag, "partitions-relabel"),
            text(tag, "partitions-other"),
            text(tag, "partitions-position-start"),
            text(tag, "partitions-position-end"),
            text(tag, "partitions-bios-grub"),
        ];
        rows.push("/srv/media/photographs".to_string());
        for row in &rows {
            let bold = shaper.width(Text::Body, true, row);
            misfits.check(tag, "sheet menu row", bold, menu.marked, row);
        }
        for title in [
            text(tag, "partitions-position"),
            text(tag, "partitions-filesystem"),
            text(tag, "partitions-unallocated"),
            new_name.clone(),
        ] {
            let bold = shaper.width(Text::Title, true, &title);
            misfits.check(tag, "sheet menu title", bold, menu.title, &title);
        }
    }
    misfits.assert_none("Partitions' sheets");
}

/// Partitions' plan as the choice in use, in every language, with the
/// longest values it takes (`/dev/nvme0n1p12`, `linux-swap`, `1023.9 GiB`,
/// "New Partition #12"): Where to install's foot saying it is in use, or
/// that it was cleared, on two lines at most on the narrowest display, as
/// every note; Review's rows for it — a mount point's path or the swap
/// file's and the operations' names on one line, what each says under it on
/// a note's two lines, no word cut — in rows as wide as the narrowest
/// display's; and the last question's line over what the plan destroys, and
/// its word where it destroys nothing, on two lines at most in the
/// narrowest dialog (the sentences under it are the pending list's, held
/// above to three).
#[test]
fn the_plan_in_use_holds_its_words_on_where_to_install_review_and_the_last_question() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let scale = scale_for(1920.0);
    let dialog = menus::dialog_room(1080.0, scale) / scale;
    let rows = narrowest().min(960.0);
    let words = |said: &str| Value::Words(said.to_string());
    for tag in TAGS {
        let mut notes = vec![text(tag, "target-partitions-let-go")];
        for count in [1, 2, 5, 22, 98] {
            notes.push(sentence(
                tag,
                "target-using-partitions",
                &[("count", Value::Count(count))],
            ));
        }
        for said in notes {
            let lines = shaper.lines(Text::Caption, &said, narrowest());
            misfits.lines(tag, "Where to install's foot", lines, 2, &said);
        }
        let new = sentence(tag, "partitions-new-name", &[("number", Value::Count(12))]);
        let mut answers = vec![sentence(
            tag,
            "review-part-new",
            &[
                ("partition", words(&new)),
                ("filesystem", words("linux-swap")),
                ("size", words("1023.9\u{a0}GiB")),
                ("disk", words("/dev/nvme0n1")),
            ],
        )];
        for id in ["review-part-formatted", "review-part-kept"] {
            answers.push(sentence(
                tag,
                id,
                &[
                    ("partition", words("/dev/nvme0n1p12")),
                    ("filesystem", words("linux-swap")),
                    ("size", words("1023.9\u{a0}GiB")),
                ],
            ));
        }
        for count in [1, 2, 5, 22, 98] {
            answers.push(sentence(
                tag,
                "review-operations",
                &[("count", Value::Count(count))],
            ));
        }
        let names = [
            "/srv/media".to_string(),
            text(tag, "partitions-swap-file-title"),
            text(tag, "partitions-title"),
        ];
        for name in &names {
            let row = row(name, "", false, false).mark(glyphs::DISK_PARTITIONS);
            let room = column::words_room_at(1.0, rows, &row);
            let wide = shaper.width(Text::Body, false, name);
            misfits.check(tag, "Review's row", wide, room, name);
        }
        for said in &answers {
            let row = row("/", said, false, false).mark(glyphs::DISK_TARGET);
            let room = column::words_room_at(1.0, rows, &row);
            let lines = shaper.lines(Text::Caption, said, room);
            misfits.lines(tag, "Review's row", lines, review::NOTE_LINES, said);
        }
        for id in ["erase-plan", "erase-plan-nothing"] {
            let said = text(tag, id);
            let lines = shaper.lines(Text::Caption, &said, dialog);
            misfits.lines(tag, "last question", lines, 2, &said);
        }
        // The plan's paged question: each partition a new table erases, and
        // a sentence about an EFI system partition or Windows' own named so,
        // on three lines at most after its bullet or its dash in the
        // narrowest dialog, with the longest values; which page it is, on
        // one line.
        let erased = sentence(
            tag,
            "erase-plan-erased",
            &[
                ("partition", words("/dev/nvme0n1p12")),
                ("filesystem", words("linux-swap")),
                ("size", words("1023.9\u{a0}GiB")),
            ],
        );
        let deleted = sentence(
            tag,
            "partitions-op-delete",
            &[
                ("partition", words("/dev/nvme0n1p12")),
                ("filesystem", words("linux-swap")),
                ("size", words("1023.9\u{a0}GiB")),
            ],
        );
        let shrunk = sentence(
            tag,
            "partitions-op-shrink",
            &[
                ("partition", words("/dev/nvme0n1p12")),
                ("from", words("1023.9\u{a0}GiB")),
                ("to", words("1023.9\u{a0}GiB")),
            ],
        );
        let formatted = sentence(
            tag,
            "partitions-op-format",
            &[
                ("partition", words("/dev/nvme0n1p12")),
                ("filesystem", words("linux-swap")),
            ],
        );
        // An EFI system partition is never shrunk (`layout::shrinkable`).
        let mut rows = vec![format!("•\u{a0}\u{a0}–\u{a0}{erased}")];
        for (what, ids) in [
            (&erased, &["erase-plan-esp", "erase-plan-windows"][..]),
            (&deleted, &["erase-plan-esp", "erase-plan-windows"][..]),
            (&formatted, &["erase-plan-esp", "erase-plan-windows"][..]),
            (&shrunk, &["erase-plan-windows"][..]),
        ] {
            for id in ids {
                let named = partitions::name_after(what.to_string(), |what| {
                    sentence(tag, id, &[("what", words(what))])
                });
                rows.push(format!("•\u{a0}\u{a0}–\u{a0}{named}"));
            }
        }
        for said in &rows {
            let lines = shaper.lines(Text::Caption, said, dialog);
            misfits.lines(tag, "plan's question", lines, 3, said);
        }
        let page = sentence(
            tag,
            "erase-plan-page",
            &[("page", Value::Count(12)), ("pages", Value::Count(12))],
        );
        let lines = shaper.lines(Text::Caption, &page, dialog);
        misfits.lines(tag, "plan's question", lines, 1, &page);
    }
    misfits.assert_none("The plan in use");
}

/// The plan's paged question (`Question::PlanErase`), in Polish and German,
/// on a display standing on its side (1080×1920) and on a landscape one
/// (1280×800), as `partitions::plan_pages` breaks it with the toolkit's own
/// measurer — the plan of its pictures, a new table erasing forty
/// partitions beside the Windows disk's shrink, format and delete: more
/// pages than one; every page within the lines the dialog holds and every
/// line within its width; its head on every page, "This cannot be undone."
/// on the last alone, which page it is on each; every line of what the plan
/// destroys said, in order (a new table's sentence again at the top of a
/// page it goes on over); and never a count of what was left out.
#[test]
fn the_plans_question_says_every_line_on_its_pages() {
    let mut shaper = Shaper::new();
    let mut misfits = Misfits::default();
    let flat = |said: &str| {
        said.chars()
            .map(|letter| match letter {
                '\u{a0}' | '\u{202f}' => ' ',
                other => other,
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    };
    for tag in ["pl", "de"] {
        let _here = i18n::speak_here(tag);
        let mut state = crate::ui::app::State::demo(true);
        partitions::stage_plan_pages(&mut state);
        state.views.partitions.in_use = true;
        let destroys = partitions::destroys(&state);
        let rows: Vec<String> = destroys
            .iter()
            .flat_map(|destroy| partitions::destroy_rows(destroy, &state.disks))
            .map(|row| flat(&row))
            .collect();
        assert_eq!(destroys.len(), 4, "{tag}");
        assert_eq!(rows.len(), 3 + 1 + 40, "{tag}");
        let earlier: Vec<String> = (1..=60)
            .map(|count| {
                sentence(
                    tag,
                    "partitions-pending-earlier",
                    &[("count", Value::Count(count))],
                )
            })
            .collect();
        for (width, height) in [(1080.0, 1920.0), (1280.0, 800.0)] {
            let place = format!("{width}x{height}");
            let scale = scale_for(height);
            let room = menus::dialog_room(width, scale) / scale;
            let most = menus::dialog_lines(height, scale);
            let pages = partitions::plan_pages(&state, most, room, |line| {
                shaper.width(Text::Caption, false, line)
            });
            assert!(pages.len() > 1, "{tag} {place}: one page");
            let head = flat(&text(tag, "erase-plan"));
            let tail = flat(&text(tag, "erase-final"));
            let mut body = String::new();
            for (at, page) in pages.iter().enumerate() {
                let lines: Vec<&str> = page.split('\n').collect();
                if lines.len() > most {
                    let said = format!(
                        "{tag:6} {place}: page {} takes {} lines of {most}",
                        at + 1,
                        lines.len()
                    );
                    misfits.0.push(said);
                }
                for line in &lines {
                    let wide = shaper.width(Text::Caption, false, line);
                    misfits.check(tag, &format!("plan's question {place}"), wide, room, line);
                }
                let said = flat(page);
                assert!(
                    said.starts_with(&head),
                    "{tag} {place}: page {} has no head",
                    at + 1
                );
                let counted = sentence(
                    tag,
                    "erase-plan-page",
                    &[
                        ("page", Value::Count(at as i64 + 1)),
                        ("pages", Value::Count(pages.len() as i64)),
                    ],
                );
                assert!(said.ends_with(&flat(&counted)), "{tag} {place}: {said}");
                let last = at + 1 == pages.len();
                assert_eq!(said.contains(&tail), last, "{tag} {place}: page {}", at + 1);
                for count in &earlier {
                    assert!(!page.contains(count.as_str()), "{tag} {place}: {count}");
                }
                body.push(' ');
                body.push_str(&said);
            }
            let mut from = 0;
            for row in &rows {
                let found = body[from..].find(row.as_str());
                let at = found.unwrap_or_else(|| panic!("{tag} {place}: {row} is not said"));
                from += at + row.len();
            }
        }
    }
    misfits.assert_none("The plan's question");
}
