mod common;

use common::{context_repo, ctx_sync, stdout};
use ctx_sync_core::config::CONFIG_FILE;
use ctx_sync_core::model::{Worker, WorkerStatus};
use ctx_sync_core::state::{ConflictRecord, StateRoot};
use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::str::contains;
use uuid::Uuid;

fn seeded_remote() -> TestRemote {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    remote
}

fn worker_file(worker: &TestWorker) -> std::path::PathBuf {
    std::fs::read_dir(context_repo(worker))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("40-worker-")
        })
        .expect("worker file")
}

#[test]
fn auto_attaches_and_registers_from_existing_config() {
    let remote = seeded_remote();
    let a = TestWorker::new(&remote);
    common::attach(&a);
    let b = TestWorker::new(&remote);
    std::fs::copy(a.project().join(CONFIG_FILE), b.project().join(CONFIG_FILE)).unwrap();

    let output = ctx_sync(&b)
        .args(["agent", "start", "--name", "b"])
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&output);
    assert!(text.starts_with("# Project Onboarding\n"), "{text}");
    assert!(text.contains("## You\n\nb ("), "{text}");
    assert!(context_repo(&b).join("00-meta.json").is_file());
    assert_eq!(
        Worker::parse(&std::fs::read_to_string(worker_file(&b)).unwrap())
            .unwrap()
            .name,
        "b"
    );
}

#[test]
fn missing_config_exits_with_4_and_guidance() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["agent", "start", "--name", "b"])
        .assert()
        .code(4)
        .stderr(contains(
            "ask a human to run `ctx-sync init` or `ctx-sync attach <gist>`",
        ));
}

#[test]
fn missing_name_exits_with_4() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["agent", "start"])
        .assert()
        .code(4)
        .stderr(contains("ctx-sync agent start --name <name>"));
}

#[test]
fn done_worker_requires_resume_to_return_to_working() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["agent", "start", "--name", "parser"])
        .assert()
        .success();
    ctx_sync(&worker).arg("done").assert().success();

    ctx_sync(&worker)
        .args(["agent", "start"])
        .assert()
        .success()
        .stdout(contains("parser ("))
        .stdout(contains("— done"))
        .stderr(contains(
            "warning: your worker status is done; pass --resume to continue",
        ));
    assert_eq!(
        Worker::parse(&std::fs::read_to_string(worker_file(&worker)).unwrap())
            .unwrap()
            .status,
        WorkerStatus::Done
    );

    ctx_sync(&worker)
        .args(["agent", "start", "--resume"])
        .assert()
        .success()
        .stdout(contains("— working"));
    assert_eq!(
        Worker::parse(&std::fs::read_to_string(worker_file(&worker)).unwrap())
            .unwrap()
            .status,
        WorkerStatus::Working
    );
}

#[test]
fn diverged_context_warns_on_stderr() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .success();
    remote.push_file("extra.md", "new", "remote update");
    ctx_sync(&worker)
        .args(["agent", "start"])
        .assert()
        .success()
        .stderr(contains(
            "warning: local context has unpushed commits; run `ctx-sync sync`",
        ));
}

#[test]
fn pulls_remote_context_before_rendering() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .success();
    ctx_sync(&worker).arg("sync").assert().success();

    let project = remote.read_file("10-project.md").unwrap();
    remote.push_file(
        "10-project.md",
        &project.replace("Test project.", "Updated project goal."),
        "update project goal",
    );
    ctx_sync(&worker)
        .args(["agent", "start"])
        .assert()
        .success()
        .stdout(contains("Updated project goal."));
}

#[test]
fn recorded_conflict_warns_on_stderr() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .success();
    let record = ConflictRecord {
        detected_at: ctx_sync_core::clock::parse_now(Some(ctx_sync_testutil::FIXED_NOW)).unwrap(),
        files: vec!["20-architecture.md".into()],
        local_head: "a".repeat(40),
        remote_head: "b".repeat(40),
    };
    let conflict_path = StateRoot::new(worker.state_home())
        .project(&Uuid::parse_str(ctx_sync_testutil::SEED_PROJECT_ID).unwrap())
        .conflict_json();
    record.save(&conflict_path).unwrap();
    ctx_sync(&worker)
        .args(["agent", "start"])
        .assert()
        .success()
        .stderr(contains(
            "warning: unresolved context conflict: 20-architecture.md; run `ctx-sync status`",
        ));
}
