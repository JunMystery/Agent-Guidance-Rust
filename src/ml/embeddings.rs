use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::ApiBuilder, Repo, RepoType};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokenizers::Tokenizer;
use tracing::info;

use crate::catalog::store::{SkillItem, list_embedded_skills, get_embedded_skill, SkillSource};
use crate::ml::llm_selector::cached_cross_encoder;
use rayon::prelude::*;

// Precomputed passage vectors for the 168 embedded skills.
// Generated via `agent-guidance --generate-passage-cache`.
// Provides instant vector search on first start — no model load needed.
const PRECOMPUTED_VECTORS: &[u8] = include_bytes!("precomputed_vectors.bin");
const PRECOMPUTED_MANIFEST: &[u8] = include_bytes!("precomputed_manifest.json");

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

static PASSAGE_CACHE: OnceLock<Mutex<Vec<Vec<f32>>>> = OnceLock::new();
static WARMUP_DONE: OnceLock<AtomicBool> = OnceLock::new();

fn cache_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance"))
        .unwrap_or_else(|| PathBuf::from(".agent-guidance"))
}

fn manifest_path() -> PathBuf {
    cache_dir().join("passage_manifest.json")
}

fn vectors_path() -> PathBuf {
    cache_dir().join("vectors.bin")
}

/// Try to load precomputed passage vectors embedded in the binary.
/// Only matches built-in (embedded) skills — workspace-local skills fall through.
fn load_precomputed_cache() -> Option<Vec<Vec<f32>>> {
    let manifest: serde_json::Value = serde_json::from_slice(PRECOMPUTED_MANIFEST).ok()?;
    let cached_skills: Vec<String> = manifest.get("skills")?.as_array()?
        .iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();

    let current_skills: Vec<String> = list_embedded_skills();
    if cached_skills != current_skills {
        return None;
    }

    let data = PRECOMPUTED_VECTORS;
    if data.len() < 8 {
        return None;
    }

    let count = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    let dim = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let expected_len = 8 + count * dim * 4;
    if data.len() != expected_len {
        return None;
    }

    let mut vectors = Vec::with_capacity(count);
    let mut offset = 8;
    for _ in 0..count {
        let mut vec = Vec::with_capacity(dim);
        for _ in 0..dim {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&data[offset..offset + 4]);
            vec.push(f32::from_le_bytes(bytes));
            offset += 4;
        }
        vectors.push(vec);
    }

    Some(vectors)
}

/// Generate precomputed passage cache files for the 168 embedded skills.
/// Called via `agent-guidance --generate-passage-cache`.
pub fn generate_precomputed_cache() -> Result<()> {
    let candidates: Vec<SkillItem> = list_embedded_skills()
        .iter()
        .filter_map(|path| {
            get_embedded_skill(path).map(|content| SkillItem {
                name: path.split('/').next().unwrap_or(path).to_string(),
                relative_path: path.clone(),
                source: SkillSource::Embedded,
                content,
            })
        })
        .collect();

    info!("Computing passage embeddings for {} skills (generate-precomputed)...", candidates.len());
    let model_guard = cached_model().map_err(|e| anyhow::anyhow!("{}", e))?;
    let texts: Vec<String> = candidates.iter().map(|c| {
        format!("{} {}", c.name, c.content.chars().take(300).collect::<String>())
    }).collect();

    let vecs: Vec<Vec<f32>> = texts.par_iter()
        .filter_map(|text| model_guard.embed_text(text, Some("passage")).ok())
        .collect();

    if vecs.is_empty() || vecs[0].is_empty() {
        return Err(anyhow::anyhow!("No passage vectors generated."));
    }
    let dim = vecs[0].len() as u32;
    let count = vecs.len() as u32;

    let mut buf = Vec::with_capacity(8 + count as usize * dim as usize * 4);
    buf.extend_from_slice(&count.to_le_bytes());
    buf.extend_from_slice(&dim.to_le_bytes());
    for v in &vecs {
        for &val in v {
            buf.extend_from_slice(&val.to_le_bytes());
        }
    }

    let manifest = serde_json::json!({
        "version": 1,
        "created_at": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        "skills": list_embedded_skills(),
    });

    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ml_dir = crate_root.join("src").join("ml");
    if ml_dir.exists() {
        let _ = std::fs::write(ml_dir.join("precomputed_vectors.bin"), &buf);
        let _ = std::fs::write(
            ml_dir.join("precomputed_manifest.json"),
            serde_json::to_string_pretty(&manifest)?,
        );
        info!("Precomputed cache written to src/ml/ ({} skills, {} bytes).", count, buf.len());
    }

    save_passage_cache(&vecs, &candidates);
    info!("On-disk cache written to {:?} ({} skills).", vectors_path(), count);

    Ok(())
}

