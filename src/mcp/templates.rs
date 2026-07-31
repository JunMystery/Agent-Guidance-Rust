pub const SERVER_ID: &str = "agent-guidance";
pub const OLD_SERVER_ID: &str = "agent-guidance-mcp";

pub const AGENT_GUIDANCE_TAG_START: &str = "<!-- agent-guidance:start -->";
pub const AGENT_GUIDANCE_TAG_END: &str = "<!-- agent-guidance:end -->";
pub const AGENT_GUIDANCE_SKILL_TAG_START: &str = "<!-- agent-guidance-skill:start -->";
pub const AGENT_GUIDANCE_SKILL_TAG_END: &str = "<!-- agent-guidance-skill:end -->";

pub const AGENT_RULES_BLOCK: &str = r#"
<!-- agent-guidance:start -->
## Agent Guidance MCP — Tool Selection Priority & Contract Protocol

| You need to... | Use THIS tool first | Contract Required Arguments |
|---|---|---|
| Start any task or phase | `task_pipeline(task="...", project_path="<path>", phase="plan")` | `project_path`, `task`, `phase` |
| Confirm skill selection | `select_skills(skills=["skill-a", "skill-b"])` | `skills` (array) |
| Check edit authorization | `require_edit_approval(project_path="...", risk_level="LOW", justification="...", architecture_pattern="Clean_Architecture"|"Layered_Architecture"|"Package_By_Feature"|"Orchestrator")` | `project_path`, `risk_level`, `justification`, `architecture_pattern` |
| Read file / extract symbol | `project_context(operation="read", relative_path="...", target_symbol="...")` | `operation`, `project_path`, `target_symbol` (optional) |
| Empirical post-code test | `guidance(operation="verify", verification_command="...", expected_output_keyword="...")` | `verification_command`, `expected_output_keyword` |
| Check coding standards | `guidance(operation="search", query="...")` | `operation`, `query` |

### Nine Mandatory Contract Rules

1. **Context & Phase First**: Call `task_pipeline(task="...", project_path="<path>", phase="<phase>")` BEFORE any file read or code change. You MUST pass `project_path` and `phase`. If skills are proposed, call `select_skills(skills=[...])` to load chosen skills (or `skills=[]` to skip).
2. **Edit Approval Contract**: Call `require_edit_approval(project_path="...", risk_level="LOW", justification="...", architecture_pattern="Clean_Architecture"|"Layered_Architecture"|"Package_By_Feature"|"Orchestrator")` before modifying files. You MUST include a valid `architecture_pattern`.
3. **Symbol-Targeted Reading**: Use `project_context(operation="read", relative_path="...", target_symbol="...")` to read exact symbols and prevent token blowout.
4. **Empirical Test Verification**: Use `guidance(operation="verify", verification_command="...", expected_output_keyword="...")` to prove feature correctness with real test output.
5. **Ground & Plan**: Verify files/functions/symbols via search BEFORE proposing changes. Never guess.
6. **Upfront Orchestrator Architecture**: Do NOT wait for files to reach 300 LOC to refactor. Create new features directly using an Orchestrator (main dispatcher) + sub-function modules upfront to prevent token waste.
7. **Intent Gate**: Classify request type before acting. If ambiguous, clarify first.
8. **Delegation Before Action**: Decompose multi-step tasks and delegate to subagents when appropriate.
9. **Per-Phase Reset**: For EACH new work phase (plan → implement → test → debug → review → refactor), re-call `task_pipeline` with that phase's goal.

**CRITICAL: All 9 contract rules apply to EVERY action without exception.**
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
