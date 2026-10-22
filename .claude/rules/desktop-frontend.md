---
paths:
  - "desktop/src/**"
  - "desktop/*.json"
  - "desktop/*.ts"
  - "desktop/index.html"
---
# Desktop Frontend Rules (not loaded at startup; read in full before writing TS/React for the desktop)

Why React and not Topcoat here: `docs/ARCHITECTURE.md` § Phase 0.

- React + TypeScript (strict) + Vite. No UI framework, no CSS-in-JS runtime, no state library
  until a screen proves it needs one.
- **The UI decides nothing.** It renders what a command returns and sends user intent back.
  Validation, ordering, launching and storage are `muster-core`.
- All IPC goes through `src/lib/ipc.ts`: one typed function per command, the only file that
  imports `@tauri-apps/api/core`. Types in `src/lib/types.ts` mirror the Rust structs by hand;
  change both in the same commit.
- **No inline styles and no `<style>` injection**: the CSP has no `'unsafe-inline'`. Styling is
  CSS files using tokens (`theming-and-design.md`). Theme variables are applied with
  `document.documentElement.style.setProperty`, which CSP permits.
- No `dangerouslySetInnerHTML`, no `eval`, no dynamic `import()` of a URL, no remote URLs in
  `src`, `href` or CSS.
- Keyboard first: every action reachable without a mouse, visible focus, `Escape` closes.
- Tests: vitest for `src/lib/**` and for components with behavior; `pnpm test`.
- Dependencies are dev-only unless there is a runtime reason; the bundle stays small.
