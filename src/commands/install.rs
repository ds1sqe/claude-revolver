use anyhow::{Context, Result};

use crate::paths;
use crate::util::{atomic_write, print_info, print_warn};

pub fn install() -> Result<()> {
    let mut installed = Vec::new();

    // Install hooks
    match install_hooks() {
        Ok(hooks) => installed.extend(hooks),
        Err(e) => print_warn(&format!("hooks: {e}")),
    }

    // Install systemd timer
    match install_systemd_units() {
        Ok(()) => installed.push("systemd timer".to_string()),
        Err(e) => print_warn(&format!("systemd: {e}")),
    }

    if installed.is_empty() {
        print_info("everything already installed");
    } else {
        print_info(&format!("installed: {}", installed.join(", ")));
        print_warn("restart Claude Code for hooks to take effect");
    }
    Ok(())
}

pub fn uninstall() -> Result<()> {
    // Remove hooks
    if let Err(e) = uninstall_hooks() {
        print_warn(&format!("hooks: {e}"));
    }

    // Remove systemd
    if let Err(e) = uninstall_systemd_units() {
        print_warn(&format!("systemd: {e}"));
    }

    print_info("uninstalled hooks and systemd timer");
    Ok(())
}

// ── Hooks ───────────────────────────────────────────────────────────────

fn install_hooks() -> Result<Vec<String>> {
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

    if upsert_hook(hooks_obj, "Stop", "claude-revolver hook stop", 10) {
        installed.push("Stop hook".to_string());
    }
    if upsert_hook(hooks_obj, "SessionStart", "claude-revolver hook session-start", 5) {
        installed.push("SessionStart hook".to_string());
    }
    if upsert_hook(hooks_obj, "PostToolUseFailure", "claude-revolver hook rate-limit", 5) {
        installed.push("PostToolUseFailure hook".to_string());
    }

    if !installed.is_empty() {
        let json = serde_json::to_string_pretty(&settings)?;
        atomic_write(&settings_path, json.as_bytes(), 0o644)?;
    }

    Ok(installed)
}

fn uninstall_hooks() -> Result<()> {
    let settings_path = paths::settings_file()?;
    if !settings_path.exists() {
        return Ok(());
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
    Ok(())
}

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

// ── Systemd ─────────────────────────────────────────────────────────────

fn install_systemd_units() -> Result<()> {
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

    Ok(())
}

fn uninstall_systemd_units() -> Result<()> {
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

    Ok(())
}
