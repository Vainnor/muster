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
  events. No `core:default`, no shell, fs or opener plugin, no `remote.urls`. Global
  shortcuts and the dialog plugin have no JS permission; the dialog plugin's own commands are
  registered but denied.
- The popup launcher window (label `popup`, `capabilities/popup.json`) gets less: theme tokens,
  settings, listing apps and groups, `launch_app`, `run_group`, `close_window` and
  `popup_ready`, which shows the hidden popup once its page has painted. It cannot add or
  remove apps, edit groups, change settings or listen to events, and launches from it go
  through the same approval dialog. `close_window` and `popup_ready` act only on the window
  that calls them, and `popup_ready` refuses any window but the popup.
- Hotkeys (the popup's and each group's) are registered from Rust (`hotkeys.rs`); the webview
  has no global-shortcut permission. `set_popup_hotkey` and `set_group_hotkey` take a string
  that `muster_core::hotkey::Hotkey` validates
  (a fixed set of modifiers and keys, Ctrl, Alt or Super required except for F1 to F12, Escape
  kept for closing) before the OS sees it. A hostile page can still pick a valid hotkey that
  gets in the way, such as Ctrl+C, which the OS then routes to Muster everywhere; the user
  changes it back in Settings. That gives it nothing it lacks already: it can launch detected
  apps directly. A hotkey belongs to one thing (the library refuses a second owner, and a
  config file with a duplicate fails validation). A group's hotkey runs the group through the
  same approval dialog as the window's Run button. A failure to start is shown in a native
  dialog, one at a time, and not for a press while a dialog is open or just after a decline;
  a failure after an entry's delay goes to the window only, so with the window closed it is
  not shown. A key held down counts as one press (X11 autorepeat would otherwise start the
  group over and over).
- A group hotkey can bring up the approval dialog while the user is typing in another app.
  A page could pick a key the user presses often for a group with a manual entry. Return and
  Space go to the dialog's default button, which is "Don't run" on macOS and Windows; the GTK
  default is unverified, so Linux is the exposed case. The 5-second cooldown after a decline
  limits retries. This needs the same manual check on each release as the GTK default below.
- A launch with `--autostart` (the login entry) starts in the tray without opening the main
  window; that flag changes nothing else.
- A second launch with `--popup` opens the popup through single instance, and
  `--run-group <id>` runs a group, for compositors where Muster cannot register hotkeys. The id
  must look like a group id (`g:`, at most 64 characters, no control characters) and name an
  existing group, and the run needs the same approval as anywhere else. Anyone who can start
  `muster` as the user can usually already run programs as the user; a sandboxed app that may
  only start other programs could use it to run groups that need no approval, which is
  accepted. There is no URL scheme, so a web page cannot. Nothing else in the forwarded
  arguments is used.
- No command takes a file path to read or write. The webview can add an app with any valid
  target and ask to launch an app or group by id. A manual app, or a group entry that is manual
  or overrides arguments or the folder, runs only after the user approves its exact target in
  a native dialog that Rust shows (`approval.rs`). The webview has no command that approves;
  `Library::approve` is called only after OK in that dialog.
- Commands treat arguments as hostile and return `CommandError { code, message }`, never raw
  `io::Error` text, which can carry paths.
- The UI applies theme tokens only when the name matches `^--mu-[a-z0-9-]+$`.

So injected script could start detected apps without overrides, and approved entries, could
add entries and groups, could turn start on login on or off without a dialog (which makes
only Muster itself start at login), and could close a group: every app it can put in a group
gets the same request to quit as its own Quit command, which leaves a save prompt up. Anything else it asks to run puts a native dialog in front of
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
- The exception is a `.bat` or `.cmd` target: Windows runs it through `cmd.exe`, which parses
  its own command line and expands `%VAR%` and `!VAR!`. Rust's standard library escapes batch
  arguments, and `launch::validate` also rejects `% ! & | < > ^ "` and line breaks in its
  arguments and those characters in its path. It treats a file as a batch file the way the
  standard library does (a case-insensitive `.bat`/`.cmd` suffix on the name), and rejects any
  program name Windows reads differently from the one shown: a trailing dot or space, a `:`
  (alternate data stream), or a path without `.exe` that has a `<path>.exe` beside it, which
  Windows would run instead.
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

