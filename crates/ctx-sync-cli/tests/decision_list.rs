mod common;

use common::ctx_sync;
use ctx_sync_testutil::{TestRemote, TestWorker};
use serde_json::Value;

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
fn lists_effective_or_all_decisions_as_text_and_json() {
    let (_remote, worker) = registered_worker();
    ctx_sync(&worker)
        .args(["decision", "add", "Use Gist"])
        .assert()
        .success();
    ctx_sync(&worker)
        .args([
            "decision",
            "add",
            "Use Git transport",
            "--supersedes",
            "20260923-001-use-gist",
        ])
        .assert()
        .success();

    let effective = ctx_sync(&worker)
        .args(["decision", "list"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert_eq!(
        String::from_utf8(effective.stdout).unwrap(),
        "20260923-002-use-git-transport  accepted  Use Git transport\n"
    );

    let all = ctx_sync(&worker)
        .args(["decision", "list", "--all"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert_eq!(
        String::from_utf8(all.stdout).unwrap(),
        "20260923-001-use-gist  superseded by 20260923-002-use-git-transport  Use Gist\n\
         20260923-002-use-git-transport  accepted  Use Git transport\n"
    );

    let json = ctx_sync(&worker)
        .args(["decision", "list", "--all", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let items: Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 2);
    assert_eq!(items[0]["id"], "20260923-001-use-gist");
    assert_eq!(
        items[0]["superseded_by"][0],
        "20260923-002-use-git-transport"
    );
    assert_eq!(items[0]["effective"], false);
    assert_eq!(items[1]["effective"], true);
}
