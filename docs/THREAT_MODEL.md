# Threat model

What Muster protects, from whom, and where the controls live. `.claude/rules/security.md` is the
per-change checklist; this file is the reasoning behind it. Update it in the same change as any
new capability, command, CSP source, plugin, file location or import format.

## Assets

1. The user's machine: Muster starts processes, so control of what it launches is control of
   the machine at the user's privilege.
2. The user's config: apps, groups, arguments and working folders, which can reveal paths and
   habits.
3. Release integrity: the binaries people download.

## Trust boundaries

```
 untrusted                     │ trusted
 ──────────────────────────────┼───────────────────────────────────────
 webview (React UI)  ── IPC ──▶│ muster-desktop commands ─▶ muster-core
 theme / group import files ──▶│ size limit ─▶ schema ─▶ semantic validation
 config.json on disk ─────────▶│ store::load (validated like an import)
 .desktop / Info.plist / .lnk ▶│ detection parsers (never a shell)
 website visitors ────────────▶│ muster-web (static pages, no input handled)
```

## Threats and controls

**Script running in the webview** (XSS through an app name, theme value or future bug).
- CSP in `desktop/src-tauri/tauri.conf.json`: `default-src 'self'`, `script-src 'self'`,
  `style-src 'self'`, `img-src 'self' data:`, `connect-src ipc: http://ipc.localhost`,
  `object-src`/`base-uri`/`form-action`/`frame-ancestors 'none'`. No inline script, no eval, no
  remote origin. `devCsp` adds only `'unsafe-inline'` styles and the Vite socket for development.
- `freezePrototype` is on.
- The capability `capabilities/main.json` grants the window `main` only Muster's own commands,
  each by name (the list is `COMMANDS` in `build.rs`), plus `core:event:allow-listen` and
  `allow-unlisten` so the UI can receive `launch-failed` events from a group run. It cannot emit
  events. No `core:default`, no shell, fs or opener plugin, no `remote.urls`. Autostart and
  shortcuts have no JS permission.
- No command takes a file path to read or write. The webview can name a launch target when it
  adds an app (validated, then stored) and can ask to launch an app or group by id; it cannot
  launch a target it supplies directly.
- Commands treat arguments as hostile and return `CommandError { code, message }`, never raw
  `io::Error` text, which can carry paths.
- The UI applies theme tokens only when the name matches `^--mu-[a-z0-9-]+$`.

So injected script could start apps the user already has in Muster, or add a new entry, which
it still could not run inside a group without the user confirming it. It cannot read files,
reach the network or run a command line.

**Launching something the user did not intend.**
- Targets are `Program { path, args, cwd }`, `AppBundle { path }` or `Url { url }`, never a
  command line. `launch::validate` requires an absolute path with no `..` or NUL that is an
  existing file (or `.app` directory), at most 64 arguments of 4096 bytes, a cwd that is a
  directory, and URLs of at most 2048 characters with no control characters, scheme `http`,
  `https` or `mailto`, and a host for http(s).
- Validation runs when a target is saved and again in `launch::plan` right before launch, so a
  config edited on disk is checked too.
- Processes start with an argument array (`std::process::Command`), null stdio and their own
  process group. Bundles and URLs go to a fixed opener as one argument: `/usr/bin/open` on
  macOS, `xdg-open` from `PATH` on Linux, `%SystemRoot%\System32\rundll32.exe
  url.dll,FileProtocolHandler` on Windows. No shell, no elevation.
- A group with a manually added entry asks for confirmation before its first run.
- Linux `Exec` lines will be tokenized by a field-code parser, with unknown codes dropped.

**Malicious import or tampered config.**
- Themes arrive as text from a file the user picked in the webview's file input; the UI checks
  the size first and Rust checks it again. Then: 16 KiB limit, then strict JSON (`deny_unknown_fields`), then values that are colors
  (`#rrggbb[aa]`) or bounded numbers and enums only. There is no field that can hold CSS, a URL,
  `url()` or `@import`. Contrast failures warn and never block.
- Config: 4 MiB limit, versioned schema, `deny_unknown_fields`, the same validation as on save.
  A file that fails is moved to `config.json.corrupt-N` and replaced by defaults, and the user is
  told. Writes are atomic.
- No secrets are stored.

**Malformed or hostile app metadata.** Any program the user installs can drop a `.desktop`
file, bundle or shortcut into a scanned folder, so detection treats them as untrusted input.
- Files over 256 KiB are skipped unread; a scan visits at most 50,000 directory entries and
  returns at most 4,096 apps, and never enters symlinked directories.
- Linux `Exec` lines are tokenized per the Desktop Entry spec into an argument array. Shell
  syntax (`;`, `|`, `$HOME`) stays literal text; unknown field codes reject the entry.
- The `.lnk` reader in `detect/windows.rs` bounds-checks every offset (a test truncates a
  shortcut at every byte). Only ASCII or UTF-16 paths are accepted, `%VARIABLE%` references
  must resolve, and only `.exe` targets are listed.
- `Info.plist` is parsed by the `plist` crate; `CFBundleIconFile` cannot be a path.
- A detected app still has to pass `launch::validate` when it is saved and when it runs.

**Network attacks.** The app makes no network request unless built with `--features updater`
and the user starts a check. Updates are verified against a public key compiled into the app
(`docs/RELEASING.md`). The website handles no input and serves a CSP of `default-src 'none'`
plus `nosniff`, `no-referrer`, COOP/CORP `same-origin`, HSTS and `frame-ancestors 'none'`.

**Supply chain.** `Cargo.lock` and `pnpm-lock.yaml` are committed. CI runs `cargo deny`,
`cargo audit` and `pnpm audit`. Actions are pinned by commit SHA with least-privilege
`permissions:`, and container base images are pinned by digest. Release artifacts get a
`SHA256SUMS.txt`.

## Accepted risks

- Builds are unsigned for now, so the OS cannot vouch for the publisher. Checksums cover
  tampering in transit only if the user checks them (`docs/RELEASING.md`).
- Muster runs whatever the user tells it to. Protecting the user from a program they chose to add
  is out of scope; the confirmation for manual entries is the only speed bump.
- Anything with write access to the user's config dir can change what Muster launches. That
  attacker can already run code as the user.
