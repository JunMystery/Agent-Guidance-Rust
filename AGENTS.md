# MANDATORY AGENT EXECUTION PROTOCOL

CRITICAL OVERRIDE: This repository is managed by the `agent-guidance` MCP Server. Route all context gathering, skill selection, and file inspection through `agent-guidance` MCP tools (Token-Bounded Reads & GraphRAG). Do not default to raw filesystem builtins (`view_file`, `list_dir`, `grep_search`, `read_file`) when MCP tools are available.

## Strict Rules
1. Turn 1 Initialization: Start every task/turn by calling `task_pipeline(task="...", project_path="<abs_path>", phase="plan")` FIRST.
2. Token-Bounded File Reads & GraphRAG: Do not dump full files. NEVER call raw `view_file` or `cat` directly on project files. Always use `project_context(operation="read", relative_path="...", target_symbol="...")` or `project_context(operation="search" | "graph_rag", query="...")` (300 LOC cap).
3. Per-File Edit Authorization Gate & 300 LOC Hard Cap: Always call `workflow_gate(action="authorize_edit", project_path="...", relative_path="<exact_file_path>", risk_level="LOW", justification="...", architecture_pattern="Auto")` individually for EACH file BEFORE creating or modifying it. Blanket authorizations without `relative_path` are rejected. All files (new and existing) MUST strictly remain < 300 LOC (aim for < 150 LOC per sub-module/component). Files >= 300 LOC are hard-blocked from adding new code; decompose into architecture-aligned sub-modules from line 1.
4. Native File Writing & Shell Command Prohibition: Once authorized by `workflow_gate`, apply file creations and modifications exclusively using the client's native file-writing tools (`write_to_file`, `replace_file_content`, `edit_file`). NEVER use terminal/shell commands (`run_command`, PowerShell `Set-Content`/`New-Item`, Bash `cat << EOF`, Python scripts) to write or edit files, as this triggers disruptive user permission modals on every action.
5. Mandatory User Skill Selection: When `task_pipeline` returns `-> SKILL_PROPOSAL`, NEVER auto-select skills or call `select_skills` immediately. You MUST trigger the IDE/CLI `ask_question` tool to let the user select skills. ONLY THEN call `select_skills(skills=[...])` with the user's chosen skills (or `select_skills(skills=[])` if skipped).
6. Background Tasks: Stop tool calling immediately when a background command is launched and wait for system reactive wakeup. Do not poll or loop `manage_task(action="status")`.

## Tool Dispatch Matrix

| Operation | MCP Tool (Direct / Prefixed / Lazy) | Required Arguments |
|---|---|---|
| Initialize Task / Turn 1 | `task_pipeline`<br>`agent-guidance_task_pipeline`<br>`call_mcp_tool(ServerName="agent-guidance", ToolName="task_pipeline")` | `task: "..."`<br>`project_path: "<abs_path>"`<br>`phase: "plan"` |
| Select / Load Skills | `select_skills`<br>`agent-guidance_select_skills` | `skills: ["skill-name"]` (or `[]` to skip) |
| Search Codebase / AST / GraphRAG | `project_context`<br>`agent-guidance_project_context` | `operation: "search"` or `"graph_rag"`<br>`project_path: "<abs_path>"`<br>`query: "..."` |
| Read File / Symbol | `project_context`<br>`agent-guidance_project_context` | `operation: "read"`<br>`project_path: "<abs_path>"`<br>`relative_path: "..."`<br>`target_symbol: "..."` (optional) |
| Extract File Outline | `project_context`<br>`agent-guidance_project_context` | `operation: "symbols"`<br>`project_path: "<abs_path>"`<br>`relative_path: "..."` |
| Authorize File Edit | `workflow_gate`<br>`agent-guidance_workflow_gate` | `action: "authorize_edit"`<br>`project_path: "<abs_path>"`<br>`relative_path: "<exact_file_path>"`<br>`risk_level: "LOW"`<br>`justification: "..."`<br>`architecture_pattern: "Auto"` |
| Apply File Creations & Edits | **Native Client Write Tools**<br>(`write_to_file`, `replace_file_content`) | *Use native IDE edit tools after `workflow_gate` authorization.*<br>⛔ **DO NOT use terminal/shell commands (`run_command`) for file I/O.** |
| Reindex Skills / Memory | `guidance`<br>`agent-guidance_guidance` | `operation: "reindex_skills"`<br>`project_path: "<abs_path>"` |
| Standards & Docs | `guidance`<br>`agent-guidance_guidance` | `operation: "search"` or `"docs"`<br>`query: "..."` |
| Verify / Empirical Test | `guidance`<br>`agent-guidance_guidance` | `operation: "verify"`<br>`verification_command: "cargo test"`<br>`expected_output_keyword: "ok"` |
| Session State | `session_continuity`<br>`agent-guidance_session_continuity` | `operation: "save"` \| `"load"` \| `"learn"` \| `"handoff"` |

## Turn-by-Turn Execution Lifecycle
1. Autonomous Initialization: Call `task_pipeline(task="<user request>", project_path="<abs_path>", phase="plan")`. If skills proposed, MUST call `ask_question` for user selection -> THEN call `select_skills(...)` with user input.
2. Context Retrieval: Call `project_context(operation="search" | "graph_rag" | "read" | "symbols", ...)`. Verify symbols and architecture pattern.
3. Plan & Design: Create/update `implementation_plan.md` if complex. Plan modular sub-components (< 150 LOC each) aligned with the scanned architecture blueprint.
4. Authorize Edit Gate: Call `workflow_gate(action="authorize_edit", project_path="...", relative_path="<exact_file_path>", risk_level="LOW", justification="...", architecture_pattern="Auto")` for EACH target file before creating or modifying it.
5. Apply Changes via Native Tools: Apply surgical changes using IDE native write tools (`write_to_file`, `replace_file_content`) respecting the < 300 LOC limit per file. Never execute terminal/shell commands to write or create files.
6. Empirical Verification: Call `guidance(operation="verify", verification_command="...", expected_output_keyword="...")` and run automated tests.

## Core Engineering Principles (Karpathy-Aligned)
- **Think Before Coding**: State assumptions explicitly. If uncertain or ambiguous, surface tradeoffs and ask rather than guess.
- **Simplicity First**: Write the minimum code that solves the problem. No speculative abstractions or unrequested configurability.
- **Surgical Changes**: Touch only what you must. Every modified line must trace to request. Do not edit unrelated code/formatting.
- **Goal-Driven Execution**: Define verifiable criteria per step. Validate empirically via tests before claiming completion.
