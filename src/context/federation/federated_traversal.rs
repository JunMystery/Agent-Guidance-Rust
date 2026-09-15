//! Federated callers and callees symbol graph traversal across linked projects.

use std::path::Path;
use rusqlite::params;
use crate::context::db::CodeGraphDb;
use crate::context::multi_project::discover_linked_projects;
use super::workspace_detector::auto_discover_workspaces;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederatedSymbolRef {
    pub repo: String,
    pub file_path: String,
    pub symbol_name: String,
    pub line: usize,
    pub edge_type: String,
}

/// Gathers callers of `symbol_name` across primary project and all discovered/configured linked projects.
pub fn federated_search_callers(
    primary_path: &Path,
    symbol_name: &str,
    limit: usize,
) -> Vec<FederatedSymbolRef> {
    let mut results = Vec::new();

    // 1. Primary repository
    if let Ok(db) = CodeGraphDb::open_for_project(primary_path) {
        query_callers_on_db(&db, "local", symbol_name, limit, &mut results);
    }

    // 2. Discovered & configured linked repositories
    let mut linked = discover_linked_projects(primary_path);
    for ws in auto_discover_workspaces(primary_path) {
        if !linked.iter().any(|l| l.root_path == ws.root_path) {
            linked.push(ws);
        }
    }

    for proj in linked {
        if results.len() >= limit {
            break;
        }
        if let Some(db) = proj.open_db() {
            query_callers_on_db(&db, &proj.name, symbol_name, limit - results.len(), &mut results);
        }
    }

    results
}

/// Gathers callees of `symbol_name` across primary project and linked projects.
pub fn federated_search_callees(
    primary_path: &Path,
    symbol_name: &str,
    limit: usize,
) -> Vec<FederatedSymbolRef> {
    let mut results = Vec::new();

    // 1. Primary repository
    if let Ok(db) = CodeGraphDb::open_for_project(primary_path) {
        query_callees_on_db(&db, "local", symbol_name, limit, &mut results);
    }

    // 2. Linked repositories
    let mut linked = discover_linked_projects(primary_path);
    for ws in auto_discover_workspaces(primary_path) {
        if !linked.iter().any(|l| l.root_path == ws.root_path) {
            linked.push(ws);
        }
    }

    for proj in linked {
        if results.len() >= limit {
            break;
        }
        if let Some(db) = proj.open_db() {
            query_callees_on_db(&db, &proj.name, symbol_name, limit - results.len(), &mut results);
        }
    }

    results
}

fn query_callers_on_db(
    db: &CodeGraphDb,
    repo_name: &str,
    target_name: &str,
    limit: usize,
    out: &mut Vec<FederatedSymbolRef>,
) {
    let query = "SELECT s1.file_path, s1.name, s1.start_line, e.edge_type
                 FROM symbol_edges e
                 JOIN symbols s1 ON e.source_id = s1.id
                 JOIN symbols s2 ON e.target_id = s2.id
                 WHERE s2.name = ?1
                 LIMIT ?2";

    if let Ok(mut stmt) = db.conn.prepare(query) {
        if let Ok(rows) = stmt.query_map(params![target_name, limit as i64], |row| {
            Ok(FederatedSymbolRef {
                repo: repo_name.to_string(),
                file_path: row.get(0)?,
                symbol_name: row.get(1)?,
                line: row.get::<_, i64>(2)? as usize,
                edge_type: row.get(3)?,
            })
        }) {
            for r in rows.flatten() {
                out.push(r);
            }
        }
    }
}

fn query_callees_on_db(
    db: &CodeGraphDb,
    repo_name: &str,
    source_name: &str,
    limit: usize,
    out: &mut Vec<FederatedSymbolRef>,
) {
    let query = "SELECT s2.file_path, s2.name, s2.start_line, e.edge_type
                 FROM symbol_edges e
                 JOIN symbols s1 ON e.source_id = s1.id
                 JOIN symbols s2 ON e.target_id = s2.id
                 WHERE s1.name = ?1
                 LIMIT ?2";

    if let Ok(mut stmt) = db.conn.prepare(query) {
        if let Ok(rows) = stmt.query_map(params![source_name, limit as i64], |row| {
            Ok(FederatedSymbolRef {
                repo: repo_name.to_string(),
                file_path: row.get(0)?,
                symbol_name: row.get(1)?,
                line: row.get::<_, i64>(2)? as usize,
                edge_type: row.get(3)?,
            })
        }) {
            for r in rows.flatten() {
                out.push(r);
            }
        }
    }
}
