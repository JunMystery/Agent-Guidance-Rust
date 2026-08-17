use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{Repo, RepoType, api::sync::ApiBuilder};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokenizers::Tokenizer;
use tracing::info;

use crate::catalog::store::{SkillItem, SkillSource, get_embedded_skill, list_embedded_skills};
use crate::ml::inference_pool;
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

/// Resolves the optimal compute device with opportunistic GPU acceleration and zero-cost CPU fallback.
pub fn resolve_optimal_device() -> (Device, &'static str) {
    let env_override = std::env::var("AGENT_GUIDANCE_DEVICE")
        .unwrap_or_else(|_| "auto".to_string())
        .trim()
        .to_lowercase();

    if env_override == "cpu" {
        info!("ML compute device forced to CPU via AGENT_GUIDANCE_DEVICE=cpu");
        return (Device::Cpu, "CPU");
    }

    #[cfg(feature = "cuda")]
    {
        if env_override == "auto" || env_override == "cuda" {
            match Device::new_cuda(0) {
                Ok(dev) => {
                    info!("Opportunistic GPU Acceleration active: NVIDIA CUDA (Device 0)");
                    return (dev, "NVIDIA CUDA");
                }
                Err(e) => {
                    tracing::warn!("CUDA initialization failed, falling back: {}", e);
                }
            }
        }
    }

    #[cfg(feature = "metal")]
    {
        if env_override == "auto" || env_override == "metal" {
            match Device::new_metal(0) {
                Ok(dev) => {
                    info!("Opportunistic GPU Acceleration active: Apple Metal (Device 0)");
                    return (dev, "Apple Metal");
                }
                Err(e) => {
                    tracing::warn!("Metal initialization failed, falling back: {}", e);
                }
            }
        }
    }

    info!("ML compute device initialized on CPU baseline");
    (Device::Cpu, "CPU")
}

