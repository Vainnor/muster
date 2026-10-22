# Architecture

Muster is one Rust workspace with two programs: the desktop launcher (Tauri 2, React UI) and the
website (Topcoat). All logic lives in `muster-core`; everything else is a thin layer over it.

## Layout

| Path | Crate | What it is |
| --- | --- | --- |
| `crates/muster-core` | `muster-core` | Model, launch validation and planning, groups, config store, theme parsing, app detection. No UI, Tauri, Topcoat or async runtime. |
| `crates/muster-ui` | `muster-ui` | Turns a validated theme into `--mu-*` CSS custom properties. Shared by the desktop UI and the website. |
| `desktop/src-tauri` | `muster-desktop` (bin `muster`) | Tauri shell: window, tray, single instance, autostart, global shortcuts, and the commands the UI may call. |
| `desktop/src` | (npm) | React + TypeScript UI, built by Vite into `desktop/dist`, which Tauri embeds. |
| `web` | `muster-web` | The marketing site, server-rendered with Topcoat. |
| `themes` | | Preset theme JSON files and `theme.schema.json`. `muster-core` embeds the presets at compile time. |

All crates are `publish = false`. `default-members` is core and ui, so a plain `cargo test` runs
without Node or the webview system libraries; `--workspace` builds everything.

## Data flow

```
 React UI (desktop/src)                 Tauri shell (muster-desktop)          muster-core
 ─────────────────────                  ────────────────────────────          ───────────
 lib/ipc.ts  invoke("app_info") ──IPC──▶ commands::app_info ────────────────▶ info::app_info
             invoke("theme_tokens",id) ▶ commands::theme_tokens ─▶ muster-ui ─▶ themes::preset
       ◀── JSON { name, value }[] ─────  CommandError { code, message } on failure
 lib/theme.ts applyTokens → document.documentElement.style (only --mu-* names accepted)
```

The webview never touches the filesystem or a process. It asks; a command validates its arguments
in Rust and calls core. Launching will follow the same path: the UI sends an id, the shell loads
the entry from the store, and `launch::plan` re-validates the target before anything spawns.

The website uses the same presets: `GET /theme.css` renders the light and dark presets through
`muster-ui`, switched by `prefers-color-scheme`.

## Desktop window lifecycle

Startup creates no window from config (`windows: []`); `setup` builds the tray and then the main
window. Closing the window destroys its webview. The run loop calls `prevent_exit` when the exit
has no code, so the process stays in the tray; the tray's Open item and a second launch (single
instance) rebuild the window. Quit exits with code 0. The goal is that an idle Muster in the tray
holds no webview processes (`.claude/rules/performance.md`).

## Phase 0: why the desktop UI is React and not Topcoat

The plan was Topcoat for both the site and the desktop UI. A spike ran Topcoat inside Tauri: an
in-process Topcoat server on `127.0.0.1` with a random port, a per-launch secret exchanged through
a one-time bootstrap URL for an `HttpOnly` cookie (constant-time compare), a `Host` header check,
and the window pointed at it.

| Check | Result |
| --- | --- |
| Pages render in the Tauri webview | pass |
| Packaged build (`cargo tauri build`) with the asset bundle | pass, once tauri's `custom-protocol` feature was enabled unconditionally so `topcoat asset bundle` and the Tauri build compile the same binary |
| Tailwind through Topcoat's pinned standalone CLI | pass |
| Requests without the secret or with a foreign `Host` are refused | pass |
| Topcoat's `$(...)` client runtime under `script-src 'self'` | **fail** |

The runtime compiles every `$(...)` expression and `@event` handler with `new Function`
(`crates/topcoat-runtime/browser/src/expression/compile.ts` in Topcoat 0.10.0), which CSP treats
as eval. Nonces and hashes do not cover it. In headless Chrome under the strict policy the runtime
throws `Failed to compile event @click`; it works once `'unsafe-eval'` is allowed. Streaming
(`live`, `suspense`) also swaps content with inline scripts (`crates/topcoat-router/src/content/view.rs`).

Allowing `'unsafe-eval'` would mean any injected `data-topcoat-on:*` attribute runs as code, so
the desktop UI moved to React + TypeScript + Vite, which builds to static files with no inline
script or eval. The website kept Topcoat in server-rendering mode only: no runtime, no JavaScript
at all, CSP `default-src 'none'`. Revisit if Topcoat ships CSP-safe expressions.

## Dependencies

Few, and each for a reason. `cargo deny check` enforces the license allowlist and crates.io as
the only source.

| Dependency | Used by | Why |
| --- | --- | --- |
| `serde`, `serde_json` | all | Config, themes and IPC are JSON. |
| `thiserror` | core, ui | Typed, matchable errors in libraries (`.claude/rules/rust.md`). |
| `anyhow` | desktop, web | Top-level error glue in binaries. |
| `tempfile` | core | Atomic config writes: temp file in the same dir, fsync, rename. Hand-rolling this gets the Windows rename and cleanup cases wrong. |
| `plist` | core | macOS `Info.plist` files, which are often in Apple's binary format; already in the lockfile through Tauri. The Windows `.lnk` reader is our own: the subset a launcher needs is about 200 lines, and keeping this input boundary in our code puts every bounds check under our tests. |
| `url` | core | WHATWG URL parsing for launch targets, so the scheme allowlist checks what the OS will actually open. |
| `tauri` 2 (`tray-icon`) | desktop | The shell. |
| `tauri-plugin-single-instance` | desktop | A second launch focuses the running app instead of starting another. |
| `tauri-plugin-autostart` | desktop | Start on login, off by default, driven from Rust. |
| `tauri-plugin-global-shortcut` | desktop | Per-group hotkeys, registered from Rust. |
| `tauri-plugin-updater` | desktop, `updater` feature only | Signed update check, compiled out by default. Its default `zip` feature is off: it only handles legacy zipped Windows updates and pulls in zip 4.x with open advisories. |
| `topcoat` `=0.10.0` | web | Server-rendered site; exact pin because the crate is pre-1.0 and the CLI must match. Features limited to router, view, serve, discover, asset, tailwind, ui. |
| `tokio`, `http` | web | Topcoat's runtime and header types. |
| React 19, Vite, TypeScript, vitest | desktop UI | See Phase 0. `@tauri-apps/api` is the only runtime package besides React. |

Licenses: the allowlist includes MPL-2.0 for `cssparser`, `selectors` and `option-ext`, which
Tauri's build tooling and `dirs` pull in. MPL is file-level copyleft and places no obligation on
Muster's own code.

`deny.toml` ignores four "unmaintained" advisories (no vulnerability) on the Linux-only path of
Tauri's stable plugins, each with a reason and a revisit date.
