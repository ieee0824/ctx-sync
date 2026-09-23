mod common;

use common::{context_repo, ctx_sync, stdout};
use ctx_sync_core::model::{Worker, WorkerStatus};
use ctx_sync_testutil::{TestRemote, TestWorker, git};
use predicates::str::contains;

fn attached_worker(remote: &TestRemote) -> TestWorker {
    let worker = TestWorker::new(remote);
    common::attach(&worker);
    worker
}

fn seeded_remote() -> TestRemote {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    remote
}

/// Short id printed as `id: <short>`.
fn printed_id(output: &std::process::Output) -> String {
    stdout(output)
        .lines()
        .find_map(|l| l.strip_prefix("id: "))
        .expect("id line")
        .to_string()
}

#[test]
fn registers_and_commits_the_worker_file() {
    let remote = seeded_remote();
    let worker = attached_worker(&remote);
    let output = ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .success()
        .stdout(contains("Registered worker"))
        .stdout(contains("name: parser"))
        .get_output()
        .clone();
    let id = printed_id(&output);
    assert_eq!(id.len(), 8);

    let repo = context_repo(&worker);
    let file = repo.join(format!("40-worker-{id}.md"));
    let parsed = Worker::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(parsed.name, "parser");
    assert_eq!(parsed.status, WorkerStatus::Working);
    assert_eq!(parsed.branch.as_deref(), Some("main"));
    let log = git(&repo, &worker.git_env(), &["log", "-1", "--format=%s"]);
    assert_eq!(log.trim(), "ctx-sync: register parser");
    // Registering does not sync.
    assert!(remote.read_file(&format!("40-worker-{id}.md")).is_none());
    // The identity is not written into the project.
    let status = worker.git(&["status", "--porcelain", "--untracked-files=all"]);
    assert_eq!(status.trim(), "?? .ctx-sync.toml");
}

#[test]
fn registering_again_keeps_the_identity() {
    let remote = seeded_remote();
    let worker = attached_worker(&remote);
    let first = ctx_sync(&worker)
        .args(["register", "parser"])
        .output()
        .unwrap();
    let second = ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .success()
        .stdout(contains("Already registered"))
        .get_output()
        .clone();
    assert_eq!(printed_id(&first), printed_id(&second));
}

#[test]
fn force_replaces_the_identity_and_abandons_the_old_worker() {
    let remote = seeded_remote();
    let worker = attached_worker(&remote);
    let first = ctx_sync(&worker)
        .args(["register", "parser"])
        .output()
        .unwrap();
    let second = ctx_sync(&worker)
        .args(["register", "other", "--force"])
        .assert()
        .success()
        .stdout(contains("Registered worker"))
        .stdout(contains("name: other"))
        .get_output()
        .clone();
    let (old, new) = (printed_id(&first), printed_id(&second));
    assert_ne!(old, new);
    let repo = context_repo(&worker);
    let old_worker =
        Worker::parse(&std::fs::read_to_string(repo.join(format!("40-worker-{old}.md"))).unwrap())
            .unwrap();
    assert_eq!(old_worker.status, WorkerStatus::Abandoned);
    assert!(repo.join(format!("40-worker-{new}.md")).is_file());
}

#[test]
fn invalid_name_exits_with_4() {
    let remote = seeded_remote();
    let worker = attached_worker(&remote);
    ctx_sync(&worker)
        .args(["register", "a b"])
        .assert()
        .code(4)
        .stderr(contains("invalid worker name"));
}

#[test]
fn unattached_project_exits_with_4() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .code(4);
}
