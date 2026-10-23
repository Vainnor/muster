//! Commands the UI may call. Each one is declared in `build.rs` and granted by name in
//! `capabilities/main.json`. Arguments are untrusted and validated in core; these functions
//! only translate.
#![allow(
    clippy::needless_pass_by_value,
    reason = "Tauri passes State and deserialized arguments by value"
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use muster_core::groups::GroupId;
use muster_core::info::AppInfo;
use muster_core::launch::LaunchTarget;
use muster_core::library::{GroupDraft, Library};
use muster_core::model::{AppEntry, AppId, Source};
use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use crate::state::{AppState, Notice};
use crate::{approval, launcher};

/// Error sent to the webview: a stable code and a message safe to show. Never a path.
#[derive(Debug, Serialize, PartialEq, Eq, Clone, Copy)]
pub(crate) struct CommandError {
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
}

impl CommandError {
    pub(crate) const CANCELLED: Self = Self {
        code: "cancelled",
        message: "Nothing was started.",
    };
    pub(crate) const COOLING_DOWN: Self = Self {
        code: "busy",
        message: "You just said no. Wait a few seconds before asking Muster again.",
    };
    pub(crate) const HOTKEY_TAKEN: Self = Self {
        code: "hotkey_taken",
        message: "Muster could not claim that hotkey. Another app may already use it; pick another.",
    };
    pub(crate) const NO_GLOBAL_HOTKEYS: Self = Self {
        code: "no_global_hotkeys",
        message: "Global hotkeys do not work on Wayland. Bind `muster --popup` to a key in your compositor instead.",
    };
    pub(crate) const BUSY: Self = Self {
        code: "busy",
        message: "Answer the open Muster dialog first.",
    };
    pub(crate) const TOO_LONG_TO_CONFIRM: Self = Self {
        code: "too_long_to_confirm",
        message: "This runs a command line too long to show for approval. Shorten its arguments.",
    };
    pub(crate) const INTERNAL: Self = Self {
        code: "internal",
        message: "Something went wrong inside Muster.",
    };
    pub(crate) const READ_ONLY: Self = Self {
        code: "read_only",
        message: "Your settings file could not be read, so changes are not saved this session.",
    };
    const UNKNOWN_THEME: Self = Self {
        code: "unknown_theme",
        message: "There is no theme with that name.",
    };
}

