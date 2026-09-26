mod common;

use common::{GIST, ctx_sync, stdout};
use ctx_sync_core::config::{CONFIG_FILE, ProjectConfig};
use ctx_sync_core::model::Meta;
use ctx_sync_testutil::{TestRemote, TestWorker};
use predicates::str::contains;

#[test]
fn custom_names_are_published_and_inherited_on_attach() {
    let remote = TestRemote::new();
    let a = TestWorker::new(&remote);
    ctx_sync(&a)
        .args([
            "init",
            "--project",
            "demo",
            "--project-file",
            "overview.md",
            "--architecture-file",
            "design.md",
            "--gist-id",
            GIST,
            "--no-input",
        ])
        .assert()
        .success();
    assert!(remote.read_file("overview.md").unwrap().contains("demo"));
    assert!(remote.read_file("design.md").is_some());
    assert!(remote.read_file("10-project.md").is_none());
    let meta = Meta::parse(&remote.read_file("00-meta.json").unwrap()).unwrap();
    assert_eq!(meta.project_file.as_deref(), Some("overview.md"));
    assert_eq!(meta.architecture_file.as_deref(), Some("design.md"));
    let config = ProjectConfig::load(&a.project().join(CONFIG_FILE)).unwrap();
    assert_eq!(config.context.project, "overview.md");

    let b = TestWorker::new(&remote);
    ctx_sync(&b).args(["attach", GIST]).assert().success();
    let config = ProjectConfig::load(&b.project().join(CONFIG_FILE)).unwrap();
    assert_eq!(config.context.project, "overview.md");
    assert_eq!(config.context.architecture, "design.md");
    let output = ctx_sync(&b)
        .arg("context")
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(stdout(&output).contains("demo"));
}

#[test]
fn invalid_name_is_rejected_before_remote_write() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args([
            "init",
            "--project-file",
            "a/b.md",
            "--gist-id",
            GIST,
            "--no-input",
        ])
        .assert()
        .code(4)
        .stderr(contains("context.project"));
    assert!(remote.read_file("00-meta.json").is_none());
}

#[test]
fn default_names_do_not_add_meta_fields() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    ctx_sync(&worker)
        .args(["init", "--gist-id", GIST, "--no-input"])
        .assert()
        .success();
    let meta = remote.read_file("00-meta.json").unwrap();
    assert!(!meta.contains("project_file"));
    assert!(!meta.contains("architecture_file"));
}

#[test]
fn existing_config_wins_and_mismatch_warns() {
    let remote = TestRemote::new();
    let a = TestWorker::new(&remote);
    ctx_sync(&a)
        .args([
            "init",
            "--project-file",
            "overview.md",
            "--gist-id",
            GIST,
            "--no-input",
        ])
        .assert()
        .success();
    let b = TestWorker::new(&remote);
    let mut config = ProjectConfig::new_gist(GIST);
    config.context.project = "other.md".into();
    config.save(&b.project().join(CONFIG_FILE)).unwrap();
    ctx_sync(&b)
        .args(["attach", GIST])
        .assert()
        .success()
        .stderr(contains(
            "warning: .ctx-sync.toml context file names differ",
        ));
    let loaded = ProjectConfig::load(&b.project().join(CONFIG_FILE)).unwrap();
    assert_eq!(loaded.context.project, "other.md");
}
