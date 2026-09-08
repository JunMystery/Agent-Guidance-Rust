use serde_json::Value;
use std::path::Path;

use crate::context::db::CodeGraphDb;
use crate::context::scanner::scan_project;
use super::helpers::validate_path;

pub(crate) fn handle_symbols(proj_path: &Path, rel_path: &str) -> String {
    if rel_path.is_empty() {
        return "Error: relative_path is required for symbols/structure operation. Example: project_context(operation=\"symbols\", project_path=\"...\", relative_path=\"src/main.rs\")".to_string();
    }

    match validate_path(proj_path, rel_path) {
        Ok(full_path) => {
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let mut symbols = Vec::new();
                for (idx, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("pub fn ")
                        || trimmed.starts_with("fn ")
                        || trimmed.starts_with("pub struct ")
                        || trimmed.starts_with("struct ")
                        || trimmed.starts_with("pub enum ")
                        || trimmed.starts_with("enum ")
                        || trimmed.starts_with("pub trait ")
                        || trimmed.starts_with("trait ")
                        || trimmed.starts_with("impl ")
                        || trimmed.starts_with("def ")
                        || trimmed.starts_with("async def ")
                        || trimmed.starts_with("class ")
                        || trimmed.starts_with("fun ")
                        || trimmed.starts_with("suspend fun ")
                        || trimmed.starts_with("override fun ")
                        || trimmed.starts_with("object ")
                        || trimmed.starts_with("companion object")
                        || trimmed.starts_with("interface ")
                        || trimmed.starts_with("export function")
                        || trimmed.starts_with("export class")
                        || trimmed.starts_with("export const")
                        || trimmed.starts_with("export interface")
                        || trimmed.starts_with("func ")
                    {
                        symbols.push(format!("L{:04}: {}", idx + 1, trimmed));
                    }
                }
                format!(
                    "# Code Symbol Signatures: {}\n\n{}",
                    rel_path,
                    if symbols.is_empty() {
                        "No top-level symbol signatures found.".to_string()
                    } else {
                        symbols.join("\n")
                    }
                )
            } else {
                format!("Failed to read file '{}'", rel_path)
            }
        }
        Err(err_msg) => format!("Security Error: {}", err_msg),
    }
}

pub(crate) fn handle_references(
    arguments: &Value,
    proj_path: &Path,
    query: &str,
    rel_path: &str,
) -> String {
    if query.is_empty() && rel_path.is_empty() {
        return "Error: query symbol is required for references operation. Example: project_context(operation=\"references\", project_path=\"...\", query=\"MyStruct\")".to_string();
    }

    // Fast Path 1: Language Server Protocol (LSP) if line or relative_path is provided
    if let Some(lsp_resp) = super::context_lsp::try_lsp_references(arguments, proj_path, query, rel_path) {
        return lsp_resp;
    }

    let mut refs = Vec::new();
    let search_term = if query.is_empty() { rel_path } else { query };

    // Fast Path 2: Query SQLite CodeGraphDb FTS5 index (< 5ms)
    if let Ok(db) = CodeGraphDb::open_for_project(proj_path) {
        if let Ok(hits) = db.search_content_fts(search_term, 30) {
            for (path, start_line, _end_line, snippet) in hits {
                let bounded_snip = snippet.lines().next().unwrap_or(&snippet).trim();
                let bounded = if bounded_snip.len() > 100 { &bounded_snip[..100] } else { bounded_snip };
                refs.push(format!("{}:L{} -> {}", path, start_line, bounded));
                if refs.len() >= 30 {
                    break;
                }
            }
        }
    }

    // Fallback: Shallow filesystem scan (depth 4) if DB is empty/uninitialized
    if refs.is_empty() {
        let files = scan_project(proj_path, 4);
        let query_lower = search_term.to_lowercase();
        for file in files.iter().filter(|f| f.file_type == "file") {
            if let Ok(path) = validate_path(proj_path, &file.path) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if line.to_lowercase().contains(&query_lower) {
                            let trimmed = line.trim();
                            let bounded_line = if trimmed.len() > 100 { &trimmed[..100] } else { trimmed };
                            refs.push(format!("{}:L{} -> {}", file.path, line_num + 1, bounded_line));
                            if refs.len() >= 30 {
                                break;
                            }
                        }
                    }
                }
            }
            if refs.len() >= 30 {
                break;
            }
        }
    }

    format!(
        "# Symbol References for '{}' (Max 30):\n\n{}",
        search_term,
        if refs.is_empty() {
            "No references found.".to_string()
        } else {
            refs.join("\n")
        }
    )
}

pub(crate) fn handle_learn_alias(
    arguments: &Value,
    proj_path: &Path,
    rel_path: &str,
) -> String {
    let alias_term = arguments.get("alias_term").and_then(|a| a.as_str()).unwrap_or("");
    let resolved_symbol = arguments.get("resolved_symbol").and_then(|s| s.as_str());
    let resolved_line = arguments.get("resolved_line").and_then(|l| l.as_u64()).map(|l| l as usize);

    if alias_term.is_empty() || rel_path.is_empty() {
        "Error: alias_term and relative_path are required for learn_alias. Example: project_context(operation=\"learn_alias\", project_path=\"...\", alias_term=\"login\", relative_path=\"src/auth/service.rs\")".to_string()
    } else {
        match CodeGraphDb::open_for_project(proj_path) {
            Ok(db) => match db.upsert_alias(alias_term, rel_path, resolved_symbol, resolved_line) {
                Ok(()) => {
                    format!(
                        "# Alias Learned Successfully ✓\n\n- Alias Term: `{}`\n- Resolved Path: `{}`\n- Symbol: `{}`\n- Line: {}\n- Persistence: Saved in `.agent-context/code_graph.db`",
                        alias_term,
                        rel_path,
                        resolved_symbol.unwrap_or("—"),
                        resolved_line.map(|l| l.to_string()).unwrap_or_else(|| "—".to_string())
                    )
                }
                Err(e) => format!("Failed to record alias: {}", e),
            },
            Err(e) => format!("Failed to open code graph database: {}", e),
        }
    }
}