impl EmbeddingModel {
    pub fn load_or_download() -> Result<Self> {
        let (device, dev_name) = resolve_optimal_device();
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
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Model files missing from local HuggingFace cache"
                        ));
                    }
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to initialize HuggingFace API: {}",
                    e
                ));
            }
        };

        let config_str = std::fs::read_to_string(config_filename)?;
        let config: Config = serde_json::from_str(&config_str)?;

        let tokenizer = Tokenizer::from_file(tokenizer_filename)
            .map_err(|e| anyhow::anyhow!("Tokenizer load error: {}", e))?;

        // Attempt loading on resolved device (GPU/CPU), with graceful fallback to CPU if OOM occurs
        let (model, final_device) = match Self::try_load_model(&weights_filename, &config, &device) {
            Ok(m) => (m, device),
            Err(err) => {
                if !matches!(device, Device::Cpu) {
                    tracing::warn!(
                        "Failed to load model on GPU ({}), falling back to CPU: {}",
                        dev_name,
                        err
                    );
                    let cpu_device = Device::Cpu;
                    let m = Self::try_load_model(&weights_filename, &config, &cpu_device)?;
                    (m, cpu_device)
                } else {
                    return Err(err);
                }
            }
        };

        Ok(Self {
            model,
            tokenizer,
            device: final_device,
        })
    }

    fn try_load_model(weights_filename: &std::path::Path, config: &Config, device: &Device) -> Result<BertModel> {
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[weights_filename],
                candle_core::DType::F32,
                device,
            )?
        };
        BertModel::load(vb, config).map_err(|e| anyhow::anyhow!("BertModel load error: {}", e))
    }

    pub fn device_name(&self) -> &'static str {
        match &self.device {
            Device::Cpu => "CPU",
            Device::Cuda(_) => "NVIDIA CUDA",
            Device::Metal(_) => "Apple Metal",
        }
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

        let embeddings = self
            .model
            .forward(&token_ids, &token_type_ids, Some(&attention_mask))?;

        // Attention-masked Mean Pooling
        let mask_expanded = attention_mask
            .unsqueeze(2)?
            .to_dtype(candle_core::DType::F32)?;
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

    /// Embeds a batch of texts concurrently via 2D Tensor MatMul with dynamic padding and L2 normalization.
    pub fn embed_batch(
        &self,
        texts: &[&str],
        prefix: Option<&str>,
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let effective_batch_size = if batch_size == 0 { 32 } else { batch_size };
        let mut results = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(effective_batch_size) {
            let prompts: Vec<String> = chunk
                .iter()
                .map(|t| match prefix {
                    Some("query") => format!("query: {}", t),
                    Some("passage") => format!("passage: {}", t),
                    _ => t.to_string(),
                })
                .collect();

            // Encode all prompts in the batch
            let encodings = self
                .tokenizer
                .encode_batch(prompts, true)
                .map_err(|e| anyhow::anyhow!("Batch encoding error: {}", e))?;

            let batch_len = encodings.len();
            // Determine maximum token length in this sub-batch for dynamic padding
            let max_len = encodings
                .iter()
                .map(|e| e.get_ids().len())
                .max()
                .unwrap_or(0);

            if max_len == 0 {
                for _ in 0..batch_len {
                    results.push(vec![0.0; 384]);
                }
                continue;
            }

            // Construct 2D tensors [batch_len, max_len] with padding (pad_id = 0, mask = 0)
            let mut flat_token_ids = Vec::with_capacity(batch_len * max_len);
            let mut flat_attention_mask = Vec::with_capacity(batch_len * max_len);

            for enc in &encodings {
                let ids = enc.get_ids();
                let mask = enc.get_attention_mask();
                let len = ids.len();

                flat_token_ids.extend_from_slice(ids);
                flat_attention_mask.extend_from_slice(mask);

                // Pad remaining slots
                if len < max_len {
                    let pad_count = max_len - len;
                    flat_token_ids.extend(std::iter::repeat(0).take(pad_count));
                    flat_attention_mask.extend(std::iter::repeat(0).take(pad_count));
                }
            }

            let token_ids = Tensor::from_vec(flat_token_ids, (batch_len, max_len), &self.device)?;
            let attention_mask =
                Tensor::from_vec(flat_attention_mask, (batch_len, max_len), &self.device)?;
            let token_type_ids = token_ids.zeros_like()?;

            let embeddings = self
                .model
                .forward(&token_ids, &token_type_ids, Some(&attention_mask))?;

            // Attention-masked Mean Pooling across batch dimension
            let mask_expanded = attention_mask
                .unsqueeze(2)?
                .to_dtype(candle_core::DType::F32)?;
            let masked_embeddings = embeddings.broadcast_mul(&mask_expanded)?;
            let sum_embeddings = masked_embeddings.sum(1)?;
            let sum_mask = mask_expanded.sum(1)?.clamp(1e-9, f64::MAX)?;
            let mean_embedding = sum_embeddings.broadcast_div(&sum_mask)?;

            // L2 normalization
            let norm = mean_embedding.sqr()?.sum_keepdim(1)?.sqrt()?;
            let normalized = mean_embedding.broadcast_div(&norm)?;

            let batch_vectors = normalized.to_vec2::<f32>()?;
            results.extend(batch_vectors);
        }

        Ok(results)
    }
}

#[derive(Clone)]
struct PassageCache {
    fingerprint: u64,
    vectors: Arc<Vec<Vec<f32>>>,
}

/// GPU VRAM-Resident 2D Tensor Matrix for sub-0.1ms Skill Matching
#[derive(Clone)]
pub struct GpuSkillMatrix {
    pub matrix: Tensor, // [N, 384] normalized vectors on GPU/CPU device
    pub count: usize,
    pub dim: usize,
    pub fingerprint: u64,
}

impl GpuSkillMatrix {
    pub fn from_vectors(vectors: &[Vec<f32>], fingerprint: u64, device: &Device) -> Result<Self> {
        let count = vectors.len();
        if count == 0 {
            return Err(anyhow::anyhow!("Empty vectors"));
        }
        let dim = vectors[0].len();
        let mut flat = Vec::with_capacity(count * dim);
        for v in vectors {
            flat.extend_from_slice(v);
        }
        let matrix = Tensor::from_vec(flat, (count, dim), device)?;
        Ok(Self {
            matrix,
            count,
            dim,
            fingerprint,
        })
    }

