/// Every command the UI may call. Each needs `allow-<name>` in `capabilities/main.json`.
const COMMANDS: &[&str] = &[
    "app_info",
    "theme_tokens",
    "list_themes",
    "preview_theme",
    "import_theme",
    "export_theme",
    "delete_theme",
    "list_apps",
    "refresh_apps",
    "launch_app",
    "add_app",
    "pick_path",
    "remove_app",
    "list_groups",
    "save_group",
    "delete_group",
    "run_group",
    "close_group",
    "get_settings",
    "set_theme",
    "startup_notice",
    "set_popup_hotkey",
    "set_group_hotkey",
    "set_start_on_login",
    "check_for_update",
    "install_update",
    "open_release_page",
    "set_check_updates_on_start",
    "close_window",
    "popup_ready",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Declaring the app's commands makes each one require an explicit `allow-<name>`
    // permission in capabilities/, instead of being callable by default.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )?;
    Ok(())
}
