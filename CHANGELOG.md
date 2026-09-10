# Changelog

All notable changes to Agent Guidance Rust MCP Server will be documented in this file.

## [1.5.8] - 2026-09-10

### Strict Modularity (< 300 LOC), Silent Tray Launch & Dashboard Polish
- **Strict `< 300 LOC` Modularity Standard & Repository Decomposition**:
  - Audited and decomposed all files in the 200–300 LOC range across the codebase; zero production source files exceed 250 LOC.
  - Refactored 12 monolithic targets into modular single-responsibility submodules:
    - `src/catalog/indexer/` (`document.rs`, `passage.rs`, `inference.rs`, `extractor.rs`, `tests.rs`, `mod.rs`)
    - `src/context/db/schema/` (`core.rs`, `graph.rs`, `content.rs`, `mod.rs`)
    - `src/mcp/router/` (`tools.rs`, `mod.rs`)
    - `src/mcp/tools/` (`skills_gate.rs`, `skills_resolver.rs`, `skills.rs`)
    - `src/mcp/config/` (`setup_cli.rs`, `setup_targets.rs`, `rules_cleaner.rs`, `setup.rs`, `rules.rs`)
    - `src/dashboard/` (`router.rs`, `stats_query_aggregates.rs`, `projects_path.rs`, `projects_prune.rs`, `projects.rs`, `stats_query.rs`, `mod.rs`)
    - `src/optimizer/` (`skeleton_slice.rs`, `skeleton.rs`)
    - Dashboard Frontend (`sidebar.js`, `projectSelect.js`, `actionsStream.js`, `actionsView.js`, `main.js`)
  - Preserved 100% public APIs, re-exports (`pub use`), and test coverage (192 unit tests passing).
- **Tray Platform Silent Dashboard Launch**:
  - Fixed Windows tray menu "Open Dashboard" to launch default web browser directly without opening a flashing CMD / command prompt window.
- **Universal Dashboard Scrollbar & UI Polish**:
  - Implemented custom modern universal scrollbars (`::-webkit-scrollbar` and `scrollbar-width: thin`) across all dashboard views, modals, and tables.
  - Removed outdated "Latency" column from Actions & Recent Calls live stream table and justified column widths.
  - Justified table layouts across live stream and breakdown views.
  - Corrected timestamps on Agent Context Ingestion Velocity & Payload Wave chart.
- **Skill Proposal Formatting & Tracking**:
  - Standardized skill proposals for `ask_question` tool to format options as `"{name} - {short_desc}"` with short descriptions (<= 38 chars) and `is_multi_select: true`.
  - Enhanced Top Skills analytics to record skills invoked via both `select_skills` and `select_skill`.
  - Synchronized execution lifecycle across `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `PROJECT-STANDARDS.md`, and IDE rules.

## [1.5.7] - 2026-09-10

### Configurable Tree Depth & Codebase Hardening
- **Configurable `project_context(operation="tree")`**:
  - Added `max_depth` parameter (default: 3, clamped range: 1..=5) to directory tree scanning in `context.rs`.
  - Added comprehensive unit tests in `src/mcp/tools_tests.rs` verifying default depth, custom depth, and boundary clamping.
- **Router & MCP Surface Alignment**:
  - Added `user_confirmed` (boolean) and `user_message` (string) to `select_skills` JSON-RPC tool input schema in `src/mcp/router/mod.rs`.
  - Fixed residual `agent-guidance-mcp` string in `src/main.rs` and `agent-guidance-mcp_task_pipeline` in `src/mcp/router/resources.rs`.
  - Synchronized `session_continuity` parameter names in documentation (`learning`, `category`, `pinned`, `next_action`).
  - Replaced phantom `guidance://` resources in documentation with the real 5 static URIs + `standards://skill/{name}`.
  - Updated Tool Gate Status table in `docs/ARCHITECTURE.md` and project context documentation (removed prefixes, updated 6-phase search cascade, fixed `view_mode` enum).
