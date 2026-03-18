# Internals

## Credential sync strategy

Claude Code auto-refreshes OAuth tokens (access tokens expire ~8h, refresh tokens are indefinite). When we swap credentials in, Claude Code uses the access token and refreshes it as needed, writing the refreshed token back to `~/.claude/.credentials.json`.

**Critical**: on switch, we must save the current live `.credentials.json` back to the outgoing account's store, or we lose refreshed tokens.

```
switch "work":
  1. cp ~/.claude/.credentials.json → accounts/personal/credentials.json  (save outgoing)
  2. cp accounts/work/credentials.json → ~/.claude/.credentials.json      (load incoming)
  3. echo "work" > active
```

On `wrap` exit (even without rate limit): always sync live credentials back to active account store.

## Account selection algorithm

When auto-swapping (in `wrap` mode), pick the best available account:

```
1. Filter out: current account, token_expired accounts, accounts with no cached usage
2. Sort by: seven_day.utilization ascending (prefer least-used weekly)
3. Tiebreak by: five_hour.utilization ascending
4. Pick first
5. If none available: warn and don't swap
```

## File permissions

| Path | Mode | Reason |
|------|------|--------|
| `~/.local/share/claude-revolver/` | 0700 | Contains tokens |
| `*/credentials.json` | 0600 | OAuth tokens |
| `usage-cache.json` | 0600 | Utilization data (private) |
| `rate-limited` | 0644 | Just a flag |
| `active` | 0644 | Just a name |

## Edge cases

| Scenario | Behavior |
|----------|----------|
| Only one account stored | Wrap mode skips auto-swap, just warns |
| All accounts rate-limited | Warn user, show reset times, don't loop |
| Token expired (401 from usage API) | Mark in cache, skip for auto-swap, notify to re-login |
| Claude Code running when switch happens externally | Won't take effect until restart — wrapper handles this |
| Credentials file doesn't exist yet | `add` fails with "run `claude login` first" |
| Monitor can't reach API | Skip update, keep stale cache, log error |
| fzf not installed | Fallback to numbered list with `select` |
| Hook not installed | Wrap mode still works (pre-check + post-exit sync), just no in-session detection |

## Non-goals

- Desktop/remote/web session support (OAuth-only, no hook/wrapper mechanism)
- API key management (different auth path, covered by `apiKeyHelper`)
- Token refresh/re-login automation (requires browser interaction)
- Per-project account pinning (could be added later via `.claude/account` file)
- Encryption at rest beyond file permissions (OS-level encryption is user's choice)
