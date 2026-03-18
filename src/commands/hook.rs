use std::io::Read;

use anyhow::Result;

use crate::types::{RateLimitHookInput, SessionStartHookInput, StopHookInput, SwapLogEntry};
use crate::{account, config, history, paths, strategy, usage, util};

/// Stop hook: check usage, swap if over threshold. Always exits 0.
pub fn stop() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let hook_input: StopHookInput = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return Ok(()), // malformed input, don't crash
    };

    // Loop guard
    if hook_input.stop_hook_active {
        return Ok(());
    }

    let active = match account::get_active()? {
        Some(a) => a,
        None => return Ok(()),
    };

    let accounts = account::list_accounts()?;
    if accounts.len() < 2 {
        return Ok(());
    }

    let cache = usage::load_cache().unwrap_or_default();
    let config = config::Config::load().unwrap_or_default();

    // Check if current account is over threshold
    let current_usage = match cache.get(&active) {
        Some(u) => u,
        None => return Ok(()), // no cache data, can't decide
    };

    // Also check the rate-limited flag
    let rate_limited = paths::rate_limited_flag()?.exists();

    let (over, reason) = usage::is_over_threshold(current_usage, &config.thresholds);
    if !over && !rate_limited {
        return Ok(());
    }

    let reason = if rate_limited {
        "rate limit hit during session".to_string()
    } else {
        reason
    };

    // Pick next account
    let next = match strategy::select_next_account(&active, &accounts, &cache, &config) {
        Some(n) => n,
        None => return Ok(()), // no suitable target
    };

    // Determine if this is a 5h temp-swap
    let is_temp = strategy::is_five_hour_temp_swap(current_usage, &config.thresholds);
    let return_to = if is_temp { Some(active.clone()) } else { None };
    let return_after = if is_temp {
        current_usage
            .five_hour
            .as_ref()
            .and_then(|w| w.resets_at.clone())
    } else {
        None
    };

    // Perform swap
    account::swap_credentials(&active, &next)?;

    // Write swap-info for the wrapper
    let swap_info = crate::types::SwapInfo {
        session_id: hook_input.session_id,
        from_account: active,
        to_account: next,
        reason,
        swapped_at: chrono::Utc::now().to_rfc3339(),
        return_to,
        return_after,
    };

    util::atomic_write_json(&paths::swap_info_file()?, &swap_info, 0o600)?;

    let _ = history::log_swap(SwapLogEntry {
        timestamp: swap_info.swapped_at.clone(),
        from_account: swap_info.from_account.clone(),
        to_account: swap_info.to_account.clone(),
        reason: swap_info.reason.clone(),
        trigger: "stop-hook".to_string(),
        session_id: Some(swap_info.session_id.clone()),
        cwd: hook_input.cwd.clone(),
        from_usage_5h: current_usage.five_hour.as_ref().map(|w| w.utilization),
        from_usage_7d: current_usage.seven_day.as_ref().map(|w| w.utilization),
        to_usage_5h: cache.get(&swap_info.to_account).and_then(|u| u.five_hour.as_ref()).map(|w| w.utilization),
        to_usage_7d: cache.get(&swap_info.to_account).and_then(|u| u.seven_day.as_ref()).map(|w| w.utilization),
        temp_swap: is_temp,
    });

    // Clear rate-limited flag
    let _ = std::fs::remove_file(paths::rate_limited_flag()?);

    Ok(())
}

/// SessionStart hook: record session → account mapping.
pub fn session_start() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let hook_input: SessionStartHookInput = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    let active = match account::get_active()? {
        Some(a) => a,
        None => return Ok(()),
    };

    // Load or create sessions
    let sessions_path = paths::sessions_file()?;
    let mut sessions: crate::types::Sessions = if sessions_path.exists() {
        let content = std::fs::read_to_string(&sessions_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Default::default()
    };

    // Add/update entry
    sessions.insert(
        hook_input.session_id,
        crate::types::SessionEntry {
            account: active,
            started_at: chrono::Utc::now().to_rfc3339(),
            source: hook_input.source.unwrap_or_else(|| "unknown".to_string()),
            cwd: hook_input.cwd.unwrap_or_default(),
        },
    );

    // Prune entries older than 7 days
    let cutoff = chrono::Utc::now() - chrono::Duration::days(7);
    sessions.retain(|_, entry| {
        chrono::DateTime::parse_from_rfc3339(&entry.started_at)
            .map(|dt| dt >= cutoff)
            .unwrap_or(true)
    });

    util::atomic_write_json(&sessions_path, &sessions, 0o600)?;

    // Write to CLAUDE_ENV_FILE if available
    if let Ok(env_file) = std::env::var("CLAUDE_ENV_FILE") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&env_file) {
            let _ = writeln!(f, "export CLAUDE_REVOLVER_ACCOUNT={}", account::get_active()?.unwrap_or_default());
        }
    }

    Ok(())
}

/// PostToolUseFailure hook: detect rate limit errors.
pub fn rate_limit() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let hook_input: RateLimitHookInput = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    let is_rate_limit = hook_input.error.as_ref().map_or(false, |e| {
        let type_match = e
            .error_type
            .as_deref()
            .map_or(false, |t| t.contains("rate_limit"));
        let msg_match = e.message.as_deref().map_or(false, |m| {
            m.contains("rate limit") || m.contains("Rate limit") || m.contains("usage limit")
        });
        type_match || msg_match
    });

    if is_rate_limit {
        let flag = paths::rate_limited_flag()?;
        let now = chrono::Utc::now().to_rfc3339();
        util::atomic_write(&flag, now.as_bytes(), 0o644)?;
    }

    Ok(())
}
