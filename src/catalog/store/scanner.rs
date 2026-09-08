use std::fs;
use std::path::Path;
use super::{SkillItem, SkillSource};

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

pub(crate) fn scan_skill_dir_recursive(root_dir: &Path, current_dir: &Path, results: &mut Vec<SkillItem>) {
    let dir_name = current_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if is_ignored_dir(&dir_name) {
        return;
    }

    // 1. Standard SKILL.md check
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

    // 2. OWASP / Cheatsheets directory check
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

    // 3. Recurse into subdirectories
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_skill_dir_recursive(root_dir, &path, results);
            }
        }
    }
}

pub(crate) fn extract_frontmatter_name(content: &str) -> Option<String> {
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

pub(crate) fn extract_skill_name(content: &str, fallback: &str) -> String {
    if let Some(fm_name) = extract_frontmatter_name(content) {
        return fm_name;
    }
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
