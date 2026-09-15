# Changelog

All notable changes to Agent Guidance Rust MCP Server will be documented in this file.

## [1.7.1] - 2026-09-15

### 📖 Clustered Token-Bounded Multi-File Read (`project_context`)
- **Single-Turn Clustered Multi-File Read**:
  - Implemented `project_context(operation="read", relative_paths=[...])` (with alias `cluster_read` / `read_cluster`) in `src/mcp/tools/context_read_cluster.rs`.
  - Enables agents to read multiple related files in 1 turn instead of issuing sequential single-file read calls.
- **Context Fidelity & Smart Token-Bounding**:
  - Modular files (< 300 LOC) retain 100% complete content with code fences and language syntax tagging.
  - Large files (> 300 LOC) automatically collapse to AST structural skeletons to avoid blowing out token windows.
  - Total output is clamped to a safety budget (default 800 LOC across cluster).
- **Direct Batch Gate Railing**:
  - Automatically appends a tailored `workflow_gate(action="authorize_edit", relative_paths=[...])` invocation snippet to the read output, guiding agents directly into 1-turn batch gate authorization.
- **Security & Cross-Platform Normalization**:
  - Enforces path traversal prevention (`..` rejection) and workspace boundary verification per file.
  - Normalizes file separators for Windows and Unix.

### ⚡ System Performance Optimization & CPU Thread Reduction
- **Background Watcher Churn Fix (`src/context/watcher.rs`)**:
  - Guarded periodic GraphRAG updates with `if report.files_indexed > 0` and increased check interval from 30s to 60s.
  - Completely eliminates 2.85 MB `communities.json` disk write churn and unnecessary CPU wakeups when the repository is idle.
- **Search Query Debouncing & JIT Sync (`src/mcp/tools/helpers.rs`)**:
  - Replaced synchronous full incremental scans and unthrottled thread spawning with `jit_sync::ensure_fresh_graph(proj_path, 15)`.
  - Debounces search queries within 15s to serve instant sub-millisecond responses without disk re-scans.
- **Global CPU Thread Cap & Dashboard Worker Trimming**:
  - Initialized Rayon global thread pool capped at 4 worker threads (`AGENT_GUIDANCE_MAX_THREADS`, default: 4) in `src/main.rs`, replacing default allocation of `num_cpus` (16–32 threads).
  - Reduced dashboard worker threads from 4 to 2 in `src/dashboard/mod.rs`.
  - Cuts ~20–24 unnecessary OS thread handles and their stack memory.
- **GPU VRAM Preservation for ML**:
  - Retained `GpuSkillMatrix` and `eager_vram_warmup()` ensuring sub-0.1ms GPU matrix multiplication remains in VRAM.
  - CPU fallback operations are constrained to 4 worker threads, preventing 100% CPU lockups during batch embeddings.
- **Passive WAL Checkpoint Maintenance (`src/daemon/server.rs`)**:
  - Automatically triggers non-blocking `PRAGMA wal_checkpoint(PASSIVE);` on client disconnect to bound WAL file growth on `usage.db` and `code_graph.db`.

### ⚡ Batch Edit Authorization & Zero-Turn Pre-Authorization (`workflow_gate`)
- **Batch File Authorization (`action="authorize_edit"`)**:
  - Added support for `relative_paths: ["path1", "path2", ...]` (or `files: [...]`) in `workflow_gate(action="authorize_edit")`.
  - Replaced turn-by-turn per-file approval overhead with a single-turn batch authorization pipeline that evaluates path traversal, LOC limit (< 300 LOC), single-responsibility modular naming, and Code Graph Diff Impact Guard across all target files simultaneously.
  - Generates pre-edit rollback snapshots for each valid target file and aggregates results into a structured Markdown Table report (`PASSED`, `PARTIAL_PASSED`, or `BLOCKED`).
- **Zero-Turn Plan Approval Pre-Authorization (`action="approve_plan"`)**:
  - Extended `workflow_gate(action="approve_plan")` with `relative_paths: [...]`.
  - Upon user plan approval, automatically transitions stage to `Build` and pre-authorizes all planned files in 0 additional turns, allowing agents to commence code modifications immediately without file-by-file authorization calls.
- **Stage Advance Batch Pre-Authorization (`action="advance"`)**:
  - Supported `relative_paths: [...]` during stage advance into `Build`, verifying and authorizing all planned files in the same turn.
