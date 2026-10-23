//! Closing a group: listing running processes, asking the ones that match to quit, and force
//! quitting what is left once the user agrees in a native dialog.
//!
//! Nothing here runs until the user presses Close. Which processes match is
//! `muster_core::close`; the arguments read here are used for that match only and are never
//! logged or sent to the webview.
//!
//! Asking to quit is the app's own Quit on macOS (`NSRunningApplication::terminate`, which lets
//! it ask about unsaved work), `taskkill` without `/F` on Windows (closes its windows), and
//! `SIGTERM` on Linux. A process that is still there afterwards is force quit only from the
//! dialog, and only if a fresh listing still matches it.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use muster_core::close::{CloseMatch, EntryClose, RunningProcess};
use muster_core::groups::GroupId;
use muster_core::model::AppId;
use tauri::{AppHandle, Runtime};

use crate::commands::CommandError;
use crate::state::AppState;

/// How long apps get to quit before Muster offers to force them.
const GRACE: Duration = Duration::from_secs(5);
/// How often Muster looks again while it waits.
const LOOK_AGAIN: Duration = Duration::from_millis(500);

/// One close at a time: a second press while the first waits would ask the same apps again.
static CLOSING: AtomicBool = AtomicBool::new(false);

struct Closing;

impl Drop for Closing {
    fn drop(&mut self) {
        CLOSING.store(false, Ordering::Release);
    }
}

/// What closing did for one entry of the group.
#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Outcome {
    Closed,
    StillRunning,
    NotRunning,
    Link,
    TooBroad,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub(crate) struct Closed {
    pub(crate) app: String,
    pub(crate) outcome: Outcome,
}

pub(crate) async fn close<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    id: &GroupId,
) -> Result<Vec<Closed>, CommandError> {
    if CLOSING.swap(true, Ordering::AcqRel) {
        return Err(CommandError::STILL_CLOSING);
    }
    let _closing = Closing;

    let first = look(state, id).await?;
    let asked: HashSet<AppId> = running(&first).collect();
    let pids: Vec<u32> = first.iter().flat_map(pids_of).collect();
    blocking(move || {
        for pid in pids {
            if let Err(e) = os::ask_to_quit(pid) {
                crate::diag::report("asking an app to quit", &e);
            }
        }
    })
    .await?;

    let mut now = first.clone();
    let mut waited = Duration::ZERO;
    while waited < GRACE && running(&now).any(|a| asked.contains(&a)) {
        blocking(|| std::thread::sleep(LOOK_AGAIN)).await?;
        waited += LOOK_AGAIN;
        now = look(state, id).await?;
    }

    let stubborn: Vec<AppId> = running(&now).filter(|a| asked.contains(a)).collect();
    if !stubborn.is_empty() {
        let names = {
            let library = state.library()?;
            stubborn
                .iter()
                .map(|a| library.find(a).map_or_else(|| a.0.clone(), |e| e.name))
                .collect::<Vec<_>>()
        };
        if crate::approval::confirm_force_quit(app, names).await? {
            // Listed again so a process that exited, and a pid the OS gave to something
            // else since, is not killed.
            let fresh = look(state, id).await?;
            let pids: Vec<u32> = fresh
                .iter()
                .filter(|e| stubborn.contains(&e.app))
                .flat_map(pids_of)
                .collect();
            blocking(move || {
                for pid in pids {
                    if let Err(e) = os::force_quit(pid) {
                        crate::diag::report("force quitting an app", &e);
                    }
                }
            })
            .await?;
            blocking(|| std::thread::sleep(LOOK_AGAIN)).await?;
            now = look(state, id).await?;
        }
    }

    let left: HashSet<AppId> = running(&now).collect();
    Ok(first
        .into_iter()
        .map(|e| Closed {
            outcome: match e.matched {
                CloseMatch::Running(_) if left.contains(&e.app) => Outcome::StillRunning,
                CloseMatch::Running(_) => Outcome::Closed,
                CloseMatch::NotRunning => Outcome::NotRunning,
                CloseMatch::Link => Outcome::Link,
                CloseMatch::TooBroad => Outcome::TooBroad,
            },
            app: e.app.0,
        })
        .collect())
}

