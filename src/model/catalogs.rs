//! The lists people choose from: system languages, time zones, keyboard
//! layouts, desktops, Flatpak applications and the image's packages. Read
//! from the live system where it has them (zoneinfo, XKB), never from this
//! development machine's settings. Owned by the data work.
//!
//! Every reader takes the root it reads under, so that the tests can hand
//! it a small tree written by hand and the interface can point it at the
//! live system (`/`) or the image (`/run/archiso/airootfs`). Nothing here
//! writes anything; what words a person reads comes from the interface's
//! catalogs, so the only text here is names the system itself carries
//! (layout descriptions, package descriptions, language names in their own
//! language).
use super::config::Desktop;
use std::collections::BTreeSet;
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

// -------------------------------------------------------------------------
// Languages

/// A system language the installer offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Language {
    /// The locale the installed system is given (`pl_PL.UTF-8`).
    pub locale: &'static str,
    /// The language's own name for itself and its country, as the legacy
    /// installer showed it (`Polski (Polska)`).
    pub native: &'static str,
    /// The same in English, for searching (`Polish (Poland)`).
    pub english: &'static str,
    /// The family catalog the installer's own words switch to when this
    /// language is chosen (`pl`, `en-GB`), or `None` when there is none.
    pub ui: Option<&'static str>,
}

impl Language {
    /// The country the locale names (`pl_PL.UTF-8` is Poland, `PL`), whose
    /// flag the language wears on the Language step, as the legacy installer
    /// wrote it before the name (`country_code_to_emoji`, from the part of
    /// the locale between `_` and `.`): Catalan and Galician wear Spain's,
    /// Māori New Zealand's. `None` for a locale that names no country, which
    /// wears the white flag the legacy installer gave such a one.
    pub fn country(&self) -> Option<&'static str> {
        territory(self.locale)
    }
}

/// The territory of a locale name — the two capital letters after its `_`,
/// before any charset or modifier (`sr_RS@latin` is `RS`) — or `None` where
/// it names none (`eo`, `C.UTF-8`) or names it some other way (`es_419`).
pub fn territory(locale: &str) -> Option<&str> {
    let (_, rest) = locale.split_once('_')?;
    let code = rest.split(['.', '@']).next().unwrap_or_default();
    (code.len() == 2 && code.bytes().all(|letter| letter.is_ascii_uppercase())).then_some(code)
}

const fn lang(
    locale: &'static str,
    native: &'static str,
    english: &'static str,
    ui: Option<&'static str>,
) -> Language {
    Language {
        locale,
        native,
        english,
        ui,
    }
}

/// The legacy installer's 65 languages (`language_widget.py`), in its
/// order: by native name, compared character by character as Python's
/// `sorted` does. Its `no_NO` is written `nb_NO` here: glibc has no
/// `no_NO` locale, so the legacy choice produced a system with no language.
/// English outside the United States reads the British catalog; European
/// Portuguese and Traditional Chinese have no catalog of their own.
pub fn languages() -> &'static [Language] {
    &LANGUAGES
}

static LANGUAGES: [Language; 65] = [
    lang(
        "af_ZA.UTF-8",
        "Afrikaans (Suid-Afrika)",
        "Afrikaans (South Africa)",
        None,
    ),
    lang(
        "id_ID.UTF-8",
        "Bahasa Indonesia (Indonesia)",
        "Indonesian (Indonesia)",
        None,
    ),
    lang(
        "ms_MY.UTF-8",
        "Bahasa Melayu (Malaysia)",
        "Malay (Malaysia)",
        None,
    ),
    lang(
        "bs_BA.UTF-8",
        "Bosanski (Bosna i Hercegovina)",
        "Bosnian (Bosnia and Herzegovina)",
        None,
    ),
    lang("ca_ES.UTF-8", "Català (Espanya)", "Catalan (Spain)", None),
    lang("da_DK.UTF-8", "Dansk (Danmark)", "Danish (Denmark)", None),
    lang(
        "de_DE.UTF-8",
        "Deutsch (Deutschland)",
        "German (Germany)",
        Some("de"),
    ),
    lang("et_EE.UTF-8", "Eesti (Eesti)", "Estonian (Estonia)", None),
    lang(
        "en_AU.UTF-8",
        "English (Australia)",
        "English (Australia)",
        Some("en-GB"),
    ),
    lang(
        "en_CA.UTF-8",
        "English (Canada)",
        "English (Canada)",
        Some("en-GB"),
    ),
    lang(
        "en_GB.UTF-8",
        "English (United Kingdom)",
        "English (United Kingdom)",
        Some("en-GB"),
    ),
    lang(
        "en_US.UTF-8",
        "English (United States)",
        "English (United States)",
        Some("en-US"),
    ),
    lang(
        "es_ES.UTF-8",
        "Español (España)",
        "Spanish (Spain)",
        Some("es"),
    ),
    lang(
        "fil_PH.UTF-8",
        "Filipino (Pilipinas)",
        "Filipino (Philippines)",
        None,
    ),
    lang(
        "fr_CA.UTF-8",
        "Français (Canada)",
        "French (Canada)",
        Some("fr"),
    ),
    lang(
        "fr_FR.UTF-8",
        "Français (France)",
        "French (France)",
        Some("fr"),
    ),
    lang("ga_IE.UTF-8", "Gaeilge (Éire)", "Irish (Ireland)", None),
    lang("gl_ES.UTF-8", "Galego (España)", "Galician (Spain)", None),
    lang(
        "hr_HR.UTF-8",
        "Hrvatski (Hrvatska)",
        "Croatian (Croatia)",
        None,
    ),
    lang("it_IT.UTF-8", "Italiano (Italia)", "Italian (Italy)", None),
    lang(
        "lv_LV.UTF-8",
        "Latviešu (Latvija)",
        "Latvian (Latvia)",
        None,
    ),
    lang(
        "lt_LT.UTF-8",
        "Lietuvių (Lietuva)",
        "Lithuanian (Lithuania)",
        None,
    ),
    lang(
        "hu_HU.UTF-8",
        "Magyar (Magyarország)",
        "Hungarian (Hungary)",
        None,
    ),
    lang(
        "nl_NL.UTF-8",
        "Nederlands (Nederland)",
        "Dutch (Netherlands)",
        None,
    ),
    lang(
        "nb_NO.UTF-8",
        "Norsk bokmål (Norge)",
        "Norwegian Bokmål (Norway)",
        None,
    ),
    lang(
        "nn_NO.UTF-8",
        "Norsk nynorsk (Noreg)",
        "Norwegian Nynorsk (Norway)",
        None,
    ),
    lang(
        "pl_PL.UTF-8",
        "Polski (Polska)",
        "Polish (Poland)",
        Some("pl"),
    ),
    lang(
        "pt_BR.UTF-8",
        "Português (Brasil)",
        "Portuguese (Brazil)",
        Some("pt-BR"),
    ),
    lang(
        "pt_PT.UTF-8",
        "Português (Portugal)",
        "Portuguese (Portugal)",
        None,
    ),
    lang(
        "ro_RO.UTF-8",
        "Română (România)",
        "Romanian (Romania)",
        None,
    ),
    lang(
        "sq_AL.UTF-8",
        "Shqip (Shqipëri)",
        "Albanian (Albania)",
        None,
    ),
    lang(
        "sk_SK.UTF-8",
        "Slovenčina (Slovensko)",
        "Slovak (Slovakia)",
        None,
    ),
    lang(
        "sl_SI.UTF-8",
        "Slovenščina (Slovenija)",
        "Slovenian (Slovenia)",
        None,
    ),
    lang(
        "so_SO.UTF-8",
        "Soomaali (Soomaaliya)",
        "Somali (Somalia)",
        None,
    ),
    lang("fi_FI.UTF-8", "Suomi (Suomi)", "Finnish (Finland)", None),
    lang("sv_SE.UTF-8", "Svenska (Sverige)", "Swedish (Sweden)", None),
    lang(
        "tl_PH.UTF-8",
        "Tagalog (Pilipinas)",
        "Tagalog (Philippines)",
        None,
    ),
    lang(
        "mi_NZ.UTF-8",
        "Te Reo Māori (Aotearoa)",
        "Maori (New Zealand)",
        None,
    ),
    lang(
        "vi_VN.UTF-8",
        "Tiếng Việt (Việt Nam)",
        "Vietnamese (Vietnam)",
        None,
    ),
    lang("tr_TR.UTF-8", "Türkçe (Türkiye)", "Turkish (Türkiye)", None),
    lang(
        "is_IS.UTF-8",
        "Íslenska (Ísland)",
        "Icelandic (Iceland)",
        None,
    ),
    lang(
        "cs_CZ.UTF-8",
        "Čeština (Česká republika)",
        "Czech (Czech Republic)",
        None,
    ),
    lang("el_GR.UTF-8", "Ελληνικά (Ελλάδα)", "Greek (Greece)", None),
    lang(
        "be_BY.UTF-8",
        "Беларуская (Беларусь)",
        "Belarusian (Belarus)",
        None,
    ),
    lang(
        "bg_BG.UTF-8",
        "Български (България)",
        "Bulgarian (Bulgaria)",
        None,
    ),
    lang(
        "mn_MN.UTF-8",
        "Монгол (Монгол)",
        "Mongolian (Mongolia)",
        None,
    ),
    lang(
        "ru_RU.UTF-8",
        "Русский (Россия)",
        "Russian (Russia)",
        Some("ru"),
    ),
    lang("sr_RS.UTF-8", "Српски (Србија)", "Serbian (Serbia)", None),
    lang(
        "uk_UA.UTF-8",
        "Українська (Україна)",
        "Ukrainian (Ukraine)",
        None,
    ),
    lang("he_IL.UTF-8", "עברית (ישראל)", "Hebrew (Israel)", None),
    lang(
        "ar_SA.UTF-8",
        "العربية (المملكة العربية السعودية)",
        "Arabic (Saudi Arabia)",
        None,
    ),
    lang("fa_IR.UTF-8", "فارسی (ایران)", "Persian (Iran)", None),
    lang("hi_IN.UTF-8", "हिन्दी (भारत)", "Hindi (India)", Some("hi")),
    lang("gu_IN.UTF-8", "ગુજરાતી (ભારત)", "Gujarati (India)", None),
    lang("ta_IN.UTF-8", "தமிழ் (இந்தியா)", "Tamil (India)", None),
    lang("kn_IN.UTF-8", "ಕನ್ನಡ (ಭಾರತ)", "Kannada (India)", None),
    lang("ml_IN.UTF-8", "മലയാളം (ഇന്ത്യ)", "Malayalam (India)", None),
    lang("th_TH.UTF-8", "ไทย (ประเทศไทย)", "Thai (Thailand)", None),
    lang("lo_LA.UTF-8", "ລາວ (ລາວ)", "Lao (Laos)", None),
    lang(
        "ka_GE.UTF-8",
        "ქართული (საქართველო)",
        "Georgian (Georgia)",
        None,
    ),
    lang("km_KH.UTF-8", "ភាសាខ្មែរ (កម្ពុជា)", "Khmer (Cambodia)", None),
    lang("ja_JP.UTF-8", "日本語 (日本)", "Japanese (Japan)", None),
    lang(
        "zh_CN.UTF-8",
        "简体中文 (中国)",
        "Chinese, Simplified (China)",
        Some("zh-CN"),
    ),
    lang(
        "zh_TW.UTF-8",
        "繁體中文 (台灣)",
        "Chinese, Traditional (Taiwan)",
        None,
    ),
    lang(
        "ko_KR.UTF-8",
        "한국어 (대한민국)",
        "Korean (South Korea)",
        None,
    ),
];

