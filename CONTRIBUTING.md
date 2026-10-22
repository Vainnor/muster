# Contributing

Thanks for helping. Muster is small on purpose: a feature lands when it fits a fast, private,
offline launcher.

## Before you start

- Open an issue for anything bigger than a bug fix, so we can agree on the approach.
- Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). All logic goes in `crates/muster-core`; the
  desktop shell and UIs stay thin.
- Read the rules in [`.claude/rules/`](.claude/rules/). They apply to everyone, not only AI
  agents: `security.md`, `privacy.md` and `rust.md` especially.

## Setup

```sh
git config core.hooksPath .githooks
pnpm --dir desktop install && pnpm --dir desktop build
cargo test --workspace
```

## The gate

Every PR passes:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check && cargo audit
pnpm --dir desktop typecheck && pnpm --dir desktop test
topcoat fmt --check web/src
```

## Commits and PRs

Conventional commits (`feat(core): add linux desktop entry parser`), one logical change each.
Fill in the PR template, including the security checklist. A new dependency needs a reason in the
PR and in `docs/ARCHITECTURE.md` § Dependencies.

## What we will not merge

Telemetry or analytics of any kind, default-on network calls, third-party requests from the app
or the site, shell-string process launching, or a widened Tauri capability without a threat-model
update.

By contributing you agree your work is licensed under the [MIT license](LICENSE).
