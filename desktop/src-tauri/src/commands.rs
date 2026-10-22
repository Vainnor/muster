//! Commands the UI may call. Each one is declared in `build.rs` and granted by name in
//! `capabilities/main.json`. Arguments are untrusted and validated here or in core.

use muster_core::info::AppInfo;
use serde::Serialize;

/// Error sent to the webview: a stable code and a message safe to show. Never a path.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct CommandError {
    code: &'static str,
    message: &'static str,
}

/// One `--mu-*` custom property.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Token {
    name: String,
    value: String,
}

#[tauri::command]
pub(crate) fn app_info() -> AppInfo {
    muster_core::info::app_info()
}

/// Design tokens for a preset theme.
#[tauri::command]
pub(crate) fn theme_tokens(id: &str) -> Result<Vec<Token>, CommandError> {
    let theme = muster_core::themes::preset(id).ok_or(CommandError {
        code: "unknown_theme",
        message: "There is no theme with that name.",
    })?;
    Ok(muster_ui::css_variables(&theme)
        .into_iter()
        .map(|v| Token {
            name: v.name,
            value: v.value,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_comes_from_core() {
        assert_eq!(app_info(), muster_core::info::app_info());
    }

    #[test]
    fn theme_tokens_for_a_preset() {
        let tokens = theme_tokens("nord").unwrap();
        assert!(
            tokens
                .iter()
                .any(|t| t.name == "--mu-bg" && t.value == "#2e3440")
        );
    }

    #[test]
    fn theme_tokens_rejects_unknown_and_path_like_ids() {
        for id in ["nope", "../../etc/passwd", ""] {
            assert_eq!(
                theme_tokens(id).unwrap_err().code,
                "unknown_theme",
                "{id:?}"
            );
        }
    }
}
