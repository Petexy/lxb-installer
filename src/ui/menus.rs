//! The words handed to the toolkit's menu and dialog, fitted to them.
//!
//! The installer raises the toolkit's own menu and question dialog, which
//! light what the pointer is over and answer a pad the way the shell's do.
//! A menu grows to its widest row, up to the window less its inset, and a
//! row it still cannot hold the toolkit ends with an ellipsis; a dialog's
//! title is one line at the dialog's width. A row such as "Przechowywanie ·
//! Standardowe" could run out of the Options menu, so what is handed over is
//! measured here against the very numbers the toolkit lays them out by. **The
//! catalogs' own words are never cut**: the catalogs' test (`fit.rs`) holds
//! every Options row, with every setting it can say, to the widest menu the
//! narrowest display allows. Where a row carries a name the machine gave — a
//! partition's label — and the whole does not fit, it is that name which is
//! cut, with an ellipsis, and the catalog's words around it stay whole.

use lxb_app::lxb_render::Ui;
use lxb_app::lxb_toolkit::{
    menu,
    metrics::{self, Metric},
    typography::Text,
};

/// How much room the toolkit's menu leaves its words at most, on a display
/// `display` wide at the toolkit's `scale` for its height: a row's label, a
/// marked row's (whose `chosen` mark takes some of it), and the title's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuRoom {
    pub label: f32,
    pub marked: f32,
    pub title: f32,
}

/// What stands around a row's words in the toolkit's menu, in reference
/// pixels: the margin and the label's padding either side.
const AROUND: f32 = 2.0 * (menu::MARGIN + menu::LABEL_PADDING);

/// How much a marked row's `chosen` mark takes of its words' room, in
/// reference pixels: as tall as `CONTEXT.glyph` of the row inside its
/// padding, and half a label's padding after it.
const MARK: f32 =
    (menu::ROW - menu::ROW_PADDING * 2.0) * menu::CONTEXT.glyph + menu::LABEL_PADDING * 0.5;

impl MenuRoom {
    /// From `lxb_toolkit::menu`'s own numbers, as `Ui::context_menu` lays a
    /// menu out: a pane grown to its widest row, never wider than the
    /// display less its inset either side (`menu::place`), its words inside
    /// the margin and the label's padding.
    pub fn of(display: f32, scale: f32) -> Self {
        let inset = Metric::PanelInset.value() * scale;
        let pane = (display - inset * 2.0).max(0.0);
        let label = (pane - AROUND * scale).max(0.0);
        Self {
            label,
            marked: (label - MARK * scale).max(0.0),
            title: label,
        }
    }
}

/// How wide the toolkit's menu grows for `rows` — the `marked` one wearing
/// its mark — and `title`, in reference pixels: its own width, or what the
/// widest of them needs with what stands around it, as `Ui::context_menu`
/// measures it (a row's words in the bold face a lit row is written in).
/// `menu::place` then holds it inside the display.
pub fn grown_width(
    ui: &mut Ui,
    rows: &[String],
    marked: Option<usize>,
    title: Option<&str>,
    scale: f32,
) -> f32 {
    let scale = scale.max(f32::EPSILON);
    let mut widest = title.map_or(0.0, |title| {
        super::text::bold_width(ui, Text::Title, title) / scale + AROUND
    });
    for (at, row) in rows.iter().enumerate() {
        let mark = if marked == Some(at) { MARK } else { 0.0 };
        let words = super::text::bold_width(ui, Text::Body, row) / scale;
        widest = widest.max(words + AROUND + mark);
    }
    widest.max(menu::WIDTH)
}

/// What to tell the toolkit a menu of `rows` rows (with a title or not),
/// `width` reference pixels wide ([`grown_width`]), grows out of, so that
/// it stands beside `button` on a display `display` big and never over it.
///
/// The toolkit lays a menu to the right of what it grows out of, else to
/// its left, centred on it from top to bottom — and where it fits on
/// neither side (a display standing on its side, or a menu grown wide for
/// long rows) it is pushed back inside the window, over the very button it
/// came from, hiding it. There the menu is told to grow out of a rectangle
/// as wide as the button, standing just so far over it that the menu's foot
/// comes to rest a gap above the button: the menu rises out of Options
/// instead of covering it.
pub fn clear_of(
    button: [f32; 4],
    rows: usize,
    titled: bool,
    width: f32,
    display: [f32; 2],
) -> [f32; 4] {
    let shape = vec![menu::Row::command(1); rows.max(1)];
    let title = titled.then_some(1);
    let visible = menu::rows_of_that_fit(&shape, title, display[1]);
    let extra = (width - menu::WIDTH).max(0.0);
    let placed = menu::Layout::new(&shape, title, button, display, 0, visible, 0, 1.0, extra).rect;
    if super::column::intersection(placed, button).is_none() {
        return button;
    }
    let gap = menu::GAP * metrics::scale_for(display[1]);
    let middle = button[1] - gap - placed[3] * 0.5;
    [button[0], middle - button[3] * 0.5, button[2], button[3]]
}

