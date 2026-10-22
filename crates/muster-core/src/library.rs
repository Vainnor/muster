//! The user's app library: detected apps merged with manually added ones, and the groups that
//! use them. Every change the UI asks for goes through here, so the rules live in one place.
//!
//! Ids: detected apps are `d:<key>` (the scanner's stable key), manual apps `m:<slug>`,
//! groups `g:<slug>`. Detected apps are not stored, except those a group uses: saving a group
//! pins a copy of each detected app it references into the config, so the group still loads
//! if the app is uninstalled or detection changes. A fresh scan replaces the pinned copy's
//! target in the listing.

use std::collections::{HashMap, HashSet};

use crate::detect::{DetectedApp, Scan};
use crate::groups::{self, Group, GroupEntry, GroupError, GroupId, Step};
use crate::launch::LaunchTarget;
use crate::model::{AppEntry, AppId, Source};
use crate::store::Config;
use crate::{Error, Result};

/// Limits on stored items, to bound config size.
pub const MAX_MANUAL_APPS: usize = 1_024;
pub const MAX_GROUPS: usize = 256;
pub const MAX_APP_NAME_LEN: usize = 128;

const DETECTED: &str = "d:";
const MANUAL: &str = "m:";
const GROUP: &str = "g:";

/// A group as the UI submits it. `id` is `None` for a new group.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupDraft {
    #[serde(default)]
    pub id: Option<GroupId>,
    pub name: String,
    pub entries: Vec<GroupEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct Library {
    config: Config,
    detected: Vec<AppEntry>,
    scanned: bool,
}

