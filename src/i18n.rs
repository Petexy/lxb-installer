//! What the installer says, in the language the person chose.
//!
//! The words themselves are in `locales/`, one Fluent catalog per language,
//! compiled into the binary; the machinery that reads them is the toolkit's,
//! so the installer, the dialogs it raises and the shell it may be running
//! under all speak from catalogs of the same shape. See
//! `lxb_app::lxb_toolkit::i18n` for how a language is matched to a catalog.
//!
//! **One difference from the rest of the family: the language can change
//! while the installer runs.** Every other application keeps the language it
//! was started in, which is what every program on a Linux desktop does. An
//! installer is the exception the legacy installer already made: the first
//! thing a person does is say which language they read, and the screens after
//! that should be in it. So there is a process-wide current language, set by
//! [`set_language`] when the person picks a system language one of the ten
//! catalogs speaks, and [`text`] and [`format`] read through it with the
//! catalog's `text_for`/`format_for`. The toolkit's own words (its file
//! chooser, which the installer never raises) stay in the session's language.
//!
//! Anything a person, a device or a file gave its name to is passed as a
//! *value* and never as an identifier: a message id is this program's own
//! writing, and a disk whose model is `{ $size }` is still a disk.
use std::sync::{LazyLock, RwLock};

pub use lxb_app::lxb_toolkit::i18n::{Catalog, FluentArgs};

/// Every language the installer is translated into. A new one is a file
/// beside `en-GB.ftl` and a line here; one that is missing a message falls
/// back to English on its own.
pub const RESOURCES: &[(&str, &str)] = &[
    ("en-GB", include_str!("../locales/en-GB.ftl")),
    // An overlay of what America writes differently, not a second catalog —
    // see `Catalog::validate`, which holds it to that.
    ("en-US", include_str!("../locales/en-US.ftl")),
    ("de", include_str!("../locales/de.ftl")),
    ("es", include_str!("../locales/es.ftl")),
    ("fr", include_str!("../locales/fr.ftl")),
    ("hi", include_str!("../locales/hi.ftl")),
    ("pl", include_str!("../locales/pl.ftl")),
    ("pt-BR", include_str!("../locales/pt-BR.ftl")),
    ("ru", include_str!("../locales/ru.ftl")),
    ("zh-CN", include_str!("../locales/zh-CN.ftl")),
];

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| Catalog::new(RESOURCES));

/// The language chosen inside the installer, or `None` for the session's.
///
/// A lock rather than a plain static because the frame loop may not stay the
/// only reader: a log line or a worker thread written in the person's
/// language would otherwise be a data race the compiler could not see.
static CHOSEN: RwLock<Option<&'static str>> = RwLock::new(None);

#[cfg(test)]
thread_local! {
    /// A test's own language for its thread ([`speak_here`]), read before
    /// the process's: what a test draws the interface's words in, in the
    /// language it names, without switching the language every other test
    /// reads.
    static HERE: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

/// For a test: every word read on this thread — and only on it — is read in
/// the catalog `locale` matches, until what this answers is dropped. Tests
/// run side by side, each on a thread of its own, so this changes nothing
/// another test reads, as [`set_language`] would.
#[cfg(test)]
pub fn speak_here(locale: &str) -> SpokenHere {
    let tag = lxb_app::lxb_toolkit::i18n::language_from(locale);
    SpokenHere(HERE.with(|here| here.replace(Some(tag))))
}

/// A test's own language on its thread, given back as it was when dropped.
#[cfg(test)]
pub struct SpokenHere(Option<&'static str>);

#[cfg(test)]
impl Drop for SpokenHere {
    fn drop(&mut self) {
        HERE.with(|here| here.set(self.0));
    }
}

/// The catalog tag every word is read in now: the one the person chose, or
/// the session's until they choose.
pub fn language() -> &'static str {
    #[cfg(test)]
    if let Some(tag) = HERE.with(std::cell::Cell::get) {
        return tag;
    }
    CHOSEN
        .read()
        .ok()
        .and_then(|chosen| *chosen)
        .unwrap_or_else(lxb_app::lxb_toolkit::i18n::language)
}

/// Speak the catalog `locale` matches from now on: a catalog tag (`pl`,
/// `pt-BR`) or a whole locale (`pl_PL.UTF-8`). Answers the tag it settled on.
///
/// Called only for a system language one of the catalogs speaks; a locale no
/// catalog speaks would land on English, which is why the language step asks
/// first rather than calling this for every row (the legacy installer kept
/// the previous language in that case, and so does this one).
pub fn set_language(locale: &str) -> &'static str {
    let tag = lxb_app::lxb_toolkit::i18n::language_from(locale);
    if let Ok(mut chosen) = CHOSEN.write() {
        *chosen = Some(tag);
    }
    tag
}

