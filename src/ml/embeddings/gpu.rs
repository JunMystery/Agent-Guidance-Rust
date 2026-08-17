use anyhow::Result;
use candle_core::{Device, Tensor};
use tracing::info;

use super::cache::{cached_model, warmup_cache};

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

/// Eagerly warm up and preload both the Embedding Model, Cross-Encoder, and GPU Skill Matrix into VRAM.
pub fn eager_vram_warmup() -> Result<()> {
    info!("[VRAM Residency] Initializing Eager VRAM Standby Warmup...");
    warmup_cache();
    drop(cached_model());
    drop(crate::ml::llm_selector::cached_cross_encoder());
    info!("[VRAM Residency] Eager VRAM Standby Warmup complete. Engine ready on VRAM.");
    Ok(())
}
