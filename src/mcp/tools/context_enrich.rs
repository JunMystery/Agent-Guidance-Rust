use crate::context::db::CodeGraphDb;
use serde_json::Value;
use std::path::Path;

pub(crate) fn handle_enrich_graph(arguments: &Value, proj_path: &Path) -> String {
    let db = match CodeGraphDb::open_for_project(proj_path) {
        Ok(d) => d,
        Err(e) => return format!("Error: Failed to open CodeGraphDb: {}", e),
    };

    let agent_id = arguments
        .get("agent_id")
        .and_then(|a| a.as_str())
        .unwrap_or("agent");

    let mut edges_added = 0usize;
    let mut summaries_added = 0usize;

    // 1. Batch edges
    if let Some(arr) = arguments.get("edges").and_then(|v| v.as_array()) {
        for item in arr {
            let src = item.get("source").or_else(|| item.get("source_symbol")).and_then(|s| s.as_str());
            let tgt = item.get("target").or_else(|| item.get("target_symbol")).and_then(|s| s.as_str());
            let rel = item.get("relation").or_else(|| item.get("relation_type")).and_then(|s| s.as_str()).unwrap_or("business_flow");
            let desc = item.get("description").and_then(|s| s.as_str());
            let conf = item.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.9);

            if let (Some(s), Some(t)) = (src, tgt) {
                if db.upsert_semantic_edge(s, t, rel, desc, conf, agent_id).is_ok() {
                    edges_added += 1;
                }
            }
        }
    } else if let (Some(s), Some(t)) = (
        arguments.get("source").or_else(|| arguments.get("source_symbol")).and_then(|s| s.as_str()),
        arguments.get("target").or_else(|| arguments.get("target_symbol")).and_then(|s| s.as_str()),
    ) {
        let rel = arguments.get("relation").or_else(|| arguments.get("relation_type")).and_then(|s| s.as_str()).unwrap_or("business_flow");
        let desc = arguments.get("description").and_then(|s| s.as_str());
        let conf = arguments.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.9);
        if db.upsert_semantic_edge(s, t, rel, desc, conf, agent_id).is_ok() {
            edges_added += 1;
        }
    }

    // 2. Batch summaries
    if let Some(arr) = arguments.get("summaries").and_then(|v| v.as_array()) {
        for item in arr {
            let mod_path = item.get("module_path").and_then(|s| s.as_str());
            let title = item.get("title").and_then(|s| s.as_str());
            let summary = item.get("summary").and_then(|s| s.as_str());
            let tags = item.get("tags").and_then(|s| s.as_str());

            if let (Some(m), Some(ti), Some(su)) = (mod_path, title, summary) {
                if db.upsert_domain_summary(m, ti, su, tags, agent_id).is_ok() {
                    summaries_added += 1;
                }
            }
        }
    } else if let (Some(m), Some(ti), Some(su)) = (
        arguments.get("module_path").and_then(|s| s.as_str()),
        arguments.get("title").and_then(|s| s.as_str()),
        arguments.get("summary").and_then(|s| s.as_str()),
    ) {
        let tags = arguments.get("tags").and_then(|s| s.as_str());
        if db.upsert_domain_summary(m, ti, su, tags, agent_id).is_ok() {
            summaries_added += 1;
        }
    }

    format!(
        "# GraphRAG Semantic Enrichment ✓\n\n- Semantic Edges Added/Updated: {}\n- Domain Summaries Added/Updated: {}\n- Enriched By: `{}`\n- Persistence: Saved in `.agent-context/code_graph.db`",
        edges_added, summaries_added, agent_id
    )
}

pub(crate) fn handle_query_semantic(arguments: &Value, proj_path: &Path) -> String {
    let db = match CodeGraphDb::open_for_project(proj_path) {
        Ok(d) => d,
        Err(e) => return format!("Error: Failed to open CodeGraphDb: {}", e),
    };

    let query = arguments
        .get("query")
        .or_else(|| arguments.get("symbol"))
        .and_then(|q| q.as_str())
        .unwrap_or("");

    let mut out = format!("# Semantic Knowledge for '{}'\n\n", query);

    // 1. Domain summaries
    if let Ok(summaries) = db.search_domain_summaries_fts(query, 5) {
        if !summaries.is_empty() {
            out.push_str("### Domain Summaries:\n");
            for s in summaries {
                out.push_str(&format!("- **{}** (`{}`): {}\n", s.title, s.module_path, s.summary));
            }
            out.push('\n');
        }
    }

    // 2. Semantic edges
    if let Ok(edges) = db.query_semantic_edges_for_symbol(query) {
        if !edges.is_empty() {
            out.push_str("### Semantic Edges:\n");
            for e in edges {
                let desc = e.description.as_deref().unwrap_or("—");
                out.push_str(&format!(
                    "- `{}` --[{}]--> `{}` (conf: {:.2}): {}\n",
                    e.source_symbol, e.relation_type, e.target_symbol, e.confidence, desc
                ));
            }
            out.push('\n');
        }
    }

    if out.ends_with("\n\n") && !out.contains("###") {
        out.push_str("No semantic knowledge or domain summaries found matching query.");
    }

    out
}
