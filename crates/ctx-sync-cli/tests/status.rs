mod common;

use common::{context_repo, ctx_sync};
use ctx_sync_testutil::{SEED_PROJECT_ID, TestRemote, TestWorker};
use predicates::str::contains;

fn seeded_remote() -> TestRemote {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    remote
}

fn registered_worker(remote: &TestRemote, name: &str) -> TestWorker {
    let worker = TestWorker::new(remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["register", name])
        .assert()
        .success();
    worker
}

fn edit_architecture(worker: &TestWorker, component: &str) {
    let path = context_repo(worker).join("20-architecture.md");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("- core\n", &format!("- {component}\n"))).unwrap();
}

#[test]
fn shows_project_remote_and_workers() {
    let remote = seeded_remote();
    let worker = registered_worker(&remote, "parser");
    let repo = context_repo(&worker);
    ctx_sync(&worker)
        .arg("status")
        .assert()
        .success()
        .stdout(contains(format!("Project: demo ({SEED_PROJECT_ID})")))
        .stdout(contains("  gist: testgist01"))
        .stdout(contains("  branch: main"))
        // register commits locally without pushing
        .stdout(contains("  ahead: 1, behind: 0 (run `ctx-sync sync`)"))
        .stdout(contains(format!("  context repo: {}", repo.display())))
        .stdout(contains("You: parser ("))
        .stdout(contains("Workers:\n\nparser ("));
}

#[test]
fn unregistered_worktree_is_reported() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("You: not registered"))
        .stdout(contains("Workers:\n\n  (none)"));
}

#[test]
fn shows_a_conflict_after_a_failed_sync() {
    let remote = seeded_remote();
    let a = registered_worker(&remote, "a");
    let b = registered_worker(&remote, "b");
    edit_architecture(&a, "core-a");
    edit_architecture(&b, "core-b");
    ctx_sync(&a).arg("sync").assert().success();
    ctx_sync(&b).arg("sync").assert().code(2);
    ctx_sync(&b)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("CONFLICT (detected at "))
        .stdout(contains("  20-architecture.md"))
        .stdout(contains("ctx-sync conflict resolve --keep-local"));
}

#[test]
fn unattached_project_exits_with_4() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker).arg("status").assert().code(4);
}
