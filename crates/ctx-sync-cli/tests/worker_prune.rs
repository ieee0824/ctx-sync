mod common;

use common::{attach, context_repo, ctx_sync, stdout};
use ctx_sync_core::model::Worker;
use ctx_sync_testutil::{TestRemote, TestWorker};

fn registered(remote: &TestRemote, name: &str) -> TestWorker {
    let worker = TestWorker::new(remote);
    attach(&worker);
    ctx_sync(&worker)
        .args(["register", name])
        .assert()
        .success();
    ctx_sync(&worker).arg("sync").assert().success();
    worker
}

fn file_for(worker: &TestWorker, name: &str) -> String {
    std::fs::read_dir(context_repo(worker))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find_map(|path| {
            let contents = std::fs::read_to_string(&path).ok()?;
            (Worker::parse(&contents).ok()?.name == name)
                .then(|| path.file_name().unwrap().to_str().unwrap().to_string())
        })
        .expect("worker file")
}

#[test]
fn dry_run_keeps_done_worker_and_sync_removes_it() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = registered(&remote, "a");
    let b = registered(&remote, "b");
    ctx_sync(&b)
        .args(["done", "--summary", "finished"])
        .assert()
        .success();

    let output = ctx_sync(&a)
        .args(["worker", "prune", "--dry-run"])
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&output);
    assert!(
        text.starts_with("Would prune 1 worker(s)\n\n- b ("),
        "{text}"
    );
    assert!(text.contains(") done\n"), "{text}");
    let b_file = file_for(&a, "b");
    assert!(remote.read_file(&b_file).is_some());

    let output = ctx_sync(&a)
        .args(["worker", "prune", "--sync"])
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&output);
    assert!(text.starts_with("Pruned 1 worker(s)\n\n- b ("), "{text}");
    assert!(text.contains("committed: "), "{text}");
    assert!(text.contains("synced: "), "{text}");
    assert!(remote.read_file(&b_file).is_none());
    assert!(remote.read_file(&file_for(&a, "a")).is_some());

    ctx_sync(&a)
        .args(["worker", "prune"])
        .assert()
        .success()
        .stdout("Nothing to prune\n");
}

#[test]
fn stale_threshold_removes_other_active_worker_but_keeps_self() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = registered(&remote, "a");
    let _b = registered(&remote, "b");
    let output = ctx_sync(&a)
        .args(["worker", "prune", "--stale-after", "1d", "--sync"])
        .env("CTX_SYNC_NOW", "2026-09-26T18:00:00+09:00")
        .assert()
        .success()
        .get_output()
        .clone();
    let text = stdout(&output);
    assert!(text.contains("Pruned 1 worker(s)"), "{text}");
    assert!(text.contains("- b ("), "{text}");
    assert!(text.contains("stale, last updated 3d ago"), "{text}");
    assert!(remote.read_file(&file_for(&a, "a")).is_some());
}

#[test]
fn bad_duration_is_a_usage_error() {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let a = registered(&remote, "a");
    ctx_sync(&a)
        .args(["worker", "prune", "--stale-after", "later"])
        .assert()
        .code(1);
}
