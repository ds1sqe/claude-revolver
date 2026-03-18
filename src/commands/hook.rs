use std::io::Read;

use anyhow::Result;

use crate::paths;
use crate::types::{
    RateLimitHookInput, RateLimitSignal, SessionStartHookInput, SessionStartedSignal,
    StopHookInput, StoppedSignal,
};
use crate::util::{atomic_write_json, ensure_dir};

/// Write a PID-namespaced signal file.
/// Returns Ok(()) or Err if not in wrapped mode.
fn write_signal<T: serde::Serialize>(name: &str, payload: &T) -> Result<()> {
    let pid = std::env::var("CLAUDE_REVOLVER_WRAPPER_PID")
        .map_err(|_| anyhow::anyhow!("not wrapped"))?;
    let dir = paths::signals_dir()?;
    ensure_dir(&dir, 0o700)?;
    let path = dir.join(format!("{pid}-{name}"));
    atomic_write_json(&path, payload, 0o600)
}

fn is_wrapped() -> bool {
    std::env::var("CLAUDE_REVOLVER_WRAPPED").is_ok()
}

/// Stop hook: reports "a turn just ended". No evaluation, no decisions.
pub fn stop() -> Result<()> {
    if !is_wrapped() {
        return Ok(());
    }

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let hook_input: StopHookInput = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    write_signal(
        "stopped",
        &StoppedSignal {
            session_id: hook_input.session_id,
        },
    )?;

    Ok(())
}

/// SessionStart hook: reports "a session just started", passes session_id.
pub fn session_start() -> Result<()> {
    if !is_wrapped() {
        return Ok(());
    }

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let hook_input: SessionStartHookInput = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    write_signal(
        "session-started",
        &SessionStartedSignal {
            session_id: hook_input.session_id,
            cwd: hook_input.cwd,
            source: hook_input.source,
        },
    )?;

    Ok(())
}

/// PostToolUseFailure hook: reports "a rate limit error occurred".
pub fn rate_limit() -> Result<()> {
    if !is_wrapped() {
        return Ok(());
    }

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
        write_signal(
            "rate-limited",
            &RateLimitSignal {
                timestamp: chrono::Utc::now().to_rfc3339(),
            },
        )?;
    }

    Ok(())
}
