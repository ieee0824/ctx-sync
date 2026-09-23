//! `pull`: fetch the Gist and fast-forward when possible.

use super::GistStore;
use crate::Result;
use crate::store::PullOutcome;

/// Never rewrites local commits: when both sides have new commits the
/// repository is left as is and `Diverged` is returned (`sync` rebases).
pub(super) fn pull(store: &GistStore) -> Result<PullOutcome> {
    let _lock = store.lock()?;
    store.ensure_unlocked()?;
    let git = store.git();
    git.run_checked(&["fetch", "-q", "origin"])?;
    let branch = store.branch()?;
    let (ahead, behind) = store.ahead_behind(&branch)?;
    if behind == 0 {
        return Ok(PullOutcome::UpToDate);
    }
    if ahead > 0 {
        return Ok(PullOutcome::Diverged { ahead, behind });
    }
    let from = store.short_head()?;
    git.run_checked(&["merge", "-q", "--ff-only", &format!("origin/{branch}")])?;
    let to = store.short_head()?;
    Ok(PullOutcome::FastForwarded { from, to })
}
