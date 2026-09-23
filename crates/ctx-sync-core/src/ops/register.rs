//! `register`: register this worktree as a worker and create its worker file.

use chrono::{DateTime, FixedOffset};
use serde::Serialize;

use super::Workspace;
use crate::Result;
use crate::model::{Worker, WorkerStatus};
use crate::repo_info;
use crate::state::{WorkerIdentity, validate_worker_name};
use crate::store::ContextStore;

#[derive(Debug, Clone, Serialize)]
pub struct RegisterOutcome {
    pub identity: WorkerIdentity,
    /// `false` when the worktree was already registered.
    pub created: bool,
    pub file_name: String,
    pub committed: Option<String>,
}

/// The identity is saved in the local state, never in the project
/// repository. The worker file is committed to the context repository but
/// not synced.
pub fn register(
    ws: &Workspace,
    name: &str,
    force: bool,
    now: DateTime<FixedOffset>,
) -> Result<RegisterOutcome> {
    validate_worker_name(name)?;
    let existing = ws.identity()?;
    if let Some(identity) = &existing
        && !force
    {
        return Ok(RegisterOutcome {
            file_name: Worker::file_name_for(&identity.id),
            identity: identity.clone(),
            created: false,
            committed: None,
        });
    }

    ws.store.ensure()?;
    if let Some(old) = existing {
        // The old identity is replaced: mark its worker file as abandoned.
        let old_file = Worker::file_name_for(&old.id);
        if let Some(text) = ws.store.read_file(&old_file)? {
            let mut worker = Worker::parse(&text)?;
            worker.status = WorkerStatus::Abandoned;
            worker.last_updated = now;
            ws.store.write_file(&old_file, &worker.render())?;
        }
    }

    let identity = WorkerIdentity::new(name, &ws.root, now)?;
    identity.save(&ws.identity_path()?)?;

    let repo = repo_info::collect(&ws.root, &[])?;
    let mut worker = Worker::new(identity.id, name, now);
    worker.branch = repo.branch;
    worker.commit = repo.commit;
    let file_name = worker.file_name();
    ws.store.write_file(&file_name, &worker.render())?;
    let committed = ws.store.commit(&format!("ctx-sync: register {name}"))?;

    Ok(RegisterOutcome {
        identity,
        created: true,
        file_name,
        committed,
    })
}
