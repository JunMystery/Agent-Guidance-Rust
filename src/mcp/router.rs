use crate::mcp::state::ServerState;
use crate::mcp::tools::handle_tool_call;
use serde_json::{Value, json};

pub fn handle_request(
    method: &str,
    params: Option<Value>,
    state: &mut ServerState,
) -> Result<Value, (i32, String)> {
    match method {
        "initialize" => {
            if let Some(ref p) = params {
                state.set_roots_from_initialize(p);
            }
            Ok(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "serverInfo": {
                    "name": "Agent Guidance MCP Rust",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }))
        }
        "notifications/initialized" => Ok(json!({})),
        "client/connect" => {
            if let Some(ref p) = params {
                if let Some(name) = p.get("name").and_then(|n| n.as_str()) {
                    state.agent_client_name = Some(name.to_string());
                }
            }
            Ok(json!({}))
        }
        "ping" => Ok(json!({})),
        "resources/list" => Ok(json!({
            "resources": [
                {
                    "uri": "agent-guidance://system/edit-allowed",
                    "name": "Edit Authorization Status",
                    "description": "Read-only JSON resource returning whether file editing is authorized based on active workflow stage and plan approval.",
                    "mimeType": "application/json"
                },
                {
                    "uri": "standards://version",
                    "name": "Server Version Info",
                    "description": "JSON object containing server version and engine metadata.",
                    "mimeType": "application/json"
                },
                {
                    "uri": "standards://manifest",
                    "name": "Standards & Skill Catalog Manifest",
                    "description": "JSON index of embedded standards and skill catalog metadata.",
                    "mimeType": "application/json"
                },
                {
                    "uri": "agent-guidance-mcp://system/priority",
                    "name": "Priority Gate Instructions",
                    "description": "Priority gate instructions returned when PRIORITY_REQUIRED occurs.",
                    "mimeType": "text/markdown"
                },
                {
                    "uri": "agent-guidance-mcp://system/gate",
                    "name": "Priority Gate Status",
                    "description": "JSON status of the priority gate and sentinel file presence.",
                    "mimeType": "application/json"
                }
            ]
        })),
        "resources/read" => {
            let params = params.ok_or((-32602, "Missing params".to_string()))?;
            let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
            match uri {
                "agent-guidance://system/edit-allowed"
                | "agent-guidance-mcp://system/edit-allowed" => {
                    let edit_allowed = state.workflow_stage == "Build" && state.plan_approved;
                    let payload = json!({
                        "edit_allowed": edit_allowed,
                        "stage": state.workflow_stage,
                        "plan_approved": state.plan_approved,
                        "fix_attempts": state.fix_attempts
                    });

                    Ok(json!({
                        "contents": [
                            {
                                "uri": uri,
                                "mimeType": "application/json",
                                "text": serde_json::to_string_pretty(&payload).unwrap_or_default()
                            }
                        ]
                    }))
                }
                "standards://version" => {
                    let payload = json!({
                        "name": "Agent Guidance MCP Rust",
                        "version": env!("CARGO_PKG_VERSION"),
                        "engine": "Native Rust 2024 Edition"
                    });

                    Ok(json!({
                        "contents": [
                            {
                                "uri": uri,
                                "mimeType": "application/json",
                                "text": serde_json::to_string_pretty(&payload).unwrap_or_default()
                            }
                        ]
                    }))
                }
                "standards://manifest" => {
                    let skills = crate::catalog::store::list_embedded_skills();
                    let payload = json!({
                        "total_embedded_skills": skills.len(),
                        "skills": skills
                    });

                    Ok(json!({
                        "contents": [
                            {
                                "uri": uri,
                                "mimeType": "application/json",
                                "text": serde_json::to_string_pretty(&payload).unwrap_or_default()
                            }
                        ]
                    }))
                }
                "agent-guidance-mcp://system/priority" => {
                    let text = "# Priority Gate Instructions\n\nCall `agent-guidance-mcp_task_pipeline` first before invoking gated tools. This unlocks the gate for your active session.";
                    Ok(json!({
                        "contents": [
                            {
                                "uri": uri,
                                "mimeType": "text/markdown",
                                "text": text
                            }
                        ]
                    }))
                }
                "agent-guidance-mcp://system/gate" => {
                    let sentinel_exists = ServerState::priority_gate_path().exists();
                    let payload = json!({
                        "priority_gate_passed": state.priority_gate_passed,
                        "sentinel_file_present": sentinel_exists
                    });

                    Ok(json!({
                        "contents": [
                            {
                                "uri": uri,
                                "mimeType": "application/json",
                                "text": serde_json::to_string_pretty(&payload).unwrap_or_default()
                            }
                        ]
                    }))
                }
                _ => {
                    if uri.starts_with("standards://skill/") {
                        let skill_name = uri.trim_start_matches("standards://skill/");
                        if let Some(content) = crate::catalog::store::get_embedded_skill(skill_name)
                        {
                            return Ok(json!({
                                "contents": [
                                    {
                                        "uri": uri,
                                        "mimeType": "text/markdown",
                                        "text": content
                                    }
                                ]
                            }));
                        }
                    }
                    Err((-32602, format!("Resource not found: {}", uri)))
                }
            }
        }
        "tools/list" => Ok(json!({
            "tools": [
                {
                    "name": "task_pipeline",
                    "description": "CALL FIRST before any coding task. Prepares recommendations, project tree, code search, and optional UI guidance in ONE optimized call. You MUST pass project_path and phase.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task": { "type": "string", "description": "The task description or goal" },
                            "project_path": { "type": "string", "description": "Absolute path of your active working repository (e.g. 'E:/Github/Device-Ping')" },
                            "phase": { "type": "string", "enum": ["plan", "implement", "test", "debug", "review", "refactor"], "description": "Active development phase for per-phase context reset" },
                            "focus": { "type": "string", "default": "general", "description": "Optional focus area (e.g. 'security', 'performance', 'frontend', 'testing') to refine skill search ranking." }
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
                            }
                        },
                        "required": ["skills"]
                    }
                },
                {
                    "name": "guidance",
                    "description": "Standards catalog, 2-stage vector search, 168 embedded skills, pre-code architecture blueprints, and empirical verification contracts.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": {
                                "type": "string",
                                "enum": ["search", "get", "list", "precode", "verify", "workflow", "ui_ux", "docs"],
                                "description": "Operation to perform: 'search' (2-stage BERT + Cross-Encoder vector search over skills), 'get' (retrieve skill content by identifier), 'list' (list registered skills catalog), 'precode' (generate upfront 300 LOC architecture blueprint), 'verify' (register empirical verification contract), 'workflow' (retrieve stage workflow guidelines), 'ui_ux' (modern UI/UX design standards), 'docs' (technical documentation search)"
                            },
                            "query": { "type": "string", "description": "Search query or keyword for 'search', 'docs', 'ui_ux', or 'precode'" },
                            "identifier": { "type": "string", "description": "Skill name/path for 'get' / 'docs', or workflow stage name for 'workflow'" },
                            "project_path": { "type": "string", "description": "Absolute path of active repository workspace" },
                            "verification_command": { "type": "string", "description": "Required for 'verify' operation: Actual shell test command to run (e.g. 'cargo test')" },
                            "expected_output_keyword": { "type": "string", "description": "Required for 'verify' operation: Expected success keyword in test output (e.g. 'ok' or 'PASSED')" }
                        },
                        "required": ["operation"]
                    }
                },
                {
                    "name": "project_context",
                    "description": "Read, search, navigate code graph, and extract symbols across project files with built-in 300 LOC token budgets. Use this tool instead of raw grep_search, list_dir, or view_file.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": {
                                "type": "string",
                                "enum": ["search", "navigate", "read", "symbols", "structure", "references", "architecture", "tree", "learn_alias", "reindex"],
                                "description": "Operation: 'search' (5-phase instant cascade), 'navigate' (semantic vector graph traversal), 'read' (read 300 LOC cap / target symbol), 'symbols'/'structure' (file symbol outline), 'references' (symbol usage graph), 'architecture' (pattern detection), 'tree' (structure), 'learn_alias' (store grep mapping), 'reindex' (full graph refresh)"
                            },
                            "project_path": { "type": "string", "description": "Absolute path of your active working repository" },
                            "query": { "type": "string", "description": "Search keyword, symbol name, natural language query, or pattern" },
                            "relative_path": { "type": "string", "description": "Relative file path within project (e.g. 'src/main.rs')" },
                            "target_symbol": { "type": "string", "description": "Specific function/class/struct/enum symbol to extract precisely" },
                            "layer": { "type": "string", "enum": ["ui", "domain", "data", "infrastructure"], "description": "Architecture domain layer of the target file" },
                            "alias_term": { "type": "string", "description": "The natural language term to learn as alias (for learn_alias)" },
                            "resolved_symbol": { "type": "string", "description": "The symbol name resolved from grep (for learn_alias)" },
                            "resolved_line": { "type": "integer", "description": "Line number of resolved symbol (for learn_alias)" },
                            "scope": { "type": "string", "enum": ["symbols", "files", "edges", "content"], "description": "Scope filter for navigate operation" },
                            "view_mode": { "type": "string", "enum": ["full", "skeleton"], "description": "View mode for 'read' operation: 'full' (capped 300 LOC) or 'skeleton' (AST structural outline with function bodies collapsed to line ranges)" }
                        },
                        "required": ["operation", "project_path"]
                    }
                },
                {
                    "name": "workflow_gate",
                    "description": "Manage active workflow stage ('check', 'status', 'set_stage', 'set_architecture', 'advance'), authorize code edit permissions with diff impact guard ('authorize_edit'), or restore snapshots ('rollback').",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["check", "status", "set_stage", "set_architecture", "authorize_edit", "advance", "rollback"], "description": "Action to perform: 'check', 'status', 'set_stage', 'set_architecture', 'authorize_edit', 'advance', or 'rollback' (restore pre-edit session snapshot)" },
                            "target_stage": { "type": "string", "description": "Target workflow stage to transition into: 'Context', 'Plan', 'Ask_Revise', 'Build', 'Test_Recheck', 'Fix', 'Proposal', or 'Review'" },
                            "user_message": { "type": "string" },
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
                    "description": "Persist, restore, or clear task session states, record project learnings into .agent-context/learnings.md, or generate cross-agent handoff summaries.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": {
                                "type": "string",
                                "enum": ["save", "load", "clear", "learn", "handoff"],
                                "description": "Operation to perform: 'save' (snapshot current session state), 'load' (restore most recent session state), 'clear' (erase all saved snapshots), 'learn' (record distilled project rule/knowledge), 'handoff' (generate cross-agent handoff protocol file)"
                            },
                            "project_path": {
                                "type": "string",
                                "description": "Absolute path of active repository workspace"
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
                            "next_action": {
                                "type": "string",
                                "description": "Recommended next action for incoming agent taking over the project (for 'handoff' operation)"
                            }
                        },
                        "required": ["operation"]
                    }
                }
            ]
        })),
        "tools/call" => {
            let params = params.ok_or((-32602, "Missing params".to_string()))?;
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            if let Err(err_msg) = state.can_call_tool(name, &arguments) {
                return Err((-32001, err_msg));
            }

            handle_tool_call(name, arguments, state)
        }
        _ => Err((-32601, format!("Method not found: {}", method))),
    }
}

