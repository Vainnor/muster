//! The theme model and its validation.
//!
//! An imported theme is untrusted input. It is size-checked before parsing, parsed with
//! unknown fields rejected, and every value is a closed type: colors are `#rrggbb` or
//! `#rrggbbaa`, lengths are bounded integers, everything else is an enum. Nothing a theme
//! carries can become raw CSS, a URL or an `@import`. `themes/theme.schema.json` states the
//! same rules for editors; this module is the one that is enforced.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Imports larger than this are refused before parsing.
pub const MAX_THEME_BYTES: usize = 16 * 1024;
pub const MAX_NAME_LEN: usize = 40;
pub const MAX_DESCRIPTION_LEN: usize = 200;
pub const MAX_WORDMARK_LEN: usize = 16;
pub const RADIUS_RANGE: std::ops::RangeInclusive<u8> = 0..=24;
pub const ICON_SIZE_RANGE: std::ops::RangeInclusive<u8> = 16..=64;

/// WCAG 2.2 AA thresholds.
pub const MIN_TEXT_CONTRAST: f64 = 4.5;
pub const MIN_UI_CONTRAST: f64 = 3.0;

/// Preset themes shipped with the app, as `(id, json)`. Their source is `themes/*.json`.
pub const PRESETS: &[(&str, &str)] = &[
    ("light", include_str!("../../../themes/light.json")),
    ("dark", include_str!("../../../themes/dark.json")),
    ("nord", include_str!("../../../themes/nord.json")),
    ("gruvbox", include_str!("../../../themes/gruvbox.json")),
    (
        "catppuccin",
        include_str!("../../../themes/catppuccin.json"),
    ),
    ("solarized", include_str!("../../../themes/solarized.json")),
    (
        "high-contrast",
        include_str!("../../../themes/high-contrast.json"),
    ),
    ("vainnor", include_str!("../../../themes/vainnor.json")),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// Editors read this; it is ignored otherwise.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<SchemaRef>,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub mode: Mode,
    pub colors: Colors,
    pub radius: u8,
    pub density: Density,
    pub font: Font,
    pub icon_size: u8,
    pub layout: Layout,
    pub motion: bool,
    #[serde(default)]
    pub heading_case: HeadingCase,
    /// Short text mark shown beside the app name, rendered as escaped text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wordmark: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeadingCase {
    #[default]
    Normal,
    Lowercase,
}

/// The `$schema` value. Only the bundled relative schema path is accepted, so a theme cannot
/// carry an arbitrary URL even in a field we ignore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaRef;

const SCHEMA_REF: &str = "./theme.schema.json";

impl Serialize for SchemaRef {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(SCHEMA_REF)
    }
}

impl<'de> Deserialize<'de> for SchemaRef {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        if raw == SCHEMA_REF {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom("unexpected $schema value"))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Density {
    Compact,
    Comfortable,
    Spacious,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Font {
    SystemSans,
    SystemMono,
    /// The font bundled with the app. Until one ships it falls back to the system mono stack.
    Bundled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    List,
    Grid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colors {
    pub background: Color,
    pub surface: Color,
    pub border: Color,
    pub text: Color,
    pub muted_text: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub danger: Color,
    pub success: Color,
    pub warning: Color,
}

/// An sRGB color with alpha, written `#rrggbb` or `#rrggbbaa`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Parses `#rrggbb` or `#rrggbbaa`. Nothing else: no names, no `rgb()`, no `url()`.
    pub fn parse(s: &str) -> Result<Self, ThemeError> {
        let hex = s.strip_prefix('#').ok_or(ThemeError::BadColor)?;
        if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ThemeError::BadColor);
        }
        let byte = |i: usize| {
            hex.get(i..i + 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .ok_or(ThemeError::BadColor)
        };
        Ok(Self {
            r: byte(0)?,
            g: byte(2)?,
            b: byte(4)?,
            a: if hex.len() == 8 { byte(6)? } else { 255 },
        })
    }

