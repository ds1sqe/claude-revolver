# claude-revolver

Multi-account OAuth credential manager for Claude Code CLI. Automatically switches accounts when you hit rate limits.

## Why

Claude Max has rolling usage caps: a 5-hour window and a 7-day ceiling. If you have multiple accounts (personal, work, etc.), hitting a limit means manual logout/login. There's no built-in way to detect approaching limits or swap automatically.

claude-revolver solves this by storing named credential sets, monitoring usage via the OAuth API, and swapping accounts transparently when thresholds are reached.

## How it works

```
claude session running
  └─ Stop hook fires → checks usage cache → account at 96%
      └─ selects next account (drain/balanced strategy)
          └─ swaps credentials on disk → writes swap-info
              └─ wrapper detects swap → claude --resume <session-id> "Go continue."
                  └─ session continues on new account
```

1. **Store** named copies of `~/.claude/.credentials.json`
2. **Monitor** usage via OAuth API (`GET /api/oauth/usage`) on a systemd timer
3. **Detect** when thresholds are reached (Stop hook checks cached usage)
4. **Swap** credentials and auto-resume the session with the new account
5. **Log** every swap event with timestamp, trigger, reason, and session context

## Install


### install from git repo
```bash
git clone https://github.com/ds1sqe/claude-revolver
cd claude-revolver
cargo install --path .
```

### install from cargo
```bash
cargo install claude-revolver

Installs to `~/.cargo/bin/`. Then set up hooks and optionally the systemd timer:

```bash
claude-revolver install hook
claude-revolver install systemd   # optional: background usage polling
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

When usage hits the threshold, the Stop hook swaps credentials and the wrapper auto-resumes. Seamless continuation across accounts.

### Without the wrapper

Even without the wrapper, the Stop hook still swaps credentials on disk. Claude prints `Resume with: claude --resume <id>` — just run it and the new account is already active.

## Configuration

```bash
claude-revolver config show
claude-revolver config set thresholds.seven_day 80
claude-revolver config set strategy.type balanced
```

Config file: `~/.config/claude-revolver/config.json`

```json
{
  "poll_interval_seconds": 300,
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
| `poll_interval_seconds` | 300 | Usage API poll interval (for systemd timer) |

Setting thresholds to `100` means reactive-only: swap only when actually rate-limited, not preemptively.

### Strategies

- **drain** — Use one account until its 7-day limit is maxed, then move to the next. Set `order` to control priority, or leave empty to auto-drain highest-7d first.
- **balanced** — Spread load evenly. Always picks the account with lowest 7-day utilization.
- **manual** — No automatic swapping. Only switch with `claude-revolver switch`.

### 5-hour temp-swap

If the active account hits the 5-hour limit but still has 7-day budget, the system temporarily switches to another account and **returns automatically** when the 5-hour window resets. This avoids wasting 7-day capacity on a short-term bottleneck.

## Commands

| Command | Description |
|---------|-------------|
| *(no args)* | TUI account picker (fzf or numbered list) |
| `add <name>` | Save current credentials as a named account |
| `remove <name>` | Remove an account (alias: `rm`) |
| `list` | List accounts with usage (alias: `ls`) |
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
| `install hook` | Install Claude Code hooks (idempotent, won't duplicate) |
| `install systemd` | Install systemd user timer for background monitoring |
| `uninstall hook` | Remove Claude Code hooks (preserves other hooks) |
| `uninstall systemd` | Remove systemd units |

## Swap history

Every account swap is logged persistently:

```bash
$ claude-revolver history
  2026-03-18T06:12:33  personal (5h:50% 7d:96%) → work (5h:10% 7d:30%)  [stop-hook]
    reason: seven_day utilization 96% >= threshold 95%
    session: 143eec0f-277e-4ce1-95f1-58eb56331874
    cwd: /home/user/project

  2026-03-18T05:00:12  work → personal  [manual]
    reason: manual switch
    cwd: /home/user
```

Each entry records: timestamp, trigger source (`stop-hook`, `wrap-precheck`, `wrap-rate-limit`, `manual`), from/to accounts with usage at swap time, reason, session ID, working directory, and whether it was a temporary 5h swap.

History is capped at 1000 entries.

## Data layout

```
~/.local/share/claude-revolver/
├── active                    # Current account name
├── usage-cache.json          # Cached usage for all accounts
├── sessions.json             # Session ID → account mapping
├── swap-history.json         # Persistent swap log
├── swap-info                 # Transient: current swap event (consumed by wrapper)
├── rate-limited              # Transient: rate-limit flag (set by hook)
└── <account-name>/
    └── credentials.json      # Stored OAuth credentials

~/.config/claude-revolver/
└── config.json               # User configuration
```

## Limitations

- **CLI terminal only** — Claude Desktop and web have no hook mechanism
- **Requires manual login** — each account must be logged in via `claude login` first
- **Session restart on swap** — OAuth tokens are cached in memory; swapping requires stop + resume

## Docs

- [Architecture](docs/architecture.md) — system diagram, hook flow, data layout
- [Internals](docs/internals.md) — selection algorithms, credential sync, edge cases

## License

MIT
