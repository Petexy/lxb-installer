//! Words cut to the room they are given.
//!
//! **Nothing clips a one-line label.** `lxb-render` lays a label out at the
//! width of its rectangle only where the rectangle can hold more than one
//! line, so every one-line name is shaped unbounded and drawn straight out
//! through whatever stands beside it — a disk model running through the size
//! next to it, a long city name through the column's edge. So it is cut here,
//! before it is handed over, by asking the measurer rather than by counting
//! letters: a Chinese character is a whole em where a Latin one is about two
//! thirds of one, and a count of letters is wrong in one language out of two.
//!
//! **A sentence is broken into lines here too, in every script.** The
//! toolkit's `Ui::wrap` breaks a line only at a space, and Chinese is
//! written without them: a whole sentence came back as one word, and was cut
//! to one line with an ellipsis however much room there was under it.
//! [`break_lines`] breaks where a line may break in each script — at a
//! space, and between any two characters of a script written without spaces
//! — but never before a closing mark nor after an opening one.

use lxb_app::lxb_render::{Align, Ui};
use lxb_app::lxb_toolkit::{palette::Role, typography::Text};

/// As much of `text` as fits in `room` on one line, cut out of the tail with
/// an ellipsis where it does not fit. Line breaks are flattened to spaces.
pub fn fit(ui: &mut Ui, style: Text, text: &str, room: f32) -> String {
    let flat = text.replace(['\n', '\r'], " ");
    cut_to(&flat, room, |candidate| ui.measure(style, candidate))
}

/// As much of a note made of parts (`Poland · Europe`) as fits in `room` on
/// one line: the whole; else its parts from the first, as many as fit — a
/// country that fills the line (Dalekie Wyspy Mniejsze Stanów
/// Zjednoczonych) is said whole and its part of the world left out, rather
/// than both cut short; and only where the first alone does not fit, that
/// cut with an ellipsis.
pub fn fit_parts(ui: &mut Ui, style: Text, text: &str, room: f32) -> String {
    let flat = text.replace(['\n', '\r'], " ");
    cut_parts(&flat, room, |candidate| ui.measure(style, candidate))
}

/// The arithmetic of [`fit_parts`].
pub fn cut_parts(text: &str, room: f32, mut measure: impl FnMut(&str) -> f32) -> String {
    let mut kept = text;
    while measure(kept) > room {
        match kept.rsplit_once(" · ") {
            Some((before, _)) => kept = before,
            None => break,
        }
    }
    cut_to(kept, room, measure)
}

/// How wide `text` is in `style` drawn in the bold face.
///
/// The measurer knows each style in its own face only — a label, a row's
/// name, in the regular one — and a primary button, a lit one or a menu's
/// focused row draws its words bold, which is wider: a button sized by the
/// regular face cut its own bold word with an ellipsis. The toolkit shapes
/// text at a size with no hinting, so a width grows exactly with the size,
/// and the Title, the one bold style, is measured and scaled to `style`'s
/// size.
pub fn bold_width(ui: &mut Ui, style: Text, text: &str) -> f32 {
    ui.measure(Text::Title, text) * style.size() / Text::Title.size()
}

/// [`fit`] for words drawn bold.
pub fn fit_bold(ui: &mut Ui, style: Text, text: &str, room: f32) -> String {
    let flat = text.replace(['\n', '\r'], " ");
    cut_to(&flat, room, |candidate| bold_width(ui, style, candidate))
}

/// The arithmetic of [`fit`], with the measurer handed in so it can be
/// tested without a renderer.
pub fn cut_to(text: &str, room: f32, mut measure: impl FnMut(&str) -> f32) -> String {
    if room <= 0.0 {
        return String::new();
    }
    if measure(text) <= room {
        return text.to_owned();
    }
    let letters: Vec<char> = text.chars().collect();
    let shortened = |keep: usize| {
        let mut cut: String = letters[..keep].iter().collect();
        while cut.ends_with(' ') {
            cut.pop();
        }
        cut.push('…');
        cut
    };
    // The longest prefix that fits, found by halving rather than by trying
    // every length: a label is measured on every frame it is drawn.
    let (mut least, mut most) = (0usize, letters.len());
    while least < most {
        let middle = least + (most - least).div_ceil(2);
        if measure(&shortened(middle)) <= room {
            least = middle;
        } else {
            most = middle - 1;
        }
    }
    if least == 0 {
        String::from("…")
    } else {
        shortened(least)
    }
}

