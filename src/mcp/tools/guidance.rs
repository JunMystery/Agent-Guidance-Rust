use serde_json::Value;

use crate::mcp::state::ServerState;
use super::ensure_not_cancelled;

pub(crate) fn handle(
    arguments: Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    ensure_not_cancelled(state)?;
    let op = arguments
        .get("operation")
        .and_then(|o| o.as_str())
        .unwrap_or("list");
    let query = arguments
        .get("query")
        .and_then(|q| q.as_str())
        .unwrap_or("")
        .to_lowercase();

    match op {
        "list" => super::guidance_search::handle_list(&arguments, state),
        "search" => super::guidance_search::handle_search(&arguments, &query, state),
        "reindex_skills" | "reindex" => super::guidance_search::handle_reindex(&arguments, state),
        "get" => super::guidance_docs::handle_get(&arguments, state),
        "docs" => super::guidance_docs::handle_docs(&arguments, &query, state),
        "workflow" => super::guidance_docs::handle_workflow(&arguments),
        "ui_ux" => super::guidance_docs::handle_ui_ux(&query),
        "precode" => Ok(super::guidance_precode::handle_precode(&arguments, &query, state)),
        "verify" => super::guidance_verify::handle_verify(&arguments, state),
        _ => Ok(format!("Guidance operation '{}' completed successfully.", op)),
    }
}