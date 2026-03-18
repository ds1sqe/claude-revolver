use anyhow::Result;

use crate::types::SwapLogEntry;
use crate::{account, history, usage, util};

/// Single path for all credential swaps (wrapper + CLI).
pub fn perform_swap(
    from: &str,
    to: &str,
    reason: &str,
    trigger: &str,
    session_id: Option<&str>,
    cwd: Option<&str>,
    temp_swap: bool,
) -> Result<()> {
    // 1. Swap credentials on disk
    account::swap_credentials(from, to)?;

    // 2. Log history (best-effort)
    let cache = usage::load_cache().unwrap_or_default();
    let _ = history::log_swap(SwapLogEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        from_account: from.to_string(),
        to_account: to.to_string(),
        reason: reason.to_string(),
        trigger: trigger.to_string(),
        session_id: session_id.map(String::from),
        cwd: cwd.map(String::from),
        from_usage_5h: cache
            .get(from)
            .and_then(|u| u.five_hour.as_ref())
            .map(|w| w.utilization),
        from_usage_7d: cache
            .get(from)
            .and_then(|u| u.seven_day.as_ref())
            .map(|w| w.utilization),
        to_usage_5h: cache
            .get(to)
            .and_then(|u| u.five_hour.as_ref())
            .map(|w| w.utilization),
        to_usage_7d: cache
            .get(to)
            .and_then(|u| u.seven_day.as_ref())
            .map(|w| w.utilization),
        temp_swap,
    });

    // 3. Notify (best-effort)
    util::notify(
        "normal",
        "Claude Revolver",
        &format!("{from} → {to}: {reason}"),
    );

    Ok(())
}
