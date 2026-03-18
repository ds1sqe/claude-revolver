use anyhow::Result;

use crate::account;
use crate::error::RevolverError;
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

    crate::swap::perform_swap(
        from,
        name,
        "manual switch",
        "manual",
        None,
        std::env::current_dir()
            .ok()
            .map(|p| p.display().to_string())
            .as_deref(),
        false,
    )?;

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
