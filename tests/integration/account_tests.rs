use predicates::prelude::*;
use serde_json::json;

use crate::helpers::{self, TestEnv};

#[test]
fn add_stores_credentials() {
    let env = TestEnv::new();
    env.write_live_credentials(&helpers::fake_credentials("personal"));

    env.cmd()
        .args(["add", "personal"])
        .assert()
        .success();

    // Credentials stored
    let stored = env.read_stored_creds("personal");
    assert_eq!(
        stored["claudeAiOauth"]["accessToken"],
        "sk-ant-oat01-fake-personal"
    );
    // Set as active (first account)
    assert_eq!(env.read_active(), "personal");
}

#[test]
fn add_second_preserves_active() {
    let env = TestEnv::new();
    env.write_live_credentials(&helpers::fake_credentials("personal"));
    env.cmd().args(["add", "personal"]).assert().success();

    // Write different live creds for second account
    env.write_live_credentials(&helpers::fake_credentials("work"));
    env.cmd().args(["add", "work"]).assert().success();

    // Active stays on first account
    assert_eq!(env.read_active(), "personal");
    // Both stored correctly
    assert_eq!(
        env.read_stored_creds("work")["claudeAiOauth"]["accessToken"],
        "sk-ant-oat01-fake-work"
    );
}

#[test]
fn add_duplicate_fails() {
    let env = TestEnv::new();
    env.write_live_credentials(&helpers::fake_credentials("personal"));
    env.cmd().args(["add", "personal"]).assert().success();

    env.cmd()
        .args(["add", "personal"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn add_invalid_name_fails() {
    let env = TestEnv::new();
    env.write_live_credentials(&helpers::fake_credentials("test"));

    env.cmd()
        .args(["add", "bad name!"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid account name"));
}

#[test]
fn list_shows_accounts() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 30.0),
        ("work", 20.0, 60.0),
    ]));

    let output = env.cmd().arg("list").assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("personal"));
    assert!(stdout.contains("work"));
    // Active marker
    assert!(stdout.contains("*"));
}

#[test]
fn list_empty() {
    let env = TestEnv::new();
    env.cmd()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("no accounts"));
}

#[test]
fn switch_swaps_credentials() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");

    env.cmd().args(["switch", "work"]).assert().success();

    // Active changed
    assert_eq!(env.read_active(), "work");
    // Live creds are now work's
    assert_eq!(
        env.read_live_creds()["claudeAiOauth"]["accessToken"],
        "sk-ant-oat01-fake-work"
    );
    // Personal's stored creds updated from live (sync-back during swap)
    assert_eq!(
        env.read_stored_creds("personal")["claudeAiOauth"]["accessToken"],
        "sk-ant-oat01-fake-personal"
    );
}

#[test]
fn switch_to_same_is_noop() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");
    env.set_live_credentials("personal");

    env.cmd()
        .args(["switch", "personal"])
        .assert()
        .success()
        .stderr(predicate::str::contains("already on"));
}

#[test]
fn switch_nonexistent_fails() {
    let env = TestEnv::new();
    env.cmd()
        .args(["switch", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn remove_active_clears_active() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    env.cmd().args(["remove", "personal"]).assert().success();

    assert!(env.read_active().is_empty());
    assert!(!env.root.path().join("data/personal").exists());
}

#[test]
fn remove_nonexistent_fails() {
    let env = TestEnv::new();
    env.cmd()
        .args(["remove", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn sync_updates_stored() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");
    env.set_live_credentials("personal");

    // Simulate token refresh: write different live credentials
    let refreshed = json!({
        "claudeAiOauth": {
            "accessToken": "sk-ant-oat01-refreshed-token",
            "refreshToken": "sk-ant-ort01-fake-personal",
            "expiresAt": 9999999999999u64,
            "scopes": ["user:inference"],
            "subscriptionType": "max",
            "rateLimitTier": "default_claude_max_20x"
        }
    });
    // Wait a moment so mtime differs
    std::thread::sleep(std::time::Duration::from_millis(50));
    env.write_live_credentials(&refreshed);

    env.cmd().arg("sync").assert().success();

    // Stored now has the refreshed token
    assert_eq!(
        env.read_stored_creds("personal")["claudeAiOauth"]["accessToken"],
        "sk-ant-oat01-refreshed-token"
    );
}
