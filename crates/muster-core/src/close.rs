//! Closing a group: which running processes belong to its apps.
//!
//! Closing quits every running copy of each entry's app that belongs to the user, whether
//! Muster started it or not. The shell lists the processes, asks the matches to quit, and
//! offers to force-quit what is left; this module only decides what matches.
//!
//! A shell or interpreter with no arguments of its own never matches: closing a group that runs
//! `/bin/bash` must not close every terminal. An entry with arguments matches only processes
//! running exactly those arguments.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::groups::{Group, GroupError};
use crate::launch::LaunchTarget;
use crate::model::{AppEntry, AppId};

/// One process as the shell saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningProcess {
    pub pid: u32,
    /// The executable, as the OS reports it (symlinks resolved).
    pub exe: PathBuf,
    /// Arguments after the program name.
    pub args: Vec<String>,
    /// Owned by the user running Muster.
    pub mine: bool,
    pub parent: Option<u32>,
}

/// What closing does for one entry of the group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseMatch {
    /// These processes run the entry's app.
    Running(Vec<u32>),
    NotRunning,
    /// A link has no process to close.
    Link,
    /// A shell or interpreter without arguments; closing it would close unrelated programs.
    TooBroad,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryClose {
    pub app: AppId,
    pub matched: CloseMatch,
}

/// Matches each entry of `group` to running processes. A process is listed under the first
/// entry it matches only, and Muster's own process (`own_pid`) never matches. A match whose
/// parent matches the same entry is left out: browsers and editors run helpers from the same
/// program, and those close with the main process, while quitting them first looks like a
/// crash to the app.
pub fn plan_close(
    group: &Group,
    apps: &[AppEntry],
    processes: &[RunningProcess],
    own_pid: u32,
) -> Result<Vec<EntryClose>, GroupError> {
    group.validate(apps)?;
    let by_id: HashMap<&AppId, &AppEntry> = apps.iter().map(|a| (&a.id, a)).collect();
    let candidates: Vec<&RunningProcess> = processes
        .iter()
        .filter(|p| p.mine && p.pid != own_pid)
        .collect();
    let mut taken = HashSet::new();
    let mut out = Vec::new();
    for entry in &group.entries {
        let app = by_id
            .get(&entry.app)
            .ok_or_else(|| GroupError::UnknownApp(entry.app.clone()))?;
        let target = entry.target_for(&app.target);
        let matched = match &target {
            LaunchTarget::Url { .. } => CloseMatch::Link,
            LaunchTarget::Program { path, args, .. } if args.is_empty() && is_shell(path) => {
                CloseMatch::TooBroad
            }
            _ => {
                let matched: Vec<&RunningProcess> = candidates
                    .iter()
                    .copied()
                    .filter(|p| matches(&target, p) && taken.insert(p.pid))
                    .collect();
                let mains: HashSet<u32> = matched.iter().map(|p| p.pid).collect();
                let pids: Vec<u32> = matched
                    .iter()
                    .filter(|p| !p.parent.is_some_and(|parent| mains.contains(&parent)))
                    .map(|p| p.pid)
                    .collect();
                if pids.is_empty() {
                    CloseMatch::NotRunning
                } else {
                    CloseMatch::Running(pids)
                }
            }
        };
        out.push(EntryClose {
            app: entry.app.clone(),
            matched,
        });
    }
    Ok(out)
}

/// Whether `process` runs `target`.
fn matches(target: &LaunchTarget, process: &RunningProcess) -> bool {
    match target {
        LaunchTarget::Program { path, args, .. } => {
            let path = resolved(path);
            let runs_it = same_path(&process.exe, &path);
            // A script runs as its interpreter with the script as the first argument.
            let interprets_it = is_shell(&process.exe)
                && process
                    .args
                    .first()
                    .is_some_and(|a| same_path(Path::new(a), &path));
            if runs_it {
                args.is_empty() || process.args == *args
            } else if interprets_it {
                args.is_empty() || process.args.get(1..) == Some(args.as_slice())
            } else {
                false
            }
        }
        // The bundle's main executable, not helpers elsewhere in the bundle.
        LaunchTarget::AppBundle { path } => process
            .exe
            .parent()
            .is_some_and(|dir| same_path(dir, &resolved(path).join("Contents/MacOS"))),
        LaunchTarget::Url { .. } => false,
    }
}