fn is_warmup_complete() -> bool {
    WARMUP_DONE.get().map(|v| v.load(Ordering::SeqCst)).unwrap_or(false)
}

fn mark_warmup_complete() {
    WARMUP_DONE.get_or_init(|| AtomicBool::new(false)).store(true, Ordering::SeqCst);
}

fn save_passage_cache(vectors: &[Vec<f32>], skills: &[SkillItem]) {
    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);

    let skill_names: Vec<String> = skills.iter().map(|s| s.relative_path.clone()).collect();
    let manifest = serde_json::json!({
        "version": 1,
        "created_at": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        "skills": skill_names,
    });
    if let Ok(content) = serde_json::to_string_pretty(&manifest) {
        let _ = std::fs::write(manifest_path(), content);
    }

    if vectors.is_empty() || vectors[0].is_empty() {
        return;
    }
    let dim = vectors[0].len() as u32;
    let count = vectors.len() as u32;
    let mut buf = Vec::with_capacity(8 + count as usize * dim as usize * 4);
    buf.extend_from_slice(&count.to_le_bytes());
    buf.extend_from_slice(&dim.to_le_bytes());
    for v in vectors {
        for &val in v {
            buf.extend_from_slice(&val.to_le_bytes());
        }
    }
    let _ = std::fs::write(vectors_path(), &buf);
}

fn load_passage_cache(skills: &[SkillItem]) -> Option<Vec<Vec<f32>>> {
    let manifest_content = std::fs::read_to_string(manifest_path()).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_content).ok()?;
    let cached_skills: Vec<String> = manifest.get("skills")?.as_array()?
        .iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();

    let current_skills: Vec<String> = skills.iter().map(|s| s.relative_path.clone()).collect();
    if cached_skills != current_skills {
        return None;
    }

    let data = std::fs::read(vectors_path()).ok()?;
    if data.len() < 8 {
        return None;
    }

    let count = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    let dim = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let expected_len = 8 + count * dim * 4;
    if data.len() != expected_len {
        return None;
    }

    let mut vectors = Vec::with_capacity(count);
    let mut offset = 8;
    for _ in 0..count {
        let mut vec = Vec::with_capacity(dim);
        for _ in 0..dim {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&data[offset..offset + 4]);
            vec.push(f32::from_le_bytes(bytes));
            offset += 4;
        }
        vectors.push(vec);
    }

    Some(vectors)
}

pub fn cached_model() -> Result<std::sync::RwLockReadGuard<'static, EmbeddingModel>, String> {
    static MODEL: OnceLock<RwLock<EmbeddingModel>> = OnceLock::new();
    let model = MODEL.get_or_init(|| {
        EmbeddingModel::load_or_download()
            .map(RwLock::new)
            .expect("Failed to load embedding model. Check HuggingFace cache and network connectivity.")
    });
    model.read().map_err(|_| "RwLock poisoned".to_string())
}

