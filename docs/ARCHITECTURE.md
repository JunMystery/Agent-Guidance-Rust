# Architecture

[Back to README](../README.md)

## Overview

Agent Guidance MCP is a **100% Native Rust 2024 Edition** MCP server that gives AI coding agents standards guidance, skill references, workflow prompts, and bounded project code context over Stdio transport. It enforces a priority gate so `agent-guidance-mcp_task_pipeline` is always called before gated tools.

---

## Rust Module Map

```
src/
├── main.rs            # Binary entrypoint (CLI args: --setup, --dashboard, --update, --session-start)
├── catalog/           # Skills catalog management
│   ├── store.rs       # Embedded rust_embed skills + workspace-local (.agents/skills) scanning
│   ├── updater.rs     # Async auto-updater for 3rd-party skill repositories
│   └── mod.rs
├── context/           # Project context & indexing
│   ├── scanner.rs     # Bounded workspace scanner & ignore filter
│   ├── db.rs          # SQLite FTS5 code symbol indexing & usage database
│   └── mod.rs
├── dashboard/         # Native HTTP usage dashboard server & embedded HTML frontend
│   └── mod.rs
├── mcp/               # Model Context Protocol engine
│   ├── protocol.rs    # JSON-RPC request & response structs
│   ├── router.rs      # Tool dispatcher & resource router
│   ├── state.rs       # ServerState priority gate, stage matrix & circuit breaker
│   ├── tools.rs       # Tool handlers (task_pipeline, guidance, project_context, etc.)
│   ├── config.rs      # IDE client auto-registration & tagged block section deployment
│   ├── templates.rs   # Embedded AGENTS.md rules & SKILL.md templates
│   └── mod.rs
├── ml/                # Machine learning & vector search
│   ├── embeddings.rs  # Candle BERT (intfloat/multilingual-e5-small) embedding engine
│   ├── llm_selector.rs# 2nd-stage context & intent re-ranker
│   └── mod.rs
└── optimizer/         # Token optimization engine
    ├── compressor.rs  # Language-aware token compressor & comment stripper
    └── mod.rs
```

---

## Enforcement Architecture

### Priority Gate (3 layers)

```
Session startup
  └─ Layer 1: Session-start hook
       hooks/session-start.sh → agent-guidance-mcp --session-start
       → Writes ~/.agent-guidance/.gate_passed sentinel
       → Injects project context JSON into AI conversation

MCP server starts
  └─ Layer 2: create_server() reads sentinel
       → _gate_sentinel_check(deploy_root) validates project path
       → Sets _priority_gate_passed = True, clears sentinel file
       → Bridges hook process and server process

Tool call
  └─ Layer 3: priority_gate_check(project_path)
       → Checks _priority_gate_passed (in-memory flag)
       → Fallback: _gate_sentinel_check(project_path) validates sentinel
       → If both fail: returns PRIORITY_REQUIRED error
```

### Tool Gate Status

| Tool | Built-in | Behavior |
|---|---|---|
| `agent-guidance-mcp_task_pipeline` | ✅ | **Unlocks** gate via `priority_gate_pass()` |
| `agent-guidance-mcp_guidance` | ✅ | Gated — blocked before `agent-guidance-mcp_task_pipeline` |
| `agent-guidance-mcp_project_context` | ✅ | Gated — blocked before `agent-guidance-mcp_task_pipeline` |
| `agent-guidance-mcp_ui_ux` | ✅ | Gated — blocked before `agent-guidance-mcp_task_pipeline` |
| `agent-guidance-mcp_session_continuity` | ✅ | Gated — blocked before `agent-guidance-mcp_task_pipeline` |
| `agent-guidance-mcp_workflow_gate` | ✅ | Gated — blocked before `agent-guidance-mcp_task_pipeline` |
| `agent-guidance-mcp_require_edit_approval` | ✅ | Not gated — callable anytime (delegates to workflow stage check) |
| `agent-guidance-mcp_usage_report` | ✅ | Not gated — callable anytime |
| `agent-guidance-mcp_health_check`, `agent-guidance-mcp_diagnose`, `agent-guidance-mcp_token_stats` | ✅ | Whitelisted — always open |

---

## Usage Tracking & Dashboard Recording

Token efficiency is measured by two independent trackers, intentionally
separate (design note F8):

