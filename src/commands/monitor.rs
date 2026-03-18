use anyhow::Result;

use crate::{account, config, paths, usage, util};

pub fn run() -> Result<()> {
    let accounts = account::list_accounts()?;
    if accounts.is_empty() {
        eprintln!("claude-revolver-monitor: no accounts configured");
        return Ok(());
    }

    let config = config::Config::load().unwrap_or_default();
    let mut cache = usage::load_cache().unwrap_or_default();
    let now_iso = chrono::Utc::now().to_rfc3339();

    for name in &accounts {
        let creds = match account::read_credentials(name) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let token = &creds.claude_ai_oauth.access_token;
        match usage::fetch_usage(token) {
            Ok(resp) => {
                let cached = usage::api_response_to_cached(&resp, &now_iso);

                // Check thresholds and notify
                if config.notify {
                    if let Some(ref w) = cached.seven_day {
                        if w.utilization >= 95.0 {
                            util::notify(
                                "critical",
                                "Claude Revolver",
                                &format!("Account '{name}' 7-day usage at {:.0}%!", w.utilization),
                            );
                        } else if w.utilization >= 90.0 {
                            util::notify(
                                "normal",
                                "Claude Revolver",
                                &format!("Account '{name}' 7-day usage at {:.0}%", w.utilization),
                            );
                        }
                    }
                    if let Some(ref w) = cached.five_hour {
                        if w.utilization >= 80.0 {
                            util::notify(
                                "normal",
                                "Claude Revolver",
                                &format!("Account '{name}' 5-hour usage at {:.0}%", w.utilization),
                            );
                        }
                    }
                }

                let u5 = cached
                    .five_hour
                    .as_ref()
                    .map_or(0.0, |w| w.utilization);
                let u7 = cached
                    .seven_day
                    .as_ref()
                    .map_or(0.0, |w| w.utilization);
                eprintln!("claude-revolver-monitor: polled '{name}': 5h={u5:.0}% 7d={u7:.0}%");

                cache.insert(name.clone(), cached);
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("401") || err_str.contains("Unauthorized") {
                    let mut entry = cache.remove(name).unwrap_or_else(|| {
                        crate::types::CachedAccountUsage {
                            five_hour: None,
                            seven_day: None,
                            seven_day_sonnet: None,
                            seven_day_opus: None,
                            polled_at: now_iso.clone(),
                            token_expired: true,
                        }
                    });
                    entry.token_expired = true;
                    entry.polled_at = now_iso.clone();
                    cache.insert(name.clone(), entry);

                    if config.notify {
                        util::notify(
                            "critical",
                            "Claude Revolver",
                            &format!("Account '{name}' token expired — run 'claude login'"),
                        );
                    }
                }
                eprintln!(
                    "claude-revolver-monitor: failed to poll '{name}': {e}"
                );
            }
        }
    }

    // Prune old sessions
    let sessions_path = paths::sessions_file()?;
    if sessions_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&sessions_path) {
            if let Ok(mut sessions) = serde_json::from_str::<crate::types::Sessions>(&content) {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(7);
                let before = sessions.len();
                sessions.retain(|_, entry| {
                    chrono::DateTime::parse_from_rfc3339(&entry.started_at)
                        .map(|dt| dt >= cutoff)
                        .unwrap_or(true)
                });
                if sessions.len() != before {
                    let _ = util::atomic_write_json(&sessions_path, &sessions, 0o600);
                }
            }
        }
    }

    usage::save_cache(&cache)?;
    eprintln!("claude-revolver-monitor: cache updated");
    Ok(())
}
