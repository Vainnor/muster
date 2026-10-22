//! macOS: `.app` bundles in `/Applications`, `/System/Applications` and `~/Applications`.
//!
//! To implement: read `Contents/Info.plist` (`CFBundleName`/`CFBundleDisplayName`,
//! `CFBundleIdentifier`, `CFBundleIconFile`), skip bundles with `LSUIElement` or
//! `LSBackgroundOnly`, and emit `LaunchTarget::AppBundle`. Launching goes through
//! LaunchServices (`open -a` is never built as a string).

use std::path::PathBuf;

use super::DetectedApp;

#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    roots
}

pub fn scan(_roots: &[PathBuf]) -> crate::Result<Vec<DetectedApp>> {
    Err(crate::Error::NotImplemented("macos app detection"))
}
