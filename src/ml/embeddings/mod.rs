pub mod backend;
pub mod binary_format;
pub mod candle_bert;
pub mod cache;
pub mod compactor;
pub mod compiler;
pub mod device;
pub mod gpu;
pub mod model;
pub mod precomputed;
pub mod precomputed_gen;
pub mod providers;
pub mod quantized;
pub mod search;

pub use device::cosine_similarity;
pub use gpu::eager_vram_warmup;
pub use cache::{
    spawn_background_auto_warmup, try_cached_model,
};
pub use precomputed::{
    catalog_fingerprint, generate_precomputed_cache, load_passage_cache,
};
pub use search::hybrid_vector_search;

#[cfg(test)]
pub use candle_core::Device;
#[cfg(test)]
pub use device::resolve_optimal_device;
#[cfg(test)]
pub use gpu::{GpuSkillMatrix, gpu_batch_cosine_similarity};
#[cfg(test)]
pub use cache::{cached_model, is_warmup_complete};

#[cfg(test)]
#[path = "../embeddings_tests.rs"]
mod tests;

#[cfg(test)]
mod quantized_tests;
