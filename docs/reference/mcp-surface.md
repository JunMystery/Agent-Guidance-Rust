# MCP Surface Reference

[Back to README](../README.md)

Complete reference for every public MCP tool, resource, and prompt exposed by the Agent Guidance MCP server.

---

## Tools (12)

All tools except `agent-guidance-mcp_health_check`, `agent-guidance-mcp_diagnose`, `agent-guidance-mcp_token_stats`, `agent-guidance-mcp_usage_report`, and `agent-guidance-mcp_require_edit_approval` require `agent-guidance-mcp_task_pipeline` to be called first. Gated tools return `PRIORITY_REQUIRED` if called before `agent-guidance-mcp_task_pipeline`.

### 1. `agent-guidance-mcp_task_pipeline` -- Call First (unlocks gate)

One-stop context preparation and priority gate unlocker. Initializes workspace boundaries, detects project architectural patterns, synthesizes dynamic split blueprints (< 300 LOC mandate), and injects memorized project learnings in an ultra-fast, token-bounded response (< 250 tokens).

```
task_pipeline(
    task: str,                         # required — description of work
    project_path: str = ".",           # project root (auto-detected if omitted)
    phase: str = "plan",               # active lifecycle phase: "plan" | "context" | "build" | "test"
) -> dict
```

**Returns formatted markdown containing:**
- `Task`, `Active Phase`, and `Project` summary (with total indexed file count).
- `💡 Project Memorized Learnings` — semantically relevant past session learnings from `.agent-context/learnings.md`.
- `📐 Dynamic Split Blueprint` — upfront modularity blueprint tailored to detected architecture (Clean Architecture, Layered Architecture, Package by Feature, CLI Pipeline, Flat Library, Orchestrator) and alerts for monolithic files ($\ge 200$ LOC).
- `Architecture Guidance` & `Core Phase Rules` — targeted execution mandates for active phase.
- `Priority Gate: PASSED` — confirms unlock for subsequent MCP tool calls.
- `-> NEXT STEP` — clear directive guiding next tool execution.

**Example:**
```
task_pipeline(task="Add JWT auth to Express API", phase="plan")
```

---

### 2. `agent-guidance-mcp_select_skills` -- Inject Selected Skills

Confirm which proposed or catalog skills to load into the active conversation context. Injects token-optimized markdown with fast passage slicing and dynamic token measurement. Pass skill names (e.g. `['android-clean-architecture']`), or pass an empty array `[]` to skip all.

```
select_skills(
    skills: list[str],                 # required — skill names to load (e.g. ["android-clean-architecture"]), or [] to skip
    task: str | None = None,           # optional active task description for semantic section slicing
    project_path: str = ".",           # absolute path of active repository workspace
    user_confirmed: bool = False,      # user confirmation flag (mandatory when proposals are pending)
) -> str
```

**Returns formatted markdown containing:**
- `Skill Selection Confirmed (<N>)` or `Skill Selection: No skills selected. Proceeding directly to task execution.`
- Sliced, compressed skill markdown bodies (`### Skill: <name>`).
- `🛡️ Language Safety Rules` — micro-guidelines detected for the project tech stack (< 1ms).
- Dynamic token accounting recorded in `.agent-context/usage.db`.

**Interaction Flow:**
1. `task_pipeline` proposes matching skills.
2. Agent prompts the user via `ask_question` tool.
3. User selects skills.
4. Agent invokes `select_skills(skills=[...], user_confirmed=true)`.

**Example:**
```
select_skills(
    skills=["android-clean-architecture"],
    task="Implement repository pattern and domain usecases",
    user_confirmed=True
)
```

---

### 3. `agent-guidance-mcp_guidance` -- Standards & Skill Catalog

Standards catalog, 2-stage vector search, 168 embedded skills, pre-code architecture blueprints, and empirical verification contracts. Supports 9 operations.

