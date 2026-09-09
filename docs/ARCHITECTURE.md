# Architecture

[Back to README](../README.md)

## Overview

Agent Guidance MCP is a **100% Native Rust 2024 Edition** MCP server that gives AI coding agents standards guidance, skill references, workflow prompts, and bounded project code context. It runs as a **ref-counted daemon** with 30s idle auto-shutdown — models are loaded once and shared across all IDE/CLI connections.

---

## Rust Module Map

```
src/
├── main.rs            # Binary entrypoint — auto-detects daemon/proxy mode
├── daemon.rs          # Unix socket daemon, connection tracking, 30s idle timeout
├── catalog/           # Skills catalog management
│   ├── store.rs       # Embedded rust_embed skills + workspace-local scanning
│   ├── updater.rs     # Async auto-updater for 3rd-party skill repositories
│   └── mod.rs
├── context/           # Project context & indexing
│   ├── scanner.rs     # Bounded workspace scanner & ignore filter
│   ├── db.rs          # SQLite FTS5 code symbol indexing & usage database
│   └── mod.rs
├── dashboard/         # Native HTTP usage dashboard server & embedded HTML frontend
│   └── mod.rs
├── mcp/               # Model Context Protocol engine
│   ├── db.rs          # SQLite usage metrics persistence, 24h pruning & daily aggregations
│   ├── protocol.rs    # JSON-RPC request & response structs
│   ├── router.rs      # Tool dispatcher & resource router
│   ├── state.rs       # ServerState priority gate, stage matrix & circuit breaker
│   ├── tools.rs       # Tool handlers (task_pipeline, guidance, project_context, etc.)
│   ├── config.rs      # IDE client auto-registration & tagged block section deployment
│   ├── templates.rs   # Embedded AGENTS.md rules & templates
│   └── mod.rs
├── ml/                # Machine learning & vector search
│   ├── embeddings.rs  # Candle BERT (intfloat/multilingual-e5-small) — cached model + passage embeddings
│   ├── llm_selector.rs# Cross-encoder (cross-encoder/ms-marco-MiniLM-L-6-v2) reranker
│   └── mod.rs
└── optimizer/         # Token optimization engine
    ├── compressor.rs  # Language-aware token compressor & comment stripper
    └── mod.rs
```

---

## Transport Architecture

## Transport Architecture

### Cross-Platform Detached Singleton Daemon & Thin Proxy

On launch from any IDE/CLI (VS Code, Cursor, Claude Code, Codex, Antigravity), `agent-guidance` auto-negotiates its runtime role:

```
agent-guidance (start)
  ├─ IPC channel active?
  │   ├─ Windows: Named Pipe `\\.\pipe\agent-guidance-mcp`
  │   └─ Unix: Domain Socket `~/.cache/agent-guidance/mcp.sock`
  │   │
  │   └─ YES → PROXY mode (< 5MB RAM):
  │            Connect IPC channel, forward stdin↔IPC↔stdout via tokio::select!, exit on EOF
  │
  └─ NO → Spawn DETACHED Daemon & Connect as PROXY:
           Spawn detached background daemon process (`DETACHED_PROCESS` on Windows, nohup on Unix),
           wait up to 3s for IPC channel to become ready, connect as Thin Proxy
```

### Shared Singleton Topology

```
IDE 1 (Proxy) ──► Named Pipe / Unix Socket ──┐
                                             ▼
IDE 2 (Proxy) ──► Named Pipe / Unix Socket ──► agent-guidance (DETACHED BACKGROUND DAEMON)
                                             ├─ IPC Server Listener (Named Pipe / Unix Socket)
IDE 3 (Proxy) ──► Named Pipe / Unix Socket ──┤─ Embedded Web Dashboard (http://127.0.0.1:11997)
                                             ├─ Global Tool Worker Pool (32 workers)
                                             ├─ SyncMlQueue Concurrency Limiter (2 permits)
                                             └─ Per-Project Isolated GraphRAG Sync
```

