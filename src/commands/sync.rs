use anyhow::Result;

use crate::account;
use crate::error::RevolverError;
use crate::util::print_info;

pub fn run() -> Result<()> {
    let active = account::get_active()?.ok_or(RevolverError::NoActiveAccount)?;
    let synced = account::sync_back()?;
    if synced {
        print_info(&format!("synced live credentials to '{active}'"));
    } else {
        print_info(&format!("'{active}' already up to date"));
    }
    Ok(())
}
