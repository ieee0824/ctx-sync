//! `decision add`: append a new decision file.
//!
//! Existing decision files are never modified. Replacing a decision means
//! adding a new one that lists the old one in `Supersedes:`.

use chrono::{DateTime, FixedOffset};
use serde::Serialize;

use super::Workspace;
use crate::ids::short_id;
use crate::model::{Decision, DecisionId, DecisionStatus, next_decision_seq, slugify};
use crate::store::{ContextStore, PullOutcome, SyncOutcome};
use crate::{Error, Result};

pub struct NewDecision {
    pub title: String,
    pub context: Option<String>,
    /// Defaults to the title.
    pub decision: Option<String>,
    pub reason: Option<String>,
    pub consequences: Option<String>,
    pub supersedes: Vec<String>,
    pub status: DecisionStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecisionAddOutcome {
    pub id: DecisionId,
    pub file_name: String,
    pub committed: Option<String>,
    pub sync: Option<SyncOutcome>,
    pub warnings: Vec<String>,
}

pub fn add_decision(
    ws: &Workspace,
    input: NewDecision,
    sync: bool,
    now: DateTime<FixedOffset>,
) -> Result<DecisionAddOutcome> {
    let identity = ws.require_identity()?;
    let title = input.title.trim().to_string();
    if title.is_empty() {
        return Err(Error::InvalidConfig("decision title is empty".into()));
    }

    // Number the decision from the latest known state to avoid collisions.
    let mut warnings = Vec::new();
    if let PullOutcome::Diverged { .. } = ws.store.pull()? {
        warnings.push(
            "local context has unpushed commits; the decision number is based on the local copy \
             (run `ctx-sync sync`)"
                .to_string(),
        );
    }
    let snapshot = ws.store.snapshot()?;

    let supersedes = input
        .supersedes
        .iter()
        .map(|raw| {
            let id = DecisionId::parse(raw.trim())?;
            if snapshot.decisions.iter().any(|d| d.id == id) {
                Ok(id)
            } else {
                Err(Error::General(format!("unknown decision id: {id}")))
            }
        })
        .collect::<Result<Vec<_>>>()?;

    let date = now.date_naive();
    let id = DecisionId::new(
        date,
        next_decision_seq(&snapshot.decisions, date),
        &slugify(&title),
    );
    let file_name = id.file_name();
    if ws.store.read_file(&file_name)?.is_some() {
        return Err(Error::General(format!("{file_name} already exists")));
    }

    let decision = Decision {
        id: id.clone(),
        decision: input.decision.unwrap_or_else(|| title.clone()),
        title,
        status: input.status,
        date,
        author: Some(short_id(&identity.id)),
        supersedes,
        context: input.context.unwrap_or_default(),
        reason: input.reason.unwrap_or_default(),
        consequences: input.consequences.unwrap_or_default(),
        extra_sections: Vec::new(),
    };
    ws.store.write_file(&file_name, &decision.render())?;
    let committed = ws.store.commit(&format!("ctx-sync: decision {id}"))?;
    let sync = if sync {
        Some(ws.store.sync(now)?)
    } else {
        None
    };
    Ok(DecisionAddOutcome {
        id,
        file_name,
        committed,
        sync,
        warnings,
    })
}
