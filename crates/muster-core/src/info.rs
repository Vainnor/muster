//! Facts about this build and the machine it runs on. Used by the about screen and the
//! vertical slice that proves the shell → core → UI path.

use serde::Serialize;

/// Name, version and platform of the running build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    /// `std::env::consts::OS`: `linux`, `macos`, `windows`, ...
    pub os: &'static str,
    /// `std::env::consts::ARCH`: `x86_64`, `aarch64`, ...
    pub arch: &'static str,
    /// The OS as shown to people: `Linux`, `macOS`, `Windows`.
    pub os_label: &'static str,
}

#[must_use]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Muster",
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        os_label: os_label(std::env::consts::OS),
    }
}

/// Human label for a `std::env::consts::OS` value.
#[must_use]
pub fn os_label(os: &str) -> &'static str {
    match os {
        "linux" => "Linux",
        "macos" => "macOS",
        "windows" => "Windows",
        "freebsd" | "openbsd" | "netbsd" | "dragonfly" => "BSD",
        _ => "Unknown OS",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_this_build() {
        let info = app_info();
        assert_eq!(info.name, "Muster");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.os, std::env::consts::OS);
        assert_eq!(info.arch, std::env::consts::ARCH);
        assert_ne!(info.os_label, "Unknown OS", "CI runs on a supported OS");
    }

    #[test]
    fn os_labels_cover_supported_platforms() {
        assert_eq!(os_label("linux"), "Linux");
        assert_eq!(os_label("macos"), "macOS");
        assert_eq!(os_label("windows"), "Windows");
        assert_eq!(os_label("haiku"), "Unknown OS");
    }

    #[test]
    fn app_info_serializes_with_stable_field_names() {
        let json = serde_json::to_value(app_info()).unwrap();
        for key in ["name", "version", "os", "arch", "os_label"] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
    }
}
