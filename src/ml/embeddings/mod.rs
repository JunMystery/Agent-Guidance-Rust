pub mod backend;
pub mod candle_bert;
pub mod cache;
pub mod device;
pub mod gpu;
pub mod model;
pub mod precomputed;
pub mod precomputed_gen;
pub mod providers;
pub mod quantized;
pub mod search;

pub use candle_core::Device;
pub use crate::catalog::store::SkillItem;
pub use backend::EmbeddingBackend;
pub use device::{cosine_similarity, resolve_optimal_device};
pub use gpu::{GpuSkillMatrix, eager_vram_warmup, gpu_batch_cosine_similarity};
pub use model::EmbeddingModel;
pub use providers::{ExecutionProvider, detect_optimal_provider};
pub use quantized::OnnxQuantizedModel;
pub use cache::{
    cached_model, clear_passage_cache, embed_skills_cache, is_warmup_complete,
    mark_warmup_complete, spawn_background_auto_warmup, try_cached_model, warmup_cache,
};
pub use precomputed::{
    catalog_fingerprint, generate_precomputed_cache, load_passage_cache, save_passage_cache,
};
pub use search::hybrid_vector_search;

#[cfg(test)]
#[path = "../embeddings_tests.rs"]
mod tests;

#[cfg(test)]
mod quantized_tests;
