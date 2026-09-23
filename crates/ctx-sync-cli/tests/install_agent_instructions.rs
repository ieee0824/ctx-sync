mod common;

use common::{ctx_sync, stdout};
use ctx_sync_testutil::{TestRemote, TestWorker, git};
use predicates::str::contains;

#[test]
fn creates_once_in_the_worktree_root_without_a_config_or_commit() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    let root = worker.project();
    let subdir = root.join("nested");
    std::fs::create_dir_all(&subdir).unwrap();
    let before = git(root, &worker.git_env(), &["rev-parse", "HEAD"]);

    ctx_sync(&worker)
        .arg("install-agent-instructions")
        .current_dir(&subdir)
        .assert()
        .success()
        .stdout(contains("Created AGENTS.md\n"));
    assert!(!root.join(".ctx-sync.toml").exists());
    assert!(!subdir.join("AGENTS.md").exists());
    let path = root.join("AGENTS.md");
    let first = std::fs::read_to_string(&path).unwrap();
    assert!(first.starts_with("# AGENTS.md\n\n## Shared Context\n"));

    let second_output = ctx_sync(&worker)
        .arg("install-agent-instructions")
        .assert()
        .success()
        .get_output()
        .clone();
    assert_eq!(
        stdout(&second_output),
        "AGENTS.md already contains a Shared Context section\n"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
    assert_eq!(first.matches("## Shared Context").count(), 1);
    assert_eq!(git(root, &worker.git_env(), &["rev-parse", "HEAD"]), before);
}

#[test]
fn appends_and_preserves_the_existing_document() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    let path = worker.project().join("AGENTS.md");
    let existing = "# Team rules\nKeep this byte-for-byte.";
    std::fs::write(&path, existing).unwrap();

    ctx_sync(&worker)
        .arg("install-agent-instructions")
        .assert()
        .success()
        .stdout(contains("Appended Shared Context section to AGENTS.md\n"));
    let result = std::fs::read_to_string(path).unwrap();
    assert!(result.starts_with(existing));
    assert!(result.contains("\n\n## Shared Context\n"));
    assert_eq!(result.matches("## Shared Context").count(), 1);
}
