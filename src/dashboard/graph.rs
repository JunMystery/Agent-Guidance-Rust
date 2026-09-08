use anyhow::Result;
use serde_json::json;
use std::path::Path;

use super::json_response;

pub(crate) fn handle_api_graph(request: tiny_http::Request, default_proj: &str) {
    let url = request.url();
    let query_str = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut project_param = None;

    for pair in query_str.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == "project" {
                let decoded = v
                    .replace("%20", " ")
                    .replace("%2F", "/")
                    .replace("%3A", ":")
                    .replace("%5C", "\\");
                if !decoded.is_empty() && decoded != "all" {
                    project_param = Some(decoded);
                }
            }
        }
    }

    let target_dir = project_param.as_deref().unwrap_or(default_proj);
    let path = Path::new(target_dir);

    match query_graph_data(path) {
        Ok(data) => json_response(request, 200, &data),
        Err(e) => json_response(request, 500, &json!({"error": e.to_string()})),
    }
}

pub fn query_graph_data(project_path: &Path) -> Result<serde_json::Value> {
    if !project_path.exists() {
        return Ok(json!({
            "graph_available": false,
            "reason": "PROJECT_NOT_FOUND",
            "message": format!("Project directory '{}' does not exist on disk", project_path.display()),
            "project_path": project_path.to_string_lossy(),
            "nodes": [],
            "edges": [],
            "communities": []
        }));
    }

    let db_path = project_path.join(".agent-context").join("code_graph.db");
    if !db_path.exists() {
        return Ok(json!({
            "graph_available": false,
            "reason": "GRAPH_NOT_INDEXED",
            "message": "Code graph has not been indexed yet. Run reindex or project_context to generate .agent-context/code_graph.db.",
            "project_path": project_path.to_string_lossy(),
            "nodes": [],
            "edges": [],
            "communities": []
        }));
    }

    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, name, kind, file_path, start_line, end_line
         FROM symbols ORDER BY (end_line - start_line) DESC LIMIT 250",
    )?;

    let nodes: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            let s_line: usize = row.get(4)?;
            let e_line: usize = row.get(5)?;
            let loc = (e_line.saturating_sub(s_line) + 1) as i64;
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "label": row.get::<_, String>(1)?,
                "kind": row.get::<_, String>(2)?,
                "file": row.get::<_, String>(3)?,
                "loc": loc,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn.prepare(
        "SELECT source_id, target_id, edge_type, weight
         FROM symbol_edges LIMIT 500",
    )?;

    let edges: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "source": row.get::<_, String>(0)?,
                "target": row.get::<_, String>(1)?,
                "type": row.get::<_, String>(2)?,
                "weight": row.get::<_, f64>(3)?,
            }))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Read communities if available
    let comm_path = project_path.join(".agent-context").join("communities.json");
    let communities: serde_json::Value = if comm_path.exists() {
        std::fs::read_to_string(&comm_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| json!([]))
    } else {
        json!([])
    };

    let engine = crate::context::graph_rag::GraphRagEngine::new(project_path);
    let mermaid_dag = engine.architecture_mermaid("Auto");

    Ok(json!({
        "graph_available": true,
        "project_path": project_path.to_string_lossy(),
        "total_nodes": nodes.len(),
        "total_edges": edges.len(),
        "nodes": nodes,
        "edges": edges,
        "communities": communities,
        "mermaid_dag": mermaid_dag
    }))
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;