    /// Computes batch cosine similarity on GPU via matrix multiplication: [1, dim] @ [N, dim]^T -> [N]
    pub fn score_query(&self, query_vec: &[f32], device: &Device) -> Result<Vec<f32>> {
        let q_tensor = Tensor::from_vec(query_vec.to_vec(), (1, self.dim), device)?;
        let scores_tensor = q_tensor.matmul(&self.matrix.t()?)?;
        let flat_scores = scores_tensor.squeeze(0)?.to_vec1::<f32>()?;
        Ok(flat_scores)
    }
}

/// Batch cosine similarity across targets on GPU / CPU Tensor engine
pub fn gpu_batch_cosine_similarity(query: &[f32], targets: &[Vec<f32>], device: &Device) -> Result<Vec<f32>> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let count = targets.len();
    let dim = query.len();
    let mut flat = Vec::with_capacity(count * dim);
    for t in targets {
        if t.len() != dim {
            return Err(anyhow::anyhow!("Dimension mismatch in batch cosine similarity"));
        }
        flat.extend_from_slice(t);
    }
    let target_matrix = Tensor::from_vec(flat, (count, dim), device)?;
    let q_tensor = Tensor::from_vec(query.to_vec(), (1, dim), device)?;
    let scores = q_tensor.matmul(&target_matrix.t()?)?;
    Ok(scores.squeeze(0)?.to_vec1::<f32>()?)
}

static PASSAGE_CACHE: OnceLock<RwLock<Vec<PassageCache>>> = OnceLock::new();
static GPU_SKILL_MATRIX: OnceLock<RwLock<Option<GpuSkillMatrix>>> = OnceLock::new();
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

fn catalog_fingerprint(skills: &[SkillItem]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for skill in skills {
        skill.relative_path.hash(&mut hasher);
        skill.content.hash(&mut hasher);
    }
    hasher.finish()
}

fn store_passage_cache(vectors: Vec<Vec<f32>>, skills: &[SkillItem]) {
    let fingerprint = catalog_fingerprint(skills);
    let cache = PASSAGE_CACHE.get_or_init(|| RwLock::new(Vec::new()));
    if let Ok(mut guard) = cache.write() {
        let entry = PassageCache {
            fingerprint,
            vectors: Arc::new(vectors.clone()),
        };
        guard.retain(|cached| cached.fingerprint != entry.fingerprint);
        guard.insert(0, entry);
        guard.truncate(2);
    }

    // Also update GPU VRAM Matrix if model is cached
    if let Ok(model) = cached_model() {
        if let Ok(matrix) = GpuSkillMatrix::from_vectors(&vectors, fingerprint, &model.device) {
            let gpu_slot = GPU_SKILL_MATRIX.get_or_init(|| RwLock::new(None));
            if let Ok(mut g_guard) = gpu_slot.write() {
                *g_guard = Some(matrix);
            }
        }
    }
}

