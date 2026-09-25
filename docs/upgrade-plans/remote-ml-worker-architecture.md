# Implementation Plan: Remote ML Worker & Centralized Skill Registry Architecture

## 1. Executive Summary
Split `agent-guidance` workloads into a dual-role topology:
- **Ultra-lightweight Client (~15 MB RAM)** on developer devices:
  - **Zero ML Models & Zero Vector Binaries**: `skills.bin`, `vectors.bin`, and neural models are **ABSENT** from the client machine. The client has no local ML inference capability.
  - Dedicated strictly to local AST code analysis (`tree-sitter`), project file reads, and edit authorization guards.
  - Transparently delegates all semantic skill searches and section slicing queries to the Remote ML Worker.
- **Dedicated Remote Server (~600–800 MB RAM)**:
  - **Sole Custodian of Vector Binaries**: The server alone compiles, holds, and executes queries against `skills.bin` and `vectors.bin`.
  - **Runtime Binary Exclusivity**: Server answers queries 100% in-memory from compiled binary bundles (`skills.bin` + `vectors.bin`). Zero live markdown parsing or disk sweeps during inference.
  - **Staging Directory Workflow**: Filesystem folder (`~/.agent-guidance/staging/skills/`) serves strictly on the server as an authoring staging area.
  - **Unlimited Dynamic Capacity ($N \ge 279$)**: Zero hardcoded skill limits. Supports hundreds or thousands of community and enterprise skills.
  - **Upsert & Deletion Semantics**: Matching slugs overwrite in-place; new slugs append; 1-N skills can be deleted/cleaned directly from the Web Dashboard.
  - **Binary Compaction Engine**: Prunes vector rows from `vectors.bin` and text records from `skills.bin`, hot-swapping in memory in <1ms without restart.
  - **SHA-256 Incremental Binary Reindex Compiler**: Compiles staged skills into the binary database in <150 ms using differential hashing.
  - **Vector-Ranked Section-Level RAG Slicing**: Slices skills by `##` headers, delivering only top 2-3 sections (30–60 lines, ~350 tokens) to slash token consumption by ~80%.

---

## 2. Client Connection Configuration System

Client connects to remote server via `~/.agent-guidance/config.toml`, environment variables, or CLI commands.

### 2.1 Configuration File (`~/.agent-guidance/config.toml`)
```toml
[server]
# Operation mode: "local" (standalone, in-process ML) | "remote" (dedicated worker)
mode = "remote"

# Protocol: "http" | "https" | "ssh"
protocol = "http"

# HTTP/HTTPS configuration
url = "http://172.16.11.24:11998"
api_key = ""                  # Optional Bearer token for protected endpoints
timeout_ms = 3500             # Max wait time before triggering fallback

# Optional SSH tunnel configuration (when protocol = "ssh")
[server.ssh]
host = "172.16.11.24"
user = "jun"
port = 22
identity_file = "~/.ssh/id_ed25519"
remote_port = 11998

[resilience]
# If remote server is unreachable, fall back to local FTS5 keyword search without crashing
fallback_to_local = true
# Cache remote embeddings locally in SQLite to reduce network round-trips
cache_vectors = true
```

### 2.2 Client Resilience & Silent Fallback Engine
When connecting to the remote ML worker:
- **Primary Route**: HTTP POST to remote worker endpoint (e.g. `http://172.16.11.24:9000/api/skills/search`).
- **Failover Route (Silent FTS5 Fallback)**:
  - If request exceeds `timeout_ms` (default 3500ms) or connection drops, client logs a warning to stderr:
    `[WARN] Remote ML worker (172.16.11.24:9000) unreachable. Silently falling back to local FTS5 keyword index.`
  - Continues processing query locally via SQLite FTS5 index without failing or interrupting the agent tool loop.
  - As soon as the remote server recovers, the next query automatically resumes remote neural embeddings.
- **First-Run Behavior (Unconfigured Server)**:
  - If client binary is launched without an active server configuration, it immediately functions in Local FTS5 mode.
  - Displays a non-blocking informational notice:
    `[NOTICE] Remote ML worker unconfigured. Operating in local FTS5 keyword mode. Run 'agent-guidance --set-server <URL>' anytime.`

