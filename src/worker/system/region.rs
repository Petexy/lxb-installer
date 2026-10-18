//! The Region step: language, keyboard and time zone.
//!
//! The legacy installer truncated `/etc/locale.gen` to two lines and forced
//! `LC_ALL` into `/etc/environment`, systemd's default environment and every
//! shell profile, which overrides whatever a person later sets per category
//! and per session. Here the chosen locale (and US English, which some
//! programs expect) is uncommented where it is and added only when absent,
//! every other locale the image enabled is commented out again, `locale.conf`
//! says `LANG` alone, and nothing sets `LC_ALL`.
//!
//! The 3.0 image enables ten locales for its live session (the languages the
//! installer and LineXinBar speak, so the live shell can follow the one
//! picked); the installed system is copied from it, and without that last
//! part it would build and keep all ten. `locale-gen` deletes the locale
//! archive before it builds what `locale.gen` names, so the ten built into
//! the image go with it.
use super::image::CONSOLE_FONT;
use super::Install;
use crate::model::config::Keyboard;
use crate::model::protocol::{Failure, Step};
use crate::worker::layout::Stop;
use crate::worker::{run, target};

/// The locale every installed system generates besides the chosen one.
pub const FALLBACK_LOCALE: &str = "en_US.UTF-8";

/// The hardware clock keeps UTC, as the legacy installer set it.
pub const ADJTIME: &str = "0.0 0 0.0\n0\nUTC\n";

/// The locales to generate: the chosen one and the fallback, once each.
pub fn wanted_locales(locale: &str) -> Vec<&str> {
    let mut wanted = vec![locale];
    if locale != FALLBACK_LOCALE {
        wanted.push(FALLBACK_LOCALE);
    }
    wanted
}

