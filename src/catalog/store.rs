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
    if let Some(file) = SkillAssets::get(path) {
        return std::str::from_utf8(file.data.as_ref())
            .ok()
            .map(|s| s.to_string());
    }
    let trimmed = path.trim_start_matches("skills/").trim_start_matches('/');
    let variations = [
        format!("{}/SKILL.md", trimmed),
        format!("skills/{}/SKILL.md", trimmed),
        format!("{}.md", trimmed),
        trimmed.to_string(),
    ];
    for v in &variations {
        if let Some(file) = SkillAssets::get(v) {
            return std::str::from_utf8(file.data.as_ref())
                .ok()
                .map(|s| s.to_string());
        }
    }
    None
}

pub fn get_skills_target_dir() -> std::path::PathBuf {
    if cfg!(test) {
        return std::env::temp_dir()
            .join(format!("agent-guidance-skills-test-{}", std::process::id()));
    }
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("skills"))
        .unwrap_or_else(|| std::path::PathBuf::from(".agent-guidance-skills"))
}

/// Extracts all embedded skills from the binary directly to disk in ~/.agent-guidance/skills/
pub fn sync_embedded_skills_to_disk() -> anyhow::Result<usize> {
    let target_dir = get_skills_target_dir();
    fs::create_dir_all(&target_dir)?;

    let mut count = 0;
    for path in SkillAssets::iter() {
        if let Some(file) = SkillAssets::get(path.as_ref()) {
            let target_file = target_dir.join(path.as_ref());
            if let Some(parent) = target_file.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target_file, file.data.as_ref())?;
            count += 1;
        }
    }
    Ok(count)
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

fn is_ignored_dir(dir_name: &str) -> bool {
    let lower = dir_name.to_lowercase();
    matches!(
        lower.as_str(),
        ".git"
            | ".github"
            | "node_modules"
            | "target"
            | "assets"
            | "img"
            | "images"
            | "dist"
            | "build"
            | "references"
            | "scripts"
            | "__pycache__"
            | "vendor"
            | "docs"
    )
}

fn is_ignored_doc_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    matches!(
        lower.as_str(),
        "readme.md"
            | "changelog.md"
            | "contributing.md"
            | "license.md"
            | "index.md"
            | "template.md"
            | "_template.md"
            | "security.md"
            | "code_of_conduct.md"
            | "authors.md"
    )
}

fn scan_skill_dir_recursive(root_dir: &Path, current_dir: &Path, results: &mut Vec<SkillItem>) {
    let dir_name = current_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if is_ignored_dir(&dir_name) {
        return;
    }

    // 1. Standard SKILL.md check: if this directory contains a SKILL.md, load it and stop recursing
    let skill_md = current_dir.join("SKILL.md");
    if skill_md.is_file() {
        if let Ok(content) = fs::read_to_string(&skill_md) {
            let fallback_name = current_dir
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("custom-skill")
                .to_string();
            let name = extract_frontmatter_name(&content).unwrap_or(fallback_name);
            let rel_path = skill_md
                .strip_prefix(root_dir)
                .unwrap_or(&skill_md)
                .to_string_lossy()
                .to_string();
            results.push(SkillItem {
                name,
                relative_path: rel_path,
                source: SkillSource::LocalWorkspace(skill_md.to_string_lossy().to_string()),
                content,
            });
        }
        return;
    }

    // 2. OWASP / Cheatsheets directory check: plain markdown cheatsheets
    if dir_name == "cheatsheets" {
        if let Ok(entries) = fs::read_dir(current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if is_ignored_doc_file(filename) {
                        continue;
                    }
                    if let Ok(content) = fs::read_to_string(&path) {
                        let fallback_name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("cheatsheet")
                            .replace('_', "-")
                            .to_lowercase();
                        let name = extract_skill_name(&content, &fallback_name);
                        let rel_path = path
                            .strip_prefix(root_dir)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string();
                        results.push(SkillItem {
                            name,
                            relative_path: rel_path,
                            source: SkillSource::LocalWorkspace(path.to_string_lossy().to_string()),
                            content,
                        });
                    }
                }
            }
        }
        return;
    }

    // 3. Recurse into subdirectories (e.g. skills/, vendor skill subdirs)
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_skill_dir_recursive(root_dir, &path, results);
            }
        }
    }
}

