# Architecture

## System overview

```
┌────────────────────────────┐
│ Config                     │
│ ~/.config/claude-revolver/ │
│   config.json              │
│   - poll_interval          │
│   - thresholds             │
│   - strategy               │
│   - auto_resume            │
│   - auto_message           │
└────────────────────────────┘
        │ read by all components
        ▼
┌────────────────────────────┐      ┌────────────────────────────┐
│ systemd user timer         │      │ SessionStart hook          │
│ (configurable interval)    │      │                            │
│                            │      │ records session → account  │
│ polls /api/oauth/usage     │      │ writes to CLAUDE_ENV_FILE  │
│ for ALL stored accounts    │      │                            │
│ writes usage-cache.json    │      └────────────────────────────┘
│ notify-send on threshold   │
└────────────┬───────────────┘
             │ writes                ┌────────────────────────────┐
             ▼                       │ Stop hook                  │
┌────────────────────────────┐       │                            │
│ ~/.local/share/            │◀───── │ reads session_id from stdin│
│   claude-revolver/         │       │ checks usage-cache         │
│                            │       │ if over threshold:         │
│ usage-cache.json           │       │   pick next account        │
│ sessions.json              │       │   swap credentials         │
│ active                     │       │   write swap-info          │
│ swap-info (transient)      │       │ exit 0 (let claude stop)   │
│ personal/credentials.json  │       └────────────────────────────┘
│ work/credentials.json      │
└────────────┬───────────────┘
             │                       ┌─────────────────────────────┐
             │ swaps into            │ Wrapper                     │
             ▼                       │ (claude-revolver wrap)      │
┌────────────────────────────┐       │                             │
│ ~/.claude/                 │       │ 1. pre-check usage          │
│   .credentials.json        │       │ 2. launch claude            │
│   (single active session)  │       │ 3. on exit:                 │
└────────────────────────────┘       │    read swap-info           │
             ▲                       │    if swapped + auto_resume │
             │ resumed with          │    → claude --resume <id>   │
             │ new credentials       │      "Go continue."         │
             │                       │ 4. loop until no swap       │
             └───────────────────────┘
```

## Hook flow

### Normal stop (usage OK)

```
claude running → Stop hook fires
                   → check usage: 45% (under threshold)
                   → exit 0
                 claude stops normally
wrapper sees exit, no swap-info → done
```

### Auto-swap stop (usage high)

```
claude running → Stop hook fires
                   → check usage: 92% (over threshold)
                   → pick "work" (lowest usage)
                   → save outgoing creds
                   → copy work creds → ~/.claude/.credentials.json
                   → write swap-info: {session_id, from: "personal", to: "work"}
                   → exit 0
                 claude stops, prints "Resume with: claude --resume <id>"
wrapper reads swap-info
  → claude --resume <id> "Go continue."
  → new session loads swapped credentials from disk
  SessionStart hook fires
    → records session <id> → account "work"
  claude resumes working
```

### Loop guard

The Stop hook checks `stop_hook_active` from stdin. If `true`, it means Claude is already continuing from a previous Stop hook action — skip the usage check to prevent infinite loops.

## Data layout

```
~/.local/share/claude-revolver/
├── active                          # plain text: current account name
├── usage-cache.json                # cached usage per account (from monitor)
├── sessions.json                   # session_id → account mapping
├── swap-info                       # transient: written by Stop hook, read by wrapper
├── rate-limit-hook.sh              # installed PostToolUseFailure hook
├── stop-hook.sh                    # installed Stop hook
├── session-start-hook.sh           # installed SessionStart hook
├── personal/
│   └── credentials.json            # stored OAuth credentials (mode 0600)
└── work/
    └── credentials.json
```

### Config: `~/.config/claude-revolver/config.json`

```json
{
  "poll_interval_seconds": 300,
  "thresholds": {
    "five_hour": 80,
    "seven_day": 90
  },
  "strategy": "least-used",
  "auto_resume": true,
  "auto_message": "Go continue.",
  "notify": true
}
```

#### Strategy options

| Strategy | Behavior |
|----------|----------|
| `least-used` | Pick account with lowest `seven_day.utilization` (default) |
| `round-robin` | Cycle through accounts in stored order |
| `manual` | Only swap when user explicitly runs `switch` |

### usage-cache.json

Written by monitor, read by hooks and wrapper.

