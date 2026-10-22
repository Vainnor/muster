# Muster

A small, fast desktop app launcher for Windows, macOS and Linux. Muster finds the apps on your
machine and lets you launch them one at a time, or in **groups**: put your editor, terminal and
browser in a "Work" group and start all of them with one click.

> **Status: early scaffold.** Detection, launching and groups are not implemented yet. See
> [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the plan and what exists today.

## Planned features

- Finds installed apps automatically; add any program, script or web link by hand.
- Groups: launch several apps with one button, in order, with optional delays, per-app arguments
  and working directory. Close a whole group again.
- Tray icon, start on login and a global hotkey per group (all optional).
- Themes: light, dark or follow your system, presets (Nord, Gruvbox, Catppuccin, Solarized,
  High Contrast, Vainnor), a theme editor, and import/export.
- Private by default: no telemetry, no account, no network access unless you press "check for
  updates". See [PRIVACY.md](PRIVACY.md).
- Light on resources: near-zero CPU when idle; closing the window frees the web view.

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
