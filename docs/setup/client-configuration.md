# Client Configuration

[Back to README](../README.md)

This server is a Rust binary that auto-detects daemon/proxy mode via Unix socket. Configure each IDE/CLI to launch `agent-guidance` as a stdio MCP command.

## VS Code And GitHub Copilot

The repository includes a workspace MCP settings file under `.vscode/mcp.json`.

When you open this repository in VS Code with GitHub Copilot installed:

1. Build or install the binary (`cargo build --release` or install via `--setup`).
2. Open the repository folder in VS Code.
3. Let VS Code detect the MCP server from `.vscode/mcp.json`.
4. Trust the server when prompted.
5. Use the tools and prompts from Copilot Chat.

The `.vscode/mcp.json` entry should point to the `agent-guidance` binary:

```json
{
  "servers": {
    "agent-guidance": {
      "command": "/absolute/path/to/agent-guidance",
      "args": []
    }
  }
}
```

You can also register directly via the VS Code CLI:

```bash
code --add-mcp '{"name":"agent-guidance","type":"stdio","command":"/path/to/agent-guidance","args":[]}'
```

## Cursor

Cursor detects MCP servers via `~/.cursor/mcp.json` (global) or `%APPDATA%\Cursor\User\mcp.json`:

```json
{
  "mcpServers": {
    "agent-guidance": {
      "command": "/path/to/agent-guidance",
      "args": []
    }
  }
}
```

## Claude Code CLI

Register user-wide directly via the Claude Code CLI:

```bash
claude mcp add --scope user agent-guidance -- /path/to/agent-guidance
```

Configuration is stored in `~/.claude.json` or `~/.claude/mcp.json`.

## ChatGPT / OpenAI Codex

ChatGPT desktop app and OpenAI Codex CLI use `~/.codex/config.toml`:

```toml
[mcp_servers.agent-guidance]
command = "/path/to/agent-guidance"
args = []
```

Or register via the Codex CLI:

```bash
codex mcp add agent-guidance -- /path/to/agent-guidance
```

## Generic MCP Client Config

Use this structure for Claude Desktop and other MCP-compatible clients:

```json
{
  "mcpServers": {
    "agent-guidance": {
      "command": "agent-guidance",
      "args": []
    }
  }
}
```

If `agent-guidance` is not in your `PATH`, use the absolute path:

```json
{
  "mcpServers": {
    "agent-guidance": {
      "command": "/home/user/.local/bin/agent-guidance",
      "args": []
    }
  }
}
```

## Auto-Detection Behavior

When launched, the binary checks for an existing daemon socket or named pipe:
- Windows: `\\.\pipe\agent-guidance-mcp`
- Unix: `~/.cache/agent-guidance/mcp.sock`

- **No socket** → starts as the daemon (loads neural models into memory, binds socket/pipe, serves MCP requests, runs web dashboard)
- **Socket exists** → starts as lightweight client proxy (forwards stdin/stdout to daemon in < 5ms)

No configuration flags needed for normal use. For testing:

| Flag | Effect |
|---|---|
| `--force-daemon`, `--daemon` | Force start as daemon |
| `--force-client`, `--proxy` | Force connect as proxy; exit if no daemon |
| `--dashboard` | Open embedded web dashboard in browser |
| `--port <PORT>`, `--dashboard-port <PORT>` | Set web dashboard port (default: 11997) |

## Environment Variables

Configure these environment variables in your client configuration `"env"` block or shell:

| Variable | Default | Description |
|---|---|---|
| `AGENT_GUIDANCE_DEVICE` | `auto` | ML compute provider: `auto`, `cpu`, `cuda` (NVIDIA), `directml` / `dml` (Windows AMD/Intel/NVIDIA), `metal` (macOS Apple Silicon) |
| `AGENT_GUIDANCE_IDLE_TIMEOUT` | `60` | Daemon idle shutdown countdown in seconds after all IDE clients disconnect (note: daemon stays alive if an IDE process like VS Code, Cursor, or Antigravity is running) |
| `AGENT_GUIDANCE_DASHBOARD_PORT`| `11997` | Port for the embedded telemetry dashboard and REST API |
| `AGENT_GUIDANCE_TOKEN_OPT` | `1` | `0` = disable markdown token compression and whitespace reduction |
| `AGENT_GUIDANCE_ONNX_PATH` | auto (`~/.agent-guidance/models`) | Custom directory containing ONNX quantized embedding models |
| `AGENT_GUIDANCE_DISABLE_ML` | `0` | `1` = bypass background VRAM auto-warmup |

## Related Docs

- [Installation](../installation.md)
- [Usage Guide](../usage.md)
- [MCP Surface](../reference/mcp-surface.md)
- [Dashboard Guide](../dashboard.md)

