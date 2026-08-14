## CRITICAL — Agent Protocol

Applies to ALL agent interactions: planning, coding, testing, debugging, reviewing, refactoring.

<!-- agent-guidance:start -->

1. **Mandatory MCP Precedence**: You **MUST call `agent-guidance` MCP tools on EVERY action**. Do NOT perform raw filesystem reads, edits, or workflows without going through `agent-guidance`.
2. **Autonomous Initialization**: Always call `agent-guidance_task_pipeline(task="...", project_path="<abs_path>", phase="plan")` at the start of every task, turn, or workflow phase.
3. **Follow MCP Directives**: Execute the exact next steps, skills proposals, and workflow stage gates returned dynamically in the MCP tool output.

<!-- agent-guidance:end -->
