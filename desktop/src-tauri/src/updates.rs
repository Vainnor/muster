//! The update check (`docs/RELEASING.md` § Updater).
//!
//! Compiled in only with the `updater` feature, which release builds turn on; other builds
//! answer that they cannot update. Nothing touches the network until the user presses "Check
//! for updates", or has turned on the check at startup. A check is one HTTPS request for
//! `latest.json` on the GitHub release. Installing downloads the file it names, verifies it
//! against the public key built into Muster, and requires the signature to name the version the
//! manifest announced, so an altered manifest cannot pass off an older release.
//!
//! Muster never asks for administrator rights (`.claude/rules/security.md`), and the updater
//! would on some installs: `.deb` and `.rpm` packages (`pkexec` or `sudo`), the Windows MSI
//! (per-machine), and a macOS app in a folder the user cannot write to. Those get the release
//! page instead.

#![cfg_attr(
    not(feature = "updater"),
    allow(
        dead_code,
        reason = "the update logic is tested in every build, used with the feature"
    )
)]

use serde::Serialize;

/// Where to download by hand.
pub(crate) const RELEASES: &str = "https://github.com/Vainnor/muster/releases/latest";

/// Release notes longer than this are cut; the release page has them in full.
const MAX_NOTES: usize = 4000;

/// A newer version, as the UI shows it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct UpdateView {
    version: String,
    notes: String,
    install: Install,
}

/// How this copy of Muster can be updated.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Install {
    /// Muster downloads, verifies and installs it, then restarts.
    InApp,
    /// A `.deb` or `.rpm`: the package manager installs it.
    PackageManager,
    /// From the release page.
    Download,
}

/// How this copy was installed, as far as the updater is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Packaging {
    MacApp { writable: bool },
    AppImage,
    LinuxPackage,
    Nsis,
    Msi,
    Unknown,
}

/// Only the installs the updater replaces without elevation update in the app: an `AppImage`, the
/// per-user NSIS install, and a macOS app whose folder the user can write to.
fn install_route(packaging: Packaging) -> Install {
    match packaging {
        Packaging::MacApp { writable: true } | Packaging::AppImage | Packaging::Nsis => {
            Install::InApp
        }
        Packaging::LinuxPackage => Install::PackageManager,
        Packaging::MacApp { writable: false } | Packaging::Msi | Packaging::Unknown => {
            Install::Download
        }
    }
}

/// Release notes as plain text: no control characters but line breaks, and at most
/// [`MAX_NOTES`] characters.
fn tidy_notes(raw: &str) -> String {
    let mut notes: String = raw
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .take(MAX_NOTES)
        .collect();
    if raw.chars().count() > MAX_NOTES {
        notes.push('…');
    }
    notes
}

/// The `.app` holding `exe` (`X.app/Contents/MacOS/x`), or `None` outside a bundle.
#[cfg_attr(
    not(target_os = "macos"),
    allow(
        dead_code,
        reason = "macOS only; built everywhere so every CI runner tests it"
    )
)]
fn app_bundle(exe: &std::path::Path) -> Option<&std::path::Path> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let is_bundle = macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app";
    is_bundle.then_some(bundle)
}

#[cfg(feature = "updater")]
pub(crate) use imp::{check, check_at_start, install};

#[cfg(not(feature = "updater"))]
#[allow(
    clippy::unused_async,
    reason = "the same signature as the updater build's"
)]
pub(crate) async fn check<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
) -> Result<Option<UpdateView>, crate::commands::CommandError> {
    Err(crate::commands::CommandError::UPDATES_NOT_BUILT)
}

#[cfg(not(feature = "updater"))]
#[allow(
    clippy::unused_async,
    reason = "the same signature as the updater build's"
)]
pub(crate) async fn install<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
) -> Result<(), crate::commands::CommandError> {
    Err(crate::commands::CommandError::UPDATES_NOT_BUILT)
}

/// Whether this build can check for updates at all.
pub(crate) const BUILT_IN: bool = cfg!(feature = "updater");

#[cfg(feature = "updater")]
mod imp {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use tauri::{AppHandle, Runtime};
    use tauri_plugin_updater::{Update, UpdaterExt};

    use super::{Install, Packaging, UpdateView, install_route, tidy_notes};
    use crate::commands::CommandError;

    // The endpoint, the public key and `requireSignedVersion` are `plugins.updater` in
    // `tauri.conf.json`, where the plugin reads them.
    const TIMEOUT: Duration = Duration::from_secs(30);

    /// The last update a check found. Install uses this one, so the webview cannot name a
    /// download of its own.
    static FOUND: Mutex<Option<Update>> = Mutex::new(None);
    /// One check or install at a time.
    static BUSY: AtomicBool = AtomicBool::new(false);

    struct Busy;

