use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use tracing::info;

use crate::catalog::store::{SkillItem, SkillSource, get_embedded_skill, list_embedded_skills};
use crate::ml::inference_pool;
use super::gpu::GpuSkillMatrix;
use super::model::EmbeddingModel;
use super::precomputed::{catalog_fingerprint, load_passage_cache, load_precomputed_cache, save_passage_cache};

#[derive(Clone)]
pub struct PassageCache {
    pub fingerprint: u64,
    pub vectors: Arc<Vec<Vec<f32>>>,
}

pub static PASSAGE_CACHE: OnceLock<RwLock<Vec<PassageCache>>> = OnceLock::new();
pub static GPU_SKILL_MATRIX: OnceLock<RwLock<Option<GpuSkillMatrix>>> = OnceLock::new();
static WARMUP_DONE: OnceLock<AtomicBool> = OnceLock::new();

pub fn store_passage_cache(vectors: Vec<Vec<f32>>, skills: &[SkillItem]) {
    let fingerprint = catalog_fingerprint(skills);
    let cache = PASSAGE_CACHE.get_or_init(|| RwLock::new(Vec::new()));
    if let Ok(mut guard) = cache.write() {
        guard.retain(|entry| entry.fingerprint != fingerprint);
        guard.push(PassageCache {
            fingerprint,
            vectors: Arc::new(vectors.clone()),
        });
    }

    if let Ok(guard) = cached_model() {
        if let Ok(gpu_matrix) = GpuSkillMatrix::from_vectors(&vectors, fingerprint, &guard.device) {
            let gpu_slot = GPU_SKILL_MATRIX.get_or_init(|| RwLock::new(None));
            if let Ok(mut g_guard) = gpu_slot.write() {
                *g_guard = Some(gpu_matrix);
                info!(
                    "[VRAM Residency] GpuSkillMatrix populated with {} skill vectors on {}.",
                    vectors.len(),
                    guard.device_name()
                );
            }
        }
    }
}

pub fn is_warmup_complete() -> bool {
    WARMUP_DONE
        .get_or_init(|| AtomicBool::new(false))
        .load(Ordering::Relaxed)
}

pub fn mark_warmup_complete() {
    WARMUP_DONE
        .get_or_init(|| AtomicBool::new(false))
        .store(true, Ordering::Relaxed);
}

static MODEL: OnceLock<Result<RwLock<EmbeddingModel>, String>> = OnceLock::new();

pub fn try_cached_model() -> Option<std::sync::RwLockReadGuard<'static, EmbeddingModel>> {
    MODEL.get().and_then(|res| res.as_ref().ok()).and_then(|rw| rw.try_read().ok())
}

pub fn cached_model() -> Result<std::sync::RwLockReadGuard<'static, EmbeddingModel>, String> {
    match MODEL.get_or_init(|| {
        EmbeddingModel::load_or_download()
            .map(RwLock::new)
            .map_err(|e| e.to_string())
    }) {
        Ok(model) => model.read().map_err(|_| "RwLock poisoned".to_string()),
        Err(err) => Err(err.clone()),
    }
}

pub fn warmup_cache() {
    let candidates: Vec<SkillItem> = list_embedded_skills()
        .iter()
        .filter_map(|path| {
            get_embedded_skill(path).map(|content| SkillItem {
                name: path.split('/').next().unwrap_or(path).to_string(),
                relative_path: path.clone(),
                source: SkillSource::Embedded,
                content,
            })
        })
        .collect();

    // 1. Precomputed cache (embedded in binary) — instant, no model needed
    if let Some(cached) = load_precomputed_cache(&candidates) {
        store_passage_cache(cached, &candidates);
        mark_warmup_complete();
        info!(
            "Loaded precomputed passage cache ({} skills).",
            candidates.len()
        );
        eager_load_embedding_model();
        return;
    }

    // 2. On-disk cache (~/.agent-guidance/vectors.bin)
    if let Some(cached) = load_passage_cache(&candidates) {
        store_passage_cache(cached, &candidates);
        mark_warmup_complete();
        info!(
            "Loaded passage cache from disk ({} skills).",
            candidates.len()
        );
        eager_load_embedding_model();
        return;
    }

    // 3. Compute from model (cache miss — first start after skill changes)
    info!(
        "Computing passage embeddings for {} skills...",
        candidates.len()
    );
    let texts: Vec<String> = candidates
        .iter()
        .map(|c| c.to_search_passage())
        .collect();

    let model_guard = cached_model().ok();
    let vecs = match model_guard.as_ref() {
        Some(guard) => inference_pool().install(|| {
            texts
                .par_iter()
                .filter_map(|text| guard.embed_text(text, Some("passage")).ok())
                .collect()
        }),
        None => Vec::new(),
    };

    save_passage_cache(&vecs, &candidates);
    store_passage_cache(vecs, &candidates);
    mark_warmup_complete();
    info!(
        "Passage embedding warmup complete ({} skills).",
        candidates.len()
    );

    // 4. Preload models so first user query doesn't pay OnceLock init
    eager_load_embedding_model();
}

pub fn eager_load_embedding_model() {
    drop(cached_model());
    drop(crate::ml::llm_selector::cached_cross_encoder());
}

pub fn clear_passage_cache() {
    let cache = PASSAGE_CACHE.get_or_init(|| RwLock::new(Vec::new()));
    if let Ok(mut guard) = cache.write() {
        guard.clear();
    }
    let gpu_slot = GPU_SKILL_MATRIX.get_or_init(|| RwLock::new(None));
    if let Ok(mut g_guard) = gpu_slot.write() {
        *g_guard = None;
    }
}

pub fn embed_skills_cache(candidates: &[SkillItem], model: &EmbeddingModel) -> Arc<Vec<Vec<f32>>> {
    let fingerprint = catalog_fingerprint(candidates);
    let cache = PASSAGE_CACHE.get_or_init(|| RwLock::new(Vec::new()));
    if let Ok(guard) = cache.read() {
        if let Some(entry) = guard.iter().find(|entry| {
            entry.fingerprint == fingerprint && entry.vectors.len() == candidates.len()
        }) {
            return entry.vectors.clone();
        }
    }

    info!(
        "[ML Pipeline] Computing passage embeddings for {} skills on-the-fly...",
        candidates.len()
    );
    let vecs: Vec<Vec<f32>> = inference_pool().install(|| {
        candidates
            .par_iter()
            .filter_map(|cand| {
                let text = cand.to_search_passage();
                model.embed_text(&text, Some("passage")).ok()
            })
            .collect()
    });
    store_passage_cache(vecs, candidates);
    cache
        .read()
        .ok()
        .and_then(|guard| {
            guard
                .iter()
                .find(|entry| entry.fingerprint == fingerprint)
                .map(|entry| entry.vectors.clone())
        })
        .unwrap_or_else(|| Arc::new(Vec::new()))
}

