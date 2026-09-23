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

fn worker_file_named(worker: &TestWorker, name: &str) -> std::path::PathBuf {
    std::fs::read_dir(context_repo(worker))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("40-worker-")
                && Worker::parse(&std::fs::read_to_string(path).unwrap())
                    .is_ok_and(|worker| worker.name == name)
        })
        .expect("named worker file")
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

#[test]
fn finish_publishes_a_handoff_for_another_agent() {
    let remote = seeded_remote();
    let a = TestWorker::new(&remote);
    let b = TestWorker::new(&remote);
    common::attach(&a);
    ctx_sync(&a)
        .args(["agent", "start", "--name", "a"])
        .assert()
        .success();
    common::attach(&b);
    ctx_sync(&b)
        .args(["agent", "start", "--name", "b"])
        .assert()
        .success();

    let output = ctx_sync(&a)
        .args([
            "agent",
            "finish",
            "--summary",
            "Implemented parser",
            "--attention",
            "Parser API changed",
            "--interface-change",
            "ParserResult gained warnings",
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&output);
    assert!(
        text.starts_with("# Handoff Complete\n\nworker: a ("),
        "{text}"
    );
    assert!(text.contains("status: working\n"), "{text}");
    assert!(text.contains("file: 40-worker-"), "{text}");
    assert!(text.contains("revision: "), "{text}");

    ctx_sync(&b)
        .args(["agent", "start"])
        .assert()
        .success()
        .stdout(contains("## You\n\nb ("))
        .stdout(contains("[a] Parser API changed"))
        .stdout(contains("a: Implemented parser"));
    ctx_sync(&b)
        .args(["agent", "finish", "--summary", "Gist backend"])
        .assert()
        .success()
        .stdout(contains("# Handoff Complete"));

    assert!(
        remote
            .read_file(
                worker_file_named(&a, "a")
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
            )
            .unwrap()
            .contains("Parser API changed")
    );
    assert!(
        remote
            .read_file(
                worker_file_named(&b, "b")
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
            )
            .unwrap()
            .contains("Gist backend")
    );
}

#[test]
fn finish_passes_through_handoff_fields_and_keeps_the_requested_status() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["agent", "start", "--name", "parser"])
        .assert()
        .success();
    ctx_sync(&worker)
        .args([
            "agent",
            "finish",
            "--task",
            "Parser",
            "--summary",
            "Waiting on API",
            "--status",
            "blocked",
            "--working-on",
            "Parsing errors",
            "--changed",
            "src/parser.rs",
            "--interface-change",
            "ParserResult gained warnings",
            "--attention",
            "Review the API",
            "--blocked-by",
            "Schema decision",
        ])
        .assert()
        .success()
        .stdout(contains("status: blocked"));

    let file = worker_file_named(&worker, "parser");
    let published = Worker::parse(
        &remote
            .read_file(file.file_name().unwrap().to_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(published.status, WorkerStatus::Blocked);
    assert_eq!(published.task, "Parser");
    assert_eq!(published.summary, "Waiting on API");
    assert_eq!(published.working_on, ["Parsing errors"]);
    assert_eq!(published.changed, ["src/parser.rs"]);
    assert_eq!(
        published.interface_changes,
        ["ParserResult gained warnings"]
    );
    assert_eq!(published.attention, ["Review the API"]);
    assert_eq!(published.blocked_by, ["Schema decision"]);
    assert_eq!(published.branch.as_deref(), Some("main"));
    assert!(published.commit.is_some());

    ctx_sync(&worker)
        .args(["agent", "finish", "--summary", "Still waiting"])
        .assert()
        .success()
        .stdout(contains("status: blocked"));
    let published = Worker::parse(
        &remote
            .read_file(file.file_name().unwrap().to_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(published.status, WorkerStatus::Blocked);
}

#[test]
fn finish_reports_credential_warnings_on_stderr() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["agent", "start", "--name", "parser"])
        .assert()
        .success();
    let example = ["ghp_", "0123456789abcdefghij0123"].concat();
    let output = ctx_sync(&worker)
        .args(["agent", "finish", "--attention", &example])
        .assert()
        .success()
        .get_output()
        .clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("warning: possible credential (github-token) in attention[0]"),
        "{stderr}"
    );
    assert!(!stderr.contains(&example));
}

#[test]
fn finish_exits_with_2_on_a_context_conflict() {
    let remote = seeded_remote();
    let a = TestWorker::new(&remote);
    let b = TestWorker::new(&remote);
    for (worker, name) in [(&a, "a"), (&b, "b")] {
        common::attach(worker);
        ctx_sync(worker)
            .args(["agent", "start", "--name", name])
            .assert()
            .success();
    }
    for (worker, replacement) in [(&a, "core-a"), (&b, "core-b")] {
        let path = context_repo(worker).join("20-architecture.md");
        let original = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            path,
            original.replace("- core\n", &format!("- {replacement}\n")),
        )
        .unwrap();
    }
    ctx_sync(&a)
        .args(["agent", "finish", "--summary", "Architecture A"])
        .assert()
        .success();
    let output = ctx_sync(&b)
        .args(["agent", "finish", "--summary", "Architecture B"])
        .assert()
        .code(2)
        .stderr(contains("Context conflict detected:"))
        .stderr(contains("20-architecture.md"))
        .get_output()
        .clone();
    assert!(output.stdout.is_empty());
    assert!(
        remote
            .read_file("20-architecture.md")
            .unwrap()
            .contains("- core-a\n")
    );
}
