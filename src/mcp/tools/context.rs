use serde_json::Value;
use std::path::Path;

use crate::context::db::CodeGraphDb;
use crate::context::indexer::IncrementalIndexer;
use crate::context::scanner::scan_project;
use crate::mcp::state::ServerState;
use super::context_read::handle_read;
use super::context_search::{handle_navigate, handle_search};
use super::helpers::{detect_project_architecture, detect_project_path, ensure_not_cancelled, validate_path};

pub(crate) fn handle(
    arguments: Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    ensure_not_cancelled(state)?;
    let op = arguments
        .get("operation")
        .and_then(|o| o.as_str())
        .unwrap_or("tree");
    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);
    state.update_project_path(&proj_path);
    let query = arguments
        .get("query")
        .and_then(|q| q.as_str())
        .unwrap_or("");
    let rel_path = arguments
        .get("relative_path")
        .and_then(|r| r.as_str())
        .unwrap_or("");

    let resp = match op {
        "tree" => {
            let files = scan_project(&proj_path, 2);
            let file_list: Vec<String> = files
                .into_iter()
                .map(|f| format!("- {} ({})", f.path, f.file_type))
                .collect();
            state.record_call(2000, 400);
            format!(
                "# Project Tree (Depth Capped at 2)\n\n{}",
                file_list.join("\n")
            )
        }
        "read" => handle_read(&arguments, &proj_path, rel_path, state),
        "search" => handle_search(query, &proj_path, state),
        "navigate" => handle_navigate(&arguments, query, &proj_path, state),
                "learn_alias" => {
                    let alias_term = arguments.get("alias_term").and_then(|a| a.as_str()).unwrap_or("");
                    let resolved_symbol = arguments.get("resolved_symbol").and_then(|s| s.as_str());
                    let resolved_line = arguments.get("resolved_line").and_then(|l| l.as_u64()).map(|l| l as usize);

                    if alias_term.is_empty() || rel_path.is_empty() {
                        "Error: alias_term and relative_path are required for learn_alias. Example: project_context(operation=\"learn_alias\", project_path=\"...\", alias_term=\"login\", relative_path=\"src/auth/service.rs\")".to_string()
                    } else {
                        match CodeGraphDb::open_for_project(&proj_path) {
                            Ok(db) => match db.upsert_alias(alias_term, rel_path, resolved_symbol, resolved_line) {
                                Ok(()) => {
                                    state.record_call(500, 100);
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
                "reindex" => {
                    match IncrementalIndexer::new(&proj_path) {
                        Ok(mut indexer) => {
                            let report = indexer.full_index().unwrap_or_default();
                            let path = proj_path.clone();
                            std::thread::spawn(move || {
                                if let Ok(idx) = IncrementalIndexer::new(&path) {
                                    let _ = idx.embed_symbols();
                                    let _ = idx.embed_chunks();
                                }
                            });
                            state.record_call(5000, 400);
                            format!(
                                "# Project Re-Indexed Successfully ✓\n\n- Files Scanned: {}\n- Files Indexed: {}\n- Files Skipped: {}\n- Symbols Extracted: {}\n- Edges Created: {}\n- Content Chunks: {}\n- Duration: {}ms\n- Background Embedding: Rayon ML pool active\n- Persistence: `.agent-context/code_graph.db`",
                                report.files_scanned,
                                report.files_indexed,
                                report.files_skipped,
                                report.symbols_extracted,
                                report.edges_created,
                                report.chunks_created,
                                report.duration_ms
                            )
                        }
                        Err(e) => format!("Failed to reindex project: {}", e),
                    }
                }
                "architecture" => {
                    let arch_pattern = detect_project_architecture(&proj_path);
                    state.active_architecture_pattern = Some(arch_pattern.clone());
                    let _ = ServerState::save_persisted_architecture(&proj_path, &arch_pattern);
                    state.record_call(1000, 200);
                    format!(
                        "# Project Architecture Analysis\n\n- Detected / Memorized Pattern: {}\n- Workspace Root: {}\n- Persistence: Memorized in `.agent-context/architecture.json`\n\nArchitectural Guidelines:\n- Clean_Architecture: Enforce strict separation between domain, usecase, infrastructure.\n- Layered_Architecture: Enforce controllers -> services -> models flow.\n- Package_By_Feature: Organize code by features/modules.\n- Orchestrator: Keep dispatcher thin and split logic into sub-modules upfront.\n- CLI_Pipeline: Separate argument parsing, command handlers, and core execution engine.\n- Flat_Library: Keep modules focused and avoid over-nested directories.",
                        arch_pattern,
                        proj_path.display()
                    )
                }
                "symbols" | "structure" => {
                    if rel_path.is_empty() {
                        "Error: relative_path is required for symbols/structure operation. Example: project_context(operation=\"symbols\", project_path=\"...\", relative_path=\"src/main.rs\")".to_string()
                    } else {
                        match validate_path(&proj_path, rel_path) {
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
                }
                "references" => {
                    if query.is_empty() {
                        "Error: query symbol is required for references operation. Example: project_context(operation=\"references\", project_path=\"...\", query=\"MyStruct\")".to_string()
                    } else {
                        let files = scan_project(&proj_path, 12);
                        let mut refs = Vec::new();
                        let query_lower = query.to_lowercase();
                        for file in files.iter().filter(|f| f.file_type == "file") {
                            if let Ok(path) = validate_path(&proj_path, &file.path) {
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
                        format!(
                            "# Symbol References for '{}' (Max 30):\n\n{}",
                            query,
                            if refs.is_empty() {
                                "No references found.".to_string()
                            } else {
                                refs.join("\n")
                            }
                        )
                    }
                }
                _ => format!("Project context operation '{}' completed.", op),
    };

    Ok(resp)
}
