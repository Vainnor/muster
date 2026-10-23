// The only module that talks to the Tauri shell. One typed function per command.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { AppInfo, AppView, ClosedEntry, CommandError, CssVar, GroupDraft, GroupView, LaunchTarget, Notice, Picked, PickKind, Settings, StepFailure, ThemeInfo, ThemePreview, UpdateView } from "./types";

export const appInfo = () => invoke<AppInfo>("app_info");
export const listThemes = () => invoke<ThemeInfo[]>("list_themes");
export const themeTokens = (id: string) => invoke<CssVar[]>("theme_tokens", { id });
export const previewTheme = (json: string) => invoke<ThemePreview>("preview_theme", { json });
export const importTheme = (json: string) => invoke<ThemePreview>("import_theme", { json });
export const exportTheme = (id: string) => invoke<string>("export_theme", { id });
export const deleteTheme = (id: string) => invoke<void>("delete_theme", { id });

export const listApps = () => invoke<AppView[]>("list_apps");
export const refreshApps = () => invoke<AppView[]>("refresh_apps");
export const launchApp = (id: string) => invoke<void>("launch_app", { id });
export const addApp = (name: string, target: LaunchTarget) => invoke<string>("add_app", { name, target });
export const pickPath = (kind: PickKind) => invoke<Picked | null>("pick_path", { kind });
export const removeApp = (id: string) => invoke<void>("remove_app", { id });

export const listGroups = () => invoke<GroupView[]>("list_groups");
export const saveGroup = (group: GroupDraft) => invoke<string>("save_group", { group });
export const deleteGroup = (id: string) => invoke<void>("delete_group", { id });
export const runGroup = (id: string) => invoke<StepFailure[]>("run_group", { id });
/** Resolves once the apps quit, or after the force-quit question Rust asks when some do not. */
export const closeGroup = (id: string) => invoke<ClosedEntry[]>("close_group", { id });

export const getSettings = () => invoke<Settings>("get_settings");
/** Asks GitHub for the latest release; null when Muster is up to date. */
export const checkForUpdate = () => invoke<UpdateView | null>("check_for_update");
/** Rust asks the user in a native dialog first, then installs and restarts. */
export const installUpdate = () => invoke<void>("install_update");
export const openReleasePage = () => invoke<void>("open_release_page");
export const setCheckUpdatesOnStart = (on: boolean) => invoke<void>("set_check_updates_on_start", { on });
export const setTheme = (id: string) => invoke<void>("set_theme", { id });
/** Returns whether the OS now starts Muster at login. */
export const setStartOnLogin = (enabled: boolean) => invoke<boolean>("set_start_on_login", { enabled });
/** Returns the hotkey as saved (canonical form), or null when removed. */
export const setGroupHotkey = (id: string, hotkey: string | null) => invoke<string | null>("set_group_hotkey", { id, hotkey });
/** Returns the hotkey as saved (canonical form), or null when turned off. */
export const setPopupHotkey = (hotkey: string | null) => invoke<string | null>("set_popup_hotkey", { hotkey });
export const startupNotice = () => invoke<Notice | null>("startup_notice");
export const closeWindow = () => invoke<void>("close_window");
/** Popup only: shows the (hidden) popup once it has painted. */
export const popupReady = () => invoke<void>("popup_ready");

/** Failures from a group run that happen after `runGroup` returned. Returns an unsubscribe. */
export function onLaunchFailed(handler: (failure: StepFailure) => void): Promise<() => void> {
  return listen<StepFailure>("launch-failed", (event) => handler(event.payload));
}

/** Commands reject with a CommandError; anything else becomes a generic one. */
export function asCommandError(e: unknown): CommandError {
  if (typeof e === "object" && e !== null && "code" in e && "message" in e) {
    const { code, message } = e as Record<string, unknown>;
    if (typeof code === "string" && typeof message === "string") return { code, message };
  }
  return { code: "internal", message: "Something went wrong inside Muster." };
}
