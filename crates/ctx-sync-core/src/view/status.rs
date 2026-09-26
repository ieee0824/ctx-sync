//! `status`: human-oriented overview of the project, sync state and workers.

use std::path::{Path, PathBuf};

use serde::Serialize;
use uuid::Uuid;

use super::{ViewOptions, WorkerSummary};
use crate::model::ContextSnapshot;
use crate::state::WorkerIdentity;
use crate::store::SyncState;

#[derive(Debug, Clone, Serialize)]
pub struct StatusView {
    pub project_name: String,
    pub project_id: Uuid,
    pub gist_id: String,
    /// Where the shared files can be edited directly.
    pub context_repo: PathBuf,
    pub sync: SyncState,
    /// (name, short id) of this worktree's worker.
    pub you: Option<(String, String)>,
    /// Every worker, including done and abandoned ones.
    pub workers: Vec<WorkerSummary>,
    pub warnings: Vec<String>,
}

pub fn build_status_view(
    snapshot: &ContextSnapshot,
    gist_id: &str,
    context_repo: &Path,
    sync: SyncState,
    you: Option<&WorkerIdentity>,
    opts: &ViewOptions,
) -> StatusView {
    StatusView {
        project_name: snapshot.meta.project_name.clone(),
        project_id: snapshot.meta.project_id,
        gist_id: gist_id.to_string(),
        context_repo: context_repo.to_path_buf(),
        sync,
        you: you.map(|id| (id.name.clone(), id.short_id())),
        workers: snapshot
            .workers
            .iter()
            .map(|w| WorkerSummary::new(w, opts))
            .collect(),
        warnings: snapshot.warnings.clone(),
    }
}

pub fn render_status_text(view: &StatusView) -> String {
    let sync = &view.sync;
    let mut blocks = vec![format!(
        "Project: {} ({})",
        view.project_name, view.project_id
    )];

    let ahead_hint = if sync.ahead > 0 {
        " (run `ctx-sync sync`)"
    } else {
        ""
    };
    blocks.push(
        [
            "Remote:".to_string(),
            format!("  gist: {}", view.gist_id),
            format!("  revision: {}", sync.revision.as_deref().unwrap_or("none")),
            format!("  branch: {}", sync.branch),
            format!(
                "  ahead: {}, behind: {}{ahead_hint}",
                sync.ahead, sync.behind
            ),
            format!(
                "  uncommitted changes: {}",
                if sync.dirty { "yes" } else { "no" }
            ),
            format!("  context repo: {}", view.context_repo.display()),
        ]
        .join("\n"),
    );

    if let Some(conflict) = &sync.conflict {
        let mut lines = vec![format!(
            "CONFLICT (detected at {}):",
            conflict.detected_at.to_rfc3339()
        )];
        lines.extend(conflict.files.iter().map(|f| format!("  {f}")));
        lines.push(String::new());
        lines.push("  Resolve manually in the context repo:".into());
        lines.push(format!("    cd {}", view.context_repo.display()));
        lines.push(format!(
            "    git rebase origin/{}   # fix the conflicts, then `git rebase --continue`",
            sync.branch
        ));
        lines.push("    ctx-sync sync".into());
        blocks.push(lines.join("\n"));
    }

    blocks.push(match &view.you {
        Some((name, id)) => format!("You: {name} ({id})"),
        None => "You: not registered (run `ctx-sync register <name>`)".into(),
    });

    blocks.push("Workers:".into());
    if view.workers.is_empty() {
        blocks.push("  (none)".into());
    }
    for w in &view.workers {
        let mut lines = vec![
            format!("{} ({})", w.name, w.short_id),
            if w.stale {
                format!("  {} (stale, {})", w.status, w.age)
            } else {
                format!("  {}", w.status)
            },
        ];
        if let Some(branch) = &w.branch {
            lines.push(format!("  branch: {branch}"));
        }
        if let Some(task) = w.task.lines().next().filter(|t| !t.is_empty()) {
            lines.push(format!("  task: {task}"));
        }
        blocks.push(lines.join("\n"));
    }

    if !view.warnings.is_empty() {
        let mut lines = vec!["Warnings:".to_string()];
        lines.extend(view.warnings.iter().map(|w| format!("  {w}")));
        blocks.push(lines.join("\n"));
    }

    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}
