mod common;

use common::{context_repo, ctx_sync, stdout};
use ctx_sync_testutil::{TestRemote, TestWorker, git};
use predicates::str::contains;

fn attached_worker() -> (TestRemote, TestWorker) {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    (remote, worker)
}

#[test]
fn prints_candidates_without_a_config() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    std::fs::write(
        worker.project().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\ntokio = \"1\"\n",
    )
    .unwrap();
    let output = ctx_sync(&worker)
        .arg("bootstrap")
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&output);
    assert!(
        text.starts_with("# Initial Context\n\n## Confirmed\n\n"),
        "{text}"
    );
    assert!(text.contains("- Language: Rust (source: Cargo.toml)"));
    assert!(text.contains("- Title: test project (source: README.md)"));
    assert!(text.contains("- Recent commits: initial commit (source: git log)"));
    assert!(text.contains(
        "## Inferred\n\n- Async runtime: Tokio appears to be used as the async runtime."
    ));
    assert!(text.contains("## Unknown\n\n- Goal: The project goal has not been confirmed."));
    // Nothing is written without --apply.
    assert!(!worker.project().join(".ctx-sync.toml").exists());
}

#[test]
fn apply_adds_initial_context_without_decisions() {
    let (_remote, worker) = attached_worker();
    ctx_sync(&worker)
        .args(["bootstrap", "--apply"])
        .assert()
        .success()
        .stdout(contains("Applied initial context to 10-project.md"))
        .stdout(contains("committed: "))
        .stdout(contains("run `ctx-sync sync`"));
    let repo = context_repo(&worker);
    let project = std::fs::read_to_string(repo.join("10-project.md")).unwrap();
    assert!(project.contains("## Goal\n\nTest project."), "{project}");
    assert!(
        project.contains("## Initial Context\n\n### Confirmed\n\n"),
        "{project}"
    );
    let decisions = std::fs::read_dir(&repo)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("30-decision-"))
        .count();
    assert_eq!(decisions, 0);
    let log = git(&repo, &worker.git_env(), &["log", "-1", "--format=%s"]);
    assert_eq!(log.trim(), "ctx-sync: bootstrap");
}

#[test]
fn applying_twice_is_an_error() {
    let (_remote, worker) = attached_worker();
    ctx_sync(&worker)
        .args(["bootstrap", "--apply"])
        .assert()
        .success();
    ctx_sync(&worker)
        .args(["bootstrap", "--apply"])
        .assert()
        .code(1)
        .stderr(contains("already has an Initial Context section"));
}

#[test]
fn apply_needs_an_attached_project() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["bootstrap", "--apply"])
        .assert()
        .code(4);
}
