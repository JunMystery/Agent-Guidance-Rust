# AGENTS.md

## Token Saving — Built-in Optimization

agent-guidance saves tokens through 4 automatic mechanisms:

1. **Bounded reads** — `agent-guidance_project_context(operation="read")` caps at 300 lines; `search` caps snippets at 300 chars and `limit` at 20 results. Replaces expensive raw file reads.
2. **Content compression** — Language-aware comment/whitespace removal, dedup, badge-stripping. Applies to every read/search response automatically.
3. **Per-type token budgets** — Source files 3K, docs 2K, skills 3K, task_pipeline 12K tokens max per call.
4. **Tracking & reporting** — Every optimized call records original vs. optimized tokens to SQLite. View with `agent-guidance_token_stats` (session) or `agent-guidance_usage_report(scope="all")` (lifetime).

**Config** (already set globally): `AGENT_GUIDANCE_FILTER_LEVEL=aggressive`, doc/skill caps reduced to 2K/3K — aggressive filtering by default.

**How to maximize savings:**
- Always use `agent-guidance_task_pipeline` → `agent-guidance_project_context`/`agent-guidance_guidance` instead of raw tools (Rule 3).
- Call `agent-guidance_token_stats` at end of each phase to verify compression ratio.
- If token usage is still high, set `AGENT_GUIDANCE_TOKEN_OPT=0` to disable (not recommended).

**Tool naming note:** Some MCP hosts prefix tool names with `agent-guidance_` (e.g. `agent-guidance_task_pipeline`), while others expose bare names (`task_pipeline`). The examples below use prefixed names. When calling, **always use the exact tool name shown in your available-tools list** — never add or drop a prefix.

## CRITICAL — Tool Rules (READ FIRST)

For EVERY user interaction — planning, implementation, testing, debugging, reviewing, refactoring, or any other action:

<!-- agent-guidance:start -->
## Agent Guidance MCP — Tool Selection Priority

| You need to... | Use THIS tool first | Why |
|---|---|---|
| Start any task or phase | `agent-guidance_task_pipeline(task="...")` | Recommendations + tree + code search + UI in ONE call |
| Check coding standards / skills | `agent-guidance_guidance(operation="search", query="...")` | No other tool provides standards or skill lookup |
| Read a file | `agent-guidance_project_context(operation="read", relative_path="...")` | Token-capped at 300 lines — prevents context blowout |
| Search codebase text | `agent-guidance_project_context(operation="search", query="...")` | Ranked, bounded results. Fallback when codegraph unavailable |
| Understand code structure | `agent-guidance_project_context(operation="structure", relative_path="...")` | Hierarchical view of classes, methods, functions in a file |
| Extract symbols | `agent-guidance_project_context(operation="symbols", relative_path="...")` | Flat list of classes, functions, methods with signatures |
| Get structured workflow | `agent-guidance_guidance(operation="workflow", identifier="plan")` → `"code"` → `"test"` | Enriched dev workflow with auto-chaining |
| Pre-code checklist | `agent-guidance_guidance(operation="precode", query="...")` | Coding conventions, security, testing, arch, deploy rules |
| Post-code verification | `agent-guidance_guidance(operation="verify", query="...")` | Auto-detect test/review/security/audit/deploy; suggests next workflow |
| Skill feedback loop | `agent-guidance_guidance(operation="feedback", identifier="...", rating=1-5, query="...")` | Rate skills to boost future recommendations for similar tasks |
| Find symbol references | `agent-guidance_project_context(operation="references", query="...")` | Locate all usages of a symbol across the codebase |
| Get UI/design guidance | `agent-guidance_ui_ux(operation="search", query="...")` | Style, colors, typography, charts, slides |
| Check workflow stage | `agent-guidance_workflow_gate(action="check"/"status"/"set_stage")` | Verify plan approvals and progress lifecycle |
| Persist/recover session | `agent-guidance_session_continuity(operation="save"/"load"/"clear")` | State recovery / task checklist continuity |
| Browse project tree | `agent-guidance_project_context(operation="tree")` | Optimized directory tree view |

### Standard Workflow Lifecycle & Feedback Loops

You must strictly follow this lifecycle for every feature or task:
`[Context] -> [Plan] -> [Ask/Revise] -> [Build] -> [Test/Recheck] -> [Fix] -> [Document/Proposal]`

**MANDATORY workflow rule:**
You BẮT BUỘC phải gọi `workflow_gate(action="check", user_message="<tin nhắn cuối cùng của user>")` ở đầu mỗi lượt phản hồi (turn) trước khi gọi bất kỳ tool nào khác. Nếu `workflow_gate` trả về `plan_approved: false` hoặc trả về lỗi `WORKFLOW_STAGE_BLOCKED`, bạn tuyệt đối không được viết code hay sửa file và phải quay lại xin chỉ thị.

1. **Planning Loop (Human <-> Agent):**
   - Propose a plan in `Plan` stage.
   - Ask the user for approval in `Ask/Revise` stage.
   - Move to `Build` ONLY after user has explicitly approved (ok, proceed, approved, start, đồng ý, làm đi...).
2. **Execution & Quality Loop (Autonomous):**
   - Transition to `Test/Recheck` after writing code in `Build`.
   - If errors/regressions arise, transition to `Fix` and then re-test.
3. **Circuit Breaker Rule:**
   - Max **3 consecutive fix attempts** for the same issue in the Execution Loop.
   - If unresolved after 3 attempts, you must STOP editing code, revert stage to `Ask/Revise`, and ask user for guidance.

### Nine Mandatory Rules

1. **Context First**: Call `agent-guidance_task_pipeline` or `agent-guidance_project_context` BEFORE any file read or code change.
2. **Standards Check**: Use `agent-guidance_guidance(operation="search")` BEFORE implementing or answering any prompt.
3. **Token Budget**: Prefer MCP tools over raw file reads — built-in limits prevent context blowout.
4. **No Direct FS**: Never manually read/search files when MCP tools do it with optimization.
5. **Ground & Plan**: Verify files/functions/symbols via search BEFORE proposing changes. Never guess.
6. **300 LOC Cap**: Split files exceeding 300 lines of code. No monolithic files.
7. **Intent Gate**: Classify request type (trivial/explicit/exploratory/open-ended/ambiguous) before acting. If ambiguous, clarify first.
8. **Delegation Before Action**: Decompose multi-step tasks and delegate to specialized subagents. Never implement directly when delegation is possible.
9. **Per-Phase Reset**: For EACH new work phase (plan → implement → test → debug → review → refactor), re-call `agent-guidance_task_pipeline` with that phase's goal. Do NOT carry old context across phases. A new phase is a new task.

**CRITICAL: All 9 rules apply to EVERY action without exception — planning, implementation, testing, debugging, reviewing, refactoring, or any other work. There is no action type exempt from these rules.**

<!-- agent-guidance:end -->
