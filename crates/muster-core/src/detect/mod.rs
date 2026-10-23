//! Finding installed apps.
//!
//! Each OS module scans the roots it is given and returns what it found; it never decides
//! where to look on its own, so tests pass fixture directories
//! (`tests/fixtures/{linux,macos,windows}`) instead of the real machine. The parsers are
//! compiled on every OS so all of them are tested on every CI runner.
//!
//! Detection runs on demand (startup, refresh), never on a timer. Entries that cannot be
//! parsed or launched are counted in [`Scan::skipped`], not reported one by one: their paths
//! are under the user's home and must not reach logs (`.claude/rules/privacy.md`).

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::launch::LaunchTarget;

pub mod linux;
pub mod macos;
pub mod windows;

/// Files larger than this are skipped unread: a `.desktop` file, `Info.plist` or `.lnk` is a
/// few KiB.
pub const MAX_ENTRY_BYTES: u64 = 256 * 1024;
/// Upper bound on directory entries visited per scan, so a huge or looping tree cannot stall it.
pub const MAX_VISITED: usize = 50_000;
/// Upper bound on the apps one scan returns.
pub const MAX_APPS: usize = 4_096;
/// Longest display name kept; longer ones are treated as garbage.
pub const MAX_NAME_LEN: usize = 256;

/// An app found on disk, before it becomes an [`AppEntry`](crate::model::AppEntry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedApp {
    /// Stable identity for this app across scans on the same machine: the desktop-file id on
    /// Linux, the bundle identifier on macOS, the shortcut's path under the Start Menu on
    /// Windows.
    pub key: String,
    pub name: String,
    pub target: LaunchTarget,
    pub icon: Option<PathBuf>,
}

/// The result of one scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    pub apps: Vec<DetectedApp>,
    /// Candidate files that were unreadable, malformed, too large, or point at nothing
    /// launchable.
    pub skipped: usize,
}

/// Scans the standard app locations of the OS this build runs on.
#[must_use]
pub fn detect_installed() -> Scan {
    #[cfg(target_os = "linux")]
    return linux::scan(&linux::default_roots(), &search_path());
    #[cfg(target_os = "macos")]
    return macos::scan(&macos::default_roots());
    #[cfg(target_os = "windows")]
    return windows::scan(&windows::default_roots(), &|name| std::env::var(name).ok());
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    Scan::default()
}

/// The directories in `$PATH`, in order.
#[must_use]
pub fn search_path() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default()
}

/// Finds `name` (a bare file name) in `dirs` as an executable file.
pub(crate) fn find_in(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        return None;
    }
    dirs.iter()
        .filter(|d| d.is_absolute())
        .map(|d| d.join(name))
        .find(|p| is_executable_file(p))
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

/// Reads a candidate file if it is a regular file no larger than [`MAX_ENTRY_BYTES`].
pub(crate) fn read_small(path: &Path) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_ENTRY_BYTES {
        return None;
    }
    // The file can grow between the check and the read, so the read is capped too.
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_ENTRY_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_ENTRY_BYTES).then_some(bytes)
}

/// One directory entry seen by [`walk`].
pub(crate) struct Visit<'a> {
    pub path: &'a Path,
    /// Path relative to the root, with `/` separators.
    pub relative: &'a str,
    /// After following a symlink.
    pub is_dir: bool,
}

/// Depth-first walk of `root`, in sorted order for deterministic results. `visit` returns
/// whether to descend into a directory. Symlinked directories are reported but never entered,
/// so links cannot loop. Unreadable directories are passed over: scans run over system trees
/// where some folders are expected to be private.
pub(crate) fn walk(root: &Path, max_depth: usize, visit: &mut dyn FnMut(&Visit<'_>) -> bool) {
    let mut budget = MAX_VISITED;
    walk_dir(root, "", 0, max_depth, &mut budget, visit);
}

fn walk_dir(
    dir: &Path,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    budget: &mut usize,
    visit: &mut dyn FnMut(&Visit<'_>) -> bool,
) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = read.filter_map(Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        let is_symlink = kind.is_symlink();
        let is_dir = if is_symlink {
            path.is_dir()
        } else {
            kind.is_dir()
        };
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let descend = visit(&Visit {
            path: &path,
            relative: &relative,
            is_dir,
        });
        if descend && is_dir && !is_symlink && depth < max_depth {
            walk_dir(&path, &relative, depth + 1, max_depth, budget, visit);
        }
    }
}

/// A display name worth showing: non-empty after trimming, bounded, no control characters.
pub(crate) fn clean_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()
        && name.chars().count() <= MAX_NAME_LEN
        && !name.chars().any(char::is_control))
    .then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_name_rejects_empty_long_and_control() {
        assert_eq!(clean_name("  Editor "), Some("Editor".into()));
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name("a\u{7}b"), None);
        assert_eq!(clean_name(&"x".repeat(MAX_NAME_LEN + 1)), None);
    }

    #[test]
    fn find_in_ignores_names_with_separators_and_relative_dirs() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(find_in(&[dir.path().into()], "../sh"), None);
        assert_eq!(find_in(&["relative".into()], "sh"), None);
        assert_eq!(find_in(&[dir.path().into()], "missing"), None);
    }

    #[cfg(unix)]
    #[test]
    fn find_in_requires_the_execute_bit() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("tool");
        std::fs::write(&tool, "").unwrap();
        assert_eq!(find_in(&[dir.path().into()], "tool"), None);
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(find_in(&[dir.path().into()], "tool"), Some(tool));
    }

    #[cfg(unix)]
    #[test]
    fn walk_does_not_enter_symlinked_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("real")).unwrap();
        std::fs::write(dir.path().join("real/a"), "").unwrap();
        std::os::unix::fs::symlink(dir.path(), dir.path().join("real/loop")).unwrap();
        let mut seen = Vec::new();
        walk(dir.path(), 8, &mut |v| {
            seen.push(v.relative.to_owned());
            true
        });
        assert_eq!(seen, ["real", "real/a", "real/loop"]);
    }

    #[test]
    fn read_small_skips_large_files() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big");
        std::fs::write(
            &big,
            vec![b'x'; usize::try_from(MAX_ENTRY_BYTES).unwrap() + 1],
        )
        .unwrap();
        assert_eq!(read_small(&big), None);
        assert_eq!(read_small(dir.path()), None, "directories are not read");
    }
}
