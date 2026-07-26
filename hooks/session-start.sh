#!/bin/bash
# agent-guidance-mcp session-start hook
# Passes priority gate and injects project context at session start.
# Tries: installed binary, python -m module, then fallback meta-skill.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TARGET_PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$PWD}"

# Auto-detect IDE/CLI name for usage tracking
if [ -z "${AGENT_CLIENT_NAME:-}" ]; then
  if [ -n "${OPENCODE_VERSION:-}" ]; then
    AGENT_CLIENT_NAME="OpenCode"
  elif [ -n "${GEMINI_CLI_VERSION:-}" ]; then
    AGENT_CLIENT_NAME="Gemini CLI"
  elif [ -n "${ANTIGRAVITY_VERSION:-}" ] || [ -f "${TARGET_PROJECT_DIR}/.antigravity/mcp.json" ]; then
    AGENT_CLIENT_NAME="Antigravity"
  elif [ -n "${VSCODE_INJECTION:-}" ]; then
    AGENT_CLIENT_NAME="VS Code"
  elif [ -n "${CURSOR_TRACE_ID:-}" ] || [ -n "${CURSOR_VERSION:-}" ]; then
    AGENT_CLIENT_NAME="Cursor"
  elif [ "${TERM_PROGRAM:-}" = "vscode" ]; then
    AGENT_CLIENT_NAME="VS Code"
  elif [ "${TERM_PROGRAM:-}" = "windsurf" ]; then
    AGENT_CLIENT_NAME="Windsurf"
  elif [ "${CLAUDE_PROJECT_DIR:-}" ]; then
    AGENT_CLIENT_NAME="Claude Code"
  fi
fi
export AGENT_CLIENT_NAME

run_session_start() {
  "$1" --session-start --project-path "$TARGET_PROJECT_DIR" 2>/dev/null
}

# Priority 1: installed Rust binary on PATH (agent-guidance)
if command -v agent-guidance >/dev/null 2>&1; then
  output=$(run_session_start agent-guidance)
  if [ -n "$output" ]; then
    echo "$output"; exit 0
  fi
fi

# Priority 2: fallback binary name (agent-guidance-mcp)
if command -v agent-guidance-mcp >/dev/null 2>&1; then
  output=$(run_session_start agent-guidance-mcp)
  if [ -n "$output" ]; then
    echo "$output"; exit 0
  fi
fi

# Fallback: inject using-agent-skills meta-skill
SKILLS_DIR="$(dirname "$SCRIPT_DIR")/skills"
META_SKILL="$SKILLS_DIR/using-agent-skills/SKILL.md"
if [ -f "$META_SKILL" ]; then
  SKILL=$(cat "$META_SKILL" | tr '\n' ' ' | sed 's/"/\\"/g')
  echo "{\"priority\":\"IMPORTANT\",\"message\":\"agent-skills loaded.\\n\\n${SKILL}\"}"
else
  echo '{"priority": "INFO", "message": "agent-guidance: session-start unavailable. Install agent-guidance for full context injection."}'
fi
