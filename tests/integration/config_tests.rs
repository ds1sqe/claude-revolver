use predicates::prelude::*;

use crate::helpers::TestEnv;

#[test]
fn config_show_defaults() {
    let env = TestEnv::new();

    env.cmd()
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"poll_interval_seconds\": 300"))
        .stdout(predicate::str::contains("\"five_hour\": 90"))
        .stdout(predicate::str::contains("\"seven_day\": 95"))
        .stdout(predicate::str::contains("\"drain\""));
}

#[test]
fn config_set_threshold_persists() {
    let env = TestEnv::new();

    env.cmd()
        .args(["config", "set", "thresholds.five_hour", "80"])
        .assert()
        .success();

    env.cmd()
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"five_hour\": 80"));
}

#[test]
fn config_set_strategy_type() {
    let env = TestEnv::new();

    env.cmd()
        .args(["config", "set", "strategy.type", "balanced"])
        .assert()
        .success();

    env.cmd()
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"balanced\""));
}

#[test]
fn config_set_auto_resume_false() {
    let env = TestEnv::new();

    env.cmd()
        .args(["config", "set", "auto_resume", "false"])
        .assert()
        .success();

    env.cmd()
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"auto_resume\": false"));
}
