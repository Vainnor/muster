//! The native dialog that approves what a manual app or an overridden group entry runs.
//!
//! The webview can ask to launch something but cannot approve it: the dialog is shown from
//! Rust through the dialog plugin, which has no JS permission, and only the "Run" button
//! records the approval (`Library::approve`). "Don't run" is the first button, which is the
//! default (Return) on macOS (rfd shows a `CFUserNotification` alert, with the first label as
//! its default button) and Windows (task dialog, first custom button), so a Return key press
//! the user meant for Muster's window cannot approve anything.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use muster_core::launch::{LaunchTarget, describe};
use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::{
    DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};

use crate::commands::CommandError;
use crate::state::AppState;

/// One dialog's text stays this short so it fits on screen and is read in full; longer
/// lists are split across several dialogs, and a single longer target is refused.
const MAX_TEXT: usize = 1024;

/// After the user declines, new requests are refused for this long, so script cannot put the
/// dialog straight back up until the user gives in.
const COOL_DOWN: Duration = Duration::from_secs(5);

const DONT_RUN: &str = "Don't run";
const RUN: &str = "Run";
const CANCEL: &str = "Cancel";

/// One dialog at a time, so a flood of requests cannot stack them up.
static ASKING: AtomicBool = AtomicBool::new(false);
static DECLINED_AT: Mutex<Option<Instant>> = Mutex::new(None);

/// True while an approval dialog is open, so the popup does not close when it loses focus.
pub(crate) fn is_asking() -> bool {
    ASKING.load(Ordering::Acquire)
}

/// Clears [`ASKING`] however `ensure` ends.
struct Asking;

impl Drop for Asking {
    fn drop(&mut self) {
        ASKING.store(false, Ordering::Release);
    }
}

const HEADER: &str = "Muster has not run this before:\n";
const FOOTER: &str = "\n\nRun it only if you added it yourself.";

/// Splits `targets` into dialog texts of at most [`MAX_TEXT`] bytes, or `None` if one target
/// alone is too long to show in full.
pub(crate) fn texts(targets: &[LaunchTarget]) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut text = String::from(HEADER);
    for target in targets {
        let line = format!("\n• {}", describe(target));
        if HEADER.len() + line.len() + FOOTER.len() > MAX_TEXT {
            return None;
        }
        if text.len() + line.len() + FOOTER.len() > MAX_TEXT {
            text.push_str(FOOTER);
            out.push(std::mem::replace(&mut text, String::from(HEADER)));
        }
        text.push_str(&line);
    }
    text.push_str(FOOTER);
    out.push(text);
    Some(out)
}

/// Whether a [`tell`] dialog is open; only one is shown at a time.
static TELLING: AtomicBool = AtomicBool::new(false);

/// Shows a result the window cannot, such as a group started by a hotkey that failed. Does
/// not wait for the user, and drops the message if one is already showing, so repeated
/// presses cannot stack dialogs.
pub(crate) fn tell<R: Runtime>(app: &AppHandle<R>, title: &str, text: String) {
    if TELLING.swap(true, Ordering::AcqRel) {
        return;
    }
    app.dialog()
        .message(text)
        .title(title)
        .kind(MessageDialogKind::Error)
        .show(|_| TELLING.store(false, Ordering::Release));
}

/// Returns once every target in `pending` is approved, asking the user if any are not.
pub(crate) async fn ensure<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    pending: Vec<LaunchTarget>,
) -> Result<(), CommandError> {
    if pending.is_empty() {
        return Ok(());
    }
    let texts = texts(&pending).ok_or(CommandError::TOO_LONG_TO_CONFIRM)?;
    let cooling = DECLINED_AT
        .lock()
        .map_err(|_| CommandError::INTERNAL)?
        .is_some_and(|at| at.elapsed() < COOL_DOWN);
    if cooling {
        return Err(CommandError::COOLING_DOWN);
    }
    if ASKING.swap(true, Ordering::AcqRel) {
        return Err(CommandError::BUSY);
    }
    let _asking = Asking;
    for text in texts {
        let handle = app.clone();
        // `blocking_show_with_result` must not run on the main thread.
        let answer = tauri::async_runtime::spawn_blocking(move || {
            handle
                .dialog()
                .message(text)
                .title("Run this?")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::YesNoCancelCustom(
                    DONT_RUN.into(),
                    RUN.into(),
                    CANCEL.into(),
                ))
                .blocking_show_with_result()
        })
        .await
        .map_err(|_| CommandError::INTERNAL)?;
        if answer != MessageDialogResult::Custom(RUN.into()) {
            *DECLINED_AT.lock().map_err(|_| CommandError::INTERNAL)? = Some(Instant::now());
            return Err(CommandError::CANCELLED);
        }
    }
    state.update(|lib| lib.approve(&pending))
}

const LEAVE_RUNNING: &str = "Leave running";
const FORCE_QUIT: &str = "Force quit";
const FORCE_HEADER: &str = "These apps did not quit when asked:\n";
const FORCE_FOOTER: &str = "\n\nForce quitting them loses anything they have not saved.";
/// Longer app names are cut, so a few names cannot fill the dialog.
const MAX_NAME: usize = 80;

