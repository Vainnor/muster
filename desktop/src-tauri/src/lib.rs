//! The Tauri shell. It owns the window, the tray and OS integration, and exposes a few thin
//! commands; every decision is made in `muster-core`.

#![forbid(unsafe_code)]

mod autostart;
mod commands;
mod hotkeys;
mod tray;
mod window;

use tauri::RunEvent;

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
            commands::theme_tokens
        ])
        .setup(|app| {
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
