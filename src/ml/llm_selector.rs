use anyhow::Result;
use candle_core::{Device, Module, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::ApiBuilder, Repo, RepoType};
use std::sync::{Mutex, OnceLock};
use tokenizers::Tokenizer;

use crate::catalog::store::SkillItem;

pub(crate) struct CrossEncoder {
    model: BertModel,
    classifier: candle_nn::Linear,
    tokenizer: Tokenizer,
    device: Device,
}

impl CrossEncoder {
    fn load() -> Result<Self> {
        let device = Device::Cpu;
        let repo = Repo::new(
            "cross-encoder/ms-marco-MiniLM-L-6-v2".to_string(),
            RepoType::Model,
        );
        let api = ApiBuilder::new().with_progress(false).build()?;
        let r = api.repo(repo);

        let config_path = r.get("config.json")?;
        let tokenizer_path = r.get("tokenizer.json")?;
        let weights_path = r.get("model.safetensors")?;

        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = serde_json::from_str(&config_str)?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Tokenizer load error: {}", e))?;

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F32, &device)?
        };

        let model = BertModel::load(vb.pp("bert"), &config)?;

        let num_labels = 2usize;
        let hidden_size = config.hidden_size;
        let classifier_w = vb.pp("classifier").get((num_labels, hidden_size), "weight")?;
        let classifier_b = vb.pp("classifier").get(num_labels, "bias")?;
        let classifier = candle_nn::Linear::new(classifier_w, Some(classifier_b));

        Ok(Self {
            model,
            classifier,
            tokenizer,
            device,
        })
    }

    fn score(&self, query: &str, candidate: &str) -> Result<f32> {
        let text = format!("{} [SEP] {}", query, candidate);
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("Encoding error: {}", e))?;

        let token_ids = Tensor::new(encoding.get_ids(), &self.device)?.unsqueeze(0)?;
        let attention_mask = Tensor::new(encoding.get_attention_mask(), &self.device)?.unsqueeze(0)?;
        let token_type_ids = token_ids.zeros_like()?;

        let hidden = self.model.forward(&token_ids, &token_type_ids, Some(&attention_mask))?;
        let cls = hidden.narrow(1, 0, 1)?.squeeze(1)?;
        let logits = self.classifier.forward(&cls)?;
        let score = logits.narrow(1, 1, 1)?.squeeze(1)?.to_scalar::<f32>()?;
        Ok(score)
    }
}

pub fn cached_cross_encoder() -> Result<std::sync::MutexGuard<'static, CrossEncoder>, String> {
    static CE: OnceLock<Mutex<CrossEncoder>> = OnceLock::new();
    let ce = CE.get_or_init(|| {
        CrossEncoder::load()
            .map(Mutex::new)
            .expect("Failed to load cross-encoder model. Reranking will use keyword fallback.")
    });
    ce.lock().map_err(|_| "Mutex poisoned".to_string())
}

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

    pub fn rerank(&self, task: &str, candidates: Vec<(f32, SkillItem)>, limit: usize) -> Vec<(f32, SkillItem)> {
        if let Ok(ce) = cached_cross_encoder() {
            let mut scored: Vec<(f32, SkillItem)> = Vec::new();
            for (_score, skill) in candidates.iter() {
                let text = format!(
                    "{} {}",
                    skill.name,
                    skill.content.chars().take(200).collect::<String>()
                );
                if let Ok(s) = ce.score(task, &text) {
                    scored.push((s, skill.clone()));
                }
            }
            if !scored.is_empty() {
                scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                return scored.into_iter().take(limit).collect();
            }
        }

        self.keyword_fallback(task, candidates, limit)
    }

    fn keyword_fallback(&self, task: &str, candidates: Vec<(f32, SkillItem)>, limit: usize) -> Vec<(f32, SkillItem)> {
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

                (base_score + bonus, skill)
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
    fn test_keyword_fallback() {
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
        let ranked = selector.keyword_fallback("rust async programming", candidates, 2);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].1.name, "rust-async");
    }
}
