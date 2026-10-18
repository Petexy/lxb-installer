//! Writing inside the target root without ever following a symlink out of
//! it. A copied image can hold absolute symlinks, which resolve against the
//! live system rather than the target; every write here refuses a path
//! whose parent directories are not real directories inside the root.
//!
//! CONTRACT (phase 0), shared by both halves of the worker.
use super::layout::Stop;
use crate::model::protocol::Failure;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

fn internal(error: impl std::fmt::Display) -> Stop {
    Stop::new(Failure::Internal, error.to_string())
}

/// `root/relative`, refusing absolute paths, `..`, and any existing parent
/// that is not a real directory.
pub fn path(root: &Path, relative: &str) -> Result<PathBuf, Stop> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(internal(format!(
            "invalid target path {}",
            relative.display()
        )));
    }
    let mut cursor = root.to_path_buf();
    if let Some(parent) = relative.parent() {
        for part in parent.components() {
            cursor.push(part);
            match fs::symlink_metadata(&cursor) {
                Ok(meta) if meta.is_dir() => {}
                Ok(_) => {
                    return Err(internal(format!(
                        "refusing a target path through {}",
                        cursor.display()
                    )))
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                Err(e) => return Err(internal(e)),
            }
        }
    }
    Ok(root.join(relative))
}

/// Creates `root/relative` and its missing parents with mode 0755.
pub fn create_dir(root: &Path, relative: &str) -> Result<(), Stop> {
    let target = path(root, relative)?;
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if !meta.is_dir() {
            return Err(internal(format!(
                "{} must be a directory",
                target.display()
            )));
        }
        return Ok(());
    }
    fs::create_dir_all(&target).map_err(internal)?;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(&target)
        .map_err(internal)?;
    set_file_mode(&directory, 0o755)
}

/// Removes a file, a symlink (not its target) or a whole directory; a path
/// that does not exist is fine.
pub fn remove(root: &Path, relative: &str) -> Result<(), Stop> {
    let target = path(root, relative)?;
    match fs::symlink_metadata(&target) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(&target).map_err(internal),
        Ok(_) => fs::remove_file(&target).map_err(internal),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(internal(e)),
    }
}

/// Replaces `root/relative` with `text`, created fresh with `mode` and
/// without following a symlink at the final component.
pub fn write(root: &Path, relative: &str, text: &str, mode: u32) -> Result<(), Stop> {
    let target = path(root, relative)?;
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if meta.is_dir() {
            return Err(internal(format!("{} is a directory", target.display())));
        }
        fs::remove_file(&target).map_err(internal)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&target)
        .map_err(|e| internal(format!("cannot write {}: {e}", target.display())))?;
    file.write_all(text.as_bytes()).map_err(internal)?;
    set_file_mode(&file, mode)
}

/// FAT's magic number in `statfs`: the EFI system partition.
const MSDOS_SUPER_MAGIC: i64 = 0x4d44;

/// Gives an open file its mode. The EFI system partition is FAT, which has
/// no modes to give (its mount options decide them all) and answers a chmod
/// with EPERM; a boot loader file written there is still correct, so that
/// refusal is not an error. Anywhere else it is.
fn set_file_mode(file: &File, mode: u32) -> Result<(), Stop> {
    match file.set_permissions(fs::Permissions::from_mode(mode)) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(libc::EPERM) && on_fat(file) => Ok(()),
        Err(error) => Err(internal(error)),
    }
}

// `f_type` is i64 on x86_64 but a narrower type on other targets, so the
// cast that clippy calls unnecessary here is what keeps it portable.
#[allow(clippy::unnecessary_cast)]
fn on_fat(file: &File) -> bool {
    let mut stats = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: fstatfs writes a whole statfs into the buffer when it returns 0,
    // and the buffer is only read in that case.
    unsafe {
        libc::fstatfs(file.as_raw_fd(), stats.as_mut_ptr()) == 0
            && stats.assume_init().f_type as i64 == MSDOS_SUPER_MAGIC
    }
}

