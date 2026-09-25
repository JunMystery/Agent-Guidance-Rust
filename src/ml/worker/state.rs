use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use anyhow::{Context, Result};

use crate::catalog::checksum::load_manifest;
use crate::ml::embeddings::binary_format::{SkillRecord, SkillsBinary, VectorBinary};
use crate::ml::embeddings::device::cosine_similarity;
use crate::ml::embeddings::precomputed::{skills_bin_path, vectors_path};

#[derive(Default)]
pub struct WorkerCatalog {
    pub skills: Vec<SkillRecord>,
    pub vectors: Vec<Vec<f32>>,
    pub name_to_idx: HashMap<String, usize>,
    pub catalog_hash: String,
    pub last_reindex_at: u64,
}

#[derive(Clone)]
pub struct WorkerState {
    pub catalog: Arc<RwLock<WorkerCatalog>>,
    pub start_time: Instant,
}

impl WorkerState {
    pub fn new_empty() -> Self {
        Self {
            catalog: Arc::new(RwLock::new(WorkerCatalog::default())),
            start_time: Instant::now(),
        }
    }

    pub fn load_from_disk() -> Result<Self> {
        let skills_path = skills_bin_path();
        let v_path = vectors_path();

        if !skills_path.exists() || !v_path.exists() {
            tracing::warn!("skills.bin or vectors.bin not found on disk at {:?}. Starting with empty catalog.", skills_path);
            return Ok(Self::new_empty());
        }

        let s_data = std::fs::read(&skills_path)
            .with_context(|| format!("Missing skills.bin at {:?}", skills_path))?;
        let v_data = std::fs::read(&v_path)
            .with_context(|| format!("Missing vectors.bin at {:?}", v_path))?;

        let sb = SkillsBinary::deserialize(&s_data)?;
        let vb = VectorBinary::deserialize(&v_data)?;

        let manifest = load_manifest();
        let (cat_hash, last_reindex) = manifest
            .map(|m| (m.catalog_hash, m.created_at))
            .unwrap_or_default();

        let mut name_to_idx = HashMap::with_capacity(sb.skills.len());
        for (i, skill) in sb.skills.iter().enumerate() {
            name_to_idx.insert(skill.name.to_lowercase(), i);
        }

        let catalog = WorkerCatalog {
            skills: sb.skills,
            vectors: vb.vectors,
            name_to_idx,
            catalog_hash: cat_hash,
            last_reindex_at: last_reindex,
        };

        Ok(Self {
            catalog: Arc::new(RwLock::new(catalog)),
            start_time: Instant::now(),
        })
    }

    pub fn hot_swap(
        &self,
        skills: Vec<SkillRecord>,
        vectors: Vec<Vec<f32>>,
        catalog_hash: String,
    ) {
        let mut name_to_idx = HashMap::with_capacity(skills.len());
        for (i, skill) in skills.iter().enumerate() {
            name_to_idx.insert(skill.name.to_lowercase(), i);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Ok(mut guard) = self.catalog.write() {
            guard.skills = skills;
            guard.vectors = vectors;
            guard.name_to_idx = name_to_idx;
            guard.catalog_hash = catalog_hash;
            guard.last_reindex_at = now;
        }
    }

    pub fn search_top_k(&self, query_vec: &[f32], max_k: usize, threshold: f32) -> Vec<(usize, f32)> {
        let guard = match self.catalog.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };

        let mut scored: Vec<(usize, f32)> = guard
            .vectors
            .iter()
            .enumerate()
            .map(|(idx, v)| (idx, cosine_similarity(query_vec, v)))
            .filter(|&(_, s)| s >= threshold)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_k);
        scored
    }
}
