# Changelog

All notable changes to Agent Guidance Rust MCP Server will be documented in this file.

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
