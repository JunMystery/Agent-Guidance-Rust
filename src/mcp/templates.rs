pub const SERVER_ID: &str = "agent-guidance";
pub const OLD_SERVER_ID: &str = "agent-guidance-mcp";

pub const AGENT_GUIDANCE_TAG_START: &str = "<!-- agent-guidance:start -->";
pub const AGENT_GUIDANCE_TAG_END: &str = "<!-- agent-guidance:end -->";
pub const AGENT_GUIDANCE_SKILL_TAG_START: &str = "<!-- agent-guidance-skill:start -->";
pub const AGENT_GUIDANCE_SKILL_TAG_END: &str = "<!-- agent-guidance-skill:end -->";

pub const AGENT_RULES_BLOCK: &str = r#"
<!-- agent-guidance:start -->
## Agent Guidance MCP — Tool Selection Priority

| You need to... | Use THIS tool first | Why |
|---|---|---|
| Start any task or phase | `task_pipeline(task="...")` | Recommendations + tree + code search + UI in ONE call |
| Check coding standards / skills | `guidance(operation="search", query="...")` | No other tool provides standards or skill lookup |
| Read a file | `project_context(operation="read", relative_path="...")` | Token-capped at 300 lines — prevents context blowout |
| Search codebase text | `project_context(operation="search", query="...")` | Ranked, bounded results. Fallback when codegraph unavailable |
| Understand code structure | `project_context(operation="structure", relative_path="...")` | Hierarchical view of classes, methods, functions in a file |
| Extract symbols | `project_context(operation="symbols", relative_path="...")` | Flat list of classes, functions, methods with signatures |
| Find symbol references | `project_context(operation="references", query="...")` | Locate all usages of a symbol across the codebase |
| Get UI/design guidance | `ui_ux(operation="search", query="...")` | Style, colors, typography, charts, slides |
| Persist/recover session | `session_continuity(operation="save"/"load"/"clear")` | State recovery / task checklist continuity |
| Browse project tree | `project_context(operation="tree")` | Optimized directory tree view |

### Nine Mandatory Rules

1. **Context First**: Call `task_pipeline` or `project_context` BEFORE any file read or code change.
2. **Standards Check**: Use `guidance(operation="search")` BEFORE implementing or answering any prompt.
3. **Token Budget**: Prefer MCP tools over raw file reads — built-in limits prevent context blowout.
4. **No Direct FS**: Never manually read/search files when MCP tools do it with optimization.
5. **Ground & Plan**: Verify files/functions/symbols via search BEFORE proposing changes. Never guess.
6. **300 LOC Cap**: Split files exceeding 300 lines of code. No monolithic files.
7. **Intent Gate**: Classify request type (trivial/explicit/exploratory/open-ended/ambiguous) before acting. If ambiguous, clarify first.
8. **Delegation Before Action**: Decompose multi-step tasks and delegate to specialized subagents. Never implement directly when delegation is possible.
9. **Per-Phase Reset**: For EACH new work phase (plan → implement → test → debug → review → refactor), re-call `task_pipeline` with that phase's goal. Do NOT carry old context across phases. A new phase is a new task.

**CRITICAL: All 9 rules apply to EVERY action without exception — planning, implementation, testing, debugging, reviewing, refactoring, or any other work. There is no action type exempt from these rules.**
<!-- agent-guidance:end -->
"#;

pub const ENFORCER_SKILL_CONTENT: &str = r#"<!-- agent-guidance-skill:start -->
---
name: agent-guidance
description: Core system standards check and token-optimized codebase context retrieval. Run this skill before performing any tool execution or codebase changes.
---

## When to use me
- Run this skill at the beginning of EVERY task, repository lookup, or codebase refactoring.
- Run this skill to check project conventions and avoid raw file reading/search operations.
- Re-run this skill at EACH phase transition (plan → implement → test → review).

## How to use me
You must invoke the `agent-guidance` tools in this priority order:
1. Call `task_pipeline(task="...")` at the start of any task or phase to retrieve workspace context, tree, and recommendations.
2. Call `guidance(operation="search", query="...")` before implementing coding standards.
3. Call `project_context(operation="read", relative_path="...")` instead of standard file reads (capped at 300 lines).
4. Call `project_context(operation="search", query="...")` instead of standard file search.

## Critical Behavioral Rules
- When unsure about anything, ASK! DO NOT GUESS.
- Propose an implementation plan before making any big or complex changes.
- For each new work phase, re-call `task_pipeline` with the phase goal. Do not carry old context.
<!-- agent-guidance-skill:end -->
"#;