```
guidance(
    operation: str,                    # required — search|get|list|precode|verify|workflow|ui_ux|docs|reindex_skills
    query: str | None = None,          # search query / keyword for search, docs, ui_ux, precode
    identifier: str | None = None,     # skill name/path for get / docs, or workflow stage for workflow
    project_path: str = ".",           # absolute path of active repository workspace
    verification_command: str | None = None,    # required for verify: shell test command (e.g. 'cargo test')
    expected_output_keyword: str | None = None, # required for verify: expected success keyword (e.g. 'ok')
) -> str
```

#### Operations

| Operation | Required Args | Description |
|---|---|---|
| `search` | `query` | 2-stage Candle BERT vector search + Cross-Encoder re-ranking across skills catalog. Fast shallow language profile matching (< 2ms). |
| `get` | `identifier` | Retrieve compressed body of an embedded skill or local workspace skill. |
| `list` | -- | List all registered skills in embedded catalog and workspace (.agents/skills). |
| `docs` | `query` \| `identifier` | Search technical documentation cheat-sheets with token-bounded markdown slicing (< 400 tokens). |
| `workflow` | `identifier` (stage) | Load stage workflow guide (`plan`, `context`, `build`, `test`, `review`). |
| `precode` | `query` | Structured pre-code checklist (Upfront Architecture & 300 LOC Cap, Language Rules, Symbol Grounding, Error Boundaries). |
| `verify` | `verification_command`, `expected_output_keyword` | Register empirical verification contract to satisfy anti-hallucination mandate. |
| `ui_ux` | `query` | Retrieve UI/UX guidelines and design system standards. |
| `reindex_skills` | -- | Force refresh semantic vector index for skills and clear passage caches. |

**Examples:**
```
guidance(operation="search", query="jwt auth express")
guidance(operation="get", identifier="android-clean-architecture")
guidance(operation="docs", query="rust testing", identifier="rust-patterns")
guidance(operation="precode", query="android kotlin UI")
guidance(operation="verify", verification_command="cargo test", expected_output_keyword="103 passed")
```

---

### 4. `agent-guidance-mcp_project_context` -- Code Graph & Semantic Search Engine

Read, search, navigate code graphs, and explore project files with built-in token budgets and persistent SQLite storage.

```
project_context(
    operation: str,                    # required — see operations below
    project_path: str = ".",
    query: str | None = None,          # search query / natural language / symbol name
    relative_path: str | None = None,  # file path for read/symbols/structure/learn_alias
    target_symbol: str | None = None,  # precise symbol extraction for read
    start_line: int | None = None,     # 1-indexed start line for read slice
    end_line: int | None = None,       # 1-indexed end line for read slice (max 300 LOC range)
    alias_term: str | None = None,     # natural language term for learn_alias
    resolved_symbol: str | None = None,# symbol name for learn_alias
    resolved_line: int | None = None,  # line number for learn_alias
    scope: str = "all",                # scope for navigate: "all" | "symbols" | "content" | "edges"
    view_mode: str = "auto",           # view mode for read: "auto" (default skeleton if >300 LOC) | "full" | "skeleton"
    mode: str = "drift",               # GraphRAG query mode: "global" | "local" | "drift"
) -> dict
```

#### Operations

| Operation | Required Args | Description |
|---|---|---|
| `search` | `query` | 5-Phase Instant Cascade (<100ms): Alias Cache (<1ms) → Symbol FTS5 (<5ms) → Symbol Vectors (<50ms) → Content FTS5 (<5ms) → RAG Content Vectors (<100ms). |
| `navigate` | `query` | Comprehensive code graph traversal gathering aliases, symbols, RAG code chunks, and DAG call/import edges simultaneously. |
| `learn_alias` | `alias_term`, `relative_path` | Explicitly record natural language query mappings with auto-decay (30/90 days). |
| `reindex` | -- | Force full AST re-parse and queue background Multilingual-E5 vector embedding for symbols and chunks. |
| `read` | `relative_path` | Bounded file read with 300 line cap, line numbering (`L{n}:`), and auto-skeletonization for large files (`view_mode="skeleton"`). Supports `start_line`/`end_line` slicing and `target_symbol` extraction. Preserves exact indentation. |
| `symbols` | `relative_path` | Extract functions, structs, enums, classes, and traits across 6+ languages. |
| `references` | `query` | Instant symbol usage lookup across codebase (<5ms SQLite FTS5 index). |
| `structure` | `relative_path` | Method-level hierarchical structure map of a specific source file. |
| `architecture` | -- | Detects and persists architectural style and hierarchical community subsystems. |
| `graph_rag` | `query` | Dual-level GraphRAG query synthesizing hierarchical community summaries with local semantic retrieval (`mode="global" \| "local" \| "drift"`). |
| `reusable` | -- | AST and Graph-based reusability analysis to detect duplicated logic, common utilities, and candidates for shared modules across the codebase. |
| `tree` | -- | Top-level repository structure overview (capped at depth 2). |

