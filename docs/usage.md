# Agent Guidance MCP Usage Guide

[Back to README](../README.md)

Common usage patterns and workflows for AI agents using the Agent Guidance MCP server.

---

## Session Lifecycle: The 6-Step Pattern

Every coding task follows a strict 6-step execution lifecycle:

1. **Initialize Context**: Call `task_pipeline(task="...", project_path="...", phase="plan")` to initialize project boundaries, detect architecture, and unlock the priority gate.
2. **Inject Skills (Optional)**: Select and load on-demand skill guides with `select_skills(skills=[...])`.
3. **Inspect Code Graph**: Search and read symbols using token-bounded operations via `project_context(operation="search"|"read"|"symbols"|"callers"|"blast_radius", ...)` (< 300 LOC cap).
4. **Plan & Get User Approval**: Create/update `implementation_plan.md` and obtain explicit user consensus.
5. **Authorize Edit**: Before creating or modifying any file, call `workflow_gate(action="authorize_edit", project_path="...", relative_path="...", risk_level="LOW", justification="...")`.
6. **Empirical Verification**: Run tests and register proof via `guidance(operation="verify", verification_command="...", expected_output_keyword="...")`.

---

## Example 1: Turn 1 Initialization (`task_pipeline`)

```json
{
  "task": "Add JWT authentication to Express API with unit tests",
  "project_path": "/absolute/path/to/project",
  "phase": "plan"
}
```

Returns project file counts, detected architecture pattern (e.g. `Clean_Architecture`), dynamic split blueprint, memorized project learnings, and unlocks subsequent gated tools.

---

## Example 2: Project Code Context & GraphRAG (`project_context`)

### Directory Tree Scan (Default max_depth = 3)

```json
{
  "operation": "tree",
  "project_path": "/absolute/path/to/project",
  "max_depth": 3
}
```

### 6-Phase Search Cascade

```json
{
  "operation": "search",
  "project_path": "/absolute/path/to/project",
  "query": "jwt auth middleware"
}
```

### Token-Bounded File Read (< 300 LOC)

```json
{
  "operation": "read",
  "project_path": "/absolute/path/to/project",
  "relative_path": "src/auth/token_service.rs",
  "start_line": 1,
  "end_line": 80
}
```

### Precise Symbol Extraction

```json
{
  "operation": "read",
  "project_path": "/absolute/path/to/project",
  "relative_path": "src/auth/token_service.rs",
  "target_symbol": "verify_token"
}
```

### Call Graph & Blast Radius Analysis

Find all functions calling `verify_token`:
```json
{
  "operation": "callers",
  "project_path": "/absolute/path/to/project",
  "query": "verify_token"
}
```

Calculate blast radius and generate Mermaid DAG:
```json
{
  "operation": "blast_radius",
  "project_path": "/absolute/path/to/project",
  "query": "TokenService"
}
```

### LSP Goto Definition

```json
{
  "operation": "definition",
  "project_path": "/absolute/path/to/project",
  "relative_path": "src/main.rs",
  "query": "AuthMiddleware"
}
```

---

## Example 3: Workflow Governance & Stage Management (`workflow_gate`)

### Check Stage Status

```json
{
  "action": "status",
  "project_path": "/absolute/path/to/project"
}
```

### Approve Plan (Requires User Consent)

```json
{
  "action": "approve_plan",
  "project_path": "/absolute/path/to/project",
  "user_confirmed": true
}
```

### Authorize File Edit (Individual File Gate)

```json
{
  "action": "authorize_edit",
  "project_path": "/absolute/path/to/project",
  "relative_path": "src/auth/token_service.rs",
  "risk_level": "LOW",
  "justification": "Add token expiry check in auth middleware"
}
```

### Transition Workflow Stage

```json
{
  "action": "set_stage",
  "project_path": "/absolute/path/to/project",
  "target_stage": "Test"
}
```

---

## Example 4: Session Memory & Continuity (`session_continuity`)

Save session progress across restarts:

```json
{
  "operation": "save",
  "project_path": "/absolute/path/to/project"
}
```

Record persistent architectural instinct or project rule:

```json
{
  "operation": "learn",
  "project_path": "/absolute/path/to/project",
  "learning": "Database migrations must execute inside an explicit SQL transaction.",
  "category": "domain_rule",
  "pinned": true
}
```

Generate cross-agent session handoff summary:

```json
{
  "operation": "handoff",
  "project_path": "/absolute/path/to/project",
  "next_action": "Run integration tests and update API documentation."
}
```

---

## Token Guidance & Best Practices

- **Enforce 300 LOC Cap**: All source files must remain strictly under 300 LOC (aim for < 150 LOC per sub-module).
- **Targeted Reads**: Prefer `target_symbol` or `start_line`/`end_line` ranges over whole-file dumps.
- **Tree Scans**: Use `max_depth=3` (default) for balanced project overview.
- **Avoid Duplication**: Search for existing utilities with `project_context(operation="reusable")` before adding new helper logic.

## Related Docs

- [MCP Surface Reference](reference/mcp-surface.md)
- [Dashboard Guide](dashboard.md)
- [Client Configuration](setup/client-configuration.md)
- [Architecture](ARCHITECTURE.md)
