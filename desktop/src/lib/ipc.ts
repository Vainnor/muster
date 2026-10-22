// The only module that talks to the Tauri shell. One typed function per command.
import { invoke } from "@tauri-apps/api/core";

import type { AppInfo, CssVar } from "./types";

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function themeTokens(id: string): Promise<CssVar[]> {
  return invoke<CssVar[]>("theme_tokens", { id });
}
