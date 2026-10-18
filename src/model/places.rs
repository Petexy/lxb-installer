//! Time zone's places and their countries, in the language the person chose.
//!
//! The time zone database names its places in English and in ASCII
//! (`Europe/Warsaw`); a person reads Warszawa, Варшава or 华沙. The names are
//! CLDR's — the exemplar cities every program asking ICU for a zone's place
//! gets, and its territory names — for every language the Language step
//! offers, with the order each language sorts them in (Chinese by pinyin,
//! Polish with Ł after L). They are carried in the binary
//! (`data/places/names.txt`, made by `scripts/cldr-places.py` from the
//! machine's ICU and committed), so nothing on the live medium is asked,
//! nothing is linked, and a picture or a test says the same on every
//! computer. Their licence's notice is `data/places/LICENSE`, installed
//! beside the program's.
//!
//! A zone the table does not know (a newer tzdata on the medium than the
//! table was made from) keeps the database's own city; a language without a
//! name of its own for a place has CLDR's fallback written in its stead,
//! which is the English name.
//!
//! Who uses it: `catalogs::name_zones` fills each `Zone`'s shown names from
//! a [`Names`] and sorts the places by its order; tests hand it a table of
//! their own through [`Names::read`].
use std::collections::HashMap;

/// The table the installer carries: every language's names and order.
pub const TABLE: &str = include_str!("../../data/places/names.txt");

/// The names one language gives the places and their countries, and the
/// order it sorts the places in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Names {
    /// A zone's city, by its tz name, where the table names it otherwise
    /// than the tz name does (`Europe/Warsaw` → `Warszawa`).
    cities: HashMap<String, String>,
    /// A country, by its ISO code.
    countries: HashMap<String, String>,
    /// A zone's place in the language's order, by its tz name.
    ranks: HashMap<String, u32>,
}

impl Names {
    /// The names `locale` (`pl_PL.UTF-8`) reads, out of the installer's own
    /// table.
    pub fn of(locale: &str) -> Self {
        Self::read(TABLE, locale)
    }

    /// No names at all: the database's own cities and countries, in its
    /// order.
    pub fn none() -> Self {
        Self::default()
    }

    /// The names `locale` reads out of `table` (written as
    /// `data/places/names.txt` is): its own section, else the first of its
    /// language's (`de_AT` reads `de_DE`), else English.
    pub fn read(table: &str, locale: &str) -> Self {
        let sections = Sections::of(table);
        let wanted = locale.split(['.', '@']).next().unwrap_or_default().trim();
        let language = wanted.split(['_', '-']).next().unwrap_or_default();
        let key = sections
            .find(wanted)
            .or_else(|| {
                sections.order.iter().copied().find(|key| {
                    key.split(['_', '-']).next() == Some(language) && !language.is_empty()
                })
            })
            .unwrap_or("en");
        let base = sections.lines("en");
        let own = sections.lines(sections.resolve(key));
        let mut names = Self::default();
        let zone = |at: &str| -> Option<&str> {
            at.parse::<usize>()
                .ok()
                .and_then(|at| sections.zones.get(at).copied())
        };
        // The base's names first, the language's own over them.
        for lines in [&base, &own] {
            for line in lines.iter() {
                let (head, name) = line.split_once('\t').unwrap_or((line, ""));
                if head.starts_with(|c: char| c.is_ascii_digit()) {
                    if let (Some(zone), false) = (zone(head), name.is_empty()) {
                        names.cities.insert(zone.to_string(), name.to_string());
                    }
                } else if !name.is_empty() {
                    names.countries.insert(head.to_string(), name.to_string());
                }
            }
        }
        let ordered = own
            .iter()
            .map(|line| line.split('\t').next().unwrap_or(line))
            .filter(|head| head.starts_with(|c: char| c.is_ascii_digit()))
            .filter_map(zone);
        for (rank, zone) in ordered.enumerate() {
            names.ranks.insert(zone.to_string(), rank as u32);
        }
        names
    }

