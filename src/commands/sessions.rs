use anyhow::Result;

use crate::paths;
use crate::types::Sessions;

pub fn run() -> Result<()> {
    let path = paths::sessions_file()?;
    if !path.exists() {
        println!("  no session tracking data");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let sessions: Sessions = serde_json::from_str(&content)?;

    if sessions.is_empty() {
        println!("  no sessions tracked");
        return Ok(());
    }

    println!(
        "  {:38} {:16} {:10} {}",
        "SESSION ID", "ACCOUNT", "SOURCE", "STARTED"
    );
    println!("  {}", "─".repeat(80));

    let mut entries: Vec<_> = sessions.iter().collect();
    entries.sort_by(|a, b| b.1.started_at.cmp(&a.1.started_at));

    for (id, entry) in entries {
        println!(
            "  {:38} {:16} {:10} {}",
            id, entry.account, entry.source, entry.started_at,
        );
    }

    Ok(())
}
