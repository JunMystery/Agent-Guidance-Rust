# Usage Guide

[Back to README](../README.md)

Use this MCP server to give AI agents standards guidance, skill references, workflow prompts, and bounded access to project code context.

## Verify With MCP Inspector

After installation, launch the MCP Inspector:

```bash
npx @modelcontextprotocol/inspector agent-guidance
```

Open the printed URL, usually `http://localhost:5173`, and inspect the registered tools, prompts, and resources.

## Recommended Agent Workflow

At the start of a coding session:

1. Call `task_pipeline(task, project_path, phase="plan")` to unlock the priority gate, initialize language/architecture detection, and propose relevant skills.
2. If skills are proposed, ask the user via IDE/CLI `ask_question` tool to select which skills to load, then invoke `select_skills(skills=[...], user_confirmed=true)`. If no skills are needed, call `select_skills(skills=[])`.
3. For large refactors, upgrades, audits, or unfamiliar code, use `project_context(operation="search", project_path=..., query=...)`, `project_context(operation="symbols", ...)`, and `project_context(operation="tree", project_path=...)`.
4. Use `guidance(operation="precode", query=task)` to get a structured upfront sub-module decomposition blueprint tailored to the project's architecture.
5. Before editing any file, verify the workflow stage allows edits: `workflow_gate(action="check")` → present implementation plan to user.
6. Authorize the edit using composite `workflow_gate(action="authorize_edit", architecture_pattern="Clean_Architecture"|"Layered_Architecture"|"Package_By_Feature"|"CLI_Pipeline"|"Flat_Library"|"Orchestrator"|"Auto")` once approved.
7. Inspect target files with `project_context(operation="read", relative_path=..., target_symbol=...)` (bounded at 300 LOC max).
8. Run empirical verification tests after changes (`cargo test`, `npm test`).
9. Register verification results using `guidance(operation="verify", verification_command=..., expected_output_keyword=...)`.
10. Use `session_continuity(operation="save", ...)` to persist task state across interruptions.

Avoid repeated broad scans during the same session unless the project changed significantly.

## Example: Standards Context

```json
{
  "task": "Build a secure API endpoint with tests",
  "project_path": "/absolute/path/to/project",
  "phase": "plan"
}
```

Use `agent-guidance-mcp_task_pipeline` for the normal first call to unlock gates and initialize architectural context. Use `agent-guidance-mcp_guidance(operation="search", query=...)` when you need catalog standards.

## Example: Project Code Context

Inspect project structure:

```json
{
  "operation": "tree",
  "project_path": "/absolute/path/to/project",
  "max_depth": 3
}
```

Search for a feature or symbol:

```json
{
  "operation": "search",
  "project_path": "/absolute/path/to/project",
  "query": "refresh token auth",
  "limit": 10
}
```

Read the current source file before editing:

```json
{
  "operation": "read",
  "project_path": "/absolute/path/to/project",
  "relative_path": "src/auth/token_service.rs",
  "start_line": 1,
  "max_lines": 160
}
```

## Example: Workflow

Use `agent-guidance-mcp_guidance(operation="workflow", identifier="<mode>", query="<subject>")` to load a workflow by mode.

For example, `agent-guidance-mcp_guidance(operation="workflow", identifier="plan", query="Build billing export")` loads the planning workflow capsule and appends the subject.

## Example: Stage Management

Check the current workflow stage:

```json
{
  "action": "status",
  "project_path": "/absolute/path/to/project"
}
```

Parse user approval and transition to Build:

```json
{
  "action": "check",
  "project_path": "/absolute/path/to/project",
  "user_message": "Proceed with the implementation"
}
```

Then:

```json
{
  "action": "set_stage",
  "project_path": "/absolute/path/to/project",
  "target_stage": "Build"
}
```

## Example: Edit Gate Check

Verify edits are allowed before writing code:

```json
{
  "project_path": "/absolute/path/to/project"
}
```

## Example: Session Continuity

Save task progress:

```json
{
  "operation": "save",
  "project_path": "/absolute/path/to/project",
  "task": "Implement billing export",
  "checklist": [
    {"title": "Design schema", "status": "done"},
    {"title": "Write migration", "status": "in_progress"}
  ]
}
```

## Token Guidance

Prefer narrow calls:

- Use `max_depth=3` or `max_depth=4` for initial tree scans.
- Use `limit=10` or `limit=20` for search.
- Use `max_lines=120` to `200` for file reads unless a broader range is necessary.
- Avoid exporting full snapshots for small one-file tasks.

See [Project Context Tools](reference/project-context-tools.md) for details on snapshot freshness and token cost.

## Related Docs

- [MCP Surface](reference/mcp-surface.md)
- [Project Context Tools](reference/project-context-tools.md)
- [Development Guide](development.md)
