# Internals

## Credential sync strategy

Claude Code caches OAuth tokens in memory. Swapping `.credentials.json` on disk does NOT affect a running session. The new credentials only take effect when a new session starts (or resumes via `--resume`).

This is why the flow is:
1. Stop hook swaps credentials on disk
2. Claude stops (outputs "Resume with: claude --resume <id>")
3. Wrapper auto-resumes — new session reads fresh credentials from disk

### Sync on switch

```
switch "work":
  1. cp ~/.claude/.credentials.json → accounts/personal/credentials.json  (save outgoing)
  2. cp accounts/work/credentials.json → ~/.claude/.credentials.json      (load incoming)
  3. echo "work" > active
```

### Sync on wrap exit

Always sync live credentials back before checking swap-info. Claude Code may have refreshed the access token during the session.

### Sync in stop hook

Before swapping, the stop hook saves outgoing credentials. This preserves any token refreshes that happened during the session.

## Rate limit windows

Two independent usage windows constrain account usage:

| Window | Resets | Nature |
|--------|--------|--------|
| **5-hour** | Rolling, every 5h | Short-term bottleneck — wait or temp-swap |
| **7-day** | Rolling, weekly | Scarce resource — the real constraint |

The strategy system treats these differently:
- **7d is the budget** — strategies optimize around weekly capacity
- **5h is a speed bump** — if you hit it, temporarily use another account and come back when it resets

## Account selection algorithm

### Swap triggers

A swap is triggered when the active account crosses a threshold:

| Trigger | Default | Meaning |
|---------|---------|---------|
| `five_hour` >= threshold | 90% | 5h window nearly exhausted |
| `seven_day` >= threshold | 95% | 7d budget nearly gone |

When triggered, the strategy picks which account to swap to.

Thresholds are adjustable from 0 to 100:
- **100%** = only swap when actually rate-limited (reactive). Relies on the `PostToolUseFailure` hook or the API returning a rate limit error. Squeezes every last percent.
- **90–95%** = swap preemptively before hitting the wall (default). Avoids mid-conversation interruptions.
- **Lower values** = swap earlier, more conservative. Useful with `balanced` strategy to keep headroom across accounts.

### 5h vs 7d: which matters more?

Example: Account A has `5h:90% 7d:20%`, Account B has `5h:20% 7d:90%`.

- A's 5h is high but resets in hours. Its 7d has 80% remaining — lots of weekly budget.
- B's 7d is almost gone for the week. Using it now drains the last 10%.

**The right choice depends on the strategy:**
- `drain`: Use B first (finish its 7d, then B is "done" for the week)
- `balanced`: Use A (lower 7d, preserves overall weekly capacity)
- `priority`: Use whichever is higher in the user's order

### Strategy: `drain` (default)

Drain one account at a time. Maximize each account's usage before moving on.

```json
{
  "strategy": {
    "type": "drain",
    "order": ["personal", "work", "client"]
  }
}
```

Algorithm:
```
1. Walk the priority order
2. For each account:
   - Skip if token_expired
   - If seven_day < 95%: use this account
3. If all >= 95%: check five_hour — pick any with five_hour < 90%
4. If all exhausted: don't swap, warn user with reset times
```

Effect with 3 accounts: uses `personal` until its 7d is maxed, then `work`, then `client`.

If no `order` is specified, auto-orders by highest current `seven_day` first (drain the closest-to-limit account, preserving fresh ones for later).

### Strategy: `balanced`

Spread load across all accounts. Keeps weekly budgets similar.

```json
{
  "strategy": {
    "type": "balanced"
  }
}
```

Algorithm:
```
1. Filter out: current account, token_expired, no cached usage
2. Sort by: seven_day.utilization ascending (lowest first)
3. Tiebreak by: five_hour.utilization ascending
4. Pick first (the freshest account)
5. If none available: don't swap
```

### Strategy: `manual`

Never auto-swap. Only switch via explicit `claude-revolver switch <name>`.

```json
{
  "strategy": {
    "type": "manual"
  }
}
```

### 5h temporary swap

Regardless of strategy, a **5h temp-swap** can occur:

If the active account hits the 5h threshold but has plenty of 7d remaining, the system temporarily switches to another account. When the original account's `five_hour.resets_at` passes, the system can swap back.

