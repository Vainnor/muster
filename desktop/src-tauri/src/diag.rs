//! Failures with no caller left to tell (callbacks, background threads). They go to stderr
//! until the local log sink (`Settings::local_logging`) exists. Messages name what was being
//! done, never a path or app name (`.claude/rules/privacy.md`).

pub(crate) fn report(context: &str, error: &dyn std::error::Error) {
    #[allow(
        clippy::print_stderr,
        reason = "local diagnostics until the log sink exists"
    )]
    {
        eprintln!("muster: {context} failed: {error}");
    }
}

/// Like [`report`], for errors whose text can hold a path: only a fixed `kind` is printed.
#[cfg_attr(
    not(feature = "updater"),
    allow(dead_code, reason = "only the updater's errors need it")
)]
pub(crate) fn report_kind(context: &str, kind: &str) {
    #[allow(
        clippy::print_stderr,
        reason = "local diagnostics until the log sink exists"
    )]
    {
        eprintln!("muster: {context} failed ({kind})");
    }
}
