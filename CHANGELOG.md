# Changelog

All notable changes to Agent Guidance Rust MCP Server will be documented in this file.

## [1.5.4] - 2026-09-09

### Singleton Shared MCP Daemon & Fast Thin Proxy
- **Auto-Negotiated Singleton Shared Daemon**: The first IDE/CLI automatically becomes the master daemon serving stdio and listening on Named Pipe / Unix Socket; subsequent IDE/CLI instances transparently launch as Thin Proxies (< 5MB RAM) with fast Win32 `ERROR_FILE_NOT_FOUND` bypass (< 0.1ms).
- **Background Auto-Warmup**: Added background auto-warmup thread spawned on stdio MCP connection without blocking handshake; responses serve in < 2ms via Fast-Path, seamlessly upgrading to Candle BERT vector cosine similarity and Cross-Encoder neural reranking once models are in memory across all IDEs.
- **Robust Precomputed Cache & GraphRAG Auto-Sync**: Eliminated multi-minute CPU embedding stalls by leveraging precomputed skill vectors when count matches, and auto-triggered code graph indexing on Turn 1 pipeline activation.
- **Global Concurrency & Resource Queue**: Global semaphore limiting concurrent tool execution to 4 permits and Rayon ThreadPool limiting ML vector search & neural reranking to 2 worker threads, preventing CPU/GPU starvation.

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
