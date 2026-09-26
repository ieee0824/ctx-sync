mod common;

use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::str::contains;

use common::{attach, ctx_sync};

#[test]
fn show_without_a_record_reports_no_conflict() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let worker = TestWorker::new(&remote);
    attach(&worker);
    ctx_sync(&worker)
        .args(["conflict", "show"])
        .assert()
        .success()
        .stdout(contains("No context conflict recorded."));
    ctx_sync(&worker)
        .args(["conflict", "resolve", "--keep-local"])
        .assert()
        .code(1)
        .stderr(contains("no context conflict recorded"));
}

#[test]
fn resolution_requires_a_valid_method() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let worker = TestWorker::new(&remote);
    attach(&worker);
    ctx_sync(&worker)
        .args(["conflict", "resolve"])
        .assert()
        .code(1);
    ctx_sync(&worker)
        .args(["conflict", "resolve", "--merged", "foo"])
        .assert()
        .code(1)
        .stderr(contains("expected FILE=PATH"));
}
