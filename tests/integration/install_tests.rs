use serde_json::json;

use crate::helpers::TestEnv;

#[test]
fn install_creates_hook_entries() {
    let env = TestEnv::new();
    env.write_settings(&json!({}));

    // install (coupled — hooks + systemd, but systemd will fail in test env)
    env.cmd()
        .arg("install")
        .assert()
        .success();

    let settings = env.read_settings();
    let hooks = &settings["hooks"];
    assert!(hooks["Stop"].as_array().unwrap().len() > 0);
    assert!(hooks["SessionStart"].as_array().unwrap().len() > 0);
    assert!(hooks["PostToolUseFailure"].as_array().unwrap().len() > 0);

    // Verify commands reference claude-revolver
    let stop_cmd = serde_json::to_string(&hooks["Stop"]).unwrap();
    assert!(stop_cmd.contains("claude-revolver hook stop"));
}

#[test]
fn install_does_not_duplicate() {
    let env = TestEnv::new();
    env.write_settings(&json!({}));

    env.cmd().arg("install").assert().success();
    env.cmd().arg("install").assert().success();

    let settings = env.read_settings();
    // Still only 1 entry per hook type
    assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
    assert_eq!(
        settings["hooks"]["SessionStart"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn install_preserves_existing_hooks() {
    let env = TestEnv::new();
    env.write_settings(&json!({
        "hooks": {
            "Stop": [
                {
                    "matcher": ".*",
                    "hooks": [{"type": "command", "command": "my-other-tool"}]
                }
            ]
        }
    }));

    env.cmd().arg("install").assert().success();

    let settings = env.read_settings();
    let stop_hooks = settings["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop_hooks.len(), 2); // existing + claude-revolver
}

#[test]
fn uninstall_removes_only_revolver_hooks() {
    let env = TestEnv::new();
    env.write_settings(&json!({
        "hooks": {
            "Stop": [
                {
                    "matcher": ".*",
                    "hooks": [{"type": "command", "command": "my-other-tool"}]
                },
                {
                    "matcher": ".*",
                    "hooks": [{"type": "command", "command": "claude-revolver hook stop"}]
                }
            ]
        }
    }));

    env.cmd().arg("uninstall").assert().success();

    let settings = env.read_settings();
    let stop_hooks = settings["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop_hooks.len(), 1);
    let remaining = serde_json::to_string(&stop_hooks[0]).unwrap();
    assert!(remaining.contains("my-other-tool"));
    assert!(!remaining.contains("claude-revolver"));
}
