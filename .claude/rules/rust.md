---
paths:
  - "crates/**"
  - "desktop/src-tauri/**"
  - "web/src/**"
  - "web/build.rs"
  - "Cargo.toml"
  - "Cargo.lock"
---
# Rust Rules (not loaded at startup; read in full before writing or reviewing Rust)

## Gate
- `cargo clippy --workspace --all-targets -- -D warnings`. `--all-targets` matters: test modules
  only compile under it. Workspace lints live in the root `Cargo.toml` `[workspace.lints]`; every
  crate has `[lints] workspace = true`.
- `cargo fmt --all`; `rustfmt.toml` is the style. In `web/`, also `topcoat fmt` (`topcoat.md`).
- Implementation goes above `#[cfg(test)] mod tests`, or clippy's `items_after_test_module` fails.
- Over seven parameters fails `too_many_arguments`: group them in a struct, never `#[allow]`.

## Errors
- **Libraries (`muster-core`, `muster-ui`) use `thiserror`**: one `Error` enum per crate (or per
  module when variants differ in kind), variants a caller can match on, `#[source]` kept.
- **Binaries (`muster-desktop`, `muster-web`) use `anyhow`** for setup and top-level glue, with
  `.context("what was being done")`.
- A Tauri command returns `Result<T, CommandError>`, a serializable type carrying a stable code
  and a user-safe message. Never send an `io::Error` text (it can hold a path) to the webview.
- No `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!` or indexing that can panic in
  non-test code (`clippy::unwrap_used`, `expect_used`, `panic`, `indexing_slicing` deny it). The
  one exception is an invariant the code itself guarantees, written as `expect("<invariant>")`
  with `#[allow(clippy::expect_used, reason = "...")]` on the line.
- Never swallow: no `let _ = fallible()`, no `.ok()` dropping an error, no `unwrap_or_default()`
  on I/O. Log and surface it, or propagate.

## `unsafe`
- `#![forbid(unsafe_code)]` in `muster-core`, `muster-ui` and `muster-web`. The workspace lint
  is `unsafe_code = "deny"`.
- A platform API that truly needs `unsafe` (Windows shell, macOS LaunchServices) goes in its own
  small module in `muster-desktop` or a dedicated `muster-sys` crate, behind a safe wrapper, with
  a `// SAFETY:` comment on every block and the reason in `docs/ARCHITECTURE.md`. Prefer a
  maintained crate that already wraps it.

## Style
- `muster-core` has no UI, Tauri, Topcoat or async-runtime dependency. If a feature seems to
  need one, the boundary is wrong.
- Types over strings: `AppId`, `GroupId`, `ThemeName` newtypes; enums for closed sets.
- Validation returns the validated type (`Theme::parse(bytes) -> Result<ValidTheme>`), so an
  unvalidated value cannot reach the code that trusts it.
- Blocking work (filesystem scans, process spawning) runs off the async runtime
  (`tauri::async_runtime::spawn_blocking`) in the shell; core stays synchronous.
- Per-OS code: `#[cfg(target_os = "...")]` at module level (`detect/linux.rs`), never scattered
  through a function. Pure parsing stays cross-platform so every OS's parser tests on every CI
  runner.
- Comments explain a non-obvious reason for the code as it is now. No history, no issue
  narratives.
