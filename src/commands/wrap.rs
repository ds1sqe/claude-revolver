use std::process::Command;
use std::time::Duration;

use anyhow::Result;

use crate::types::{RateLimitSignal, SessionStartedSignal, StoppedSignal};
use crate::{account, config, paths, sessions, strategy, swap, usage, util};

struct WrapperState {
    wrapper_pid: u32,
    active: String,
    args: Vec<String>,
    session_id: Option<String>,
    config: config::Config,
}

impl WrapperState {
    fn signal_path(&self, name: &str) -> Result<std::path::PathBuf> {
        paths::signal_file(self.wrapper_pid, name)
    }

    fn clear_signals(&self) -> Result<()> {
        let dir = paths::signals_dir()?;
        if !dir.exists() {
            return Ok(());
        }
        let prefix = format!("{}-", self.wrapper_pid);
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(&prefix)
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}

fn read_signal<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Option<T> {
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let val: T = serde_json::from_str(&content).ok()?;
    let _ = std::fs::remove_file(path);
    Some(val)
}

pub fn run(args: &[String]) -> Result<()> {
    let config = config::Config::load().unwrap_or_default();
    let mut state = WrapperState {
        wrapper_pid: std::process::id(),
        active: match account::get_active()? {
            Some(a) => a,
            None => {
                util::print_warn(
                    "no active account set — launching claude without account management",
                );
                return exec_claude(args);
            }
        },
        args: args.to_vec(),
        session_id: None,
        config,
    };

    let accounts = account::list_accounts()?;

    // Pre-check: swap before first launch if already over threshold
    if accounts.len() > 1 {
        let cache = usage::load_cache().unwrap_or_default();
        if let Some(current_usage) = cache.get(&state.active) {
            let (over, reason) = usage::is_over_threshold(current_usage, &state.config.thresholds);
            if over {
                if let Some(next) =
                    strategy::select_next_account(&state.active, &accounts, &cache, &state.config)
                {
                    util::print_warn(&format!(
                        "active account '{}' usage high ({reason})",
                        state.active
                    ));
                    swap::perform_swap(
                        &state.active,
                        &next,
                        &reason,
                        "precheck",
                        None,
                        std::env::current_dir()
                            .ok()
                            .map(|p| p.display().to_string())
                            .as_deref(),
                        false,
                    )?;
                    state.active = next.clone();
                    util::print_info(&format!("auto-switched to '{next}'"));
                } else {
                    util::print_warn("all accounts near limits — proceeding with current");
                }
            }
        }
    }

    // Ensure signals dir exists
    util::ensure_dir(&paths::signals_dir()?, 0o700)?;

    let mut killed_for_swap;
    let mut rebalance_target: Option<String>;

    loop {
        state.clear_signals()?;
        state.session_id = None;
        killed_for_swap = false;
        rebalance_target = None;

        util::print_info(&format!("launching claude (account: {})", state.active));

        let mut child = Command::new("claude")
            .env("CLAUDE_REVOLVER_WRAPPED", "1")
            .env(
                "CLAUDE_REVOLVER_WRAPPER_PID",
                state.wrapper_pid.to_string(),
            )
            .args(&state.args)
            .spawn()
            .map_err(|e| anyhow::anyhow!("failed to launch claude: {e}"))?;

        // ── Poll loop: monitor signals while claude runs ──
        let exit_status = loop {
            // Child exited naturally
            if let Some(status) = child.try_wait()? {
                break status;
            }

            // Learn session_id from SessionStart hook
            if state.session_id.is_none() {
                if let Some(s) = read_signal::<SessionStartedSignal>(
                    &state.signal_path("session-started")?,
                ) {
                    state.session_id = Some(s.session_id.clone());
                    let _ = sessions::register(
                        &s.session_id,
                        &state.active,
                        s.cwd,
                        s.source,
                    );
                }
            }

            // Rate limit → kill immediately
            if state.signal_path("rate-limited")?.exists() {
                killed_for_swap = true;
                let _ = child.kill();
                break child.wait()?;
            }

            // Turn ended → wrapper evaluates threshold
            if let Some(_stopped) =
                read_signal::<StoppedSignal>(&state.signal_path("stopped")?)
            {
                let cache = usage::load_cache().unwrap_or_default();
                if let Some(current) = cache.get(&state.active) {
                    let (over, _) =
                        usage::is_over_threshold(current, &state.config.thresholds);
                    if over {
                        killed_for_swap = true;
                        let _ = child.kill();
                        break child.wait()?;
                    }
                }

                // 2. Should we rebalance? (drain mode only)
                let accounts = account::list_accounts().unwrap_or_default();
                if let Some(target) = strategy::should_rebalance(
                    &state.active,
                    &accounts,
                    &cache,
                    &state.config,
                ) {
                    rebalance_target = Some(target);
                    killed_for_swap = true;
                    let _ = child.kill();
                    break child.wait()?;
                }
            }

            std::thread::sleep(Duration::from_secs(1));
        };

        // ── Post-exit ──
        let _ = account::sync_back();

        if killed_for_swap {
            // Read remaining signals
            let _ = read_signal::<RateLimitSignal>(&state.signal_path("rate-limited")?);

            let (next, reason, trigger) = if let Some(target) = rebalance_target.take() {
                // Rebalance: we already know the target
                (
                    target,
                    "rebalance to priority account".to_string(),
                    "rebalance",
                )
            } else {
                // Threshold/rate-limit: strategy picks next
                let cache = usage::load_cache().unwrap_or_default();
                let accounts = account::list_accounts()?;

                let reason = if let Some(current) = cache.get(&state.active) {
                    let (_, reason) =
                        usage::is_over_threshold(current, &state.config.thresholds);
                    if reason.is_empty() {
                        "rate limit hit".to_string()
                    } else {
                        reason
                    }
                } else {
                    "rate limit hit".to_string()
                };

                match strategy::select_next_account(
                    &state.active,
                    &accounts,
                    &cache,
                    &state.config,
                ) {
                    Some(next) => (next, reason, "threshold"),
                    None => {
                        util::print_warn("no accounts available for swap");
                        // Fall through to normal exit
                        if let Some(ref sid) = state.session_id {
                            let _ = sessions::close(sid);
                        }
                        state.clear_signals()?;
                        std::process::exit(exit_status.code().unwrap_or(0));
                    }
                }
            };

            util::print_warn(&format!(
                "swapped from '{}' to '{next}': {reason}",
                state.active
            ));

            swap::perform_swap(
                &state.active,
                &next,
                &reason,
                trigger,
                state.session_id.as_deref(),
                std::env::current_dir()
                    .ok()
                    .map(|p| p.display().to_string())
                    .as_deref(),
                false,
            )?;

            state.active = next;

            if state.config.auto_resume {
                if let Some(ref sid) = state.session_id {
                    util::print_info(&format!("auto-resuming session {sid}"));
                    state.args = vec![
                        "--resume".into(),
                        sid.clone(),
                        state.config.auto_message.clone(),
                    ];
                    continue; // re-enter loop
                }
            }
            // No session_id or no auto_resume → exit
            return Ok(());
        }

        // Normal exit
        if let Some(ref sid) = state.session_id {
            let _ = sessions::close(sid);
        }
        state.clear_signals()?;
        std::process::exit(exit_status.code().unwrap_or(0));
    }
}

fn exec_claude(args: &[String]) -> Result<()> {
    let status = Command::new("claude")
        .args(args)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to launch claude: {e}"))?;
    std::process::exit(status.code().unwrap_or(0));
}
