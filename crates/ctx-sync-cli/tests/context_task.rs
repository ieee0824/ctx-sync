mod common;

use common::{attach, ctx_sync, stdout};
use ctx_sync_testutil::{TestRemote, TestWorker};

fn setup() -> (TestRemote, TestWorker, TestWorker) {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = TestWorker::new(&remote);
    let b = TestWorker::new(&remote);
    attach(&a);
    attach(&b);
    ctx_sync(&a).args(["register", "alice"]).assert().success();
    ctx_sync(&a)
        .args(["handoff", "--task", "Parser", "--sync"])
        .assert()
        .success();
    ctx_sync(&b).arg("pull").assert().success();
    ctx_sync(&b).args(["register", "bob"]).assert().success();
    ctx_sync(&b)
        .args(["handoff", "--task", "Gist backend", "--sync"])
        .assert()
        .success();
    ctx_sync(&b)
        .args(["decision", "add", "Use Git transport", "--sync"])
        .assert()
        .success();
    ctx_sync(&a).arg("pull").assert().success();
    (remote, a, b)
}

#[test]
fn task_filter_limits_workers_in_markdown() {
    let (_remote, a, _b) = setup();
    let output = ctx_sync(&a)
        .args(["context", "--task", "parser"])
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&output);
    assert!(text.contains("Task filter: parser"));
    assert!(text.contains("### alice ("));
    assert!(!text.contains("### bob ("));
}

#[test]
fn task_filter_and_json_select_a_decision() {
    let (_remote, a, _b) = setup();
    let output = ctx_sync(&a)
        .args(["context", "--task", "git", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["task"], "git");
    assert_eq!(json["decisions"].as_array().unwrap().len(), 1);
}

#[test]
fn context_without_task_keeps_all_active_workers() {
    let (_remote, a, _b) = setup();
    let output = ctx_sync(&a)
        .arg("context")
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&output);
    assert!(text.contains("### alice ("));
    assert!(text.contains("### bob ("));
    assert!(!text.contains("Task filter:"));
}
