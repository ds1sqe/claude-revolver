#!/usr/bin/env bash
set -euo pipefail

# claude-revolver installer

readonly SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
readonly BIN_DIR="$HOME/.local/bin"
readonly DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/claude-revolver"
readonly UNIT_DIR="$HOME/.config/systemd/user"

die()  { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }
info() { printf '\033[36m::\033[0m %s\n' "$*"; }
warn() { printf '\033[33mwarn:\033[0m %s\n' "$*"; }

# Check dependencies
info "checking dependencies..."
missing=()
command -v jq   &>/dev/null || missing+=(jq)
command -v curl &>/dev/null || missing+=(curl)
command -v bash &>/dev/null || missing+=(bash)
if ((${#missing[@]})); then
    die "missing required: ${missing[*]} — install with your package manager"
fi
command -v fzf &>/dev/null || warn "fzf not found — TUI picker will use fallback mode"
command -v notify-send &>/dev/null || warn "notify-send not found — desktop notifications disabled"

# Create data directory
info "creating data directory..."
mkdir -p "$DATA_DIR"
chmod 700 "$DATA_DIR"

# Install scripts
info "installing scripts to $BIN_DIR..."
mkdir -p "$BIN_DIR"

cp "$SCRIPT_DIR/claude-revolver" "$BIN_DIR/claude-revolver"
chmod 755 "$BIN_DIR/claude-revolver"

cp "$SCRIPT_DIR/claude-revolver-monitor" "$BIN_DIR/claude-revolver-monitor"
chmod 755 "$BIN_DIR/claude-revolver-monitor"

# Copy hook script to data dir
cp "$SCRIPT_DIR/rate-limit-hook.sh" "$DATA_DIR/rate-limit-hook.sh"
chmod 755 "$DATA_DIR/rate-limit-hook.sh"

# Check PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    warn "$BIN_DIR is not in PATH — add it to your shell config"
fi

# Install systemd units
if command -v systemctl &>/dev/null; then
    printf '\n'
    read -rp "Install systemd timer for background usage monitoring? [Y/n] " reply
    case "${reply:-Y}" in
        [Yy]*)
            info "installing systemd units..."
            mkdir -p "$UNIT_DIR"
            cp "$SCRIPT_DIR/systemd/claude-revolver-monitor.service" "$UNIT_DIR/"
            cp "$SCRIPT_DIR/systemd/claude-revolver-monitor.timer" "$UNIT_DIR/"
            systemctl --user daemon-reload
            systemctl --user enable --now claude-revolver-monitor.timer
            info "systemd timer enabled (polls every 15 min)"
            ;;
        *)
            info "skipped systemd setup"
            ;;
    esac
else
    warn "systemctl not found — skipping systemd setup"
fi

# Install hook
if [[ -f "$HOME/.claude/settings.json" ]]; then
    printf '\n'
    read -rp "Install rate-limit detection hook in Claude Code? [Y/n] " reply
    case "${reply:-Y}" in
        [Yy]*)
            "$BIN_DIR/claude-revolver" install-hook
            ;;
        *)
            info "skipped hook setup"
            ;;
    esac
else
    warn "~/.claude/settings.json not found — skipping hook setup"
fi

printf '\n'
info "installed claude-revolver $("$BIN_DIR/claude-revolver" version)"
printf '\n'
echo "Next steps:"
echo "  1. claude-revolver add <name>     # save current account"
echo "  2. claude logout && claude login   # login to another account"
echo "  3. claude-revolver add <name>      # save that one too"
echo "  4. claude-revolver                 # TUI picker to switch"
echo ""
echo "Optional: alias claude='claude-revolver wrap --'"
