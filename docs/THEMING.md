# Theming

A theme is a small JSON file. `muster-core` validates it and `muster-ui` turns it into CSS custom
properties named `--mu-*`, which every stylesheet in the desktop UI and the website uses.

## Presets

`themes/*.json`, embedded into `muster-core` at compile time: `light`, `dark`, `nord`,
`gruvbox`, `catppuccin`, `solarized` (Solarized Light), `high-contrast`, `vainnor`. "Follow
system" is not a file: the UI picks `light` or `dark` from `prefers-color-scheme`. Every preset
passes the contrast checks below; a test enforces it.

`vainnor` is the brand theme, "vainnor slate": blue-slate grounds (`#10141b` background, `#161b24`
panels), cool text, the lilac accent `#b5a8f5`, rust orange `#e8803f` for warnings, a monospace
font, lowercase headings and the `^v█` wordmark.

## Format

Validated twice, by `themes/theme.schema.json` (for editors) and by `muster_core::themes::parse`,
which is the authority.

| Field | Type | Notes |
| --- | --- | --- |
| `$schema` | `"./theme.schema.json"` | Required, exact. |
| `id` | string | 1–40 lowercase letters, digits or `-`. |
| `name` | string | At most 40 characters. |
| `description` | string, optional | At most 200 characters. |
| `mode` | `light` \| `dark` | Sets `color-scheme`. |
| `colors` | object | `background`, `surface`, `border`, `text`, `muted_text`, `accent`, `accent_text`, `danger`, `success`, `warning`. Each `#rrggbb` or `#rrggbbaa`. |
| `radius` | integer 0–24 | px. |
| `density` | `compact` \| `comfortable` \| `spacious` | Scales spacing by 3/4, 1, 5/4. |
| `font` | `system-sans` \| `system-mono` \| `bundled` | `bundled` falls back to the mono stack until a font ships. |
| `icon_size` | integer 16–64 | px. |
| `layout` | `list` \| `grid` | |
| `motion` | boolean | `false` sets durations to 0. |
| `heading_case` | `normal` \| `lowercase` | |
| `wordmark` | string, optional | At most 16 characters, plain text. |

Unknown fields are errors. Files over 16 KiB are rejected before parsing. There is no field for
raw CSS, URLs, images or fonts, so an imported theme cannot load anything or inject a rule.

## Contrast

`parse` returns warnings, never errors, for pairs under WCAG AA: `text` and `muted_text` on
`background` and on `surface`, and `accent_text` on `accent`, need 4.5:1; `accent` on
`background` needs 3:1. The UI shows the warnings and still lets the user apply the theme.

## Tokens

`muster_ui::css_variables` emits:

- Colors: `--mu-bg`, `--mu-surface`, `--mu-border`, `--mu-text`, `--mu-muted`, `--mu-accent`,
  `--mu-accent-text`, `--mu-danger`, `--mu-success`, `--mu-warning`, `--mu-focus-ring`,
  `--mu-color-scheme`.
- Shape and type: `--mu-radius`, `--mu-font`, `--mu-font-mono`, `--mu-icon-size`,
  `--mu-heading-transform`, `--mu-layout`, `--mu-duration`.
- Spacing `--mu-space-1` … `--mu-space-8`: 4, 8, 12, 16, 24, 32, 48, 64 px times density.
- Type `--mu-text-xs`, `-sm`, `-base`, `-md`, `-lg`, `-xl`: 12, 13, 14, 16, 20, 24 px.

`muster_ui::stylesheet` wraps them in a selector and adds a `prefers-reduced-motion` block that
forces `--mu-duration: 0ms`. Fonts are system stacks only; nothing is downloaded.

On the website, `web/styles.css` maps Topcoat UI's variable names (`--background`, `--primary`,
…) onto these tokens.

## Import and export

Settings → Import a theme file reads a `.json` file the user picks (through the webview's file
input; Muster has no filesystem permission) and sends its text to `preview_theme`, which runs
`themes::parse` and returns the tokens and contrast warnings without saving. The window shows the
theme live; Import saves it into `config.json` under `themes`, Cancel restores the chosen theme.

- An imported theme with the same `id` as an earlier import replaces it. The ids of presets and
  `system` are refused (`ThemeError::ReservedId`); change the `id` to import a variant.
- At most 64 imported themes. Each is validated again every time the config loads.
- Export shows a theme as JSON (with `$schema`) to copy and save. Deleting the theme in use
  switches to "follow system".

## Adding a field or token

Change, in one commit: the Rust type and validator, `theme.schema.json`, every preset,
`muster-ui`, this file, and the tests.
