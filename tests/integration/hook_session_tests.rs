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
fn session_start_noop_without_wrapped_env() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    env.cmd()
        .args(["hook", "session-start"])
        .write_stdin(json!({"session_id": "s1"}).to_string())
        .assert()
        .success();

    // No signal written
    assert!(!env.signal_exists(WRAPPER_PID, "session-started"));
}

#[test]
fn session_start_writes_signal() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    hook_cmd(&env)
        .args(["hook", "session-start"])
        .write_stdin(
            json!({
                "session_id": "sess-abc",
                "cwd": "/home/user/project",
                "source": "cli"
            })
            .to_string(),
        )
        .assert()
        .success();

    assert!(env.signal_exists(WRAPPER_PID, "session-started"));
    let sig = env.read_signal(WRAPPER_PID, "session-started");
    assert_eq!(sig["session_id"], "sess-abc");
    assert_eq!(sig["cwd"], "/home/user/project");
    assert_eq!(sig["source"], "cli");
}

#[test]
fn session_start_does_not_write_sessions_json() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    hook_cmd(&env)
        .args(["hook", "session-start"])
        .write_stdin(json!({"session_id": "s1"}).to_string())
        .assert()
        .success();

    // Hook no longer writes sessions.json — wrapper owns that
    assert!(!env.data_dir.join("sessions.json").exists());
}

#[test]
fn session_start_malformed_input_does_not_crash() {
    let env = TestEnv::new();

    hook_cmd(&env)
        .args(["hook", "session-start"])
        .write_stdin("garbage")
        .assert()
        .success();
}