- **Modularity Gate & Standards Synchronization**:
  - Expanded `gate_edit_modularity.rs` to enforce all 40 compound plural container suffixes across programming languages.
  - Fixed duplicate header in `PROJECT-STANDARDS.md` and synchronized 40 suffix patterns across `PROJECT-STANDARDS.md`, `AGENTS.md`, and `GEMINI.md`.

## [1.5.6] - 2026-09-09

### Real-Time MCP Crash/Diagnostics Engine & Architecture Graph Visual Overhaul
- **Real-Time Crash & Diagnostics Engine (`mcp_logger.rs`)**:
  - Global `std::panic::set_hook` capturing panic payload, location `[file:line:column]`, and unmasked backtraces via `std::backtrace::Backtrace::force_capture()`.
  - Double-write architecture: High-speed SQLite persistence in `mcp_logs` table + Emergency fallback file `~/.agent-guidance/logs/crash.log` with unbuffered `sync_all()` hardware fsync.
  - Multi-tier retention policies (Crash: 30 days, Error: 14 days, Warn: 7 days, Info: 3 days) with 10,000-entry FIFO rolling quota and 5-file rotating crash log cap.
  - STDIO EOF & Pipe Disconnection Detection: Distinguishes between clean IDE disconnects and I/O stream failures.
- **Web Dashboard Diagnostics (`--dashboard` / `#logs`)**:
  - Added new `⚠️ Diagnostics` view with real-time KPI stat cards (Crashes, Errors, Warnings, Total).
  - Multi-level quick filter pills (`All`, `💥 Crashes`, `❌ Errors`, `⚠️ Warnings`) and real-time substring search across messages, sources, and stack traces.
  - Interactive collapsible drawer to inspect complete runtime stack traces, error context, and execution metadata.
  - Integrated with automated `--cleanup` and interactive `🧹 Clear` action.
- **Architecture Graph `#graph` Visual Overhaul**:
  - **Spacious Node Layout**: Replaced overlapping radial distribution with Fermat spiral (phyllotaxis) initial placement, tuned ForceAtlas2 edge rest lengths (95–115px), softened repulsion falloff, and widened community macro separation.
  - **Zero Label Collision**: Implemented greedy bounding-box label collision culling, multi-tier Zoom Level-of-Detail (LOD), dynamic top ~12% hub detection, and dark contrast halos (`strokeText` 3.5px) for 100% readable text labels.
  - **Silky Fiber Linkers & Rapid Photons**: Redesigned graph linkers as delicate, translucent optical fibers (0.5px–0.65px) with graceful Bezier draping and tuned photon comet pulses with neon glow trails.

## [1.5.5] - 2026-09-09

### Detached Singleton Daemon, 60s Idle Cooldown & Resilient Multi-IDE Architecture
- **True Detached Background Daemon**: Decoupled Master Daemon from IDE parent process trees using Windows `DETACHED_PROCESS` and `CREATE_NO_WINDOW`. Closing or reloading any IDE never terminates the daemon, eliminating fate-sharing crashes (`0xc0000409`).
- **60s Idle Cooldown Lifecycle**: Ref-counted active connections via `ACTIVE_CLIENTS`. When all IDE sessions disconnect, the daemon enters a 60-second cooldown timer. If an IDE connects within 60s, shutdown is cancelled; if 60s expires with zero connections, daemon cleanly shuts down.
- **Sync ML Inference Queue**: Added `SyncMlQueue` with RAII `SyncMlPermit` limiting concurrent heavy ML embedding inference to 2 permits, preventing VRAM/RAM exhaustion when multiple IDEs search skills simultaneously.
- **Per-Project Isolated GraphRAG JIT Sync**: Converted single global sync timestamp into a per-project canonical path map (`PROJECT_LAST_SYNC: RwLock<HashMap<PathBuf, u64>>`), allowing independent repos to sync and update their code graphs without blocking each other.
- **CLI Guard & Zombie Elimination**: Added `--version` / `-v` flags for instant exit, guarded CLI flags to prevent accidental stdio hangs on invalid args, and upgraded `proxy_stream` to `tokio::select!` preventing zombie processes.

