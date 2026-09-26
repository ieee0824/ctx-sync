use chrono::{DateTime, FixedOffset};
use ctx_sync_core::config::ProjectConfig;
use ctx_sync_core::ops::{Workspace, conflict_show};
use ctx_sync_core::state::{ConflictRecord, LocalProject, Protocol, StateRoot};
use ctx_sync_core::store::{ContextStore, GistStore, RemoteSpec};
use ctx_sync_core::view::render_conflict_text;
use ctx_sync_testutil::TestRemote;
use tempfile::TempDir;
use uuid::Uuid;

const ARCHITECTURE: &str = "20-architecture.md";

struct Fixture {
    _home: TempDir,
    ws: Workspace,
}

fn now() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339("2026-09-23T18:30:00+09:00").unwrap()
}

fn fixture(remote: &TestRemote) -> Fixture {
    let home = tempfile::tempdir().unwrap();
    let project_state = StateRoot::new(home.path().join("state")).project(&Uuid::nil());
    let store = GistStore::new(
        &project_state,
        RemoteSpec::new("x", Protocol::Https).with_url_override(Some(remote.url())),
    )
    .with_git_env(remote.git_env());
    store.ensure().unwrap();
    let ws = Workspace {
        root: home.path().join("project"),
        config: ProjectConfig::new_gist("x"),
        project_id: Uuid::nil(),
        project_state,
        local: LocalProject {
            gist_id: "x".into(),
            protocol: Protocol::Https,
            attached_at: now(),
        },
        store,
    };
    Fixture { _home: home, ws }
}

fn edit_architecture(ws: &Workspace, component: &str) {
    let text = ws.store.read_file(ARCHITECTURE).unwrap().unwrap();
    ws.store
        .write_file(
            ARCHITECTURE,
            &text.replace("- core\n", &format!("- {component}\n")),
        )
        .unwrap();
    ws.store.commit(component).unwrap();
}

#[test]
fn shows_both_sides_and_the_diff_of_a_recorded_conflict() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = fixture(&remote);
    let b = fixture(&remote);
    edit_architecture(&a.ws, "core-a");
    edit_architecture(&b.ws, "core-b");
    a.ws.store.sync(now()).unwrap();
    assert_eq!(b.ws.store.sync(now()).unwrap_err().exit_code(), 2);

    let view = conflict_show(&b.ws).unwrap();
    assert!(view.record.is_some());
    assert_eq!(view.files.len(), 1);
    assert_eq!(view.files[0].file, ARCHITECTURE);
    assert!(view.files[0].remote.as_ref().unwrap().contains("- core-a"));
    assert!(view.files[0].local.as_ref().unwrap().contains("- core-b"));
    assert!(view.files[0].diff.contains("- core-a"));
    assert!(view.files[0].diff.contains("- core-b"));
    let rendered = render_conflict_text(&view);
    assert!(rendered.contains("## 20-architecture.md"));
    assert!(rendered.contains("remote: "));
}

#[test]
fn reports_no_conflict_when_no_record_exists() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = fixture(&remote);
    let view = conflict_show(&a.ws).unwrap();
    assert!(view.record.is_none());
    assert!(view.files.is_empty());
    assert_eq!(
        render_conflict_text(&view),
        "No context conflict recorded.\n"
    );
}

#[test]
fn a_missing_commit_has_actionable_guidance() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = fixture(&remote);
    let record = ConflictRecord {
        detected_at: now(),
        files: vec![ARCHITECTURE.into()],
        local_head: "a".repeat(40),
        remote_head: "b".repeat(40),
    };
    record.save(a.ws.store.conflict_path()).unwrap();
    let error = conflict_show(&a.ws).unwrap_err();
    assert!(error.to_string().contains("run `ctx-sync pull`"));
}