/// A label with nothing in it, in the current language.
pub fn text(id: &'static str) -> &'static str {
    text_in(language(), id)
}

/// The same, in a language named rather than the current one. What the
/// language step can use to show a row in the language it would switch to.
pub fn text_in(locale: &str, id: &'static str) -> &'static str {
    CATALOG.text_for(locale, id)
}

/// A sentence with values in it. Reached through [`crate::message`].
pub fn format(id: &str, args: &FluentArgs<'_>) -> String {
    CATALOG.format_for(language(), id, args)
}

/// The same, in a language named rather than the current one: what a test
/// reads every catalog's sentences through without switching the process's
/// language, which every other test reads.
pub fn format_in(locale: &str, id: &str, args: &FluentArgs<'_>) -> String {
    CATALOG.format_for(locale, id, args)
}

/// A quantity the installer has written out, with the decimal mark the
/// current language uses. The toolkit's own `decimal` follows the session's
/// language, which is not necessarily the one this program is speaking.
pub fn decimal(number: String) -> String {
    // German, Spanish, French, Polish, Portuguese and Russian write the
    // fraction after a comma; the two Englishes, Hindi and Chinese after a
    // full stop.
    if matches!(language(), "en-GB" | "en-US" | "hi" | "zh-CN") {
        number
    } else {
        number.replace('.', ",")
    }
}

/// A sentence with values in it.
///
/// A count is passed as a number — `"count" => parts` — never as text: a
/// language picks the form of the noun by looking at the number, and Fluent
/// handed a string falls silently to the form nothing else uses.
#[macro_export]
macro_rules! message {
    ($id:literal $(, $name:literal => $value:expr)* $(,)?) => {{
        let mut args = $crate::i18n::FluentArgs::new();
        $(args.set($name, $value);)*
        $crate::i18n::format($id, &args)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every language carries the same messages, with the same variables in
    /// them, and every one of them formats. An overlay is held to the
    /// opposite rule: everything in it must be a message English has, take
    /// the same variables, and actually say something else.
    #[test]
    fn the_catalogs_carry_the_same_messages() {
        Catalog::validate(RESOURCES);
    }

    /// And every message this program asks for is one they have — the
    /// failure that would otherwise be drawn on screen as the identifier
    /// itself, in every language including English.
    #[test]
    fn every_message_this_program_asks_for_is_one_the_catalogs_have() {
        Catalog::check_references(env!("CARGO_MANIFEST_DIR"), RESOURCES);
    }

    /// And every word of them can be drawn by a face the toolkit ships.
    ///
    /// A live ISO has few fonts of its own, so a Hindi or Chinese label the
    /// shipped faces cannot draw would come out as a row of empty boxes on
    /// exactly the machine the installer runs on.
    #[test]
    fn every_word_the_catalogs_carry_can_be_drawn() {
        lxb_app::lxb_render::faces::check_drawable(RESOURCES);
    }

    /// A test's own language holds on its thread alone, and is given back
    /// as it was once let go of.
    #[test]
    fn a_tests_own_language_holds_on_its_thread_alone() {
        let process = language();
        {
            let _polish = speak_here("pl_PL.UTF-8");
            assert_eq!(language(), "pl");
            assert_eq!(text("erase-back"), text_in("pl", "erase-back"));
            {
                let _german = speak_here("de");
                assert_eq!(language(), "de");
            }
            assert_eq!(language(), "pl");
            let elsewhere = std::thread::spawn(language).join().unwrap();
            assert_eq!(elsewhere, process, "another thread heard it");
        }
        assert_eq!(language(), process);
    }

    /// A language named outright is read in that language, and a regional
    /// overlay answers only where it differs. Asked through `text_in`, never
    /// by switching the process's language: tests run side by side, and one
    /// that switched it would change the words every other test reads.
    #[test]
    fn a_named_language_is_read_without_switching_the_process() {
        let british = text_in("en-GB", "desktop-caption");
        let american = text_in("en_US.UTF-8", "desktop-caption");
        assert_ne!(british, american, "the overlay said nothing different");
        assert_eq!(
            text_in("en-US", "app-name"),
            text_in("en-GB", "app-name"),
            "the overlay lost a message it does not carry"
        );
        // A locale no catalog speaks is answered in English, not with ids.
        assert_eq!(text_in("cs_CZ.UTF-8", "app-name"), "Linexin Installer");
    }
}
