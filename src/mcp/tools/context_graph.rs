use serde_json::Value;
use std::path::Path;

use crate::context::graph_rag::{CommunityLevel, GraphRagEngine, GraphRagQueryMode};
use crate::mcp::state::ServerState;
use super::helpers::detect_project_architecture;

pub(crate) fn handle_architecture(proj_path: &Path, state: &mut ServerState) -> String {
    let arch_pattern = detect_project_architecture(proj_path);
    state.active_architecture_pattern = Some(arch_pattern.clone());
    let _ = ServerState::save_persisted_architecture(proj_path, &arch_pattern);

    let engine = GraphRagEngine::new(proj_path);
    let hierarchy = engine.load_or_build(&arch_pattern);

    let subsystems = if hierarchy.communities.is_empty() {
        "No communities indexed yet. Run `project_context(operation=\"reindex\")` to build hierarchy.".to_string()
    } else {
        hierarchy
            .get_by_level(CommunityLevel::MacroSubsystem)
            .iter()
            .map(|c| {
                format!(
                    "- **{}** (Layer: {}): {}",
                    c.summary.title, c.summary.layer, c.summary.description
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mermaid_dag = engine.architecture_mermaid(&arch_pattern);

    format!(
        "# Project Architecture Analysis (GraphRAG)\n\n- Detected / Memorized Pattern: **{}**\n- Workspace Root: {}\n- Total Hierarchical Communities: {}\n- Persistence: Memorized in `.agent-context/architecture.json`\n\n### Core Community Subsystems:\n{}\n\n### Architecture Dependency DAG:\n```mermaid\n{}\n```",
        arch_pattern,
        proj_path.display(),
        hierarchy.communities.len(),
        subsystems,
        mermaid_dag
    )
}

pub(crate) fn handle_graph_rag(
    arguments: &Value,
    query: &str,
    proj_path: &Path,
) -> String {
    let mode_str = arguments
        .get("mode")
        .and_then(|m| m.as_str())
        .unwrap_or("drift");
    let mode = GraphRagQueryMode::from_str(mode_str);
    let arch_pattern = detect_project_architecture(proj_path);
    let engine = GraphRagEngine::new(proj_path);
    let blast_radius = if let Ok(dag) = engine.symbol_blast_radius_mermaid(query.trim()) {
        if dag.lines().count() > 2 {
            format!("\n\n### Dependency Flow & Blast Radius (Mermaid DAG):\n```mermaid\n{}\n```", dag)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    match engine.query(query, mode, &arch_pattern) {
        Ok(res) => {
            format!(
                "# {}\n\nMode: {:?}\n\n{}{}",
                res.title,
                res.mode,
                res.sections.join("\n\n"),
                blast_radius
            )
        }
        Err(e) => format!("GraphRAG query error: {}", e),
    }
}

pub(crate) fn handle_reusable(proj_path: &Path) -> String {
    let engine = GraphRagEngine::new(proj_path);
    match engine.analyze_reusability() {
        Ok(report) => report,
        Err(e) => format!("Reusability analysis error: {}", e),
    }
}