```json
{
  "personal": {
    "five_hour": { "utilization": 20.0, "resets_at": "2026-03-18T02:00:00Z" },
    "seven_day": { "utilization": 73.0, "resets_at": "2026-03-20T04:00:00Z" },
    "seven_day_sonnet": { "utilization": 7.0, "resets_at": "2026-03-20T04:00:00Z" },
    "polled_at": "2026-03-17T23:45:00Z",
    "token_expired": false
  },
  "work": {
    "five_hour": { "utilization": 5.0, "resets_at": "2026-03-18T01:00:00Z" },
    "seven_day": { "utilization": 30.0, "resets_at": "2026-03-21T00:00:00Z" },
    "polled_at": "2026-03-17T23:45:00Z",
    "token_expired": false
  }
}
```

### sessions.json

Session-to-account tracking. Written by SessionStart hook.

```json
{
  "143eec0f-277e-4ce1-95f1-58eb56331874": {
    "account": "personal",
    "started_at": "2026-03-18T01:00:00Z",
    "source": "startup",
    "cwd": "/home/ds1sqe/proj/reovim"
  },
  "a8b3c2d1-e4f5-6789-abcd-ef0123456789": {
    "account": "work",
    "started_at": "2026-03-18T02:30:00Z",
    "source": "resume",
    "cwd": "/home/ds1sqe/proj/reovim"
  }
}
```

### swap-info (transient)

Written by Stop hook when a swap happens. Read and deleted by wrapper.

```json
{
  "session_id": "143eec0f-277e-4ce1-95f1-58eb56331874",
  "from_account": "personal",
  "to_account": "work",
  "reason": "seven_day utilization 92% > threshold 90%",
  "swapped_at": "2026-03-18T01:45:00Z"
}
```

### credentials.json (what Claude Code stores)

```json
{
  "claudeAiOauth": {
    "accessToken": "sk-ant-oat01-...",
    "refreshToken": "sk-ant-ort01-...",
    "expiresAt": 1773812455580,
    "scopes": ["user:file_upload", "user:inference", "user:mcp_servers", "user:profile", "user:sessions:claude_code"],
    "subscriptionType": "max",
    "rateLimitTier": "default_claude_max_20x"
  }
}
```

## Components

### 1. `claude-revolver` — CLI/TUI

Single bash script. All account management + wrapper.

**Account management**: `add`, `remove`, `list`, `switch`, `status`, `sync`

**TUI picker** (no args, via fzf) with usage preview.

**Wrapper** (`claude-revolver wrap [-- args...]`):
1. Pre-check usage cache, auto-swap if over threshold
2. Launch `claude "$@"`
3. On exit: sync credentials, check `swap-info`
4. If swap happened and `auto_resume: true`:
   - `claude --resume <session_id> "<auto_message>"`
   - Loop
5. If swap happened and `auto_resume: false`:
   - Print: "Switched to 'work'. Resume: claude --resume <id>"

**Config management**: `config show`, `config set <key> <value>`

**Session tracking**: `sessions` — show session→account mapping

### 2. Stop hook (`stop-hook.sh`)

Fired when Claude finishes a turn or session.

```
stdin → parse session_id, stop_hook_active
  → if stop_hook_active: exit 0 (loop guard)
  → read usage-cache for active account
  → if under threshold: exit 0
  → pick next account (per strategy)
  → sync outgoing credentials
  → swap incoming credentials
  → write swap-info
  → exit 0 (let claude stop)
```

### 3. SessionStart hook (`session-start-hook.sh`)

Fired on session start or resume.

```
stdin → parse session_id, source
  → read active account
  → write to sessions.json: session_id → {account, started_at, source, cwd}
  → optionally set env vars via CLAUDE_ENV_FILE
  → exit 0
```

### 4. PostToolUseFailure hook (`rate-limit-hook.sh`)

Catches mid-session rate limits. Writes `rate-limited` flag for the wrapper.

### 5. `claude-revolver-monitor` — systemd helper

Polls usage API for all accounts at configurable interval. Writes `usage-cache.json`. Sends desktop notifications at thresholds.

### 6. systemd units

- `claude-revolver-monitor.service` — oneshot, runs monitor
- `claude-revolver-monitor.timer` — configurable interval (default 5 min)

## Verified assumptions

- `~/.claude/.credentials.json` holds a single OAuth session (`claudeAiOauth` key)
- Swapping `.credentials.json` on disk + restarting/resuming picks up new credentials
- `claude --resume <id>` works across credential changes
- `claude --resume <id> "message"` passes an initial prompt
- Stop hook receives `session_id` and `stop_hook_active` in stdin JSON
- SessionStart hook receives `session_id`, `source`, and can write to `CLAUDE_ENV_FILE`
- OAuth usage API: `GET https://api.anthropic.com/api/oauth/usage` with Bearer token
- `PostToolUseFailure` hook receives error type for rate limit detection
