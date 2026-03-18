use anyhow::Result;

use crate::config::Thresholds;
use crate::paths;
use crate::types::{CachedAccountUsage, UsageApiResponse, UsageCache};
use crate::util::atomic_write_json;

/// Fetch live usage from the OAuth API.
pub fn fetch_usage(token: &str) -> Result<UsageApiResponse> {
    let url = paths::usage_api_url();
    let mut resp = ureq::get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .call()
        .map_err(|e| anyhow::anyhow!("usage API request failed: {e}"))?;

    let body_str = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| anyhow::anyhow!("usage API response read error: {e}"))?;
    let body: UsageApiResponse = serde_json::from_str(&body_str)
        .map_err(|e| anyhow::anyhow!("usage API response parse error: {e}"))?;

    Ok(body)
}

/// Load usage cache from disk.
pub fn load_cache() -> Result<UsageCache> {
    let path = paths::usage_cache_file()?;
    if !path.exists() {
        return Ok(UsageCache::new());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Save usage cache to disk atomically.
pub fn save_cache(cache: &UsageCache) -> Result<()> {
    let path = paths::usage_cache_file()?;
    atomic_write_json(&path, cache, 0o600)
}

/// Check if an account's usage is over the configured thresholds.
/// Returns (is_over, reason).
pub fn is_over_threshold(usage: &CachedAccountUsage, thresholds: &Thresholds) -> (bool, String) {
    if let Some(ref w) = usage.seven_day {
        if w.utilization >= thresholds.seven_day as f64 {
            return (
                true,
                format!(
                    "seven_day utilization {:.0}% >= threshold {}%",
                    w.utilization, thresholds.seven_day
                ),
            );
        }
    }

    if let Some(ref w) = usage.five_hour {
        if w.utilization >= thresholds.five_hour as f64 {
            return (
                true,
                format!(
                    "five_hour utilization {:.0}% >= threshold {}%",
                    w.utilization, thresholds.five_hour
                ),
            );
        }
    }

    (false, String::new())
}

/// Convert API response to cached account usage.
pub fn api_response_to_cached(resp: &UsageApiResponse, now_iso: &str) -> CachedAccountUsage {
    CachedAccountUsage {
        five_hour: resp.five_hour.clone(),
        seven_day: resp.seven_day.clone(),
        seven_day_sonnet: resp.seven_day_sonnet.clone(),
        seven_day_opus: resp.seven_day_opus.clone(),
        polled_at: now_iso.to_string(),
        token_expired: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UsageWindow;

    #[test]
    fn test_over_threshold_seven_day() {
        let usage = CachedAccountUsage {
            five_hour: Some(UsageWindow {
                utilization: 20.0,
                resets_at: String::new(),
            }),
            seven_day: Some(UsageWindow {
                utilization: 96.0,
                resets_at: String::new(),
            }),
            seven_day_sonnet: None,
            seven_day_opus: None,
            polled_at: String::new(),
            token_expired: false,
        };
        let thresholds = Thresholds {
            five_hour: 90,
            seven_day: 95,
        };
        let (over, reason) = is_over_threshold(&usage, &thresholds);
        assert!(over);
        assert!(reason.contains("seven_day"));
    }

    #[test]
    fn test_under_threshold() {
        let usage = CachedAccountUsage {
            five_hour: Some(UsageWindow {
                utilization: 20.0,
                resets_at: String::new(),
            }),
            seven_day: Some(UsageWindow {
                utilization: 50.0,
                resets_at: String::new(),
            }),
            seven_day_sonnet: None,
            seven_day_opus: None,
            polled_at: String::new(),
            token_expired: false,
        };
        let thresholds = Thresholds::default();
        let (over, _) = is_over_threshold(&usage, &thresholds);
        assert!(!over);
    }

    #[test]
    fn test_threshold_at_100() {
        let usage = CachedAccountUsage {
            five_hour: Some(UsageWindow {
                utilization: 99.0,
                resets_at: String::new(),
            }),
            seven_day: Some(UsageWindow {
                utilization: 99.0,
                resets_at: String::new(),
            }),
            seven_day_sonnet: None,
            seven_day_opus: None,
            polled_at: String::new(),
            token_expired: false,
        };
        // Threshold at 100 means only swap when actually at 100%
        let thresholds = Thresholds {
            five_hour: 100,
            seven_day: 100,
        };
        let (over, _) = is_over_threshold(&usage, &thresholds);
        assert!(!over);
    }
}
