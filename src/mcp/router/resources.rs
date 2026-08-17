use crate::mcp::state::ServerState;
use serde_json::{Value, json};

pub(crate) fn handle_list() -> Value {
    json!({
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
        })
}

pub(crate) fn handle_read(params: Option<Value>, state: &ServerState) -> Result<Value, (i32, String)> {
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
