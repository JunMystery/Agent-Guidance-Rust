use std::collections::HashSet;
use anyhow::{Context, Result};
use tracing::info;

use super::binary_format::{SkillRecord, SkillsBinary, VectorBinary};
use super::precomputed::{cache_dir, skills_bin_path, vectors_path};
use crate::catalog::checksum::{compute_catalog_hash, load_manifest, save_manifest};
use crate::catalog::staging::purge_staging_skill;
use crate::catalog::tombstone::add_tombstones;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompactionResult {
    pub deleted_count: usize,
    pub remaining_count: usize,
    pub purged_staging_count: usize,
    pub new_catalog_hash: String,
}

pub fn delete_skills_and_compact(slugs: &[String], purge_staging: bool) -> Result<CompactionResult> {
    if slugs.is_empty() {
        return Ok(CompactionResult {
            deleted_count: 0,
            remaining_count: 0,
            purged_staging_count: 0,
            new_catalog_hash: String::new(),
        });
    }

    let slug_set: HashSet<String> = slugs.iter().map(|s| s.trim().to_lowercase()).collect();

    // 1. Record tombstones to prevent revival
    add_tombstones(&slug_set)?;

    // 2. Optionally purge from staging directory
    let mut purged_staging_count = 0;
    if purge_staging {
        for slug in &slug_set {
            if purge_staging_skill(slug).unwrap_or(false) {
                purged_staging_count += 1;
            }
        }
    }

    // 3. Compact skills.bin and vectors.bin if they exist
    let skills_path = skills_bin_path();
    let v_path = vectors_path();

    let mut remaining_skills = Vec::new();
    let mut remaining_vectors = Vec::new();
    let mut deleted_count = 0;

    let has_skills_bin = skills_path.exists();
    let has_vectors_bin = v_path.exists();

    if has_skills_bin && has_vectors_bin {
        let skills_data = std::fs::read(&skills_path).context("Failed to read skills.bin")?;
        let vectors_data = std::fs::read(&v_path).context("Failed to read vectors.bin")?;

        let sb = SkillsBinary::deserialize(&skills_data)?;
        let vb = VectorBinary::deserialize(&vectors_data)?;

        for (idx, skill) in sb.skills.into_iter().enumerate() {
            if slug_set.contains(&skill.name.to_lowercase()) {
                deleted_count += 1;
                continue;
            }
            if let Some(v) = vb.vectors.get(idx) {
                remaining_vectors.push(v.clone());
            }
            remaining_skills.push(skill);
        }

        // Atomically write compacted binaries
        let new_sb = SkillsBinary::new(remaining_skills.clone());
        let new_vb = VectorBinary::new(remaining_vectors);

        let sb_tmp = skills_path.with_extension("tmp");
        std::fs::write(&sb_tmp, new_sb.serialize())?;
        std::fs::rename(sb_tmp, &skills_path)?;

        let vb_tmp = v_path.with_extension("tmp");
        std::fs::write(&vb_tmp, new_vb.serialize())?;
        std::fs::rename(vb_tmp, &v_path)?;

        info!(
            "Compacted binaries: removed {} skills, {} remaining",
            deleted_count,
            remaining_skills.len()
        );
    } else {
        deleted_count = slug_set.len();
    }

    // 4. Update skills_manifest.json
    let mut new_catalog_hash = String::new();
    if let Some(mut manifest) = load_manifest() {
        manifest.skills.retain(|k, _| !slug_set.contains(&k.to_lowercase()));
        manifest.count = manifest.skills.len();
        let entries: Vec<_> = manifest.skills.values().cloned().collect();
        new_catalog_hash = compute_catalog_hash(&entries);
        manifest.catalog_hash = new_catalog_hash.clone();
        save_manifest(&manifest)?;
    }

    Ok(CompactionResult {
        deleted_count,
        remaining_count: remaining_skills.len(),
        purged_staging_count,
        new_catalog_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compaction_empty() {
        let res = delete_skills_and_compact(&[], false).unwrap();
        assert_eq!(res.deleted_count, 0);
    }

    #[test]
    fn test_compaction_10_to_7_skills() {
        let mut skills = Vec::new();
        let mut vectors = Vec::new();
        for i in 0..10 {
            skills.push(SkillRecord {
                name: format!("skill-{i}"),
                relative_path: format!("skills/skill-{i}/SKILL.md"),
                content: format!("# Skill {i}\nContent for {i}"),
            });
            vectors.push(vec![i as f32, (i * 2) as f32, (i * 3) as f32, (i * 4) as f32]);
        }

        let sb = SkillsBinary::new(skills);
        let vb = VectorBinary::new(vectors);
        let sb_bytes = sb.serialize();
        let vb_bytes = vb.serialize();

        let decoded_sb = SkillsBinary::deserialize(&sb_bytes).unwrap();
        let decoded_vb = VectorBinary::deserialize(&vb_bytes).unwrap();
        assert_eq!(decoded_sb.skills.len(), 10);
        assert_eq!(decoded_vb.count, 10);

        let to_delete: HashSet<String> = ["skill-2", "skill-5", "skill-8"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let mut rem_skills = Vec::new();
        let mut rem_vectors = Vec::new();
        for (idx, skill) in decoded_sb.skills.into_iter().enumerate() {
            if to_delete.contains(&skill.name) {
                continue;
            }
            rem_skills.push(skill);
            rem_vectors.push(decoded_vb.vectors[idx].clone());
        }

        let compacted_sb = SkillsBinary::new(rem_skills);
        let compacted_vb = VectorBinary::new(rem_vectors);

        let c_sb_bytes = compacted_sb.serialize();
        let c_vb_bytes = compacted_vb.serialize();

        let final_sb = SkillsBinary::deserialize(&c_sb_bytes).unwrap();
        let final_vb = VectorBinary::deserialize(&c_vb_bytes).unwrap();

        assert_eq!(final_sb.skills.len(), 7);
        assert_eq!(final_vb.count, 7);
        assert_eq!(final_vb.dim, 4);

        for skill in &final_sb.skills {
            assert!(!to_delete.contains(&skill.name));
        }

        assert_eq!(final_sb.skills[0].name, "skill-0");
        assert_eq!(final_sb.skills[1].name, "skill-1");
        assert_eq!(final_sb.skills[2].name, "skill-3"); // skipped 2
        assert_eq!(final_sb.skills[3].name, "skill-4");
        assert_eq!(final_sb.skills[4].name, "skill-6"); // skipped 5
        assert_eq!(final_sb.skills[5].name, "skill-7");
        assert_eq!(final_sb.skills[6].name, "skill-9"); // skipped 8

        assert_eq!(final_vb.vectors[2], vec![3.0, 6.0, 9.0, 12.0]);
    }
}
