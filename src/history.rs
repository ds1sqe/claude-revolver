use anyhow::Result;

use crate::paths;
use crate::types::SwapLogEntry;
use crate::util::atomic_write_json;

const MAX_ENTRIES: usize = 1000;

/// Append a swap event to the history log (best-effort).
pub fn log_swap(entry: SwapLogEntry) -> Result<()> {
    let mut history = load_history()?;
    history.insert(0, entry);
    history.truncate(MAX_ENTRIES);
    let path = paths::swap_history_file()?;
    atomic_write_json(&path, &history, 0o600)
}

/// Load swap history from disk.
pub fn load_history() -> Result<Vec<SwapLogEntry>> {
    let path = paths::swap_history_file()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Delete the history file.
pub fn clear_history() -> Result<()> {
    let path = paths::swap_history_file()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}