    /// WCAG relative luminance, ignoring alpha.
    #[must_use]
    pub fn luminance(self) -> f64 {
        let channel = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)?;
        if self.a != 255 {
            write!(f, "{:02x}", self.a)?;
        }
        Ok(())
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// WCAG contrast ratio between two colors, 1.0 to 21.0.
#[must_use]
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ThemeError {
    #[error("theme file is larger than {MAX_THEME_BYTES} bytes")]
    TooLarge,
    #[error("theme file is not valid: {0}")]
    Malformed(String),
    #[error("color must be #rrggbb or #rrggbbaa")]
    BadColor,
    #[error("theme id must be 1 to {MAX_NAME_LEN} lowercase letters, digits or dashes")]
    BadId,
    #[error("theme name must be 1 to {MAX_NAME_LEN} printable characters")]
    BadName,
    #[error("description must be at most {MAX_DESCRIPTION_LEN} printable characters")]
    BadDescription,
    #[error("wordmark must be 1 to {MAX_WORDMARK_LEN} printable characters")]
    BadWordmark,
    #[error("radius must be between 0 and 24")]
    BadRadius,
    #[error("icon size must be between 16 and 64")]
    BadIconSize,
}

/// A pair of colors below the WCAG AA threshold. Shown to the user; never blocks the import.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContrastWarning {
    pub foreground: &'static str,
    pub background: &'static str,
    pub ratio: f64,
    pub required: f64,
}

/// A theme that passed validation. The only way to get one is [`parse`] or [`preset`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidTheme(Theme);

impl ValidTheme {
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.0
    }

