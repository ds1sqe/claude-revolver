# claude-revolver

Multi-account OAuth credential manager for Claude Code CLI with automatic rate-limit-aware account switching.

## Problem

Claude Max has rolling usage limits (5-hour window + 7-day ceiling). With multiple accounts (personal, work, etc.), switching requires manual logout/login. No way to detect approaching limits or auto-swap.

## How it works

Stores named copies of `~/.claude/.credentials.json` and swaps them on demand. A systemd timer polls the OAuth usage API in the background, and a wrapper around `claude` auto-switches accounts when limits are hit.

## Install

```bash
git clone https://github.com/ds1sqe/claude-revolver
cd claude-revolver
./install.sh
```

Installs to `~/.local/bin/`. Optionally sets up a systemd timer and a Claude Code hook.

### Dependencies

- `bash` (>=4.0), `jq`, `curl` — required
- `fzf` — optional, for TUI picker (falls back to numbered list)
- `notify-send` — optional, for desktop notifications
- `systemd` — optional, for background usage monitoring

## Quick start

```bash
# Save your current login
claude-revolver add personal

# Login to another account and save it
claude logout && claude login
claude-revolver add work

# Switch with TUI picker
claude-revolver

# Or switch directly
claude-revolver switch work

# Launch claude with auto-swap on rate limit
claude-revolver wrap
```

### Transparent wrapping

Add to `.zshrc` / `.bashrc`:

```bash
alias claude='claude-revolver wrap --'
```

Every `claude` invocation goes through the wrapper — pre-checks usage, auto-swaps on rate limit exit.

## Commands

| Command | Description |
|---------|-------------|
| *(no args)* | TUI account picker (fzf) |
| `list`, `ls` | List accounts with usage |
| `add <name>` | Save current credentials as named account |
| `switch`, `sw <name>` | Switch to a named account |
| `remove`, `rm <name>` | Remove an account |
| `status [name]` | Show account info with live usage query |
| `sync` | Save live credentials back to store |
| `wrap [-- args...]` | Launch claude with auto-swap |
| `install-hook` | Add rate-limit detection hook to Claude Code |
| `install-systemd` | Install background usage monitor timer |
| `uninstall-hook` | Remove the hook |
| `uninstall-systemd` | Remove systemd units |

## Optional setup

### Background usage monitoring

```bash
claude-revolver install-systemd
```

Polls the OAuth usage API every 15 minutes for all accounts. Sends desktop notifications when approaching limits. Writes `usage-cache.json` used by `list` and `wrap`.

### Rate limit detection hook

```bash
claude-revolver install-hook
```

Adds a `PostToolUseFailure` hook to `~/.claude/settings.json` that detects rate limit errors mid-session and writes a flag file. The `wrap` command picks this up on exit and offers to switch accounts.

## Limitations

- **CLI terminal only** — Claude Desktop and remote sessions use OAuth exclusively with no hook/wrapper mechanism
- **Requires manual login** — each account must be logged in via `claude login` before adding (OAuth requires browser)
- **Restart needed** — switching credentials requires restarting Claude Code

## Docs

- [Architecture](docs/architecture.md) — system diagram, data layout, components
- [Internals](docs/internals.md) — credential sync, account selection, edge cases, security
