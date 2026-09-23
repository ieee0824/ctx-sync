use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn prints_version() {
    Command::cargo_bin("ctx-sync")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains("ctx-sync 0.1.0"));
}
