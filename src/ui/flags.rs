//! The flags the Language step's rows wear: each language its country's, at
//! the left of its row and as tall as the row's two lines — its own name and
//! the English one under it — together, the way the search field's magnifier
//! spans its heading and what is typed in it.
//!
//! The legacy installer wrote the flag as an emoji before the name
//! (`language_widget.py`, `country_code_to_emoji`), which GNOME drew from
//! Noto Color Emoji. These are the same flags: the font's waved flags, cut
//! out of it by `scripts/flags-from-noto.py` into `data/flags/` (under the
//! SIL Open Font License 1.1, which is carried there) and carried in the
//! program. They are not drawn as letters of that font: the toolkit's
//! renderer does draw a colour emoji from it where the machine has the font,
//! but at full strength whatever its words are faded to — a colour glyph
//! does not take its run's alpha — so a flag stood whole on a page still
//! coming in, its row's words not yet there. A picture is faded with the
//! words (the carry counts the toolkit's pictures as words), takes a row's
//! ink, and needs no font on the machine.
//!
//! The toolkit draws a picture from a file, as the desktops' are
//! ([`super::pictures`]): each flag is written once per run, the first time
//! it is shown, into the same private directory, and handed over by its path
//! from then on. A picture is decoded on the toolkit's own threads, so a flag
//! shown for the first time comes a frame or two after its row; the column
//! asks for those frames itself, for a while, since nothing else would draw
//! one on a page standing still.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::OnceLock;

use lxb_app::lxb_render::{Fit, Ui};
use lxb_app::lxb_toolkit::typography::Text;

/// The flag of a language whose locale names no country: the white flag the
/// legacy installer wrote for one.
pub const WHITE: &str = "white";

/// How many flags are carried: one for each of the 56 countries the Language
/// step's locales name, and the white flag.
const COUNT: usize = 57;

/// Every flag carried, by its country's ISO 3166 code, with its picture.
const FLAGS: [(&str, &[u8]); COUNT] = [
    ("AL", include_bytes!("../../data/flags/al.png")),
    ("AU", include_bytes!("../../data/flags/au.png")),
    ("BA", include_bytes!("../../data/flags/ba.png")),
    ("BG", include_bytes!("../../data/flags/bg.png")),
    ("BR", include_bytes!("../../data/flags/br.png")),
    ("BY", include_bytes!("../../data/flags/by.png")),
    ("CA", include_bytes!("../../data/flags/ca.png")),
    ("CN", include_bytes!("../../data/flags/cn.png")),
    ("CZ", include_bytes!("../../data/flags/cz.png")),
    ("DE", include_bytes!("../../data/flags/de.png")),
    ("DK", include_bytes!("../../data/flags/dk.png")),
    ("EE", include_bytes!("../../data/flags/ee.png")),
    ("ES", include_bytes!("../../data/flags/es.png")),
    ("FI", include_bytes!("../../data/flags/fi.png")),
    ("FR", include_bytes!("../../data/flags/fr.png")),
    ("GB", include_bytes!("../../data/flags/gb.png")),
    ("GE", include_bytes!("../../data/flags/ge.png")),
    ("GR", include_bytes!("../../data/flags/gr.png")),
    ("HR", include_bytes!("../../data/flags/hr.png")),
    ("HU", include_bytes!("../../data/flags/hu.png")),
    ("ID", include_bytes!("../../data/flags/id.png")),
    ("IE", include_bytes!("../../data/flags/ie.png")),
    ("IL", include_bytes!("../../data/flags/il.png")),
    ("IN", include_bytes!("../../data/flags/in.png")),
    ("IR", include_bytes!("../../data/flags/ir.png")),
    ("IS", include_bytes!("../../data/flags/is.png")),
    ("IT", include_bytes!("../../data/flags/it.png")),
    ("JP", include_bytes!("../../data/flags/jp.png")),
    ("KH", include_bytes!("../../data/flags/kh.png")),
    ("KR", include_bytes!("../../data/flags/kr.png")),
    ("LA", include_bytes!("../../data/flags/la.png")),
    ("LT", include_bytes!("../../data/flags/lt.png")),
    ("LV", include_bytes!("../../data/flags/lv.png")),
    ("MN", include_bytes!("../../data/flags/mn.png")),
    ("MY", include_bytes!("../../data/flags/my.png")),
    ("NL", include_bytes!("../../data/flags/nl.png")),
    ("NO", include_bytes!("../../data/flags/no.png")),
    ("NZ", include_bytes!("../../data/flags/nz.png")),
    ("PH", include_bytes!("../../data/flags/ph.png")),
    ("PL", include_bytes!("../../data/flags/pl.png")),
    ("PT", include_bytes!("../../data/flags/pt.png")),
    ("RO", include_bytes!("../../data/flags/ro.png")),
    ("RS", include_bytes!("../../data/flags/rs.png")),
    ("RU", include_bytes!("../../data/flags/ru.png")),
    ("SA", include_bytes!("../../data/flags/sa.png")),
    ("SE", include_bytes!("../../data/flags/se.png")),
    ("SI", include_bytes!("../../data/flags/si.png")),
    ("SK", include_bytes!("../../data/flags/sk.png")),
    ("SO", include_bytes!("../../data/flags/so.png")),
    ("TH", include_bytes!("../../data/flags/th.png")),
    ("TR", include_bytes!("../../data/flags/tr.png")),
    ("TW", include_bytes!("../../data/flags/tw.png")),
    ("UA", include_bytes!("../../data/flags/ua.png")),
    ("US", include_bytes!("../../data/flags/us.png")),
    ("VN", include_bytes!("../../data/flags/vn.png")),
    ("ZA", include_bytes!("../../data/flags/za.png")),
    (WHITE, include_bytes!("../../data/flags/white.png")),
];

