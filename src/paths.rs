use std::path::PathBuf;

use anyhow::{Context, Result};

fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().context("cannot determine home directory")
}

pub fn data_dir() -> Result<PathBuf> {
    if let Ok(val) = std::env::var("CLAUDE_REVOLVER_DATA_DIR") {
        return Ok(PathBuf::from(val));
    }
    let base = dirs::data_dir()
        .or_else(|| home_dir().ok().map(|h| h.join(".local/share")))
        .context("cannot determine data directory")?;
    Ok(base.join("claude-revolver"))
}

pub fn config_dir() -> Result<PathBuf> {
    if let Ok(val) = std::env::var("CLAUDE_REVOLVER_CONFIG_DIR") {
        return Ok(PathBuf::from(val));
    }
    let base = dirs::config_dir()
        .or_else(|| home_dir().ok().map(|h| h.join(".config")))
        .context("cannot determine config directory")?;
    Ok(base.join("claude-revolver"))
}

pub fn claude_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".claude"))
}

pub fn cred_file() -> Result<PathBuf> {
    Ok(claude_dir()?.join(".credentials.json"))
}

pub fn active_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("active"))
}

pub fn usage_cache_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("usage-cache.json"))
}

pub fn sessions_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("sessions.json"))
}

pub fn swap_info_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("swap-info"))
}

pub fn swap_history_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("swap-history.json"))
}

pub fn rate_limited_flag() -> Result<PathBuf> {
    Ok(data_dir()?.join("rate-limited"))
}

pub fn account_dir(name: &str) -> Result<PathBuf> {
    Ok(data_dir()?.join(name))
}

pub fn account_cred_file(name: &str) -> Result<PathBuf> {
    Ok(account_dir(name)?.join("credentials.json"))
}

pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn settings_file() -> Result<PathBuf> {
    Ok(claude_dir()?.join("settings.json"))
}

pub fn usage_api_url() -> String {
    std::env::var("CLAUDE_REVOLVER_USAGE_API_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com/api/oauth/usage".to_string())
}
