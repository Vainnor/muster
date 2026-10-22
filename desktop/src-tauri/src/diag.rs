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
