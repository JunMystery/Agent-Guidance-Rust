# MANDATORY AGENT EXECUTION PROTOCOL

CRITICAL OVERRIDE: This repository is managed by the `agent-guidance` MCP Server. Route all context gathering, skill selection, and file inspection through `agent-guidance` MCP tools (Token-Bounded Reads & GraphRAG). Do not use raw filesystem builtins (`view_file`, `list_dir`, `grep_search`, `read_file`) when MCP tools are available.

## Execution Lifecycle & Strict Rules
1. **Turn 1 Initialization**: Call `task_pipeline(task="<user request>", project_path="<abs_path>", phase="plan")` FIRST. If skills are proposed, trigger the IDE/CLI `ask_question` tool for user selection, then call `select_skills(...)`.
2. **Token-Bounded Context & GraphRAG**: Search and inspect code exclusively via `project_context(operation="search" | "graph_rag" | "read" | "symbols", ...)` (300 LOC cap). Never dump full files.
3. **Plan & Design**: Create/update `implementation_plan.md` for complex tasks and obtain user plan approval before entering the `Build` stage.
4. **Per-File Edit Authorization Gate & 300 LOC Hard Cap**: Always call `workflow_gate(action="authorize_edit", project_path="...", relative_path="<exact_file_path>", risk_level="LOW", justification="...", architecture_pattern="Auto")` individually for EACH file BEFORE creating or modifying it. All files MUST remain strictly < 300 LOC (target < 150 LOC per sub-module).
5. **Apply Surgical Changes**: Apply modular changes respecting the authorized architecture pattern.
6. **Empirical Verification**: Call `guidance(operation="verify", verification_command="...", expected_output_keyword="...")` and run automated tests.
7. **Background Tasks**: Stop tool calling immediately when a background command is launched and wait for system reactive wakeup.

## Quick Tool Reference
| Phase / Task | MCP Tool | Operation / Action |
|---|---|---|
| Turn 1 Init & Skills | `task_pipeline` / `select_skills` | `phase="plan"` / `skills=[...]` |
| GraphRAG & Code Search | `project_context` | `operation="search"` \| `"graph_rag"` |
| Token-Bounded File/Symbol Read | `project_context` | `operation="read"` (`target_symbol="..."`) \| `"symbols"` |
| Edit Authorization Gate | `workflow_gate` | `action="authorize_edit"` (`relative_path="..."`) |
| Standards, Docs & Verification | `guidance` | `operation="search"` \| `"docs"` \| `"verify"` |
| Session Continuity & Memory | `session_continuity` | `operation="save"` \| `"load"` \| `"learn"` \| `"handoff"` |