/// How the box a flag is drawn in is shaped, its width over its height: the
/// font's waved flag as it is cut, 126 by 95. The white flag, on its pole, is
/// drawn as tall in it and narrower.
pub const ASPECT: f32 = 126.0 / 95.0;

/// The flag `country` wears: its own where one is carried, else the white
/// flag.
pub fn of(country: Option<&str>) -> &'static str {
    country
        .and_then(|code| FLAGS.iter().find(|(name, _)| *name == code))
        .map_or(WHITE, |(name, _)| name)
}

/// Where `flag` is among those carried.
fn place(flag: &str) -> Option<usize> {
    FLAGS.iter().position(|(name, _)| *name == flag)
}

/// How tall a flag is at the design language's `scale`: a column's row's two
/// lines together, a name in Body and the line under it in Caption
/// (`Page::line` of each).
pub fn height_at(scale: f32) -> f32 {
    (Text::Body.size() + Text::Caption.size()) * Text::LINE * scale
}

/// How wide the box a flag is drawn in is at `scale`.
pub fn width_at(scale: f32) -> f32 {
    height_at(scale) * ASPECT
}

/// The file `flag`'s picture is drawn from, written the first time it is
/// asked for; `None` for a flag not carried, or where it could not be
/// written.
fn path(flag: &str) -> Option<PathBuf> {
    static WRITTEN: [OnceLock<Option<PathBuf>>; COUNT] = [const { OnceLock::new() }; COUNT];
    let at = place(flag)?;
    WRITTEN[at]
        .get_or_init(|| {
            let (name, bytes) = FLAGS[at];
            let directory = super::pictures::directory()?.join("flags");
            super::pictures::write_once(&directory, &format!("{}.png", name.to_lowercase()), bytes)
        })
        .clone()
}

/// How many frames each flag has been asked for without being there yet.
static MISSED: [AtomicU16; COUNT] = [const { AtomicU16::new(0) }; COUNT];

/// How many frames a flag not there yet asks another one for: two seconds'
/// worth. A picture that never comes (a file that could not be read back)
/// stops asking then, rather than keeping the page drawing for nothing.
const PATIENCE: u16 = 120;

