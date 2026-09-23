use assert_cmd::Command;
use predicates::str::contains;

fn ctx_sync() -> Command {
    Command::cargo_bin("ctx-sync").unwrap()
}

#[test]
fn help_lists_all_commands() {
    let assert = ctx_sync().arg("--help").assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    for command in [
        "init",
        "attach",
        "bootstrap",
        "register",
        "pull",
        "sync",
        "context",
        "onboard",
        "status",
        "handoff",
        "decision",
        "done",
        "agent",
        "install-agent-instructions",
    ] {
        assert!(
            stdout.lines().any(|l| l.trim_start().starts_with(command)),
            "{command} missing in:\n{stdout}"
        );
    }
}

#[test]
fn nested_help_succeeds() {
    ctx_sync()
        .args(["decision", "add", "--help"])
        .assert()
        .success();
    ctx_sync()
        .args(["agent", "finish", "--help"])
        .assert()
        .success();
}

#[test]
fn unimplemented_command_exits_1() {
    ctx_sync()
        .arg("install-agent-instructions")
        .assert()
        .code(1)
        .stderr(contains("not implemented: install-agent-instructions"));
}

#[test]
fn usage_error_exits_1() {
    ctx_sync().arg("--unknown-flag").assert().code(1);
}

#[test]
fn version_exits_0() {
    ctx_sync().arg("--version").assert().code(0);
}
