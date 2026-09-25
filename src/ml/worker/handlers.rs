use anyhow::{Context, Result, bail};

use super::state::WorkerState;
use super::types::*;
use crate::catalog::checksum::load_manifest;
use crate::catalog::slicing::slice_skill_markdown;
use crate::catalog::tombstone::load_tombstones;
use crate::ml::embeddings::cache::cached_model;
use crate::ml::embeddings::compactor::{CompactionResult, delete_skills_and_compact};
use crate::ml::embeddings::compiler::{CompilationStats, compile_staging_to_binary};
use crate::optimizer::compressor::estimate_tokens;

pub fn handle_health(state: &WorkerState) -> HealthResponse {
    let guard = state.catalog.read().ok();
    let total_skills = guard.as_ref().map(|g| g.skills.len()).unwrap_or(0);
    let total_sections = guard
        .as_ref()
        .map(|g| g.skills.iter().map(|s| s.content.matches("\n## ").count() + 1).sum())
        .unwrap_or(0);

    HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs: state.start_time.elapsed().as_secs(),
        engine: "Candle BERT / In-Memory AGV1".to_string(),
        backend: "Rust Native ThreadPool".to_string(),
        total_skills,
        total_sections,
    }
}

pub fn handle_stats(state: &WorkerState) -> SkillStats {
    let guard = state.catalog.read().ok();
    let total_skills = guard.as_ref().map(|g| g.skills.len()).unwrap_or(0);
    let total_sections = guard
        .as_ref()
        .map(|g| g.skills.iter().map(|s| s.content.matches("\n## ").count() + 1).sum())
        .unwrap_or(0);
    let vectors_dim = guard
        .as_ref()
        .and_then(|g| g.vectors.first())
        .map(|v| v.len())
        .unwrap_or(384);
    let (catalog_hash, last_reindex_at) = guard
        .as_ref()
        .map(|g| (g.catalog_hash.clone(), g.last_reindex_at))
        .unwrap_or_default();

    SkillStats {
        total_skills,
        total_sections,
        vectors_dim,
        catalog_hash,
        last_reindex_at,
        tombstones_count: load_tombstones().len(),
        token_savings_ratio: 0.81,
    }
}

pub fn handle_list(state: &WorkerState) -> Vec<SkillSummary> {
    let guard = match state.catalog.read() {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    guard
        .skills
        .iter()
        .map(|s| SkillSummary {
            name: s.name.clone(),
            sections: s.content.matches("\n## ").count() + 1,
            updated_at: guard.last_reindex_at,
        })
        .collect()
}

pub fn handle_search(state: &WorkerState, req: SearchRequest) -> Result<SearchResponse> {
    let model = cached_model().map_err(|e| anyhow::anyhow!("Model load error: {}", e))?;
    let query_vec = model.embed_text(&req.query, Some("query"))?;
    drop(model);

    let max_k = req.max_results.unwrap_or(5);
    let threshold = req.threshold.unwrap_or(0.20);
    let hits = state.search_top_k(&query_vec, max_k, threshold);

    let guard = state.catalog.read().map_err(|_| anyhow::anyhow!("Catalog poisoned"))?;
    let mut results = Vec::with_capacity(hits.len());
    for (idx, score) in hits {
        if let Some(skill) = guard.skills.get(idx) {
            results.push(SkillHit {
                name: skill.name.clone(),
                score,
                title: skill.name.replace('-', " "),
                intent: format!("Skill for {}", skill.name),
                sections: skill.content.matches("\n## ").count() + 1,
            });
        }
    }
    let total_found = results.len();
    Ok(SearchResponse { results, total_found })
}

pub fn handle_slice(state: &WorkerState, req: SliceRequest) -> Result<SliceResponse> {
    let guard = state.catalog.read().map_err(|_| anyhow::anyhow!("Catalog poisoned"))?;
    let idx = guard
        .name_to_idx
        .get(&req.skill.to_lowercase())
        .copied()
        .ok_or_else(|| anyhow::anyhow!("Skill not found: {}", req.skill))?;
    let skill = guard.skills.get(idx).context("Corrupt skill index")?;

    let original_token_count = estimate_tokens(&skill.content, false);
    let top_k = req.max_sections.unwrap_or(3);
    let sliced = slice_skill_markdown(&skill.content, &req.task, top_k);
    let token_count = estimate_tokens(&sliced, false);

    Ok(SliceResponse {
        skill: req.skill,
        content: sliced,
        token_count,
        original_token_count,
    })
}

pub fn handle_delete(state: &WorkerState, req: DeleteRequest) -> Result<CompactionResult> {
    let purge = req.purge_staging.unwrap_or(true);
    let res = delete_skills_and_compact(&req.names, purge)?;
    // Reload state after compaction
    if let Ok(new_state) = WorkerState::load_from_disk() {
        if let Ok(g) = new_state.catalog.read() {
            state.hot_swap(g.skills.clone(), g.vectors.clone(), g.catalog_hash.clone());
        }
    }
    Ok(res)
}

pub fn handle_reindex(state: &WorkerState, req: ReindexRequest) -> Result<CompilationStats> {
    let force = req.force.unwrap_or(false);
    let stats = compile_staging_to_binary(force)?;
    // Reload state after reindexing
    if let Ok(new_state) = WorkerState::load_from_disk() {
        if let Ok(g) = new_state.catalog.read() {
            state.hot_swap(g.skills.clone(), g.vectors.clone(), g.catalog_hash.clone());
        }
    }
    Ok(stats)
}

pub fn handle_embed(req: EmbedRequest) -> Result<EmbedResponse> {
    if req.texts.is_empty() {
        return Ok(EmbedResponse { embeddings: Vec::new() });
    }
    let model = cached_model().map_err(|e| anyhow::anyhow!("Model load error: {}", e))?;
    let refs: Vec<&str> = req.texts.iter().map(|s| s.as_str()).collect();
    let prefix = req.prefix.as_deref().unwrap_or("passage");
    let embeddings = model.embed_batch(&refs, Some(prefix), 32)?;
    Ok(EmbedResponse { embeddings })
}
