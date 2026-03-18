use serde_json::json;

use crate::helpers::{self, TestEnv};

// ── Guard conditions (no swap expected) ───────────────────────────────

#[test]
fn stop_noop_when_stop_hook_active() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 30.0),
    ]));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":true}"#)
        .assert()
        .success();

    assert!(!env.swap_info_exists());
    assert_eq!(env.read_active(), "personal");
}

#[test]
fn stop_noop_on_malformed_input() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin("not json at all")
        .assert()
        .success();

    assert!(!env.swap_info_exists());
}

#[test]
fn stop_noop_single_account() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[("personal", 99.0, 99.0)]));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    assert!(!env.swap_info_exists());
}

#[test]
fn stop_noop_no_cache_data() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    // No usage cache written

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    assert!(!env.swap_info_exists());
}

#[test]
fn stop_noop_under_threshold() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 40.0),
        ("work", 10.0, 30.0),
    ]));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    assert!(!env.swap_info_exists());
    assert_eq!(env.read_active(), "personal");
}

// ── Swap triggers ─────────────────────────────────────────────────────

#[test]
fn stop_swaps_when_seven_day_over_threshold() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 30.0),
    ]));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"sess-001","stop_hook_active":false}"#)
        .assert()
        .success();

    // Active changed
    assert_eq!(env.read_active(), "work");
    // Live creds are now work's
    assert_eq!(
        env.read_live_creds()["claudeAiOauth"]["accessToken"],
        "sk-ant-oat01-fake-work"
    );
    // Swap info written
    let info = env.read_swap_info();
    assert_eq!(info["from_account"], "personal");
    assert_eq!(info["to_account"], "work");
    assert_eq!(info["session_id"], "sess-001");
    assert!(info["reason"].as_str().unwrap().contains("seven_day"));
    // Not a temp swap (7d is over threshold)
    assert!(info["return_to"].is_null());
    assert!(info["return_after"].is_null());
}

#[test]
fn stop_five_hour_temp_swap() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    // 5h over threshold (92 >= 90), 7d under (40 < 95) → temp swap
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 92.0, 40.0),
        ("work", 10.0, 30.0),
    ]));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"sess-002","stop_hook_active":false}"#)
        .assert()
        .success();

    assert_eq!(env.read_active(), "work");
    let info = env.read_swap_info();
    assert_eq!(info["return_to"], "personal");
    // return_after should be the 5h resets_at timestamp
    assert_eq!(info["return_after"], "2026-03-18T10:00:00Z");
}

#[test]
fn stop_permanent_swap_no_return_fields() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    // Both 5h and 7d over → permanent swap (7d triggers, not a temp)
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 92.0, 96.0),
        ("work", 10.0, 30.0),
    ]));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"sess-003","stop_hook_active":false}"#)
        .assert()
        .success();

    let info = env.read_swap_info();
    // 7d triggers first in is_over_threshold, and 7d IS over → not a temp swap
    assert!(info["return_to"].is_null());
}

#[test]
fn stop_rate_limited_flag_triggers_swap() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    // Under threshold but rate-limited flag exists
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 40.0),
        ("work", 10.0, 30.0),
    ]));
    env.set_rate_limited();

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"sess-004","stop_hook_active":false}"#)
        .assert()
        .success();

    assert_eq!(env.read_active(), "work");
    let info = env.read_swap_info();
    assert!(info["reason"].as_str().unwrap().contains("rate limit"));
    // Flag should be cleared
    assert!(!env.rate_limited_exists());
}

#[test]
fn stop_clears_rate_limited_flag() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    // Over threshold AND rate-limited flag
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 30.0),
    ]));
    env.set_rate_limited();

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"sess-005","stop_hook_active":false}"#)
        .assert()
        .success();

    assert!(env.swap_info_exists());
    assert!(!env.rate_limited_exists());
}

// ── Strategy verification ─────────────────────────────────────────────

#[test]
fn stop_drain_auto_order_picks_highest_7d() {
    let env = TestEnv::new();
    env.add_account("extra");
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    // personal 7d:96% (over), work 7d:90% (highest under 95%), extra 7d:20%
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 90.0),
        ("extra", 5.0, 20.0),
    ]));
    // Default config: drain strategy, no order

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    // Drain auto-order picks highest 7d under 95% → work (90%)
    let info = env.read_swap_info();
    assert_eq!(info["to_account"], "work");
}

#[test]
fn stop_drain_explicit_order() {
    let env = TestEnv::new();
    env.add_account("extra");
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 30.0),
        ("extra", 5.0, 50.0),
    ]));
    env.write_config(&helpers::fake_config(json!({
        "strategy": {"type": "drain", "order": ["extra", "work"]}
    })));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    // First in explicit order with 7d<95% → extra
    let info = env.read_swap_info();
    assert_eq!(info["to_account"], "extra");
}

#[test]
fn stop_drain_skips_maxed_7d() {
    let env = TestEnv::new();
    env.add_account("extra");
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 97.0), // maxed
        ("extra", 5.0, 20.0),
    ]));
    env.write_config(&helpers::fake_config(json!({
        "strategy": {"type": "drain", "order": ["work", "extra"]}
    })));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    // work maxed at 97%, skip to extra
    let info = env.read_swap_info();
    assert_eq!(info["to_account"], "extra");
}