/// Whether `root/relative` exists, without following a final symlink.
pub fn exists(root: &Path, relative: &str) -> bool {
    path(root, relative)
        .map(|p| fs::symlink_metadata(p).is_ok())
        .unwrap_or(false)
}

/// Whether `root/relative` is a regular file (a symlink is not).
pub fn is_file(root: &Path, relative: &str) -> bool {
    path(root, relative)
        .map(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_file()))
        .unwrap_or(false)
}

/// Whether `root/relative` is a real directory (a symlink is not).
pub fn is_dir(root: &Path, relative: &str) -> bool {
    path(root, relative)
        .map(|p| fs::symlink_metadata(p).is_ok_and(|m| m.is_dir()))
        .unwrap_or(false)
}

/// The names in the directory `root/relative`, sorted; empty when it does
/// not exist or is not a real directory.
pub fn list(root: &Path, relative: &str) -> Result<Vec<String>, Stop> {
    let directory = path(root, relative)?;
    match fs::symlink_metadata(&directory) {
        Ok(meta) if meta.is_dir() => {}
        _ => return Ok(Vec::new()),
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(&directory).map_err(internal)? {
        let entry = entry.map_err(internal)?;
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// Replaces `root/relative` with a symlink pointing at `points_to`, which is
/// written as given (relative links resolve inside the target, which is why
/// the callers use them).
pub fn symlink(root: &Path, relative: &str, points_to: &str) -> Result<(), Stop> {
    remove(root, relative)?;
    let target = path(root, relative)?;
    std::os::unix::fs::symlink(points_to, &target)
        .map_err(|e| internal(format!("cannot link {}: {e}", target.display())))
}

/// Moves `root/from` over `root/to` (both inside the target, same
/// filesystem). A missing `from` is fine and returns false; a directory at
/// `to` is refused rather than replaced.
pub fn rename(root: &Path, from: &str, to: &str) -> Result<bool, Stop> {
    let source = path(root, from)?;
    if fs::symlink_metadata(&source).is_err() {
        return Ok(false);
    }
    let destination = path(root, to)?;
    if let Ok(meta) = fs::symlink_metadata(&destination) {
        if meta.is_dir() {
            return Err(internal(format!(
                "{} is a directory",
                destination.display()
            )));
        }
    }
    fs::rename(&source, &destination).map_err(|e| {
        internal(format!(
            "cannot move {} to {}: {e}",
            source.display(),
            destination.display()
        ))
    })?;
    Ok(true)
}

/// Gives `root/relative` the mode `mode` if it is a regular file or a real
/// directory; a symlink or a missing path is left alone.
pub fn set_mode(root: &Path, relative: &str, mode: u32) -> Result<(), Stop> {
    let target = path(root, relative)?;
    match fs::symlink_metadata(&target) {
        Ok(meta) if meta.is_file() || meta.is_dir() => {
            fs::set_permissions(&target, fs::Permissions::from_mode(mode)).map_err(internal)
        }
        _ => Ok(()),
    }
}

/// Copies the regular file `source` (anywhere: the image, or a file already
/// inside the target) to `root/relative`, replacing what is there without
/// following a symlink, created with `mode`.
pub fn copy_file(source: &Path, root: &Path, relative: &str, mode: u32) -> Result<(), Stop> {
    let meta = fs::symlink_metadata(source)
        .map_err(|e| internal(format!("cannot read {}: {e}", source.display())))?;
    if !meta.is_file() {
        return Err(internal(format!(
            "{} is not a regular file",
            source.display()
        )));
    }
    let target = path(root, relative)?;
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if meta.is_dir() {
            return Err(internal(format!("{} is a directory", target.display())));
        }
        fs::remove_file(&target).map_err(internal)?;
    }
    let mut from = File::open(source)
        .map_err(|e| internal(format!("cannot read {}: {e}", source.display())))?;
    let mut to = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&target)
        .map_err(|e| internal(format!("cannot write {}: {e}", target.display())))?;
    std::io::copy(&mut from, &mut to)
        .map_err(|e| internal(format!("cannot copy to {}: {e}", target.display())))?;
    to.sync_all().map_err(internal)?;
    set_file_mode(&to, mode)
}

/// Copies the directory `source` into `root/relative`, recursively: regular
/// files and directories only (a symlink inside is skipped, never followed),
/// files 0644 and directories 0755. Used for the boot loader's own files,
/// which go onto the EFI partition where modes mean nothing anyway.
pub fn copy_tree(source: &Path, root: &Path, relative: &str) -> Result<(), Stop> {
    create_dir(root, relative)?;
    let mut entries: Vec<_> = fs::read_dir(source)
        .map_err(|e| internal(format!("cannot read {}: {e}", source.display())))?
        .collect::<Result<_, _>>()
        .map_err(internal)?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let meta = fs::symlink_metadata(entry.path()).map_err(internal)?;
        let inside = format!("{relative}/{name}");
        if meta.is_dir() {
            copy_tree(&entry.path(), root, &inside)?;
        } else if meta.is_file() {
            copy_file(&entry.path(), root, &inside, 0o644)?;
        }
    }
    Ok(())
}

