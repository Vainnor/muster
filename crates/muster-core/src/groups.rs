//! Named groups of apps that launch together.
//!
//! A group is an ordered list of entries. Each entry points at an [`AppEntry`] by id and can
//! override its arguments and working directory and add a delay before it starts.
//! [`plan_group`] turns a group into launch steps; the shell runs them in order, sleeping for
//! each delay, and one failed entry does not stop the rest.
//!
//! An entry that runs a manually added app, or that overrides an app's arguments or working
//! directory, runs only once the user has approved its exact target ([`unapproved`]). The
//! shell asks in a native dialog; the webview cannot approve anything.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::launch::{LaunchPlan, LaunchTarget, MAX_ARG_LEN, MAX_ARGS, TargetError};
use crate::model::{AppEntry, AppId, Source};

pub const MAX_GROUP_NAME_LEN: usize = 64;
pub const MAX_GROUP_ENTRIES: usize = 32;
/// Longest delay one entry may wait before it starts.
pub const MAX_DELAY_MS: u32 = 60_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GroupId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    /// Launch order is the order of this list.
    pub entries: Vec<GroupEntry>,
    /// Global shortcut, for example `CmdOrCtrl+Alt+1`. Registered by the shell.
    #[serde(default)]
    pub hotkey: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[error("an entry's arguments are too many, too long or contain NUL")]
    BadArguments,
    #[error("an entry's working directory must be an absolute path without `..`")]
    BadWorkingDir,
    #[error("group runs a target the user has not approved")]
    NeedsConfirmation,
}

/// One entry of a planned group run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub app: AppId,
    /// Wait this long before starting this entry.
    pub delay: Duration,
    /// The validated plan, or why this entry cannot start. Checked now, at launch time.
    pub plan: Result<LaunchPlan, TargetError>,
}

/// Plans a run of `group`. Fails as a whole if the group is invalid or runs a target that is
/// not in `approved`; individual targets that fail validation become failed steps.
pub fn plan_group(
    group: &Group,
    apps: &[AppEntry],
    approved: &[LaunchTarget],
) -> Result<Vec<Step>, GroupError> {
    group.validate(apps)?;
    if !unapproved(group, apps, approved).is_empty() {
        return Err(GroupError::NeedsConfirmation);
    }
    let by_id: HashMap<&AppId, &AppEntry> = apps.iter().map(|a| (&a.id, a)).collect();
    group
        .entries
        .iter()
        .map(|entry| {
            let app = by_id
                .get(&entry.app)
                .ok_or_else(|| GroupError::UnknownApp(entry.app.clone()))?;
            Ok(Step {
                app: entry.app.clone(),
                delay: Duration::from_millis(u64::from(entry.delay_ms)),
                plan: crate::launch::plan(&entry.target_for(&app.target)),
            })
        })
        .collect()
}

/// The targets of `group` that need the user's approval and do not have it yet, in launch
/// order without repeats. These are exactly what the group would run for those entries,
/// overrides applied.
#[must_use]
pub fn unapproved(
    group: &Group,
    apps: &[AppEntry],
    approved: &[LaunchTarget],
) -> Vec<LaunchTarget> {
    let by_id: HashMap<&AppId, &AppEntry> = apps.iter().map(|a| (&a.id, a)).collect();
    let mut out: Vec<LaunchTarget> = Vec::new();
    for entry in &group.entries {
        let Some(app) = by_id.get(&entry.app) else {
            continue;
        };
        if !entry.needs_approval(app) {
            continue;
        }
        let target = entry.target_for(&app.target);
        if !approved.contains(&target) && !out.contains(&target) {
            out.push(target);
        }
    }
    out
}

impl GroupEntry {
    /// A manually added app, or any override of a detected app's arguments or folder, can run
    /// an arbitrary command line, so the user approves it first.
    #[must_use]
    pub fn needs_approval(&self, app: &AppEntry) -> bool {
        app.source == Source::Manual || self.args.is_some() || self.cwd.is_some()
    }

