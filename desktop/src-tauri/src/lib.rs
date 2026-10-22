//! The Tauri shell. It owns the window, the tray and OS integration, and exposes a few thin
//! commands; every decision is made in `muster-core`.

#![forbid(unsafe_code)]

mod autostart;
mod commands;
mod diag;
mod hotkeys;
mod launcher;
mod state;
mod tray;
mod window;

use anyhow::Context as _;
use tauri::{Manager, RunEvent};

/// Builds and runs the app until the user quits from the tray.
pub fn run() -> anyhow::Result<()> {
    // Single instance goes first so a second launch exits before doing any work.
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::show_or_log(app);
        }))
        .plugin(autostart::plugin())
        .plugin(hotkeys::plugin());

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
            commands::group_confirmation,
            commands::confirm_group,
            commands::run_group,
            commands::get_settings,
            commands::set_theme,
            commands::startup_notice,
            commands::close_window
        ])
        .setup(|app| {
            let dir = app
                .path()
                .app_config_dir()
                .context("finding the app config directory")?;
            std::fs::create_dir_all(&dir).context("creating the app config directory")?;
            app.manage(state::AppState::load(dir));
            tray::create(app.handle())?;
            window::show(app.handle())?;
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
