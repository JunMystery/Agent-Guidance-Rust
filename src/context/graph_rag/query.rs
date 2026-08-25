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

/// Local Search: targeted entity search and fan-out across 1-hop & 2-hop DAG neighbors.
pub fn execute_local_search(
    query: &str,
    db: &CodeGraphDb,
    hierarchy: &CommunityHierarchy,
) -> Result<QueryResult> {
    let mut sections = Vec::new();

    // 1. Find matching target symbols
    let symbols = db.search_symbols(query, 5)?;
    if symbols.is_empty() {
        sections.push(format!("No matching entity symbols found for query '{}'.", query));
        return Ok(QueryResult {
            mode: GraphRagQueryMode::Local,
            title: format!("GraphRAG Local Search for '{}'", query),
            sections,
        });
    }

    for (file_path, sym_name, line) in &symbols {
        let mut entity_block = format!("### Entity: `{}` (`{}:L{}`)\n", sym_name, file_path, line);

        // Check community membership
        if let Some(comm) = hierarchy.find_community_for_file(file_path) {
            entity_block.push_str(&format!("- Community: **{}** (Layer: {})\n", comm.summary.title, comm.summary.layer));
        }

        // 2. Query 1-hop & 2-hop DAG relations
        if let Ok(related) = db.search_related_symbols(sym_name) {
            if !related.is_empty() {
                entity_block.push_str("#### Graph Relations (1-Hop Fan-out):\n");
                for (s1, edge, s2) in related.iter().take(8) {
                    entity_block.push_str(&format!("- `{}` --[{}]--> `{}`\n", s1, edge, s2));
                }
            }
        }

        sections.push(entity_block);
    }

    Ok(QueryResult {
        mode: GraphRagQueryMode::Local,
        title: format!("GraphRAG Local Search for '{}'", query),
        sections,
    })
}

/// DRIFT Search: Dual-route combining Community Hierarchy context + exact AST traversal.
pub fn execute_drift_search(
    query: &str,
    db: &CodeGraphDb,
    hierarchy: &CommunityHierarchy,
) -> Result<QueryResult> {
    let mut sections = Vec::new();

    // Route 1: Top-down Community Context
    let global_res = execute_global_search(query, hierarchy);
    if !global_res.sections.is_empty() {
        sections.push("## 🌐 Route 1: High-Level Community Context (Top-Down)".to_string());
        for sec in global_res.sections.into_iter().take(3) {
            sections.push(sec);
        }
    }

    // Route 2: Bottom-up Factual Entity Traversal
    let local_res = execute_local_search(query, db, hierarchy)?;
    if !local_res.sections.is_empty() {
        sections.push("## 🎯 Route 2: Entity & Dependency Fan-out (Bottom-Up)".to_string());
        for sec in local_res.sections.into_iter().take(3) {
            sections.push(sec);
        }
    }

    Ok(QueryResult {
        mode: GraphRagQueryMode::Drift,
        title: format!("GraphRAG DRIFT Search (Dual-Route) for '{}'", query),
        sections,
    })
}