fn extract_frontmatter_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name:") {
            let val = trimmed.trim_start_matches("name:").trim();
            let cleaned = val.trim_matches('"').trim_matches('\'').trim();
            if !cleaned.is_empty() {
                return Some(cleaned.to_string());
            }
        }
    }
    None
}

fn extract_skill_name(content: &str, fallback: &str) -> String {
    if let Some(fm_name) = extract_frontmatter_name(content) {
        return fm_name;
    }
    // Check first heading if present (e.g. "# SQL Injection Prevention Cheat Sheet")
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            let title = trimmed.trim_start_matches("# ").trim();
            if !title.is_empty() {
                let slug: String = title
                    .to_lowercase()
                    .replace(' ', "-")
                    .replace('_', "-")
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-')
                    .collect();
                if !slug.is_empty() {
                    return slug;
                }
            }
        }
    }
    fallback.to_string()
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

    #[test]
    fn test_scan_ignores_non_skill_md_files() {
        let tmp_dir = std::env::temp_dir().join(format!(
            "test_skill_filter_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let skills_root = tmp_dir.join("skills");

        // Simulate a cloned repo with lots of non-skill .md files
        let repo_dir = skills_root.join("cloned-repo");
        let _ = fs::create_dir_all(&repo_dir);
        File::create(repo_dir.join("README.md")).unwrap().write_all(b"# Repo readme").unwrap();
        File::create(repo_dir.join("CHANGELOG.md")).unwrap().write_all(b"# Changes").unwrap();

        let docs_dir = repo_dir.join("docs");
        let _ = fs::create_dir_all(&docs_dir);
        File::create(docs_dir.join("guide.md")).unwrap().write_all(b"# Guide").unwrap();

        // Place a real skill nested inside the repo
        let real_skill = repo_dir.join("sub").join("real-skill");
        let _ = fs::create_dir_all(&real_skill);
        File::create(real_skill.join("SKILL.md")).unwrap()
            .write_all(b"---\nname: real-skill\n---\nActual skill content").unwrap();

        // Also place reference docs next to a skill (should NOT be loaded)
        let refs_dir = real_skill.join("references");
        let _ = fs::create_dir_all(&refs_dir);
        File::create(refs_dir.join("api-docs.md")).unwrap().write_all(b"# API reference").unwrap();

        // Test scan_skill_dir_recursive directly to isolate from global home dirs
        let mut results = Vec::new();
        scan_skill_dir_recursive(&skills_root, &skills_root, &mut results);

        // Only the real SKILL.md should be found — not README, CHANGELOG, guide, or api-docs
        assert_eq!(results.len(), 1, "Expected exactly 1 skill, got: {:?}", results.iter().map(|s| &s.name).collect::<Vec<_>>());
        assert_eq!(results[0].name, "real-skill");

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_scan_owasp_cheatsheets_plain_mds() {
        let tmp_dir = std::env::temp_dir().join(format!(
            "test_owasp_cheatsheets_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let owasp_root = tmp_dir.join("owasp");
        let cheatsheets_dir = owasp_root.join("cheatsheets");
        let _ = fs::create_dir_all(&cheatsheets_dir);

        // Plain MD cheatsheet
        File::create(cheatsheets_dir.join("SQL_Injection_Prevention_Cheat_Sheet.md"))
            .unwrap()
            .write_all(b"# SQL Injection Prevention Cheat Sheet\n\nDefense in depth against SQLi.")
            .unwrap();

        // Ignored readme in cheatsheets dir
        File::create(cheatsheets_dir.join("README.md"))
            .unwrap()
            .write_all(b"# Cheatsheets Index")
            .unwrap();

        let mut results = Vec::new();
        scan_skill_dir_recursive(&owasp_root, &owasp_root, &mut results);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "sql-injection-prevention-cheat-sheet");
        assert!(results[0].content.contains("Defense in depth against SQLi"));

        let _ = fs::remove_dir_all(tmp_dir);
    }
}
