use serde_json::{json, Value};
use crate::mcp::state::ServerState;
use crate::mcp::tools::handle_tool_call;

pub fn handle_request(
    method: &str,
    params: Option<Value>,
    state: &mut ServerState,
) -> Result<Value, (i32, String)> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "serverInfo": {
                "name": "Agent Guidance MCP Rust",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "notifications/initialized" => Ok(json!({})),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({
            "tools": [
                {
                    "name": "task_pipeline",
                    "description": "CALL FIRST before any coding task. Prepares recommendations, project tree, code search, and optional UI guidance in ONE optimized call.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task": { "type": "string" },
                            "project_path": { "type": "string", "default": "." },
                            "focus": { "type": "string", "default": "general" }
                        },
                        "required": ["task"]
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
                            "identifier": { "type": "string" }
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
                            "project_path": { "type": "string", "default": "." },
                            "query": { "type": "string" }
                        },
                        "required": ["operation"]
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
                            "action": { "type": "string" }
                        },
                        "required": ["action"]
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
            handle_tool_call(name, arguments, state)
        },
        _ => Err((-32601, format!("Method not found: {}", method))),
    }
}
