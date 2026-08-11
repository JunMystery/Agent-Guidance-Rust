use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Embed)]
#[folder = "skills/"]
pub struct SkillAssets;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillSource {
    Embedded,
    LocalWorkspace(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillItem {
    pub name: String,
    pub relative_path: String,
    pub source: SkillSource,
    pub content: String,
}

pub fn list_embedded_skills() -> Vec<String> {
    SkillAssets::iter()
        .map(|path| path.as_ref().to_string())
        .collect()
}

pub fn get_embedded_skill(path: &str) -> Option<String> {
    SkillAssets::get(path).and_then(|file| {
        std::str::from_utf8(file.data.as_ref())
            .ok()
            .map(|s| s.to_string())
    })
}

pub fn scan_workspace_skills(proj_path: &Path) -> Vec<SkillItem> {
    let mut results = Vec::new();
    let mut search_dirs = vec![
        proj_path.join(".agents").join("skills"),
        proj_path.join(".opencode").join("skills"),
        proj_path.join(".claude").join("skills"),
    ];

    if let Some(home) = dirs::home_dir() {
        search_dirs.push(home.join(".agents").join("skills"));
        search_dirs.push(home.join(".agent-guidance").join("skills"));
    }

    for base_dir in search_dirs {
        if base_dir.exists() && base_dir.is_dir() {
            scan_skill_dir_recursive(&base_dir, &base_dir, &mut results);
        }
    }

    results
}

fn scan_skill_dir_recursive(root_dir: &Path, current_dir: &Path, results: &mut Vec<SkillItem>) {
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_skill_dir_recursive(root_dir, &path, results);
            } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    let fallback_name = if filename.to_lowercase() == "skill.md" {
                        path.parent()
                            .and_then(|p| p.file_name())
                            .and_then(|s| s.to_str())
                            .unwrap_or("custom-skill")
                            .to_string()
                    } else {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("custom-skill")
                            .replace('_', " ")
                    };

                    let name = extract_frontmatter_name(&content).unwrap_or(fallback_name);
                    let rel_path = path
                        .strip_prefix(root_dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();

                    results.push(SkillItem {
                        name,
                        relative_path: rel_path.clone(),
                        source: SkillSource::LocalWorkspace(path.to_string_lossy().to_string()),
                        content,
                    });
                }
            }
        }
    }
}

fn extract_frontmatter_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name:") {
            let val = trimmed.trim_start_matches("name:").trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

pub fn load_all_skills(proj_path: &Path) -> Vec<SkillItem> {
    let mut skills = Vec::new();

    // 1. Embedded skills
    for path in list_embedded_skills() {
        if let Some(content) = get_embedded_skill(&path) {
            let name = path.split('/').next().unwrap_or(&path).to_string();

            skills.push(SkillItem {
                name,
                relative_path: path.clone(),
                source: SkillSource::Embedded,
                content,
            });
        }
    }

    // 2. Scanned workspace local skills
    let local_skills = scan_workspace_skills(proj_path);
    for local in local_skills {
        if !skills.iter().any(|s| s.name == local.name) {
            skills.push(local);
        }
    }

    skills
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_extract_frontmatter_name() {
        let content = "---\nname: my-test-skill\ndescription: test\n---";
        assert_eq!(
            extract_frontmatter_name(content),
            Some("my-test-skill".to_string())
        );
    }

    #[test]
    fn test_scan_workspace_skills() {
        let tmp_dir = std::env::temp_dir().join("test_agent_skills");
        let skill_dir = tmp_dir.join(".agents").join("skills").join("test-skill");
        let _ = fs::create_dir_all(&skill_dir);
        let skill_file = skill_dir.join("SKILL.md");
        let mut file = File::create(&skill_file).unwrap();
        writeln!(file, "---\nname: test-skill\n---").unwrap();

        let scanned = scan_workspace_skills(&tmp_dir);
        assert!(!scanned.is_empty());
        assert_eq!(scanned[0].name, "test-skill");

        let _ = fs::remove_dir_all(tmp_dir);
    }
}
