use ctx_sync_core::Result;
use ctx_sync_core::ops::{Runtime, Workspace};
use ctx_sync_core::store::{ContextStore, PullOutcome};

/// Updates the context repository only; the project tree is never touched.
pub fn run() -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    match ws.store.pull()? {
        PullOutcome::UpToDate => {
            let revision = ws.store.revision()?.unwrap_or_default();
            println!("Already up to date ({revision})");
        }
        PullOutcome::FastForwarded { from, to } => println!("Updated {from}..{to}"),
        PullOutcome::Diverged { ahead, behind } => {
            println!(
                "Local context has {ahead} unpushed commit(s) and remote has {behind} new commit(s)"
            );
            eprintln!("warning: run `ctx-sync sync` to rebase and push");
        }
    }
    Ok(())
}
