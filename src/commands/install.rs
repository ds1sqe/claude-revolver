use anyhow::{Context, Result};

use crate::paths;
use crate::util::{atomic_write, print_info, print_warn};

pub fn install_hook() -> Result<()> {
    let settings_path = paths::settings_file()?;
    if !settings_path.exists() {
        anyhow::bail!("{} not found", settings_path.display());
    }

    let content = std::fs::read_to_string(&settings_path)?;
    let mut settings: serde_json::Value = serde_json::from_str(&content)?;

    // Check if already installed
    if let Some(hooks) = settings.get("hooks") {
        let hooks_str = serde_json::to_string(hooks)?;
        if hooks_str.contains("claude-revolver") {
            print_info("hooks already installed");
            return Ok(());
        }
    }

    let hooks = settings
        .as_object_mut()
        .context("settings is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hooks_obj = hooks
        .as_object_mut()
        .context("hooks is not an object")?;

    // Install Stop hook
    let stop_entry = serde_json::json!([{
        "matcher": ".*",
        "hooks": [{
            "type": "command",
            "command": "claude-revolver hook stop",
            "timeout": 10
        }]
    }]);
    merge_hook_array(hooks_obj, "Stop", stop_entry);

    // Install SessionStart hook
    let session_start_entry = serde_json::json!([{
        "matcher": ".*",
        "hooks": [{
            "type": "command",
            "command": "claude-revolver hook session-start",
            "timeout": 5
        }]
    }]);
    merge_hook_array(hooks_obj, "SessionStart", session_start_entry);

    // Install PostToolUseFailure hook
    let rate_limit_entry = serde_json::json!([{
        "matcher": ".*",
        "hooks": [{
            "type": "command",
            "command": "claude-revolver hook rate-limit",
            "timeout": 5
        }]
    }]);
    merge_hook_array(hooks_obj, "PostToolUseFailure", rate_limit_entry);

    let json = serde_json::to_string_pretty(&settings)?;
    atomic_write(&settings_path, json.as_bytes(), 0o644)?;

    print_info("hooks installed (Stop, SessionStart, PostToolUseFailure)");
    print_warn("restart Claude Code for hooks to take effect");
    Ok(())
}

pub fn uninstall_hook() -> Result<()> {
    let settings_path = paths::settings_file()?;
    if !settings_path.exists() {
        anyhow::bail!("{} not found", settings_path.display());
    }

    let content = std::fs::read_to_string(&settings_path)?;
    let mut settings: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for key in &["Stop", "SessionStart", "PostToolUseFailure"] {
            if let Some(arr) = hooks.get_mut(*key).and_then(|v| v.as_array_mut()) {
                arr.retain(|entry| {
                    let s = serde_json::to_string(entry).unwrap_or_default();
                    !s.contains("claude-revolver")
                });
                if arr.is_empty() {
                    hooks.remove(*key);
                }
            }
        }
    }

    let json = serde_json::to_string_pretty(&settings)?;
    atomic_write(&settings_path, json.as_bytes(), 0o644)?;

    print_info("hooks removed from settings");
    Ok(())
}

pub fn install_systemd() -> Result<()> {
    let unit_dir = dirs::home_dir()
        .context("cannot determine home directory")?
        .join(".config/systemd/user");

    std::fs::create_dir_all(&unit_dir)?;

    let service = include_str!("../../systemd/claude-revolver-monitor.service");
    let timer = include_str!("../../systemd/claude-revolver-monitor.timer");

    std::fs::write(unit_dir.join("claude-revolver-monitor.service"), service)?;
    std::fs::write(unit_dir.join("claude-revolver-monitor.timer"), timer)?;

    let status = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl daemon-reload failed");
    }

    let status = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "claude-revolver-monitor.timer"])
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl enable failed");
    }

    print_info("systemd timer installed and started");
    Ok(())
}

pub fn uninstall_systemd() -> Result<()> {
    let _ = std::process::Command::new("systemctl")
        .args([
            "--user",
            "disable",
            "--now",
            "claude-revolver-monitor.timer",
        ])
        .status();

    let unit_dir = dirs::home_dir()
        .context("cannot determine home directory")?
        .join(".config/systemd/user");

    let _ = std::fs::remove_file(unit_dir.join("claude-revolver-monitor.service"));
    let _ = std::fs::remove_file(unit_dir.join("claude-revolver-monitor.timer"));

    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();

    print_info("systemd units removed");
    Ok(())
}

/// Merge a hook entry array into the existing hooks, appending to existing arrays.
fn merge_hook_array(
    hooks: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    new_entries: serde_json::Value,
) {
    let existing = hooks
        .entry(key)
        .or_insert_with(|| serde_json::json!([]));

    if let (Some(arr), Some(new_arr)) = (existing.as_array_mut(), new_entries.as_array()) {
        arr.extend(new_arr.iter().cloned());
    }
}
