// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    match muster_desktop::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            #[allow(
                clippy::print_stderr,
                reason = "the app failed to start; no UI exists to report it"
            )]
            {
                eprintln!("muster failed to start: {e:#}");
            }
            ExitCode::FAILURE
        }
    }
}
