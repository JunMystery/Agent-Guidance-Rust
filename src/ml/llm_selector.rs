use anyhow::Result;
use hf_hub::{api::sync::ApiBuilder, Repo, RepoType};

#[allow(dead_code)]
pub struct LLMSelector {
    loaded: bool,
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
        let api = ApiBuilder::new().with_progress(false).build()?;
        let repo = api.repo(Repo::new(
            "Qwen/Qwen2.5-0.5B-Instruct".to_string(),
            RepoType::Model,
        ));

        // Locate local files without re-downloading
        let _config_filename = repo.get("config.json")?;
        let _tokenizer_filename = repo.get("tokenizer.json")?;

        self.loaded = true;
        Ok(())
    }

    pub fn select_skills(&self, _task: &str, candidates: Vec<String>, limit: usize) -> Vec<String> {
        // High-speed candidate fallback filter
        candidates.into_iter().take(limit).collect()
    }
}
