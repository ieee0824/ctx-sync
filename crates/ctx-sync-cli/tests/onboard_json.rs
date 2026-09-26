mod common;

use common::ctx_sync;
use ctx_sync_testutil::{TestRemote, TestWorker};
use serde_json::Value;

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

fn json_output(worker: &TestWorker, args: &[&str]) -> Value {
    let output = ctx_sync(worker)
        .args(args)
        .assert()
        .success()
        .get_output()
        .clone();
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn onboard_json_contains_project_and_current_worker() {
    let remote = seeded_remote();
    let worker = registered_worker(&remote, "a");
    let view = json_output(&worker, &["onboard", "--json"]);
    assert_eq!(view["project_name"], "demo");
    assert_eq!(view["you"]["name"], "a");
}

#[test]
fn agent_start_json_reports_existing_registration() {
    let remote = seeded_remote();
    let worker = registered_worker(&remote, "a");
    let outcome = json_output(&worker, &["agent", "start", "--json"]);
    assert_eq!(outcome["onboard"]["project_name"], "demo");
    assert_eq!(outcome["registered"], false);
    assert_eq!(outcome["onboard"]["you"]["name"], "a");
}

#[test]
fn agent_start_json_reports_new_registration() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    let outcome = json_output(&worker, &["agent", "start", "--name", "b", "--json"]);
    assert_eq!(outcome["registered"], true);
    assert_eq!(outcome["onboard"]["you"]["name"], "b");
}

#[test]
fn default_output_remains_markdown() {
    let remote = seeded_remote();
    let worker = registered_worker(&remote, "a");
    for args in [vec!["onboard"], vec!["agent", "start"]] {
        let output = ctx_sync(&worker)
            .args(args)
            .assert()
            .success()
            .get_output()
            .clone();
        assert!(output.stdout.starts_with(b"# Project Onboarding"));
    }
}
