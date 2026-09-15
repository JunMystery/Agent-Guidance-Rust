use anyhow::Result;
use serde_json::json;
use std::path::Path;

use super::json_response;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphQueryParams {
    pub project: Option<String>,
    pub view: String,
    pub file: Option<String>,
}

pub(crate) fn parse_graph_query(query_str: &str) -> GraphQueryParams {
    let (mut project, mut view, mut file) = (None, None, None);
    for pair in query_str.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let decoded = v
                .replace("%20", " ")
                .replace("%2F", "/")
                .replace("%3A", ":")
                .replace("%5C", "\\");
            match k {
                "project" => {
                    if !decoded.is_empty() && decoded != "all" {
                        project = Some(decoded);
                    }
                }
                "view" => {
                    if !decoded.is_empty() {
                        view = Some(decoded);
                    }
                }
                "file" => {
                    if !decoded.is_empty() {
                        file = Some(decoded);
                    }
                }
                _ => {}
            }
        }
    }
    GraphQueryParams {
        project,
        view: view.unwrap_or_else(|| "symbols".to_string()),
        file,
    }
}

pub(crate) fn handle_api_graph(request: tiny_http::Request, default_proj: &str) {
    let url = request.url();
    let query_str = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let params = parse_graph_query(query_str);

    let effective_proj = params.project.as_deref().or_else(|| {
        if !default_proj.is_empty() && default_proj != "all" && default_proj != "." {
            Some(default_proj)
        } else {
            None
        }
    });

    let target_dir = match effective_proj {
        Some(p) => p,
        None => {
            json_response(
                request,
                200,
                &json!({
                    "graph_available": false,
                    "reason": "PROJECT_REQUIRED",
                    "message": "Please select a specific project from the Tracked Projects dropdown to view its architecture graph.",
                    "nodes": [],
                    "edges": [],
                    "communities": []
                }),
            );
            return;
        }
    };
    let path = Path::new(target_dir);

    if params.view == "file_functions" && params.file.is_none() {
        json_response(
            request,
            400,
            &json!({"error": "Missing required 'file' query parameter for view=file_functions"}),
        );
        return;
    }

    match query_graph_data_with_view(path, &params.view, params.file.as_deref()) {
        Ok(data) => json_response(request, 200, &data),
        Err(e) => json_response(request, 500, &json!({"error": e.to_string()})),
    }
}

pub fn query_graph_data(project_path: &Path) -> Result<serde_json::Value> {
    query_graph_data_with_view(project_path, "symbols", None)
}

pub fn query_graph_data_with_view(
    project_path: &Path,
    view: &str,
    file: Option<&str>,
) -> Result<serde_json::Value> {
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

    match view {
        "files" => {
            let res = super::graph_file_query::query_file_graph_data(&conn, project_path)?;
            Ok(serde_json::to_value(res)?)
        }
        "file_functions" => {
            let target_file = file.unwrap_or("");
            let res = super::graph_function_query::query_file_functions_data(&conn, project_path, target_file)?;
            Ok(serde_json::to_value(res)?)
        }
        _ => query_symbol_graph_data(&conn, project_path),
    }
}

fn query_symbol_graph_data(conn: &rusqlite::Connection, project_path: &Path) -> Result<serde_json::Value> {
    let query_res = super::graph_query::load_connected_graph_data(conn, 300)?;
    let nodes = query_res.nodes;
    let mut edges = query_res.edges;

    if let Ok(mut sem_stmt) = conn.prepare(
        "SELECT source_symbol, target_symbol, relation_type, confidence, description
         FROM semantic_edges LIMIT 500",
    ) {
        if let Ok(sem_edges) = sem_stmt.query_map([], |row| {
            let src: String = row.get(0)?;
            let tgt: String = row.get(1)?;
            let rel: String = row.get(2)?;
            let conf: f64 = row.get(3)?;
            let desc: Option<String> = row.get(4)?;
            Ok(json!({
                "source": src, "target": tgt, "type": rel, "weight": conf,
                "origin": "semantic", "dashed": true, "description": desc,
            }))
        }) {
            for se in sem_edges.filter_map(|r| r.ok()) { edges.push(se); }
        }
    }

    let comm_path = project_path.join(".agent-context").join("communities.json");
    let raw_comm: serde_json::Value = if comm_path.exists() {
        std::fs::read_to_string(&comm_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| json!([]))
    } else {
        json!([])
    };

    let communities = if let Some(arr) = raw_comm.get("communities").and_then(|c| c.as_array()) {
        json!(arr)
    } else if raw_comm.is_array() {
        raw_comm
    } else {
        json!([])
    };

    let engine = crate::context::graph_rag::GraphRagEngine::new(project_path);
    let mermaid_dag = engine.architecture_mermaid("Auto");

    Ok(json!({
        "graph_available": true,
        "view": "symbols",
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