/// The language and country parts of a locale (`pl`, `PL` from
/// `pl_PL.UTF-8`; `sr`, `RS` from `sr_RS@latin`).
fn locale_parts(locale: &str) -> (&str, Option<&str>) {
    let name = locale.split(['.', '@']).next().unwrap_or_default();
    match name.split_once('_') {
        Some((language, country)) if !country.is_empty() => (language, Some(country)),
        _ => (name, None),
    }
}

// -------------------------------------------------------------------------
// Time zones

/// A time zone, split for the two columns the interface shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zone {
    /// The tz name the system is given (`America/Argentina/Buenos_Aires`).
    pub name: String,
    /// The first part of the name (`America`); `UTC` for UTC itself.
    pub region: String,
    /// The rest, readable (`Argentina / Buenos Aires`); empty for UTC.
    pub city: String,
    /// The country the place keeps its clock for, as the time zone database
    /// names it in `iso3166.tab` (`Poland`): what a search finds a place by
    /// besides its city. Empty where the list says none (UTC, a list read
    /// from `tzdata.zi`) or the names of the countries are missing.
    pub country: String,
    /// That country's ISO code (`PL`), by which its name is found in the
    /// person's language; empty where the list says none.
    pub code: String,
    /// The city as the person's language names it (`Warszawa`, `Варшава`),
    /// set by [`name_zones`]: CLDR's name where the installer carries one,
    /// else the database's own city, the last part of `city`. Empty for
    /// UTC.
    pub shown_city: String,
    /// The country as the person's language names it (`Polska`), set by
    /// [`name_zones`]: CLDR's name, else `country`.
    pub shown_country: String,
}

/// Say every zone's city and country in the language `names` speaks, and
/// sort the places of each part of the world as that language sorts them
/// (`names`' order; a place it does not know after those it does, by its
/// city). The parts of the world keep the database's order.
pub fn name_zones(zones: &mut [Zone], names: &super::places::Names) {
    for zone in zones.iter_mut() {
        let own = zone.city.rsplit(" / ").next().unwrap_or_default();
        zone.shown_city = names.city(&zone.name).unwrap_or(own).to_string();
        zone.shown_country = names
            .country(&zone.code)
            .unwrap_or(&zone.country)
            .to_string();
    }
    zones.sort_by(|a, b| {
        let rank = |zone: &Zone| names.rank(&zone.name).unwrap_or(u32::MAX);
        (&a.region, rank(a), &a.city, &a.name).cmp(&(&b.region, rank(b), &b.city, &b.name))
    });
}

/// The time zones under `root/usr/share/zoneinfo`, plus UTC, sorted by
/// region and then city.
///
/// The list comes from `zone.tab`, falling back to `zone1970.tab` and then
/// to the `Zone` lines of `tzdata.zi` when a file is missing. `zone.tab` is
/// the one built for choosing: one row per country and clock, so every
/// country's own cities are there to be found (Oslo, Copenhagen, Zagreb),
/// all in ASCII. `zone1970.tab` folds countries whose clocks have agreed
/// since 1970 into one zone (Norway, Denmark and Sweden into
/// `Europe/Berlin`), so a Norwegian would not find Oslo; `tzdata.zi` also
/// names every old alias (`US/Eastern`) and has no countries at all. Each
/// name is kept only when its file exists, because that file is what the
/// installed system's `/etc/localtime` will point to.
///
/// Each place carries its country's name, out of `iso3166.tab` beside them,
/// by the code `zone.tab` gives it (the first of `zone1970.tab`'s).
pub fn zones(root: &Path) -> Vec<Zone> {
    let dir = root.join("usr/share/zoneinfo");
    let read = |file: &str| std::fs::read_to_string(dir.join(file)).ok();
    let mut named = read("zone.tab")
        .map(|text| tab_zones(&text))
        .filter(|names| !names.is_empty())
        .or_else(|| read("zone1970.tab").map(|text| tab_zones(&text)))
        .filter(|names| !names.is_empty())
        .or_else(|| {
            read("tzdata.zi").map(|text| {
                tzdata_zones(&text)
                    .into_iter()
                    .map(|name| (name, String::new()))
                    .collect()
            })
        })
        .unwrap_or_default();
    named.retain(|(name, _)| zone_name_ok(name) && dir.join(name).is_file());
    named.push(("UTC".to_string(), String::new()));
    let countries = read("iso3166.tab")
        .map(|text| country_names(&text))
        .unwrap_or_default();
    let mut zones: Vec<Zone> = named
        .iter()
        .map(|(name, code)| Zone {
            country: countries.get(code).cloned().unwrap_or_default(),
            code: code.clone(),
            ..zone(name)
        })
        .collect();
    zones.sort_by(|a, b| (&a.region, &a.city, &a.name).cmp(&(&b.region, &b.city, &b.name)));
    zones.dedup_by(|a, b| a.name == b.name);
    name_zones(&mut zones, &super::places::Names::none());
    zones
}

/// The places of `zone.tab` or `zone1970.tab` (their third column), each
/// with its country's code (the first of the first column's).
fn tab_zones(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut columns = line.split_whitespace();
            let codes = columns.next()?;
            let name = columns.nth(1)?;
            let code = codes.split(',').next().unwrap_or_default();
            Some((name.to_string(), code.to_string()))
        })
        .collect()
}

/// The countries `iso3166.tab` names, by their codes.
fn country_names(text: &str) -> std::collections::HashMap<String, String> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let (code, name) = line.split_once('\t')?;
            Some((code.trim().to_string(), name.trim().to_string()))
        })
        .filter(|(code, name)| !code.is_empty() && !name.is_empty())
        .collect()
}

/// The places named by `Zone` lines of `tzdata.zi` (zic's compact source,
/// where `Z` abbreviates `Zone`). The `Etc/` zones and names without a
/// region (`EST`, `Factory`) are offsets and placeholders, not places.
fn tzdata_zones(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            match words.next()? {
                "Z" | "Zone" => words.next(),
                _ => None,
            }
        })
        .filter(|name| name.contains('/') && !name.starts_with("Etc/"))
        .map(str::to_string)
        .collect()
}

/// The shape `config::validate_region` accepts, so that nothing offered
/// here is refused there and no name can leave the zoneinfo directory.
fn zone_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 100
        && name.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_+-".contains(&c))
        })
}

fn zone(name: &str) -> Zone {
    let (region, rest) = name.split_once('/').unwrap_or((name, ""));
    let city = rest.replace('_', " ").replace('/', " / ");
    Zone {
        name: name.to_string(),
        region: region.to_string(),
        shown_city: city.rsplit(" / ").next().unwrap_or_default().to_string(),
        city,
        country: String::new(),
        code: String::new(),
        shown_country: String::new(),
    }
}

/// The zone a person speaking `locale` most likely lives in: the zone its
/// country is known by, when `zones` has it. `None` for a locale without a
/// country, a country with no zone, or a zone the image lacks.
pub fn guess_zone(locale: &str, zones: &[Zone]) -> Option<String> {
    let country = locale_parts(locale).1?.to_ascii_uppercase();
    let index = COUNTRY_ZONES
        .binary_search_by(|(code, _)| (*code).cmp(country.as_str()))
        .ok()?;
    let name = COUNTRY_ZONES[index].1;
    zones
        .iter()
        .any(|zone| zone.name == name)
        .then(|| name.to_string())
}

