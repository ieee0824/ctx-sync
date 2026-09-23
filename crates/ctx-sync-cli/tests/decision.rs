mod common;

use common::{context_repo, ctx_sync, stdout};
use ctx_sync_core::model::{Decision, DecisionStatus};
use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

const FIRST: &str = "30-decision-20260923-001-use-git-transport.md";
const SECOND: &str = "30-decision-20260923-002-use-git-transport.md";

fn registered_worker(remote: &TestRemote) -> (TestWorker, String) {
    let worker = TestWorker::new(remote);
    common::attach(&worker);
    let output = ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .success()
        .get_output()
        .clone();
    let id = stdout(&output)
        .lines()
        .find_map(|l| l.strip_prefix("id: ").map(str::to_string))
        .unwrap();
    (worker, id)
}

fn seeded_remote() -> TestRemote {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    remote
}

fn read(worker: &TestWorker, file: &str) -> String {
    std::fs::read_to_string(context_repo(worker).join(file)).unwrap()
}

#[test]
fn adds_a_decision_file() {
    let remote = seeded_remote();
    let (worker, short_id) = registered_worker(&remote);
    ctx_sync(&worker)
        .args([
            "decision",
            "add",
            "Use Git transport",
            "--context",
            "Need concurrent updates",
            "--reason",
            "Git already provides conflict detection",
        ])
        .assert()
        .success()
        .stdout(contains("Added decision"))
        .stdout(contains("id: 20260923-001-use-git-transport"))
        .stdout(contains(format!("file: {FIRST}")))
        .stdout(contains("synced: no (run `ctx-sync sync`)"));
    let decision = Decision::parse(FIRST, &read(&worker, FIRST)).unwrap();
    assert_eq!(decision.title, "Use Git transport");
    assert_eq!(decision.status, DecisionStatus::Accepted);
    assert_eq!(decision.context, "Need concurrent updates");
    assert_eq!(decision.reason, "Git already provides conflict detection");
    assert_eq!(decision.decision, "Use Git transport");
    assert_eq!(decision.author.as_deref(), Some(short_id.as_str()));
}

#[test]
fn same_title_gets_the_next_number_without_overwriting() {
    let remote = seeded_remote();
    let (worker, _) = registered_worker(&remote);
    ctx_sync(&worker)
        .args(["decision", "add", "Use Git transport"])
        .assert()
        .success();
    let first = read(&worker, FIRST);
    ctx_sync(&worker)
        .args(["decision", "add", "Use Git transport"])
        .assert()
        .success()
        .stdout(contains(format!("file: {SECOND}")));
    assert_eq!(read(&worker, FIRST), first);
}

#[test]
fn supersedes_an_existing_decision() {
    let remote = seeded_remote();
    let (worker, _) = registered_worker(&remote);
    ctx_sync(&worker)
        .args(["decision", "add", "Use Git transport"])
        .assert()
        .success();
    let first = read(&worker, FIRST);
    ctx_sync(&worker)
        .args([
            "decision",
            "add",
            "Use Git transport",
            "--decision",
            "Use Git over SSH",
            "--supersedes",
            "20260923-001-use-git-transport",
        ])
        .assert()
        .success();
    assert!(read(&worker, SECOND).contains("Supersedes: 20260923-001-use-git-transport"));
    assert_eq!(read(&worker, FIRST), first);
    // Only the new decision is effective.
    ctx_sync(&worker)
        .arg("context")
        .assert()
        .success()
        .stdout(contains(
            "### 20260923-002-use-git-transport: Use Git transport",
        ))
        .stdout(contains("### 20260923-001").not());
}

#[test]
fn unknown_superseded_id_exits_with_1() {
    let remote = seeded_remote();
    let (worker, _) = registered_worker(&remote);
    ctx_sync(&worker)
        .args(["decision", "add", "X", "--supersedes", "20990101-001-x"])
        .assert()
        .code(1)
        .stderr(contains("unknown decision id: 20990101-001-x"));
}

#[test]
fn sync_pushes_the_decision_and_status_can_be_proposed() {
    let remote = seeded_remote();
    let (worker, _) = registered_worker(&remote);
    ctx_sync(&worker)
        .args([
            "decision",
            "add",
            "Use Git transport",
            "--status",
            "proposed",
            "--sync",
        ])
        .assert()
        .success();
    let pushed = remote.read_file(FIRST).unwrap();
    assert!(pushed.contains("Status: proposed"));
}

#[test]
fn empty_title_and_unregistered_worktree_exit_with_4() {
    let remote = seeded_remote();
    let (worker, _) = registered_worker(&remote);
    ctx_sync(&worker)
        .args(["decision", "add", "  "])
        .assert()
        .code(4);

    let other = TestWorker::new(&remote);
    common::attach(&other);
    ctx_sync(&other)
        .args(["decision", "add", "X"])
        .assert()
        .code(4)
        .stderr(contains("ctx-sync register"));
}
