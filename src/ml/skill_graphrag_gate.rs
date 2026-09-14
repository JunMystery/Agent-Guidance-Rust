use std::path::Path;
use crate::catalog::store::SkillItem;
use crate::context::db::{CodeGraphDb, cosine_similarity};
use crate::ml::embeddings::cache::{embed_skills_cache, try_cached_model};

const UNIVERSAL_KEYWORDS: &[&str] = &[
    "workflow", "guidance", "agent", "flow", "test", "review", "git", "cost", "media", "doc",
];

/// Stage 1.5 Context Gating: Refines Stage 1 skill candidates by Max-Pooling against
/// Top-K symbol & chunk vectors from GraphRAG context.
pub fn apply_graphrag_context_gating(
    proj_path: &Path,
    query: &str,
    candidates: Vec<(f32, SkillItem)>,
    limit: usize,
) -> Vec<(f32, SkillItem)> {
    if candidates.is_empty() {
        return candidates;
    }

    let q_lower = query.to_lowercase();
    let words: Vec<&str> = q_lower.split_whitespace().collect();

    // Graceful Bypass: universal meta tasks bypass codebase context gating
    let is_universal_query = words.iter().any(|w| UNIVERSAL_KEYWORDS.contains(w));
    if is_universal_query {
        return candidates.into_iter().take(limit).collect();
    }

    // Graceful Bypass: if no code graph database exists, preserve candidates
    let db = match CodeGraphDb::open_for_project(proj_path) {
        Ok(db) => db,
        Err(_) => return candidates.into_iter().take(limit).collect(),
    };

    // Graceful Bypass: if embedding model not warmed up, preserve candidates
    let model = match try_cached_model() {
        Some(m) => m,
        None => return candidates.into_iter().take(limit).collect(),
    };

    let qv = match model.embed_text(query, Some("query")) {
        Ok(v) => v,
        Err(_) => return candidates.into_iter().take(limit).collect(),
    };

    // Retrieve Top-K context vectors from GraphRAG
    let context_vectors = match db.fetch_top_context_vectors(&qv, 10) {
        Ok(cv) if !cv.is_empty() => cv,
        _ => return candidates.into_iter().take(limit).collect(),
    };

    let skill_items: Vec<SkillItem> = candidates.iter().map(|(_, s)| s.clone()).collect();
    let c_vecs = embed_skills_cache(&skill_items, &model);
    if c_vecs.is_empty() {
        return candidates.into_iter().take(limit).collect();
    }

    let mut scored_candidates: Vec<(f32, SkillItem)> = Vec::new();
    for (i, (base_score, skill)) in candidates.into_iter().enumerate() {
        let skill_vec = match c_vecs.get(i) {
            Some(v) => v,
            None => {
                scored_candidates.push((base_score, skill));
                continue;
            }
        };

        let name_lower = skill.name.to_lowercase();
        let is_directly_mentioned = q_lower.contains(&name_lower);
        let is_universal_skill = UNIVERSAL_KEYWORDS.iter().any(|k| name_lower.contains(k));

        // Max-Pooling: best similarity match against any top context vector
        let max_sim = context_vectors
            .iter()
            .map(|cv| cosine_similarity(skill_vec, cv))
            .fold(0.0f32, f32::max);

        let mut adjusted_score = base_score;

        if max_sim >= 0.40 {
            // Soft-gating bonus for strong contextual resonance
            adjusted_score += 0.40 * max_sim;
        } else if max_sim < 0.25 && !is_directly_mentioned && !is_universal_skill {
            // Soft-gating penalty for low contextual relevance
            let penalty = (max_sim / 0.25).clamp(0.30, 1.0);
            adjusted_score *= penalty;
        }

        // Cutoff irrelevant candidates falling below floor threshold
        if adjusted_score < 0.15 && !is_directly_mentioned {
            continue;
        }

        scored_candidates.push((adjusted_score, skill));
    }

    scored_candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored_candidates.into_iter().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::store::{SkillItem, SkillSource};

    fn make_test_skill(name: &str, content: &str) -> SkillItem {
        SkillItem {
            name: name.to_string(),
            relative_path: format!("{}/SKILL.md", name),
            source: SkillSource::Embedded,
            content: content.to_string(),
        }
    }

    #[test]
    fn test_bypass_on_universal_query() {
        let skills = vec![
            (1.0, make_test_skill("git-commit", "Git commit helper")),
            (0.8, make_test_skill("rust-helper", "Rust coding")),
        ];
        let res = apply_graphrag_context_gating(
            Path::new("/nonexistent"),
            "please git commit my changes",
            skills.clone(),
            5,
        );
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].1.name, "git-commit");
    }

    #[test]
    fn test_bypass_on_missing_db() {
        let skills = vec![(0.9, make_test_skill("react-ui", "React components"))];
        let res = apply_graphrag_context_gating(
            Path::new("/path/that/does/not/exist"),
            "create react button",
            skills.clone(),
            5,
        );
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].0, 0.9);
    }
}
