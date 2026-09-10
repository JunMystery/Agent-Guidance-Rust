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

    let task_arg = arguments.get("task").and_then(|v| v.as_str()).unwrap_or("");
    let workflow_arg = arguments.get("workflow").and_then(|v| v.as_str()).unwrap_or("");
    let tech_stack_arg = arguments.get("tech_stack").and_then(|v| v.as_str()).unwrap_or("");
    let files_arg = arguments.get("files").or_else(|| arguments.get("related_files"));

    let mut query_parts = Vec::new();
    if !query.trim().is_empty() {
        query_parts.push(query.trim().to_string());
    }
    if !task_arg.trim().is_empty() && !query.contains(task_arg) {
        query_parts.push(task_arg.trim().to_string());
    }
    if !workflow_arg.trim().is_empty() && !query.contains(workflow_arg) {
        query_parts.push(workflow_arg.trim().to_string());
    }
    if !tech_stack_arg.trim().is_empty() && !query.contains(tech_stack_arg) {
        query_parts.push(tech_stack_arg.trim().to_string());
    }
    if let Some(files) = files_arg {
        let f_str = if let Some(arr) = files.as_array() {
            arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" ")
        } else if let Some(s) = files.as_str() {
            s.to_string()
        } else {
            String::new()
        };
        if !f_str.is_empty() {
            query_parts.push(f_str);
        }
    }
    if query_parts.is_empty() {
        if let Some(intent) = &state.user_intent_summary {
            query_parts.push(intent.clone());
        }
    }
    if query_parts.is_empty() && !state.modified_files.is_empty() {
        query_parts.push(state.modified_files.join(" "));
    }

    let search_query = if query_parts.is_empty() {
        "general programming guidance".to_string()
    } else {
        query_parts.join(" ")
    };

    let profile = detect_language_fast(&proj_path, &search_query);
    let all_skills = load_all_skills(&proj_path);

    // Stage 1: 1st Stage Candidate Selection
    let stage1_results = hybrid_vector_search(&search_query, &all_skills, 20);
    ensure_not_cancelled(state)?;

    // Stage 2: 2nd Stage Context & Intent Re-ranking
    let selector = LLMSelector::new();
    let final_results = selector.rerank(&search_query, stage1_results, &profile, 20);
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

fn extract_description(content: &str) -> Option<String> {
    let mut lines = content.lines();
    while let Some(line) = lines.next() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("description:") {
            let mut val = rest.trim();
            if val == ">" || val == "|" || val.is_empty() {
                if let Some(next) = lines.next() {
                    val = next.trim();
                }
            }
            let cleaned = val.trim_matches(|c| matches!(c, '"' | '\'' | '`')).trim();
            if !cleaned.is_empty() && cleaned != ">" && cleaned != "|" {
                let first = cleaned.split(". ").next().unwrap_or(cleaned).trim();
                let short = if first.len() > 38 {
                    format!("{}...", &first[..35].trim_end())
                } else {
                    first.to_string()
                };
                return Some(short);
            }
        }
    }
    None
}

    let options: Vec<String> = deduped_results
        .iter()
        .map(|(_score, item)| {
            let short_desc = extract_description(&item.content)
                .unwrap_or_else(|| "Skill guidance".to_string());
            if short_desc.is_empty() {
                item.name.clone()
            } else {
                format!("{} - {}", item.name, short_desc)
            }
        })
        .collect();

    let ask_question_example = serde_json::json!({
        "questions": [
            {
                "question": "Which skills would you like to inject for this task?",
                "options": options,
                "is_multi_select": true
            }
        ]
    });

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
        "-> No matching skills found.".to_string()
    } else {
        format!(
            "-> SKILL_PROPOSAL: MANDATORY USER INTERACTION REQUIRED. Do NOT call `select_skills` automatically.\nYou MUST trigger the IDE tool `ask_question` with `is_multi_select: true` so the user selects which skills to inject:\n```json\nask_question({})\n```\nAfter the user responds, call `select_skills(skills=[...], user_confirmed=true)` (or `select_skills(skills=[])` if none selected).",
            serde_json::to_string_pretty(&ask_question_example).unwrap_or_default()
        )
    };

    Ok(format!(
        "# 2-Stage Skill Search Results for '{}'\n\nStage 1 (Candle BERT Vector Cosine Similarity) -> Stage 2 (Cross-Encoder Re-ranking)\nMatches Found: {}\n\nRecommended Skills:\n{}\n\n{}",
        search_query,
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
