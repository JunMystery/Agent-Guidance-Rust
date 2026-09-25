#!/usr/bin/env bash
set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; PURPLE='\033[0;35m'; BOLD='\033[1m'; GRAY='\033[0;90m'; NC='\033[0m'

echo -e "\n${PURPLE}${BOLD}╔══════════════════════════════════════════════════════════════╗"
echo -e "║           Agent Guidance Rust (macOS/Linux)                  ║"
echo -e "╚══════════════════════════════════════════════════════════════╝${NC}\n"

ACTION="1"; PROFILE="1"; SERVER_URL="http://127.0.0.1:11998"
BIND_ADDR="0.0.0.0"; WORKER_PORT=11998; DASHBOARD_PORT=11997; API_KEY=""
NON_INTERACTIVE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --uninstall) ACTION="2"; shift ;;
        --standalone) PROFILE="1"; shift ;;
        --client) PROFILE="2"; shift ;;
        --server) PROFILE="3"; shift ;;
        --profile) PROFILE="$2"; shift 2 ;;
        --server-url) SERVER_URL="$2"; shift 2 ;;
        --bind) BIND_ADDR="$2"; shift 2 ;;
        --worker-port) WORKER_PORT="$2"; shift 2 ;;
        --dashboard-port|--port) DASHBOARD_PORT="$2"; shift 2 ;;
        --api-key) API_KEY="$2"; shift 2 ;;
        -y|--yes|--non-interactive) NON_INTERACTIVE=true; shift ;;
        *) shift ;;
    esac
done

read_tty() {
    local prompt="$1" default="$2" val=""
    if [ "$NON_INTERACTIVE" = true ]; then echo "$default"; return; fi
    if [ -c /dev/tty ]; then read -p "$prompt [$default]: " val < /dev/tty || val="";
    elif [ -t 0 ]; then read -p "$prompt [$default]: " val || val="";
    else val="$default"; fi
    echo "${val:-$default}"
}

perform_uninstall() {
    echo -e "\n${RED}${BOLD}🗑️  Uninstalling Agent Guidance...${NC}\n"
    if [ "$(uname -s)" = "Linux" ] && command -v systemctl &>/dev/null; then
        systemctl --user stop agent-guidance 2>/dev/null || true
        systemctl --user disable agent-guidance 2>/dev/null || true
        rm -f "$HOME/.config/systemd/user/agent-guidance.service" 2>/dev/null || true
    elif [ "$(uname -s)" = "Darwin" ]; then
        local plist="$HOME/Library/LaunchAgents/com.junmystery.agent-guidance.plist"
        launchctl unload "$plist" 2>/dev/null || true
        rm -f "$plist" 2>/dev/null || true
    fi
    killall agent-guidance agent-guidance-mcp &>/dev/null || pkill -f agent-guidance &>/dev/null || true
    [ -d "$HOME/.agent-guidance" ] && rm -rf "$HOME/.agent-guidance" && echo -e "  ${GREEN}✓${NC} Removed $HOME/.agent-guidance"
    rm -f "$HOME/.local/bin/agent-guidance" "$HOME/.cargo/bin/agent-guidance" 2>/dev/null || true
    echo -e "\n${GREEN}${BOLD}✓ Uninstallation complete!${NC}\n"; exit 0
}

if [ "$ACTION" != "2" ] && [ "$NON_INTERACTIVE" = false ]; then
    echo -e "${BOLD}Select action:${NC}"
    echo -e "  ${GREEN}[1]${NC} Install / Update"
    echo -e "  ${RED}[2]${NC} Uninstall"
    ACTION="$(read_tty "Choice" "1")"
fi
[ "$ACTION" = "2" ] && perform_uninstall

if [ "$NON_INTERACTIVE" = false ]; then
    echo -e "\n${BOLD}Select installation profile:${NC}"
    echo -e "  ${GREEN}[1] Full Standalone${NC}       (Single binary with local Candle/ORT + SQLite FTS5)"
    echo -e "  ${CYAN}[2] Lightweight Client${NC}    (Zero-ML ~15 MB RAM, forwards queries to Remote ML Worker)"
    echo -e "  ${PURPLE}[3] Dedicated Server Worker${NC} (Dedicated ML node, compiles binary registry, runs OS daemon)"
    PROFILE="$(read_tty "Profile" "1")"
fi

killall agent-guidance &>/dev/null || pkill -f agent-guidance &>/dev/null || true
mkdir -p "$HOME/.local/bin"
LOCAL_BIN="$HOME/.local/bin"

