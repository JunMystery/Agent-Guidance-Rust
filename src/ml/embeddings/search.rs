use std::sync::{Arc, RwLock};
use crate::catalog::store::SkillItem;
use super::cache::{cached_model, embed_skills_cache, GPU_SKILL_MATRIX};
use super::precomputed::catalog_fingerprint;

pub fn hybrid_vector_search(
    query: &str,
    candidates: &[SkillItem],
    top_k: usize,
) -> Vec<(f32, SkillItem)> {
    if candidates.is_empty() {
        return Vec::new();
    }

    crate::mcp::db::log_embed_query(query);

    let q_lower = query.to_lowercase();
    let words: Vec<&str> = q_lower.split_whitespace().collect();

    let model_res = super::cache::try_cached_model().ok_or_else(|| "Model warming up in background".to_string());
    let (q_vec, c_vecs) = match &model_res {
        Ok(model) => {
            let q = model.embed_text(query, Some("query")).ok();
            let c = embed_skills_cache(candidates, model);
            (q, c)
        }
        _ => (None::<Vec<f32>>, Arc::new(Vec::new())),
    };

    let mut scored: Vec<(f32, usize)> = Vec::new();
    if let Some(ref qv) = q_vec {
        if !c_vecs.is_empty() {
            let fingerprint = catalog_fingerprint(candidates);
            let gpu_scores: Option<Vec<f32>> = if let Ok(ref model) = model_res {
                let gpu_slot = GPU_SKILL_MATRIX.get_or_init(|| RwLock::new(None));
                if let Ok(guard) = gpu_slot.read() {
                    if let Some(ref matrix) = *guard {
                        if matrix.fingerprint == fingerprint && matrix.count == candidates.len() {
                            matrix.score_query(qv, &model.device).ok()
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            for (i, cand) in candidates.iter().enumerate() {
                let base_score = match &gpu_scores {
                    Some(scores) => scores.get(i).copied().unwrap_or(0.0),
                    None => {
                        let vec_i = if i < c_vecs.len() {
                            &c_vecs[i]
                        } else {
                            continue;
                        };
                        qv.iter().zip(vec_i.iter()).map(|(a, b)| a * b).sum()
                    }
                };

                let mut score = base_score;
                let name_lower = cand.name.to_lowercase();
                let doc = cand.to_semantic_doc();
                let intent_lower = doc.intent.to_lowercase();
                let actions_lower: String = doc.action_triggers.join(" ").to_lowercase();

                if name_lower == q_lower {
                    score += 0.5;
                } else if name_lower.contains(&q_lower) {
                    score += 0.3;
                } else if !intent_lower.is_empty() && intent_lower.contains(&q_lower) {
                    score += 0.25;
                }

                for w in &words {
                    if name_lower.contains(w) {
                        score += 0.1;
                    }
                    if actions_lower.contains(w) {
                        score += 0.12;
                    }
                    if intent_lower.contains(w) {
                        score += 0.08;
                    }
                }

                scored.push((score, i));
            }

            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            return scored
                .into_iter()
                .take(top_k)
                .map(|(s, i)| (s, candidates[i].clone()))
                .collect();
        }
    }

    for (i, cand) in candidates.iter().enumerate() {
        let name_lower = cand.name.to_lowercase();
        let doc = cand.to_semantic_doc();
        let intent_lower = doc.intent.to_lowercase();
        let desc_lower = doc.description.to_lowercase();
        let triggers_lower: String = doc.triggers.join(" ").to_lowercase();
        let actions_lower: String = doc.action_triggers.join(" ").to_lowercase();
        let keywords_lower: String = doc.keywords.join(" ").to_lowercase();
        let rules_lower: String = doc.micro_rules.join(" ").to_lowercase();
        let content_lower = cand.content.to_lowercase();
        let mut score = 0.0f32;

        if name_lower == q_lower {
            score += 1.0;
        } else if name_lower.contains(&q_lower) {
            score += 0.7;
        }

        if !intent_lower.is_empty() && intent_lower.contains(&q_lower) {
            score += 0.6;
        }
        if !desc_lower.is_empty() && desc_lower.contains(&q_lower) {
            score += 0.4;
        }
        if !triggers_lower.is_empty() && triggers_lower.contains(&q_lower) {
            score += 0.5;
        }
        if !actions_lower.is_empty() && actions_lower.contains(&q_lower) {
            score += 0.6;
        }

        for w in &words {
            if name_lower.contains(w) {
                score += 0.4;
            }
            if actions_lower.contains(w) {
                score += 0.35;
            }
            if intent_lower.contains(w) {
                score += 0.25;
            }
            if triggers_lower.contains(w) {
                score += 0.25;
            }
            if rules_lower.contains(w) {
                score += 0.2;
            }
            if desc_lower.contains(w) {
                score += 0.15;
            }
            if keywords_lower.contains(w) {
                score += 0.2;
            }
            if content_lower.contains(w) {
                score += 0.05;
            }
        }

        if score > 0.0 || query.is_empty() {
            scored.push((score, i));
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored
        .into_iter()
        .take(top_k)
        .map(|(s, i)| (s, candidates[i].clone()))
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::store::{SkillItem, SkillSource};

    #[test]
    fn test_fast_path_keyword_search_returns_relevant_skills() {
        let candidates = vec![
            SkillItem {
                name: "rust-performance".to_string(),
                relative_path: "rust-performance/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "High performance Rust profiling, memory allocation, and CPU optimization techniques.".to_string(),
            },
            SkillItem {
                name: "react-components".to_string(),
                relative_path: "react-components/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "Building reusable React UI components and hooks.".to_string(),
            },
        ];

        let results = hybrid_vector_search("optimize rust CPU", &candidates, 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].1.name, "rust-performance");
    }

    #[test]
    fn test_implicit_intent_and_action_matching() {
        let candidates = vec![
            SkillItem {
                name: "sql-injection-prevention-cheat-sheet".to_string(),
                relative_path: "sql-injection-prevention-cheat-sheet/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "# SQL Injection Prevention\n\nDefense in depth against SQL injection.\n\n## Primary Defenses\n- Use Parameterized Queries".to_string(),
            },
            SkillItem {
                name: "docker-security".to_string(),
                relative_path: "docker-security/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "# Docker Security\n\nContainer hardening guidelines.".to_string(),
            },
        ];

        // Prompt without naming "sql-injection-prevention-cheat-sheet"
        let results = hybrid_vector_search("refactor database query execution", &candidates, 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].1.name, "sql-injection-prevention-cheat-sheet");
    }
}
