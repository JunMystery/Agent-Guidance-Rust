//! Query engine retrieving functions and call relationships for a specific target file or all files.

use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

use super::graph_contract::{FileFunctionsResponse, FunctionGraphEdge, FunctionGraphNode};

struct RawEdge {
    src: String, tgt: String, etype: String, weight: f64, conf: f64, call_line: Option<usize>,
    s1_file: String, s1_name: String, s1_kind: String, s1_s: usize, s1_e: usize,
    s2_file: String, s2_name: String, s2_kind: String, s2_s: usize, s2_e: usize,
}

fn parse_edge_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawEdge> {
    Ok(RawEdge {
        src: row.get(0)?, tgt: row.get(1)?, etype: row.get(2)?, weight: row.get(3)?,
        conf: row.get(4)?, call_line: row.get(5)?,
        s1_file: row.get(6)?, s1_name: row.get(7)?, s1_kind: row.get(8)?, s1_s: row.get(9)?, s1_e: row.get(10)?,
        s2_file: row.get(11)?, s2_name: row.get(12)?, s2_kind: row.get(13)?, s2_s: row.get(14)?, s2_e: row.get(15)?,
    })
}

fn make_node(id: String, name: String, kind: String, file: String, s: usize, e: usize, ext: bool, scope: &str) -> FunctionGraphNode {
    FunctionGraphNode {
        id: id.clone(), label: name.clone(), name, kind, file_path: file.clone(), file,
        start_line: s, end_line: e, loc: e.saturating_sub(s) + 1,
        is_external: ext, scope: scope.to_string(), deg: 0,
    }
}

