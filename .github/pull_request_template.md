## What and why

<!-- What changes, and the reason. Link the issue: Closes #N -->

## How it was verified

<!-- Commands run, tiers that did not run (other OSes, GUI), screenshots for UI. -->

## Checklist

- [ ] `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
- [ ] Desktop UI: `pnpm --dir desktop typecheck && pnpm --dir desktop test` (if touched)
- [ ] Website: `topcoat fmt --check web/src` (if touched)
- [ ] Docs updated per `.claude/rules/docs.md`

### Security and privacy (`.claude/rules/security.md`)

- [ ] No new capability, plugin permission or CSP source (or justified here and in `docs/THREAT_MODEL.md`)
- [ ] New commands validate input in Rust and are granted by name
- [ ] No shell strings or elevation; launch targets validated at save and at launch
- [ ] Untrusted input (imports, config, webview args) is size-limited and validated
- [ ] No network call, telemetry or third-party request added
- [ ] No secrets, personal paths or user data in code, logs, fixtures or this PR
- [ ] New dependencies justified in `docs/ARCHITECTURE.md`
