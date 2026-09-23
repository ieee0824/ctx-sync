//! High-level commands for an agent's work session.

use std::path::PathBuf;

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use uuid::Uuid;

use super::{
    AttachOptions, HandoffInput, HandoffOutcome, Runtime, Workspace, attach, handoff, register,
};
use crate::config::{CONFIG_FILE, ProjectConfig, find_project_root};
use crate::fs_util::TempDirGuard;
use crate::model::{ContextSnapshot, WorkerStatus};
use crate::state::{Protocol, StateRoot};
use crate::store::{ContextStore, PullOutcome};
use crate::view::{OnboardView, build_onboard_view};
use crate::{Error, Result};

pub struct AgentStartOptions {
    pub cwd: PathBuf,
    pub name: Option<String>,
    pub resume: bool,
    pub now: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentStartOutcome {
    pub onboard: OnboardView,
    pub attached: bool,
    pub registered: bool,
    pub resumed: bool,
    pub warnings: Vec<String>,
}

pub fn agent_start(rt: &Runtime, opts: AgentStartOptions) -> Result<AgentStartOutcome> {
    let root = find_project_root(&opts.cwd).map_err(|_| {
        Error::InvalidConfig(
            "no .ctx-sync.toml found; ask a human to run `ctx-sync init` or `ctx-sync attach <gist>`"
                .into(),
        )
    })?;

    let mut attached = false;
    let ws = match Workspace::open(rt, &root) {
        Ok(ws) => ws,
        Err(Error::InvalidConfig(message))
            if message.starts_with("this machine is not attached to gist ") =>
        {
            let config = ProjectConfig::load(&root.join(CONFIG_FILE))?;
            attach(
                rt,
                AttachOptions {
                    project_root: root.clone(),
                    gist: config.remote.id,
                    protocol: Protocol::Https,
                    now: opts.now,
                },
            )?;
            attached = true;
            Workspace::open(rt, &root)?
        }
        Err(error) => return Err(error),
    };

    let mut registered = false;
    let identity = match ws.identity()? {
        Some(identity) => identity,
        None => {
            let name = opts.name.as_deref().ok_or_else(|| {
                Error::InvalidConfig(
                    "worker is not registered; run `ctx-sync agent start --name <name>` or `ctx-sync register <name>`"
                        .into(),
                )
            })?;
            let outcome = register(&ws, name, false, opts.now)?;
            registered = outcome.created;
            outcome.identity
        }
    };

    let mut warnings = Vec::new();
    let diverged = matches!(ws.store.pull()?, PullOutcome::Diverged { .. });
    if diverged {
        warnings.push("local context has unpushed commits; run `ctx-sync sync`".into());
    }
    if let Some(conflict) = ws.store.sync_state()?.conflict {
        warnings.push(format!(
            "unresolved context conflict: {}; run `ctx-sync status`",
            conflict.files.join(", ")
        ));
    }

    let mut snapshot = ws.store.snapshot()?;
    let mut resumed = false;
    if let Some(worker) = snapshot
        .workers
        .iter()
        .find(|worker| worker.id == identity.id)
        && matches!(worker.status, WorkerStatus::Done | WorkerStatus::Abandoned)
    {
        if opts.resume {
            handoff(
                &ws,
                HandoffInput {
                    status: Some(WorkerStatus::Working),
                    ..Default::default()
                },
                false,
                opts.now,
            )?;
            resumed = true;
            snapshot = ws.store.snapshot()?;
        } else {
            warnings.push(format!(
                "your worker status is {}; pass --resume to continue",
                worker.status
            ));
        }
    }

    // Pull cannot fast-forward a worktree with local commits. Read the latest
    // remote snapshot separately so onboarding still includes other workers'
    // updates, while retaining this worktree's unpushed worker and decisions.
    if diverged {
        snapshot = snapshot_with_remote_updates(rt, &ws, snapshot, identity.id)?;
    }

    Ok(AgentStartOutcome {
        onboard: build_onboard_view(&snapshot, Some(identity.id), 5),
        attached,
        registered,
        resumed,
        warnings,
    })
}

fn snapshot_with_remote_updates(
    rt: &Runtime,
    ws: &Workspace,
    local: ContextSnapshot,
    own_id: Uuid,
) -> Result<ContextSnapshot> {
    let tmp = TempDirGuard::new(rt.state_root.tmp_dir().join(Uuid::new_v4().to_string()));
    let state = StateRoot::new(tmp.path()).project(&Uuid::nil());
    let store = rt.gist_store(&state, &ws.config.remote.id, ws.local.protocol);
    store.ensure()?;
    let mut remote = store.snapshot()?;

    if let Some(own_worker) = local.workers.iter().find(|worker| worker.id == own_id) {
        remote.workers.retain(|worker| worker.id != own_id);
        remote.workers.push(own_worker.clone());
        remote
            .workers
            .sort_by(|a, b| (&a.name, a.id).cmp(&(&b.name, b.id)));
    }
    for decision in local.decisions {
        if !remote.decisions.iter().any(|item| item.id == decision.id) {
            remote.decisions.push(decision);
        }
    }
    remote.decisions.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(remote)
}

/// Publish this worker's handoff and synchronize the context repository.
pub fn agent_finish(
    ws: &Workspace,
    input: HandoffInput,
    now: DateTime<FixedOffset>,
) -> Result<HandoffOutcome> {
    handoff(ws, input, true, now)
}
