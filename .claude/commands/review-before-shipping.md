---
description: Review the committed HEAD before pushing or opening a PR — full gate, security and privacy checklist, docs sync, scope check.
---

Review the commit you are about to ship. Report findings; fix only what this branch introduced.

## 1. Pin the commit
`git rev-parse HEAD` is the reviewed SHA. The worktree must be clean (`git status --porcelain`,
including `??` lines). If you change anything below, commit and start again from this step.

## 2. Scope
`git diff origin/main...HEAD --stat`. Name the requirement each changed file serves. Anything no
requirement asks for comes back out, or goes to the user.

## 3. Gate
Run the gate from `CLAUDE.md` → "Quality gate" for everything the diff can reach, output to a
scratch file. Read every `test result:` line; name any failing test.

## 4. Checklists
- `security.md` → "Change checklist", item by item, with the file:line that satisfies each one
  or "not applicable" with why.
- `privacy.md`: does any line send, store or log something new?
- `docs.md`: does each row the diff triggers have its doc updated in this diff?
- Rust: no new `unwrap`/`expect`/`unsafe` outside the `rust.md` exceptions
  (`git diff origin/main...HEAD -- '*.rs' | grep -nE '^\+.*(unwrap\(|expect\(|unsafe )'`).

## 5. Fresh eyes
For a change that touches `desktop/src-tauri/`, `launch`, `store`, imports or the CSP, dispatch
the `security-reviewer` agent with the SHA and wait for it. For a new dependency, dispatch
`dependency-researcher`.

## 6. Report
Gate results by tier (and what did not run, such as other-OS CI jobs and GUI behavior), each
finding as problem / why / effect / risk, and whether HEAD is ready to push.