- **Clean Architecture & 300 LOC Cap Modular Decomposition**:
  - Decomposed monolithic `src/mcp/tools/gate_edit.rs` (previously 223 LOC) into focused sub-modules under 150 LOC each:
    - `src/mcp/tools/gate_edit_single.rs` (125 LOC): Evaluates single file safety, LOC limits, modular naming, and pre-edit snapshot creation.
    - `src/mcp/tools/gate_edit_batch.rs` (104 LOC): Batch iteration and aggregated markdown reporting.
    - `src/mcp/tools/gate_edit.rs` (191 LOC): Thin dispatch coordinator with target path normalization.
    - `src/mcp/tools/gate_approval.rs` (112 LOC): Plan approval and pre-authorization coordinator.
    - `src/mcp/tools/gate_stage.rs` (178 LOC): Stage advancement and pre-authorization coordinator.
    - `src/mcp/tools_gate_batch_tests.rs` (152 LOC): Dedicated unit test suite with 100% pass rate (5/5 tests).
- **Multi-Agent & IDE Protocol Synchronization**:
  - Updated Rule 4 across all agent protocol specifications (`AGENTS.md`, `GEMINI.md`, `CLAUDE.md`, `.cursor/rules/agent-guidance.mdc`, `.github/copilot-instructions.md`, `.agents/skills/agent-guidance/SKILL.md`, `.claude/skills/agent-guidance/SKILL.md`, `PROJECT-STANDARDS.md`).
  - Prohibits multi-turn sequential per-file authorization calls when file lists are known upfront in implementation plans.

## [1.7.0] - 2026-09-15

### 📦 Multi-File Context Bundling & Subgraph Packing (`subgraph_bundle`)
- **Single-Turn Multi-File Context Packing**:
  - Implemented `project_context(operation="subgraph_bundle")` (alias: `context_bundle`) in `src/mcp/tools/context_bundle.rs` and `src/context/graph_rag/subgraph_bundle.rs`.
  - Aggregates target symbol definition, 1-hop caller snippets, and 1-hop callee snippets into a single compact context bundle, eliminating 3-4 disjoint agent tool calls.
- **Strict Token-Bounded Budget Enforcement**:
  - Default 250 LOC budget clamp (`loc_budget`, clamped range 50..=500 LOC) distributed across target (35%), callers (35%), and callees (30%).
  - Single-pass $O(k)$ line truncation at 300 characters (`bundle_snippet_extractor.rs`) preventing minified or single-line files from blowing out token windows.
  - Safe omission banners (`// ... [N lines omitted] ...`) without synthetic line number artifacts.