### 2.3 Catalog Hash ETag & Hash-Invalidated LRU Cache
- **32-Byte Catalog ETag**:
  - Server includes a 32-byte SHA-256 `catalog_hash` (or `ETag`) in every response header:
    `ETag: "a3f5b72e91c0..."`
  - Client caches the catalog hash locally. On subsequent queries, client sends:
    `If-None-Match: "a3f5b72e91c0..."`
  - If unchanged, server skips catalog serialization and returns `304 Not Modified` (< 1 ms).
- **Client Hash-Invalidated LRU Cache (`usage.db`)**:
  - Client stores remote embeddings and top search results locally in SQLite.
  - Cache entries are tagged with `catalog_hash`.
  - As long as server's `catalog_hash` is unchanged, repeat queries resolve instantly from local disk (< 2 ms) without network round-trips.
  - When the server reindexes, modifies, or deletes skills, the new `catalog_hash` immediately invalidates stale cache entries.

### 2.4 Bearer Token Security Model
- When the remote worker binds to `0.0.0.0`, all API endpoints require authentication via Bearer token:
  `Authorization: Bearer <api_key>`
- Configured in `~/.agent-guidance/config.toml` (client) and `~/.agent-guidance/server.toml` (server).
- Requests with invalid or missing tokens receive `401 Unauthorized` with JSON error payload.

### 2.3 CLI Configuration Commands
- Set server endpoint:
  ```bash
  agent-guidance --set-server http://172.16.11.24:11998
  ```
- Test server connectivity & latency:
  ```bash
  agent-guidance --test-server
  # Output: [OK] Connected to 172.16.11.24:11998 | Latency: 8.4 ms | Models: Candle BERT (VRAM)
  ```
- Switch back to local standalone mode:
  ```bash
  agent-guidance --set-server local
  ```

### 2.4 Dynamic Client Dashboard Auto-Configuration (`http://127.0.0.1:11997`)

Servers have arbitrary IPs, hostnames, and ports (`172.16.11.24`, `192.168.1.100`, `ai-server.lan:11998`, etc.).
The installation script in Client Mode installs the lightweight binary and immediately opens or surfaces the **Client Dashboard** (`http://127.0.0.1:11997`).

- **Server Connection Setup Card**:
  - **Server Endpoint Input**: Arbitrary IP, hostname, or domain with port (e.g. `http://172.16.11.24:11998`, `https://ai-node.internal:11998`).
  - **Protocol Selector**: `HTTP` / `HTTPS` / `SSH Tunnel`.
  - **API Key / Secret Token**: Optional Bearer token for protected environments.
  - **Timeout & Resilience**: Configurable timeout (default: 3500ms) and local FTS5 fallback checkbox.
- **Interactive Actions**:
  - **"Test Connection" Button**:
    - Sends probe to `GET http://<entered-ip>:<port>/health`.
    - Renders live latency & model status pill:
      `[Connected] 172.16.11.24:11998 | Latency: 7.4 ms | Models: Candle BERT (VRAM) | 279 Skills Loaded`
  - **"Apply & Auto-Configure" Button**:
    - Automatically persists settings into `~/.agent-guidance/config.toml`.
    - Automatically scans and updates IDE MCP configs (VS Code `mcp.json`, Claude Code `~/.claude.json`, Cursor, Windsurf, Zed) so IDEs point to local client binary.
    - Hot-swaps remote connection inside running client daemon immediately (zero daemon or IDE restart required).
    - Displays confirmation: *"Remote Server 172.16.11.24:11998 active. IDE configurations verified."*

### 2.6 Client Dashboard Custom Port Configuration

If port `11997` is occupied or developer prefers a custom port (e.g. `12000`, `8080`):

1. **CLI Flag**:
   ```bash
   agent-guidance --dashboard --port 12000
   ```
### 2.5 Headless Server Administration & Custom Port Configuration

Servers often have firewall constraints or port conflicts requiring custom ports instead of defaults (`11997` / `11998`).

#### Custom Port & Host Resolution Hierarchy
Custom ports and bind addresses can be configured through three priority levels:
1. **CLI Flags** (Highest priority):
   ```bash
   # Bind both dashboard and worker API to custom ports and open interface
   agent-guidance --server --bind 0.0.0.0 --port 8080 --worker-port 9000
   ```
2. **Environment Variables**:
   - `AGENT_GUIDANCE_BIND_ADDR` (e.g. `0.0.0.0` or `192.168.1.50`)
   - `AGENT_GUIDANCE_DASHBOARD_PORT` (e.g. `8080`)
   - `AGENT_GUIDANCE_WORKER_PORT` (e.g. `9000`)
