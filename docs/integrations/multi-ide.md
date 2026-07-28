# Multi-IDE / Multi-CLI Usage

Agent Guidance MCP runs as a **ref-counted daemon** over a Unix socket. The first process becomes the daemon (loads models, binds socket); subsequent processes auto-detect the socket and connect as a stdio-to-socket proxy. All share a single model instance.

## How It Works

Each IDE registers `agent-guidance` in its MCP config:

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

### Connection Flow

```
IDE 1 → agent-guidance ── stdin/stdout ──→ DAEMON
                                               │
                                          Unix socket
                                               │
IDE 2 → agent-guidance ── stdin/stdout ──→ PROXY ──→ socket
IDE 3 → agent-guidance ── stdin/stdout ──→ PROXY ──→ socket
```

| Step | What happens |
|---|---|
| IDE 1 launches | No socket exists → becomes **daemon**, loads models (~20s init), handles stdio |
| IDE 2 launches | Socket exists → becomes **proxy**, connects socket, forwards stdio↔socket |
| IDE 3, 4, ... | Same as IDE 2 — all share the daemon's models |
| IDE 1 exits | Daemon continues serving remaining connections |
| All IDEs exit | Ref count hits 0 → **30s idle timer** → daemon exits, RAM freed |

### Models & Cache (shared)

| Component | Shared? | Notes |
|---|---|---|
| BERT embedding model | ✅ `OnceLock<Mutex<...>>` | Loaded once, 118MB RSS |
| Cross-encoder | ✅ `OnceLock<Mutex<...>>` | Loaded once, 80MB RSS |
| Passage embeddings (276 skills) | ✅ `OnceLock<Mutex<Vec<Vec<f32>>>>` | Pre-computed during init, 424KB |
| ServerState (workflow stage, plan) | ❌ Per connection | Each IDE has independent state |
| Usage tracking | ❌ Per connection | SQLite per-process |

### Socket path

```
~/.cache/agent-guidance/mcp.sock
```

Created when the daemon starts, deleted on clean shutdown. The directory is auto-created.

## Which Tools Trigger Model Load

Models are loaded once during `initialize` (first MCP handshake). All tools after that are instant:

| Tool | First call latency | Subsequent |
|---|---|---|
| `task_pipeline` | ~200ms (cache hit) | ~200ms |
| `guidance(search)` | ~200ms | ~200ms |
| All other tools | Instant | Instant |
| **initialize** (daemon only) | **~20s** (warmup) | 0s (subsequent IDEs) |

## Benefits

- **Single model in RAM**: 200MB total regardless of IDE count (vs 200MB × N without daemon)
- **No cold start**: second IDE onward skip the 20s model warmup
- **Zero config**: auto-detection via socket existence — no systemd, no manual daemon management
- **Self-cleaning**: 30s idle timeout means no zombie processes

## RAM Usage Estimates

| Setup | Without daemon | With daemon |
|---|---|---|
| 1 IDE | 200 MB | 200 MB |
| 2 IDEs | 400 MB | **200 MB** |
| 3 IDEs | 600 MB | **200 MB** |
| N IDEs | N × 200 MB | **200 MB** |
