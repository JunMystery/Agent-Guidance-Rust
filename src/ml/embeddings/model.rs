//! Unified Embedding Model Coordinator
//!
//! Opportunistically instantiates the accelerated Int8 ONNX Runtime engine
//! (with DirectML, CUDA, or CPU SIMD) and seamlessly falls back to Candle BERT.

use anyhow::Result;
use candle_core::Device;
use hf_hub::{Cache, Repo, RepoType};
use std::path::PathBuf;
use tracing::info;

use super::backend::EmbeddingBackend;
use super::candle_bert::CandleBertInner;
use super::quantized::OnnxQuantizedModel;

pub(crate) fn resolve_repo_paths(model_id: &str, files: &[&str]) -> Result<Vec<PathBuf>> {
    let repo = Repo::new(model_id.to_string(), RepoType::Model);
    let cache_repo = Cache::from_env().repo(repo.clone());
    let mut paths = Vec::with_capacity(files.len());
    let mut all_cached = true;
    for f in files {
        match cache_repo.get(f) {
            Some(p) => paths.push(p),
            None => {
                all_cached = false;
                break;
            }
        }
    }
    if all_cached {
        return Ok(paths);
    }
    anyhow::bail!(
        "Model '{}' not locally cached in ~/.cache/huggingface. Using rich embedded manifest.",
        model_id
    )
}

pub struct EmbeddingModel {
    pub backend: EmbeddingBackend,
    pub device: Device,
}

impl EmbeddingModel {
    pub fn load_or_download() -> Result<Self> {
        // 1. Opportunistic ONNX Int8 quantized vector acceleration
        if let Some(onnx) = try_load_onnx_quantized() {
            info!(
                "EmbeddingModel active with ONNX Int8 acceleration on {}",
                onnx.device_name()
            );
            return Ok(Self {
                backend: EmbeddingBackend::OnnxInt8(Box::new(onnx)),
                device: Device::Cpu,
            });
        }

        // 2. Pure-Rust Candle BERT baseline fallback
        let inner = CandleBertInner::load_or_download()?;
        let device = inner.device.clone();
        Ok(Self {
            backend: EmbeddingBackend::CandleBert(inner),
            device,
        })
    }

    pub fn device_name(&self) -> &'static str {
        self.backend.device_name()
    }

    pub fn is_quantized(&self) -> bool {
        self.backend.is_quantized()
    }

    pub fn embed_text(&self, text: &str, prefix: Option<&str>) -> Result<Vec<f32>> {
        self.backend.embed_text(text, prefix)
    }

    pub fn embed_batch(
        &self,
        texts: &[&str],
        prefix: Option<&str>,
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        self.backend.embed_batch(texts, prefix, batch_size)
    }
}

fn try_load_onnx_quantized() -> Option<OnnxQuantizedModel> {
    // 1. Check custom path via environment variable
    if let Ok(env_path) = std::env::var("AGENT_GUIDANCE_ONNX_PATH") {
        let p = PathBuf::from(env_path.trim());
        if let Ok(model) = OnnxQuantizedModel::load_from_dir(&p) {
            return Some(model);
        }
    }

    // 2. Check local data directory (~/.agent-guidance/models/)
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".agent-guidance").join("models");
        if let Ok(model) = OnnxQuantizedModel::load_from_dir(&p) {
            return Some(model);
        }
    }

    // 3. Check HuggingFace hub cache for ONNX exports
    let repo_names = ["BAAI/bge-small-en-v1.5", "intfloat/multilingual-e5-small"];
    for repo_name in &repo_names {
        let repo = Repo::new(repo_name.to_string(), RepoType::Model);
        let cache_repo = Cache::from_env().repo(repo);
        if let (Some(onnx), Some(_tok)) = (
            cache_repo
                .get("model_quantized.onnx")
                .or_else(|| cache_repo.get("model_int8.onnx"))
                .or_else(|| cache_repo.get("model.onnx")),
            cache_repo.get("tokenizer.json"),
        ) {
            if let Some(parent) = onnx.parent() {
                if let Ok(model) = OnnxQuantizedModel::load_from_dir(parent) {
                    return Some(model);
                }
            }
        }
    }

    None
}
