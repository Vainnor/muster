//! The native dialog that approves what a manual app or an overridden group entry runs.
//!
//! The webview can ask to launch something but cannot approve it: the dialog is shown from
//! Rust through the dialog plugin, which has no JS permission, and only an OK there records
//! the approval (`Library::approve`).

use std::sync::atomic::{AtomicBool, Ordering};

use muster_core::launch::{LaunchTarget, describe};
use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::commands::CommandError;
use crate::state::AppState;

/// Longer than this and the dialog would scroll or clip, hiding part of what runs.
const MAX_TEXT: usize = 4 * 1024;

/// One dialog at a time, so a flood of requests cannot stack them up.
static ASKING: AtomicBool = AtomicBool::new(false);

/// Clears [`ASKING`] however `ensure` ends.
struct Asking;

impl Drop for Asking {
    fn drop(&mut self) {
        ASKING.store(false, Ordering::Release);
    }
}

/// The dialog text, or `None` if it would be too long to read in full.
pub(crate) fn text(targets: &[LaunchTarget]) -> Option<String> {
    let mut text = String::from("Muster will run:\n");
    for target in targets {
        text.push_str("\n• ");
        text.push_str(&describe(target));
    }
    text.push_str("\n\nOnly continue if you added these yourself.");
    (text.len() <= MAX_TEXT).then_some(text)
}

/// Returns once every target in `pending` is approved, asking the user if any are.
pub(crate) async fn ensure<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    pending: Vec<LaunchTarget>,
) -> Result<(), CommandError> {
    if pending.is_empty() {
        return Ok(());
    }
    let text = text(&pending).ok_or(CommandError::TOO_LONG_TO_CONFIRM)?;
    if ASKING.swap(true, Ordering::AcqRel) {
        return Err(CommandError::BUSY);
    }
    let _asking = Asking;
    let handle = app.clone();
    // `blocking_show` must not run on the main thread.
    let answer = tauri::async_runtime::spawn_blocking(move || {
        handle
            .dialog()
            .message(text)
            .title("Run this for the first time?")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Run".into(),
                "Cancel".into(),
            ))
            .blocking_show()
    })
    .await;
    if !answer.map_err(|_| CommandError::INTERNAL)? {
        return Err(CommandError::CANCELLED);
    }
    state.update(|lib| lib.approve(&pending))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_lists_each_target_quoted() {
        let t = text(&[LaunchTarget::Url {
            url: "https://a.example".into(),
        }])
        .unwrap();
        assert!(t.contains("• open \"https://a.example\""), "{t}");
    }

    #[test]
    fn refuses_text_too_long_to_read() {
        let long = LaunchTarget::Program {
            path: "/bin/sh".into(),
            args: vec!["x".repeat(MAX_TEXT)],
            cwd: None,
        };
        assert_eq!(text(&[long]), None);
    }
}
