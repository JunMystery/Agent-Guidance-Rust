#!/usr/bin/env bash
set -e

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; PURPLE='\033[0;35m'; BOLD='\033[1m'
GRAY='\033[0;90m'; NC='\033[0m'

# ── Header ────────────────────────────────────────────────────────────────────
echo -e ""
echo -e "${PURPLE}${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${PURPLE}${BOLD}║           Agent Guidance Rust (macOS/Linux)                  ║${NC}"
echo -e "${PURPLE}${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
echo -e ""

# ── Choose mode FIRST ─────────────────────────────────────────────────────────
echo -e "${BOLD}What would you like to do?${NC}"
echo -e "  ${GREEN}[1]${NC} Install Rust Edition (removes Python edition & builds Rust server)"
echo -e "  ${RED}[2]${NC} Uninstall — remove entire Agent Guidance directory & toolchains"
echo -e ""

if [ -c /dev/tty ]; then
    read -p "Choice [1]: " ACTION < /dev/tty || ACTION=""
elif [ -t 0 ]; then
    read -p "Choice [1]: " ACTION || ACTION=""
else
    # Non-interactive fallback — default to action 1
    ACTION="1"
fi
ACTION="${ACTION:-1}"

# ── Uninstall path ────────────────────────────────────────────────────────────
if [ "$ACTION" = "2" ]; then
    echo -e ""
    echo -e "${RED}${BOLD}🗑️  Completely uninstalling Agent Guidance...${NC}"
    echo -e ""

    # Stop any running processes
    killall agent-guidance agent-guidance-mcp &>/dev/null || true
    pkill -f agent-guidance &>/dev/null || true

    # Remove Python uv tool registration if present
    if command -v uv &> /dev/null; then
        uv tool uninstall agent-guidance-mcp 2>/dev/null || true
    fi

    # Completely remove configuration and data directory
    if [ -d "$HOME/.agent-guidance" ]; then
        rm -rf "$HOME/.agent-guidance"
        echo -e "  ${GREEN}✓${NC} Completely removed directory ${GRAY}$HOME/.agent-guidance${NC}"
    fi

    # Remove binary symlinks if present
    rm -f "$HOME/.local/bin/agent-guidance" "$HOME/.local/bin/agent-guidance-mcp" 2>/dev/null || true

    echo -e ""
    echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}${BOLD}║       ✓  Complete uninstallation finished!                  ║${NC}"
    echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo -e ""
    exit 0
fi

# ── Install Path: Enforce Exclusive Rust Edition ──────────────────────────────
echo -e ""
echo -e "${YELLOW}⚡ Enforcing exclusive edition: Removing Python runtime & old installs...${NC}"

# Kill old running servers
killall agent-guidance-mcp &>/dev/null || true
pkill -f agent-guidance-mcp &>/dev/null || true

# Remove old Python uv tool install if exists
if command -v uv &> /dev/null; then
    uv tool uninstall agent-guidance-mcp 2>/dev/null || true
elif [ -f "$HOME/.local/bin/uv" ]; then
    "$HOME/.local/bin/uv" tool uninstall agent-guidance-mcp 2>/dev/null || true
fi
rm -f "$HOME/.local/bin/agent-guidance-mcp" 2>/dev/null || true

# ── Check Rust/Cargo ──────────────────────────────────────────────────────────
echo -e "${BOLD}Checking Rust toolchain (cargo)...${NC}"
if ! command -v cargo &> /dev/null && [ ! -f "$HOME/.cargo/bin/cargo" ]; then
    echo -e "  ${YELLOW}⚡${NC} Rust toolchain not found. Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
else
    export PATH="$HOME/.cargo/bin:$PATH"
    echo -e "  ${GREEN}✓${NC} Found Cargo in PATH"
fi

# ── Prepare ~/.local/bin ──────────────────────────────────────────────────────
mkdir -p "$HOME/.local/bin"
LOCAL_BIN="$HOME/.local/bin"

BUILD_DIR=""
if [ -f "./Cargo.toml" ] && grep -q 'name = "agent-guidance"' ./Cargo.toml 2>/dev/null; then
    BUILD_DIR="$(pwd)"
elif [ -f "$(dirname "$0")/../Cargo.toml" ] && grep -q 'name = "agent-guidance"' "$(dirname "$0")/../Cargo.toml" 2>/dev/null; then
    BUILD_DIR="$(cd "$(dirname "$0")/.." && pwd)"