/// The force-quit question for `names`, at most [`MAX_TEXT`] bytes.
pub(crate) fn force_quit_text(names: &[String]) -> String {
    let mut text = String::from(FORCE_HEADER);
    for (shown, name) in names.iter().enumerate() {
        let name: String = name
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_NAME)
            .collect();
        let line = format!("\n• {name}");
        let rest = format!("\n• and {} more", names.len() - shown);
        if text.len() + line.len() + rest.len() + FORCE_FOOTER.len() > MAX_TEXT {
            text.push_str(&rest);
            break;
        }
        text.push_str(&line);
    }
    text.push_str(FORCE_FOOTER);
    text
}

/// Asks whether to force quit the apps in `names`. "Leave running" is the first button, so it
/// is the default, and only "Force quit" answers yes. With another Muster dialog open, or
/// within the cooldown after the user said no to any of them, the answer is no without asking,
/// so script cannot keep the question up until a stray key press answers it.
pub(crate) async fn confirm_force_quit<R: Runtime>(
    app: &AppHandle<R>,
    names: Vec<String>,
) -> Result<bool, CommandError> {
    let cooling = DECLINED_AT
        .lock()
        .map_err(|_| CommandError::INTERNAL)?
        .is_some_and(|at| at.elapsed() < COOL_DOWN);
    if cooling || ASKING.swap(true, Ordering::AcqRel) {
        return Ok(false);
    }
    let _asking = Asking;
    let force = ask(
        app,
        "Force quit?",
        force_quit_text(&names),
        LEAVE_RUNNING,
        FORCE_QUIT,
    )
    .await?;
    if !force {
        *DECLINED_AT.lock().map_err(|_| CommandError::INTERNAL)? = Some(Instant::now());
    }
    Ok(force)
}

#[cfg(feature = "updater")]
/// Asks to install `version`, which restarts Muster. "Not now" is the first button and the
/// default; only "Install" answers yes. With another Muster dialog open, the answer is no.
pub(crate) async fn confirm_update<R: Runtime>(
    app: &AppHandle<R>,
    version: &str,
) -> Result<bool, CommandError> {
    let cooling = DECLINED_AT
        .lock()
        .map_err(|_| CommandError::INTERNAL)?
        .is_some_and(|at| at.elapsed() < COOL_DOWN);
    if cooling || ASKING.swap(true, Ordering::AcqRel) {
        return Ok(false);
    }
    let _asking = Asking;
    let version: String = version
        .chars()
        .filter(|c| !c.is_control())
        .take(32)
        .collect();
    let text = format!("Install Muster {version}? Muster restarts when it is done.");
    let install = ask(app, "Install the update?", text, "Not now", "Install").await?;
    if !install {
        *DECLINED_AT.lock().map_err(|_| CommandError::INTERNAL)? = Some(Instant::now());
    }
    Ok(install)
}

/// A warning dialog whose first button, `no`, is the default; true only for `yes`. The caller
/// holds [`ASKING`].
async fn ask<R: Runtime>(
    app: &AppHandle<R>,
    title: &'static str,
    text: String,
    no: &'static str,
    yes: &'static str,
) -> Result<bool, CommandError> {
    let handle = app.clone();
    // `blocking_show_with_result` must not run on the main thread.
    let answer = tauri::async_runtime::spawn_blocking(move || {
        handle
            .dialog()
            .message(text)
            .title(title)
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::YesNoCancelCustom(
                no.into(),
                yes.into(),
                CANCEL.into(),
            ))
            .blocking_show_with_result()
    })
    .await
    .map_err(|_| CommandError::INTERNAL)?;
    Ok(answer == MessageDialogResult::Custom(yes.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(n: usize) -> LaunchTarget {
        LaunchTarget::Url {
            url: format!("https://site-{n}.example"),
        }
    }

    #[test]
    fn text_lists_each_target_quoted() {
        let t = texts(&[url(1)]).unwrap();
        assert_eq!(t.len(), 1);
        assert!(
            t[0].contains("• open \"https://site-1.example/\""),
            "{}",
            t[0]
        );
    }

    #[test]
    fn long_lists_split_into_dialogs_that_each_fit() {
        let targets: Vec<_> = (0..60).map(url).collect();
        let t = texts(&targets).unwrap();
        assert!(t.len() > 1);
        assert!(t.iter().all(|t| t.len() <= MAX_TEXT && t.ends_with(FOOTER)));
        let shown: usize = t.iter().map(|t| t.matches('•').count()).sum();
        assert_eq!(shown, 60, "every target appears exactly once");
    }

    #[test]
    fn force_quit_text_names_each_app() {
        let text = force_quit_text(&["Editor".into(), "Sync".into()]);
        assert_eq!(
            text,
            format!("{FORCE_HEADER}\n• Editor\n• Sync{FORCE_FOOTER}")
        );
    }

    #[test]
    fn force_quit_text_fits_however_many_apps() {
        let names: Vec<String> = (0..200).map(|n| format!("{n:>200}")).collect();
        let text = force_quit_text(&names);
        assert!(text.len() <= MAX_TEXT, "{}", text.len());
        assert!(text.contains(" more"));
        assert!(text.ends_with(FORCE_FOOTER));
        assert!(!force_quit_text(&["a\nb".into()]).contains("a\nb"));
    }

    #[test]
    fn refuses_a_target_too_long_to_read() {
        let long = LaunchTarget::Program {
            path: "/bin/sh".into(),
            args: vec!["x".repeat(MAX_TEXT)],
            cwd: None,
        };
        assert_eq!(texts(&[long]), None);
    }
}
