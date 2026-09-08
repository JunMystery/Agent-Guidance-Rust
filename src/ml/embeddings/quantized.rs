//! Int8 Quantized ONNX Embedding Model Engine
//!
//! Provides ultra-fast embedding inference via ONNX Runtime with Int8 quantization,
//! supporting DirectML, CUDA, and CPU execution providers with dynamic batching.

use anyhow::Result;
use std::path::Path;
use std::sync::Mutex;
use tokenizers::Tokenizer;
use tracing::info;

use super::providers::{ExecutionProvider, configure_session_builder, detect_optimal_provider};

pub struct OnnxQuantizedModel {
    session: Mutex<ort::session::Session>,
    tokenizer: Tokenizer,
    pub provider: ExecutionProvider,
    pub is_quantized: bool,
    pub model_name: String,
}

impl OnnxQuantizedModel {
    pub fn load_from_dir(model_dir: &Path) -> Result<Self> {
        let (onnx_path, is_quantized) = Self::discover_model_file(model_dir)?;
        let tokenizer_path = model_dir.join("tokenizer.json");

        if !tokenizer_path.exists() {
            anyhow::bail!("Tokenizer missing at {:?}", tokenizer_path);
        }

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Tokenizer load error: {}", e))?;

        let (provider, prov_name) = detect_optimal_provider();
        let builder = ort::session::Session::builder()
            .map_err(|e| anyhow::anyhow!("Session builder error: {}", e))?;
        let mut configured = configure_session_builder(builder, provider)?;

        let session = configured
            .commit_from_file(&onnx_path)
            .map_err(|e| anyhow::anyhow!("ONNX commit error ({:?}): {}", onnx_path, e))?;

        let model_name = onnx_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("model.onnx")
            .to_string();

        info!(
            "ONNX Quantized Engine initialized with '{}' on {} (Int8: {})",
            model_name, prov_name, is_quantized
        );

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            provider,
            is_quantized,
            model_name,
        })
    }

    pub(crate) fn discover_model_file(model_dir: &Path) -> Result<(std::path::PathBuf, bool)> {
        let candidates = [
            ("model_quantized.onnx", true),
            ("model_int8.onnx", true),
            ("bge-small-en-v1.5-q8.onnx", true),
            ("model_q8.onnx", true),
            ("model.onnx", false),
        ];

        for (filename, quantized) in &candidates {
            let path = model_dir.join(filename);
            if path.exists() {
                return Ok((path, *quantized));
            }
        }

        anyhow::bail!("No suitable ONNX model file found in {:?}", model_dir)
    }

    pub fn device_name(&self) -> &'static str {
        self.provider.name()
    }

    pub fn embed_text(&self, text: &str, prefix: Option<&str>) -> Result<Vec<f32>> {
        let batch = self.embed_batch(&[text], prefix, 1)?;
        batch
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty embedding generated"))
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
            .map_err(|e| anyhow::anyhow!("Tokenizer truncation error: {}", e))?;

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
                .map_err(|e| anyhow::anyhow!("Batch tokenization error: {}", e))?;

            let b_size = encodings.len();
            let max_len = encodings.iter().map(|e| e.get_ids().len()).max().unwrap_or(0);
            if max_len == 0 {
                continue;
            }

            let mut batch_ids = vec![0i64; b_size * max_len];
            let mut batch_mask = vec![0i64; b_size * max_len];

            for (i, enc) in encodings.iter().enumerate() {
                let ids = enc.get_ids();
                let mask = enc.get_attention_mask();
                let len = ids.len();
                let start = i * max_len;

                for j in 0..len {
                    batch_ids[start + j] = ids[j] as i64;
                    batch_mask[start + j] = mask[j] as i64;
                }
            }

            let input_ids_tensor =
                ort::value::Value::from_array((vec![b_size, max_len], batch_ids.clone()))?;
            let attention_mask_tensor =
                ort::value::Value::from_array((vec![b_size, max_len], batch_mask.clone()))?;

            let inputs = ort::inputs! {
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
            };

            let mut session_guard = self
                .session
                .lock()
                .map_err(|_| anyhow::anyhow!("Session lock poisoned"))?;
            let outputs = session_guard.run(inputs)?;

            let output_tensor = outputs
                .get("last_hidden_state")
                .or_else(|| outputs.get("sentence_embedding"))
                .ok_or_else(|| anyhow::anyhow!("Output tensor not found in ONNX session"))?;

            let (shape, data) = output_tensor.try_extract_tensor::<f32>()?;
            let pooled = Self::pool_and_normalize(shape, data, &batch_mask, b_size, max_len)?;
            results.extend(pooled);
        }

        Ok(results)
    }

    pub(crate) fn pool_and_normalize(
        shape: &[i64],
        data: &[f32],
        mask: &[i64],
        b_size: usize,
        max_len: usize,
    ) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(b_size);
        if shape.len() == 3 {
            let hidden_dim = shape[2] as usize;
            for i in 0..b_size {
                let mut vec = vec![0.0f32; hidden_dim];
                let mut count = 0.0f32;
                for j in 0..max_len {
                    if mask[i * max_len + j] > 0 {
                        let offset = (i * max_len + j) * hidden_dim;
                        for h in 0..hidden_dim {
                            vec[h] += data[offset + h];
                        }
                        count += 1.0;
                    }
                }
                if count > 0.0 {
                    for h in 0..hidden_dim {
                        vec[h] /= count;
                    }
                }
                Self::normalize_vector(&mut vec);
                results.push(vec);
            }
        } else if shape.len() == 2 {
            let hidden_dim = shape[1] as usize;
            for i in 0..b_size {
                let mut vec = data[i * hidden_dim..(i + 1) * hidden_dim].to_vec();
                Self::normalize_vector(&mut vec);
                results.push(vec);
            }
        } else {
            anyhow::bail!("Unsupported ONNX output tensor shape: {:?}", shape);
        }
        Ok(results)
    }

    fn normalize_vector(vec: &mut [f32]) {
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
        for x in vec.iter_mut() {
            *x /= norm;
        }
    }
}
