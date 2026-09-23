//! `sync_state`: the synchronization state, without touching the network.

use super::GistStore;
use crate::Result;
use crate::state::ConflictRecord;
use crate::store::SyncState;

/// Reads local information only: `behind` reflects the last fetch.
/// No lock is taken because nothing is modified.
pub(super) fn sync_state(store: &GistStore) -> Result<SyncState> {
    let git = store.git();
    let branch = store.branch()?;
    let head = git.run(&["rev-parse", "--short", "HEAD"])?;
    let revision = head.success.then(|| head.stdout.trim().to_string());
    let remote_ref = format!("refs/remotes/origin/{branch}");
    let has_remote = git
        .run(&["rev-parse", "--verify", "--quiet", &remote_ref])?
        .success;
    let (ahead, behind) = if has_remote && revision.is_some() {
        store.ahead_behind(&branch)?
    } else {
        (0, 0)
    };
    let dirty = !git
        .run_checked(&["status", "--porcelain"])?
        .trim()
        .is_empty();
    let conflict = ConflictRecord::load(store.conflict_path())?;
    Ok(SyncState {
        branch,
        revision,
        ahead,
        behind,
        dirty,
        conflict,
    })
}