    /// The app's target with this entry's overrides. Only programs take arguments and a
    /// working directory; overrides on a bundle or URL entry are ignored.
    #[must_use]
    pub fn target_for(&self, base: &LaunchTarget) -> LaunchTarget {
        match base {
            LaunchTarget::Program { path, args, cwd } => LaunchTarget::Program {
                path: path.clone(),
                args: self.args.clone().unwrap_or_else(|| args.clone()),
                cwd: self.cwd.clone().or_else(|| cwd.clone()),
            },
            other => other.clone(),
        }
    }
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
            if let Some(args) = &entry.args
                && (args.len() > MAX_ARGS
                    || args
                        .iter()
                        .any(|a| a.len() > MAX_ARG_LEN || a.contains('\0')))
            {
                return Err(GroupError::BadArguments);
            }
            if let Some(cwd) = &entry.cwd
                && (!cwd.is_absolute()
                    || cwd
                        .components()
                        .any(|c| matches!(c, std::path::Component::ParentDir)))
            {
                return Err(GroupError::BadWorkingDir);
            }
            if !known.contains(&entry.app) {
                return Err(GroupError::UnknownApp(entry.app.clone()));
            }
        }
        Ok(())
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
    fn manual_entries_and_overrides_need_approval_of_their_exact_target() {
        let apps = [
            app("editor", Source::Detected),
            app("script", Source::Manual),
        ];
        assert!(unapproved(&group(vec![entry("editor")]), &apps, &[]).is_empty());

        let g = group(vec![entry("script"), entry("script")]);
        let pending = unapproved(&g, &apps, &[]);
        assert_eq!(pending, vec![apps[1].target.clone()], "listed once");
        assert!(unapproved(&g, &apps, &pending).is_empty());

        let mut overridden = entry("editor");
        overridden.cwd = Some(PathBuf::from("/tmp"));
        assert_eq!(unapproved(&group(vec![overridden]), &apps, &[]).len(), 1);
    }

    #[test]
    fn approval_covers_the_overridden_command_line_not_the_base() {
        let dir = tempfile::tempdir().unwrap();
        let sh = dir.path().join("sh");
        std::fs::write(&sh, "").unwrap();
        let apps = [program_app("sh", &sh, Source::Detected)];
        let mut e = entry("sh");
        e.args = Some(vec!["-c".into(), "id".into()]);
        let g = group(vec![e.clone()]);
        let approved = unapproved(&g, &apps, &[]);
        assert!(
            matches!(&approved[0], LaunchTarget::Program { args, .. } if args == &["-c", "id"]),
            "the approval shows the override"
        );
        assert!(plan_group(&g, &apps, &approved).is_ok());

        e.args = Some(vec!["-c".into(), "echo other".into()]);
        let changed = group(vec![e]);
        assert_eq!(
            plan_group(&changed, &apps, &approved),
            Err(GroupError::NeedsConfirmation)
        );
    }

    fn program_app(id: &str, path: &std::path::Path, source: Source) -> AppEntry {
        AppEntry {
            id: AppId(id.into()),
            name: id.into(),
            target: LaunchTarget::Program {
                path: path.into(),
                args: vec!["--default".into()],
                cwd: None,
            },
            source,
        }
    }

    #[test]
    fn plan_applies_overrides_delays_and_order() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool");
        std::fs::write(&exe, "").unwrap();
        let apps = [
            program_app("a", &exe, Source::Detected),
            app("web", Source::Detected),
        ];
        let mut first = entry("a");
        first.args = Some(vec!["--work".into()]);
        first.cwd = Some(dir.path().into());
        let mut second = entry("web");
        second.delay_ms = 1500;
        second.args = Some(vec!["ignored".into()]);

        let g = group(vec![first, second, entry("a")]);
        let approved = unapproved(&g, &apps, &[]);
        let steps = plan_group(&g, &apps, &approved).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(
            steps[0].plan,
            Ok(LaunchPlan::Spawn {
                program: exe.clone(),
                args: vec!["--work".into()],
                cwd: Some(dir.path().into())
            })
        );
        assert_eq!(steps[1].delay, Duration::from_millis(1500));
        assert!(matches!(steps[1].plan, Ok(LaunchPlan::OpenUrl { .. })));
        assert_eq!(
            steps[2].plan,
            Ok(LaunchPlan::Spawn {
                program: exe,
                args: vec!["--default".into()],
                cwd: None
            })
        );
    }

    #[test]
    fn rejects_bad_argument_and_folder_overrides() {
        let apps = [app("a", Source::Detected)];
        let mut e = entry("a");
        e.args = Some(vec!["x".into(); MAX_ARGS + 1]);
        assert_eq!(
            group(vec![e.clone()]).validate(&apps),
            Err(GroupError::BadArguments)
        );
        e.args = Some(vec!["a\0b".into()]);
        assert_eq!(
            group(vec![e.clone()]).validate(&apps),
            Err(GroupError::BadArguments)
        );
        e.args = None;
        for cwd in ["relative", "/a/../b"] {
            e.cwd = Some(cwd.into());
            assert_eq!(
                group(vec![e.clone()]).validate(&apps),
                Err(GroupError::BadWorkingDir),
                "{cwd}"
            );
        }
    }

    #[test]
    fn plan_keeps_going_past_a_missing_target() {
        let dir = tempfile::tempdir().unwrap();
        let apps = [
            program_app("gone", &dir.path().join("gone"), Source::Detected),
            app("web", Source::Detected),
        ];
        let steps = plan_group(&group(vec![entry("gone"), entry("web")]), &apps, &[]).unwrap();
        assert_eq!(steps[0].plan, Err(TargetError::NotFound));
        assert!(steps[1].plan.is_ok());
    }

    #[test]
    fn plan_refuses_an_unapproved_manual_entry() {
        let apps = [app("script", Source::Manual)];
        let g = group(vec![entry("script")]);
        assert_eq!(
            plan_group(&g, &apps, &[]),
            Err(GroupError::NeedsConfirmation)
        );
        assert!(plan_group(&g, &apps, &[apps[0].target.clone()]).is_ok());
    }
}
