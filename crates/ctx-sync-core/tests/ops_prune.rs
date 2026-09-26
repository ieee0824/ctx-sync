use chrono::{DateTime, FixedOffset, TimeDelta};
use ctx_sync_core::config::ProjectConfig;
use ctx_sync_core::model::{Worker, WorkerStatus};
use ctx_sync_core::ops::{PruneOptions, PruneReason, Workspace, prune_workers};
use ctx_sync_core::state::{LocalProject, Protocol, StateRoot, WorkerIdentity};
use ctx_sync_core::store::{ContextStore, GistStore, RemoteSpec};
use ctx_sync_testutil::{TestRemote, TestWorker, git};
use uuid::Uuid;

fn now() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339("2026-09-26T18:00:00+09:00").unwrap()
}

fn id(number: u32) -> Uuid {
    Uuid::parse_str(&format!("{number:08x}-0000-4000-8000-000000000001")).unwrap()
}

struct Fixture {
    remote: TestRemote,
    worker: TestWorker,
    ws: Workspace,
}

impl Fixture {
    fn new() -> Self {
        let remote = TestRemote::new();
        remote.seed_context("demo");
        let worker = TestWorker::new(&remote);
        let project_state = StateRoot::new(worker.state_home()).project(&Uuid::nil());
        let store = GistStore::new(
            &project_state,
            RemoteSpec::new("x", Protocol::Https).with_url_override(Some(remote.url())),
        )
        .with_git_env(remote.git_env());
        store.ensure().unwrap();
        let ws = Workspace {
            root: worker.project().to_path_buf(),
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
        Self { remote, worker, ws }
    }

    fn seed_workers(&self) {
        for (number, name, status, age) in [
            (1, "done", WorkerStatus::Done, 1),
            (2, "abandoned", WorkerStatus::Abandoned, 1),
            (3, "recent", WorkerStatus::Working, 1),
            (4, "stale", WorkerStatus::Working, 72),
        ] {
            let mut worker = Worker::new(id(number), name, now() - TimeDelta::hours(age));
            worker.status = status;
            self.ws
                .store
                .write_file(&worker.file_name(), &worker.render())
                .unwrap();
        }
        self.ws.store.commit("seed workers").unwrap();
    }

    fn options(&self) -> PruneOptions {
        PruneOptions {
            stale_after: None,
            dry_run: false,
            sync: false,
            now: now(),
        }
    }
}

#[test]
fn default_prunes_finished_workers_and_commits_deletions() {
    let fixture = Fixture::new();
    fixture.seed_workers();
    let outcome = prune_workers(&fixture.ws, fixture.options()).unwrap();
    assert_eq!(outcome.removed.len(), 2);
    assert_eq!(outcome.removed[0].name, "abandoned");
    assert_eq!(outcome.removed[0].reason, PruneReason::Abandoned);
    assert_eq!(outcome.removed[1].name, "done");
    assert_eq!(outcome.removed[1].reason, PruneReason::Done);
    assert!(outcome.committed.is_some());
    assert!(outcome.sync.is_none());
    assert!(
        fixture
            .ws
            .store
            .read_file(&Worker::file_name_for(&id(1)))
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .ws
            .store
            .read_file(&Worker::file_name_for(&id(2)))
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .ws
            .store
            .read_file(&Worker::file_name_for(&id(3)))
            .unwrap()
            .is_some()
    );
    assert!(
        fixture
            .ws
            .store
            .read_file(&Worker::file_name_for(&id(4)))
            .unwrap()
            .is_some()
    );
    assert_eq!(
        git(
            fixture.ws.store.repo_dir(),
            &fixture.worker.git_env(),
            &["log", "-1", "--format=%s"]
        )
        .trim(),
        "ctx-sync: prune 2 worker(s)"
    );
}

#[test]
fn stale_threshold_and_identity_are_respected() {
    let fixture = Fixture::new();
    fixture.seed_workers();
    WorkerIdentity {
        id: id(1),
        name: "done".into(),
        registered_at: now(),
        worktree: fixture.worker.project().to_path_buf(),
    }
    .save(&fixture.ws.identity_path().unwrap())
    .unwrap();
    let mut options = fixture.options();
    options.stale_after = Some(TimeDelta::days(1));
    let outcome = prune_workers(&fixture.ws, options).unwrap();
    assert_eq!(
        outcome
            .removed
            .iter()
            .map(|worker| worker.name.as_str())
            .collect::<Vec<_>>(),
        ["abandoned", "stale"]
    );
    assert_eq!(outcome.removed[1].reason, PruneReason::Stale);
    assert!(
        fixture
            .ws
            .store
            .read_file(&Worker::file_name_for(&id(1)))
            .unwrap()
            .is_some()
    );
}

#[test]
fn dry_run_preserves_files_and_empty_selection_needs_no_commit() {
    let fixture = Fixture::new();
    fixture.seed_workers();
    let mut options = fixture.options();
    options.dry_run = true;
    options.sync = true;
    let outcome = prune_workers(&fixture.ws, options).unwrap();
    assert_eq!(outcome.removed.len(), 2);
    assert!(outcome.committed.is_none());
    assert!(outcome.sync.is_none());
    assert!(
        fixture
            .ws
            .store
            .read_file(&Worker::file_name_for(&id(1)))
            .unwrap()
            .is_some()
    );

    let empty = Fixture::new();
    let outcome = prune_workers(&empty.ws, empty.options()).unwrap();
    assert!(outcome.removed.is_empty());
    assert!(outcome.committed.is_none());
}

#[test]
fn sync_removes_worker_files_from_remote() {
    let fixture = Fixture::new();
    fixture.seed_workers();
    let mut options = fixture.options();
    options.sync = true;
    let outcome = prune_workers(&fixture.ws, options).unwrap();
    assert!(outcome.sync.unwrap().pushed);
    assert!(
        fixture
            .remote
            .read_file(&Worker::file_name_for(&id(1)))
            .is_none()
    );
    assert!(
        fixture
            .remote
            .read_file(&Worker::file_name_for(&id(3)))
            .is_some()
    );
}
