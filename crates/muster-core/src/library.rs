//! The user's app library: detected apps merged with manually added ones, and the groups that
//! use them. Every change the UI asks for goes through here, so the rules live in one place.
//!
//! Ids: detected apps are `d:<key>` (the scanner's stable key), manual apps `m:<slug>`,
//! groups `g:<slug>`. Detected apps are not stored, except those a group uses: saving a group
//! pins a copy of each detected app it references into the config, so the group still loads
//! if the app is uninstalled or detection changes. A fresh scan replaces the pinned copy's
//! target in the listing.
//!
//! Approvals: a manual app, or a group entry that is manual or overrides arguments or folder,
//! runs only if its exact target is in `config.approved`. Planning such a run fails with
//! [`Error::NeedsApproval`]; the shell shows [`Library::pending_for_app`] or
//! [`Library::pending_for_group`] in a native dialog and calls [`Library::approve`] on OK.
//! Approvals nothing can produce any more are dropped on every change.

use std::collections::{HashMap, HashSet};

use crate::detect::{DetectedApp, Scan};
use crate::groups::{self, Group, GroupEntry, GroupError, GroupId, Step};
use crate::launch::{LaunchPlan, LaunchTarget};
use crate::model::{AppEntry, AppId, Source};
use crate::store::Config;
use crate::themes::{self, Parsed, ThemeError, ValidTheme};
use crate::{Error, Result};

