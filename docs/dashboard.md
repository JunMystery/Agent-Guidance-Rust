# 📊 Agent Guidance Web Dashboard & Visual GraphRAG

Agent Guidance includes an embedded, high-performance telemetry dashboard and interactive GraphRAG architecture visualizer. The dashboard runs directly inside the singleton daemon process via `tiny_http` with zero external web server or Node.js runtime dependencies.

---

## 🌐 Accessing the Dashboard

By default, the dashboard runs at:
```text
http://127.0.0.1:11997
```

### CLI Options

You can launch the dashboard or customize its listening port using the following flags:

```bash
# Start dashboard and open in default browser
agent-guidance --dashboard

# Specify a custom port (via flag or alias)
agent-guidance --dashboard --port 12345
agent-guidance --dashboard --dashboard-port 12345
agent-guidance --dashboard -p 12345

# Scope dashboard to a specific project
agent-guidance --dashboard --project "e:\Github\MyProject"
```

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `AGENT_GUIDANCE_DASHBOARD_PORT` | `11997` | Port for the embedded telemetry dashboard and REST API |

---

## 🖥️ Views & Features

The web dashboard is organized into 4 primary views accessible via the responsive sidebar:

### 1. Dashboard (Overview)
- **Token Efficiency & Velocity Dynamics**: Visual charts tracking original prompt token waves vs. compressed outgoing payloads, calculating real-time token reduction percentages (typically 30–50% savings).
- **Executive Metric Cards**:
  - Total MCP Tool Calls executed across sessions.
  - Raw tokens ingested vs. compressed tokens returned.
  - Overall token savings percentage.
  - Peak processing velocity (tokens/sec).
  - Active IDE client sessions.
- **Recent Activity Summary**: Quick-glance stream of recently executed tool actions and active workflow stages.

### 2. Actions Log
- **Execution Telemetry**: Detailed, filterable audit log of every MCP tool invocation (`task_pipeline`, `workflow_gate`, `project_context`, `select_skills`, `guidance`, `session_continuity`).
- **Configurable Pagination**: Choose between 10, 20, 50, or 100 entries per page (default: 10).
- **Search Query**: Real-time client-side search box to filter logs by tool name, file path, stage, or error message.
- **Log Management**: Quick button to clear historical logs (`POST /api/logs/clear`).

### 3. Architecture Graph (Visual GraphRAG)
- **Interactive ForceAtlas2 Physics Simulation**: Full-canvas visual graph rendering AST symbols (functions, structs, classes, enums, traits), cross-file import dependencies, and Leiden hierarchical community clusters.
- **Visual Contrast & Halos**: Node clustering with contrast halos and collision culling for smooth navigation across codebases with thousands of symbols.
- **Deep Symbol & Blast Radius Inspector**:
  - Click any node to open the inspector panel.
  - View incoming callers (`callers`), outgoing dependencies (`callees`), and architectural layer classification.
  - View **Blast Radius Risk Score** assessing downstream impact before making code modifications.
- **Parallel Symbol Search**: Search symbols in real time to locate nodes and center the physics camera.

### 4. Diagnostics
- **System Health & Runtime Info**: Daemon uptime, memory usage, CPU/GPU compute provider (`CPU`, `CUDA`, `DirectML`, `Metal`).
- **IDE Process Detector**: Status of connected IDE client instances and detected IDE processes (monitoring 26 supported IDE binaries).
- **ML Engine Status**: Candle BERT embeddings engine status, loaded ONNX models, and vector cache stats.
- **Filesystem Watcher**: Active watches, debounce status, and queue depth for real-time AST re-indexing.

---

## ⌨️ Sidebar & Navigation Shortcuts

- **Sidebar Toggle**: Click the sidebar hamburger toggle or press `Ctrl + B` to expand/collapse the navigation sidebar.
- **Fixed Viewport Lock**: The sidebar is styled with fixed viewport height, preventing the footer or controls from jumping out of view on long content pages.
- **Bilingual Interface (i18n)**: Seamless instant switching between English (`en`) and Vietnamese (`vi`) with preference saved in local storage.

---

## 🔌 REST API Reference

The embedded HTTP server exposes a complete REST API for programmatic telemetry access, CI/CD integration, and dashboard automation:

| Method | Endpoint | Description |
|---|---|---|
| `GET` | `/api/stats` | Aggregated metrics (tool call counts, token savings, duration, active sessions). Accepts optional `?project=<path>`. |
| `GET` | `/api/projects` | List of all registered and tracked repositories in `~/.agent-guidance/usage.db`. |
| `POST` | `/api/projects/prune` | Remove stale or missing projects from the registry. |
| `GET` | `/api/graph` | Code graph data (nodes, edges, communities, blast radius metrics). Requires `?project=<path>`. |
| `POST` | `/api/cleanup` | Trigger database vacuuming, expired log pruning, and dead project cleanup. |
| `GET` | `/api/logs` | Paginated action logs. Parameters: `page` (default 1), `limit` (default 10), `search` (optional keyword). |
| `POST` | `/api/logs/clear` | Clear all execution action logs from the database. |
| `POST` | `/api/engine/refresh` | Trigger immediate on-demand AST symbol indexing and GraphRAG re-clustering for active project. |

### Example API Request

```bash
# Fetch aggregate telemetry statistics
curl http://127.0.0.1:11997/api/stats

# Fetch action logs with pagination and search
curl "http://127.0.0.1:11997/api/logs?page=1&limit=20&search=workflow_gate"
```

---

## 🛠️ Related Documentation

- [Getting Started](getting-started.md)
- [Architecture & Daemon Design](ARCHITECTURE.md)
- [Client Configuration](setup/client-configuration.md)
- [MCP Surface Reference](reference/mcp-surface.md)
