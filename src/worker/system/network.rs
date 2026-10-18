//! Whether the internet answers, asked once before writing.
//!
//! Some choices cannot be installed without it — Kinexin or LineXinBar when
//! the image does not carry it with its login screen, CEDM — and those stop
//! the installation before the disk is touched. Others only use it when it
//! is there (updates, Flatpaks, the snapshot tools), and their steps turn a
//! missing connection into a warning instead. When nothing chosen uses the internet it is not asked at
//! all, so an offline installation never waits for a probe to time out.
use super::image::Image;
use crate::model::catalogs;
use crate::model::config::{Desktop, Filesystem, InstallConfig};
use crate::model::protocol::Failure;
use crate::worker::layout::Stop;
use crate::worker::run;

/// Whether the choices cannot be installed at all without the internet.
pub fn required(config: &InstallConfig, image: &Image) -> bool {
    match config.desktop {
        Desktop::Linexin => false,
        Desktop::Kinexin => !image.carries(Desktop::Kinexin),
        Desktop::LineXinBar => !image.carries(Desktop::LineXinBar),
    }
}

/// Whether anything chosen uses the internet when it is there.
pub fn wanted(config: &InstallConfig, image: &Image) -> bool {
    let software = &config.software;
    required(config, image)
        || software.updates
        || !software.flatpaks.is_empty()
        || (software.snapshots && config.storage.filesystem == Filesystem::Btrfs)
}

/// What the answer means: `Ok(online)`, or `NeedsInternet` when the
/// internet is required and did not answer. `probe` is only called when
/// something wants the internet.
pub fn decide(required: bool, wanted: bool, probe: impl FnOnce() -> bool) -> Result<bool, Failure> {
    if !wanted {
        return Ok(false);
    }
    let online = probe();
    if required && !online {
        return Err(Failure::NeedsInternet);
    }
    Ok(online)
}

/// Asks the internet when the choices use it; `NeedsInternet` when they
/// cannot do without it and it does not answer.
pub fn check(config: &InstallConfig, image: &Image) -> Result<bool, Stop> {
    let required = required(config, image);
    let wanted = wanted(config, image);
    let answer = decide(required, wanted, catalogs::online);
    match answer {
        Ok(online) if wanted => run::log(&format!("network: online={online}")),
        Ok(_) => run::log("network: nothing chosen uses the internet; not asked"),
        Err(_) => {}
    }
    answer.map_err(|failure| {
        Stop::new(
            failure,
            format!(
                "{:?} must be downloaded and the internet does not answer",
                config.desktop
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_internet_is_only_asked_when_something_uses_it() {
        let mut asked = false;
        assert_eq!(
            decide(false, false, || {
                asked = true;
                true
            }),
            Ok(false)
        );
        assert!(!asked);
    }

    #[test]
    fn only_a_required_download_stops_when_offline() {
        assert_eq!(decide(true, true, || false), Err(Failure::NeedsInternet));
        assert_eq!(decide(true, true, || true), Ok(true));
        assert_eq!(decide(false, true, || false), Ok(false));
        assert_eq!(decide(false, true, || true), Ok(true));
    }

    #[test]
    fn what_needs_the_internet_follows_the_choices_and_the_image() {
        use crate::model::config::Target;
        use crate::worker::partition::fixtures::{config, disk_empty};
        use crate::worker::system::image::fixture;
        let gnome = fixture(&[Desktop::Linexin], &[]);
        let everything = fixture(&[Desktop::Linexin, Desktop::LineXinBar], &[]);
        let mut config = config(disk_empty(512), Target::WholeDisk);
        config.software.snapshots = false;
        assert!(!required(&config, &gnome));
        assert!(!wanted(&config, &gnome));

        config.desktop = Desktop::Kinexin;
        assert!(required(&config, &everything));
        config.desktop = Desktop::LineXinBar;
        assert!(required(&config, &gnome));
        assert!(!required(&config, &everything));

        config.desktop = Desktop::Linexin;
        config.software.updates = true;
        assert!(wanted(&config, &gnome) && !required(&config, &gnome));
        config.software.updates = false;
        config.software.flatpaks = vec!["org.example.App".into()];
        assert!(wanted(&config, &gnome));
        config.software.flatpaks.clear();
        config.software.snapshots = true;
        assert!(!wanted(&config, &gnome), "snapshots only on Btrfs");
        config.storage.filesystem = Filesystem::Btrfs;
        assert!(wanted(&config, &gnome));
    }
}
