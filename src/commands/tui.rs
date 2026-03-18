use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::Result;

use crate::{account, usage, util};

pub fn run() -> Result<()> {
    let accounts = account::list_accounts()?;
    if accounts.is_empty() {
        anyhow::bail!("no accounts — use 'claude-revolver add <name>' to save current credentials");
    }

    let active = account::get_active()?;
    let cache = usage::load_cache().unwrap_or_default();

    if is_fzf_available() {
        run_fzf(&accounts, active.as_deref(), &cache)
    } else {
        run_fallback(&accounts, active.as_deref())
    }
}

fn is_fzf_available() -> bool {
    Command::new("fzf")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_fzf(
    accounts: &[String],
    active: Option<&str>,
    cache: &crate::types::UsageCache,
) -> Result<()> {
    let mut lines = Vec::new();
    for acct in accounts {
        let marker = if active == Some(acct.as_str()) {
            "*"
        } else {
            " "
        };

        let sub_type = account::read_credentials(acct)
            .ok()
            .and_then(|c| c.claude_ai_oauth.subscription_type)
            .unwrap_or_else(|| "?".to_string());

        let (u5, u7) = cache
            .get(acct)
            .map(|u| {
                (
                    u.five_hour
                        .as_ref()
                        .map(|w| format!("{:.0}%", w.utilization))
                        .unwrap_or_else(|| "?".to_string()),
                    u.seven_day
                        .as_ref()
                        .map(|w| format!("{:.0}%", w.utilization))
                        .unwrap_or_else(|| "?".to_string()),
                )
            })
            .unwrap_or_else(|| ("?".to_string(), "?".to_string()));

        lines.push(format!(
            "{marker} {acct:16} {sub_type:8} 5h:{u5:5} 7d:{u7:5}"
        ));
    }

    let input = lines.join("\n");
    let header = format!("current: {}", active.unwrap_or("none"));

    let exe = std::env::current_exe()?.display().to_string();
    let mut child = Command::new("fzf")
        .args([
            "--prompt",
            "switch account > ",
            "--header",
            &header,
            "--ansi",
            "--no-multi",
            "--preview",
            &format!("{exe} status {{2}}"),
            "--preview-window",
            "right:50%:wrap",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Ok(()); // user cancelled
    }

    let selected = String::from_utf8_lossy(&output.stdout);
    let name = selected.split_whitespace().nth(1);

    if let Some(name) = name {
        crate::commands::switch::run(name)?;
    }

    Ok(())
}

fn run_fallback(accounts: &[String], active: Option<&str>) -> Result<()> {
    println!("Select account (current: {}):", active.unwrap_or("none"));
    for (i, acct) in accounts.iter().enumerate() {
        let marker = if active == Some(acct.as_str()) {
            "*"
        } else {
            " "
        };
        println!("  {}) {} {}", i + 1, marker, acct);
    }

    print!("Enter number: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let choice: usize = input.trim().parse().unwrap_or(0);
    if choice >= 1 && choice <= accounts.len() {
        crate::commands::switch::run(&accounts[choice - 1])?;
    } else {
        util::print_error("invalid selection");
    }

    Ok(())
}