3. **Configuration File (`~/.agent-guidance/config.toml`)**:
   ```toml
   [server]
   bind = "0.0.0.0"
   worker_port = 9000      # Custom ML Worker port
   dashboard_port = 8080   # Custom Dashboard/Admin port
   api_key = "optional-secret-key"
   ```

#### Path A: Network-Accessible HTML Dashboard (`--bind 0.0.0.0`)
- Accessible from any client browser: `http://<server-ip>:<custom_port>` (e.g. `http://172.16.11.24:8080`).
- Web UI displays active worker port, connection string for clients, live telemetry, and staging controls.

#### Path B: Interactive Terminal CLI Setup Wizard (`agent-guidance --setup-server`)
- Interactive setup prompts for custom ports:
  ```text
  ? Listen Address [0.0.0.0]: 0.0.0.0
  ? ML Worker API Port [11998]: 9000
  ? Admin Dashboard Port [11997]: 8080
  ? Optional Bearer API Key: [none]
  ? Preload models now? (Y/n): y
  ? Enable systemd auto-start service? (Y/n): y
  ```
- Generates systemd service with exact custom ports and bind address:
  `ExecStart=/home/jun/.local/bin/agent-guidance --server --bind 0.0.0.0 --port 8080 --worker-port 9000`

### 2.6 Client Dashboard Custom Port Configuration

If port `11997` is occupied on client or developer prefers a custom port (e.g. `12000`, `8080`):

1. **CLI Flag**:
   ```bash
   agent-guidance --dashboard --port 12000
   ```
2. **Environment Variable**:
   ```bash
   export AGENT_GUIDANCE_DASHBOARD_PORT=12000
   ```
3. **Client Configuration File (`~/.agent-guidance/config.toml`)**:
   ```toml
   [dashboard]
   port = 12000          # Custom client dashboard port
   bind = "127.0.0.1"    # Default loopback
   auto_open = true      # Automatically launch browser on startup
   ```
4. **Client HTML GUI (Settings Tab)**:
   - Developer changes port field from `11997` to `12000` $\rightarrow$ clicks **"Save & Apply"**.
   - Client daemon rebinds listener and browser automatically redirects to `http://127.0.0.1:12000`.
   - System tray icon menu immediately updates its "Open Dashboard" shortcut to the new URL.
5. **Automatic Port Conflict Fallback**:
   - If configured port is already in use (`EADDRINUSE`), daemon logs warning and automatically increments (`11998`, `11999`, etc.), preventing startup failure.

#### Client Configuration with Custom Server Port
- Client connects via CLI:
  ```bash
  agent-guidance --set-server http://172.16.11.24:9000
  ```
- Or through Client Dashboard UI (`http://127.0.0.1:11997`):
  Enter `http://172.16.11.24:9000` in endpoint input $\rightarrow$ click **"Test Connection"** $\rightarrow$ click **"Apply & Auto-Configure"**.

---

## 3. Server Skill Architecture: Server-Exclusive Binary Runtime vs. Staging

**Critical Architecture Invariant**:
- **Client has ZERO ML**: The client installation contains **NO `skills.bin`**, **NO `vectors.bin`**, and **NO neural model weights**. The client cannot run semantic vector search or embedding math locally.
- **Server is Sole Custodian**: The server alone holds the neural models, compiles markdown into `skills.bin`, generates section embeddings in `vectors.bin`, and services all semantic queries.

### 3.1 Staging Directory (`~/.agent-guidance/staging/skills/`)
- Purpose: **Server-side staging area only**.
- Developers or server admins drop custom/updated `SKILL.md` folders into this staging folder on the server.
- Staging directory supports any number of skills ($N \ge 279$).
- **Conflict Resolution (Staged Overrides Built-in)**:
  - If a skill in staging shares the same slug/name as a built-in default skill, the staged custom version **takes absolute precedence**.
  - Binary compilation incorporates the custom staged skill into `skills.bin` and replaces its vectors in `vectors.bin`.
  - When upgrading server binaries, untouched built-in skills receive updates, while custom staged skills remain intact and active.
- The live query engine **NEVER** reads from this staging directory during agent tool calls (`guidance` / `select_skills`). This guarantees zero file lock contention, zero filesystem I/O latency, and absolute crash safety.

