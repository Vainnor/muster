---
paths:
  - "themes/**"
  - "crates/muster-ui/**"
  - "desktop/src/**"
  - "web/src/**"
  - "web/styles.css"
---
# Theming and Design Rules (not loaded at startup; read in full before UI or theme work)

The theme format and token list are `docs/THEMING.md`.

## Tokens only
- Every color, radius, spacing, font and duration in CSS is a `var(--mu-*)` token from
  `muster-ui`. No hex, `rgb()`, named color or pixel radius in a component stylesheet. A missing
  token is added to `muster-ui` and `docs/THEMING.md`, not inlined.
- Spacing scale: `--mu-space-1..8` (4, 8, 12, 16, 24, 32, 48, 64 px, scaled by density).
  Type scale: `--mu-text-{xs,sm,base,md,lg,xl}` (12, 13, 14, 16, 20, 24 px). Nothing in between.
- A theme changes token values, never layout or behavior.

## Accessibility (WCAG 2.2 AA)
- Text contrast ≥ 4.5:1 (3:1 for large text and UI boundaries). Presets must pass; the Rust
  validator warns on imported themes that fail and the UI shows the warning. It never blocks.
- Visible focus on every interactive element (`--mu-focus-ring`), never `outline: none` without
  a replacement.
- Full keyboard navigation; logical tab order; arrow keys inside lists and grids.
- `prefers-reduced-motion: reduce` and the theme's `motion: false` both disable transitions.
- `prefers-color-scheme` drives the default theme when "follow system" is on.
- Hit targets ≥ 24×24 px. Icons that carry meaning have a text label.

## Launcher UX
- **Fast**: the window shows content on first paint; no spinners for local data.
- **Search-first**: focus lands in the search field; typing filters immediately; `Enter`
  launches the first match.
- **Keyboard-first**: every action has a shortcut; `Escape` clears, then closes.
- **No modal clutter**: one confirmation exists (first run of a group with manual entries).
  Everything else is inline and undoable.
- Settings for non-technical users: presets with live preview first, the editor second.
