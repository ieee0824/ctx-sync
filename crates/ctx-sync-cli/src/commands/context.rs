use ctx_sync_core::ops::{Runtime, Workspace};
use ctx_sync_core::store::ContextStore;
use ctx_sync_core::view::{
    ViewOptions, build_context_view, filter_context_view, render_context_markdown,
};
use ctx_sync_core::{Error, Result, clock};

use crate::cli::ContextArgs;

/// Prints the local snapshot; run `ctx-sync pull` first for the latest one.
pub fn run(args: &ContextArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let mut view = build_context_view(
        &ws.store.snapshot()?,
        &ViewOptions {
            now: clock::now()?,
            stale_after: args.stale_after,
        },
    );
    if let Some(task) = &args.task {
        view = filter_context_view(view, task);
    }
    if args.json {
        let json = serde_json::to_string_pretty(&view)
            .map_err(|e| Error::General(format!("cannot serialize context: {e}")))?;
        println!("{json}");
    } else {
        print!("{}", render_context_markdown(&view));
    }
    Ok(())
}