### 3.2 Unlimited Dynamic Capacity Specification
- Binary header format: `[count: u32][dim: u32]` (supports up to $4.29 \times 10^9$ entries).
- Offset table in `skills.bin`: `Vec<SkillOffsetEntry>` dynamically scaled to exact count $N$.
- In-memory `GpuSkillMatrix`: Dynamically allocates 2D Tensor shape `[N, 384]` at load time.
- Matrix multiplication: Batch cosine dot product `[1, 384] @ [N, 384]^T -> [N]` scales linearly on CPU/GPU.

### 3.3 Deletion & Binary Compaction Engine (1-N Skills)
1. **Dashboard Trigger**: User selects 1 to $N$ skills and clicks **"Delete Selected"** (with option to also purge from staging).
2. **API Endpoint**: `POST /api/skills/delete` with `{ "names": ["skill-a", "skill-b"], "purge_staging": true }`.
3. **Vector Compaction**: Slices out target vector rows; decrements `count` in binary header.
4. **Content Compaction**: Rebuilds `skills.bin` offset table without deleted entries and repacks contiguous text blocks.
5. **Tombstone Blacklist** (for Built-In Skills): Deleted built-in skills recorded in `tombstones: ["..."]` in `skills_manifest.json` so defaults do not revive.
6. **Atomic Hot-Swap**: Memory pointers swap in `< 1 ms`.

### 3.4 SHA-256 Differential Reindex Compiler (`agent-guidance --reindex-skills`)
1. **Diff Stage**: Compares SHA-256 hashes of all markdown files in `staging/skills/` against `skills_manifest.json`.
2. **Vector Stage**:
   - Unchanged files $\rightarrow$ **0 ms** skip (reuse pre-compiled binary vectors).
   - Modified/New files $\rightarrow$ Run BERT embedding on changed sections only.
   - Deleted files $\rightarrow$ Prune from binary records.
3. **Binary Baking Stage**:
   - Packs compressed markdown text into `skills.bin` (indexed by dynamic offset table).
   - Packs Little-Endian float arrays into `vectors.bin` (`[count: u32][dim: u32][raw bytes]`).
4. **Atomic Hot-Swap**: Writes to `.tmp` files and atomically renames.

### 3.5 Vector-Ranked Section-Level RAG Slicing
- Slices skills by `##` headers and embeds individual sections.
- When an agent selects a skill, the server matches the active task prompt against section embeddings.
- Returns only the Top 2-3 most relevant sections (~350 tokens) instead of the entire 300+ line document.

### 3.6 Bidirectional Skill Stats Synchronization to Client
The server automatically transmits live skill metadata back to connected clients:
- **Server Skill Stats Endpoint (`GET /api/skills/stats`)** (also returned in `/health` and search headers):
  ```json
  {
    "total_skills": 284,
    "total_sections": 1142,
    "vectors_dim": 384,
    "last_reindex_at": "2026-09-25T14:30:22Z",
    "catalog_hash": "a3f5b72e91...",
    "tombstones_count": 2,
    "token_savings_ratio": 0.81
  }
  ```
- **Client Presentation**:
  - **Client CLI (`agent-guidance --stats` / `--status`)**:
    Displays live remote catalog info:
    `Remote Skills: 284 loaded (1,142 sections) | Freshness: Today 14:30 | Server: 172.16.11.24:9000`
  - **Client Dashboard**:
    Renders a dedicated "Remote ML & Skill Registry" telemetry card showing server model, total skills count, and estimated tokens saved from surgical RAG slicing.
  - **Local Persistence**:
    Client caches received stats in local `usage.db` so IDE agents immediately know available skill capacity even during offline or momentary network blips.

### 3.7 Multi-Threaded Server Worker Concurrency (Thread Pool)
- **High-Throughput Parallel Inference**:
  - Handles concurrent queries from multiple developer machines without serial bottleneck.
  - Deploys a worker thread pool (2–4 threads) with cloned ONNX sessions and multithreaded Candle BERT pipelines.
  - Thread pool balances requests dynamically across available CPU cores and GPU VRAM streams.
  - Memory footprint scales to ~600–800 MB on the server, ensuring sub-10ms response latency even during peak multi-developer coding sessions.

---

## 4. Server-Side Dashboard Enhancements (`src/dashboard_src/`)

Dedicated server management views on port `11998` / `11997`:

