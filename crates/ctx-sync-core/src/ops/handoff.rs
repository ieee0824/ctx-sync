//! `handoff`: update this worker's own file for other workers.
//!
//! Only the caller's worker file is ever written. The file describes the
//! current state, so list arguments replace the stored lists by default.

use chrono::{DateTime, FixedOffset};
use serde::Serialize;

use super::Workspace;
use super::claim::{changed_file_conflicts, conflict_warning};
use crate::Result;
use crate::ids::short_id;
use crate::model::{Worker, WorkerStatus};
use crate::repo_info::{self, RepoInfo};
use crate::secrets;
use crate::store::{ContextStore, SyncOutcome};

/// Files taken from git are capped so that worker files stay small.
const MAX_AUTO_CHANGED: usize = 50;

#[derive(Debug, Clone, Default)]
pub struct HandoffInput {
    pub task: Option<String>,
    pub summary: Option<String>,
    pub status: Option<WorkerStatus>,
    pub working_on: Option<Vec<String>>,
    pub changed: Option<Vec<String>>,
    pub interface_changes: Option<Vec<String>>,
    pub attention: Option<Vec<String>>,
    pub blocked_by: Option<Vec<String>>,
    /// Append list values instead of replacing them.
    pub append: bool,
}

/// Applies `input` to `worker`. Pure: no I/O.
///
/// - `None` keeps the current value, `Some` replaces it (or appends when
///   `append` is set, without duplicates).
/// - Without `changed`, the files reported by git are used when there are any.
/// - Branch and commit always follow the project repository.
pub fn apply_handoff(
    worker: &Worker,
    input: &HandoffInput,
    repo: &RepoInfo,
    now: DateTime<FixedOffset>,
) -> Worker {
    let list = |current: &Vec<String>, new: &Option<Vec<String>>| match new {
        None => current.clone(),
        Some(new) if input.append => {
            let mut merged = current.clone();
            for item in new {
                if !merged.contains(item) {
                    merged.push(item.clone());
                }
            }
            merged
        }
        Some(new) => new.clone(),
    };

    let mut updated = worker.clone();
    if let Some(task) = &input.task {
        updated.task = task.clone();
    }
    if let Some(summary) = &input.summary {
        updated.summary = summary.clone();
    }
    if let Some(status) = input.status {
        updated.status = status;
    }
    updated.working_on = list(&worker.working_on, &input.working_on);
    updated.changed = match &input.changed {
        None if !repo.changed_files.is_empty() => capped(&repo.changed_files),
        changed => list(&worker.changed, changed),
    };
    updated.interface_changes = list(&worker.interface_changes, &input.interface_changes);
    updated.attention = list(&worker.attention, &input.attention);
    updated.blocked_by = list(&worker.blocked_by, &input.blocked_by);
    if repo.branch.is_some() {
        updated.branch = repo.branch.clone();
    }
    if repo.commit.is_some() {
        updated.commit = repo.commit.clone();
    }
    updated.last_updated = now;
    updated
}