/// The words of one row: `whole` where it fits `room` in the bold face a
/// focused row is drawn in; else, where the row carries a name the machine
/// gave (`named`, somewhere in `whole`), the whole with that name cut as
/// far as it has to be; else the whole cut with an ellipsis.
pub fn row_words(ui: &mut Ui, whole: &str, named: Option<&str>, room: f32) -> String {
    let mut measure = |words: &str| super::text::bold_width(ui, Text::Body, words);
    cut_the_name(whole, named, room, &mut measure)
}

/// The arithmetic of [`row_words`], with the measurer handed in so it can be
/// tested without a renderer.
fn cut_the_name(
    whole: &str,
    named: Option<&str>,
    room: f32,
    measure: &mut impl FnMut(&str) -> f32,
) -> String {
    if measure(whole) <= room {
        return whole.to_string();
    }
    if let Some(name) = named.filter(|name| !name.is_empty() && whole.contains(*name)) {
        // How wide the rest of the row is, and so how much is left for the
        // name: the row with the name taken out, plus the name cut to fit.
        let without = measure(&whole.replacen(name, "", 1));
        let cut = super::text::cut_to(name, room - without, &mut *measure);
        let said = whole.replacen(name, &cut, 1);
        if !cut.is_empty() && measure(&said) <= room {
            return said;
        }
    }
    super::text::cut_to(whole, room, &mut *measure)
}

/// How much room the toolkit's dialog leaves its title and its row of
/// answers, as `Ui::dialog` lays it out: `Metric::DialogWidth`, or seven
/// tenths of the display where that is less, inside the panel's padding.
pub fn dialog_room(display: f32, scale: f32) -> f32 {
    let width = (Metric::DialogWidth.value() * scale).min(display * 0.7);
    (width - Metric::PanelPadding.value() * scale * 2.0).max(0.0)
}

