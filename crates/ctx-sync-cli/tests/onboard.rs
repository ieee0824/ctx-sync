mod common;

use common::ctx_sync;
use ctx_sync_testutil::{TestRemote, TestWorker};
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

#[test]
fn onboarding_shows_you_and_the_other_workers() {
    let remote = seeded_remote();
    let a = registered_worker(&remote, "a");
    ctx_sync(&a)
        .args([
            "handoff",
            "--task",
            "Parser",
            "--summary",
            "Parser implemented",
            "--attention",
            "ParserResult changed",
            "--sync",
        ])
        .assert()
        .success();
    let b = registered_worker(&remote, "b");
    ctx_sync(&b)
        .arg("onboard")
        .assert()
        .success()
        .stdout(contains("# Project Onboarding\n\n## Project\n\ndemo\n"))
        .stdout(contains("### Goal\n\nTest project."))
        .stdout(contains("## You\n\nb ("))
        .stdout(contains("## Active Workers\n\n### a ("))
        .stdout(contains("- [a] ParserResult changed"))
        .stdout(contains("a: Parser implemented"));
}

#[test]
fn unregistered_worktree_gets_onboarding_without_you() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    let output = ctx_sync(&worker).arg("onboard").output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("# Project Onboarding"));
    assert!(!text.contains("## You"));
}

#[test]
fn unattached_project_exits_with_4() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker).arg("onboard").assert().code(4);
}
