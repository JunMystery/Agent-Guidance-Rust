//! Query engine aggregating file-level dependencies from symbols and symbol_edges.

use anyhow::Result;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::graph_contract::{FileCallDetail, FileGraphEdge, FileGraphNode, FileGraphResponse};

pub fn query_file_graph_data(conn: &Connection, project_path: &Path) -> Result<FileGraphResponse> {
    let has_files_tbl = conn.prepare("SELECT path FROM files LIMIT 0").is_ok();
    let has_chunks_tbl = conn.prepare("SELECT file_path FROM content_chunks LIMIT 0").is_ok();
    let has_call_line = conn.prepare("SELECT call_line FROM symbol_edges LIMIT 0").is_ok();

    let loc_expr = if has_chunks_tbl {
        "COALESCE((SELECT MAX(end_line) FROM content_chunks WHERE file_path = f.path), (SELECT MAX(end_line) FROM symbols WHERE file_path = f.path), 0)"
    } else {
        "COALESCE((SELECT MAX(end_line) FROM symbols WHERE file_path = f.path), 0)"
    };
    let files_from = if has_files_tbl {
        "(SELECT path FROM files UNION SELECT file_path AS path FROM symbols) f"
    } else {
        "(SELECT DISTINCT file_path AS path FROM symbols) f"
    };

    let node_sql = format!(
        "SELECT f.path, {}, (SELECT COUNT(*) FROM symbols WHERE file_path = f.path) FROM {}",
        loc_expr, files_from
    );

    let mut file_map: HashMap<String, FileGraphNode> = HashMap::new();
    let mut node_stmt = conn.prepare(&node_sql)?;
    let node_rows = node_stmt.query_map([], |row| {
        let p: String = row.get(0)?;
        let loc: usize = row.get(1)?;
        let syms: usize = row.get(2)?;
        Ok((p, loc, syms))
    })?;

    for r in node_rows.flatten() {
        file_map.insert(r.0.clone(), FileGraphNode {
            id: r.0.clone(),
            path: r.0,
            loc: r.1,
            total_symbols: r.2,
            in_degree: 0,
            out_degree: 0,
        });
    }

    let call_col = if has_call_line { "e.call_line" } else { "NULL" };
    let edge_sql = format!(
        "SELECT s1.file_path, s2.file_path, s1.id, s1.name, s2.id, s2.name, e.edge_type, e.weight, {}
         FROM symbol_edges e
         JOIN symbols s1 ON e.source_id = s1.id
         JOIN symbols s2 ON e.target_id = s2.id
         WHERE s1.file_path != s2.file_path",
        call_col
    );

    let mut edge_stmt = conn.prepare(&edge_sql)?;
    let edge_rows = edge_stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, f64>(7)?,
            row.get::<_, Option<usize>>(8)?,
        ))
    })?;

    let mut edge_map: HashMap<(String, String), FileGraphEdge> = HashMap::new();
    let mut in_deg_map: HashMap<String, HashSet<String>> = HashMap::new();
    let mut out_deg_map: HashMap<String, HashSet<String>> = HashMap::new();

    for r in edge_rows.flatten() {
        let (src_f, tgt_f, src_id, src_name, tgt_id, tgt_name, edge_type, weight, call_line) = r;
        if src_f == tgt_f {
            continue;
        }

        out_deg_map.entry(src_f.clone()).or_default().insert(tgt_f.clone());
        in_deg_map.entry(tgt_f.clone()).or_default().insert(src_f.clone());

        let entry = edge_map.entry((src_f.clone(), tgt_f.clone())).or_insert_with(|| FileGraphEdge {
            source: src_f,
            target: tgt_f,
            weight: 0.0,
            calls: Vec::new(),
        });
        entry.weight += weight;
        entry.calls.push(FileCallDetail {
            source_func: src_name.clone(),
            target_func: tgt_name.clone(),
            caller: src_name,
            callee: tgt_name,
            source_id: Some(src_id),
            target_id: Some(tgt_id),
            edge_type,
            call_line,
        });
    }

    for (path, node) in file_map.iter_mut() {
        node.in_degree = in_deg_map.get(path).map_or(0, |s| s.len());
        node.out_degree = out_deg_map.get(path).map_or(0, |s| s.len());
    }

    let mut nodes: Vec<FileGraphNode> = file_map.into_values().collect();
    nodes.sort_by(|a, b| {
        (b.in_degree + b.out_degree)
            .cmp(&(a.in_degree + a.out_degree))
            .then_with(|| a.path.cmp(&b.path))
    });

    let mut edges: Vec<FileGraphEdge> = edge_map.into_values().collect();
    edges.sort_by(|a, b| {
        b.weight
            .partial_cmp(&a.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.source.cmp(&b.source))
    });

    Ok(FileGraphResponse {
        graph_available: true,
        view: "files".to_string(),
        project_path: project_path.to_string_lossy().to_string(),
        total_nodes: nodes.len(),
        total_edges: edges.len(),
        nodes,
        edges,
    })
}
