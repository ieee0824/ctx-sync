mod common;

use common::{context_repo, ctx_sync, stdout};
use ctx_sync_core::model::{Decision, Worker};
use ctx_sync_testutil::{TestRemote, TestWorker};

fn registered_worker() -> (TestRemote, TestWorker, String) {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    let output = ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .success()
        .get_output()
        .clone();
    let id = stdout(&output)
        .lines()
        .find_map(|line| line.strip_prefix("id: ").map(str::to_string))
        .unwrap();
    (remote, worker, format!("40-worker-{id}.md"))
}

fn local_worker(worker: &TestWorker, file: &str) -> Worker {
    Worker::parse(&std::fs::read_to_string(context_repo(worker).join(file)).unwrap()).unwrap()
}

#[test]
fn handoff_accepts_a_summary_starting_with_dash() {
    let (_remote, worker, file) = registered_worker();
    ctx_sync(&worker)
        .args(["handoff", "--summary=-x flag removed"])
        .assert()
        .success();
    assert_eq!(local_worker(&worker, &file).summary, "-x flag removed");
}

#[test]
fn handoff_accepts_repeated_attention_values_starting_with_dash() {
    let (_remote, worker, file) = registered_worker();
    ctx_sync(&worker)
        .args(["handoff", "--attention=-y", "--attention=-z"])
        .assert()
        .success();
    assert_eq!(local_worker(&worker, &file).attention, ["-y", "-z"]);
}

#[test]
fn decision_accepts_a_reason_starting_with_dash() {
    let (_remote, worker, _file) = registered_worker();
    ctx_sync(&worker)
        .args(["decision", "add", "Drop -v", "--reason=-v is noisy"])
        .assert()
        .success();
    let file = "30-decision-20260923-001-drop-v.md";
    let decision = Decision::parse(
        file,
        &std::fs::read_to_string(context_repo(&worker).join(file)).unwrap(),
    )
    .unwrap();
    assert_eq!(decision.reason, "-v is noisy");
}

#[test]
fn separate_dash_prefixed_summary_is_rejected() {
    let (_remote, worker, _file) = registered_worker();
    ctx_sync(&worker)
        .args(["handoff", "--summary", "-x"])
        .assert()
        .code(1);
}
