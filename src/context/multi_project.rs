//! Multi-project cross-referencing and virtual monorepo discovery.

use std::path::{Path, PathBuf};
use serde_json::Value;
use crate::context::db::CodeGraphDb;

/// A linked secondary project or workspace package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedProject {
    pub name: String,
    pub root_path: PathBuf,
}

impl LinkedProject {
    /// Opens the code graph database of this linked project in read-only mode.
    pub fn open_db(&self) -> Option<CodeGraphDb> {
        let db_path = self.root_path.join(".agent-context").join("code_graph.db");
        if db_path.exists() {
            CodeGraphDb::open_read_only(&db_path).ok()
        } else {
            None
        }
    }
}

/// Discovers linked projects from configuration, environment variables, and monorepo structure.
pub fn discover_linked_projects(primary_path: &Path) -> Vec<LinkedProject> {
    let mut projects = Vec::new();
    let primary_canonical = primary_path.canonicalize().unwrap_or_else(|_| primary_path.to_path_buf());

    // 1. From .agent-context/linked_projects.json
    let config_file = primary_path.join(".agent-context").join("linked_projects.json");
    if let Ok(content) = std::fs::read_to_string(&config_file) {
        if let Ok(val) = serde_json::from_str::<Value>(&content) {
            if let Some(arr) = val.as_array() {
                for item in arr {
                    if let Some(path_str) = item.as_str() {
                        add_project_if_valid(primary_path, path_str, None, &primary_canonical, &mut projects);
                    } else if let Some(obj) = item.as_object() {
                        let path_str = obj.get("path").and_then(|p| p.as_str()).unwrap_or("");
                        let name_str = obj.get("name").and_then(|n| n.as_str());
                        if !path_str.is_empty() {
                            add_project_if_valid(primary_path, path_str, name_str, &primary_canonical, &mut projects);
                        }
                    }
                }
            }
        }
    }

    // 2. From AGENT_GUIDANCE_LINKED_PROJECTS environment variable
    if let Ok(env_val) = std::env::var("AGENT_GUIDANCE_LINKED_PROJECTS") {
        let separator = if cfg!(windows) { ';' } else { ':' };
        for path_str in env_val.split(separator) {
            let trimmed = path_str.trim();
            if !trimmed.is_empty() {
                add_project_if_valid(primary_path, trimmed, None, &primary_canonical, &mut projects);
            }
        }
    }

    projects
}

fn add_project_if_valid(
    base_dir: &Path,
    path_str: &str,
    name_override: Option<&str>,
    primary_canonical: &Path,
    projects: &mut Vec<LinkedProject>,
) {
    let raw_path = Path::new(path_str);
    let abs_path = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        base_dir.join(raw_path)
    };

    if let Ok(canon) = abs_path.canonicalize() {
        if canon == primary_canonical {
            return; // Avoid self-link
        }
        if projects.iter().any(|p| p.root_path == canon) {
            return; // Avoid duplicate
        }
        let name = name_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| canon.file_name().and_then(|n| n.to_str()).unwrap_or("project").to_string());

        projects.push(LinkedProject {
            name,
            root_path: canon,
        });
    }
}

/// Cascades symbol search across all linked projects.
pub fn search_linked_symbols(
    linked: &[LinkedProject],
    query: &str,
    limit_per_project: usize,
) -> Vec<(String, String, String, usize)> {
    let mut results = Vec::new();
    for proj in linked {
        if let Some(db) = proj.open_db() {
            if let Ok(syms) = db.search_symbols(query, limit_per_project) {
                for (path, name, line) in syms {
                    results.push((proj.name.clone(), path, name, line));
                }
            }
        }
    }
    results
}

/// Cascades FTS content search across all linked projects.
pub fn search_linked_content(
    linked: &[LinkedProject],
    query: &str,
    limit_per_project: usize,
) -> Vec<(String, String, usize, usize, String)> {
    let mut results = Vec::new();
    for proj in linked {
        if let Some(db) = proj.open_db() {
            if let Ok(hits) = db.search_content_fts(query, limit_per_project) {
                for (path, start, end, snip) in hits {
                    results.push((proj.name.clone(), path, start, end, snip));
                }
            }
        }
    }
    results
}

/// Resolves a cross-project relative file path (e.g. `linked:shared-lib/src/util.rs`).
pub fn resolve_cross_project_path<'a>(
    rel_path: &'a str,
    linked: &'a [LinkedProject],
) -> Option<(&'a LinkedProject, &'a str)> {
    let trimmed = rel_path.strip_prefix("linked:").unwrap_or(rel_path);
    let (prefix, subpath) = trimmed.split_once(['/', '\\'])?;

    for proj in linked {
        if proj.name.eq_ignore_ascii_case(prefix) {
            return Some((proj, subpath));
        }
    }
    None
}

#[cfg(test)]
#[path = "multi_project_tests.rs"]
mod tests;
