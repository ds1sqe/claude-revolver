use anyhow::Result;

use crate::account;
use crate::usage;
use crate::util::fmt_util;

pub fn run() -> Result<()> {
    let accounts = account::list_accounts()?;
    let active = account::get_active()?;
    let cache = usage::load_cache().unwrap_or_default();

    if accounts.is_empty() {
        println!("  no accounts — use 'claude-revolver add <name>' to save current credentials");
        return Ok(());
    }

    println!(
        "  {:2} {:16} {:8} {:12} {:12}",
        "", "NAME", "TYPE", "5h", "7d"
    );
    println!("  {}", "─".repeat(52));

    for name in &accounts {
        let marker = if active.as_deref() == Some(name.as_str()) {
            "*"
        } else {
            " "
        };

        let sub_type = account::read_credentials(name)
            .ok()
            .and_then(|c| c.claude_ai_oauth.subscription_type)
            .unwrap_or_else(|| "?".to_string());

        let (u5, u7) = cache
            .get(name)
            .map(|u| {
                (
                    u.five_hour.as_ref().map(|w| w.utilization),
                    u.seven_day.as_ref().map(|w| w.utilization),
                )
            })
            .unwrap_or((None, None));

        println!(
            "  {} {:16} {:8} {}   {}",
            marker,
            name,
            sub_type,
            fmt_util(u5, "5h"),
            fmt_util(u7, "7d"),
        );
    }

    Ok(())
}
