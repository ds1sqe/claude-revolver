use anyhow::Result;

use crate::account;
use crate::error::RevolverError;
use crate::types::SwapLogEntry;
use crate::util::{print_info, print_warn};

pub fn run(name: &str) -> Result<()> {
    if !crate::paths::account_cred_file(name)?.exists() {
        return Err(RevolverError::AccountNotFound(name.to_string()).into());
    }

    let current = account::get_active()?;
    let from = current.as_deref().unwrap_or("");

    if from == name {
        print_info(&format!("already on '{name}'"));
        return Ok(());
    }

    account::swap_credentials(from, name)?;

    let _ = crate::history::log_swap(SwapLogEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        from_account: from.to_string(),
        to_account: name.to_string(),
        reason: "manual switch".to_string(),
        trigger: "manual".to_string(),
        session_id: None,
        cwd: std::env::current_dir().ok().map(|p| p.display().to_string()),
        from_usage_5h: None,
        from_usage_7d: None,
        to_usage_5h: None,
        to_usage_7d: None,
        temp_swap: false,
    });

    let creds = account::read_credentials(name)?;
    let sub_type = creds
        .claude_ai_oauth
        .subscription_type
        .as_deref()
        .unwrap_or("unknown");
    print_info(&format!("switched to '{name}' ({sub_type})"));
    print_warn("restart Claude Code for the switch to take effect");
    Ok(())
}
