//! The native file picker behind the Browse buttons in "Add an app".
//!
//! Rust shows it through the dialog plugin, which has no JS permission, and hands the webview
//! only the path the user chose. Muster reads nothing at that path here; `add_app` validates
//! it in core like a typed path, and launching validates it again.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{Runtime, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::approval;
use crate::commands::CommandError;

/// What the Browse button is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PickKind {
    Program,
    AppBundle,
    Folder,
}

/// The chosen path, and what it turned out to be: on macOS a program picker also lets the
/// user choose an `.app`, which is added as a bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Picked {
    pub(crate) kind: PickKind,
    pub(crate) path: String,
}

/// After the user cancels, the picker stays closed this long, so script cannot put it straight
/// back up until a stray Return picks something.
const COOL_DOWN: Duration = Duration::from_secs(2);

static CANCELLED_AT: Mutex<Option<Instant>> = Mutex::new(None);

/// What a path chosen for `asked` should be added as.
pub(crate) fn kind_of(asked: PickKind, path: &Path) -> PickKind {
    let is_app = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("app"));
    if asked == PickKind::Program && is_app && path.is_dir() {
        PickKind::AppBundle
    } else {
        asked
    }
}

/// Shows the picker over `window` and waits for it. `None` when the user cancels.
pub(crate) async fn pick<R: Runtime>(
    window: WebviewWindow<R>,
    kind: PickKind,
) -> Result<Option<Picked>, CommandError> {
    let cooling = CANCELLED_AT
        .lock()
        .map_err(|_| CommandError::INTERNAL)?
        .is_some_and(|at| at.elapsed() < COOL_DOWN);
    if cooling {
        return Err(CommandError::PICKER_COOLING_DOWN);
    }
    // One Muster dialog at a time: never over an approval dialog, never two pickers.
    let _asking = approval::claim()?;
    // The blocking pickers must not run on the main thread.
    let chosen = tauri::async_runtime::spawn_blocking(move || {
        let dialog = window.dialog().file().set_parent(&window);
        match kind {
            PickKind::Program => programs(dialog)
                .set_title("Choose a program")
                .blocking_pick_file(),
            PickKind::AppBundle => dialog
                .set_title("Choose an app")
                .set_directory("/Applications")
                .add_filter("Apps", &["app"])
                .blocking_pick_file(),
            PickKind::Folder => dialog
                .set_title("Choose a working folder")
                .blocking_pick_folder(),
        }
    })
    .await
    .map_err(|_| CommandError::INTERNAL)?;
    let Some(chosen) = chosen else {
        *CANCELLED_AT.lock().map_err(|_| CommandError::INTERNAL)? = Some(Instant::now());
        return Ok(None);
    };
    let path: PathBuf = chosen.into_path().map_err(|_| CommandError::INTERNAL)?;
    let kind = kind_of(kind, &path);
    let path = path
        .into_os_string()
        .into_string()
        .map_err(|_| CommandError::PATH_NOT_UNICODE)?;
    Ok(Some(Picked { kind, path }))
}

/// Windows lists programs and scripts first; elsewhere executables have no extension to
/// filter on.
#[cfg(windows)]
fn programs<R: Runtime>(
    dialog: tauri_plugin_dialog::FileDialogBuilder<R>,
) -> tauri_plugin_dialog::FileDialogBuilder<R> {
    dialog
        .add_filter("Programs", &["exe", "bat", "cmd", "com"])
        .add_filter("All files", &["*"])
}

#[cfg(not(windows))]
fn programs<R: Runtime>(
    dialog: tauri_plugin_dialog::FileDialogBuilder<R>,
) -> tauri_plugin_dialog::FileDialogBuilder<R> {
    dialog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_app_chosen_as_a_program_is_added_as_a_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("Editor.APP");
        std::fs::create_dir(&bundle).unwrap();
        assert_eq!(kind_of(PickKind::Program, &bundle), PickKind::AppBundle);
    }

    #[test]
    fn a_file_named_like_an_app_stays_a_program() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("odd.app");
        std::fs::write(&file, b"").unwrap();
        assert_eq!(kind_of(PickKind::Program, &file), PickKind::Program);
    }

    #[test]
    fn folders_and_bundles_keep_the_kind_asked_for() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("Tools.app");
        std::fs::create_dir(&bundle).unwrap();
        assert_eq!(kind_of(PickKind::Folder, &bundle), PickKind::Folder);
        assert_eq!(kind_of(PickKind::AppBundle, &bundle), PickKind::AppBundle);
    }

    #[test]
    fn kinds_use_the_names_the_ui_sends() {
        assert_eq!(
            serde_json::to_string(&PickKind::AppBundle).unwrap(),
            r#""app_bundle""#
        );
    }
}
