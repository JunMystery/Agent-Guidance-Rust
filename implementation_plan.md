# Implementation Plan: Remote ML Worker & Centralized Skill Registry

Detailed Specification: [docs/upgrade-plans/remote-ml-worker-architecture.md](docs/upgrade-plans/remote-ml-worker-architecture.md)

## Objective
Split `agent-guidance` workloads to support:
1. **Ultra-lightweight Client (~15 MB RAM)**:
   - **Zero Local ML & Zero Vector Binaries**: `skills.bin` and `vectors.bin` are **NOT** installed or stored on the client. The client has no local ML models or neural vector execution.
   - Runs local AST code indexing (`tree-sitter`), stage gate enforcement, and tool proxying.
   - Forwards all skill queries and task context to the remote server over HTTP.
2. **Dedicated Server Worker (~600–800 MB RAM)**:
   - **Sole Custodian of `skills.bin` & `vectors.bin`**: Only the server compiles, loads, and queries the binary skill and vector databases.
   - **Runtime Binary Exclusivity**: Queries read 100% in-memory from compiled binary packages (`skills.bin` + `vectors.bin`). Zero live markdown parsing.
   - **Multi-Threaded Worker Pool**: Serves concurrent queries from multiple client developers simultaneously.
   - **Unlimited Dynamic Capacity ($N \ge 279$)**: Zero hardcoded skill limits.
   - **1-N Skill Cleaning & Binary Compaction**: Delete 1-N skills directly from Dashboard or CLI; compacts `.bin` files and hot-swaps in memory (< 1 ms).
   - **Tombstones**: Prevents deleted built-in skills from reviving.
   - **Staging Directory**: `~/.agent-guidance/staging/skills/` is purely on the server for staging new/modified skills (staged skills override built-in defaults).
   - **SHA-256 Differential Compiler**: `agent-guidance --reindex-skills` compiles staging into the binary bundle in <150 ms.
   - **Vector-Ranked Section-Level RAG Slicing**: Slices skills by `##` headers, delivering only top 2-3 sections (30–60 lines, ~350 tokens) to slash token consumption by ~80%.

---

## Proposed Changes

### Milestone 1: Client Connection Configuration Subsystem & Dashboard Setup
- [ ] Create `src/config/mod.rs` & `src/config/schema.rs`:
  - Parse and serialize `~/.agent-guidance/config.toml`.
  - Support arbitrary server endpoint URL/IP and custom ports (`url = "http://<ip>:<custom_port>"`), protocol (`http`|`https`|`ssh`), API key, timeout, local fallback.
  - Support environment variable overrides (`AGENT_GUIDANCE_SERVER_MODE`, `AGENT_GUIDANCE_SERVER_URL`, etc.).
- [ ] Add CLI flags in [src/main.rs](src/main.rs):
  - `agent-guidance --set-server <URL|local>` (accepts custom ports e.g. `http://172.16.11.24:9000`)
  - `agent-guidance --test-server`
  - `agent-guidance --stats` / `--status` (print client status, ping latency, and remote skill stats)
- [ ] Add Client Dashboard configuration UI (`http://127.0.0.1:11997`):
  - Arbitrary Server IP/Hostname input field with custom port support.
  - "Test Connection" button: live ping, latency measurement, and server model/skills status probe.
  - "Apply & Auto-Configure" button:
    - POST `/api/config/server` saves to `~/.agent-guidance/config.toml`.
    - Automatically updates IDE MCP configs across detected IDEs (VS Code, Claude Code, Cursor, etc.).
    - Hot-swaps remote connection inside running client daemon immediately (zero restarts).

### Milestone 2: Staging Directory, Binary Compiler & Compactor
- [ ] Create staging directory `~/.agent-guidance/staging/skills/`.
- [ ] Implement conflict precedence: staged skills in `staging/skills/` override built-in defaults with identical slugs.
- [ ] Implement `src/catalog/checksum.rs` for SHA-256 hash tracking per staged file against `skills_manifest.json`.
- [ ] Build Binary Compactor & Compiler in `src/ml/embeddings/compiler.rs`:
  - Dynamically scale offset table for arbitrary skill count $N$.
  - Compile markdown text into packed `skills.bin` table.
  - Compile section embeddings into Little-Endian `vectors.bin` (`[count: u32][dim: u32][raw bytes]`).
  - Implement 1-N deletion & compaction: slice out vector rows, pack remaining entries, decrement count.
  - Implement tombstone blacklist for deleted built-in skills.
- [ ] Update runtime loader (`src/ml/embeddings/precomputed.rs`):
  - Server runtime loads EXCLUSIVELY from `skills.bin` and `vectors.bin`. Zero reads from staging during live query answering.
- [ ] Support CLI commands:
  - `agent-guidance --reindex-skills [--force]` (recompile staging to binary)
  - `agent-guidance --delete-skill <NAME...>` (prune 1-N skills from CLI)

