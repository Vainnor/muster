//! Start on login. The plugin is registered but autostart stays off until the user enables it
//! in settings (not built yet); enabling goes through `ManagerExt::autolaunch()` from Rust.
//! The webview has no autostart permission.

use tauri::Runtime;
use tauri::plugin::TauriPlugin;
use tauri_plugin_autostart::MacosLauncher;

pub(crate) fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None)
}