```
Account A: 5h:92% (resets in 2h), 7d:40%
Account B: 5h:10%, 7d:60%

→ Temp-swap to B
→ When A's 5h resets → swap back to A (still has 7d budget)
```

This is tracked in `swap-info`:
```json
{
  "session_id": "...",
  "from_account": "personal",
  "to_account": "work",
  "reason": "five_hour temp-swap",
  "return_to": "personal",
  "return_after": "2026-03-18T04:00:00Z"
}
```

The Stop hook checks: if `return_to` is set and `return_after` has passed, swap back instead of forward.

## Configuration

### Default config

```json
{
  "poll_interval_seconds": 300,
  "thresholds": {
    "five_hour": 90,
    "seven_day": 95
  },
  "strategy": {
    "type": "drain",
    "order": []
  },
  "auto_resume": true,
  "auto_message": "Go continue.",
  "notify": true
}
```

### Config fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `poll_interval_seconds` | int | 300 | How often systemd timer polls usage API |
| `thresholds.five_hour` | int | 90 | 5h utilization % that triggers swap (0–100) |
| `thresholds.seven_day` | int | 95 | 7d utilization % that triggers swap (0–100) |
| `strategy.type` | string | `drain` | `drain`, `balanced`, or `manual` |
| `strategy.order` | array | `[]` | Priority order for `drain` (empty = auto-order by 7d desc) |
| `auto_resume` | bool | true | Auto-resume session after swap |
| `auto_message` | string | `Go continue.` | Prompt sent on auto-resume |
| `notify` | bool | true | Desktop notifications via notify-send |

## Session tracking

`sessions.json` maps session IDs to accounts. Updated by the SessionStart hook on every session start/resume.

Useful for:
- Knowing which account a session is using (locally visible)
- Auditing account usage per project/session
- The stop hook knowing which account to sync back to

### Cleanup

Sessions older than 7 days are pruned automatically by the monitor.

## Hook interaction model

Three hooks work together:

```
SessionStart  →  records session → account mapping
                 sets env vars if needed

     (claude works)

PostToolUseFailure  →  catches mid-session rate limit errors
                       writes rate-limited flag

     (claude finishes or hits rate limit)

Stop  →  checks usage cache + rate-limited flag
         if over threshold: swap credentials, write swap-info
         always exit 0 (never block claude from stopping)
```

### Why Stop hook exits 0 (never blocks)

Blocking the Stop hook (exit 2) would make Claude continue the session. But credentials are cached in memory — the swapped-on-disk credentials won't take effect until restart. So we must let Claude stop and resume with fresh credentials.

### Loop guard (`stop_hook_active`)

When Claude continues from a Stop hook block, `stop_hook_active: true` is set in the next Stop hook invocation. We check this and skip our logic to prevent:
1. Check usage → swap → block → Claude continues → Stop fires again → swap again → ...

Even though we exit 0, the guard is still checked as a safety measure.

## File permissions

| Path | Mode | Reason |
|------|------|--------|
| `~/.local/share/claude-revolver/` | 0700 | Contains tokens |
| `~/.config/claude-revolver/` | 0755 | Config only, no secrets |
| `*/credentials.json` | 0600 | OAuth tokens |
| `usage-cache.json` | 0600 | Utilization data (private) |
| `sessions.json` | 0600 | Session IDs (private) |
| `swap-info` | 0600 | Contains account names |
| `active` | 0644 | Just a name |

## Edge cases

| Scenario | Behavior |
|----------|----------|
| Only one account | Skip swap, warn in stop hook output |
| All accounts over threshold | Don't swap, log warning, let user decide |
| All accounts token_expired | Don't swap, notify user to re-login |
| Claude exits abnormally (crash/kill) | Wrapper still syncs + checks swap-info |
| Wrapper not used (bare `claude`) | Stop hook still swaps, user sees resume command in output |
| Multiple claude sessions simultaneously | Each gets its own swap-info file (keyed by session_id) |
| `--resume` with no wrapper | Works fine — credentials already swapped by stop hook |
| Config file missing | Use defaults |
| Monitor can't reach API | Keep stale cache, log error, skip notification |
| fzf not installed | Fallback to numbered list with `select` |

## Non-goals

- Desktop/remote/web session support (no hook/wrapper mechanism)
- API key management (different auth path, covered by `apiKeyHelper`)
- Token refresh/re-login automation (requires browser interaction)
- Encryption at rest beyond file permissions
