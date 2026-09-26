use ctx_sync_core::model::format_age;
use ctx_sync_core::ops::{self, PruneOptions, PruneReason, Runtime, Workspace};
use ctx_sync_core::{Result, clock};

use super::hint_on_conflict;
use crate::cli::{WorkerCommand, WorkerPruneArgs};

pub fn run(command: &WorkerCommand) -> Result<()> {
    match command {
        WorkerCommand::Prune(args) => prune(args),
    }
}

fn prune(args: &WorkerPruneArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let now = clock::now()?;
    let outcome = hint_on_conflict(ops::prune_workers(
        &ws,
        PruneOptions {
            stale_after: args.stale_after,
            dry_run: args.dry_run,
            sync: args.sync,
            now,
        },
    ))?;
    if outcome.removed.is_empty() {
        println!("Nothing to prune");
    } else {
        if args.dry_run {
            println!("Would prune {} worker(s)", outcome.removed.len());
        } else {
            println!("Pruned {} worker(s)", outcome.removed.len());
        }
        println!();
        for worker in &outcome.removed {
            let reason = match worker.reason {
                PruneReason::Done => "done".to_string(),
                PruneReason::Abandoned => "abandoned".to_string(),
                PruneReason::Stale => format!(
                    "stale, last updated {} ago",
                    format_age(now.signed_duration_since(worker.last_updated))
                ),
            };
            println!("- {} ({}) {reason}", worker.name, worker.short_id);
        }
        if !args.dry_run {
            println!();
            match &outcome.committed {
                Some(commit) => println!("committed: {commit}"),
                None => println!("committed: no changes"),
            }
            match &outcome.sync {
                Some(sync) => println!("synced: {}", sync.revision),
                None => println!("synced: no (run `ctx-sync sync`)"),
            }
        }
    }
    for warning in outcome
        .warnings
        .iter()
        .chain(outcome.sync.iter().flat_map(|sync| sync.warnings.iter()))
    {
        eprintln!("warning: {warning}");
    }
    Ok(())
}
