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

## Generic MCP Client Config

Use this structure for Claude Desktop, Cursor, and other MCP-compatible clients:

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

When launched, the binary checks for an existing daemon socket at `~/.cache/agent-guidance/mcp.sock`:

- **No socket** → becomes the daemon (loads models, binds socket, handles this client)
- **Socket exists** → becomes a proxy (forwards stdin/stdout to the daemon)

No configuration flags needed for normal use. For testing:

| Flag | Effect |
|---|---|
| `--force-daemon` | Start as daemon even if socket exists |
| `--force-client` | Connect as proxy; exit if no daemon |

## Token Optimization & Compression

Configure these environment variables in your MCP client's `"env"` settings:

| Variable | Default | Description |
|---|---|---|
| `AGENT_GUIDANCE_TOKEN_OPT` | `1` | `0` = disable all optimization & compression |
| `AGENT_GUIDANCE_FILTER_LEVEL` | `minimal` | `none` / `minimal` / `aggressive` — comment stripping depth |
| `AGENT_GUIDANCE_DOC_MAX_TOKENS` | `8000` | Max token cap for standard documents |
| `AGENT_GUIDANCE_SKILL_MAX_TOKENS` | `8000` | Max token cap for skill guides |
| `AGENT_GUIDANCE_TRACK_SAVINGS` | `1` | `0` = disable token analytics |
| `AGENT_GUIDANCE_ROOT` | auto | Override path to standards corpus |

### Filter Levels

| Value | Effect |
|---|---|
| `none` | No compression |
| `minimal` (default) | Strips block headers and whitespace, preserves inline comments |
| `aggressive` | Strips all docstrings and comments for maximum compression |

## Related Docs

- [Installation](../installation.md)
- [Usage Guide](../usage.md)
- [MCP Surface](../reference/mcp-surface.md)
