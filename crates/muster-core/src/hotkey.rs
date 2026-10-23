//! The global hotkey that opens the popup launcher.
//!
//! A [`Hotkey`] is validated on the way in and stored in one canonical form, `Ctrl+Alt+Space`,
//! which is also the accelerator syntax the shell's global-shortcut plugin parses. Only a
//! known subset of keys is accepted, so nothing the plugin would read differently can be saved.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Longest accepted input. The longest valid hotkey (`Ctrl+Alt+Shift+Super+BracketRight`) is
/// 33 bytes.
pub const MAX_HOTKEY_LEN: usize = 64;

/// The hotkey on a fresh install.
pub const DEFAULT_POPUP: &str = "Alt+Space";

/// Modifiers in canonical order, each with the spellings accepted for it.
const MODIFIERS: &[(&str, &[&str])] = &[
    ("Ctrl", &["ctrl", "control"]),
    ("Alt", &["alt", "option"]),
    ("Shift", &["shift"]),
    ("Super", &["super", "cmd", "command", "meta", "win"]),
];

/// Keys other than letters, digits and F1 to F12. Escape is left out: it closes the popup.
const NAMED_KEYS: &[&str] = &[
    "Space",
    "Enter",
    "Tab",
    "Backspace",
    "Delete",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Up",
    "Down",
    "Left",
    "Right",
    "Backquote",
    "Minus",
    "Equal",
    "BracketLeft",
    "BracketRight",
    "Backslash",
    "Semicolon",
    "Quote",
    "Comma",
    "Period",
    "Slash",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HotkeyError {
    #[error("hotkey is empty or longer than {MAX_HOTKEY_LEN} bytes")]
    BadLength,
    #[error("`{0}` is not a modifier Muster knows (Ctrl, Alt, Shift, Super)")]
    UnknownModifier(String),
    #[error("a modifier appears twice")]
    RepeatedModifier,
    #[error("`{0}` is not a key Muster can use for a hotkey")]
    UnknownKey(String),
    #[error("a hotkey needs Ctrl, Alt or Super, unless the key is F1 to F12")]
    NeedsModifier,
}

/// A validated hotkey in canonical form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Hotkey(String);

impl Hotkey {
    /// Parses `Alt+Space`, `cmd+shift+k` and the like. Case and spaces around `+` do not
    /// matter; the result is always canonical.
    pub fn parse(raw: &str) -> Result<Self, HotkeyError> {
        if raw.trim().is_empty() || raw.len() > MAX_HOTKEY_LEN {
            return Err(HotkeyError::BadLength);
        }
        let mut parts: Vec<&str> = raw.split('+').map(str::trim).collect();
        let key = parts.pop().unwrap_or_default();
        let key = canonical_key(key).ok_or_else(|| HotkeyError::UnknownKey(key.to_owned()))?;

        let mut held = [false; MODIFIERS.len()];
        for part in parts {
            let lower = part.to_ascii_lowercase();
            let index = MODIFIERS
                .iter()
                .position(|(_, names)| names.contains(&lower.as_str()))
                .ok_or_else(|| HotkeyError::UnknownModifier(part.to_owned()))?;
            if let Some(seen) = held.get_mut(index)
                && std::mem::replace(seen, true)
            {
                return Err(HotkeyError::RepeatedModifier);
            }
        }
        let is_function_key = key
            .strip_prefix('F')
            .is_some_and(|n| n.parse::<u8>().is_ok());
        // Shift alone is not enough: Shift+A is a capital A, and the grab is system-wide.
        let shift_only = held
            .iter()
            .zip(MODIFIERS)
            .all(|(on, (name, _))| !on || *name == "Shift");
        if shift_only && !is_function_key {
            return Err(HotkeyError::NeedsModifier);
        }

        let mut out: Vec<&str> = MODIFIERS
            .iter()
            .zip(held)
            .filter_map(|((name, _), on)| on.then_some(*name))
            .collect();
        out.push(&key);
        Ok(Self(out.join("+")))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The hotkey on a fresh install.
    #[must_use]
    pub fn default_popup() -> Self {
        Self(DEFAULT_POPUP.to_owned())
    }
}

/// The canonical spelling of a key, or `None` if it is not one Muster accepts.
fn canonical_key(key: &str) -> Option<String> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return (c.is_ascii_alphanumeric()).then(|| c.to_ascii_uppercase().to_string());
    }
    let upper = key.to_ascii_uppercase();
    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
        return ((1..=12).contains(&n) && upper == format!("F{n}")).then(|| format!("F{n}"));
    }
    NAMED_KEYS
        .iter()
        .find(|name| name.eq_ignore_ascii_case(key))
        .map(|name| (*name).to_owned())
}

