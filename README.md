# Agent Guidance (Rust MCP Server)

[![Version](https://img.shields.io/badge/Version-v1.2.2-blue.svg)](Cargo.toml)
[![Rust 2024](https://img.shields.io/badge/Rust-2024-orange.svg)](https://www.rust-lang.org/)
[![Role](https://img.shields.io/badge/Role-Orchestrator%20Manager-indigo.svg)](#-core-purpose--system-architecture)
[![Smart Skills](https://img.shields.io/badge/Smart%20Skills-168%2B%20ML%20Search-cyan.svg)](#-smart-skills-system)
[![MCP Protocol](https://img.shields.io/badge/MCP-2024--11--05-green.svg)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

![Agent Guidance Orchestrator Manager](docs/images/hero-banner.png)

> **Agent Guidance** is a native, high-performance **MCP (Model Context Protocol) Server & Orchestrator Manager** written in Rust. It supervises AI Coding Agents (Antigravity, Claude, Codex, Windsurf, OpenCode) to strictly enforce enterprise architecture patterns and deliver intelligent **Smart Skills Calls** via local ML vector search.

---

## 🎯 Core Purpose & System Architecture

1. **System-Wide Agent Guidance (Orchestrator System)**:
   - Orchestrates the entire AI agent lifecycle across all operations (`task_pipeline`, `workflow_gate`, `require_edit_approval`, `project_context`, `session_continuity`, and priority gates).
   - Serves as an active **Manager & Supervisor** enforcing architectural boundaries (**Clean Architecture**, **Layered Architecture**, **Package-by-Feature**, and **Orchestrator**) upfront across all development phases.
2. **Smart Skills Calls (Local ML Vector Search)**:
   - Features an intelligent 2-stage hybrid search engine (Candle BERT vector embeddings + Cross-Encoder reranking) to dynamically discover and route queries to over **168+ specialized skills** on-demand.

---

## 🏗️ Architectural Workflow

![Orchestrator Workflow Flowchart](docs/images/orchestrator-flow.png)

### The 7-Stage Workflow Gate
`Agent Guidance` governs the AI agent lifecycle through strict stage transitions:

`Context` $\longrightarrow$ `Plan` $\longrightarrow$ `Ask_Revise` $\longrightarrow$ `Build` $\longrightarrow$ `Test_Recheck` $\longrightarrow$ `Fix` $\longrightarrow$ `Proposal`

- **Hard Edit Gate (`require_edit_approval`)**: Code modification is BLOCKED until `plan_approved = true` and the agent explicitly declares a valid `architecture_pattern`:
  - `Clean_Architecture`
  - `Layered_Architecture`
  - `Package_By_Feature`
  - `Orchestrator`
- **Circuit Breaker**: If 3 consecutive fix attempts fail during `Fix`, the MCP server automatically trips, resets stage to `Ask_Revise`, and requests human intervention.

---

## 🧠 Smart Skills System

![Smart Skills Routing System](docs/images/smart-skills-routing.png)

The built-in ML catalog engine leverages local Rust bindings for Hugging Face `candle` to perform sub-millisecond semantic skill discovery:

- **Stage 1 (Cosine Similarity)**: Scans 168+ skills using Candle BERT vector embeddings.
- **Stage 2 (Intent Reranking)**: Cross-encoder reranks top candidates based on domain context.
- **On-Demand Loading**: Skills are injected dynamically into context only when needed.

---

## 🚀 Quickstart & Setup

### Automatic Installation

Run the one-liner setup script for your operating system to compile the release binary and auto-register `agent-guidance` across installed IDEs:

**Windows (PowerShell):**
```powershell
powershell -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.ps1 | iex"
```

**Windows (CMD Prompt):**
```cmd
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.ps1 | iex"
```

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.sh | bash
```

### Manual Build via Cargo

```bash
git clone https://github.com/JunMystery/Agent-Guidance-Rust.git
cd agent-guidance
cargo build --release
```

The compiled binary will be placed at `./target/release/agent-guidance`.

---

## 🛠️ MCP Tool Suite Reference

| Tool Name | Action / Role | Mandatory Arguments |
| :--- | :--- | :--- |
| `task_pipeline` | **Call First**: Prepares recommendations, project tree, and phase context. | `task`, `project_path`, `phase` |
| `workflow_gate` | **Manager Gate**: Manages stage transitions and plan approval status (`check`, `set_stage`). | `action` |
| `require_edit_approval` | **Architecture Gate**: Authorizes code edits under declared architecture pattern. | `project_path`, `risk_level`, `justification`, `architecture_pattern` |
| `guidance` | **Smart Skills & Rules**: Executes 2-stage vector search for skills & pre-code checklists. | `operation` (`search` / `precode` / `verify`) |
| `project_context` | **Bounded Context**: Reads symbol signatures and directory trees under strict LOC budgets. | `operation` (`tree` / `read` / `symbols`) |
| `session_continuity` | **Session State**: Saves and restores task states across agent invocations. | `operation` (`save` / `load`) |

---

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for details.
