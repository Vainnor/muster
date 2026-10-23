//! The config file: versioned, written atomically, validated strictly on load.
//!
//! The shell passes the directory (the OS app-config dir from Tauri's path API), so this
//! module never guesses locations and tests run against a temp dir.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::groups::Group;
use crate::launch::LaunchTarget;
use crate::model::{AppEntry, Source};
use crate::{Error, Result};

pub const CONFIG_FILE: &str = "config.json";
pub const CURRENT_VERSION: u32 = 1;
/// A config larger than this is refused before parsing.
pub const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub apps: Vec<AppEntry>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub settings: Settings,
    /// Imported themes. Re-validated on every load.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<crate::themes::Theme>,
    /// Exact targets the user approved in the shell's native dialog. A manual app or an
    /// overridden group entry runs only if its target is here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approved: Vec<LaunchTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Preset name or custom theme id; `system` follows the OS light/dark setting.
    pub theme: String,
    /// The manual update check is always available; this enables checking at startup.
    pub check_updates_on_start: bool,
    pub local_logging: bool,
    /// Opens the popup launcher from anywhere. `None` turns it off. Missing from configs
    /// written before the popup existed, which get the default.
    #[serde(default = "default_popup_hotkey")]
    pub popup_hotkey: Option<crate::hotkey::Hotkey>,
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "serde's default must return the field's type"
)]
fn default_popup_hotkey() -> Option<crate::hotkey::Hotkey> {
    Some(crate::hotkey::Hotkey::default_popup())
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            check_updates_on_start: false,
            local_logging: true,
            popup_hotkey: default_popup_hotkey(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            apps: Vec::new(),
            groups: Vec::new(),
            settings: Settings::default(),
            themes: Vec::new(),
            approved: Vec::new(),
        }
    }
}

impl Config {
    fn validate(&self) -> Result<()> {
        if self.version != CURRENT_VERSION {
            return Err(Error::InvalidConfig(format!(
                "unsupported version {}",
                self.version
            )));
        }
        self.validate_items()?;
        for group in &self.groups {
            group.validate(&self.apps)?;
        }
        let mut ids = std::collections::HashSet::new();
        for theme in &self.themes {
            if crate::themes::is_reserved_id(&theme.id) {
                return Err(crate::themes::ThemeError::ReservedId.into());
            }
            if !ids.insert(theme.id.as_str()) {
                return Err(Error::InvalidConfig(format!(
                    "duplicate theme id {}",
                    theme.id
                )));
            }
            crate::themes::revalidate(theme.clone())?;
        }
        if self.themes.len() > crate::library::MAX_THEMES {
            return Err(Error::InvalidConfig("too many themes".into()));
        }
        Ok(())
    }
}

impl Config {
    /// The rules the library applies when it adds items, applied again to a file from disk:
    /// unique ids with the prefix their source implies, clean names, and the item limits.
    fn validate_items(&self) -> Result<()> {
        use crate::library::{MAX_APP_NAME_LEN, MAX_APPROVED, MAX_GROUPS, MAX_MANUAL_APPS};
        let bad = |what: &str| Err(Error::InvalidConfig(what.to_owned()));
        let mut ids = std::collections::HashSet::new();
        let mut manual = 0;
        for app in &self.apps {
            let prefix = match app.source {
                Source::Manual => {
                    manual += 1;
                    "m:"
                }
                Source::Detected => "d:",
            };
            if !app.id.0.starts_with(prefix) || !ids.insert(app.id.0.as_str()) {
                return bad("app ids must be unique and match their source");
            }
            if !clean_name(&app.name, MAX_APP_NAME_LEN) {
                return bad("app name");
            }
        }
        if manual > MAX_MANUAL_APPS {
            return bad("too many apps");
        }
        let mut group_ids = std::collections::HashSet::new();
        for group in &self.groups {
            if !group.id.0.starts_with("g:") || !group_ids.insert(group.id.0.as_str()) {
                return bad("group ids must be unique");
            }
        }
        if self.groups.len() > MAX_GROUPS {
            return bad("too many groups");
        }
        if self.approved.len() > MAX_APPROVED {
            return bad("too many approvals");
        }
        Ok(())
    }
}

