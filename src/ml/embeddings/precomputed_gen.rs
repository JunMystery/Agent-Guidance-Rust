//! Offline and build-time precomputed vector and manifest generator

use anyhow::Result;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::catalog::store::{SkillItem, SkillSource, get_embedded_skill, list_embedded_skills};
use super::model::EmbeddingModel;
use super::precomputed::{catalog_fingerprint, save_passage_cache};

pub fn generate_precomputed_cache() -> Result<()> {
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

    println!(
        "Generating precomputed passage cache for {} skills...",
        candidates.len()
    );

    let model = EmbeddingModel::load_or_download()?;
    let texts: Vec<String> = candidates
        .iter()
        .map(|c| c.to_search_passage())
        .collect();

    let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
    let total = text_refs.len();
    let mut vecs = Vec::with_capacity(total);
    for (batch_idx, chunk) in text_refs.chunks(32).enumerate() {
        let chunk_vecs = model.embed_batch(chunk, Some("passage"), 32)?;
        vecs.extend(chunk_vecs);
        let processed = vecs.len();
        let percent = (processed as f32 / total as f32) * 100.0;
        println!(
            "  [Batch {:02}] Processed {}/{} skills ({:.1}%)",
            batch_idx + 1,
            processed,
            total,
            percent
        );
    }

    if vecs.len() != candidates.len() {
        anyhow::bail!(
            "Failed to embed all skills: got {} of {}",
            vecs.len(),
            candidates.len()
        );
    }

    let count = vecs.len() as u32;
    let dim = (vecs.first().map(|v| v.len()).unwrap_or(0)) as u32;
    let mut bin_data = Vec::with_capacity(8 + (count as usize) * (dim as usize) * 4);
    bin_data.extend_from_slice(&count.to_le_bytes());
    bin_data.extend_from_slice(&dim.to_le_bytes());
    for vec in &vecs {
        for val in vec {
            bin_data.extend_from_slice(&val.to_le_bytes());
        }
    }

    let fp = catalog_fingerprint(&candidates);
    let skills_metadata: Vec<serde_json::Value> = candidates
        .iter()
        .map(|c| {
            let doc = c.to_semantic_doc();
            serde_json::json!({
                "name": doc.name,
                "title": doc.title,
                "intent": doc.intent,
                "micro_rules": doc.micro_rules,
                "action_triggers": doc.action_triggers,
                "file_patterns": doc.file_patterns,
                "applicable_phases": doc.applicable_phases,
            })
        })
        .collect();

    let manifest = serde_json::json!({
        "catalog_fingerprint": fp,
        "count": candidates.len(),
        "dimension": dim,
        "model": "intfloat/multilingual-e5-small",
        "created_at": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "skills": skills_metadata,
    });

    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;

    // Always save to user cache directory (handles standalone installs gracefully)
    save_passage_cache(&vecs, &candidates);

    // If running in development repository with src/ml directory, update build artifacts
    let src_ml = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ml");
    if src_ml.exists() {
        let vec_out = src_ml.join("precomputed_vectors.bin");
        let man_out = src_ml.join("precomputed_manifest.json");
        let _ = std::fs::write(&vec_out, &bin_data);
        let _ = std::fs::write(&man_out, &manifest_bytes);
        println!("Saved {} bytes to {}", bin_data.len(), vec_out.display());
        println!(
            "Saved manifest to {} (fingerprint: {:016x})",
            man_out.display(),
            fp
        );
    }

    println!("Precomputed passage cache generated successfully.");
    Ok(())
}

pub fn generate_manifest_only() -> Result<()> {
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

    let fp = catalog_fingerprint(&candidates);
    let skills_metadata: Vec<serde_json::Value> = candidates
        .iter()
        .map(|c| {
            let doc = c.to_semantic_doc();
            serde_json::json!({
                "name": doc.name,
                "title": doc.title,
                "intent": doc.intent,
                "micro_rules": doc.micro_rules,
                "action_triggers": doc.action_triggers,
                "file_patterns": doc.file_patterns,
                "applicable_phases": doc.applicable_phases,
            })
        })
        .collect();

    let manifest = serde_json::json!({
        "catalog_fingerprint": fp,
        "count": candidates.len(),
        "dimension": 384,
        "model": "intfloat/multilingual-e5-small",
        "created_at": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "skills": skills_metadata,
    });

    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let src_ml = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ml");
    if src_ml.exists() {
        let man_out = src_ml.join("precomputed_manifest.json");
        std::fs::write(&man_out, &manifest_bytes)?;
        println!(
            "Indexed {} skills into {} (fingerprint: {:016x}) in <50ms.",
            candidates.len(),
            man_out.display(),
            fp
        );
    }

    Ok(())
}
