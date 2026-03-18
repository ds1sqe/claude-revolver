use crate::config::{Config, StrategyConfig};
use crate::types::UsageCache;

/// Select the next account to swap to based on the configured strategy.
/// Returns None if no suitable account is found.
pub fn select_next_account(
    current: &str,
    accounts: &[String],
    cache: &UsageCache,
    config: &Config,
) -> Option<String> {
    match config.strategy.strategy_type.as_str() {
        "drain" => select_drain(current, accounts, cache, &config.strategy),
        "balanced" => select_balanced(current, accounts, cache),
        "manual" => None,
        _ => select_drain(current, accounts, cache, &config.strategy),
    }
}

/// Drain strategy: use accounts in priority order until each hits its 7d limit.
fn select_drain(
    current: &str,
    accounts: &[String],
    cache: &UsageCache,
    strategy: &StrategyConfig,
) -> Option<String> {
    let ordered: Vec<&String> = if strategy.order.is_empty() {
        // Auto-order: highest 7d utilization first (drain closest-to-limit)
        let mut sorted: Vec<&String> = accounts.iter().filter(|a| a.as_str() != current).collect();
        sorted.sort_by(|a, b| {
            let a_7d = get_seven_day_util(cache, a);
            let b_7d = get_seven_day_util(cache, b);
            b_7d.partial_cmp(&a_7d).unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted
    } else {
        // User-defined order
        strategy
            .order
            .iter()
            .filter(|a| a.as_str() != current && accounts.contains(a))
            .collect()
    };

    // First pass: find account with 7d under 95%
    for acct in &ordered {
        if is_available(cache, acct) {
            let u7 = get_seven_day_util(cache, acct);
            if u7 < 95.0 {
                return Some(acct.to_string());
            }
        }
    }

    // Second pass: any account with 5h capacity
    for acct in &ordered {
        if is_available(cache, acct) {
            let u5 = get_five_hour_util(cache, acct);
            if u5 < 90.0 {
                return Some(acct.to_string());
            }
        }
    }

    None
}

/// Balanced strategy: pick the account with the lowest 7d utilization.
fn select_balanced(current: &str, accounts: &[String], cache: &UsageCache) -> Option<String> {
    let mut candidates: Vec<&String> = accounts
        .iter()
        .filter(|a| a.as_str() != current && is_available(cache, a))
        .collect();

    candidates.sort_by(|a, b| {
        let a_7d = get_seven_day_util(cache, a);
        let b_7d = get_seven_day_util(cache, b);
        a_7d.partial_cmp(&b_7d)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let a_5h = get_five_hour_util(cache, a);
                let b_5h = get_five_hour_util(cache, b);
                a_5h.partial_cmp(&b_5h)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    candidates.first().map(|a| a.to_string())
}

/// Check if an account is available (not expired, has cached usage).
fn is_available(cache: &UsageCache, name: &str) -> bool {
    match cache.get(name) {
        Some(u) => !u.token_expired,
        None => true, // No cache = assume available
    }
}

fn get_seven_day_util(cache: &UsageCache, name: &str) -> f64 {
    cache
        .get(name)
        .and_then(|u| u.seven_day.as_ref())
        .map(|w| w.utilization)
        .unwrap_or(0.0)
}

fn get_five_hour_util(cache: &UsageCache, name: &str) -> f64 {
    cache
        .get(name)
        .and_then(|u| u.five_hour.as_ref())
        .map(|w| w.utilization)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CachedAccountUsage, UsageWindow};

    fn make_usage(five_h: f64, seven_d: f64) -> CachedAccountUsage {
        CachedAccountUsage {
            five_hour: Some(UsageWindow {
                utilization: five_h,
                resets_at: Some("2026-03-18T04:00:00Z".to_string()),
            }),
            seven_day: Some(UsageWindow {
                utilization: seven_d,
                resets_at: Some("2026-03-20T04:00:00Z".to_string()),
            }),
            seven_day_sonnet: None,
            seven_day_opus: None,
            polled_at: String::new(),
            token_expired: false,
        }
    }

    fn make_expired() -> CachedAccountUsage {
        let mut u = make_usage(0.0, 0.0);
        u.token_expired = true;
        u
    }

    #[test]
    fn test_balanced_picks_lowest_7d() {
        let accounts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut cache = UsageCache::new();
        cache.insert("a".to_string(), make_usage(20.0, 80.0));
        cache.insert("b".to_string(), make_usage(50.0, 30.0));
        cache.insert("c".to_string(), make_usage(10.0, 60.0));

        let result = select_balanced("a", &accounts, &cache);
        assert_eq!(result, Some("b".to_string())); // lowest 7d
    }

    #[test]
    fn test_balanced_skips_expired() {
        let accounts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut cache = UsageCache::new();
        cache.insert("a".to_string(), make_usage(20.0, 80.0));
        cache.insert("b".to_string(), make_expired());
        cache.insert("c".to_string(), make_usage(10.0, 60.0));

        let result = select_balanced("a", &accounts, &cache);
        assert_eq!(result, Some("c".to_string()));
    }

    #[test]
    fn test_drain_auto_order_picks_highest_7d() {
        let accounts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut cache = UsageCache::new();
        cache.insert("a".to_string(), make_usage(20.0, 40.0));
        cache.insert("b".to_string(), make_usage(10.0, 90.0)); // highest 7d, but under 95
        cache.insert("c".to_string(), make_usage(5.0, 20.0));

        let config = StrategyConfig {
            strategy_type: "drain".to_string(),
            order: vec![],
        };

        let result = select_drain("a", &accounts, &cache, &config);
        assert_eq!(result, Some("b".to_string())); // drain highest-7d first
    }

    #[test]
    fn test_drain_priority_order() {
        let accounts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut cache = UsageCache::new();
        cache.insert("a".to_string(), make_usage(20.0, 40.0));
        cache.insert("b".to_string(), make_usage(10.0, 30.0));
        cache.insert("c".to_string(), make_usage(5.0, 20.0));

        let config = StrategyConfig {
            strategy_type: "drain".to_string(),
            order: vec!["c".to_string(), "b".to_string(), "a".to_string()],
        };

        let result = select_drain("a", &accounts, &cache, &config);
        assert_eq!(result, Some("c".to_string())); // first in priority order
    }

    #[test]
    fn test_drain_skips_maxed_7d() {
        let accounts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut cache = UsageCache::new();
        cache.insert("a".to_string(), make_usage(20.0, 40.0));
        cache.insert("b".to_string(), make_usage(10.0, 97.0)); // over 95
        cache.insert("c".to_string(), make_usage(5.0, 20.0));

        let config = StrategyConfig {
            strategy_type: "drain".to_string(),
            order: vec!["b".to_string(), "c".to_string()],
        };

        let result = select_drain("a", &accounts, &cache, &config);
        assert_eq!(result, Some("c".to_string())); // b is maxed, pick c
    }

    #[test]
    fn test_no_candidates() {
        let accounts = vec!["a".to_string()];
        let cache = UsageCache::new();

        let result = select_balanced("a", &accounts, &cache);
        assert_eq!(result, None);
    }

}