/// Each country's first row in tzdata's `zone.tab`, sorted by country code.
/// Where that first row is an outlying corner of a large country (Lord Howe
/// Island for Australia, Kaliningrad for Russia, Crimea for Ukraine), the
/// zone most of its people live in is used instead, and the comment names
/// the row it replaces. Written here rather than read from the image because
/// [`guess_zone`] judges from the list alone.
static COUNTRY_ZONES: &[(&str, &str)] = &[
    ("AD", "Europe/Andorra"),
    ("AE", "Asia/Dubai"),
    ("AF", "Asia/Kabul"),
    ("AG", "America/Antigua"),
    ("AI", "America/Anguilla"),
    ("AL", "Europe/Tirane"),
    ("AM", "Asia/Yerevan"),
    ("AO", "Africa/Luanda"),
    ("AQ", "Antarctica/McMurdo"),
    ("AR", "America/Argentina/Buenos_Aires"),
    ("AS", "Pacific/Pago_Pago"),
    ("AT", "Europe/Vienna"),
    ("AU", "Australia/Sydney"), // zone.tab lists Australia/Lord_Howe first
    ("AW", "America/Aruba"),
    ("AX", "Europe/Mariehamn"),
    ("AZ", "Asia/Baku"),
    ("BA", "Europe/Sarajevo"),
    ("BB", "America/Barbados"),
    ("BD", "Asia/Dhaka"),
    ("BE", "Europe/Brussels"),
    ("BF", "Africa/Ouagadougou"),
    ("BG", "Europe/Sofia"),
    ("BH", "Asia/Bahrain"),
    ("BI", "Africa/Bujumbura"),
    ("BJ", "Africa/Porto-Novo"),
    ("BL", "America/St_Barthelemy"),
    ("BM", "Atlantic/Bermuda"),
    ("BN", "Asia/Brunei"),
    ("BO", "America/La_Paz"),
    ("BQ", "America/Kralendijk"),
    ("BR", "America/Sao_Paulo"), // zone.tab lists America/Noronha first
    ("BS", "America/Nassau"),
    ("BT", "Asia/Thimphu"),
    ("BW", "Africa/Gaborone"),
    ("BY", "Europe/Minsk"),
    ("BZ", "America/Belize"),
    ("CA", "America/Toronto"), // zone.tab lists America/St_Johns first
    ("CC", "Indian/Cocos"),
    ("CD", "Africa/Kinshasa"),
    ("CF", "Africa/Bangui"),
    ("CG", "Africa/Brazzaville"),
    ("CH", "Europe/Zurich"),
    ("CI", "Africa/Abidjan"),
    ("CK", "Pacific/Rarotonga"),
    ("CL", "America/Santiago"),
    ("CM", "Africa/Douala"),
    ("CN", "Asia/Shanghai"),
    ("CO", "America/Bogota"),
    ("CR", "America/Costa_Rica"),
    ("CU", "America/Havana"),
    ("CV", "Atlantic/Cape_Verde"),
    ("CW", "America/Curacao"),
    ("CX", "Indian/Christmas"),
    ("CY", "Asia/Nicosia"),
    ("CZ", "Europe/Prague"),
    ("DE", "Europe/Berlin"),
    ("DJ", "Africa/Djibouti"),
    ("DK", "Europe/Copenhagen"),
    ("DM", "America/Dominica"),
    ("DO", "America/Santo_Domingo"),
    ("DZ", "Africa/Algiers"),
    ("EC", "America/Guayaquil"),
    ("EE", "Europe/Tallinn"),
    ("EG", "Africa/Cairo"),
    ("EH", "Africa/El_Aaiun"),
    ("ER", "Africa/Asmara"),
    ("ES", "Europe/Madrid"),
    ("ET", "Africa/Addis_Ababa"),
    ("FI", "Europe/Helsinki"),
    ("FJ", "Pacific/Fiji"),
    ("FK", "Atlantic/Stanley"),
    ("FM", "Pacific/Chuuk"),
    ("FO", "Atlantic/Faroe"),
    ("FR", "Europe/Paris"),
    ("GA", "Africa/Libreville"),
    ("GB", "Europe/London"),
    ("GD", "America/Grenada"),
    ("GE", "Asia/Tbilisi"),
    ("GF", "America/Cayenne"),
    ("GG", "Europe/Guernsey"),
    ("GH", "Africa/Accra"),
    ("GI", "Europe/Gibraltar"),
    ("GL", "America/Nuuk"),
    ("GM", "Africa/Banjul"),
    ("GN", "Africa/Conakry"),
    ("GP", "America/Guadeloupe"),
    ("GQ", "Africa/Malabo"),
    ("GR", "Europe/Athens"),
    ("GS", "Atlantic/South_Georgia"),
    ("GT", "America/Guatemala"),
    ("GU", "Pacific/Guam"),
    ("GW", "Africa/Bissau"),
    ("GY", "America/Guyana"),
    ("HK", "Asia/Hong_Kong"),
    ("HN", "America/Tegucigalpa"),
    ("HR", "Europe/Zagreb"),
    ("HT", "America/Port-au-Prince"),
    ("HU", "Europe/Budapest"),
    ("ID", "Asia/Jakarta"),
    ("IE", "Europe/Dublin"),
    ("IL", "Asia/Jerusalem"),
    ("IM", "Europe/Isle_of_Man"),
    ("IN", "Asia/Kolkata"),
    ("IO", "Indian/Chagos"),
    ("IQ", "Asia/Baghdad"),
    ("IR", "Asia/Tehran"),
    ("IS", "Atlantic/Reykjavik"),
    ("IT", "Europe/Rome"),
    ("JE", "Europe/Jersey"),
    ("JM", "America/Jamaica"),
    ("JO", "Asia/Amman"),
    ("JP", "Asia/Tokyo"),
    ("KE", "Africa/Nairobi"),
    ("KG", "Asia/Bishkek"),
    ("KH", "Asia/Phnom_Penh"),
    ("KI", "Pacific/Tarawa"),
    ("KM", "Indian/Comoro"),
    ("KN", "America/St_Kitts"),
    ("KP", "Asia/Pyongyang"),
    ("KR", "Asia/Seoul"),
    ("KW", "Asia/Kuwait"),
    ("KY", "America/Cayman"),
    ("KZ", "Asia/Almaty"),
    ("LA", "Asia/Vientiane"),
    ("LB", "Asia/Beirut"),
    ("LC", "America/St_Lucia"),
    ("LI", "Europe/Vaduz"),
    ("LK", "Asia/Colombo"),
    ("LR", "Africa/Monrovia"),
    ("LS", "Africa/Maseru"),
    ("LT", "Europe/Vilnius"),
    ("LU", "Europe/Luxembourg"),
    ("LV", "Europe/Riga"),
    ("LY", "Africa/Tripoli"),
    ("MA", "Africa/Casablanca"),
    ("MC", "Europe/Monaco"),
    ("MD", "Europe/Chisinau"),
    ("ME", "Europe/Podgorica"),
    ("MF", "America/Marigot"),
    ("MG", "Indian/Antananarivo"),
    ("MH", "Pacific/Majuro"),
    ("MK", "Europe/Skopje"),
    ("ML", "Africa/Bamako"),
    ("MM", "Asia/Yangon"),
    ("MN", "Asia/Ulaanbaatar"),
    ("MO", "Asia/Macau"),
    ("MP", "Pacific/Saipan"),
    ("MQ", "America/Martinique"),
    ("MR", "Africa/Nouakchott"),
    ("MS", "America/Montserrat"),
    ("MT", "Europe/Malta"),
    ("MU", "Indian/Mauritius"),
    ("MV", "Indian/Maldives"),
    ("MW", "Africa/Blantyre"),
    ("MX", "America/Mexico_City"),
    ("MY", "Asia/Kuala_Lumpur"),
    ("MZ", "Africa/Maputo"),
    ("NA", "Africa/Windhoek"),
    ("NC", "Pacific/Noumea"),
    ("NE", "Africa/Niamey"),
    ("NF", "Pacific/Norfolk"),
    ("NG", "Africa/Lagos"),
    ("NI", "America/Managua"),
    ("NL", "Europe/Amsterdam"),
    ("NO", "Europe/Oslo"),
    ("NP", "Asia/Kathmandu"),
    ("NR", "Pacific/Nauru"),
    ("NU", "Pacific/Niue"),
    ("NZ", "Pacific/Auckland"),
    ("OM", "Asia/Muscat"),
    ("PA", "America/Panama"),
    ("PE", "America/Lima"),
    ("PF", "Pacific/Tahiti"),
    ("PG", "Pacific/Port_Moresby"),
    ("PH", "Asia/Manila"),
    ("PK", "Asia/Karachi"),
    ("PL", "Europe/Warsaw"),
    ("PM", "America/Miquelon"),
    ("PN", "Pacific/Pitcairn"),
    ("PR", "America/Puerto_Rico"),
    ("PS", "Asia/Gaza"),
    ("PT", "Europe/Lisbon"),
    ("PW", "Pacific/Palau"),
    ("PY", "America/Asuncion"),
    ("QA", "Asia/Qatar"),
    ("RE", "Indian/Reunion"),
    ("RO", "Europe/Bucharest"),
    ("RS", "Europe/Belgrade"),
    ("RU", "Europe/Moscow"), // zone.tab lists Europe/Kaliningrad first
    ("RW", "Africa/Kigali"),
    ("SA", "Asia/Riyadh"),
    ("SB", "Pacific/Guadalcanal"),
    ("SC", "Indian/Mahe"),
    ("SD", "Africa/Khartoum"),
    ("SE", "Europe/Stockholm"),
    ("SG", "Asia/Singapore"),
    ("SH", "Atlantic/St_Helena"),
    ("SI", "Europe/Ljubljana"),
    ("SJ", "Arctic/Longyearbyen"),
    ("SK", "Europe/Bratislava"),
    ("SL", "Africa/Freetown"),
    ("SM", "Europe/San_Marino"),
    ("SN", "Africa/Dakar"),
    ("SO", "Africa/Mogadishu"),
    ("SR", "America/Paramaribo"),
    ("SS", "Africa/Juba"),
    ("ST", "Africa/Sao_Tome"),
    ("SV", "America/El_Salvador"),
    ("SX", "America/Lower_Princes"),
    ("SY", "Asia/Damascus"),
    ("SZ", "Africa/Mbabane"),
    ("TC", "America/Grand_Turk"),
    ("TD", "Africa/Ndjamena"),
    ("TF", "Indian/Kerguelen"),
    ("TG", "Africa/Lome"),
    ("TH", "Asia/Bangkok"),
    ("TJ", "Asia/Dushanbe"),
    ("TK", "Pacific/Fakaofo"),
    ("TL", "Asia/Dili"),
    ("TM", "Asia/Ashgabat"),
    ("TN", "Africa/Tunis"),
    ("TO", "Pacific/Tongatapu"),
    ("TR", "Europe/Istanbul"),
    ("TT", "America/Port_of_Spain"),
    ("TV", "Pacific/Funafuti"),
    ("TW", "Asia/Taipei"),
    ("TZ", "Africa/Dar_es_Salaam"),
    ("UA", "Europe/Kyiv"), // zone.tab lists Europe/Simferopol first
    ("UG", "Africa/Kampala"),
    ("UM", "Pacific/Midway"),
    ("US", "America/New_York"),
    ("UY", "America/Montevideo"),
    ("UZ", "Asia/Tashkent"), // zone.tab lists Asia/Samarkand first
    ("VA", "Europe/Vatican"),
    ("VC", "America/St_Vincent"),
    ("VE", "America/Caracas"),
    ("VG", "America/Tortola"),
    ("VI", "America/St_Thomas"),
    ("VN", "Asia/Ho_Chi_Minh"),
    ("VU", "Pacific/Efate"),
    ("WF", "Pacific/Wallis"),
    ("WS", "Pacific/Apia"),
    ("YE", "Asia/Aden"),
    ("YT", "Indian/Mayotte"),
    ("ZA", "Africa/Johannesburg"),
    ("ZM", "Africa/Lusaka"),
    ("ZW", "Africa/Harare"),
];

// -------------------------------------------------------------------------
// Keyboard layouts

/// A named variant of a keyboard layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyboardVariant {
    /// The XKB variant (`dvorak`).
    pub variant: String,
    /// XKB's own name for it (`English (Dvorak)`).
    pub description: String,
}

/// An XKB keyboard layout and its named variants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyboardLayout {
    /// The XKB layout (`pl`).
    pub layout: String,
    /// XKB's own name for it (`Polish`).
    pub description: String,
    /// The named variants, in the file's order. The layout on its own (the
    /// empty variant) is not among them: it is the layout itself, and the
    /// interface offers it first as the legacy installer did.
    pub variants: Vec<KeyboardVariant>,
}

/// The keyboard layouts XKB knows under `root`, from
/// `usr/share/X11/xkb/rules/evdev.xml` (or `base.xml` when evdev.xml is
/// missing or names none), sorted by description as the legacy installer
/// sorted them. XKB's placeholder `custom` layout is left out.
pub fn keyboard_layouts(root: &Path) -> Vec<KeyboardLayout> {
    let rules = root.join("usr/share/X11/xkb/rules");
    for file in ["evdev.xml", "base.xml"] {
        if let Ok(text) = std::fs::read_to_string(rules.join(file)) {
            let layouts = parse_xkb_layouts(&text);
            if !layouts.is_empty() {
                return layouts;
            }
        }
    }
    Vec::new()
}

