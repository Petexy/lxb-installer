//! A search over one of the long lists — Language, Time zone, Keyboard —
//! the field LineXinBar's own long lists keep at their head (the file
//! picker's Search row, the library's, the keyboard layouts'), here a field
//! over the list.
//!
//! * **Summoned by Options**: North on a pad, and on a keyboard what the
//!   toolkit reads as the same button, as the shell does — the Menu key, F10
//!   and the letter Y where no field takes it (`Action::Menu`); a right click
//!   too. The legend says so ("Search"). It comes up with the light in it: with a keyboard in hand typing goes
//!   straight in, and with a pad a board rises at once — the shell's under
//!   LineXinBar, the installer's elsewhere (`State::type_with_pad`).
//! * **Every letter narrows the list at once**, as the shell's search does
//!   ("results live while typing"): a row stays when every word typed is
//!   found in what it is called, whatever the case and the accents (`plain`)
//!   — a language by its own name and its English one, a place by its city,
//!   its country and its part of the world, a keyboard by its layout and its
//!   kind.
//! * **The light lands on the first match**: typing done — Enter, the
//!   board's Done, Down — takes it there, as the shell's Enter rests on the
//!   first thing found; Accept chooses it.
//! * **Back empties the field, and then closes it**, the list whole again
//!   and the light on what it was on; a third Back is the screen's own.
//!
//! Each of the three screens keeps its own [`Search`] in its view, and they
//! share one field (`FieldId::Search`), since one screen is up at a time.

use super::field::{self, Edit};

/// A search field: whether it is up, and what is typed in it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Search {
    open: bool,
    query: String,
}

/// What Back did to a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backed {
    /// What was typed is gone; the field is still up, and the list whole.
    Emptied,
    /// The field is gone.
    Closed,
    /// There was no field: Back is the screen's own.
    Nothing,
}

impl Search {
    /// Whether the field is up.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// What is typed in it.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Whether the list shows only what the search found: the field is up
    /// and something other than spaces is typed in it.
    pub fn narrows(&self) -> bool {
        self.open && !self.query.trim().is_empty()
    }

    /// Bring the field up. Answers whether it was down.
    pub fn open(&mut self) -> bool {
        !std::mem::replace(&mut self.open, true)
    }

    /// Put the field away and forget what was typed in it.
    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
    }

    /// Back: what was typed goes first, then the field.
    pub fn back(&mut self) -> Backed {
        if !self.open {
            Backed::Nothing
        } else if !self.query.is_empty() {
            self.query.clear();
            Backed::Emptied
        } else {
            self.open = false;
            Backed::Closed
        }
    }

    /// Type into the field, which comes up if it was down. Answers whether
    /// what is typed in it changed.
    pub fn edit(&mut self, edit: &Edit) -> bool {
        self.open = true;
        let was = self.query.clone();
        field::apply(&mut self.query, edit);
        self.query != was
    }

    /// Whether something called by `words` stays in the list: everything
    /// does while the search does not narrow it.
    pub fn finds(&self, words: &[&str]) -> bool {
        !self.narrows() || matches(&self.query, words)
    }
}

/// Whether every word of `query` is found in `words`, read together: in any
/// order, at the start of a word or inside one, whatever the case and the
/// accents. "new york" finds New York, "lodz" Łódź and "polsk" Polski; a
/// script written without spaces is found by any run of its characters.
pub fn matches(query: &str, words: &[&str]) -> bool {
    let haystack = plain(&words.join(" "));
    plain(query)
        .split_whitespace()
        .all(|word| haystack.contains(word))
}

/// `text` as it is compared: in small letters, the Latin letters' accents
/// folded away (`ł` as `l`, `ß` as `ss`) so that a keyboard without them
/// still finds them, and every other script as it is.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for letter in text.chars().flat_map(char::to_lowercase) {
        match folded(letter) {
            Some(spelt) => out.push_str(spelt),
            None => out.push(letter),
        }
    }
    out
}

/// A Latin letter with an accent, without it; `None` for anything else.
fn folded(letter: char) -> Option<&'static str> {
    Some(match letter {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ą' | 'ă' => "a",
        'ç' | 'ć' | 'č' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
        'ğ' => "g",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ı' => "i",
        'ł' | 'ľ' | 'ĺ' => "l",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'ŕ' | 'ř' => "r",
        'ś' | 'š' | 'ş' | 'ș' => "s",
        'ß' => "ss",
        'ť' | 'ţ' | 'ț' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'æ' => "ae",
        'œ' => "oe",
        'þ' => "th",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_typed_is_found_whatever_its_case_and_accents() {
        assert!(matches("polsk", &["Polski (Polska)", "Polish (Poland)"]));
        assert!(matches("POLISH", &["Polski (Polska)", "Polish (Poland)"]));
        assert!(matches("lodz", &["Łódź"]));
        assert!(matches("Łódź", &["Lodz"]), "an accent typed finds it bare");
        assert!(matches("strasse", &["Straße"]));
        assert!(matches("york new", &["New York", "United States"]));
        assert!(matches(
            "deu",
            &["Deutsch (Deutschland)", "German (Germany)"]
        ));
        assert!(!matches("deu polsk", &["Deutsch (Deutschland)"]));
        assert!(matches("рус", &["Русский (Россия)", "Russian (Russia)"]));
        assert!(matches("中文", &["中文（中国）", "Chinese (China)"]));
        assert!(matches("中国", &["中文（中国）"]), "a run inside the words");
        assert!(matches("हिन्", &["हिन्दी (भारत)", "Hindi (India)"]));
        assert!(matches("", &["anything"]), "nothing typed finds everything");
        assert!(matches("   ", &["anything"]));
    }

    #[test]
    fn back_empties_the_field_then_closes_it() {
        let mut search = Search::default();
        assert_eq!(search.back(), Backed::Nothing, "a field that was not up");
        assert!(search.open());
        assert!(!search.open(), "up already");
        assert!(search.edit(&Edit::Wrote("pol".into())));
        assert!(search.narrows());
        assert_eq!(search.back(), Backed::Emptied);
        assert!(search.is_open() && !search.narrows());
        assert_eq!(search.back(), Backed::Closed);
        assert!(!search.is_open());
        assert_eq!(search.back(), Backed::Nothing);
    }

    #[test]
    fn typing_brings_the_field_up_and_only_letters_narrow_the_list() {
        let mut search = Search::default();
        assert!(search.finds(&["Polski"]), "a search not up hid a row");
        assert!(search.edit(&Edit::Wrote(" ".into())));
        assert!(search.is_open());
        assert!(!search.narrows(), "a space narrowed the list");
        assert!(search.finds(&["Polski"]));
        assert!(search.edit(&Edit::Wrote("x".into())));
        assert!(!search.finds(&["Polski"]));
        assert!(search.edit(&Edit::Rubbed));
        assert_eq!(search.query(), " ");
        search.close();
        assert_eq!(search, Search::default());
    }
}
