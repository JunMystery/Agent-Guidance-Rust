# Agent Guidance (Rust MCP Server)

[![Version](https://img.shields.io/badge/Version-v1.3.7-blue.svg)](Cargo.toml)
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
| **`task_pipeline`** | **Entrypoint Orchestrator** | `task`, `project_path`, `phase` | CALL FIRST. Scans project, unlocks priority gate, proposes deduplicated skills, and returns upfront architecture guidance. |
| **`select_skills`** | **Skill Context Loader** | `skills` | Loads compressed skill instructions into context. Supports pre-baked embedded catalog, direct name lookup, and local workspace skills. |
| **`workflow_gate`** | **Stage & Edit Gate** | `action` | Manages stage transitions (`check`, `status`, `set_stage`, `advance`, `authorize_edit`). Blocks code editing until plan is approved. |
| **`project_context`** | **Bounded Code Inspector** | `operation` (`tree` / `read` / `search` / `symbols` / `references` / `architecture`) | Token-bounded code search, symbol AST extraction, hard 300 LOC clamping, and decomposition blueprints. |
| **`guidance`** | **Skills & Rule Engine** | `operation` (`search` / `docs` / `workflow` / `precode` / `verify`) | 2-stage vector search, language-specific precode safety rules (Kotlin, Go, Rust, TS), and empirical verification contracts. |
| **`session_continuity`** | **State Persistence** | `operation` (`save` / `load` / `clear`) | Persists active task states and workflow gates across agent turns with 30-day automatic garbage collection. |

---

## 🎯 Key Capabilities

### 1. Autonomous Single-Entrypoint Orchestration
- Governs the complete AI agent lifecycle through `task_pipeline`. The MCP server inspects the workspace, unlocks priority gates, selects skills, and dynamically directs next steps.
- Enforces enterprise architecture styles (**Clean Architecture**, **Layered Architecture**, **Package-by-Feature**, **CLI Pipeline**, **Flat Library**, **Orchestrator**) with cross-session persistence in `.agent-context/architecture.json`.

### 2. Hardened 300 LOC Cap & Upfront Decomposition
- Physically clamps file reads at 300 lines max and automatically injects architectural decomposition mandates on large files.
- Generates concrete Upfront Split Blueprints per pattern during pre-code guidance.

### 3. Universal In-Engine Token Compression
- Automatically intercepts and compresses all outgoing MCP tool responses, stripping HTML comments, badges, and redundant whitespace.
- Reduces context payload size by **30–50%** while logging real-time token savings to SQLite (`~/.agent-guidance/usage.db`).

### 4. Multi-Session & Multi-IDE Isolation
- Assigns process-isolated Session IDs (`session_{PID}_{ClientName}`) to eliminate state collisions across concurrent IDEs (VS Code, Cursor, Antigravity) or CLI tools in the same codebase.

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

- **Stage 1 (Cosine Similarity)**: Scans 279+ skills using Candle BERT vector embeddings with precomputed binary vector acceleration ($<5\text{ ms}$).
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
  --setup             Install and configure MCP server across all IDE clients
  --verify-setup      Verify MCP configuration paths in all IDE clients
  --update            Sync and download 3rd-party skill repositories into ~/.agent-guidance/skills
  --upgrade           Pull latest source from git, rebuild release binary, and update all IDE configs
  --self-update       Alias for --upgrade
  --dashboard         Start real-time web usage dashboard at http://127.0.0.1:3000
  --uninstall         Remove MCP server configurations from all IDE clients
  --help, -h          Print help message
```

---

## 📚 Documentation Index

Comprehensive guides, architecture deep-dives, and client setup instructions are available in the [`docs/`](docs/) directory:

| Section | Topic | Documentation Link |
| :--- | :--- | :--- |
| **Architecture** | System Design & Lifecycles | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) |
| **Getting Started** | Quickstart & Overview | [`docs/getting-started.md`](docs/getting-started.md) |
| **Installation** | Platform Setup & Upgrades | [`docs/installation.md`](docs/installation.md) |
| **Usage Guide** | Orchestrator & Workflow Usage | [`docs/usage.md`](docs/usage.md) |
| **Development** | Contributing & Testing | [`docs/development.md`](docs/development.md) |
| **IDE Setup** | Antigravity, Cursor, VS Code, Windsurf | [`docs/setup/`](docs/setup/) |
| **Skills Guide** | Skill Anatomy & Catalog Policy | [`docs/skills/SKILLS_OVERVIEW.md`](docs/skills/SKILLS_OVERVIEW.md) |
| **Reference** | MCP Surface & Protocol Spec | [`docs/reference/mcp-surface.md`](docs/reference/mcp-surface.md) |

---

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for details.
