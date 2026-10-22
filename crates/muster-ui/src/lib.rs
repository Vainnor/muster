//! Design tokens and the theme → CSS custom properties pipeline.
//!
//! Every visual value the desktop UI and the website use is a `--mu-*` custom property
//! produced here from a validated [`Theme`]. Components reference tokens only
//! (`.claude/rules/theming-and-design.md`). Because the input is a [`ValidTheme`], every value
//! written is a hex color, a bounded integer length or a fixed keyword; nothing a theme file
//! contains is copied into CSS as text.

#![forbid(unsafe_code)]

use muster_core::themes::{Density, Font, HeadingCase, Layout, Mode, Theme, ValidTheme};

/// Font stacks. System fonts only: nothing is fetched from a third party.
pub const FONT_SANS: &str = "system-ui, -apple-system, \"Segoe UI\", Roboto, \"Noto Sans\", \
                             \"Helvetica Neue\", Arial, sans-serif";
pub const FONT_MONO: &str = "ui-monospace, \"SF Mono\", \"Cascadia Mono\", \"JetBrains Mono\", \
                             Menlo, Consolas, \"DejaVu Sans Mono\", monospace";

/// Base spacing scale in px, before density scaling.
pub const SPACE_SCALE: [u16; 8] = [4, 8, 12, 16, 24, 32, 48, 64];
/// Type scale in px: xs, sm, base, md, lg, xl.
pub const TEXT_SCALE: [(&str, u16); 6] = [
    ("xs", 12),
    ("sm", 13),
    ("base", 14),
    ("md", 16),
    ("lg", 20),
    ("xl", 24),
];

/// One `--name: value` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssVar {
    pub name: String,
    pub value: String,
}

/// All tokens for `theme`, in a stable order.
#[must_use]
pub fn css_variables(theme: &ValidTheme) -> Vec<CssVar> {
    let t: &Theme = theme.theme();
    let c = &t.colors;
    let mut vars = Vec::with_capacity(40);
    let mut push = |name: &str, value: String| {
        vars.push(CssVar {
            name: format!("--mu-{name}"),
            value,
        });
    };

    push(
        "color-scheme",
        match t.mode {
            Mode::Light => "light",
            Mode::Dark => "dark",
        }
        .into(),
    );
    for (name, color) in [
        ("bg", c.background),
        ("surface", c.surface),
        ("border", c.border),
        ("text", c.text),
        ("muted", c.muted_text),
        ("accent", c.accent),
        ("accent-text", c.accent_text),
        ("danger", c.danger),
        ("success", c.success),
        ("warning", c.warning),
    ] {
        push(name, color.to_string());
    }
    push("focus-ring", format!("0 0 0 2px {}", c.accent));

    push("radius", format!("{}px", t.radius));
    let (num, den) = density_factor(t.density);
    for (i, base) in SPACE_SCALE.iter().enumerate() {
        push(
            &format!("space-{}", i + 1),
            format!("{}px", base * num / den),
        );
    }
    for (name, px) in TEXT_SCALE {
        push(&format!("text-{name}"), format!("{px}px"));
    }
    push(
        "font",
        match t.font {
            Font::SystemSans => FONT_SANS,
            Font::SystemMono | Font::Bundled => FONT_MONO,
        }
        .into(),
    );
    push("font-mono", FONT_MONO.into());
    push("icon-size", format!("{}px", t.icon_size));
    push(
        "layout",
        match t.layout {
            Layout::List => "list",
            Layout::Grid => "grid",
        }
        .into(),
    );
    push(
        "heading-transform",
        match t.heading_case {
            HeadingCase::Normal => "none",
            HeadingCase::Lowercase => "lowercase",
        }
        .into(),
    );
    push("duration", if t.motion { "150ms" } else { "0ms" }.into());
    vars
}

/// The tokens as a stylesheet rule for `selector`, for serving as a CSS file. A
/// `prefers-reduced-motion` block forces durations to zero whatever the theme says.
#[must_use]
pub fn stylesheet(theme: &ValidTheme, selector: &str) -> String {
    let mut lines = vec![format!("{selector} {{")];
    lines.extend(
        css_variables(theme)
            .into_iter()
            .map(|v| format!("  {}: {};", v.name, v.value)),
    );
    lines.push("  color-scheme: var(--mu-color-scheme);".into());
    lines.push("}".into());
    lines.push(format!(
        "@media (prefers-reduced-motion: reduce) {{ {selector} {{ --mu-duration: 0ms; }} }}"
    ));
    let mut css = lines.join("\n");
    css.push('\n');
    css
}

/// Spacing multiplier as a fraction, so the scale stays in whole pixels.
fn density_factor(density: Density) -> (u16, u16) {
    match density {
        Density::Compact => (3, 4),
        Density::Comfortable => (1, 1),
        Density::Spacious => (5, 4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muster_core::themes::{PRESETS, preset};

    fn var<'a>(vars: &'a [CssVar], name: &str) -> &'a str {
        &vars.iter().find(|v| v.name == name).unwrap().value
    }

    #[test]
    fn maps_theme_colors_and_lengths() {
        let vars = css_variables(&preset("nord").unwrap());
        assert_eq!(var(&vars, "--mu-bg"), "#2e3440");
        assert_eq!(var(&vars, "--mu-accent"), "#88c0d0");
        assert_eq!(var(&vars, "--mu-radius"), "4px");
        assert_eq!(var(&vars, "--mu-space-4"), "16px");
        assert_eq!(var(&vars, "--mu-color-scheme"), "dark");
    }

    #[test]
    fn density_scales_spacing() {
        let vars = css_variables(&preset("vainnor").unwrap());
        assert_eq!(var(&vars, "--mu-space-4"), "12px", "compact is 3/4");
        let vars = css_variables(&preset("high-contrast").unwrap());
        assert_eq!(var(&vars, "--mu-space-4"), "20px", "spacious is 5/4");
    }

    #[test]
    fn vainnor_is_monospace_lowercase_and_still() {
        let vars = css_variables(&preset("vainnor").unwrap());
        assert_eq!(var(&vars, "--mu-font"), FONT_MONO);
        assert_eq!(var(&vars, "--mu-heading-transform"), "lowercase");
        assert_eq!(var(&vars, "--mu-duration"), "0ms");
    }

    #[test]
    fn every_preset_produces_the_same_token_set() {
        let names = |id: &str| -> Vec<String> {
            css_variables(&preset(id).unwrap())
                .into_iter()
                .map(|v| v.name)
                .collect()
        };
        let first = names(PRESETS[0].0);
        for (id, _) in PRESETS {
            assert_eq!(names(id), first, "{id}");
        }
    }

    #[test]
    fn values_never_contain_css_control_characters() {
        for (id, _) in PRESETS {
            for v in css_variables(&preset(id).unwrap()) {
                assert!(
                    !v.value.contains([';', '{', '}', '<', '\\']) && !v.value.contains("url("),
                    "{id} {}: {}",
                    v.name,
                    v.value
                );
            }
        }
    }

    #[test]
    fn stylesheet_wraps_vars_and_honors_reduced_motion() {
        let css = stylesheet(&preset("light").unwrap(), ":root");
        assert!(css.starts_with(":root {\n"));
        assert!(css.contains("  --mu-bg: #ffffff;\n"));
        assert!(css.contains("prefers-reduced-motion: reduce"));
    }
}
