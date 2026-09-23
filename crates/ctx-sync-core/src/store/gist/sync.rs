//! `sync`: commit, fetch, rebase and push with optimistic concurrency.
//!
//! There is no distributed lock. When another worker pushes between our
//! fetch and push, the push is rejected as non-fast-forward and the loop
//! starts over. A rebase conflict is never resolved automatically: the
//! rebase is aborted, the conflict is recorded and the command stops.

use chrono::{DateTime, FixedOffset};

use super::GistStore;
use crate::git::{GitFailure, classify, failure_to_error};
use crate::model::duplicate_decision_seqs;
use crate::state::ConflictRecord;
use crate::store::{ContextStore, SyncOutcome};
use crate::{Error, Result};

pub(super) const MAX_ATTEMPTS: u32 = 3;

/// Steps of the sync loop, abstracted so that the loop can be tested
/// without git.
pub(super) trait SyncOps {
    fn fetch(&mut self) -> Result<()>;
    fn ahead_behind(&mut self) -> Result<(u32, u32)>;
    /// `Ok(None)` on success, `Ok(Some(files))` on a conflict (already aborted).
    fn rebase(&mut self) -> Result<Option<Vec<String>>>;
    /// `Ok(true)` on success, `Ok(false)` when rejected as non-fast-forward.
    fn push(&mut self) -> Result<bool>;
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum LoopResult {
    Pushed { attempts: u32 },
    NothingToPush { attempts: u32 },
    Conflict { files: Vec<String> },
}

pub(super) fn sync_loop(ops: &mut impl SyncOps) -> Result<LoopResult> {
    for attempt in 1..=MAX_ATTEMPTS {
        ops.fetch()?;
        let (mut ahead, behind) = ops.ahead_behind()?;
        if behind > 0 {
            if let Some(files) = ops.rebase()? {
                return Ok(LoopResult::Conflict { files });
            }
            ahead = ops.ahead_behind()?.0;
        }
        if ahead == 0 {
            return Ok(LoopResult::NothingToPush { attempts: attempt });
        }
        if ops.push()? {
            return Ok(LoopResult::Pushed { attempts: attempt });
        }
    }
    Err(Error::SyncRetryExceeded)
}

pub(super) fn sync(store: &GistStore, now: DateTime<FixedOffset>) -> Result<SyncOutcome> {
    let _lock = store.lock()?;
    store.ensure_unlocked()?;
    let committed = store.commit_unlocked("ctx-sync: sync")?;
    let mut ops = GitSyncOps {
        store,
        branch: store.branch()?,
        conflict_heads: None,
    };
    let (pushed, attempts) = match sync_loop(&mut ops)? {
        LoopResult::Pushed { attempts } => (true, attempts),
        LoopResult::NothingToPush { attempts } => (false, attempts),
        LoopResult::Conflict { files } => {
            let (local_head, remote_head) = ops.conflict_heads.take().unwrap_or_default();
            ConflictRecord {
                detected_at: now,
                files: files.clone(),
                local_head,
                remote_head,
            }
            .save(store.conflict_path())?;
            return Err(Error::ContextConflict { files });
        }
    };
    ConflictRecord::clear(store.conflict_path())?;
    Ok(SyncOutcome {
        committed,
        pushed,
        revision: store.short_head()?,
        attempts,
        warnings: duplicate_warnings(store),
    })
}

/// Warnings for decisions sharing a number. A snapshot that cannot be read
/// does not fail the sync.
fn duplicate_warnings(store: &GistStore) -> Vec<String> {
    let Ok(snapshot) = store.snapshot() else {
        return Vec::new();
    };
    duplicate_decision_seqs(&snapshot.decisions)
        .into_iter()
        .map(|dup| {
            let ids: Vec<&str> = dup.ids.iter().map(|id| id.as_str()).collect();
            format!(
                "duplicate decision number {}-{:03}: {}",
                dup.date,
                dup.seq,
                ids.join(", ")
            )
        })
        .collect()
}

struct GitSyncOps<'a> {
    store: &'a GistStore,
    branch: String,
    /// (local HEAD, origin/<branch>) of the rebase that conflicted.
    conflict_heads: Option<(String, String)>,
}

