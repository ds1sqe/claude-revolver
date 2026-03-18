# Architecture

## System overview

```
┌────────────┐  signals/{pid}-*  ┌──────────────────────┐
│   Hooks    │──────────────────▶│  Wrapper (the brain) │
│ (dumb      │  session-started  │                      │
│  reporters)│  stopped          │  poll loop:          │
│            │  rate-limited     │  - learn session_id  │
└────────────┘                   │  - evaluate usage    │
                                 │  - kill if needed    │
                                 │  - swap + resume     │
                                 └──────────┬───────────┘
                                            │
                                      perform_swap()
                                            │
                               ┌────────────┼────────────┐
                               ▼            ▼            ▼
                          active     credentials   history

┌────────────┐
│  Monitor   │──── writes ──→ usage-cache.json (only)
│ (systemd)  │
│ (1 min)    │
└────────────┘

┌────────────┐
│  CLI       │──── perform_swap() ──→ active, credentials, history
│ (manual)   │
└────────────┘
```

## Ownership model

Two owners with non-overlapping lifetimes. Like Rust's borrow checker.

| Owner | Writes | Role |
|---|---|---|
| **Wrapper** | `active`, `credentials`, `history`, `sessions` | Single brain for automated swaps |
| **CLI** | `active`, `credentials`, `history` | Manual user commands (switch, add, remove) |
| **Hooks** | `signals/{pid}-*` only | Dumb reporters, gated by env var |
| **Monitor** | `usage-cache` only | Data collector, no decisions |

## PID-namespaced signals

Multiple wrappers can run in parallel (different terminals). Each wrapper
passes its PID to hooks via env var. Hooks namespace all signal files.

```
Wrapper A (PID 1234)                    Wrapper B (PID 5678)
  env: WRAPPER_PID=1234                  env: WRAPPER_PID=5678

  hooks write:                           hooks write:
    signals/1234-session-started           signals/5678-session-started
    signals/1234-stopped                   signals/5678-stopped
    signals/1234-rate-limited              signals/5678-rate-limited

  wrapper reads:                         wrapper reads:
    signals/1234-*                         signals/5678-*
```

Zero cross-talk. Each wrapper only reads its own signals.

## Hook flow

Three hooks, all gated by `CLAUDE_REVOLVER_WRAPPED` env var.
All signal filenames namespaced by `CLAUDE_REVOLVER_WRAPPER_PID`.
Hooks never evaluate thresholds, read config, or mutate state.

### Three dumb signals

| Signal | Hook | Content | Meaning |
|---|---|---|---|
| `{pid}-session-started` | SessionStart | `{ session_id, cwd, source }` | "A session just started" |
| `{pid}-stopped` | Stop | `{ session_id }` | "A turn just ended" |
| `{pid}-rate-limited` | RateLimit | `{ timestamp }` | "A rate limit error occurred" |

### Without wrapper

If `CLAUDE_REVOLVER_WRAPPED` is not set, all hooks are no-ops.
No signals written, no interference with bare `claude` sessions.

## Wrapper flow

```
┌─ Wrapper ────────────────────────────────────────────────────┐
│                                                              │
│  state = { wrapper_pid, active, args, session_id, config }   │
│                                                              │
│  pre-check: if active over threshold → swap before launch    │
│                                                              │
│  ┌─loop──────────────────────────────────────────────────┐   │
│  │                                                       │   │
│  │  1. clear signals/{pid}-*                             │   │
│  │                                                       │   │
│  │  2. spawn claude                                      │   │
│  │     env: CLAUDE_REVOLVER_WRAPPED=1                    │   │
│  │          CLAUDE_REVOLVER_WRAPPER_PID={pid}            │   │
│  │     args: state.args                                  │   │
│  │                                                       │   │
│  │  3. poll loop (1s):                                   │   │
│  │     - try_wait: child exited? → break                 │   │
│  │     - session-started signal? → learn session_id      │   │
│  │     - rate-limited signal?    → kill child, break     │   │
│  │     - stopped signal?         → evaluate usage cache  │   │
│  │                                  over? → kill, break  │   │
│  │                                  under? → continue    │   │
│  │                                                       │   │
│  │  4. post-exit:                                        │   │
│  │     sync_back() (save refreshed token)                │   │
│  │     if killed_for_swap:                               │   │
│  │       select next account (strategy)                  │   │
│  │       perform_swap(active → next)                     │   │
│  │       args = ["--resume", sid, auto_message]          │   │
│  │       continue loop                                   │   │
│  │     else:                                             │   │
│  │       close session, clean signals, exit              │   │
│  │                                                       │   │
│  └───────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

## perform_swap(): single path

One function for all credential swaps. Called from exactly 2 places:

1. **Wrapper** — after killing child for threshold/rate-limit
2. **CLI `switch`** — manual user command

```rust
perform_swap(from, to, reason, trigger, session_id, cwd, temp_swap)
  → account::swap_credentials(from, to)
  → history::log_swap(...)
  → notify(from, to, reason)
```

## Data layout

```
~/.local/share/claude-revolver/
├── active                    # Current account name
├── usage-cache.json          # Monitor writes, wrapper reads
├── sessions.json             # Wrapper writes (register/close)
├── swap-history.json         # perform_swap() writes
├── signals/                  # PID-namespaced, transient
│   ├── {pid}-session-started
│   ├── {pid}-stopped
│   └── {pid}-rate-limited
└── <account-name>/
    └── credentials.json      # Stored OAuth credentials (mode 0600)

~/.config/claude-revolver/
└── config.json               # User configuration
```

### credentials.json (what Claude Code stores)

```json
{
  "claudeAiOauth": {
    "accessToken": "sk-ant-oat01-...",
    "refreshToken": "sk-ant-ort01-...",
    "expiresAt": 1773812455580,
    "scopes": ["user:inference", "user:sessions:claude_code"],
    "subscriptionType": "max",
    "rateLimitTier": "default_claude_max_20x"
  }
}
```

### usage-cache.json

Written by monitor (1-min timer), read by wrapper poll loop.

```json
{
  "personal": {
    "five_hour": { "utilization": 20.0, "resets_at": "2026-03-18T02:00:00Z" },
    "seven_day": { "utilization": 73.0, "resets_at": "2026-03-20T04:00:00Z" },
    "polled_at": "2026-03-17T23:45:00Z",
    "token_expired": false
  }
}
```

## File permissions

| Path | Mode | Reason |
|------|------|--------|
| `~/.local/share/claude-revolver/` | 0700 | Contains tokens |
| `~/.config/claude-revolver/` | 0755 | Config only, no secrets |
| `*/credentials.json` | 0600 | OAuth tokens |
| `usage-cache.json` | 0600 | Utilization data |
| `sessions.json` | 0600 | Session IDs |
| `signals/` | 0700 | Transient signals |
| `active` | 0644 | Just a name |

## Verified assumptions

- `~/.claude/.credentials.json` holds a single OAuth session (`claudeAiOauth` key)
- Swapping `.credentials.json` on disk + restarting/resuming picks up new credentials
- `claude --resume <id>` works across credential changes
- `claude --resume <id> "message"` passes an initial prompt
- Stop hook receives `session_id` in stdin JSON
- SessionStart hook receives `session_id`, `source`, `cwd`
- OAuth usage API: `GET https://api.anthropic.com/api/oauth/usage` with Bearer token
- `PostToolUseFailure` hook receives error type for rate limit detection
