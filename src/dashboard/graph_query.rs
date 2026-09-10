//! Database queries and 1-hop connected neighborhood expansion for the dashboard graph view.

use anyhow::Result;
use rusqlite::Connection;
use serde_json::json;
use std::collections::{HashMap, HashSet};

pub struct GraphQueryResult {
    pub nodes: Vec<serde_json::Value>,
    pub edges: Vec<serde_json::Value>,
}

fn parse_node_row(row: &rusqlite::Row<'_>, has_rich: bool) -> rusqlite::Result<(String, serde_json::Value)> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let kind: String = row.get(2)?;
    let file: String = row.get(3)?;
    let parent: Option<String> = row.get(4)?;
    let s_line: usize = row.get(5)?;
    let e_line: usize = row.get(6)?;
    let deg: i64 = row.get(7).unwrap_or(0);
    let (lang, namespace, receiver) = if has_rich {
        let l_db: Option<String> = row.get(8)?;
        let ns: Option<String> = row.get(9)?;
        let rc: Option<String> = row.get(10)?;
        let l = l_db.filter(|s| s != "unknown").unwrap_or_else(|| {
            crate::context::ast::AstLanguage::from_path(&file).as_str().to_string()
        });
        (l, ns, rc)
    } else {
        (crate::context::ast::AstLanguage::from_path(&file).as_str().to_string(), None, None)
    };
    let loc = (e_line.saturating_sub(s_line) + 1) as i64;
    Ok((
        id.clone(),
        json!({
            "id": id,
            "label": name,
            "kind": kind,
            "file": file,
            "parent": parent,
            "lang": lang,
            "namespace": namespace,
            "receiver": receiver,
            "loc": loc,
            "deg": deg,
        }),
    ))
}

