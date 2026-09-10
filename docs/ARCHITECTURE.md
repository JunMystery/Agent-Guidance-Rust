# Architecture

[Back to README](../README.md)

## Overview

Agent Guidance MCP is a **100% Native Rust 2024 Edition** MCP server that gives AI coding agents standards guidance, skill references, workflow prompts, and bounded project code context. It runs as a **ref-counted singleton daemon** with a 60s idle cooldown timer. To prevent dropping models prematurely, an **IDE Process Detector** continuously monitors the OS process table for 26 IDE binaries (VS Code, Cursor, Antigravity, Windsurf, Claude, etc.) — keeping models warm in memory as long as any developer IDE remains open.

---

## Rust Module Map

The codebase strictly enforces Clean Architecture and a mandatory **< 300 LOC hard cap per file**:

```
src/
├── main.rs            # Binary entrypoint — CLI flags, auto-detects daemon/proxy mode
├── daemon/            # Singleton Daemon & IPC Subsystem (< 300 LOC each)
│   ├── lifecycle.rs   # Ref-counted client connection tracking, 60s cooldown timer
│   ├── ide_detector.rs# Process scanner for 26 IDEs (prevents premature teardown)
│   ├── server.rs      # Named pipe (Windows) / Unix socket server dispatch
│   ├── handler.rs     # Worker thread pool (32 concurrent permits, zero-alloc bypass)
│   ├── spawn.rs       # Detached spawn (Windows WMI breakaway + Unix nohup)
│   ├── tray.rs        # Windows taskbar system notification tray icon
│   └── lock.rs        # File-lock singleton mutex guard
├── catalog/           # Skills Catalog & Architecture Blueprints
│   ├── store.rs       # 279 embedded skills (rust_embed) + workspace .agents/skills scanning
│   ├── blueprint.rs   # Upfront dynamic decomposition blueprints for 6 architecture patterns
│   ├── rules.rs       # Tech stack & language-specific micro-rulesets
│   └── mod.rs
├── context/           # Project Context, AST Analysis & GraphRAG Engine
│   ├── scanner.rs     # Bounded workspace scanner (max_depth=3 default, .gitignore filters)
│   ├── db.rs          # SQLite code_graph.db (FTS5 symbols, call edges, AST metadata)
│   ├── graph_rag/     # Hierarchical Leiden community clustering & RAG summarization
│   └── hnsw/          # High-performance in-memory vector index for symbol retrieval
├── dashboard/         # Embedded Web Dashboard Server & REST API (< 300 LOC each)
│   ├── stats.rs       # GET /api/stats with 2s TTL in-memory cache
│   ├── stats_query.rs # SQLite queries for 24h summaries, tool breakdown, phase cadence
│   ├── graph.rs       # GET /api/graph code graph serializer & Mermaid DAG exporter
│   ├── projects.rs    # GET /api/projects multi-repository registry with active/missing disk checks
│   ├── logs_api.rs    # GET /api/logs paginated diagnostic log query & clear endpoint
│   └── mod.rs         # tiny_http worker pool, asset router, port allocator (11997)
├── dashboard_src/     # Dashboard Frontend SPA (HTML5, Vanilla ES Modules, CSS, SVG)
├── mcp/               # Model Context Protocol Engine (< 300 LOC each)
│   ├── router/        # MCP JSON-RPC protocol router & tool schema definitions
│   ├── state/         # ServerState, stage state machine, 31-keyword multi-lingual approval
│   ├── tools/         # 6 core tool dispatchers (pipeline, skills, guidance, context, gate, continuity)
│   ├── db.rs          # SQLite usage.db telemetry logging & daily aggregations
│   └── mod.rs
├── ml/                # Machine Learning & Vector Search Engine
│   ├── embeddings/    # Candle BERT (multilingual-e5-small) + cached passage vectors
│   ├── cross_encoder.rs# MiniLM cross-encoder reranker
│   └── mod.rs
└── optimizer/         # Token Optimization Engine
    ├── compressor.rs  # Language-aware token compressor & whitespace stripper
    ├── skeleton.rs    # AST code skeletonizer (folds function bodies to line ranges)
    └── mod.rs
```

---

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
| `task_pipeline` | ✅ Unlocks | Unlocks priority gate and transitions stage Context → Plan |
| `select_skills` | ✅ Ungated | Whitelisted; confirm and inject skills into context |
| `workflow_gate` | ✅ Ungated | Whitelisted governance & state machine tool |
| `session_continuity` | ✅ Ungated | Whitelisted session persistence & learnings |
| `guidance` | 🔒 Gated | Blocked before `task_pipeline` (`PRIORITY_REQUIRED`) |
| `project_context` | 🔒 Gated | Blocked before `task_pipeline` (`PRIORITY_REQUIRED`) |
| `ui_ux` | 🔒 Gated | Blocked before `task_pipeline` (`PRIORITY_REQUIRED`) |

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
  ├─ detect_project_architecture() → Clean_Architecture / Layered_Architecture / Package_By_Feature / Orchestrator / CLI_Pipeline / Flat_Library
  ├─ query SQLite code_graph.db → fast-path file count (<1ms)
  ├─ generate_dynamic_blueprint() → upfront modularity blueprint (<300 LOC target)
  ├─ get_semantic_relevant_learnings() → inject past session learnings
  ├─ get_phase_rules() → phase-targeted execution mandates
  ├─ evaluate approval keywords → auto-transition Plan -> Build if approved
  └─ unlock priority gate (PASSED)
