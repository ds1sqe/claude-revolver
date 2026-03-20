# claude-revolver

Multi-account OAuth credential manager for Claude Code CLI. Automatically switches accounts when you hit rate limits.

## Why

Claude Max has rolling usage caps: a 5-hour window and a 7-day ceiling. If you have multiple accounts (personal, work, etc.), hitting a limit means manual logout/login. There's no built-in way to detect approaching limits or swap automatically.

claude-revolver solves this by storing named credential sets, monitoring usage via the OAuth API, and swapping accounts transparently when thresholds are reached.

## How it works

```
claude-revolver wrap
  └─ wrapper spawns claude with CLAUDE_REVOLVER_WRAPPED=1
      └─ poll loop (1s):
          ├─ SessionStart hook → signals/{pid}-session-started (session_id)
          ├─ RateLimit hook    → signals/{pid}-rate-limited    → wrapper kills child
          ├─ Stop hook         → signals/{pid}-stopped         → wrapper evaluates usage
          │                                                       over threshold? → kill
          └─ child exits
              └─ wrapper: sync_back → evaluate → perform_swap → resume
                  └─ claude --resume <session-id> "Go continue."
```

1. **Store** named copies of `~/.claude/.credentials.json`
2. **Monitor** usage via OAuth API (`GET /api/oauth/usage`) on a 1-minute systemd timer
3. **Detect** via wrapper poll loop: rate-limit signals trigger immediate kill, stop signals trigger threshold evaluation
4. **Swap** credentials via single `perform_swap()` path and auto-resume the session
5. **Log** every swap event with timestamp, trigger, reason, usage, and session context

### Architecture: two owners, zero races

| Owner | Writes | Role |
|---|---|---|
| **Wrapper** | `active`, `credentials`, `history`, `sessions` | Single brain for automated swaps |
| **CLI** | `active`, `credentials`, `history` | Manual user commands |
| **Hooks** | `signals/{pid}-*` only | Dumb signal emitters, gated by env var |
| **Monitor** | `usage-cache` only | Data collector, no decisions |

Hooks are PID-namespaced — multiple wrappers in different terminals never cross-talk.

## Install

### From git repo
```bash
git clone https://github.com/ds1sqe/claude-revolver
cd claude-revolver
cargo install --path .
```

### From cargo
```bash
cargo install claude-revolver
```

Installs to `~/.cargo/bin/`. Then set up hooks and systemd timer:

```bash
claude-revolver install    # installs hooks + systemd timer (coupled)
```

### Dependencies

- **Rust** (build only)
- **fzf** — optional, for TUI account picker (falls back to numbered list)
- **notify-send** — optional, for desktop notifications
- **systemd** — optional, for background usage monitoring

## Quick start

```bash
# Save your current login as "personal"
claude-revolver add personal

# Login to another account and save it
claude logout && claude login
claude-revolver add work

# Switch with TUI picker
claude-revolver

# Or switch directly
claude-revolver switch work

# Check usage across all accounts
claude-revolver list

# Launch claude with auto-swap + auto-resume
claude-revolver wrap
```

### Transparent wrapping

Add to `.zshrc` / `.bashrc`:

```bash
alias claude='claude-revolver wrap --'
```

The wrapper spawns claude and polls for hook signals. When usage crosses a threshold or a rate limit is hit, the wrapper kills claude, swaps credentials, and auto-resumes. Seamless continuation across accounts.

### Without the wrapper

Without the wrapper, hooks are no-ops (gated by `CLAUDE_REVOLVER_WRAPPED` env var). Use `claude-revolver switch` for manual account changes.

## Configuration

```bash
claude-revolver config show
claude-revolver config set thresholds.seven_day 80
claude-revolver config set strategy.type balanced
```

Config file: `~/.config/claude-revolver/config.json`

