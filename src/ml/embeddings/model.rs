use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{Cache, Repo, RepoType, api::sync::ApiBuilder};
use tokenizers::Tokenizer;
use tracing::info;

use super::device::resolve_optimal_device;

pub(crate) fn resolve_repo_paths(
    model_id: &str,
    files: &[&str],
) -> Result<Vec<std::path::PathBuf>> {
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
    let api = ApiBuilder::new().with_progress(false).build()?;
    let remote = api.repo(repo);
    let mut remote_paths = Vec::with_capacity(files.len());
    for f in files {
        remote_paths.push(remote.get(f)?);
    }
    Ok(remote_paths)
}

pub struct EmbeddingModel {
    pub(crate) model: BertModel,
    pub(crate) tokenizer: Tokenizer,
    pub(crate) device: Device,
}

impl EmbeddingModel {
    pub fn load_or_download() -> Result<Self> {
        let (device, dev_name) = resolve_optimal_device();
        let paths = resolve_repo_paths(
            "intfloat/multilingual-e5-small",
            &["config.json", "tokenizer.json", "model.safetensors"],
        )?;
        let config_filename = &paths[0];
        let tokenizer_filename = &paths[1];
        let weights_filename = &paths[2];

        let config: Config = serde_json::from_str(
            &std::fs::read_to_string(config_filename)
            .map_err(|e| anyhow::anyhow!("Failed to read model config: {}", e))?,
        )
        .map_err(|e| anyhow::anyhow!("Failed to parse model config JSON: {}", e))?;

        let tokenizer = Tokenizer::from_file(tokenizer_filename)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        let model = Self::try_load_model(weights_filename, &config, &device)
            .map_err(|e| anyhow::anyhow!("Failed to load BertModel: {}", e))?;

        info!("EmbeddingModel loaded successfully on {}.", dev_name);
        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    fn try_load_model(
        weights_filename: &std::path::Path,
        config: &Config,
        device: &Device,
    ) -> Result<BertModel> {
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_filename], candle_core::DType::F32, device)?
        };
        let model = BertModel::load(vb, config)?;
        Ok(model)
    }

    pub fn device_name(&self) -> &'static str {
        match self.device {
            Device::Cpu => "CPU",
            Device::Cuda(_) => "NVIDIA CUDA",
            Device::Metal(_) => "Apple Metal",
        }
    }

    pub fn embed_text(&self, text: &str, prefix: Option<&str>) -> Result<Vec<f32>> {
        let formatted = match prefix {
            Some("query") => format!("query: {}", text),
            Some("passage") => format!("passage: {}", text),
            _ => text.to_string(),
        };

        let mut tokenizer = self.tokenizer.clone();
        tokenizer
            .with_padding(None)
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: 512,
                ..Default::default()
            }))
            .map_err(|e| anyhow::anyhow!("Tokenizer truncation error: {}", e))?;

        let encoding = tokenizer
            .encode(formatted, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();

        let input_ids = Tensor::new(input_ids, &self.device)?.unsqueeze(0)?;
        let token_type_ids = input_ids.zeros_like()?;
        let attention_mask_tensor = Tensor::new(attention_mask, &self.device)?.unsqueeze(0)?;

        let embeddings = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask_tensor))?;

        // Mean pooling over token dimension
        let mask_f32 = attention_mask_tensor.to_dtype(candle_core::DType::F32)?;
        let mask_expanded = mask_f32.unsqueeze(2)?;
        let sum_embeddings = embeddings.broadcast_mul(&mask_expanded)?.sum(1)?;
        let sum_mask = mask_f32.sum(1)?.unsqueeze(1)?;
        let mean_embedding = sum_embeddings.broadcast_div(&sum_mask)?;

        // L2 normalize
        let norm = mean_embedding.sqr()?.sum_keepdim(1)?.sqrt()?;
        let normalized = mean_embedding.broadcast_div(&norm)?;

        let vector = normalized.squeeze(0)?.to_vec1::<f32>()?;
        Ok(vector)
    }

    pub fn embed_batch(
        &self,
        texts: &[&str],
        prefix: Option<&str>,
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let chunk_size = if batch_size == 0 { 32 } else { batch_size };
        let mut results = Vec::with_capacity(texts.len());

        let mut tokenizer = self.tokenizer.clone();
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: 512,
                ..Default::default()
            }))
            .map_err(|e| anyhow::anyhow!("Tokenizer truncation config error: {}", e))?;

        for chunk in texts.chunks(chunk_size) {
            let formatted_chunk: Vec<String> = chunk
                .iter()
                .map(|t| match prefix {
                    Some("query") => format!("query: {}", t),
                    Some("passage") => format!("passage: {}", t),
                    _ => t.to_string(),
                })
                .collect();

            let encodings = tokenizer
                .encode_batch(formatted_chunk, true)
                .map_err(|e| anyhow::anyhow!("Batch tokenization failed: {}", e))?;

            let b_size = encodings.len();
            let max_len = encodings.iter().map(|e| e.get_ids().len()).max().unwrap_or(0);

            if max_len == 0 {
                continue;
            }

            let mut batch_input_ids = vec![0u32; b_size * max_len];
            let mut batch_attention_mask = vec![0u32; b_size * max_len];

            for (i, enc) in encodings.iter().enumerate() {
                let ids = enc.get_ids();
                let mask = enc.get_attention_mask();
                let len = ids.len();
                let start = i * max_len;

                batch_input_ids[start..start + len].copy_from_slice(ids);
                batch_attention_mask[start..start + len].copy_from_slice(mask);
            }

            let input_ids = Tensor::from_vec(batch_input_ids, (b_size, max_len), &self.device)?;
            let attention_mask = Tensor::from_vec(batch_attention_mask, (b_size, max_len), &self.device)?;
            let token_type_ids = input_ids.zeros_like()?;

            let embeddings = self
                .model
                .forward(&input_ids, &token_type_ids, Some(&attention_mask))?;

            // Mean pooling
            let mask_f32 = attention_mask.to_dtype(candle_core::DType::F32)?;
            let mask_expanded = mask_f32.unsqueeze(2)?;
            let sum_embeddings = embeddings.broadcast_mul(&mask_expanded)?.sum(1)?;
            let sum_mask = mask_f32.sum(1)?.unsqueeze(1)?;
            let mean_embedding = sum_embeddings.broadcast_div(&sum_mask)?;

            // L2 normalize
            let norm = mean_embedding.sqr()?.sum_keepdim(1)?.sqrt()?;
            let normalized = mean_embedding.broadcast_div(&norm)?;

            let batch_vectors = normalized.to_vec2::<f32>()?;
            results.extend(batch_vectors);
        }

        Ok(results)
    }
}
