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

# ── Choose mode ───────────────────────────────────────────────────────────────
echo -e "${BOLD}What would you like to do?${NC}"
echo -e "  ${GREEN}[1]${NC} Install / Update  (build latest Rust server + update dashboard)"
echo -e "  ${RED}[2]${NC} Uninstall         (remove binary, data directory)"
echo -e ""

if [ -c /dev/tty ]; then
    read -p "Choice [1]: " ACTION < /dev/tty || ACTION=""
elif [ -t 0 ]; then
    read -p "Choice [1]: " ACTION || ACTION=""
else
    ACTION="1"
fi
ACTION="${ACTION:-1}"

# ── Uninstall path ────────────────────────────────────────────────────────────
if [ "$ACTION" = "2" ]; then
    echo -e ""
    echo -e "${RED}${BOLD}🗑️  Uninstalling Agent Guidance...${NC}"
    echo -e ""

    # Stop any running processes
    killall agent-guidance &>/dev/null || true
    pkill -f agent-guidance &>/dev/null || true

    # Completely remove configuration and data directory
    if [ -d "$HOME/.agent-guidance" ]; then
        rm -rf "$HOME/.agent-guidance"
        echo -e "  ${GREEN}✓${NC} Removed directory ${GRAY}$HOME/.agent-guidance${NC}"
    fi

    # Remove binary
    rm -f "$HOME/.local/bin/agent-guidance" 2>/dev/null || true
    rm -f "$HOME/.cargo/bin/agent-guidance" 2>/dev/null || true

    echo -e ""
    echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}${BOLD}║         ✓  Uninstallation finished!                         ║${NC}"
    echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo -e ""
    exit 0
fi

# ── Install / Update path ─────────────────────────────────────────────────────
echo -e ""
echo -e "${YELLOW}⚡ Stopping any running agent-guidance processes...${NC}"
killall agent-guidance &>/dev/null || pkill -f agent-guidance &>/dev/null || true

# ── Check Rust/Cargo ──────────────────────────────────────────────────────────
echo -e "${BOLD}Checking Rust toolchain (cargo)...${NC}"
if ! command -v cargo &>/dev/null && [ ! -f "$HOME/.cargo/bin/cargo" ]; then
    echo -e "  ${YELLOW}⚡${NC} Rust not found. Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
else
    export PATH="$HOME/.cargo/bin:$PATH"
    echo -e "  ${GREEN}✓${NC} Found Cargo in PATH"
fi

# ── Prepare ~/.local/bin ──────────────────────────────────────────────────────
mkdir -p "$HOME/.local/bin"
LOCAL_BIN="$HOME/.local/bin"

# ── Detect build source (local dev or remote clone) ───────────────────────────
BUILD_DIR=""
if [ -f "./Cargo.toml" ] && grep -q 'name = "agent-guidance"' ./Cargo.toml 2>/dev/null; then
    BUILD_DIR="$(pwd)"
elif [ -f "$(dirname "$0")/../Cargo.toml" ] && grep -q 'name = "agent-guidance"' "$(dirname "$0")/../Cargo.toml" 2>/dev/null; then
    BUILD_DIR="$(cd "$(dirname "$0")/.." && pwd)"
fi

# ── Spinner helper ────────────────────────────────────────────────────────────
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
        printf "\r  ${RED}❌${NC} %sFailed (exit %d). See log below:\n" "$msg" $exit_code
        cat "$log_file"
        rm -f "$log_file" 2>/dev/null || true
        exit 1
    fi
}

# ── Use /tmp target dir on non-POSIX filesystems (exFAT/NTFS) ────────────────
CARGO_TARGET=""
detect_cargo_target() {
    local dir="$1"
    local DEV
    DEV="$(df -P "$dir" | tail -1 | awk '{print $1}')"
    if mount 2>/dev/null | grep "^$DEV " | grep -qiE 'exfat|fuseblk|ntfs|ntfs3'; then
        CARGO_TARGET="CARGO_TARGET_DIR=/tmp/agent-guidance-target"
    fi
}

# ── Build ──────────────────────────────────────────────────────────────────────
if [ -n "$BUILD_DIR" ]; then
    echo -e ""
    echo -e "${CYAN}⚙️  Building release binary from local source (${BUILD_DIR})...${NC}"
    detect_cargo_target "$BUILD_DIR"
    run_with_spinner "cd '$BUILD_DIR' && $CARGO_TARGET RUSTFLAGS='-A warnings' cargo build --release --quiet" "Compiling Rust server + embedding dashboard assets... "
    TARGET_BIN="$BUILD_DIR/target/release/agent-guidance"
    [ -f "/tmp/agent-guidance-target/release/agent-guidance" ] && TARGET_BIN="/tmp/agent-guidance-target/release/agent-guidance"
    rm -f "$LOCAL_BIN/agent-guidance" 2>/dev/null || true
    cp "$TARGET_BIN" "$LOCAL_BIN/agent-guidance"
else
    echo -e ""
    echo -e "${CYAN}📦 Fetching latest source from GitHub \& building...${NC}"
    GLOBAL_SRC="$HOME/.agent-guidance/src"
    if [ -f "$GLOBAL_SRC/Cargo.toml" ] && grep -q 'name = "agent-guidance"' "$GLOBAL_SRC/Cargo.toml" 2>/dev/null; then
        echo -e "  ${GRAY}Pulling latest changes from origin/main...${NC}"
        (cd "$GLOBAL_SRC" && git fetch --depth 1 origin main &>/dev/null && git reset --hard origin/main &>/dev/null) || true
    else
        rm -rf "$GLOBAL_SRC"
        mkdir -p "$GLOBAL_SRC"
        git clone --depth 1 https://github.com/JunMystery/Agent-Guidance-Rust.git "$GLOBAL_SRC" &>/dev/null
    fi
    detect_cargo_target "$GLOBAL_SRC"
    run_with_spinner "cd '$GLOBAL_SRC' && $CARGO_TARGET RUSTFLAGS='-A warnings' cargo build --release --quiet" "Compiling Rust server + embedding dashboard assets... "
    TARGET_BIN="$GLOBAL_SRC/target/release/agent-guidance"
    [ -f "/tmp/agent-guidance-target/release/agent-guidance" ] && TARGET_BIN="/tmp/agent-guidance-target/release/agent-guidance"
    rm -f "$LOCAL_BIN/agent-guidance" 2>/dev/null || true
    cp "$TARGET_BIN" "$LOCAL_BIN/agent-guidance"
fi

# ── Register with IDEs ────────────────────────────────────────────────────────
echo -e ""
echo -e "${PURPLE}▶${NC} Registering server with detected IDE clients..."
"$LOCAL_BIN/agent-guidance" --setup

# ── Precompute skill embedding cache ──────────────────────────────────────────
echo -e ""
echo -e "${PURPLE}▶${NC} Precomputing skill passage embedding cache..."
"$LOCAL_BIN/agent-guidance" --generate-passage-cache

# ── Done ──────────────────────────────────────────────────────────────────────
echo -e ""
echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}${BOLD}║         ✓  Agent Guidance Installed / Updated!               ║${NC}"
echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
echo -e ""
echo -e "  ${BOLD}Binary:${NC}    ${CYAN}$LOCAL_BIN/agent-guidance${NC}"
echo -e "  ${BOLD}Dashboard:${NC} ${CYAN}agent-guidance --dashboard${NC}  (serves updated HTML/JS embedded in binary)"
echo -e ""