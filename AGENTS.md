# 🚨 SYSTEM DIRECTIVE: MANDATORY AGENT EXECUTION PROTOCOL

> **CRITICAL OVERRIDE FOR ALL AI AGENTS & LLMs (Antigravity, Cursor, Claude Code, Windsurf, OpenCode, Codex, Aider, Gemini):**
> This repository is managed by the **`agent-guidance` MCP Server**.
> You **MUST** route all context gathering, skill selection, file inspection, and editing through `agent-guidance` MCP tools.
> **DO NOT** default to raw filesystem builtins (`view_file`, `list_dir`, `grep_search`, `read_file`) when `agent-guidance` MCP tools are available.

---

## 🛑 STRICT FORBIDDEN ACTIONS (Zero-Tolerance Violations)

1. ❌ **FORBIDDEN: Starting any task/turn without calling `task_pipeline` FIRST.**
   - At Turn 1, you MUST immediately call `task_pipeline` (or `agent-guidance_task_pipeline` / `call_mcp_tool(ServerName="agent-guidance", ToolName="task_pipeline")`).
2. ❌ **FORBIDDEN: Raw filesystem dumps & uncapped reads.**
   - DO NOT call `view_file`, `cat`, or read whole files directly.
   - ALWAYS use `project_context(operation="read", relative_path="...", target_symbol="...")` or `project_context(operation="search", query="...")` (300 LOC cap).
3. ❌ **FORBIDDEN: Editing code without prior gate authorization.**
   - ALWAYS call `workflow_gate(action="authorize_edit", project_path="...", risk_level="LOW", justification="...", architecture_pattern="Auto")` before modifying any files.
4. ❌ **FORBIDDEN: Skipping dynamic next steps.**
   - Whenever an MCP tool output contains `-> SKILL_PROPOSAL` or `-> MANDATORY NEXT STEP`, you MUST execute that directive immediately.

---

## ⚡ Tool Dispatch & Mapping Matrix

Depending on your client platform, the MCP tools may appear as standalone functions or with server prefixes:

| Required Operation | MCP Tool Name (Direct / Prefixed / Lazy) | Required Arguments |
|---|---|---|
| **Initialize Task / Turn 1** | `task_pipeline`<br>`agent-guidance_task_pipeline`<br>`mcp__agent-guidance__task_pipeline`<br>`call_mcp_tool(ServerName="agent-guidance", ToolName="task_pipeline")` | `task: "..."`<br>`project_path: "<abs_path>"`<br>`phase: "plan"` |
| **Select / Load Skills** | `select_skills`<br>`agent-guidance_select_skills` | `skills: ["skill-name"]` (or `skills: []` to skip) |
| **Search Codebase / AST** | `project_context`<br>`agent-guidance_project_context` | `operation: "search"`<br>`project_path: "<abs_path>"`<br>`query: "..."` |
| **Read File / Symbol** | `project_context`<br>`agent-guidance_project_context` | `operation: "read"`<br>`project_path: "<abs_path>"`<br>`relative_path: "..."`<br>`target_symbol: "..."` *(optional)* |
| **Extract File Outline** | `project_context`<br>`agent-guidance_project_context` | `operation: "symbols"`<br>`project_path: "<abs_path>"`<br>`relative_path: "..."` |
| **Code Edits & Gate Auth** | `workflow_gate`<br>`agent-guidance_workflow_gate` | `action: "authorize_edit"`<br>`project_path: "<abs_path>"`<br>`relative_path: "..."`<br>`risk_level: "LOW"`<br>`justification: "..."`<br>`architecture_pattern: "Auto"` |
| **Reindex Skills / Memory** | `guidance`<br>`agent-guidance_guidance` | `operation: "reindex_skills"`<br>`project_path: "<abs_path>"` |
| **Standards & Cheat Sheets** | `guidance`<br>`agent-guidance_guidance` | `operation: "search"` or `"docs"`<br>`query: "..."` |
| **Verify / Empirical Test** | `guidance`<br>`agent-guidance_guidance` | `operation: "verify"`<br>`verification_command: "cargo test"`<br>`expected_output_keyword: "ok"` |
| **Session State & Learnings**| `session_continuity`<br>`agent-guidance_session_continuity` | `operation: "save" \| "load" \| "learn" \| "handoff"` |

---

## 🔄 Turn-by-Turn Execution Lifecycle

```
[User Request Received]
         │
         ▼
[Step 1: Autonomous Initialization]
Call: task_pipeline(task="<user request>", project_path="<abs_path>", phase="plan")
         │
         ├─► If Skills Proposed: Trigger ask_question to user -> Call select_skills(...)
         │
         ▼
[Step 2: Context Retrieval & Symbol Grounding]
Call: project_context(operation="search" | "read" | "symbols", ...)
Verify symbols & architecture pattern before designing changes.
         │
         ▼
[Step 3: Plan & Design]
Create/Update implementation_plan.md artifact if complex.
         │
         ▼
[Step 4: Authorize Edit Gate]
Call: workflow_gate(action="authorize_edit", project_path="...", risk_level="LOW", justification="...", architecture_pattern="Auto")
         │
         ▼
[Step 5: Apply Code Changes]
Apply minimal, targeted modifications respecting 300 LOC limit per file.
         │
         ▼
[Step 6: Empirical Verification]
Call: guidance(operation="verify", verification_command="...", expected_output_keyword="...")
Run automated tests to prove correctness.
```

**MANDATE**: Follow this protocol on EVERY action. Non-compliance degrades model performance, violates token budgets, and corrupts agent coordination.