/// The ink of every lesser line — the note under a name, a card's or a
/// disk's, a field's label, the caption under a title, a column's heading,
/// the legend's words, a note in the foot — wherever it stands and whether
/// the light is on it or not: the panel's white, a step quieter than the
/// line it serves.
///
/// Not the palette's soft lavender, which is cut for the page's own
/// ground: on the installer's violet glass it is hard to read, and under
/// the light very nearly the light's own colour. What sets a lesser line
/// apart is its size and weight — a caption under a line of body text,
/// regular under bold — not a fainter colour.
pub const LESSER: (Role, f32) = (Role::Text, 0.9);

/// The ink an empty field's hint is written in: the palette's soft ink, the
/// lavender [`LESSER`] leaves aside — barely there on the field's glass, lit
/// or not, so that what a person has typed, in the panel's white, is the
/// one thing in a field that reads as written.
///
/// The one lesser line kept out of [`LESSER`]'s white: a hint white with
/// every other lesser line would make an empty form read as one already
/// filled in. A hint is not a line to be read so much as a place still to
/// be written in, and the soft ink says so. Not the accent's own soft tint,
/// which is the light's colour: under the light the hint would be gone.
pub const HINT: (Role, f32) = (Role::TextSoft, 1.0);

/// The ink a refusal is written in: the panel's own white, at full
/// strength.
///
/// Red on the glass reads poorly — the toolkit draws its Danger darker than
/// the palette says, and on the violet glass a red line is barely there — so
/// what is refused is said in white. The outline round a control that cannot
/// be used, the Error sound and the words themselves say it is a refusal.
pub const REFUSAL: (Role, f32) = (Role::Text, 1.0);

/// The ink a note meant in `role` is drawn in: a refusal (`Role::Danger`)
/// in the full white of a refusal ([`REFUSAL`]), anything else as a lesser
/// line ([`LESSER`]).
pub fn note_ink(role: Role) -> (Role, f32) {
    if role == Role::Danger {
        REFUSAL
    } else {
        LESSER
    }
}

/// How large a field's heading is written against the caption it would
/// otherwise be: the shell's smallest words, its menus' stamps
/// (`CONTEXT_STAMP_SIZE`, 18 reference pixels against the caption's 22).
///
/// A field's heading is a little smaller than what is typed in it, so the
/// form's own words and the person's are told apart. At the caption's size, a
/// step under what is typed, they read as one size; a step under the caption
/// — which an empty field's hint keeps — the three are told apart at a
/// glance: the heading, the hint, what was typed.
pub const HEADING: f32 = 18.0 / 22.0;

/// [`label_faded`] written `scale` times its style's size: what is drawn at
/// the style's size is carried, scaled about its line's middle on the side
/// it is aligned to, so it stands in `rect` as a label of that size would.
/// The toolkit writes only its five sizes; this is how a size between them
/// is written (a field's heading, [`HEADING`]).
#[allow(clippy::too_many_arguments)]
pub fn label_scaled(
    ui: &mut Ui,
    rect: [f32; 4],
    style: Text,
    text: &str,
    role: Role,
    alpha: f32,
    align: Align,
    scale: f32,
) {
    if alpha <= 0.0 || scale <= 0.0 {
        return;
    }
    let (written, about) = written_for(rect, align, scale);
    let from = ui.drawn();
    label_faded(ui, written, style, text, role, alpha, align);
    let to = ui.drawn();
    ui.carry_between(
        from,
        to,
        lxb_app::lxb_render::Carry {
            scale,
            about,
            ..lxb_app::lxb_render::Carry::standing(rect)
        },
    );
}

