//! Building the start-up image.
//!
//! `mkinitcpio -P` builds every preset, which by now is the installed one
//! (the System step put `<pkgbase>-postinstall.preset` in the live one's
//! place and removed archiso's settings). On Btrfs the `btrfs` module is
//! named in `MODULES` as the legacy installer did: autodetection usually
//! finds it, and when it does not the system cannot mount its own root. A
//! system that cannot start must not be reported as installed, so any
//! failure here stops the installation.
use super::Install;
use crate::model::config::Filesystem;
use crate::model::protocol::{Failure, Step};
use crate::worker::layout::Stop;
use crate::worker::{run, target};

pub const MKINITCPIO_CONF: &str = "etc/mkinitcpio.conf";

/// `Step::Initramfs`.
pub fn run(install: &mut Install) -> Result<(), Stop> {
    let root = install.root().to_path_buf();
    if install.mounted.filesystem == Filesystem::Btrfs {
        let text = target::read(&root, MKINITCPIO_CONF)?.unwrap_or_default();
        if let Some(edited) = with_module(&text, "btrfs") {
            target::write(&root, MKINITCPIO_CONF, &edited, 0o644)?;
            run::log("initramfs: btrfs added to MODULES");
        }
    }
    let mut total = 0;
    for name in target::list(&root, "etc/mkinitcpio.d")? {
        if name.ends_with(".preset") {
            let text = target::read(&root, &format!("etc/mkinitcpio.d/{name}"))?;
            total += preset_count(text.as_deref().unwrap_or_default());
        }
    }
    let mut done = 0;
    run::chroot_streaming(&root, "mkinitcpio", &["-P"], &mut |line| {
        run::log(line);
        if line.contains("Image generation successful") {
            done += 1;
            install.progress(Step::Initramfs, done, total);
        }
    })?;
    let image = format!("boot/initramfs-{}.img", install.image.kernel.pkgbase);
    if !target::is_file(&root, &image) {
        return Err(Stop::new(
            Failure::CommandFailed,
            format!("mkinitcpio did not make /{image}"),
        ));
    }
    Ok(())
}

/// `mkinitcpio.conf` with `module` in its `MODULES` array (the last one
/// that is not commented out, which is the one mkinitcpio uses), or a
/// `MODULES` line of its own when there is none; `None` when the module is
/// there already.
pub fn with_module(text: &str, module: &str) -> Option<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let index = lines
        .iter()
        .rposition(|line| line.trim_start().starts_with("MODULES="));
    let Some(index) = index else {
        let mut out = text.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("MODULES=({module})\n"));
        return Some(out);
    };
    let line = lines[index];
    let bare = line.trim_end_matches(['\n', '\r']);
    let ending = &line[bare.len()..];
    let (head, value) = bare.split_once('=').expect("the line has an =");
    let value = value.trim_start();
    let edited = if let Some(inside) = value.strip_prefix('(') {
        match inside.split_once(')') {
            Some((words, after)) => {
                if words.split_whitespace().any(|w| w == module) {
                    return None;
                }
                let mut words: Vec<&str> = words.split_whitespace().collect();
                words.push(module);
                format!("{head}=({}){after}", words.join(" "))
            }
            // An array that goes on over the next lines: the module goes
            // first, which is the same array.
            None => format!("{head}=({module} {inside}"),
        }
    } else {
        // The old way of writing it, a quoted string, which mkinitcpio no
        // longer reads; rewritten as the array it now wants.
        let words = value.trim_matches(['"', '\'']);
        if words.split_whitespace().any(|w| w == module) {
            return None;
        }
        let mut words: Vec<&str> = words.split_whitespace().collect();
        words.push(module);
        format!("{head}=({})", words.join(" "))
    };
    let mut out = String::with_capacity(text.len() + module.len() + 1);
    for (i, line) in lines.iter().enumerate() {
        if i == index {
            out.push_str(&edited);
            out.push_str(ending);
        } else {
            out.push_str(line);
        }
    }
    Some(out)
}

/// How many images a preset builds: the entries of its `PRESETS` array.
pub fn preset_count(preset: &str) -> u64 {
    preset
        .lines()
        .rev()
        .find_map(|line| line.trim_start().strip_prefix("PRESETS="))
        .map(|value| {
            value
                .trim()
                .trim_start_matches('(')
                .split(')')
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .count() as u64
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn btrfs_joins_an_empty_or_full_modules_array_once() {
        let conf = "# vim:set ft=sh\n#MODULES=(old)\nMODULES=()\nBINARIES=()\n";
        let edited = with_module(conf, "btrfs").unwrap();
        assert_eq!(
            edited,
            "# vim:set ft=sh\n#MODULES=(old)\nMODULES=(btrfs)\nBINARIES=()\n"
        );
        assert_eq!(with_module(&edited, "btrfs"), None);
        assert_eq!(
            with_module("MODULES=(i915 nvme) # early KMS\n", "btrfs").unwrap(),
            "MODULES=(i915 nvme btrfs) # early KMS\n"
        );
        assert_eq!(with_module("MODULES=( btrfs )\n", "btrfs"), None);
    }

    #[test]
    fn the_last_modules_line_is_the_one_edited() {
        assert_eq!(
            with_module("MODULES=(a)\nMODULES=(b)\n", "btrfs").unwrap(),
            "MODULES=(a)\nMODULES=(b btrfs)\n"
        );
    }

    #[test]
    fn a_missing_or_old_style_modules_line_is_written_as_an_array() {
        assert_eq!(
            with_module("HOOKS=(base udev)", "btrfs").unwrap(),
            "HOOKS=(base udev)\nMODULES=(btrfs)\n"
        );
        assert_eq!(
            with_module("MODULES=\"ext4\"\n", "btrfs").unwrap(),
            "MODULES=(ext4 btrfs)\n"
        );
        assert_eq!(
            with_module("MODULES=(\n  nvme\n)\n", "btrfs").unwrap(),
            "MODULES=(btrfs \n  nvme\n)\n"
        );
    }

    #[test]
    fn presets_are_counted_from_their_array() {
        let preset = "ALL_kver='/boot/vmlinuz-linux'\n#PRESETS=('default')\nPRESETS=('default' 'fallback')\n";
        assert_eq!(preset_count(preset), 2);
        assert_eq!(preset_count("PRESETS=('default')"), 1);
        assert_eq!(preset_count("ALL_kver=x"), 0);
    }
}
