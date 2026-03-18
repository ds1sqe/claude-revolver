use serde_json::json;

use crate::helpers::TestEnv;

#[test]
fn install_hook_creates_entries() {
    let env = TestEnv::new();
    env.write_settings(&json!({}));

    env.cmd()
        .args(["install", "hook"])
        .assert()
        .success();

    let settings = env.read_settings();
    let hooks = settings["hooks"].as_object().unwrap();

    assert!(hooks.contains_key("Stop"));
    assert!(hooks.contains_key("SessionStart"));
    assert!(hooks.contains_key("PostToolUseFailure"));

    // Each hook array has exactly one entry
    assert_eq!(hooks["Stop"].as_array().unwrap().len(), 1);
    assert_eq!(hooks["SessionStart"].as_array().unwrap().len(), 1);
    assert_eq!(hooks["PostToolUseFailure"].as_array().unwrap().len(), 1);
}

#[test]
fn install_hook_does_not_duplicate() {
    let env = TestEnv::new();
    env.write_settings(&json!({}));

    // Install twice
    env.cmd().args(["install", "hook"]).assert().success();
    env.cmd().args(["install", "hook"]).assert().success();

    let settings = env.read_settings();
    let hooks = settings["hooks"].as_object().unwrap();

    // Still exactly one entry per hook type
    assert_eq!(hooks["Stop"].as_array().unwrap().len(), 1);
    assert_eq!(hooks["SessionStart"].as_array().unwrap().len(), 1);
    assert_eq!(hooks["PostToolUseFailure"].as_array().unwrap().len(), 1);
}

#[test]
fn install_hook_preserves_existing_hooks() {
    let env = TestEnv::new();
    // Pre-existing user hook
    env.write_settings(&json!({
        "hooks": {
            "Stop": [{
                "matcher": ".*",
                "hooks": [{
                    "type": "command",
                    "command": "my-custom-hook",
                    "timeout": 5
                }]
            }]
        }
    }));

    env.cmd().args(["install", "hook"]).assert().success();

    let settings = env.read_settings();
    let stop = settings["hooks"]["Stop"].as_array().unwrap();

    // User hook preserved + claude-revolver added = 2 entries
    assert_eq!(stop.len(), 2);

    let commands: Vec<&str> = stop
        .iter()
        .flat_map(|e| e["hooks"].as_array())
        .flatten()
        .filter_map(|h| h["command"].as_str())
        .collect();
    assert!(commands.contains(&"my-custom-hook"));
    assert!(commands.contains(&"claude-revolver hook stop"));
}

#[test]
fn install_hook_no_duplicate_with_existing_user_hooks() {
    let env = TestEnv::new();
    // Pre-existing user hook + already installed claude-revolver hook
    env.write_settings(&json!({
        "hooks": {
            "Stop": [
                {
                    "matcher": ".*",
                    "hooks": [{"type": "command", "command": "my-custom-hook", "timeout": 5}]
                },
                {
                    "matcher": ".*",
                    "hooks": [{"type": "command", "command": "claude-revolver hook stop", "timeout": 10}]
                }
            ]
        }
    }));

    env.cmd().args(["install", "hook"]).assert().success();

    let settings = env.read_settings();
    let stop = settings["hooks"]["Stop"].as_array().unwrap();

    // Still 2 entries — no duplication
    assert_eq!(stop.len(), 2);
}

#[test]
fn uninstall_hook_removes_only_revolver() {
    let env = TestEnv::new();
    env.write_settings(&json!({
        "hooks": {
            "Stop": [
                {
                    "matcher": ".*",
                    "hooks": [{"type": "command", "command": "my-custom-hook", "timeout": 5}]
                },
                {
                    "matcher": ".*",
                    "hooks": [{"type": "command", "command": "claude-revolver hook stop", "timeout": 10}]
                }
            ],
            "SessionStart": [{
                "matcher": ".*",
                "hooks": [{"type": "command", "command": "claude-revolver hook session-start", "timeout": 5}]
            }]
        }
    }));

    env.cmd().args(["uninstall", "hook"]).assert().success();

    let settings = env.read_settings();
    let hooks = settings["hooks"].as_object().unwrap();

    // Stop still has the user hook
    let stop = hooks["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 1);
    assert!(serde_json::to_string(&stop[0]).unwrap().contains("my-custom-hook"));

    // SessionStart was only claude-revolver, so the key should be removed
    assert!(!hooks.contains_key("SessionStart"));
}
