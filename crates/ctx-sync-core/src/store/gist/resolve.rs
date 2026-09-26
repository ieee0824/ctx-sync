//! Rebase a conflicted context repository with an explicit merge preference.

use super::GistStore;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseStrategy {
    /// In a rebase, `theirs` refers to the local commit being replayed.
    KeepLocal,
    /// In a rebase, `ours` refers to the fetched remote branch.
    KeepRemote,
}

impl GistStore {
    pub fn rebase_with_strategy(&self, strategy: RebaseStrategy) -> Result<()> {
        let _lock = self.lock()?;
        self.ensure_unlocked()?;
        let git = self.git();
        git.run_checked(&["fetch", "-q", "origin"])?;
        self.commit_unlocked("ctx-sync: sync")?;
        let branch = self.branch()?;
        let upstream = format!("origin/{branch}");
        let preference = match strategy {
            RebaseStrategy::KeepLocal => "theirs",
            RebaseStrategy::KeepRemote => "ours",
        };
        let mut args = self.identity_args()?;
        args.extend(["rebase", "-q", "-X", preference, &upstream]);
        let result = git.run(&args)?;
        if result.success {
            return Ok(());
        }
        let abort = git.run(&["rebase", "--abort"])?;
        let detail = result.stderr.trim();
        let message = if abort.success {
            format!("git rebase failed: {detail}")
        } else {
            format!(
                "git rebase failed: {detail}; rebase --abort failed: {}",
                abort.stderr.trim()
            )
        };
        Err(Error::General(message))
    }
}