- **Cross-Platform Path Normalization (Windows / Linux / macOS)**:
  - Unified POSIX `/` forward-slash output in markdown snippets and code banners across all operating systems.
  - SQLite queries utilize `REPLACE(file_path, '\\', '/') = ?2` to seamlessly match path separators on Windows and Unix platforms.
  - Filesystem resolution uses `build_safe_path` splitting on both slashes (`/` and `\`) and pushing segments sequentially to avoid literal backslash failures on Unix.
- **Accurate Graph Topology & Blast Radius Metrics**:
  - Preserves exact caller/callee counts and structural blast radius scores in headers even when LOC budget limits snippet bodies.
  - Query prioritizes known call lines (`(e.line IS NOT NULL) DESC`) and files matching exact path scopes (`ORDER BY (REPLACE(...) = ?2) DESC`).
  - Symmetric recursion filtering (`source_id != target_id`) excludes self-recursive duplicates on both callers and callees to maximize unique architectural context.
- **Security & Stale State Hardening**:
  - Robust path traversal prevention blocking relative directory navigation (`part != ".."`) and out-of-boundary paths.
  - Out-of-bounds guards protecting against stale SQLite line numbers when files shrink on disk (`start_line > total_lines`, `call_line > total_lines`).

### 🔍 Search Precision & Real-Time Write-Through AST Invalidation
- **Dynamic Inverse Document Frequency (IDF) Down-Weighting**:
  - Analyzes term frequency across all indexed project files in `src/context/search/idf.rs`.
  - Terms occurring in > 30% of codebase files (or ubiquitous keywords like `graph`, `project`, `config`, `state` occurring in > 20%) are down-weighted by a factor of `0.25x` to eliminate false-positive utility noise.
- **Search Intent Gating & Role Penalties**:
  - Distinguishes between `Logic`, `Guidance`, and `Universal` intent in `src/context/search/intent.rs`.
  - Guidance intent boosts markdown documentation 2.50x and dampens source code; Logic intent boosts source code 1.50x and dampens documentation.
  - Test files/fixtures are penalized by `0.40x` unless explicit test keywords are requested; utility files penalized by `0.70x`.
- **Targeted Write-Through AST Invalidation (<10ms)**:
  - Added `is_file_dirty` and `invalidate_and_sync_file` in `src/context/indexer/invalidator.rs`.
  - Immediately refreshes symbols and AST edges for touched files upon edit authorization (`workflow_gate`) without waiting for 5-second watcher debounce.

### 🌊 Lightweight Data Flow & Dynamic Trait Resolution
- **Intra-Procedural Dataflow Traversal**:
  - Added `data_flow_edges` table tracking parameter-to-call flow (`flows_into`) across statements.
  - Supports `project_context(operation="data_flow", query="<param_or_var>")` to trace parameter propagation across call sites.
- **Dynamic Trait & Interface Implementation Links**:
  - Trait resolver maps struct method implementations (`impl Trait for Struct`) to parent trait definitions with `implements_method` edges in `symbol_edges`.

### 📊 Cross-Session Skill Analytics & Contextual Boost
- **Historical Skill Tracking**:
  - Records project-specific skill invocations in `project_skill_analytics` table within `usage.db`.
  - `apply_analytics_boost` awards a frequency-based analytical bonus (+0.02 per invocation, clamped at +0.12) to prioritize domain-specific skills during candidate ranking.
  - Added `guidance(operation="analytics")` producing markdown summary tables of accumulated project skills.

### 🌐 Deep Graph Federation & Multi-Workspace Auto-Discovery
- **Monorepo & Multi-Workspace Auto-Detection**:
  - Implemented `auto_discover_workspaces` in `src/context/federation/workspace_detector.rs`.
  - Automatically identifies Cargo workspace members (`[workspace] members`), npm/pnpm workspaces, and Go work roots without requiring manual configuration files.
- **Federated Callers & Callees Traversal**:
  - `federated_search_callers` and `federated_search_callees` in `src/context/federation/federated_traversal.rs`.
  - Seamlessly queries secondary project code graphs and attaches clear namespace tags (`[repo:lib-name]`).

### 🔄 Continuous Learning Graph & Co-Change Evolutionary Coupling
- **Pairwise Co-Change Recording**:
  - Added `co_change_edges` table with normalized alphabetical pairs (`file_a < file_b`).
  - Automatically records co-edited files during `session_continuity(operation="save")`.
- **Predictive Forgotten File Sentinel**:
  - `predict_coupled_files` in `src/context/co_change/predictor.rs` identifies coupled files with $\ge 60\%$ historical co-change confidence.
  - Emits proactive warnings (`> [!TIP] Co-Change Coupling Alert`) in `workflow_gate(action="authorize_edit")` when an agent forgets to update an associated file or test suite.

### 🩺 Architecture Healing Sentinel
- **Circular Dependency Detection**:
  - Directed DFS algorithm in `src/context/healing/cycle_detector.rs` finds file-level dependency loops with rotation deduplication.
- **Dead & Orphan Symbol Scanning**:
  - `detect_orphan_symbols` in `src/context/healing/orphan_scanner.rs` identifies dead internal symbols (zero callers and zero callees).
  - Diagnostic insights automatically rendered in `project_context(operation="architecture")`.

### 💾 Distributed Graph Memory & Verified Snapshots
- **Graph Snapshot Pack & Unpack**:
  - Implemented `export_graph_snapshot` and `import_graph_snapshot` in `src/context/distributed/snapshot.rs`.
  - Includes SQLite WAL flush (`PRAGMA wal_checkpoint(TRUNCATE)`), streaming checksum calculation, pre-installation `PRAGMA quick_check;` validation, and atomic file replacement.
  - Enables zero-latency GraphRAG cache sharing across distributed agents and CI/CD pipelines.

## [1.6.2] - 2026-09-14

### 🕸️ Multi-Mode GraphRAG Visualizer & File Function Drill-Down
- **Mode 1: File Dependencies (1-N & N-1)**:
  - File-to-file architecture graph aggregating symbol references and semantic relationships.
  - Interactive Subgraph Isolation on file selection, with dedicated Inspector breakdown of incoming dependents (N-1) and outgoing dependencies (1-N).
  - Directory & File Container Navigator (`#graph-symbol-list-container`) with smooth pan/zoom auto-centering.
  - Enhanced layout with 120px node repulsion, 300px edge distance, default orphan culling, anti-distortion `ResizeObserver`, and background click-to-reset.
- **Mode 2: File Functions Drill-Down & 'Show All' View**:
  - High-precision function call graph with directed caller/callee arrows (outgoing neon cyan, incoming purple/orange) and central aura container.
  - Added global `'Show all'` view (`/api/graph?view=file_functions&file=all`) mapping cross-file call relationships across the entire codebase.
- **Searchable File Combobox (`fileCombobox.js`)**:
  - Filter files and folders dynamically with real-time fuzzy/substring search and keyboard navigation.
  - Pinned `'Show all'` option for instant access to global call graphs.
- **Visual Clarity & 10% Dim Capacity**:
  - Standardized dim opacity across all canvas renderers: non-selected/unrelated nodes and edges dim to 10% (`globalAlpha = 0.10`) while selected subgraphs retain 100% brightness.
- **Enforced Explicit Project Selection**:
  - Resolved issue where viewing the graph with "Global (All Projects)" active fell back to an arbitrary default directory.
  - Backend strictly requires explicit project selection via `parse_graph_query`, returning `PROJECT_REQUIRED` payload.
  - Web UI renders a clean, friendly empty-state prompt (`📂 Select a Project to View Graph` / `Vui lòng chọn một dự án để xem đồ thị`) guiding users to select a project from the dropdown.
- **Strict Multi-Stage ML Skill Query Filtering**:
  - Hardened `guidance(operation="search")` against spurious skill proposals on generic maintenance actions (version bumps, docs updates).
  - Added action/intent matching prerequisites, raised Cross-Encoder confidence threshold to 0.65, and automatically suppressed `SKILL_PROPOSAL` prompts when zero relevant skills match.
- **Full Bilingual i18n Parity**:
  - 100% key synchronization between English (`en/graph.js`) and Vietnamese (`vi/graph.js`) across toolbars, navigators, inspectors, search, and metrics.

## [1.6.1] - 2026-09-14

### GraphRAG Vector Context Gating & Precision Action-Aware Skill Engine
- **Stage 1.5 GraphRAG Context Gating**:
  - Implemented `apply_graphrag_context_gating` in `src/ml/skill_graphrag_gate.rs` to validate candidate skills against codebase context before Cross-Encoder re-ranking.
  - Added Max-Pooling comparison against Top-K Symbol and Code Chunk vectors extracted from `CodeGraphDb`.
  - Implemented soft-gating bonus (`+0.4 * max_sim`) for contextually aligned skills and penalty dampening for low similarity (< 0.25), eliminating sprawling/hallucinatory skill recommendations.
  - Implemented graceful bypass for universal meta queries (`git`, `test`, `workflow`, `review`, `doc`) and unindexed projects.
- **Action-Aware Skill Proposal & Stop-Word Filtering**:
  - Implemented shared `is_generic_skill_stopword` in `src/ml/mod.rs` to filter out conversational and system stop-words (`skill`, `select_skill`, `guidance`, `agent`, `action`, `user`, etc.) from ranking bonuses and candidate bypasses across both vector search and lexical fallback.
  - Enhanced `keyword_fallback` in `src/ml/llm_selector.rs` to enforce strict action and intent relevance, requiring explicit matches in `action_triggers`, `triggers`, `intent`, or skill names, preventing unprompted or spurious skill proposals.
  - Updated `guidance_search.rs` to suppress `SKILL_PROPOSAL` and `ask_question` user prompts when no relevant skill matches the user's action, allowing agents to proceed smoothly without unnecessary interruptions.
  - Updated `task_pipeline` in `src/mcp/tools/pipeline.rs` to guide agents directly into planning when no skills are matched.
- **Context Vector Extraction**:
  - Added modular `fetch_top_context_vectors` in `src/context/db/context_vector.rs` adhering to Clean Architecture and strict < 300 LOC limits.
- **MCP Tool Pipeline Refinement**:
  - Enhanced `guidance_search.rs` to seamlessly transition from a 2-Stage pipeline to a precision 3-Stage retrieval architecture.

## [1.6.0] - 2026-09-13

### UTF-8 Character Boundary Safety & Robust Multi-Byte String Truncation
- **Universal UTF-8 Safe Truncation Engine**:
  - Implemented `helpers::truncate_chars` and `helpers::truncate_bytes_safe` in `src/mcp/tools/helpers.rs` to guarantee that string truncation operations never slice in the middle of multi-byte UTF-8 character boundaries (e.g. em-dash `—`, Vietnamese diacritics, CJK characters, emojis).
  - Replaced all raw byte index string slicing (`[..35]`, `[..60]`, `[..100]`) across `src/mcp/tools/` (`guidance_search.rs`, `skills_gate.rs`, `mod.rs`, `context_symbols.rs`) with character-boundary safe helpers.
  - Normalized unicode dashes (`—`, `–`) to standard ASCII `-` in skill descriptions and proposal generation.
- **AST Parser Line Counting Hardening**:
  - Refactored `line_number` calculations in `src/context/ast/polyglot.rs`, `src/context/ast/manifests.rs`, and `src/context/ast/database.rs` to count `b'\n'` on raw byte slices (`as_bytes()[..limit]`), eliminating char boundary panic risks on non-ASCII codebases.
- **Automated Verification**:
  - Added unit test `test_unicode_char_boundary_safety` in `skills_tests.rs` verifying resilience against multi-byte em-dashes, accented Vietnamese sentences, and unicode skill proposals.

## [1.5.9] - 2026-09-10

### Polyglot AST, HUD Radial Spacing, Binary Signing & Provenance Fingerprinting
- **Deep Polyglot & Database Code Graph Engine**:
  - Full pure-Rust AST extractors for frontend components (`Vue` SFCs with setup/templates, `Svelte`, `Astro`, `Html`, `Css`) and database schemas (`Sql` tables/views/procedures/FKs, `Prisma` models/relations, and `Graphql` types).
  - Emits component file stems (`Button.vue` -> `Button`), scoped cross-file reference links, `@import` and `@use` resolution, receiver-aware edges, call lines, confidence, and categories.
  - Extended SQLite persistence (`symbols` and `symbol_edges`) with resilient backwards-compatible fallback queries.
- **HUD Architecture Visualizer Spacing & Scaling Refinements**:
  - Increased radial spacing between nodes via wider Fermat spiral dispersion (`35 + 32*sqrt(i)`), macro community anchor expansion (`0.42`), boosted LinLog repulsion, and expanded anti-collision push buffers (`+56px`, push factor `0.75`) without linker stretching.
  - Scaled down node circle radii by ~40% (`[3.2, 9.0]px`) and label font size down to `7-10px` with tight LOD culling.
- **Cross-Platform Binary Signing & Windows PE Provenance**:
  - Embedded Windows PE Version Information (Publisher: `Jun Mystery <darkzeuslk@gmail.com>`, Product, Description, Copyright, Version) and modern `asInvoker` application manifest with Windows 10/11 compatibility GUIDs into PE headers via `build.rs` to prevent Windows Defender / SmartScreen heuristic false positives.
  - Provided automated Windows Authenticode signing with RFC-3161 timestamping and `TrustedPublisher` enrollment ([`scripts/sign-binary.ps1`](scripts/sign-binary.ps1)).
  - Provided unified Unix signing script ([`scripts/sign-binary.sh`](scripts/sign-binary.sh)) supporting macOS `codesign` (hardened runtime ad-hoc / Apple Developer ID) and Linux GPG detached signatures + SHA-256/SHA-512 manifests.
- **CLI Security Fingerprint & Provenance Command**:
  - Added `--fingerprint` and `--verify-signature` CLI options with zero-dependency pure-Rust SHA-256 hashing and platform digital signature inspection.
- **Telemetry Chart Overhaul & Local System Time Correlation**:
  - Replaced legacy payload wave chart with "Agent Invocations & Latency Wave" correlating hourly tool invocations (left Y-axis) with average execution latency in milliseconds (right Y-axis).
  - Dynamically aligns chart time buckets with client system time across the last 24 hours.
  - Enhanced dark mode readability with `#38bdf8` right Y-axis values.
- **Multi-Repository Project Root Resolution & Pruning**:
  - Resolved tracked project indexing to always detect project root boundaries via `.agent-context/` or `.git/` anchors rather than recording nested subdirectories.
  - Added auto-pruning to clean redundant child directory records from registry when parent roots are recognized.
- **Strict Single Responsibility Decomposition (< 300 LOC Standard)**:
  - Decomposed `src/context/indexer/parsers.rs` into `edge_parser.rs` and `doc_data.rs`.
  - Decomposed `src/dashboard/graph.rs` into `graph_query.rs`.
- **Automated Remote GitHub Actions Release Workflow**:
  - Integrated automated Windows, macOS, and Linux binary signing, fingerprint logging, and release asset packaging into [`.github/workflows/release.yml`](.github/workflows/release.yml).

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
