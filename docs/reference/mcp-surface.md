# MCP Surface Reference

[Back to README](../README.md)

Complete reference for every public MCP tool, resource, and prompt exposed by the Agent Guidance MCP server.

---

## Tools (6)

All code inspection, modification, and execution tools require `task_pipeline` to be called first. Gated tools return `PRIORITY_REQUIRED` if called before `task_pipeline`. Ungated tools are `workflow_gate`, `session_continuity`, and `select_skills`.

### 1. `task_pipeline` -- Call First (Unlocks Priority Gate)

One-stop context preparation and priority gate unlocker. Initializes workspace boundaries, detects project architectural patterns, synthesizes dynamic split blueprints (< 300 LOC mandate), and injects memorized project learnings in an ultra-fast, token-bounded response (< 250 tokens).

```python
task_pipeline(
    task: str,                         # required — description of work
    project_path: str,                 # required — absolute path of active working repository
    phase: str,                        # required — active development phase: "plan" | "build" | "implement" | "test" | "debug" | "review" | "refactor"
) -> str
```

**Returns formatted markdown containing:**
- `Task`, `Active Phase`, and `Project` summary (with total indexed file count).
- `💡 Project Memorized Learnings` — semantically relevant past session learnings from `.agent-context/learnings.md`.
- `📐 Dynamic Split Blueprint` — upfront modularity blueprint tailored to detected architecture (Clean Architecture, Layered Architecture, Package by Feature, CLI Pipeline, Flat Library, Orchestrator) and alerts for monolithic files ($\ge 200$ LOC).
- `Architecture Guidance` & `Core Phase Rules` — targeted execution mandates for active phase.
- `Priority Gate: PASSED` — confirms unlock for subsequent MCP tool calls.
- `-> NEXT STEP` — clear directive guiding next tool execution.

**Auto-Plan-Approval Feature:**
If the `task` parameter text contains user approval keywords (e.g. `proceed`, `approved`, `lgtm`, `tiến hành`, `đồng ý`, or IDE artifact approval strings like `the user has approved this document`), `task_pipeline` automatically sets `plan_approved = true`, `edit_authorized = true`, and transitions active stage directly to `Build`.

**Example:**
```python
task_pipeline(task="Add JWT auth to Express API", project_path="E:/Github/MyApp", phase="plan")
```

---

### 2. `select_skills` -- Inject Selected Skills

Confirm which proposed or catalog skills to load into the active conversation context. Injects token-optimized markdown with fast passage slicing and dynamic token measurement. Pass skill names (e.g. `['android-clean-architecture']`), or pass an empty array `[]` to skip all.

```python
select_skills(
    skills: list[str],                 # required — skill names to load (e.g. ["android-clean-architecture"]), or [] to skip
    task: str | None = None,           # optional active task description for semantic section slicing
    project_path: str | None = None,   # absolute path of active repository workspace
    user_confirmed: bool = False,      # user confirmation flag (mandatory when proposals are pending)
    user_message: str | None = None,   # optional message or feedback from user regarding skill selection
) -> str
```

**Returns formatted markdown containing:**
- `Skill Selection Confirmed (<N>)` or `Skill Selection: No skills selected. Proceeding directly to task execution.`
- Sliced, compressed skill markdown bodies (`### Skill: <name>`).
- `🛡️ Language Safety Rules` — micro-guidelines detected for the project tech stack (< 1ms).
- Dynamic token accounting recorded in `.agent-context/usage.db`.

**Aliases:** Tool alias `select_skill`; argument alias `skill` (string or comma-separated).

**Example:**
```python
select_skills(
    skills=["android-clean-architecture"],
    task="Implement repository pattern and domain usecases",
    user_confirmed=True
)
```

---

### 3. `guidance` -- Standards & Skill Catalog

Standards catalog, 2-stage vector search, 279 embedded skills (440 vector passages), pre-code architecture blueprints, and empirical verification contracts. Supports 9 operations.

```python
guidance(
    operation: str,                    # required — search|get|list|precode|verify|workflow|ui_ux|docs|reindex_skills
    query: str | None = None,          # search query / keyword for search, docs, ui_ux, precode
    identifier: str | None = None,     # skill name/path for get / docs, or workflow stage for workflow
    project_path: str | None = None,   # absolute path of active repository workspace
    verification_command: str | None = None,    # required for verify: shell test command (e.g. 'cargo test')
    expected_output_keyword: str | None = None, # required for verify: expected success keyword (e.g. 'ok')
) -> str
```

