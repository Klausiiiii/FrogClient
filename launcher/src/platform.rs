//! Platform-specific bits; everything else in the launcher stays OS-neutral.

use std::io;
use std::path::Path;

#[cfg(windows)]
pub const CLASSPATH_SEP: &str = ";";
#[cfg(not(windows))]
pub const CLASSPATH_SEP: &str = ":";

/// Makes `link` point at the directory `target` (junction on Windows, symlink elsewhere).
/// A real directory already at `link` has its contents moved into `target` first.
pub fn link_dir(target: &Path, link: &Path) -> io::Result<()> {
    std::fs::create_dir_all(target)?;
    if is_dir_link(link) {
        if std::fs::canonicalize(link).ok() == std::fs::canonicalize(target).ok() {
            return Ok(());
        }
        remove_dir_link(link)?;
    } else if link.is_dir() {
        for entry in std::fs::read_dir(link)? {
            let entry = entry?;
            let dest = target.join(entry.file_name());
            // Never overwrite shared data with an instance-local copy.
            if !dest.exists() {
                std::fs::rename(entry.path(), dest)?;
            }
        }
        std::fs::remove_dir_all(link)?;
    }
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent)?;
    }
    create_dir_link(target, link)
}

#[cfg(windows)]
fn is_dir_link(path: &Path) -> bool {
    junction::exists(path).unwrap_or(false)
        || std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

#[cfg(not(windows))]
fn is_dir_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

#[cfg(windows)]
fn remove_dir_link(path: &Path) -> io::Result<()> {
    junction::delete(path).or_else(|_| std::fs::remove_dir(path))?;
    std::fs::remove_dir(path).or(Ok(()))
}

#[cfg(not(windows))]
fn remove_dir_link(path: &Path) -> io::Result<()> {
    std::fs::remove_file(path)
}

#[cfg(windows)]
fn create_dir_link(target: &Path, link: &Path) -> io::Result<()> {
    junction::create(target, link)
}

#[cfg(not(windows))]
fn create_dir_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

/// Hard link where possible, copy otherwise. Replaces an existing `dst`.
pub fn link_or_copy_file(src: &Path, dst: &Path) -> io::Result<()> {
    if dst.exists() || std::fs::symlink_metadata(dst).is_ok() {
        std::fs::remove_file(dst)?;
    }
    if std::fs::hard_link(src, dst).is_err() {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}