| Tracker | Module | Lifetime | Purpose |
|---|---|---|---|
| `TokenTracker` (in-memory) | `token_analytics.py` | Per process, ephemeral | Live token savings; surfaced via `agent-guidance-mcp_token_stats` |
| `UsageTracker` (persistent) | `usage.py` | Append-only SQLite, 30-day retention | Drives the `--dashboard`; survives restarts |

### Persistent recorder (`usage.py`)

`UsageTracker` writes to a single global SQLite DB at `~/.agent-guidance/usage.db`.
Writes are queued to a background flusher thread so the tool path is never
blocked. Each `UsageTracker` instance is tagged with a `run_id` (`uuid4().hex`);
the server creates one instance per process, so `run_id` identifies a process run.

The stats API (`_query_stats` in `dashboard_server.py` and `UsageTracker.summary` in `usage.py`) retrieves grouped tool breakdown aggregates, raw `recent_actions` (the last 20 raw tool calls ordered by timestamp `started_at` DESC) to drive the call-by-call dashboard section, and a `hourly_savings` array for the 24h combo chart.

### 24-hour hourly aggregation (`hourly_savings`)

`_query_stats` treats the `tool_calls` table as a **rolling 24h window**. On
every `/api/stats` call it:

1. **Auto-cleanup** — deletes rows older than 24h
   (`DELETE FROM tool_calls WHERE started_at < now - 86400`). The historical
   30-day retention from `UsageTracker` is intentionally NOT applied to the
   dashboard view; the hourly chart only ever spans the last 24 hours.
2. **Per-hour buckets** — groups surviving rows by `(started_at / 3600)` and
   sums `tokens_original`, `tokens_optimized`, and their delta (`saved`) per
   hour.
3. **Emits 24 sequential buckets ending at the current hour**
   (`(now//3600) - 23 … now//3600`), each tagged with its local `date`
   (`YYYY-MM-DD`) and `hour` (0–23). The last bucket is always the current hour
   (`is_current: true`). Buckets with no activity carry zeros, so the chart has
   a fixed 24-column axis.

The frontend renders this as a **dual-axis combo chart** (inline SVG, no
dependencies):

- **Saved** → vertical **columns** on the **left Y-axis** (blue).
- **Original / Optimized** → overlaid **polylines + dots** on the **right Y-axis**
  (orange / green), each scaled to its own max so Saved is never flattened.
- A thin vertical **separator** line marks where the calendar `date` changes
  between adjacent buckets; the **current hour** is highlighted with a "now"
  pill and brighter fill.
- Added UX: a 4-cell **KPI strip** (Original / Optimized / Saved / Savings%), a
  titled chart card, light horizontal **gridlines**, axis unit labels, and a
  styled **hover/focus tooltip** (keyboard-accessible, `prefers-reduced-motion`
  respected).

### Dashboard views & navigation

The standalone dashboard (`dashboard_server.py`, pure stdlib HTTP) serves HTML
from `dashboard_src/` written to `~/.agent-guidance/dashboard/`. Current view
layout (sidebar order):

- **Dashboard** (`view-dashboard`) — session overview + top skills, **plus**
  the **Embed Status** block (model/daemon status + last 20 embed queries,
  refresh button) inlined at the end of the view.
- **Actions Log** (`view-actions`) — tool-breakdown table **plus** the Token
  Savings block (KPI strip + lifetime summary bars + 24h dual-axis combo chart).
  Token Savings was merged into this view and removed from the sidebar.
- **Recent Calls** (`view-recent-calls`) — last 20 raw `tool_calls`.
- **Quick Guides** (`view-guides`) — four guide cards **plus** the **MCP Tools**
  table inlined at the end of the view. Both Embed Status and MCP Tools content
  are inlined into other views and have **no sidebar entries** of their own.

The `~/.agent-guidance/dashboard/` assets are regenerated by
`write_default_dashboard` whenever the server starts.

Tables:
- `tool_calls` — one row per tool invocation: `tool_name`, `operation`,
  `tokens_original`, `tokens_optimized`, `duration_ms`, `project_path`,
  `run_id`, `error_message`.
