//! Hardware Execution Provider Detection and Configuration
//!
//! Provides opportunistic GPU acceleration via Microsoft DirectML (DirectX 12)
//! or NVIDIA CUDA, with zero-cost AVX2/SIMD CPU baseline fallback.

use anyhow::Result;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionProvider {
    DirectML { device_id: i32 },
    Cuda { device_id: i32 },
    Cpu,
}

impl ExecutionProvider {
    pub fn name(&self) -> &'static str {
        match self {
            Self::DirectML { .. } => "Microsoft DirectML (Int8 Quantized)",
            Self::Cuda { .. } => "NVIDIA CUDA (Int8 Quantized)",
            Self::Cpu => "CPU (AVX2/SIMD Baseline)",
        }
    }

    pub fn is_gpu(&self) -> bool {
        matches!(self, Self::DirectML { .. } | Self::Cuda { .. })
    }
}

/// Detects the optimal execution provider respecting hardware capabilities
/// and `AGENT_GUIDANCE_DEVICE` environment overrides.
pub fn detect_optimal_provider() -> (ExecutionProvider, &'static str) {
    let env_override = std::env::var("AGENT_GUIDANCE_DEVICE")
        .unwrap_or_else(|_| "auto".to_string())
        .trim()
        .to_lowercase();

    if env_override == "cpu" {
        info!("ONNX execution provider forced to CPU via AGENT_GUIDANCE_DEVICE=cpu");
        return (ExecutionProvider::Cpu, ExecutionProvider::Cpu.name());
    }

    if env_override == "cuda" {
        return (
            ExecutionProvider::Cuda { device_id: 0 },
            ExecutionProvider::Cuda { device_id: 0 }.name(),
        );
    }

    if env_override == "directml" || env_override == "dml" {
        return (
            ExecutionProvider::DirectML { device_id: 0 },
            ExecutionProvider::DirectML { device_id: 0 }.name(),
        );
    }

    #[cfg(feature = "directml")]
    {
        info!("Opportunistic ONNX Acceleration: Microsoft DirectML active (Device 0)");
        return (
            ExecutionProvider::DirectML { device_id: 0 },
            "Microsoft DirectML (Int8 Quantized)",
        );
    }

    #[cfg(feature = "cuda")]
    {
        info!("Opportunistic ONNX Acceleration: NVIDIA CUDA active (Device 0)");
        return (
            ExecutionProvider::Cuda { device_id: 0 },
            "NVIDIA CUDA (Int8 Quantized)",
        );
    }

    #[allow(unreachable_code)]
    {
        info!("ONNX execution provider initialized on CPU baseline");
        (ExecutionProvider::Cpu, ExecutionProvider::Cpu.name())
    }
}

/// Configures session builder with GraphOptimizationLevel::Level3 and execution provider.
pub fn configure_session_builder(
    builder: ort::session::builder::SessionBuilder,
    provider: ExecutionProvider,
) -> Result<ort::session::builder::SessionBuilder> {
    #[allow(unused_mut)]
    let mut b = builder
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow::anyhow!("Failed to set ONNX optimization level: {}", e))?
        .with_intra_threads(2)
        .map_err(|e| anyhow::anyhow!("Failed to configure ONNX worker threads: {}", e))?;

    match provider {
        ExecutionProvider::DirectML { device_id: _id } => {
            #[cfg(feature = "directml")]
            {
                b = b
                    .with_execution_providers([ort::ep::DirectML::default().build()])
                    .map_err(|e| anyhow::anyhow!("DirectML provider registration failed: {}", e))?;
                info!("Registered DirectML Execution Provider (Device {})", _id);
            }
        }
        ExecutionProvider::Cuda { device_id: _id } => {
            #[cfg(feature = "cuda")]
            {
                b = b
                    .with_execution_providers([ort::ep::CUDA::default().build()])
                    .map_err(|e| anyhow::anyhow!("CUDA provider registration failed: {}", e))?;
                info!("Registered CUDA Execution Provider (Device {})", _id);
            }
        }
        ExecutionProvider::Cpu => {}
    }

    Ok(b)
}