- **Detached Background Daemon**: Runs completely decoupled from IDE parent process trees (`DETACHED_PROCESS` / `CREATE_NO_WINDOW` on Windows). Closing or reloading any IDE never kills the daemon, preventing fate-sharing crashes (`0xc0000409`).
- **Embedded Web Dashboard**: Automatically spawned in the background on port `11997` (customizable via `--port`, `--dashboard-port`, or `AGENT_GUIDANCE_DASHBOARD_PORT`), always reachable whenever any agent session is active.
- **Thin Proxy**: Every IDE session acts as a lightweight stdio-to-IPC bridge (< 5MB RAM, ~0.1ms connect time), exiting cleanly via `tokio::select!` without leaving zombie processes.
- **Fail-Safe Fallback**: If IPC spawning fails, processes automatically fall back to direct stdio handling.

### Global Concurrency, ML Queue & Per-Project GraphRAG

To prevent CPU/GPU starvation and thread thrashing across multiple connected IDEs:

```
                     Incoming Tool / ML Requests
                                │
        ┌───────────────────────┴───────────────────────┐
        ▼                                               ▼
Global Worker Pool (32 workers)                SyncMlQueue (RAII SyncMlPermit)
  - 32 concurrent execution permits              - Max 2 concurrent heavy ML inferences
  - Throttles file reading & AST scans           - FIFO admission for Candle BERT embeddings
  - Prevents SQLite lock contention              - Prevents VRAM/RAM exhaustion across IDEs
```

- **Per-Project GraphRAG JIT Sync**: `PROJECT_LAST_SYNC: RwLock<HashMap<PathBuf, u64>>` tracks debounce intervals per canonical project path. Inspecting code in Repo A never delays or blocks GraphRAG updates for Repo B.

### Connection Tracking & 60s Idle Cooldown

```
                    ACTIVE_CLIENTS (AtomicUsize)
                    ┌──────────────────────────┐
                    │     active_clients = N   │
                    └──────────────────────────┘
                                 │
                    ┌────────────┴────────────┐
                    │  60s cooldown timer     │
                    │  (triggered when N == 0)│
                    └────────────┬────────────┘
                                 │
              ╔══════════════════╧══════════════════╗
              ║ All IDE connections closed (N = 0)  ║
              ║ → start 60s cooldown timer         ║
              ║ → IDE reconnects? → cancel cooldown║
              ║ → still 0 after 60s? → clean exit   ║
              ╚═════════════════════════════════════╝
```

| Event | `active_clients` | Action |
|---|---|---|
| New connection accepted | `+= 1` | Spawn `handle_mcp_lines` task |
| Connection closed | `-= 1` | Check if 0 |
| active_clients reaches 0 | — | Start 60s countdown (checks every 1s) |
| New connection during countdown | `+= 1` | Cancel countdown |
| 60s elapsed, still 0 | — | Delete socket / close pipe, exit process |

---

## Model Architecture

### Zero-Friction Background Auto-Warmup

The daemon accepts connections immediately without blocking handshake (< 2ms response time via Fast-Path):

```
daemon start / connection opened
  └─ spawn_background_auto_warmup() (detached thread)
      ├─ load_precomputed_cache() → instant 440-vector load (0.1ms if count matches)
      ├─ cached_model() → Candle BERT OnceLock init (~0.5s disk load)
      └─ cached_cross_encoder() → CrossEncoder OnceLock init (~0.07s)
```

- **Non-blocking Handshake**: Immediate tool availability; requests initially use fast heuristic paths.
- **Seamless Upgrade**: As soon as OnceLock initialization completes, subsequent queries automatically leverage BERT vector cosine similarity and Cross-Encoder neural reranking.
- **Turn-1 GraphRAG Indexing**: `task_pipeline` automatically invokes `ensure_indexed()` on Turn 1 to sync workspace AST symbols and code chunks into SQLite without manual intervention.

| Component | Model | Size | Init Time |
|---|---|---|---|
| Embedding | `intfloat/multilingual-e5-small` (384-dim) | 118MB | ~560ms (background) |
| Cross-encoder | `cross-encoder/ms-marco-MiniLM-L-6-v2` | 80MB | ~70ms (background) |
| Precomputed Cache | 440 skill passage vectors | — | ~0.1ms (instant) |