    impl Drop for Busy {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::Release);
        }
    }

    fn busy() -> Result<Busy, CommandError> {
        if BUSY.swap(true, Ordering::AcqRel) {
            return Err(CommandError::UPDATE_BUSY);
        }
        Ok(Busy)
    }

    pub(crate) async fn check<R: Runtime>(
        app: &AppHandle<R>,
    ) -> Result<Option<UpdateView>, CommandError> {
        let _busy = busy()?;
        let found = app
            .updater_builder()
            .timeout(TIMEOUT)
            .build()
            .map_err(|_| CommandError::INTERNAL)?
            .check()
            .await
            .map_err(|e| {
                crate::diag::report("checking for updates", &e);
                CommandError::UPDATE_CHECK_FAILED
            })?;
        let view = found.as_ref().map(|update| UpdateView {
            version: update
                .version
                .chars()
                .filter(|c| !c.is_control())
                .take(32)
                .collect(),
            notes: tidy_notes(update.body.as_deref().unwrap_or_default()),
            install: install_route(packaging()),
        });
        *FOUND.lock().map_err(|_| CommandError::INTERNAL)? = found;
        Ok(view)
    }

    /// Installs the update the last check found, after the user agrees in a native dialog, and
    /// restarts. On Windows the installer ends Muster itself.
    pub(crate) async fn install<R: Runtime>(app: &AppHandle<R>) -> Result<(), CommandError> {
        let _busy = busy()?;
        if install_route(packaging()) != Install::InApp {
            return Err(CommandError::INSTALL_ELSEWHERE);
        }
        let update = FOUND
            .lock()
            .map_err(|_| CommandError::INTERNAL)?
            .clone()
            .ok_or(CommandError::NO_UPDATE_FOUND)?;
        if !crate::approval::confirm_update(app, &update.version).await? {
            return Err(CommandError::CANCELLED);
        }
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| {
                crate::diag::report("installing the update", &e);
                CommandError::UPDATE_INSTALL_FAILED
            })?;
        app.restart()
    }

    /// The check the user turned on in Settings, once at startup. A newer version is offered
    /// in a native dialog, since the window may not be open.
    pub(crate) fn check_at_start<R: Runtime>(app: &AppHandle<R>) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let Ok(Some(found)) = check(&app).await else {
                return;
            };
            if found.install == Install::InApp {
                if let Err(e) = install(&app).await
                    && e.code != CommandError::CANCELLED.code
                {
                    crate::approval::tell(&app, "Muster could not update", e.message.to_owned());
                }
            } else {
                crate::approval::tell(
                    &app,
                    "A new version of Muster is out",
                    format!(
                        "Muster {} is available. {}",
                        found.version,
                        if found.install == Install::PackageManager {
                            "Update it with your package manager."
                        } else {
                            "Download it from github.com/Vainnor/muster/releases."
                        }
                    ),
                );
            }
        });
    }

    fn packaging() -> Packaging {
        use tauri::utils::config::BundleType;
        match tauri::utils::platform::bundle_type() {
            Some(BundleType::AppImage) => Packaging::AppImage,
            Some(BundleType::Deb | BundleType::Rpm) => Packaging::LinuxPackage,
            Some(BundleType::Nsis) => Packaging::Nsis,
            Some(BundleType::Msi) => Packaging::Msi,
            Some(BundleType::App) => Packaging::MacApp {
                writable: mac_app_writable(),
            },
            _ => Packaging::Unknown,
        }
    }

    /// Whether the user can replace the `.app` Muster runs from: the updater swaps the bundle
    /// inside its folder, and asks for an administrator password when it cannot. Probed by
    /// creating and removing a file in both, since permission bits do not tell (ACLs, a
    /// read-only volume, App Translocation).
    fn mac_app_writable() -> bool {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let Some(bundle) = super::app_bundle(&exe) else {
            return false;
        };
        let Some(folder) = bundle.parent() else {
            return false;
        };
        [folder, bundle].iter().all(|dir| {
            let probe = dir.join(format!(".muster-update-probe-{}", std::process::id()));
            let created = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&probe)
                .is_ok();
            if created && let Err(e) = std::fs::remove_file(&probe) {
                crate::diag::report("removing the update probe file", &e);
            }
            created
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn only_installs_that_need_no_admin_update_in_the_app() {
        assert_eq!(install_route(Packaging::AppImage), Install::InApp);
        assert_eq!(install_route(Packaging::Nsis), Install::InApp);
        assert_eq!(
            install_route(Packaging::MacApp { writable: true }),
            Install::InApp
        );
        assert_eq!(
            install_route(Packaging::MacApp { writable: false }),
            Install::Download
        );
        assert_eq!(install_route(Packaging::Msi), Install::Download);
        assert_eq!(install_route(Packaging::Unknown), Install::Download);
        assert_eq!(
            install_route(Packaging::LinuxPackage),
            Install::PackageManager
        );
    }

    #[test]
    fn notes_lose_control_characters_and_stay_short() {
        assert_eq!(tidy_notes("Fixes\n- one\u{1b}[31m\r"), "Fixes\n- one[31m");
        let long = "a".repeat(MAX_NOTES + 10);
        let tidy = tidy_notes(&long);
        assert_eq!(tidy.chars().count(), MAX_NOTES + 1);
        assert!(tidy.ends_with('…'));
    }

    #[test]
    fn finds_the_bundle_around_the_executable() {
        assert_eq!(
            app_bundle(Path::new("/Applications/Muster.app/Contents/MacOS/muster")),
            Some(Path::new("/Applications/Muster.app"))
        );
        assert_eq!(
            app_bundle(Path::new("/opt/muster/target/debug/muster")),
            None
        );
    }
}
