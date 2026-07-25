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

# ── Check Rust/Cargo ──────────────────────────────────────────────────────────
if ! command -v cargo &> /dev/null && [ ! -f "$HOME/.cargo/bin/cargo" ]; then
    echo -e "${YELLOW}⚡ Rust toolchain not found. Installing rustup...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
else
    export PATH="$HOME/.cargo/bin:$PATH"
fi

echo -e "${CYAN}🔨 Building release binary from Cargo...${NC}"
cargo build --release

echo -e ""
echo -e "${GREEN}${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}${BOLD}║         ✓  Build completed successfully!                    ║${NC}"
echo -e "${GREEN}${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
echo -e ""
echo -e "  ${BOLD}Executable Binary Path:${NC}"
echo -e "    ${CYAN}$(pwd)/target/release/agent-guidance${NC}"
echo -e ""