impl SyncOps for GitSyncOps<'_> {
    fn fetch(&mut self) -> Result<()> {
        self.store.git().run_checked(&["fetch", "-q", "origin"])?;
        Ok(())
    }

    fn ahead_behind(&mut self) -> Result<(u32, u32)> {
        self.store.ahead_behind(&self.branch)
    }

    fn rebase(&mut self) -> Result<Option<Vec<String>>> {
        let git = self.store.git();
        let upstream = format!("origin/{}", self.branch);
        let local_head = git.run_line(&["rev-parse", "HEAD"])?;
        let remote_head = git.run_line(&["rev-parse", &upstream])?;
        // Rebasing rewrites commits, so it needs a committer identity too.
        let mut args = self.store.identity_args()?;
        args.extend(["rebase", "-q", upstream.as_str()]);
        let out = git.run(&args)?;
        if out.success {
            return Ok(None);
        }
        let files: Vec<String> = git
            .run_checked(&["diff", "--name-only", "--diff-filter=U"])?
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        // Never leave the context repository in the middle of a rebase.
        git.run_checked(&["rebase", "--abort"])?;
        if files.is_empty() {
            return Err(Error::General(format!(
                "git rebase failed: {}",
                out.stderr.trim()
            )));
        }
        self.conflict_heads = Some((local_head, remote_head));
        Ok(Some(files))
    }

    fn push(&mut self) -> Result<bool> {
        let refspec = format!("HEAD:{}", self.branch);
        // Git hooks are not skipped.
        let out = self.store.git().run(&["push", "-q", "origin", &refspec])?;
        if out.success {
            return Ok(true);
        }
        match classify(&out.stderr) {
            GitFailure::NonFastForward => Ok(false),
            kind => {
                let mut message = format!("git push failed: {}", out.stderr.trim());
                if out.stderr.to_lowercase().contains("hook") {
                    message.push_str(" (a git hook may have rejected the push)");
                }
                Err(failure_to_error(kind, message))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scripted responses; records how often each step ran.
    #[derive(Default)]
    struct FakeOps {
        ahead_behind: Vec<(u32, u32)>,
        rebase_conflict: Option<Vec<String>>,
        push_results: Vec<bool>,
        fetches: u32,
        rebases: u32,
        pushes: u32,
    }

    impl SyncOps for FakeOps {
        fn fetch(&mut self) -> Result<()> {
            self.fetches += 1;
            Ok(())
        }

        fn ahead_behind(&mut self) -> Result<(u32, u32)> {
            Ok(self.ahead_behind.remove(0))
        }

        fn rebase(&mut self) -> Result<Option<Vec<String>>> {
            self.rebases += 1;
            Ok(self.rebase_conflict.clone())
        }

        fn push(&mut self) -> Result<bool> {
            self.pushes += 1;
            Ok(self.push_results.remove(0))
        }
    }

    #[test]
    fn pushes_local_commits() {
        let mut ops = FakeOps {
            ahead_behind: vec![(1, 0)],
            push_results: vec![true],
            ..Default::default()
        };
        assert_eq!(
            sync_loop(&mut ops).unwrap(),
            LoopResult::Pushed { attempts: 1 }
        );
        assert_eq!((ops.fetches, ops.rebases, ops.pushes), (1, 0, 1));
    }

    #[test]
    fn retries_after_non_fast_forward() {
        let mut ops = FakeOps {
            ahead_behind: vec![(1, 0), (1, 1), (1, 0)],
            push_results: vec![false, true],
            ..Default::default()
        };
        assert_eq!(
            sync_loop(&mut ops).unwrap(),
            LoopResult::Pushed { attempts: 2 }
        );
        assert_eq!((ops.fetches, ops.rebases, ops.pushes), (2, 1, 2));
    }

    #[test]
    fn gives_up_after_three_rejections() {
        let mut ops = FakeOps {
            ahead_behind: vec![(1, 0); 3],
            push_results: vec![false; 3],
            ..Default::default()
        };
        let err = sync_loop(&mut ops).unwrap_err();
        assert!(matches!(err, Error::SyncRetryExceeded), "{err:?}");
        assert_eq!(err.exit_code(), 5);
        assert_eq!(ops.pushes, 3);
    }

    #[test]
    fn stops_on_conflict_without_pushing() {
        let mut ops = FakeOps {
            ahead_behind: vec![(1, 1)],
            rebase_conflict: Some(vec!["20-architecture.md".into()]),
            ..Default::default()
        };
        assert_eq!(
            sync_loop(&mut ops).unwrap(),
            LoopResult::Conflict {
                files: vec!["20-architecture.md".into()]
            }
        );
        assert_eq!(ops.pushes, 0);
    }

    #[test]
    fn nothing_to_push() {
        let mut ops = FakeOps {
            ahead_behind: vec![(0, 0)],
            ..Default::default()
        };
        assert_eq!(
            sync_loop(&mut ops).unwrap(),
            LoopResult::NothingToPush { attempts: 1 }
        );
        assert_eq!(ops.pushes, 0);
    }

    #[test]
    fn fast_forward_only_is_nothing_to_push() {
        let mut ops = FakeOps {
            ahead_behind: vec![(0, 2), (0, 0)],
            ..Default::default()
        };
        assert_eq!(
            sync_loop(&mut ops).unwrap(),
            LoopResult::NothingToPush { attempts: 1 }
        );
        assert_eq!((ops.rebases, ops.pushes), (1, 0));
    }
}
