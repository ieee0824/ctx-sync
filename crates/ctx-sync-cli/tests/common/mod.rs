//! Helpers shared by the CLI integration tests.
#![allow(dead_code)]

use std::path::PathBuf;

use assert_cmd::Command;
use ctx_sync_core::state::StateRoot;
use ctx_sync_testutil::{SEED_PROJECT_ID, TestWorker};
use uuid::Uuid;

pub const GIST: &str = "testgist01";

/// `ctx-sync` running in the worker's project with the worker's environment.
pub fn ctx_sync(worker: &TestWorker) -> Command {
    let mut cmd = Command::cargo_bin("ctx-sync").unwrap();
    cmd.current_dir(worker.project()).envs(worker.env());
    cmd
}

/// Runs `ctx-sync attach testgist01` and asserts success.
pub fn attach(worker: &TestWorker) {
    ctx_sync(worker).args(["attach", GIST]).assert().success();
}

/// Context repository of the seeded project for this worker.
pub fn context_repo(worker: &TestWorker) -> PathBuf {
    StateRoot::new(worker.state_home())
        .project(&Uuid::parse_str(SEED_PROJECT_ID).unwrap())
        .context_repo()
}

pub fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}
