//! Closing a group: which running processes belong to its apps.
//!
//! Closing quits every running copy of each entry's app that belongs to the user, whether
//! Muster started it or not. The shell lists the processes, asks the matches to quit, and
//! offers to force-quit what is left; this module only decides what matches.
//!
//! What may be closed is gated like what may run: a manual entry, or one that overrides
//! arguments or the folder, closes only once the user has approved its exact target, so a
//! page cannot add `/usr/bin/gnome-shell` as an app and close it. A shell, interpreter or
//! shared launcher with no arguments never matches, since closing a group that runs
//! `/bin/bash` must not close every terminal, and an entry with arguments matches only
//! processes running exactly those arguments.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use crate::groups::{Group, GroupError};
use crate::launch::LaunchTarget;
use crate::model::{AppEntry, AppId};

/// One process as the shell saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningProcess {
    pub pid: u32,
    /// The executable as the OS reports it. Linux and Windows resolve symlinks; matching also
    /// tries the target as written.
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
    /// A shell, interpreter or shared launcher without arguments; closing it would close
    /// unrelated programs.
    TooBroad,
    /// An `AppImage`: its process that matches serves the app's files, and quitting that first
    /// crashes the app instead of closing it.
    Unsupported,
    /// A target the user has not approved yet; it closes once the group has run it.
    NeedsApproval,
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
    approved: &[LaunchTarget],
    processes: &[RunningProcess],
    own_pid: u32,
) -> Result<Vec<EntryClose>, GroupError> {
    group.validate(apps)?;
    let by_id: HashMap<&AppId, &AppEntry> = apps.iter().map(|a| (&a.id, a)).collect();
    let pending = crate::groups::unapproved(group, apps, approved);
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
        let matched = match Target::new(&target) {
            Target::Link => CloseMatch::Link,
            _ if pending.contains(&target) => CloseMatch::NeedsApproval,
            Target::Program { names, args } => {
                if is_app_image(&names) {
                    CloseMatch::Unsupported
                } else if args.is_empty() && names.iter().any(|n| is_too_broad(n)) {
                    CloseMatch::TooBroad
                } else {
                    running(&candidates, &mut taken, |p| runs_program(p, &names, args))
                }
            }
            Target::Bundle { executables } => running(&candidates, &mut taken, |p| {
                p.exe
                    .parent()
                    .is_some_and(|dir| executables.iter().any(|e| same_path(dir, e)))
            }),
        };
        out.push(EntryClose {
            app: entry.app.clone(),
            matched,
        });
    }
    Ok(out)
}

/// A target with its paths worked out once, rather than once per process.
enum Target<'a> {
    Link,
    /// The program as written and resolved (they differ through a symlink).
    Program {
        names: Vec<PathBuf>,
        args: &'a [String],
    },
    /// The bundle's `Contents/MacOS`, as written and resolved. Helpers elsewhere in the bundle
    /// do not match.
    Bundle {
        executables: Vec<PathBuf>,
    },
}

impl<'a> Target<'a> {
    fn new(target: &'a LaunchTarget) -> Self {
        match target {
            LaunchTarget::Url { .. } => Self::Link,
            LaunchTarget::Program { path, args, .. } => Self::Program {
                names: both_forms(path),
                args,
            },
            LaunchTarget::AppBundle { path } => Self::Bundle {
                executables: both_forms(path)
                    .into_iter()
                    .map(|p| p.join("Contents").join("MacOS"))
                    .collect(),
            },
        }
    }
}

