use predicates::prelude::*;
use serde_json::json;

use crate::helpers::{self, TestEnv};

#[test]
fn history_empty() {
    let env = TestEnv::new();

    env.cmd()
        .arg("history")
        .assert()
        .success()
        .stdout(predicate::str::contains("no swap history"));
}

#[test]
fn history_shows_entries() {
    let env = TestEnv::new();
    env.write_swap_history(&json!([
        {
            "timestamp": "2026-03-18T06:00:00Z",
            "from_account": "personal",
            "to_account": "work",
            "reason": "seven_day utilization 96% >= threshold 95%",
            "trigger": "stop-hook",
            "session_id": "sess-001",
            "cwd": "/home/user/project",
            "temp_swap": false
        },
        {
            "timestamp": "2026-03-18T05:00:00Z",
            "from_account": "work",
            "to_account": "personal",
            "reason": "manual switch",
            "trigger": "manual",
            "cwd": "/tmp",
            "temp_swap": false
        }
    ]));

    let output = env.cmd().arg("history").assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("personal"));
    assert!(stdout.contains("work"));
    assert!(stdout.contains("stop-hook"));
    assert!(stdout.contains("manual"));
    // Session ID and CWD shown
    assert!(stdout.contains("session: sess-001"));
    assert!(stdout.contains("cwd: /home/user/project"));
    assert!(stdout.contains("cwd: /tmp"));
}

#[test]
fn history_count_flag() {
    let env = TestEnv::new();
    // Create 5 entries
    let entries: Vec<_> = (0..5)
        .map(|i| {
            json!({
                "timestamp": format!("2026-03-18T0{}:00:00Z", i),
                "from_account": "a",
                "to_account": "b",
                "reason": format!("swap {i}"),
                "trigger": "manual",
                "temp_swap": false
            })
        })
        .collect();
    env.write_swap_history(&json!(entries));

    let output = env.cmd().args(["history", "-n", "2"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Should show "3 more entries"
    assert!(stdout.contains("3 more entries"));
}

#[test]
fn history_clear() {
    let env = TestEnv::new();
    env.write_swap_history(&json!([{
        "timestamp": "2026-03-18T06:00:00Z",
        "from_account": "a",
        "to_account": "b",
        "reason": "test",
        "trigger": "manual",
        "temp_swap": false
    }]));

    env.cmd()
        .args(["history", "--clear"])
        .assert()
        .success()
        .stderr(predicate::str::contains("cleared"));

    // File should be gone
    assert!(!env.root.path().join("data/swap-history.json").exists());

    // Subsequent history shows empty
    env.cmd()
        .arg("history")
        .assert()
        .success()
        .stdout(predicate::str::contains("no swap history"));
}

#[test]
fn history_logged_on_manual_switch() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");

    env.cmd().args(["switch", "work"]).assert().success();

    let history = env.read_swap_history();
    let entries = history.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["trigger"], "manual");
    assert_eq!(entries[0]["from_account"], "personal");
    assert_eq!(entries[0]["to_account"], "work");
    assert_eq!(entries[0]["reason"], "manual switch");
    assert_eq!(entries[0]["temp_swap"], false);
    assert!(entries[0]["session_id"].is_null());
}

#[test]
fn history_logged_on_wrap_precheck() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 30.0),
    ]));

    env.cmd_with_fake_claude()
        .arg("wrap")
        .assert()
        .success();

    let history = env.read_swap_history();
    let entries = history.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["trigger"], "precheck");
    assert_eq!(entries[0]["from_account"], "personal");
    assert_eq!(entries[0]["to_account"], "work");
    assert!(entries[0]["reason"].as_str().unwrap().contains("seven_day"));
}

#[test]
fn history_cap_at_1000() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");

    // Pre-populate with 1000 entries
    let entries: Vec<_> = (0..1000)
        .map(|i| {
            json!({
                "timestamp": format!("2026-03-17T{:02}:{:02}:00Z", i / 60, i % 60),
                "from_account": "old",
                "to_account": "old",
                "reason": format!("old entry {i}"),
                "trigger": "manual",
                "temp_swap": false
            })
        })
        .collect();
    env.write_swap_history(&json!(entries));

    // Trigger one more swap via manual switch
    env.cmd().args(["switch", "work"]).assert().success();

    let history = env.read_swap_history();
    let entries = history.as_array().unwrap();
    // Capped at 1000
    assert_eq!(entries.len(), 1000);
    // Newest is first
    assert_eq!(entries[0]["trigger"], "manual");
    assert_eq!(entries[0]["from_account"], "personal");
    assert_eq!(entries[0]["to_account"], "work");
}
