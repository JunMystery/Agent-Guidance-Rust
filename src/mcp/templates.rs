pub const SERVER_ID: &str = "agent-guidance";
pub const OLD_SERVER_ID: &str = "agent-guidance-mcp";

pub const AGENT_GUIDANCE_TAG_START: &str = "<!-- agent-guidance:start -->";
pub const AGENT_GUIDANCE_TAG_END: &str = "<!-- agent-guidance:end -->";
pub const AGENT_GUIDANCE_SKILL_TAG_START: &str = "<!-- agent-guidance-skill:start -->";
pub const AGENT_GUIDANCE_SKILL_TAG_END: &str = "<!-- agent-guidance-skill:end -->";

pub const AGENT_RULES_BLOCK: &str = r#"
<!-- agent-guidance:start -->
# 🚨 SYSTEM DIRECTIVE: MANDATORY AGENT EXECUTION PROTOCOL

> **CRITICAL OVERRIDE FOR ALL AI AGENTS & LLMs:**
> You **MUST** route all context gathering, skill selection, file inspection, and editing through `agent-guidance` MCP tools.
> **DO NOT** default to raw filesystem builtins (`view_file`, `list_dir`, `grep_search`, `read_file`) when `agent-guidance` MCP tools are available.

## 🛑 STRICT FORBIDDEN ACTIONS
1. ❌ **FORBIDDEN: Starting any task/turn without calling `task_pipeline` FIRST.**
2. ❌ **FORBIDDEN: Raw filesystem dumps & uncapped reads.** Always use `project_context(operation="read"|"search", ...)` (300 LOC cap).
3. ❌ **FORBIDDEN: Editing code without prior gate authorization.** Always call `workflow_gate(action="authorize_edit", ...)` before writing files.
4. ❌ **FORBIDDEN: Skipping dynamic next steps.** Execute any `-> SKILL_PROPOSAL` or `-> MANDATORY NEXT STEP` immediately.

| You need to... | Use THIS tool first | Contract Required Arguments |
|---|---|---|
| Start any task or phase | `task_pipeline(task="...", project_path="<path>", phase="plan")` | `project_path`, `task`, `phase` |
| Confirm skill selection | `select_skills(skills=["skill-a", "skill-b"])` | `skills` (array) |
| Search codebase / keywords | `project_context(operation="search", query="...")` | `operation`, `project_path`, `query` |
| Read file / extract symbol | `project_context(operation="read", relative_path="...", target_symbol="...")` | `operation`, `project_path`, `relative_path`, `target_symbol` (optional) |
| List code symbols in file | `project_context(operation="symbols", relative_path="...")` | `operation`, `project_path`, `relative_path` |
| Find symbol usages | `project_context(operation="references", query="...")` | `operation`, `project_path`, `query` |
| Check edit authorization | `workflow_gate(action="authorize_edit", project_path="...", risk_level="LOW", justification="...", architecture_pattern="Auto"|"Clean_Architecture"|"Layered_Architecture"|"Package_By_Feature"|"Orchestrator")` | `action`, `project_path`, `risk_level`, `justification`, `architecture_pattern` |
| Reindex skills / memory | `guidance(operation="reindex_skills", project_path="...")` | `operation`, `project_path` |
| Empirical post-code test | `guidance(operation="verify", verification_command="...", expected_output_keyword="...")` | `verification_command`, `expected_output_keyword` |
| Check coding standards | `guidance(operation="search", query="...")` | `operation`, `query` |

**CRITICAL: All contract rules apply to EVERY action without exception.**
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
You must invoke the 6 core `agent-guidance` MCP tools in this priority order:
1. Call `task_pipeline(task="...", project_path="<path>", phase="plan")` at the start of any task or phase to retrieve workspace context, tree, and skill recommendations.
2. If skills are proposed, trigger the IDE/CLI `ask_question` tool to present recommended skills interactively, then call `select_skills(skills=[...])` to load chosen skills.
3. Call `workflow_gate(action="check")` and `workflow_gate(action="authorize_edit", ...)` before writing/editing files.
4. Call `project_context(operation="read", relative_path="...", target_symbol="...")` instead of standard file reads (capped at 300 lines).
5. Call `project_context(operation="search", query="...")` instead of standard file searches.
6. Call `guidance(operation="search", query="...")` for coding standards or `guidance(operation="ui_ux", query="...")` for UI/UX design rules.
7. Call `guidance(operation="verify", verification_command="...", expected_output_keyword="...")` for empirical post-code testing.

## Critical Behavioral Rules
- When unsure about anything, trigger the IDE/CLI `ask_question` tool! DO NOT GUESS.
- Propose an implementation plan before making any big or complex changes.
- For each new work phase, re-call `task_pipeline` with that phase's goal and request user confirmation for stage transitions.
<!-- agent-guidance-skill:end -->
"#;
