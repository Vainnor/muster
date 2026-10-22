---
description: Read-only performance check of the branch against the budgets in performance.md — idle work, polling, window lifecycle, detection cost, bundle size.
---

Assess the current branch against `.claude/rules/performance.md`. Read-only: report, do not fix.
Every number is measured or labeled an estimate.

1. `git diff origin/main...HEAD --stat`. For each changed file, say what triggers it: a user
   action, an OS event, startup, or nothing (idle). Anything that runs while idle is a finding.
2. Grep the diff for timers and loops: `interval`, `sleep`, `loop {`, `setInterval`,
   `setTimeout`, `requestAnimationFrame`, `notify::` watchers. Each needs a trigger and an end.
3. Window lifecycle: closing still destroys the webview (`desktop/src-tauri/src/window.rs`); no
   new `hide()` path keeps it alive.
4. Detection and launch: work is off the main thread; no rescans per render; no per-entry
   process spawn or filesystem walk beyond the documented roots.
5. Frontend: `pnpm --dir desktop build` and report the `dist/` size against `origin/main`.
6. If a release build is available, measure idle CPU and tray-only memory with the commands in
   `performance.md` → "How to measure" and compare with the budgets.

Report: each finding with its trigger and cost, the measured numbers, and what you could not
measure on this OS.
