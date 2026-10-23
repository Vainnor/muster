# Releasing

## Cut a release

1. On `main` with CI green, bump `version` in the root `Cargo.toml` (`[workspace.package]`) and
   in `desktop/src-tauri/tauri.conf.json`. Run `cargo check` so `Cargo.lock` follows. Commit
   `chore: release vX.Y.Z`.
2. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. `.github/workflows/release.yml` builds on macOS (universal), Linux (x64) and Windows (x64)
   with `pnpm tauri build` and uploads the bundles as workflow artifacts. A publish job then
   downloads them, writes `SHA256SUMS.txt` and creates the draft release with all of them.
4. Install from the draft on each OS you can reach and check the checksums. On each, add an
   app by hand and launch it: the approval dialog must list its command line, and Return,
   Escape and closing the dialog must all leave it not started; only "Run" starts it. Then
   write the notes and publish. The website's download button points at `releases/latest`, so publishing is the
   switch.

Only the publish job has `contents: write`, and it runs no build code; the build jobs and every
other job in the repo are read-only.

## Icons

`desktop/app-icon.svg` is the source for every app icon, and `web/src/favicon.svg` uses the
same shapes. After changing it, run `pnpm --dir desktop tauri icon app-icon.svg -o <tmp>` and
copy the files `tauri.conf.json` lists, plus `icon.png`, into `desktop/src-tauri/icons/`.

## Unsigned builds

Builds are not code-signed or notarized yet, and the release notes say so.

- **macOS**: Gatekeeper reports the app as from an unidentified developer, or as damaged when it
  carries the download quarantine flag. Open it once with right-click → Open, or run
  `xattr -dr com.apple.quarantine /Applications/Muster.app` after checking the checksum.
- **Windows**: SmartScreen shows "Windows protected your PC". Choose More info → Run anyway.
- **Linux**: the AppImage needs `chmod +x`; `.deb` and `.rpm` install normally.

Verify a download with `sha256sum -c SHA256SUMS.txt --ignore-missing` (macOS:
`shasum -a 256 -c`).

Signing later needs an Apple Developer ID certificate and notarization credentials, and a
Windows code-signing certificate. They go in GitHub Actions secrets and are passed to
`tauri-action` through its documented environment variables; none of it goes in the repository.

## Updater (disabled)

The updater plugin is compiled only with `--features updater` and the app never checks unless the
user asks or turns the check on. To enable it:

1. Generate a signing key pair on your own machine: `cargo tauri signer generate -w ~/.tauri/muster.key`.
   Keep the private key and its password outside the repo (a password manager); `.gitignore` and
   `.claude/settings.json` block `*.key` and `~/.tauri`.
2. Put the public key in `plugins.updater.pubkey` in `tauri.conf.json` with the endpoint
   `https://github.com/Vainnor/muster/releases/latest/download/latest.json`.
3. Set `bundle.createUpdaterArtifacts` to `true`.
4. Add the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, pass them as env to the `tauri-action` step, build with
   `--features updater`, and set `uploadUpdaterJson: true`.
5. Update `PRIVACY.md` and the website privacy page if the request changes.

Losing the private key means existing installs can no longer verify updates; back it up.

## Website

`web.yml` builds the container image on changes but does not push or deploy it. To deploy, push
the image to a registry you control and apply `web/deploy/` (`web/README.md`).
