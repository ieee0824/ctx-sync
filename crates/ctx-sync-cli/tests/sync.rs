//! `ctx-sync sync` against a local bare repository standing in for the Gist.
//!
//! `attach` is not implemented yet (#44), so the attached state is prepared
//! directly with the core API.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use ctx_sync_core::clock::parse_now;
use ctx_sync_core::config::{CONFIG_FILE, ProjectConfig};
use ctx_sync_core::state::{Index, LocalProject, Protocol, StateRoot};
use ctx_sync_core::store::{ContextStore, GistStore, RemoteSpec};
use ctx_sync_testutil::{FIXED_NOW, SEED_PROJECT_ID, TestRemote, TestWorker};
use predicates::str::contains;
use uuid::Uuid;

const GIST: &str = "testgist01";

/// Writes `.ctx-sync.toml`, `index.json` and `local.json`, and clones the
/// context repository. Returns the context repository path.
fn attach(worker: &TestWorker, remote: &TestRemote) -> PathBuf {
    let root = StateRoot::new(worker.state_home());
    let id = Uuid::parse_str(SEED_PROJECT_ID).unwrap();
    ProjectConfig::new_gist(GIST)
        .save(&worker.project().join(CONFIG_FILE))
        .unwrap();
    let mut index = Index::load(&root).unwrap();
    index.insert(GIST, id);
    index.save(&root).unwrap();
    let state = root.project(&id);
    LocalProject {
        gist_id: GIST.into(),
        protocol: Protocol::Https,
        attached_at: parse_now(Some(FIXED_NOW)).unwrap(),
    }
    .save(&state.local_json())
    .unwrap();
    GistStore::new(
        &state,
        RemoteSpec::new(GIST, Protocol::Https).with_url_override(Some(remote.url())),
    )
    .with_git_env(worker.git_env())
    .ensure()
    .unwrap();
    state.context_repo()
}

fn ctx_sync(worker: &TestWorker) -> Command {
    let mut cmd = Command::cargo_bin("ctx-sync").unwrap();
    cmd.current_dir(worker.project()).envs(worker.env());
    cmd
}

fn edit_architecture(repo: &Path, component: &str) {
    let path = repo.join("20-architecture.md");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("- core\n", &format!("- {component}\n"))).unwrap();
}

#[test]
fn sync_pushes_changes_then_has_nothing_to_push() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let worker = TestWorker::new(&remote);
    let repo = attach(&worker, &remote);
    std::fs::write(repo.join("40-worker-aaaaaaaa.md"), "x").unwrap();

    ctx_sync(&worker)
        .arg("sync")
        .assert()
        .success()
        .stdout(contains("Synced ("));
    assert_eq!(
        remote.read_file("40-worker-aaaaaaaa.md").as_deref(),
        Some("x")
    );

    ctx_sync(&worker)
        .arg("sync")
        .assert()
        .success()
        .stdout(contains("Nothing to push ("));
}

#[test]
fn conflicting_architecture_edits_exit_with_2() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = TestWorker::new(&remote);
    let b = TestWorker::new(&remote);
    let repo_a = attach(&a, &remote);
    let repo_b = attach(&b, &remote);
    edit_architecture(&repo_a, "core-a");
    edit_architecture(&repo_b, "core-b");

    ctx_sync(&a).arg("sync").assert().success();
    ctx_sync(&b)
        .arg("sync")
        .assert()
        .code(2)
        .stderr(contains("Context conflict detected:"))
        .stderr(contains("20-architecture.md"))
        .stderr(contains("ctx-sync status"));
    assert!(
        remote
            .read_file("20-architecture.md")
            .unwrap()
            .contains("- core-a\n")
    );
}

#[test]
fn sync_outside_an_attached_project_exits_with_4() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker).arg("sync").assert().code(4);

    ProjectConfig::new_gist(GIST)
        .save(&worker.project().join(CONFIG_FILE))
        .unwrap();
    ctx_sync(&worker)
        .arg("sync")
        .assert()
        .code(4)
        .stderr(contains("ctx-sync attach testgist01"));
}