/// The layouts in the text of an XKB rules registry (`evdev.xml`). Pure.
///
/// The registry is plain XML of one known shape, so a small reader is
/// enough and no XML crate is needed: it understands elements, comments,
/// the declaration and DOCTYPE, CDATA, quoted attributes (which may hold a
/// `>`), and the five named entities and numeric character references. A
/// layout's name and description are those of `layoutList/layout/configItem`;
/// a variant's are those of `layout/variantList/variant/configItem`, so the
/// names inside models, options or variants are never taken for a layout's.
fn parse_xkb_layouts(xml: &str) -> Vec<KeyboardLayout> {
    let mut layouts = Vec::new();
    let mut stack: Vec<&str> = Vec::new();
    let mut text = String::new();
    let mut layout: Option<KeyboardLayout> = None;
    let mut variant: Option<KeyboardVariant> = None;
    for token in xml_tokens(xml) {
        match token {
            Token::Open(name) => {
                text.clear();
                let in_layout_list = stack.last() == Some(&"layoutList");
                let in_variant_list = stack.ends_with(&["layout", "variantList"]);
                stack.push(name);
                if name == "layout" && in_layout_list {
                    layout = Some(KeyboardLayout {
                        layout: String::new(),
                        description: String::new(),
                        variants: Vec::new(),
                    });
                } else if name == "variant" && in_variant_list {
                    variant = Some(KeyboardVariant {
                        variant: String::new(),
                        description: String::new(),
                    });
                }
            }
            Token::Text(chunk) => text.push_str(&chunk),
            Token::Close(name) => {
                let value = text.trim();
                if stack.ends_with(&["layoutList", "layout", "configItem", name]) {
                    if let Some(layout) = layout.as_mut() {
                        match name {
                            "name" => layout.layout = value.to_string(),
                            "description" => layout.description = value.to_string(),
                            _ => {}
                        }
                    }
                } else if stack.ends_with(&["variantList", "variant", "configItem", name]) {
                    if let Some(variant) = variant.as_mut() {
                        match name {
                            "name" => variant.variant = value.to_string(),
                            "description" => variant.description = value.to_string(),
                            _ => {}
                        }
                    }
                } else if name == "variant" && stack.ends_with(&["layout", "variantList", name]) {
                    if let (Some(mut done), Some(layout)) = (variant.take(), layout.as_mut()) {
                        if !done.variant.is_empty() {
                            if done.description.is_empty() {
                                done.description = done.variant.clone();
                            }
                            layout.variants.push(done);
                        }
                    }
                } else if name == "layout" && stack.ends_with(&["layoutList", name]) {
                    if let Some(mut done) = layout.take() {
                        if !done.layout.is_empty() && done.layout != "custom" {
                            if done.description.is_empty() {
                                done.description = done.layout.clone();
                            }
                            layouts.push(done);
                        }
                    }
                }
                text.clear();
                // A stray closing tag closes back to its opening one, and
                // one that was never opened is ignored, so a damaged file
                // loses at most the element it damaged.
                if let Some(at) = stack.iter().rposition(|open| *open == name) {
                    stack.truncate(at);
                }
            }
        }
    }
    layouts.sort_by_cached_key(|layout| (layout.description.to_lowercase(), layout.layout.clone()));
    layouts
}

#[derive(Debug, PartialEq, Eq)]
enum Token<'a> {
    Open(&'a str),
    Close(&'a str),
    Text(String),
}

/// The elements and text of an XML document, in order. An empty element
/// (`<a/>`) is an opening and a closing; comments, the declaration and the
/// DOCTYPE are skipped. Reading stops at the first construct left open.
fn xml_tokens(xml: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut rest = xml;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            tokens.push(Token::Text(decode_entities(rest)));
            break;
        };
        if lt > 0 {
            tokens.push(Token::Text(decode_entities(&rest[..lt])));
        }
        rest = &rest[lt..];
        if let Some(after) = rest.strip_prefix("<!--") {
            let Some(end) = after.find("-->") else { break };
            rest = &after[end + 3..];
        } else if let Some(after) = rest.strip_prefix("<![CDATA[") {
            let Some(end) = after.find("]]>") else { break };
            tokens.push(Token::Text(after[..end].to_string()));
            rest = &after[end + 3..];
        } else if let Some(after) = rest.strip_prefix("<?") {
            let Some(end) = after.find("?>") else { break };
            rest = &after[end + 2..];
        } else if rest.starts_with("<!") {
            // A DOCTYPE, whose internal subset in brackets may hold `>`.
            let Some(end) = markup_end(rest, true) else {
                break;
            };
            rest = &rest[end + 1..];
        } else if let Some(after) = rest.strip_prefix("</") {
            let Some(end) = after.find('>') else { break };
            tokens.push(Token::Close(after[..end].trim()));
            rest = &after[end + 1..];
        } else {
            let Some(end) = markup_end(rest, false) else {
                break;
            };
            let inner = &rest[1..end];
            let empty = inner.ends_with('/');
            let inner = inner.strip_suffix('/').unwrap_or(inner);
            let name_end = inner
                .find(|c: char| c.is_whitespace())
                .unwrap_or(inner.len());
            let name = &inner[..name_end];
            if !name.is_empty() {
                tokens.push(Token::Open(name));
                if empty {
                    tokens.push(Token::Close(name));
                }
            }
            rest = &rest[end + 1..];
        }
    }
    tokens
}

/// Where the markup that starts `text` ends: its `>`, skipping any inside
/// quotes and, for a DOCTYPE, inside its bracketed subset.
fn markup_end(text: &str, brackets: bool) -> Option<usize> {
    let mut quote = None;
    let mut depth = 0usize;
    for (at, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '[') if brackets => depth += 1,
            (None, ']') if brackets => depth = depth.saturating_sub(1),
            (None, '>') if depth == 0 => return Some(at),
            _ => {}
        }
    }
    None
}

/// XML's five named entities and numeric references; anything else that
/// starts with `&` is left as it is written.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let decoded = rest.find(';').and_then(|semi| {
            let entity = &rest[1..semi];
            let c = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => {
                    let number = entity.strip_prefix('#')?;
                    let code = match number.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => number.parse().ok()?,
                    };
                    char::from_u32(code)
                }
            };
            c.map(|c| (c, semi))
        });
        match decoded {
            Some((c, semi)) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The console keymap for an XKB layout and variant, as the legacy
/// installer chose it (`keyboard_layout_widget.py`): the first row of
/// systemd's `kbd-model-map` under `root` for exactly this layout and
/// variant, else its first row for the layout alone, else the layout's own
/// name when kbd has a keymap of that name, else `us`. A console keymap that
/// does not exist would leave the console without one.
pub fn console_keymap(root: &Path, layout: &str, variant: &str) -> String {
    let map =
        std::fs::read_to_string(root.join("usr/share/systemd/kbd-model-map")).unwrap_or_default();
    if let Some(keymap) = kbd_model_map_keymap(&map, layout, variant) {
        return keymap;
    }
    if keymap_token_ok(layout) && keymap_exists(&root.join("usr/share/kbd/keymaps"), layout, 0) {
        return layout.to_string();
    }
    "us".to_string()
}

/// The keymap `kbd-model-map` gives an XKB layout and variant: the first
/// row for the exact pair, else the first row for the layout with no
/// variant, else the first row for the layout with any variant. The last
/// step is what gives Switzerland a Swiss console: the map has rows for `ch`
/// only with a variant (`fr_CH` → `ch`/`fr`), and without it `de_CH` would
/// fall through to a US keymap. Only a row's first layout and variant count
/// (`mk,us` is `mk`), and `-` is no variant.
fn kbd_model_map_keymap(map: &str, layout: &str, variant: &str) -> Option<String> {
    let mut base = None;
    let mut any = None;
    for line in map.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 || !keymap_token_ok(parts[0]) {
            continue;
        }
        let row_layout = parts[1].split(',').next().unwrap_or_default();
        let row_variant = match parts[3].split(',').next().unwrap_or_default() {
            "-" => "",
            other => other,
        };
        if row_layout != layout {
            continue;
        }
        if row_variant == variant {
            return Some(parts[0].to_string());
        }
        if row_variant.is_empty() && base.is_none() {
            base = Some(parts[0].to_string());
        }
        if any.is_none() {
            any = Some(parts[0].to_string());
        }
    }
    base.or(any)
}

/// What `config::validate_region` accepts for a console keymap.
fn keymap_token_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 64
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-+.".contains(&c))
}

/// Whether a keymap file named `name` (`de.map.gz`, `de.map`…) is anywhere
/// below `dir`, as `localectl list-keymaps` would list it: the `include`
/// directories hold fragments, not keymaps. Symbolic links are taken as
/// files and never followed into, so a loop cannot trap the walk.
fn keymap_exists(dir: &Path, name: &str, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if file_name != "include" && keymap_exists(&entry.path(), name, depth + 1) {
                return true;
            }
        } else if keymap_stem(file_name) == Some(name) {
            return true;
        }
    }
    false
}

fn keymap_stem(file_name: &str) -> Option<&str> {
    let plain = [".gz", ".bz2", ".xz", ".zst"]
        .iter()
        .find_map(|suffix| file_name.strip_suffix(suffix))
        .unwrap_or(file_name);
    plain.strip_suffix(".map")
}

/// The XKB layout and variant a person speaking `locale` most likely types
/// on, as `(layout, variant)`: the country's layout (`de_CH` is Swiss,
/// `pt_BR` Brazilian), with the exceptions languages make (French in Canada,
/// Spanish in Latin America, the Indian scripts), and `us` where people
/// mostly type on a US keyboard or nothing else is known. The interface
/// checks the pair against [`keyboard_layouts`] and falls back to the layout
/// alone, then to `us`.
pub fn guess_keyboard(locale: &str) -> (String, String) {
    let (language, country) = locale_parts(locale);
    let country = country.map(str::to_ascii_uppercase).unwrap_or_default();
    let pair = |layout: &str, variant: &str| (layout.to_string(), variant.to_string());
    match (language, country.as_str()) {
        ("en", "GB") => return pair("gb", ""),
        ("en", "IE") => return pair("ie", ""),
        ("en", _) => return pair("us", ""),
        ("fr", "CA") => return pair("ca", ""),
        ("fr", "CH") => return pair("ch", "fr"),
        ("es", country) if !country.is_empty() && country != "ES" => return pair("latam", ""),
        ("mi", _) => return pair("nz", "mao"),
        ("ar", _) => return pair("ara", ""),
        ("hi", _) => return pair("in", ""),
        ("gu", _) => return pair("in", "guj"),
        ("kn", _) => return pair("in", "kan"),
        ("ml", _) => return pair("in", "mal"),
        ("ta", _) => return pair("in", "tam"),
        ("te", _) => return pair("in", "tel"),
        _ => {}
    }
    let by_country = COUNTRY_LAYOUTS
        .iter()
        .find(|(code, _)| *code == country)
        .map(|(_, layout)| *layout);
    let by_language = LANGUAGE_LAYOUTS
        .iter()
        .find(|(code, _)| *code == language)
        .map(|(_, layout)| *layout);
    pair(by_country.or(by_language).unwrap_or("us"), "")
}

