mod common;

use common::ctx_sync;
use ctx_sync_testutil::{TestRemote, TestWorker};

const LATER: &str = "2026-09-26T18:00:00+09:00";

fn older_worker() -> (TestRemote, TestWorker) {
    let remote = TestRemote::new();
    remote.seed_context("demo");
    let worker = TestWorker::new(&remote);
    common::attach(&worker);
    ctx_sync(&worker)
        .args(["register", "parser"])
        .assert()
        .success();
    ctx_sync(&worker)
        .args(["handoff", "--task", "Parser implementation", "--sync"])
        .assert()
        .success();
    (remote, worker)
}

fn output(worker: &TestWorker, args: &[&str]) -> String {
    let output = ctx_sync(worker)
        .args(args)
        .env("CTX_SYNC_NOW", LATER)
        .assert()
        .success()
        .get_output()
        .clone();
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn default_and_custom_thresholds_apply_to_all_views() {
    let (_remote, worker) = older_worker();
    for (base, stale_marker) in [
        (vec!["status"], "working (stale, 3d)"),
        (vec!["context"], "working (stale: last updated 3d ago)"),
        (vec!["onboard"], "working, stale (3d)"),
        (vec!["agent", "start"], "working, stale (3d)"),
    ] {
        let text = output(&worker, &base);
        assert!(text.contains(stale_marker), "{base:?}: {text}");

        let mut custom = base.clone();
        custom.extend(["--stale-after", "1w"]);
        let text = output(&worker, &custom);
        assert!(!text.contains("stale"), "{custom:?}: {text}");
    }
}

#[test]
fn invalid_duration_is_a_cli_argument_error() {
    let (_remote, worker) = older_worker();
    for mut args in [
        vec!["status"],
        vec!["context"],
        vec!["onboard"],
        vec!["agent", "start"],
    ] {
        args.extend(["--stale-after", "3x"]);
        ctx_sync(&worker).args(&args).assert().code(1);
    }
}
