use serde_json::Value;
pub use serde_json::json;
pub use std::path::Path;
use tracing::info;

use crate::mcp::state::ServerState;
use crate::optimizer::compressor::{compress_markdown, estimate_tokens};

mod helpers;
pub use helpers::*;

mod pipeline;
mod skills;
mod guidance;
mod guidance_docs;
mod guidance_precode;
mod guidance_search;
mod guidance_verify;
mod context;
mod context_read;
mod context_search;
mod context_graph;
mod context_symbols;
pub(crate) mod context_enrich;
pub(crate) mod context_lsp;
mod continuity;
mod continuity_sessions;
mod gate;
mod gate_stage;
mod gate_approval;
pub(crate) mod gate_edit;
mod gate_edit_modularity;

fn extract_target(args: &Value) -> Option<String> {
    if let Some(p) = args.get("relative_path").or_else(|| args.get("file_path")).or_else(|| args.get("path")).and_then(|v| v.as_str()) {
        if let Some(sym) = args.get("target_symbol").and_then(|v| v.as_str()) {
            return Some(format!("{}#{}", p, sym));
        }
        return Some(p.to_string());
    }
    if let Some(sym) = args.get("target_symbol").and_then(|v| v.as_str()) {
        return Some(format!("#{}", sym));
    }
    if let Some(q) = args.get("query").or_else(|| args.get("alias_term")).or_else(|| args.get("key")).and_then(|v| v.as_str()) {
        let trimmed = q.trim();
        if trimmed.len() > 60 {
            return Some(format!("{}...", &trimmed[..60]));
        }
        return Some(trimmed.to_string());
    }
    if let Some(t) = args.get("task").and_then(|v| v.as_str()) {
        let trimmed = t.trim();
        if trimmed.len() > 60 {
            return Some(format!("{}...", &trimmed[..60]));
        }
        return Some(trimmed.to_string());
    }
    None
}

pub fn handle_tool_call(
    name: &str,
    arguments: Value,
    state: &mut ServerState,
) -> Result<Value, (i32, String)> {
    info!("Handling tool call: {}", name);
    ensure_not_cancelled(state)?;

    let start_time = std::time::Instant::now();
    let target = extract_target(&arguments);
    let op = arguments
        .get("operation")
        .or_else(|| arguments.get("action"))
        .or_else(|| arguments.get("phase"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            if name == "select_skills" || name == "select_skill" {
                let skills_val = arguments.get("skills").or_else(|| arguments.get("skill"));
                if let Some(arr) = skills_val.and_then(|v| v.as_array()) {
                    let names: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                    if !names.is_empty() {
                        Some(names.join(", "))
                    } else {
                        Some("none".to_string())
                    }
                } else if let Some(s) = skills_val.and_then(|v| v.as_str()) {
                    Some(s.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        });

    let res = match handle_tool_call_internal(name, arguments, state) {
        Ok(mut val) => {
            let duration = start_time.elapsed().as_millis() as u64;

            // Universal token optimization & compression across all MCP tool responses
            let opt_enabled = std::env::var("AGENT_GUIDANCE_TOKEN_OPT")
                .map(|v| v != "0")
                .unwrap_or(true);

            let mut orig_tokens = 0;
            let mut opt_tokens = 0;

            if let Some(content_arr) = val.get_mut("content").and_then(|c| c.as_array_mut()) {
                for item in content_arr.iter_mut() {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        let orig = estimate_tokens(text, false) as u64;
                        orig_tokens += orig;
                        if opt_enabled {
                            let compressed = compress_markdown(text);
                            let opt = estimate_tokens(&compressed, false) as u64;
                            opt_tokens += opt;
                            if let Some(obj) = item.as_object_mut() {
                                obj.insert("text".to_string(), Value::String(compressed));
                            }
                        } else {
                            opt_tokens += orig;
                        }
                    }
                }
            }

            state.record_call(orig_tokens, opt_tokens);
            let proj_path = state.project_path.as_deref().or_else(|| state.workspace_roots.first().map(|s| s.as_str()));
            crate::mcp::db::log_tool_call(name, op.as_deref(), target.as_deref(), orig_tokens, opt_tokens, duration, None, proj_path);
            Ok(val)
        }
        Err(err) => {
            let duration = start_time.elapsed().as_millis() as u64;
            let proj_path = state.project_path.as_deref().or_else(|| state.workspace_roots.first().map(|s| s.as_str()));
            crate::mcp::db::log_tool_call(name, op.as_deref(), target.as_deref(), 0, 0, duration, Some(&err.1), proj_path);
            Err(err)
        }
    };
    res
}

fn handle_tool_call_internal(
    name: &str,
    arguments: Value,
    state: &mut ServerState,
) -> Result<Value, (i32, String)> {
    let raw_result = match name {
        "task_pipeline" => pipeline::handle(arguments, state),
        "select_skills" | "select_skill" => skills::handle(arguments, state),
        "guidance" => guidance::handle(arguments, state),
        "ui_ux" => {
            let query = arguments
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("general");
            Ok(format!(
                "# UI/UX Guidelines for '{}'\n\n- Styling: Modern CSS, Glassmorphism, Dynamic Animations\n- Color Palette: Dark mode default, curated HSL gradients\n- Typography: Inter/Outfit via Google Fonts\n- Accessibility: Semantic HTML5, unique IDs",
                query
            ))
        }
        "project_context" => context::handle(arguments, state),
        "session_continuity" => continuity::handle(arguments, state),
        "workflow_gate" => gate::handle(arguments, state),
        _ => Err((-32601, format!("Method not found: {}", name))),
    }?;

    Ok(serde_json::json!({
        "content": [
            {
                "type": "text",
                "text": raw_result
            }
        ]
    }))
}

#[cfg(test)]
#[path = "../tools_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tools_read_tests.rs"]
mod read_tests;
