use anyhow::Result;

use crate::account;
use crate::error::RevolverError;
use crate::usage;
use crate::util::print_info;

pub fn run(name: Option<&str>) -> Result<()> {
    let name = match name {
        Some(n) => n.to_string(),
        None => account::get_active()?.ok_or(RevolverError::NoActiveAccount)?,
    };

    let creds = account::read_credentials(&name)?;
    let oauth = &creds.claude_ai_oauth;

    println!("Account:      {name}");
    println!(
        "Subscription: {}",
        oauth.subscription_type.as_deref().unwrap_or("unknown")
    );
    println!(
        "Rate tier:    {}",
        oauth.rate_limit_tier.as_deref().unwrap_or("unknown")
    );

    let active = account::get_active()?;
    println!(
        "Active:       {}",
        if active.as_deref() == Some(name.as_str()) {
            "yes"
        } else {
            "no"
        }
    );

    // Live usage query
    print_info("querying live usage...");
    match usage::fetch_usage(&oauth.access_token) {
        Ok(resp) => {
            println!();
            if let Some(ref w) = resp.five_hour {
                let resets = w.resets_at.as_deref().unwrap_or("?");
                println!("5-hour:       {:.0}% (resets {resets})", w.utilization);
            }
            if let Some(ref w) = resp.seven_day {
                let resets = w.resets_at.as_deref().unwrap_or("?");
                println!("7-day:        {:.0}% (resets {resets})", w.utilization);
            }
            if let Some(ref w) = resp.seven_day_sonnet {
                println!("7d sonnet:    {:.0}%", w.utilization);
            }
            if let Some(ref w) = resp.seven_day_opus {
                println!("7d opus:      {:.0}%", w.utilization);
            }
        }
        Err(e) => {
            crate::util::print_warn(&format!("failed to query usage API: {e}"));
        }
    }

    Ok(())
}
