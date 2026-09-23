//! End-to-end v0.1 acceptance scenarios against a local bare Git remote.

mod common;

use std::path::{Path, PathBuf};
use std::process::Output;

use common::{GIST, ctx_sync, stdout};
use ctx_sync_core::model::Worker;
use ctx_sync_core::state::StateRoot;
use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::str::contains;
use uuid::Uuid;

struct Project {
    remote: TestRemote,
    a: TestWorker,
    b: TestWorker,
    id: Uuid,
}

impl Project {
    /// Start with an unseeded bare remote; only the CLI may create context.
    fn new() -> Self {
        let remote = TestRemote::new();
        assert!(remote.read_file("00-meta.json").is_none());
        let a = TestWorker::new(&remote);
        let b = TestWorker::new(&remote);
        let output = ctx_sync(&a)
            .args(["init", "--project", "demo", "--gist-id", GIST, "--no-input"])
            .assert()
            .success()
            .get_output()
            .clone();
        let id = Uuid::parse_str(&output_field(&output, "project id: ")).unwrap();
        assert!(remote.read_file("00-meta.json").is_some());
        Self { remote, a, b, id }
    }

    fn context_repo(&self, worker: &TestWorker) -> PathBuf {
        StateRoot::new(worker.state_home())
            .project(&self.id)
            .context_repo()
    }

    fn attach_b(&self) {
        ctx_sync(&self.b)
            .args(["attach", GIST])
            .assert()
            .success()
            .stdout(contains(format!("project id: {}", self.id)));
    }
}

fn output_field(output: &Output, prefix: &str) -> String {
    stdout(output)
        .lines()
        .find_map(|line| line.strip_prefix(prefix).map(str::to_string))
        .unwrap_or_else(|| panic!("missing {prefix:?} in:\n{}", stdout(output)))
}

fn edit_components_line(repo: &Path, replacement: &str) -> String {
    let path = repo.join("20-architecture.md");
    let original = std::fs::read_to_string(&path).unwrap();
    let old = "## Components\n\nTBD\n";
    assert_eq!(original.matches(old).count(), 1);
    let edited = original.replacen(old, &format!("## Components\n\n{replacement}\n"), 1);
    std::fs::write(path, &edited).unwrap();
    edited
}

#[test]
fn basic_flow_from_init_through_two_worker_handoffs() {
    let project = Project::new();
    ctx_sync(&project.a)
        .args(["register", "a"])
        .assert()
        .success();
    project.attach_b();
    ctx_sync(&project.b)
        .args(["register", "b"])
        .assert()
        .success();

    ctx_sync(&project.a)
        .args(["agent", "start"])
        .assert()
        .success();
    ctx_sync(&project.a)
        .args([
            "handoff",
            "--task",
            "Parser",
            "--summary",
            "Parser implemented",
            "--attention",
            "ParserResult changed",
        ])
        .assert()
        .success();
    ctx_sync(&project.a).arg("sync").assert().success();

    ctx_sync(&project.b)
        .args(["agent", "start"])
        .assert()
        .success()
        .stdout(contains("Parser implemented"))
        .stdout(contains("ParserResult changed"));
    ctx_sync(&project.b)
        .args([
            "handoff",
            "--task",
            "Gist backend",
            "--summary",
            "Gist backend implemented",
        ])
        .assert()
        .success();
    ctx_sync(&project.b).arg("sync").assert().success();
    ctx_sync(&project.a).arg("pull").assert().success();
    ctx_sync(&project.a)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("b ("))
        .stdout(contains("task: Gist backend"));
    ctx_sync(&project.a)
        .arg("context")
        .assert()
        .success()
        .stdout(contains("Gist backend implemented"));
}

#[test]
fn concurrent_updates_to_different_worker_files_both_survive() {
    let project = Project::new();
    let a_registration = ctx_sync(&project.a)
        .args(["register", "a"])
        .assert()
        .success()
        .get_output()
        .clone();
    project.attach_b();
    let b_registration = ctx_sync(&project.b)
        .args(["register", "b"])
        .assert()
        .success()
        .get_output()
        .clone();
    let a_file = format!("40-worker-{}.md", output_field(&a_registration, "id: "));
    let b_file = format!("40-worker-{}.md", output_field(&b_registration, "id: "));
    assert_ne!(a_file, b_file);

    ctx_sync(&project.a)
        .args(["handoff", "--task", "Parser", "--summary", "A latest"])
        .assert()
        .success();
    ctx_sync(&project.b)
        .args(["handoff", "--task", "Gist", "--summary", "B latest"])
        .assert()
        .success();
    ctx_sync(&project.a).arg("sync").assert().success();
    ctx_sync(&project.b).arg("sync").assert().success();

    let a = Worker::parse(&project.remote.read_file(&a_file).expect("A on remote")).unwrap();
    let b = Worker::parse(&project.remote.read_file(&b_file).expect("B on remote")).unwrap();
    assert_eq!(
        (a.name.as_str(), a.task.as_str(), a.summary.as_str()),
        ("a", "Parser", "A latest")
    );
    assert_eq!(
        (b.name.as_str(), b.task.as_str(), b.summary.as_str()),
        ("b", "Gist", "B latest")
    );
}

#[test]
fn conflicting_architecture_edits_stop_without_overwriting_remote() {
    let project = Project::new();
    project.attach_b();
    let a_repo = project.context_repo(&project.a);
    let b_repo = project.context_repo(&project.b);
    assert!(a_repo.join("20-architecture.md").is_file());
    assert!(b_repo.join("20-architecture.md").is_file());
    let a_content = edit_components_line(&a_repo, "A design");
    edit_components_line(&b_repo, "B design");

    ctx_sync(&project.a).arg("sync").assert().success();
    ctx_sync(&project.b)
        .arg("sync")
        .assert()
        .code(2)
        .stderr(contains("Context conflict detected:"))
        .stderr(contains("20-architecture.md"));
    assert_eq!(
        project.remote.read_file("20-architecture.md"),
        Some(a_content)
    );
    ctx_sync(&project.b)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("CONFLICT"))
        .stdout(contains("20-architecture.md"));
}

#[test]
fn agent_finish_is_visible_to_the_next_agent_start() {
    let project = Project::new();
    ctx_sync(&project.a)
        .args(["register", "a"])
        .assert()
        .success();
    project.attach_b();
    ctx_sync(&project.b)
        .args(["register", "b"])
        .assert()
        .success();

    ctx_sync(&project.a)
        .args([
            "agent",
            "finish",
            "--summary",
            "Implemented parser",
            "--attention",
            "ParserResult changed",
        ])
        .assert()
        .success();
    ctx_sync(&project.b)
        .args(["agent", "start"])
        .assert()
        .success()
        .stdout(contains("Implemented parser"))
        .stdout(contains("ParserResult changed"));
}
