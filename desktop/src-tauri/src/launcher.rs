//! Starts processes and runs groups. Plans come from `muster-core`, already validated.

use std::process::Child;
use std::time::Duration;

use muster_core::groups::Step;
use muster_core::launch::LaunchPlan;
use tauri::{AppHandle, Emitter, Runtime};

use crate::commands::CommandError;
use crate::diag;

/// Event sent when an entry of a running group fails after the `run_group` command returned.
pub(crate) const LAUNCH_FAILED: &str = "launch-failed";

/// Small stacks: these threads only block in `wait` or `sleep`.
const STACK: usize = 64 * 1024;

#[derive(Debug, Clone, serde::Serialize)]
struct LaunchFailed {
    app: String,
    message: &'static str,
}

/// Starts one plan and reaps it on a dedicated thread, so it never becomes a zombie and
/// nothing polls for its exit.
pub(crate) fn start(plan: &LaunchPlan) -> Result<(), CommandError> {
    let child = muster_core::launch::launch(plan).map_err(CommandError::from)?;
    reap(child);
    Ok(())
}

fn reap(mut child: Child) {
    let spawned = std::thread::Builder::new()
        .name("muster-reap".into())
        .stack_size(STACK)
        .spawn(move || {
            // The exit status of a user's app is theirs, not an error of Muster's. `wait`
            // can only fail if the child was already reaped, which nothing else does.
            if let Err(e) = child.wait() {
                diag::report("waiting for a launched app", &e);
            }
        });
    if let Err(e) = spawned {
        // The app is running; only its exit status is lost, and it lingers as a zombie
        // until Muster exits.
        diag::report("starting the reaper thread", &e);
    }
}

/// Runs the valid steps in order on a thread that lives for the run only.
pub(crate) fn run_steps<R: Runtime>(
    app: AppHandle<R>,
    steps: Vec<Step>,
) -> Result<(), CommandError> {
    std::thread::Builder::new()
        .name("muster-group".into())
        .stack_size(STACK)
        .spawn(move || {
            for step in steps {
                let Ok(plan) = step.plan else { continue };
                if step.delay > Duration::ZERO {
                    std::thread::sleep(step.delay);
                }
                if let Err(e) = start(&plan) {
                    let payload = LaunchFailed {
                        app: step.app.0,
                        message: e.message,
                    };
                    if let Err(e) = app.emit(LAUNCH_FAILED, payload) {
                        diag::report("reporting a failed launch", &e);
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|_| CommandError::INTERNAL)
}
