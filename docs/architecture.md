# Architecture

## System overview

```
                     ┌────────────────────────────┐
                     │   systemd user timer       │
                     │   (every 15 min)           │
                     │                            │
                     │  polls /api/oauth/usage    │
                     │  for ALL stored accounts   │
                     │  writes usage-cache.json   │
                     │  notify-send on threshold  │
                     └────────────┬───────────────┘
                                  │ writes
                                  ▼
┌────────────────┐    ┌────────────────────────────┐    ┌─────────────────────┐
│ claude-revolver│───▶│ ~/.local/share/            │◀───│ rate-limit-hook.sh  │
│ wrap           │    │   claude-revolver/         │    │ (PostToolUseFailure)│
│                │    │                            │    │                     │
│ pre-check   ───┼───▶│ usage-cache.json           │    │ detects rate_limit  │
│ launch claude  │    │ active                     │    │ writes rate-limited │
│ post-check   ──┼───▶│ rate-limited (flag)        │◀───│ flag file           │
│ auto-swap      │    │ personal/credentials.json  │    └─────────────────────┘
│ restart?       │    │ work/credentials.json      │
└────────────────┘    └────────────────────────────┘
                                  │ swaps into
                                  ▼
                     ┌────────────────────────────┐
                     │ ~/.claude/                 │
                     │   .credentials.json        │
                     │   (single active session)  │
                     └────────────────────────────┘
```

## Data layout

```
~/.local/share/claude-revolver/
├── active                          # plain text: current account name
├── usage-cache.json                # cached usage data for all accounts
├── rate-limited                    # flag file (presence = rate limited)
├── rate-limit-hook.sh              # installed hook script
├── personal/
│   └── credentials.json            # copy of .credentials.json (mode 0600)
└── work/
    └── credentials.json
```

### usage-cache.json

Written by `claude-revolver-monitor`, read by `list` and `wrap`.

```json
{
  "personal": {
    "five_hour": { "utilization": 20.0, "resets_at": "2026-03-18T02:00:00Z" },
    "seven_day": { "utilization": 73.0, "resets_at": "2026-03-20T04:00:00Z" },
    "seven_day_sonnet": { "utilization": 7.0, "resets_at": "2026-03-20T04:00:00Z" },
    "polled_at": "2026-03-17T23:45:00Z"
  },
  "work": {
    "five_hour": { "utilization": 5.0, "resets_at": "2026-03-18T01:00:00Z" },
    "seven_day": { "utilization": 30.0, "resets_at": "2026-03-21T00:00:00Z" },
    "seven_day_sonnet": null,
    "polled_at": "2026-03-17T23:45:00Z"
  }
}
```

### credentials.json

Each stored account holds a copy of what Claude Code writes to `~/.claude/.credentials.json`:

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

Single bash script. Entry point for all user interaction.

**TUI picker** (no args, via fzf):

```
  Switch account > _
  Current: personal | 5h: 20%  7d: 73%
  ───────────────────────────────────────
  > personal   max   5h:20%  7d:73%  *
    work       max   5h: 5%  7d:30%
```

**Wrapper mode** (`claude-revolver wrap [-- args...]`):

1. Read `usage-cache.json`
2. If active account `five_hour > 90` or `seven_day > 95` — auto-switch to least-used account
3. Clear `rate-limited` flag
4. Launch `claude` with all args
5. On exit: sync credentials back, check rate-limited flag
6. If flagged: switch to next account, prompt restart

### 2. `rate-limit-hook.sh` — PostToolUseFailure hook

Reads hook input JSON from stdin. If error type/message contains "rate_limit" or "Rate limit", writes timestamp to `rate-limited` flag file. Always exits 0 (never blocks Claude Code).

Registered in `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PostToolUseFailure": [{
      "matcher": ".*",
      "hooks": [{
        "type": "command",
        "command": "bash ~/.local/share/claude-revolver/rate-limit-hook.sh",
        "timeout": 5
      }]
    }]
  }
}
```

### 3. `claude-revolver-monitor` — systemd helper

Oneshot script run by systemd timer every 15 minutes.

1. For each stored account: extract token, call `GET /api/oauth/usage`
2. On success: update usage cache
3. On 401: mark `token_expired` in cache, notify user
4. Check thresholds: notify at 80% (5h), 90%/95% (7d)
5. Write `usage-cache.json` atomically (tmp + mv)

### 4. systemd units

- `claude-revolver-monitor.service` — oneshot, runs the monitor script
- `claude-revolver-monitor.timer` — fires every 15 minutes, persistent across reboots

## Verified assumptions

- `~/.claude/.credentials.json` holds a single OAuth session (`claudeAiOauth` key)
- `CLAUDE_CONFIG_DIR` changes the entire config dir (too broad — loses shared settings/history)
- Swapping `.credentials.json` content is the right granularity
- OAuth usage API: `GET https://api.anthropic.com/api/oauth/usage` with Bearer token returns `five_hour.utilization`, `seven_day.utilization`, per-model breakdowns, and `resets_at` timestamps
- `PostToolUseFailure` hook receives error type — can detect `rate_limit_error`
- Claude Code refreshes tokens automatically — stored refresh tokens have indefinite lifetime