    /// The city of the zone called `zone` (its tz name), where the language
    /// names it otherwise than the tz name does.
    pub fn city(&self, zone: &str) -> Option<&str> {
        self.cities.get(zone).map(String::as_str)
    }

    /// The country with the ISO code `code`, in the language.
    pub fn country(&self, code: &str) -> Option<&str> {
        self.countries.get(code).map(String::as_str)
    }

    /// Where the zone called `zone` comes in the language's order of its
    /// places; `None` for a zone the table does not know.
    pub fn rank(&self, zone: &str) -> Option<u32> {
        self.ranks.get(zone).copied()
    }
}

/// Every zone the carried table names, with the code of its country: what
/// the catalogs' test holds the names of to the room they get.
#[cfg(test)]
pub fn carried() -> Vec<(&'static str, &'static str)> {
    TABLE
        .lines()
        .skip_while(|line| *line != "[zones]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('\t'))
        .collect()
}

/// The table cut into its sections.
struct Sections<'a> {
    /// Every zone the table names, by its place in `[zones]`.
    zones: Vec<&'a str>,
    /// Each section's lines, by its key (`en`, `pl_PL`).
    lines: HashMap<&'a str, Vec<&'a str>>,
    /// A section that says everything as another does (`en_US` → `en`).
    same: HashMap<&'a str, &'a str>,
    /// The keys in the table's order.
    order: Vec<&'a str>,
}

