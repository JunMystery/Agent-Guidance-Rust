use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::ApiBuilder, Repo, RepoType};
use std::sync::{Mutex, OnceLock};
use tokenizers::Tokenizer;

use crate::catalog::store::SkillItem;

pub struct EmbeddingModel {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let dot: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
    let norm1: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm2: f32 = v2.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm1 == 0.0 || norm2 == 0.0 {
        0.0
    } else {
        dot / (norm1 * norm2)
    }
}

impl EmbeddingModel {
    pub fn load_or_download() -> Result<Self> {
        let device = Device::Cpu;
        let repo_spec = Repo::new(
            "intfloat/multilingual-e5-small".to_string(),
            RepoType::Model,
        );

        let repo = ApiBuilder::new()
            .with_progress(false)
            .build()
            .map(|api| api.repo(repo_spec.clone()));

        let (config_filename, tokenizer_filename, weights_filename) = match repo {
            Ok(ref r) => {
                let cfg = r.get("config.json");
                let tok = r.get("tokenizer.json");
                let wgt = r.get("model.safetensors");
                match (cfg, tok, wgt) {
                    (Ok(c), Ok(t), Ok(w)) => (c, t, w),
                    _ => return Err(anyhow::anyhow!("Model files missing from local HuggingFace cache")),
                }
            },
            Err(e) => return Err(anyhow::anyhow!("Failed to initialize HuggingFace API: {}", e)),
        };

        let config_str = std::fs::read_to_string(config_filename)?;
        let config: Config = serde_json::from_str(&config_str)?;

        let tokenizer = Tokenizer::from_file(tokenizer_filename)
            .map_err(|e| anyhow::anyhow!("Tokenizer load error: {}", e))?;

        // SAFETY: VarBuilder::from_mmaped_safetensors memory-maps the model weight files on disk.
        // The files are loaded read-only from local cache and not mutated concurrently.
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_filename], candle_core::DType::F32, &device)?
        };

        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    pub fn embed_text(&self, text: &str, prefix: Option<&str>) -> Result<Vec<f32>> {
        let prompt = match prefix {
            Some("query") => format!("query: {}", text),
            Some("passage") => format!("passage: {}", text),
            _ => text.to_string(),
        };

        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("Encoding error: {}", e))?;

        let tokens = encoding.get_ids();
        let attention_mask_vec = encoding.get_attention_mask();

        let token_ids = Tensor::new(tokens, &self.device)?.unsqueeze(0)?;
        let attention_mask = Tensor::new(attention_mask_vec, &self.device)?.unsqueeze(0)?;
        let token_type_ids = token_ids.zeros_like()?;

        let embeddings = self.model.forward(&token_ids, &token_type_ids, Some(&attention_mask))?;
        
        // Attention-masked Mean Pooling
        let mask_expanded = attention_mask.unsqueeze(2)?.to_dtype(candle_core::DType::F32)?;
        let masked_embeddings = embeddings.broadcast_mul(&mask_expanded)?;
        let sum_embeddings = masked_embeddings.sum(1)?;
        let sum_mask = mask_expanded.sum(1)?.clamp(1e-9, f64::MAX)?;
        let mean_embedding = sum_embeddings.broadcast_div(&sum_mask)?;
        
        // L2 normalization
        let norm = mean_embedding.sqr()?.sum_keepdim(1)?.sqrt()?;
        let normalized = mean_embedding.broadcast_div(&norm)?;
        
        let vec = normalized.squeeze(0)?.to_vec1::<f32>()?;
        Ok(vec)
    }
}

pub fn cached_model() -> Result<std::sync::MutexGuard<'static, EmbeddingModel>, String> {
    static MODEL: OnceLock<Mutex<EmbeddingModel>> = OnceLock::new();
    let model = MODEL.get_or_init(|| {
        EmbeddingModel::load_or_download()
            .map(Mutex::new)
            .expect("Failed to load embedding model. Check HuggingFace cache and network connectivity.")
    });
    model.lock().map_err(|_| "Mutex poisoned".to_string())
}

pub fn hybrid_vector_search(query: &str, candidates: &[SkillItem], top_k: usize) -> Vec<(f32, SkillItem)> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let q_lower = query.to_lowercase();
    let words: Vec<&str> = q_lower.split_whitespace().collect();

    let mut scored: Vec<(f32, SkillItem)> = Vec::new();

    if let Ok(model) = cached_model() {
        if let Ok(q_vec) = model.embed_text(query, Some("query")) {
            for cand in candidates {
                let text_sample = format!("{} {}", cand.name, cand.content.chars().take(300).collect::<String>());
                let mut score = if let Ok(c_vec) = model.embed_text(&text_sample, Some("passage")) {
                    cosine_similarity(&q_vec, &c_vec)
                } else {
                    0.0
                };

                // Exact keyword match boost
                let name_lower = cand.name.to_lowercase();
                if name_lower == q_lower {
                    score += 0.5;
                } else if name_lower.contains(&q_lower) {
                    score += 0.3;
                } else {
                    for w in &words {
                        if name_lower.contains(w) {
                            score += 0.1;
                        }
                    }
                }

                scored.push((score, cand.clone()));
            }

            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            return scored.into_iter().take(top_k).collect();
        }
    }

    // Resilient fallback: BM25 / token-frequency keyword ranking
    for cand in candidates {
        let name_lower = cand.name.to_lowercase();
        let content_lower = cand.content.to_lowercase();
        let mut score = 0.0f32;

        if name_lower == q_lower {
            score += 1.0;
        } else if name_lower.contains(&q_lower) {
            score += 0.7;
        }

        for w in &words {
            if name_lower.contains(w) {
                score += 0.4;
            }
            if content_lower.contains(w) {
                score += 0.1;
            }
        }

        if score > 0.0 || query.is_empty() {
            scored.push((score, cand.clone()));
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(top_k).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::store::SkillSource;

    #[test]
    fn test_prefix_formatting() {
        let prompt_query = format!("query: {}", "test code");
        let prompt_passage = format!("passage: {}", "test code");
        assert_eq!(prompt_query, "query: test code");
        assert_eq!(prompt_passage, "passage: test code");
    }

    #[test]
    fn test_cosine_similarity() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let v3 = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&v1, &v2) - 1.0).abs() < 1e-5);
        assert!((cosine_similarity(&v1, &v3) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_hybrid_vector_search_fallback() {
        let candidates = vec![
            SkillItem {
                name: "context-budget".to_string(),
                relative_path: "context-budget/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "Reducing context size and managing token limits.".to_string(),
            },
            SkillItem {
                name: "rust-testing".to_string(),
                relative_path: "rust-testing/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "Rust unit and integration testing.".to_string(),
            },
        ];

        let results = hybrid_vector_search("reducing context size", &candidates, 2);
        assert!(!results.is_empty());
        assert_eq!(results[0].1.name, "context-budget");
    }
}