### Cached Passage Embeddings (`PASSAGE_CACHE`)

```rust
static PASSAGE_CACHE: OnceLock<RwLock<Vec<PassageCache>>> = OnceLock::new();
```

- Embedded and workspace-local skill passage vectors are cached by catalog fingerprint during `warmup_cache()`
- Shared across all daemon connections via module-level `OnceLock`
- Subsequent `task_pipeline` calls: 1 query embed plus cached vector scoring
- Skill content truncated to 300 chars for embedding

### Hybrid Vector Search

```
task_pipeline(query)
  ├─ embed query: "query: ..." → q_vec (384-dim)
  ├─ for each skill: normalized dot product with cached_passage[i]
  ├─ keyword boost:
  │   ├─ name exact match:  +0.5
  │   ├─ name contains:     +0.3
  │   └─ word in name:      +0.1
  └─ sort → top 8 → LLMSelector::rerank()
```

### Cross-Encoder Reranker

Second-stage re-ranking using a single-output regression cross-encoder:

```rust
struct CrossEncoder {
    model: BertModel,      // MiniLM-L-6 backbone
    classifier: Linear,    // [1, 384] single-output head (not 2-class)
}
```

- Scores: relevance logit from `narrow(1, 0, 1)` (single label)
- Fallback: keyword-frequency ranking if cross-encoder fails

### `--setup` Pre-download

```bash
agent-guidance --setup
  ├─ Configure MCP clients in IDE configs
  └─ download_models()
      ├─ hf_hub: intfloat/multilingual-e5-small → ~/.cache/huggingface/
      └─ hf_hub: cross-encoder/ms-marco-MiniLM-L-6-v2 → ~/.cache/huggingface/
```

Both models are cached on disk by `hf-hub`. The `--setup` flag pre-downloads them so the first MCP session doesn't wait for network.

---

## Priority Gate (2 layers)

```
Tool call
  └─ can_call_tool(name, state)
      ├─ priority_gate_passed? → pass
      └─ blocked → return WORKFLOW_STAGE_BLOCKED

task_pipeline call
  └─ priority_gate_pass()
      └─ Unlocks gate for subsequent calls
```

### Tool Gate Status

| Tool | Gate | Notes |
|---|---|---|
| `task_pipeline` | ✅ Unlocks | Sets `priority_gate_passed = true` |
| `guidance` | 🔒 Gated | Blocked before `task_pipeline` |
| `project_context` | 🔒 Gated | Blocked before `task_pipeline` |
| `ui_ux` | 🔒 Gated | Blocked before `task_pipeline` |
| `session_continuity` | 🔒 Gated | Blocked before `task_pipeline` |
| `workflow_gate` | 🔒 Gated | Blocked before `task_pipeline` |
| `require_edit_approval` | ✅ Open | Delegates to workflow stage check |
| `usage_report` | ✅ Open | — |
| `health_check`, `diagnose`, `token_stats` | ✅ Open | Whitelisted |

---

## Key Flows

### Tool Call Flow

```
AI calls tool
  ├─ can_call_tool(name, arguments)
  │   ├─ priority_gate_passed? → pass
  │   └─ blocked → WORKFLOW_STAGE_BLOCKED
  ├─ handle_request(method, params, &mut state)
  │   └─ match method
  │       ├─ "initialize" → load models, warm up, return capabilities
  │       ├─ "tools/list"  → return tool list
  │       ├─ "tools/call"  → handle_tool_call(name, arguments, state)
  │       └─ "resources/*" → serve resources
  └─ write JSON-RPC response
```

### Task Pipeline

```
task_pipeline(task, project_path, phase)
  ├─ detect_project_path() → resolve workspace root
  ├─ detect_project_architecture() → Clean_Arch / Layered / Feature / CLI
  ├─ query SQLite code_graph.db → fast-path file count (<1ms)
  ├─ generate_dynamic_blueprint() → upfront modularity blueprint (<300 LOC)
  ├─ get_semantic_relevant_learnings() → inject past session learnings
  ├─ get_phase_rules() → phase-targeted execution mandates
  └─ unlock priority gate (PASSED)
```

