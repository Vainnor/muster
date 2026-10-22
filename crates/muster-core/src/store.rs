//! The config file: versioned, written atomically, validated strictly on load.
//!
//! The shell passes the directory (the OS app-config dir from Tauri's path API), so this
//! module never guesses locations and tests run against a temp dir.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::groups::Group;
use crate::model::AppEntry;
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Preset name or custom theme id; `system` follows the OS light/dark setting.
    pub theme: String,
    /// The manual update check is always available; this enables checking at startup.
    pub check_updates_on_start: bool,
    pub local_logging: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            check_updates_on_start: false,
            local_logging: true,
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
