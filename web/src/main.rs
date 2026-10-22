//! The Muster website. Server-rendered HTML and CSS only: no JavaScript, no cookies, no
//! third-party requests (`.claude/rules/topcoat.md`, `.claude/rules/privacy.md`).

#![forbid(unsafe_code)]

mod app;
// Vendored from `topcoat ui add`; kept close to upstream so updates diff cleanly.
#[allow(
    dead_code,
    unreachable_pub,
    clippy::pedantic,
    reason = "vendored Topcoat UI components, edited as little as possible"
)]
mod components;
mod download;
mod security;
mod site;
mod theme_css;

use std::process::ExitCode;

use topcoat::asset::AssetBundle;

#[tokio::main]
async fn main() -> ExitCode {
    // `MUSTER_WEB_ASSETS` points at the bundle in the container image; locally the bundle
    // sits next to the binary (`topcoat asset bundle`).
    let assets = match std::env::var_os("MUSTER_WEB_ASSETS") {
        Some(dir) => AssetBundle::load_dir(dir),
        None => AssetBundle::load(),
    };
    let result = match assets {
        Ok(assets) => topcoat::start(app::router(assets)).await,
        Err(e) => Err(std::io::Error::other(e.to_string())),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            #[allow(
                clippy::print_stderr,
                reason = "fatal startup error, no logger configured"
            )]
            {
                eprintln!("muster-web: {e}");
            }
            ExitCode::FAILURE
        }
    }
}