/// The OS reports executables with symlinks resolved (`/usr/bin/firefox` is often a link), so
/// the target is resolved too. A target that cannot be resolved is compared as written.
fn resolved(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned())
}

#[cfg(windows)]
fn same_path(a: &Path, b: &Path) -> bool {
    a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
}

#[cfg(not(windows))]
fn same_path(a: &Path, b: &Path) -> bool {
    a == b
}

/// Shells and script interpreters: running one with no arguments says nothing about which
/// program it is.
const SHELLS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "fish",
    "dash",
    "ksh",
    "tcsh",
    "csh",
    "nu",
    "env",
    "cmd",
    "pwsh",
    "powershell",
    "wscript",
    "cscript",
    "python",
    "node",
    "ruby",
    "perl",
    "php",
    "lua",
    "osascript",
    "java",
];

fn is_shell(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    let stem = stem.to_ascii_lowercase();
    // `python3.12`, `pwsh-preview` and the like.
    let base = stem
        .split(|c: char| c.is_ascii_digit() || c == '.' || c == '-')
        .next()
        .unwrap_or_default();
    SHELLS.contains(&base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groups::{GroupEntry, GroupId};
    use crate::model::Source;

    fn app(id: &str, target: LaunchTarget) -> AppEntry {
        AppEntry {
            id: AppId(id.into()),
            name: id.into(),
            source: Source::Manual,
            target,
        }
    }

    fn program(path: &str, args: &[&str]) -> LaunchTarget {
        LaunchTarget::Program {
            path: path.into(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            cwd: None,
        }
    }

    fn group(entries: &[(&str, Option<&[&str]>)]) -> Group {
        Group {
            id: GroupId("g:g".into()),
            name: "G".into(),
            entries: entries
                .iter()
                .map(|(id, args)| GroupEntry {
                    app: AppId((*id).into()),
                    args: args.map(|a| a.iter().map(|s| (*s).to_owned()).collect()),
                    cwd: None,
                    delay_ms: 0,
                })
                .collect(),
            hotkey: None,
        }
    }

    fn process(pid: u32, exe: &str, args: &[&str]) -> RunningProcess {
        RunningProcess {
            pid,
            exe: exe.into(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            mine: true,
            parent: None,
        }
    }

    fn close(
        apps: &[AppEntry],
        group: &Group,
        processes: &[RunningProcess],
    ) -> Vec<(String, CloseMatch)> {
        plan_close(group, apps, processes, 1)
            .unwrap()
            .into_iter()
            .map(|e| (e.app.0, e.matched))
            .collect()
    }

    #[test]
    fn closes_every_copy_of_a_program_whoever_started_it() {
        let apps = [app("m:editor", program("/opt/editor/editor", &[]))];
        let processes = [
            process(10, "/opt/editor/editor", &["notes.txt"]),
            process(11, "/opt/editor/editor", &[]),
            process(12, "/opt/other/other", &[]),
        ];
        assert_eq!(
            close(&apps, &group(&[("m:editor", None)]), &processes),
            [("m:editor".into(), CloseMatch::Running(vec![10, 11]))]
        );
    }

    #[test]
    fn an_entry_with_arguments_closes_only_that_command_line() {
        let apps = [app(
            "m:sync",
            program("/opt/sync/sync", &["--watch", "work"]),
        )];
        let processes = [
            process(10, "/opt/sync/sync", &["--watch", "work"]),
            process(11, "/opt/sync/sync", &["--watch", "home"]),
        ];
        assert_eq!(
            close(&apps, &group(&[("m:sync", None)]), &processes),
            [("m:sync".into(), CloseMatch::Running(vec![10]))]
        );
        // A group override replaces the app's own arguments, for matching too.
        assert_eq!(
            close(
                &apps,
                &group(&[("m:sync", Some(&["--watch", "home"]))]),
                &processes
            ),
            [("m:sync".into(), CloseMatch::Running(vec![11]))]
        );
    }

    #[test]
    fn helpers_close_with_their_main_process() {
        let apps = [app("m:browser", program("/opt/browser/browser", &[]))];
        let mut helper = process(11, "/opt/browser/browser", &["--type=renderer"]);
        helper.parent = Some(10);
        // A second window started on its own has a parent outside the app.
        let mut other = process(12, "/opt/browser/browser", &[]);
        other.parent = Some(2);
        let processes = [process(10, "/opt/browser/browser", &[]), helper, other];
        assert_eq!(
            close(&apps, &group(&[("m:browser", None)]), &processes),
            [("m:browser".into(), CloseMatch::Running(vec![10, 12]))]
        );
    }

    #[test]
    fn a_bare_shell_is_too_broad_to_close() {
        let apps = [app("m:shell", program("/bin/bash", &[]))];
        let processes = [process(10, "/bin/bash", &[])];
        assert_eq!(
            close(&apps, &group(&[("m:shell", None)]), &processes),
            [("m:shell".into(), CloseMatch::TooBroad)]
        );
    }

    #[test]
    fn a_shell_with_a_script_closes_only_that_script() {
        let apps = [app(
            "m:backup",
            program("/bin/bash", &["/home/someone/backup.sh"]),
        )];
        let processes = [
            process(10, "/bin/bash", &["/home/someone/backup.sh"]),
            process(11, "/bin/bash", &[]),
        ];
        assert_eq!(
            close(&apps, &group(&[("m:backup", None)]), &processes),
            [("m:backup".into(), CloseMatch::Running(vec![10]))]
        );
    }

    #[test]
    fn a_script_target_matches_its_interpreter_process() {
        let apps = [app("m:tool", program("/home/someone/bin/tool.py", &[]))];
        let processes = [
            process(
                10,
                "/usr/bin/python3.12",
                &["/home/someone/bin/tool.py", "--serve"],
            ),
            process(11, "/usr/bin/python3.12", &["/home/someone/other.py"]),
            // An editor with the script open is not running it.
            process(12, "/opt/editor/editor", &["/home/someone/bin/tool.py"]),
        ];
        assert_eq!(
            close(&apps, &group(&[("m:tool", None)]), &processes),
            [("m:tool".into(), CloseMatch::Running(vec![10]))]
        );
    }

    #[test]
    fn a_bundle_closes_its_main_executable_not_helpers() {
        let apps = [app(
            "d:browser",
            LaunchTarget::AppBundle {
                path: "/Applications/Browser.app".into(),
            },
        )];
        let processes = [
            process(10, "/Applications/Browser.app/Contents/MacOS/Browser", &[]),
            process(
                11,
                "/Applications/Browser.app/Contents/Frameworks/Helper.app/Contents/MacOS/Helper",
                &[],
            ),
        ];
        assert_eq!(
            close(&apps, &group(&[("d:browser", None)]), &processes),
            [("d:browser".into(), CloseMatch::Running(vec![10]))]
        );
    }

    #[test]
    fn links_cannot_close_and_absent_apps_are_not_running() {
        let apps = [
            app(
                "m:site",
                LaunchTarget::Url {
                    url: "https://example.com".into(),
                },
            ),
            app("m:editor", program("/opt/editor/editor", &[])),
        ];
        assert_eq!(
            close(&apps, &group(&[("m:site", None), ("m:editor", None)]), &[]),
            [
                ("m:site".into(), CloseMatch::Link),
                ("m:editor".into(), CloseMatch::NotRunning),
            ]
        );
    }

    #[test]
    fn skips_other_users_muster_itself_and_repeats() {
        let apps = [app("m:editor", program("/opt/editor/editor", &[]))];
        let mut theirs = process(10, "/opt/editor/editor", &[]);
        theirs.mine = false;
        let processes = [
            theirs,
            process(1, "/opt/editor/editor", &[]),
            process(12, "/opt/editor/editor", &[]),
        ];
        assert_eq!(
            close(
                &apps,
                &group(&[("m:editor", None), ("m:editor", None)]),
                &processes
            ),
            [
                ("m:editor".into(), CloseMatch::Running(vec![12])),
                ("m:editor".into(), CloseMatch::NotRunning),
            ]
        );
    }

    #[test]
    fn recognizes_shells_and_interpreters_by_name() {
        for shell in [
            "/bin/sh",
            "/usr/bin/python3.12",
            "cmd.exe",
            "PowerShell.exe",
            "/usr/bin/env",
            "pwsh-preview",
        ] {
            assert!(is_shell(Path::new(shell)), "{shell}");
        }
        for program in ["/opt/editor/editor", "/usr/bin/bashtop", "/usr/bin/nodepad"] {
            assert!(!is_shell(Path::new(program)), "{program}");
        }
    }
}
