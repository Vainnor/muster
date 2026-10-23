//! Global hotkeys: the popup's and each group's. Shortcuts are registered from Rust only; the
//! webview has no global-shortcut permission. `muster_core::library::Library::hotkeys` says
//! which should exist, and [`sync`] makes the OS match.
//!
//! Wayland: the plugin cannot grab keys there, so Muster does not try. The user binds
//! `muster --popup` or `muster --run-group <id>` in their compositor instead, and single
//! instance forwards it.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::str::FromStr;
use std::sync::Mutex;

use anyhow::Context as _;
use muster_core::groups::GroupId;
use muster_core::hotkey::Hotkey;
use muster_core::library::HotkeyAction;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// What the OS has accepted, and what each does. It is only ever locked briefly and never
/// while the plugin is called: registering blocks until the main thread runs it.
static REGISTERED: Mutex<Option<HashMap<Shortcut, HotkeyAction>>> = Mutex::new(None);
/// Makes changes happen one at a time.
static CHANGING: Mutex<()> = Mutex::new(());

pub(crate) fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let Some(action) = action_for(shortcut) else {
                return;
            };
            // On X11 this runs on the plugin's own thread, which must not wait for the main
            // thread: a hotkey change there waits for this thread, so building a window here
            // could deadlock. Hand the work to the main thread instead.
            let handle = app.clone();
            if let Err(e) = app.run_on_main_thread(move || perform(&handle, action)) {
                crate::diag::report("handling a hotkey", &e);
            }
        })
        .build()
}

fn action_for(shortcut: &Shortcut) -> Option<HotkeyAction> {
    let registered = REGISTERED.lock().ok()?;
    registered.as_ref()?.get(shortcut).cloned()
}

/// Runs what a hotkey (or `--popup`/`--run-group` from a second launch) asks for.
pub(crate) fn perform<R: Runtime>(app: &AppHandle<R>, action: HotkeyAction) {
    match action {
        HotkeyAction::Popup => crate::popup::toggle_or_log(app),
        HotkeyAction::RunGroup(id) => run_group(app, id),
    }
}

/// Runs a group the way the window does, approval dialog included. With no window to show
/// the result, a failure is reported in a native dialog.
fn run_group<R: Runtime>(app: &AppHandle<R>, id: GroupId) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<crate::state::AppState>();
        let problem = match crate::commands::run_group(app.clone(), state, id.0).await {
            Ok(failures) if failures.is_empty() => None,
            Ok(failures) => Some(
                failures
                    .iter()
                    .map(|f| f.message)
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            // Declining the approval dialog is the user's answer, not a failure.
            Err(e) if e.code == crate::commands::CommandError::CANCELLED.code => None,
            Err(e) => Some(e.message.to_owned()),
        };
        if let Some(text) = problem {
            crate::approval::tell(&app, "Muster could not start the group", text);
        }
    });
}

/// Whether the OS accepted `hotkey` for `action`.
pub(crate) fn is_active(hotkey: Option<&Hotkey>, action: &HotkeyAction) -> bool {
    let Some(shortcut) = hotkey.and_then(|h| Shortcut::from_str(h.as_str()).ok()) else {
        return false;
    };
    action_for(&shortcut).as_ref() == Some(action)
}

/// Makes the registered hotkeys exactly `wanted`, and returns the ones the OS refused (usually
/// because another app holds them). A refused hotkey stays unregistered; the rest still work.
pub(crate) fn sync<R: Runtime>(
    app: &AppHandle<R>,
    wanted: &[(Hotkey, HotkeyAction)],
) -> anyhow::Result<Vec<Hotkey>> {
    let _one_at_a_time = CHANGING
        .lock()
        .map_err(|_| anyhow::anyhow!("hotkey lock poisoned"))?;
    if session_is_wayland() {
        return Ok(wanted.iter().map(|(h, _)| h.clone()).collect());
    }
    let mut want = HashMap::new();
    for (hotkey, action) in wanted {
        let shortcut = Shortcut::from_str(hotkey.as_str()).context("parsing a validated hotkey")?;
        want.insert(shortcut, (hotkey, action));
    }
    let shortcuts = app.global_shortcut();
    let mut registered = snapshot()?;

    let stale: Vec<Shortcut> = registered
        .keys()
        .filter(|s| !want.contains_key(s))
        .copied()
        .collect();
    for shortcut in stale {
        let released = shortcuts.unregister(shortcut);
        if released.is_ok() {
            registered.remove(&shortcut);
        }
        store(registered.clone())?;
        released.context("releasing a hotkey")?;
    }

    let mut refused = Vec::new();
    for (shortcut, (hotkey, action)) in want {
        match registered.entry(shortcut) {
            // Already held; it may now do something else.
            Entry::Occupied(mut held) => {
                held.insert(action.clone());
            }
            Entry::Vacant(free) => {
                if shortcuts.register(shortcut).is_ok() {
                    free.insert(action.clone());
                } else {
                    refused.push(hotkey.clone());
                }
            }
        }
    }
    store(registered)?;
    Ok(refused)
}

fn snapshot() -> anyhow::Result<HashMap<Shortcut, HotkeyAction>> {
    let registered = REGISTERED
        .lock()
        .map_err(|_| anyhow::anyhow!("hotkey lock poisoned"))?;
    Ok(registered.clone().unwrap_or_default())
}

fn store(registered: HashMap<Shortcut, HotkeyAction>) -> anyhow::Result<()> {
    *REGISTERED
        .lock()
        .map_err(|_| anyhow::anyhow!("hotkey lock poisoned"))? = Some(registered);
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