/// Lists processes and matches them to the group as it is now.
async fn look(state: &AppState, id: &GroupId) -> Result<Vec<EntryClose>, CommandError> {
    let processes = blocking(os::list).await?.map_err(|e| {
        crate::diag::report("listing running apps", e.as_ref());
        CommandError::INTERNAL
    })?;
    Ok(state
        .library()?
        .plan_close(id, &processes, std::process::id())?)
}

fn running(entries: &[EntryClose]) -> impl Iterator<Item = AppId> + '_ {
    entries
        .iter()
        .filter(|e| matches!(e.matched, CloseMatch::Running(_)))
        .map(|e| e.app.clone())
}

fn pids_of(entry: &EntryClose) -> Vec<u32> {
    match &entry.matched {
        CloseMatch::Running(pids) => pids.clone(),
        _ => Vec::new(),
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| CommandError::INTERNAL)
}

/// `/proc/<pid>/cmdline`: arguments separated and ended by NUL, the program first.
#[cfg_attr(
    not(target_os = "linux"),
    allow(
        dead_code,
        reason = "Linux only; built everywhere so every CI runner tests it"
    )
)]
fn parse_cmdline(raw: &[u8]) -> Vec<String> {
    let raw = raw.strip_suffix(&[0]).unwrap_or(raw);
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split(|b| *b == 0)
        .skip(1)
        .map(|a| String::from_utf8_lossy(a).into_owned())
        .collect()
}

/// The parent pid from `/proc/<pid>/stat`. The command name in parentheses may itself hold
/// spaces and `)`, so the fields are read after the last `)`.
#[cfg_attr(
    not(target_os = "linux"),
    allow(
        dead_code,
        reason = "Linux only; built everywhere so every CI runner tests it"
    )
)]
fn parse_parent(stat: &str) -> Option<u32> {
    let (_, rest) = stat.rsplit_once(')')?;
    rest.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(unix)]
fn signal(pid: u32, signal: rustix::process::Signal) -> std::io::Result<()> {
    let pid = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .ok_or(std::io::ErrorKind::InvalidInput)?;
    rustix::process::kill_process(pid, signal).map_err(std::io::Error::from)
}

#[cfg(target_os = "linux")]
mod os {
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    use anyhow::Context as _;
    use muster_core::close::RunningProcess;
    use rustix::process::Signal;

    pub(super) fn list() -> anyhow::Result<Vec<RunningProcess>> {
        let me = std::fs::metadata("/proc/self")
            .context("reading Muster's own process")?
            .uid();
        let mut out = Vec::new();
        for entry in std::fs::read_dir("/proc").context("listing /proc")? {
            let entry = entry.context("listing /proc")?;
            let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse().ok()) else {
                continue;
            };
            out.extend(read(&entry.path(), pid, me));
        }
        Ok(out)
    }

    /// `None` for a process that exited meanwhile or whose files are not readable (another
    /// user's): neither is one Muster may close.
    fn read(dir: &Path, pid: u32, me: u32) -> Option<RunningProcess> {
        let mine = std::fs::metadata(dir).ok()?.uid() == me;
        let exe = std::fs::read_link(dir.join("exe")).ok()?;
        let args = super::parse_cmdline(&std::fs::read(dir.join("cmdline")).ok()?);
        let parent = std::fs::read_to_string(dir.join("stat"))
            .ok()
            .as_deref()
            .and_then(super::parse_parent);
        Some(RunningProcess {
            pid,
            exe,
            args,
            mine,
            parent,
        })
    }

    pub(super) fn ask_to_quit(pid: u32) -> std::io::Result<()> {
        super::signal(pid, Signal::TERM)
    }

    pub(super) fn force_quit(pid: u32) -> std::io::Result<()> {
        super::signal(pid, Signal::KILL)
    }
}

