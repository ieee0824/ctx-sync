use ctx_sync_core::ops::{self, HandoffOutcome, Runtime, Workspace};
use ctx_sync_core::{Result, clock};

use super::hint_on_conflict;
use crate::cli::HandoffArgs;
use crate::convert::handoff_input;

pub fn run(args: &HandoffArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let outcome = hint_on_conflict(ops::handoff(
        &ws,
        handoff_input(&args.fields),
        args.sync,
        clock::now()?,
    ))?;
    print_outcome(&outcome);
    Ok(())
}

/// Shared by `handoff`, `done` and `agent finish`.
pub(crate) fn print_outcome(outcome: &HandoffOutcome) {
    let sync_warnings = outcome.sync.iter().flat_map(|s| s.warnings.iter());
    for warning in outcome.warnings.iter().chain(sync_warnings) {
        eprintln!("warning: {warning}");
    }
    println!(
        "Updated worker {} ({})",
        outcome.worker_name, outcome.short_id
    );
    println!();
    println!("status: {}", outcome.status);
    println!("file: {}", outcome.file_name);
    match &outcome.committed {
        Some(commit) => println!("committed: {commit}"),
        None => println!("committed: no changes"),
    }
    match &outcome.sync {
        Some(sync) => println!("synced: {}", sync.revision),
        None => println!("synced: no (run `ctx-sync sync`)"),
    }
}