**Closing something the user did not mean to close.** `close_group` (main window only, not
the popup) quits every running copy of each app in the group that belongs to the user, as
the user chose; `muster_core::close::plan_close` decides what matches.
- Only the user's own processes, never Muster itself, and never one whose program cannot be
  read (another user's, or one that exited).
- An entry with arguments matches only that exact command line. A shell or interpreter with
  no arguments (`bash`, `cmd`, `python`, `env` and others) matches nothing, since it would
  close every terminal. A script matches its interpreter only when the script is the first
  argument. A bundle matches its `Contents/MacOS` executable, not helpers inside it. A match
  whose parent also matches is left to close with its parent.
- The request to quit is the polite one on each OS: `NSRunningApplication::terminate` for a
  macOS app (`SIGTERM` for a process with no app), `taskkill /PID` without `/F` on Windows,
  `SIGTERM` on Linux. `taskkill` runs from `%SystemRoot%\System32` by absolute path with an
  argument array, and never from `PATH`.
- Force quitting (`SIGKILL`, `taskkill /F`) happens only after "Force quit" in a native dialog
  from Rust, which names the apps; "Leave running" is the first and default button, as in the
  approval dialog. Right before killing, Muster lists and matches again, so a process that
  exited and a pid the OS has since reused are left alone. A pid reused in the moment between
  that listing and the signal is an accepted risk.
- One close runs at a time. The process list (arguments included) stays in memory for that
  close only and is never logged or sent to the webview, which gets back one outcome per entry.

**A login entry that starts the wrong program.** Start on login (`autostart.rs`, built by
`muster_core::login`) writes one entry outside the app's own folders, only when the user turns
it on in Settings: `~/Library/LaunchAgents/com.vainnor.muster.plist` on macOS,
`$XDG_CONFIG_HOME/autostart/com.vainnor.muster.desktop` on Linux, and the `Muster` value under
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` on Windows. Turning it off removes it.
- The entry starts Muster's own executable with the single argument `--autostart`, which only
  keeps the main window closed. On Linux it starts the AppImage instead, but only when the
  executable is inside that AppImage's mount (`APPDIR`), since `APPIMAGE` is inherited by
  anything an AppImage starts. A program that is not an existing file (Linux's
  `muster (deleted)` after an upgrade) or a macOS App Translocation copy is refused. The webview sends a
  boolean (`set_start_on_login`); it cannot choose the program, the arguments or the path.
- Each format is quoted by its own rules, and tests parse every entry back: the plist through
  the plist reader, the `Exec` line through the detection tokenizer (`"`, `` ` ``, `$`, `\` and
  `%` escaped), the Run value with the program in quotes so Windows does not try
  `C:\Program.exe` first. A path that is relative, not Unicode, has a control character, or (on
  Windows) contains `"` is refused rather than written.
- No elevation: the macOS and Linux entries are per user, and the Windows value is under
  `HKCU`.

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
  deepest such file in a debug build. `refresh_apps` runs one scan at a time, and a request that
  waited behind a scan returns that scan's result, so a burst of requests costs one scan.
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
upload it. The website deploy job holds the one stored secret, `HOMELAB_DEPLOY_TOKEN`, which can
write only to `Vainnor/homelab-infra`; the job runs no checkout or build code and sends only the
image name, tag and commit.

## Accepted risks

- Builds are unsigned for now, so the OS cannot vouch for the publisher. Checksums cover
  tampering in transit only if the user checks them (`docs/RELEASING.md`).
- Muster runs whatever the user tells it to. Protecting the user from a program they chose to add
  is out of scope; the approval dialog for manual entries and overrides is the only speed bump.
- Anything with write access to the user's config dir can change what Muster launches. That
  attacker can already run code as the user.
- Tauri 2 buffers and parses an IPC request in full before the command runs, with no size
  limit, so a string from the webview is in Muster's main process before Rust checks its size
  (`themes::MAX_THEME_BYTES` and the other limits bound what is parsed and stored, not what IPC
  accepts). Injected script could send enough to crash Muster; it could not make it run
  anything.
