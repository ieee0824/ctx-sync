//! `decision add`: append a new decision file.
//!
//! Existing decision files are never modified. Replacing a decision means
//! adding a new one that lists the old one in `Supersedes:`.

use chrono::{DateTime, FixedOffset};
use serde::Serialize;

use super::Workspace;
use crate::ids::short_id;
use crate::model::{Decision, DecisionId, DecisionStatus, next_decision_seq, slugify};
use crate::secrets;
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

    let mut warnings: Vec<String> = [
        ("title", Some(&title)),
        ("context", input.context.as_ref()),
        ("decision", input.decision.as_ref()),
        ("reason", input.reason.as_ref()),
        ("consequences", input.consequences.as_ref()),
    ]
    .into_iter()
    .filter_map(|(field, text)| text.map(|t| secrets::scan(field, t)))
    .flatten()
    .map(|finding| secrets::warning_message(&finding))
    .collect();

    // Number the decision from the latest known state to avoid collisions.
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

/// Append a replacement decision, warning when the old decision is not effective.
pub fn supersede_decision(
    ws: &Workspace,
    old: &str,
    mut input: NewDecision,
    sync: bool,
    now: DateTime<FixedOffset>,
) -> Result<DecisionAddOutcome> {
    ws.require_identity()?;
    ws.store.pull()?;
    let snapshot = ws.store.snapshot()?;
    let old_decision = snapshot
        .decisions
        .iter()
        .find(|decision| decision.id.as_str() == old)
        .ok_or_else(|| Error::General(format!("unknown decision id: {old}")))?;
    let mut superseded_by: Vec<_> = snapshot
        .decisions
        .iter()
        .filter(|decision| decision.supersedes.contains(&old_decision.id))
        .map(|decision| decision.id.as_str())
        .collect();
    superseded_by.sort_unstable();
    let warning = if !superseded_by.is_empty() {
        Some(format!(
            "decision {old} is not effective (superseded by {})",
            superseded_by.join(", ")
        ))
    } else if old_decision.status != DecisionStatus::Accepted {
        Some(format!(
            "decision {old} is not effective (status {})",
            old_decision.status
        ))
    } else {
        None
    };
    if !input.supersedes.iter().any(|id| id == old) {
        input.supersedes.insert(0, old.to_string());
    }
    let mut outcome = add_decision(ws, input, sync, now)?;
    if let Some(warning) = warning {
        outcome.warnings.push(warning);
    }
    Ok(outcome)
}
