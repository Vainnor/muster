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
    "remove_app",
    "list_groups",
    "save_group",
    "delete_group",
    "run_group",
    "get_settings",
    "set_theme",
    "startup_notice",
    "close_window",
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