fn capped(files: &[String]) -> Vec<String> {
    if files.len() <= MAX_AUTO_CHANGED {
        return files.to_vec();
    }
    let mut out = files[..MAX_AUTO_CHANGED].to_vec();
    out.push(format!(
        "... and {} more files",
        files.len() - MAX_AUTO_CHANGED
    ));
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct HandoffOutcome {
    pub worker_name: String,
    pub short_id: String,
    pub status: WorkerStatus,
    pub file_name: String,
    pub committed: Option<String>,
    pub sync: Option<SyncOutcome>,
    pub warnings: Vec<String>,
}

pub fn handoff(
    ws: &Workspace,
    input: HandoffInput,
    sync: bool,
    now: DateTime<FixedOffset>,
) -> Result<HandoffOutcome> {
    let identity = ws.require_identity()?;
    let mut warnings = secret_warnings(&input);
    ws.store.ensure()?;
    let file_name = Worker::file_name_for(&identity.id);
    let current = match ws.store.read_file(&file_name)? {
        Some(text) => Worker::parse(&text)?,
        None => Worker::new(identity.id, &identity.name, now),
    };
    let repo = repo_info::collect(&ws.root, &[])?;
    let mut updated = apply_handoff(&current, &input, &repo, now);
    updated.name = identity.name.clone();
    let snapshot = ws.store.snapshot()?;
    warnings.extend(
        changed_file_conflicts(&updated.changed, identity.id, &snapshot.workers)
            .iter()
            .map(conflict_warning),
    );
    ws.store.write_file(&file_name, &updated.render())?;
    let committed = ws
        .store
        .commit(&format!("ctx-sync: handoff {}", identity.name))?;
    let sync = if sync {
        Some(ws.store.sync(now)?)
    } else {
        None
    };
    Ok(HandoffOutcome {
        worker_name: identity.name,
        short_id: short_id(&identity.id),
        status: updated.status,
        file_name,
        committed,
        sync,
        warnings,
    })
}

/// Warnings for obvious credentials in the values given to this handoff.
/// Field names follow the CLI options (`attention[0]`, ...).
fn secret_warnings(input: &HandoffInput) -> Vec<String> {
    let mut fields: Vec<(String, &str)> = Vec::new();
    for (name, value) in [("task", &input.task), ("summary", &input.summary)] {
        if let Some(value) = value {
            fields.push((name.to_string(), value));
        }
    }
    let lists = [
        ("working-on", &input.working_on),
        ("changed", &input.changed),
        ("interface-change", &input.interface_changes),
        ("attention", &input.attention),
        ("blocked-by", &input.blocked_by),
    ];
    for (name, items) in lists {
        for (i, item) in items.iter().flatten().enumerate() {
            fields.push((format!("{name}[{i}]"), item));
        }
    }
    fields
        .iter()
        .flat_map(|(field, text)| secrets::scan(field, text))
        .map(|finding| secrets::warning_message(&finding))
        .collect()
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::clock::parse_now;

    fn t(s: &str) -> DateTime<FixedOffset> {
        parse_now(Some(s)).unwrap()
    }

    fn worker() -> Worker {
        let mut w = Worker::new(Uuid::new_v4(), "parser", t("2026-09-23T18:00:00+09:00"));
        w.task = "old task".into();
        w.attention = vec!["old".into()];
        w.changed = vec!["old.rs".into()];
        w.branch = Some("old-branch".into());
        w
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    const NOW: &str = "2026-09-23T19:00:00+09:00";

    #[test]
    fn list_values_replace_by_default() {
        let input = HandoffInput {
            attention: Some(strings(&["new", "second"])),
            ..Default::default()
        };
        let updated = apply_handoff(&worker(), &input, &RepoInfo::default(), t(NOW));
        assert_eq!(updated.attention, ["new", "second"]);
    }

    #[test]
    fn append_adds_without_duplicates() {
        let input = HandoffInput {
            attention: Some(strings(&["old", "new", "new"])),
            append: true,
            ..Default::default()
        };
        let updated = apply_handoff(&worker(), &input, &RepoInfo::default(), t(NOW));
        assert_eq!(updated.attention, ["old", "new"]);
    }

    #[test]
    fn changed_defaults_to_the_repository_changes() {
        let repo = RepoInfo {
            branch: None,
            commit: None,
            changed_files: strings(&["src/a.rs"]),
        };
        let updated = apply_handoff(&worker(), &HandoffInput::default(), &repo, t(NOW));
        assert_eq!(updated.changed, ["src/a.rs"]);

        let untouched = apply_handoff(
            &worker(),
            &HandoffInput::default(),
            &RepoInfo::default(),
            t(NOW),
        );
        assert_eq!(untouched.changed, ["old.rs"]);

        let explicit = HandoffInput {
            changed: Some(strings(&["given.rs"])),
            ..Default::default()
        };
        assert_eq!(
            apply_handoff(&worker(), &explicit, &repo, t(NOW)).changed,
            ["given.rs"]
        );
    }

    #[test]
    fn repository_changes_are_capped() {
        let files: Vec<String> = (0..60).map(|i| format!("src/f{i:02}.rs")).collect();
        let repo = RepoInfo {
            changed_files: files,
            ..Default::default()
        };
        let updated = apply_handoff(&worker(), &HandoffInput::default(), &repo, t(NOW));
        assert_eq!(updated.changed.len(), 51);
        assert_eq!(updated.changed[49], "src/f49.rs");
        assert_eq!(updated.changed[50], "... and 10 more files");
    }

    #[test]
    fn nothing_given_updates_time_and_repository_only() {
        let repo = RepoInfo {
            branch: Some("feat/parser".into()),
            commit: Some("abc1234".into()),
            changed_files: vec![],
        };
        let before = worker();
        let updated = apply_handoff(&before, &HandoffInput::default(), &repo, t(NOW));
        assert_eq!(updated.last_updated, t(NOW));
        assert_eq!(updated.branch.as_deref(), Some("feat/parser"));
        assert_eq!(updated.commit.as_deref(), Some("abc1234"));
        assert_eq!(updated.task, before.task);
        assert_eq!(updated.attention, before.attention);
        assert_eq!(updated.status, before.status);
    }

    #[test]
    fn scalars_and_status_are_replaced() {
        let input = HandoffInput {
            task: Some("new task".into()),
            summary: Some("done it".into()),
            status: Some(WorkerStatus::Done),
            ..Default::default()
        };
        let updated = apply_handoff(&worker(), &input, &RepoInfo::default(), t(NOW));
        assert_eq!(updated.task, "new task");
        assert_eq!(updated.summary, "done it");
        assert_eq!(updated.status, WorkerStatus::Done);
        // A detached or non-git project keeps the last known branch.
        assert_eq!(updated.branch.as_deref(), Some("old-branch"));
    }
}