pub fn query_file_functions_data(conn: &Connection, project_path: &Path, target_file: &str) -> Result<FileFunctionsResponse> {
    let target_norm = target_file.replace('\\', "/");
    let is_all = target_norm == "all" || target_norm.is_empty();

    let has_rich_edge = conn.prepare("SELECT confidence, call_line FROM symbol_edges LIMIT 0").is_ok();
    let edge_cols = if has_rich_edge { "e.confidence, e.call_line" } else { "1.0 as confidence, NULL as call_line" };

    if is_all {
        let mut node_map: HashMap<String, FunctionGraphNode> = HashMap::new();
        let mut edges = Vec::new();
        let edge_sql = format!(
            "SELECT e.source_id, e.target_id, e.edge_type, e.weight, {},
                    s1.file_path, s1.name, s1.kind, s1.start_line, s1.end_line,
                    s2.file_path, s2.name, s2.kind, s2.start_line, s2.end_line
             FROM symbol_edges e
             JOIN symbols s1 ON e.source_id = s1.id
             JOIN symbols s2 ON e.target_id = s2.id
             WHERE replace(s1.file_path, '\\', '/') != replace(s2.file_path, '\\', '/')
             ORDER BY e.weight DESC LIMIT 300",
            edge_cols
        );
        let mut edge_stmt = conn.prepare(&edge_sql)?;
        let edge_rows = edge_stmt.query_map([], parse_edge_row)?;

        for r in edge_rows.flatten() {
            node_map.entry(r.src.clone()).or_insert_with(|| make_node(r.src.clone(), r.s1_name, r.s1_kind, r.s1_file, r.s1_s, r.s1_e, false, "all"));
            node_map.entry(r.tgt.clone()).or_insert_with(|| make_node(r.tgt.clone(), r.s2_name, r.s2_kind, r.s2_file, r.s2_s, r.s2_e, false, "all"));
            edges.push(FunctionGraphEdge {
                source: r.src, target: r.tgt, edge_type: r.etype, direction: "outgoing".to_string(),
                weight: r.weight, confidence: r.conf, call_line: r.call_line, origin: "ast".to_string(), dashed: false,
            });
        }
        let mut deg_map: HashMap<String, usize> = HashMap::new();
        for e in &edges {
            *deg_map.entry(e.source.clone()).or_default() += 1;
            *deg_map.entry(e.target.clone()).or_default() += 1;
        }
        for (id, node) in node_map.iter_mut() { node.deg = deg_map.get(id).copied().unwrap_or(0); }
        let mut nodes: Vec<FunctionGraphNode> = node_map.into_values().collect();
        nodes.sort_by(|a, b| a.file_path.cmp(&b.file_path).then_with(|| a.start_line.cmp(&b.start_line)));

        return Ok(FileFunctionsResponse {
            graph_available: true, view: "file_functions".to_string(),
            project_path: project_path.to_string_lossy().to_string(),
            file: "all".to_string(), total_nodes: nodes.len(), total_edges: edges.len(), nodes, edges,
        });
    }

    let mut internal_nodes: HashMap<String, FunctionGraphNode> = HashMap::new();
    let mut external_nodes: HashMap<String, FunctionGraphNode> = HashMap::new();
    let mut edges = Vec::new();

    let mut sym_stmt = conn.prepare(
        "SELECT id, name, kind, file_path, start_line, end_line
         FROM symbols WHERE replace(file_path, '\\', '/') = ?1 OR file_path = ?1 ORDER BY start_line ASC",
    )?;
    let sym_rows = sym_stmt.query_map([&target_norm], |row| {
        let (id, name, kind, fpath, s, e): (String, String, String, String, usize, usize) =
            (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?);
        Ok(make_node(id, name, kind, fpath, s, e, false, "internal"))
    })?;
    for node in sym_rows.flatten() { internal_nodes.insert(node.id.clone(), node); }

    let edge_sql = format!(
        "SELECT e.source_id, e.target_id, e.edge_type, e.weight, {},
                s1.file_path, s1.name, s1.kind, s1.start_line, s1.end_line,
                s2.file_path, s2.name, s2.kind, s2.start_line, s2.end_line
         FROM symbol_edges e
         JOIN symbols s1 ON e.source_id = s1.id
         JOIN symbols s2 ON e.target_id = s2.id
         WHERE replace(s1.file_path, '\\', '/') = ?1 OR replace(s2.file_path, '\\', '/') = ?1
            OR s1.file_path = ?1 OR s2.file_path = ?1",
        edge_cols
    );
    let mut edge_stmt = conn.prepare(&edge_sql)?;
    let edge_rows = edge_stmt.query_map([&target_norm], parse_edge_row)?;

    for r in edge_rows.flatten() {
        let is_s1 = r.s1_file.replace('\\', "/") == target_norm || r.s1_file == target_file;
        let is_s2 = r.s2_file.replace('\\', "/") == target_norm || r.s2_file == target_file;
        let direction = if is_s1 && is_s2 {
            "internal".to_string()
        } else if is_s1 {
            if !external_nodes.contains_key(&r.tgt) && !internal_nodes.contains_key(&r.tgt) {
                external_nodes.insert(r.tgt.clone(), make_node(r.tgt.clone(), r.s2_name, r.s2_kind, r.s2_file, r.s2_s, r.s2_e, true, "external"));
            }
            "outgoing".to_string()
        } else {
            if !external_nodes.contains_key(&r.src) && !internal_nodes.contains_key(&r.src) {
                external_nodes.insert(r.src.clone(), make_node(r.src.clone(), r.s1_name, r.s1_kind, r.s1_file, r.s1_s, r.s1_e, true, "external"));
            }
            "incoming".to_string()
        };

        edges.push(FunctionGraphEdge {
            source: r.src, target: r.tgt, edge_type: r.etype, direction,
            weight: r.weight, confidence: r.conf, call_line: r.call_line, origin: "ast".to_string(), dashed: false,
        });
    }

    let mut deg_map: HashMap<String, usize> = HashMap::new();
    for e in &edges {
        *deg_map.entry(e.source.clone()).or_default() += 1;
        *deg_map.entry(e.target.clone()).or_default() += 1;
    }
    for (id, node) in internal_nodes.iter_mut() { node.deg = deg_map.get(id).copied().unwrap_or(0); }
    for (id, node) in external_nodes.iter_mut() { node.deg = deg_map.get(id).copied().unwrap_or(0); }

    let mut nodes: Vec<FunctionGraphNode> = internal_nodes.into_values().collect();
    nodes.sort_by_key(|n| n.start_line);
    let mut ext_nodes: Vec<FunctionGraphNode> = external_nodes.into_values().collect();
    ext_nodes.sort_by(|a, b| a.file_path.cmp(&b.file_path).then_with(|| a.start_line.cmp(&b.start_line)));
    nodes.extend(ext_nodes);

    Ok(FileFunctionsResponse {
        graph_available: true, view: "file_functions".to_string(),
        project_path: project_path.to_string_lossy().to_string(),
        file: target_file.to_string(), total_nodes: nodes.len(), total_edges: edges.len(), nodes, edges,
    })
}
