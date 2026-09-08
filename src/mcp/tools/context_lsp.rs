//! LSP definition, type definition, and symbol references bridge handlers.

use serde_json::Value;
use std::path::Path;

use crate::context::db::CodeGraphDb;
use crate::context::lsp::{
    line_from_lsp, query_lsp_definition, query_lsp_references, query_lsp_type_definition,
    uri_to_rel_path,
};

/// Attempts to resolve references using an active language server.
pub(crate) fn try_lsp_references(
    arguments: &Value,
    proj_path: &Path,
    query: &str,
    rel_path: &str,
) -> Option<String> {
    if rel_path.is_empty() {
        return None;
    }

    let line = arguments
        .get("line")
        .or_else(|| arguments.get("start_line"))
        .and_then(|l| l.as_u64())
        .map(|l| l as usize)?;

    let col = arguments
        .get("character")
        .or_else(|| arguments.get("col"))
        .and_then(|c| c.as_u64())
        .map(|c| c as usize)
        .unwrap_or(1);

    let (server_name, locs) = query_lsp_references(proj_path, rel_path, line, col)?;
    if locs.is_empty() {
        return None;
    }

    let mut lines = Vec::new();
    for loc in locs.iter().take(30) {
        let file_rel = uri_to_rel_path(&loc.uri, proj_path);
        let file_line = line_from_lsp(loc.range.start.line);
        lines.push(format!("- {}:L{} [LSP reference]", file_rel, file_line));
    }

    Some(format!(
        "# Symbol References for '{}' (Source: LSP [{}], Total: {})\n\n{}",
        if query.is_empty() { rel_path } else { query },
        server_name,
        lines.len(),
        lines.join("\n")
    ))
}

/// Handles the `definition` operation with LSP dispatch and SQLite fallback.
pub(crate) fn handle_definition(
    arguments: &Value,
    proj_path: &Path,
    query: &str,
    rel_path: &str,
) -> String {
    let line = arguments
        .get("line")
        .or_else(|| arguments.get("start_line"))
        .and_then(|l| l.as_u64())
        .map(|l| l as usize);
    let col = arguments
        .get("character")
        .or_else(|| arguments.get("col"))
        .and_then(|c| c.as_u64())
        .map(|c| c as usize)
        .unwrap_or(1);

    // 1. LSP Query
    if !rel_path.is_empty() {
        if let Some(target_line) = line {
            if let Some((server_name, locs)) = query_lsp_definition(proj_path, rel_path, target_line, col) {
                if !locs.is_empty() {
                    let mut lines = Vec::new();
                    for loc in locs.iter().take(10) {
                        let file_rel = uri_to_rel_path(&loc.uri, proj_path);
                        let file_line = line_from_lsp(loc.range.start.line);
                        lines.push(format!("- {}:L{} [LSP definition]", file_rel, file_line));
                    }
                    return format!(
                        "# Symbol Definition for '{}' (Source: LSP [{}])\n\n{}",
                        if query.is_empty() { rel_path } else { query },
                        server_name,
                        lines.join("\n")
                    );
                }
            }
        }
    }

    // 2. Fallback: SQLite symbol index
    let search_term = if query.is_empty() { rel_path } else { query };
    if let Ok(db) = CodeGraphDb::open_for_project(proj_path) {
        if let Ok(syms) = db.search_symbols(search_term, 10) {
            if !syms.is_empty() {
                let lines: Vec<String> = syms
                    .into_iter()
                    .map(|(p, n, l)| format!("- {}:L{} → `{}` [symbol index]", p, l, n))
                    .collect();
                return format!(
                    "# Symbol Definition for '{}' (Source: SQLite Symbol Index Fallback):\n\n{}",
                    search_term,
                    lines.join("\n")
                );
            }
        }
    }

    format!(
        "# Symbol Definition for '{}':\n\nNo exact definition found in symbol index or LSP.",
        search_term
    )
}

/// Handles the `type_definition` operation with LSP dispatch and fallback.
pub(crate) fn handle_type_definition(
    arguments: &Value,
    proj_path: &Path,
    query: &str,
    rel_path: &str,
) -> String {
    let line = arguments
        .get("line")
        .or_else(|| arguments.get("start_line"))
        .and_then(|l| l.as_u64())
        .map(|l| l as usize);
    let col = arguments
        .get("character")
        .or_else(|| arguments.get("col"))
        .and_then(|c| c.as_u64())
        .map(|c| c as usize)
        .unwrap_or(1);

    if !rel_path.is_empty() {
        if let Some(target_line) = line {
            if let Some((server_name, locs)) = query_lsp_type_definition(proj_path, rel_path, target_line, col) {
                if !locs.is_empty() {
                    let mut lines = Vec::new();
                    for loc in locs.iter().take(10) {
                        let file_rel = uri_to_rel_path(&loc.uri, proj_path);
                        let file_line = line_from_lsp(loc.range.start.line);
                        lines.push(format!("- {}:L{} [LSP type definition]", file_rel, file_line));
                    }
                    return format!(
                        "# Type Definition for '{}' (Source: LSP [{}])\n\n{}",
                        if query.is_empty() { rel_path } else { query },
                        server_name,
                        lines.join("\n")
                    );
                }
            }
        }
    }

    handle_definition(arguments, proj_path, query, rel_path)
}