### Milestone 3: Server ML Worker Daemon REST API & Headless Administration
- [ ] Implement `src/ml/worker/` HTTP JSON-RPC handler (configurable custom port `--worker-port <PORT>`, default `11998`, and `--bind <HOST>`, default `127.0.0.1` / `0.0.0.0`).
- [ ] Implement Multi-Threaded Worker Pool (2-4 threads with cloned ONNX sessions) for parallel high-throughput client query servicing.
- [ ] Expose:
  - `POST /api/skills/search` (Prompt -> Ranked skills list from in-memory binary)
  - `POST /api/skills/slice` (Skill + Prompt -> Top 2-3 vector-ranked sections from binary content)
  - `POST /api/skills/delete` (Delete 1-N skills, compact binary, hot-swap memory)
  - `POST /api/skills/reindex` (Trigger staging $\rightarrow$ binary compile + atomic hot-swap)
  - `POST /api/embed` (Code chunks -> `[f32; 384]`)
  - `GET /api/skills/list` (Skills list, sections, tombstone status)
  - `GET /api/skills/stats` (Total skills count $N$, total sections, catalog SHA hash, last reindexed timestamp, tokens saved ratio)
  - `GET /health` (Server status, VRAM/RAM, uptime)
- [ ] Bidirectional Skill Stats Synchronization to Client:
  - Client CLI `agent-guidance --stats` renders remote catalog size, sections count, and server status.
  - Client Dashboard displays "Remote Skill Registry" stats card.
  - Client caches remote skill stats in local `usage.db`.
- [ ] Headless Server Administration & Custom Ports:
  - CLI flags: `--bind 0.0.0.0`, `--port <PORT>` (custom dashboard, e.g. 8080), `--worker-port <PORT>` (custom worker, e.g. 9000).
  - Environment variables: `AGENT_GUIDANCE_BIND_ADDR`, `AGENT_GUIDANCE_DASHBOARD_PORT`, `AGENT_GUIDANCE_WORKER_PORT`.
  - Interactive Terminal CLI Setup Wizard: `agent-guidance --setup-server` prompts for custom ports, bind address, auth key, model preload, and systemd service generation.
  - CLI management commands: `agent-guidance --server-status`, `agent-guidance --reindex-skills`, `agent-guidance --delete-skill <NAME...>`.

### Milestone 4: Client Auto Task Forwarding & Remote Adapter
- [ ] In `src/mcp/state.rs`: Store active task string during `task_pipeline` call.
- [ ] In `src/mcp/tools/skills.rs`: Automatically forward cached task string to `slice_skill_markdown` / remote slice API when agent omits `task` parameter.
- [ ] Ensure injected context delivers only top 2-3 sections (30–60 LOC) instead of entire SKILL.md.
- [ ] In Client Remote Adapter:
  - Hash-Invalidated LRU cache stored in SQLite (`usage.db`), validated against server `catalog_hash`.
  - Cache received skill stats (`GET /api/skills/stats`) in local SQLite (`usage.db`).
  - Feed remote stats into `agent-guidance --stats` and Client Dashboard telemetry card.
  - First-run fallback: if unconfigured, operate in local FTS5 keyword mode with setup notice.

### Milestone 5: Server Dashboard Staging, Bulk Deletion & Port Setup GUI
- [ ] Add **Skill Registry & Bulk Action Manager** in [src/dashboard_src/](src/dashboard_src/):
  - Multi-select checkboxes for 1-N selection.
  - Red "Delete Selected (N)" button with confirmation modal and staging purge toggle.
  - "Compile Staging to Binary (SHA Diff)" live action button with progress modal.
  - Binary vector file size metrics and server telemetry (VRAM/RAM, inference latency, query throughput).
- [ ] Add **Server Network & Port Configuration Tab (HTML GUI)**:
  - Custom Worker API Port input (e.g. `9000`, default `11998`).
  - Custom Dashboard Port input (e.g. `8080`, default `11997`).
  - Network Bind Address dropdown (`0.0.0.0` vs `127.0.0.1` vs custom IP).
  - Optional Bearer API Key input / token generator.
  - "Save & Rebind" button: persists to `config.toml` and hot-rebinds listener without reloading models.
  - "Client Connection Helper" card: displays one-click copyable `agent-guidance --set-server http://<ip>:<port>`.

### Milestone 6: Installer Upgrade
- [ ] Update [scripts/install.sh](scripts/install.sh) and [scripts/install.ps1](scripts/install.ps1):
  - `[1] Full Standalone`
  - `[2] Lightweight Client` (prompts for server URL, sets `~/.agent-guidance/config.toml`)
  - `[3] Dedicated Server Worker` (creates staging directory, initializes default binary, sets up systemd unit)

---

## Verification Plan

### Automated Tests
- Unit test client config parsing and serialization (`~/.agent-guidance/config.toml`).
- Unit test 1-N deletion: verify deleting 3 skills from a 10-skill test binary compacts `vectors.bin` to exactly 7 contiguous rows and updates header count to 7.
- Unit test tombstones: verify deleted built-in skills stay excluded across server reboots.
- Unit test SHA-256 hashing and change detection logic.
- Verify unchanged skills are skipped during reindexing.
- Verify section-level vector retrieval returns top 2-3 sections under 350 tokens.
- Verify binary vector file layout (`[count: u32][dim: u32][f32...]`) loads in `< 1 ms`.
- Unit test `GET /api/skills/stats` serialization and client-side stats caching in SQLite.

### Manual Verification
- Run `agent-guidance --set-server http://172.16.11.24:11998` and `agent-guidance --test-server`.
- In Dashboard, check 3 skills and click "Delete Selected".
- Confirm skills immediately vanish from dashboard and search queries without restarting daemon.
- Verify `vectors.bin` size on disk shrinks accordingly.
- Drop new skill in staging, click "Compile Staging to Binary", verify it appears and is queryable in <1ms.
