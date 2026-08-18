pub mod device;
pub mod model;
pub mod gpu;
pub mod cache;
pub mod precomputed;
pub mod search;

pub use candle_core::Device;
pub use crate::catalog::store::SkillItem;
pub use device::{cosine_similarity, resolve_optimal_device};
pub use model::EmbeddingModel;
pub use gpu::{GpuSkillMatrix, gpu_batch_cosine_similarity, eager_vram_warmup};
pub use cache::{
    cached_model, clear_passage_cache, embed_skills_cache, is_warmup_complete,
    mark_warmup_complete, warmup_cache,
};
pub use precomputed::{
    catalog_fingerprint, generate_precomputed_cache, load_passage_cache, save_passage_cache,
};
pub use search::hybrid_vector_search;

#[cfg(test)]
#[path = "../embeddings_tests.rs"]
mod tests;
