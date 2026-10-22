//! Global hotkeys, one per group. The plugin is registered so the wiring exists, but no
//! shortcut is registered until groups are implemented. Shortcuts are registered from Rust
//! only; the webview has no global-shortcut permission.
//!
//! Wayland: the plugin cannot grab keys there. Detect it and tell the user instead of failing.

use tauri::Runtime;
use tauri::plugin::TauriPlugin;

pub(crate) fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new().build()
}
