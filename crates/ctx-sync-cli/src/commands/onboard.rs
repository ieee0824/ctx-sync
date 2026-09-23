use ctx_sync_core::Result;
use ctx_sync_core::ops::{Runtime, Workspace};
use ctx_sync_core::store::ContextStore;
use ctx_sync_core::view::{build_onboard_view, render_onboard_markdown};

/// Number of recent worker summaries shown.
pub(crate) const RECENT_LIMIT: usize = 5;

pub fn run() -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let you = ws.identity()?.map(|identity| identity.id);
    let view = build_onboard_view(&ws.store.snapshot()?, you, RECENT_LIMIT);
    print!("{}", render_onboard_markdown(&view));
    Ok(())
}
