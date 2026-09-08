use serde_json::Value;

use crate::mcp::learnings;
use crate::mcp::state::ServerState;
use super::continuity_sessions::{
    handle_clear, handle_list, handle_load, handle_save, handle_switch,
};
use super::helpers::{detect_project_path, ensure_not_cancelled};

pub(crate) fn handle(
    arguments: Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    ensure_not_cancelled(state)?;
    let op = arguments
        .get("operation")
        .and_then(|o| o.as_str())
        .unwrap_or("load");
    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);

    let resp = match op {
        "save" => handle_save(state, &proj_path),
        "load" => handle_load(state, &proj_path),
        "clear" => handle_clear(state, &proj_path),
        "learn" => {
            let learning = arguments
                .get("learning")
                .and_then(|l| l.as_str())
                .unwrap_or("");
            let category = arguments
                .get("category")
                .and_then(|c| c.as_str())
                .unwrap_or("general");
            let is_pinned = arguments
                .get("pinned")
                .or_else(|| arguments.get("is_pinned"))
                .and_then(|p| p.as_bool())
                .unwrap_or(false);
            match learnings::record_project_learning(&proj_path, learning, category, is_pinned) {
                Ok(msg) => msg,
                Err(e) => format!("Failed to record learning: {}", e),
            }
        }
        "handoff" => {
            let next_action = arguments
                .get("next_action")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            match learnings::write_handoff_summary(&proj_path, state, next_action) {
                Ok(msg) => msg,
                Err(e) => format!("Failed to generate handoff protocol: {}", e),
            }
        }
        "diff" | "changes" => learnings::generate_session_diff_summary(&proj_path, state),
        "list" | "sessions" => handle_list(state, &proj_path),
        "switch" => handle_switch(&arguments, state, &proj_path),
        _ => format!("# Session Continuity: [{}]\n\nSession state active.", op),
    };
    Ok(resp)
}