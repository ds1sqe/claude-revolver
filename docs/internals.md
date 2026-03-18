# Internals

## Credential sync strategy

Claude Code caches OAuth tokens in memory. Swapping `.credentials.json` on disk does NOT affect a running session. The new credentials only take effect when a new session starts (or resumes via `--resume`).

This is why the wrapper must kill the child process before swapping:
1. Wrapper detects swap condition (threshold or rate limit)
2. Wrapper kills claude child process
3. `perform_swap()` swaps credentials on disk
4. Wrapper relaunches: `claude --resume <session-id> "Go continue."`
5. Resumed session reads fresh credentials from disk

### Sync on switch

```
switch "work":
  1. save live creds → personal/credentials.json  (outgoing)
  2. copy work/credentials.json → ~/.claude/.credentials.json (incoming)
  3. write "work" to active file
```

### Sync on wrap exit

Always `sync_back()` before evaluating swap. Claude Code may have refreshed the access token during the session.

## Rate limit windows

Two independent usage windows constrain account usage:

| Window | Resets | Nature |
|--------|--------|--------|
| **5-hour** | Rolling, every 5h | Short-term bottleneck |
| **7-day** | Rolling, weekly | Scarce resource — the real constraint |

The strategy system treats these differently:
- **7d is the budget** — strategies optimize around weekly capacity
- **5h is a speed bump** — temporary bottleneck, resets quickly

## Account selection algorithm

### Swap triggers

Wrapper evaluates thresholds after each turn (when it reads the `stopped` signal):

| Trigger | Default | Meaning |
|---------|---------|---------|
| `five_hour` >= threshold | 90% | 5h window nearly exhausted |
| `seven_day` >= threshold | 95% | 7d budget nearly gone |
| Rate limit signal | — | Already rate-limited (immediate kill) |

Thresholds are adjustable from 0 to 100:
- **100%** = only swap when actually rate-limited (reactive)
- **90–95%** = swap preemptively before hitting the wall (default)
- **Lower values** = swap earlier, more conservative

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
2. For each account (skip current, skip token_expired):
   - If seven_day < 95%: use this account
3. If all >= 95%: check five_hour — pick any with five_hour < 90%
4. If all exhausted: don't swap, warn user
```

If no `order` specified, auto-orders by highest current `seven_day` first (drain closest-to-limit account, preserving fresh ones).

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
1. Filter out: current account, token_expired
2. Sort by: seven_day.utilization ascending (lowest first)
3. Tiebreak by: five_hour.utilization ascending
4. Pick first (the freshest account)
```

### Strategy: `manual`

Never auto-swap. Only switch via explicit `claude-revolver switch <name>`.

## Configuration

### Default config

```json
{
  "poll_interval_seconds": 60,
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
| `poll_interval_seconds` | int | 60 | How often systemd timer polls usage API |
| `thresholds.five_hour` | int | 90 | 5h utilization % that triggers swap (0–100) |
| `thresholds.seven_day` | int | 95 | 7d utilization % that triggers swap (0–100) |
| `strategy.type` | string | `drain` | `drain`, `balanced`, or `manual` |
| `strategy.order` | array | `[]` | Priority order for `drain` (empty = auto) |
| `auto_resume` | bool | true | Auto-resume session after swap |
| `auto_message` | string | `Go continue.` | Prompt sent on auto-resume |
| `notify` | bool | true | Desktop notifications via notify-send |

## Session tracking

`sessions.json` maps session IDs to accounts. Written exclusively by the wrapper:
- **Register** on `session-started` signal (during poll loop)
- **Close** (remove) on normal session exit

Sessions older than 7 days are pruned when new sessions are registered.

## Wrapper poll loop detail

The wrapper uses `spawn()` + `try_wait()` polling (not blocking `.status()`):

```rust
loop {
    if child.try_wait()? → exited, break

    // Learn session_id (~1s after session starts)
    if session-started signal exists → read, store, delete

    // Rate limit → immediate kill
    if rate-limited signal exists → kill child, break

    // Turn ended → wrapper evaluates threshold
    if stopped signal exists → {
        read usage-cache
        if over threshold → kill child, break
        // else: let claude continue
    }

    sleep(1s)
}
```

This gives ≤1s latency for signal detection while keeping the design simple.

## Edge cases

| Scenario | Behavior |
|----------|----------|
| Only one account | Skip swap, no swap target available |
| All accounts over threshold | Don't swap, warn user |
| All accounts token_expired | Don't swap, notify user to re-login |
| Claude exits abnormally | Wrapper still syncs + evaluates |
| Bare `claude` (no wrapper) | Hooks are no-ops, no interference |
| Multiple wrappers simultaneously | PID-namespaced signals, zero cross-talk |
| Config file missing | Use defaults |
| Monitor can't reach API | Keep stale cache, log error |
| fzf not installed | Fallback to numbered list |
| Usage cache stale (>1min) | Wrapper uses what's available, monitor refreshes |

## Non-goals

- Desktop/remote/web session support (no hook/wrapper mechanism)
- API key management (different auth path)
- Token refresh/re-login automation (requires browser interaction)
- Encryption at rest beyond file permissions
