//! Automatic monorepo package and multi-workspace detection.

use std::path::{Path, PathBuf};
use crate::context::multi_project::LinkedProject;

/// Auto-discovers workspace packages (Cargo, npm/pnpm, Go) under `root`.
pub fn auto_discover_workspaces(root: &Path) -> Vec<LinkedProject> {
    let mut projects = Vec::new();
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    // 1. Rust Cargo Workspace members
    let cargo_toml = root.join("Cargo.toml");
    if cargo_toml.exists() {
        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            parse_cargo_workspace_members(&content, root, &root_canon, &mut projects);
        }
    }

    // 2. Node / JS package.json workspaces
    let package_json = root.join("package.json");
    if package_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&package_json) {
            parse_package_json_workspaces(&content, root, &root_canon, &mut projects);
        }
    }

    projects
}

fn parse_cargo_workspace_members(
    content: &str,
    base: &Path,
    root_canon: &Path,
    projects: &mut Vec<LinkedProject>,
) {
    let mut in_members = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("members") && trimmed.contains('=') {
            in_members = true;
            extract_quoted_strings(trimmed, base, root_canon, projects);
            if trimmed.contains(']') {
                in_members = false;
            }
            continue;
        }
        if in_members {
            extract_quoted_strings(trimmed, base, root_canon, projects);
            if trimmed.starts_with(']') {
                in_members = false;
            }
        }
    }
}

fn extract_quoted_strings(
    line: &str,
    base: &Path,
    root_canon: &Path,
    projects: &mut Vec<LinkedProject>,
) {
    let parts: Vec<&str> = line.split('"').collect();
    for i in (1..parts.len()).step_by(2) {
        let trimmed = parts[i].trim();
        if !trimmed.is_empty() {
            add_member_package(base, trimmed, root_canon, projects);
        }
    }
}

fn parse_package_json_workspaces(
    content: &str,
    base: &Path,
    root_canon: &Path,
    projects: &mut Vec<LinkedProject>,
) {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(workspaces) = val.get("workspaces").and_then(|w| w.as_array()) {
            for item in workspaces {
                if let Some(pat) = item.as_str() {
                    add_member_package(base, pat, root_canon, projects);
                }
            }
        }
    }
}

fn add_member_package(
    base: &Path,
    pattern: &str,
    root_canon: &Path,
    projects: &mut Vec<LinkedProject>,
) {
    if pattern.contains('*') {
        let clean_pat = pattern.trim_end_matches("/*").trim_end_matches("\\*");
        let target_dir = base.join(clean_pat);
        if let Ok(entries) = std::fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    register_project_if_valid(&p, root_canon, projects);
                }
            }
        }
    } else {
        let target_dir = base.join(pattern);
        if target_dir.is_dir() {
            register_project_if_valid(&target_dir, root_canon, projects);
        }
    }
}

fn register_project_if_valid(
    path: &Path,
    root_canon: &Path,
    projects: &mut Vec<LinkedProject>,
) {
    if let Ok(canon) = path.canonicalize() {
        if canon == root_canon || projects.iter().any(|p| p.root_path == canon) {
            return;
        }
        let name = canon
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("subproject")
            .to_string();

        projects.push(LinkedProject {
            name,
            root_path: canon,
        });
    }
}