#### Operations

| Operation | Required Args | Description |
|---|---|---|
| `search` | `query` | 2-stage Candle BERT vector search + Cross-Encoder re-ranking across skills catalog. Fast shallow language profile matching (< 2ms). |
| `get` | `identifier` | Retrieve compressed body of an embedded skill or local workspace skill. |
| `list` | -- | List all registered skills in embedded catalog and workspace (`.agents/skills`). |
| `docs` | `query` \| `identifier` | Search technical documentation cheat-sheets with token-bounded markdown slicing (< 400 tokens). |
| `workflow` | `identifier` (stage) | Load stage workflow guide (`plan`, `context`, `build`, `test`, `review`). |
| `precode` | `query` | Structured pre-code checklist (Upfront Architecture & 300 LOC Cap, Language Rules, Symbol Grounding, Error Boundaries). |
| `verify` | `verification_command`, `expected_output_keyword` | Register empirical verification contract to satisfy anti-hallucination mandate. |
| `ui_ux` | `query` | Retrieve UI/UX guidelines and modern design standards. |
| `reindex_skills` | -- | Force refresh semantic vector index for skills and clear passage caches. (Alias: `reindex`). |

**Examples:**
```python
guidance(operation="search", query="jwt auth express")
guidance(operation="get", identifier="android-clean-architecture")
guidance(operation="docs", query="rust testing", identifier="rust-patterns")
guidance(operation="precode", query="android kotlin UI")
guidance(operation="verify", verification_command="cargo test --bin agent-guidance", expected_output_keyword="test result: ok")
```

---

### 4. `project_context` -- Code Graph, AST & Semantic Search Engine

Read, search, navigate code graphs, and explore project files with built-in token budgets (300 LOC hard cap), AST analysis, and persistent SQLite storage.

```python
project_context(
    operation: str,                    # required — see operations below
    project_path: str,                 # required — absolute path of active repository workspace
    query: str | None = None,          # search query / natural language / symbol name
    relative_path: str | None = None,  # relative file path for read, symbols, structure, learn_alias
    target_symbol: str | None = None,  # precise function/struct/class symbol to extract
    max_depth: int = 3,                # max directory depth for tree operation (default: 3, max: 5)
    start_line: int | None = None,     # 1-indexed start line for read slice
    end_line: int | None = None,       # 1-indexed end line for read slice (max 300 LOC range)
    view_mode: str = "full",           # read mode: "full" | "skeleton" | "zoom" | "slice"
    mode: str = "drift",               # GraphRAG query mode: "global" | "local" | "drift" | "basic"
    scope: str = "all",                # scope for navigate: "symbols" | "files" | "edges" | "content"
    layer: str | None = None,          # architecture domain layer ("ui" | "domain" | "data" | "infrastructure")
    alias_term: str | None = None,     # natural language term for learn_alias
    resolved_symbol: str | None = None,# symbol name for learn_alias
    resolved_line: int | None = None,  # line number for learn_alias
    edges: list[dict] | None = None,   # semantic edges for enrich_graph: [{source, target, relation, description, confidence}]
    summaries: list[dict] | None = None# domain summaries for enrich_graph: [{module_path, title, summary, tags}]
) -> str
```

#### Operations

