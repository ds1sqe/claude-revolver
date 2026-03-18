use serde_json::json;

use crate::helpers::{self, TestEnv};

#[test]
fn wrap_pre_check_swaps_before_launch() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    // personal over threshold
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 30.0),
    ]));

    env.cmd_with_fake_claude()
        .args(["wrap", "--", "hello"])
        .assert()
        .success();

    // Should have pre-swapped to work before launching
    assert_eq!(env.read_active(), "work");
    // Claude was invoked
    let invocations = env.read_claude_invocations();
    assert!(!invocations.is_empty());
}

#[test]
fn wrap_no_precheck_when_under_threshold() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 40.0),
        ("work", 10.0, 30.0),
    ]));

    env.cmd_with_fake_claude()
        .args(["wrap", "--", "hello"])
        .assert()
        .success();

    // No swap — stays on personal
    assert_eq!(env.read_active(), "personal");
    let invocations = env.read_claude_invocations();
    assert!(!invocations.is_empty());
}

#[test]
fn wrap_auto_resume_on_swap_info() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");

    // Stage swap-info: fake claude will copy it to swap-info during "session"
    env.stage_swap_info(&json!({
        "session_id": "sess-001",
        "from_account": "personal",
        "to_account": "work",
        "reason": "seven_day over threshold",
        "swapped_at": "2026-03-18T06:00:00Z"
    }));

    // Also need work's creds to be loadable by the live path after swap
    // (The swap-info means stop hook already did the swap, so active should be "work"
    //  and live creds should be work's. But since this is fake, we need to set it up.)
    // Actually, in the real flow: stop hook runs during claude session and does the swap.
    // The wrap command reads swap-info after claude exits and sees to_account=work.
    // The wrap command then sets active=work. But the swap already happened in the hook.
    // For our fake test: we need to simulate that swap already happened.
    env.set_live_credentials("work");

    env.cmd_with_fake_claude()
        .args(["wrap", "--", "hello"])
        .assert()
        .success();

    let invocations = env.read_claude_invocations();
    assert!(invocations.len() >= 2, "Expected at least 2 claude invocations, got: {:?}", invocations);
    // First invocation: original args
    assert!(invocations[0].contains("hello"));
    // Second invocation: auto-resume
    assert!(invocations[1].contains("--resume"));
    assert!(invocations[1].contains("sess-001"));
    assert!(invocations[1].contains("Go continue."));
}

#[test]
fn wrap_manual_resume_prints_instructions() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");

    env.write_config(&helpers::fake_config(json!({
        "auto_resume": false
    })));

    env.stage_swap_info(&json!({
        "session_id": "sess-002",
        "from_account": "personal",
        "to_account": "work",
        "reason": "seven_day over threshold",
        "swapped_at": "2026-03-18T06:00:00Z"
    }));

    env.set_live_credentials("work");

    let output = env
        .cmd_with_fake_claude()
        .args(["wrap", "--", "hello"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("--resume"));
    assert!(stdout.contains("sess-002"));

    // Only one invocation (no auto-resume)
    let invocations = env.read_claude_invocations();
    assert_eq!(invocations.len(), 1);
}

#[test]
fn wrap_clears_flags_before_launch() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");
    env.set_live_credentials("personal");

    // Pre-existing flags
    env.write_swap_info(&json!({
        "session_id": "old",
        "from_account": "a",
        "to_account": "b",
        "reason": "stale",
        "swapped_at": "2026-03-17T00:00:00Z"
    }));
    env.set_rate_limited();

    env.cmd_with_fake_claude()
        .args(["wrap", "--", "hello"])
        .assert()
        .success();

    // Both flags cleared by wrap before launching claude
    // After claude exits (with no new flags), they should be gone
    assert!(!env.swap_info_exists());
    assert!(!env.rate_limited_exists());
}
