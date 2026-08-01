## Token Saving & Optimization

Automatically optimizes token usage via:

1. **Bounded Reads:** Caps `read` at 300 lines, `search` at 300 chars/snippet and 20 results.
2. **Compression:** Auto-removes comments, whitespace, dedups, and badges.
3. **Budgets:** Max tokens/call — Source (3K), Docs (2K), Skills (3K), Task Pipeline (12K). Default: `AGENT_GUIDANCE_FILTER_LEVEL=aggressive`.
4. **Tracking:** Logs savings to SQLite (`agent-guidance_token_stats` / `agent-guidance_usage_report`).

**Best Practices:**

* Use `agent-guidance_task_pipeline` over raw tools.
* Check savings using `agent-guidance_token_stats` after each phase.
* Set `AGENT_GUIDANCE_TOKEN_OPT=0` only if necessary to disable.
* **Tool Naming:** Match exact tool names provided by MCP host (with/without `agent-guidance_` prefix).

---

## CRITICAL — Tool Rules

Applies to ALL agent interactions: planning, coding, testing, debugging, reviewing, refactoring.

<!-- agent-guidance:start -->

### Tool Selection Priority

| Objective | Primary Tool | Note |
| --- | --- | --- |
| Start task/phase | `agent-guidance_task_pipeline(task="...")` | One-call context, tree, recommendations |
| Coding standards/skills | `agent-guidance_guidance(operation="search", query="...")` | Only source for standards & skills |
| Read file | `agent-guidance_project_context(operation="read", relative_path="...")` | Capped at 300 lines |
| Search codebase | `agent-guidance_project_context(operation="search", query="...")` | Bounded text search |
| View file structure | `agent-guidance_project_context(operation="structure", relative_path="...")` | Class/method/function hierarchy |
| Extract symbols | `agent-guidance_project_context(operation="symbols", relative_path="...")` | Flat list of signatures |
| Symbol references | `agent-guidance_project_context(operation="references", query="...")` | Find usages across codebase |
| Directory tree | `agent-guidance_project_context(operation="tree")` | Optimized codebase tree |
| Structured workflow | `agent-guidance_guidance(operation="workflow", identifier="plan"|"code"|"test")` | Auto-chained workflow |
| Pre-code checklist | `agent-guidance_guidance(operation="precode", query="...")` | Rules for arch, security, conventions |
| Post-code verification | `agent-guidance_verify(query="...")` | Auto-detect tests/reviews/audits |
| Rate skill usefulness | `agent-guidance_guidance(operation="feedback", ...)` | Improve future recommendations |
| UI/UX guidance | `agent-guidance_ui_ux(operation="search", query="...")` | Styles, typography, charts |
| Workflow stage gate | `agent-guidance_workflow_gate(action="check"|"status"|"set_stage")` | Stage check & approval status |
| Session state | `agent-guidance_session_continuity(operation="save"|"load"|"clear")` | State/checklist recovery |

---

### Workflow Lifecycle & Gate Rules

**Standard Lifecycle:** `[Context] -> [Plan] -> [Ask/Revise] -> [Build] -> [Test/Recheck] -> [Fix] -> [Document]/[Proposal]`

**Gate Check & Intent Routing:**
1. **Investigatory / Read-Only Requests:** Call `workflow_gate(action="check")` or `project_context(...)` directly. No edit authorization or full pipeline needed.
2. **Task & Coding Requests:** Call `workflow_gate(action="check")` and `task_pipeline(...)` in parallel at turn start.
3. **Stage Transitions & Edit Authorization:** Use the composite `workflow_gate(action="advance", user_message="...", target_stage="Build", architecture_pattern="...")` to perform check + stage transition + edit authorization in a **single MCP call**.
4. **Planning Loop:** Propose plan → Request approval → Move to `Build` ONLY after explicit approval.
5. **Execution Loop:** Move to `Test/Recheck` after coding. If bugs occur, move to `Fix` then re-test.
6. **Circuit Breaker:** Max 3 consecutive fix attempts per issue. If failed, STOP editing, set stage to `Ask/Revise`, and seek user guidance.

---

### 9 Core Rules

1. **Context & Parallel Init:** Run `task_pipeline` or `project_context` to gain context. You MAY invoke `workflow_gate(check)` and `task_pipeline` in parallel during initial turn execution. Call `select_skills` only if specialized skills are needed.
2. **Fast Edit Authorization:** Use composite `workflow_gate(action="advance", ...)` or `workflow_gate(action="authorize_edit", ...)` before modifying files.
3. **Token Budget:** Always prioritize MCP tools over raw filesystem access.
4. **No Direct FS:** Avoid direct file reads/searches when optimized MCP tools exist.
5. **Ground & Plan:** Verify codebase facts via search before proposing changes.
6. **Upfront Architecture & 300 LOC Cap:** Design and write code using **Upfront Architecture (Clean Architecture, Layered Architecture, Package-by-Feature, or Orchestrator)** from line 1. Do NOT wait for files to reach 300 LOC to refactor. Split entry dispatchers from sub-module handlers upfront to prevent token waste.
7. **Intent Gate:** Classify request type before acting (investigatory vs. build). Skip write-gates for investigatory requests.
8. **Delegation First:** Decompose and delegate multi-step tasks to subagents when applicable.
9. **Phase Progression:** Update task goals as work progresses. Use composite `workflow_gate(action="advance")` to transition stages efficiently without redundant intermediate calls.

<!-- agent-guidance:end -->