    /// Serializes for export.
    pub fn to_json(&self) -> Result<String, ThemeError> {
        serde_json::to_string_pretty(&self.0).map_err(|e| ThemeError::Malformed(e.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub theme: ValidTheme,
    pub warnings: Vec<ContrastWarning>,
}

/// Parses and validates a theme from untrusted bytes.
pub fn parse(bytes: &[u8]) -> Result<Parsed, ThemeError> {
    if bytes.len() > MAX_THEME_BYTES {
        return Err(ThemeError::TooLarge);
    }
    let theme: Theme =
        serde_json::from_slice(bytes).map_err(|e| ThemeError::Malformed(e.to_string()))?;
    validate(&theme)?;
    let warnings = contrast_warnings(&theme.colors);
    Ok(Parsed {
        theme: ValidTheme(theme),
        warnings,
    })
}

/// A bundled preset by id.
#[must_use]
pub fn preset(id: &str) -> Option<ValidTheme> {
    PRESETS
        .iter()
        .find(|(pid, _)| *pid == id)
        .and_then(|(_, json)| parse(json.as_bytes()).ok())
        .map(|p| p.theme)
}

fn validate(theme: &Theme) -> Result<(), ThemeError> {
    let id_ok = !theme.id.is_empty()
        && theme.id.len() <= MAX_NAME_LEN
        && theme
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !id_ok {
        return Err(ThemeError::BadId);
    }
    let name_len = theme.name.trim().chars().count();
    if name_len == 0 || name_len > MAX_NAME_LEN || theme.name.chars().any(char::is_control) {
        return Err(ThemeError::BadName);
    }
    if !printable(theme.description.as_deref(), 0, MAX_DESCRIPTION_LEN) {
        return Err(ThemeError::BadDescription);
    }
    if !printable(theme.wordmark.as_deref(), 1, MAX_WORDMARK_LEN) {
        return Err(ThemeError::BadWordmark);
    }
    if !RADIUS_RANGE.contains(&theme.radius) {
        return Err(ThemeError::BadRadius);
    }
    if !ICON_SIZE_RANGE.contains(&theme.icon_size) {
        return Err(ThemeError::BadIconSize);
    }
    Ok(())
}

/// `None` passes; `Some` must be `min..=max` characters with no control characters.
fn printable(value: Option<&str>, min: usize, max: usize) -> bool {
    value.is_none_or(|v| {
        let len = v.chars().count();
        (min..=max).contains(&len) && !v.chars().any(char::is_control)
    })
}

fn contrast_warnings(c: &Colors) -> Vec<ContrastWarning> {
    let pairs: [(&'static str, Color, &'static str, Color, f64); 6] = [
        (
            "text",
            c.text,
            "background",
            c.background,
            MIN_TEXT_CONTRAST,
        ),
        ("text", c.text, "surface", c.surface, MIN_TEXT_CONTRAST),
        (
            "muted_text",
            c.muted_text,
            "background",
            c.background,
            MIN_TEXT_CONTRAST,
        ),
        (
            "muted_text",
            c.muted_text,
            "surface",
            c.surface,
            MIN_TEXT_CONTRAST,
        ),
        (
            "accent_text",
            c.accent_text,
            "accent",
            c.accent,
            MIN_TEXT_CONTRAST,
        ),
        (
            "accent",
            c.accent,
            "background",
            c.background,
            MIN_UI_CONTRAST,
        ),
    ];
    pairs
        .into_iter()
        .filter_map(|(fg_name, fg, bg_name, bg, required)| {
            let ratio = contrast_ratio(fg, bg);
            (ratio < required).then_some(ContrastWarning {
                foreground: fg_name,
                background: bg_name,
                ratio: (ratio * 100.0).round() / 100.0,
                required,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> serde_json::Value {
        serde_json::from_str(PRESETS[0].1).unwrap()
    }

    fn parse_value(v: &serde_json::Value) -> Result<Parsed, ThemeError> {
        parse(serde_json::to_string(v).unwrap().as_bytes())
    }

    #[test]
    fn every_preset_parses_and_passes_contrast() {
        for (id, json) in PRESETS {
            let parsed = parse(json.as_bytes()).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(parsed.theme.theme().id, *id, "file id matches preset id");
            assert!(parsed.warnings.is_empty(), "{id}: {:?}", parsed.warnings);
        }
    }

    #[test]
    fn preset_lookup() {
        assert!(preset("nord").is_some());
        assert!(preset("does-not-exist").is_none());
    }

    #[test]
    fn color_accepts_only_hex() {
        assert_eq!(
            Color::parse("#2e3440"),
            Ok(Color {
                r: 0x2e,
                g: 0x34,
                b: 0x40,
                a: 255
            })
        );
        assert_eq!(Color::parse("#2E344080").map(|c| c.a), Ok(0x80));
        for bad in [
            "2e3440",
            "#fff",
            "#gggggg",
            "red",
            "rgb(0,0,0)",
            "url(https://x)",
            "#000000;}body{",
            "#00000000ff",
        ] {
            assert_eq!(Color::parse(bad), Err(ThemeError::BadColor), "{bad}");
        }
    }

    #[test]
    fn color_display_round_trips() {
        for s in ["#2e3440", "#2e344080"] {
            assert_eq!(Color::parse(s).unwrap().to_string(), s);
        }
    }

    #[test]
    fn rejects_css_injection_in_any_color_field() {
        let mut v = sample();
        v["colors"]["accent"] = "#000; background: url(https://evil.example/x)".into();
        assert!(matches!(parse_value(&v), Err(ThemeError::Malformed(_))));
    }

    #[test]
    fn rejects_unknown_fields_and_foreign_schema() {
        let mut v = sample();
        v["css"] = "@import url(https://evil.example)".into();
        assert!(matches!(parse_value(&v), Err(ThemeError::Malformed(_))));
        let mut v = sample();
        v["$schema"] = "https://evil.example/schema.json".into();
        assert!(matches!(parse_value(&v), Err(ThemeError::Malformed(_))));
    }

    #[test]
    fn rejects_oversized_input_before_parsing() {
        let big = vec![b' '; MAX_THEME_BYTES + 1];
        assert_eq!(parse(&big), Err(ThemeError::TooLarge));
    }

    #[test]
    fn rejects_out_of_range_lengths_and_bad_names() {
        let mut v = sample();
        v["radius"] = 25.into();
        assert_eq!(parse_value(&v), Err(ThemeError::BadRadius));
        let mut v = sample();
        v["icon_size"] = 8.into();
        assert_eq!(parse_value(&v), Err(ThemeError::BadIconSize));
        let mut v = sample();
        v["id"] = "Not Valid".into();
        assert_eq!(parse_value(&v), Err(ThemeError::BadId));
        let mut v = sample();
        v["name"] = "a\u{7}b".into();
        assert_eq!(parse_value(&v), Err(ThemeError::BadName));
        let mut v = sample();
        v["wordmark"] = "x".repeat(MAX_WORDMARK_LEN + 1).into();
        assert_eq!(parse_value(&v), Err(ThemeError::BadWordmark));
        let mut v = sample();
        v["description"] = "line\nbreak".into();
        assert_eq!(parse_value(&v), Err(ThemeError::BadDescription));
    }

    #[test]
    fn low_contrast_warns_but_still_imports() {
        let mut v = sample();
        v["colors"]["text"] = v["colors"]["background"].clone();
        let parsed = parse_value(&v).unwrap();
        assert!(parsed.warnings.iter().any(|w| w.foreground == "text"
            && w.background == "background"
            && (w.ratio - 1.0).abs() < f64::EPSILON));
    }

    #[test]
    fn contrast_ratio_matches_wcag_reference_values() {
        let black = Color::parse("#000000").unwrap();
        let white = Color::parse("#ffffff").unwrap();
        assert!((contrast_ratio(black, white) - 21.0).abs() < 1e-9);
        assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-9);
        // #767676 on white is the classic 4.54:1 AA boundary.
        let grey = Color::parse("#767676").unwrap();
        assert!((contrast_ratio(grey, white) - 4.54).abs() < 0.01);
    }

    #[test]
    fn export_round_trips() {
        let theme = preset("vainnor").unwrap();
        let again = parse(theme.to_json().unwrap().as_bytes()).unwrap().theme;
        assert_eq!(again, theme);
    }
}