/// Executes balanced sampling and 1-hop edge expansion to produce a cohesive graph subgraph.
pub fn load_connected_graph_data(conn: &Connection, max_nodes: usize) -> Result<GraphQueryResult> {
    let primary_limit = (max_nodes * 2 / 3).max(120);
    let has_rich_cols = conn.prepare("SELECT language, namespace, receiver FROM symbols LIMIT 0").is_ok();
    let has_rich_edge_cols = conn.prepare("SELECT confidence, category, call_line FROM symbol_edges LIMIT 0").is_ok();

    let mut node_map = HashMap::new();

    // 1. Select primary hub nodes partitioned across top-level architectural directories
    let node_cols = if has_rich_cols { "s.language, s.namespace, s.receiver" } else { "NULL, NULL, NULL" };
    let primary_sql = format!(
        "WITH ranked_symbols AS (
            SELECT s.id, s.name, s.kind, s.file_path, s.parent, s.start_line, s.end_line,
                   {},
                   (SELECT COUNT(*) FROM symbol_edges e WHERE e.source_id = s.id OR e.target_id = s.id) as degree,
                   ROW_NUMBER() OVER (
                       PARTITION BY substr(replace(s.file_path, '\\', '/'), 1, 
                           case when instr(replace(s.file_path, '\\', '/'), '/') > 0 
                                then instr(replace(s.file_path, '\\', '/'), '/') - 1 
                                else length(s.file_path) end)
                       ORDER BY (SELECT COUNT(*) FROM symbol_edges e WHERE e.source_id = s.id OR e.target_id = s.id) DESC,
                                (s.end_line - s.start_line) DESC
                   ) as rank_in_domain
            FROM symbols s
            WHERE s.kind != 'package'
        )
        SELECT id, name, kind, file_path, parent, start_line, end_line, degree,
               language, namespace, receiver
        FROM ranked_symbols
        WHERE rank_in_domain <= 35
        ORDER BY degree DESC
        LIMIT ?1",
        if has_rich_cols { "s.language, s.namespace, s.receiver" } else { "NULL as language, NULL as namespace, NULL as receiver" }
    );

    let mut stmt = conn.prepare(&primary_sql)?;
    let rows = stmt.query_map([primary_limit], |row| parse_node_row(row, has_rich_cols))?;
    for r in rows.filter_map(|res| res.ok()) {
        node_map.insert(r.0, r.1);
    }

    // 2. Fetch edges touching the selected primary nodes
    let mut candidate_edges = Vec::new();
    let mut neighbor_ids = HashSet::new();

    let edge_cols = if has_rich_edge_cols {
        "e.confidence, e.category, e.call_line"
    } else {
        "1.0 as confidence, 'symbol' as category, NULL as call_line"
    };

    let edge_sql = format!(
        "WITH selected AS (
            SELECT id FROM (
                SELECT s.id,
                       ROW_NUMBER() OVER (
                           PARTITION BY substr(replace(s.file_path, '\\', '/'), 1, 
                               case when instr(replace(s.file_path, '\\', '/'), '/') > 0 
                                    then instr(replace(s.file_path, '\\', '/'), '/') - 1 
                                    else length(s.file_path) end)
                           ORDER BY (SELECT COUNT(*) FROM symbol_edges e WHERE e.source_id = s.id OR e.target_id = s.id) DESC,
                                    (s.end_line - s.start_line) DESC
                       ) as rank_in_domain
                FROM symbols s
                WHERE s.kind != 'package'
            ) WHERE rank_in_domain <= 35 LIMIT ?1
        )
        SELECT e.source_id, e.target_id, e.edge_type, e.weight, {}
        FROM symbol_edges e
        WHERE e.source_id IN (SELECT id FROM selected)
           OR e.target_id IN (SELECT id FROM selected)
        ORDER BY e.weight DESC
        LIMIT 800",
        edge_cols
    );

    let mut edge_stmt = conn.prepare(&edge_sql)?;
    let edge_rows = edge_stmt.query_map([primary_limit], |row| {
        let src: String = row.get(0)?;
        let tgt: String = row.get(1)?;
        let etype: String = row.get(2)?;
        let weight: f64 = row.get(3)?;
        let conf: f64 = row.get(4).unwrap_or(1.0);
        let cat: String = row.get(5).unwrap_or_else(|_| "symbol".to_string());
        let call_line: Option<usize> = row.get(6)?;
        Ok((src, tgt, etype, weight, conf, cat, call_line))
    })?;

    for er in edge_rows.filter_map(|r| r.ok()) {
        let (src, tgt, etype, weight, conf, cat, call_line) = er;
        if !node_map.contains_key(&src) {
            neighbor_ids.insert(src.clone());
        }
        if !node_map.contains_key(&tgt) {
            neighbor_ids.insert(tgt.clone());
        }
        candidate_edges.push((src, tgt, etype, weight, conf, cat, call_line));
    }

    // 3. Hydrate neighbor nodes up to max_nodes limit
    let remaining_budget = max_nodes.saturating_sub(node_map.len());
    if remaining_budget > 0 && !neighbor_ids.is_empty() {
        let n_sql = format!(
            "SELECT s.id, s.name, s.kind, s.file_path, s.parent, s.start_line, s.end_line,
                    (SELECT COUNT(*) FROM symbol_edges e WHERE e.source_id = s.id OR e.target_id = s.id) as degree,
                    {}
             FROM symbols s
             WHERE s.id = ?1",
            node_cols
        );
        let mut n_stmt = conn.prepare(&n_sql)?;

        for nid in neighbor_ids.iter().take(remaining_budget) {
            if let Ok(mut q) = n_stmt.query([nid]) {
                if let Ok(Some(row)) = q.next() {
                    if let Ok((id, val)) = parse_node_row(&row, has_rich_cols) {
                        node_map.insert(id, val);
                    }
                }
            }
        }
    }

    // 4. Closed set edge filtering: only emit edges where BOTH endpoints exist in node_map
    let mut edges = Vec::new();
    for (src, tgt, etype, weight, conf, cat, call_line) in candidate_edges {
        if node_map.contains_key(&src) && node_map.contains_key(&tgt) {
            edges.push(json!({
                "source": src,
                "target": tgt,
                "type": etype,
                "weight": weight,
                "confidence": conf,
                "category": cat,
                "call_line": call_line,
                "origin": "ast",
                "dashed": false,
            }));
        }
    }

    let mut sorted_nodes: Vec<serde_json::Value> = node_map.into_values().collect();
    sorted_nodes.sort_by(|a, b| {
        let deg_b = b.get("deg").and_then(|v| v.as_i64()).unwrap_or(0);
        let deg_a = a.get("deg").and_then(|v| v.as_i64()).unwrap_or(0);
        let loc_b = b.get("loc").and_then(|v| v.as_i64()).unwrap_or(0);
        let loc_a = a.get("loc").and_then(|v| v.as_i64()).unwrap_or(0);
        deg_b.cmp(&deg_a).then_with(|| loc_b.cmp(&loc_a))
    });

    Ok(GraphQueryResult {
        nodes: sorted_nodes,
        edges,
    })
}