/// Countries whose people type on their own XKB layout. Countries left out
/// (the Netherlands, Indonesia, Malaysia, the Philippines, Somalia, South
/// Africa among them) mostly type on a US keyboard.
static COUNTRY_LAYOUTS: &[(&str, &str)] = &[
    ("AL", "al"),
    ("AT", "at"),
    ("BA", "ba"),
    ("BE", "be"),
    ("BG", "bg"),
    ("BR", "br"),
    ("BY", "by"),
    ("CH", "ch"),
    ("CN", "cn"),
    ("CZ", "cz"),
    ("DE", "de"),
    ("DK", "dk"),
    ("EE", "ee"),
    ("ES", "es"),
    ("FI", "fi"),
    ("FR", "fr"),
    ("GB", "gb"),
    ("GE", "ge"),
    ("GR", "gr"),
    ("HR", "hr"),
    ("HU", "hu"),
    ("IE", "ie"),
    ("IL", "il"),
    ("IN", "in"),
    ("IR", "ir"),
    ("IS", "is"),
    ("IT", "it"),
    ("JP", "jp"),
    ("KH", "kh"),
    ("KR", "kr"),
    ("LA", "la"),
    ("LT", "lt"),
    ("LV", "lv"),
    ("MN", "mn"),
    ("NO", "no"),
    ("PL", "pl"),
    ("PT", "pt"),
    ("RO", "ro"),
    ("RS", "rs"),
    ("RU", "ru"),
    ("SE", "se"),
    ("SI", "si"),
    ("SK", "sk"),
    ("TH", "th"),
    ("TR", "tr"),
    ("TW", "tw"),
    ("UA", "ua"),
    ("US", "us"),
    ("VN", "vn"),
];

/// For a locale without a country (or one not listed above): the layout of
/// the language's own country.
static LANGUAGE_LAYOUTS: &[(&str, &str)] = &[
    ("be", "by"),
    ("bg", "bg"),
    ("cs", "cz"),
    ("da", "dk"),
    ("de", "de"),
    ("el", "gr"),
    ("es", "es"),
    ("et", "ee"),
    ("fa", "ir"),
    ("fi", "fi"),
    ("fr", "fr"),
    ("he", "il"),
    ("hr", "hr"),
    ("hu", "hu"),
    ("it", "it"),
    ("ja", "jp"),
    ("ka", "ge"),
    ("ko", "kr"),
    ("lt", "lt"),
    ("lv", "lv"),
    ("nb", "no"),
    ("nn", "no"),
    ("pl", "pl"),
    ("pt", "pt"),
    ("ro", "ro"),
    ("ru", "ru"),
    ("sk", "sk"),
    ("sl", "si"),
    ("sq", "al"),
    ("sr", "rs"),
    ("sv", "se"),
    ("th", "th"),
    ("tr", "tr"),
    ("uk", "ua"),
    ("vi", "vn"),
];

// -------------------------------------------------------------------------
// Desktops

/// A desktop the installer offers, and whether the image already carries
/// it (so it installs without the internet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesktopOffer {
    pub desktop: Desktop,
    pub in_image: bool,
}

/// The three desktops, always in the order Linexin, Kinexin, LineXinBar,
/// each marked by whether the image at `image` has its packages: Linexin
/// when `gdm` and `gnome-shell` are installed, Kinexin when
/// `kinexin-desktop` and `cedm` (its login screen) are, LineXinBar when
/// `cedm` is and either `lxb` or both `lxb-compositor` and `lxb-desktop` are.
pub fn desktops(image: &Path) -> Vec<DesktopOffer> {
    let installed = installed_packages(image);
    let has = |name: &str| installed.contains(name);
    vec![
        DesktopOffer {
            desktop: Desktop::Linexin,
            in_image: has("gdm") && has("gnome-shell"),
        },
        DesktopOffer {
            desktop: Desktop::Kinexin,
            in_image: has("kinexin-desktop") && has("cedm"),
        },
        DesktopOffer {
            desktop: Desktop::LineXinBar,
            in_image: has("cedm") && (has("lxb") || (has("lxb-compositor") && has("lxb-desktop"))),
        },
    ]
}

/// The packages installed in the image, from the names of pacman's local
/// database directories (`var/lib/pacman/local/<name>-<version>-<rel>/`).
/// Read from the directory names alone, as pacman itself lists them, so no
/// file inside needs to be opened.
fn installed_packages(image: &Path) -> BTreeSet<String> {
    let Ok(entries) = std::fs::read_dir(image.join("var/lib/pacman/local")) else {
        return BTreeSet::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| package_name(entry.file_name().to_str()?).map(str::to_string))
        .collect()
}

/// The package name in a local database entry: a package name may hold
/// hyphens, its version and release never do, so the name is all before
/// the second hyphen from the end (`gnome-shell` from
/// `gnome-shell-1:48.1-1`).
fn package_name(entry: &str) -> Option<&str> {
    let mut parts = entry.rsplitn(3, '-');
    let release = parts.next()?;
    let version = parts.next()?;
    let name = parts.next()?;
    (!name.is_empty() && !version.is_empty() && !release.is_empty()).then_some(name)
}

// -------------------------------------------------------------------------
// Flatpak applications

/// A Flatpak application offered from Flathub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlatpakApp {
    /// The Flathub application ID.
    pub id: &'static str,
    /// The application's own name.
    pub name: &'static str,
}

/// The legacy installer's seven Flatpak applications
/// (`de_picker_widget.py`), in its order.
pub fn flatpaks() -> &'static [FlatpakApp] {
    &FLATPAKS
}

static FLATPAKS: [FlatpakApp; 7] = [
    FlatpakApp {
        id: "app.zen_browser.zen",
        name: "Zen Browser",
    },
    FlatpakApp {
        id: "io.github.Faugus.faugus-launcher",
        name: "Faugus Launcher",
    },
    FlatpakApp {
        id: "it.mijorus.gearlever",
        name: "Gear Lever",
    },
    FlatpakApp {
        id: "com.github.tchx84.Flatseal",
        name: "Flatseal",
    },
    FlatpakApp {
        id: "com.usebottles.bottles",
        name: "Bottles",
    },
    FlatpakApp {
        id: "app.twintaillauncher.ttl",
        name: "Twin Tail Launcher",
    },
    FlatpakApp {
        id: "com.heroicgameslauncher.hgl",
        name: "Heroic Games Launcher",
    },
];

// -------------------------------------------------------------------------
// The image's packages

/// The groups the legacy installer sorted removable packages into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageCategory {
    Multimedia,
    Fonts,
    Network,
    Libraries,
    Applications,
}

/// A package of the image a person may choose to leave out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    /// pacman's description of it, as the package wrote it.
    pub description: String,
    pub category: PackageCategory,
}

/// The removable packages in the text of `pacman -Qei` (run in the C
/// locale), in pacman's order. Pure.
///
/// Hidden, as in the legacy installer, are the packages a system cannot
/// start or be managed without, boot loaders, the desktops (the desktop
/// step owns them), the display server's base and GPU drivers (the driver
/// pruning owns those). Hidden besides are the family's own shell and login
/// screen and both installers, kernels and their initramfs builders,
/// firmware and microcode, and the rest of the GPU stack, all of which a
/// person removing would find out about only when the system did not start.
pub fn parse_packages(text: &str) -> Vec<Package> {
    struct Entry {
        name: String,
        description: String,
        groups: String,
    }
    fn finish(entry: Option<Entry>, packages: &mut Vec<Package>) {
        let Some(entry) = entry else { return };
        if let Some(category) = categorize(&entry.name, &entry.groups) {
            packages.push(Package {
                name: entry.name,
                description: entry.description,
                category,
            });
        }
    }
    let mut packages = Vec::new();
    let mut entry: Option<Entry> = None;
    let mut last_key = "";
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) {
            // pacman wraps a long field onto indented lines; only a
            // description's continuation is worth keeping.
            let more = line.trim();
            if let (Some(entry), "Description", false) = (entry.as_mut(), last_key, more.is_empty())
            {
                entry.description.push(' ');
                entry.description.push_str(more);
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            last_key = "";
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        last_key = match key {
            "Name" => {
                finish(entry.take(), &mut packages);
                entry = Some(Entry {
                    name: value.to_string(),
                    description: String::new(),
                    groups: String::new(),
                });
                "Name"
            }
            "Description" => {
                if let Some(entry) = entry.as_mut() {
                    entry.description = value.to_string();
                }
                "Description"
            }
            "Groups" => {
                if let Some(entry) = entry.as_mut() {
                    entry.groups = if value == "None" {
                        String::new()
                    } else {
                        value.to_string()
                    };
                }
                "Groups"
            }
            _ => "",
        };
    }
    finish(entry, &mut packages);
    packages
}

/// The legacy installer's list of packages that can never be left out.
const ESSENTIAL_PACKAGES: &[&str] = &[
    "base",
    "linux",
    "linux-headers",
    "linux-firmware",
    "linux-api-headers",
    "grub",
    "efibootmgr",
    "systemd",
    "systemd-libs",
    "systemd-sysvcompat",
    "pacman",
    "glibc",
    "bash",
    "sudo",
    "filesystem",
    "mkinitcpio",
    "dbus",
    "shadow",
    "util-linux",
    "coreutils",
    "gcc-libs",
    "glib2",
    "iana-etc",
    "tzdata",
    "keyutils",
    "libcap",
    "openssl",
    "zlib",
    "xz",
    "bzip2",
    "gzip",
    "tar",
    "findutils",
    "grep",
    "sed",
    "gawk",
    "procps-ng",
    "psmisc",
    "e2fsprogs",
    "dosfstools",
    "btrfs-progs",
    "iproute2",
    "iputils",
    "kbd",
];

/// The category a package is listed under, or `None` when it is hidden.
/// The legacy rules (`de_picker_widget.py`, `_categorize_package`) in their
/// order, with the family's additions before them.
fn categorize(name: &str, groups: &str) -> Option<PackageCategory> {
    let n = name.to_lowercase();
    let g = groups.to_lowercase();
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| n.starts_with(p));
    let contains = |parts: &[&str]| parts.iter().any(|p| n.contains(p));

    if ESSENTIAL_PACKAGES.contains(&name) {
        return None;
    }
    // The family's shell, login screen and installers.
    if n.starts_with("lxb")
        || matches!(n.as_str(), "cedm" | "linexin-installer")
        || n.starts_with("greetd")
    {
        return None;
    }
    // Kernels, their headers and modules, and what builds their initramfs.
    if n == "linux" || starts(&["linux-", "mkinitcpio", "dracut", "booster"]) {
        return None;
    }
    // Firmware and CPU microcode (the driver pruning owns microcode).
    if contains(&["firmware"]) || n.ends_with("-ucode") {
        return None;
    }
    // Boot loaders (the legacy list, and the others a system may start by).
    if contains(&[
        "grub",
        "refind",
        "efibootmgr",
        "os-prober",
        "shim",
        "limine",
        "syslinux",
        "memtest86",
    ]) {
        return None;
    }
    // Desktops: the desktop step owns them.
    if g.contains("gnome")
        || n.starts_with("gnome-")
        || matches!(n.as_str(), "gdm" | "mutter" | "nautilus")
    {
        return None;
    }
    if g.contains("plasma") || g.contains("kde") || starts(&["plasma-", "kde", "kinexin"]) {
        return None;
    }
    if starts(&["xdg-", "xorg-", "wayland", "libx11", "libxkb"]) {
        return None;
    }
    // GPU drivers and the stack under them.
    if contains(&[
        "nvidia",
        "mesa",
        "vulkan",
        "xf86-",
        "libva",
        "libdrm",
        "vdpau",
        "opencl",
        "libglvnd",
        "rocm",
        "cuda",
        "intel-media",
        "intel-compute",
        "amdgpu",
        "radeon",
    ]) {
        return None;
    }

    if contains(&["pipewire", "wireplumber", "gstreamer", "gst-", "ffmpeg"]) {
        return Some(PackageCategory::Multimedia);
    }
    if starts(&["ttf-", "otf-", "noto-fonts", "adobe-source"]) {
        return Some(PackageCategory::Fonts);
    }
    if contains(&[
        "networkmanager",
        "firewall",
        "openssh",
        "bluez",
        "bluetooth",
    ]) {
        return Some(PackageCategory::Network);
    }
    if starts(&["python-", "lib", "perl-"]) {
        return Some(PackageCategory::Libraries);
    }
    Some(PackageCategory::Applications)
}

