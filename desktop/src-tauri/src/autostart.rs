//! Start on login. Off until the user turns it on in Settings. `muster_core::login` builds the
//! entry with each OS's quoting; this module says where it goes and which program it starts.
//! The entry starts Muster with `--autostart`, which keeps the main window closed.
//!
//! Whether it is on is read from the OS each time, not from the config file, so an entry the
//! user deletes is shown as off. Disabling it in the OS instead (Task Manager's Startup tab,
//! macOS "Allow in the Background") keeps the entry, so Settings still shows it on.

use anyhow::Context as _;
use muster_core::login::{self, Program};
use tauri::{AppHandle, Runtime};

pub(crate) use muster_core::login::FLAG;

/// Whether Muster starts on login.
pub(crate) fn is_enabled<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<bool> {
    os::is_enabled(app)
}

/// Turns start on login on or off.
pub(crate) fn set<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> anyhow::Result<()> {
    if enabled {
        os::enable(app, &program(app)?)
    } else {
        os::disable(app)
    }
}

/// The program the entry starts: this executable, or on Linux the `AppImage` it runs from.
#[cfg_attr(
    not(target_os = "linux"),
    allow(unused_variables, reason = "used on Linux only")
)]
fn program<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<Program> {
    let exe = std::env::current_exe().context("finding Muster's executable")?;
    #[cfg(target_os = "linux")]
    let (appimage, appdir) = (
        tauri::Manager::env(app).appimage,
        std::env::var_os("APPDIR"),
    );
    #[cfg(not(target_os = "linux"))]
    let (appimage, appdir): (Option<std::ffi::OsString>, Option<std::ffi::OsString>) = (None, None);
    login::entry_program(
        &exe,
        appimage.as_deref().map(std::path::Path::new),
        appdir.as_deref().map(std::path::Path::new),
    )
    .context("choosing the program to start")
}

/// `~/Library/LaunchAgents/<identifier>.plist`.
#[cfg(target_os = "macos")]
mod os {
    use super::{AppHandle, Program, Runtime, login};
    use anyhow::Context as _;
    use std::path::PathBuf;
    use tauri::Manager as _;

    fn entry<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<(PathBuf, String)> {
        let label = app.config().identifier.clone();
        let home = app.path().home_dir().context("finding the home folder")?;
        Ok((
            home.join("Library/LaunchAgents")
                .join(format!("{label}.plist")),
            label,
        ))
    }

    pub(super) fn enable<R: Runtime>(app: &AppHandle<R>, program: &Program) -> anyhow::Result<()> {
        let (path, label) = entry(app)?;
        let plist = login::launch_agent(program, &label).context("building the LaunchAgent")?;
        login::write_entry(&path, &plist).context("writing the LaunchAgent")
    }

    pub(super) fn disable<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
        login::remove_entry(&entry(app)?.0).context("removing the LaunchAgent")
    }

    pub(super) fn is_enabled<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<bool> {
        login::entry_exists(&entry(app)?.0).context("checking the LaunchAgent")
    }
}

/// `$XDG_CONFIG_HOME/autostart/<identifier>.desktop`.
#[cfg(target_os = "linux")]
mod os {
    use super::{AppHandle, Program, Runtime, login};
    use anyhow::Context as _;
    use std::path::PathBuf;
    use tauri::Manager as _;

    fn entry<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<PathBuf> {
        let id = &app.config().identifier;
        let config = app
            .path()
            .config_dir()
            .context("finding the config folder")?;
        Ok(config.join("autostart").join(format!("{id}.desktop")))
    }

    pub(super) fn enable<R: Runtime>(app: &AppHandle<R>, program: &Program) -> anyhow::Result<()> {
        let contents = login::desktop_entry(program);
        login::write_entry(&entry(app)?, contents.as_bytes()).context("writing the autostart entry")
    }

    pub(super) fn disable<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
        login::remove_entry(&entry(app)?).context("removing the autostart entry")
    }

    pub(super) fn is_enabled<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<bool> {
        login::entry_exists(&entry(app)?).context("checking the autostart entry")
    }
}

/// A value named `Muster` under `HKCU\...\Run`. Task Manager can also disable it through its
/// own `StartupApproved` key; Muster does not read or change that.
#[cfg(windows)]
mod os {
    use super::{AppHandle, Program, Runtime, login};
    use anyhow::Context as _;
    use windows_registry::CURRENT_USER;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    /// `HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND)`: the key or value does not exist.
    const NOT_FOUND: i32 = -2_147_024_894;

    pub(super) fn enable<R: Runtime>(_app: &AppHandle<R>, program: &Program) -> anyhow::Result<()> {
        let value = login::run_value(program).context("building the Run value")?;
        CURRENT_USER
            .create(RUN)
            .and_then(|key| key.set_string(login::NAME, value))
            .context("writing the Run value")
    }

    pub(super) fn disable<R: Runtime>(_app: &AppHandle<R>) -> anyhow::Result<()> {
        match CURRENT_USER
            .create(RUN)
            .and_then(|key| key.remove_value(login::NAME))
        {
            Err(e) if e.code().0 != NOT_FOUND => Err(e).context("removing the Run value"),
            _ => Ok(()),
        }
    }

    pub(super) fn is_enabled<R: Runtime>(_app: &AppHandle<R>) -> anyhow::Result<bool> {
        match CURRENT_USER
            .open(RUN)
            .and_then(|key| key.get_string(login::NAME))
        {
            Ok(_) => Ok(true),
            Err(e) if e.code().0 == NOT_FOUND => Ok(false),
            Err(e) => Err(e).context("reading the Run value"),
        }
    }
}