**Examples:**
```
project_context(operation="search", query="xử lý timeout API")
project_context(operation="navigate", query="PaymentService", scope="all")
project_context(operation="graph_rag", query="authentication flow", mode="drift")
project_context(operation="reusable")
project_context(operation="learn_alias", alias_term="thanh toán", relative_path="src/payment.rs", resolved_symbol="PaymentGateway")
project_context(operation="read", relative_path="src/main.rs", start_line=1, end_line=100)
project_context(operation="read", relative_path="src/main.rs", target_symbol="main")
project_context(operation="read", relative_path="src/large_service.rs", view_mode="skeleton")
project_context(operation="references", query="handle_read")
project_context(operation="reindex")
```

---

### 5. `agent-guidance-mcp_ui_ux` -- Design Guidance

UI/UX Pro Max design guidance. Supports 3 operations.

```
ui_ux(
    operation: str,                    # required — search|design_system|slides
    query: str,                        # required — search query
    domain: str | None = None,         # style|color|chart|landing|product|ux|typography|icons|react|web
    stack: str | None = None,          # react|nextjs|vue|svelte|astro|etc.
    project_name: str | None = None,   # project name for design_system
    output_format: str = "markdown",   # "markdown" | "ascii"
    limit: int = 3,                    # max results
) -> dict
```

#### Operations

| Operation | Description |
|---|---|
| `search` | Search UI/UX guidance by domain and stack |
| `design_system` | Generate full design system (colors, typography, patterns, style) |
| `slides` | Search slide/presentation guidance |

**Examples:**
```
ui_ux(operation="search", query="minimalist dashboard design", domain="style")
ui_ux(operation="design_system", query="SaaS landing page", project_name="MyApp")
ui_ux(operation="slides", query="pitch deck", domain="landing")
```

---

### 6. `agent-guidance-mcp_session_continuity` -- State Persistence & Handoff

Persist or recover task session state, switch between archived sessions, calculate file modification diffs, store categorized project learnings into `.agent-context/learnings.md` (with pinned item survival), and generate cross-agent handoff summaries.

```
session_continuity(
    operation: str,                    # required — save | load | clear | learn | handoff | diff | list | switch
    project_path: str = ".",
    session_id: str | None = None,     # target session ID for switch
    learning: str | None = None,       # required for learn
    category: str | None = None,       # "build_test" | "environment" | "architecture" | "domain_rule" | "general"
    pinned: bool = False,              # pin learning item to protect from FIFO eviction
    next_action: str | None = None,    # recommended next action for handoff
) -> dict
```

| Operation | Description |
|---|---|
| `save` | Save active session snapshot to `.agent-context/sessions/{session_id}.json` |
| `load` | Load latest session snapshot and reset permission flags |
| `clear` | Clear all session snapshots and reset state |
| `learn` | Record distilled project learning into `.agent-context/learnings.md` with category tag and 30-item FIFO cap |
| `handoff` | Generate `.agent-context/handoff.md` summary for seamless multi-IDE / multi-agent handover |
| `diff` | Generate session modification diff table with lines delta and blast-radius impact risk |
| `list` | Display active and archived sessions with IDE client, workflow stage, architecture, and file counts |
| `switch` | Switch active session to target `session_id` with Zero-Trust permission reset |