/// `locale.gen` with the wanted locales enabled and only those: a locale
/// already enabled is left alone, the commented line for it
/// (`#pl_PL.UTF-8 UTF-8`) is uncommented in place, and a locale with no line
/// at all is added at the end; any other enabled entry (`de_DE.UTF-8 UTF-8`,
/// `hi_IN UTF-8`) is commented out with a bare `#`, the way glibc's own list
/// writes them, so it can be uncommented again. Every other line stays as it
/// was — the header's examples too (`#  en_US.UTF-8 UTF-8`), told apart by
/// the space after the `#`.
pub fn enable_locales(text: &str, wanted: &[&str]) -> String {
    let is_entry = |line: &str, locale: &str| {
        let mut words = line.split_whitespace();
        words.next() == Some(locale) && words.next() == Some("UTF-8") && words.next().is_none()
    };
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| {
            let bare = line.trim_start();
            let enabled = !bare.is_empty() && !bare.starts_with('#');
            if enabled && !wanted.iter().any(|locale| is_entry(line, locale)) {
                format!("#{bare}")
            } else {
                line.to_string()
            }
        })
        .collect();
    for locale in wanted {
        if lines.iter().any(|line| is_entry(line, locale)) {
            continue;
        }
        let commented = lines.iter_mut().find(|line| {
            line.strip_prefix('#').is_some_and(|rest| {
                !rest.starts_with(char::is_whitespace) && is_entry(rest, locale)
            })
        });
        match commented {
            Some(line) => *line = format!("{locale} UTF-8"),
            None => lines.push(format!("{locale} UTF-8")),
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

pub fn locale_conf(locale: &str) -> String {
    format!("LANG={locale}\n")
}

/// `vconsole.conf`: the console keymap, and the legacy console font when the
/// image has it (a font that is not there would leave the console unset).
pub fn vconsole_conf(keymap: &str, font: bool) -> String {
    let mut text = format!("KEYMAP={keymap}\n");
    if font {
        text.push_str(&format!("FONT={CONSOLE_FONT}\n"));
    }
    text
}

/// The X keyboard configuration, in the shape `localectl set-x11-keymap`
/// writes, which is also what gdm and a Wayland session read the system
/// layout from.
pub fn keyboard_conf(keyboard: &Keyboard) -> String {
    let mut text = String::from(
        "Section \"InputClass\"\n        Identifier \"system-keyboard\"\n        MatchIsKeyboard \"on\"\n",
    );
    text.push_str(&format!(
        "        Option \"XkbLayout\" \"{}\"\n",
        keyboard.layout
    ));
    if !keyboard.variant.is_empty() {
        text.push_str(&format!(
            "        Option \"XkbVariant\" \"{}\"\n",
            keyboard.variant
        ));
    }
    text.push_str("EndSection\n");
    text
}

/// Where `/etc/localtime` points: relative, so it resolves inside the
/// installed system and not on the live one while the installer works.
pub fn zone_link(zone: &str) -> String {
    format!("../usr/share/zoneinfo/{zone}")
}

pub fn run(install: &mut Install) -> Result<(), Stop> {
    let root = install.root().to_path_buf();
    // Borrowed for the whole run, not from `install`, so it stays readable
    // while progress is reported.
    let region = &install.config.region;

    let text = target::read(&root, "etc/locale.gen")?
        .ok_or_else(|| Stop::new(Failure::ImageInvalid, "the target has no /etc/locale.gen"))?;
    let enabled = enable_locales(&text, &wanted_locales(&region.locale));
    if enabled != text {
        target::write(&root, "etc/locale.gen", &enabled, 0o644)?;
    }
    run::chroot(&root, "locale-gen", &[])?;
    target::write(
        &root,
        "etc/locale.conf",
        &locale_conf(&region.locale),
        0o644,
    )?;
    install.progress(Step::Region, 1, 3);

    let vconsole = vconsole_conf(&region.keyboard.console, install.image.console_font);
    target::write(&root, "etc/vconsole.conf", &vconsole, 0o644)?;
    target::create_dir(&root, "etc/X11")?;
    target::create_dir(&root, "etc/X11/xorg.conf.d")?;
    target::write(
        &root,
        "etc/X11/xorg.conf.d/00-keyboard.conf",
        &keyboard_conf(&region.keyboard),
        0o644,
    )?;
    install.progress(Step::Region, 2, 3);

    target::symlink(&root, "etc/localtime", &zone_link(&region.timezone))?;
    target::write(&root, "etc/adjtime", ADJTIME, 0o644)?;
    install.progress(Step::Region, 3, 3);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCALE_GEN: &str = "\
# Configuration file for locale-gen
#
#  en_US.UTF-8 UTF-8
#en_US ISO-8859-1
#en_US.UTF-8 UTF-8
#pl_PL ISO-8859-2
#pl_PL.UTF-8 UTF-8
";

    #[test]
    fn chosen_locales_are_uncommented_and_nothing_is_lost() {
        let enabled = enable_locales(LOCALE_GEN, &wanted_locales("pl_PL.UTF-8"));
        assert_eq!(
            enabled,
            "\
# Configuration file for locale-gen
#
#  en_US.UTF-8 UTF-8
#en_US ISO-8859-1
en_US.UTF-8 UTF-8
#pl_PL ISO-8859-2
pl_PL.UTF-8 UTF-8
"
        );
        assert_eq!(enabled.lines().count(), LOCALE_GEN.lines().count());
        // A locale already enabled is not enabled a second time.
        let twice = enable_locales("en_US.UTF-8 UTF-8\n#en_US.UTF-8 UTF-8\n", &["en_US.UTF-8"]);
        assert_eq!(twice, "en_US.UTF-8 UTF-8\n#en_US.UTF-8 UTF-8\n");
        assert_eq!(
            enable_locales(&enabled, &wanted_locales("pl_PL.UTF-8")),
            enabled
        );
    }

    /// Shaped like a live image's own list: a few comment lines, then the
    /// languages its live session may switch to, every one enabled.
    const LIVE_LOCALE_GEN: &str = "\
# The live medium's locales.
#  (one per line)
de_DE.UTF-8 UTF-8
en_GB.UTF-8 UTF-8
en_US.UTF-8 UTF-8
hi_IN UTF-8
pl_PL.UTF-8 UTF-8
  ru_RU.UTF-8 UTF-8
";

    #[test]
    fn locales_the_image_enabled_for_its_live_session_are_not_kept() {
        let enabled = enable_locales(LIVE_LOCALE_GEN, &wanted_locales("pl_PL.UTF-8"));
        assert_eq!(
            enabled,
            "\
# The live medium's locales.
#  (one per line)
#de_DE.UTF-8 UTF-8
#en_GB.UTF-8 UTF-8
en_US.UTF-8 UTF-8
#hi_IN UTF-8
pl_PL.UTF-8 UTF-8
#ru_RU.UTF-8 UTF-8
"
        );
        // Run again, nothing changes; and a line commented out here is one
        // the next choice can uncomment.
        assert_eq!(
            enable_locales(&enabled, &wanted_locales("pl_PL.UTF-8")),
            enabled
        );
        let german = enable_locales(&enabled, &wanted_locales("de_DE.UTF-8"));
        let on: Vec<&str> = german.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(on, ["de_DE.UTF-8 UTF-8", "en_US.UTF-8 UTF-8"]);
        // US English alone keeps one line.
        let us = enable_locales(LIVE_LOCALE_GEN, &wanted_locales("en_US.UTF-8"));
        let on: Vec<&str> = us.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(on, ["en_US.UTF-8 UTF-8"]);
        // Another charset of the chosen language is not the one chosen.
        let latin = enable_locales("pl_PL ISO-8859-2\n", &["pl_PL.UTF-8"]);
        assert_eq!(latin, "#pl_PL ISO-8859-2\npl_PL.UTF-8 UTF-8\n");
    }

    #[test]
    fn a_locale_without_a_line_is_added() {
        let enabled = enable_locales("#de_DE.UTF-8 UTF-8\n", &wanted_locales("fy_NL.UTF-8"));
        assert_eq!(
            enabled,
            "#de_DE.UTF-8 UTF-8\nfy_NL.UTF-8 UTF-8\nen_US.UTF-8 UTF-8\n"
        );
        assert_eq!(wanted_locales("en_US.UTF-8"), ["en_US.UTF-8"]);
        // A locale whose name only starts the same is not the one chosen.
        let similar = enable_locales("#en_US.UTF-8@x UTF-8\n", &["en_US.UTF-8"]);
        assert!(similar.starts_with("#en_US.UTF-8@x UTF-8\n"));
        assert!(similar.ends_with("\nen_US.UTF-8 UTF-8\n"));
    }

    #[test]
    fn locale_conf_says_lang_alone() {
        let text = locale_conf("pl_PL.UTF-8");
        assert_eq!(text, "LANG=pl_PL.UTF-8\n");
        assert!(!text.contains("LC_"));
    }

    #[test]
    fn the_console_font_is_set_only_when_the_image_has_it() {
        assert_eq!(vconsole_conf("pl2", true), "KEYMAP=pl2\nFONT=ter-v16n\n");
        assert_eq!(vconsole_conf("us", false), "KEYMAP=us\n");
    }

    #[test]
    fn the_x_keyboard_names_layout_and_variant() {
        let mut keyboard = Keyboard {
            layout: "de".into(),
            variant: "nodeadkeys".into(),
            console: "de-latin1-nodeadkeys".into(),
        };
        let text = keyboard_conf(&keyboard);
        assert!(text.contains("Identifier \"system-keyboard\""));
        assert!(text.contains("Option \"XkbLayout\" \"de\""));
        assert!(text.contains("Option \"XkbVariant\" \"nodeadkeys\""));
        assert!(text.ends_with("EndSection\n"));
        keyboard.variant.clear();
        assert!(!keyboard_conf(&keyboard).contains("XkbVariant"));
    }

    #[test]
    fn localtime_points_inside_the_installed_system() {
        assert_eq!(
            zone_link("Europe/Warsaw"),
            "../usr/share/zoneinfo/Europe/Warsaw"
        );
        assert_eq!(ADJTIME.lines().last(), Some("UTC"));
    }
}
