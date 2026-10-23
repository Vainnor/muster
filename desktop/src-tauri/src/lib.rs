//! The Tauri shell. It owns the window, the tray and OS integration, and exposes a few thin
//! commands; every decision is made in `muster-core`.

#![forbid(unsafe_code)]

mod approval;
mod autostart;
mod commands;
mod diag;
mod hotkeys;
mod launcher;
mod popup;
mod processes;
mod state;
mod tray;
mod window;

use anyhow::Context as _;
use muster_core::groups::GroupId;
use muster_core::library::HotkeyAction;
use tauri::{Manager, RunEvent};

/// The flag a launch passes to run a group, for compositors where Muster cannot register a
/// global hotkey: `muster --run-group g:work`.
const RUN_GROUP_FLAG: &str = "--run-group";

/// The group id after `--run-group`, if any. Only a value that looks like a group id is taken;
/// the library still has to find it, and the run goes through the usual approval.
fn run_group_arg<'a>(mut args: impl Iterator<Item = &'a str>) -> Option<GroupId> {
    args.find(|a| *a == RUN_GROUP_FLAG)?;
    let id = args.next()?;
    let valid = id.starts_with("g:") && id.len() <= 64 && !id.chars().any(char::is_control);
    valid.then(|| GroupId(id.to_owned()))
}

/// Builds and runs the app until the user quits from the tray.
pub fn run() -> anyhow::Result<()> {
    // Single instance goes first so a second launch exits before doing any work.
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(id) = run_group_arg(argv.iter().map(String::as_str)) {
                hotkeys::perform(app, HotkeyAction::RunGroup(id));
            } else if argv.iter().any(|a| a == popup::FLAG) {
                popup::toggle_or_log(app);
            } else if argv.iter().any(|a| a == autostart::FLAG) {
                // The login entry fired while Muster already runs: nothing to open.
            } else {
                window::show_or_log(app);
            }
        }))
        .plugin(hotkeys::plugin())
        // Rust-only: the capability grants no `dialog:*` permission, so JS cannot open one.
        .plugin(tauri_plugin_dialog::init());

    #[cfg(feature = "updater")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    let app = builder
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::theme_tokens,
            commands::list_themes,
            commands::preview_theme,
            commands::import_theme,
            commands::export_theme,
            commands::delete_theme,
            commands::list_apps,
            commands::refresh_apps,
            commands::launch_app,
            commands::add_app,
            commands::remove_app,
            commands::list_groups,
            commands::save_group,
            commands::delete_group,
            commands::run_group,
            commands::close_group,
            commands::get_settings,
            commands::set_theme,
            commands::startup_notice,
            commands::set_popup_hotkey,
            commands::set_group_hotkey,
            commands::set_start_on_login,
            commands::close_window,
            commands::popup_ready
        ])
        .setup(|app| {
            let dir = app
                .path()
                .app_config_dir()
                .context("finding the app config directory")?;
            std::fs::create_dir_all(&dir).context("creating the app config directory")?;
            let state = state::AppState::load(dir);
            let wanted = state
                .library()
                .map_err(|_| anyhow::anyhow!("reading settings"))?
                .hotkeys();
            app.manage(state);
            tray::create(app.handle())?;
            // A hotkey another app holds must not stop Muster starting; Settings and the
            // group list show which ones the OS refused.
            if let Err(e) = hotkeys::sync(app.handle(), &wanted) {
                diag::report("registering hotkeys", e.as_ref());
            }
            // Started by the login entry, Muster stays in the tray until the user asks.
            let at_login = std::env::args_os().any(|a| a == autostart::FLAG);
            let args: Vec<String> = std::env::args_os()
                .filter_map(|a| a.into_string().ok())
                .collect();
            if let Some(id) = run_group_arg(args.iter().map(String::as_str)) {
                hotkeys::perform(app.handle(), HotkeyAction::RunGroup(id));
            } else if std::env::args_os().any(|a| a == popup::FLAG) {
                popup::toggle(app.handle())?;
            } else if !at_login {
                window::show(app.handle())?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())?;

    app.run(|_app, event| {
        // Closing the window destroys it (and its webview) but keeps Muster in the tray.
        // Quit from the tray calls `exit(0)`, which arrives here with a code and is allowed.
        if let RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(args: &[&str]) -> Option<String> {
        run_group_arg(args.iter().copied()).map(|id| id.0)
    }

    #[test]
    fn reads_the_group_id_after_the_flag() {
        assert_eq!(
            arg(&["muster", "--run-group", "g:work"]).as_deref(),
            Some("g:work")
        );
    }

    #[test]
    fn ignores_a_missing_or_malformed_group_id() {
        for args in [
            &["muster", "--run-group"][..],
            &["muster", "--run-group", "m:editor"],
            &["muster", "--run-group", "g:a\nb"],
            &["muster", "g:work"],
        ] {
            assert_eq!(arg(args), None, "{args:?}");
        }
        let long = format!("g:{}", "a".repeat(63));
        assert_eq!(arg(&["muster", "--run-group", &long]), None);
    }
}
