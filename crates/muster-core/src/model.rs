//! The entries a user sees in the launcher.

use serde::{Deserialize, Serialize};

use crate::launch::LaunchTarget;

/// Stable identifier of an app entry, unique within one config.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppId(pub String);

/// Where an entry came from. Manually added entries are treated as less trusted: a group that
/// contains one asks for confirmation before its first run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Detected,
    Manual,
}

/// One launchable thing: a detected app, or an executable, script or URL the user added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppEntry {
    pub id: AppId,
    pub name: String,
    pub target: LaunchTarget,
    pub source: Source,
}
