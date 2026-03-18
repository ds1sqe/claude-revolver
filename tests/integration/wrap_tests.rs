use predicates::prelude::*;

use crate::helpers::{self, TestEnv};

#[test]
fn wrap_pre_check_swaps_before_launch() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0), // over 95% threshold
        ("work", 10.0, 20.0),
    ]));

    env.cmd_with_fake_claude()
        .arg("wrap")
        .assert()
        .success()
        .stderr(predicate::str::contains("auto-switched to 'work'"));

    // Pre-check swap happened (wrapper owns the swap)
    assert_eq!(env.read_active(), "work");
    assert_eq!(
        env.read_live_creds()["claudeAiOauth"]["accessToken"],
        "sk-ant-oat01-fake-work"
    );
}

#[test]
fn wrap_launches_claude_with_env() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");
    env.set_live_credentials("personal");

    env.cmd_with_fake_claude()
        .args(["wrap", "--", "--verbose"])
        .assert()
        .success();

    let invocations = env.read_claude_invocations();
    assert!(!invocations.is_empty());
    assert!(invocations[0].contains("--verbose"));
}

#[test]
fn wrap_no_accounts_launches_without_management() {
    let env = TestEnv::new();

    env.cmd_with_fake_claude()
        .arg("wrap")
        .assert()
        .success()
        .stderr(predicate::str::contains("no active account"));
}

#[test]
fn wrap_clears_signals_on_normal_exit() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");
    env.set_live_credentials("personal");

    env.cmd_with_fake_claude()
        .arg("wrap")
        .assert()
        .success();

    // signals dir should be clean (or not exist)
    let signals_dir = env.data_dir.join("signals");
    if signals_dir.exists() {
        let entries: Vec<_> = std::fs::read_dir(&signals_dir)
            .unwrap()
            .collect();
        assert!(entries.is_empty(), "signals should be cleaned up on normal exit");
    }
}