/// Where [`label_scaled`] writes its words at the style's size, and the
/// point it carries them about: the rectangle that, scaled by `scale` about
/// that point — its line's middle, on the side it is aligned to — is `rect`.
fn written_for(rect: [f32; 4], align: Align, scale: f32) -> ([f32; 4], [f32; 2]) {
    let [x, y, w, h] = rect;
    let about = match align {
        Align::Left => [x, y + h * 0.5],
        Align::Centre => [x + w * 0.5, y + h * 0.5],
        Align::Right => [x + w, y + h * 0.5],
    };
    let (wide, tall) = (w / scale, h / scale);
    let left = match align {
        Align::Left => x,
        Align::Centre => about[0] - wide * 0.5,
        Align::Right => x + w - wide,
    };
    ([left, about[1] - tall * 0.5, wide, tall], about)
}

/// A one-line label cut to its rectangle and drawn in `role`.
pub fn label(ui: &mut Ui, rect: [f32; 4], style: Text, text: &str, role: Role, align: Align) {
    let cut = fit(ui, style, text, rect[2]);
    ui.label(rect, style, &cut, role, align);
}

/// The same at a strength: `alpha` multiplies the ink.
pub fn label_faded(
    ui: &mut Ui,
    rect: [f32; 4],
    style: Text,
    text: &str,
    role: Role,
    alpha: f32,
    align: Align,
) {
    if alpha <= 0.0 {
        return;
    }
    let cut = fit(ui, style, text, rect[2]);
    let tint = ui.tinted(role, alpha);
    ui.label_tinted(rect, style, &cut, tint, align);
}

/// Lines of `text` wrapped to `room`, at most `lines` of them, the last cut
/// with an ellipsis where more was left over.
pub fn wrapped(ui: &mut Ui, style: Text, text: &str, room: f32, lines: usize) -> Vec<String> {
    wrapped_by(text, room, lines, |candidate| ui.measure(style, candidate))
}