```json
{
  "poll_interval_seconds": 60,
  "thresholds": {
    "five_hour": 90,
    "seven_day": 95
  },
  "strategy": {
    "type": "drain",
    "order": ["personal", "work"]
  },
  "auto_resume": true,
  "auto_message": "Go continue.",
  "notify": true
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `thresholds.five_hour` | 90 | Swap when 5h utilization reaches this % (0-100) |
| `thresholds.seven_day` | 95 | Swap when 7d utilization reaches this % (0-100) |
| `strategy.type` | `drain` | `drain`, `balanced`, or `manual` |
| `strategy.order` | `[]` | Priority order for drain (empty = auto by highest 7d) |
| `auto_resume` | true | Auto-resume session after swap |
| `auto_message` | `Go continue.` | Message sent on auto-resume |
| `notify` | true | Desktop notifications via notify-send |
| `poll_interval_seconds` | 60 | Usage API poll interval (for systemd timer) |

Setting thresholds to `100` means reactive-only: swap only when actually rate-limited, not preemptively.

### Strategies

- **drain** — Use one account until its 7-day limit is maxed, then move to the next. Set `order` to control priority, or leave empty to auto-drain highest-7d first.
- **balanced** — Spread load evenly. Always picks the account with lowest 7-day utilization.
- **manual** — No automatic swapping. Only switch with `claude-revolver switch`.

### 5-hour recovery (drain mode)

If the active account hits the 5-hour threshold but still has 7-day budget, the wrapper swaps to another account. On every subsequent turn end, it checks: **has the priority account recovered?** If so, it swaps back automatically. No timers or state tracking — the strategy re-evaluates what's optimal on every turn.

## Commands

| Command | Description |
|---------|-------------|
| *(no args)* | TUI account picker (fzf or numbered list) |
| `add <name>` | Save current credentials as a named account |
| `remove <name>` | Remove an account (alias: `rm`) |
| `list` | List accounts with usage and reset times (alias: `ls`) |
| `switch <name>` | Switch to a named account (alias: `sw`) |
| `status [name]` | Show account info with live usage query (alias: `st`) |
| `sync` | Save live credentials back to the active account's store |
| `sessions` | Show session-to-account mapping |
| `history [-n N]` | Show swap history log (default: last 20) |
| `history --clear` | Clear swap history |
| `wrap [-- args...]` | Launch claude with auto-swap and auto-resume |
| `config show` | Show current configuration |
| `config set <key> <val>` | Set a config value (dotted paths, e.g. `thresholds.five_hour`) |
| `monitor` | Poll usage API for all accounts (called by systemd timer) |
| `install` | Install hooks and systemd timer (idempotent) |
| `uninstall` | Remove hooks and systemd timer |

## Swap history

Every account swap is logged persistently:

```bash
$ claude-revolver history
  2026-03-18T06:12:33  personal (5h:50% 7d:96%) → work (5h:10% 7d:30%)  [threshold]
    reason: seven_day utilization 96% >= threshold 95%
    session: 143eec0f-277e-4ce1-95f1-58eb56331874
    cwd: /home/user/project

  2026-03-18T05:00:12  work → personal  [manual]
    reason: manual switch
    cwd: /home/user
```

Each entry records: timestamp, trigger source (`precheck`, `threshold`, `manual`), from/to accounts with usage at swap time, reason, session ID, working directory.

History is capped at 1000 entries.

## Data layout

```
~/.local/share/claude-revolver/
├── active                    # Current account name
├── usage-cache.json          # Cached usage for all accounts (monitor writes)
├── sessions.json             # Session ID → account mapping (wrapper writes)
├── swap-history.json         # Persistent swap log (perform_swap writes)
├── signals/                  # PID-namespaced, transient (hooks write, wrapper reads)
│   ├── {pid}-session-started
│   ├── {pid}-stopped
│   └── {pid}-rate-limited
└── <account-name>/
    └── credentials.json      # Stored OAuth credentials

~/.config/claude-revolver/
└── config.json               # User configuration
```

## Limitations

- **CLI terminal only** — Claude Desktop and web have no hook mechanism
- **Requires manual login** — each account must be logged in via `claude login` first
- **Session restart on swap** — OAuth tokens are cached in memory; swapping requires stop + resume
- **Wrapper required for auto-swap** — hooks are no-ops without the wrapper

## License

MIT
