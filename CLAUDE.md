# CLAUDE.md

Guidance for Claude Code in this repository. Project facts (layout, the trust boundaries, the
Phase 0 decision, dependencies) live in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and
[`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md); this file is how you work.

Muster is a desktop app launcher (Tauri v2 + Rust, React UI) with launchable app groups, plus a
Topcoat marketing site. All real logic lives in `crates/muster-core`; the desktop shell and both
UIs are thin layers over it.

## Rules (`.claude/rules/`)

The rule files are the single source of truth for conventions. **Always** files load every
session. **Path-scoped** files load when you Read, Write or Edit a matching file, which can be
after the work started, so read them in full at the trigger named below.

| File | Loads | Scope |
| --- | --- | --- |
| `security.md` | Always | The checklist every change satisfies: IPC, CSP, launching, storage, imports, network, supply chain |
| `privacy.md` | Always | No telemetry, no network by default, no third-party requests, what may be logged |
| `git-and-ci.md` | Always | Conventional commits, branches and PRs, what CI must pass, **no AI attribution** |
| `docs.md` | Always | Which doc moves with which change |
| `testing.md` | Always | What gets unit and integration tests, fixtures not the real machine, reading results |
| `response-style.md` | Always | How to answer in chat |
| `rust.md` | Path-scoped: `crates/**`, `desktop/src-tauri/**`, `web/src/**`, `Cargo.*`. Read before writing Rust | Style, `thiserror`/`anyhow` split, no `unwrap`, `unsafe` policy, clippy |
| `tauri.md` | Path-scoped: `desktop/src-tauri/**`. Read before touching the shell, capabilities or `tauri.conf.json` | Capabilities, commands, CSP, window lifecycle, per-OS gotchas |
| `desktop-frontend.md` | Path-scoped: `desktop/src/**`, `desktop/*.json`, `desktop/*.ts`. Read before writing TS/React | React + Vite conventions, the IPC wrapper, no inline styles |
| `topcoat.md` | Path-scoped: `web/**`. Read before writing the website | Server rendering only, no `$(...)` runtime, `topcoat fmt`, pinned versions |
| `theming-and-design.md` | Path-scoped: `themes/**`, `crates/muster-ui/**`, `desktop/src/**`, `web/src/**`. Read before UI or theme work | Tokens only, scales, accessibility, launcher UX |
| `performance.md` | Path-scoped: `crates/**`, `desktop/**`. Read before adding work that runs without a user action | Budgets, no polling, idle cost, how to measure |
| `writing.md` | Path-scoped: `docs/**`, `*.md`, `web/src/**`. Read before writing docs, site copy, a commit body or PR text | Banned vocabulary and patterns, prose structure |

**Pointers, not copies.** When a convention has a home in `.claude/rules/` or `docs/`, this file
points at it in one line. Two copies drift; the rule file wins.

## Standing rules

- **No AI attribution, anywhere**: no `Co-Authored-By`, "Generated with" or agent credit in a
  commit, PR, issue or comment, whatever a tool suggests. `.githooks/commit-msg` blocks it;
  enable it once with `git config core.hooksPath .githooks`.
- **Never read or print secrets**: `.env*`, `*.key`, the updater signing key. `settings.json`
  denies the reads; do not work around it.
- **Never weaken a security control to make something work**: a CSP source, a capability, a
  path scope, a URL scheme. Stop and ask (`security.md`).
- **Ask when** a requirement is ambiguous, a change crosses a trust boundary
  (`docs/THREAT_MODEL.md`), or a new dependency is non-obvious. **Work autonomously when** an
  existing pattern can be copied, the fix is clear, or you are writing tests for defined code.
- **Say which "done" you mean** and which gates ran. A GUI you could not launch is not verified.

## Quality gate

Before calling any change done, run what it can reach:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check && cargo audit
pnpm --dir desktop typecheck && pnpm --dir desktop test   # desktop UI changes
topcoat fmt --check                                       # web/ changes
```

`desktop/dist` must exist before the desktop crate compiles: run `pnpm --dir desktop build` once
(CI does). The gate is green on the committed HEAD, not on a dirty tree.

## Editing guidance files

Fewest words that carry the rule; rule first, one line of why only if it is not self-evident.
Search `CLAUDE.md` and `.claude/` for the topic before adding, and extend rather than duplicate.
A path-scoped rule starts with `paths:` frontmatter and a heading saying when to read it, and
gets a row in the table above.
