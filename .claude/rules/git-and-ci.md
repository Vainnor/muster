# Git and CI Rules (always loaded)

## Commits
- Conventional commits: `type(scope): summary`, imperative, lowercase, no trailing period, at
  most 72 characters. Types: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `build`, `ci`,
  `chore`, `security`. Scopes: `core`, `ui`, `desktop`, `web`, `themes`, `docs`, `ci`, `deps`.
- One logical change per commit. A refactor and the feature built on it are two commits.
- The body says why, in prose (`writing.md`). `Closes #N` when it maps to an issue.
- **No AI attribution**: no `Co-Authored-By`, "Generated with", session link or agent credit.
  `.githooks/commit-msg` rejects it (`git config core.hooksPath .githooks`).
- Stage explicit paths; never `git add -A` or `git add .`. Never `--no-verify`.
- Never force-push `main`; never rewrite published history.

## Branches and PRs
- `main` is always releasable. Work goes on `type/short-slug` (`feat/linux-detection`) and lands
  through a PR once the repo has collaborators; until then small commits straight to `main` are
  fine when the full gate is green.
- A PR fills in `.github/pull_request_template.md`, including the security checklist.
- Squash-merge; the PR title becomes the commit and follows the commit format.

## What CI must pass (`.github/workflows/ci.yml`)
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings` on Linux, macOS and Windows
- `cargo test --workspace` on all three
- `cargo deny check` and `cargo audit`
- desktop UI: `pnpm typecheck`, `pnpm test`, `pnpm build`, `pnpm audit --audit-level=high`
- `topcoat fmt --check web/src` for `web/`

A red job is fixed, not re-run until green. A check is never disabled to merge.

## Releases
Tag `vX.Y.Z` on `main` after bumping the version in `Cargo.toml` (workspace) and
`desktop/src-tauri/tauri.conf.json`. `release.yml` builds and drafts the release;
`docs/RELEASING.md` has the rest.
