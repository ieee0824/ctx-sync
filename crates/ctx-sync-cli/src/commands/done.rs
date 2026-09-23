use ctx_sync_core::model::WorkerStatus;
use ctx_sync_core::ops::{self, HandoffInput, Runtime, Workspace};
use ctx_sync_core::{Result, clock};

use super::handoff::print_outcome;
use super::hint_on_conflict;
use crate::cli::DoneArgs;

/// A handoff that marks this worker as done and always syncs.
pub fn run(args: &DoneArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let input = HandoffInput {
        status: Some(WorkerStatus::Done),
        summary: args.summary.clone(),
        ..Default::default()
    };
    let outcome = hint_on_conflict(ops::handoff(&ws, input, true, clock::now()?))?;
    print_outcome(&outcome);
    Ok(())
}