fn running(
    candidates: &[&RunningProcess],
    taken: &mut HashSet<u32>,
    is_match: impl Fn(&RunningProcess) -> bool,
) -> CloseMatch {
    let matched: Vec<&RunningProcess> = candidates
        .iter()
        .copied()
        .filter(|p| is_match(p) && taken.insert(p.pid))
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

/// Whether `process` runs the program `names` with `args` (any arguments when `args` is
/// empty). A script runs as its interpreter with the script as the first argument.
fn runs_program(process: &RunningProcess, names: &[PathBuf], args: &[String]) -> bool {
    let is = |p: &Path| names.iter().any(|n| same_path(p, n));
    if is(&process.exe) {
        args.is_empty() || process.args == args
    } else if is_too_broad(&process.exe) && process.args.first().is_some_and(|a| is(Path::new(a))) {
        args.is_empty() || process.args.get(1..) == Some(args)
    } else {
        false
    }
}

/// `path` as written and with symlinks resolved, without Windows' `\\?\` prefix, which the OS
/// does not use when it reports a process's executable.
fn both_forms(path: &Path) -> Vec<PathBuf> {
    let mut out = vec![path.to_owned()];
    if let Ok(resolved) = std::fs::canonicalize(path) {
        let resolved = without_verbatim_prefix(resolved);
        if !out.contains(&resolved) {
            out.push(resolved);
        }
    }
    out
}

/// `\\?\C:\x` → `C:\x` and `\\?\UNC\host\share\x` → `\\host\share\x`; anything else unchanged.
fn without_verbatim_prefix(path: PathBuf) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path;
    };
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

/// Paths compare by component, so `/` and `\` agree on Windows, and there without regard to
/// case, as the file system does.
fn same_path(a: &Path, b: &Path) -> bool {
    let key = |c: Component<'_>| {
        let part = c.as_os_str();
        if cfg!(windows) {
            part.to_ascii_lowercase()
        } else {
            part.to_owned()
        }
    };
    a.components().map(key).eq(b.components().map(key))
}

/// Shells, script interpreters and launchers that run other programs: running one with no
/// arguments says nothing about which program it is.
const TOO_BROAD: &[&str] = &[
    // Shells
    "sh",
    "ash",
    "bash",
    "rbash",
    "dash",
    "zsh",
    "fish",
    "ksh",
    "mksh",
    "tcsh",
    "csh",
    "nu",
    "xonsh",
    "elvish",
    "busybox",
    "cmd",
    "pwsh",
    "powershell",
    "wscript",
    "cscript",
    "conhost",
    // Interpreters and runtimes
    "python",
    "pythonw",
    "py",
    "pypy",
    "node",
    "nodejs",
    "bun",
    "deno",
    "gjs",
    "ruby",
    "perl",
    "php",
    "lua",
    "luajit",
    "tclsh",
    "wish",
    "rscript",
    "java",
    "javaw",
    "dotnet",
    "mono",
    "osascript",
    "electron",
    // Launchers and the desktop itself
    "env",
    "snap",
    "flatpak",
    "wine",
    "wsl",
    "explorer",
    "rundll32",
    "mshta",
    "open",
    "xdg-open",
    "systemd",
    "dbus",
    "gnome-shell",
    "plasmashell",
    "kwin_wayland",
    "kwin_x11",
    "finder",
    "dock",
    "loginwindow",
];

fn is_too_broad(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    let stem = stem.to_ascii_lowercase();
    if TOO_BROAD.contains(&stem.as_str()) {
        return true;
    }
    // `python3.12`, `pwsh-preview` and the like.
    let base = stem
        .split(|c: char| c.is_ascii_digit() || c == '.' || c == '-')
        .next()
        .unwrap_or_default();
    TOO_BROAD.contains(&base)
}

