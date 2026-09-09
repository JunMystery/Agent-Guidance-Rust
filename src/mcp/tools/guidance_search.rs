use serde_json::Value;

use crate::catalog::language_detector::detect_language_fast;
use crate::catalog::store::{SkillSource, load_all_skills};
use crate::mcp::state::ServerState;
use crate::ml::embeddings::hybrid_vector_search;
use crate::ml::llm_selector::LLMSelector;
use super::helpers::{detect_project_path, ensure_not_cancelled};

pub(crate) fn handle_list(arguments: &Value, state: &mut ServerState) -> Result<String, (i32, String)> {
    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);
    let skills = load_all_skills(&proj_path);
    let names: Vec<String> = skills
        .iter()
        .map(|s| format!("- {} ({})", s.name, s.relative_path))
        .collect();
    Ok(format!(
        "# Registered Skills Catalog ({})\n\n{}",
        names.len(),
        names.join("\n")
    ))
}

pub(crate) fn handle_search(
    arguments: &Value,
    query: &str,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);

    ensure_not_cancelled(state)?;
    let profile = detect_language_fast(&proj_path, query);
    let all_skills = load_all_skills(&proj_path);

    // Stage 1: 1st Stage Candidate Selection
    let stage1_results = hybrid_vector_search(query, &all_skills, 20);
    ensure_not_cancelled(state)?;

    // Stage 2: 2nd Stage Context & Intent Re-ranking
    let selector = LLMSelector::new();
    let final_results = selector.rerank(query, stage1_results, &profile, 20);
    ensure_not_cancelled(state)?;

    let mut seen_names = std::collections::HashSet::new();
    let mut deduped_results = Vec::new();
    for (score, item) in final_results {
        if seen_names.insert(item.name.clone()) {
            deduped_results.push((score, item));
            if deduped_results.len() >= 6 {
                break;
            }
        }
    }

    state.pending_skill_proposals = deduped_results
        .iter()
        .map(|(score, item)| {
            (item.name.clone(), item.relative_path.clone(), *score)
        })
        .collect();

    let formatted_results: Vec<String> = deduped_results
        .into_iter()
        .map(|(score, item)| {
            let source_tag = match &item.source {
                SkillSource::Embedded => "[Embedded]".to_string(),
                SkillSource::LocalWorkspace(path) => {
                    format!("[Local Workspace: {}]", path)
                }
            };
            format!(
                "- {} {} (Score: {:.2})\n  Path: {}",
                item.name, source_tag, score, item.relative_path
            )
        })
        .collect();

    let next_step_prompt = if formatted_results.is_empty() {
        "-> No matching skills found."
    } else {
        "-> SKILL_PROPOSAL: MANDATORY USER INTERACTION REQUIRED. Do NOT call `select_skills` automatically. You MUST trigger the IDE/CLI `ask_question` tool with the proposed skills so the user chooses which to activate, then call `select_skills(skills=[...])` with their choices (or `select_skills(skills=[])` if skipped)."
    };

    Ok(format!(
        "# 2-Stage Skill Search Results for '{}'\n\nStage 1 (Candle BERT Vector Cosine Similarity) -> Stage 2 (Cross-Encoder Re-ranking)\nMatches Found: {}\n\nRecommended Skills:\n{}\n\n{}",
        query,
        formatted_results.len(),
        if formatted_results.is_empty() {
            "No matching skills found.".to_string()
        } else {
            formatted_results.join("\n")
        },
        next_step_prompt
    ))
}

pub(crate) fn handle_reindex(
    arguments: &Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);

    // Invalidate in-memory caches
    crate::ml::embeddings::cache::clear_passage_cache();
    crate::context::cache::invalidate_snapshot(&proj_path);

    let all_skills = load_all_skills(&proj_path);
    let count = all_skills.len();
    let fp = crate::ml::embeddings::catalog_fingerprint(&all_skills);

    // Warmup / embed if model available
    let model_status = if let Some(vecs) = crate::ml::embeddings::precomputed::load_precomputed_cache(&all_skills)
        .or_else(|| crate::ml::embeddings::load_passage_cache(&all_skills))
    {
        format!(
            "Loaded cached embeddings for {} skills (dimension: {})",
            vecs.len(),
            vecs.first().map(|v| v.len()).unwrap_or(0)
        )
    } else if all_skills.len() <= 64 {
        if let Some(model) = crate::ml::embeddings::cache::try_cached_model() {
            let vecs = crate::ml::embeddings::embed_skills_cache(&all_skills, &model);
            format!(
                "Computed embeddings for {} skills (dimension: {})",
                vecs.len(),
                vecs.first().map(|v| v.len()).unwrap_or(0)
            )
        } else {
            "Metadata indexed (embeddings loaded on-demand)".to_string()
        }
    } else {
        "Metadata indexed (embeddings loaded on-demand)".to_string()
    };

    Ok(format!(
        "# Skill Semantic Index Refreshed\n\n- Total Skills: {}\n- Catalog Fingerprint: {:016x}\n- Status: {}\n- Cache Path: `~/.agent-guidance/vectors.bin`\n\nAll workspace and embedded skills reindexed with rich semantic passages.",
        count, fp, model_status
    ))
}