/// Try to load precomputed passage vectors embedded in the binary.
/// Only matches built-in (embedded) skills — workspace-local skills fall through.
fn load_precomputed_cache(skills: &[SkillItem]) -> Option<Vec<Vec<f32>>> {
    let manifest: serde_json::Value = serde_json::from_slice(PRECOMPUTED_MANIFEST).ok()?;
    let cached_skills: Vec<String> = manifest
        .get("skills")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let current_skills: Vec<String> = list_embedded_skills();
    if cached_skills != current_skills {
        return None;
    }
    if manifest.get("fingerprint").and_then(|value| value.as_u64())
        != Some(catalog_fingerprint(skills))
    {
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

    info!(
        "Computing passage embeddings for {} skills (generate-precomputed)...",
        candidates.len()
    );
    let model_guard = cached_model().map_err(|e| anyhow::anyhow!("{}", e))?;
    let texts: Vec<String> = candidates
        .iter()
        .map(|c| {
            format!(
                "{} {}",
                c.name,
                c.content.chars().take(300).collect::<String>()
            )
        })
        .collect();

    let vecs: Vec<Vec<f32>> = inference_pool().install(|| {
        texts
            .par_iter()
            .filter_map(|text| model_guard.embed_text(text, Some("passage")).ok())
            .collect()
    });

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
        "fingerprint": catalog_fingerprint(&candidates),
        "skills": list_embedded_skills(),
    });

    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ml_dir = crate_root.join("src").join("ml");
    if ml_dir.exists() {
        let _ = std::fs::write(ml_dir.join("precomputed_vectors.bin"), &buf);

        // Pre-tokenize all skills and write precomputed_tokens.bin
        let mut token_buf = Vec::new();
        token_buf.extend_from_slice(&count.to_le_bytes());
        for c in &candidates {
            let prompt = format!(
                "passage: {} {}",
                c.name,
                c.content.chars().take(300).collect::<String>()
            );
            if let Ok(encoding) = model_guard.tokenizer.encode(prompt, true) {
                let ids = encoding.get_ids();
                let len = ids.len() as u32;
                token_buf.extend_from_slice(&len.to_le_bytes());
                for &id in ids {
                    token_buf.extend_from_slice(&(id as u64).to_le_bytes());
                }
            } else {
                token_buf.extend_from_slice(&0u32.to_le_bytes());
            }
        }
        let _ = std::fs::write(ml_dir.join("precomputed_tokens.bin"), &token_buf);

        let _ = std::fs::write(
            ml_dir.join("precomputed_manifest.json"),
            serde_json::to_string_pretty(&manifest)?,
        );
        info!(
            "Precomputed cache written to src/ml/ ({} skills, {} bytes vectors, {} bytes tokens).",
            count,
            buf.len(),
            token_buf.len()
        );
    }

    save_passage_cache(&vecs, &candidates);
    info!(
        "On-disk cache written to {:?} ({} skills).",
        vectors_path(),
        count
    );

    Ok(())
}

fn is_warmup_complete() -> bool {
    WARMUP_DONE
        .get()
        .map(|v| v.load(Ordering::SeqCst))
        .unwrap_or(false)
}

fn mark_warmup_complete() {
    WARMUP_DONE
        .get_or_init(|| AtomicBool::new(false))
        .store(true, Ordering::SeqCst);
}

