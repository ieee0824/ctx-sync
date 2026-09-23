use ctx_sync_core::ops::{Runtime, Workspace};
use ctx_sync_core::store::ContextStore;
use ctx_sync_core::{Error, Result, clock};

pub fn run() -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let outcome = ws.store.sync(clock::now()?).inspect_err(|e| {
        if matches!(e, Error::ContextConflict { .. }) {
            eprintln!("hint: run `ctx-sync status` for details");
        }
    })?;
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
