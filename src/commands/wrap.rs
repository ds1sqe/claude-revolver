use std::io::Write;
use std::process::Command;

use anyhow::Result;

use crate::{account, config, paths, strategy, usage, util};

pub fn run(args: &[String]) -> Result<()> {
    let config = config::Config::load().unwrap_or_default();
    let accounts = account::list_accounts()?;

    let mut active = match account::get_active()? {
        Some(a) => a,
        None => {
            util::print_warn("no active account set — launching claude without account management");
            return exec_claude(args);
        }
    };

    // Pre-check: should we auto-swap before launching?
    if accounts.len() > 1 {
        let cache = usage::load_cache().unwrap_or_default();
        if let Some(current_usage) = cache.get(&active) {
            let (over, reason) = usage::is_over_threshold(current_usage, &config.thresholds);
            if over {
                if let Some(next) = strategy::select_next_account(&active, &accounts, &cache, &config) {
                    util::print_warn(&format!("active account '{active}' usage high ({reason})"));
                    account::swap_credentials(&active, &next)?;
                    active = next.clone();
                    util::print_info(&format!("auto-switched to '{next}'"));
                } else {
                    util::print_warn("all accounts near limits — proceeding with current");
                }
            }
        }
    }

    // Main loop: launch claude, check swap-info on exit, optionally resume
    loop {
        util::print_info(&format!("launching claude (account: {active})"));

        // Clear flags
        let _ = std::fs::remove_file(paths::rate_limited_flag()?);
        let _ = std::fs::remove_file(paths::swap_info_file()?);

        // Launch claude
        let status = Command::new("claude")
            .args(args)
            .status()
            .map_err(|e| anyhow::anyhow!("failed to launch claude: {e}"))?;

        // Always sync back
        let _ = account::sync_back();

        // Check swap-info (written by the Stop hook)
        let swap_info_path = paths::swap_info_file()?;
        if swap_info_path.exists() {
            let content = std::fs::read_to_string(&swap_info_path)?;
            let swap_info: crate::types::SwapInfo = serde_json::from_str(&content)?;
            let _ = std::fs::remove_file(&swap_info_path);

            active = swap_info.to_account.clone();

            if config.auto_resume {
                let cache = usage::load_cache().unwrap_or_default();
                let u5 = cache
                    .get(&active)
                    .and_then(|u| u.five_hour.as_ref())
                    .map(|w| format!("{:.0}%", w.utilization))
                    .unwrap_or_else(|| "?".to_string());
                let u7 = cache
                    .get(&active)
                    .and_then(|u| u.seven_day.as_ref())
                    .map(|w| format!("{:.0}%", w.utilization))
                    .unwrap_or_else(|| "?".to_string());

                util::print_warn(&format!(
                    "swapped from '{}' to '{}' (5h:{u5} 7d:{u7}): {}",
                    swap_info.from_account, active, swap_info.reason
                ));
                util::print_info(&format!(
                    "auto-resuming session {}",
                    swap_info.session_id
                ));

                // Resume with the configured message
                let _status = Command::new("claude")
                    .args(["--resume", &swap_info.session_id, &config.auto_message])
                    .status()
                    .map_err(|e| anyhow::anyhow!("failed to resume claude: {e}"))?;

                let _ = account::sync_back();

                // Check again for another swap
                if paths::swap_info_file()?.exists() {
                    continue;
                }

                return Ok(());
            } else {
                // Manual resume mode
                println!();
                util::print_warn(&format!(
                    "swapped from '{}' to '{}': {}",
                    swap_info.from_account, active, swap_info.reason
                ));
                println!(
                    "Resume: claude --resume {} \"{}\"",
                    swap_info.session_id, config.auto_message
                );
                return Ok(());
            }
        }

        // Check rate-limited flag (set by PostToolUseFailure hook, no swap-info from Stop hook)
        if paths::rate_limited_flag()?.exists() && accounts.len() > 1 {
            let _ = std::fs::remove_file(paths::rate_limited_flag()?);

            let cache = usage::load_cache().unwrap_or_default();
            if let Some(next) =
                strategy::select_next_account(&active, &accounts, &cache, &config)
            {
                util::print_warn(&format!("rate limited on '{active}'"));
                account::swap_credentials(&active, &next)?;
                active = next.clone();
                util::print_info(&format!("switched to '{next}'"));

                print!("\nRestart claude? [Y/n] ");
                std::io::stdout().flush()?;
                let mut reply = String::new();
                std::io::stdin().read_line(&mut reply)?;
                if reply.trim().is_empty() || reply.trim().starts_with(['Y', 'y']) {
                    continue;
                }
            } else {
                util::print_warn("rate limited but no other accounts available");
            }
        }

        // Normal exit
        std::process::exit(status.code().unwrap_or(0));
    }
}

fn exec_claude(args: &[String]) -> Result<()> {
    let status = Command::new("claude")
        .args(args)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to launch claude: {e}"))?;
    std::process::exit(status.code().unwrap_or(0));
}
