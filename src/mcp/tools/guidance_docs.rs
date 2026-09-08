use serde_json::Value;

use crate::catalog::language_detector::detect_language_fast;
use crate::catalog::slicing::slice_skill_markdown;
use crate::catalog::store::{get_embedded_skill, load_all_skills};
use crate::mcp::state::ServerState;
use crate::ml::embeddings::hybrid_vector_search;
use crate::ml::llm_selector::LLMSelector;
use crate::optimizer::compressor::compress_markdown;
use super::helpers::{detect_project_path, validate_path};

pub(crate) fn handle_get(
    arguments: &Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    let id = arguments
        .get("identifier")
        .and_then(|i| i.as_str())
        .unwrap_or("");
    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);
    if !id.is_empty() {
        crate::mcp::db::log_skill_load(id);
    }
    if let Some(content) = get_embedded_skill(id) {
        Ok(compress_markdown(&content))
    } else if let Ok(full_path) = validate_path(&proj_path, id) {
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            Ok(compress_markdown(&content))
        } else {
            Ok(format!("Skill asset not found: {}", id))
        }
    } else {
        Ok(format!("Skill asset not found: {}", id))
    }
}

pub(crate) fn handle_docs(
    arguments: &Value,
    query: &str,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    let id = arguments
        .get("identifier")
        .and_then(|i| i.as_str())
        .unwrap_or("general");

    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);
    let all_skills = load_all_skills(&proj_path);
    let search_term = if !query.is_empty() { query } else { id };

    let stage1 = hybrid_vector_search(search_term, &all_skills, 5);
    let selector = LLMSelector::new();
    let profile = detect_language_fast(&proj_path, search_term);
    let reranked = selector.rerank(search_term, stage1, &profile, 3);

    if reranked.is_empty() {
        Ok(format!(
            "# Documentation Guidance for '{}' ({})\n\nNo matching documentation skills found in catalog for search term: '{}'.",
            id, query, search_term
        ))
    } else {
        let mut docs_sections = Vec::new();
        for (score, item) in reranked {
            let raw_content = get_embedded_skill(&item.relative_path)
                .or_else(|| {
                    validate_path(&proj_path, &item.relative_path)
                        .ok()
                        .and_then(|p| std::fs::read_to_string(p).ok())
                });

            if let Some(content) = raw_content {
                // Surgical slicing extracts top relevant sections, saving ~75% tokens
                let sliced = slice_skill_markdown(&content, search_term, 3);
                docs_sections.push(format!(
                    "### Doc Skill: {} (Score: {:.2})\nPath: {}\n\n{}",
                    item.name, score, item.relative_path, sliced
                ));
            }
        }
        Ok(format!(
            "# Documentation Guidance for '{}'\n\nQuery: '{}'\n\n{}",
            id,
            search_term,
            docs_sections.join("\n\n---\n\n")
        ))
    }
}

pub(crate) fn handle_workflow(arguments: &Value) -> Result<String, (i32, String)> {
    let stage = arguments
        .get("identifier")
        .and_then(|i| i.as_str())
        .unwrap_or("plan")
        .to_lowercase();

    let candidates = [
        format!("workflow-modes/references/workflow-{}.md", stage),
        format!("workflow-modes/references/{}.md", stage),
        format!("skills/{}/SKILL.md", stage),
    ];
    let content = candidates
        .iter()
        .find_map(|p| get_embedded_skill(p).map(|c| compress_markdown(&c)))
        .unwrap_or_else(|| {
            format!("# Dev Workflow Guidance: [{}]\n\nRecommended Flow: Context -> Plan -> Ask/Revise -> Build -> Test/Recheck -> Fix -> Document", stage)
        });
    Ok(content)
}

pub(crate) fn handle_ui_ux(query: &str) -> Result<String, (i32, String)> {
    let q = if query.is_empty() { "general" } else { query };
    Ok(format!(
        "# UI/UX Guidelines for '{}'\n\n- Styling: Modern CSS, Glassmorphism, Responsive Grid\n- Color Palette: Dark mode default, accessible HSL contrast ratios\n- Typography: Inter/Outfit readable hierarchy\n- Accessibility: Semantic HTML5, unique ARIA IDs",
        q
    ))
}
