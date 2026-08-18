use anyhow::Result;
use rayon::prelude::*;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::catalog::store::{SkillItem, SkillSource, get_embedded_skill, list_embedded_skills};
use crate::ml::inference_pool;
use super::model::EmbeddingModel;

pub const PRECOMPUTED_VECTORS: &[u8] = include_bytes!("../precomputed_vectors.bin");
pub const PRECOMPUTED_MANIFEST: &[u8] = include_bytes!("../precomputed_manifest.json");

pub fn cache_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance"))
        .unwrap_or_else(|| PathBuf::from(".agent-guidance"))
}

pub fn manifest_path() -> PathBuf {
    cache_dir().join("passage_manifest.json")
}

pub fn vectors_path() -> PathBuf {
    cache_dir().join("vectors.bin")
}

pub fn catalog_fingerprint(skills: &[SkillItem]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for skill in skills {
        skill.relative_path.hash(&mut hasher);
        skill.content.hash(&mut hasher);
    }
    hasher.finish()
}

pub fn load_precomputed_cache(skills: &[SkillItem]) -> Option<Vec<Vec<f32>>> {
    if PRECOMPUTED_VECTORS.is_empty() || PRECOMPUTED_MANIFEST.is_empty() {
        return None;
    }

    let manifest: serde_json::Value = serde_json::from_slice(PRECOMPUTED_MANIFEST).ok()?;
    let manifest_count = manifest.get("count")?.as_u64()? as usize;
    if manifest_count != skills.len() {
        return None;
    }

    let manifest_fp = manifest.get("catalog_fingerprint")?.as_u64()?;
    let current_fp = catalog_fingerprint(skills);
    if manifest_fp != current_fp {
        return None;
    }

    let data = PRECOMPUTED_VECTORS;
    if data.len() < 8 {
        return None;
    }

    let count = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    let dim = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let expected_len = 8 + count * dim * 4;
    if data.len() != expected_len || count != skills.len() {
        return None;
    }

    let mut vectors = Vec::with_capacity(count);
    let mut offset = 8;
    for _ in 0..count {
        let mut vec = Vec::with_capacity(dim);
        for _ in 0..dim {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&data[offset..offset + 4]);
            vec.push(f32::from_le_bytes(bytes));
            offset += 4;
        }
        vectors.push(vec);
    }

    Some(vectors)
}

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
    let vecs: Vec<Vec<f32>> = model.embed_batch(&text_refs, Some("passage"), 32)?;

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
    let manifest = serde_json::json!({
        "catalog_fingerprint": fp,
        "count": candidates.len(),
        "dimension": dim,
        "model": "intfloat/multilingual-e5-small",
        "created_at": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
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

pub fn save_passage_cache(vectors: &[Vec<f32>], skills: &[SkillItem]) {
    if vectors.is_empty() {
        return;
    }

    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);

    let fp = catalog_fingerprint(skills);
    let manifest = serde_json::json!({
        "catalog_fingerprint": fp,
        "count": skills.len(),
        "created_at": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    });

    if let Ok(manifest_str) = serde_json::to_string(&manifest) {
        let _ = std::fs::write(manifest_path(), manifest_str);
    }

    let count = vectors.len() as u32;
    let dim = (vectors.first().map(|v| v.len()).unwrap_or(0)) as u32;
    let mut data = Vec::with_capacity(8 + (count as usize) * (dim as usize) * 4);
    data.extend_from_slice(&count.to_le_bytes());
    data.extend_from_slice(&dim.to_le_bytes());
    for vec in vectors {
        for val in vec {
            data.extend_from_slice(&val.to_le_bytes());
        }
    }
    let _ = std::fs::write(vectors_path(), data);
}

pub fn load_passage_cache(skills: &[SkillItem]) -> Option<Vec<Vec<f32>>> {
    let manifest_str = std::fs::read_to_string(manifest_path()).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_str).ok()?;
    let cached_fp = manifest.get("catalog_fingerprint")?.as_u64()?;
    let current_fp = catalog_fingerprint(skills);
    if cached_fp != current_fp {
        return None;
    }

    let data = std::fs::read(vectors_path()).ok()?;
    if data.len() < 8 {
        return None;
    }

    let count = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    let dim = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let expected_len = 8 + count * dim * 4;
    if data.len() != expected_len {
        return None;
    }

    let mut vectors = Vec::with_capacity(count);
    let mut offset = 8;
    for _ in 0..count {
        let mut vec = Vec::with_capacity(dim);
        for _ in 0..dim {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&data[offset..offset + 4]);
            vec.push(f32::from_le_bytes(bytes));
            offset += 4;
        }
        vectors.push(vec);
    }

    Some(vectors)
}
