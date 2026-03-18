use anyhow::Result;

use crate::account;
use crate::util::{print_info, print_warn};

pub fn run(name: &str) -> Result<()> {
    let was_active = account::remove_account(name)?;
    if was_active {
        print_warn(&format!("removed active account '{name}'"));
    } else {
        print_info(&format!("removed '{name}'"));
    }
    Ok(())
}
