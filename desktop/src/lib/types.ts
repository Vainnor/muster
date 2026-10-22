// Hand-written mirrors of the Rust types the commands return. Change both sides together.

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