---

### 7. `agent-guidance-mcp_workflow_gate` -- Stage Enforcement & Impact Guard

Manage workflow stages, authorize code edits with Code Graph dependency risk checks, and restore pre-edit snapshots.

```
workflow_gate(
    action: str,                       # required — check | status | set_stage | set_architecture | advance | authorize_edit | approve_plan | pass_verification | rollback
    project_path: str = ".",
    relative_path: str | None = None,  # required for authorize_edit — target file to create or modify (enforces < 300 LOC cap and blast radius analysis)
    architecture_pattern: str | None = None, # target architecture pattern for authorize_edit (default: "Auto")
    risk_level: str = "LOW",           # LOW | MEDIUM | HIGH (for authorize_edit / advance)
    justification: str | None = None,  # explanation/mitigation plan when editing files or decomposing monoliths
    user_message: str | None = None,   # user's approval text for check
    user_confirmed: bool = False,      # user confirmation flag for approve_plan
    target_stage: str | None = None,   # valid target stage for set_stage / advance
) -> dict
```

| Action | Description |
|---|---|
| `check` | Check current stage and evaluate user message for plan approvals |
| `status` | Display full workflow state, plan approval, and token metrics |
| `set_stage` | Manually transition workflow stage |
| `set_architecture` | Detect, lock, and persist architectural pattern in `.agent-context/architecture.json` |
| `advance` | Composite check, approval, and transition in a single step (verifies 300 LOC cap on modified files) |
| `authorize_edit` | Evaluate target file risk via Code Graph, enforce < 300 LOC cap, trigger zero-turn transition (Plan → Build), auto-create pre-edit snapshot, and grant file-scoped edit permission |
| `approve_plan` | Record explicit plan approval (requires `user_confirmed=True`) and create session checkpoint |
| `pass_verification` | Record empirical test verification pass and reset fix attempt counter |
| `rollback` | Restore pre-edit file snapshot from `.agent-context/snapshots/{session_id}/` |

**Stage lifecycle:** `Context → Plan → Ask_Revise → Build → Test_Recheck → Fix → Proposal`
Transition to `Build` requires `plan_approved=true`. The circuit breaker resets to `Ask_Revise` after 3 failed fix attempts.

---

### 8. `agent-guidance-mcp_require_edit_approval` -- Edit Permission Gate

Final gate check before any write/edit/bash operation. Returns error unless workflow stage is `Build` with `plan_approved=true`.

```
require_edit_approval(
    project_path: str = ".",
) -> dict
```

**Returns:** `{success, allowed, stage, plan_approved}` — blocked calls include a `resolution` field with steps to unblock.

---

### 9. `agent-guidance-mcp_usage_report` -- Usage Statistics

```
usage_report(scope: str = "session") -> dict
```

Returns persistent usage statistics: tool calls, skill loads, embed queries, token savings per session or lifetime. Data stored in `.agent-context/usage.db`.

| Param | Default | Description |
|-------|---------|-------------|
| `scope` | `"session"` | `"session"` for active session, `"all"` for lifetime |

Example response:
```json
{
  "scope": "all",
  "sessions": [{"client_name": "OpenCode", "session_label": "Phase 1", "duration_seconds": 7200}],
  "totals": {"tool_calls": 187, "token_savings": 3800, "savings_pct": 61.3},
  "tool_breakdown": [{"tool_name": "guidance", "operation": "search", "cnt": 42}]
}
```

View the dashboard in a browser: `agent-guidance-mcp --dashboard`

---

### 10. `agent-guidance-mcp_token_stats` -- Session Statistics

```
token_stats() -> dict
```

Returns token optimization statistics: `total_calls`, `total_original_tokens`, `total_optimized_tokens`, `total_saved_tokens`, `overall_savings_pct`, `recent_records`.

---

### 11. `agent-guidance-mcp_health_check` -- Server Status

```
health_check() -> dict
```

Returns `status`, `server`, `version`, `entries` (catalog entry count).

---

### 12. `agent-guidance-mcp_diagnose` -- Self-Diagnostics

