# Muster

A small, fast desktop app launcher for Windows, macOS and Linux. Muster finds the apps on your
machine and lets you launch them one at a time, or in **groups**: put your editor, terminal and
browser in a "Work" group and start all of them with one click.

> **Status: early preview.** Not released yet; builds are unsigned.

## Features

Working now:

- Finds installed apps automatically (`.desktop` entries on Linux, `.app` bundles on macOS,
  Start Menu shortcuts on Windows); add any program, script or web link by hand.
- Search-first launcher: type to filter, arrow keys to move, Enter to start, Escape to close.
- Popup launcher: a hotkey you choose (Alt+Space by default) opens a small search window from
  anywhere to start an app or group with the keyboard or mouse. On Wayland, bind
  `muster --popup` in your compositor instead.
- Groups: launch several apps with one action, in order, with optional delays, per-app
  arguments and working folder.
- Before it runs something you added by hand, or an app with arguments or a folder you set,
  Muster shows the exact command line in a system dialog and asks you first. It asks again if
  that command line changes.
- Tray icon; closing the window keeps Muster in the tray.
- Themes: light, dark or follow your system, presets (Nord, Gruvbox, Catppuccin, Solarized,
  High Contrast, Vainnor, Vainnor Ember, Prism), and theme files you import or export, with a
  live preview and a warning when text would be hard to read.
- Private by default: no telemetry, no account, and no network access at all. See
  [PRIVACY.md](PRIVACY.md).
- Light on resources: no CPU use while idle; closing the window frees the web view.

Planned:

- Close a whole group again; a global hotkey per group; start on login (optional).
- A theme editor.
- An update check that runs only when you press it.

## Install

Download the build for your OS from
[Releases](https://github.com/Vainnor/muster/releases/latest). Builds are **not code-signed** yet,
so Windows SmartScreen and macOS Gatekeeper will warn the first time; see
[docs/RELEASING.md](docs/RELEASING.md#unsigned-builds) for how to open them.

## Repository layout

| Path | What |
| --- | --- |
| `crates/muster-core` | All app logic: detection, launching, groups, storage, theme model. No UI deps. |
| `crates/muster-ui` | Design tokens and the theme → CSS variables pipeline, shared by app and site. |
| `desktop/` | Tauri v2 shell (`src-tauri/`) and the React + TypeScript UI. |
| `web/` | The website (Topcoat), Dockerfile and example Kubernetes manifests. |
| `themes/` | Preset themes and `theme.schema.json`. |
| `docs/` | Architecture, threat model, theming, releasing. |

## Building

Requirements: Rust (pinned in `rust-toolchain.toml`), Node 22+ with pnpm, and on Linux the
WebKitGTK dev packages (`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`).

```sh
git config core.hooksPath .githooks       # commit-message checks
pnpm --dir desktop install
pnpm --dir desktop build                  # the desktop crate embeds desktop/dist
cargo test --workspace
cargo tauri dev --config desktop/src-tauri/tauri.conf.json   # or: cd desktop && pnpm tauri dev
```

The website: see [web/README.md](web/README.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Security issues: [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE) © 2026 Vainnor
