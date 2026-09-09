use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::catalog::store::SkillItem;

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
    let data = PRECOMPUTED_VECTORS;
    if data.len() < 8 {
        return None;
    }

    let count = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    let dim = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let expected_len = 8 + count * dim * 4;
    if data.len() != expected_len {
        return None;
    }

    let manifest_fp = manifest.get("catalog_fingerprint").and_then(|v| v.as_u64());
    let current_fp = catalog_fingerprint(skills);

    // Fast path: exact match in length and catalog fingerprint
    if count == skills.len() && manifest_fp == Some(current_fp) {
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
        return Some(vectors);
    }

    // Dynamic alignment path: when workspace skills slightly differ from binary manifest
    let manifest_skills = manifest.get("skills")?.as_array()?;
    let mut name_to_vec = std::collections::HashMap::with_capacity(count);
    let mut offset = 8;
    for i in 0..count {
        let mut vec = Vec::with_capacity(dim);
        for _ in 0..dim {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&data[offset..offset + 4]);
            vec.push(f32::from_le_bytes(bytes));
            offset += 4;
        }
        if let Some(skill_name) = manifest_skills.get(i).and_then(|s| s.get("name")).and_then(|n| n.as_str()) {
            name_to_vec.insert(skill_name.to_lowercase(), vec);
        }
    }

    let mut aligned_vectors = Vec::with_capacity(skills.len());
    let zero_vec = vec![0.0f32; dim];
    for skill in skills {
        if let Some(v) = name_to_vec.get(&skill.name.to_lowercase()) {
            aligned_vectors.push(v.clone());
        } else {
            aligned_vectors.push(zero_vec.clone());
        }
    }

    Some(aligned_vectors)
}

pub use super::precomputed_gen::{generate_manifest_only, generate_precomputed_cache};

pub fn save_passage_cache(vectors: &[Vec<f32>], skills: &[SkillItem]) {
    if vectors.is_empty() {
        return;
    }

    let dir = cache_dir();
    let _ = std::fs::create_dir_all(&dir);

    let fp = catalog_fingerprint(skills);
    let skills_metadata: Vec<serde_json::Value> = skills
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
        "count": skills.len(),
        "created_at": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "skills": skills_metadata,
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
