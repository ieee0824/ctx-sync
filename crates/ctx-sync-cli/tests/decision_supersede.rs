mod common;

use common::{context_repo, ctx_sync};
use ctx_sync_core::model::Decision;
use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::str::contains;

const FIRST_ID: &str = "20260923-001-use-gist";
const FIRST_FILE: &str = "30-decision-20260923-001-use-gist.md";
const SECOND_FILE: &str = "30-decision-20260923-002-use-git-transport.md";

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
fn supersede_appends_a_replacement_and_preserves_the_old_file() {
    let (_remote, worker) = registered_worker();
    ctx_sync(&worker)
        .args(["decision", "add", "Use Gist"])
        .assert()
        .success();
    let original = std::fs::read_to_string(context_repo(&worker).join(FIRST_FILE)).unwrap();
    ctx_sync(&worker)
        .args([
            "decision",
            "supersede",
            FIRST_ID,
            "Use Git transport",
            "--reason",
            "Git handles concurrency",
        ])
        .assert()
        .success()
        .stdout(contains(
            "Added decision\nsupersedes: 20260923-001-use-gist",
        ))
        .stdout(contains("id: 20260923-002-use-git-transport"))
        .stdout(contains(format!("file: {SECOND_FILE}")));
    assert_eq!(
        std::fs::read_to_string(context_repo(&worker).join(FIRST_FILE)).unwrap(),
        original
    );
    let replacement = Decision::parse(
        SECOND_FILE,
        &std::fs::read_to_string(context_repo(&worker).join(SECOND_FILE)).unwrap(),
    )
    .unwrap();
    assert_eq!(replacement.supersedes[0].as_str(), FIRST_ID);
    assert_eq!(replacement.reason, "Git handles concurrency");
    ctx_sync(&worker)
        .args(["decision", "list"])
        .assert()
        .success()
        .stdout("20260923-002-use-git-transport  accepted  Use Git transport\n");
}

#[test]
fn warns_but_continues_when_old_decision_is_not_effective() {
    let (_remote, worker) = registered_worker();
    ctx_sync(&worker)
        .args(["decision", "add", "Use Gist"])
        .assert()
        .success();
    ctx_sync(&worker)
        .args(["decision", "supersede", FIRST_ID, "Use Git transport"])
        .assert()
        .success();
    ctx_sync(&worker)
        .args(["decision", "supersede", FIRST_ID, "Use SSH"])
        .assert()
        .success()
        .stderr(contains(
            "decision 20260923-001-use-gist is not effective (superseded by 20260923-002-use-git-transport)",
        ));
    ctx_sync(&worker)
        .args(["decision", "add", "Maybe S3", "--status", "proposed"])
        .assert()
        .success();
    ctx_sync(&worker)
        .args([
            "decision",
            "supersede",
            "20260923-004-maybe-s3",
            "Use Local",
        ])
        .assert()
        .success()
        .stderr(contains("is not effective (status proposed)"));
}

#[test]
fn unknown_id_exits_with_one_and_unregistered_exits_with_four() {
    let (remote, worker) = registered_worker();
    ctx_sync(&worker)
        .args([
            "decision",
            "supersede",
            "20260923-999-missing",
            "Replacement",
        ])
        .assert()
        .code(1)
        .stderr(contains("unknown decision id: 20260923-999-missing"));
    let other = TestWorker::new(&remote);
    common::attach(&other);
    ctx_sync(&other)
        .args([
            "decision",
            "supersede",
            "20260923-999-missing",
            "Replacement",
        ])
        .assert()
        .code(4);
}
