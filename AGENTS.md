# MANDATORY AGENT EXECUTION PROTOCOL

CRITICAL OVERRIDE: This repository is managed by the `agent-guidance` MCP Server. Route all context gathering, skill selection, file inspection, and editing through `agent-guidance` MCP tools. Do not default to raw filesystem builtins (`view_file`, `list_dir`, `grep_search`, `read_file`) when MCP tools are available.

## Strict Rules
1. Turn 1 Initialization: Start every task/turn by calling `task_pipeline(task="...", project_path="<abs_path>", phase="plan")` FIRST.
2. Token-Bounded File Reads: Do not dump full files. Use `project_context(operation="read", relative_path="...", target_symbol="...")` or `project_context(operation="search", query="...")` (300 LOC cap).
3. Edit Authorization Gate: Always call `workflow_gate(action="authorize_edit", project_path="...", relative_path="...", risk_level="LOW", justification="...", architecture_pattern="Auto")` before modifying files.
4. Mandatory User Skill Selection: When `task_pipeline` returns `-> SKILL_PROPOSAL`, NEVER auto-select skills or call `select_skills` immediately. You MUST trigger the IDE/CLI `ask_question` tool to let the user select skills. ONLY THEN call `select_skills(skills=[...])` with the user's chosen skills (or `select_skills(skills=[])` if skipped).
5. Background Tasks: Stop tool calling immediately when a background command is launched and wait for system reactive wakeup. Do not poll or loop `manage_task(action="status")`.

## Tool Dispatch Matrix

| Operation | MCP Tool (Direct / Prefixed / Lazy) | Required Arguments |
|---|---|---|
| Initialize Task / Turn 1 | `task_pipeline`<br>`agent-guidance_task_pipeline`<br>`call_mcp_tool(ServerName="agent-guidance", ToolName="task_pipeline")` | `task: "..."`<br>`project_path: "<abs_path>"`<br>`phase: "plan"` |
| Select / Load Skills | `select_skills`<br>`agent-guidance_select_skills` | `skills: ["skill-name"]` (or `[]` to skip) |
| Search Codebase / AST | `project_context`<br>`agent-guidance_project_context` | `operation: "search"`<br>`project_path: "<abs_path>"`<br>`query: "..."` |
| Read File / Symbol | `project_context`<br>`agent-guidance_project_context` | `operation: "read"`<br>`project_path: "<abs_path>"`<br>`relative_path: "..."`<br>`target_symbol: "..."` (optional) |
| Extract File Outline | `project_context`<br>`agent-guidance_project_context` | `operation: "symbols"`<br>`project_path: "<abs_path>"`<br>`relative_path: "..."` |
| Code Edits & Gate Auth | `workflow_gate`<br>`agent-guidance_workflow_gate` | `action: "authorize_edit"`<br>`project_path: "<abs_path>"`<br>`relative_path: "..."`<br>`risk_level: "LOW"`<br>`justification: "..."`<br>`architecture_pattern: "Auto"` |
| Reindex Skills / Memory | `guidance`<br>`agent-guidance_guidance` | `operation: "reindex_skills"`<br>`project_path: "<abs_path>"` |
| Standards & Docs | `guidance`<br>`agent-guidance_guidance` | `operation: "search"` or `"docs"`<br>`query: "..."` |
| Verify / Empirical Test | `guidance`<br>`agent-guidance_guidance` | `operation: "verify"`<br>`verification_command: "cargo test"`<br>`expected_output_keyword: "ok"` |
| Session State | `session_continuity`<br>`agent-guidance_session_continuity` | `operation: "save"` \| `"load"` \| `"learn"` \| `"handoff"` |

## Turn-by-Turn Execution Lifecycle
1. Autonomous Initialization: Call `task_pipeline(task="<user request>", project_path="<abs_path>", phase="plan")`. If skills proposed, MUST call `ask_question` for user selection -> THEN call `select_skills(...)` with user input.
2. Context Retrieval: Call `project_context(operation="search" | "read" | "symbols", ...)`. Verify symbols and architecture pattern.
3. Plan & Design: Create/update `implementation_plan.md` if complex.
4. Authorize Edit Gate: Call `workflow_gate(action="authorize_edit", project_path="...", risk_level="LOW", justification="...", architecture_pattern="Auto")`.
5. Apply Changes: Apply targeted edits respecting 300 LOC limit per file.
6. Empirical Verification: Call `guidance(operation="verify", verification_command="...", expected_output_keyword="...")` and run tests.
