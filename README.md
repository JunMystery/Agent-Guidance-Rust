# 🦀 Agent Guidance MCP Server

[![Version](https://img.shields.io/badge/Version-v1.5.7-blue.svg)](Cargo.toml)
[![Rust 2024](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)
[![Role](https://img.shields.io/badge/Role-Autonomous%20Orchestrator-indigo.svg)](#-key-capabilities)
[![Smart Skills](https://img.shields.io/badge/Smart%20Skills-279%2B%20ML%20Search-cyan.svg)](#-smart-skills-system)
[![Multi-Session Isolation](https://img.shields.io/badge/Multi--Session-Isolated-green.svg)](#-multi-session-isolation)
[![Universal Token Optimization](https://img.shields.io/badge/Token%20Opt-300%20LOC%20Clamped-purple.svg)](#-universal-token-optimization)
[![MCP Protocol](https://img.shields.io/badge/MCP-2024--11--05-green.svg)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

![Agent Guidance Orchestrator Manager](docs/images/hero-banner.png)

> **Agent Guidance** is a native, high-performance **MCP (Model Context Protocol) Server & Autonomous Orchestrator** written in Rust. It supervises AI Coding Agents (Antigravity, Claude Code, Cursor, Windsurf, Devin, OpenCode) to enforce enterprise architecture patterns, isolate multi-IDE session states, prevent context window blowups via token compression, and deliver sub-millisecond **Smart Skills Calls** via local ML vector search.

---

## 🚀 Quickstart & Installation

### Automatic One-Line Install (Recommended)

Run the one-liner setup script for your operating system to download the latest release binary, pre-cache local ML models, and auto-register `agent-guidance` across all detected IDE clients:

**Windows (PowerShell):**
```powershell
powershell -Command "iwr https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.ps1 -OutFile $env:TEMP\i.ps1; & $env:TEMP\i.ps1"
```

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.sh | bash
```

### Manual Build via Cargo

```bash
git clone https://github.com/JunMystery/Agent-Guidance-Rust.git
cd Agent-Guidance-Rust
cargo build --release
./target/release/agent-guidance --setup
```

---

## 🛠️ MCP Tool Suite Reference

Agent Guidance exposes 6 high-efficiency MCP tools designed to minimize agent round-trips and token waste:

| Tool Name | Role / Action | Mandatory Arguments | Key Capabilities |
| :--- | :--- | :--- | :--- |
| **`task_pipeline`** | **Entrypoint Orchestrator** | `task`, `project_path`, `phase` | CALL FIRST. Scans project, unlocks priority gate, proposes skills, synthesizes **Dynamic Split Blueprints** (detects $\ge 200$ LOC files) and **Skill Recipes**, and injects **Memorized Learnings**. |
| **`select_skills`** | **Semantic Skill Loader** | `skills` | Loads skill instructions into context with **Semantic Slicing** (Top-3 sections via Multilingual-E5 saving ~70% tokens) and injects language safety micro-guidance. |
| **`workflow_gate`** | **Stage & Impact Guard** | `action` | Manages stage transitions (`check`, `status`, `set_stage`, `advance`, `authorize_edit`, `rollback`). Features **Zero-Turn Advance**, **Code Graph Impact Risk Gating**, and **Pre-edit Snapshot Rollback**. |
| **`project_context`** | **Code Graph, GraphRAG & AST Skeleton** | `operation` (`graph_rag` / `search` / `navigate` / `read` / `symbols` / `references` / `callers` / `callees` / `blast_radius` / `definition` / `type_definition` / `architecture` / `tree` / `learn_alias` / `reindex` / `enrich_graph` / `semantic_query`) | **Hierarchical Leiden GraphRAG** (`global`, `local`, `drift`, `basic`), 6-phase cascade search (<100ms), RAG code chunk vectors, AST symbol extraction, call-graph fanout (`callers`/`callees`/`blast_radius`), LSP definitions, configurable tree depth (`max_depth: 3`), and **AST Structural Skeletonization** (`view_mode="skeleton"`, saving 90-95% tokens on files >300 LOC). |
| **`guidance`** | **Skills & Rule Engine** | `operation` (`search` / `docs` / `workflow` / `precode` / `verify`) | 2-stage vector search over 279 embedded skills (440 vectors), language-specific precode safety rules (Kotlin, Go, Rust, TS, Python), and empirical verification contracts. |
| **`session_continuity`** | **Memory & Handoff** | `operation` (`save` / `load` / `clear` / `learn` / `handoff`) | Persists active task states, records **Categorized Project Learnings** in `.agent-context/learnings.md` (30-item FIFO cap), and generates **Cross-Agent Handoff** summaries in `.agent-context/handoff.md`. |

---

## 🎯 Key Capabilities

### 1. Hierarchical Codebase GraphRAG (Global, Local, DRIFT Search)
- **Leiden Hierarchical Community Clustering**: Partitions codebase symbols and AST relationships into Level 0 (Macro Subsystems), Level 1 (Feature Modules), and Level 2 (Micro Clusters).
- **The 4 Query Modes**:
  - **Global Search**: High-level reasoning across community summaries.
  - **Local Search**: Targeted symbol search with 1-hop & 2-hop DAG call/import fan-out.
  - **DRIFT Search**: Dual-route combining macro community layer context with micro AST signatures.
  - **Basic Search**: Fast HNSW vector and FTS5 fallback.
- **Continuous Reactive Watcher**: Background file watcher automatically updates AST nodes and re-clusters community summaries upon code modifications.

### 2. Autonomous Single-Entrypoint Orchestration
- Governs the complete AI agent lifecycle through `task_pipeline`. The MCP server inspects the workspace, unlocks priority gates, selects skills, and dynamically directs next steps.
- Enforces enterprise architecture styles (**Clean Architecture**, **Layered Architecture**, **Package-by-Feature**, **CLI Pipeline**, **Flat Library**, **Orchestrator**) with cross-session persistence in `.agent-context/architecture.json`.

### 3. High-Speed 6-Phase Search Cascade (<100ms)
- Replaces slow raw disk scans with an instant multi-tier cascade stored in `<project_root>/.agent-context/code_graph.db`:
  1. **Phase 1: Alias Cache (<1ms)**: Instant lookup for learned natural language queries.
  2. **Phase 2: Symbol FTS5 (<5ms)**: SQLite FTS5 index on all functions, structs, enums, classes, and traits.
  3. **Phase 3: Symbol Vectors (<50ms)**: BERT semantic similarity on symbol signatures.
  4. **Phase 4: Content FTS5 (<5ms)**: Full-text search across 50-line code chunks.
  5. **Phase 5: RAG Content Vectors (<100ms)**: Multilingual-E5 semantic search on actual code chunks.
  6. **Phase 6: Linked Projects (<120ms)**: Cross-workspace semantic and symbol search across linked repositories.
- **Adaptive Alias Learning**: Automatically learns successful queries, increasing confidence with reuse and decaying inactive mappings (50% reduction after 30 days, purged after 90 days).
- **Proactive Background File Watcher**: Uses OS-level file monitoring (`notify`) with a 5s debounce to incrementally update AST symbols, DAG edges, and RAG chunks before the agent even issues a query.

### 4. Hardened 300 LOC Cap & Upfront Decomposition
- Physically clamps file reads at 300 lines max and automatically injects architectural decomposition mandates on large files.
- Generates concrete Upfront Split Blueprints per pattern during pre-code guidance.

### 5. Universal In-Engine Token Compression
- Automatically intercepts and compresses all outgoing MCP tool responses, stripping HTML comments, badges, and redundant whitespace.
- Reduces context payload size by **30–50%** while logging real-time token savings to SQLite (`~/.agent-guidance/usage.db`).

### 6. Multi-Session & Multi-IDE Isolation
- Assigns process-isolated Session IDs (`session_{PID}_{ClientName}`) to eliminate state collisions across concurrent IDEs (VS Code, Cursor, Antigravity) or CLI tools in the same codebase.

### 7. Real-Time Web Dashboard & Visual GraphRAG

![Agent Guidance Real-Time Web Dashboard & Visual GraphRAG](docs/images/dashboard-GraphRAG.png)

- **Interactive Architecture Graph**: Real-time canvas visualization of codebase symbols, call/import dependencies, and Leiden community clusters powered by a ForceAtlas2 physics simulation engine with collision culling and contrast halos.
- **Deep Symbol & Blast Radius Inspector**: Click any node on the graph or search by name to inspect callers (incoming), dependencies (outgoing), architectural tiers, and blast radius risk assessment.
- **Parallel Symbol & Function Navigator**: Search and navigate across all project files and functions simultaneously with instantaneous filtering.
- **Token Savings & Velocity Dynamics**: Interactive telemetry charts tracking original vs. compressed payload waves, peak velocity, and execution traces.
- **Bilingual Interface (i18n)**: Full native support for English (`en`) and Vietnamese (`vi`) with instant dynamic switching and persistent preferences.
- **Zero-Friction Singleton Daemon**: Runs quietly in the background, serving all concurrent IDE instances, and automatically shuts down immediately once the last IDE window closes.

---

## 🏗️ Architectural Workflow

![Orchestrator Workflow Flowchart](docs/images/orchestrator-flow.png)

### The 7-Stage Workflow Gate
`Context` $\longrightarrow$ `Plan` $\longrightarrow$ `Ask_Revise` $\longrightarrow$ `Build` $\longrightarrow$ `Test_Recheck` $\longrightarrow$ `Fix` $\longrightarrow$ `Proposal`

- **Composite Gate Action (`workflow_gate action="advance"`)**: Performs stage check, transition, and architecture pattern authorization in a single composite MCP call.
- **Hard Edit Gate (`workflow_gate action="authorize_edit"`)**: Code modification is BLOCKED until `plan_approved = true` and a valid `architecture_pattern` is verified.
- **Circuit Breaker**: If 3 consecutive fix attempts fail during `Fix`, the MCP server automatically trips, resets stage to `Ask_Revise`, and requests human intervention.

---

## 🧠 Smart Skills System

The built-in ML catalog engine leverages local Rust bindings for Hugging Face `candle` to perform sub-millisecond semantic skill discovery:

- **Stage 1 (Cosine Similarity)**: Scans 279 embedded skills (440 precomputed vector embeddings) using Candle BERT vector embeddings with precomputed binary vector acceleration ($<5\text{ ms}$).
- **Stage 2 (Intent Reranking)**: Cross-encoder (`ms-marco-MiniLM-L-6-v2`) reranks top candidates with language profile boosting.
- **On-Demand Loading**: Skills are injected dynamically into context via `select_skills(skills=[...])` only when confirmed.

### Custom Skill Sets (User Extensibility)
You can easily add your own custom skills without rebuilding or reconfiguring the MCP server:
- **Global Custom Skills**: Simply copy or paste your skill directories/markdown files directly into:
  - **`~/.agent-guidance/skills/`** (or `~/.agents/skills/`)
- **Workspace-Specific Skills**: Place custom skills directly in your active project repository under:
  - **`<project_root>/.agents/skills/`**
  - **`<project_root>/.opencode/skills/`**
  - **`<project_root>/.claude/skills/`**

All `.md` files in these directories are automatically scanned, parsed for YAML frontmatter (`name: ...`), and indexed into the local search catalog on the fly.

---

## ⚡ Universal Token Optimization

- **Hard Clamping**: Capped at 300 LOC per file read, 20 results per search, 30 references per symbol search, and 15 items per tree preview.
- **Symbol-Targeted Extraction**: Extract exact function/struct blocks using `project_context(operation="read", target_symbol="...")` saving up to 85% of tokens.
- **Dynamic Compression**: Automatic stripping of markdown comments, badges, and empty lines across all responses.
- **SQLite Analytics**: All tool metrics, durations, and token savings are logged to `~/.agent-guidance/usage.db`.

---

## 🔒 Multi-Session Isolation

When running multiple AI agents across different IDEs or terminals simultaneously in the same repository, `Agent Guidance` maintains total isolation:

```text
.agent-context/
├── architecture.json                    (Persistent Architecture Memory)
├── sessions/
│   ├── session_14820_antigravity.json   (Build Stage - Plan Approved)
│   ├── session_29401_cursor.json        (Plan Stage - Awaiting Approval)
│   └── session_8812_cli.json            (Context Stage)
└── session.json                         (Legacy Atomic Pointer)
```

- **Automated GC Policy**: On startup and session load, stale session files older than 30 days are automatically purged. If total session files exceed 100, the oldest files are pruned.

---

## 💻 CLI Commands & Maintenance

`agent-guidance` provides built-in CLI commands for managing IDE clients, updates, and metrics:

```bash
agent-guidance [OPTIONS]

Options:
  --setup                  Install and configure MCP server across all IDE clients
  --verify-setup           Verify MCP configuration paths in all IDE clients
  --upgrade                Download and install latest release package, update IDE configs
  --self-update            Alias for --upgrade
  --daemon, -d             Force start in background singleton daemon mode
  --proxy                  Force connect as client proxy to daemon; exit if no daemon
  --dashboard              Start real-time web usage dashboard at http://127.0.0.1:11997
  --port, -p <PORT>        Custom dashboard port (default: 11997, alias: --dashboard-port)
  --project <PATH>         Filter dashboard to a specific project path or name
  --prune-missing          Prune non-existent projects from usage tracking registry
  --cleanup                Auto-clean expired logs, prune dead projects, and vacuum SQLite DB
  --retention-days <N>     Retention window in days for detail logs (default: 7)
  --reindex-skills         Precompute and build rich semantic vector index for all skills
  --uninstall              Remove MCP server configurations from all IDE clients
  --help, -h               Print help message
```

---

## 📂 Project Structure

```text
Agent-Guidance-Rust/
├── src/
│   ├── main.rs                   # CLI entrypoint, argument parsing, stdio MCP dispatcher
│   ├── catalog/                  # Built-in and custom skills scanner, indexer, YAML parser
│   ├── context/                  # Codebase indexing, AST parsing, GraphRAG, 5-phase cascade search
│   │   ├── graph_rag/            # Hierarchical Leiden community clustering, DRIFT/Local/Global search
│   │   ├── indexer/              # Tree-sitter AST symbol and reference extraction
│   │   ├── scanner/              # File walker, gitignore resolution, change detection
│   │   └── watcher/              # Real-time background filesystem watcher
│   ├── daemon/                   # Zero-friction singleton daemon, IPC named pipe/socket, client lifecycle
│   ├── dashboard/                # Embedded tiny_http web server, REST endpoints, SQLite telemetry queries
│   ├── dashboard_src/            # Frontend SPA (Vanilla JS + CSS, zero runtime npm dependencies)
│   │   ├── index.html            # Dashboard layout and accessible view containers
│   │   ├── js/i18n/              # Modular bilingual dictionaries (EN/VI: core, telemetry, graph)
│   │   └── js/render/            # Canvas graph visualizer, ForceAtlas2 layout engine, symbol inspector
│   ├── mcp/                      # Model Context Protocol implementation & tool execution handlers
│   │   ├── tools/                # task_pipeline, select_skills, workflow_gate, project_context, guidance
│   │   ├── state/                # Multi-session state machine, priority gate, checkpointing
│   │   └── db/                   # SQLite database operations, automatic cleanup, and vacuuming
│   ├── ml/                       # Candle BERT neural embeddings, vector similarity, ONNX inference
│   └── optimizer/                # Universal token compression engine, AST code skeletonizer
├── skills/                       # Pre-packaged domain skills catalog (279 embedded skills, 440 vectors)
├── docs/                         # Architectural diagrams, specifications, setup guides
│   └── images/                   # Dashboard screenshots, hero banners, and flowcharts
└── scripts/                      # Automated installation and maintenance scripts (PowerShell, Bash)
```

---

## 📚 Documentation Index

Comprehensive guides, architecture deep-dives, and client setup instructions are available in the [`docs/`](docs/) directory:

| Section | Topic | Documentation Link |
| :--- | :--- | :--- |
| **Architecture** | System Design & Lifecycles | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| **Getting Started** | Quickstart & Overview | [`docs/getting-started.md`](docs/getting-started.md) |
| **Web Dashboard** | Real-Time Telemetry & Visual GraphRAG | [`docs/dashboard.md`](docs/dashboard.md) |
| **Installation** | Platform Setup & Upgrades | [`docs/installation.md`](docs/installation.md) |
| **Usage Guide** | Orchestrator & Workflow Usage | [`docs/usage.md`](docs/usage.md) |
| **Development** | Contributing & Testing | [`docs/development.md`](docs/development.md) |
| **IDE Setup** | Antigravity, Cursor, VS Code, Windsurf | [`docs/setup/`](docs/setup/) |
| **Skills Guide** | Skill Anatomy & Catalog Policy | [`docs/skills/SKILLS_OVERVIEW.md`](docs/skills/SKILLS_OVERVIEW.md) |
| **Reference** | MCP Surface & Protocol Spec | [`docs/reference/mcp-surface.md`](docs/reference/mcp-surface.md) |

---

## 🙏 Credits & Acknowledgments

This project references and acknowledges the following third-party security resources:

| Resource | Description | Repository |
| :--- | :--- | :--- |
| **ECC** | Elliptic Curve Cryptography reference implementation | [affaan-m/ECC](https://github.com/affaan-m/ECC) |
| **OWASP CheatSheetSeries** | Collection of high-value security cheat sheets for application security | [OWASP/CheatSheetSeries](https://github.com/OWASP/CheatSheetSeries) |

---

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for details.