impl Library {
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            detected: Vec::new(),
            scanned: false,
        }
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Whether a scan result has been applied this session.
    #[must_use]
    pub fn scanned(&self) -> bool {
        self.scanned
    }

    /// Replaces the detected apps with a fresh scan.
    pub fn set_detected(&mut self, scan: &Scan) {
        self.detected = scan.apps.iter().map(detected_entry).collect();
        self.scanned = true;
    }

    /// Every app, sorted by name: detected apps, then pinned detected apps that the last scan
    /// did not find, then manual apps.
    #[must_use]
    pub fn apps(&self) -> Vec<AppEntry> {
        let fresh: HashSet<&AppId> = self.detected.iter().map(|a| &a.id).collect();
        let mut apps: Vec<AppEntry> = self
            .detected
            .iter()
            .chain(self.config.apps.iter().filter(|a| !fresh.contains(&a.id)))
            .cloned()
            .collect();
        apps.sort_by_cached_key(|a| (a.name.to_lowercase(), a.id.0.clone()));
        apps
    }

    #[must_use]
    pub fn find(&self, id: &AppId) -> Option<AppEntry> {
        self.detected
            .iter()
            .chain(&self.config.apps)
            .find(|a| &a.id == id)
            .cloned()
    }

    /// Adds an app by hand. The target is validated now and again whenever it launches.
    pub fn add_manual(&mut self, name: &str, target: LaunchTarget) -> Result<AppId> {
        let name = clean(name, MAX_APP_NAME_LEN).ok_or(Error::InvalidName)?;
        crate::launch::validate(&target)?;
        let manual = self
            .config
            .apps
            .iter()
            .filter(|a| a.source == Source::Manual)
            .count();
        if manual >= MAX_MANUAL_APPS {
            return Err(Error::LimitReached);
        }
        let taken: HashSet<&str> = self.config.apps.iter().map(|a| a.id.0.as_str()).collect();
        let id = AppId(unique_id(MANUAL, &name, &taken));
        self.config.apps.push(AppEntry {
            id: id.clone(),
            name,
            target,
            source: Source::Manual,
        });
        Ok(id)
    }

    /// Removes a manual app and every group entry that used it.
    pub fn remove_manual(&mut self, id: &AppId) -> Result<()> {
        let before = self.config.apps.len();
        self.config
            .apps
            .retain(|a| !(a.id == *id && a.source == Source::Manual));
        if self.config.apps.len() == before {
            return Err(Error::UnknownId);
        }
        for group in &mut self.config.groups {
            group.entries.retain(|e| e.app != *id);
        }
        Ok(())
    }

    /// Creates or replaces a group. A group that was confirmed stays confirmed only if it gains
    /// no manual entry it did not already have.
    pub fn save_group(&mut self, draft: GroupDraft) -> Result<GroupId> {
        let name = clean(&draft.name, groups::MAX_GROUP_NAME_LEN)
            .ok_or(Error::InvalidGroup(GroupError::BadName))?;
        let existing = match &draft.id {
            Some(id) => Some(self.group(id).ok_or(Error::UnknownId)?.clone()),
            None if self.config.groups.len() >= MAX_GROUPS => return Err(Error::LimitReached),
            None => None,
        };
        let id = if let Some(g) = &existing {
            g.id.clone()
        } else {
            let taken: HashSet<&str> = self.config.groups.iter().map(|g| g.id.0.as_str()).collect();
            GroupId(unique_id(GROUP, &name, &taken))
        };
        let manual_in = |entries: &[GroupEntry]| -> HashSet<AppId> {
            entries
                .iter()
                .filter(|e| {
                    self.find(&e.app)
                        .is_some_and(|a| a.source == Source::Manual)
                })
                .map(|e| e.app.clone())
                .collect()
        };
        let confirmed = existing.as_ref().is_some_and(|old| {
            old.confirmed && manual_in(&draft.entries).is_subset(&manual_in(&old.entries))
        });
        let group = Group {
            id: id.clone(),
            name,
            entries: draft.entries,
            hotkey: existing.and_then(|g| g.hotkey),
            confirmed,
        };

        let mut next = self.config.clone();
        next.groups.retain(|g| g.id != id);
        next.groups.push(group);
        self.pin_detected(&mut next);
        let group = next
            .groups
            .iter()
            .find(|g| g.id == id)
            .ok_or(Error::UnknownId)?;
        group.validate(&next.apps)?;
        self.config = next;
        Ok(id)
    }

    pub fn delete_group(&mut self, id: &GroupId) -> Result<()> {
        let before = self.config.groups.len();
        self.config.groups.retain(|g| g.id != *id);
        if self.config.groups.len() == before {
            return Err(Error::UnknownId);
        }
        let mut next = self.config.clone();
        self.pin_detected(&mut next);
        self.config = next;
        Ok(())
    }

    /// Records the user's confirmation of a group's manual entries.
    pub fn confirm_group(&mut self, id: &GroupId) -> Result<()> {
        let group = self
            .config
            .groups
            .iter_mut()
            .find(|g| g.id == *id)
            .ok_or(Error::UnknownId)?;
        group.confirmed = true;
        Ok(())
    }

    #[must_use]
    pub fn group(&self, id: &GroupId) -> Option<&Group> {
        self.config.groups.iter().find(|g| g.id == *id)
    }

    /// The manual apps in a group, which the confirmation shows in full.
    #[must_use]
    pub fn manual_entries(&self, id: &GroupId) -> Vec<AppEntry> {
        let Some(group) = self.group(id) else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        group
            .entries
            .iter()
            .filter_map(|e| self.find(&e.app))
            .filter(|a| a.source == Source::Manual && seen.insert(a.id.clone()))
            .collect()
    }

    /// Plans a run of a group, using the freshest target for each app.
    pub fn plan_run(&self, id: &GroupId) -> Result<Vec<Step>> {
        let group = self.group(id).ok_or(Error::UnknownId)?;
        Ok(groups::plan_group(group, &self.apps())?)
    }

    /// Keeps exactly the detected apps that groups reference in `config.apps`, refreshed from
    /// the latest scan.
    fn pin_detected(&self, config: &mut Config) {
        let used: HashSet<AppId> = config
            .groups
            .iter()
            .flat_map(|g| g.entries.iter().map(|e| e.app.clone()))
            .collect();
        let fresh: HashMap<&AppId, &AppEntry> = self.detected.iter().map(|a| (&a.id, a)).collect();
        let mut pinned: HashSet<AppId> = HashSet::new();
        config.apps.retain_mut(|a| {
            if a.source == Source::Manual {
                return true;
            }
            if !used.contains(&a.id) {
                return false;
            }
            if let Some(f) = fresh.get(&a.id) {
                *a = (*f).clone();
            }
            pinned.insert(a.id.clone());
            true
        });
        for id in &used {
            if !pinned.contains(id)
                && let Some(f) = fresh.get(id)
            {
                config.apps.push((*f).clone());
            }
        }
    }
}

