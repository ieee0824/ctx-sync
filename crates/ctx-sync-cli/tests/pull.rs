mod common;

use common::{context_repo, ctx_sync};
use ctx_sync_testutil::{TestRemote, TestWorker};
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

#[test]
fn pull_right_after_attach_is_up_to_date() {
    let remote = seeded_remote();
    let worker = attached_worker(&remote);
    ctx_sync(&worker)
        .arg("pull")
        .assert()
        .success()
        .stdout(contains("Already up to date ("));
}

#[test]
fn pull_fast_forwards_remote_changes() {
    let remote = seeded_remote();
    let worker = attached_worker(&remote);
    remote.push_file("x.md", "hello", "add x");
    ctx_sync(&worker)
        .arg("pull")
        .assert()
        .success()
        .stdout(contains("Updated "))
        .stdout(contains(".."));
    assert_eq!(
        std::fs::read_to_string(context_repo(&worker).join("x.md")).unwrap(),
        "hello"
    );
}

#[test]
fn pull_does_not_touch_the_project_tree() {
    let remote = seeded_remote();
    let worker = attached_worker(&remote);
    let before = worker.git(&["status", "--porcelain", "--untracked-files=all"]);
    remote.push_file("x.md", "hello", "add x");
    ctx_sync(&worker).arg("pull").assert().success();
    let after = worker.git(&["status", "--porcelain", "--untracked-files=all"]);
    assert_eq!(before, after);
}

#[test]
fn diverged_history_is_reported_but_not_an_error() {
    let remote = seeded_remote();
    let worker = attached_worker(&remote);
    // `register` commits the worker file locally without syncing.
    ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .success();
    remote.push_file("x.md", "hello", "add x");
    ctx_sync(&worker)
        .arg("pull")
        .assert()
        .success()
        .stdout(contains(
            "Local context has 1 unpushed commit(s) and remote has 1 new commit(s)",
        ))
        .stderr(contains("warning: run `ctx-sync sync` to rebase and push"));
}

#[test]
fn pull_outside_an_attached_project_exits_with_4() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker).arg("pull").assert().code(4);
}