detect_assets() {
    local os arch prof_tag candidates=()
    [ "$(uname -s)" = "Darwin" ] && os="macos" || os="linux"
    arch="$(uname -m)"
    [ "$arch" = "arm64" ] && arch="aarch64"
    case "$PROFILE" in
        2|client) prof_tag="client" ;;
        3|server) prof_tag="server" ;;
        *) prof_tag="standalone" ;;
    esac
    candidates+=("agent-guidance-${prof_tag}-${os}-${arch}.tar.gz")
    [ "$prof_tag" != "standalone" ] && candidates+=("agent-guidance-standalone-${os}-${arch}.tar.gz")
    candidates+=("agent-guidance-${os}-${arch}.tar.gz")
    echo "${candidates[@]}"
}

try_download() {
    local repo="JunMystery/Agent-Guidance-Rust" version=""
    if command -v curl &>/dev/null; then
        version="$(curl -sSL "https://api.github.com/repos/${repo}/releases/latest" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
    elif command -v wget &>/dev/null; then
        version="$(wget -qO- "https://api.github.com/repos/${repo}/releases/latest" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
    fi
    version="${version:-v1.8.0}"
    local tmp_dir; tmp_dir="$(mktemp -d 2>/dev/null || echo "/tmp/ag-download-$$")"
    mkdir -p "$tmp_dir"

    for asset in $(detect_assets); do
        local url="https://github.com/${repo}/releases/download/${version}/${asset}"
        echo -e "  ${GRAY}Probing release asset: ${asset}...${NC}"
        local ok=false
        if command -v curl &>/dev/null && curl -fsSL "$url" -o "$tmp_dir/$asset" 2>/dev/null; then ok=true;
        elif command -v wget &>/dev/null && wget -q "$url" -O "$tmp_dir/$asset" 2>/dev/null; then ok=true; fi
        if [ "$ok" = true ] && [ -s "$tmp_dir/$asset" ]; then
            echo -e "  ${GREEN}✓${NC} Downloaded prebuilt release ${version} (${asset})"
            tar -xzf "$tmp_dir/$asset" -C "$tmp_dir" 2>/dev/null || continue
            if [ -f "$tmp_dir/agent-guidance" ]; then
                rm -f "$LOCAL_BIN/agent-guidance" 2>/dev/null || true
                mv "$tmp_dir/agent-guidance" "$LOCAL_BIN/agent-guidance"
                chmod +x "$LOCAL_BIN/agent-guidance"
                rm -rf "$tmp_dir"; return 0
            fi
        fi
    done
    rm -rf "$tmp_dir"; return 1
}

build_source() {
    if ! command -v cargo &>/dev/null && [ ! -f "$HOME/.cargo/bin/cargo" ]; then
        echo -e "  ${YELLOW}⚡ Installing rustup...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    fi
    export PATH="$HOME/.cargo/bin:$PATH"
    local sdir=""
    if [ -f "./Cargo.toml" ] && grep -q 'name = "agent-guidance"' ./Cargo.toml 2>/dev/null; then sdir="$(pwd)"
    elif [ -f "$(dirname "$0")/../Cargo.toml" ] && grep -q 'name = "agent-guidance"' "$(dirname "$0")/../Cargo.toml" 2>/dev/null; then
        sdir="$(cd "$(dirname "$0")/.." && pwd)"
    else
        sdir="$HOME/.agent-guidance/src"
        if [ -f "$sdir/Cargo.toml" ]; then (cd "$sdir" && git pull --depth 1 &>/dev/null) || true;
        else rm -rf "$sdir"; mkdir -p "$sdir"; git clone --depth 1 https://github.com/JunMystery/Agent-Guidance-Rust.git "$sdir" &>/dev/null; fi
    fi
    echo -e "  ${CYAN}Compiling release binary...${NC}"
    (cd "$sdir" && RUSTFLAGS='-A warnings' cargo build --release --quiet)
    rm -f "$LOCAL_BIN/agent-guidance" 2>/dev/null || true
    cp "$sdir/target/release/agent-guidance" "$LOCAL_BIN/agent-guidance"
}

if ! try_download; then
    echo -e "  ${YELLOW}Prebuilt unavailable, building from source...${NC}"
    build_source
fi

register_ides() {
    echo -e "\n${PURPLE}▶${NC} Registering server with detected IDE clients..."
    "$LOCAL_BIN/agent-guidance" --setup
    local cdir="$HOME/.cursor"
    mkdir -p "$cdir" 2>/dev/null || true
    if [ ! -f "$cdir/mcp.json" ]; then
        printf '{\n  "mcpServers": {\n    "agent-guidance": {\n      "command": "%s",\n      "args": []\n    }\n  }\n}\n' "$LOCAL_BIN/agent-guidance" > "$cdir/mcp.json"
    fi
    local payload="{\"name\":\"agent-guidance\",\"type\":\"stdio\",\"command\":\"$LOCAL_BIN/agent-guidance\",\"args\":[]}"
    for cmd in code code-insiders; do command -v "$cmd" &>/dev/null && "$cmd" --add-mcp "$payload" &>/dev/null || true; done
    command -v claude &>/dev/null && claude mcp add --scope user agent-guidance -- "$LOCAL_BIN/agent-guidance" &>/dev/null || true
    command -v codex &>/dev/null && codex mcp add agent-guidance -- "$LOCAL_BIN/agent-guidance" &>/dev/null || true
}

