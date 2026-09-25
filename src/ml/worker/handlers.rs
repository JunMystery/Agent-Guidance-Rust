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
    let candidate_k = std::cmp::max(max_k * 3, 10);
    let hits = state.search_top_k(&query_vec, candidate_k, threshold);

    let guard = state.catalog.read().map_err(|_| anyhow::anyhow!("Catalog poisoned"))?;
    let mut candidate_items = Vec::with_capacity(hits.len());
    for (idx, score) in hits {
        if let Some(skill) = guard.skills.get(idx) {
            candidate_items.push((score, skill));
        }
    }

    // Stage 2: Cross-Encoder Reranking if available
    if let Ok(ce) = crate::ml::cross_encoder::cached_cross_encoder() {
        for item in &mut candidate_items {
            let excerpt: String = item.1.content.chars().take(350).collect();
            if let Ok(ce_score) = ce.score(&req.query, &excerpt) {
                item.0 = item.0 * 0.4 + ce_score * 0.6;
            }
        }
        candidate_items.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    let mut results = Vec::with_capacity(std::cmp::min(candidate_items.len(), max_k));
    for (score, skill) in candidate_items.into_iter().take(max_k) {
        results.push(SkillHit {
            name: skill.name.clone(),
            score,
            title: skill.name.replace('-', " "),
            intent: format!("Skill for {}", skill.name),
            sections: skill.content.matches("\n## ").count() + 1,
        });
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

    let sliced = if !req.task.trim().is_empty() {
        match slice_sections_neural(&skill.content, &req.task, top_k) {
            Ok(res) => res,
            Err(_) => slice_skill_markdown(&skill.content, &req.task, top_k),
        }
    } else {
        slice_skill_markdown(&skill.content, &req.task, top_k)
    };

    let token_count = estimate_tokens(&sliced, false);

    Ok(SliceResponse {
        skill: req.skill,
        content: sliced,
        token_count,
        original_token_count,
    })
}

fn slice_sections_neural(content: &str, task: &str, top_k: usize) -> Result<String> {
    let sections = crate::catalog::slicing::split_markdown_into_sections(content);
    if sections.len() <= top_k {
        return Ok(crate::optimizer::compressor::compress_markdown(content));
    }

    let model = cached_model().map_err(|e| anyhow::anyhow!("Model unavailable: {}", e))?;
    let query_vec = model.embed_text(task, Some("query"))?;
    let section_texts: Vec<&str> = sections.iter().map(|s| s.content.as_str()).collect();
    let sec_embeddings = model.embed_batch(&section_texts, Some("passage"), 16)?;

    let mut scored: Vec<(f32, &crate::catalog::slicing::MarkdownSection)> = Vec::with_capacity(sections.len());
    for (i, sec) in sections.iter().enumerate() {
        if let Some(s_vec) = sec_embeddings.get(i) {
            let sim = crate::ml::embeddings::device::cosine_similarity(&query_vec, s_vec);
            scored.push((sim, sec));
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let sliced = scored
        .into_iter()
        .take(top_k)
        .map(|(score, s)| format!("#### {} (Relevance: {:.2})\n{}", s.title, score, crate::optimizer::compressor::compress_markdown(&s.content)))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    Ok(sliced)
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
