use ctx_sync_core::Result;
use ctx_sync_core::ops::{Runtime, Workspace};
use ctx_sync_core::store::ContextStore;
use ctx_sync_core::view::{build_status_view, render_status_text};

/// Local information only: nothing is fetched.
pub fn run() -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let snapshot = ws.store.snapshot()?;
    let sync = ws.store.sync_state()?;
    let identity = ws.identity()?;
    let view = build_status_view(
        &snapshot,
        &ws.config.remote.id,
        ws.store.repo_dir(),
        sync,
        identity.as_ref(),
    );
    print!("{}", render_status_text(&view));
    Ok(())
}
