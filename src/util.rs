use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{Context, Result};

/// Atomic write: write to a temp file in the same directory, then rename.
pub fn atomic_write(path: &Path, content: &[u8], mode: u32) -> Result<()> {
    let dir = path
        .parent()
        .context("cannot determine parent directory")?;
    fs::create_dir_all(dir)?;

    let tmp_path = dir.join(format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file"),
        std::process::id()
    ));

    let mut f = fs::File::create(&tmp_path)
        .with_context(|| format!("cannot create temp file {}", tmp_path.display()))?;
    f.write_all(content)?;
    f.sync_all()?;
    drop(f);

    fs::set_permissions(&tmp_path, fs::Permissions::from_mode(mode))?;
    fs::rename(&tmp_path, path)
        .with_context(|| format!("cannot rename {} to {}", tmp_path.display(), path.display()))?;
    Ok(())
}

/// Atomic JSON write with pretty formatting.
pub fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T, mode: u32) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    atomic_write(path, json.as_bytes(), mode)
}

/// Ensure a directory exists with the given permissions.
pub fn ensure_dir(path: &Path, mode: u32) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

/// Colored terminal output helpers.
pub fn print_info(msg: &str) {
    eprintln!("\x1b[36m::\x1b[0m {msg}");
}

pub fn print_warn(msg: &str) {
    eprintln!("\x1b[33mwarn:\x1b[0m {msg}");
}

pub fn print_error(msg: &str) {
    eprintln!("\x1b[31merror:\x1b[0m {msg}");
}

/// Format utilization with color.
pub fn fmt_util(val: Option<f64>, label: &str) -> String {
    match val {
        None => format!("{label}:--%"),
        Some(v) => {
            let color = if v >= 90.0 {
                "\x1b[31m"
            } else if v >= 70.0 {
                "\x1b[33m"
            } else {
                "\x1b[32m"
            };
            format!("{color}{label}:{v:.0}%\x1b[0m")
        }
    }
}

/// Send a desktop notification (best-effort).
pub fn notify(urgency: &str, title: &str, body: &str) {
    let _ = std::process::Command::new("notify-send")
        .args(["-u", urgency, title, body])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Validate account name: alphanumeric, hyphens, underscores.
pub fn validate_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