### 3-Tier Search Fallback

```
project_context(search, query)
  ├─ FTS5 (SQLite full-text index)
  ├─ Documentation + manifests
  ├─ Structural + config files
  └─ General code files (capped)
```

---

## Database & Metrics Subsystem (`usage.db`)

Every MCP tool invocation, skill activation, and ML vector search is logged to `~/.agent-guidance/usage.db` via `src/mcp/db.rs` using a process & thread mutex guard (`DB_MUTEX`).

### Database Schema

| Table | Purpose | Retention |
|---|---|---|
| `tool_calls` | Logs individual tool invocations (`tool_name`, `operation`, `started_at`, `duration_ms`, `tokens_original`, `tokens_optimized`, `error_message`) | Pruned after 24h |
| `skill_loads` | Logs skill views/reads (`skill_id`, `loaded_at`) | Pruned after 24h |
| `embed_queries` | Logs text vector embeddings (`query`, `created_at`) | Pruned after 24h |
| `llm_queries` | Logs LLM cross-encoder rerank queries | Pruned after 24h |
| `daily_summaries` | Permanent ISO date (`YYYY-MM-DD`) aggregate totals (`tool_calls`, `skills_loaded`, `embed_queries`, `tokens_original`, `tokens_optimized`) | Permanent (Lifetime) |

### 24-Hour Raw Log Auto-Pruning & 50-Item Capping
- **Auto-Pruning**: On every database write cycle (`log_tool_call`, `log_skill_load`, `log_embed_query`), raw records older than **24 hours** (`started_at < now - 86400`) are automatically deleted.
- **50-Item View Capping**: All list queries (`recent_actions`, `tool_breakdown`, `top_skills`, `embed_recent`) are hard-capped to `LIMIT 50`.

### Multi-Timeframe Dashboard Aggregations
The dashboard server (`src/dashboard.rs`) aggregates metrics dynamically across 4 standard timeframes:
- **`past_24h`**: Sum of active 24-hour raw event logs (`WHERE started_at >= cutoff_24h`).
- **`last_7d`**: Aggregated sum from `daily_summaries` (`WHERE day >= cutoff_7d`).
- **`last_30d`**: Aggregated sum from `daily_summaries` (`WHERE day >= cutoff_30d`).
- **`lifetime`**: Permanent sum of all records in `daily_summaries`.

---

## Deployment

### Setup

```bash
agent-guidance --setup
  ├─ configure_mcp_clients() → register in IDE configs
  ├─ configure_global_rules() → append AGENTS.md rules
  ├─ configure_workspace_rules() → append tagged blocks to .cursorrules, etc.
  ├─ configure_skills_enforcer() → write SKILL.md to skill dirs
  └─ download_models() → pre-cache BERT + cross-encoder from HuggingFace
```

### CLI Flags

| Flag | Action |
|---|---|
| `--setup` | Register MCP clients + pre-download models + sync skills |
| `--upgrade` | Download & install latest release package from GitHub |
| `--self-update` | Alias for --upgrade |
| `--session-start` | Pass priority gate (for hooks) |
| `--re-gate` | Re-pass priority gate (subagent recovery) |
| `--uninstall` | Remove all registrations + rules |
| `--force-daemon` | Start as daemon (skip auto-detect) |
| `--force-client` | Connect as proxy (fail if no daemon) |
| `--dashboard` | Start HTTP usage dashboard |
| `--project-path` | Specify project root for --session-start |

### Uninstall

```bash
agent-guidance --uninstall
  ├─ remove_mcp_clients() → delete from IDE configs
  ├─ remove_global_rules() → strip tagged blocks
  └─ remove_workspace_rules() → strip tagged blocks
```

All rule/skill sections use HTML-comment tags (`<!-- agent-guidance:start -->` / `<!-- agent-guidance:end -->`) for reliable find-and-replace.

---

## Related

- [MCP Surface](reference/mcp-surface.md) — full tool/resource reference
- [Development Guide](development.md) — tests, project structure, maintainer
- [Installation](installation.md) — automatic and manual setup
- [README](../README.md) — project overview