pub fn warmup_cache() {
    let candidates: Vec<SkillItem> = list_embedded_skills()
        .iter()
        .filter_map(|path| {
            get_embedded_skill(path).map(|content| SkillItem {
                name: path.split('/').next().unwrap_or(path).to_string(),
                relative_path: path.clone(),
                source: SkillSource::Embedded,
                content,
            })
        })
        .collect();

    // 1. Precomputed cache (embedded in binary) — instant, no model needed
    if let Some(cached) = load_precomputed_cache() {
        if let Ok(mut guard) = PASSAGE_CACHE.get_or_init(|| Mutex::new(Vec::new())).lock() {
            *guard = cached;
        }
        mark_warmup_complete();
        info!("Loaded precomputed passage cache ({} skills).", candidates.len());
        eager_load_models();
        return;
    }

    // 2. On-disk cache (~/.agent-guidance/vectors.bin)
    if let Some(cached) = load_passage_cache(&candidates) {
        if let Ok(mut guard) = PASSAGE_CACHE.get_or_init(|| Mutex::new(Vec::new())).lock() {
            *guard = cached;
        }
        mark_warmup_complete();
        info!("Loaded passage cache from disk ({} skills).", candidates.len());
        eager_load_models();
        return;
    }

    // 3. Compute from model (cache miss — first start after skill changes)
    info!("Computing passage embeddings for {} skills...", candidates.len());
    let cache = PASSAGE_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let texts: Vec<String> = candidates.iter().map(|c| {
        format!("{} {}", c.name, c.content.chars().take(300).collect::<String>())
    }).collect();

    let model_guard = cached_model().ok();
    let vecs = match model_guard.as_ref() {
        Some(guard) => {
            texts.par_iter()
                .filter_map(|text| guard.embed_text(text, Some("passage")).ok())
                .collect()
        },
        None => Vec::new(),
    };

    save_passage_cache(&vecs, &candidates);
    if let Ok(mut guard) = cache.lock() {
        *guard = vecs;
    }
    mark_warmup_complete();
    info!("Passage embedding warmup complete ({} skills).", candidates.len());

    // 4. Preload models so first user query doesn't pay OnceLock init
    eager_load_models();
}

/// Eagerly initialize model OnceLocks — moves ~630ms load off first user query.
fn eager_load_models() {
    drop(cached_model());
    drop(cached_cross_encoder());
}

fn embed_skills_cache(candidates: &[SkillItem], model: &EmbeddingModel) -> Vec<Vec<f32>> {
    let cache = PASSAGE_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let guard = cache.lock().unwrap();
    if !guard.is_empty() {
        return guard.clone();
    }
    if !is_warmup_complete() {
        return Vec::new();
    }
    drop(guard);
    let mut vecs = Vec::with_capacity(candidates.len());
    for cand in candidates {
        let text = format!("{} {}", cand.name, cand.content.chars().take(300).collect::<String>());
        if let Ok(v) = model.embed_text(&text, Some("passage")) {
            vecs.push(v);
        }
    }
    if let Ok(mut guard) = cache.lock() {
        *guard = vecs.clone();
    }
    vecs
}

pub fn hybrid_vector_search(query: &str, candidates: &[SkillItem], top_k: usize) -> Vec<(f32, SkillItem)> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let q_lower = query.to_lowercase();
    let words: Vec<&str> = q_lower.split_whitespace().collect();

    let (q_vec, c_vecs) = match cached_model() {
        Ok(model) => {
            let q = model.embed_text(query, Some("query")).ok();
            let c = embed_skills_cache(candidates, &model);
            (q, c)
        }
        _ => (None, Vec::new()),
    };

    let mut scored: Vec<(f32, SkillItem)> = Vec::new();
    if let Some(ref qv) = q_vec {
        if !c_vecs.is_empty() {
            for (i, cand) in candidates.iter().enumerate() {
                let vec_i = if i < c_vecs.len() { &c_vecs[i] } else { continue };
                let mut score = cosine_similarity(qv, vec_i);

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