| Operation | Required Args | Description |
|---|---|---|
| `search` | `query` | 6-Phase Instant Cascade (<100ms): Alias Cache (<1ms) → Symbol FTS5 (<5ms) → Symbol Vectors (<50ms) → Content FTS5 (<5ms) → RAG Content Vectors (<100ms) → Linked Sibling Projects. Top matches auto-cached to alias index. |
| `tree` | -- | Repository directory tree scan. Accepts `max_depth` (default: 3, range: 1..=5). |
| `read` | `relative_path` | Bounded file read with 300 LOC cap. Auto-skeletonizes files > 300 LOC unless line slice or `target_symbol` specified. Supports `start_line`/`end_line` slicing, `view_mode="zoom"` or `"slice"` (folds sibling function bodies). |
| `symbols` | `relative_path` | Extract functions, structs, enums, classes, and traits across 6+ languages (Alias: `structure`). |
| `references` | `query` | Instant symbol usage lookup across codebase (<5ms SQLite FTS5 index). |
| `callers` | `query` | Graph query traversing AST call edges to identify all functions/modules that call target symbol (Alias: `incoming_calls`). |
| `callees` | `query` | Graph query traversing AST call edges to identify all functions/dependencies called by target symbol (Alias: `outgoing_calls`). |
| `blast_radius` | `query` | Calculates logarithmic change risk score and renders Mermaid LR diagram of downstream blast radius (Alias: `impact`). |
| `definition` | `query`, `relative_path` | LSP goto-definition with fallback to SQLite symbol index (Alias: `goto_definition`). |
| `type_definition` | `query`, `relative_path` | LSP type definition lookup with fallback to symbol table. |
| `graph_rag` | `query` | Hierarchical Leiden community GraphRAG synthesizing macro community summaries with local micro-retrieval (`mode="global" \| "local" \| "drift"`). |
| `reusable` | -- | AST & Graph analysis detecting duplicate implementations (>85% semantic match) and candidates for shared modules (Aliases: `shared`, `reusable_candidates`, `detect_duplicates`). |
| `architecture` | -- | Detects and generates Mermaid layer flowchart of project architecture pattern. |
| `navigate` | `query` | Semantic vector graph traversal across symbols, chunks, and edges simultaneously. |
| `learn_alias` | `alias_term`, `relative_path` | Store natural language alias mapping in SQLite with automated 30/90-day decay. |
| `reindex` | -- | Force full AST re-parse and queue background Multilingual-E5 vector embedding for symbols and chunks. |
| `enrich_graph` | `edges` \| `summaries` | Push agent-discovered semantic edges and domain summaries into `.agent-context/code_graph.db` (Alias: `enrich`). |
| `semantic_query`| `query` | Query agent domain summaries and semantic edges via vector search (Alias: `semantic`). |

**Examples:**
```python
project_context(operation="tree", project_path="E:/Github/MyApp", max_depth=3)
project_context(operation="search", project_path="E:/Github/MyApp", query="handle_api_stats")
project_context(operation="callers", project_path="E:/Github/MyApp", query="scan_project")
project_context(operation="callees", project_path="E:/Github/MyApp", query="handle_request")
project_context(operation="blast_radius", project_path="E:/Github/MyApp", query="ServerState")
project_context(operation="read", project_path="E:/Github/MyApp", relative_path="src/main.rs", start_line=1, end_line=120)
project_context(operation="read", project_path="E:/Github/MyApp", relative_path="src/router.rs", target_symbol="handle_request")
project_context(operation="read", project_path="E:/Github/MyApp", relative_path="src/router.rs", view_mode="zoom", target_symbol="dispatch")
project_context(operation="graph_rag", project_path="E:/Github/MyApp", query="authentication lifecycle", mode="drift")
```

---

### 5. `workflow_gate` -- Governance & Stage State Machine

Manage active workflow stages, authorize individual file edits (< 300 LOC hard cap & modularity rules), approve implementation plans, record test verifications, or restore rollback snapshots.

```python
workflow_gate(
    action: str,                       # required — check|status|set_stage|set_architecture|authorize_edit|advance|rollback|approve_plan|pass_verification
    project_path: str | None = None,   # working repository path
    relative_path: str | None = None,  # specific file to authorize edit on (required for authorize_edit)
    target_stage: str | None = None,   # target stage for set_stage / advance: Context|Plan|Ask_Revise|Build|Test_Recheck|Fix|Proposal|Review
    risk_level: str = "LOW",           # LOW | MEDIUM | HIGH (for authorize_edit / advance)
    justification: str | None = None,  # reason & test mitigation (mandatory for files with >8 dependents or refactoring >=300 LOC files)
    architecture_pattern: str = "Auto",# Auto | Clean_Architecture | Layered_Architecture | Package_By_Feature | Orchestrator | CLI_Pipeline | Flat_Library
    user_confirmed: bool = False,      # user confirmation flag (required for approve_plan)
    user_message: str | None = None,   # evaluate message for approval keywords
) -> str
```

