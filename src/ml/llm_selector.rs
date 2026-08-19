use crate::catalog::store::SkillItem;
use rayon::prelude::*;
pub use super::cross_encoder::cached_cross_encoder;

const MAX_CROSS_ENCODER_CANDIDATES: usize = 8;

pub struct LLMSelector;

impl Default for LLMSelector {
    fn default() -> Self {
        Self
    }
}

impl LLMSelector {
    pub fn new() -> Self {
        Self
    }

    pub fn rerank(
        &self,
        task: &str,
        candidates: Vec<(f32, SkillItem)>,
        profile: &crate::catalog::language_detector::ProjectLanguageProfile,
        limit: usize,
    ) -> Vec<(f32, SkillItem)> {
        let task_lower = task.to_lowercase();
        let candidate_filter = |skill: &SkillItem| -> bool {
            let name_lower = skill.name.to_lowercase();
            let relative_lower = skill.relative_path.to_lowercase();

            // Direct mention in prompt bypasses filtering
            if task_lower.contains(&name_lower) {
                return true;
            }

            // Universal/general skills always pass
            let universal_keywords = [
                "workflow", "guidance", "agent", "flow", "test", "review", "git", "cost", "media",
                "doc",
            ];
            if universal_keywords
                .iter()
                .any(|k| name_lower.contains(k) || relative_lower.contains(k))
            {
                return true;
            }

            // Check language affinity
            let lang_keywords = [
                ("rust", "rust"),
                ("python", "python"),
                ("javascript", "javascript"),
                ("typescript", "typescript"),
                ("go", "go"),
                ("golang", "go"),
                ("java", "java"),
                ("cpp", "c++"),
                ("c++", "c++"),
                ("ruby", "ruby"),
                ("php", "php"),
            ];

            let mut skill_langs = Vec::new();
            for (kw, lang) in lang_keywords {
                if name_lower.contains(kw) || relative_lower.contains(kw) {
                    skill_langs.push(lang);
                }
            }

            if !skill_langs.is_empty() {
                // Skill is language-bound -> must match project's primary languages or direct prompt mention
                if !profile.primary_languages.is_empty() {
                    let matches_primary = skill_langs
                        .iter()
                        .any(|l| profile.primary_languages.contains(*l));
                    if !matches_primary {
                        return false;
                    }
                }
            }

            // Check domain tech affinity (e.g. database, sql, docker, frontend)
            let domain_keywords = [
                ("database", "database"),
                ("sql", "sql"),
                ("docker", "docker"),
                ("devops", "devops"),
                ("frontend", "frontend"),
                ("web", "web"),
                ("bash", "bash"),
                ("shell", "shell"),
            ];

            for (kw, domain) in domain_keywords {
                if name_lower.contains(kw) || relative_lower.contains(kw) {
                    if !profile.secondary_tech.is_empty()
                        && !profile.secondary_tech.contains(domain)
                    {
                        // Project does not have this domain tech stack
                        return false;
                    }
                }
            }

            true
        };

        let filtered_candidates: Vec<(f32, SkillItem)> = candidates
            .iter()
            .filter(|(_, skill)| candidate_filter(skill))
            .cloned()
            .collect();

        let bounded_candidates: Vec<(f32, SkillItem)> = filtered_candidates
            .iter()
            .take(MAX_CROSS_ENCODER_CANDIDATES)
            .cloned()
            .collect();
        let cross_encoder = super::cross_encoder::try_cached_cross_encoder();
        let mut scored: Vec<(f32, SkillItem)> = crate::ml::inference_pool().install(|| {
            bounded_candidates
                .par_iter()
                .filter_map(|(_score, skill)| {
                    let text = skill.to_search_passage();
                    cross_encoder.as_ref().and_then(|ce| {
                        ce.score(task, &text).ok().map(|logit| {
                            let mut prob = 1.0 / (1.0 + (-logit).exp());
                            let name_lower = skill.name.to_lowercase();
                            if task_lower.contains(&name_lower) {
                                prob += 0.5;
                            }
                            (prob, skill.clone())
                        })
                    })
                })
                .collect()
        });
        if !scored.is_empty() {
            tracing::info!(
                "[ML Pipeline] Cross-Encoder reranked {} candidate skills successfully.",
                scored.len()
            );
            scored.par_sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            // Relevance threshold cutoff (>= 0.35 probability)
            let filtered: Vec<(f32, SkillItem)> = scored
                .into_iter()
                .filter(|(prob, _)| *prob >= 0.35)
                .take(limit)
                .collect();
            return filtered;
        }

        tracing::info!("[ML Pipeline] Fallback to keyword-based reranking.");
        self.keyword_fallback(task, filtered_candidates, limit)
    }

    fn keyword_fallback(
        &self,
        task: &str,
        candidates: Vec<(f32, SkillItem)>,
        limit: usize,
    ) -> Vec<(f32, SkillItem)> {
        let task_keywords: Vec<String> = task
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if task_keywords.is_empty() {
            return candidates.into_iter().take(limit).collect();
        }

        let mut scored: Vec<(f32, usize)> = candidates
            .iter()
            .enumerate()
            .map(|(i, (base_score, skill))| {
                let name_lower = skill.name.to_lowercase();
                let doc = skill.to_semantic_doc();
                let intent_lower = doc.intent.to_lowercase();
                let desc_lower = doc.description.to_lowercase();
                let triggers_lower = doc.triggers.join(" ").to_lowercase();
                let actions_lower = doc.action_triggers.join(" ").to_lowercase();
                let keywords_lower = doc.keywords.join(" ").to_lowercase();
                let rules_lower = doc.micro_rules.join(" ").to_lowercase();
                let content_lower = skill.content.to_lowercase();
                let mut bonus = 0.0f32;

                for kw in &task_keywords {
                    if name_lower == *kw {
                        bonus += 0.3;
                    } else if name_lower.contains(kw) {
                        bonus += 0.15;
                    }
                    if actions_lower.contains(kw) {
                        bonus += 0.2;
                    }
                    if intent_lower.contains(kw) {
                        bonus += 0.15;
                    }
                    if triggers_lower.contains(kw) {
                        bonus += 0.15;
                    }
                    if rules_lower.contains(kw) {
                        bonus += 0.1;
                    }
                    if desc_lower.contains(kw) {
                        bonus += 0.1;
                    }
                    if keywords_lower.contains(kw) {
                        bonus += 0.1;
                    }
                    if content_lower.contains(kw) {
                        bonus += 0.03;
                    }
                }

                (*base_score + bonus, i)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .into_iter()
            .take(limit)
            .map(|(score, i)| (score, candidates[i].1.clone()))
            .collect()
    }
}

#[cfg(test)]
#[path = "llm_selector_tests.rs"]
mod tests;