impl<'a> Sections<'a> {
    fn of(table: &'a str) -> Self {
        let mut sections = Self {
            zones: Vec::new(),
            lines: HashMap::new(),
            same: HashMap::new(),
            order: Vec::new(),
        };
        let mut current: Option<&'a str> = None;
        for line in table.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if let Some(head) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                match head.split_once('=') {
                    Some((key, other)) => {
                        sections.same.insert(key, other);
                        sections.order.push(key);
                        current = None;
                    }
                    None => {
                        if head != "zones" {
                            sections.order.push(head);
                        }
                        current = Some(head);
                    }
                }
                continue;
            }
            match current {
                Some("zones") => sections.zones.push(line.split('\t').next().unwrap_or(line)),
                Some(key) => sections.lines.entry(key).or_default().push(line),
                None => {}
            }
        }
        sections
    }

    /// The section `key` names, if the table has one.
    fn find(&self, key: &str) -> Option<&'a str> {
        self.order.iter().copied().find(|one| *one == key)
    }

    /// The section whose lines `key` reads: its own, or the one it says
    /// everything as (followed a few steps at most).
    fn resolve(&self, key: &'a str) -> &'a str {
        let mut key = key;
        for _ in 0..4 {
            match self.same.get(key) {
                Some(other) => key = other,
                None => break,
            }
        }
        key
    }

    fn lines(&self, key: &str) -> Vec<&'a str> {
        self.lines.get(key).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::catalogs;

    /// A table of its own, written as `data/places/names.txt` is.
    const FIXTURE: &str = "\
# a table for the tests
[zones]
America/Indiana/Knox
America/New_York
Europe/Warsaw
[en]
1
0\tKnox, Indiana
2
PL\tPoland
US\tUnited States
[pl_PL]
0\tKnox, Indiana
1\tNowy Jork
2\tWarszawa
PL\tPolska
US\tStany Zjednoczone
[en_US=en]
[pl_XX=pl_PL]
";

    #[test]
    fn a_language_reads_its_own_names_over_the_base() {
        let polish = Names::read(FIXTURE, "pl_PL.UTF-8");
        assert_eq!(polish.city("Europe/Warsaw"), Some("Warszawa"));
        assert_eq!(polish.city("America/New_York"), Some("Nowy Jork"));
        assert_eq!(polish.country("US"), Some("Stany Zjednoczone"));
        assert_eq!(polish.rank("America/Indiana/Knox"), Some(0));
        assert_eq!(polish.rank("Europe/Warsaw"), Some(2));
        assert_eq!(polish.rank("Asia/Tokyo"), None, "a zone the table lacks");
        // English: the base, in its own order, a name only where it differs
        // from the tz name's city.
        let english = Names::read(FIXTURE, "en_US.UTF-8");
        assert_eq!(english.city("America/Indiana/Knox"), Some("Knox, Indiana"));
        assert_eq!(english.city("Europe/Warsaw"), None);
        assert_eq!(english.country("PL"), Some("Poland"));
        assert_eq!(english.rank("America/New_York"), Some(0));
        assert_eq!(english.rank("America/Indiana/Knox"), Some(1));
    }

    #[test]
    fn a_locale_without_a_section_reads_its_language_or_english() {
        let fixture = Names::read(FIXTURE, "pl_PL.UTF-8");
        assert_eq!(
            Names::read(FIXTURE, "pl_XX.UTF-8"),
            fixture,
            "said as another"
        );
        assert_eq!(Names::read(FIXTURE, "pl_AT"), fixture, "its language's");
        assert_eq!(Names::read(FIXTURE, "pl_PL.UTF-8@euro"), fixture);
        let english = Names::read(FIXTURE, "en_US.UTF-8");
        assert_eq!(Names::read(FIXTURE, "xx_ZZ.UTF-8"), english);
        assert_eq!(Names::read(FIXTURE, ""), english);
        assert_eq!(Names::read("", "pl_PL"), Names::none(), "no table at all");
    }

    /// The table the installer carries speaks every language the Language
    /// step offers, each in its own script and order.
    #[test]
    fn the_carried_table_names_every_language_offered() {
        let sections = Sections::of(TABLE);
        assert!(sections.zones.len() > 400, "{} zones", sections.zones.len());
        assert!(sections.zones.windows(2).all(|pair| pair[0] < pair[1]));
        for language in catalogs::languages() {
            let key = language.locale.split('.').next().unwrap();
            assert!(sections.find(key).is_some(), "no section for {key}");
            let names = Names::of(language.locale);
            assert_eq!(names.ranks.len(), sections.zones.len(), "{key}");
            assert!(names.country("PL").is_some(), "{key}");
        }
        let polish = Names::of("pl_PL.UTF-8");
        assert_eq!(polish.city("Europe/Warsaw"), Some("Warszawa"));
        assert_eq!(polish.country("DE"), Some("Niemcy"));
        assert_eq!(
            Names::of("ru_RU.UTF-8").city("Europe/Moscow"),
            Some("Москва")
        );
        assert_eq!(Names::of("zh_CN.UTF-8").city("Asia/Shanghai"), Some("上海"));
        assert_eq!(Names::of("de_DE.UTF-8").city("Europe/Vienna"), Some("Wien"));
        assert_eq!(Names::of("ja_JP.UTF-8").city("Asia/Tokyo"), Some("東京"));
        assert_eq!(Names::of("en_US.UTF-8").city("America/New_York"), None);
        // Polish sorts Ł between L and M, as CLDR's collation does (code
        // points would put Ałmaty after Amman); Chinese by pinyin (阿比让 ā,
        // 纽约 niǔ, 上海 shàng), not by code point (上 < 纽 < 阿).
        let rank = |names: &Names, zone| names.rank(zone).unwrap();
        assert!(rank(&polish, "Africa/Algiers") < rank(&polish, "Asia/Almaty"));
        assert!(rank(&polish, "Asia/Almaty") < rank(&polish, "Asia/Amman"));
        let chinese = Names::of("zh_CN.UTF-8");
        assert!(rank(&chinese, "Africa/Abidjan") < rank(&chinese, "America/New_York"));
        assert!(rank(&chinese, "America/New_York") < rank(&chinese, "Asia/Shanghai"));
    }
}
