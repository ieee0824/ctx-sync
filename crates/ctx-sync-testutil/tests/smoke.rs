use ctx_sync_testutil::{FIXED_NOW, SEED_PROJECT_ID, TestRemote, TestWorker, git};

#[test]
fn remote_starts_with_readme() {
    let remote = TestRemote::new();
    assert!(remote.read_file("README.md").is_some());
    assert_eq!(remote.log(), vec!["initial commit"]);
}

#[test]
fn push_file_adds_a_commit() {
    let remote = TestRemote::new();
    remote.push_file("a.md", "x", "add a");
    assert_eq!(remote.read_file("a.md").as_deref(), Some("x"));
    assert_eq!(remote.log()[0], "add a");

    remote.push_file("b.md", "y", "add b");
    assert_eq!(remote.log(), vec!["add b", "add a", "initial commit"]);
    assert_eq!(remote.read_file("a.md").as_deref(), Some("x"));
}

#[test]
fn read_file_of_missing_file_is_none() {
    let remote = TestRemote::new();
    assert!(remote.read_file("nope.md").is_none());
}

#[test]
fn seed_context_pushes_ctx_sync_files() {
    let remote = TestRemote::new();
    assert_eq!(remote.seed_context("demo"), SEED_PROJECT_ID);
    let meta = remote.read_file("00-meta.json").unwrap();
    assert!(meta.contains(SEED_PROJECT_ID), "{meta}");
    assert!(meta.contains("\"project_name\":\"demo\""), "{meta}");
    assert!(
        remote
            .read_file("10-project.md")
            .unwrap()
            .starts_with("# Project")
    );
    assert!(
        remote
            .read_file("20-architecture.md")
            .unwrap()
            .starts_with("# Architecture")
    );
}

#[test]
fn worker_project_is_on_main() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    assert_eq!(
        worker.git(&["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "main"
    );
    assert!(worker.state_home().is_dir());
}

#[test]
fn worker_env_points_to_remote_and_state_home() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    let env = worker.env();
    let get = |key: &str| {
        env.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("{key} missing"))
    };
    assert_eq!(get("CTX_SYNC_HOME"), worker.state_home().to_string_lossy());
    assert_eq!(get("CTX_SYNC_REMOTE_URL_OVERRIDE"), remote.url());
    assert_eq!(get("CTX_SYNC_NOW"), FIXED_NOW);
    assert_eq!(get("GIT_CONFIG_NOSYSTEM"), "1");
}

#[test]
fn git_config_is_isolated() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    let email = git(
        worker.project(),
        &worker.git_env(),
        &["config", "--global", "user.email"],
    );
    assert_eq!(email.trim(), "test@example.com");
    // Only the test gitconfig is visible: the developer's global settings
    // such as core.hooksPath or commit.gpgsign must not leak in.
    let global = worker.git(&["config", "--global", "--list"]);
    assert!(!global.to_lowercase().contains("hookspath"), "{global}");
    assert!(!global.to_lowercase().contains("gpgsign"), "{global}");
    assert!(global.contains("init.defaultbranch=main"), "{global}");
}

#[test]
fn worker_can_clone_the_remote() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let worker = TestWorker::new(&remote);
    let clone = worker.project().join("clone");
    git(
        worker.project(),
        &worker.git_env(),
        &["clone", "-q", &remote.url(), clone.to_str().unwrap()],
    );
    assert!(clone.join("00-meta.json").is_file());
}
