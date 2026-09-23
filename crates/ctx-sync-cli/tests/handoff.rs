mod common;

use common::{context_repo, ctx_sync, stdout};
use ctx_sync_core::model::{Worker, WorkerStatus};
use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::str::contains;

fn seeded_remote() -> TestRemote {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    remote
}

/// Attaches and registers `name`; returns the worker and its file name.
fn registered(remote: &TestRemote, name: &str) -> (TestWorker, String) {
    let worker = TestWorker::new(remote);
    common::attach(&worker);
    let output = ctx_sync(&worker)
        .args(["register", name])
        .assert()
        .success()
        .get_output()
        .clone();
    let id = stdout(&output)
        .lines()
        .find_map(|l| l.strip_prefix("id: ").map(str::to_string))
        .unwrap();
    (worker, format!("40-worker-{id}.md"))
}

fn local_worker(worker: &TestWorker, file: &str) -> Worker {
    Worker::parse(&std::fs::read_to_string(context_repo(worker).join(file)).unwrap()).unwrap()
}

#[test]
fn handoff_updates_the_worker_file() {
    let remote = seeded_remote();
    let (worker, file) = registered(&remote, "parser");
    ctx_sync(&worker)
        .args([
            "handoff",
            "--task",
            "Parser",
            "--attention",
            "API changed",
            "--attention",
            "Needs review",
            "--interface-change",
            "ParserResult gained warnings",
        ])
        .assert()
        .success()
        .stdout(contains("Updated worker parser ("))
        .stdout(contains("status: working"))
        .stdout(contains(format!("file: {file}")))
        .stdout(contains("committed: "))
        .stdout(contains("synced: no (run `ctx-sync sync`)"));
    let w = local_worker(&worker, &file);
    assert_eq!(w.task, "Parser");
    assert_eq!(w.attention, ["API changed", "Needs review"]);
    assert_eq!(w.interface_changes, ["ParserResult gained warnings"]);
    // Not synced yet.
    assert!(
        !remote
            .read_file(&file)
            .unwrap_or_default()
            .contains("API changed")
    );
}

#[test]
fn handoff_with_sync_pushes_the_worker_file() {
    let remote = seeded_remote();
    let (worker, file) = registered(&remote, "parser");
    let output = ctx_sync(&worker)
        .args(["handoff", "--summary", "Parser implemented", "--sync"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(!stdout(&output).contains("synced: no"));
    let pushed = Worker::parse(&remote.read_file(&file).unwrap()).unwrap();
    assert_eq!(pushed.summary, "Parser implemented");
}

#[test]
fn changed_files_come_from_the_project_repository() {
    let remote = seeded_remote();
    let (worker, file) = registered(&remote, "parser");
    std::fs::write(worker.project().join("parser.rs"), "// parser\n").unwrap();
    ctx_sync(&worker).arg("handoff").assert().success();
    let w = local_worker(&worker, &file);
    assert_eq!(w.changed, [".ctx-sync.toml", "parser.rs"]);
    assert_eq!(w.branch.as_deref(), Some("main"));

    ctx_sync(&worker)
        .args(["handoff", "--changed", "src/explicit.rs"])
        .assert()
        .success();
    assert_eq!(local_worker(&worker, &file).changed, ["src/explicit.rs"]);
}

#[test]
fn status_and_blockers_can_be_set() {
    let remote = seeded_remote();
    let (worker, file) = registered(&remote, "parser");
    ctx_sync(&worker)
        .args([
            "handoff",
            "--status",
            "blocked",
            "--blocked-by",
            "API review",
        ])
        .assert()
        .success()
        .stdout(contains("status: blocked"));
    let w = local_worker(&worker, &file);
    assert_eq!(w.status, WorkerStatus::Blocked);
    assert_eq!(w.blocked_by, ["API review"]);
}

#[test]
fn append_keeps_existing_items() {
    let remote = seeded_remote();
    let (worker, file) = registered(&remote, "parser");
    ctx_sync(&worker)
        .args(["handoff", "--attention", "first"])
        .assert()
        .success();
    ctx_sync(&worker)
        .args(["handoff", "--attention", "second", "--append"])
        .assert()
        .success();
    assert_eq!(local_worker(&worker, &file).attention, ["first", "second"]);
}

#[test]
fn done_marks_the_worker_done_and_syncs() {
    let remote = seeded_remote();
    let (worker, file) = registered(&remote, "parser");
    ctx_sync(&worker)
        .args(["done", "--summary", "finished"])
        .assert()
        .success()
        .stdout(contains("status: done"));
    let pushed = Worker::parse(&remote.read_file(&file).unwrap()).unwrap();
    assert_eq!(pushed.status, WorkerStatus::Done);
    assert_eq!(pushed.summary, "finished");
}

#[test]
fn unregistered_worktree_exits_with_4() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["handoff", "--task", "x"])
        .assert()
        .code(4)
        .stderr(contains("ctx-sync register"));
    ctx_sync(&worker).arg("done").assert().code(4);
}
