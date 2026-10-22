---
paths:
  - "crates/**"
  - "desktop/**"
---
# Performance Rules (not loaded at startup; read in full before adding work that runs without a user action)

## Budgets (release build, mid-range laptop)

| Metric | Budget |
| --- | --- |
| Cold start to interactive window | < 500 ms |
| Idle CPU, window open, no input | ~0 % (< 0.1 % averaged over 60 s) |
| Idle CPU, window closed (tray only) | 0 % (no wakeups beyond the OS event loop) |
| Memory, window closed (tray only) | < 30 MB RSS for the main process, no webview processes |
| Memory, window open | < 150 MB total including webview processes |
| App detection, on demand | < 1 s for 500 entries |
| Installer size | < 15 MB |

A change that moves a metric past its budget is a bug, and a change that moves one by more
than 10 % says so in the PR with numbers.

## Rules
- **No polling, no timers while idle.** Work happens because of a user action or an OS event.
  An optional file watch (later) uses OS notifications, debounced, and is off by default.
- **Detection runs on demand** (startup, refresh button), off the main thread, and caches the
  result in memory for the session.
- **Closing the window destroys the webview** (`tauri.md` → "Window lifecycle").
- No background threads that outlive their task. No async runtime work while idle.
- Measure before optimizing; attach the before/after numbers.

## How to measure
- Cold start: `hyperfine --warmup 3 --runs 10 '<app> --measure-startup'` once the flag exists;
  until then, time from launch to first paint with the webview inspector's performance tab.
- Idle CPU and wakeups: macOS `top -o cpu -pid <pid>` and Activity Monitor "Idle Wake Ups";
  Linux `pidstat -u -p <pid> 5 12` and `powertop`; Windows Process Explorer (CPU cycles delta).
- Memory: sum RSS of the app and its webview children (`ps -o rss= -p <pids>` on macOS/Linux;
  Task Manager "Details" on Windows: `msedgewebview2.exe` children).
- Size: the artifacts `release.yml` produces.
