use serde_json::Value;
use std::path::Path;

use crate::mcp::state::ServerState;
use super::helpers::{embed_query, ensure_indexed};

pub(crate) fn handle_search(
    query: &str,
    proj_path: &Path,
    state: &mut ServerState,
) -> String {
                    if query.is_empty() {
                        "Error: query is required for search operation. Example: project_context(operation=\"search\", project_path=\"...\", query=\"search_term\")".to_string()
                    } else {
                        let db_opt = ensure_indexed(&proj_path);
                        let mut results = Vec::new();
                        let mut source = "fallback_scan";

                        if let Some(ref db) = db_opt {
                            // Phase 1: Alias Lookup (<1ms)
                            if let Ok(aliases) = db.lookup_aliases(query, 10) {
                                if !aliases.is_empty() {
                                    source = "alias_cache";
                                    for a in aliases {
                                        results.push(format!(
                                            "- {}:L{} → `{}` (confidence: {:.2}) [alias cache]",
                                            a.resolved_path,
                                            a.resolved_line.unwrap_or(1),
                                            a.resolved_symbol.as_deref().unwrap_or("—"),
                                            a.confidence
                                        ));
                                    }
                                }
                            }

                            // Phase 2: FTS5 Symbol Names (<5ms)
                            if results.is_empty() {
                                if let Ok(syms) = db.search_symbols(query, 15) {
                                    if !syms.is_empty() {
                                        source = "symbol_fts";
                                        for (path, name, line) in syms {
                                            results.push(format!("- {}:L{} → `{}` [symbol index]", path, line, name));
                                        }
                                    }
                                }
                            }

                            // Phase 3: Vector Symbol Signatures (<50ms)
                            if results.is_empty() {
                                if let Some(qv) = embed_query(query) {
                                    if let Ok(vec_syms) = db.vector_search_symbols(&qv, 10, 0.65) {
                                        if !vec_syms.is_empty() {
                                            source = "symbol_vector";
                                            for v in vec_syms {
                                                results.push(format!(
                                                    "- {}:L{} → `{}` (cosine: {:.2}) [semantic symbol]",
                                                    v.file_path, v.start_line, v.name, v.score
                                                ));
                                            }
                                        }
                                    }
                                }
                            }

                            // Phase 4: FTS5 Content Chunks (<5ms)
                            if results.is_empty() {
                                if let Ok(content_hits) = db.search_content_fts(query, 10) {
                                    if !content_hits.is_empty() {
                                        source = "content_fts";
                                        for (path, start, end, snip) in content_hits {
                                            results.push(format!(
                                                "- {}:L{}-{} → `{}` [content index]",
                                                path, start, end, snip.replace('\n', " ")
                                            ));
                                        }
                                    }
                                }
                            }

                            // Phase 5: RAG Vector Content Chunks (<100ms)
                            if results.is_empty() {
                                if let Some(qv) = embed_query(query) {
                                    if let Ok(chunks) = db.vector_search_chunks(&qv, 10, 0.60) {
                                        if !chunks.is_empty() {
                                            source = "content_vector";
                                            for c in chunks {
                                                results.push(format!(
                                                    "- {}:L{}-{} (cosine: {:.2}) [RAG content vector]",
                                                    c.file_path, c.start_line, c.end_line, c.score
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Auto-Learn: learn top result if resolved from non-alias source
                        if !results.is_empty() && source != "alias_cache" {
                            if let Some(ref db) = db_opt {
                                let first_line = &results[0];
                                if let Some(path_part) = first_line.strip_prefix("- ") {
                                    let rel = path_part.split(':').next().unwrap_or("").trim();
                                    if !rel.is_empty() {
                                        let _ = db.upsert_alias(query, rel, None, None);
                                    }
                                }
                            }
                        }

                        state.record_call(2000, 400);
                        format!(
                            "# Context Search Results for '{}'\n\nSource: {} | Cascade: [alias → sym_fts → sym_vec → content_fts → rag_vec]\n\n{}",
                            query,
                            source,
                            if results.is_empty() {
                                "No matches found across symbol, full-text, and RAG semantic index.".to_string()
                            } else {
                                results.join("\n")
                            }
                        )
                    }
}

pub(crate) fn handle_navigate(
    arguments: &Value,
    query: &str,
    proj_path: &Path,
    state: &mut ServerState,
) -> String {
                    if query.is_empty() {
                        "Error: query is required for navigate operation.".to_string()
                    } else {
                        let db_opt = ensure_indexed(&proj_path);
                        let scope = arguments.get("scope").and_then(|s| s.as_str()).unwrap_or("all");
                        let mut sections = Vec::new();

                        if let Some(ref db) = db_opt {
                            // 1. Alias section
                            if let Ok(aliases) = db.lookup_aliases(query, 5) {
                                if !aliases.is_empty() {
                                    let lines: Vec<String> = aliases.iter().map(|a| {
                                        format!("- {}:L{} → `{}` (confidence: {:.2})", a.resolved_path, a.resolved_line.unwrap_or(1), a.resolved_symbol.as_deref().unwrap_or("—"), a.confidence)
                                    }).collect();
                                    sections.push(format!("## From Alias Cache (Instant)\n\n{}", lines.join("\n")));
                                }
                            }

                            // 2. Symbol FTS section
                            if scope == "all" || scope == "symbols" {
                                if let Ok(syms) = db.search_symbols(query, 5) {
                                    if !syms.is_empty() {
                                        let lines: Vec<String> = syms.iter().map(|(p, n, l)| format!("- {}:L{} → `{}`", p, l, n)).collect();
                                        sections.push(format!("## From Symbol Index (FTS5)\n\n{}", lines.join("\n")));
                                    }
                                }
                            }

                            // 3. Content FTS section
                            if scope == "all" || scope == "content" {
                                if let Ok(content_hits) = db.search_content_fts(query, 5) {
                                    if !content_hits.is_empty() {
                                        let lines: Vec<String> = content_hits.iter().map(|(p, s, e, snip)| format!("- {}:L{}-{} → `{}`", p, s, e, snip.replace('\n', " "))).collect();
                                        sections.push(format!("## From Content Full-Text (FTS5)\n\n{}", lines.join("\n")));
                                    }
                                }
                            }

                            // 4. Vector Semantic section
                            if let Some(qv) = embed_query(query) {
                                if scope == "all" || scope == "symbols" {
                                    if let Ok(sym_vecs) = db.vector_search_symbols(&qv, 5, 0.65) {
                                        if !sym_vecs.is_empty() {
                                            let lines: Vec<String> = sym_vecs.iter().map(|v| format!("- {}:L{} → `{}` (cosine: {:.2})", v.file_path, v.start_line, v.name, v.score)).collect();
                                            sections.push(format!("## From Symbol Semantic Vector\n\n{}", lines.join("\n")));
                                        }
                                    }
                                }

                                if scope == "all" || scope == "content" {
                                    if let Ok(chunk_vecs) = db.vector_search_chunks(&qv, 5, 0.60) {
                                        if !chunk_vecs.is_empty() {
                                            let lines: Vec<String> = chunk_vecs.iter().map(|c| format!("- {}:L{}-{} (cosine: {:.2})", c.file_path, c.start_line, c.end_line, c.score)).collect();
                                            sections.push(format!("## From RAG Code Chunk Semantic Vector\n\n{}", lines.join("\n")));
                                        }
                                    }
                                }
                            }

                            // 5. Related Graph Edges
                            if scope == "all" || scope == "edges" {
                                if let Ok(related) = db.search_related_symbols(query) {
                                    if !related.is_empty() {
                                        let lines: Vec<String> = related.iter().map(|(s1, edge, s2)| format!("- `{}` --[{}]--> `{}`", s1, edge, s2)).collect();
                                        sections.push(format!("## Related Graph Symbols (DAG)\n\n{}", lines.join("\n")));
                                    }
                                }
                            }
                        }

                        state.record_call(3000, 500);
                        format!(
                            "# Code Graph Navigation for '{}'\n\n{}",
                            query,
                            if sections.is_empty() {
                                "No navigation nodes found.".to_string()
                            } else {
                                sections.join("\n\n")
                            }
                        )
                    }
}