/// Reads `root/relative` as text, or `None` when it does not exist.
pub fn read(root: &Path, relative: &str) -> Result<Option<String>, Stop> {
    let target = path(root, relative)?;
    match fs::read_to_string(&target) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(internal(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lxb-target-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn refuses_symlinked_parents_and_traversal() {
        let root = scratch("refuse");
        std::os::unix::fs::symlink("/tmp", root.join("etc")).unwrap();
        assert!(path(&root, "etc/hostname").is_err());
        assert!(path(&root, "../hostname").is_err());
        assert!(path(&root, "/etc/hostname").is_err());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn writes_replace_a_symlink_instead_of_following_it() {
        let root = scratch("replace");
        fs::create_dir(root.join("etc")).unwrap();
        let outside = root.join("outside");
        fs::write(&outside, "untouched").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("etc/hostname")).unwrap();
        write(&root, "etc/hostname", "linexin\n", 0o644).unwrap();
        assert_eq!(fs::read_to_string(&outside).unwrap(), "untouched");
        assert_eq!(read(&root, "etc/hostname").unwrap().unwrap(), "linexin\n");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn symlinks_renames_and_listings_stay_inside_the_root() {
        let root = scratch("links");
        fs::create_dir_all(root.join("etc")).unwrap();
        symlink(&root, "etc/localtime", "../usr/share/zoneinfo/UTC").unwrap();
        assert_eq!(
            fs::read_link(root.join("etc/localtime")).unwrap(),
            Path::new("../usr/share/zoneinfo/UTC")
        );
        assert!(exists(&root, "etc/localtime"));
        assert!(!is_file(&root, "etc/localtime"));

        write(&root, "etc/a-postinstall", "new", 0o644).unwrap();
        write(&root, "etc/a", "old", 0o644).unwrap();
        assert!(rename(&root, "etc/a-postinstall", "etc/a").unwrap());
        assert!(!rename(&root, "etc/missing", "etc/b").unwrap());
        assert_eq!(read(&root, "etc/a").unwrap().unwrap(), "new");
        assert_eq!(list(&root, "etc").unwrap(), ["a", "localtime"]);
        assert!(list(&root, "nowhere").unwrap().is_empty());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn trees_are_copied_without_their_symlinks() {
        let source = scratch("tree-source");
        fs::create_dir_all(source.join("icons")).unwrap();
        fs::write(source.join("refind_x64.efi"), "loader").unwrap();
        fs::write(source.join("icons/os.png"), "icon").unwrap();
        std::os::unix::fs::symlink("/etc/passwd", source.join("escape")).unwrap();
        let root = scratch("tree-target");
        copy_tree(&source, &root, "boot/EFI/refind").unwrap();
        assert!(is_file(&root, "boot/EFI/refind/refind_x64.efi"));
        assert!(is_file(&root, "boot/EFI/refind/icons/os.png"));
        assert!(!exists(&root, "boot/EFI/refind/escape"));
        fs::remove_dir_all(&source).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }
}
