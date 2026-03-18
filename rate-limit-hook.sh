#!/usr/bin/env bash
# claude-revolver rate limit detection hook
# Registered as PostToolUseFailure hook in Claude Code settings
# Reads hook input JSON from stdin, writes flag file on rate limit detection

INPUT=$(cat)
ERROR_TYPE=$(echo "$INPUT" | jq -r '.error.type // ""' 2>/dev/null)
ERROR_MSG=$(echo "$INPUT" | jq -r '.error.message // ""' 2>/dev/null)

if [[ "$ERROR_TYPE" == *"rate_limit"* ]] || \
   [[ "$ERROR_MSG" == *"rate limit"* ]] || \
   [[ "$ERROR_MSG" == *"Rate limit"* ]] || \
   [[ "$ERROR_MSG" == *"usage limit"* ]]; then
    DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/claude-revolver"
    date -Iseconds > "$DATA_DIR/rate-limited" 2>/dev/null || true
fi

exit 0  # never block claude
