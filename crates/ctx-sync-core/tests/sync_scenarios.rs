//! Several workers syncing through one remote (a local bare repository
//! standing in for the Gist).

use std::path::Path;

use chrono::{DateTime, NaiveDate};
use ctx_sync_core::Error;
use ctx_sync_core::git::Git;
use ctx_sync_core::model::{Decision, DecisionId, DecisionStatus};
use ctx_sync_core::state::{ConflictRecord, Protocol, StateRoot};
use ctx_sync_core::store::{ContextStore, GistStore, RemoteSpec};
use ctx_sync_testutil::TestRemote;
use tempfile::TempDir;
use uuid::Uuid;

const ARCHITECTURE: &str = "20-architecture.md";

struct Worker {
    _home: TempDir,
    store: GistStore,
    env: Vec<(String, String)>,
}

impl Worker {
    fn new(remote: &TestRemote) -> Self {
        let home = tempfile::tempdir().unwrap();
        let state = StateRoot::new(home.path()).project(&Uuid::nil());
        let store = GistStore::new(
            &state,
            RemoteSpec::new("x", Protocol::Https).with_url_override(Some(remote.url())),
        )
        .with_git_env(remote.git_env());
        store.ensure().unwrap();
        Self {
            _home: home,
            store,
            env: remote.git_env(),
        }
    }

    fn git(&self) -> Git {
        Git::new(self.store.repo_dir()).with_env(&self.env)
    }

    fn edit_architecture(&self, component: &str) {
        let text = self.store.read_file(ARCHITECTURE).unwrap().unwrap();
        let text = text.replace("- core\n", &format!("- {component}\n"));
        self.store.write_file(ARCHITECTURE, &text).unwrap();
    }
}

fn now() -> DateTime<chrono::FixedOffset> {
    DateTime::parse_from_rfc3339("2026-09-23T18:30:00+09:00").unwrap()
}

fn seeded_remote() -> TestRemote {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    remote
}

fn exists(repo: &Path, name: &str) -> bool {
    repo.join(".git").join(name).exists()
}

#[test]
fn workers_updating_different_files_both_succeed() {
    let remote = seeded_remote();
    let a = Worker::new(&remote);
    let b = Worker::new(&remote);

    a.store.write_file("40-worker-aaaaaaaa.md", "a").unwrap();
    a.store.commit("worker a").unwrap();
    // B's change is left uncommitted: sync commits it.
    b.store.write_file("40-worker-bbbbbbbb.md", "b").unwrap();

    assert!(a.store.sync(now()).unwrap().pushed);
    let outcome = b.store.sync(now()).unwrap();
    assert!(outcome.pushed);
    assert!(outcome.committed.is_some());
    assert_eq!(outcome.attempts, 1);

    assert_eq!(
        remote.read_file("40-worker-aaaaaaaa.md").as_deref(),
        Some("a")
    );
    assert_eq!(
        remote.read_file("40-worker-bbbbbbbb.md").as_deref(),
        Some("b")
    );
    assert_eq!(remote.log()[0], "ctx-sync: sync");
}

#[test]
fn sync_without_changes_pushes_nothing() {
    let remote = seeded_remote();
    let a = Worker::new(&remote);
    let before = remote.log().len();
    let outcome = a.store.sync(now()).unwrap();
    assert!(!outcome.pushed);
    assert!(outcome.committed.is_none());
    assert_eq!(remote.log().len(), before);
}

#[test]
fn sync_picks_up_remote_changes() {
    let remote = seeded_remote();
    let a = Worker::new(&remote);
    remote.push_file("40-worker-cccccccc.md", "c", "other worker");
    a.store.sync(now()).unwrap();
    assert_eq!(
        a.store
            .read_file("40-worker-cccccccc.md")
            .unwrap()
            .as_deref(),
        Some("c")
    );
}

