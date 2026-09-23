use std::path::Path;

use chrono::DateTime;
use ctx_sync_core::state::{ConflictRecord, Protocol, StateRoot};
use ctx_sync_core::store::{ContextStore, GistStore, RemoteSpec};
use ctx_sync_testutil::TestRemote;
use uuid::Uuid;

fn store(remote: &TestRemote, home: &Path) -> GistStore {
    let state = StateRoot::new(home).project(&Uuid::nil());
    GistStore::new(
        &state,
        RemoteSpec::new("x", Protocol::Https).with_url_override(Some(remote.url())),
    )
    .with_git_env(remote.git_env())
}

#[test]
fn fresh_clone_is_clean_and_in_sync() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();
    let state = store.sync_state().unwrap();
    assert_eq!(state.branch, "main");
    assert_eq!(state.revision, store.revision().unwrap());
    assert_eq!((state.ahead, state.behind), (0, 0));
    assert!(!state.dirty);
    assert!(state.conflict.is_none());
}

#[test]
fn reports_uncommitted_and_unpushed_changes() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();

    store.write_file("a.md", "x").unwrap();
    assert!(store.sync_state().unwrap().dirty);

    store.commit("add a").unwrap();
    let state = store.sync_state().unwrap();
    assert!(!state.dirty);
    assert_eq!((state.ahead, state.behind), (1, 0));
}

#[test]
fn behind_reflects_the_last_fetch_only() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();
    remote.push_file("b.md", "y", "add b");
    // No fetch yet: the remote change is not visible.
    assert_eq!(store.sync_state().unwrap().behind, 0);
    let git = ctx_sync_core::git::Git::new(store.repo_dir()).with_env(&remote.git_env());
    git.run_checked(&["fetch", "-q", "origin"]).unwrap();
    assert_eq!(store.sync_state().unwrap().behind, 1);
}

#[test]
fn reports_a_recorded_conflict() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();
    let record = ConflictRecord {
        detected_at: DateTime::parse_from_rfc3339("2026-09-23T18:30:00+09:00").unwrap(),
        files: vec!["20-architecture.md".into()],
        local_head: "a".repeat(40),
        remote_head: "b".repeat(40),
    };
    record.save(store.conflict_path()).unwrap();
    assert_eq!(store.sync_state().unwrap().conflict, Some(record));
}