/// The library entry for a detected app.
#[must_use]
pub fn detected_entry(app: &DetectedApp) -> AppEntry {
    AppEntry {
        id: AppId(format!("{DETECTED}{}", app.key)),
        name: app.name.clone(),
        target: app.target.clone(),
        source: Source::Detected,
    }
}

fn clean(name: &str, max: usize) -> Option<String> {
    let name = name.trim();
    let len = name.chars().count();
    (len > 0 && len <= max && !name.chars().any(char::is_control)).then(|| name.to_owned())
}

/// `prefix` + a slug of `name` (lowercase ASCII letters, digits and `-`, at most 32), with a
/// numeric suffix if it is taken.
fn unique_id(prefix: &str, name: &str, taken: &HashSet<&str>) -> String {
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        if slug.len() >= 32 {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    let slug = if slug.is_empty() { "item" } else { slug };
    let base = format!("{prefix}{slug}");
    if !taken.contains(base.as_str()) {
        return base;
    }
    // `taken` has at most `MAX_MANUAL_APPS` or `MAX_GROUPS` ids, so a free suffix exists
    // within one more than that.
    (2..=taken.len() + 2)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken.contains(id.as_str()))
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn url(u: &str) -> LaunchTarget {
        LaunchTarget::Url { url: u.into() }
    }

    fn scan(apps: &[(&str, &str)]) -> Scan {
        Scan {
            apps: apps
                .iter()
                .map(|(key, name)| DetectedApp {
                    key: (*key).into(),
                    name: (*name).into(),
                    target: url(&format!("https://example.com/{key}")),
                    icon: None,
                })
                .collect(),
            skipped: 0,
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

    fn draft(id: Option<&GroupId>, name: &str, entries: &[&str]) -> GroupDraft {
        GroupDraft {
            id: id.cloned(),
            name: name.into(),
            entries: entries.iter().map(|e| entry(e)).collect(),
        }
    }

    #[test]
    fn lists_detected_and_manual_apps_by_name() {
        let mut lib = Library::new(Config::default());
        lib.set_detected(&scan(&[("zed", "Zed"), ("editor", "editor")]));
        lib.add_manual("Blog", url("https://blog.example")).unwrap();
        let names: Vec<_> = lib.apps().into_iter().map(|a| (a.id.0, a.name)).collect();
        assert_eq!(
            names,
            [
                ("m:blog".into(), "Blog".into()),
                ("d:editor".into(), "editor".into()),
                ("d:zed".into(), "Zed".into())
            ]
        );
    }

    #[test]
    fn manual_ids_are_unique_slugs() {
        let mut lib = Library::new(Config::default());
        let a = lib
            .add_manual("My Tool!", url("https://a.example"))
            .unwrap();
        let b = lib.add_manual("my tool", url("https://b.example")).unwrap();
        let c = lib.add_manual("☃", url("https://c.example")).unwrap();
        assert_eq!(
            (a.0.as_str(), b.0.as_str(), c.0.as_str()),
            ("m:my-tool", "m:my-tool-2", "m:item")
        );
    }

    #[test]
    fn add_manual_validates_name_and_target() {
        let mut lib = Library::new(Config::default());
        assert!(matches!(
            lib.add_manual("  ", url("https://a.example")),
            Err(Error::InvalidName)
        ));
        assert!(matches!(
            lib.add_manual("x", url("javascript:alert(1)")),
            Err(Error::InvalidTarget(_))
        ));
        let missing = LaunchTarget::Program {
            path: PathBuf::from("/definitely/missing"),
            args: vec![],
            cwd: None,
        };
        assert!(matches!(
            lib.add_manual("x", missing),
            Err(Error::InvalidTarget(_))
        ));
    }

    #[test]
    fn saving_a_group_pins_the_detected_apps_it_uses() {
        let mut lib = Library::new(Config::default());
        lib.set_detected(&scan(&[("editor", "Editor"), ("browser", "Browser")]));
        let id = lib.save_group(draft(None, "Work", &["d:editor"])).unwrap();
        assert_eq!(id.0, "g:work");
        let pinned: Vec<_> = lib.config().apps.iter().map(|a| a.id.0.as_str()).collect();
        assert_eq!(pinned, ["d:editor"]);

        // The app vanishes from a later scan; the group and the pinned copy survive.
        lib.set_detected(&scan(&[("browser", "Browser")]));
        assert!(lib.apps().iter().any(|a| a.id.0 == "d:editor"));
        assert!(lib.plan_run(&id).is_ok());

        lib.delete_group(&id).unwrap();
        assert!(lib.config().apps.is_empty(), "unused pins are dropped");
    }

    #[test]
    fn save_group_rejects_unknown_apps_and_bad_names() {
        let mut lib = Library::new(Config::default());
        assert!(matches!(
            lib.save_group(draft(None, "Work", &["d:nope"])),
            Err(Error::InvalidGroup(GroupError::UnknownApp(_)))
        ));
        assert!(matches!(
            lib.save_group(draft(None, "", &[])),
            Err(Error::InvalidGroup(GroupError::BadName))
        ));
        assert!(matches!(
            lib.save_group(draft(Some(&GroupId("g:none".into())), "X", &[])),
            Err(Error::UnknownId)
        ));
        assert!(
            lib.config().groups.is_empty(),
            "a failed save changes nothing"
        );
    }

    #[test]
    fn adding_a_new_manual_entry_requires_confirmation_again() {
        let mut lib = Library::new(Config::default());
        lib.set_detected(&scan(&[("editor", "Editor")]));
        let a = lib
            .add_manual("Script A", url("https://a.example"))
            .unwrap();
        let b = lib
            .add_manual("Script B", url("https://b.example"))
            .unwrap();
        let id = lib.save_group(draft(None, "Work", &[&a.0])).unwrap();

        assert!(matches!(
            lib.plan_run(&id),
            Err(Error::InvalidGroup(GroupError::NeedsConfirmation))
        ));
        assert_eq!(lib.manual_entries(&id).len(), 1);
        lib.confirm_group(&id).unwrap();
        assert!(lib.plan_run(&id).is_ok());

        // Reordering or adding a detected app keeps the confirmation.
        lib.save_group(draft(Some(&id), "Work", &["d:editor", &a.0]))
            .unwrap();
        assert!(lib.group(&id).unwrap().confirmed);
        // A manual entry the user has not confirmed clears it.
        lib.save_group(draft(Some(&id), "Work", &[&a.0, &b.0]))
            .unwrap();
        assert!(!lib.group(&id).unwrap().confirmed);
    }

    #[test]
    fn removing_a_manual_app_drops_it_from_groups() {
        let mut lib = Library::new(Config::default());
        let a = lib.add_manual("A", url("https://a.example")).unwrap();
        let id = lib.save_group(draft(None, "G", &[&a.0])).unwrap();
        lib.remove_manual(&a).unwrap();
        assert!(lib.group(&id).unwrap().entries.is_empty());
        assert!(matches!(
            lib.remove_manual(&AppId("d:x".into())),
            Err(Error::UnknownId)
        ));
    }

    #[test]
    fn saved_config_round_trips_through_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let mut lib = Library::new(Config::default());
        lib.set_detected(&scan(&[("editor", "Editor")]));
        let a = lib.add_manual("A", url("https://a.example")).unwrap();
        lib.save_group(draft(None, "G", &["d:editor", &a.0]))
            .unwrap();
        crate::store::save(dir.path(), lib.config()).unwrap();
        let loaded = crate::store::load(dir.path()).unwrap();
        assert_eq!(loaded.config(), lib.config());
    }
}
