use std::path::Path;

use ctx_sync_core::state::{Protocol, StateRoot};
use ctx_sync_core::store::{ContextStore, GistStore, PullOutcome, RemoteSpec};
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
fn pull_without_changes_is_up_to_date() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    assert_eq!(store.pull().unwrap(), PullOutcome::UpToDate);
}

#[test]
fn pull_clones_when_needed() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.pull().unwrap();
    assert!(store.read_file("README.md").unwrap().is_some());
}

#[test]
fn pull_fast_forwards_remote_changes() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();
    let before = store.revision().unwrap().unwrap();

    remote.push_file("x.md", "hello", "add x");
    match store.pull().unwrap() {
        PullOutcome::FastForwarded { from, to } => {
            assert_eq!(from, before);
            assert_ne!(to, before);
            assert_eq!(store.revision().unwrap(), Some(to));
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(store.read_file("x.md").unwrap().as_deref(), Some("hello"));
    assert_eq!(store.pull().unwrap(), PullOutcome::UpToDate);
}

#[test]
fn pull_leaves_diverged_history_alone() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();

    store.write_file("local.md", "local").unwrap();
    let local = store.commit("local change").unwrap().unwrap();
    remote.push_file("remote.md", "remote", "remote change");

    assert_eq!(
        store.pull().unwrap(),
        PullOutcome::Diverged {
            ahead: 1,
            behind: 1
        }
    );
    assert_eq!(store.revision().unwrap(), Some(local));
    assert!(store.read_file("remote.md").unwrap().is_none());
}
