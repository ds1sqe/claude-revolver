use serde_json::json;

use crate::helpers::TestEnv;

const WRAPPER_PID: u32 = 99999;

fn hook_cmd(env: &TestEnv) -> assert_cmd::Command {
    let mut c = env.cmd();
    c.env("CLAUDE_REVOLVER_WRAPPED", "1")
        .env("CLAUDE_REVOLVER_WRAPPER_PID", WRAPPER_PID.to_string());
    c
}

#[test]
fn rate_limit_noop_without_wrapped_env() {
    let env = TestEnv::new();

    env.cmd()
        .args(["hook", "rate-limit"])
        .write_stdin(
            json!({"error": {"type": "rate_limit_error", "message": "Rate limit"}}).to_string(),
        )
        .assert()
        .success();

    assert!(!env.signal_exists(WRAPPER_PID, "rate-limited"));
}

#[test]
fn rate_limit_writes_signal_on_type_match() {
    let env = TestEnv::new();

    hook_cmd(&env)
        .args(["hook", "rate-limit"])
        .write_stdin(
            json!({"error": {"type": "rate_limit_error", "message": "too fast"}}).to_string(),
        )
        .assert()
        .success();

    assert!(env.signal_exists(WRAPPER_PID, "rate-limited"));
    let sig = env.read_signal(WRAPPER_PID, "rate-limited");
    assert!(sig["timestamp"].as_str().is_some());
}

#[test]
fn rate_limit_writes_signal_on_message_match() {
    let env = TestEnv::new();

    hook_cmd(&env)
        .args(["hook", "rate-limit"])
        .write_stdin(
            json!({"error": {"type": "other", "message": "Rate limit exceeded"}}).to_string(),
        )
        .assert()
        .success();

    assert!(env.signal_exists(WRAPPER_PID, "rate-limited"));
}

#[test]
fn rate_limit_ignores_non_rate_limit_error() {
    let env = TestEnv::new();

    hook_cmd(&env)
        .args(["hook", "rate-limit"])
        .write_stdin(
            json!({"error": {"type": "invalid_request", "message": "bad param"}}).to_string(),
        )
        .assert()
        .success();

    assert!(!env.signal_exists(WRAPPER_PID, "rate-limited"));
}

#[test]
fn rate_limit_ignores_null_error() {
    let env = TestEnv::new();

    hook_cmd(&env)
        .args(["hook", "rate-limit"])
        .write_stdin(json!({"error": null}).to_string())
        .assert()
        .success();

    assert!(!env.signal_exists(WRAPPER_PID, "rate-limited"));
}

#[test]
fn rate_limit_malformed_input_does_not_crash() {
    let env = TestEnv::new();

    hook_cmd(&env)
        .args(["hook", "rate-limit"])
        .write_stdin("not json at all")
        .assert()
        .success();
}