/// The removable packages of the running system (the live image), from
/// `pacman -Qei`. Read-only: pacman only reads its database. The error is a
/// line for the log, never shown as it is.
pub fn removable_packages() -> Result<Vec<Package>, String> {
    // Started here rather than through `run`, so in a test build it asks the
    // fence itself, before pacman is even looked up, and is refused: no test
    // reads this computer's packages (see `run::fence`).
    #[cfg(test)]
    crate::worker::run::fence::check("pacman", &["-Qei", "--color", "never"]);
    let program = crate::worker::run::command_path("pacman").map_err(|stop| stop.detail)?;
    let output = Command::new(program)
        .args(["-Qei", "--color", "never"])
        .env_clear()
        .env("PATH", crate::worker::run::SAFE_PATH)
        // pacman translates its field names; the parser reads the English.
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("pacman could not start: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "pacman -Qei failed: {} {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(parse_packages(&String::from_utf8_lossy(&output.stdout)))
}

// -------------------------------------------------------------------------
// The internet

/// Whether the internet can be reached: NetworkManager says the machine's
/// connectivity is `full`, or, when it says otherwise or cannot be asked,
/// a TCP connection to `petexy.github.io:443` opens within 3 seconds.
///
/// It can take several seconds (and resolving the name has no timeout of
/// its own), so callers must run it off the interface's thread and hand the
/// answer back.
pub fn online() -> bool {
    nm_connectivity_full() || reaches("petexy.github.io:443", Duration::from_secs(3))
}

fn nm_connectivity_full() -> bool {
    // Started here rather than through `run`, so in a test build it asks the
    // fence itself, before nmcli is even looked up, and is refused: no test
    // asks this computer's network, nor the worker's check of the internet
    // before writing (see `run::fence`).
    #[cfg(test)]
    crate::worker::run::fence::check("nmcli", &["-t", "-f", "CONNECTIVITY", "general"]);
    let Ok(program) = crate::worker::run::command_path("nmcli") else {
        return false;
    };
    let mut command = Command::new(program);
    command
        .args(["-t", "-f", "CONNECTIVITY", "general"])
        .env_clear()
        .env("PATH", crate::worker::run::SAFE_PATH)
        .env("LC_ALL", "C");
    output_within(command, Duration::from_secs(3)).is_some_and(|output| {
        output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "full"
    })
}

/// A command's output when it finishes within `limit`; past that it is
/// killed, so a NetworkManager that does not answer cannot hold the check.
fn output_within(mut command: Command, limit: Duration) -> Option<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let mut stdout = Vec::new();
    child.stdout.take()?.read_to_end(&mut stdout).ok()?;
    Some(Output {
        status,
        stdout,
        stderr: Vec::new(),
    })
}

/// Whether a TCP connection to `address` opens, trying each address the
/// name resolves to within what is left of `limit`.
fn reaches(address: &str, limit: Duration) -> bool {
    // In a test build the fence refuses the connection before the name is
    // even resolved: no test reaches the network (see `run::fence`).
    #[cfg(test)]
    crate::worker::run::fence::network(address);
    let deadline = Instant::now() + limit;
    let Ok(addresses) = address.to_socket_addrs() else {
        return false;
    };
    for socket in addresses {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        if TcpStream::connect_timeout(&socket, left).is_ok() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::{validate_region, Keyboard, Region};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A directory of hand-written files under the system's temporary
    /// directory, removed when the test ends.
    struct FakeRoot(PathBuf);

    impl FakeRoot {
        fn new(name: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "lxb-installer-catalogs-{}-{n}-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            FakeRoot(path)
        }

        fn write(&self, relative: &str, text: &str) {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }

        fn dir(&self, relative: &str) {
            std::fs::create_dir_all(self.0.join(relative)).unwrap();
        }
    }

    impl Drop for FakeRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn names(zones: &[Zone]) -> Vec<&str> {
        zones.iter().map(|z| z.name.as_str()).collect()
    }

    // ---------------------------------------------------------------------
    // Languages

    #[test]
    fn languages_are_the_legacy_list_in_its_order() {
        let list = languages();
        assert_eq!(list.len(), 65);
        assert!(list.windows(2).all(|w| w[0].native < w[1].native));
        assert_eq!(list[0].locale, "af_ZA.UTF-8");
        let locales: BTreeSet<&str> = list.iter().map(|l| l.locale).collect();
        assert_eq!(locales.len(), 65);
        assert!(locales.contains("nb_NO.UTF-8"));
        assert!(!locales.contains("no_NO.UTF-8"));
        for language in list {
            let region = Region {
                locale: language.locale.to_string(),
                ..Region::default()
            };
            assert_eq!(validate_region(&region), Ok(()), "{}", language.locale);
            assert!(!language.english.is_empty() && !language.native.is_empty());
        }
    }

    /// Every language names its country, as the legacy installer read it
    /// off the locale for the flag it wrote before the name.
    #[test]
    fn every_language_names_its_country() {
        for language in languages() {
            let legacy = language
                .locale
                .split('_')
                .nth(1)
                .and_then(|rest| rest.split('.').next());
            assert_eq!(language.country(), legacy, "{}", language.locale);
        }
        let country = |locale: &str| {
            languages()
                .iter()
                .find(|l| l.locale == locale)
                .unwrap_or_else(|| panic!("{locale}"))
                .country()
        };
        assert_eq!(country("pl_PL.UTF-8"), Some("PL"));
        assert_eq!(country("en_GB.UTF-8"), Some("GB"));
        assert_eq!(country("fil_PH.UTF-8"), Some("PH"));
        assert_eq!(country("nb_NO.UTF-8"), Some("NO"));
        assert_eq!(country("ca_ES.UTF-8"), Some("ES"));
        assert_eq!(country("mi_NZ.UTF-8"), Some("NZ"));
        assert_eq!(country("zh_TW.UTF-8"), Some("TW"));
    }

    #[test]
    fn a_locale_naming_no_country_has_none() {
        assert_eq!(territory("sr_RS@latin"), Some("RS"));
        assert_eq!(territory("de_CH"), Some("CH"));
        for none in [
            "eo",
            "eo.UTF-8",
            "C.UTF-8",
            "POSIX",
            "es_419.UTF-8",
            "pl_pl",
            "",
            "_",
        ] {
            assert_eq!(territory(none), None, "{none:?}");
        }
    }

    #[test]
    fn languages_switch_to_the_ten_catalogs() {
        let ui = |locale: &str| {
            languages()
                .iter()
                .find(|l| l.locale == locale)
                .unwrap_or_else(|| panic!("{locale}"))
                .ui
        };
        assert_eq!(ui("pl_PL.UTF-8"), Some("pl"));
        assert_eq!(ui("en_US.UTF-8"), Some("en-US"));
        for english in ["en_GB.UTF-8", "en_AU.UTF-8", "en_CA.UTF-8"] {
            assert_eq!(ui(english), Some("en-GB"));
        }
        assert_eq!(ui("fr_CA.UTF-8"), Some("fr"));
        assert_eq!(ui("pt_BR.UTF-8"), Some("pt-BR"));
        assert_eq!(ui("pt_PT.UTF-8"), None);
        assert_eq!(ui("zh_CN.UTF-8"), Some("zh-CN"));
        assert_eq!(ui("zh_TW.UTF-8"), None);
        assert_eq!(ui("ca_ES.UTF-8"), None);
        let tags: BTreeSet<&str> = languages().iter().filter_map(|l| l.ui).collect();
        let expected: BTreeSet<&str> = [
            "de", "en-GB", "en-US", "es", "fr", "hi", "pl", "pt-BR", "ru", "zh-CN",
        ]
        .into();
        assert_eq!(tags, expected);
    }

    // ---------------------------------------------------------------------
    // Time zones

    const ZONE_TAB: &str = "\
# tzdb timezone descriptions (deprecated version)
#country-
#code\tcoordinates\tTZ\t\t\tcomments
AR\t-3436-05827\tAmerica/Argentina/Buenos_Aires\tBuenos Aires (BA, CF)
NO\t+5955+01045\tEurope/Oslo
PL\t+5215+02100\tEurope/Warsaw
TT\t+1039-06131\tAmerica/Port_of_Spain
US\t+404251-0740023\tAmerica/New_York\tEastern (most areas)
XX\t+0000+00000\tAtlantis/Lost\tlisted, but no file
XX\t+0000+00000\t../../etc/passwd\ta name that would leave the directory
";

    fn zone_root(name: &str) -> FakeRoot {
        let root = FakeRoot::new(name);
        for zone in [
            "America/Argentina/Buenos_Aires",
            "Europe/Oslo",
            "Europe/Warsaw",
            "America/Port_of_Spain",
            "America/New_York",
            "Europe/Berlin",
            "UTC",
        ] {
            root.write(&format!("usr/share/zoneinfo/{zone}"), "TZif");
        }
        root
    }

    #[test]
    fn zones_come_from_zone_tab_with_utc() {
        let root = zone_root("zone-tab");
        root.write("usr/share/zoneinfo/zone.tab", ZONE_TAB);
        root.write(
            "usr/share/zoneinfo/iso3166.tab",
            "# ISO 3166 alpha-2 country codes\n#\nAR\tArgentina\nNO\tNorway\n\
             PL\tPoland\nTT\tTrinidad & Tobago\nUS\tUnited States\n",
        );
        let zones = zones(&root.0);
        assert_eq!(
            names(&zones),
            [
                "America/Argentina/Buenos_Aires",
                "America/New_York",
                "America/Port_of_Spain",
                "Europe/Oslo",
                "Europe/Warsaw",
                "UTC",
            ]
        );
        assert_eq!(
            zones[0],
            Zone {
                name: "America/Argentina/Buenos_Aires".into(),
                region: "America".into(),
                city: "Argentina / Buenos Aires".into(),
                country: "Argentina".into(),
                code: "AR".into(),
                shown_city: "Buenos Aires".into(),
                shown_country: "Argentina".into(),
            }
        );
        assert_eq!(zones[2].city, "Port of Spain");
        assert_eq!(zones[2].country, "Trinidad & Tobago");
        assert_eq!(zones[4].country, "Poland");
        assert_eq!(zones[5].country, "", "UTC keeps no country's clock");
        assert_eq!(
            (zones[5].region.as_str(), zones[5].city.as_str()),
            ("UTC", "")
        );
    }

    #[test]
    fn zones_fall_back_to_zone1970_then_tzdata() {
        let root = zone_root("zone1970");
        root.write(
            "usr/share/zoneinfo/zone1970.tab",
            "# comment\nDE,DK,NO,SE,SJ\t+5230+01322\tEurope/Berlin\tmost of Germany\n\
             PL\t+5215+02100\tEurope/Warsaw\n",
        );
        assert_eq!(
            names(&zones(&root.0)),
            ["Europe/Berlin", "Europe/Warsaw", "UTC"]
        );
        // No names of countries beside them: none is said.
        assert!(zones(&root.0).iter().all(|zone| zone.country.is_empty()));
        root.write(
            "usr/share/zoneinfo/iso3166.tab",
            "DE\tGermany\nPL\tPoland\n",
        );
        assert_eq!(zones(&root.0)[0].country, "Germany", "the first code");

        let root = zone_root("tzdata-zi");
        root.write(
            "usr/share/zoneinfo/tzdata.zi",
            "# version 2026a\n\
             R E 1977 1980 - Ap Su>=1 1u 1 S\n\
             Z Europe/Berlin 0:53:28 - LMT 1893 Ap\n\
             1 c CE%sT\n\
             Z America/New_York -4:56:2 - LMT 1883 N 18 17u\n\
             Z Etc/GMT+5 -5 - -05\n\
             Z EST -5 - EST\n\
             L Europe/Berlin Europe/Copenhagen\n",
        );
        assert_eq!(
            names(&zones(&root.0)),
            ["America/New_York", "Europe/Berlin", "UTC"]
        );

        // With nothing to read, UTC is still there to choose.
        let empty = FakeRoot::new("no-zoneinfo");
        assert_eq!(names(&zones(&empty.0)), ["UTC"]);
    }

    #[test]
    fn the_zone_is_guessed_from_the_locales_country() {
        let zones: Vec<Zone> = [
            "Europe/Warsaw",
            "Australia/Sydney",
            "Australia/Lord_Howe",
            "America/New_York",
            "Europe/Belgrade",
            "UTC",
        ]
        .into_iter()
        .map(zone)
        .collect();
        let guess = |locale: &str| guess_zone(locale, &zones);
        assert_eq!(guess("pl_PL.UTF-8").as_deref(), Some("Europe/Warsaw"));
        assert_eq!(guess("en_US.UTF-8").as_deref(), Some("America/New_York"));
        // Not Lord Howe Island, which zone.tab lists first for Australia.
        assert_eq!(guess("en_AU.UTF-8").as_deref(), Some("Australia/Sydney"));
        assert_eq!(guess("sr_RS@latin").as_deref(), Some("Europe/Belgrade"));
        // A zone the image lacks, a locale with no country, an unknown one.
        assert_eq!(guess("de_DE.UTF-8"), None);
        assert_eq!(guess("C"), None);
        assert_eq!(guess("eo.UTF-8"), None);
        assert_eq!(guess("xx_ZZ.UTF-8"), None);
    }

    #[test]
    fn every_country_zone_is_sorted_and_well_formed() {
        assert!(COUNTRY_ZONES.windows(2).all(|w| w[0].0 < w[1].0));
        for (country, zone) in COUNTRY_ZONES {
            assert!(country.len() == 2 && country.bytes().all(|c| c.is_ascii_uppercase()));
            assert!(zone_name_ok(zone), "{zone}");
        }
        // Every language offered has a country with a zone.
        for language in languages() {
            let country = locale_parts(language.locale).1.unwrap();
            assert!(
                COUNTRY_ZONES.iter().any(|(code, _)| *code == country),
                "{}",
                language.locale
            );
        }
    }

    // ---------------------------------------------------------------------
    // Keyboard layouts

    const EVDEV: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE xkbConfigRegistry SYSTEM "xkb.dtd" [
  <!ENTITY example "a > b">
]>
<xkbConfigRegistry version="1.1">
  <modelList>
    <model>
      <configItem>
        <name>pc105</name>
        <description>Generic 105-key PC</description>
        <vendor>Generic</vendor>
      </configItem>
    </model>
  </modelList>
  <layoutList>
    <layout>
      <configItem>
        <name>us</name>
        <!-- Keyboard indicator for English layouts -->
        <shortDescription>en</shortDescription>
        <description>English (US)</description>
        <countryList>
          <iso3166Id>US</iso3166Id>
        </countryList>
      </configItem>
      <variantList>
        <variant>
          <configItem>
            <name>dvorak</name>
            <description>English (Dvorak)</description>
          </configItem>
        </variant>
        <variant>
          <configItem popularity="exotic" note="a > b">
            <name>intl</name>
            <description>English (US, intl., with dead keys)</description>
          </configItem>
        </variant>
        <variant>
          <configItem>
            <name>nameless-description</name>
          </configItem>
        </variant>
      </variantList>
    </layout>
    <layout>
      <configItem>
        <name>custom</name>
        <description>A user-defined custom Layout</description>
      </configItem>
    </layout>
    <layout>
      <configItem>
        <name>pl</name>
        <description>Polish</description>
      </configItem>
      <variantList/>
    </layout>
    <layout>
      <configItem>
        <name>xx</name>
        <description>&quot;Invented&quot; &amp; &lt;Test&gt; Board &#x41;&#66; &apos;</description>
      </configItem>
    </layout>
    <layout>
      <configItem>
        <name>ch</name>
        <description><![CDATA[German (Switzerland)]]></description>
      </configItem>
      <variantList>
        <variant>
          <configItem>
            <name>fr</name>
            <description>French (Switzerland)</description>
          </configItem>
        </variant>
      </variantList>
    </layout>
  </layoutList>
  <optionList>
    <group allowMultipleSelection="true">
      <configItem>
        <name>grp</name>
        <description>Switching to another layout</description>
      </configItem>
      <option>
        <configItem>
          <name>grp:toggle</name>
          <description>Right Alt</description>
        </configItem>
      </option>
    </group>
  </optionList>
</xkbConfigRegistry>
"#;

    fn variant(variant: &str, description: &str) -> KeyboardVariant {
        KeyboardVariant {
            variant: variant.into(),
            description: description.into(),
        }
    }

    #[test]
    fn layouts_are_read_from_evdev_xml() {
        let root = FakeRoot::new("evdev");
        root.write("usr/share/X11/xkb/rules/evdev.xml", EVDEV);
        root.write(
            "usr/share/X11/xkb/rules/base.xml",
            "<xkbConfigRegistry><layoutList><layout><configItem><name>base</name>\
             </configItem></layout></layoutList></xkbConfigRegistry>",
        );
        let layouts = keyboard_layouts(&root.0);
        let codes: Vec<&str> = layouts.iter().map(|l| l.layout.as_str()).collect();
        // Sorted by description without regard to case; `custom` left out.
        assert_eq!(codes, ["xx", "us", "ch", "pl"]);
        assert_eq!(layouts[0].description, "\"Invented\" & <Test> Board AB '");
        assert_eq!(layouts[1].description, "English (US)");
        assert_eq!(
            layouts[1].variants,
            [
                variant("dvorak", "English (Dvorak)"),
                variant("intl", "English (US, intl., with dead keys)"),
                variant("nameless-description", "nameless-description"),
            ]
        );
        assert_eq!(layouts[2].description, "German (Switzerland)");
        assert_eq!(layouts[2].variants, [variant("fr", "French (Switzerland)")]);
        assert!(layouts[3].variants.is_empty());
    }

    #[test]
    fn base_xml_stands_in_for_a_missing_evdev_xml() {
        let root = FakeRoot::new("base");
        root.write(
            "usr/share/X11/xkb/rules/base.xml",
            "<xkbConfigRegistry><layoutList><layout><configItem><name>de</name>\
             <description>German</description></configItem></layout></layoutList>\
             </xkbConfigRegistry>",
        );
        let layouts = keyboard_layouts(&root.0);
        assert_eq!(layouts.len(), 1);
        assert_eq!(
            (layouts[0].layout.as_str(), layouts[0].description.as_str()),
            ("de", "German")
        );
        // A damaged evdev.xml that names nothing gives way to base.xml too.
        root.write(
            "usr/share/X11/xkb/rules/evdev.xml",
            "<xkbConfigRegistry><layoutL",
        );
        assert_eq!(keyboard_layouts(&root.0).len(), 1);
        assert!(keyboard_layouts(&FakeRoot::new("no-xkb").0).is_empty());
    }

    #[test]
    fn the_xml_reader_skips_what_is_not_an_element() {
        let tokens = xml_tokens(
            "<?xml version=\"1.0\"?><!DOCTYPE r [<!ENTITY e \"]>\">]><r a='x>y'>\
             <!-- <fake> --><e/>t&amp;<![CDATA[<raw>&amp;]]></r>",
        );
        assert_eq!(
            tokens,
            [
                Token::Open("r"),
                Token::Open("e"),
                Token::Close("e"),
                Token::Text("t&".into()),
                Token::Text("<raw>&amp;".into()),
                Token::Close("r"),
            ]
        );
        assert_eq!(
            decode_entities("&amp;&lt;&gt;&quot;&apos;&#65;&#x42;&bogus; & x&#xFFFFFFFF;"),
            "&<>\"'AB&bogus; & x&#xFFFFFFFF;"
        );
        // Something left open ends the reading without a panic.
        assert_eq!(xml_tokens("<a><!-- open"), [Token::Open("a")]);
        assert_eq!(xml_tokens("<a b=\"open>"), []);
    }

    const KBD_MODEL_MAP: &str = "\
# consolelayout\t\txlayout\txmodel\t\txvariant\txoptions\tbcp47
sg\t\t\tch\tpc105\t\tde_nodeadkeys\tterminate:ctrl_alt_bksp\tde-CH
sg-latin1\t\tch\tpc105\t\t-\tterminate:ctrl_alt_bksp
us\t\t\tus\tpc105\t\t-\tterminate:ctrl_alt_bksp
dvorak\t\t\tus\tpc105\t\tdvorak\tterminate:ctrl_alt_bksp
pl2\t\t\tpl\tpc105\t\t-\tterminate:ctrl_alt_bksp
pl\t\t\tpl\tpc105\t\t-\tterminate:ctrl_alt_bksp
mk-utf\t\t\tmk,us\tpc105\t\t-\tterminate:ctrl_alt_bksp,grp:shifts_toggle
bad;name\t\tzz\tpc105\t\t-\tterminate:ctrl_alt_bksp
short\tline
";

    #[test]
    fn the_console_keymap_follows_kbd_model_map_then_kbd() {
        let root = FakeRoot::new("kbd");
        root.write("usr/share/systemd/kbd-model-map", KBD_MODEL_MAP);
        root.write("usr/share/kbd/keymaps/i386/qwertz/de.map.gz", "");
        root.write("usr/share/kbd/keymaps/i386/azerty/fr-latin9.map.gz", "");
        root.write("usr/share/kbd/keymaps/i386/include/fr.map", "");
        let keymap = |layout: &str, variant: &str| console_keymap(&root.0, layout, variant);
        // The first row for the exact pair; the first variant-less row.
        assert_eq!(keymap("ch", "de_nodeadkeys"), "sg");
        assert_eq!(keymap("pl", ""), "pl2");
        assert_eq!(keymap("us", "dvorak"), "dvorak");
        // A variant without a row of its own takes the layout's.
        assert_eq!(keymap("ch", "fr"), "sg-latin1");
        assert_eq!(keymap("pl", "dvorak"), "pl2");
        // A row's first layout is the one that counts.
        assert_eq!(keymap("mk", ""), "mk-utf");
        // No row: a keymap of the layout's name, but not a fragment from
        // an include directory and not a malformed row.
        assert_eq!(keymap("de", "nodeadkeys"), "de");
        assert_eq!(keymap("fr", ""), "us");
        assert_eq!(keymap("zz", ""), "us");
        assert_eq!(keymap("../i386", ""), "us");
        // With neither file, `us`.
        assert_eq!(console_keymap(&FakeRoot::new("no-kbd").0, "de", ""), "us");
    }

    #[test]
    fn the_keyboard_is_guessed_from_the_locale() {
        let guess = |locale: &str| {
            let (layout, variant) = guess_keyboard(locale);
            format!("{layout}:{variant}")
        };
        assert_eq!(guess("pl_PL.UTF-8"), "pl:");
        assert_eq!(guess("de_CH.UTF-8"), "ch:");
        assert_eq!(guess("en_GB.UTF-8"), "gb:");
        assert_eq!(guess("en_US.UTF-8"), "us:");
        assert_eq!(guess("en_CA.UTF-8"), "us:");
        assert_eq!(guess("fr_CA.UTF-8"), "ca:");
        assert_eq!(guess("fr_CH.UTF-8"), "ch:fr");
        assert_eq!(guess("pt_BR.UTF-8"), "br:");
        assert_eq!(guess("es_MX.UTF-8"), "latam:");
        assert_eq!(guess("es_ES.UTF-8"), "es:");
        assert_eq!(guess("ca_ES.UTF-8"), "es:");
        assert_eq!(guess("nb_NO.UTF-8"), "no:");
        assert_eq!(guess("ta_IN.UTF-8"), "in:tam");
        assert_eq!(guess("mi_NZ.UTF-8"), "nz:mao");
        assert_eq!(guess("nl_NL.UTF-8"), "us:");
        assert_eq!(guess("sr_RS@latin"), "rs:");
        assert_eq!(guess("de"), "de:");
        assert_eq!(guess("C"), "us:");
        for language in languages() {
            let (layout, variant) = guess_keyboard(language.locale);
            let region = Region {
                keyboard: Keyboard {
                    layout,
                    variant,
                    console: "us".into(),
                },
                ..Region::default()
            };
            assert_eq!(validate_region(&region), Ok(()), "{}", language.locale);
        }
    }

    // ---------------------------------------------------------------------
    // Desktops

    #[test]
    fn desktops_are_marked_by_the_images_packages() {
        let image = FakeRoot::new("pacman");
        image.write("var/lib/pacman/local/ALPM_DB_VERSION", "9");
        for entry in [
            "gdm-48.0-1",
            "gnome-shell-1:48.1-1",
            "cedm-0.9.4-1",
            "lxb-compositor-0.9.4-1",
        ] {
            image.dir(&format!("var/lib/pacman/local/{entry}"));
        }
        let offers = desktops(&image.0);
        let order: Vec<Desktop> = offers.iter().map(|o| o.desktop).collect();
        assert_eq!(
            order,
            [Desktop::Linexin, Desktop::Kinexin, Desktop::LineXinBar]
        );
        let marks: Vec<bool> = offers.iter().map(|o| o.in_image).collect();
        assert_eq!(marks, [true, false, false]);

        image.dir("var/lib/pacman/local/lxb-desktop-0.9.4-1");
        image.dir("var/lib/pacman/local/kinexin-desktop-1.0-2");
        let marks: Vec<bool> = desktops(&image.0).iter().map(|o| o.in_image).collect();
        assert_eq!(marks, [true, true, true]);

        // Kinexin signs in through CEDM, so an image with Plasma and no CEDM
        // has it downloaded.
        let plasma = FakeRoot::new("pacman-plasma");
        plasma.dir("var/lib/pacman/local/kinexin-desktop-1.0-2");
        assert!(!desktops(&plasma.0)[1].in_image);

        // `lxb` alone stands for both halves; without cedm nothing does.
        let other = FakeRoot::new("pacman-lxb");
        other.dir("var/lib/pacman/local/lxb-0.9.4-1");
        let marks: Vec<bool> = desktops(&other.0).iter().map(|o| o.in_image).collect();
        assert_eq!(marks, [false, false, false]);
        other.dir("var/lib/pacman/local/cedm-0.9.4-1");
        assert!(desktops(&other.0)[2].in_image);

        let none: Vec<bool> = desktops(&FakeRoot::new("no-pacman").0)
            .iter()
            .map(|o| o.in_image)
            .collect();
        assert_eq!(none, [false, false, false]);
    }

    #[test]
    fn package_names_are_read_from_database_entries() {
        assert_eq!(package_name("gnome-shell-1:48.1-1"), Some("gnome-shell"));
        assert_eq!(package_name("lxb-0.9.4-1"), Some("lxb"));
        assert_eq!(package_name("ALPM_DB_VERSION"), None);
        assert_eq!(package_name("lonely-1"), None);
        assert_eq!(package_name("-1.0-1"), None);
    }

    // ---------------------------------------------------------------------
    // Flatpaks and packages

    #[test]
    fn flatpaks_are_the_legacy_seven() {
        let ids: BTreeSet<&str> = flatpaks().iter().map(|f| f.id).collect();
        assert_eq!(ids.len(), 7);
        assert!(ids.contains("com.github.tchx84.Flatseal"));
        assert!(ids.contains("com.heroicgameslauncher.hgl"));
        assert_eq!(flatpaks()[0].name, "Zen Browser");
    }

    const PACMAN_QEI: &str = "\
Name            : example-player
Version         : 1.2.3-1
Description     : An invented media player that plays
                  invented media
Architecture    : x86_64
Groups          : None
Optional Deps   : example-codecs: more formats [installed]
                  example-extra: extras
Install Reason  : Explicitly installed

Name            : base
Version         : 3-2
Description     : Minimal package set
Groups          : None

Name            : firefox
Version         : 140.0-1
Description     : Fast, Private & Safe Web Browser
Groups          : None

Name            : gst-plugins-good
Description     : Multimedia graph framework - good plugins
Groups          : None

Name            : noto-fonts-emoji
Description     : Emoji font
Groups          : None

Name            : networkmanager-openvpn
Description     : NetworkManager VPN plugin
Groups          : None

Name            : python-requests
Description     : HTTP for humans
Groups          : None

Name            : evince
Description     : Document viewer
Groups          : gnome

Name            : lxb-toolkit
Description     : The family toolkit
Groups          : None

Name            : cedm
Description     : Login screen
Groups          : None

Name            : greetd-example
Description     : A greeter
Groups          : None

Name            : linexin-installer
Description     : The old installer
Groups          : None

Name            : linux-zen
Description     : A kernel
Groups          : None

Name            : sof-firmware
Description     : Sound firmware
Groups          : None

Name            : intel-ucode
Description     : Microcode
Groups          : None

Name            : limine
Description     : A boot loader
Groups          : None

Name            : lib32-mesa
Description     : Graphics
Groups          : None

Name            : libva
Description     : Video acceleration
Groups          : None

Name            : vlc
Description     : Media player
";

    #[test]
    fn packages_are_categorised_and_hidden_as_legacy_did() {
        let packages = parse_packages(PACMAN_QEI);
        let listed: Vec<(&str, PackageCategory)> = packages
            .iter()
            .map(|p| (p.name.as_str(), p.category))
            .collect();
        assert_eq!(
            listed,
            [
                ("example-player", PackageCategory::Applications),
                ("firefox", PackageCategory::Applications),
                ("gst-plugins-good", PackageCategory::Multimedia),
                ("noto-fonts-emoji", PackageCategory::Fonts),
                ("networkmanager-openvpn", PackageCategory::Network),
                ("python-requests", PackageCategory::Libraries),
                ("vlc", PackageCategory::Applications),
            ]
        );
        assert_eq!(
            packages[0].description,
            "An invented media player that plays invented media"
        );
        assert_eq!(packages[1].description, "Fast, Private & Safe Web Browser");
        assert!(parse_packages("").is_empty());
    }

    #[test]
    fn a_layout_known_only_with_variants_still_gets_its_own_console_map() {
        let map = "# model map\nfr_CH ch pc105 fr terminate:ctrl_alt_bksp\nsg ch pc105 de_nodeadkeys terminate:ctrl_alt_bksp\n";
        assert_eq!(
            kbd_model_map_keymap(map, "ch", "").as_deref(),
            Some("fr_CH")
        );
        assert_eq!(
            kbd_model_map_keymap(map, "ch", "de_nodeadkeys").as_deref(),
            Some("sg")
        );
        assert_eq!(kbd_model_map_keymap(map, "de", ""), None);
    }

    /// pacman and nmcli are started here rather than through `run`, and the
    /// connection is opened here, so each asks the test build's fence itself,
    /// first thing, and is refused. The thread finds no program meanwhile
    /// (`fence::FindNothing`): were a question ever lost, or moved after the
    /// lookup, the starter would find nothing and this test would fail, with
    /// nothing started. `online` itself is not called: were both of its
    /// questions lost at once it would resolve a name and connect, so its two
    /// halves are each asked here instead, the connection to an address that
    /// needs no resolving and that this computer itself would turn away.
    #[test]
    fn packages_and_the_internet_ask_the_fence_first() {
        use crate::worker::run::fence;
        let _nothing = fence::FindNothing::here();
        let words = fence::refusal(removable_packages).expect("pacman's list went on");
        assert!(
            words.contains("test build: pacman is not a program"),
            "{words}"
        );
        let words = fence::refusal(nm_connectivity_full).expect("nmcli's answer went on");
        assert!(
            words.contains("test build: nmcli is not a program"),
            "{words}"
        );
        let words = fence::refusal(|| reaches("127.0.0.1:9", Duration::from_millis(1)))
            .expect("a connection was tried");
        assert!(
            words.contains("test build: no test reaches the network"),
            "{words}"
        );
    }
}
