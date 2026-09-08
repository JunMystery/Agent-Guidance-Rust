//! Unified Embedding Backend Abstraction
//!
//! Provides a seamless dispatch between accelerated Int8 ONNX Runtime inference
//! and CPU/CUDA/Metal Candle BERT fallback.

use anyhow::Result;
use super::quantized::OnnxQuantizedModel;

pub enum EmbeddingBackend {
    OnnxInt8(Box<OnnxQuantizedModel>),
    CandleBert(super::candle_bert::CandleBertInner),
}

impl EmbeddingBackend {
    pub fn embed_text(&self, text: &str, prefix: Option<&str>) -> Result<Vec<f32>> {
        match self {
            Self::OnnxInt8(onnx) => onnx.embed_text(text, prefix),
            Self::CandleBert(bert) => bert.embed_text(text, prefix),
        }
    }

    pub fn embed_batch(
        &self,
        texts: &[&str],
        prefix: Option<&str>,
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        match self {
            Self::OnnxInt8(onnx) => onnx.embed_batch(texts, prefix, batch_size),
            Self::CandleBert(bert) => bert.embed_batch(texts, prefix, batch_size),
        }
    }

    pub fn device_name(&self) -> &'static str {
        match self {
            Self::OnnxInt8(onnx) => onnx.device_name(),
            Self::CandleBert(bert) => bert.device_name(),
        }
    }

    pub fn is_quantized(&self) -> bool {
        match self {
            Self::OnnxInt8(onnx) => onnx.is_quantized,
            Self::CandleBert(_) => false,
        }
    }
}
