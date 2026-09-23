//! `agent start`: prepare a worktree and return its onboarding context.

use std::path::PathBuf;

use chrono::{DateTime, FixedOffset};
use serde::Serialize;

use super::{AttachOptions, HandoffInput, Runtime, Workspace, attach, handoff, register};
use crate::config::{CONFIG_FILE, ProjectConfig, find_project_root};
use crate::model::WorkerStatus;
use crate::state::Protocol;
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
    if matches!(ws.store.pull()?, PullOutcome::Diverged { .. }) {
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

    Ok(AgentStartOutcome {
        onboard: build_onboard_view(&snapshot, Some(identity.id), 5),
        attached,
        registered,
        resumed,
        warnings,
    })
}
