use anyhow::Result;
use candle_core::{Device, Module, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{Repo, RepoType, api::sync::ApiBuilder};
use std::sync::{OnceLock, RwLock};
use tokenizers::Tokenizer;

use crate::catalog::store::SkillItem;
use rayon::prelude::*;

const MAX_CROSS_ENCODER_CANDIDATES: usize = 8;

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

        let num_labels = 1usize;
        let hidden_size = config.hidden_size;
        let classifier_w = vb
            .pp("classifier")
            .get((num_labels, hidden_size), "weight")?;
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
        let attention_mask =
            Tensor::new(encoding.get_attention_mask(), &self.device)?.unsqueeze(0)?;
        let token_type_ids = token_ids.zeros_like()?;

        let hidden = self
            .model
            .forward(&token_ids, &token_type_ids, Some(&attention_mask))?;
        let cls = hidden.narrow(1, 0, 1)?.squeeze(1)?;
        let logits = self.classifier.forward(&cls)?;
        let score = logits.narrow(1, 0, 1)?.squeeze(1)?.to_scalar::<f32>()?;
        Ok(score)
    }
}

pub fn cached_cross_encoder() -> Result<std::sync::RwLockReadGuard<'static, CrossEncoder>, String> {
    static CE: OnceLock<Result<RwLock<CrossEncoder>, String>> = OnceLock::new();
    match CE.get_or_init(|| {
        CrossEncoder::load()
            .map(RwLock::new)
            .map_err(|error| error.to_string())
    }) {
        Ok(encoder) => encoder.read().map_err(|_| "RwLock poisoned".to_string()),
        Err(error) => Err(error.clone()),
    }
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
        let cross_encoder = cached_cross_encoder().ok();
        let mut scored: Vec<(f32, SkillItem)> = crate::ml::inference_pool().install(|| {
            bounded_candidates
                .par_iter()
                .filter_map(|(_score, skill)| {
                    let text = format!(
                        "{} {}",
                        skill.name,
                        skill.content.chars().take(200).collect::<String>()
                    );
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
                let content_snippet_lower: String = skill
                    .content
                    .chars()
                    .take(300)
                    .collect::<String>()
                    .to_lowercase();
                let mut bonus = 0.0f32;

                for kw in &task_keywords {
                    if name_lower == *kw {
                        bonus += 0.3;
                    } else if name_lower.contains(kw) {
                        bonus += 0.15;
                    }
                    if content_snippet_lower.contains(kw) {
                        bonus += 0.05;
                    }
                }

                (*base_score + bonus, i)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut candidates_vec = candidates;
        let mut results = Vec::new();
        for (_, i) in scored.into_iter().take(limit) {
            results.push(std::mem::replace(
                &mut candidates_vec[i],
                (
                    0.0,
                    SkillItem {
                        name: String::new(),
                        relative_path: String::new(),
                        source: crate::catalog::store::SkillSource::Embedded,
                        content: String::new(),
                    },
                ),
            ));
        }
        results
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
            (
                0.5,
                SkillItem {
                    name: "rust-testing".to_string(),
                    relative_path: "rust-testing/SKILL.md".to_string(),
                    source: SkillSource::Embedded,
                    content: "Testing in Rust.".to_string(),
                },
            ),
            (
                0.8,
                SkillItem {
                    name: "rust-async".to_string(),
                    relative_path: "rust-async/SKILL.md".to_string(),
                    source: SkillSource::Embedded,
                    content: "Async programming in Rust.".to_string(),
                },
            ),
        ];
        let ranked = selector.keyword_fallback("rust async programming", candidates, 2);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].1.name, "rust-async");
    }

    #[test]
    fn test_language_aware_filtering() {
        use crate::catalog::language_detector::ProjectLanguageProfile;

        let selector = LLMSelector::new();
        let candidates = vec![
            (
                0.8,
                SkillItem {
                    name: "python-fastapi-guide".to_string(),
                    relative_path: "skills/python-fastapi/SKILL.md".to_string(),
                    content: "FastAPI guidelines".to_string(),
                    source: SkillSource::Embedded,
                },
            ),
            (
                0.8,
                SkillItem {
                    name: "rust-best-practices".to_string(),
                    relative_path: "skills/rust-best-practices/SKILL.md".to_string(),
                    content: "Rust coding guidelines".to_string(),
                    source: SkillSource::Embedded,
                },
            ),
        ];

        let mut rust_profile = ProjectLanguageProfile::default();
        rust_profile.primary_languages.insert("rust".to_string());

        let results = selector.rerank("optimize code", candidates, &rust_profile, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.name, "rust-best-practices");
    }
}
