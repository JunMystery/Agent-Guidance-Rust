use serde_json::{json, Value};
use crate::mcp::state::ServerState;
use crate::mcp::tools::handle_tool_call;

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
        },
        "notifications/initialized" => Ok(json!({})),
        "client/connect" => {
            if let Some(ref p) = params {
                if let Some(name) = p.get("name").and_then(|n| n.as_str()) {
                    state.agent_client_name = Some(name.to_string());
                }
            }
            Ok(json!({}))
        },
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
                "agent-guidance://system/edit-allowed" | "agent-guidance-mcp://system/edit-allowed" => {
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
                },
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
                },
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
                },
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
                },
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
                },
                _ => {
                    if uri.starts_with("standards://skill/") {
                        let skill_name = uri.trim_start_matches("standards://skill/");
                        if let Some(content) = crate::catalog::store::get_embedded_skill(skill_name) {
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
        },
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
                            "focus": { "type": "string", "default": "general" }
                        },
                        "required": ["task", "project_path", "phase"]
                    }
                },
                {
                    "name": "select_skills",
                    "description": "Confirm which proposed skills to load. Returns compressed SKILL.md contents inline for all selected skills. Pass skill names from the proposed list, or pass an empty array to skip all.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "skills": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Skill names to load (from proposed list). Empty array = skip all."
                            }
                        },
                        "required": ["skills"]
                    }
                },
                {
                    "name": "guidance",
                    "description": "Standards catalog and skill lookup. 168 skills available on-demand.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": { "type": "string" },
                            "query": { "type": "string" },
                            "identifier": { "type": "string" },
                            "verification_command": { "type": "string", "description": "Required for 'verify' operation: Lệnh shell kiểm thử thực tế (ví dụ: 'cargo test')" },
                            "expected_output_keyword": { "type": "string", "description": "Required for 'verify' operation: Từ khóa kết quả kỳ vọng (ví dụ: 'PASSED')" }
                        },
                        "required": ["operation"]
                    }
                },
                {
                    "name": "project_context",
                    "description": "Read and search project files with built-in token budgets.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": { "type": "string" },
                            "project_path": { "type": "string", "description": "Absolute path of your active working repository (e.g. 'E:/Github/Device-Ping')" },
                            "query": { "type": "string" },
                            "relative_path": { "type": "string" },
                            "target_symbol": { "type": "string", "description": "Target function/class/struct symbol to extract precisely for token-saving read" },
                            "layer": { "type": "string", "enum": ["ui", "domain", "data", "infrastructure"], "description": "Architecture domain layer of the target file" }
                        },
                        "required": ["operation", "project_path"]
                    }
                },
                {
                    "name": "ui_ux",
                    "description": "UI/UX design guidance.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": { "type": "string" },
                            "query": { "type": "string" }
                        },
                        "required": ["operation", "query"]
                    }
                },
                {
                    "name": "session_continuity",
                    "description": "Persist or recover task session state for continuity.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "operation": { "type": "string" }
                        },
                        "required": ["operation"]
                    }
                },
                {
                    "name": "workflow_gate",
                    "description": "Manage and validate the active workflow stage.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string" },
                            "target_stage": { "type": "string" },
                            "user_message": { "type": "string" }
                        },
                        "required": ["action"]
                    }
                },
                {
                    "name": "require_edit_approval",
                    "description": "Final gate check to verify if the active workflow stage permits code editing. Must declare risk_level, justification, and architecture_pattern ('Clean_Architecture', 'Layered_Architecture', 'Package_By_Feature', or 'Orchestrator').",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "project_path": { "type": "string", "description": "Absolute path of your active working repository" },
                            "risk_level": { "type": "string", "enum": ["LOW", "MEDIUM", "HIGH"], "description": "Declared risk level of proposed edits" },
                            "justification": { "type": "string", "description": "Reason and rationale for code edits" },
                            "architecture_pattern": { "type": "string", "enum": ["Clean_Architecture", "Layered_Architecture", "Package_By_Feature", "Orchestrator"], "description": "Upfront architecture pattern declared for implementation" }
                        },
                        "required": ["project_path", "risk_level", "justification", "architecture_pattern"]
                    }
                },
                {
                    "name": "token_stats",
                    "description": "Return token optimization statistics for this session. No parameters.",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "usage_report",
                    "description": "Return recorded usage statistics for the current or all sessions.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "scope": { "type": "string", "default": "session" }
                        }
                    }
                },
                {
                    "name": "health_check",
                    "description": "Return server health status and basic metadata. No parameters.",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "diagnose",
                    "description": "Perform comprehensive self-diagnostics on the server.",
                    "inputSchema": { "type": "object", "properties": {} }
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
        },
        _ => Err((-32601, format!("Method not found: {}", method))),
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
        assert!(contents["text"].as_str().unwrap().contains("Agent Guidance MCP Rust"));

        // 3. Test resources/read for agent-guidance-mcp://system/priority
        let read_params = json!({"uri": "agent-guidance-mcp://system/priority"});
        let read_res = handle_request("resources/read", Some(read_params), &mut state);
        assert!(read_res.is_ok());
        let read_val = read_res.unwrap();
        let contents = &read_val["contents"][0];
        assert_eq!(contents["mimeType"], "text/markdown");
        assert!(contents["text"].as_str().unwrap().contains("Priority Gate Instructions"));
    }
}