## [1.5.4] - 2026-09-09

### Singleton Shared MCP Daemon & Fast Thin Proxy
- **Auto-Negotiated Singleton Shared Daemon**: The first IDE/CLI automatically becomes the master daemon serving stdio and listening on Named Pipe / Unix Socket; subsequent IDE/CLI instances transparently launch as Thin Proxies (< 5MB RAM) with fast Win32 `ERROR_FILE_NOT_FOUND` bypass (< 0.1ms).
- **Background Auto-Warmup**: Added background auto-warmup thread spawned on stdio MCP connection without blocking handshake; responses serve in < 2ms via Fast-Path, seamlessly upgrading to Candle BERT vector cosine similarity and Cross-Encoder neural reranking once models are in memory across all IDEs.
- **Robust Precomputed Cache & GraphRAG Auto-Sync**: Eliminated multi-minute CPU embedding stalls by leveraging precomputed skill vectors when count matches, and auto-triggered code graph indexing on Turn 1 pipeline activation.
- **Global Concurrency & Resource Queue**: Global semaphore limiting concurrent tool execution to 4 permits and Rayon ThreadPool limiting ML vector search & neural reranking to 2 worker threads, preventing CPU/GPU starvation.
- **Always-On Embedded Dashboard**: Master daemon automatically launches the embedded HTTP analytics dashboard in the background on default port 11997 (customizable via CLI `--port` / `--dashboard-port` or `AGENT_GUIDANCE_DASHBOARD_PORT`), accessible anytime an instance is running.

## [1.5.3] - 2026-09-09

### Automated Snapshot Retention & Cleanup
- **Snapshot TTL & LRU Pruning**: Added automatic background cleanup in `src/mcp/snapshots.rs` enforcing a 7-day TTL expiration policy and a maximum of 20 session snapshot folders.
- **Backward Compatibility**: Modularized snapshot lifecycle methods and re-exported clean APIs in `src/mcp/impact.rs`.

### Multi-Client MCP Auto-Registration & Verification
- **VS Code CLI Registration**: Integrated automatic registration via `code --add-mcp` across Windows and Unix install scripts and Rust `--setup` command.
- **Cursor MCP Support**: Added global `~/.cursor/mcp.json` and `%APPDATA%\Cursor\User\mcp.json` auto-configuration and verification.
- **Claude Code CLI Registration**: Integrated user-scoped registration via `claude mcp add --scope user agent-guidance -- <bin>`.
- **ChatGPT / OpenAI Codex Integration**: Added official `~/.codex/config.toml` TOML generator, CLI registration via `codex mcp add`, and verification in `agent-guidance --verify-setup`.

### IDE GUI Plan Approval & Gate Flow Fixes
- **GUI Approval Detection**: Expanded `process_user_message` to recognize IDE-generated approval signals (`The user has approved this document`, `Proceeded with Implementation Plan`, `Comments on artifact URI:`, `proceed with`).
- **Zero-Turn Approval Ingestion in Pipeline**: `task_pipeline` automatically senses GUI approval messages and transitions to `Build` without wiping plan approval state.
- **Flexible Workflow Gate**: Added support for `plan_approved: true` in `set_stage` / `advance`, added `"advance_stage"` alias, and eliminated aggressive mandatory `ask_question` prompting.

### Architecture Graph Layout & Viewport Fixes
- **Concentric Layout Engine**: Re-centered ForceAtlas2 simulation coordinates to `(0, 0)` and partitioned orphan nodes into clean concentric orbital rings, eliminating off-center drift and circle distortions.
- **Dynamic Auto-Fit Viewport**: Implemented `fitToView` viewport calculations with auto-centering on graph load, fit reset, and filter changes.
- **Community Aura Bounding**: Capped community boundary radii to prevent outlier nodes from blowing up cluster auras.

