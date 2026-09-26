mod common;

use std::path::Path;

use ctx_sync_core::state::{ConflictRecord, WorkerIdentity};
use ctx_sync_core::store::SyncState;
use ctx_sync_core::view::{ViewOptions, build_status_view, render_status_text};
use uuid::Uuid;

const REPO: &str = "/state/projects/6f1c2b7e-0000-4000-8000-000000000001/context-repo";

fn sync_state() -> SyncState {
    SyncState {
        branch: "main".into(),
        revision: Some("abc1234".into()),
        ahead: 0,
        behind: 0,
        dirty: false,
        conflict: None,
    }
}

fn you() -> WorkerIdentity {
    WorkerIdentity {
        id: Uuid::parse_str(common::PARSER_ID).unwrap(),
        name: "parser".into(),
        registered_at: common::time("2026-09-23T18:00:00+09:00"),
        worktree: "/work/parser".into(),
    }
}

fn opts() -> ViewOptions {
    ViewOptions::new(common::time("2026-09-23T19:00:00+09:00"))
}

fn render(sync: SyncState, you: Option<&WorkerIdentity>) -> String {
    let view = build_status_view(
        &common::snapshot(),
        "0123456789abcdef",
        Path::new(REPO),
        sync,
        you,
        &opts(),
    );
    render_status_text(&view)
}

#[test]
fn renders_the_expected_text() {
    let expected = format!(
        "Project: demo ({project})

Remote:
  gist: 0123456789abcdef
  revision: abc1234
  branch: main
  ahead: 0, behind: 0
  uncommitted changes: no
  context repo: {REPO}

You: parser (11111111)

Workers:

gist (22222222)
  done
  branch: feat/gist
  task: Gist backend

parser (11111111)
  working
  branch: feat/parser
  task: Parser implementation

ui (33333333)
  blocked
  task: UI
",
        project = common::PROJECT_ID
    );
    assert_eq!(render(sync_state(), Some(&you())), expected);
}

#[test]
fn shows_a_recorded_conflict_with_resolution_steps() {
    let mut sync = sync_state();
    sync.conflict = Some(ConflictRecord {
        detected_at: common::time("2026-09-23T18:30:00+09:00"),
        files: vec!["20-architecture.md".into()],
        local_head: "a".repeat(40),
        remote_head: "b".repeat(40),
    });
    let text = render(sync, Some(&you()));
    let expected = format!(
        "  context repo: {REPO}

CONFLICT (detected at 2026-09-23T18:30:00+09:00):
  20-architecture.md

  Resolve manually in the context repo:
    cd {REPO}
    git rebase origin/main   # fix the conflicts, then `git rebase --continue`
    ctx-sync sync

You: parser (11111111)"
    );
    assert!(text.contains(&expected), "{text}");
}

#[test]
fn unpushed_commits_suggest_sync() {
    let mut sync = sync_state();
    sync.ahead = 1;
    sync.behind = 2;
    sync.dirty = true;
    let text = render(sync, Some(&you()));
    assert!(
        text.contains("  ahead: 1, behind: 2 (run `ctx-sync sync`)\n"),
        "{text}"
    );
    assert!(text.contains("  uncommitted changes: yes\n"));
}

#[test]
fn unregistered_worktree_and_warnings() {
    let mut snapshot = common::snapshot();
    snapshot.warnings = vec!["skipped 40-worker-x.md: missing ID".into()];
    let view = build_status_view(&snapshot, "g", Path::new(REPO), sync_state(), None, &opts());
    let text = render_status_text(&view);
    assert!(text.contains("You: not registered (run `ctx-sync register <name>`)"));
    assert!(
        text.ends_with("Warnings:\n  skipped 40-worker-x.md: missing ID\n"),
        "{text}"
    );
}

#[test]
fn view_serializes_to_json() {
    let view = build_status_view(
        &common::snapshot(),
        "g",
        Path::new(REPO),
        sync_state(),
        Some(&you()),
        &opts(),
    );
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["workers"].as_array().unwrap().len(), 3);
    assert_eq!(json["sync"]["branch"], "main");
    assert_eq!(json["you"][0], "parser");
}

#[test]
fn stale_workers_are_marked_in_text_and_json() {
    let opts = ViewOptions::new(common::time("2026-09-26T19:00:00+09:00"));
    let view = build_status_view(
        &common::snapshot(),
        "g",
        Path::new(REPO),
        sync_state(),
        Some(&you()),
        &opts,
    );
    let text = render_status_text(&view);
    assert!(text.contains("  working (stale, 3d)"), "{text}");
    assert!(text.contains("  blocked (stale, 3d)"), "{text}");
    assert!(text.contains("gist (22222222)\n  done\n"), "{text}");

    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["workers"][1]["stale"], true);
    assert_eq!(json["workers"][1]["age"], "3d");
    assert_eq!(json["workers"][0]["stale"], false);
}