fn clean_name(name: &str, max: usize) -> bool {
    let len = name.trim().chars().count();
    len > 0 && len <= max && !name.chars().any(char::is_control)
}

/// What [`load`] found.
#[derive(Debug)]
pub enum Loaded {
    /// The file was valid.
    Ok(Config),
    /// No file yet: defaults.
    Fresh(Config),
    /// The file was invalid. It was moved to `backup` and defaults are in use; tell the user.
    Recovered { config: Config, backup: PathBuf },
}

impl Loaded {
    #[must_use]
    pub fn config(&self) -> &Config {
        match self {
            Self::Ok(c) | Self::Fresh(c) | Self::Recovered { config: c, .. } => c,
        }
    }
}

/// Loads `dir/config.json`. A corrupt or invalid file never stops the app: it is backed up
/// and replaced by defaults.
pub fn load(dir: &Path) -> Result<Loaded> {
    let path = dir.join(CONFIG_FILE);
    let meta = match fs::metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Loaded::Fresh(Config::default()));
        }
        Err(source) => return Err(Error::Io { path, source }),
    };
    if meta.len() <= MAX_CONFIG_BYTES {
        let bytes = fs::read(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        if let Ok(config) = parse(&bytes) {
            return Ok(Loaded::Ok(config));
        }
    }
    let backup = backup_path(dir);
    fs::rename(&path, &backup).map_err(|source| Error::Io { path, source })?;
    Ok(Loaded::Recovered {
        config: Config::default(),
        backup,
    })
}

/// Parses and validates config bytes.
pub fn parse(bytes: &[u8]) -> Result<Config> {
    let config: Config =
        serde_json::from_slice(bytes).map_err(|e| Error::InvalidConfig(e.to_string()))?;
    config.validate()?;
    Ok(config)
}

/// Writes `dir/config.json` atomically: a temp file in the same directory, flushed to disk,
/// then renamed over the old file. A crash leaves either the old or the new file, never half.
pub fn save(dir: &Path, config: &Config) -> Result<()> {
    config.validate()?;
    let io = |source| Error::Io {
        path: dir.to_path_buf(),
        source,
    };
    fs::create_dir_all(dir).map_err(io)?;
    let json =
        serde_json::to_vec_pretty(config).map_err(|e| Error::InvalidConfig(e.to_string()))?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(io)?;
    tmp.write_all(&json).map_err(io)?;
    tmp.as_file().sync_all().map_err(io)?;
    tmp.persist(dir.join(CONFIG_FILE))
        .map_err(|e| io(e.error))?;
    Ok(())
}

