//! Picks the download button's label from request headers, on the server. The site sends no
//! JavaScript, so there is no client-side detection.

/// Desktop platforms Muster builds for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Platform {
    Windows,
    MacOs,
    Linux,
}

impl Platform {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Windows => "Windows",
            Self::MacOs => "macOS",
            Self::Linux => "Linux",
        }
    }
}

/// `sec_ch_ua_platform` is the `Sec-CH-UA-Platform` client hint (sent by Chromium browsers,
/// quoted, e.g. `"macOS"`); `user_agent` is the fallback. Phones and tablets get `None`:
/// there is no build for them, so the page shows the generic button.
pub(crate) fn detect(
    sec_ch_ua_platform: Option<&str>,
    user_agent: Option<&str>,
) -> Option<Platform> {
    if let Some(hint) = sec_ch_ua_platform {
        match hint.trim().trim_matches('"') {
            "Windows" => return Some(Platform::Windows),
            "macOS" => return Some(Platform::MacOs),
            "Linux" | "Chrome OS" => return Some(Platform::Linux),
            "Android" | "iOS" => return None,
            _ => {}
        }
    }
    let ua = user_agent?;
    if ua.contains("Android") || ua.contains("iPhone") || ua.contains("iPad") {
        None
    } else if ua.contains("Windows") {
        Some(Platform::Windows)
    } else if ua.contains("Macintosh") || ua.contains("Mac OS X") {
        Some(Platform::MacOs)
    } else if ua.contains("Linux") || ua.contains("X11") || ua.contains("CrOS") {
        Some(Platform::Linux)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX_LINUX: &str =
        "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";
    const SAFARI_MAC: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";
    const EDGE_WINDOWS: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0";
    const CHROME_ANDROID: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36";
    const SAFARI_IPAD: &str = "Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

    #[test]
    fn user_agent_fallback() {
        assert_eq!(detect(None, Some(FIREFOX_LINUX)), Some(Platform::Linux));
        assert_eq!(detect(None, Some(SAFARI_MAC)), Some(Platform::MacOs));
        assert_eq!(detect(None, Some(EDGE_WINDOWS)), Some(Platform::Windows));
    }

    #[test]
    fn mobile_gets_no_platform() {
        assert_eq!(
            detect(None, Some(CHROME_ANDROID)),
            None,
            "Android says Linux too"
        );
        assert_eq!(
            detect(None, Some(SAFARI_IPAD)),
            None,
            "iPad says Mac OS X too"
        );
        assert_eq!(detect(Some("\"Android\""), Some(CHROME_ANDROID)), None);
    }

    #[test]
    fn client_hint_wins_over_user_agent() {
        assert_eq!(
            detect(Some("\"macOS\""), Some(EDGE_WINDOWS)),
            Some(Platform::MacOs)
        );
        assert_eq!(detect(Some("\"Windows\""), None), Some(Platform::Windows));
    }

    #[test]
    fn unknown_inputs_fall_through() {
        assert_eq!(
            detect(Some("\"Fuchsia\""), Some(FIREFOX_LINUX)),
            Some(Platform::Linux)
        );
        assert_eq!(detect(None, Some("curl/8.9")), None);
        assert_eq!(detect(None, None), None);
    }
}
