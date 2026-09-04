use serde_json::{Value, json};
use std::path::Path;

use crate::catalog::store::{SkillSource, get_embedded_skill, list_embedded_skills, load_all_skills};
use crate::context::cache::project_snapshot;
use crate::mcp::state::ServerState;
use crate::ml::embeddings::hybrid_vector_search;
use crate::ml::llm_selector::LLMSelector;
use crate::optimizer::compressor::compress_markdown;
use super::{detect_project_architecture, detect_project_path, ensure_not_cancelled, validate_path};

pub(crate) fn handle(
    arguments: Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    ensure_not_cancelled(state)?;
    let op = arguments
        .get("operation")
        .and_then(|o| o.as_str())
        .unwrap_or("list");
    let query = arguments
        .get("query")
        .and_then(|q| q.as_str())
        .unwrap_or("")
        .to_lowercase();
    state.record_call(1000, 300);

    let resp = match op {
        "list" => {
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            let snapshot = project_snapshot(&proj_path);
            let names: Vec<String> = snapshot
                .skills
                .iter()
                .map(|s| format!("- {} ({})", s.name, s.relative_path))
                .collect();
            format!(
                "# Registered Skills Catalog ({})\n\n{}",
                names.len(),
                names.join("\n")
            )
        }
        "get" => {
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
                compress_markdown(&content)
            } else if let Ok(content) = std::fs::read_to_string(id) {
                compress_markdown(&content)
            } else if let Ok(full_path) = validate_path(&proj_path, id) {
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    compress_markdown(&content)
                } else {
                    format!("Skill asset not found: {}", id)
                }
            } else {
                format!("Skill asset not found: {}", id)
            }
        }
        "search" => {
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            let snapshot = project_snapshot(&proj_path);
            ensure_not_cancelled(state)?;
            let profile = crate::catalog::language_detector::detect_language_profile(
                snapshot.files.as_ref(),
                &query,
            );
            let all_skills = snapshot.skills.as_ref();

            // Stage 1: 1st Stage Candidate Selection
            let stage1_results = hybrid_vector_search(&query, all_skills, 20);
            ensure_not_cancelled(state)?;

            // Stage 2: 2nd Stage Context & Intent Re-ranking
            let selector = LLMSelector::new();
            let final_results = selector.rerank(&query, stage1_results, &profile, 20);
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

            if state.pending_skill_proposals.is_empty() {
                state.pending_skill_proposals = deduped_results
                    .iter()
                    .map(|(score, item)| {
                        (item.name.clone(), item.relative_path.clone(), *score)
                    })
                    .collect();
            }

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

            format!(
                "# 2-Stage Skill Search Results for '{}'\n\nStage 1 (Candle BERT Vector Cosine Similarity) -> Stage 2 (Cross-Encoder Re-ranking)\nMatches Found: {}\n\nRecommended Skills:\n{}\n\n{}",
                query,
                formatted_results.len(),
                if formatted_results.is_empty() {
                    "No matching skills found.".to_string()
                } else {
                    formatted_results.join("\n")
                },
                next_step_prompt
            )
        }
        "ui_ux" => {
            let q = if query.is_empty() { "general" } else { &query };
            format!(
                "# UI/UX Guidelines for '{}'\n\n- Styling: Modern CSS, Glassmorphism, Dynamic Animations\n- Color Palette: Dark mode default, curated HSL gradients\n- Typography: Inter/Outfit via Google Fonts\n- Accessibility: Semantic HTML5, unique IDs",
                q
            )
        }
        "docs" => {
            let id = arguments
                .get("identifier")
                .and_then(|i| i.as_str())
                .unwrap_or("general");

            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            let snapshot = project_snapshot(&proj_path);
            let search_term = if !query.is_empty() { &query } else { id };

            let stage1 = hybrid_vector_search(search_term, snapshot.skills.as_ref(), 5);
            let selector = LLMSelector::new();
            let profile = crate::catalog::language_detector::detect_language_profile(
                snapshot.files.as_ref(),
                search_term,
            );
            let reranked = selector.rerank(search_term, stage1, &profile, 3);

            if reranked.is_empty() {
                format!(
                    "# Documentation Guidance for '{}' ({})\n\nNo matching documentation skills found in catalog for search term: '{}'.",
                    id, query, search_term
                )
            } else {
                let mut docs_sections = Vec::new();
                for (score, item) in reranked {
                    if let Some(content) = get_embedded_skill(&item.relative_path) {
                        docs_sections.push(format!(
                            "### Doc Skill: {} (Score: {:.2})\nPath: {}\n\n{}",
                            item.name,
                            score,
                            item.relative_path,
                            compress_markdown(&content)
                        ));
                    }
                }
                format!(
                    "# Documentation Guidance for '{}'\n\nQuery: '{}'\n\n{}",
                    id,
                    search_term,
                    docs_sections.join("\n\n---\n\n")
                )
            }
        }
        "workflow" => {
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
            candidates
                .iter()
                .find_map(|p| get_embedded_skill(p).map(|c| compress_markdown(&c)))
                .unwrap_or_else(|| {
                    format!("# Dev Workflow Guidance: [{}]\n\nRecommended Flow: Context -> Plan -> Ask/Revise -> Build -> Test/Recheck -> Fix -> Document", stage)
                })
        }
        "precode" => super::guidance_precode::handle_precode(&arguments, &query, state),
        "verify" => {
            let v_cmd = arguments
                .get("verification_command")
                .and_then(|v| v.as_str());
            let v_kw = arguments
                .get("expected_output_keyword")
                .and_then(|k| k.as_str());

            if let (Some(cmd), Some(kw)) = (v_cmd, v_kw) {
                state.verification_command = Some(cmd.to_string());
                state.expected_output_keyword = Some(kw.to_string());
                state.verification_passed = false;
                format!(
                    "# Empirical Verification Contract Registered\n\n- Verification Command: `{}`\n- Expected Output Keyword: `{}`\n- Verification Status: REGISTERED (Awaiting test execution output)\n\n✓ Run verification command to satisfy anti-hallucination requirement.",
                    cmd, kw
                )
            } else {
                format!(
                    "# Anti-Hallucination Post-Code Verification Checklist\n\n1. **Empirical Verification Required**: Trigger IDE/CLI `ask_question` tool to let user select verification test command (or confirm manual testing), then pass `verification_command` (e.g. 'cargo test') and `expected_output_keyword` (e.g. 'PASSED').\n2. **User Requirement Alignment**: Re-read the original user prompt and verify all explicitly requested features exist.\n3. **Zero Unverified Assumptions**: Base success strictly on empirical evidence, not speculative assumptions."
                )
            }
        }
        "reindex_skills" | "reindex" => {
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);

            // Invalidate in-memory caches
            crate::ml::embeddings::cache::clear_passage_cache();
            crate::context::cache::invalidate_snapshot(&proj_path);

            let snapshot = project_snapshot(&proj_path);
            let all_skills = snapshot.skills.as_ref();
            let count = all_skills.len();
            let fp = crate::ml::embeddings::catalog_fingerprint(all_skills);

            // Warmup / embed if model available
            let model_status = if let Some(vecs) = crate::ml::embeddings::precomputed::load_precomputed_cache(all_skills)
                .or_else(|| crate::ml::embeddings::load_passage_cache(all_skills))
            {
                format!(
                    "Loaded cached embeddings for {} skills (dimension: {})",
                    vecs.len(),
                    vecs.first().map(|v| v.len()).unwrap_or(0)
                )
            } else if all_skills.len() <= 64 {
                if let Some(model) = crate::ml::embeddings::cache::try_cached_model() {
                    let vecs = crate::ml::embeddings::embed_skills_cache(all_skills, &model);
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

            format!(
                "# Skill Semantic Index Refreshed\n\n- Total Skills: {}\n- Catalog Fingerprint: {:016x}\n- Status: {}\n- Cache Path: `~/.agent-guidance/vectors.bin`\n\n✓ All workspace and embedded skills reindexed with rich semantic passages.",
                count, fp, model_status
            )
        }
        _ => format!("Guidance operation '{}' completed successfully.", op),
      };
    Ok(resp)
}