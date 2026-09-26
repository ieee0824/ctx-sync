//! `context`: the relatively complete view of the shared context, for AI
//! agents (`ctx-sync context`).

use chrono::NaiveDate;
use serde::Serialize;
use uuid::Uuid;

use super::{
    AttentionItem, ViewOptions, WorkerSummary, collect_attention, doc_body, labeled, or_none,
    render_attention_item,
};
use crate::model::md::{demote_headings, render_list};
use crate::model::{ContextSnapshot, effective_decisions};

#[derive(Debug, Clone, Serialize)]
pub struct DecisionSummary {
    pub id: String,
    pub title: String,
    pub date: NaiveDate,
    pub decision: String,
    pub supersedes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextView {
    pub project_name: String,
    pub project_id: Uuid,
    /// `10-project.md` without its title.
    pub project: String,
    /// `20-architecture.md` without its title.
    pub architecture: String,
    /// Accepted decisions that are not superseded.
    pub decisions: Vec<DecisionSummary>,
    /// Working and blocked workers only.
    pub active_workers: Vec<WorkerSummary>,
    pub attention: Vec<AttentionItem>,
    pub warnings: Vec<String>,
}

pub fn build_context_view(snapshot: &ContextSnapshot, opts: &ViewOptions) -> ContextView {
    ContextView {
        project_name: snapshot.meta.project_name.clone(),
        project_id: snapshot.meta.project_id,
        project: doc_body(&snapshot.project),
        architecture: doc_body(&snapshot.architecture),
        decisions: effective_decisions(&snapshot.decisions)
            .into_iter()
            .map(|d| DecisionSummary {
                id: d.id.to_string(),
                title: d.title.clone(),
                date: d.date,
                decision: d.decision.clone(),
                supersedes: d.supersedes.iter().map(ToString::to_string).collect(),
            })
            .collect(),
        active_workers: snapshot
            .workers
            .iter()
            .filter(|w| w.status.is_active())
            .map(|w| WorkerSummary::new(w, opts))
            .collect(),
        attention: collect_attention(&snapshot.workers),
        warnings: snapshot.warnings.clone(),
    }
}

pub fn render_context_markdown(view: &ContextView) -> String {
    let mut blocks = vec![
        "# Shared Project Context".to_string(),
        format!(
            "Project: {}\nProject ID: {}",
            view.project_name, view.project_id
        ),
        "## Project".to_string(),
        or_none(demote_headings(&view.project, 1)),
        "## Architecture".to_string(),
        or_none(demote_headings(&view.architecture, 1)),
        "## Accepted Decisions".to_string(),
    ];
    if view.decisions.is_empty() {
        blocks.push("_None_".into());
    }
    for d in &view.decisions {
        blocks.push(format!("### {}: {}", d.id, d.title));
        let mut body = Vec::new();
        if !d.supersedes.is_empty() {
            body.push(format!("Supersedes: {}", d.supersedes.join(", ")));
        }
        if !d.decision.is_empty() {
            body.push(demote_headings(&d.decision, 3));
        }
        if !body.is_empty() {
            blocks.push(body.join("\n\n"));
        }
    }

    blocks.push("## Active Workers".into());
    if view.active_workers.is_empty() {
        blocks.push("_None_".into());
    }
    for w in &view.active_workers {
        blocks.push(format!("### {} ({})", w.name, w.short_id));
        blocks.push(render_worker(w));
    }

    blocks.push("## Attention".into());
    blocks.push(or_none(
        view.attention
            .iter()
            .map(render_attention_item)
            .collect::<Vec<_>>()
            .join("\n"),
    ));

    if !view.warnings.is_empty() {
        blocks.push("## Warnings".into());
        blocks.push(render_list(&view.warnings));
    }
    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

/// Worker details; empty items are left out.
fn render_worker(w: &WorkerSummary) -> String {
    let status = if w.stale {
        format!("{} (stale: last updated {} ago)", w.status, w.age)
    } else {
        w.status.to_string()
    };
    let mut lines = vec![format!("Status: {status}")];
    if let Some(branch) = &w.branch {
        lines.push(format!("Branch: {branch}"));
    }
    if !w.task.is_empty() {
        lines.push(labeled("Task", &w.task));
    }
    let lists = [
        ("Working On", &w.working_on),
        ("Changed", &w.changed),
        ("Interface Changes", &w.interface_changes),
        ("Attention", &w.attention),
        ("Blocked By", &w.blocked_by),
    ];
    for (label, items) in lists {
        if !items.is_empty() {
            lines.push(format!("{label}:\n{}", render_list(items)));
        }
    }
    if !w.summary.is_empty() {
        lines.push(labeled("Summary", &w.summary));
    }
    lines.join("\n")
}
