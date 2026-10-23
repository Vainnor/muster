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
  events. No `core:default`, no shell, fs or opener plugin, no `remote.urls`. Autostart,
  shortcuts and the dialog plugin have no JS permission; the dialog plugin's own commands are
  registered but denied.
- No command takes a file path to read or write. The webview can add an app with any valid
  target and ask to launch an app or group by id. A manual app, or a group entry that is manual
  or overrides arguments or the folder, runs only after the user approves its exact target in
  a native dialog that Rust shows (`approval.rs`). The webview has no command that approves;
  `Library::approve` is called only after OK in that dialog.
- Commands treat arguments as hostile and return `CommandError { code, message }`, never raw
  `io::Error` text, which can carry paths.
- The UI applies theme tokens only when the name matches `^--mu-[a-z0-9-]+$`.

So injected script could start detected apps without overrides, and approved entries, and
could add entries and groups. Anything else it asks to run puts a native dialog in front of
the user listing the exact command line; it cannot answer that dialog. It cannot read files or
reach the network.

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
- Approval is per target, not per group: `config.approved` holds the exact targets the user
  OK'd. Changing an entry's arguments or folder, or a pinned app's target changing on a rescan,
  makes a different target that needs approving again. The dialog shows every path and
  argument quoted and escaped, and links in the parsed form the opener receives (an IDN host
  as punycode) (`launch::describe`). Each dialog is at most 1 KiB of text; a longer list is
  split across dialogs and a single longer target is refused.
- "Don't run" is the first button, the default on macOS (a `CFUserNotification` alert) and
  Windows (task dialog), so a stray Return key cannot approve; only the "Run" button does.
  Escape and closing map to Cancel on Windows and GTK. On macOS the system decides what Escape
  does, and GTK's default button is not verified; both need a manual check on each release. After the
  user declines, Muster refuses new approval requests for 5 seconds, and only one dialog is
  open at a time.
- Overrides on detected apps need approval too: arguments for a detected shell or PowerShell
  are a command line.

**Malicious import or tampered config.**
- Themes arrive as text from a file the user picked in the webview's file input; the UI checks
  the size first and Rust checks it again. Then: 16 KiB limit, then strict JSON (`deny_unknown_fields`), then values that are colors
  (`#rrggbb[aa]`) or bounded numbers and enums only. There is no field that can hold CSS, a URL,
  `url()` or `@import`. Contrast failures warn and never block.
- Config: 4 MiB limit, versioned schema, `deny_unknown_fields` on every stored type, and the
  rules the library applies when it adds items: unique ids with the prefix their source
  implies, clean names, item limits, group rules. Targets are validated at launch.
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
- `Info.plist` is parsed by the `plist` crate; `CFBundleIconFile` cannot be a path. The parser
  reads through a 1 MiB read budget (`PLIST_READ_BUDGET`): a binary plist can point at one
  object many times, so a file of a few hundred bytes would otherwise expand exponentially.
  The plist crate drops nested values recursively, including inside `from_reader` when a parse
  fails, and a file under the cap can nest up to 131,072 levels (ASCII format). So each parse
  runs on its own thread with a 64 MiB stack (`PARSE_STACK`), which tests show holds the
  deepest such file in a debug build.
- A detected app is validated by `launch::plan` each time it runs; a pinned copy in the config
  is not validated when it is pinned.

**Network attacks.** The app makes no network request unless built with `--features updater`
and the user starts a check. Updates are verified against a public key compiled into the app
(`docs/RELEASING.md`). The website handles no input and serves a CSP of `default-src 'none'`
plus `nosniff`, `no-referrer`, COOP/CORP `same-origin`, HSTS and `frame-ancestors 'none'`.

**Supply chain.** `Cargo.lock` and `pnpm-lock.yaml` are committed. CI runs `cargo deny`,
`cargo audit` and `pnpm audit`. Actions are pinned by commit SHA with least-privilege
`permissions:`, and container base images are pinned by digest. Release artifacts get a
`SHA256SUMS.txt`. Jobs that run build code have read-only tokens; the jobs that write (the
release draft, the website image push to ghcr.io) only download what a build job produced and
upload it.

## Accepted risks

- Builds are unsigned for now, so the OS cannot vouch for the publisher. Checksums cover
  tampering in transit only if the user checks them (`docs/RELEASING.md`).
- Muster runs whatever the user tells it to. Protecting the user from a program they chose to add
  is out of scope; the approval dialog for manual entries and overrides is the only speed bump.
- Anything with write access to the user's config dir can change what Muster launches. That
  attacker can already run code as the user.