## [1.5.2] - 2026-09-08

### AI Agent Direct Orchestration for GraphRAG Semantic Layer
- **Semantic Edges & Domain Summaries Schema**: Added `semantic_edges`, `domain_summaries`, and FTS5 virtual tables with SQLite triggers ensuring real-time bidirectional synchronization.
- **Agent Context Enrichment Tools**: Added `project_context(operation="enrich_graph")` and `project_context(operation="semantic_query")` enabling autonomous agents to inject high-level architectural insights and semantic relations directly into the GraphRAG knowledge base without relying on external MCP layers.
- **Drift & Local Search Semantic Integration**: Integrated semantic domain summaries and agent-curated edges into DRIFT and Local GraphRAG search pipelines.

### Dashboard Real-Time Capabilities & Visual Enhancements
- **Top Skills Real-Time Polling**: Added auto-refresh polling on the Top Skills tab with `visibilitychange` lifecycle pause/resume, zero-flicker table rendering, and dynamic `skills-poll` status badge.
- **Interactive Force-Directed Canvas**: Added animated dotted semantic edges with energy particles (`#ec4899`), dynamic responsive auto-centering, and layout optimization.
- **Project Duplication & Normalization Fix**: Fixed project path partition casing bug (`e:` vs `E:`) to guarantee unique project identity across Windows drive letters.
- **Skills Vector Index Refresh**: Recomputed and refreshed full Candle-BERT semantic vector index for all 440 skills.

## [1.5.1] - 2026-09-08

### Bounded Line Range Read & Indentation Preservation
- **Explicit Line Slicing**: Added `start_line` and `end_line` parameters to `project_context(operation="read")` allowing exact range reads up to 300 LOC.
- **Line Numbering Format**: Each line is output with `L{line_no}: <content>` format for precise AI location targeting.
- **Whitespace & Indentation Integrity**: Fully preserved whitespace and indentation structure across all bounded reads; added automated warnings for indentation-sensitive languages (`.py`, `.yaml`, `.yml`, `Makefile`, `.nim`).

### Guidance Protocol & Emoji Removal
- **Emoji-Free MCP Surface**: Removed all decorative emojis from MCP guidance messages, templates, rules, and CLI tools across the repository to ensure strict token efficiency and clean markdown parsing.

## [1.5.0] - 2026-09-08

### ⚡ Int8 Quantized Vector Acceleration with DirectML/CUDA (Proposal 10)
- **Hardware Execution Providers**: Opportunistic GPU acceleration via **Microsoft DirectML** (DirectX 12 on Windows across AMD, Intel, and NVIDIA GPUs) and **NVIDIA CUDA**, with zero-cost AVX2/SIMD CPU baseline fallback.
- **Int8 Quantized Inference**: Dynamic batch tokenization and tensor pooling for Int8 quantized models (`bge-small-en-v1.5-q8.onnx`, `model_quantized.onnx`), delivering a **10x throughput speedup** and reducing per-chunk embedding latency from 20ms to `< 1.8ms`.
- **Unified Embedding Backend**: Seamless runtime dispatch between ONNX Int8 hardware acceleration and pure-Rust Candle BERT baseline.

### 🔍 AST Semantic Context Slicing / Zoom Read (Proposal 8)
- **Precision Tree-Sitter Slicing**: Preserves 100% of types, structs, traits, imports, and documentation while folding un-targeted sibling bodies into `/* L{}-{}: {} lines folded */`.
- **Target Symbol Focus**: Fully expands the requested function or method with `// >>> [ZOOM FOCUS: ...] >>>` banners.
- **Live Token Savings Telemetry**: Returns 60% to 80% token savings metrics directly in `project_context(operation="read", view_mode="zoom")`.
- **Heuristic Slicing Fallback**: Robust structural folding fallback in `src/optimizer/skeleton.rs` for non-Tree-Sitter files.

