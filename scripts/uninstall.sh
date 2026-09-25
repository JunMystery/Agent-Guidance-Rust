#!/usr/bin/env bash
set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; PURPLE='\033[0;35m'; BOLD='\033[1m'
GRAY='\033[0;90m'; NC='\033[0m'

echo -e ""
echo -e "${RED}${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${RED}${BOLD}║           Agent Guidance Rust (macOS/Linux)                  ║${NC}"
echo -e "${RED}${BOLD}║                   Uninstaller                                ║${NC}"
echo -e "${RED}${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
echo -e ""

if [ "$(uname -s)" = "Linux" ] && command -v systemctl &>/dev/null; then
    systemctl --user stop agent-guidance 2>/dev/null || true
    systemctl --user disable agent-guidance 2>/dev/null || true
    rm -f "$HOME/.config/systemd/user/agent-guidance.service" 2>/dev/null || true
elif [ "$(uname -s)" = "Darwin" ]; then
    plist="$HOME/Library/LaunchAgents/com.junmystery.agent-guidance.plist"
    launchctl unload "$plist" 2>/dev/null || true
    rm -f "$plist" 2>/dev/null || true
fi

killall agent-guidance agent-guidance-mcp &>/dev/null || true
pkill -f agent-guidance &>/dev/null || true

if command -v uv &> /dev/null; then
    uv tool uninstall agent-guidance-mcp 2>/dev/null || true
fi

if [ -d "$HOME/.agent-guidance" ]; then
    rm -rf "$HOME/.agent-guidance"
    echo -e "  ${GREEN}✓${NC} Completely removed directory ${GRAY}$HOME/.agent-guidance${NC}"
fi

rm -f "$HOME/.local/bin/agent-guidance" "$HOME/.local/bin/agent-guidance-mcp" 2>/dev/null || true
rm -f "$HOME/.cargo/bin/agent-guidance" 2>/dev/null || true

echo -e ""
echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}${BOLD}║       ✓  Complete uninstallation finished!                  ║${NC}"
echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
echo -e ""
