mod common;

use common::{GIST, ctx_sync, stdout};
use ctx_sync_core::config::{CONFIG_FILE, ProjectConfig};
use ctx_sync_core::model::Meta;
use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::str::contains;

fn project_id(output: &std::process::Output) -> String {
    stdout(output)
        .lines()
        .find_map(|l| l.strip_prefix("project id: ").map(str::to_string))
        .expect("project id line")
}

#[test]
fn initializes_a_project_without_input() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    let output = ctx_sync(&worker)
        .args(["init", "--project", "demo", "--gist-id", GIST, "--no-input"])
        .assert()
        .success()
        .stdout(contains("Initialized ctx-sync project"))
        .stdout(contains("project: demo"))
        .stdout(contains("gist: testgist01"))
        .stdout(contains("config: .ctx-sync.toml"))
        .stdout(contains("ctx-sync register <name>"))
        .get_output()
        .clone();
    let config = ProjectConfig::load(&worker.project().join(CONFIG_FILE)).unwrap();
    assert_eq!(config.remote.id, GIST);
    let meta = Meta::parse(&remote.read_file("00-meta.json").unwrap()).unwrap();
    assert_eq!(meta.project_id.to_string(), project_id(&output));
}

#[test]
fn project_name_defaults_to_the_directory_name() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    // stdin is not a terminal, so no prompt is shown even without --no-input.
    ctx_sync(&worker)
        .args(["init", "--gist-id", GIST])
        .assert()
        .success()
        .stdout(contains("project: project"));
}

#[test]
fn second_init_exits_with_4() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["init", "--project", "demo", "--gist-id", GIST, "--no-input"])
        .assert()
        .success();
    ctx_sync(&worker)
        .args(["init", "--project", "demo", "--gist-id", GIST, "--no-input"])
        .assert()
        .code(4)
        .stderr(contains("--force"));
    ctx_sync(&worker)
        .args([
            "init",
            "--project",
            "demo",
            "--gist-id",
            GIST,
            "--no-input",
            "--force",
        ])
        .assert()
        .code(4)
        .stderr(contains("ctx-sync attach"));
}

#[test]
fn remote_choice_is_required_without_input() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["init", "--no-input"])
        .assert()
        .code(4)
        .stderr(contains("--create-gist or --gist-id"));
}

#[test]
fn create_gist_and_gist_id_conflict() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["init", "--create-gist", "--gist-id", "x"])
        .assert()
        .code(1);
}

#[test]
fn another_worker_can_attach_register_and_sync() {
    let remote = TestRemote::new();
    let a = TestWorker::new(&remote);
    let output = ctx_sync(&a)
        .args(["init", "--project", "demo", "--gist-id", GIST, "--no-input"])
        .assert()
        .success()
        .get_output()
        .clone();
    let id = project_id(&output);

    let b = TestWorker::new(&remote);
    ctx_sync(&b)
        .args(["attach", GIST])
        .assert()
        .success()
        .stdout(contains(format!("project id: {id}")));
    ctx_sync(&b).args(["register", "b"]).assert().success();
    ctx_sync(&b)
        .arg("sync")
        .assert()
        .success()
        .stdout(contains("Synced ("));
}
