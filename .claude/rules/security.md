# Security Rules (always loaded)

The trust boundaries and threats are `docs/THREAT_MODEL.md`. This file is the checklist every
change satisfies. A change that cannot satisfy a line stops and asks; it never loosens the
control to get working.

---

## Webview and IPC
- The webview is untrusted. It renders; `muster-core` decides. A Tauri command validates its
  arguments as if they came from the network.
- Every app command is declared in `desktop/src-tauri/build.rs` (`AppManifest::commands`) and
  granted by name in `desktop/src-tauri/capabilities/main.json`. Nothing is allowed by default.
- No `core:default`, no `shell` plugin, no `fs` plugin, no `opener` with open scopes. A plugin
  permission granted to JS needs a reason in the capability's `description`.
- Filesystem access happens in Rust only, inside the app's own config and cache dirs, except the
  launch target the user picked.
- Capabilities list `windows`, never `remote.urls`. No remote origin ever reaches IPC.
- CSP in `tauri.conf.json`: `default-src 'self'`, `script-src 'self'`, no `'unsafe-inline'`,
  no `'unsafe-eval'`, no remote origins. `devCsp` may relax styles for Vite HMR, nothing else.
  Each source added needs a comment-worthy reason in the PR and a THREAT_MODEL update.

## Launching
- Launch through `std::process::Command` with an argument array, or the OS open API. Never
  build a shell string; never pass user input through `sh -c`, `cmd /c` or `open -a "$x"`.
- Validate a target when it is saved **and again at launch** (`muster_core::launch::validate`):
  absolute path, exists, is a file, no `..` components; URL scheme in `http`, `https`, `mailto`.
- Linux `.desktop` `Exec` lines are tokenized by the field-code parser, never handed to a shell.
  Unknown field codes are dropped; `%f`/`%u` are never filled from untrusted text.
- No elevation, ever: no `runas`, `sudo`, `pkexec`, `osascript ... with administrator`.
- A group holding a manually added entry asks for confirmation before its first run.

## Storage and imports
- Config lives in the OS app-config dir, versioned, written atomically (temp file + rename),
  validated on load. A file that fails validation is backed up and replaced by defaults, and the
  user is told.
- No secrets are stored: no credentials, tokens or keys.
- Imported themes and groups are untrusted: size limit first, then schema, then semantic
  validation. Theme values are colors and lengths only: no raw CSS, `url()`, `@import`,
  `expression(`, `;`, `{`, `}`, `<`.
- An imported group shows every executable it would run before it is saved.

## Network and supply chain
- No network by default (`privacy.md`). The updater is compiled only with `--features updater`
  and is off in config; it never auto-downloads.
- New dependency: say why in the PR and in `docs/ARCHITECTURE.md` § Dependencies. `cargo deny
  check` and `cargo audit` pass; `pnpm audit --audit-level=high` passes.
- GitHub Actions are pinned by commit SHA with the tag in a comment; every workflow and job sets
  least-privilege `permissions:`.
- Never commit a key, token, `.env` or signing material.

## Change checklist
Before every commit, verify:
1. No new capability, plugin permission or CSP source without a reason and a THREAT_MODEL edit
2. Every new command validates its input in Rust and is granted by name
3. No shell string, no elevation, launch targets validated twice
4. Untrusted input (imports, config on disk, webview arguments) is size-limited and validated
5. No secret, personal path or user data in logs, fixtures, issues or commits
6. No new network call; if one is unavoidable it is opt-in and documented in `PRIVACY.md`
7. Dependencies justified; `cargo deny`/`cargo audit` green