fi

# Function for animated build progress spinner
run_with_spinner() {
    local cmd="$1"
    local msg="$2"
    local spin=('⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏')
    local log_file="$(mktemp 2>/dev/null || echo "/tmp/agent-guidance-build.log")"
    
    eval "$cmd" < /dev/null &>"$log_file" &
    local pid=$!
    
    local i=0
    while kill -0 $pid 2>/dev/null; do
        i=$(( (i+1) % 10 ))
        printf "\r  ${CYAN}%s${NC} %s" "${spin[$i]}" "$msg"
        sleep 0.15
    done
    
    wait $pid
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        printf "\r  ${GREEN}✓${NC} %sFinished successfully!            \n" "$msg"
        rm -f "$log_file" 2>/dev/null || true
    else
        printf "\r  ${RED}❌${NC} %sFailed with exit code %d.        \n" "$msg" $exit_code
        echo -e "${RED}Build error log:${NC}"
        cat "$log_file"
        rm -f "$log_file" 2>/dev/null || true
        exit 1
    fi
}

if [ -n "$BUILD_DIR" ]; then
    echo -e ""
    echo -e "${CYAN}⚙️  Building release binary from local source (${BUILD_DIR})...${NC}"
    # Use /tmp target dir if on non-POSIX/fuseblk filesystem (e.g. exFAT/NTFS)
    DEV="$(df -P "$BUILD_DIR" | tail -1 | awk '{print $1}')"
    if mount | grep "^$DEV " | grep -qiE 'exfat|fuseblk|ntfs|ntfs3'; then
        CARGO_TARGET="CARGO_TARGET_DIR=/tmp/agent-guidance-target"
    fi
    run_with_spinner "cd '$BUILD_DIR' && $CARGO_TARGET RUSTFLAGS='-A warnings' cargo build --release --quiet" "Compiling dependencies & Rust server... "
    killall agent-guidance &>/dev/null || pkill -f agent-guidance &>/dev/null || true
    TARGET_BIN="$BUILD_DIR/target/release/agent-guidance"
    [ -f "/tmp/agent-guidance-target/release/agent-guidance" ] && TARGET_BIN="/tmp/agent-guidance-target/release/agent-guidance"
    rm -f "$LOCAL_BIN/agent-guidance" 2>/dev/null || true
    cp "$TARGET_BIN" "$LOCAL_BIN/agent-guidance"
else
    echo -e ""
    echo -e "${CYAN}📦 Fetching source repository & building Rust server...${NC}"
    GLOBAL_SRC="$HOME/.agent-guidance/src"
    if [ -f "$GLOBAL_SRC/Cargo.toml" ] && grep -q 'name = "agent-guidance"' "$GLOBAL_SRC/Cargo.toml" 2>/dev/null; then
        (cd "$GLOBAL_SRC" && git fetch --depth 1 origin main &>/dev/null && git reset --hard origin/main &>/dev/null) || true
    else
        rm -rf "$GLOBAL_SRC"
        mkdir -p "$GLOBAL_SRC"
        git clone --depth 1 https://github.com/JunMystery/Agent-Guidance-Rust.git "$GLOBAL_SRC" &>/dev/null
    fi
    run_with_spinner "cd '$GLOBAL_SRC' && RUSTFLAGS='-A warnings' cargo build --release --quiet" "Compiling dependencies & Rust server... "
    killall agent-guidance &>/dev/null || pkill -f agent-guidance &>/dev/null || true
    rm -f "$LOCAL_BIN/agent-guidance" 2>/dev/null || true
    cp "$GLOBAL_SRC/target/release/agent-guidance" "$LOCAL_BIN/agent-guidance"
fi

echo -e ""
echo -e "${PURPLE}▶${NC} Registering Agent Guidance Rust server with detected IDE clients..."
"$HOME/.local/bin/agent-guidance" --setup

echo -e ""
echo -e "${PURPLE}▶${NC} Precomputing skill passage cache for instant first startup..."
"$HOME/.local/bin/agent-guidance" --generate-passage-cache

echo -e ""
echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}${BOLD}║         ✓  Rust Edition Installed Successfully!              ║${NC}"
echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
echo -e ""
echo -