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

    let hooks = settings
        .as_object_mut()
        .context("settings is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hooks_obj = hooks
        .as_object_mut()
        .context("hooks is not an object")?;

    let mut installed = Vec::new();

    // Install Stop hook
    if upsert_hook(hooks_obj, "Stop", "claude-revolver hook stop", 10) {
        installed.push("Stop");
    }

    // Install SessionStart hook
    if upsert_hook(hooks_obj, "SessionStart", "claude-revolver hook session-start", 5) {
        installed.push("SessionStart");
    }

    // Install PostToolUseFailure hook
    if upsert_hook(hooks_obj, "PostToolUseFailure", "claude-revolver hook rate-limit", 5) {
        installed.push("PostToolUseFailure");
    }

    if installed.is_empty() {
        print_info("all hooks already installed");
        return Ok(());
    }

    let json = serde_json::to_string_pretty(&settings)?;
    atomic_write(&settings_path, json.as_bytes(), 0o644)?;

    print_info(&format!("hooks installed ({})", installed.join(", ")));
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

/// Insert a hook entry if no claude-revolver entry exists for this hook type.
/// Returns true if a new entry was added, false if already present.
fn upsert_hook(
    hooks: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    command: &str,
    timeout: u64,
) -> bool {
    let arr = hooks
        .entry(key)
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .expect("hook value should be an array");

    // Check if claude-revolver entry already exists
    let already = arr.iter().any(|entry| {
        serde_json::to_string(entry)
            .unwrap_or_default()
            .contains("claude-revolver")
    });

    if already {
        return false;
    }

    arr.push(serde_json::json!({
        "matcher": ".*",
        "hooks": [{
            "type": "command",
            "command": command,
            "timeout": timeout
        }]
    }));

    true
}
