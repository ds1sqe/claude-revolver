use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("claude-revolver").unwrap()
}

#[test]
fn help_exits_zero() {
    cmd().arg("help").assert().success().stdout(
        predicate::str::contains("Multi-account OAuth credential manager"),
    );
}

#[test]
fn version_flag() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("claude-revolver"));
}
