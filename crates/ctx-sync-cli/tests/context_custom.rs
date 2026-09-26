mod common;

use ctx_sync_core::config::{CONFIG_FILE, ProjectConfig};
use ctx_sync_testutil::{TestRemote, TestWorker};

use common::{context_repo, ctx_sync, stdout};

#[test]
fn context_and_bootstrap_use_configured_file_names() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    remote.push_file(
        "overview.md",
        "# Project\n\n## Goal\n\nCustom goal.\n",
        "custom project",
    );
    remote.push_file(
        "design.md",
        "# Architecture\n\n## Components\n\n- custom\n",
        "custom architecture",
    );
    let worker = TestWorker::new(&remote);
    let mut config = ProjectConfig::new_gist(common::GIST);
    config.context.project = "overview.md".into();
    config.context.architecture = "design.md".into();
    config.save(&worker.project().join(CONFIG_FILE)).unwrap();
    common::attach(&worker);

    let output = ctx_sync(&worker)
        .arg("context")
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&output);
    assert!(text.contains("Custom goal."));
    assert!(text.contains("- custom"));
    assert!(!text.contains("Test project."));

    ctx_sync(&worker)
        .args(["bootstrap", "--apply"])
        .assert()
        .success();
    let project = std::fs::read_to_string(context_repo(&worker).join("overview.md")).unwrap();
    assert!(project.contains("## Initial Context"));
    let default = std::fs::read_to_string(context_repo(&worker).join("10-project.md")).unwrap();
    assert!(!default.contains("## Initial Context"));
}
