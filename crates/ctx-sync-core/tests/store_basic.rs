use std::path::Path;

use ctx_sync_core::Error;
use ctx_sync_core::state::{Protocol, StateRoot};
use ctx_sync_core::store::{ContextStore, GistStore, RemoteSpec};
use ctx_sync_testutil::{TestRemote, git, git_env};
use uuid::Uuid;

fn store_for(url: String, home: &Path, env: Vec<(String, String)>) -> GistStore {
    let state = StateRoot::new(home).project(&Uuid::nil());
    GistStore::new(
        &state,
        RemoteSpec::new("x", Protocol::Https).with_url_override(Some(url)),
    )
    .with_git_env(env)
}

fn store(remote: &TestRemote, home: &Path) -> GistStore {
    store_for(remote.url(), home, remote.git_env())
}

#[test]
fn ensure_clones_the_remote() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();
    assert!(store.repo_dir().join(".git").is_dir());
    assert!(store.read_file("README.md").unwrap().is_some());
    assert!(store.read_file("missing.md").unwrap().is_none());
    // A second ensure keeps the existing clone.
    store.ensure().unwrap();
    assert_eq!(store.branch().unwrap(), "main");
}

#[test]
fn commit_returns_none_without_changes() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();
    let before = store.revision().unwrap().unwrap();

    store.write_file("a.md", "x").unwrap();
    let committed = store.commit("add a").unwrap().unwrap();
    assert_ne!(committed, before);
    assert_eq!(store.revision().unwrap(), Some(committed));
    assert!(store.commit("nothing").unwrap().is_none());
    assert_eq!(store.read_file("a.md").unwrap().as_deref(), Some("x"));
}

#[test]
fn removes_a_committed_file_and_records_the_deletion() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();
    store.write_file("obsolete.md", "old").unwrap();
    store.commit("add obsolete file").unwrap();

    assert!(store.remove_file("obsolete.md").unwrap());
    assert!(store.commit("remove obsolete file").unwrap().is_some());
    assert!(store.read_file("obsolete.md").unwrap().is_none());
    assert!(!store.remove_file("obsolete.md").unwrap());
}

#[test]
fn rejects_non_flat_file_names() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();
    for name in ["../a.md", "dir/a.md", "dir\\a.md", ".hidden", ""] {
        assert!(store.write_file(name, "x").is_err(), "{name}");
        assert!(store.read_file(name).is_err(), "{name}");
        assert!(store.remove_file(name).is_err(), "{name}");
    }
}

#[test]
fn snapshot_without_meta_is_invalid_config() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();
    let err = store.snapshot().unwrap_err();
    assert!(matches!(err, Error::InvalidConfig(_)), "{err:?}");
}

#[test]
fn snapshot_reads_seeded_context() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    store.ensure().unwrap();
    assert_eq!(store.snapshot().unwrap().meta.project_name, "demo");
}

#[test]
fn missing_remote_is_a_remote_error() {
    let home = tempfile::tempdir().unwrap();
    let env = git_env(&home.path().join("config"));
    let store = store_for("/nonexistent/ctx-sync-remote.git".into(), home.path(), env);
    let err = store.ensure().unwrap_err();
    assert_eq!(err.exit_code(), 5, "{err}");
}

#[test]
fn existing_non_git_directory_is_an_error() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    let store = store(&remote, home.path());
    std::fs::create_dir_all(store.repo_dir()).unwrap();
    std::fs::write(store.repo_dir().join("stray.txt"), "x").unwrap();
    let err = store.ensure().unwrap_err();
    assert!(err.to_string().contains("not a git repository"), "{err}");
}

#[test]
fn commit_works_without_git_identity() {
    let remote = TestRemote::new();
    let home = tempfile::tempdir().unwrap();
    // A git config without user.name / user.email, like a fresh sandbox.
    let empty_config = home.path().join("empty-gitconfig");
    std::fs::write(&empty_config, "[protocol \"file\"]\n    allow = always\n").unwrap();
    let env = vec![
        (
            "GIT_CONFIG_GLOBAL".to_string(),
            empty_config.to_string_lossy().into_owned(),
        ),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ];
    let store = store_for(remote.url(), home.path(), env.clone());
    store.ensure().unwrap();
    store.write_file("a.md", "x").unwrap();
    assert!(store.commit("add a").unwrap().is_some());
    let author = git(store.repo_dir(), &env, &["log", "-1", "--format=%an <%ae>"]);
    assert_eq!(author.trim(), "ctx-sync <ctx-sync@localhost>");
}