configure_server_daemon() {
    echo -e "\n${PURPLE}▶${NC} Configuring Dedicated Server Worker..."
    mkdir -p "$HOME/.agent-guidance/staging/skills"
    [ ! -f "$HOME/.agent-guidance/tombstones.json" ] && echo "[]" > "$HOME/.agent-guidance/tombstones.json"
    if [ "$NON_INTERACTIVE" = false ]; then
        BIND_ADDR="$(read_tty "Bind Address" "$BIND_ADDR")"
        WORKER_PORT="$(read_tty "ML Worker Port" "$WORKER_PORT")"
        DASHBOARD_PORT="$(read_tty "Dashboard Port" "$DASHBOARD_PORT")"
        API_KEY="$(read_tty "Optional Bearer API Key" "$API_KEY")"
    fi
    local key_arg=""
    [ -n "$API_KEY" ] && key_arg=" --api-key $API_KEY"

    if [ "$(uname -s)" = "Linux" ]; then
        local udir="$HOME/.config/systemd/user" unit="$udir/agent-guidance.service"
        mkdir -p "$udir"
        cat <<EOF > "$unit"
[Unit]
Description=Agent Guidance Remote ML Worker
After=network.target

[Service]
Type=simple
ExecStart=$LOCAL_BIN/agent-guidance --server --bind $BIND_ADDR --worker-port $WORKER_PORT --port $DASHBOARD_PORT$key_arg
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
EOF
        if command -v systemctl &>/dev/null; then
            systemctl --user daemon-reload 2>/dev/null || true
            systemctl --user enable --now agent-guidance 2>/dev/null || true
            echo -e "  ${GREEN}✓${NC} Started systemd service: agent-guidance"
        fi
    elif [ "$(uname -s)" = "Darwin" ]; then
        local pdir="$HOME/Library/LaunchAgents" plist="$pdir/com.junmystery.agent-guidance.plist"
        mkdir -p "$pdir"
        cat <<EOF > "$plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
<key>Label</key><string>com.junmystery.agent-guidance</string>
<key>ProgramArguments</key>
<array>
<string>$LOCAL_BIN/agent-guidance</string>
<string>--server</string>
<string>--bind</string>
<string>$BIND_ADDR</string>
<string>--worker-port</string>
<string>$WORKER_PORT</string>
<string>--port</string>
<string>$DASHBOARD_PORT</string>
$( [ -n "$API_KEY" ] && echo "<string>--api-key</string><string>$API_KEY</string>" )
</array>
<key>RunAtLoad</key><true/>
<key>KeepAlive</key><true/>
</dict>
</plist>
EOF
        launchctl unload "$plist" 2>/dev/null || true
        launchctl load "$plist" 2>/dev/null || true
        echo -e "  ${GREEN}✓${NC} Loaded launchd daemon: com.junmystery.agent-guidance"
    fi
    echo -e "\n${GREEN}${BOLD}✓ Server worker daemon active!${NC}"
    echo -e "  ${BOLD}Worker API:${NC}      http://${BIND_ADDR}:${WORKER_PORT}"
    echo -e "  ${BOLD}Dashboard:${NC}       http://${BIND_ADDR}:${DASHBOARD_PORT}"
    echo -e "  ${BOLD}Client Setup:${NC}    ${CYAN}agent-guidance --set-server http://<SERVER_IP>:${WORKER_PORT}${NC}"
}

case "$PROFILE" in
    2|client)
        if [ "$NON_INTERACTIVE" = false ]; then
            SERVER_URL="$(read_tty "Remote Worker Server URL" "$SERVER_URL")"
        fi
        "$LOCAL_BIN/agent-guidance" --set-server "$SERVER_URL"
        register_ides
        echo -e "\n${GREEN}✓ Lightweight Client configured!${NC} (Connected to ${SERVER_URL})"
        "$LOCAL_BIN/agent-guidance" --stats || true
        ;;
    3|server)
        configure_server_daemon
        ;;
    *)
        register_ides
        echo -e "\n${GREEN}${BOLD}✓ Agent Guidance Standalone Installed!${NC}"
        ;;
esac

echo -e "  ${BOLD}Binary:${NC} ${GREEN}${LOCAL_BIN}/agent-guidance${NC}\n"
