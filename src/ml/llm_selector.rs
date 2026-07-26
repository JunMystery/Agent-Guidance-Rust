use anyhow::Result;
use hf_hub::{api::sync::ApiBuilder, Repo, RepoType};

use crate::catalog::store::SkillItem;

#[allow(dead_code)]
pub struct LLMSelector {
    loaded: bool,
}

impl Default for LLMSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl LLMSelector {
    pub fn new() -> Self {
        Self { loaded: false }
    }

    #[allow(dead_code)]
    pub fn load_from_local_cache(&mut self) -> Result<()> {
        if self.loaded {
            return Ok(());
        }

        let repo_spec = Repo::new(
            "Qwen/Qwen2.5-0.5B-Instruct".to_string(),
            RepoType::Model,
        );

        let repo = ApiBuilder::new()
            .with_progress(false)
            .build()
            .map(|api| api.repo(repo_spec));

        if let Ok(r) = repo {
            let _cfg = r.get("config.json");
            let _tok = r.get("tokenizer.json");
            self.loaded = true;
        }

        Ok(())
    }

    /// 2nd Stage Selection: Re-ranks 1st stage vector candidates using task context & instruction prompt matching
    pub fn rerank(&self, task: &str, candidates: Vec<(f32, SkillItem)>, limit: usize) -> Vec<(f32, SkillItem)> {
        let task_keywords: Vec<String> = task
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if task_keywords.is_empty() {
            return candidates.into_iter().take(limit).collect();
        }

        let mut scored: Vec<(f32, SkillItem)> = candidates
            .into_iter()
            .map(|(base_score, skill)| {
                let name_lower = skill.name.to_lowercase();
                let content_lower = skill.content.to_lowercase();
                let mut bonus = 0.0f32;

                for kw in &task_keywords {
                    if name_lower == *kw {
                        bonus += 0.3;
                    } else if name_lower.contains(kw) {
                        bonus += 0.15;
                    }
                    if content_lower.contains(kw) {
                        bonus += 0.05;
                    }
                }

                let final_score = base_score + bonus;
                (final_score, skill)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(limit).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::store::SkillSource;

    #[test]
    fn test_select_skills_ranking() {
        let selector = LLMSelector::new();
        let candidates = vec![
            (0.5, SkillItem {
                name: "rust-testing".to_string(),
                relative_path: "rust-testing/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "Testing in Rust.".to_string(),
            }),
            (0.8, SkillItem {
                name: "rust-async".to_string(),
                relative_path: "rust-async/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "Async programming in Rust.".to_string(),
            }),
        ];
        let ranked = selector.rerank("rust async programming", candidates, 2);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].1.name, "rust-async");
    }
}
