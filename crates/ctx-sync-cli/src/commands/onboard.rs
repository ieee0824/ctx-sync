use ctx_sync_core::clock;
use ctx_sync_core::ops::{Runtime, Workspace};
use ctx_sync_core::store::ContextStore;
use ctx_sync_core::view::{ViewOptions, build_onboard_view, render_onboard_markdown};
use ctx_sync_core::{Error, Result};

use crate::cli::OnboardArgs;

/// Number of recent worker summaries shown.
pub(crate) const RECENT_LIMIT: usize = 5;

pub fn run(args: &OnboardArgs) -> Result<()> {
    let rt = Runtime::from_env()?;
    let ws = Workspace::open(&rt, &std::env::current_dir()?)?;
    let you = ws.identity()?.map(|identity| identity.id);
    let view = build_onboard_view(
        &ws.store.snapshot()?,
        you,
        RECENT_LIMIT,
        &ViewOptions::new(clock::now()?),
    );
    if args.json {
        let json = serde_json::to_string_pretty(&view)
            .map_err(|e| Error::General(format!("cannot serialize onboard: {e}")))?;
        println!("{json}");
    } else {
        print!("{}", render_onboard_markdown(&view));
    }
    Ok(())
}
