// The only module that talks to the Tauri shell. One typed function per command.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  AppInfo,
  AppView,
  CommandError,
  CssVar,
  GroupDraft,
  GroupView,
  LaunchTarget,
  Notice,
  Settings,
  StepFailure,
  ThemeInfo,
  ThemePreview,
} from "./types";

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
export const removeApp = (id: string) => invoke<void>("remove_app", { id });

export const listGroups = () => invoke<GroupView[]>("list_groups");
export const saveGroup = (group: GroupDraft) => invoke<string>("save_group", { group });
export const deleteGroup = (id: string) => invoke<void>("delete_group", { id });
export const runGroup = (id: string) => invoke<StepFailure[]>("run_group", { id });

export const getSettings = () => invoke<Settings>("get_settings");
export const setTheme = (id: string) => invoke<void>("set_theme", { id });
export const startupNotice = () => invoke<Notice | null>("startup_notice");
export const closeWindow = () => invoke<void>("close_window");

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