### 📊 Mermaid Visual Architecture DAGs & Web Dashboard Network (Proposal 6)
- **Live Mermaid Architecture Flowcharts**: Automatically appends layer-based `graph TD` subgraphs and symbol blast-radius `graph LR` flowcharts to `project_context(operation="architecture")` and `project_context(operation="graph_rag")`.
- **Interactive 2D Force-Directed Canvas**: Real-time interactive network visualization on the Web Dashboard (`/api/graph?project=...`) with critical hub glowing rings (`🔥 Hub`), symbol filter pills (`All`, `Functions`, `Structs`, `Hubs`), and a 1-click `📋 Copy Mermaid DAG` button.

### 🧪 Quality & Verification
- **Full Test Suite Perfection**: **149 passed**, 0 failed, 6 ignored.
- **Strict 300 LOC Hard Cap**: All new and refactored sub-modules comply strictly with the codebase modularity cap (target < 150 LOC per sub-module).

## [1.4.13] - 2026-09-04

### ⚡ Token Optimization & Lean Pipeline
- **Removed Unrequested Skill Proposals**: Eliminated skill search/recommendations and recipes from `task_pipeline` to stop token waste on every turn 1 initialization.
- **Compacted Tree and Blueprint**: Replaced 15-file listing with scanned file counts; suppressed empty blueprint blocks.
- **Workflow Gate & Context Compact**: Condensed verbose multi-line upfront architecture warnings to single-line notices, and removed repetitive advice footers from `project_context`.
- **Prohibited Shell Read Commands**: Added strict prohibitions against `Get-Content`, `cat`, `type`, and script reads.

## [1.4.12] - 2026-08-31

### ♻️ DRY & Reusable Code Intelligence (GraphRAG & ML)
- **GraphRAG Topology Fan-In Analysis**: Added automated in-degree caller analysis and ranking (`reusability_score`) to detect core shared utilities across modules.
- **ML Semantic Clone Detection**: Integrated embedding cosine similarity ($\ge 88\%$) to detect duplicate logic across files and issue automated DRY warnings.
- **MCP project_context Operations**: Added `operation="reusable"` / `"detect_duplicates"` / `"reusable_candidates"` to inspect reusable symbols.

### 🛡️ Agent Guidance & Token Bounding Hardening
- **Strict File Inspection Enforcement**: Added explicit negative constraints prohibiting native IDE inspection tools (`view_file`, `grep_search`, `find_by_name`, `list_dir`) to prevent token window explosion.
- **DRY & Shared Code Protocol**: Enforced mandatory reuse of existing helpers in `shared/`, `utils/`, `common/` before authoring new code.
- **Test Suite Perfection**: 103 unit tests passing with zero failures.

## [1.4.11] - 2026-08-27

### 🚀 Skills Catalog & Indexing
- **Catalog Refresh & Expansion**: Updated embedded skill suite to **440 skills** across engineering, security, architecture, performance, testing, and creative automation.
- **New Core Skills**:
  - skill-comply: Automated agent compliance measurement, multi-strictness scenario generation, and deterministic tool-call sequence analysis.
  - 	asteforge-video: File-driven multimodal discovery, taste interview distillation, style-pack schema validation, and frame-accurate EDL/FCPXML timeline export.
- **Instant Manifest Generator**: Added deterministic --build-manifest mode to parse AST metadata and SipHash fingerprints for 440 skills in <50ms.

### 🛡️ Architecture & Gate Reliability
- **Architecture Resolver Hardening**: Fixed edge case where "Auto" or "None" persisted in .agent-context/ or GraphRAG communities could block edit authorizations.
- **Expanded Patterns**: Full native support for CLI_Pipeline and Flat_Library alongside Clean_Architecture, Layered_Architecture, Package_By_Feature, and Orchestrator.
- **Test Suite Perfection**: 98 automated unit tests passing cleanly with zero failures.

### 📦 Build & Release Automation
- Clean multi-target release build workflow with cross-platform packaging for Windows (x86_64), Linux (x86_64), and macOS (Apple Silicon + Intel).
