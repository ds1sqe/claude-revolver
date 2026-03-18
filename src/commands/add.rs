use anyhow::Result;

use crate::account;
use crate::util::print_info;

pub fn run(name: &str) -> Result<()> {
    account::add_account(name)?;

    let creds = account::read_credentials(name)?;
    let sub_type = creds
        .claude_ai_oauth
        .subscription_type
        .as_deref()
        .unwrap_or("unknown");
    print_info(&format!("saved current credentials as '{name}' ({sub_type})"));
    Ok(())
}
