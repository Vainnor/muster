---
paths:
  - "desktop/src-tauri/**"
---
# Tauri Rules (not loaded at startup; read in full before touching the shell, capabilities or `tauri.conf.json`)

The checklist is `security.md` → "Webview and IPC". This file is how the shell is built.

## Shape
- `src/lib.rs` builds the app; `src/commands.rs` holds `#[tauri::command]` functions, each a
  few lines that call `muster-core` and map errors to `CommandError`; `src/tray.rs`,
  `src/window.rs`, `src/popup.rs`, `src/hotkeys.rs`, `src/autostart.rs` own one concern each.
- A new command: write it in `commands.rs`, add it to `generate_handler!`, to
  `AppManifest::commands` in `build.rs`, and grant `allow-<name>` in
  `capabilities/main.json`. All four, or it is either unreachable or ungated.
- Plugins controlled from Rust (global shortcut, dialog) get **no** JS permission.

## Capabilities and config
- One capability file per window kind. `main.json` lists only the app's own `allow-*`
  permissions; adding `core:*` needs a reason in its `description`.
- A new capability file is also listed in `app.security.capabilities` in `tauri.conf.json`;
  a non-empty list enables only the files it names.
- `app.security.csp` is security config (`security.md`); `freezePrototype: true`;
  `withGlobalTauri: false`; no `dangerousRemoteDomainIpcAccess`, no `assetProtocol` scope.
- The updater is behind the `updater` cargo feature and `plugins.updater` stays without
  `active`/`endpoints` until `docs/RELEASING.md` § Updater is done.

## Window lifecycle (performance)
- Closing the window **destroys** it; the app keeps running in the tray. Showing it again
  creates a fresh window. Never `hide()` to keep a webview alive in the background.
- `RunEvent::ExitRequested` with no exit code is prevented so the tray stays; Quit from the
  tray exits with code 0.
- Single instance: a second launch focuses (or recreates) the existing window and exits; with
  `--popup` it toggles the popup instead, and with `--run-group <id>` it runs that group.
- The popup is destroyed like the main window, never hidden.

## Per-OS gotchas
- **Linux**: needs `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` to
  build. The tray needs an AppIndicator host; on GNOME without the extension the tray is
  invisible, so the app must stay usable with the window alone. Global shortcuts do not work on
  Wayland through the plugin; feature-detect and say so in the UI.
- **macOS**: closing the last window must not quit (handled by `ExitRequested`). Unsigned builds
  trip Gatekeeper (`docs/RELEASING.md`). Start on login uses a LaunchAgent; do not use AppleScript.
- **Windows**: WebView2 is required (the installer bootstraps it). Unsigned builds show
  SmartScreen. `windows_subsystem = "windows"` in release so no console opens. Paths are
  case-insensitive and may use `\\?\`; compare with `dunce`-style normalization.
