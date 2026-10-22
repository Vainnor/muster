fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Declaring the app's commands makes each one require an explicit `allow-<name>`
    // permission in capabilities/, instead of being callable by default.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["app_info", "theme_tokens"])),
    )?;
    Ok(())
}
