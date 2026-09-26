//! Advisory path claims for coordinating active workers.

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use uuid::Uuid;

use super::Workspace;
use crate::Result;
use crate::ids::short_id;
use crate::model::claim::{matches, overlaps, validate_pattern};
use crate::model::{Worker, WorkerStatus};
use crate::store::{ContextStore, PullOutcome, SyncOutcome};

#[derive(Debug, Clone)]
pub struct ClaimInput {
    pub patterns: Vec<String>,
    /// Release the named patterns, or every own claim when empty.
    pub release: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaimConflict {
    pub worker: String,
    pub short_id: String,
    pub mine: String,
    pub theirs: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimOutcome {
    pub claims: Vec<String>,
    pub conflicts: Vec<ClaimConflict>,
    pub committed: Option<String>,
    pub sync: Option<SyncOutcome>,
    pub warnings: Vec<String>,
}

pub fn claim(
    ws: &Workspace,
    input: ClaimInput,
    sync: bool,
    now: DateTime<FixedOffset>,
) -> Result<ClaimOutcome> {
    let identity = ws.require_identity()?;
    for pattern in &input.patterns {
        validate_pattern(pattern)?;
    }
    let mut warnings = Vec::new();
    if let PullOutcome::Diverged { ahead, behind } = ws.store.pull()? {
        warnings.push(format!(
            "context diverged ({ahead} ahead, {behind} behind); run `ctx-sync sync`"
        ));
    }

    let file_name = Worker::file_name_for(&identity.id);
    let mut worker = match ws.store.read_file(&file_name)? {
        Some(text) => Worker::parse(&text)?,
        None => Worker::new(identity.id, &identity.name, now),
    };
    worker.name = identity.name.clone();
    if input.release {
        if input.patterns.is_empty() {
            worker.claims.clear();
        } else {
            worker.claims.retain(|p| !input.patterns.contains(p));
        }
    } else {
        for pattern in input.patterns {
            if !worker.claims.contains(&pattern) {
                worker.claims.push(pattern);
            }
        }
    }
    worker.last_updated = now;
    ws.store.write_file(&file_name, &worker.render())?;
    let committed = ws
        .store
        .commit(&format!("ctx-sync: claim {}", identity.name))?;
    let sync = if sync {
        Some(ws.store.sync(now)?)
    } else {
        None
    };

    let snapshot = ws.store.snapshot()?;
    warnings.extend(snapshot.warnings);
    let conflicts = claim_conflicts(&worker.claims, identity.id, &snapshot.workers);
    warnings.extend(conflicts.iter().map(conflict_warning));
    Ok(ClaimOutcome {
        claims: worker.claims,
        conflicts,
        committed,
        sync,
        warnings,
    })
}

pub fn claim_conflicts(mine: &[String], me: Uuid, workers: &[Worker]) -> Vec<ClaimConflict> {
    find_conflicts(mine, me, workers, overlaps)
}

pub fn changed_file_conflicts(
    changed: &[String],
    me: Uuid,
    workers: &[Worker],
) -> Vec<ClaimConflict> {
    find_conflicts(changed, me, workers, |pattern, path| matches(path, pattern))
}

fn find_conflicts(
    mine: &[String],
    me: Uuid,
    workers: &[Worker],
    test: impl Fn(&str, &str) -> bool,
) -> Vec<ClaimConflict> {
    let mut conflicts = Vec::new();
    for worker in workers {
        if worker.id == me
            || !matches!(worker.status, WorkerStatus::Working | WorkerStatus::Blocked)
        {
            continue;
        }
        for own in mine {
            for theirs in &worker.claims {
                if test(own, theirs) {
                    conflicts.push(ClaimConflict {
                        worker: worker.name.clone(),
                        short_id: short_id(&worker.id),
                        mine: own.clone(),
                        theirs: theirs.clone(),
                    });
                }
            }
        }
    }
    conflicts
}

pub fn conflict_warning(c: &ClaimConflict) -> String {
    format!(
        "{} overlaps with {} ({})'s claim {}",
        c.mine, c.worker, c.short_id, c.theirs
    )
}
