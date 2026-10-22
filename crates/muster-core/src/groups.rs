//! Named groups of apps that launch together.
//!
//! A group is an ordered list of entries. Each entry points at an [`AppEntry`] by id and can
//! override its arguments and working directory and add a delay before it starts. Launching,
//! closing a group, and hotkeys are wired later; this module owns the data and its rules.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::{AppEntry, AppId, Source};

pub const MAX_GROUP_NAME_LEN: usize = 64;
pub const MAX_GROUP_ENTRIES: usize = 32;
/// Longest delay one entry may wait before it starts.
pub const MAX_DELAY_MS: u32 = 60_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GroupId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    /// Launch order is the order of this list.
    pub entries: Vec<GroupEntry>,
    /// Global shortcut, for example `CmdOrCtrl+Alt+1`. Registered by the shell.
    #[serde(default)]
    pub hotkey: Option<String>,
    /// Set once the user has confirmed a group that contains manually added entries.
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupEntry {
    pub app: AppId,
    /// Replaces the app's own arguments when set.
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    /// Wait this long after the previous entry started.
    #[serde(default)]
    pub delay_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GroupError {
    #[error("group name must be 1 to {MAX_GROUP_NAME_LEN} characters")]
    BadName,
    #[error("a group holds at most {MAX_GROUP_ENTRIES} entries")]
    TooManyEntries,
    #[error("delay is longer than {MAX_DELAY_MS} ms")]
    DelayTooLong,
    #[error("group refers to an app that does not exist")]
    UnknownApp(AppId),
}

impl Group {
    /// Checks the group's own rules and that every entry refers to a known app.
    pub fn validate(&self, apps: &[AppEntry]) -> Result<(), GroupError> {
        let name_len = self.name.trim().chars().count();
        if name_len == 0 || name_len > MAX_GROUP_NAME_LEN || self.name.chars().any(char::is_control)
        {
            return Err(GroupError::BadName);
        }
        if self.entries.len() > MAX_GROUP_ENTRIES {
            return Err(GroupError::TooManyEntries);
        }
        let known: HashSet<&AppId> = apps.iter().map(|a| &a.id).collect();
        for entry in &self.entries {
            if entry.delay_ms > MAX_DELAY_MS {
                return Err(GroupError::DelayTooLong);
            }
            if !known.contains(&entry.app) {
                return Err(GroupError::UnknownApp(entry.app.clone()));
            }
        }
        Ok(())
    }

    /// True when the group must be confirmed before it runs: it contains a manually added
    /// entry and the user has not confirmed it yet.
    #[must_use]
    pub fn needs_confirmation(&self, apps: &[AppEntry]) -> bool {
        if self.confirmed {
            return false;
        }
        let by_id: HashMap<&AppId, &AppEntry> = apps.iter().map(|a| (&a.id, a)).collect();
        self.entries
            .iter()
            .filter_map(|e| by_id.get(&e.app))
            .any(|a| a.source == Source::Manual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::LaunchTarget;

    fn app(id: &str, source: Source) -> AppEntry {
        AppEntry {
            id: AppId(id.into()),
            name: id.into(),
            target: LaunchTarget::Url {
                url: "https://example.com".into(),
            },
            source,
        }
    }

    fn entry(id: &str) -> GroupEntry {
        GroupEntry {
            app: AppId(id.into()),
            args: None,
            cwd: None,
            delay_ms: 0,
        }
    }

    fn group(entries: Vec<GroupEntry>) -> Group {
        Group {
            id: GroupId("work".into()),
            name: "Work".into(),
            entries,
            hotkey: None,
            confirmed: false,
        }
    }

    #[test]
    fn valid_group_passes() {
        let apps = [app("editor", Source::Detected)];
        assert_eq!(group(vec![entry("editor")]).validate(&apps), Ok(()));
    }

    #[test]
    fn rejects_blank_long_and_control_names() {
        for name in ["", "   ", &"x".repeat(MAX_GROUP_NAME_LEN + 1), "a\nb"] {
            let mut g = group(vec![]);
            g.name = name.to_owned();
            assert_eq!(g.validate(&[]), Err(GroupError::BadName), "{name:?}");
        }
    }

    #[test]
    fn rejects_unknown_app_and_long_delay() {
        let apps = [app("editor", Source::Detected)];
        assert_eq!(
            group(vec![entry("ghost")]).validate(&apps),
            Err(GroupError::UnknownApp(AppId("ghost".into())))
        );
        let mut slow = entry("editor");
        slow.delay_ms = MAX_DELAY_MS + 1;
        assert_eq!(
            group(vec![slow]).validate(&apps),
            Err(GroupError::DelayTooLong)
        );
    }

    #[test]
    fn rejects_too_many_entries() {
        let apps = [app("editor", Source::Detected)];
        let g = group(vec![entry("editor"); MAX_GROUP_ENTRIES + 1]);
        assert_eq!(g.validate(&apps), Err(GroupError::TooManyEntries));
    }

    #[test]
    fn manual_entry_needs_confirmation_until_confirmed() {
        let apps = [
            app("editor", Source::Detected),
            app("script", Source::Manual),
        ];
        assert!(!group(vec![entry("editor")]).needs_confirmation(&apps));
        let mut g = group(vec![entry("editor"), entry("script")]);
        assert!(g.needs_confirmation(&apps));
        g.confirmed = true;
        assert!(!g.needs_confirmation(&apps));
    }
}
