//! A row that is a switch: "Install updates", "Root account", "Snapshots".
//!
//! A row like any other — a name, and what it does under it — with a switch
//! at its end that says whether it is on. Accept flips it; there is nothing
//! else to do with a switch, so the light stays where it is.

use lxb_app::lxb_render::{Align, Press, Ui};
use lxb_app::lxb_toolkit::{
    metrics::Metric, motion, palette::Role, settings::IconStyle, typography::Text,
};
use lxb_app::Page;

use super::motion::Light;
use super::text;

/// One switch row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toggle<'a> {
    pub name: &'a str,
    pub note: Option<&'a str>,
    pub on: bool,
    pub enabled: bool,
    /// The mark at the row's start, as a column's row wears one (You's
    /// administrator switch wears the shell's shield), or none.
    pub mark: Option<&'a str>,
    /// The switch cannot be turned on now (updates with no internet): the
    /// row is outlined like one that cannot be used and its note is said as
    /// a refusal — but the light still stops on it, so a press is answered with
    /// the Error sound and the reason rather than with silence.
    pub refused: bool,
}

/// How tall a switch's track is, as a share of a row's height, how much
/// longer than tall it is, and how far the dark capsule round it stands past
/// it, as a share of its height.
const TRACK: f32 = 0.38;
const TRACK_LONG: f32 = 1.9;
const RING: f32 = 0.14;

/// How wide a switch's track is at the end of a row of the design language.
pub fn track_width(page: &Page) -> f32 {
    track_width_at(page.scaled(1.0))
}

/// [`track_width`] at the design language's `scale`, with no page.
pub fn track_width_at(scale: f32) -> f32 {
    Metric::RowHeight.value() * scale * TRACK * TRACK_LONG
}

/// How much room a switch row `width` wide leaves its words, at `scale`,
/// with a mark at its start or without.
pub fn words_room_at(scale: f32, width: f32, marked: bool) -> f32 {
    let pad = Metric::RowPadding.value() * scale;
    let gap = Metric::Gap.value() * scale;
    let mark = if marked {
        mark_size_at(scale) + gap
    } else {
        0.0
    };
    (width - pad * 2.0 - track_width_at(scale) - gap - mark).max(0.0)
}

/// How large the mark at a switch row's start is, at `scale`: a column's
/// row's, which is Review's.
fn mark_size_at(scale: f32) -> f32 {
    Metric::ItemIcon.value() * scale * 0.6
}

/// How tall a switch row `width` wide is: a row of the design language, and
/// a line more for each line its name and its note take past the first —
/// words too long for their line wrap rather than being cut.
pub fn height(page: &mut Page, width: f32, toggle: &Toggle<'_>) -> f32 {
    let names = name_lines(page, width, toggle).len();
    let notes = note_lines(page, width, toggle, NOTE_LINES).len();
    page.metric(Metric::RowHeight)
        + names.saturating_sub(1) as f32 * page.line(Text::Body)
        + notes.saturating_sub(1) as f32 * page.line(Text::Caption)
}

/// The lines a switch row's name is drawn on in a row `width` wide: one, or
/// two where it does not fit ("Учётная запись администратора" beside its
/// switch on a display standing on its side).
fn name_lines(page: &mut Page, width: f32, toggle: &Toggle<'_>) -> Vec<String> {
    let room = words_room_at(page.scaled(1.0), width, toggle.mark.is_some());
    text::wrapped(page.ui(), Text::Body, toggle.name, room, NAME_LINES)
}

/// The most lines a switch row's name is given.
pub const NAME_LINES: usize = 2;

/// The lines a switch row's note is drawn on in a row `width` wide, up to
/// `most`.
fn note_lines(page: &mut Page, width: f32, toggle: &Toggle<'_>, most: usize) -> Vec<String> {
    let Some(note) = toggle.note else {
        return Vec::new();
    };
    let room = words_room_at(page.scaled(1.0), width, toggle.mark.is_some());
    text::wrapped(page.ui(), Text::Caption, note, room, most.max(1))
}

/// The most lines a switch row's note is given.
pub const NOTE_LINES: usize = 3;