/// Limits on stored items, to bound config size.
pub const MAX_MANUAL_APPS: usize = 1_024;
pub const MAX_GROUPS: usize = 256;
pub const MAX_APP_NAME_LEN: usize = 128;
pub const MAX_THEMES: usize = 64;
/// Every manual app plus every group entry, each approved once.
pub const MAX_APPROVED: usize = MAX_MANUAL_APPS + MAX_GROUPS * groups::MAX_GROUP_ENTRIES;

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
        self.prune_approved();
        Ok(())
    }

    /// Creates or replaces a group. Approvals belong to targets, not groups, so an entry whose
    /// command line changes needs approving again.
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
        let group = Group {
            id: id.clone(),
            name,
            entries: draft.entries,
            hotkey: existing.and_then(|g| g.hotkey),
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
        self.prune_approved();
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
        self.prune_approved();
        Ok(())
    }

    /// What launching app `id` would run that the user has not approved: its target if it was
    /// added by hand and not approved, otherwise nothing.
    pub fn pending_for_app(&self, id: &AppId) -> Result<Vec<LaunchTarget>> {
        let app = self.find(id).ok_or(Error::UnknownId)?;
        let pending = app.source == Source::Manual && !self.config.approved.contains(&app.target);
        Ok(if pending {
            vec![app.target]
        } else {
            Vec::new()
        })
    }

    /// The exact targets group `id` would run that need approval and do not have it.
    pub fn pending_for_group(&self, id: &GroupId) -> Result<Vec<LaunchTarget>> {
        let group = self.group(id).ok_or(Error::UnknownId)?;
        Ok(groups::unapproved(
            group,
            &self.apps(),
            &self.config.approved,
        ))
    }

    /// Records that the user approved `targets`. Only the shell calls this, after the user
    /// pressed OK in a native dialog that listed them; targets no app or group can run are
    /// ignored.
    pub fn approve(&mut self, targets: &[LaunchTarget]) -> Result<()> {
        for target in targets {
            if !self.config.approved.contains(target) {
                self.config.approved.push(target.clone());
            }
        }
        self.prune_approved();
        if self.config.approved.len() > MAX_APPROVED {
            return Err(Error::LimitReached);
        }
        Ok(())
    }

    /// Plans launching one app. A manual app must be approved first.
    pub fn plan_app(&self, id: &AppId) -> Result<LaunchPlan> {
        if !self.pending_for_app(id)?.is_empty() {
            return Err(Error::NeedsApproval);
        }
        let app = self.find(id).ok_or(Error::UnknownId)?;
        Ok(crate::launch::plan(&app.target)?)
    }

    #[must_use]
    pub fn group(&self, id: &GroupId) -> Option<&Group> {
        self.config.groups.iter().find(|g| g.id == *id)
    }

    /// Plans a run of a group, using the freshest target for each app.
    pub fn plan_run(&self, id: &GroupId) -> Result<Vec<Step>> {
        let group = self.group(id).ok_or(Error::UnknownId)?;
        match groups::plan_group(group, &self.apps(), &self.config.approved) {
            Err(GroupError::NeedsConfirmation) => Err(Error::NeedsApproval),
            other => Ok(other?),
        }
    }

    /// A preset or imported theme by id.
    #[must_use]
    pub fn theme(&self, id: &str) -> Option<ValidTheme> {
        themes::preset(id).or_else(|| {
            self.config
                .themes
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| themes::revalidate(t.clone()).ok())
                .map(|p| p.theme)
        })
    }

    /// Imported themes, in import order.
    #[must_use]
    pub fn custom_themes(&self) -> Vec<ValidTheme> {
        self.config
            .themes
            .iter()
            .filter_map(|t| themes::revalidate(t.clone()).ok())
            .map(|p| p.theme)
            .collect()
    }

    /// Imports a theme file. An imported theme with the id of an earlier import replaces it;
    /// the id of a built-in theme is refused. Contrast problems are returned, not refused.
    pub fn import_theme(&mut self, bytes: &[u8]) -> Result<Parsed> {
        let parsed = themes::parse(bytes)?;
        let id = parsed.theme.theme().id.clone();
        if themes::is_reserved_id(&id) {
            return Err(ThemeError::ReservedId.into());
        }
        let existing = self.config.themes.iter().position(|t| t.id == id);
        if existing.is_none() && self.config.themes.len() >= MAX_THEMES {
            return Err(Error::LimitReached);
        }
        let mut theme = parsed.theme.theme().clone();
        theme.schema = None;
        match existing {
            Some(i) => {
                if let Some(slot) = self.config.themes.get_mut(i) {
                    *slot = theme;
                }
            }
            None => self.config.themes.push(theme),
        }
        Ok(parsed)
    }

    /// Removes an imported theme. If it was in use, Muster goes back to following the system.
    pub fn remove_theme(&mut self, id: &str) -> Result<()> {
        let before = self.config.themes.len();
        self.config.themes.retain(|t| t.id != id);
        if self.config.themes.len() == before {
            return Err(Error::UnknownId);
        }
        if self.config.settings.theme == id {
            self.config.settings.theme = "system".into();
        }
        Ok(())
    }

    /// Drops approvals that no manual app or approval-needing group entry would run.
    fn prune_approved(&mut self) {
        let apps = self.apps();
        let by_id: HashMap<&AppId, &AppEntry> = apps.iter().map(|a| (&a.id, a)).collect();
        let mut live: HashSet<LaunchTarget> = apps
            .iter()
            .filter(|a| a.source == Source::Manual)
            .map(|a| a.target.clone())
            .collect();
        for entry in self.config.groups.iter().flat_map(|g| &g.entries) {
            if let Some(app) = by_id.get(&entry.app)
                && entry.needs_approval(app)
            {
                live.insert(entry.target_for(&app.target));
            }
        }
        self.config.approved.retain(|t| live.contains(t));
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
    fn manual_apps_and_groups_run_only_after_their_targets_are_approved() {
        let mut lib = Library::new(Config::default());
        lib.set_detected(&scan(&[("editor", "Editor")]));
        let a = lib
            .add_manual("Script A", url("https://a.example"))
            .unwrap();
        let b = lib
            .add_manual("Script B", url("https://b.example"))
            .unwrap();
        assert!(matches!(lib.plan_app(&a), Err(Error::NeedsApproval)));
        assert!(
            lib.pending_for_app(&AppId("d:editor".into()))
                .unwrap()
                .is_empty()
        );

        let id = lib
            .save_group(draft(None, "Work", &["d:editor", &a.0]))
            .unwrap();
        assert!(matches!(lib.plan_run(&id), Err(Error::NeedsApproval)));
        let pending = lib.pending_for_group(&id).unwrap();
        assert_eq!(pending, vec![url("https://a.example")]);
        lib.approve(&pending).unwrap();
        assert!(lib.plan_run(&id).is_ok());
        assert!(lib.plan_app(&a).is_ok(), "the same target, approved once");

        // Reordering keeps the approval; a new manual entry needs its own.
        lib.save_group(draft(Some(&id), "Work", &[&a.0, "d:editor"]))
            .unwrap();
        assert!(lib.plan_run(&id).is_ok());
        lib.save_group(draft(Some(&id), "Work", &[&a.0, &b.0]))
            .unwrap();
        assert_eq!(
            lib.pending_for_group(&id).unwrap(),
            vec![url("https://b.example")]
        );
    }

    #[test]
    fn approvals_nothing_can_run_are_dropped() {
        let mut lib = Library::new(Config::default());
        let a = lib.add_manual("A", url("https://a.example")).unwrap();
        lib.approve(&lib.pending_for_app(&a).unwrap()).unwrap();
        lib.approve(&[url("https://never.example")]).unwrap();
        assert_eq!(lib.config().approved, vec![url("https://a.example")]);
        lib.remove_manual(&a).unwrap();
        assert!(lib.config().approved.is_empty());
    }

    #[test]
    fn changing_an_override_needs_approval_again() {
        let dir = tempfile::tempdir().unwrap();
        let sh = dir.path().join("sh");
        std::fs::write(&sh, "").unwrap();
        let mut lib = Library::new(Config::default());
        lib.set_detected(&Scan {
            apps: vec![DetectedApp {
                key: "sh".into(),
                name: "Shell".into(),
                target: LaunchTarget::Program {
                    path: sh,
                    args: vec![],
                    cwd: None,
                },
                icon: None,
            }],
            skipped: 0,
        });
        let with_args = |args: &[&str]| GroupDraft {
            id: None,
            name: "G".into(),
            entries: vec![GroupEntry {
                args: Some(args.iter().map(|s| (*s).into()).collect()),
                ..entry("d:sh")
            }],
        };
        let id = lib.save_group(with_args(&["-c", "id"])).unwrap();
        lib.approve(&lib.pending_for_group(&id).unwrap()).unwrap();
        assert!(lib.plan_run(&id).is_ok());
        lib.save_group(GroupDraft {
            id: Some(id.clone()),
            ..with_args(&["-c", "echo other"])
        })
        .unwrap();
        assert!(matches!(lib.plan_run(&id), Err(Error::NeedsApproval)));
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

    fn theme_json(id: &str, text: &str) -> Vec<u8> {
        let mut theme: serde_json::Value =
            serde_json::from_str(crate::themes::PRESETS[0].1).unwrap();
        theme["id"] = id.into();
        theme["name"] = "Mine".into();
        theme["colors"]["text"] = text.into();
        serde_json::to_vec(&theme).unwrap()
    }

    #[test]
    fn imports_replaces_and_removes_custom_themes() {
        let mut lib = Library::new(Config::default());
        let parsed = lib.import_theme(&theme_json("mine", "#000000")).unwrap();
        assert!(parsed.warnings.is_empty());
        assert!(lib.theme("mine").is_some());
        // Low contrast warns but still imports, replacing the earlier one.
        let parsed = lib.import_theme(&theme_json("mine", "#eeeeee")).unwrap();
        assert!(!parsed.warnings.is_empty());
        assert_eq!(lib.custom_themes().len(), 1);

        lib.config_mut().settings.theme = "mine".into();
        lib.remove_theme("mine").unwrap();
        assert_eq!(lib.config().settings.theme, "system");
        assert!(matches!(lib.remove_theme("mine"), Err(Error::UnknownId)));
    }

    #[test]
    fn import_refuses_built_in_ids_and_bad_files() {
        let mut lib = Library::new(Config::default());
        for id in ["dark", "system"] {
            assert!(matches!(
                lib.import_theme(&theme_json(id, "#000000")),
                Err(Error::InvalidTheme(ThemeError::ReservedId))
            ));
        }
        assert!(matches!(
            lib.import_theme(b"{}"),
            Err(Error::InvalidTheme(_))
        ));
        assert!(lib.custom_themes().is_empty());
    }

    #[test]
    fn store_rejects_a_tampered_custom_theme() {
        let mut lib = Library::new(Config::default());
        lib.import_theme(&theme_json("mine", "#000000")).unwrap();
        let mut json: serde_json::Value = serde_json::to_value(lib.config()).unwrap();
        json["themes"][0]["radius"] = 200.into();
        assert!(crate::store::parse(&serde_json::to_vec(&json).unwrap()).is_err());
        json["themes"][0]["radius"] = 4.into();
        json["themes"][0]["id"] = "dark".into();
        assert!(crate::store::parse(&serde_json::to_vec(&json).unwrap()).is_err());
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
