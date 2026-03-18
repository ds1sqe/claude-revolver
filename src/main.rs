mod account;
mod cli;
mod commands;
mod config;
mod error;
mod history;
mod paths;
mod sessions;
mod strategy;
mod swap;
mod types;
mod usage;
mod util;

use clap::Parser;

use cli::{Cli, Command, ConfigAction, HookKind};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        None => commands::tui::run(),
        Some(cmd) => match cmd {
            Command::Add { name } => commands::add::run(&name),
            Command::Remove { name } => commands::remove::run(&name),
            Command::List => commands::list::run(),
            Command::Switch { name } => commands::switch::run(&name),
            Command::Status { name } => commands::status::run(name.as_deref()),
            Command::Sync => commands::sync::run(),
            Command::Sessions => commands::sessions::run(),
            Command::History { count, clear } => commands::history::run(count, clear),
            Command::Wrap { args } => commands::wrap::run(&args),
            Command::Config { action } => match action {
                ConfigAction::Show => commands::config::show(),
                ConfigAction::Set { key, value } => commands::config::set(&key, &value),
            },
            Command::Monitor => commands::monitor::run(),
            Command::Hook { kind } => match kind {
                HookKind::Stop => commands::hook::stop(),
                HookKind::SessionStart => commands::hook::session_start(),
                HookKind::RateLimit => commands::hook::rate_limit(),
            },
            Command::Install => commands::install::install(),
            Command::Uninstall => commands::install::uninstall(),
        },
    };

    if let Err(e) = result {
        util::print_error(&format!("{e:#}"));
        std::process::exit(1);
    }
}