fn backup_path(dir: &Path) -> PathBuf {
    let mut n = 0u32;
    loop {
        let candidate = dir.join(format!("{CONFIG_FILE}.corrupt-{n}"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_loads_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(load(dir.path()).unwrap(), Loaded::Fresh(c) if c == Config::default()));
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.settings.theme = "nord".into();
        save(dir.path(), &config).unwrap();
        assert!(matches!(load(dir.path()).unwrap(), Loaded::Ok(c) if c == config));
    }

    #[test]
    fn save_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &Config::default()).unwrap();
        save(dir.path(), &Config::default()).unwrap();
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from(CONFIG_FILE)]);
    }

    #[test]
    fn corrupt_file_is_backed_up_and_replaced_by_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(CONFIG_FILE), b"{ not json").unwrap();
        let Loaded::Recovered { config, backup } = load(dir.path()).unwrap() else {
            panic!("expected recovery");
        };
        assert_eq!(config, Config::default());
        assert_eq!(fs::read(&backup).unwrap(), b"{ not json");
        assert!(!dir.path().join(CONFIG_FILE).exists());
    }

    #[test]
    fn second_corruption_gets_its_own_backup() {
        let dir = tempfile::tempdir().unwrap();
        for _ in 0..2 {
            fs::write(dir.path().join(CONFIG_FILE), b"[]").unwrap();
            load(dir.path()).unwrap();
        }
        assert!(dir.path().join("config.json.corrupt-0").exists());
        assert!(dir.path().join("config.json.corrupt-1").exists());
    }

    #[test]
    fn rejects_unknown_fields_wrong_version_and_oversize() {
        assert!(parse(br#"{"version":1,"evil":true}"#).is_err());
        assert!(parse(br#"{"version":99}"#).is_err());
        let dir = tempfile::tempdir().unwrap();
        let big = vec![b' '; usize::try_from(MAX_CONFIG_BYTES).unwrap() + 1];
        fs::write(dir.path().join(CONFIG_FILE), big).unwrap();
        assert!(matches!(
            load(dir.path()).unwrap(),
            Loaded::Recovered { .. }
        ));
    }

    fn settings_json(extra: &str) -> Vec<u8> {
        format!(
            r#"{{"version":{CURRENT_VERSION},"settings":{{"theme":"system",
            "check_updates_on_start":false,"local_logging":true{extra}}}}}"#
        )
        .into_bytes()
    }

    #[test]
    fn a_config_from_before_the_popup_gets_the_default_hotkey() {
        let config = parse(&settings_json("")).unwrap();
        assert_eq!(
            config.settings.popup_hotkey,
            Some(crate::hotkey::Hotkey::default_popup())
        );
    }

    #[test]
    fn a_null_popup_hotkey_means_off() {
        let config = parse(&settings_json(r#","popup_hotkey":null"#)).unwrap();
        assert_eq!(config.settings.popup_hotkey, None);
    }

    #[test]
    fn rejects_an_invalid_popup_hotkey() {
        assert!(parse(&settings_json(r#","popup_hotkey":"Ctrl+Escape""#)).is_err());
        let ok = parse(&settings_json(r#","popup_hotkey":"ctrl+k""#)).unwrap();
        assert_eq!(ok.settings.popup_hotkey.unwrap().as_str(), "Ctrl+K");
    }

    fn with(edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
        let mut json = serde_json::json!({
            "version": CURRENT_VERSION,
            "apps": [{"id": "m:a", "name": "A", "source": "manual",
                      "target": {"kind": "url", "url": "https://a.example"}}],
            "groups": [{"id": "g:g", "name": "G", "entries": [{"app": "m:a"}]}],
        });
        edit(&mut json);
        serde_json::to_vec(&json).unwrap()
    }

    #[test]
    fn accepts_a_well_formed_file() {
        assert!(parse(&with(|_| {})).is_ok());
    }

    #[test]
    fn rejects_items_the_library_would_never_write() {
        type Edit = Box<dyn FnOnce(&mut serde_json::Value)>;
        let cases: [(&str, Edit); 8] = [
            (
                "duplicate app id",
                Box::new(|j| {
                    let a = j["apps"][0].clone();
                    j["apps"].as_array_mut().unwrap().push(a);
                }),
            ),
            (
                "manual app with a detected id",
                Box::new(|j| j["apps"][0]["id"] = "d:a".into()),
            ),
            (
                "control character in a name",
                Box::new(|j| j["apps"][0]["name"] = "A\u{7}".into()),
            ),
            (
                "group id without its prefix",
                Box::new(|j| j["groups"][0]["id"] = "g".into()),
            ),
            (
                "duplicate group id",
                Box::new(|j| {
                    let g = j["groups"][0].clone();
                    j["groups"].as_array_mut().unwrap().push(g);
                }),
            ),
            (
                "unknown app field",
                Box::new(|j| j["apps"][0]["extra"] = true.into()),
            ),
            (
                "unknown group field",
                Box::new(|j| j["groups"][0]["confirmed"] = true.into()),
            ),
            (
                "unknown target field",
                Box::new(|j| j["apps"][0]["target"]["args"] = "x".into()),
            ),
        ];
        for (what, edit) in cases {
            assert!(parse(&with(edit)).is_err(), "{what}");
        }
    }

    #[test]
    fn save_refuses_an_invalid_config() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            version: 2,
            ..Config::default()
        };
        assert!(save(dir.path(), &config).is_err());
        assert!(!dir.path().join(CONFIG_FILE).exists());
    }
}
