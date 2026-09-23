mod common;

use common::{GIST, context_repo, ctx_sync};
use ctx_sync_core::config::{CONFIG_FILE, ProjectConfig};
use ctx_sync_testutil::{SEED_PROJECT_ID, TestRemote, TestWorker};
use predicates::str::contains;

fn seeded_remote() -> TestRemote {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    remote
}

#[test]
fn attaches_to_a_seeded_gist() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["attach", GIST])
        .assert()
        .success()
        .stdout(contains("Attached to ctx-sync project"))
        .stdout(contains("project: demo"))
        .stdout(contains(format!("project id: {SEED_PROJECT_ID}")))
        .stdout(contains("gist: testgist01"))
        .stdout(contains("config: .ctx-sync.toml (created)"));

    let config = ProjectConfig::load(&worker.project().join(CONFIG_FILE)).unwrap();
    assert_eq!(config.remote.id, GIST);
    assert!(context_repo(&worker).join("00-meta.json").is_file());
}

#[test]
fn accepts_a_gist_url() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["attach", "https://gist.github.com/someone/testgist01"])
        .assert()
        .success()
        .stdout(contains(format!("project id: {SEED_PROJECT_ID}")))
        .stdout(contains("gist: testgist01"));
}

#[test]
fn attaching_again_keeps_the_config() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    remote.push_file("40-worker-cccccccc.md", "c", "other worker");
    ctx_sync(&worker)
        .args(["attach", GIST])
        .assert()
        .success()
        .stdout(contains("config: .ctx-sync.toml (unchanged)"));
    // The existing clone was reused and updated.
    assert!(
        context_repo(&worker)
            .join("40-worker-cccccccc.md")
            .is_file()
    );
}

#[test]
fn attaches_from_a_subdirectory_to_the_repository_root() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    let sub = worker.project().join("src/deep");
    std::fs::create_dir_all(&sub).unwrap();
    ctx_sync(&worker)
        .current_dir(&sub)
        .args(["attach", GIST])
        .assert()
        .success();
    assert!(worker.project().join(CONFIG_FILE).is_file());
    assert!(!sub.join(CONFIG_FILE).exists());
}

#[test]
fn gist_without_meta_is_rejected() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["attach", GIST])
        .assert()
        .code(4)
        .stderr(contains("not a ctx-sync context"));
    assert!(!worker.project().join(CONFIG_FILE).exists());
}

#[test]
fn another_gist_in_the_config_is_rejected() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    ProjectConfig::new_gist("othergist")
        .save(&worker.project().join(CONFIG_FILE))
        .unwrap();
    ctx_sync(&worker)
        .args(["attach", GIST])
        .assert()
        .code(4)
        .stderr(contains("already attached to gist othergist"));
}

#[test]
fn invalid_gist_id_is_rejected() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["attach", "not/a/gist"])
        .assert()
        .code(4);
}

#[test]
fn sync_works_after_attach() {
    let remote = seeded_remote();
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .arg("sync")
        .assert()
        .success()
        .stdout(contains("Nothing to push ("));
}
