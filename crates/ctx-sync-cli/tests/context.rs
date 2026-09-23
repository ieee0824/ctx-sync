mod common;

use common::{ctx_sync, stdout};
use ctx_sync_testutil::{SEED_PROJECT_ID, TestRemote, TestWorker};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn registered_worker(remote: &TestRemote, name: &str) -> TestWorker {
    let worker = TestWorker::new(remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["register", name])
        .assert()
        .success();
    worker
}

fn seeded_remote() -> TestRemote {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    remote
}

#[test]
fn prints_the_shared_context_as_markdown() {
    let remote = seeded_remote();
    let worker = registered_worker(&remote, "parser");
    let output = ctx_sync(&worker)
        .arg("context")
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&output);
    assert!(
        text.starts_with("# Shared Project Context\n\nProject: demo\n"),
        "{text}"
    );
    assert!(text.contains(&format!("Project ID: {SEED_PROJECT_ID}")));
    assert!(text.contains("## Project\n\n### Goal\n\nTest project."));
    assert!(text.contains("## Active Workers\n\n### parser ("));
}

#[test]
fn prints_json_with_the_same_content() {
    let remote = seeded_remote();
    let worker = registered_worker(&remote, "parser");
    let output = ctx_sync(&worker)
        .args(["context", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(json["project_name"], "demo");
    assert_eq!(json["project_id"], SEED_PROJECT_ID);
    assert_eq!(json["decisions"].as_array().unwrap().len(), 0);
    assert_eq!(json["active_workers"][0]["name"], "parser");
}

#[test]
fn does_not_pull() {
    let remote = seeded_remote();
    let worker = registered_worker(&remote, "parser");
    // Push the registration so that the next pull can fast-forward.
    ctx_sync(&worker).arg("sync").assert().success();
    remote.push_file(
        "30-decision-20260923-001-x.md",
        "# Decision: Remote only\n\nStatus: accepted\n\n## Decision\n\nx\n",
        "remote decision",
    );
    ctx_sync(&worker)
        .arg("context")
        .assert()
        .success()
        .stdout(contains("Remote only").not());
    ctx_sync(&worker).arg("pull").assert().success();
    ctx_sync(&worker)
        .arg("context")
        .assert()
        .success()
        .stdout(contains("### 20260923-001-x: Remote only"));
}

#[test]
fn unattached_project_exits_with_4() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker).arg("context").assert().code(4);
}
