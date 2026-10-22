---
paths:
  - "web/**"
---
# Topcoat Rules (not loaded at startup; read in full before writing the website)

Topcoat (https://github.com/tokio-rs/topcoat) is early-stage and experimental. Its `main`
branch docs (`llms.txt`, `docs/`) are the source of truth, not memory; check the docs for the
**pinned version's tag** before using an API, and say when something is uncertain.

## How we use it
- **Server rendering only.** Pages are `#[page]`/`#[layout]` with `view!` and `#[component]`,
  module routing under `web/src/app/`. No `$(...)` expressions, signals, shards, procedures,
  `live!` or `suspense`: the runtime compiles expressions with `new Function` and streamed swaps
  use inline scripts, both blocked by our CSP (`docs/ARCHITECTURE.md` § Phase 0). The site
  needs no JavaScript.
- Features: `default-features = false`, then only `router`, `view`, `serve`, `discover`,
  `asset`, `tailwind`, `ui`. No `cookie`, `session` or `runtime`.
- Tailwind through the `tailwind` feature (no Node), pinned with `version_checksum` in
  `build.rs`. Colors and radii come from `muster-ui` tokens; Tailwind's palette classes
  (`bg-slate-900`) are not used.
- Components from `topcoat ui add <name>` are vendored into `web/src/components/` and edited
  freely; `components.toml` is committed. Their theme variables map onto our tokens in
  `web/styles.css`. Do not load the font the UI theme names; we use system fonts.
- Internal links use `href!(handler)`; external links are plain strings.
- Every response passes the security-headers layer (`web/src/security.rs`).
- `topcoat fmt web/src` after `cargo fmt` (a bare `topcoat fmt` also scans `target/` and fails); CI runs `topcoat fmt --check web/src`.

## Pinning and upgrades
- `topcoat = "=X.Y.Z"` exact, in both `[dependencies]` and `[build-dependencies]`, matching the
  installed `topcoat-cli`. Dependabot proposes bumps; read the CHANGELOG, rebuild, re-run the
  asset bundle and look at every page before merging.
- No cleverness: no custom registries, no undocumented internals, no reliance on behavior the
  docs do not state.