impl From<muster_core::Error> for CommandError {
    fn from(e: muster_core::Error) -> Self {
        use muster_core::groups::GroupError;
        use muster_core::launch::TargetError;
        let message = match &e {
            muster_core::Error::InvalidTarget(t) => match t {
                TargetError::NotFound => "That file or app does not exist.",
                TargetError::NotAFile => "That path is a folder, not a program.",
                TargetError::NotABundle => "That path is not an .app bundle.",
                TargetError::RelativePath | TargetError::ParentTraversal => {
                    "Use a full path, for example /usr/bin/app or C:\\Apps\\app.exe."
                }
                TargetError::BadWorkingDir => "The working folder does not exist.",
                TargetError::TooManyArgs | TargetError::ArgTooLong | TargetError::NulByte => {
                    "The arguments are too long or contain invalid characters."
                }
                TargetError::BatchArgument => {
                    "Arguments to a .bat or .cmd file cannot contain % ! & | < > ^, quotes or line breaks."
                }
                TargetError::BatchPath => {
                    "The path to a .bat or .cmd file cannot contain % ! & | < > ^ or quotes."
                }
                TargetError::AmbiguousFileName => {
                    "The program's file name cannot end in a dot or space or contain a colon."
                }
                TargetError::ShadowedByExe => {
                    "A file with the same name plus .exe is next to it, and Windows would run that instead."
                }
                TargetError::BadUrl => "That is not a valid web or mail link.",
                TargetError::SchemeNotAllowed(_) => {
                    "Only http, https and mailto links can be added."
                }
            },
            muster_core::Error::InvalidGroup(g) => match g {
                GroupError::BadName => "Give the group a name of 1 to 64 characters.",
                GroupError::TooManyEntries => "A group holds at most 32 apps.",
                GroupError::DelayTooLong => "A delay can be at most 60 seconds.",
                GroupError::UnknownApp(_) => "The group uses an app that no longer exists.",
                GroupError::BadArguments => {
                    "The arguments are too long or contain invalid characters."
                }
                GroupError::BadWorkingDir => "The working folder must be a full path.",
                GroupError::NeedsConfirmation => "Muster needs your OK before it runs this.",
            },
            muster_core::Error::InvalidName => "Give it a name of 1 to 128 characters.",
            muster_core::Error::UnknownId => "That app or group no longer exists.",
            muster_core::Error::LimitReached => "You have reached the limit for this kind of item.",
            muster_core::Error::Launch { .. } => "The app could not be started.",
            muster_core::Error::NoOpener => "No program is installed to open links (xdg-open).",
            muster_core::Error::NeedsApproval => "Muster needs your OK before it runs this.",
            muster_core::Error::InvalidTheme(t) => {
                use muster_core::themes::ThemeError;
                match t {
                    ThemeError::TooLarge => "That theme file is larger than 16 KiB.",
                    ThemeError::Malformed(_) => {
                        "That is not a Muster theme file, or it has fields Muster does not know."
                    }
                    ThemeError::BadColor => "Colors must be written as #rrggbb or #rrggbbaa.",
                    ThemeError::BadId => {
                        "The theme id must use lowercase letters, digits and dashes."
                    }
                    ThemeError::BadName => "The theme needs a name of 1 to 40 characters.",
                    ThemeError::BadDescription => "The theme description is too long.",
                    ThemeError::BadWordmark => "The theme wordmark must be 1 to 16 characters.",
                    ThemeError::BadRadius => "The corner radius must be between 0 and 24.",
                    ThemeError::BadIconSize => "The icon size must be between 16 and 64.",
                    ThemeError::ReservedId => {
                        "That theme id belongs to a built-in theme; change its id."
                    }
                }
            }
            muster_core::Error::InvalidHotkey(h) => {
                use muster_core::hotkey::HotkeyError;
                match h {
                    HotkeyError::NeedsModifier => {
                        "Hold Ctrl, Alt, Shift or Super with the key (F1 to F12 can stand alone)."
                    }
                    HotkeyError::UnknownKey(_) => {
                        "Muster cannot use that key for a hotkey. Escape is kept for closing."
                    }
                    HotkeyError::UnknownModifier(_)
                    | HotkeyError::RepeatedModifier
                    | HotkeyError::BadLength => "That is not a hotkey Muster understands.",
                }
            }
            muster_core::Error::Io { .. } | muster_core::Error::InvalidConfig(_) => {
                "Your settings could not be saved."
            }
        };
        let code = match &e {
            muster_core::Error::InvalidGroup(GroupError::NeedsConfirmation) => "needs_approval",
            other => other.code(),
        };
        Self { code, message }
    }
}

/// One `--mu-*` custom property.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Token {
    name: String,
    value: String,
}

/// An app as the UI shows it. `detail` is the path or link, for the user's own eyes.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct AppView {
    id: String,
    name: String,
    source: Source,
    kind: &'static str,
    detail: String,
}

