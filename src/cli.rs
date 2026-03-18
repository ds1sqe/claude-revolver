use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "claude-revolver",
    version,
    about = "Multi-account OAuth credential manager for Claude Code CLI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Save current credentials as a named account
    Add { name: String },

    /// Remove a named account
    #[command(alias = "rm")]
    Remove { name: String },

    /// List all accounts with usage
    #[command(alias = "ls")]
    List,

    /// Switch to a named account
    #[command(alias = "sw")]
    Switch { name: String },

    /// Show account info with live usage query
    #[command(alias = "st")]
    Status { name: Option<String> },

    /// Save live credentials back to store
    Sync,

    /// Show session-to-account mapping
    Sessions,

    /// Launch claude with auto-swap and auto-resume
    Wrap {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Poll usage API for all accounts (called by systemd)
    Monitor,

    /// Hook handlers (called by Claude Code)
    Hook {
        #[command(subcommand)]
        kind: HookKind,
    },

    /// Install hooks or systemd units
    Install {
        #[command(subcommand)]
        target: InstallTarget,
    },

    /// Uninstall hooks or systemd units
    Uninstall {
        #[command(subcommand)]
        target: InstallTarget,
    },
}

#[derive(Subcommand)]
pub enum HookKind {
    /// Stop hook — checks usage, swaps if over threshold
    Stop,
    /// SessionStart hook — records session-to-account mapping
    SessionStart,
    /// PostToolUseFailure hook — detects rate limit errors
    RateLimit,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value (dotted key path)
    Set { key: String, value: String },
}

#[derive(Subcommand, Clone)]
pub enum InstallTarget {
    /// Install/uninstall Claude Code hooks
    Hook,
    /// Install/uninstall systemd user timer
    Systemd,
}
