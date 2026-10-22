//! Finding installed apps.
//!
//! Each OS module scans the roots it is given and returns what it found; it never decides
//! where to look on its own, so tests pass fixture directories
//! (`tests/fixtures/{linux,macos,windows}`) instead of the real machine. The parsers are
//! compiled on every OS so all of them are tested on every CI runner.
//!
//! Detection runs on demand (startup, refresh), never on a timer.

use std::path::PathBuf;

use crate::launch::LaunchTarget;

pub mod linux;
pub mod macos;
pub mod windows;

/// An app found on disk, before it becomes an [`AppEntry`](crate::model::AppEntry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedApp {
    pub name: String,
    pub target: LaunchTarget,
    pub icon: Option<PathBuf>,
}

/// Scans the standard app locations of the OS this build runs on.
pub fn detect_installed() -> crate::Result<Vec<DetectedApp>> {
    #[cfg(target_os = "linux")]
    return linux::scan(&linux::default_roots());
    #[cfg(target_os = "macos")]
    return macos::scan(&macos::default_roots());
    #[cfg(target_os = "windows")]
    return windows::scan(&windows::default_roots());
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    Ok(Vec::new())
}
