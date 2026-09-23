use ctx_sync_core::ops::{self, NewDecision, Runtime, Workspace};
use ctx_sync_core::{Result, clock};

use super::hint_on_conflict;
use crate::cli::{DecisionAddArgs, DecisionCommand};
use crate::convert::decision_status;

pub fn run(command: &DecisionCommand) -> Result<()> {
    match command {
        DecisionCommand::Add(args) => add(args),
    }
}

fn add(args: &DecisionAddArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let input = NewDecision {
        title: args.title.clone(),
        context: args.context.clone(),
        decision: args.decision.clone(),
        reason: args.reason.clone(),
        consequences: args.consequences.clone(),
        supersedes: args.supersedes.clone(),
        status: decision_status(args.status),
    };
    let outcome = hint_on_conflict(ops::add_decision(&ws, input, args.sync, clock::now()?))?;
    let sync_warnings = outcome.sync.iter().flat_map(|s| s.warnings.iter());
    for warning in outcome.warnings.iter().chain(sync_warnings) {
        eprintln!("warning: {warning}");
    }
    println!("Added decision");
    println!();
    println!("id: {}", outcome.id);
    println!("file: {}", outcome.file_name);
    match &outcome.committed {
        Some(commit) => println!("committed: {commit}"),
        None => println!("committed: no changes"),
    }
    match &outcome.sync {
        Some(sync) => println!("synced: {}", sync.revision),
        None => println!("synced: no (run `ctx-sync sync`)"),
    }
    Ok(())
}
