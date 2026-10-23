//! The global hotkey that opens the popup launcher. Shortcuts are registered from Rust only;
//! the webview has no global-shortcut permission.
//!
//! Wayland: the plugin cannot grab keys there, so Muster does not try. The user binds
//! `muster --popup` in their compositor instead, and single instance forwards it.

use std::str::FromStr;
use std::sync::Mutex;

use anyhow::Context as _;
use muster_core::hotkey::Hotkey;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// The registered popup shortcut. It is only ever locked briefly and never while the plugin is
/// called: registering blocks until the main thread runs it.
static POPUP: Mutex<Option<Shortcut>> = Mutex::new(None);
/// Makes changes to the popup shortcut happen one at a time.
static CHANGING: Mutex<()> = Mutex::new(());

pub(crate) fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            // On X11 this runs on the plugin's own thread, which must not wait for the main
            // thread: a hotkey change there waits for this thread, so building the window
            // here could deadlock. Hand the toggle to the main thread instead.
            if event.state == ShortcutState::Pressed && current() == Some(*shortcut) {
                let handle = app.clone();
                if let Err(e) = app.run_on_main_thread(move || crate::popup::toggle_or_log(&handle))
                {
                    crate::diag::report("opening the popup", &e);
                }
            }
        })
        .build()
}

fn current() -> Option<Shortcut> {
    POPUP.lock().ok().and_then(|popup| *popup)
}

/// Whether `saved` is the hotkey registered with the OS right now. False when nothing is
/// registered, or when a failed restore left a different one registered.
pub(crate) fn popup_active(saved: Option<&Hotkey>) -> bool {
    let saved = saved.and_then(|h| Shortcut::from_str(h.as_str()).ok());
    saved.is_some() && current() == saved
}

/// Makes `hotkey` the popup's hotkey, or turns it off for `None`. If the OS refuses the new
/// one (usually because another app holds it), the old one stays registered.
pub(crate) fn set_popup<R: Runtime>(
    app: &AppHandle<R>,
    hotkey: Option<&Hotkey>,
) -> anyhow::Result<()> {
    let _one_at_a_time = CHANGING
        .lock()
        .map_err(|_| anyhow::anyhow!("hotkey lock poisoned"))?;
    if session_is_wayland() {
        return match hotkey {
            Some(_) => Err(anyhow::anyhow!(
                "global hotkeys are not available on Wayland"
            )),
            None => Ok(()),
        };
    }
    let new = hotkey
        .map(|h| Shortcut::from_str(h.as_str()))
        .transpose()
        .context("parsing a validated hotkey")?;
    let old = current();
    if new == old {
        return Ok(());
    }
    let shortcuts = app.global_shortcut();
    if let Some(old) = old {
        shortcuts
            .unregister(old)
            .context("releasing the old hotkey")?;
        store(None)?;
    }
    if let Some(new) = new
        && let Err(e) = shortcuts.register(new)
    {
        if let Some(old) = old {
            shortcuts
                .register(old)
                .context("restoring the old hotkey")?;
            store(Some(old))?;
        }
        return Err(e).context("registering the hotkey");
    }
    store(new)
}

fn store(shortcut: Option<Shortcut>) -> anyhow::Result<()> {
    *POPUP
        .lock()
        .map_err(|_| anyhow::anyhow!("hotkey lock poisoned"))? = shortcut;
    Ok(())
}

/// True in a Wayland session, where the plugin cannot grab keys.
#[cfg(target_os = "linux")]
pub(crate) fn session_is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t.eq_ignore_ascii_case("wayland"))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn session_is_wayland() -> bool {
    false
}
