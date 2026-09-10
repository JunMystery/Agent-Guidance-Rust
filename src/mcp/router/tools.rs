use serde_json::{Value, json};

pub fn get_tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "task_pipeline",
                "description": "CALL FIRST before any coding task. Initializes workspace context, resets phase state, detects architecture, and checks modularity blueprint in ONE call. You MUST pass project_path and phase.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task": { "type": "string", "description": "The task description or goal" },
                        "project_path": { "type": "string", "description": "Absolute path of your active working repository (e.g. 'E:/Github/Device-Ping')" },
                        "phase": { "type": "string", "enum": ["plan", "build", "implement", "test", "debug", "review", "refactor"], "description": "Active development phase for per-phase context reset" }
                    },
                    "required": ["task", "project_path", "phase"]
                }
            },
            {
                "name": "select_skills",
                "description": "Confirm which proposed or catalog skills to load into the active conversation context. Returns compressed SKILL.md contents inline. Pass skill names (e.g. ['android-clean-architecture']), or pass an empty array [] to skip all.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "skills": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Skill names to load (e.g. ['android-clean-architecture', 'error-handling']). Pass empty array [] to proceed without loading skills."
                        },
                        "task": {
                            "type": "string",
                            "description": "Optional active task description to trigger semantic slicing and extract top relevant skill sections (saving ~70% tokens)"
                        },
                        "project_path": {
                            "type": "string",
                            "description": "Absolute path of active repository workspace"
                        },
                        "user_confirmed": {
                            "type": "boolean",
                            "description": "Whether user explicitly confirmed/selected the skills"
                        },
                        "autonomous": {
                            "type": "boolean",
                            "description": "Bypass interactive user confirmation for autonomous/headless subagents"
                        },
                        "user_message": {
                            "type": "string",
                            "description": "Optional message or feedback from the user regarding skill selection"
                        }
                    },
                    "required": ["skills"]
                }
            },
            {
                "name": "guidance",
                "description": "Standards catalog, 2-stage vector search, 279 embedded skills (440 vector passages), pre-code architecture blueprints, and empirical verification contracts.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["search", "list", "get", "docs", "workflow", "ui_ux", "precode", "verify", "reindex"],
                            "description": "Operation to perform: 'search' (hybrid candidate selection + LLM cross-encoder rerank), 'list' (list all skills), 'get' (fetch skill content), 'docs' (framework & library reference docs), 'workflow' (development workflow guides), 'ui_ux' (UI/UX design principles), 'precode' (pre-coding blueprint & contract check), 'verify' (anti-hallucination verification contract), or 'reindex' (regenerate local passage vector index)"
                        },
                        "query": { "type": "string", "description": "Search query or keyword for 'search' operation" },
                        "topic": { "type": "string", "description": "Topic or skill name for 'get', 'docs', or 'workflow' operation" },
                        "framework": { "type": "string", "description": "Framework or technology for 'precode' operation" },
                        "files": { "type": "array", "items": { "type": "string" }, "description": "Files intended to modify for 'precode' check" },
                        "verification_command": { "type": "string", "description": "Command to run to verify the claim (e.g. 'cargo test', 'python -m pytest', 'git status')" },
                        "expected_output_keyword": { "type": "string", "description": "Keyword or substring that MUST appear in the command stdout/stderr to prove truth" },
                        "max_results": { "type": "integer", "description": "Maximum number of results to return (default: 5)" },
                        "project_path": { "type": "string", "description": "Absolute path of active repository workspace" }
                    },
                    "required": ["operation"]
                }
            },
            {
                "name": "project_context",
                "description": "MANDATORY code reader & GraphRAG engine for large repositories (5k-50k LOC). Combines lexical index, symbol index, and call graph. Replaces full-file dumps with bounded reads (capped at 300 lines) and AST skeletons.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["search", "graph_rag", "read", "symbols", "callers", "callees", "tree", "architecture", "learn_alias", "enrich_graph", "navigate"],
                            "description": "Operation to perform: 'search' (lexical + symbol search), 'graph_rag' (knowledge-graph RAG), 'read' (token-bounded file reader), 'symbols' (outline of functions/classes), 'callers' (find references), 'callees' (find dependencies), 'tree' (directory structure), 'architecture' (detect codebase architecture pattern), 'learn_alias' (register natural language alias), 'enrich_graph' (inject semantic relations), or 'navigate' (hierarchical code navigation)"
                        },
                        "mode": { "type": "string", "enum": ["global", "local", "drift", "basic"], "description": "Query mode for 'graph_rag' operation: 'global' (community summaries), 'local' (entity fan-out), 'drift' (dual-route), or 'basic'" },
                        "project_path": { "type": "string", "description": "Absolute path of your active working repository" },
                        "query": { "type": "string", "description": "Search keyword, symbol name, natural language query, or pattern" },
                        "relative_path": { "type": "string", "description": "Relative file path within project (e.g. 'src/main.rs')" },
                        "target_symbol": { "type": "string", "description": "Specific function/class/struct/enum symbol to extract precisely" },
                        "layer": { "type": "string", "enum": ["ui", "domain", "data", "infrastructure"], "description": "Architecture domain layer of the target file" },
                        "max_depth": { "type": "integer", "description": "Maximum directory depth to scan for 'tree' operation (default: 3, max: 5)" },
                        "start_line": { "type": "integer", "description": "Start line number for 'read' operation (1-indexed)" },
                        "end_line": { "type": "integer", "description": "End line number for 'read' operation (1-indexed)" },
                        "alias_term": { "type": "string", "description": "The natural language term to learn as alias (for learn_alias)" },
                        "resolved_symbol": { "type": "string", "description": "The symbol name resolved from grep (for learn_alias)" },
                        "resolved_line": { "type": "integer", "description": "Line number of resolved symbol (for learn_alias)" },
                        "edges": { "type": "array", "items": { "type": "object" }, "description": "Array of semantic edges for enrich_graph: [{source, target, relation, description, confidence}]" },
                        "summaries": { "type": "array", "items": { "type": "object" }, "description": "Array of domain summaries for enrich_graph: [{module_path, title, summary, tags}]" },
                        "scope": { "type": "string", "enum": ["symbols", "files", "edges", "content"], "description": "Scope filter for navigate operation" },
                        "view_mode": { "type": "string", "enum": ["full", "skeleton", "zoom", "slice"], "description": "View mode for 'read' operation: 'full' (capped 300 LOC), 'skeleton' (AST structural outline), 'zoom'/'slice' (focused implementation with folded siblings)" }
                    },
                    "required": ["operation", "project_path"]
                }
            },
            {
                "name": "workflow_gate",
                "description": "Manage active workflow stage ('check', 'status', 'set_stage', 'set_architecture', 'advance', 'approve_plan', 'pass_verification'), authorize code edit permissions with diff impact guard ('authorize_edit'), or restore snapshots ('rollback').",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["check", "status", "set_stage", "set_architecture", "authorize_edit", "advance", "rollback", "approve_plan", "approve", "pass_verification"], "description": "Action to perform: 'check', 'status', 'set_stage', 'set_architecture', 'authorize_edit', 'advance', 'rollback' (restore pre-edit session snapshot), 'approve_plan' (record plan approval), or 'pass_verification' (reset fix attempts)" },
                        "target_stage": { "type": "string", "description": "Target workflow stage to transition into: 'Context', 'Plan', 'Ask_Revise', 'Build', 'Test_Recheck', 'Fix', 'Proposal', or 'Review'" },
                        "user_message": { "type": "string" },
                        "user_confirmed": { "type": "boolean", "description": "Confirm plan approval or stage change (required for approve_plan)" },
                        "project_path": { "type": "string", "description": "Absolute path of working repository (for authorize_edit / advance / rollback)" },
                        "relative_path": { "type": "string", "description": "Specific file relative path to authorize edit on (triggers Code Graph Diff Impact Guard)" },
                        "risk_level": { "type": "string", "enum": ["LOW", "MEDIUM", "HIGH"], "description": "Declared risk level (for authorize_edit / advance)" },
                        "justification": { "type": "string", "description": "Reason and test mitigation plan for edits (Mandatory for High Risk / Critical Hub files)" },
                        "architecture_pattern": { "type": "string", "enum": ["Auto", "Clean_Architecture", "Layered_Architecture", "Package_By_Feature", "Orchestrator", "CLI_Pipeline", "Flat_Library"], "description": "Declared architecture pattern (for authorize_edit / advance). Default is 'Auto' (auto-detects project architecture)." }
                    },
                    "required": ["action"]
                }
            },
            {
                "name": "session_continuity",
                "description": "Persist, restore, or clear task session states, switch sessions, generate session modification diffs, record project learnings into .agent-context/learnings.md, or generate cross-agent handoff summaries.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["save", "load", "clear", "learn", "handoff", "diff", "changes", "list", "sessions", "switch"],
                            "description": "Operation to perform: 'save' (snapshot current session state), 'load' (restore most recent session state), 'clear' (erase all saved snapshots), 'learn' (record distilled project rule/knowledge), 'handoff' (generate cross-agent handoff protocol file), 'diff' (session file modification summary with line deltas), 'list' (list active and archived session states), or 'switch' (switch to target session ID)"
                        },
                        "project_path": {
                            "type": "string",
                            "description": "Absolute path of active repository workspace"
                        },
                        "session_id": {
                            "type": "string",
                            "description": "Target session ID to switch to (for 'switch' operation)"
                        },
                        "learning": {
                            "type": "string",
                            "description": "The specific knowledge, insight, or rule to memorize (for 'learn' operation)"
                        },
                        "category": {
                            "type": "string",
                            "enum": ["build_test", "environment", "architecture", "domain_rule", "general"],
                            "description": "Category tag for the learning item (for 'learn' operation)"
                        },
                        "pinned": {
                            "type": "boolean",
                            "description": "Pin the learning item to protect it from FIFO eviction (for 'learn' operation)"
                        },
                        "next_action": {
                            "type": "string",
                            "description": "Recommended next action for incoming agent taking over the project (for 'handoff' operation)"
                        }
                    },
                    "required": ["operation"]
                }
            }
        ]
    })
}
