# Agent Guidance (Rust MCP Server)

[![Version](https://img.shields.io/badge/Version-v1.3.6-blue.svg)](Cargo.toml)
[![Rust 2024](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)
[![Role](https://img.shields.io/badge/Role-Autonomous%20Orchestrator-indigo.svg)](#-core-purpose--system-architecture)
[![Smart Skills](https://img.shields.io/badge/Smart%20Skills-279%2B%20ML%20Search-cyan.svg)](#-smart-skills-system)
[![Multi-Session Isolation](https://img.shields.io/badge/Multi--Session-Isolated-green.svg)](#-multi-session-isolation--session-gc)
[![Universal Token Optimization](https://img.shields.io/badge/Token%20Opt-300%20LOC%20Clamped-purple.svg)](#-universal-token-saving--optimization)
[![MCP Protocol](https://img.shields.io/badge/MCP-2024--11--05-green.svg)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

![Agent Guidance Orchestrator Manager](docs/images/hero-banner.png)

> **Agent Guidance** is a native, high-performance **MCP (Model Context Protocol) Server & Autonomous Orchestrator** written in Rust. It supervises AI Coding Agents (Antigravity, Claude, Codex, Windsurf, OpenCode) to strictly enforce enterprise architecture patterns, isolate multi-IDE session states, prevent context window blowups via universal token compression, and deliver intelligent **Smart Skills Calls** via local ML vector search.

---

## 🎯 Core Purpose & System Architecture

1. **Autonomous Single-Entrypoint Orchestration**:
   - Governs the complete AI agent lifecycle through a single entrypoint (`task_pipeline`). The MCP server inspects the workspace, unlocks priority gates, selects skills, and dynamically directs next steps.
   - Enforces architectural styles (**Clean Architecture**, **Layered Architecture**, **Package-by-Feature**, **CLI Pipeline**, **Flat Library**, **Orchestrator**) with cross-session persistence in `.agent-context/architecture.json`.
2. **Hardened 300 LOC Cap & Upfront Decomposition**:
   - Physically clamps file reads at 300 lines max and automatically injects architectural decomposition mandates on large files.
   - Generates concrete Upfront Split Blueprints per pattern in pre-code guidance.
3. **Universal In-Engine Token Compression**:
   - Automatically intercepts and compresses all outgoing MCP tool responses, stripping HTML comments, badges, and redundant whitespace, while logging accurate token savings to SQLite (`~/.agent-guidance/usage.db`).
4. **Multi-Session & Multi-IDE Isolation**:
   - Assigns process-isolated Session IDs (`session_{PID}_{ClientName}`) to eliminate state collisions across concurrent IDEs (VS Code, Cursor, Antigravity) or CLI tools in the same codebase.
5. **Smart Skills System (Local ML Vector Search)**:
   - Features a 2-stage hybrid search engine (Candle BERT vector embeddings + Cross-Encoder reranking) dynamically routing queries across **279+ specialized skills** on-demand.

---

## 🏗️ Architectural Workflow

![Orchestrator Workflow Flowchart](docs/images/orchestrator-flow.png)

### The 7-Stage Workflow Gate
`Agent Guidance` governs the AI agent lifecycle through strict stage transitions:

`Context` $\longrightarrow$ `Plan` $\longrightarrow$ `Ask_Revise` $\longrightarrow$ `Build` $\longrightarrow$ `Test_Recheck` $\longrightarrow$ `Fix` $\longrightarrow$ `Proposal`

- **Composite Gate Action (`workflow_gate action="advance"`)**: Performs stage check, transition, and architecture pattern authorization in a single composite MCP call.
- **Hard Edit Gate (`workflow_gate action="authorize_edit"`)**: Code modification is BLOCKED until `plan_approved = true` and a valid `architecture_pattern` is verified:
  - `Auto` *(Default: Auto-detects and memorizes pattern)*
  - `Clean_Architecture`
  - `Layered_Architecture`
  - `Package_By_Feature`
  - `CLI_Pipeline`
  - `Flat_Library`
  - `Orchestrator`
- **Security & Idempotency**:
  - `workflow_gate(action="check")` is strictly read-only and idempotent.
  - Multi-lingual user intent recognition (English & Vietnamese approval keywords).
  - Empirical verification contracts require real test execution output (`verification_passed = false` until verified).
- **Circuit Breaker**: If 3 consecutive fix attempts fail during `Fix`, the MCP server automatically trips, resets stage to `Ask_Revise`, and requests human intervention.

---

## ⚡ Universal Token Saving & Optimization

- **Hard Clamping**: Capped at 300 LOC per file read, 20 results per search, 30 references per symbol search, and 15 items per tree preview.
- **Symbol-Targeted Extraction**: Extract exact function/struct blocks using `project_context(operation="read", target_symbol="...")` saving up to 85% of tokens.
- **Dynamic Compression**: Automatic stripping of markdown comments, badges, and empty lines across all responses.
- **SQLite Analytics**: All tool metrics, durations, and token savings are logged to `~/.agent-guidance/usage.db`.

---

## 🔒 Multi-Session Isolation & Session GC

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

## 🧠 Smart Skills System

The built-in ML catalog engine leverages local Rust bindings for Hugging Face `candle` to perform sub-millisecond semantic skill discovery:

- **Stage 1 (Cosine Similarity)**: Scans 279+ skills using Candle BERT vector embeddings.
- **Stage 2 (Intent Reranking)**: Cross-encoder reranks top candidates based on domain context.
- **On-Demand Loading**: Skills are injected dynamically into context via `select_skills(skills=[...])` only when confirmed.

---

## 🚀 Quickstart & Setup

### Automatic Installation

Run the one-liner setup script for your operating system to compile the release binary and auto-register `agent-guidance` across installed IDEs:

**Windows (PowerShell / CMD):**
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
```

The compiled binary will be placed at `./target/release/agent-guidance`.

---

## 🛠️ MCP Tool Suite Reference

| Tool Name | Action / Role | Mandatory Arguments | Key Features |
| :--- | :--- | :--- | :--- |
| `task_pipeline` | **Call First**: Entrypoint orchestrator. Initializes context, scans project, unlocks priority gate, and proposes skills. | `task`, `project_path`, `phase` | Auto-resets permissions on new plan phase, injects short-term rules. |
| `select_skills` | **Confirm Skills**: Confirms and loads requested skill markdown instructions. | `skills` | Loads selected skills from 2-stage proposals. |
| `workflow_gate` | **Manager Gate**: Manages stage transitions (`check`, `status`, `set_stage`, `set_architecture`, `advance`, `authorize_edit`). | `action` | Composite `advance`, persistent architecture locking, circuit breaker. |
| `project_context` | **Bounded Context**: Reads symbols, references, and directory trees under strict LOC budgets. | `operation` (`tree` / `read` / `search` / `symbols` / `references` / `architecture`) | Symbol-targeted extraction, hard 300 LOC cap, decomposition warnings. |
| `guidance` | **Skills & Rules**: Executes 2-stage vector search for skills, pre-code blueprints, & verification contracts. | `operation` (`search` / `docs` / `workflow` / `precode` / `verify`) | Dynamic upfront split blueprints & empirical verification contracts. |
| `session_continuity` | **Session State**: Saves and restores task states across agent invocations. | `operation` (`save` / `load` / `clear`) | Session-isolated persistence with 30-day GC. |

---

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for details.
