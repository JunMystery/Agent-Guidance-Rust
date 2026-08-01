//! ONNX Runtime Engine Abstraction with Fallback Support
//!
//! Provides ultra-low-latency ML inference using `ort` (ONNX Runtime)
//! for Stage 1 text embeddings (`multilingual-e5-small`) and Stage 2
//! cross-encoder reranking (`ms-marco-MiniLM-L-6-v2`).

use anyhow::Result;
use std::path::Path;
use tokenizers::Tokenizer;
use tracing::info;

pub struct OnnxEmbeddingModel {
    session: ort::session::Session,
    tokenizer: Tokenizer,
}

impl OnnxEmbeddingModel {
    pub fn load_from_dir(model_dir: &Path) -> Result<Self> {
        let onnx_path = model_dir.join("model.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        if !onnx_path.exists() || !tokenizer_path.exists() {
            anyhow::bail!("ONNX model artifacts missing at {:?}", model_dir);
        }

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Tokenizer load error: {}", e))?;

        let mut builder = ort::session::Session::builder()
            .map_err(|e| anyhow::anyhow!("Session builder error: {}", e))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("Optimization level error: {}", e))?;

        let session = builder
            .commit_from_file(&onnx_path)
            .map_err(|e| anyhow::anyhow!("ONNX model commit error: {}", e))?;

        info!("ONNX Runtime embedding engine initialized from {:?}", onnx_path);
        Ok(Self { session, tokenizer })
    }

    pub fn embed_text(&mut self, text: &str, prefix: Option<&str>) -> Result<Vec<f32>> {
        let prompt = match prefix {
            Some("query") => format!("query: {}", text),
            Some("passage") => format!("passage: {}", text),
            _ => text.to_string(),
        };

        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("Encoding error: {}", e))?;

        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let attention_mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&m| m as i64)
            .collect();
        let seq_len = input_ids.len();

        let input_ids_tensor = ort::value::Value::from_array((vec![1, seq_len], input_ids))?;
        let attention_mask_tensor = ort::value::Value::from_array((vec![1, seq_len], attention_mask))?;

        let inputs = ort::inputs! {
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
        };

        let outputs = self.session.run(inputs)?;

        let output_tensor = outputs
            .get("last_hidden_state")
            .or_else(|| outputs.get("sentence_embedding"))
            .ok_or_else(|| anyhow::anyhow!("Could not find output tensor in ONNX session"))?;

        let (shape, data) = output_tensor.try_extract_tensor::<f32>()?;

        // Mean pooling over sequence dimension if last_hidden_state [1, seq_len, hidden_dim]
        if shape.len() == 3 {
            let hidden_dim = shape[2] as usize;
            let mut vec = vec![0.0f32; hidden_dim];
            let mut count = 0.0f32;

            for i in 0..seq_len {
                for h in 0..hidden_dim {
                    vec[h] += data[i * hidden_dim + h];
                }
                count += 1.0;
            }

            if count > 0.0 {
                for h in 0..hidden_dim {
                    vec[h] /= count;
                }
            }

            // L2 Normalization
            let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
            for x in &mut vec {
                *x /= norm;
            }

            Ok(vec)
        } else if shape.len() == 2 {
            // Already pooled sentence_embedding [1, hidden_dim]
            let mut vec = data.to_vec();
            let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
            for x in &mut vec {
                *x /= norm;
            }
            Ok(vec)
        } else {
            anyhow::bail!("Unexpected ONNX tensor shape: {:?}", shape);
        }
    }
}
