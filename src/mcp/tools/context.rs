use serde_json::Value;

use crate::context::indexer::IncrementalIndexer;
use crate::context::scanner::scan_project;
use crate::mcp::state::ServerState;
use super::context_graph::{handle_architecture, handle_graph_rag, handle_reusable};
use super::context_lsp::{handle_definition, handle_type_definition};
use super::context_read::handle_read;
use super::context_search::{handle_navigate, handle_search};
use super::context_symbols::{handle_learn_alias, handle_references, handle_symbols};
use super::helpers::{detect_project_path, ensure_not_cancelled};

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
            format!(
                "# Project Tree (Depth Capped at 2)\n\n{}",
                file_list.join("\n")
            )
        }
        "read" => handle_read(&arguments, &proj_path, rel_path, state),
        "search" => handle_search(query, &proj_path, state),
        "navigate" => handle_navigate(&arguments, query, &proj_path, state),
        "learn_alias" => handle_learn_alias(&arguments, &proj_path, rel_path),
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
                    format!(
                        "# Project Re-Indexed Successfully\n\n- Files Scanned: {}\n- Files Indexed: {}\n- Files Skipped: {}\n- Symbols Extracted: {}\n- Edges Created: {}\n- Content Chunks: {}\n- Duration: {}ms\n- Background Embedding: Rayon ML pool active\n- Persistence: `.agent-context/code_graph.db`",
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
        "architecture" => handle_architecture(&proj_path, state),
        "graph_rag" => handle_graph_rag(&arguments, query, &proj_path),
        "reusable" | "shared" | "reusable_candidates" | "detect_duplicates" => {
            handle_reusable(&proj_path)
        }
        "symbols" | "structure" => handle_symbols(&proj_path, rel_path),
        "references" => handle_references(&arguments, &proj_path, query, rel_path),
        "definition" | "goto_definition" => handle_definition(&arguments, &proj_path, query, rel_path),
        "type_definition" => handle_type_definition(&arguments, &proj_path, query, rel_path),
        _ => format!("Project context operation '{}' completed.", op),
    };

    Ok(resp)
}