#[test]
fn same_architecture_line_is_a_conflict() {
    let remote = seeded_remote();
    let a = Worker::new(&remote);
    let b = Worker::new(&remote);

    a.edit_architecture("core-a");
    a.store.commit("a edits architecture").unwrap();
    b.edit_architecture("core-b");
    b.store.commit("b edits architecture").unwrap();

    a.store.sync(now()).unwrap();
    let err = b.store.sync(now()).unwrap_err();
    match &err {
        Error::ContextConflict { files } => assert_eq!(files, &[ARCHITECTURE]),
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(err.exit_code(), 2);
    assert_eq!(
        err.to_string(),
        "Context conflict detected:\n\n20-architecture.md"
    );

    // Nothing was overwritten on the remote.
    assert!(
        remote
            .read_file(ARCHITECTURE)
            .unwrap()
            .contains("- core-a\n")
    );

    // The conflict is recorded and the repository is not left mid-rebase.
    let record = ConflictRecord::load(b.store.conflict_path())
        .unwrap()
        .unwrap();
    assert_eq!(record.files, [ARCHITECTURE]);
    assert_eq!(record.detected_at, now());
    let repo = b.store.repo_dir();
    assert!(!exists(repo, "rebase-merge") && !exists(repo, "rebase-apply"));
    let state = b.store.sync_state().unwrap();
    assert_eq!(state.conflict, Some(record));
    assert_eq!((state.ahead, state.behind), (1, 1));
    assert!(!state.dirty);
    assert!(
        b.store
            .read_file(ARCHITECTURE)
            .unwrap()
            .unwrap()
            .contains("- core-b\n")
    );
}

#[test]
fn conflict_can_be_resolved_manually() {
    let remote = seeded_remote();
    let a = Worker::new(&remote);
    let b = Worker::new(&remote);
    a.edit_architecture("core-a");
    a.store.commit("a").unwrap();
    b.edit_architecture("core-b");
    b.store.commit("b").unwrap();
    a.store.sync(now()).unwrap();
    assert!(b.store.sync(now()).is_err());

    // The documented manual resolution.
    let git = b.git();
    assert!(!git.run(&["rebase", "origin/main"]).unwrap().success);
    let resolved = remote
        .read_file(ARCHITECTURE)
        .unwrap()
        .replace("- core-a\n", "- core-a\n- core-b\n");
    std::fs::write(b.store.repo_dir().join(ARCHITECTURE), &resolved).unwrap();
    git.run_checked(&["add", ARCHITECTURE]).unwrap();
    git.run_checked(&["-c", "core.editor=true", "rebase", "--continue"])
        .unwrap();

    assert!(b.store.sync(now()).unwrap().pushed);
    assert!(
        ConflictRecord::load(b.store.conflict_path())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        remote.read_file(ARCHITECTURE).as_deref(),
        Some(resolved.as_str())
    );
}

#[test]
fn duplicate_decision_numbers_are_warned_about() {
    let remote = seeded_remote();
    let a = Worker::new(&remote);
    let b = Worker::new(&remote);
    let date = NaiveDate::from_ymd_opt(2026, 9, 23).unwrap();
    for (worker, slug) in [(&a, "use-git"), (&b, "use-gist")] {
        let decision = Decision {
            id: DecisionId::new(date, 1, slug),
            title: slug.into(),
            status: DecisionStatus::Accepted,
            date,
            author: None,
            supersedes: vec![],
            context: String::new(),
            decision: slug.into(),
            reason: String::new(),
            consequences: String::new(),
            extra_sections: vec![],
        };
        worker
            .store
            .write_file(&decision.file_name(), &decision.render())
            .unwrap();
    }
    assert!(a.store.sync(now()).unwrap().warnings.is_empty());
    let outcome = b.store.sync(now()).unwrap();
    assert!(outcome.pushed);
    assert_eq!(
        outcome.warnings,
        ["duplicate decision number 20260923-001: 20260923-001-use-gist, 20260923-001-use-git"]
    );
}

#[test]
fn missing_remote_is_a_remote_error() {
    let home = tempfile::tempdir().unwrap();
    let state = StateRoot::new(home.path()).project(&Uuid::nil());
    let store = GistStore::new(
        &state,
        RemoteSpec::new("x", Protocol::Https)
            .with_url_override(Some("/nonexistent/ctx-sync-remote.git".into())),
    )
    .with_git_env(ctx_sync_testutil::git_env(&home.path().join("config")));
    assert_eq!(store.ensure().unwrap_err().exit_code(), 5);
    assert_eq!(store.sync(now()).unwrap_err().exit_code(), 5);
}
