mod common;

use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::str::contains;

use common::{attach, context_repo, ctx_sync, stdout};

const ARCHITECTURE: &str = "20-architecture.md";

fn prepare_conflict() -> (TestRemote, TestWorker, TestWorker) {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = TestWorker::new(&remote);
    let b = TestWorker::new(&remote);
    attach(&a);
    attach(&b);
    for (worker, component) in [(&a, "core-a"), (&b, "core-b")] {
        let path = context_repo(worker).join(ARCHITECTURE);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(path, text.replace("- core\n", &format!("- {component}\n"))).unwrap();
    }
    ctx_sync(&a).arg("sync").assert().success();
    ctx_sync(&b).arg("sync").assert().code(2);
    (remote, a, b)
}

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

#[test]
fn show_displays_both_sides_as_text_and_json() {
    let (_remote, _a, b) = prepare_conflict();
    let text = stdout(
        &ctx_sync(&b)
            .args(["conflict", "show"])
            .assert()
            .success()
            .get_output()
            .clone(),
    );
    assert!(text.contains("## 20-architecture.md"));
    assert!(text.contains("- core-a"));
    assert!(text.contains("- core-b"));

    let output = ctx_sync(&b)
        .args(["conflict", "show", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["files"][0]["file"], ARCHITECTURE);
}

#[test]
fn keep_local_pushes_local_content_and_clears_conflict() {
    let (remote, _a, b) = prepare_conflict();
    ctx_sync(&b)
        .args(["conflict", "resolve", "--keep-local"])
        .assert()
        .success()
        .stdout(contains("Resolved context conflict (keep-local)"));
    let text = remote.read_file(ARCHITECTURE).unwrap();
    assert!(text.contains("- core-b\n"));
    assert!(!text.contains("- core-a\n"));
    let status = ctx_sync(&b)
        .arg("status")
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(!stdout(&status).contains("CONFLICT"));
}

#[test]
fn keep_remote_updates_local_content() {
    let (remote, _a, b) = prepare_conflict();
    ctx_sync(&b)
        .args(["conflict", "resolve", "--keep-remote"])
        .assert()
        .success();
    let remote_text = remote.read_file(ARCHITECTURE).unwrap();
    let local_text = std::fs::read_to_string(context_repo(&b).join(ARCHITECTURE)).unwrap();
    assert!(remote_text.contains("- core-a\n"));
    assert!(!remote_text.contains("- core-b\n"));
    assert_eq!(local_text, remote_text);
}

#[test]
fn merged_content_reaches_remote_and_other_worker_after_pull() {
    let (remote, a, b) = prepare_conflict();
    let merged = remote
        .read_file(ARCHITECTURE)
        .unwrap()
        .replace("- core-a\n", "- core-a\n- core-b\n");
    let merged_path = b.project().join("merged-architecture.md");
    std::fs::write(&merged_path, &merged).unwrap();
    let option = format!("{ARCHITECTURE}={}", merged_path.display());
    ctx_sync(&b)
        .args(["conflict", "resolve", "--merged", &option])
        .assert()
        .success()
        .stdout(contains("Resolved context conflict (merged)"));
    assert_eq!(
        remote.read_file(ARCHITECTURE).as_deref(),
        Some(merged.as_str())
    );

    ctx_sync(&a).arg("pull").assert().success();
    assert_eq!(
        std::fs::read_to_string(context_repo(&a).join(ARCHITECTURE)).unwrap(),
        merged
    );
}