- `skill_loads` — `record_skill_load` calls (e.g. `agent-guidance-mcp_guidance(get)`, workspace
  skills, and LLM `recommend` picks), with `project_path` / `run_id`.
  `embed_used = 1` flags loads backed by an e5 embed query (F2/F5/F6):
  `recommend` picks are recorded as loads with `embed_used = 1`; bulk
  `search` hits are intentionally NOT counted to avoid inflating the
  "skills called" metric with candidate lists.
- `embed_queries` — semantic search queries (`agent-guidance-mcp_guidance(search/recommend)`),
  recorded ONLY when a real vector is computed (semantic success), with
  `model_name` / `vector_dim` / `result_count` / `run_id`. Keyword-only
  fallback (no vector) does NOT write a row (F3/F4).
- `llm_queries` — LLM skill-selector queries (Qwen2.5-0.5B-Instruct used by
  `agent-guidance-mcp_guidance(recommend)`), with `model_name` / `duration_ms` / `result_count`
  / `run_id` (F1).

### Single recording path (no double counting)

Every tool records exactly ONE `tool_calls` row. The row is written inside the
pipeline via `_record_savings()` (in `pipeline_helpers.py`), which persists both
the in-memory token savings and the persistent
`usage.record_tool_call(..., tokens_original=, tokens_optimized=,
project_path=)`. The server `@mcp.tool` handlers previously emitted a second
NULL-token row through a separate `_track_usage` writer; that duplicate path was
removed so the dashboard's `COUNT(*)` reflects real call counts.

- `agent-guidance-mcp_task_pipeline` → recorded once as `("task_pipeline", "run")`.
- `agent-guidance-mcp_guidance` / `agent-guidance-mcp_project_context` / `agent-guidance-mcp_ui_ux` / `agent-guidance-mcp_session_continuity` → recorded
  inside their pipelines. `agent-guidance-mcp_session_continuity` records a single row whose
  original == optimized, so savings read 0 (the payload is small control JSON).
- `agent-guidance-mcp_guidance(operation="workflow")` → recorded via `_record_savings` (the
   standalone `workflow`/`workflow_prompt` tools were consolidated into `guidance`).
- Resolved transitive skill dependencies (`agent-guidance-mcp_guidance(resolve_dependencies=True)`)
  are each recorded via `record_skill_load`.

### Error-path recording

Handlers wrap the pipeline call in `try/except`; on failure `_track_error()`
records a `tool_calls` row carrying the exception message in `error_message`
(duration still captured), so failed calls are never dropped from the dashboard.

### Attribution

