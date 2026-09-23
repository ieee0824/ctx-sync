use ctx_sync_core::repo_info::{RepoInfo, collect};
use ctx_sync_testutil::{TestRemote, TestWorker};

#[test]
fn clean_repository() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    let info = collect(worker.project(), &worker.git_env()).unwrap();
    assert_eq!(info.branch.as_deref(), Some("main"));
    let head = worker.git(&["rev-parse", "--short", "HEAD"]);
    assert_eq!(info.commit.as_deref(), Some(head.trim()));
    assert!(info.changed_files.is_empty());
}

#[test]
fn uncommitted_changes_are_listed() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    std::fs::write(worker.project().join("README.md"), "changed\n").unwrap();
    std::fs::create_dir_all(worker.project().join("src")).unwrap();
    std::fs::write(worker.project().join("src/new.rs"), "fn main() {}\n").unwrap();
    std::fs::write(worker.project().join("a b.txt"), "x").unwrap();
    let info = collect(worker.project(), &worker.git_env()).unwrap();
    assert_eq!(info.changed_files, ["README.md", "a b.txt", "src/new.rs"]);
}

#[test]
fn renamed_files_use_the_new_name() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    worker.git(&["mv", "README.md", "GUIDE.md"]);
    let info = collect(worker.project(), &worker.git_env()).unwrap();
    assert_eq!(info.changed_files, ["GUIDE.md"]);
}

#[test]
fn committed_changes_since_the_upstream_are_listed() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    // A project remote whose default branch is main.
    let upstream = tempfile::tempdir().unwrap();
    let bare = upstream.path().join("project.git");
    worker.git(&["init", "-q", "--bare", bare.to_str().unwrap()]);
    worker.git(&["remote", "add", "origin", bare.to_str().unwrap()]);
    worker.git(&["push", "-q", "origin", "main"]);
    worker.git(&["remote", "set-head", "origin", "main"]);

    worker.git(&["checkout", "-q", "-b", "feat/parser"]);
    std::fs::write(worker.project().join("parser.rs"), "// parser\n").unwrap();
    worker.git(&["add", "parser.rs"]);
    worker.git(&["commit", "-q", "-m", "add parser"]);
    std::fs::write(worker.project().join("wip.rs"), "// wip\n").unwrap();

    let info = collect(worker.project(), &worker.git_env()).unwrap();
    assert_eq!(info.branch.as_deref(), Some("feat/parser"));
    assert_eq!(info.changed_files, ["parser.rs", "wip.rs"]);
}

#[test]
fn detached_head_has_no_branch() {
    let remote = TestRemote::new();
    let worker = TestWorker::new(&remote);
    worker.git(&["checkout", "-q", "--detach"]);
    let info = collect(worker.project(), &worker.git_env()).unwrap();
    assert!(info.branch.is_none());
    assert!(info.commit.is_some());
}

#[test]
fn non_git_directory_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let env = ctx_sync_testutil::git_env(&dir.path().join("config"));
    let project = dir.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    assert_eq!(collect(&project, &env).unwrap(), RepoInfo::default());
}
