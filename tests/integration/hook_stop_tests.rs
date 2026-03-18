use serde_json::json;

use crate::helpers::TestEnv;

const WRAPPER_PID: u32 = 99999;

fn hook_cmd(env: &TestEnv) -> assert_cmd::Command {
    let mut c = env.cmd();
    c.env("CLAUDE_REVOLVER_WRAPPED", "1")
        .env("CLAUDE_REVOLVER_WRAPPER_PID", WRAPPER_PID.to_string());
    c
}

// ── Gating ──────────────────────────────────────────────────────────────

#[test]
fn stop_noop_without_wrapped_env() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    env.cmd()
        .args(["hook", "stop"])
        .write_stdin(json!({"session_id": "s1"}).to_string())
        .assert()
        .success();

    assert!(!env.signal_exists(WRAPPER_PID, "stopped"));
}

// ── Signal writing ──────────────────────────────────────────────────────

#[test]
fn stop_writes_stopped_signal() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    hook_cmd(&env)
        .args(["hook", "stop"])
        .write_stdin(json!({"session_id": "sess-123"}).to_string())
        .assert()
        .success();

    assert!(env.signal_exists(WRAPPER_PID, "stopped"));
    let sig = env.read_signal(WRAPPER_PID, "stopped");
    assert_eq!(sig["session_id"], "sess-123");
}

#[test]
fn stop_does_not_swap_credentials() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.add_account("work");
    env.set_active("personal");
    env.set_live_credentials("personal");

    hook_cmd(&env)
        .args(["hook", "stop"])
        .write_stdin(json!({"session_id": "s1"}).to_string())
        .assert()
        .success();

    // Hook NEVER swaps — active and live creds unchanged
    assert_eq!(env.read_active(), "personal");
    assert_eq!(
        env.read_live_creds()["claudeAiOauth"]["accessToken"],
        "sk-ant-oat01-fake-personal"
    );
}

#[test]
fn stop_malformed_input_does_not_crash() {
    let env = TestEnv::new();

    hook_cmd(&env)
        .args(["hook", "stop"])
        .write_stdin("not json")
        .assert()
        .success();
}

#[test]
fn stop_signals_namespaced_by_pid() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    let pid1: u32 = 11111;
    let pid2: u32 = 22222;

    env.cmd()
        .env("CLAUDE_REVOLVER_WRAPPED", "1")
        .env("CLAUDE_REVOLVER_WRAPPER_PID", pid1.to_string())
        .args(["hook", "stop"])
        .write_stdin(json!({"session_id": "s-from-pid1"}).to_string())
        .assert()
        .success();

    env.cmd()
        .env("CLAUDE_REVOLVER_WRAPPED", "1")
        .env("CLAUDE_REVOLVER_WRAPPER_PID", pid2.to_string())
        .args(["hook", "stop"])
        .write_stdin(json!({"session_id": "s-from-pid2"}).to_string())
        .assert()
        .success();

    assert!(env.signal_exists(pid1, "stopped"));
    assert!(env.signal_exists(pid2, "stopped"));
    let sig1 = env.read_signal(pid1, "stopped");
    let sig2 = env.read_signal(pid2, "stopped");
    assert_eq!(sig1["session_id"], "s-from-pid1");
    assert_eq!(sig2["session_id"], "s-from-pid2");
}