/// Draw a switch row in `rect`, the light on it when `light` is handed in.
/// `spot` is the number a click on it comes back as.
pub fn draw(
    page: &mut Page,
    rect: [f32; 4],
    toggle: Toggle<'_>,
    light: Option<&mut Light>,
    spot: u32,
    pressing: Option<f32>,
    dt: f32,
) {
    let lit = light.is_some();
    let (radius, shape) = super::column::radius(page, rect);
    if let Some(light) = light {
        light.lay(page, motion::pressed(rect, pressing), shape, pressing, dt);
    }
    let press = match pressing {
        Some(through) if lit => Press::Going(through),
        _ => Press::from(lit),
    };
    let pad = page.metric(Metric::RowPadding);
    let gap = page.metric(Metric::Gap);
    let name_line = page.line(Text::Body);
    let note_line = page.line(Text::Caption);
    let names = name_lines(page, rect[2], &toggle);
    let names_tall = name_line * names.len().max(1) as f32;
    let room_for = (((rect[3] - page.metric(Metric::RowHeight) - (names_tall - name_line))
        / note_line)
        .round()
        .max(0.0) as usize)
        + 1;
    let lines = note_lines(page, rect[2], &toggle, room_for.min(NOTE_LINES));
    let icons = page.icons();
    let scale = page.scaled(1.0);
    let ui = page.ui();
    if toggle.enabled {
        ui.spot(spot, rect);
    }
    let sunk = if toggle.enabled && !toggle.refused {
        ui.control(rect, radius, press, rect[2], 1.0)
    } else {
        ui.control_out(rect, radius, 1.0);
        rect
    };
    let [x, y, _, h] = sunk;
    let ink = if !toggle.enabled || toggle.refused {
        0.55
    } else if lit {
        1.0
    } else {
        0.86
    };
    let track = switch(ui, sunk, pad, toggle.on, ink, icons);
    let mut left = x + pad;
    if let Some(mark) = toggle.mark {
        let size = mark_size_at(scale);
        ui.icon_tinted(
            [left, y + (h - size) * 0.5, size, size],
            mark,
            icons,
            Role::Text,
            ink,
        );
        left += size + gap;
    }
    let room = (track[0] - gap - left).max(0.0);
    let top = y + (h - names_tall - note_line * lines.len() as f32) * 0.5;
    let (role, alpha) = if toggle.refused {
        text::REFUSAL
    } else {
        text::LESSER
    };
    for (index, name) in names.iter().enumerate() {
        text::label_faded(
            ui,
            [left, top + index as f32 * name_line, room, name_line],
            Text::Body,
            name,
            Role::Text,
            ink,
            Align::Left,
        );
    }
    let top = top + names_tall - name_line;
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
            role,
            alpha,
            Align::Left,
        );
    }
}

/// The switch at the right-hand end of a row `rect` (already sunk by its
/// press): the toolkit's (`Ui::switch`), a capsule of the panel's dark glass
/// with the accent's track in it when on and a knob at the end it is switched
/// to — each part a bead of the glossy material every mark wears, as the tick
/// does. `ink` is how strongly the row is drawn. Answers the track's
/// rectangle, so the words beside it stop short of it.
///
/// The dark capsule stands round the track, a little past it, where the flat
/// ring it replaces stood: without it a switch that was on, on a lit row, was
/// the light's violet on the light's violet with a white knob floating in it
/// — the same fault as the `chosen` mark's (see [`super::mark`]). It is
/// counted with the words (`Ui::words_between`), so a page coming in brings
/// its switches up with its words, not ahead of them in empty bars.
pub fn switch(
    ui: &mut Ui,
    rect: [f32; 4],
    pad: f32,
    on: bool,
    ink: f32,
    icons: IconStyle,
) -> [f32; 4] {
    let [x, y, w, h] = rect;
    // Sized by a row of the design language, never by a row grown to a
    // long note: a taller row keeps the same switch, in its middle.
    let track_h = h.min(ui.m(Metric::RowHeight)) * TRACK;
    let track_w = track_h * TRACK_LONG;
    let track = [
        x + w - pad - track_w,
        y + (h - track_h) * 0.5,
        track_w,
        track_h,
    ];
    let ring = track_h * RING;
    let from = ui.drawn();
    ui.switch(
        [
            track[0] - ring,
            track[1] - ring,
            track_w + ring * 2.0,
            track_h + ring * 2.0,
        ],
        on,
        icons,
        ink,
    );
    let to = ui.drawn();
    ui.words_between(from, to);
    track
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The toolkit's switch, as tall as the track and its ring, is the track
    /// and its ring long: the track stands where it stood, and the words
    /// beside it keep their room.
    #[test]
    fn the_toolkits_switch_stands_round_the_track() {
        let track_h = 30.0;
        let tall = track_h * (1.0 + RING * 2.0);
        let long = Ui::switch_width(tall);
        let expected = track_h * TRACK_LONG + track_h * RING * 2.0;
        assert!((long - expected).abs() < 0.2, "{long} for {expected}");
    }
}
