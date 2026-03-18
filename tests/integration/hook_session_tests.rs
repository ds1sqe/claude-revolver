use serde_json::json;

use crate::helpers::TestEnv;

#[test]
fn session_start_records_entry() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    env.cmd()
        .args(["hook", "session-start"])
        .write_stdin(r#"{"session_id":"sess-001","source":"startup","cwd":"/tmp/project"}"#)
        .assert()
        .success();

    let sessions = env.read_sessions();
    assert_eq!(sessions["sess-001"]["account"], "personal");
    assert_eq!(sessions["sess-001"]["source"], "startup");
    assert_eq!(sessions["sess-001"]["cwd"], "/tmp/project");
    // started_at should be a non-empty timestamp
    assert!(!sessions["sess-001"]["started_at"].as_str().unwrap().is_empty());
}

#[test]
fn session_start_prunes_old_entries() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    // Pre-populate with an 8-day-old entry
    let old_sessions = json!({
        "old-session": {
            "account": "personal",
            "started_at": "2026-03-09T12:00:00Z",
            "source": "startup",
            "cwd": "/tmp"
        }
    });
    env.write_sessions(&old_sessions);

    env.cmd()
        .args(["hook", "session-start"])
        .write_stdin(r#"{"session_id":"new-session","source":"startup","cwd":"/tmp"}"#)
        .assert()
        .success();

    let sessions = env.read_sessions();
    // Old session pruned (>7 days)
    assert!(sessions.get("old-session").is_none());
    // New session present
    assert_eq!(sessions["new-session"]["account"], "personal");
}

#[test]
fn session_start_defaults_source_and_cwd() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    env.cmd()
        .args(["hook", "session-start"])
        .write_stdin(r#"{"session_id":"sess-minimal"}"#)
        .assert()
        .success();

    let sessions = env.read_sessions();
    assert_eq!(sessions["sess-minimal"]["source"], "unknown");
    assert_eq!(sessions["sess-minimal"]["cwd"], "");
}

#[test]
fn session_start_malformed_noop() {
    let env = TestEnv::new();
    env.add_account("personal");
    env.set_active("personal");

    env.cmd()
        .args(["hook", "session-start"])
        .write_stdin("garbage input")
        .assert()
        .success();

    // No sessions file written
    assert!(!env.root.path().join("data/sessions.json").exists());
}

#[test]
fn session_start_no_active_noop() {
    let env = TestEnv::new();
    // No active account set

    env.cmd()
        .args(["hook", "session-start"])
        .write_stdin(r#"{"session_id":"sess-001","source":"startup"}"#)
        .assert()
        .success();

    assert!(!env.root.path().join("data/sessions.json").exists());
}
