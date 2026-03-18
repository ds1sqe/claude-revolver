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
        "  {:2} {:16} {:8} {:12} {:22} {:12} {}",
        "", "NAME", "TYPE", "5h", "5h RESETS", "7d", "7d RESETS"
    );
    println!("  {}", "─".repeat(85));

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

        let cached = cache.get(name);

        let u5 = cached.and_then(|u| u.five_hour.as_ref().map(|w| w.utilization));
        let u7 = cached.and_then(|u| u.seven_day.as_ref().map(|w| w.utilization));

        let r5 = cached
            .and_then(|u| u.five_hour.as_ref())
            .and_then(|w| w.resets_at.as_deref())
            .and_then(|s| s.get(..19))
            .unwrap_or("--");
        let r7 = cached
            .and_then(|u| u.seven_day.as_ref())
            .and_then(|w| w.resets_at.as_deref())
            .and_then(|s| s.get(..19))
            .unwrap_or("--");

        println!(
            "  {} {:16} {:8} {}   {:22} {}   {}",
            marker,
            name,
            sub_type,
            fmt_util(u5, "5h"),
            r5,
            fmt_util(u7, "7d"),
            r7,
        );
    }

    Ok(())
}
