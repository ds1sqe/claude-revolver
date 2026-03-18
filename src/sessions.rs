use anyhow::Result;

use crate::paths;
use crate::types::{SessionEntry, Sessions};
use crate::util::atomic_write_json;

pub fn load() -> Result<Sessions> {
    let path = paths::sessions_file()?;
    if !path.exists() {
        return Ok(Sessions::new());
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

fn save(sessions: &Sessions) -> Result<()> {
    let path = paths::sessions_file()?;
    atomic_write_json(&path, sessions, 0o600)
}

pub fn register(
    session_id: &str,
    account: &str,
    cwd: Option<String>,
    source: Option<String>,
) -> Result<()> {
    let mut sessions = load()?;
    sessions.insert(
        session_id.to_string(),
        SessionEntry {
            account: account.to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
            source: source.unwrap_or_else(|| "unknown".to_string()),
            cwd: cwd.unwrap_or_default(),
        },
    );
    prune_old(&mut sessions);
    save(&sessions)
}

pub fn close(session_id: &str) -> Result<()> {
    let mut sessions = load()?;
    sessions.remove(session_id);
    save(&sessions)
}

fn prune_old(sessions: &mut Sessions) {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(7);
    sessions.retain(|_, entry| {
        chrono::DateTime::parse_from_rfc3339(&entry.started_at)
            .map(|dt| dt >= cutoff)
            .unwrap_or(true)
    });
}
