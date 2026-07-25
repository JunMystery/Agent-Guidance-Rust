use anyhow::Result;
use hf_hub::{api::sync::ApiBuilder, Repo, RepoType};

#[allow(dead_code)]
pub struct LLMSelector {
    loaded: bool,
}

impl Default for LLMSelector {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl LLMSelector {
    pub fn new() -> Self {
        Self { loaded: false }
    }

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

    /// 2nd Stage Selection: Re-ranks 1st stage candidates using task context & instruction prompt matching
    pub fn select_skills(&self, task: &str, candidates: Vec<String>, limit: usize) -> Vec<String> {
        let task_keywords: Vec<String> = task
            .to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if task_keywords.is_empty() {
            return candidates.into_iter().take(limit).collect();
        }

        let mut scored: Vec<(usize, String)> = candidates
            .into_iter()
            .map(|cand| {
                let cand_lower = cand.to_lowercase();
                let mut score = 0;
                for kw in &task_keywords {
                    if cand_lower == *kw {
                        score += 10;
                    } else if cand_lower.contains(kw) {
                        score += 3;
                    }
                }
                (score, cand)
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, cand)| cand).take(limit).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_skills_ranking() {
        let selector = LLMSelector::new();
        let candidates = vec![
            "rust-testing".to_string(),
            "python-web".to_string(),
            "rust-async".to_string(),
        ];
        let ranked = selector.select_skills("rust async programming", candidates, 2);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0], "rust-async");
    }
}
