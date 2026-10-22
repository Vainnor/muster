//! The library held for the session, and how it is saved.
//!
//! Every change is made on a copy, written to disk atomically, and only then swapped in, so the
//! in-memory state never runs ahead of the file. If the config could not be read at all (not
//! merely invalid), Muster runs on defaults and refuses to save rather than overwrite a file it
//! could not read.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use muster_core::library::Library;
use muster_core::store::{self, Loaded};

use crate::commands::CommandError;

/// Shown once in the UI after startup.
#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Notice {
    /// The config file was invalid; it was renamed to `config.json.corrupt-N`.
    ConfigRecovered,
    /// The config file could not be read; changes this session are not saved.
    ConfigUnreadable,
}

pub(crate) struct AppState {
    dir: PathBuf,
    writable: bool,
    library: Mutex<Library>,
    notice: Mutex<Option<Notice>>,
}

impl AppState {
    pub(crate) fn load(dir: PathBuf) -> Self {
        let (library, writable, notice) = match store::load(&dir) {
            Ok(Loaded::Ok(config) | Loaded::Fresh(config)) => (Library::new(config), true, None),
            Ok(Loaded::Recovered { config, .. }) => {
                (Library::new(config), true, Some(Notice::ConfigRecovered))
            }
            Err(_) => (Library::default(), false, Some(Notice::ConfigUnreadable)),
        };
        Self {
            dir,
            writable,
            library: Mutex::new(library),
            notice: Mutex::new(notice),
        }
    }

    pub(crate) fn library(&self) -> Result<MutexGuard<'_, Library>, CommandError> {
        self.library.lock().map_err(|_| CommandError::INTERNAL)
    }

    /// Applies `change` to a copy, saves it, then keeps it.
    pub(crate) fn update<T>(
        &self,
        change: impl FnOnce(&mut Library) -> muster_core::Result<T>,
    ) -> Result<T, CommandError> {
        let mut library = self.library()?;
        let mut next = library.clone();
        let out = change(&mut next).map_err(CommandError::from)?;
        if !self.writable {
            return Err(CommandError::READ_ONLY);
        }
        store::save(&self.dir, next.config()).map_err(CommandError::from)?;
        *library = next;
        Ok(out)
    }

    /// The startup notice, once.
    pub(crate) fn take_notice(&self) -> Result<Option<Notice>, CommandError> {
        Ok(self
            .notice
            .lock()
            .map_err(|_| CommandError::INTERNAL)?
            .take())
    }
}