/// The dialog's body, broken into its lines here and handed over one line
/// to a line.
///
/// The dialog keeps the lines it is given and breaks only one wider than
/// its pane, at spaces — so a Chinese sentence, which has none, would run
/// whole past the pane's edge. Broken here by `text::break_lines`, against
/// the room and the face the dialog measures its lines by, every line fits
/// and is drawn as it came: a disk's name held together (`text::unbroken`)
/// stays on one line, and no line begins with a lone "·". A line break the
/// body already has is kept, each line broken on its own: Partitions' list
/// of pending operations is one to a line, and ran together as a paragraph.
pub fn body_words(ui: &mut Ui, body: &str, room: f32) -> String {
    body.split('\n')
        .flat_map(|line| {
            super::text::break_lines(line, room, |candidate| ui.measure(Text::Caption, candidate))
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// How many lines of its body the toolkit's dialog holds on a window
/// `height` tall at the design language's `scale`, as `Ui::dialog` lays it
/// out — its padding, its title, a gap either side of the body, the row of
/// answers — with the panel's inset clear above and below it: a dialog is
/// centred and grows with its body, and one taller than the window ran off
/// both its edges.
pub fn dialog_lines(height: f32, scale: f32) -> usize {
    let metric = |metric: Metric| metric.value() * scale;
    let line = |text: Text| text.size() * Text::LINE * scale;
    let room = height
        - metric(Metric::PanelInset) * 2.0
        - metric(Metric::PanelPadding) * 2.0
        - line(Text::Title)
        - metric(Metric::Gap) * 2.0
        - metric(Metric::RowHeight) * 0.6;
    (room / line(Text::Caption)).floor().max(1.0) as usize
}

/// How wide the dialog draws a row of `answers`: each in the regular face
/// with a row's padding either side, a gap between them.
pub fn answers_width(ui: &mut Ui, answers: &[&str], scale: f32) -> f32 {
    let pad = Metric::RowPadding.value() * scale;
    let gap = Metric::Gap.value() * scale;
    let words: f32 = answers
        .iter()
        .map(|answer| ui.measure(Text::Label, answer) + pad * 2.0)
        .sum();
    words + gap * answers.len().saturating_sub(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ten points a letter, the ellipsis included.
    fn even(text: &str) -> f32 {
        text.chars().count() as f32 * 10.0
    }

    #[test]
    fn a_menu_beside_its_button_is_left_there_and_one_over_it_rises_above_it() {
        // Options on a landscape panel: the menu fits to its left.
        let wide = [1200.0, 900.0, 200.0, 54.0];
        assert_eq!(clear_of(wide, 3, true, menu::WIDTH, [1920.0, 1080.0]), wide);
        // Options on 800x1280: no room either side, so the menu is told to
        // grow out of a place over the button, and comes to rest above it.
        let display = [800.0, 1280.0];
        let button = [330.0, 1080.0, 230.0, 64.0];
        let anchor = clear_of(button, 3, true, menu::WIDTH, display);
        assert_ne!(anchor, button);
        let shape = vec![menu::Row::command(1); 3];
        let placed = menu::Layout::new(&shape, Some(1), anchor, display, 0, 3, 0, 1.0, 0.0).rect;
        assert!(
            placed[1] + placed[3] <= button[1],
            "the menu still covers the button: {placed:?}"
        );
    }

    #[test]
    fn a_menu_grown_too_wide_for_either_side_of_its_button_rises_above_it() {
        // On 1280x800 a menu of its own width fits beside Options; grown
        // for long rows it no longer does, and would cover the button.
        let display = [1280.0, 800.0];
        let scale = 800.0 / 1080.0;
        let button = [580.0, 610.0, 150.0, 44.0];
        assert_eq!(clear_of(button, 3, true, menu::WIDTH, display), button);
        let wide = clear_of(button, 3, true, 800.0, display);
        assert_ne!(wide, button);
        let shape = vec![menu::Row::command(1); 3];
        let extra = 800.0 - menu::WIDTH;
        let placed = menu::Layout::new(&shape, Some(1), wide, display, 0, 3, 0, 1.0, extra).rect;
        assert!(
            placed[1] + placed[3] <= button[1],
            "{placed:?} over the button"
        );
        assert!(placed[2] > menu::WIDTH * scale, "the menu did not grow");
    }

    #[test]
    fn a_row_that_fits_is_said_whole() {
        let whole = "Your files · With the system";
        assert_eq!(cut_the_name(whole, None, 300.0, &mut even), whole);
    }

    #[test]
    fn where_a_row_does_not_fit_the_name_the_machine_gave_is_cut_and_nothing_else() {
        let whole = "Your files · “Family photos”, kept";
        let cut = cut_the_name(whole, Some("“Family photos”"), 250.0, &mut even);
        assert_eq!(cut, "Your files · “Fami…, kept");
        assert!(even(&cut) <= 250.0);
    }

    #[test]
    fn a_row_with_no_room_even_for_a_cut_name_is_cut_at_its_end() {
        let whole = "Your files · “Family photos”, kept";
        let cut = cut_the_name(whole, Some("“Family photos”"), 120.0, &mut even);
        assert!(cut.ends_with('…') && even(&cut) <= 120.0, "{cut}");
    }

    #[test]
    fn a_menu_may_grow_to_the_display_less_its_inset() {
        // 1080x1920 at its scale: 607.5 reference pixels wide, an inset of
        // 14, a margin of 24 and a padding of 24 either side.
        let scale = 1920.0 / 1080.0;
        let room = MenuRoom::of(1080.0, scale);
        let label = (1080.0 / scale - 28.0 - 96.0) * scale;
        assert!((room.label - label).abs() < 0.01, "{room:?}");
        assert!(room.marked < room.label && room.marked > room.label * 0.85);
        assert_eq!(room.title, room.label);
        assert!(MenuRoom::of(300.0, 1.0).label <= 300.0 - 28.0 - 96.0 + 0.01);
    }

    #[test]
    fn a_dialog_holds_as_many_lines_as_the_window_has_room_for() {
        // Every height has the same reference room: the scale is the
        // height's.
        let lines = dialog_lines(1080.0, 1.0);
        assert!((25..40).contains(&lines), "{lines}");
        assert_eq!(dialog_lines(1920.0, 1920.0 / 1080.0), lines);
        assert_eq!(dialog_lines(100.0, 1.0), 1);
    }

    #[test]
    fn a_dialog_on_a_display_standing_on_its_side_is_seven_tenths_of_it() {
        // 800x1280: the toolkit's scale for 1280 tall.
        let scale = 1280.0 / 1080.0;
        let room = dialog_room(800.0, scale);
        assert!((room - (560.0 - 88.0 * scale)).abs() < 0.01, "{room}");
        assert!((dialog_room(1920.0, 1.0) - (680.0 - 88.0)).abs() < 0.01);
    }
}