/// [`wrapped`], measured by `measure`: for words drawn in another face than
/// their style's own (bold, [`bold_width`]).
pub fn wrapped_by(
    text: &str,
    room: f32,
    lines: usize,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<String> {
    let mut all = broken(text, room, &mut measure);
    if all.len() > lines && lines > 0 {
        // What is left over, joined back as it was written: with a space
        // where the break was at one, and with none between two characters
        // of a script that has no spaces.
        let mut rest = String::new();
        for (line, spaced) in all.drain(lines - 1..) {
            if spaced && !rest.is_empty() {
                rest.push(' ');
            }
            rest.push_str(&line);
        }
        let mut last = cut_to(&rest, room, &mut measure);
        if !last.ends_with('…') {
            last.push('…');
        }
        all.push((last, false));
    }
    // A single word wider than the room comes back whole, so each line is cut
    // on its own as well.
    all.into_iter()
        .map(|(line, _)| cut_to(&line, room, &mut measure))
        .collect()
}

/// `text` broken into lines no wider than `room`, by `measure`: wherever a
/// line may break (see [`pieces`]), as many pieces to a line as fit, and a
/// piece wider than the room on a line of its own, uncut.
///
/// What the catalogs' test counts lines with, so what it counts is what is
/// drawn. `Ui::wrap` would break a Chinese sentence nowhere.
pub fn break_lines(text: &str, room: f32, mut measure: impl FnMut(&str) -> f32) -> Vec<String> {
    broken(text, room, &mut measure)
        .into_iter()
        .map(|(line, _)| line)
        .collect()
}

/// `words` held together on one line: its spaces made no-break spaces,
/// which a line is not broken at — for a name, "Example NVMe SSD · 512 GB",
/// which broken across two lines reads as two things. [`break_lines`]
/// breaks one too wide for a whole line at them all the same, by the rules
/// it breaks at a space by.
pub fn unbroken(words: &str) -> String {
    words.replace(' ', "\u{a0}")
}

/// Whether `letter` is a space a line is not broken at.
fn holds(letter: char) -> bool {
    matches!(letter, '\u{a0}' | '\u{2007}' | '\u{202f}')
}

/// [`break_lines`], each line with whether it began after a space.
fn broken(text: &str, room: f32, measure: &mut impl FnMut(&str) -> f32) -> Vec<(String, bool)> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_spaced = false;
    // A piece held together by no-break spaces that is wider than a whole
    // line is broken at them after all, rather than drawn past the edge —
    // as at a space, so its "·" still never begins a line and its size is
    // still never parted from its unit.
    // So is a word joined by hyphens that is wider than a whole line
    // (Петропавловск-Камчатский, a place's name on a display standing on
    // its side): after a hyphen, as such a word is broken in print. Only
    // then — a hyphenated word that fits is never parted.
    let mut all = Vec::new();
    for (piece, spaced) in pieces(text) {
        if piece.contains(holds) && measure(piece) > room {
            for (at, (part, part_spaced)) in cut(piece, true).into_iter().enumerate() {
                all.push((part, if at == 0 { spaced } else { part_spaced }));
            }
        } else if hyphened(piece) && measure(piece) > room {
            for (at, part) in after_hyphens(piece).into_iter().enumerate() {
                all.push((part, at == 0 && spaced));
            }
        } else {
            all.push((piece, spaced));
        }
    }
    for (piece, spaced) in all {
        let mut candidate = line.clone();
        if spaced && !line.is_empty() {
            candidate.push(' ');
        }
        candidate.push_str(piece);
        if !line.is_empty() && measure(&candidate) > room {
            lines.push((std::mem::take(&mut line), line_spaced));
            line = piece.to_string();
            line_spaced = spaced;
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push((line, line_spaced));
    }
    lines
}

/// Whether `piece` holds a hyphen between two letters.
fn hyphened(piece: &str) -> bool {
    piece.contains('-') && after_hyphens(piece).len() > 1
}

/// `piece` cut after each hyphen that stands between two letters, the
/// hyphen kept with what comes before it.
fn after_hyphens(piece: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut from = 0;
    let mut letters = piece.char_indices().peekable();
    let mut before: Option<char> = None;
    while let Some((at, letter)) = letters.next() {
        let next = letters.peek().map(|(_, next)| *next);
        if letter == '-'
            && before.is_some_and(char::is_alphabetic)
            && next.is_some_and(char::is_alphabetic)
        {
            parts.push(&piece[from..at + 1]);
            from = at + 1;
        }
        before = Some(letter);
    }
    parts.push(&piece[from..]);
    parts
}

/// `text` cut where a line may break, each piece with whether a space stood
/// before it.
///
/// A line breaks at a space, as in every language with spaces; and between
/// two characters of Chinese or Japanese, or where one of those meets a
/// letter, as those scripts are broken — except before a mark that closes
/// (a full stop, a comma, a closing bracket or quote: "。" never starts a
/// line) and after one that opens. The same holds across a space: a mark
/// standing alone that closes ("»", "·", a French "?") stays at the end of
/// the line before, one that opens ("«", "„", "¿") goes down with what it
/// opens, and so does a number with the word after it — "400 GB" is never
/// broken between its two halves. Spaces themselves belong to no piece; a
/// no-break space is no break, and belongs to the piece it stands in.
pub fn pieces(text: &str) -> Vec<(&str, bool)> {
    cut(text, false)
}

/// [`pieces`], and with `through_holds` cut at no-break spaces as well, as
/// if they were spaces: how a name held together that is wider than a whole
/// line is broken after all.
fn cut(text: &str, through_holds: bool) -> Vec<(&str, bool)> {
    // Where each piece begins and ends in `text`, and whether a space stood
    // before it.
    let mut found: Vec<(usize, usize, bool)> = Vec::new();
    let mut start: Option<usize> = None;
    let mut spaced = false;
    let mut last: Option<char> = None;
    for (at, letter) in text.char_indices() {
        if letter.is_whitespace() && (through_holds || !holds(letter)) {
            if let Some(from) = start.take() {
                found.push((from, at, spaced));
            }
            spaced = true;
            last = None;
            continue;
        }
        let breaks_here = match last {
            None => false,
            Some(before) => {
                (unspaced(before) || unspaced(letter)) && !closes(letter) && !opens(before)
            }
        };
        if breaks_here {
            if let Some(from) = start.take() {
                found.push((from, at, spaced));
                spaced = false;
            }
        }
        if start.is_none() {
            start = Some(at);
        }
        last = Some(letter);
    }
    if let Some(from) = start {
        found.push((from, text.len(), spaced));
    }
    // A piece that opens, or a number, joins the piece after it; one that
    // closes joins the piece before it. A piece's text is a stretch of
    // `text`, so two joined are the stretch from the one's start to the
    // other's end, the space between them included.
    let mut forward: Vec<(usize, usize, bool)> = Vec::with_capacity(found.len());
    let mut held: Option<(usize, bool)> = None;
    for (from, to, spaced) in found {
        let (from, spaced) = held.take().unwrap_or((from, spaced));
        let piece = &text[from..to];
        if piece.chars().all(opens) || is_number(piece) {
            held = Some((from, spaced));
            continue;
        }
        forward.push((from, to, spaced));
    }
    if let Some((from, spaced)) = held {
        forward.push((from, text.len(), spaced));
    }
    let mut joined: Vec<(usize, usize, bool)> = Vec::with_capacity(forward.len());
    for (from, to, spaced) in forward {
        match joined.last_mut() {
            Some(before) if text[from..to].chars().all(closes) => before.1 = to,
            _ => joined.push((from, to, spaced)),
        }
    }
    joined
        .into_iter()
        .map(|(from, to, spaced)| (&text[from..to], spaced))
        .collect()
}

/// Whether `piece` is a number: figures, with a decimal mark or a
/// thousands mark among them.
fn is_number(piece: &str) -> bool {
    piece.chars().any(|letter| letter.is_ascii_digit())
        && piece
            .chars()
            .all(|letter| letter.is_ascii_digit() || letter == '.' || letter == ',')
}

/// Whether `letter` belongs to a script written without spaces between its
/// words, where a line may break between any two characters: Chinese,
/// Japanese, and their punctuation and full-width forms.
fn unspaced(letter: char) -> bool {
    matches!(letter,
        '\u{2E80}'..='\u{2FDF}'      // radicals
        | '\u{3000}'..='\u{30FF}'    // the CJK marks, hiragana, katakana
        | '\u{3100}'..='\u{31FF}'    // bopomofo, katakana extensions
        | '\u{3400}'..='\u{4DBF}'    // ideographs, extension A
        | '\u{4E00}'..='\u{9FFF}'    // ideographs
        | '\u{F900}'..='\u{FAFF}'    // compatibility ideographs
        | '\u{FE30}'..='\u{FE4F}'    // compatibility forms
        | '\u{FF00}'..='\u{FFEF}'    // full-width forms
        | '\u{20000}'..='\u{3FFFF}') // ideographs, the later extensions
}

/// Whether `letter` closes something, and so never begins a line.
fn closes(letter: char) -> bool {
    CLOSING.contains(letter)
}

/// Whether `letter` opens something, and so never ends a line.
fn opens(letter: char) -> bool {
    OPENING.contains(letter)
}

/// The marks that close: stops, commas, closing brackets and quotes, in
/// full width and in Latin, and the dot and the dashes that stand between
/// two things.
const CLOSING: &str = "。，、；：！？）」』】》〉〕］｝％…・”’.,;:!?)]}%»·—–";

/// The marks that open: brackets and quotes, and Spanish's inverted marks.
const OPENING: &str = "（「『【《〈〔［｛“‘([{«„¿¡";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fields_heading_is_smaller_than_its_hint_and_what_is_typed() {
        let heading = Text::Caption.size() * HEADING;
        assert!(heading < Text::Caption.size(), "the hint is a caption");
        assert!(
            Text::Caption.size() < Text::Body.size(),
            "what is typed is body"
        );
        assert!((heading - 18.0).abs() < 1e-4, "the shell's smallest words");
    }

    #[test]
    fn words_written_scaled_stand_in_their_rectangle() {
        let rect = [100.0, 40.0, 300.0, 18.0];
        for align in [Align::Left, Align::Centre, Align::Right] {
            let ([x, y, w, h], [ax, ay]) = written_for(rect, align, HEADING);
            let carried = [
                ax + (x - ax) * HEADING,
                ay + (y - ay) * HEADING,
                w * HEADING,
                h * HEADING,
            ];
            for (got, want) in carried.iter().zip(rect) {
                assert!((got - want).abs() < 1e-3, "{carried:?} is not {rect:?}");
            }
        }
    }

    #[test]
    fn nothing_is_written_in_red() {
        assert_eq!(note_ink(Role::Danger), REFUSAL);
        assert_eq!(REFUSAL.0, Role::Text);
    }

    #[test]
    fn a_lesser_line_is_white_and_a_little_quieter_than_its_name() {
        let (role, alpha) = LESSER;
        assert_eq!(role, Role::Text, "a lesser line in the soft lavender");
        assert!((0.85..1.0).contains(&alpha), "{alpha}");
        assert_eq!(note_ink(Role::Text), LESSER);
    }

    /// An empty field's hint is the one lesser line in the soft ink, never
    /// the white of what is typed.
    #[test]
    fn a_fields_hint_is_in_the_soft_ink_and_not_in_the_white_of_typing() {
        assert_eq!(HINT.0, Role::TextSoft);
        assert_ne!(
            HINT.0, LESSER.0,
            "the hint went white with the lesser lines"
        );
        assert!(HINT.1 > 0.5, "a hint too faint to read: {}", HINT.1);
    }

    #[test]
    fn a_name_held_together_goes_down_whole_and_one_too_wide_still_breaks() {
        let name = unbroken("Example NVMe SSD · 512 GB");
        let sentence = format!("Everything on {name} will be erased.");
        // Room for the name on a line, and for "Everything on Example"
        // before it, but not for the whole name there.
        let lines = break_lines(&sentence, 260.0, even);
        assert_eq!(lines[0], "Everything on", "{lines:?}");
        assert_eq!(lines[1], name, "{lines:?}");
        // A name wider than a whole line breaks at its own spaces.
        let lines = break_lines(&sentence, 120.0, even);
        assert!(lines.iter().all(|line| even(line) <= 120.0), "{lines:?}");
        assert!(lines.iter().all(|line| !line.starts_with('·')), "{lines:?}");
    }

    #[test]
    fn a_name_too_wide_for_a_line_still_breaks_as_a_sentence_does() {
        // At every width, whether the name goes down whole or is broken at
        // its own spaces: its "·" never begins a line and its size is never
        // parted from its unit. (At 160 its pieces were once cut at every
        // no-break space, and a line began "· 512 GB".)
        let name = unbroken("Example NVMe SSD · 512 GB");
        let sentence = format!("Everything on {name} will be erased.");
        let words = |line: &str| line.replace('\u{a0}', " ");
        for room in (60..=400).step_by(10) {
            let lines = break_lines(&sentence, room as f32, even);
            for line in &lines {
                assert!(!line.starts_with('·'), "at {room}: {lines:?}");
                assert!(
                    !line.starts_with("GB"),
                    "at {room} 512 GB was parted: {lines:?}"
                );
            }
            let joined: Vec<String> = lines.iter().map(|line| words(line)).collect();
            assert_eq!(joined.join(" "), words(&sentence), "at {room}");
        }
    }

    /// Ten points a letter, the ellipsis included.
    fn even(text: &str) -> f32 {
        text.chars().count() as f32 * 10.0
    }

    #[test]
    fn what_fits_is_left_alone() {
        assert_eq!(cut_to("Warsaw", 60.0, even), "Warsaw");
    }

    #[test]
    fn what_does_not_fit_is_cut_from_the_tail_with_an_ellipsis() {
        let cut = cut_to("Samsung SSD 990 PRO", 80.0, even);
        assert_eq!(cut, "Samsung…");
        assert!(even(&cut) <= 80.0);
    }

    #[test]
    fn a_note_of_parts_loses_its_last_parts_before_it_is_cut() {
        assert_eq!(cut_parts("Poland · Europe", 160.0, even), "Poland · Europe");
        assert_eq!(cut_parts("Poland · Europe", 100.0, even), "Poland");
        assert_eq!(cut_parts("a · b · c", 50.0, even), "a · b");
        assert_eq!(cut_parts("Mozambique · Africa", 60.0, even), "Mozam…");
    }

    #[test]
    fn a_cut_never_ends_in_a_space_before_the_ellipsis() {
        assert_eq!(cut_to("ab cdefgh", 40.0, even), "ab…");
    }

    /// A word joined by hyphens is parted after a hyphen only where it is
    /// wider than a whole line; one that fits goes down whole.
    #[test]
    fn a_hyphenated_word_breaks_after_a_hyphen_only_where_it_must() {
        let name = "Петропавловск-Камчатский";
        assert_eq!(break_lines(name, 250.0, even), [name]);
        assert_eq!(
            break_lines(name, 150.0, even),
            ["Петропавловск-", "Камчатский"]
        );
        assert_eq!(
            break_lines("Бюзинген-на-Верхнем-Рейне", 130.0, even),
            ["Бюзинген-на-", "Верхнем-Рейне"]
        );
        // Room for "Lower" but not "Prince-Quarter" beside it: the
        // hyphenated word goes down whole, as it fits on a line of its own.
        assert_eq!(
            break_lines("Lower Prince-Quarter", 150.0, even),
            ["Lower", "Prince-Quarter"]
        );
        // A hyphen that does not stand between two letters is no break.
        assert_eq!(break_lines("-5 to 10", 30.0, even), ["-5", "to", "10"]);
        assert_eq!(after_hyphens("a-b-"), ["a-", "b-"]);
    }

    #[test]
    fn a_sentence_breaks_at_its_spaces() {
        let lines = break_lines("one two three four", 90.0, even);
        assert_eq!(lines, ["one two", "three", "four"]);
    }

    #[test]
    fn chinese_breaks_between_its_characters_and_never_before_a_full_stop() {
        // Nine characters to a line: the sentence has no space anywhere, and
        // came back from the toolkit as one word, cut to one line.
        let said = "磁盘的部分内容可能已被清除。请重新完成各个步骤，再次选择磁盘。";
        let lines = break_lines(said, 90.0, even);
        assert!(lines.len() > 2, "{lines:?}");
        for line in &lines {
            assert!(even(line) <= 90.0, "{line} is wider than its room");
            assert!(!line.starts_with(['。', '，']), "{line} starts with a stop");
        }
        assert_eq!(lines.concat(), said, "a letter was lost or added");
    }

    #[test]
    fn a_latin_word_inside_chinese_stays_whole_and_keeps_its_spaces() {
        let said = "要立即安装 Linexin 吗？";
        let pieces: Vec<&str> = pieces(said).into_iter().map(|(piece, _)| piece).collect();
        assert_eq!(pieces, ["要", "立", "即", "安", "装", "Linexin", "吗？"]);
        let lines = break_lines(said, 100.0, even);
        assert_eq!(lines, ["要立即安装", "Linexin 吗？"]);
    }

    #[test]
    fn a_size_is_never_broken_between_its_figure_and_its_unit() {
        let said = "Remplacer « Windows » · 400 Go";
        let pieces: Vec<&str> = pieces(said).into_iter().map(|(piece, _)| piece).collect();
        assert_eq!(pieces, ["Remplacer", "« Windows » ·", "400 Go"]);
        let lines = break_lines(said, 250.0, even);
        assert_eq!(lines, ["Remplacer « Windows » ·", "400 Go"]);
    }

    #[test]
    fn an_opening_bracket_goes_down_with_what_it_opens() {
        let pieces: Vec<&str> = pieces("选择「磁盘」。")
            .into_iter()
            .map(|(piece, _)| piece)
            .collect();
        assert_eq!(pieces, ["选", "择", "「磁", "盘」。"]);
    }

    #[test]
    fn no_room_at_all_is_nothing_and_a_sliver_is_an_ellipsis() {
        assert_eq!(cut_to("anything", 0.0, even), "");
        assert_eq!(cut_to("anything", 12.0, even), "…");
    }
}
