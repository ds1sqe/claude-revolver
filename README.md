# claude-revolver

Multi-account OAuth credential manager for Claude Code CLI with automatic rate-limit-aware account switching.

## Problem

Claude Max has rolling usage limits (5-hour window + 7-day ceiling). With multiple accounts (personal, work, etc.), switching requires manual logout/login. No way to detect approaching limits or auto-swap.

## How it works

1. **Stores** named copies of `~/.claude/.credentials.json`
2. **Monitors** usage via OAuth API (systemd timer, every 5 min)
3. **Detects** when you're approaching limits (Stop hook checks usage cache)
4. **Swaps** credentials on disk and auto-resumes your session with the new account

```
claude running → Stop hook → usage at 92% → swap to "work" → claude stops
  wrapper → claude --resume <session-id> "Go continue." → resumes with new account
```

Sessions are tracked per-account, so you can see which account each session used.

## Install

```bash
git clone https://github.com/ds1sqe/claude-revolver
cd claude-revolver
./install.sh
```

Installs to `~/.local/bin/`. Sets up hooks and systemd timer.

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

# Launch claude with auto-swap + auto-resume
claude-revolver wrap
```

### Transparent wrapping

Add to `.zshrc` / `.bashrc`:

```bash
alias claude='claude-revolver wrap --'
```

When usage hits the threshold, the Stop hook swaps credentials and the wrapper auto-resumes — seamless continuation across accounts.

### Without the wrapper

Even without the wrapper, the Stop hook still swaps credentials on disk. Claude prints `Resume with: claude --resume <id>` as usual — just run it and the new account is active.

## Configuration

`~/.config/claude-revolver/config.json`:

```json
{
  "poll_interval_seconds": 300,
  "thresholds": {
    "five_hour": 90,
    "seven_day": 95
  },
  "strategy": {
    "type": "drain",
    "order": ["personal", "work", "client"]
  },
  "auto_resume": true,
  "auto_message": "Go continue.",
  "notify": true
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `poll_interval_seconds` | 300 | Usage API poll frequency |
| `thresholds.five_hour` | 90 | 5h utilization % trigger (0–100) |
| `thresholds.seven_day` | 95 | 7d utilization % trigger (0–100) |
| `strategy.type` | `drain` | `drain`, `balanced`, or `manual` |
| `strategy.order` | `[]` | Priority list for `drain` (empty = auto by 7d desc) |
| `auto_resume` | true | Auto-resume session after swap |
| `auto_message` | `Go continue.` | Prompt sent on auto-resume |
| `notify` | true | Desktop notifications |

### Strategies

- **`drain`** — Use one account until its 7d is maxed, then the next. Set `order` to control priority, or leave empty to auto-drain highest-7d first.
- **`balanced`** — Spread load. Always pick the account with lowest 7d utilization.
- **`manual`** — No auto-swap. Only switch with `claude-revolver switch`.

### 5h temp-swap

If the active account hits the 5h limit but still has 7d budget, the system temporarily switches and **swaps back** when the 5h window resets. This avoids wasting 7d capacity on a short-term bottleneck.

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
| `sessions` | Show session → account mapping |
| `wrap [-- args...]` | Launch claude with auto-swap + auto-resume |
| `config show` | Show current config |
| `config set <key> <val>` | Update a config value |
| `install-hook` | Install all hooks (Stop, SessionStart, PostToolUseFailure) |
| `install-systemd` | Install background usage monitor timer |
| `uninstall-hook` | Remove hooks |
| `uninstall-systemd` | Remove systemd units |

## Limitations

- **CLI terminal only** — Claude Desktop / remote / web use OAuth with no hook mechanism
- **Requires manual login** — each account must be logged in via `claude login` first
- **Session restart** — credentials are cached in memory; swapping requires stop + resume

## Docs

- [Architecture](docs/architecture.md) — system diagram, hook flow, data layout, components
- [Internals](docs/internals.md) — credential sync, selection algorithm, config, edge cases
