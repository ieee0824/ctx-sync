//! `onboard`: what an agent joining the project needs first
//! (`ctx-sync onboard`, and the output of `ctx-sync agent start`).

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use uuid::Uuid;

use super::{
    AttentionItem, ViewOptions, collect_attention, doc_body, or_none, render_attention_item,
};
use crate::ids::short_id;
use crate::model::md::{demote_headings, render_list};
use crate::model::{
    ContextSnapshot, Worker, WorkerStatus, effective_decisions, format_age, is_stale,
};

const MAX_SUMMARY_CHARS: usize = 120;
const MAX_FILES: usize = 10;

#[derive(Debug, Clone, Serialize)]
pub struct DecisionBrief {
    pub id: String,
    pub title: String,
    /// First line of the decision, shortened.
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerBrief {
    pub name: String,
    pub short_id: String,
    pub status: WorkerStatus,
    pub stale: bool,
    /// Time since the last worker update, formatted for display.
    pub age: String,
    pub task: String,
    /// Changed files, at most 10 (plus a `... and N more` line).
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentChange {
    pub worker: String,
    pub last_updated: DateTime<FixedOffset>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OnboardView {
    pub project_name: String,
    /// "Goal" section of `10-project.md`.
    pub goal: String,
    /// "Non Goals" section of `10-project.md`.
    pub non_goals: String,
    /// `20-architecture.md` without its title.
    pub architecture: String,
    /// This worktree's worker, when registered.
    pub you: Option<WorkerBrief>,
    pub important_decisions: Vec<DecisionBrief>,
    /// Active workers other than you.
    pub active_workers: Vec<WorkerBrief>,
    pub needs_attention: Vec<AttentionItem>,
    pub recent_changes: Vec<RecentChange>,
    pub warnings: Vec<String>,
}

pub fn build_onboard_view(
    snapshot: &ContextSnapshot,
    you: Option<Uuid>,
    recent_limit: usize,
    opts: &ViewOptions,
) -> OnboardView {
    let section = |heading: &str| {
        snapshot
            .project
            .section_body(heading)
            .unwrap_or_default()
            .to_string()
    };
    let mut recent: Vec<&Worker> = snapshot
        .workers
        .iter()
        .filter(|w| !w.summary.trim().is_empty())
        .collect();
    recent.sort_by_key(|w| std::cmp::Reverse(w.last_updated));

    OnboardView {
        project_name: snapshot.meta.project_name.clone(),
        goal: section("Goal"),
        non_goals: section("Non Goals"),
        architecture: doc_body(&snapshot.architecture),
        you: you.and_then(|id| {
            snapshot
                .workers
                .iter()
                .find(|w| w.id == id)
                .map(|w| brief(w, opts))
        }),
        important_decisions: effective_decisions(&snapshot.decisions)
            .into_iter()
            .map(|d| DecisionBrief {
                id: d.id.to_string(),
                title: d.title.clone(),
                summary: shorten(first_line(&d.decision)),
            })
            .collect(),
        active_workers: snapshot
            .workers
            .iter()
            .filter(|w| w.status.is_active() && Some(w.id) != you)
            .map(|w| brief(w, opts))
            .collect(),
        needs_attention: collect_attention(&snapshot.workers),
        recent_changes: recent
            .into_iter()
            .take(recent_limit)
            .map(|w| RecentChange {
                worker: w.name.clone(),
                last_updated: w.last_updated,
                summary: first_line(&w.summary).to_string(),
            })
            .collect(),
        warnings: snapshot.warnings.clone(),
    }
}

pub fn render_onboard_markdown(view: &OnboardView) -> String {
    let mut blocks = vec![
        "# Project Onboarding".to_string(),
        "## Project".to_string(),
        view.project_name.clone(),
        "### Goal".to_string(),
        or_none(demote_headings(&view.goal, 2)),
        "### Non Goals".to_string(),
        or_none(demote_headings(&view.non_goals, 2)),
    ];

    if let Some(you) = &view.you {
        blocks.push("## You".into());
        let mut lines = vec![format!(
            "{} ({}) — {}",
            you.name,
            you.short_id,
            status_with_age(you)
        )];
        if !you.task.is_empty() {
            lines.push(format!("Task: {}", first_line(&you.task)));
        }
        blocks.push(lines.join("\n"));
    }

    blocks.push("## Current Architecture".into());
    blocks.push(or_none(demote_headings(&view.architecture, 1)));

    blocks.push("## Important Decisions".into());
    blocks.push(or_none(
        view.important_decisions
            .iter()
            .map(|d| {
                if d.summary.is_empty() {
                    format!("- {}: {}", d.id, d.title)
                } else {
                    format!("- {}: {} — {}", d.id, d.title, d.summary)
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    ));

    blocks.push("## Active Workers".into());
    if view.active_workers.is_empty() {
        blocks.push("_None_".into());
    }
    for w in &view.active_workers {
        blocks.push(format!(
            "### {} ({}) — {}",
            w.name,
            w.short_id,
            status_with_age(w)
        ));
        if !w.task.is_empty() {
            blocks.push(format!("Task:\n{}", w.task));
        }
        if !w.files.is_empty() {
            blocks.push(format!("Files:\n{}", render_list(&w.files)));
        }
    }

    blocks.push("## Needs Attention".into());
    blocks.push(or_none(
        view.needs_attention
            .iter()
            .map(render_attention_item)
            .collect::<Vec<_>>()
            .join("\n"),
    ));

    blocks.push("## Recent Relevant Changes".into());
    blocks.push(or_none(
        view.recent_changes
            .iter()
            .map(|c| {
                format!(
                    "- {} {}: {}",
                    c.last_updated.to_rfc3339(),
                    c.worker,
                    c.summary
                )
            })
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

fn brief(w: &Worker, opts: &ViewOptions) -> WorkerBrief {
    let mut files: Vec<String> = w.changed.iter().take(MAX_FILES).cloned().collect();
    if w.changed.len() > MAX_FILES {
        files.push(format!("... and {} more", w.changed.len() - MAX_FILES));
    }
    WorkerBrief {
        name: w.name.clone(),
        short_id: short_id(&w.id),
        status: w.status,
        stale: is_stale(w, opts.now, opts.stale_after),
        age: format_age(opts.now.signed_duration_since(w.last_updated)),
        task: w.task.clone(),
        files,
    }
}

fn status_with_age(worker: &WorkerBrief) -> String {
    if worker.stale {
        format!("{}, stale ({})", worker.status, worker.age)
    } else {
        worker.status.to_string()
    }
}

fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
}

fn shorten(text: &str) -> String {
    if text.chars().count() <= MAX_SUMMARY_CHARS {
        text.to_string()
    } else {
        let mut short: String = text.chars().take(MAX_SUMMARY_CHARS).collect();
        short.push('…');
        short
    }
}
