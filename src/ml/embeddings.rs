use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::ApiBuilder, Repo, RepoType};
use tokenizers::Tokenizer;

#[allow(dead_code)]
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

#[allow(dead_code)]
impl EmbeddingModel {
    pub fn load_from_local_cache() -> Result<Self> {
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

    pub fn rank_candidates(&self, query: &str, candidates: &[String], top_k: usize) -> Result<Vec<(f32, String)>> {
        let query_vec = self.embed_text(query, Some("query"))?;
        let mut scored = Vec::new();
        for cand in candidates {
            if let Ok(cand_vec) = self.embed_text(cand, Some("passage")) {
                let score = cosine_similarity(&query_vec, &cand_vec);
                scored.push((score, cand.clone()));
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored.into_iter().take(top_k).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