/// sysinfo on Windows and macOS: the Windows command line needs reads of another process's
/// memory, and macOS needs `sysctl` calls, neither of which Muster writes as `unsafe` itself.
#[cfg(any(windows, target_os = "macos"))]
fn list_with_sysinfo() -> anyhow::Result<Vec<RunningProcess>> {
    use anyhow::Context as _;
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_cmd(UpdateKind::Always)
            .with_user(UpdateKind::Always),
    );
    let own = sysinfo::get_current_pid().map_err(anyhow::Error::msg)?;
    let me = system
        .process(own)
        .and_then(sysinfo::Process::user_id)
        .cloned()
        .context("finding the user Muster runs as")?;
    Ok(system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            Some(RunningProcess {
                pid: pid.as_u32(),
                exe: process.exe()?.to_owned(),
                args: process
                    .cmd()
                    .iter()
                    .skip(1)
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect(),
                mine: process.user_id() == Some(&me),
                parent: process.parent().map(sysinfo::Pid::as_u32),
            })
        })
        .collect())
}

#[cfg(target_os = "macos")]
mod os {
    use muster_core::close::RunningProcess;
    use objc2_app_kit::NSRunningApplication;
    use rustix::process::Signal;

    pub(super) fn list() -> anyhow::Result<Vec<RunningProcess>> {
        super::list_with_sysinfo()
    }

    /// An app gets a Quit, as if chosen from its menu, so it can ask about unsaved work; a
    /// process with no app (a command-line program) gets `SIGTERM`.
    pub(super) fn ask_to_quit(pid: u32) -> std::io::Result<()> {
        let raw = i32::try_from(pid).map_err(|_| std::io::ErrorKind::InvalidInput)?;
        let is_app = objc2::rc::autoreleasepool(|_| {
            NSRunningApplication::runningApplicationWithProcessIdentifier(raw).map(|app| {
                // `false` means it is quitting already or will not take the request; it is
                // never signalled instead, which would skip its save prompt.
                app.terminate();
            })
        });
        match is_app {
            Some(()) => Ok(()),
            None => super::signal(pid, Signal::TERM),
        }
    }

    pub(super) fn force_quit(pid: u32) -> std::io::Result<()> {
        super::signal(pid, Signal::KILL)
    }
}

#[cfg(windows)]
mod os {
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    use muster_core::close::RunningProcess;

    pub(super) fn list() -> anyhow::Result<Vec<RunningProcess>> {
        super::list_with_sysinfo()
    }

    /// `taskkill` without `/F` asks each of the process's windows to close.
    pub(super) fn ask_to_quit(pid: u32) -> std::io::Result<()> {
        taskkill(&["/PID", &pid.to_string()])
    }

    pub(super) fn force_quit(pid: u32) -> std::io::Result<()> {
        taskkill(&["/F", "/PID", &pid.to_string()])
    }

    /// By absolute path, so a `taskkill.exe` next to Muster or in the working dir is not run.
    fn taskkill(args: &[&str]) -> std::io::Result<()> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or(std::io::ErrorKind::NotFound)?;
        let status = Command::new(root.join(r"System32\taskkill.exe"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("taskkill did not succeed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_arguments_after_the_program() {
        assert_eq!(
            parse_cmdline(b"/opt/sync/sync\0--watch\0my work\0"),
            ["--watch", "my work"]
        );
        assert_eq!(parse_cmdline(b"/opt/editor/editor\0"), Vec::<String>::new());
        // A kernel thread has an empty command line.
        assert_eq!(parse_cmdline(b""), Vec::<String>::new());
    }

    #[test]
    fn reads_the_parent_past_a_name_with_parentheses() {
        assert_eq!(
            parse_parent("4242 (Web Content (x)) S 4100 4242 4100 0 -1"),
            Some(4100)
        );
        assert_eq!(parse_parent("4242 (editor"), None);
    }

    #[test]
    fn a_closed_entry_is_reported_by_app() {
        let closed = Closed {
            app: "m:editor".into(),
            outcome: Outcome::StillRunning,
        };
        assert_eq!(
            serde_json::to_value(closed).unwrap(),
            serde_json::json!({ "app": "m:editor", "outcome": "still_running" })
        );
    }
}
