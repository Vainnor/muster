//! The popup launcher: a small window with only the search list, opened by the global hotkey
//! or `muster --popup`. Like the main window it is created on demand and destroyed when it
//! closes, so it holds no webview while hidden (`.claude/rules/performance.md`).

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub(crate) const POPUP: &str = "popup";

/// The flag a second launch passes to open the popup, for compositors where Muster cannot
/// register a global hotkey itself.
pub(crate) const FLAG: &str = "--popup";

/// Opens the popup, or closes it if it is already open.
pub(crate) fn toggle<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(POPUP) {
        return window.destroy();
    }
    let window = WebviewWindowBuilder::new(app, POPUP, WebviewUrl::App("popup.html".into()))
        .title("Muster")
        .inner_size(560.0, 360.0)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        // Hidden until the page calls `popup_ready`, so it never shows a half-drawn frame.
        .visible(false)
        .focused(false)
        .build()?;
    let handle = app.clone();
    window.on_window_event(move |event| {
        // Clicking elsewhere closes it, as launchers do. The approval dialog takes focus
        // too, and the popup must outlive it to finish the launch.
        if matches!(event, WindowEvent::Focused(false))
            && !crate::approval::is_asking()
            && let Some(window) = handle.get_webview_window(POPUP)
            && let Err(e) = window.destroy()
        {
            crate::diag::report("closing the popup", &e);
        }
    });
    Ok(())
}

/// For callbacks that cannot return an error (hotkey, single instance).
pub(crate) fn toggle_or_log<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = toggle(app) {
        crate::diag::report("opening the popup", &e);
    }
}
