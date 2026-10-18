//! The pictures of the three desktops, carried inside the program.
//!
//! The installer runs from a live system where nothing of it is installed but
//! the binary, so the pictures travel in it (`include_bytes!`) rather than
//! beside it. The toolkit draws a picture from a file — it decodes it on its
//! own threads, once, and keeps it on the card's texture for as long as it is
//! shown — so each picture is written out once per run into the person's
//! private runtime directory and handed over by its path from then on.
//!
//! Never into `/tmp`: a directory everybody can write to is one where a link
//! planted under the expected name would have the picture written somewhere
//! else. Where there is no private directory to write to, there is no picture,
//! and the card shows its mark instead.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::model::config::Desktop;

static LINEXIN: &[u8] = include_bytes!("../../data/pictures/linexin.png");
static KINEXIN: &[u8] = include_bytes!("../../data/pictures/kinexin.png");
static LINEXINBAR: &[u8] = include_bytes!("../../data/pictures/linexinbar.png");

/// The bytes and file name of a desktop's picture.
fn source(desktop: Desktop) -> (&'static [u8], &'static str) {
    match desktop {
        Desktop::Linexin => (LINEXIN, "linexin.png"),
        Desktop::Kinexin => (KINEXIN, "kinexin.png"),
        Desktop::LineXinBar => (LINEXINBAR, "linexinbar.png"),
    }
}

/// Where the pictures are written: the session's private runtime directory,
/// else the person's own cache. Only an absolute path is believed. The
/// Language step's flags go into `flags/` under it (`super::flags`).
pub(super) fn directory() -> Option<PathBuf> {
    let absolute = |name: &str| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    absolute("XDG_RUNTIME_DIR")
        .or_else(|| absolute("XDG_CACHE_HOME"))
        .or_else(|| absolute("HOME").map(|home| home.join(".cache")))
        .map(|base| base.join("lxb-installer").join("pictures"))
}

/// Write `bytes` to `name` in `directory`, unless the same bytes are already
/// there: beside it first and moved into place, so a picture cut off halfway
/// is never one the toolkit is handed.
pub(super) fn write_once(directory: &Path, name: &str, bytes: &[u8]) -> Option<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)
        .ok()?;
    let path = directory.join(name);
    if std::fs::read(&path).is_ok_and(|there| there == bytes) {
        return Some(path);
    }
    let part = directory.join(format!(".{name}.part"));
    let written = std::fs::File::create(&part).and_then(|mut file| file.write_all(bytes));
    if written.is_err() || std::fs::rename(&part, &path).is_err() {
        let _ = std::fs::remove_file(&part);
        return None;
    }
    Some(path)
}

/// The file a desktop's picture can be drawn from, written the first time
/// it is asked for; `None` where it could not be written.
pub fn path(desktop: Desktop) -> Option<PathBuf> {
    static WRITTEN: [OnceLock<Option<PathBuf>>; 3] =
        [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let slot = match desktop {
        Desktop::Linexin => 0,
        Desktop::Kinexin => 1,
        Desktop::LineXinBar => 2,
    };
    WRITTEN[slot]
        .get_or_init(|| {
            let (bytes, name) = source(desktop);
            write_once(&directory()?, name, bytes)
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_desktop_carries_a_picture_that_is_a_png() {
        for desktop in [Desktop::Linexin, Desktop::Kinexin, Desktop::LineXinBar] {
            let (bytes, name) = source(desktop);
            assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "{name}");
            assert!(name.ends_with(".png"));
        }
    }
}
