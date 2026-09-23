mod common;

use common::ctx_sync;
use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

/// Fake credentials are assembled at runtime so that the source does not
/// look like it contains secrets (and does not trip secret scanners).
fn fake(parts: &[&str]) -> String {
    parts.concat()
}

fn registered_worker() -> (TestRemote, TestWorker) {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .success();
    (remote, worker)
}

#[test]
fn handoff_warns_about_a_token_without_printing_it() {
    let (_remote, worker) = registered_worker();
    ctx_sync(&worker)
        .args([
            "handoff",
            "--attention",
            &fake(&["see ghp_", "0123456789abcdefghij0123"]),
        ])
        .assert()
        .success()
        .stderr(contains(
            "warning: possible credential (github-token) in attention[0]",
        ))
        .stderr(contains("ghp_0123").not())
        .stdout(contains("ghp_0123").not());
}

#[test]
fn done_and_decision_add_are_checked_too() {
    let (_remote, worker) = registered_worker();
    ctx_sync(&worker)
        .args([
            "decision",
            "add",
            "Use the API",
            "--reason",
            &fake(&["key is sk-", "abcdefghijklmnopqrstuvwx"]),
        ])
        .assert()
        .success()
        .stderr(contains(
            "warning: possible credential (openai-api-key) in reason",
        ));
    ctx_sync(&worker)
        .args([
            "done",
            &fake(&["--summary=-----BEGIN RSA ", "PRIVATE KEY", "-----"]),
        ])
        .assert()
        .success()
        .stderr(contains(
            "warning: possible credential (private-key) in summary",
        ));
}

#[test]
fn ordinary_text_has_no_warning() {
    let (_remote, worker) = registered_worker();
    ctx_sync(&worker)
        .args(["handoff", "--attention", "task-skills need review"])
        .assert()
        .success()
        .stderr(contains("possible credential").not());
}