### 4.1 Skill Registry & Bulk Action Manager
- **Skills Explorer**: Search, filter, and preview all loaded skills (embedded + custom) with section counts and memory sizes.
- **Bulk Selection & Deletion (1-N)**:
  - Checkbox per row + "Select All" toggle.
  - Red **"Delete Selected (N)"** button.
  - Confirmation modal with checkbox: `[x] Also delete files from staging directory`.
  - Instant live removal from table upon confirmation.
- **Restore Default Skills**: Action button to untombstone and restore original built-in 279 catalog.

### 4.2 One-Click Incremental Reindex Trigger
- Live button: **"Compile Staging to Binary (SHA Diff)"**.
- Progress modal:
  - Scanned staged files count
  - Unchanged files skipped
  - Re-embedded sections count
  - Compacted vector count and size on disk (`vectors.bin`)

### 4.3 Server Health & Worker Telemetry
- **RAM / VRAM Gauge**: Real-time memory consumption of Candle BERT and ORT models.
- **Inference Latency Metrics**: Average embedding time per batch (ms) and cross-encoder rerank latency (ms).
- **Connected Client Monitor**: Active client sessions querying the server.

### 4.4 Server Network & Port Configuration Tab (HTML GUI)
A dedicated administrative tab in the Server Dashboard (`http://<server-ip>:<dashboard-port>`):
- **Port & Binding Controls**:
  - **Worker API Port Input**: Text field for custom port (e.g. `9000`, default `11998`) with inline port availability test.
  - **Admin Dashboard Port Input**: Custom dashboard port (e.g. `8080`, default `11997`).
  - **Network Bind Address Dropdown**: `0.0.0.0 (All Interfaces - LAN/WAN)` vs `127.0.0.1 (Loopback / SSH Tunnel Only)` vs custom interface IP.
  - **Bearer API Key**: Field to view, set, or auto-generate a secret token for securing worker endpoints.
- **Action Buttons**:
  - **"Save & Rebind" Button**:
    - Updates `~/.agent-guidance/config.toml` (or `server.toml`).
    - Dynamically hot-rebinds the ML Worker HTTP listener to the new custom port without dropping model weights in VRAM.
  - **"Client Connection Helper" Card**:
    - Automatically displays the exact connection command for developers:
      `agent-guidance --set-server http://172.16.11.24:9000`
    - One-click copy button for easy sharing with team members.

---

## 5. Dual-Role Architecture Specification

```
Personal Developer Device (Client)             Remote Server (Worker Daemon)
┌───────────────────────────────────────┐      ┌───────────────────────────────────────┐
│ • ZERO ML Models & ZERO Vector Bins   │      │ • SOLE CUSTODIAN of Binary Vectors:   │
│   (skills.bin & vectors.bin ABSENT)   │      │     vectors.bin & skills.bin (<1ms)   │
│ • Tree-sitter AST & Graph Engine      │      │ • Multi-Threaded Worker Pool (2-4x)   │
│ • Local SQLite (.agent-context/*.db)  │      │ • Candle BERT Embedder in VRAM/RAM    │
│ • Config: ~/.agent-guidance/config.toml│     │ • ONNX Cross-Encoder Reranker         │
│ • File reading & Diff Guards          │      │ • Staging Directory (Unlimited N):    │
│ • RAM: ~15 MB                         │      │     ~/.agent-guidance/staging/skills/ │
│ • Local Dashboard (Code Graph/Settings│      │ • Binary Compactor (1-N Deletion)     │
│ • Auto Task Context Forwarding        │      │ • SHA-256 Compiler & Hot-Swap Engine  │
│                                       │      │ • RAM: ~600–800 MB                    │
│                                       │      │ • Server Dashboard (Registry & Bulk)  │
└──────────────────┬────────────────────┘      └──────────────────▲────────────────────┘
                   │                                              │
                   │ (HTTP / JSON-RPC over TCP / Unix / SSH)       │
                   ├─────────── POST /api/skills/search ──────────┤
                   │            Prompt query -> Ranked skills     │
                   │                                              │
                   ├─────────── POST /api/skills/slice ───────────┤
                   │            Skill + Prompt -> Top 2-3 Sections│
                   │                                              │
                   ├─────────── POST /api/skills/delete ──────────┤
                   │            Delete 1-N skills + Compact Bin   │
                   │                                              │
                   ├─────────── POST /api/skills/reindex ─────────┤
                   │            Compile staging -> binary hot-swap│
                   │                                              │
                   ├─────────── GET  /api/skills/stats ───────────┤
                   │            Catalog size, sections, freshness │
                   │                                              │
                   ├─────────── POST /api/embed ──────────────────┤
                   │            Batch code chunks -> [f32; 384]   │
                   │                                              │
                   └─────────── GET  /health ─────────────────────┘
                                Ping latency & model status
```