fn is_app_image(names: &[PathBuf]) -> bool {
    names.iter().any(|n| {
        n.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("appimage"))
    })
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
        // Every target approved; approval itself has its own test.
        let approved: Vec<LaunchTarget> = group
            .entries
            .iter()
            .filter_map(|e| {
                let app = apps.iter().find(|a| a.id == e.app)?;
                Some(e.target_for(&app.target))
            })
            .collect();
        plan_close(group, apps, &approved, processes, 1)
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
    fn an_unapproved_manual_entry_is_not_closed() {
        let apps = [app("m:shell", program("/usr/bin/gnome-shell", &[]))];
        let processes = [process(10, "/usr/bin/gnome-shell", &[])];
        let closing = plan_close(&group(&[("m:shell", None)]), &apps, &[], &processes, 1);
        assert_eq!(closing.unwrap()[0].matched, CloseMatch::NeedsApproval);
    }

    #[test]
    fn a_detected_app_closes_without_approval() {
        let mut editor = app("d:editor", program("/opt/editor/editor", &[]));
        editor.source = Source::Detected;
        let processes = [process(10, "/opt/editor/editor", &[])];
        let closing = plan_close(&group(&[("d:editor", None)]), &[editor], &[], &processes, 1);
        assert_eq!(closing.unwrap()[0].matched, CloseMatch::Running(vec![10]));
    }

    #[test]
    fn an_app_image_is_not_closed() {
        let apps = [app(
            "m:notes",
            program("/home/someone/Apps/Notes.AppImage", &[]),
        )];
        let processes = [process(10, "/home/someone/Apps/Notes.AppImage", &[])];
        assert_eq!(
            close(&apps, &group(&[("m:notes", None)]), &processes),
            [("m:notes".into(), CloseMatch::Unsupported)]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_program_matches_through_a_symlink_either_way() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("editor-1.2");
        std::fs::write(&real, b"").unwrap();
        let link = dir.path().join("editor");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let real = std::fs::canonicalize(&real).unwrap();
        let apps = [app("m:editor", program(link.to_str().unwrap(), &[]))];
        let processes = [
            process(10, real.to_str().unwrap(), &[]),
            process(11, link.to_str().unwrap(), &[]),
        ];
        assert_eq!(
            close(&apps, &group(&[("m:editor", None)]), &processes),
            [("m:editor".into(), CloseMatch::Running(vec![10, 11]))]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_an_interpreter_is_too_broad() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("python3.12");
        std::fs::write(&real, b"").unwrap();
        // A name that is not on the list itself.
        let link = dir.path().join("snakes");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let apps = [app("m:py", program(link.to_str().unwrap(), &[]))];
        assert_eq!(
            close(&apps, &group(&[("m:py", None)]), &[]),
            [("m:py".into(), CloseMatch::TooBroad)]
        );
    }

    #[test]
    fn a_program_matches_the_path_the_os_reports() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("editor.exe");
        std::fs::write(&file, b"").unwrap();
        // What Windows reports for a running process: resolved, but without `\\?\`.
        let reported = without_verbatim_prefix(std::fs::canonicalize(&file).unwrap());
        let apps = [app("m:editor", program(file.to_str().unwrap(), &[]))];
        let processes = [process(10, reported.to_str().unwrap(), &[])];
        assert_eq!(
            close(&apps, &group(&[("m:editor", None)]), &processes),
            [("m:editor".into(), CloseMatch::Running(vec![10]))]
        );
    }

    #[test]
    fn drops_the_verbatim_prefix_windows_adds() {
        assert_eq!(
            without_verbatim_prefix(r"\\?\C:\Program Files\App\app.exe".into()),
            PathBuf::from(r"C:\Program Files\App\app.exe")
        );
        assert_eq!(
            without_verbatim_prefix(r"\\?\UNC\host\share\app.exe".into()),
            PathBuf::from(r"\\host\share\app.exe")
        );
        assert_eq!(
            without_verbatim_prefix("/opt/app".into()),
            PathBuf::from("/opt/app")
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
            "/usr/bin/nodejs",
            "/snap/bin/snap",
            "explorer.exe",
        ] {
            assert!(is_too_broad(Path::new(shell)), "{shell}");
        }
        for program in ["/opt/editor/editor", "/usr/bin/bashtop", "/usr/bin/nodepad"] {
            assert!(!is_too_broad(Path::new(program)), "{program}");
        }
    }
}
