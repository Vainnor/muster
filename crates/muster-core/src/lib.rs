//! Muster's app logic. Everything the launcher decides lives here; the desktop shell and the
//! website are thin layers that call into this crate.
//!
//! This crate has no UI, Tauri, Topcoat or async-runtime dependency, so every rule can be
//! unit-tested on any OS.

#![forbid(unsafe_code)]

pub mod close;
pub mod detect;
pub mod error;
pub mod groups;
pub mod hotkey;
pub mod info;
pub mod launch;
pub mod library;
pub mod login;
pub mod model;
pub mod store;
pub mod themes;

pub use error::{Error, Result};