impl From<&AppEntry> for AppView {
    fn from(a: &AppEntry) -> Self {
        let (kind, detail) = match &a.target {
            LaunchTarget::Program { .. } => ("program", muster_core::launch::describe(&a.target)),
            LaunchTarget::AppBundle { path } => ("bundle", path.display().to_string()),
            LaunchTarget::Url { url } => ("url", url.clone()),
        };
        Self {
            id: a.id.0.clone(),
            name: a.name.clone(),
            source: a.source,
            kind,
            detail,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct GroupEntryView {
    app: String,
    app_name: Option<String>,
    args: Option<Vec<String>>,
    cwd: Option<PathBuf>,
    delay_ms: u32,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct GroupView {
    id: String,
    name: String,
    entries: Vec<GroupEntryView>,
    needs_confirmation: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Settings {
    theme: String,
    /// The saved popup hotkey, or `None` when it is off.
    popup_hotkey: Option<String>,
    /// Whether the OS accepted it. False with a saved hotkey means another app holds it.
    popup_hotkey_active: bool,
    /// Wayland sessions cannot register global hotkeys at all.
    wayland: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct StepFailure {
    app: String,
    message: &'static str,
}

fn app_views(library: &Library) -> Vec<AppView> {
    library.apps().iter().map(AppView::from).collect()
}

#[tauri::command]
pub(crate) fn app_info() -> AppInfo {
    muster_core::info::app_info()
}

/// Design tokens for a preset or imported theme.
#[tauri::command]
pub(crate) fn theme_tokens(
    state: State<'_, AppState>,
    id: &str,
) -> Result<Vec<Token>, CommandError> {
    let theme = state
        .library()?
        .theme(id)
        .ok_or(CommandError::UNKNOWN_THEME)?;
    Ok(tokens(&theme))
}

fn tokens(theme: &muster_core::themes::ValidTheme) -> Vec<Token> {
    muster_ui::css_variables(theme)
        .into_iter()
        .map(|v| Token {
            name: v.name,
            value: v.value,
        })
        .collect()
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct ThemeInfo {
    id: String,
    name: String,
    mode: muster_core::themes::Mode,
    custom: bool,
}

impl ThemeInfo {
    fn new(theme: &muster_core::themes::ValidTheme, custom: bool) -> Self {
        let t = theme.theme();
        Self {
            id: t.id.clone(),
            name: t.name.clone(),
            mode: t.mode,
            custom,
        }
    }
}

/// Presets in display order, then imported themes.
#[tauri::command]
pub(crate) fn list_themes(state: State<'_, AppState>) -> Result<Vec<ThemeInfo>, CommandError> {
    let presets = muster_core::themes::PRESETS
        .iter()
        .filter_map(|(id, _)| muster_core::themes::preset(id))
        .map(|t| ThemeInfo::new(&t, false));
    let custom: Vec<_> = state
        .library()?
        .custom_themes()
        .iter()
        .map(|t| ThemeInfo::new(t, true))
        .collect();
    Ok(presets.chain(custom).collect())
}

/// A contrast problem in words the UI can show.
#[derive(Debug, Serialize, PartialEq)]
pub(crate) struct ContrastWarning {
    foreground: &'static str,
    background: &'static str,
    ratio: f64,
    required: f64,
}

#[derive(Debug, Serialize, PartialEq)]
pub(crate) struct ThemePreview {
    theme: ThemeInfo,
    tokens: Vec<Token>,
    warnings: Vec<ContrastWarning>,
}

fn preview(parsed: &muster_core::themes::Parsed, custom: bool) -> ThemePreview {
    ThemePreview {
        theme: ThemeInfo::new(&parsed.theme, custom),
        tokens: tokens(&parsed.theme),
        warnings: parsed
            .warnings
            .iter()
            .map(|w| ContrastWarning {
                foreground: w.foreground,
                background: w.background,
                ratio: w.ratio,
                required: w.required,
            })
            .collect(),
    }
}

/// Validates a theme file's text without saving it, for the live preview.
#[tauri::command]
pub(crate) fn preview_theme(json: String) -> Result<ThemePreview, CommandError> {
    let parsed = muster_core::themes::parse(json.as_bytes()).map_err(muster_core::Error::from)?;
    if muster_core::themes::is_reserved_id(&parsed.theme.theme().id) {
        return Err(muster_core::Error::from(muster_core::themes::ThemeError::ReservedId).into());
    }
    Ok(preview(&parsed, true))
}

/// Saves an imported theme. Contrast warnings come back with it; they never block.
#[tauri::command]
pub(crate) fn import_theme(
    state: State<'_, AppState>,
    json: String,
) -> Result<ThemePreview, CommandError> {
    state
        .update(|lib| lib.import_theme(json.as_bytes()))
        .map(|parsed| preview(&parsed, true))
}

/// The theme file for a preset or imported theme.
#[tauri::command]
pub(crate) fn export_theme(state: State<'_, AppState>, id: &str) -> Result<String, CommandError> {
    let theme = state
        .library()?
        .theme(id)
        .ok_or(CommandError::UNKNOWN_THEME)?;
    let mut exported = theme.theme().clone();
    exported.schema = Some(muster_core::themes::SchemaRef);
    serde_json::to_string_pretty(&exported).map_err(|_| CommandError::INTERNAL)
}

#[tauri::command]
pub(crate) fn delete_theme(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    state.update(|lib| lib.remove_theme(&id))
}

/// All apps. Scans the first time it is called in a session.
#[tauri::command]
pub(crate) async fn list_apps(state: State<'_, AppState>) -> Result<Vec<AppView>, CommandError> {
    if !state.library()?.scanned() {
        return refresh_apps(state).await;
    }
    let library = state.library()?;
    Ok(app_views(&library))
}

/// One scan at a time: concurrent requests wait their turn, so the webview cannot start many
/// scans at once, each with its own plist-parsing thread.
static SCAN: tauri::async_runtime::Mutex<()> = tauri::async_runtime::Mutex::const_new(());
/// Counts finished scans, so a request that waited behind one uses its result.
static SCANS_DONE: AtomicU64 = AtomicU64::new(0);

/// Scans the disk again, off the main thread.
#[tauri::command]
pub(crate) async fn refresh_apps(state: State<'_, AppState>) -> Result<Vec<AppView>, CommandError> {
    let before = SCANS_DONE.load(Ordering::Acquire);
    let _one_at_a_time = SCAN.lock().await;
    if SCANS_DONE.load(Ordering::Acquire) != before {
        let library = state.library()?;
        return Ok(app_views(&library));
    }
    let scan = tauri::async_runtime::spawn_blocking(muster_core::detect::detect_installed)
        .await
        .map_err(|_| CommandError::INTERNAL)?;
    let mut library = state.library()?;
    library.set_detected(&scan);
    SCANS_DONE.fetch_add(1, Ordering::Release);
    Ok(app_views(&library))
}

/// Starts one app. A manual app the user has not approved yet asks first, in a native dialog.
#[tauri::command]
pub(crate) async fn launch_app<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), CommandError> {
    let id = AppId(id);
    let pending = state.library()?.pending_for_app(&id)?;
    approval::ensure(&app, &state, pending).await?;
    let plan = state.library()?.plan_app(&id)?;
    launcher::start(&plan)
}

#[tauri::command]
pub(crate) fn add_app(
    state: State<'_, AppState>,
    name: String,
    target: LaunchTarget,
) -> Result<String, CommandError> {
    state
        .update(|lib| lib.add_manual(&name, target))
        .map(|id| id.0)
}

#[tauri::command]
pub(crate) fn remove_app(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    state.update(|lib| lib.remove_manual(&AppId(id)))
}

#[tauri::command]
pub(crate) fn list_groups(state: State<'_, AppState>) -> Result<Vec<GroupView>, CommandError> {
    let library = state.library()?;
    Ok(library
        .config()
        .groups
        .iter()
        .map(|g| GroupView {
            id: g.id.0.clone(),
            name: g.name.clone(),
            entries: g
                .entries
                .iter()
                .map(|e| GroupEntryView {
                    app: e.app.0.clone(),
                    app_name: library.find(&e.app).map(|a| a.name),
                    args: e.args.clone(),
                    cwd: e.cwd.clone(),
                    delay_ms: e.delay_ms,
                })
                .collect(),
            needs_confirmation: library
                .pending_for_group(&g.id)
                .is_ok_and(|p| !p.is_empty()),
        })
        .collect())
}

#[tauri::command]
pub(crate) fn save_group(
    state: State<'_, AppState>,
    group: GroupDraft,
) -> Result<String, CommandError> {
    state.update(|lib| lib.save_group(group)).map(|id| id.0)
}

#[tauri::command]
pub(crate) fn delete_group(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    state.update(|lib| lib.delete_group(&GroupId(id)))
}

/// Starts a group. Targets that need approval and lack it are listed in a native dialog first.
/// Returns the entries that cannot start right now; the rest launch in order on a background
/// thread, and failures after a delay arrive as `launch-failed` events.
#[tauri::command]
pub(crate) async fn run_group<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<StepFailure>, CommandError> {
    let id = GroupId(id);
    let pending = state.library()?.pending_for_group(&id)?;
    approval::ensure(&app, &state, pending).await?;
    let steps = state.library()?.plan_run(&id)?;
    let failures = steps
        .iter()
        .filter_map(|s| match &s.plan {
            Err(e) => Some(StepFailure {
                app: s.app.0.clone(),
                message: CommandError::from(muster_core::Error::from(e.clone())).message,
            }),
            Ok(_) => None,
        })
        .collect();
    launcher::run_steps(app, steps)?;
    Ok(failures)
}

#[tauri::command]
pub(crate) fn get_settings(state: State<'_, AppState>) -> Result<Settings, CommandError> {
    let library = state.library()?;
    let settings = &library.config().settings;
    Ok(Settings {
        theme: settings.theme.clone(),
        popup_hotkey: settings.popup_hotkey.as_ref().map(ToString::to_string),
        popup_hotkey_active: crate::hotkeys::popup_active(settings.popup_hotkey.as_ref()),
        wayland: crate::hotkeys::session_is_wayland(),
    })
}

/// Sets the popup hotkey, or turns it off with `None`. The OS has to accept it before it is
/// saved; if saving fails, the previous hotkey is registered again.
#[tauri::command]
pub(crate) fn set_popup_hotkey<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    hotkey: Option<String>,
) -> Result<Option<String>, CommandError> {
    let hotkey = hotkey
        .as_deref()
        .map(muster_core::hotkey::Hotkey::parse)
        .transpose()
        .map_err(muster_core::Error::from)?;
    if hotkey.is_some() && crate::hotkeys::session_is_wayland() {
        return Err(CommandError::NO_GLOBAL_HOTKEYS);
    }
    let old = state.library()?.config().settings.popup_hotkey.clone();
    crate::hotkeys::set_popup(&app, hotkey.as_ref()).map_err(|e| {
        crate::diag::report("registering the popup hotkey", e.as_ref());
        CommandError::HOTKEY_TAKEN
    })?;
    let saved = hotkey.clone();
    if let Err(e) = state.update(|lib| {
        lib.config_mut().settings.popup_hotkey = saved;
        Ok(())
    }) {
        if let Err(restore) = crate::hotkeys::set_popup(&app, old.as_ref()) {
            crate::diag::report("restoring the popup hotkey", restore.as_ref());
        }
        return Err(e);
    }
    Ok(hotkey.map(|h| h.to_string()))
}

/// `system`, a preset id or an imported theme's id.
#[tauri::command]
pub(crate) fn set_theme(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    if id != "system" && state.library()?.theme(&id).is_none() {
        return Err(CommandError::UNKNOWN_THEME);
    }
    state.update(|lib| {
        lib.config_mut().settings.theme = id;
        Ok(())
    })
}

#[tauri::command]
pub(crate) fn startup_notice(state: State<'_, AppState>) -> Result<Option<Notice>, CommandError> {
    state.take_notice()
}

/// Closes (destroys) the window that asked: the main window or the popup. Muster stays in
/// the tray.
#[tauri::command]
pub(crate) fn close_window<R: Runtime>(
    window: tauri::WebviewWindow<R>,
) -> Result<(), CommandError> {
    window.destroy().map_err(|_| CommandError::INTERNAL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_comes_from_core() {
        assert_eq!(app_info(), muster_core::info::app_info());
    }

    #[test]
    fn tokens_for_a_preset() {
        let nord = muster_core::themes::preset("nord").unwrap();
        assert!(
            tokens(&nord)
                .iter()
                .any(|t| t.name == "--mu-bg" && t.value == "#2e3440")
        );
    }

    #[test]
    fn preview_refuses_built_in_ids_and_reports_contrast() {
        let dark = muster_core::themes::PRESETS
            .iter()
            .find(|(id, _)| *id == "dark")
            .unwrap()
            .1;
        assert_eq!(
            preview_theme(dark.into()).unwrap_err().code,
            "invalid_theme"
        );

        let mut theme: serde_json::Value = serde_json::from_str(dark).unwrap();
        theme["id"] = "mine".into();
        theme["colors"]["text"] = theme["colors"]["background"].clone();
        let p = preview_theme(theme.to_string()).unwrap();
        assert!(p.theme.custom);
        assert!(p.warnings.iter().any(|w| w.foreground == "text"));
    }

    #[test]
    fn errors_keep_core_codes_and_never_carry_paths() {
        let io = muster_core::Error::Io {
            path: "/home/someone/.config/muster".into(),
            source: std::io::Error::other("disk full"),
        };
        let e = CommandError::from(io);
        assert_eq!(e.code, "io");
        assert!(!e.message.contains("/home"));
        let confirm =
            muster_core::Error::InvalidGroup(muster_core::groups::GroupError::NeedsConfirmation);
        assert_eq!(CommandError::from(confirm).code, "needs_approval");
    }

    #[test]
    fn app_view_shows_program_and_arguments() {
        let entry = AppEntry {
            id: AppId("m:tool".into()),
            name: "Tool".into(),
            target: LaunchTarget::Program {
                path: "/opt/tool".into(),
                args: vec!["--fast".into()],
                cwd: None,
            },
            source: Source::Manual,
        };
        let view = AppView::from(&entry);
        assert_eq!(
            (view.kind, view.detail.as_str()),
            ("program", r#""/opt/tool" "--fast""#)
        );
    }
}
