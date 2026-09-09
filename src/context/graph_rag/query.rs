use anyhow::Result;
use std::path::Path;
use super::community::{CommunityHierarchy, CommunityLevel};
use crate::context::db::CodeGraphDb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphRagQueryMode {
    Global,
    Local,
    Drift,
    Basic,
}

impl GraphRagQueryMode {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "global" => Self::Global,
            "local" => Self::Local,
            "drift" => Self::Drift,
            _ => Self::Basic,
        }
    }
}

pub struct QueryResult {
    pub mode: GraphRagQueryMode,
    pub title: String,
    pub sections: Vec<String>,
}

/// Global Search: reasoning across Level 0 & Level 1 Community Summaries.
pub fn execute_global_search(query: &str, hierarchy: &CommunityHierarchy) -> QueryResult {
    let mut sections = Vec::new();
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();

    sections.push(format!(
        "### Project Architecture Overview [{}]\n- Overall Architecture Pattern: **{}**\n- Total Communities Indexed: {}\n",
        hierarchy.project_name,
        hierarchy.detected_architecture,
        hierarchy.communities.len()
    ));

    // Match Level 0 (Macro Subsystems) and Level 1 (Feature Modules)
    let mut scored_communities: Vec<(usize, &super::community::Community)> = Vec::new();
    for comm in &hierarchy.communities {
        if comm.level == CommunityLevel::MicroCluster {
            continue;
        }
        let text = format!("{} {} {}", comm.summary.title, comm.summary.layer, comm.summary.description).to_lowercase();
        let mut score = 0;
        for word in &query_words {
            if text.contains(word) {
                score += 3;
            }
        }
        if comm.level == CommunityLevel::MacroSubsystem {
            score += 2;
        }
        if score > 0 || query.is_empty() || query_words.is_empty() {
            scored_communities.push((score, comm));
        }
    }

    scored_communities.sort_by(|a, b| b.0.cmp(&a.0));

    for (_, comm) in scored_communities.iter().take(6) {
        let mut block = format!(
            "#### [{:?}] {} (Layer: {})\n{}\n",
            comm.level, comm.summary.title, comm.summary.layer, comm.summary.description
        );
        if !comm.summary.key_entities.is_empty() {
            block.push_str(&format!("- Key Symbols: {}\n", comm.summary.key_entities.join(", ")));
        }
        if !comm.summary.export_interfaces.is_empty() {
            block.push_str(&format!("- Exported Contracts: {}\n", comm.summary.export_interfaces.join(", ")));
        }
        if !comm.summary.dependencies.is_empty() {
            block.push_str(&format!("- Downstream Dependencies: {}\n", comm.summary.dependencies.join(", ")));
        }
        sections.push(block);
    }

    QueryResult {
        mode: GraphRagQueryMode::Global,
        title: format!("GraphRAG Global Search for '{}'", query),
        sections,
    }
}

