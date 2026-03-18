use anyhow::Result;

use crate::history;
use crate::util::print_info;

pub fn run(count: usize, clear: bool) -> Result<()> {
    if clear {
        history::clear_history()?;
        print_info("swap history cleared");
        return Ok(());
    }

    let entries = history::load_history()?;
    if entries.is_empty() {
        println!("  no swap history");
        return Ok(());
    }

    for (i, entry) in entries.iter().take(count).enumerate() {
        if i > 0 {
            println!();
        }

        let ts = entry.timestamp.get(..19).unwrap_or(&entry.timestamp);
        let temp = if entry.temp_swap { " (temp)" } else { "" };

        let from_usage = format_usage(entry.from_usage_5h, entry.from_usage_7d);
        let to_usage = format_usage(entry.to_usage_5h, entry.to_usage_7d);

        println!(
            "  \x1b[36m{ts}\x1b[0m  {from}{fu} \x1b[33m→\x1b[0m {to}{tu}  [{trigger}]{temp}",
            from = entry.from_account,
            fu = from_usage,
            to = entry.to_account,
            tu = to_usage,
            trigger = entry.trigger,
        );
        println!("    reason: {}", entry.reason);

        if let Some(ref sid) = entry.session_id {
            println!("    session: {sid}");
        }
        if let Some(ref cwd) = entry.cwd {
            if !cwd.is_empty() {
                println!("    cwd: {cwd}");
            }
        }
    }

    if entries.len() > count {
        println!(
            "\n  ({} more entries, use -n to show more)",
            entries.len() - count
        );
    }

    Ok(())
}

fn format_usage(u5h: Option<f64>, u7d: Option<f64>) -> String {
    match (u5h, u7d) {
        (Some(h), Some(d)) => format!(" (5h:{h:.0}% 7d:{d:.0}%)"),
        (Some(h), None) => format!(" (5h:{h:.0}%)"),
        (None, Some(d)) => format!(" (7d:{d:.0}%)"),
        (None, None) => String::new(),
    }
}
