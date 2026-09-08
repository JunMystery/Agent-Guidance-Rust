//! Batched symbol and code chunk embedding with content-addressable caching.

use anyhow::Result;
use crate::context::db::CodeGraphDb;
use super::compute_hash;

/// Embeds symbols without vectors in batches, checking content-addressable cache first.
pub fn embed_symbols_batched(db: &CodeGraphDb, limit: usize, batch_size: usize) -> Result<usize> {
    let unindexed = db.get_symbols_without_vectors(limit)?;
    if unindexed.is_empty() {
        return Ok(0);
    }

    let mut cache_hits = 0;
    let mut misses = Vec::new();

    for (id, name, kind, file_path, sig) in unindexed {
        let passage = format!("{} in {} — {}", kind, file_path, sig.as_deref().unwrap_or(&name));
        let p_hash = compute_hash(&passage);

        if let Ok(Some(cached_vec)) = db.get_cached_embedding(&p_hash) {
            let _ = db.store_symbol_vector(&id, &cached_vec, "multilingual-e5-small");
            cache_hits += 1;
        } else {
            misses.push((id, p_hash, passage));
        }
    }

    if misses.is_empty() {
        return Ok(cache_hits);
    }

    let model_guard = match crate::ml::embeddings::try_cached_model() {
        Some(m) => m,
        None => return Ok(cache_hits),
    };

    let texts: Vec<&str> = misses.iter().map(|m| m.2.as_str()).collect();
    let b_size = if batch_size == 0 { 32 } else { batch_size };
    let vectors = model_guard.embed_batch(&texts, Some("passage"), b_size)?;

    let mut fresh_embedded = 0;
    for ((id, p_hash, _), vec) in misses.iter().zip(vectors.iter()) {
        let _ = db.store_symbol_vector(id, vec, "multilingual-e5-small");
        let _ = db.store_cached_embedding(p_hash, vec, "multilingual-e5-small");
        fresh_embedded += 1;
    }

    Ok(cache_hits + fresh_embedded)
}

/// Embeds code chunks without vectors in batches, checking content-addressable cache first.
pub fn embed_chunks_batched(db: &CodeGraphDb, limit: usize, batch_size: usize) -> Result<usize> {
    let unindexed = db.get_chunks_without_vectors(limit)?;
    if unindexed.is_empty() {
        return Ok(0);
    }

    let mut cache_hits = 0;
    let mut misses = Vec::new();

    for (chunk_id, file_path, start, end, text) in unindexed {
        let truncated: String = text.chars().take(512).collect();
        let passage = format!("code in {} lines {}-{}: {}", file_path, start, end, truncated);
        let p_hash = compute_hash(&passage);

        if let Ok(Some(cached_vec)) = db.get_cached_embedding(&p_hash) {
            let _ = db.store_chunk_vector(chunk_id, &cached_vec, "multilingual-e5-small");
            cache_hits += 1;
        } else {
            misses.push((chunk_id, p_hash, passage));
        }
    }

    if misses.is_empty() {
        return Ok(cache_hits);
    }

    let model_guard = match crate::ml::embeddings::try_cached_model() {
        Some(m) => m,
        None => return Ok(cache_hits),
    };

    let texts: Vec<&str> = misses.iter().map(|m| m.2.as_str()).collect();
    let b_size = if batch_size == 0 { 32 } else { batch_size };
    let vectors = model_guard.embed_batch(&texts, Some("passage"), b_size)?;

    let mut fresh_embedded = 0;
    for ((chunk_id, p_hash, _), vec) in misses.iter().zip(vectors.iter()) {
        let _ = db.store_chunk_vector(*chunk_id, vec, "multilingual-e5-small");
        let _ = db.store_cached_embedding(p_hash, vec, "multilingual-e5-small");
        fresh_embedded += 1;
    }

    Ok(cache_hits + fresh_embedded)
}
