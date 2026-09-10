# Getting Started with Agent Guidance MCP

Agent Guidance MCP is a 100% Native Rust MCP server that gives AI coding agents standards guidance, skill references, workflow prompts, bounded project code context, and token optimization — all over Stdio transport.

## Quick Start

### 1. Install

**Windows (PowerShell):**
```powershell
powershell -Command "iwr https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.ps1 -OutFile $env:TEMP\i.ps1; & $env:TEMP\i.ps1"
```

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/JunMystery/Agent-Guidance-Rust/main/scripts/install.sh | bash
```

See [Installation](installation.md) for manual setup.

### 2. Verify

Test the server with MCP Inspector:

```bash
DANGEROUSLY_OMIT_AUTH=true npx @modelcontextprotocol/inspector target/release/agent-guidance
```

### 3. Use in your workflow

Every session starts with a single call:

```python
task_pipeline(task="Describe what you're building", project_path=".", phase="plan")
```

This returns skill recommendations, project tree, and code search — all in one optimized call.

Then use the other tools as needed:

| Tool | When to call | Mandatory Parameters |
|---|---|---|
| `select_skills(skills=[...])` | Inject selected skills into conversation context | `skills` |
| `guidance(operation="search", query=...)` | Find relevant skills, blueprints, and standards | `operation`, `query` |
| `project_context(operation="read", ...)` | Read exact symbols and inspect code graphs (< 300 LOC) | `operation`, `project_path`, `relative_path` |
| `workflow_gate(action="authorize_edit", ...)` | Authorize individual file edits under < 300 LOC cap | `action`, `project_path`, `relative_path` |
| `workflow_gate(action="set_stage", ...)` | Transition workflow stages (`Context → Plan → Build...`) | `action`, `target_stage` |
| `session_continuity(operation="save", ...)` | Save session memory across restarts | `operation`, `project_path` |

## Key Concepts

### Priority Gate
`task_pipeline` must be called before code inspection or edit tools. This ensures the agent always has project context before acting.

### Workflow Stages & Edit Governance
The server enforces a 7-stage lifecycle: `Context → Plan → Ask_Revise → Build → Test_Recheck → Fix → Proposal / Review`. Use `workflow_gate` to manage transitions. Edits are only allowed in `Build` stage with `plan_approved=true`, authorized per-file via `workflow_gate(action="authorize_edit")`.

### Token Optimization & Context Shield
Every MCP response is filtered through a language-aware compressor that strips redundant comments, collapses whitespace, and enforces hard 300 LOC token budgets — shielding your prompt context from token bloat.

### Skill Catalog
279 embedded skills (440 vector passages) covering backend, frontend, testing, security, DevOps, data, research, and 12+ language ecosystems. Loaded via `select_skills` or `guidance(operation="get", identifier="<name>")` — no context wasted on unused skills.

## Next Steps

- [Usage Guide](usage.md) — detailed workflow examples
- [MCP Surface](reference/mcp-surface.md) — all 6 tools, resources, and error codes
- [Dashboard Guide](dashboard.md) — real-time web telemetry and architecture graph
- [Installation](installation.md) — manual setup and configuration
- [Architecture](ARCHITECTURE.md) — how the server works internally
