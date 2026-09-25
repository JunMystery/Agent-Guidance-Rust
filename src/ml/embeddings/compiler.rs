use std::collections::HashMap;
use std::time::Instant;
use anyhow::{Context, Result};
use tracing::info;

use super::binary_format::{SkillRecord, SkillsBinary, VectorBinary};
use super::model::EmbeddingModel;
use super::precomputed::{load_precomputed_cache, skills_bin_path, vectors_path};
use crate::catalog::checksum::{
    SkillManifestEntry, SkillsManifest, compute_catalog_hash, compute_sha256,
    detect_catalog_diff, load_manifest, save_manifest,
};
use crate::catalog::staging::{merge_with_precedence, scan_staging_skills};
use crate::catalog::store::{SkillItem, SkillSource, get_embedded_skill, list_embedded_skills};
use crate::catalog::tombstone::load_tombstones;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompilationStats {
    pub total_skills: usize,
    pub unchanged_skipped: usize,
    pub reindexed: usize,
    pub duration_ms: u128,
    pub catalog_hash: String,
}

pub fn compile_staging_to_binary(force: bool) -> Result<CompilationStats> {
    let start = Instant::now();
    let _ = crate::catalog::staging::ensure_staging_dir();
    let tombstones = load_tombstones();

    // 1. Gather embedded skills (filtered by tombstones)
    let mut embedded = Vec::new();
    for path in list_embedded_skills() {
        if let Some(content) = get_embedded_skill(&path) {
            let name = path.split('/').next().unwrap_or(&path).to_string();
            if !tombstones.contains(&name.to_lowercase()) {
                embedded.push(SkillItem {
                    name,
                    relative_path: path.clone(),
                    source: SkillSource::Embedded,
                    content,
                });
            }
        }
    }

    // 2. Gather staged skills & merge with precedence
    let staged = scan_staging_skills();
    let skills = merge_with_precedence(embedded, staged, &tombstones);

    let prev_manifest = if force { None } else { load_manifest() };
    let diff = detect_catalog_diff(&skills, prev_manifest.as_ref());

    // 3. Load existing vectors to reuse for unchanged skills
    let mut cached_vectors: HashMap<String, Vec<f32>> = HashMap::new();
    if let Ok(v_data) = std::fs::read(vectors_path()) {
        if let Ok(vb) = VectorBinary::deserialize(&v_data) {
            if let Ok(s_data) = std::fs::read(skills_bin_path()) {
                if let Ok(sb) = SkillsBinary::deserialize(&s_data) {
                    for (idx, skill) in sb.skills.into_iter().enumerate() {
                        if let Some(vec) = vb.vectors.get(idx) {
                            cached_vectors.insert(skill.name.to_lowercase(), vec.clone());
                        }
                    }
                }
            }
        }
    }

    // Fallback to precomputed vectors if available
    if cached_vectors.is_empty() {
        if let Some(vecs) = load_precomputed_cache(&skills) {
            for (idx, skill) in skills.iter().enumerate() {
                if let Some(vec) = vecs.get(idx) {
                    cached_vectors.insert(skill.name.to_lowercase(), vec.clone());
                }
            }
        }
    }

    // 4. Check if reindexing is needed
    let to_embed: Vec<&SkillItem> = if force {
        skills.iter().collect()
    } else {
        diff.modified_or_new
            .iter()
            .filter(|s| !cached_vectors.contains_key(&s.name.to_lowercase()))
            .collect()
    };

    let mut newly_embedded: HashMap<String, Vec<f32>> = HashMap::new();
    let reindexed_count = to_embed.len();

    if !to_embed.is_empty() {
        info!("Embedding {} modified or new skills...", reindexed_count);
        let model = EmbeddingModel::load_or_download()?;
        let passages: Vec<String> = to_embed
            .iter()
            .map(|s| s.to_search_passage())
            .collect();
        let refs: Vec<&str> = passages.iter().map(|s| s.as_str()).collect();

        for (chunk_idx, chunk) in refs.chunks(32).enumerate() {
            let chunk_vecs = model.embed_batch(chunk, Some("passage"), 32)?;
            for (offset, vec) in chunk_vecs.into_iter().enumerate() {
                let skill_idx = chunk_idx * 32 + offset;
                let skill_name = &to_embed[skill_idx].name;
                newly_embedded.insert(skill_name.to_lowercase(), vec);
            }
        }
    }

    // 5. Assemble all final vectors in exact skill order
    let mut final_records = Vec::with_capacity(skills.len());
    let mut final_vectors = Vec::with_capacity(skills.len());
    let mut manifest_entries = HashMap::with_capacity(skills.len());

    let default_dim = newly_embedded
        .values()
        .next()
        .or_else(|| cached_vectors.values().next())
        .map(|v| v.len())
        .unwrap_or(384);

    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for skill in &skills {
        let norm = skill.name.to_lowercase();
        let vec = newly_embedded
            .remove(&norm)
            .or_else(|| cached_vectors.remove(&norm))
            .unwrap_or_else(|| vec![0.0f32; default_dim]);

        final_vectors.push(vec);
        final_records.push(SkillRecord {
            name: skill.name.clone(),
            relative_path: skill.relative_path.clone(),
            content: skill.content.clone(),
        });

        let sha = compute_sha256(skill.content.as_bytes());
        let sec_count = skill.content.matches("\n## ").count() + 1;
        manifest_entries.insert(
            norm.clone(),
            SkillManifestEntry {
                name: skill.name.clone(),
                sha256: sha,
                section_count: sec_count,
                updated_at: now_ts,
            },
        );
    }

    // 6. Atomically persist binary files
    let sb = SkillsBinary::new(final_records);
    let vb = VectorBinary::new(final_vectors);

    let s_path = skills_bin_path();
    if let Some(parent) = s_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s_tmp = s_path.with_extension("tmp");
    std::fs::write(&s_tmp, sb.serialize())?;
    std::fs::rename(s_tmp, &s_path)?;

    let v_path = vectors_path();
    let v_tmp = v_path.with_extension("tmp");
    std::fs::write(&v_tmp, vb.serialize())?;
    std::fs::rename(v_tmp, &v_path)?;

    // 7. Update skills_manifest.json
    let entries_vec: Vec<_> = manifest_entries.values().cloned().collect();
    let cat_hash = compute_catalog_hash(&entries_vec);
    let manifest = SkillsManifest {
        catalog_hash: cat_hash.clone(),
        count: skills.len(),
        created_at: now_ts,
        skills: manifest_entries,
    };
    save_manifest(&manifest)?;

    let duration_ms = start.elapsed().as_millis();
    info!(
        "Binary compilation complete: {} total skills ({} reindexed, {} skipped) in {} ms. Catalog hash: {}",
        skills.len(),
        reindexed_count,
        diff.unchanged.len(),
        duration_ms,
        cat_hash
    );

    Ok(CompilationStats {
        total_skills: skills.len(),
        unchanged_skipped: skills.len().saturating_sub(reindexed_count),
        reindexed: reindexed_count,
        duration_ms,
        catalog_hash: cat_hash,
    })
}
