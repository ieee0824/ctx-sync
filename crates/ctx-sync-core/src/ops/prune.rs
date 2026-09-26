//! Explicit maintenance of finished and stale worker files.

use chrono::{DateTime, FixedOffset, TimeDelta};
use serde::Serialize;

use super::Workspace;
use crate::Result;
use crate::ids::short_id;
use crate::model::{WorkerStatus, is_stale};
use crate::store::{ContextStore, PullOutcome, SyncOutcome};

pub struct PruneOptions {
    /// Also target stale active workers when set.
    pub stale_after: Option<TimeDelta>,
    pub dry_run: bool,
    pub sync: bool,
    pub now: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PruneReason {
    Done,
    Abandoned,
    Stale,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrunedWorker {
    pub name: String,
    pub short_id: String,
    pub file_name: String,
    pub last_updated: DateTime<FixedOffset>,
    pub reason: PruneReason,
}

#[derive(Debug, Clone, Serialize)]
pub struct PruneOutcome {
    /// Planned removals when dry_run is true, actual removals otherwise.
    pub removed: Vec<PrunedWorker>,
    pub committed: Option<String>,
    pub sync: Option<SyncOutcome>,
    pub warnings: Vec<String>,
}

pub fn prune_workers(ws: &Workspace, opts: PruneOptions) -> Result<PruneOutcome> {
    let mut warnings = Vec::new();
    if let PullOutcome::Diverged { ahead, behind } = ws.store.pull()? {
        warnings.push(format!(
            "context diverged ({ahead} ahead, {behind} behind); run `ctx-sync sync`"
        ));
    }
    let snapshot = ws.store.snapshot()?;
    warnings.extend(snapshot.warnings);
    let me = ws.identity()?.map(|identity| identity.id);
    let removed: Vec<PrunedWorker> = snapshot
        .workers
        .iter()
        .filter(|worker| Some(worker.id) != me)
        .filter_map(|worker| {
            let reason = match worker.status {
                WorkerStatus::Done => PruneReason::Done,
                WorkerStatus::Abandoned => PruneReason::Abandoned,
                _ if opts
                    .stale_after
                    .is_some_and(|threshold| is_stale(worker, opts.now, threshold)) =>
                {
                    PruneReason::Stale
                }
                _ => return None,
            };
            Some(PrunedWorker {
                name: worker.name.clone(),
                short_id: short_id(&worker.id),
                file_name: worker.file_name(),
                last_updated: worker.last_updated,
                reason,
            })
        })
        .collect();
    if opts.dry_run {
        return Ok(PruneOutcome {
            removed,
            committed: None,
            sync: None,
            warnings,
        });
    }

    let mut actual = Vec::new();
    for worker in removed {
        if ws.store.remove_file(&worker.file_name)? {
            actual.push(worker);
        }
    }
    let committed = if actual.is_empty() {
        None
    } else {
        ws.store
            .commit(&format!("ctx-sync: prune {} worker(s)", actual.len()))?
    };
    let sync = if !actual.is_empty() && opts.sync {
        Some(ws.store.sync(opts.now)?)
    } else {
        None
    };
    Ok(PruneOutcome {
        removed: actual,
        committed,
        sync,
        warnings,
    })
}
