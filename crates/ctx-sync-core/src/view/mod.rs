//! Views of the shared context.
//!
//! Each view is built as a serializable struct from a `ContextSnapshot` and
//! rendered separately, so that a `--json` output only needs `serde_json`.

use chrono::{DateTime, FixedOffset, TimeDelta};
use serde::Serialize;

use crate::ids::short_id;
use crate::model::{MdDoc, Worker, WorkerStatus, default_stale_after, format_age, is_stale};

pub mod conflict;
pub mod context;
pub mod decisions;
pub mod onboard;
pub mod relevance;
pub mod status;

pub use conflict::{ConflictFile, ConflictView, render_conflict_text};
pub use context::{ContextView, DecisionSummary, build_context_view, render_context_markdown};
pub use decisions::{DecisionListItem, build_decision_list, render_decision_list};
pub use onboard::{
    DecisionBrief, OnboardView, RecentChange, WorkerBrief, build_onboard_view,
    render_onboard_markdown,
};
pub use relevance::{filter_context_view, relevance, task_tokens};
pub use status::{StatusView, build_status_view, render_status_text};

#[derive(Debug, Clone, Copy)]
pub struct ViewOptions {
    pub now: DateTime<FixedOffset>,
    pub stale_after: TimeDelta,
}

impl ViewOptions {
    pub fn new(now: DateTime<FixedOffset>) -> Self {
        Self {
            now,
            stale_after: default_stale_after(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerSummary {
    pub short_id: String,
    pub name: String,
    pub status: WorkerStatus,
    pub stale: bool,
    /// Time since the last worker update, formatted for display.
    pub age: String,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub task: String,
    pub working_on: Vec<String>,
    pub changed: Vec<String>,
    pub interface_changes: Vec<String>,
    pub attention: Vec<String>,
    pub blocked_by: Vec<String>,
    pub summary: String,
    pub last_updated: DateTime<FixedOffset>,
}

impl WorkerSummary {
    pub fn new(w: &Worker, opts: &ViewOptions) -> Self {
        Self {
            short_id: short_id(&w.id),
            name: w.name.clone(),
            status: w.status,
            stale: is_stale(w, opts.now, opts.stale_after),
            age: format_age(opts.now.signed_duration_since(w.last_updated)),
            branch: w.branch.clone(),
            commit: w.commit.clone(),
            task: w.task.clone(),
            working_on: w.working_on.clone(),
            changed: w.changed.clone(),
            interface_changes: w.interface_changes.clone(),
            attention: w.attention.clone(),
            blocked_by: w.blocked_by.clone(),
            summary: w.summary.clone(),
            last_updated: w.last_updated,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    Attention,
    InterfaceChange,
    BlockedBy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttentionItem {
    pub worker: String,
    pub kind: AttentionKind,
    pub text: String,
}

/// Attention items, interface changes and blockers of active workers, in
/// worker order. Finished workers are left out to keep the context small.
pub fn collect_attention(workers: &[Worker]) -> Vec<AttentionItem> {
    let mut items = Vec::new();
    for w in workers.iter().filter(|w| w.status.is_active()) {
        let groups = [
            (AttentionKind::Attention, &w.attention),
            (AttentionKind::InterfaceChange, &w.interface_changes),
            (AttentionKind::BlockedBy, &w.blocked_by),
        ];
        for (kind, texts) in groups {
            items.extend(texts.iter().map(|text| AttentionItem {
                worker: w.name.clone(),
                kind,
                text: text.clone(),
            }));
        }
    }
    items
}

/// `- [parser] text`, `- [parser] interface: text`, `- [parser] blocked by: text`.
pub fn render_attention_item(item: &AttentionItem) -> String {
    let label = match item.kind {
        AttentionKind::Attention => "",
        AttentionKind::InterfaceChange => "interface: ",
        AttentionKind::BlockedBy => "blocked by: ",
    };
    format!("- [{}] {label}{}", item.worker, item.text)
}

/// A document without its `# title` line: fields, preamble and sections.
pub(crate) fn doc_body(doc: &MdDoc) -> String {
    doc.render()
        .split_once("\n\n")
        .map(|(_, rest)| rest.trim_end().to_string())
        .unwrap_or_default()
}

/// `_None_` for an empty section so that every heading has content.
pub(crate) fn or_none(text: String) -> String {
    if text.trim().is_empty() {
        "_None_".to_string()
    } else {
        text
    }
}

/// `Label: value` for one line, `Label:\nvalue` for several lines.
pub(crate) fn labeled(label: &str, value: &str) -> String {
    if value.contains('\n') {
        format!("{label}:\n{value}")
    } else {
        format!("{label}: {value}")
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::clock::parse_now;

    fn worker(name: &str, status: WorkerStatus) -> Worker {
        let mut w = Worker::new(
            Uuid::new_v4(),
            name,
            parse_now(Some("2026-09-23T18:00:00+09:00")).unwrap(),
        );
        w.status = status;
        w.attention = vec![format!("{name} attention")];
        w.interface_changes = vec![format!("{name} interface")];
        w.blocked_by = vec![format!("{name} blocker")];
        w
    }

    #[test]
    fn attention_comes_from_active_workers_only() {
        let workers = [
            worker("a", WorkerStatus::Working),
            worker("b", WorkerStatus::Done),
            worker("c", WorkerStatus::Blocked),
        ];
        let rendered: Vec<String> = collect_attention(&workers)
            .iter()
            .map(render_attention_item)
            .collect();
        assert_eq!(
            rendered,
            [
                "- [a] a attention",
                "- [a] interface: a interface",
                "- [a] blocked by: a blocker",
                "- [c] c attention",
                "- [c] interface: c interface",
                "- [c] blocked by: c blocker",
            ]
        );
    }

    #[test]
    fn doc_body_drops_the_title() {
        let doc = MdDoc::parse("# Project\n\n## Goal\n\nShare context.\n").unwrap();
        assert_eq!(doc_body(&doc), "## Goal\n\nShare context.");
        assert_eq!(doc_body(&MdDoc::parse("# Empty\n").unwrap()), "");
    }

    #[test]
    fn labeled_uses_a_block_for_multiple_lines() {
        assert_eq!(labeled("Task", "one"), "Task: one");
        assert_eq!(labeled("Task", "one\ntwo"), "Task:\none\ntwo");
    }
}
