use candle_core::Device;
use tracing::info;

pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm1 = 0.0f32;
    let mut norm2 = 0.0f32;
    for (&a, &b) in v1.iter().zip(v2.iter()) {
        dot += a * b;
        norm1 += a * a;
        norm2 += b * b;
    }
    let denom = (norm1 * norm2).sqrt();
    if denom < 1e-9 {
        0.0
    } else {
        dot / denom
    }
}

/// Computes dot-product similarity directly for unit-normalized vectors (norm = 1.0).
#[inline]
pub fn dot_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    v1.iter().zip(v2.iter()).map(|(&a, &b)| a * b).sum()
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
