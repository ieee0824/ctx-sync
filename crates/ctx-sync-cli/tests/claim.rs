mod common;

use common::{attach, ctx_sync, stdout};
use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn registered(remote: &TestRemote, name: &str) -> TestWorker {
    let worker = TestWorker::new(remote);
    attach(&worker);
    ctx_sync(&worker)
        .args(["register", name])
        .assert()
        .success();
    worker
}

#[test]
fn claims_are_listed_warn_on_overlap_and_can_be_released() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = registered(&remote, "alice");

    ctx_sync(&a)
        .arg("claim")
        .assert()
        .success()
        .stdout("No claims.\n");
    ctx_sync(&a)
        .args(["claim", "src/parser/**", "--sync"])
        .assert()
        .success()
        .stdout(contains("Claims of alice ("))
        .stdout(contains("- src/parser/**"))
        .stdout(contains("synced: "));
    let b = registered(&remote, "bob");
    let listing = ctx_sync(&b)
        .arg("claim")
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&listing);
    assert!(text.contains("alice ("));
    assert!(text.contains(": src/parser/**"));

    ctx_sync(&b)
        .args(["claim", "src/**"])
        .assert()
        .success()
        .stderr(contains("warning: src/** overlaps with alice ("));
    ctx_sync(&a)
        .args(["claim", "--release"])
        .assert()
        .success()
        .stdout(contains("Claims of alice ("))
        .stdout(contains("- src/parser/**").not());
}