`project_path` (the tool's `project_path` argument) and `run_id` let the
dashboard group calls by project and by process run. `agent-guidance-mcp_guidance`/`agent-guidance-mcp_ui_ux` handlers
currently record with `project_path=None` (global) because they expose no
`project_path` argument; `agent-guidance-mcp_task_pipeline`/`agent-guidance-mcp_project_context`/`agent-guidance-mcp_session_continuity`
carry the real path.

---

## Key Flows

### Installation Flow

```
install.sh / install.ps1
  ├─ Install uv (Python toolchain)
  ├─ Install agent-guidance-mcp via uv tool
  ├─ agent-guidance-mcp --setup
  │   ├─ Register MCP client in IDE configs (Claude, Cursor, VS Code, etc.)
  │   ├─ Deploy AGENT_RULES_BLOCK to global AGENTS.md and workspace rule files
  │   └─ Deploy agent-guidance/SKILL.md to all skill directories
  └─ agent-guidance-mcp --update
      ├─ Pre-download embedding model (cached, skip if present)
      ├─ For each 3rd-party repo: query GitHub API for latest commit SHA
      │   ├─ SHA matches cache AND target dir exists → skip download
      │   └─ Otherwise → git clone / zip download → extract → save SHA
      └─ Save update state to ~/.agent-guidance/.update-state.json
```

### Session-Start Flow

```
CLI agent starts
  └─ hooks/session-start.sh (Claude Code)
       ├─ agent-guidance-mcp --session-start --project-path $PWD
       │   ├─ Build catalog
       │   ├─ priority_gate_pass() + _gate_sentinel_write()
       │   ├─ Run task_pipeline for default context
       │   └─ Output JSON: {priority, message}
       └─ JSON injected into AI conversation (project context visible)

Non-hook clients (OpenCode, Gemini, etc.)
  └─ Gate is locked on startup
  └─ Agent must call task_pipeline first (enforced by PRIORITY_REQUIRED error)
```

### Tool Call Flow

```
AI calls gated tool (guidance, project_context, etc.)
  ├─ handler() → priority_gate_check(catalog.root)
  │   ├─ _priority_gate_passed? → pass
  │   └─ _gate_sentinel_check(root)? → pass + clear sentinel
  │       └─ None → return PRIORITY_REQUIRED
  └─ pass → execute pipeline → return result
```

### Search Flow (3-tier fallback)

```
project_context(operation="search", query="...")
  ├─ FTS5 (SQLite full-text index) → resolves ~90% of queries
  │   └─ Empty/no results →
  ├─ Tier 1: Documentation + manifests (README, ARCHITECTURE, pyproject.toml, etc.)
  │   └─ Empty →
  ├─ Tier 2: Structural + config (Dockerfile, main.py, tsconfig, tests/, src/)
  │   └─ Empty →
  └─ Tier 3: General code files (capped at 300, parallel scan)
      └─ Return ranked matches
```

### Semantic Skill Search (embeddings pipeline)

`agent-guidance-mcp_guidance(operation="search")` blends keyword matching with vector cosine
similarity over `intfloat/multilingual-e5-small` (384-dim, `passage:` / `query:`
prefixes, normalized). Implementation notes:

- **Hybrid ranking** (`catalog.search_entries`): keyword score uses
  word-boundary matching across title/description/path/content; the semantic
  score (clamped to ≥ 0) is scaled ×150 and added. An entry is dropped only when
  *both* signals are empty, so a negative cosine can never suppress a
  keyword-relevant result.
- **Scope**: any catalog entry that has a precomputed vector is ranked
  semantically — not just skills. Documents/standards gain semantic ranking
  once they are embedded (see regeneration below).
- **Staleness auto-heal** (`catalog._ensure_local_skills_embedded`): each
  precomputed entry stores a content hash in `skills_embeddings.json`
  (`__meta__.hashes`). On first search, entries whose source changed are
  re-embedded and the file is rewritten atomically. Entries with no vector
  (workspace-local skills) are embedded on demand. Persistence is skipped under
  `pytest` so the bundled file is never mutated by tests.
- **Daemon diagnostics** (`embeddings.py`): if the shared embedding daemon fails
  to spawn, its stdout/stderr are appended to `~/.agent-guidance/daemon.log`
  instead of being discarded.
- **Telemetry**: `agent-guidance-mcp_guidance(search/recommend)` records each query to
  `embed_queries` with `prefix_type = "query"` — but ONLY when a real e5
  vector is computed (semantic success). Keyword-only fallback paths write
  no `embed_queries` row (F3/F4). The LLM `recommend` selector additionally
  records one `llm_queries` row per call (F1).

**Regeneration required for document embeddings (F4-A):** the bundled
`src/agent_guidance_mcp/skills_embeddings.json` is generated by
`scripts/generate-catalog-embeddings.py`, which now embeds *every* catalog entry
(skills + documents) and writes the `__meta__.hashes` block. After changing
skill/document content — or to enable semantic ranking for documents — re-run:

```bash
python scripts/generate-catalog-embeddings.py
```

Without regeneration the existing skills-only file still works (semantic for
skills, keyword-only for documents, auto-heal inactive until hashes exist).

### Update Flow

```
agent-guidance-mcp --update
  ├─ Pre-download embedding model (SentenceTransformer; skip load if cached)
  ├─ Pre-download LLM skill selector (Qwen2.5-0.5B; skip load if cached)
  ├─ For each UPDATER_REPOS key (ecc, ui_ux, anthropic, owasp, system_design):
  │   ├─ Fetch latest commit SHA from GitHub API
  │   ├─ If cached SHA matches AND target dir exists: skip
  │   └─ Else: git clone / download → extract → save new SHA
  └─ Save state to ~/.agent-guidance/.update-state.json
```

---

## Deploy & Uninstall

```
--setup
  ├─ configure_mcp_clients() → register in IDE configs (Claude, Gemini, Cursor, VS Code, Continue)
  ├─ configure_opencode() → register in opencode.json (global + local)
  ├─ configure_codex() → register in .codex/config.toml
  ├─ configure_global_rules() → append AGENT_RULES_BLOCK to AGENTS.md / CLAUDE.md
  ├─ configure_workspace_rules() → append tagged block to .cursorrules, .clinerules, etc.
  └─ configure_skills_enforcer() → write SKILL.md to skill dirs

--uninstall
  ├─ remove_mcp_clients() → delete from all IDE configs
  ├─ remove_opencode_and_codex() → delete from opencode.json + .codex/config.toml
  ├─ remove_global_rules() → strip tagged block from AGENTS.md / CLAUDE.md, delete skill dirs
  └─ remove_workspace_rules() → strip tagged block from .cursorrules, .clinerules, etc.
```

All rule/skill sections use HTML-comment tags (`<!-- agent-guidance:start -->` / `<!-- agent-guidance:end -->`) for reliable find-and-replace by Python.

---

## Configuration

### Environment Variables

| Variable | Purpose |
|---|---|
| `AGENT_AUTO_UPDATE_INTERVAL` | Auto-update frequency: `weekly` (default) or `monthly` |
| `AGENT_GUIDANCE_ROOT` | Custom standards corpus path |
| `AGENT_PROJECT_ROOT` | Override project root for context tools |
| `AGENT_PROJECT_ALLOWED_ROOTS` | Whitelist directories for security |
| `AGENT_EMBEDDING_DAEMON` | Disable embed daemon (forces in-process model, default `1`) |
| `AGENT_WATCHER_ENABLED` | Enable/disable CodeGraph file watcher |
| `AGENT_WATCHER_INTERVAL` | Watcher poll interval (seconds) |
| `AGENT_WATCHER_DEBOUNCE_MULTIPLIER` | Debounce multiplier after changes |
| `AGENT_WATCHER_REF_THRESHOLD` | Batch size before full reference resolve |
| `AGENT_CLIENT_NAME` | Client/IDE name for usage tracking |

### CLI Flags

| Flag | Action |
|---|---|
| `--setup` | Register MCP server in all IDE clients |
| `--update` | Download skills + LLM model(s) (skip load if already cached) |
| `--auto-update [weekly\|monthly]` | Enable scheduled automatic skill updates |
| `--session-start` | Auto-pass gate, inject project context (used by hook) |
| `--uninstall` | Remove all registrations, rules, and skill folders |
| `--project-path` | Project root for `--session-start` and `--re-gate` |
| `--re-gate` | Re-pass priority gate and refresh sentinel (for subagent recovery) |

---

## 3rd-Party Updates

Five repositories managed by `updater.py`:

| Key | Repository | Update check |
|---|---|---|
| `ecc` | `affaan-m/ECC` (main) | Commit SHA |
| `agent-guidance-mcp_ui_ux` | `nextlevelbuilder/ui-ux-pro-max-skill` (main) | Commit SHA |
| `anthropic` | `anthropics/skills` (main) | Commit SHA |
| `owasp` | `OWASP/CheatSheetSeries` (master) | Commit SHA |
| `system_design` | `donnemartin/system-design-primer` (master) | Commit SHA |

Updates skip when target directory exists AND cached commit SHA matches the latest on GitHub. State is persisted in `~/.agent-guidance/.update-state.json`.

---

## Local Development (debug against this repo)

The MCP server is installed globally via `uv tool install`
(`$HOME/.local/bin/agent-guidance-mcp`), which is what the one-line installer
and the global `~/.config/opencode/opencode.json` use — leave that untouched for
end users.

To develop against the **live source** in this repo without reinstalling:

- The project ships a **gitignored** project-local opencode config
  (`.opencode/opencode.json`) that also launches `agent-guidance-mcp` from
  `PATH`. A local `.pth` shim in the uv tool venv
  (`~/.local/share/uv/tools/agent-guidance-mcp/.../site-packages/zz_debug_src.pth`)
  redirects the import to this repo's `src/`, so edits are live. The `.pth`
  path is **machine-specific** and must be recreated per checkout — it is not
  part of the install and should not be copied between machines.
- `agent-guidance-mcp_require_edit_approval` is **not gated** (callable directly, no `agent-guidance-mcp_task_pipeline` first) so edit
  checks can run during development.

Launch opencode from inside this repo so the project-local config takes
precedence over the global one.

---

## Related

- [MCP Surface](reference/mcp-surface.md) — full tool/resource/prompt reference
- [Development Guide](development.md) — tests, project structure, maintainer
- [Installation](installation.md) — automatic and manual setup
- [README](../README.md) — project overview