#[test]
fn stop_drain_fallback_5h() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    // Both 7d over 95%, but work has 5h capacity
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 20.0, 97.0),
    ]));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    // Fallback: work has 5h:20% < 90%
    let info = env.read_swap_info();
    assert_eq!(info["to_account"], "work");
}

#[test]
fn stop_balanced_picks_lowest_7d() {
    let env = TestEnv::new();
    env.add_account("extra");
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 60.0),
        ("extra", 5.0, 30.0),
    ]));
    env.write_config(&helpers::fake_config(json!({
        "strategy": {"type": "balanced"}
    })));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    // Balanced: lowest 7d → extra (30%)
    let info = env.read_swap_info();
    assert_eq!(info["to_account"], "extra");
}

#[test]
fn stop_balanced_skips_expired() {
    let env = TestEnv::new();
    env.add_account("extra");
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache_with_expired(&[
        ("personal", 50.0, 96.0, false),
        ("work", 10.0, 20.0, true), // expired
        ("extra", 5.0, 60.0, false),
    ]));
    env.write_config(&helpers::fake_config(json!({
        "strategy": {"type": "balanced"}
    })));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    // work expired, skip to extra
    let info = env.read_swap_info();
    assert_eq!(info["to_account"], "extra");
}

#[test]
fn stop_manual_never_swaps() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 30.0),
    ]));
    env.write_config(&helpers::fake_config(json!({
        "strategy": {"type": "manual"}
    })));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    assert!(!env.swap_info_exists());
    assert_eq!(env.read_active(), "personal");
}

#[test]
fn stop_all_exhausted_no_swap() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    // All maxed: 7d>95% and 5h>90%
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 95.0, 96.0),
        ("work", 92.0, 97.0),
    ]));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    assert!(!env.swap_info_exists());
}

// ── Custom thresholds ─────────────────────────────────────────────────

#[test]
fn stop_custom_low_threshold_triggers() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 75.0, 85.0),
        ("work", 10.0, 30.0),
    ]));
    env.write_config(&helpers::fake_config(json!({
        "thresholds": {"five_hour": 70, "seven_day": 80}
    })));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    // 85% >= 80% threshold → swap
    assert!(env.swap_info_exists());
    assert_eq!(env.read_active(), "work");
}

#[test]
fn stop_default_threshold_no_trigger() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    // Same usage, default thresholds (five_hour:90, seven_day:95)
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 75.0, 85.0),
        ("work", 10.0, 30.0),
    ]));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    // 85% < 95% default → no swap
    assert!(!env.swap_info_exists());
}

// ── Credential integrity ──────────────────────────────────────────────

#[test]
fn stop_preserves_outgoing_credentials() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    // Simulate a token refresh: live creds differ from stored
    let refreshed = json!({
        "claudeAiOauth": {
            "accessToken": "sk-ant-oat01-refreshed-during-session",
            "refreshToken": "sk-ant-ort01-fake-personal",
            "expiresAt": 9999999999999u64,
            "scopes": ["user:inference"],
            "subscriptionType": "max",
            "rateLimitTier": "default_claude_max_20x"
        }
    });
    env.write_live_credentials(&refreshed);
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 30.0),
    ]));

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"abc","stop_hook_active":false}"#)
        .assert()
        .success();

    // Personal's stored creds should now have the refreshed token
    assert_eq!(
        env.read_stored_creds("personal")["claudeAiOauth"]["accessToken"],
        "sk-ant-oat01-refreshed-during-session"
    );
}

// ── End-to-end scenarios ──────────────────────────────────────────────

#[test]
fn full_swap_lifecycle() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 96.0),
        ("work", 10.0, 30.0),
    ]));

    // Step 1: Stop hook triggers swap
    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"sess-lifecycle","stop_hook_active":false}"#)
        .assert()
        .success();

    assert_eq!(env.read_active(), "work");

    // Step 2: Session start on the new account
    env.cmd()
        .args(["hook", "session-start"])
        .write_stdin(r#"{"session_id":"sess-lifecycle","source":"resume","cwd":"/tmp"}"#)
        .assert()
        .success();

    let sessions = env.read_sessions();
    assert_eq!(sessions["sess-lifecycle"]["account"], "work");
}

#[test]
fn rate_limit_then_stop() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");
    env.write_usage_cache(&helpers::fake_usage_cache(&[
        ("personal", 50.0, 40.0), // under threshold
        ("work", 10.0, 30.0),
    ]));

    // Step 1: Rate limit hook sets flag
    env.cmd()
        .args(["hook", "rate-limit"])
        .write_stdin(r#"{"error":{"type":"rate_limit_error","message":"Rate limit exceeded"}}"#)
        .assert()
        .success();
    assert!(env.rate_limited_exists());

    // Step 2: Stop hook picks up the flag
    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(r#"{"session_id":"sess-rl","stop_hook_active":false}"#)
        .assert()
        .success();

    assert!(env.swap_info_exists());
    assert!(!env.rate_limited_exists());
    assert_eq!(env.read_active(), "work");
}
