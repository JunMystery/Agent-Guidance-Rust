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
3. Call `project_context(operation="read", relative_path="...", target_symbol="...")` instead of standard file reads (capped at 300 lines).
4. Call `project_context(operation="search", query="...")` instead of standard file searches.
5. Call `workflow_gate(action="authorize_edit", project_path="...", relative_path="<file>", risk_level="LOW", justification="...")` individually for EACH file before modifying or creating it.
6. Call `guidance(operation="search", query="...")` for coding standards or `guidance(operation="ui_ux", query="...")` for UI/UX design rules.
7. Call `guidance(operation="verify", verification_command="...", expected_output_keyword="...")` for empirical post-code testing.

## Critical Behavioral Rules
- When unsure about anything, trigger the IDE/CLI `ask_question` tool! DO NOT GUESS.
- Propose an implementation plan before making any big or complex changes.
- Every created or modified file MUST be < 300 LOC (aim for < 150 LOC per sub-module/component).
- For each new work phase, re-call `task_pipeline` with that phase's goal and request user confirmation for stage transitions.
