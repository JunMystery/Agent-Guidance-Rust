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

**MANDATORY Gate Check:**
Call `workflow_gate(action="check", user_message="<latest_user_message>")` at the start of EVERY turn before using other tools. If `plan_approved: false` or `WORKFLOW_STAGE_BLOCKED`, STOP editing/writing code and request user approval.

1. **Planning Loop:** Propose plan → Request approval → Move to `Build` ONLY after explicit approval.
2. **Execution Loop:** Move to `Test/Recheck` after coding. If bugs occur, move to `Fix` then re-test.
3. **Circuit Breaker:** Max 3 consecutive fix attempts per issue. If failed, STOP editing, set stage to `Ask/Revise`, and seek user guidance.

---

### 9 Core Rules (No Exceptions)

1. **Context First:** Run `agent-guidance_task_pipeline` or `agent-guidance_project_context` before reading files or modifying code. If skills are proposed, trigger the IDE/CLI `ask_question` tool to let the user interactively choose skills, then call `select_skills(skills=[...])`.
2. **Standards Check & Edit Authorization:** Run `agent-guidance_guidance(operation="search")` before implementation. Call `require_edit_approval` before modifying files. If architecture pattern or edit approval is missing/blocked, trigger the IDE/CLI `ask_question` tool to prompt the user.
3. **Token Budget:** Always prioritize MCP tools over raw filesystem access.
4. **No Direct FS:** Avoid direct file reads/searches when optimized MCP tools exist.
5. **Ground & Plan:** Verify codebase facts via search before proposing changes.
6. **Upfront Architecture & 300 LOC Cap:** Design and write code using **Upfront Architecture (Clean Architecture, Layered Architecture, Package-by-Feature, or Orchestrator)** from line 1. Do NOT wait for files to reach 300 LOC to refactor. Split entry dispatchers from sub-module handlers upfront to prevent token waste.
7. **Intent Gate:** Classify request type before acting. If ambiguous or underspecified, trigger the IDE/CLI `ask_question` tool to clarify user intent first.
8. **Delegation First:** Decompose and delegate multi-step tasks to subagents when applicable.
9. **Per-Phase Reset:** Call `agent-guidance_task_pipeline` with the goal for EACH new phase (plan, code, test, debug, review, refactor) and trigger `ask_question` to request user confirmation for stage transitions (`workflow_gate`).

<!-- agent-guidance:end -->
