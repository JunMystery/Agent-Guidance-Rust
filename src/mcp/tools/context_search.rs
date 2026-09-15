use serde_json::Value;
use std::path::Path;

use crate::mcp::state::ServerState;
use super::helpers::{embed_query, ensure_indexed};

pub(crate) fn handle_search(
    query: &str,
    proj_path: &Path,
    intent: Option<&str>,
    _state: &mut ServerState,
) -> String {
    if query.is_empty() {
        return "Error: query is required for search operation. Example: project_context(operation=\"search\", project_path=\"...\", query=\"search_term\")".to_string();
    }

    let db_opt = ensure_indexed(proj_path);
    let mut results = Vec::new();
    let mut source = "fallback_scan";
    let mut detected_intent = crate::context::search::SearchIntent::Universal;

    if let Some(ref db) = db_opt {
        let exec = crate::context::search::execute_ranked_search(
            db,
            query,
            intent,
            10,
            |q| embed_query(q),
        );
        source = exec.source;
        detected_intent = exec.intent;

        for r in exec.results {
            let label = match r.source {
                "alias_cache" => format!("(confidence: {:.2}) [alias cache]", (r.score / 2.0).clamp(0.0, 1.0)),
                "symbol_fts" => format!("(score: {:.2}) [symbol index: bm25]", r.score),
                "symbol_vector" => format!("(cosine: {:.2}) [semantic symbol]", r.score),
                "content_fts" => format!("(score: {:.2}) [content index: bm25]", r.score),
                "content_vector" => format!("(cosine: {:.2}) [RAG content vector]", r.score),
                other => format!("({}) [{}]", r.score, other),
            };

            let line_fmt = if let Some(end) = r.end_line {
                format!("{}:L{}-{}", r.path, r.line, end)
            } else {
                format!("{}:L{}", r.path, r.line)
            };

            if let Some(snip) = r.snippet {
                results.push(format!("- {} → `{}` {}", line_fmt, snip.replace('\n', " "), label));
            } else {
                results.push(format!("- {} → `{}` {}", line_fmt, r.name, label));
            }
        }
    }

    // Phase 6: Multi-Project Cascade (<20ms)
    if results.is_empty() {
        let linked = crate::context::multi_project::discover_linked_projects(proj_path);
        if !linked.is_empty() {
            let syms = crate::context::multi_project::search_linked_symbols(&linked, query, 5);
            if !syms.is_empty() {
                source = "linked_symbol";
                for (proj_name, path, name, line) in syms {
                    results.push(format!(
                        "- linked:{}/{}:L{} → `{}` [linked symbol: {}]",
                        proj_name, path, line, name, proj_name
                    ));
                }
            } else {
                let content_hits = crate::context::multi_project::search_linked_content(&linked, query, 5);
                if !content_hits.is_empty() {
                    source = "linked_content";
                    for (proj_name, path, start, end, snip) in content_hits {
                        results.push(format!(
                            "- linked:{}/{}:L{}-{} → `{}` [linked content: {}]",
                            proj_name, path, start, end, snip.replace('\n', " "), proj_name
                        ));
                    }
                }
            }
        }
    }

    // Auto-Learn: learn top result if resolved from non-alias source
    if !results.is_empty() && source != "alias_cache" && !source.starts_with("linked_") {
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

    let intent_tag = match detected_intent {
        crate::context::search::SearchIntent::Logic => "Logic",
        crate::context::search::SearchIntent::Guidance => "Guidance",
        crate::context::search::SearchIntent::Universal => "Universal",
    };

    format!(
        "# Context Search Results for '{}' [Intent: {}]\n\nSource: {} | Cascade: [alias → sym_fts(bm25) → sym_vec → content_fts(bm25) → rag_vec → linked_projects]\n\n{}",
        query,
        intent_tag,
        source,
        if results.is_empty() {
            "No matches found across symbol, full-text, and RAG semantic index.".to_string()
        } else {
            results.join("\n")
        }
    )
}

pub(crate) fn handle_navigate(
    arguments: &Value,
    query: &str,
    proj_path: &Path,
    _state: &mut ServerState,
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

                        // 6. Linked Projects section
                        let linked = crate::context::multi_project::discover_linked_projects(proj_path);
                        if !linked.is_empty() && (scope == "all" || scope == "symbols" || scope == "linked") {
                            let syms = crate::context::multi_project::search_linked_symbols(&linked, query, 5);
                            if !syms.is_empty() {
                                let lines: Vec<String> = syms
                                    .into_iter()
                                    .map(|(proj, p, n, l)| format!("- linked:{}/{}:L{} → `{}`", proj, p, l, n))
                                    .collect();
                                sections.push(format!("## From Linked Projects (Cross-Workspace)\n\n{}", lines.join("\n")));
                            }
                        }

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