/// Draw `flag` in `rect` at `alpha`: as tall as the box and in its middle,
/// as wide as its own shape makes it. Answers whether it is still on its way
/// — being decoded, the first time it is shown — and so wants another frame
/// soon.
pub fn draw(ui: &mut Ui, rect: [f32; 4], flag: &str, alpha: f32) -> bool {
    let (Some(at), Some(path)) = (place(flag), path(flag)) else {
        return false;
    };
    if ui.picture(rect, 0.0, &path, Fit::Contain, alpha) {
        MISSED[at].store(0, Ordering::Relaxed);
        return false;
    }
    missed(at)
}

/// Count a frame the flag at `at` was asked for and was not there; answers
/// whether it is still worth another frame soon.
fn missed(at: usize) -> bool {
    let before = MISSED[at]
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
            Some(count.saturating_add(1))
        })
        .unwrap_or(u16::MAX);
    before < PATIENCE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::catalogs;
    use lxb_app::lxb_toolkit::metrics::{scale_for, Metric};

    /// A PNG's size, read from its header.
    fn size(png: &[u8]) -> (u32, u32) {
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n") && &png[12..16] == b"IHDR");
        let word = |at: usize| u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
        (word(16), word(20))
    }

    /// Each language wears its own country's flag, as the legacy installer
    /// wrote it (`country_code_to_emoji` of the locale's country): never the
    /// white flag, which only a locale naming no country wears.
    #[test]
    fn every_language_wears_its_own_countrys_flag() {
        for language in catalogs::languages() {
            let country = language.country().expect(language.locale);
            assert_eq!(of(language.country()), country, "{}", language.locale);
        }
        assert_eq!(of(Some("PL")), "PL");
        assert_eq!(of(None), WHITE);
        assert_eq!(of(Some("ZZ")), WHITE, "a country with no flag carried");
        assert_eq!(of(Some("pl")), WHITE, "a code is written in capitals");
    }

    /// Every flag carried is worn by a language (or is the white flag), and
    /// is a picture of the font's waved flag as it is cut: no flag carried
    /// for nothing, none stretched.
    #[test]
    fn every_flag_carried_is_worn_and_is_a_waved_flag() {
        let worn: std::collections::BTreeSet<&str> = catalogs::languages()
            .iter()
            .filter_map(|language| language.country())
            .collect();
        for (name, png) in FLAGS {
            let (width, height) = size(png);
            if name == WHITE {
                assert!(width > 0 && height > 0);
                continue;
            }
            assert!(worn.contains(name), "{name} is worn by no language");
            assert_eq!(name.len(), 2, "{name}");
            assert_eq!((width, height), (126, 95), "{name}");
            assert_eq!(width as f32 / height as f32, ASPECT, "{name}");
        }
        assert_eq!(worn.len() + 1, FLAGS.len());
    }

    /// A flag not there yet asks for frames for a while, and then no more:
    /// a picture that never comes does not keep the page drawing.
    #[test]
    fn a_flag_on_its_way_asks_for_frames_and_then_gives_up() {
        let at = place(WHITE).unwrap();
        MISSED[at].store(0, Ordering::Relaxed);
        for _ in 0..PATIENCE {
            assert!(missed(at));
        }
        assert!(!missed(at), "still asking after {PATIENCE} frames");
        MISSED[at].store(u16::MAX, Ordering::Relaxed);
        assert!(!missed(at), "counting past the most began again");
        MISSED[at].store(0, Ordering::Relaxed);
    }

    /// A flag is as tall as a row's two lines together — its name and the
    /// line under it, as `Page::line` measures each at any height — and so
    /// stands inside the row with room above and below, as the search
    /// field's magnifier stands inside the field.
    #[test]
    fn a_flag_is_as_tall_as_a_rows_two_lines() {
        for height in [800.0, 1080.0, 1280.0, 1920.0, 2160.0] {
            let scale = scale_for(height);
            let lines = Text::Body.on(height) * Text::LINE + Text::Caption.on(height) * Text::LINE;
            assert!((height_at(scale) - lines).abs() < 0.01, "{height}");
            assert!(height_at(scale) < Metric::RowHeight.on(height), "{height}");
            assert!((width_at(scale) - lines * ASPECT).abs() < 0.01, "{height}");
        }
    }
}