#### Actions

| Action | Description |
|---|---|
| `check` | Check current stage and evaluate `user_message` for approval keywords. |
| `status` | Display full workflow state: active stage, plan approval flag, fix attempt count, edit authorization status. |
| `set_stage` | Manually transition workflow stage (`Context`, `Plan`, `Ask_Revise`, `Build`, `Test_Recheck`, `Fix`, `Proposal`, `Review`). |
| `set_architecture` | Detect, lock, and persist architectural pattern in `.agent-context/architecture.json`. |
| `authorize_edit` | Authorize editing a specific file. Verifies < 300 LOC cap, enforces modularity rules on new files, triggers zero-turn transition (Plan → Build if plan approved), creates pre-edit rollback snapshot, and unlocks file write permission. |
| `advance` | Composite check, approval, and stage transition in a single call. Validates post-edit 300 LOC cap on all modified files before allowing advance to `Proposal`/`Review`. (Alias: `advance_stage`). |
| `approve_plan` | Record explicit plan approval (requires `user_confirmed=True` or approval keyword) and reset permissions. (Alias: `approve`). |
| `pass_verification` | Record empirical test verification pass and reset fix attempt counter. |
| `rollback` | Restore pre-edit file snapshot from `.agent-context/snapshots/{session_id}/`. |

#### Stage Lifecycle
$$\text{Context} \longrightarrow \text{Plan} \longrightarrow \text{Ask\_Revise} \longrightarrow \text{Build} \longrightarrow \text{Test\_Recheck} \longrightarrow \text{Fix} \longrightarrow \text{Proposal / Review}$$

* **Build Gate**: Transitioning to `Build` or editing files requires `plan_approved = true`.
* **Fix Circuit Breaker**: Exceeding 3 consecutive fix attempts trips the circuit breaker, resetting stage to `Ask_Revise`, resetting `plan_approved = false`, and requiring user intervention.
* **Modularity Gate (New Files)**: Rejects 40 compound plural container suffixes (`*modals`, `*services`, `*handlers`, `*views`, `*helpers`, etc.) with `COMPOUND_FILE_NAME_PROHIBITED` and multi-component descriptions with `MULTI_COMPONENT_NEW_FILE_PROHIBITED`.
* **300 LOC Refactor Bypass**: Files with $\ge 300$ LOC cannot receive new code unless `justification` contains explicit refactoring keywords (`refactor`, `decompose`, `extract`, `split`, `tách file`).
* **Critical Hub Guard**: Files with $> 8$ dependents require `justification` with $\ge 10$ characters.

**Examples:**
```python
workflow_gate(action="status", project_path="E:/Github/MyApp")
workflow_gate(action="approve_plan", project_path="E:/Github/MyApp", user_confirmed=True)
workflow_gate(action="authorize_edit", project_path="E:/Github/MyApp", relative_path="src/auth.rs", risk_level="LOW", justification="Add JWT token parser")
workflow_gate(action="set_stage", project_path="E:/Github/MyApp", target_stage="Test")
workflow_gate(action="pass_verification", project_path="E:/Github/MyApp")
workflow_gate(action="rollback", project_path="E:/Github/MyApp")
```

---

### 6. `session_continuity` -- Session State & Memory

Save, restore, switch, and summarize cross-session agent context with zero-trust permission resets.

```python
session_continuity(
    operation: str,                    # required — save | load | switch | list | diff | learn | handoff | clear
    project_path: str,                 # required — absolute path of active repository workspace
    session_id: str | None = None,     # session ID to switch to (for switch)
    learning: str | None = None,       # knowledge, insight, or rule to memorize (for learn)
    category: str | None = None,       # category tag: "build_test" | "environment" | "architecture" | "domain_rule" | "general" (for learn)
    pinned: bool = False,              # pin learning item to protect from FIFO eviction (for learn)
    next_action: str | None = None,    # recommended next action for incoming agent (for handoff)
) -> str
```

#### Operations

