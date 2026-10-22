//! Windows: Start Menu shortcuts (`.lnk`) under `%ProgramData%` and `%APPDATA%`
//! `\Microsoft\Windows\Start Menu\Programs`.
//!
//! To implement: resolve each `.lnk` to its target, arguments and working directory with a
//! maintained parser (no COM from core), skip uninstallers and targets that are not `.exe`,
//! and emit `LaunchTarget::Program`. Store (UWP) apps come later through `shell:AppsFolder`.

use std::path::PathBuf;

use super::DetectedApp;

#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    ["ProgramData", "APPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|base| PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs"))
        .collect()
}

pub fn scan(_roots: &[PathBuf]) -> crate::Result<Vec<DetectedApp>> {
    Err(crate::Error::NotImplemented("windows app detection"))
}
