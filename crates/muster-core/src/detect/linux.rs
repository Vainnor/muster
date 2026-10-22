//! Linux: freedesktop `.desktop` entries under `$XDG_DATA_DIRS/applications` and
//! `$XDG_DATA_HOME/applications`.
//!
//! To implement: parse `[Desktop Entry]` (`Type=Application`, `Name`, `Exec`, `Icon`,
//! `NoDisplay`, `Hidden`, `TryExec`), and tokenize `Exec` per the Desktop Entry spec's quoting
//! rules into an argument array. Field codes: `%%` becomes `%`; `%f %F %u %U` are dropped
//! (Muster launches without files); `%i %c %k` are dropped; unknown codes reject the entry.
//! The result is never passed to a shell.

use std::path::{Path, PathBuf};

use super::DetectedApp;

/// The application directories from the XDG base-directory spec.
#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".local/share")));
    let data_dirs = std::env::var_os("XDG_DATA_DIRS").map_or_else(
        || vec!["/usr/local/share".into(), "/usr/share".into()],
        |v| std::env::split_paths(&v).collect::<Vec<_>>(),
    );
    data_home
        .into_iter()
        .chain(data_dirs)
        .map(|d| d.join("applications"))
        .collect()
}

pub fn scan(_roots: &[PathBuf]) -> crate::Result<Vec<DetectedApp>> {
    Err(crate::Error::NotImplemented("linux app detection"))
}

/// Parses one `.desktop` file.
pub fn parse_desktop_entry(_path: &Path, _contents: &str) -> crate::Result<Option<DetectedApp>> {
    Err(crate::Error::NotImplemented("desktop entry parsing"))
}
