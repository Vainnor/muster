// Hand-written mirrors of the Rust types the commands take and return. Change both sides
// together (desktop/src-tauri/src/commands.rs, muster-core).

/** muster_core::info::AppInfo */
export interface AppInfo {
  name: string;
  version: string;
  os: string;
  arch: string;
  os_label: string;
}

/** muster_ui::CssVar, as sent by the `theme_tokens` command. */
export interface CssVar {
  name: string;
  value: string;
}

/** The desktop shell's CommandError. */
export interface CommandError {
  code: string;
  message: string;
}

export type Source = "detected" | "manual";

/** commands::AppView */
export interface AppView {
  id: string;
  name: string;
  source: Source;
  kind: "program" | "bundle" | "url";
  detail: string;
}

/** muster_core::launch::LaunchTarget */
export type LaunchTarget =
  | { kind: "program"; path: string; args: string[]; cwd: string | null }
  | { kind: "app_bundle"; path: string }
  | { kind: "url"; url: string };

/** muster_core::groups::GroupEntry (as sent) and commands::GroupEntryView (as received). */
export interface GroupEntry {
  app: string;
  args: string[] | null;
  cwd: string | null;
  delay_ms: number;
}

export interface GroupEntryView extends GroupEntry {
  app_name: string | null;
}

/** commands::GroupView */
export interface GroupView {
  id: string;
  name: string;
  entries: GroupEntryView[];
  needs_confirmation: boolean;
}

/** muster_core::library::GroupDraft */
export interface GroupDraft {
  id: string | null;
  name: string;
  entries: GroupEntry[];
}

/** commands::StepFailure and the `launch-failed` event payload. */
export interface StepFailure {
  app: string;
  message: string;
}

export interface Settings {
  theme: string;
  /** The saved popup hotkey, or null when it is off. */
  popup_hotkey: string | null;
  /** False with a saved hotkey: the OS refused it, usually because another app holds it. */
  popup_hotkey_active: boolean;
  /** Wayland sessions cannot register global hotkeys. */
  wayland: boolean;
  /** Whether the OS will start Muster at login; null when that could not be read. */
  start_on_login: boolean | null;
}

/** state::Notice */
export type Notice = "config_recovered" | "config_unreadable";

/** commands::ThemeInfo */
export interface ThemeInfo {
  id: string;
  name: string;
  mode: "light" | "dark";
  custom: boolean;
}

/** commands::ContrastWarning */
export interface ContrastWarning {
  foreground: string;
  background: string;
  ratio: number;
  required: number;
}

/** commands::ThemePreview */
export interface ThemePreview {
  theme: ThemeInfo;
  tokens: CssVar[];
  warnings: ContrastWarning[];
}
