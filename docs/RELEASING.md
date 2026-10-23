# Releasing

## Cut a release

1. On `main` with CI green, bump `version` in the root `Cargo.toml` (`[workspace.package]`) and
   in `desktop/src-tauri/tauri.conf.json`. Run `cargo check` so `Cargo.lock` follows. Commit
   `chore: release vX.Y.Z`.
2. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. `.github/workflows/release.yml` builds on macOS (universal), Linux (x64) and Windows (x64)
   with `pnpm tauri build --features updater` and uploads the bundles as workflow artifacts. A
   sign job signs the updater files (§ Updater), and a publish job writes `latest.json` and
   `SHA256SUMS.txt` and creates the draft release with all of them.
4. Install from the draft on each OS you can reach and check the checksums. On each, add an
   app by hand and launch it: the approval dialog must list its command line, and Return,
   Escape and closing the dialog must all leave it not started; only "Run" starts it. Then
   write the notes and publish. The website's download button and the update check both read
   `releases/latest`, so publishing is the switch. After publishing, press "Check for updates"
   in the previous release to see it offered and installed.

Only the publish job has `contents: write`, and it runs no build code; the build jobs and every
other job in the repo are read-only.

## Icons

`desktop/app-icon.svg` is the source for every app icon, and `web/src/favicon.svg` uses the
same shapes. After changing it, run `pnpm --dir desktop tauri icon app-icon.svg -o <tmp>` and
copy the files `tauri.conf.json` lists, plus `icon.png`, into `desktop/src-tauri/icons/`.

## Unsigned builds

Builds are not code-signed or notarized, and the release notes say so.

- **macOS**: Gatekeeper reports the app as from an unidentified developer, or as damaged when it
  carries the download quarantine flag. Open it once with right-click → Open, or run
  `xattr -dr com.apple.quarantine /Applications/Muster.app` after checking the checksum.
- **Windows**: SmartScreen shows "Windows protected your PC". Choose More info → Run anyway.
- **Linux**: the AppImage needs `chmod +x`; `.deb` and `.rpm` install normally.

Verify a download with `sha256sum -c SHA256SUMS.txt --ignore-missing` (macOS:
`shasum -a 256 -c`).

Signing later needs an Apple Developer ID certificate and notarization credentials, and a
Windows code-signing certificate. They go in GitHub Actions secrets and are passed to the
`pnpm tauri build` step through Tauri's documented environment variables; none of it goes in
the repository.

## Updater

Release builds are built with `--features updater`. The app checks only when the user presses
"Check for updates" or has turned on the check at startup (`desktop/src-tauri/src/updates.rs`).
`plugins.updater` in `tauri.conf.json` holds the manifest URL
(`https://github.com/Vainnor/muster/releases/latest/download/latest.json`), the public key and
`requireSignedVersion: true`.

`release.yml` builds without any secret, then a separate sign job signs the macOS
`Muster_<version>_universal.app.tar.gz`, the AppImage and the NSIS `-setup.exe` with
`tauri signer sign --app-version <version>`, and the publish job writes `latest.json` from
those signatures. The `.deb`, `.rpm` and MSI are left out on purpose: updating them needs
administrator rights, which Muster never asks for, so those installs are sent to the release
page. Because the manifest is read from `releases/latest`, publishing the draft is what offers
the update to installed copies.

The key pair:

1. Generate it on your own machine: `pnpm --dir desktop tauri signer generate -w ~/.tauri/muster.key`.
   Keep the private key and its password outside the repo (a password manager); `.gitignore` and
   `.claude/settings.json` block `*.key` and `~/.tauri`.
2. Add the repository secrets `TAURI_SIGNING_PRIVATE_KEY` (the key file's contents) and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
3. The public key, `~/.tauri/muster.key.pub`, is `plugins.updater.pubkey`.

Losing the private key means installed copies can no longer verify updates; back it up. A new
key needs a release signed with the old one that ships the new public key.

## Website

`web.yml` builds the website image on every push to `main` and on every version tag, and pushes
it to `ghcr.io/vainnor/muster-web`. The image version is the workspace version in `Cargo.toml`
plus the short commit, for example `0.1.0-a1b2c3d`.

| Push | Tags |
| --- | --- |
| `main` | `0.1.0-a1b2c3d`, `a1b2c3d`, `main`, `latest` |
| tag `v0.1.0` | `0.1.0-a1b2c3d`, `a1b2c3d`, `0.1.0`, `0.1` |

A version tag that does not match `Cargo.toml` fails the build. Pull requests that touch the
site build the image without pushing it. The build job has a read-only token; only the publish
job, which runs no build code, gets `packages: write`.

After a `main` push, the deploy job sends a `repository_dispatch` of type `muster-web-image` to
`Vainnor/homelab-infra` with `image`, `tag` (`0.1.0-a1b2c3d`) and `sha`; homelab-infra does
the rollout. It needs the `HOMELAB_DEPLOY_TOKEN` secret, a fine-grained token scoped to
homelab-infra only with Contents: read and write. Without it, the image still pushes and the
deploy job fails with that instruction. A version tag never dispatches.