```
diagnose() -> dict
```

Comprehensive diagnostics across 7 subsystems:

| Key | Contents |
|---|---|
| `system` | OS, PID, project root |
| `tree_sitter` | Installed status, supported languages |
| `database` | CodeGraph DB path, exists, size, files_indexed, symbols_indexed, call_edges_indexed, status |
| `context7_api` | DNS resolution, IP, TCP connection status |
| `watcher` | DB exists, DB size |
| `catalog` | Entry count, categories |

---

## Resources (7)

| URI | MIME | Description |
|---|---|---|
| `standards://manifest` | `application/json` | Full manifest: entry_count, kinds, categories, all entries with identifiers/paths/URIs |
| `standards://version` | `application/json` | `{"server": "agent-guidance-mcp", "version": "1.5.6", "mcp_protocol": "2024-11-05"}` |
| `standards://document/{identifier}` | `text/markdown` | Standards document content by slug (token-optimized) |
| `standards://skill/{name}` | `text/markdown` | On-demand skill capsule by name (token-optimized) |
| `agent-guidance-mcp://system/priority` | `text/markdown` | Priority gate instructions — returned by `PRIORITY_REQUIRED` errors |
| `agent-guidance-mcp://system/gate` | `application/json` | Priority gate status: passed + sentinel present |
| `agent-guidance-mcp://system/edit-allowed` | `application/json` | Edit permission check based on workflow stage |

---

## Workflow access

Workflow modes are accessed through two separate tools:

**Content mode** — `agent-guidance-mcp_guidance(operation="workflow", identifier="<mode>")` loads workflow instructions for a given phase. The previous standalone `workflow` / `workflow_prompt` tools were consolidated into `guidance`. Supported modes:

| Mode | Description |
|---|---|
| `init` | Project initialization |
| `plan` | Planning workflow (default) |
| `design` | Design phase |
| `visualize` | Visualization |
| `code` | Implementation |
| `run` | Execution |
| `test` | Testing |
| `deploy` | Deployment |
| `debug` | Debugging |
| `refactor` | Refactoring |
| `audit` | Audit |
| `rollback` | Rollback |
| `recap` | Recap |
| `review` | Code review |
| `next` | Next steps |
| `help` | Help |
| `readme` | README generation |
| `customize` | Customization |
| `brainstorm` | Brainstorming |
| `save_brain` | Save brainstorm output |

**Stage management** — `agent-guidance-mcp_workflow_gate(action="status"|"check"|"set_stage")` manages the 7-stage workflow lifecycle (`Context → Plan → Ask_Revise → Build → Test_Recheck → Fix → Proposal`). See section 6 for full documentation.

---

## Internal Subsystems

These modules power the MCP tools but are not directly callable via the MCP protocol.

| Subsystem | Module | Role |
|---|---|---|
| **Daemon** | `src/daemon.rs` | Unix socket lifecycle, connection tracking, 30s idle timeout |
| **MCP Router** | `src/mcp/router.rs` | Tool dispatcher, resource router, initialize handshake |
| **MCP State** | `src/mcp/state.rs` | ServerState priority gate, stage transitions, circuit breaker |
| **MCP Tools** | `src/mcp/tools.rs` | Tool handlers (task_pipeline, guidance, project_context, etc.) |
| **Embeddings** | `src/ml/embeddings.rs` | Candle BERT embedding engine + cached passage vectors + warmup_cache() |
| **Reranker** | `src/ml/llm_selector.rs` | Cross-encoder skill reranker with keyword fallback |
| **Skill Catalog** | `src/catalog/store.rs` | Embedded skill loading + workspace-local scanning |
| **Project Scanner** | `src/context/scanner.rs` | Bounded workspace traversal & ignore filter |
| **CodeGraph DB** | `src/context/db.rs` | SQLite FTS5 symbol index. Tables: `files`, `symbols`, `call_edges`, `symbols_fts` (virtual). WAL mode. |
| **Token Compressor** | `src/optimizer/compressor.rs` | Language-aware comment/whitespace stripping |
