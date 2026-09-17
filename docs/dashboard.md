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

### Performance & Zero-Freeze Telemetry Loop
- **Adaptive 5s Polling Cadence**: Chained `setTimeout` execution (`scheduleNextPoll()`) replaces `setInterval` loops, guaranteeing zero overlapping requests and reducing telemetry network traffic by 80%.
- **Telemetry Data-Hash Dirty Checking**: Incoming payloads are fingerprinted client-side; if telemetry numbers haven't changed, all DOM table and SVG re-renders are completely bypassed (0% idle CPU overhead).
- **Lightweight SVG Charts**: Streamlined vector paths without software Gaussian blur filters ensure smooth 60+ FPS rendering on low-power and legacy hardware.
- **Dynamic View Unmounting**: Inactive heavy subtrees (such as the 6,000+ node Graph file tree) are automatically unmounted when navigating to the Dashboard view, keeping document DOM size under 850 nodes.

### 1. Dashboard (Overview)
- **Agent Invocations & Latency Wave**: Dual-axis spline chart displaying hourly tool invocations (left Y-axis) correlated with average execution latency in milliseconds (right Y-axis), tracking activity against the user's system timezone across the past 24 hours.
- **Executive Metric Cards & KPI Header**:
  - Total Operations / Tool Invocations executed across sessions.
  - Ingestion Velocity (Saved vs. Ingestion tokens).
  - Average execution latency (ms).
  - Context reduction percentage (typically 30–50% savings).
  - Active IDE client sessions.
- **Multi-Repository Project Root Resolution**: Automatic detection of project boundaries using `.agent-context/` or `.git/` anchors, ensuring nested folders within a repository map to the true project root.
- **Recent Activity Summary**: Quick-glance stream of recently executed tool actions and active workflow stages.

### 2. Actions Log
- **Execution Telemetry**: Detailed, filterable audit log of every MCP tool invocation (`task_pipeline`, `workflow_gate`, `project_context`, `select_skills`, `guidance`, `session_continuity`).
- **Configurable Pagination**: Choose between 10, 20, 50, or 100 entries per page (default: 10).
- **Search Query**: Real-time client-side search box to filter logs by tool name, file path, stage, or error message.
- **Log Management**: Quick button to clear historical logs (`POST /api/logs/clear`).

### 3. Architecture Graph (Visual GraphRAG)
The visualizer offers 3 specialized modes to explore and analyze codebase architecture at multiple granularities:

- **Mode 1: File Dependencies (1-N & N-1)**:
  - **File Architecture Graph**: Visualizes high-level inter-file dependencies derived from AST symbol calls and semantic imports, with node sizes proportional to file complexity (LOC and symbol density).
  - **Interactive Subgraph Isolation**: Selecting a file node isolates the graph to show only that file and its immediate incoming (N-1 dependents) and outgoing (1-N dependencies). Click background to reset filter.
  - **Directory & File Container Navigator**: Collapsible navigator (`#graph-symbol-list-container`) grouping files by folder, with auto-centering camera navigation.
  - **Anti-Distortion Layout**: Fixed aspect ratio and dynamic `ResizeObserver` maintaining visual stability during panel resizing.
  - **Quick Drill-Down**: Direct button to jump from any file node directly into its function call graph in Mode 2.

- **Mode 2: File Functions Drill-Down & Global Call Graph**:
  - **Single-File Function Call Isolation**: Visualizes all functions/methods inside a chosen file within a central container/aura, connecting to external caller and callee functions.
  - **Directed Call Arrows**: Color-coded directional edges distinguishing outgoing calls (neon cyan/green) from incoming callers (purple/orange).
  - **Global 'Show All' View**: Analyze the complete cross-file call graph across the entire project (`file=all`).
  - **Searchable Combobox (`fileCombobox.js`)**: Real-time dropdown search by file or folder path with keyboard navigation (Arrow Up/Down, Enter, Esc) and pinned `'Show all'`.

- **Mode 3: Symbol Graph**:
  - Full-canvas granular AST symbol graph (functions, structs, classes, enums, traits) with Leiden community clustering and blast radius impact analysis.

- **Standardized 10% Dim Capacity**:
  - In all visualizer modes, selecting a node dims all unrelated nodes and edges to exactly 10% opacity (`globalAlpha = 0.10`), keeping the active subgraph crisp at 100% brightness.

- **Enforced Project Selection**:
  - Graph visualizer strictly requires an explicit project selection from the Tracked Projects dropdown (`/api/graph?project=...`). When Global (All) is active, a friendly selection guide (`📂 Select a Project to View Graph`) is displayed until a project is chosen.

- **Bilingual Interface (i18n Parity)**:
  - Full localization support across toolbars, legends, inspector panels, search combobox, and metrics in both English and Vietnamese.

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
| `GET` | `/api/projects` | List of all registered repositories in `~/.agent-guidance/usage.db`, automatically resolved to their project roots. |
| `POST` | `/api/projects/prune` | Remove stale, missing, or redundant child directory records from the registry. |
| `GET` | `/api/graph` | Code graph data. Parameters: `project` (required), `view` (`symbols`, `files`, `file_functions`), `file` (relative path or `all` for `file_functions`). |
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