/// Local Search: targeted entity search and multi-hop neighborhood traversal.
pub fn execute_local_search(
    query: &str,
    db: &CodeGraphDb,
    hierarchy: &CommunityHierarchy,
) -> Result<QueryResult> {
    let mut sections = Vec::new();

    // 0. AI Agent Domain Summaries & Semantic Knowledge
    if let Ok(summaries) = db.search_domain_summaries_fts(query, 3) {
        if !summaries.is_empty() {
            let mut summary_block = String::from("### AI Agent Domain Summaries\n");
            for s in summaries {
                summary_block.push_str(&format!("- **{}** (`{}`): {}\n", s.title, s.module_path, s.summary));
            }
            sections.push(summary_block);
        }
    }

    // 1. Direct Multi-Hop Neighborhood Fetch
    if let Ok(Some(n)) = super::neighborhood::fetch_symbol_neighborhood(db, hierarchy, query) {
        let mut block = format!(
            "### Target Entity: `{}` (`{}`)\n- File: `{}:L{}-L{}`\n",
            n.name, n.kind, n.file_path, n.start_line, n.end_line
        );
        if let (Some(title), Some(layer)) = (&n.community_title, &n.community_layer) {
            block.push_str(&format!("- Community Subsystem: **{}** (Layer: {})\n", title, layer));
        }
        if let Some(sig) = &n.signature {
            block.push_str(&format!("- Signature: `{}`\n", sig));
        }

        if let Some(code) = &n.code_excerpt {
            let bounded: Vec<&str> = code.lines().take(20).collect();
            block.push_str(&format!("\n#### Code Implementation Excerpt:\n```text\n{}\n```\n", bounded.join("\n")));
        }

        if !n.callers.is_empty() {
            block.push_str(&format!("\n#### 1-Hop Callers ({})\n", n.callers.len()));
            for c in n.callers.iter().take(8) {
                block.push_str(&format!("- `{}` in `{}:L{}` [{}, weight: {:.1}]\n", c.name, c.file_path, c.start_line, c.edge_type, c.weight));
            }
        }

        if !n.callees.is_empty() {
            block.push_str(&format!("\n#### 1-Hop Dependencies ({})\n", n.callees.len()));
            for c in n.callees.iter().take(8) {
                block.push_str(&format!("- `{}` in `{}:L{}` [{}, weight: {:.1}]\n", c.name, c.file_path, c.start_line, c.edge_type, c.weight));
            }
        }

        if !n.transitive_chains.is_empty() {
            block.push_str("\n#### 2-Hop Transitive Dependency Chains:\n");
            for (s1, s2, s3) in n.transitive_chains.iter().take(4) {
                block.push_str(&format!("- `{}` -> `{}` -> `{}`\n", s1, s2, s3));
            }
        }

        block.push_str(&format!("\n#### Subgraph Neighborhood DAG:\n```mermaid\n{}\n```", n.mermaid_dag));
        sections.push(block);
    } else {
        // Fallback: search symbols matching query
        let symbols = db.search_symbols(query, 3).unwrap_or_default();
        if symbols.is_empty() {
            sections.push(format!("No matching entity symbols or code chunks found for query '{}'.", query));
        } else {
            for (file_path, sym_name, line) in symbols {
                if let Ok(Some(sub_n)) = super::neighborhood::fetch_symbol_neighborhood(db, hierarchy, &sym_name) {
                    sections.push(format!(
                        "### Entity: `{}` (`{}:L{}`)\n- Callers: {} | Callees: {}\n- Community: {}\n",
                        sub_n.name, file_path, line, sub_n.callers.len(), sub_n.callees.len(),
                        sub_n.community_title.as_deref().unwrap_or("Core")
                    ));
                }
            }
        }
    }

    if let Ok(edges) = db.query_semantic_edges_for_symbol(query) {
        if !edges.is_empty() {
            let mut edge_block = String::from("#### Semantic Edges (AI Inferred)\n");
            for e in edges.iter().take(6) {
                let desc = e.description.as_deref().unwrap_or("—");
                edge_block.push_str(&format!("- `{}` --[{}]--> `{}`: {}\n", e.source_symbol, e.relation_type, e.target_symbol, desc));
            }
            sections.push(edge_block);
        }
    }

    Ok(QueryResult {
        mode: GraphRagQueryMode::Local,
        title: format!("GraphRAG Local Multi-Hop Search for '{}'", query),
        sections,
    })
}

/// DRIFT Search: Dual-route combining Community Hierarchy context + exact AST multi-hop traversal.
pub fn execute_drift_search(
    query: &str,
    db: &CodeGraphDb,
    hierarchy: &CommunityHierarchy,
) -> Result<QueryResult> {
    let mut sections = Vec::new();

    // Route 1: Top-down Community Context
    let global_res = execute_global_search(query, hierarchy);
    if !global_res.sections.is_empty() {
        sections.push("## Route 1: High-Level Community Architecture (Top-Down)".to_string());
        for sec in global_res.sections.into_iter().take(3) {
            sections.push(sec);
        }
    }

    // Route 2: Bottom-up Multi-hop Entity Neighborhood Traversal
    let local_res = execute_local_search(query, db, hierarchy)?;
    if !local_res.sections.is_empty() {
        sections.push("## Route 2: AST Multi-Hop Neighborhood & Call Graph (Bottom-Up)".to_string());
        for sec in local_res.sections.into_iter().take(3) {
            sections.push(sec);
        }
    }

    Ok(QueryResult {
        mode: GraphRagQueryMode::Drift,
        title: format!("GraphRAG DRIFT Search (Dual-Route Multi-Hop) for '{}'", query),
        sections,
    })
}