pub fn is_read_only_request(method: &str, params: &Option<Value>) -> bool {
    match method {
        "initialize"
        | "notifications/initialized"
        | "ping"
        | "resources/list"
        | "resources/read"
        | "tools/list" => true,
        "client/connect" => false,
        "tools/call" => {
            if let Some(p) = params {
                let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = p.get("arguments");
                match name {
                    "project_context" | "guidance" => true,
                    "workflow_gate" => {
                        let action = args
                            .and_then(|a| a.get("action"))
                            .and_then(|act| act.as_str())
                            .unwrap_or("check");
                        matches!(action, "check" | "status")
                    }
                    "session_continuity" => {
                        let op = args
                            .and_then(|a| a.get("operation"))
                            .and_then(|o| o.as_str())
                            .unwrap_or("load");
                        op == "load"
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::state::ServerState;

    #[test]
    fn test_router_resources_list_and_read() {
        let mut state = ServerState::new();

        // 1. Test resources/list
        let list_res = handle_request("resources/list", None, &mut state);
        assert!(list_res.is_ok());
        let list_val = list_res.unwrap();
        let resources = list_val["resources"].as_array().unwrap();
        assert!(resources.len() >= 5);

        // 2. Test resources/read for standards://version
        let read_params = json!({"uri": "standards://version"});
        let read_res = handle_request("resources/read", Some(read_params), &mut state);
        assert!(read_res.is_ok());
        let read_val = read_res.unwrap();
        let contents = &read_val["contents"][0];
        assert_eq!(contents["mimeType"], "application/json");
        assert!(
            contents["text"]
                .as_str()
                .unwrap()
                .contains("Agent Guidance MCP Rust")
        );

        // 3. Test resources/read for agent-guidance-mcp://system/priority
        let read_params = json!({"uri": "agent-guidance-mcp://system/priority"});
        let read_res = handle_request("resources/read", Some(read_params), &mut state);
        assert!(read_res.is_ok());
        let read_val = read_res.unwrap();
        let contents = &read_val["contents"][0];
        assert_eq!(contents["mimeType"], "text/markdown");
        assert!(
            contents["text"]
                .as_str()
                .unwrap()
                .contains("Priority Gate Instructions")
        );
    }
}