impl TryFrom<String> for Hotkey {
    type Error = HotkeyError;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::parse(&raw)
    }
}

impl From<Hotkey> for String {
    fn from(hotkey: Hotkey) -> Self {
        hotkey.0
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(raw: &str) -> String {
        Hotkey::parse(raw).unwrap().0
    }

    #[test]
    fn canonicalizes_case_spacing_aliases_and_order() {
        assert_eq!(ok("alt+space"), "Alt+Space");
        assert_eq!(ok(" Shift + cmd + k "), "Shift+Super+K");
        assert_eq!(ok("option+CONTROL+pageup"), "Ctrl+Alt+PageUp");
        assert_eq!(ok("Win+1"), "Super+1");
        assert_eq!(ok("Meta+Slash"), "Super+Slash");
    }

    #[test]
    fn the_default_parses_to_itself() {
        assert_eq!(ok(DEFAULT_POPUP), DEFAULT_POPUP);
        assert_eq!(Hotkey::default_popup().as_str(), DEFAULT_POPUP);
    }

    #[test]
    fn a_function_key_may_stand_alone() {
        assert_eq!(ok("f12"), "F12");
        assert_eq!(ok("F1"), "F1");
        assert_eq!(ok("Shift+F1"), "Shift+F1");
    }

    #[test]
    fn other_keys_need_a_modifier() {
        for raw in [
            "A",
            "Space",
            "Enter",
            "1",
            "Shift+A",
            "Shift+1",
            "Shift+Space",
        ] {
            assert_eq!(Hotkey::parse(raw), Err(HotkeyError::NeedsModifier), "{raw}");
        }
    }

    #[test]
    fn rejects_unknown_keys_including_escape() {
        for (raw, key) in [
            ("Alt+Escape", "Escape"),
            ("Alt+F13", "F13"),
            ("Alt+F01", "F01"),
            ("Alt+Å", "Å"),
            ("Alt+", ""),
            ("Ctrl+Alt", "Alt"),
        ] {
            assert_eq!(
                Hotkey::parse(raw),
                Err(HotkeyError::UnknownKey(key.into())),
                "{raw}"
            );
        }
    }

    #[test]
    fn rejects_unknown_and_repeated_modifiers() {
        assert_eq!(
            Hotkey::parse("Hyper+A"),
            Err(HotkeyError::UnknownModifier("Hyper".into()))
        );
        assert_eq!(
            Hotkey::parse("Ctrl+Control+A"),
            Err(HotkeyError::RepeatedModifier)
        );
    }

    #[test]
    fn rejects_empty_and_overlong_input() {
        assert_eq!(Hotkey::parse("  "), Err(HotkeyError::BadLength));
        let long = format!("{}A", "Ctrl+".repeat(13));
        assert_eq!(Hotkey::parse(&long), Err(HotkeyError::BadLength));
    }

    #[test]
    fn deserializing_validates() {
        let good: Hotkey = serde_json::from_str("\"ctrl+space\"").unwrap();
        assert_eq!(good.as_str(), "Ctrl+Space");
        assert!(serde_json::from_str::<Hotkey>("\"Ctrl+Escape\"").is_err());
    }
}
