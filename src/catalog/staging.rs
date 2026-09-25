use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use super::store::scanner::scan_skill_dir_recursive;
use super::store::{SkillItem, SkillSource};
use super::tombstone::load_tombstones;

pub fn staging_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("staging").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agent-guidance").join("staging").join("skills"))
}

pub fn ensure_staging_dir() -> anyhow::Result<PathBuf> {
    let dir = staging_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn scan_staging_skills() -> Vec<SkillItem> {
    let dir = staging_dir();
    if !dir.exists() || !dir.is_dir() {
        return Vec::new();
    }
    let mut results = Vec::new();
    scan_skill_dir_recursive(&dir, &dir, &mut results);
    // Mark source as Staged
    for item in &mut results {
        if let SkillSource::LocalWorkspace(path) = &item.source {
            item.relative_path = format!("staging/{}", item.name);
            item.source = SkillSource::LocalWorkspace(path.clone());
        }
    }
    results
}

pub fn purge_staging_skill(slug: &str) -> anyhow::Result<bool> {
    let dir = staging_dir();
    let norm = slug.trim().to_lowercase();
    if norm.is_empty() {
        return Ok(false);
    }

    let target_dir = dir.join(&norm);
    if target_dir.exists() && target_dir.is_dir() {
        fs::remove_dir_all(&target_dir)?;
        return Ok(true);
    }

    // Also check for direct markdown file e.g. skill-name.md
    let target_file = dir.join(format!("{}.md", norm));
    if target_file.exists() && target_file.is_file() {
        fs::remove_file(&target_file)?;
        return Ok(true);
    }

    Ok(false)
}

/// Applies staged precedence over embedded skills, and filters out tombstoned skills.
pub fn merge_with_precedence(
    embedded: Vec<SkillItem>,
    staged: Vec<SkillItem>,
    tombstones: &HashSet<String>,
) -> Vec<SkillItem> {
    let mut map: HashMap<String, SkillItem> = HashMap::with_capacity(embedded.len() + staged.len());

    // 1. Insert embedded skills that are not tombstoned
    for item in embedded {
        let key = item.name.to_lowercase();
        if !tombstones.contains(&key) {
            map.insert(key, item);
        }
    }

    // 2. Insert or override with staged skills (staged skills take absolute precedence)
    for item in staged {
        let key = item.name.to_lowercase();
        if !tombstones.contains(&key) {
            map.insert(key, item);
        }
    }

    let mut merged: Vec<SkillItem> = map.into_values().collect();
    merged.sort_by(|a, b| a.name.cmp(&b.name));
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_precedence_and_tombstones() {
        let embedded = vec![
            SkillItem {
                name: "skill-a".to_string(),
                relative_path: "skills/skill-a/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "original-a".to_string(),
            },
            SkillItem {
                name: "skill-b".to_string(),
                relative_path: "skills/skill-b/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "original-b".to_string(),
            },
        ];

        let staged = vec![
            SkillItem {
                name: "skill-a".to_string(),
                relative_path: "staging/skill-a".to_string(),
                source: SkillSource::LocalWorkspace("/path".to_string()),
                content: "staged-a".to_string(),
            },
            SkillItem {
                name: "skill-c".to_string(),
                relative_path: "staging/skill-c".to_string(),
                source: SkillSource::LocalWorkspace("/path-c".to_string()),
                content: "staged-c".to_string(),
            },
        ];

        let mut tombstones = HashSet::new();
        tombstones.insert("skill-b".to_string());

        let merged = merge_with_precedence(embedded, staged, &tombstones);
        assert_eq!(merged.len(), 2);

        let skill_a = merged.iter().find(|s| s.name == "skill-a").unwrap();
        assert_eq!(skill_a.content, "staged-a");

        let skill_c = merged.iter().find(|s| s.name == "skill-c").unwrap();
        assert_eq!(skill_c.content, "staged-c");

        assert!(merged.iter().all(|s| s.name != "skill-b"));
    }
}