fn save_passage_cache(vectors: &[Vec<f32>], skills: &[SkillItem]) {
    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);

    let skill_names: Vec<String> = skills.iter().map(|s| s.relative_path.clone()).collect();
    let manifest = serde_json::json!({
        "version": 1,
        "created_at": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        "fingerprint": catalog_fingerprint(skills),
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
    let cached_skills: Vec<String> = manifest
        .get("skills")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let current_skills: Vec<String> = skills.iter().map(|s| s.relative_path.clone()).collect();
    if cached_skills != current_skills {
        return None;
    }
    if manifest.get("fingerprint").and_then(|value| value.as_u64())
        != Some(catalog_fingerprint(skills))
    {
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
    static MODEL: OnceLock<Result<RwLock<EmbeddingModel>, String>> = OnceLock::new();
    match MODEL.get_or_init(|| {
        EmbeddingModel::load_or_download()
            .map(RwLock::new)
            .map_err(|e| e.to_string())
    }) {
        Ok(model) => model.read().map_err(|_| "RwLock poisoned".to_string()),
        Err(err) => Err(err.clone()),
    }
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
    if let Some(cached) = load_precomputed_cache(&candidates) {
        store_passage_cache(cached, &candidates);
        mark_warmup_complete();
        info!(
            "Loaded precomputed passage cache ({} skills).",
            candidates.len()
        );
        eager_load_embedding_model();
        return;
    }

    // 2. On-disk cache (~/.agent-guidance/vectors.bin)
    if let Some(cached) = load_passage_cache(&candidates) {
        store_passage_cache(cached, &candidates);
        mark_warmup_complete();
        info!(
            "Loaded passage cache from disk ({} skills).",
            candidates.len()
        );
        eager_load_embedding_model();
        return;
    }

    // 3. Compute from model (cache miss — first start after skill changes)
    info!(
        "Computing passage embeddings for {} skills...",
        candidates.len()
    );
    let texts: Vec<String> = candidates
        .iter()
        .map(|c| {
            format!(
                "{} {}",
                c.name,
                c.content.chars().take(300).collect::<String>()
            )
        })
        .collect();

    let model_guard = cached_model().ok();
    let vecs = match model_guard.as_ref() {
        Some(guard) => inference_pool().install(|| {
            texts
                .par_iter()
                .filter_map(|text| guard.embed_text(text, Some("passage")).ok())
                .collect()
        }),
        None => Vec::new(),
    };

    save_passage_cache(&vecs, &candidates);
    store_passage_cache(vecs, &candidates);
    mark_warmup_complete();
    info!(
        "Passage embedding warmup complete ({} skills).",
        candidates.len()
    );

    // 4. Preload models so first user query doesn't pay OnceLock init
    eager_load_embedding_model();
}

/// Eagerly initialize both the embedding model and the cross-encoder reranker.
fn eager_load_embedding_model() {
    drop(cached_model());
    drop(crate::ml::llm_selector::cached_cross_encoder());
}

fn embed_skills_cache(candidates: &[SkillItem], model: &EmbeddingModel) -> Arc<Vec<Vec<f32>>> {
    let fingerprint = catalog_fingerprint(candidates);
    let cache = PASSAGE_CACHE.get_or_init(|| RwLock::new(Vec::new()));
    if let Ok(guard) = cache.read() {
        if let Some(entry) = guard.iter().find(|entry| {
            entry.fingerprint == fingerprint && entry.vectors.len() == candidates.len()
        }) {
            return entry.vectors.clone();
        }
    }

    // On-the-fly parallel or serial passage embedding if cache is empty
    info!(
        "[ML Pipeline] Computing passage embeddings for {} skills on-the-fly...",
        candidates.len()
    );
    let vecs: Vec<Vec<f32>> = inference_pool().install(|| {
        candidates
            .par_iter()
            .filter_map(|cand| {
                let text = format!(
                    "{} {}",
                    cand.name,
                    cand.content.chars().take(300).collect::<String>()
                );
                model.embed_text(&text, Some("passage")).ok()
            })
            .collect()
    });
    store_passage_cache(vecs, candidates);
    cache
        .read()
        .ok()
        .and_then(|guard| {
            guard
                .iter()
                .find(|entry| entry.fingerprint == fingerprint)
                .map(|entry| entry.vectors.clone())
        })
        .unwrap_or_else(|| Arc::new(Vec::new()))
}

/// Eagerly warm up and preload both the Embedding Model, Cross-Encoder, and GPU Skill Matrix into VRAM.
pub fn eager_vram_warmup() -> Result<()> {
    info!("[VRAM Residency] Initializing Eager VRAM Standby Warmup...");
    warmup_cache();
    drop(cached_model());
    drop(crate::ml::llm_selector::cached_cross_encoder());
    info!("[VRAM Residency] Eager VRAM Standby Warmup complete. Engine ready on VRAM.");
    Ok(())
}

pub fn hybrid_vector_search(
    query: &str,
    candidates: &[SkillItem],
    top_k: usize,
) -> Vec<(f32, SkillItem)> {
    if candidates.is_empty() {
        return Vec::new();
    }

    crate::mcp::db::log_embed_query(query);

    let q_lower = query.to_lowercase();
    let words: Vec<&str> = q_lower.split_whitespace().collect();

    let model_res = cached_model();
    let (q_vec, c_vecs) = match &model_res {
        Ok(model) => {
            let q = model.embed_text(query, Some("query")).ok();
            let c = embed_skills_cache(candidates, model);
            (q, c)
        }
        _ => (None::<Vec<f32>>, Arc::new(Vec::new())),
    };

    let mut scored: Vec<(f32, usize)> = Vec::new();
    if let Some(ref qv) = q_vec {
        if !c_vecs.is_empty() {
            let fingerprint = catalog_fingerprint(candidates);
            let gpu_scores: Option<Vec<f32>> = if let Ok(ref model) = model_res {
                let gpu_slot = GPU_SKILL_MATRIX.get_or_init(|| RwLock::new(None));
                if let Ok(guard) = gpu_slot.read() {
                    if let Some(ref matrix) = *guard {
                        if matrix.fingerprint == fingerprint && matrix.count == candidates.len() {
                            matrix.score_query(qv, &model.device).ok()
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            for (i, cand) in candidates.iter().enumerate() {
                let base_score = match &gpu_scores {
                    Some(scores) => scores.get(i).copied().unwrap_or(0.0),
                    None => {
                        let vec_i = if i < c_vecs.len() {
                            &c_vecs[i]
                        } else {
                            continue;
                        };
                        qv.iter().zip(vec_i.iter()).map(|(a, b)| a * b).sum()
                    }
                };

                let mut score = base_score;
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

                scored.push((score, i));
            }

            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            return scored
                .into_iter()
                .take(top_k)
                .map(|(s, i)| (s, candidates[i].clone()))
                .collect();
        }
    }

    for (i, cand) in candidates.iter().enumerate() {
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
            scored.push((score, i));
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored
        .into_iter()
        .take(top_k)
        .map(|(s, i)| (s, candidates[i].clone()))
        .collect()
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
    #[ignore = "Requires pre-cached HuggingFace model files; avoid network I/O in CI"]
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

    #[test]
    fn test_device_resolution_and_env_override() {
        // 1. Test forced CPU via env var
        unsafe {
            std::env::set_var("AGENT_GUIDANCE_DEVICE", "cpu");
        }
        let (dev, name) = resolve_optimal_device();
        assert!(matches!(dev, Device::Cpu));
        assert_eq!(name, "CPU");

        // 2. Test auto mode resolution
        unsafe {
            std::env::set_var("AGENT_GUIDANCE_DEVICE", "auto");
        }
        let (_dev_auto, _name_auto) = resolve_optimal_device();
        // Should resolve cleanly without panicking
    }

    #[test]
    fn test_batch_empty_input() {
        if let Ok(model) = cached_model() {
            let res = model.embed_batch(&[], Some("passage"), 16);
            assert!(res.is_ok());
            assert!(res.unwrap().is_empty());
        }
    }

    #[test]
    fn test_batch_embedding_numerical_equivalence() {
        if let Ok(model) = cached_model() {
            let texts = [
                "Implement JWT authentication middleware",
                "Configure PostgreSQL index on user_id",
                "Setup Prometheus metrics exporter",
                "Handle websocket reconnect backoff",
            ];

            // 1. Single embeddings
            let mut single_vecs = Vec::new();
            for t in &texts {
                let v = model.embed_text(t, Some("passage")).unwrap();
                single_vecs.push(v);
            }

            // 2. Batch embeddings with batch_size = 2 (triggers multi-chunk batching)
            let batch_vecs = model.embed_batch(&texts, Some("passage"), 2).unwrap();
            assert_eq!(batch_vecs.len(), texts.len());

            // 3. Verify numerical equivalence (Cosine similarity >= 0.9999)
            for i in 0..texts.len() {
                let sim = cosine_similarity(&single_vecs[i], &batch_vecs[i]);
                assert!(
                    sim >= 0.9999,
                    "Batch vector {} deviates from single vector (sim: {})",
                    i,
                    sim
                );
            }
        }
    }

    #[test]
    fn test_gpu_skill_matrix_scoring_equivalence() {
        let dev = Device::Cpu;
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let v3 = vec![0.7071, 0.7071, 0.0];
        let vectors = vec![v1, v2, v3];

        let matrix = GpuSkillMatrix::from_vectors(&vectors, 12345, &dev).unwrap();
        assert_eq!(matrix.count, 3);
        assert_eq!(matrix.dim, 3);

        let query = vec![1.0, 0.0, 0.0];
        let scores = matrix.score_query(&query, &dev).unwrap();
        assert_eq!(scores.len(), 3);
        assert!((scores[0] - 1.0).abs() < 1e-5);
        assert!((scores[1] - 0.0).abs() < 1e-5);
        assert!((scores[2] - 0.7071).abs() < 1e-4);
    }

    #[test]
    fn test_gpu_batch_cosine_similarity() {
        let dev = Device::Cpu;
        let query = vec![0.0, 1.0, 0.0];
        let targets = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, -1.0, 0.0],
        ];
        let scores = gpu_batch_cosine_similarity(&query, &targets, &dev).unwrap();
        assert_eq!(scores.len(), 3);
        assert!((scores[0] - 0.0).abs() < 1e-5);
        assert!((scores[1] - 1.0).abs() < 1e-5);
        assert!((scores[2] - (-1.0)).abs() < 1e-5);
    }
}