```

### 6-Phase Instant Cascade Search (< 100ms)

```
project_context(search, query)
  ├─ Phase 1: Alias Cache (<1ms, instant symbol/path hit)
  ├─ Phase 2: Symbol FTS5 (<5ms, SQLite trigram & exact symbol index)
  ├─ Phase 3: Symbol Vector Search (<50ms, in-memory HNSW cosine similarity)
  ├─ Phase 4: Content Chunk FTS5 (<5ms, full-text snippet matches)
  ├─ Phase 5: RAG Content Vectors (<100ms, chunk embeddings)
  └─ Phase 6: Linked Sibling Projects (cross-workspace dependencies)
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

### MCP Crash & Diagnostic Logging Subsystem (`mcp_logger.rs`)
To ensure total runtime observability and instant root-cause identification:
- **Panic Hook Interception**: Global `std::panic::set_hook` intercepts panics, capturing exact source locations `[file:line:column]`, payload strings, and complete unmasked backtraces via `std::backtrace::Backtrace::force_capture()`.
- **Double-Write Resilience**:
  1. High-speed indexed persistence in the `mcp_logs` SQLite table.
  2. Unbuffered hardware sync (`sync_all()`) to emergency fallback file `~/.agent-guidance/logs/crash.log` (rotating cap: 5 files x 2 MB).
- **Multi-Tier Retention Policy**:
  - `CRASH`: **30 days** retention (critical post-mortem).
  - `ERROR`: **14 days** retention (tool timeouts, IO failures).
  - `WARN`: **7 days** retention (token/file boundaries, STDIO disconnects).
  - `INFO`: **3 days** retention (debug diagnostics).
  - Hard rolling quota of **10,000 entries** managed via FIFO cleanup.
- **Web Dashboard Diagnostics View (`#logs`)**:
  - Dedicated `⚠️ Diagnostics` view exposing live KPI status cards, level filters, real-time message/stacktrace search, and an expandable drawer to inspect complete runtime stack traces.

---

## MCP Daemon & Cross-Platform System Tray Architecture

### Multi-IDE Lifecycle & Decoupled Process
The MCP daemon is fully decoupled from the launching IDE process to ensure zero interruption across multi-IDE workflows:
- **Windows Job Object Breakaway**: Spawned with `CREATE_BREAKAWAY_FROM_JOB | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`. Fallback to WMI `Win32_Process::Create` ensures the daemon is parented directly under the Windows OS WMI service (`WmiPrvSE.exe`), immune to IDE parent tree teardown.
- **Unix Process Group Detachment**: Configured with `process_group(0)` to prevent `SIGHUP` cascade when the launching IDE closes.
- **Active IDE Detection Engine (`ide_detector.rs`)**: Scans OS process table via `sysinfo` for all major IDE binaries (VS Code, Cursor, Antigravity, Windsurf, Claude Desktop, Trae, Zed, JetBrains suite, Visual Studio).
- **Graceful Multi-Client Lifecycle (`lifecycle.rs`)**: If `ACTIVE_CLIENTS` drops to 0, the daemon queries `has_running_ide_processes()`. As long as ANY IDE remains active, the daemon stays alive indefinitely. When 0 IDEs remain, a 60-second cooldown buffer runs; if an IDE is reopened during cooldown, shutdown is immediately cancelled.

### Cross-Platform System Tray (`tray.rs`, `tray_windows.rs`, `tray_unix.rs`)
A native system tray icon runs in a dedicated background thread:
- **Windows (`tray_windows.rs`)**:
  - Implements native Win32 `Shell_NotifyIconW`, hidden message pump window, and `TrackPopupMenu`.
  - **Desktop Session Attachment**: Automatically bridges thread context to `"Default"` interactive desktop via `OpenDesktopW` and `SetThreadDesktop`, ensuring tray visibility even when spawned from IDE sandboxed desktop stations (`exebox-...`).
- **macOS & Linux (`tray_unix.rs`)**:
  - macOS uses Cocoa `NSStatusBar` native menus.
  - Linux uses pure Rust D-Bus `StatusNotifierItem` via `ksni` (zero C library dependency, eliminating `libdbus`/`libappindicator` system package requirements).
  - Gracefully disables in headless environments (`DISPLAY` / `WAYLAND_DISPLAY` missing).
- **Application Icon Embedding**:
  - `build.rs` compiles `docs/images/logo.png` directly into Windows PE resources via `winres`.
- **System Tray Actions**:
  - Status indicator (`Agent Guidance (Running)`).
  - `Open Web Dashboard`: Launches default browser to `http://127.0.0.1:{port}/#dashboard`.
  - `Open Data Folder`: Opens local cache/data directory in native file manager.
  - `Exit MCP Daemon`: Clean shutdown of daemon process.

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