| Operation | Description |
|---|---|
| `save` | Persist active conversation memory, current stage, modified files, and session metadata into `.agent-context/sessions/session_{pid}_{timestamp}.json`. |
| `load` | Load a specific session by `session_id`. Resets permissions to zero-trust state (`plan_approved=false`, `edit_authorized=false`). |
| `switch` | Atomic handoff to an existing session with permission reset. |
| `list` | List all saved session checkpoints with timestamps, stage, notes, and pinned status (Alias: `sessions`). |
| `diff` | Generate unified git-style change summary of files modified during the active session (Alias: `changes`). |
| `learn` | Record permanent cross-session project rule or architectural instinct into `.agent-context/learnings.md` with category and optional pinned status. |
| `handoff` | Create structured session handoff document summarizing changes, test status, and `next_action`. |
| `clear` | Remove unpinned session checkpoints and purge rollback snapshots. |

**Examples:**
```python
session_continuity(operation="save", project_path="E:/Github/MyApp")
session_continuity(operation="learn", project_path="E:/Github/MyApp", learning="Dashboard components must remain strictly under 120 LOC", category="architecture", pinned=True)
session_continuity(operation="list", project_path="E:/Github/MyApp")
session_continuity(operation="diff", project_path="E:/Github/MyApp")
session_continuity(operation="handoff", project_path="E:/Github/MyApp", next_action="Auth completed. Next step: frontend login modal")
```

---

## Domain Error Codes

The MCP server returns standardized domain error codes in response text when execution is blocked by safety gates:

| Error Code | Source Tool | Cause | Resolution |
|---|---|---|---|
| `PRIORITY_REQUIRED` | All gated tools | Tool invoked before `task_pipeline` | Call `task_pipeline(task="...", project_path="...", phase="plan")` first. |
| `WORKFLOW_STAGE_BLOCKED` | `workflow_gate` | Attempted file edit or stage transition without meeting stage prerequisites | Check `workflow_gate(action="status")`, approve plan with `approve_plan`, and advance to `Build`. |
| `USER_APPROVAL_REQUIRED` | `workflow_gate` | `approve_plan` called without `user_confirmed=True` | Obtain explicit user approval and invoke `workflow_gate(action="approve_plan", user_confirmed=True)`. |
| `USER_CONFIRMATION_REQUIRED` | `select_skills` | Skill selection invoked while proposals were pending without confirmation | Pass `user_confirmed=True` after user confirms selection. |
| `RELATIVE_PATH_REQUIRED` | `workflow_gate` | `authorize_edit` called without file path | Pass exact relative file path in `relative_path`. |
| `PATH_TRAVERSAL_PROHIBITED` | `workflow_gate` | Path escapes workspace root via `..` or invalid symlink | Specify relative path inside project boundaries. |
| `300_LOC_CAP_EXCEEDED` | `workflow_gate` | Attempted to advance stage to `Proposal`/`Review` while a modified file has $\ge 300$ LOC | Decompose file into sub-modules (< 150 LOC each). |
| `COMPOUND_FILE_NAME_PROHIBITED` | `workflow_gate` | New file name contains a compound plural container suffix (`*modals`, `*services`, etc.) | Name files for a single responsibility. |
| `MULTI_COMPONENT_NEW_FILE_PROHIBITED` | `workflow_gate` | New file justification describes multiple distinct components | Decompose into separate single-responsibility files. |
| `HIGH_RISK_JUSTIFICATION_REQUIRED` | `workflow_gate` | Modifying a critical hub file (>8 dependents) with justification < 10 chars | Provide detailed rationale and test verification plan in `justification`. |

---

## Resources (5 Static + 1 Dynamic)

Resources provide direct read-only access to system status, embedded reference guides, and skill content:

1. `agent-guidance://system/edit-allowed` — Read-only JSON resource returning whether file editing is authorized based on active workflow stage and plan approval.
2. `standards://version` — JSON object containing server version and engine metadata.
3. `standards://manifest` — JSON index of embedded standards and skill catalog metadata.
4. `agent-guidance://system/priority` — Priority gate instructions returned when `PRIORITY_REQUIRED` occurs (also supports legacy alias `agent-guidance-mcp://system/priority`).
5. `agent-guidance://system/gate` — JSON status of the priority gate and sentinel file presence (also supports legacy alias `agent-guidance-mcp://system/gate`).
6. `standards://skill/{name}` (Dynamic) — Direct read access to full uncompressed markdown body of any embedded skill (e.g. `standards://skill/rust-patterns`).
