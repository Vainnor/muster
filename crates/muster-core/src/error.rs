use std::path::PathBuf;

/// Every failure `muster-core` reports. Variants are stable so callers can map them to
/// user-facing messages without parsing text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("launch target is invalid: {0}")]
    InvalidTarget(#[from] crate::launch::TargetError),

    #[error("group is invalid: {0}")]
    InvalidGroup(#[from] crate::groups::GroupError),

    #[error("theme is invalid: {0}")]
    InvalidTheme(#[from] crate::themes::ThemeError),

    #[error("config file {path} could not be read or written")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("config file is not valid: {0}")]
    InvalidConfig(String),

    #[error("name is empty, too long or has control characters")]
    InvalidName,

    #[error("there is no app or group with that id")]
    UnknownId,

    #[error("the limit for this kind of item is reached")]
    LimitReached,

    #[error("the app could not be started")]
    Launch {
        #[source]
        source: std::io::Error,
    },

    #[error("no program is available to open links and bundles")]
    NoOpener,

    /// The app or group runs a target the user has not approved. The shell asks, then calls
    /// [`crate::library::Library::approve`].
    #[error("the user has not approved what this would run")]
    NeedsApproval,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// A short, stable code for the variant, safe to show to the user or send to the webview.
    /// Never includes paths or other user data.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidTarget(_) => "invalid_target",
            Self::InvalidGroup(_) => "invalid_group",
            Self::InvalidTheme(_) => "invalid_theme",
            Self::Io { .. } => "io",
            Self::InvalidConfig(_) => "invalid_config",
            Self::InvalidName => "invalid_name",
            Self::UnknownId => "unknown_id",
            Self::LimitReached => "limit_reached",
            Self::Launch { .. } => "launch_failed",
            Self::NoOpener => "no_opener",
            Self::NeedsApproval => "needs_approval",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(Error::NeedsApproval.code(), "needs_approval");
        assert_eq!(Error::UnknownId.code(), "unknown_id");
    }
}
