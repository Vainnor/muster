//! The main window. It is created on demand and destroyed when closed, so an idle Muster in
//! the tray holds no webview at all (`.claude/rules/performance.md`).

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

pub(crate) const MAIN: &str = "main";

/// Focuses the main window, creating it if it was closed.
pub(crate) fn show<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN) {
        window.unminimize()?;
        window.show()?;
        return window.set_focus();
    }
    WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("Muster")
        .inner_size(720.0, 480.0)
        .min_inner_size(480.0, 320.0)
        .center()
        .focused(true)
        .build()?;
    Ok(())
}

/// Destroys the main window and its webview. Muster keeps running in the tray.
pub(crate) fn close<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    match app.get_webview_window(MAIN) {
        Some(window) => window.destroy(),
        None => Ok(()),
    }
}

/// For callbacks that cannot return an error (tray, single instance).
pub(crate) fn show_or_log<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = show(app) {
        crate::diag::report("opening the main window", &e);
    }
}
