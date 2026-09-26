use ctx_sync_core::model::Worker;
use ctx_sync_core::ops::{self, ClaimInput, Runtime, Workspace};
use ctx_sync_core::store::ContextStore;
use ctx_sync_core::{Result, clock};

use super::hint_on_conflict;
use crate::cli::ClaimArgs;

pub fn run(args: &ClaimArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    if args.patterns.is_empty() && !args.release {
        let snapshot = ws.store.snapshot()?;
        let workers: Vec<&Worker> = snapshot
            .workers
            .iter()
            .filter(|worker| worker.status.is_active() && !worker.claims.is_empty())
            .collect();
        if workers.is_empty() {
            println!("No claims.");
        } else {
            for worker in workers {
                println!(
                    "{} ({}): {}",
                    worker.name,
                    ctx_sync_core::ids::short_id(&worker.id),
                    worker.claims.join(", ")
                );
            }
        }
        return Ok(());
    }

    let identity = ws.require_identity()?;
    let outcome = hint_on_conflict(ops::claim(
        &ws,
        ClaimInput {
            patterns: args.patterns.clone(),
            release: args.release,
        },
        args.sync,
        clock::now()?,
    ))?;
    println!("Claims of {} ({}):", identity.name, identity.short_id());
    for pattern in &outcome.claims {
        println!("- {pattern}");
    }
    match &outcome.committed {
        Some(commit) => println!("committed: {commit}"),
        None => println!("committed: no changes"),
    }
    match &outcome.sync {
        Some(sync) => println!("synced: {}", sync.revision),
        None => println!("synced: no (run `ctx-sync sync`)"),
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
