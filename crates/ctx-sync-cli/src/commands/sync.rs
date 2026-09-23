use ctx_sync_core::ops::{Runtime, Workspace};
use ctx_sync_core::store::ContextStore;
use ctx_sync_core::{Result, clock};

use super::hint_on_conflict;

pub fn run() -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let outcome = hint_on_conflict(ws.store.sync(clock::now()?))?;
    for warning in &outcome.warnings {
        eprintln!("warning: {warning}");
    }
    if outcome.pushed {
        println!("Synced ({})", outcome.revision);
    } else {
        println!("Nothing to push ({})", outcome.revision);
    }
    Ok(())
}