---

## 6. Work Breakdown Structure (WBS)

### Phase 1: Client Configuration Subsystem (`src/config/`)
- [ ] Create `src/config/mod.rs` & `src/config/schema.rs`:
  - Parse and serialize `~/.agent-guidance/config.toml`.
  - Support arbitrary server endpoint URL/IP and custom ports (`url = "http://<ip>:<custom_port>"`), protocol, API key, timeout, local fallback.
  - Support environment variable overrides.
- [ ] Add CLI flags in [src/main.rs](src/main.rs):
  - `agent-guidance --set-server <URL|local>`
  - `agent-guidance --test-server`
  - `agent-guidance --stats` / `--status` (print client status, ping latency, and remote skill stats)
- [ ] Expose configuration read/write in local dashboard API (`/api/client/config`).

### Phase 2: Staging Directory, Binary Compiler & Compactor
- [ ] Create staging directory `~/.agent-guidance/staging/skills/`.
- [ ] Implement `src/catalog/checksum.rs`:
  - Calculate SHA-256 per staged markdown file.
  - Track `skills_manifest.json` with hash, sections, and tombstone list.
- [ ] Build Binary Compactor & Compiler in `src/ml/embeddings/compiler.rs`:
  - Dynamically scale offset table for arbitrary skill count $N$.
  - Compile markdown text into dynamic `skills.bin` offset table.
  - Compile section embeddings into Little-Endian `vectors.bin` (`[count: u32][dim: u32][raw bytes]`).
  - Implement 1-N deletion & compaction: slice out vector rows, pack remaining entries, decrement count.
  - Implement tombstone filter: prevent deleted built-in skills from reviving.
- [ ] Update runtime loader (`src/ml/embeddings/precomputed.rs`):
  - Server runtime loads EXCLUSIVELY from `skills.bin` and `vectors.bin`. Zero reads from staging during query answering.

### Phase 3: ML Worker Daemon REST API
- [ ] Implement `src/ml/worker/` HTTP JSON-RPC handler on port `11998` (with configurable `--bind` and `--worker-port`):
  - `POST /api/skills/search`: Search in-memory binary vector matrix.
  - `POST /api/skills/slice`: Vector-rank and return top 2-3 sections from binary content.
  - `POST /api/skills/delete`: Prune 1-N skills, compact binary files, and hot-swap memory.
  - `POST /api/skills/reindex`: Trigger compiler to hot-swap binary in memory.
  - `POST /api/embed`: Return BERT embeddings for code chunks.
  - `GET /api/skills/list`: Return registered skills, metadata, and tombstone status.
  - `GET /api/skills/stats`: Return total skills count $N$, total sections, catalog SHA hash, last reindex timestamp, tokens saved ratio.
  - `GET /health`: Return server memory, VRAM, and model status.

### Phase 4: Client Auto Task Context Forwarding & Remote Adapter
- [ ] In `src/mcp/state.rs`: Cache active task query from `task_pipeline`.
- [ ] In `src/mcp/tools/skills.rs`: Auto-forward cached task to remote `/api/skills/slice`.
- [ ] Ensure injected context delivers only top 2-3 sections (30–60 LOC) instead of entire SKILL.md.
- [ ] Synchronize and cache server skills stats locally in `usage.db`:
  - Update cached skills count, sections, catalog hash, and token savings ratio.
  - Display remote skills stats in client CLI `--stats` and Client Dashboard.

### Phase 5: Server & Client Dashboard UI
- [ ] Client Dashboard (`11997`): Add Connection Settings tab with Test Ping button.
- [ ] Server Dashboard (`11998`):
  - Skill Registry with 1-N bulk checkboxes and red "Delete Selected" button.
  - "Compile Staging to Binary (SHA Diff)" action button with progress modal.
  - Telemetry: VRAM / RAM profile, inference latency, connected clients.

### Phase 6: Installer Split
- [ ] Update `scripts/install.sh` and `scripts/install.ps1`:
  - `[1] Full Standalone`
  - `[2] Lightweight Client` (prompts for server URL, sets `~/.agent-guidance/config.toml`)
  - `[3] Dedicated Server Worker` (creates staging directory, initializes default binary, sets up systemd unit).
