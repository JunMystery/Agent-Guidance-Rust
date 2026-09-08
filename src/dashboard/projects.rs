//! Tracked projects discovery, status inspection, and prune management.

use anyhow::Result;
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::json_response;

/// Represents a tracked project with live filesystem existence check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedProject {
    pub name: String,
    pub path: String,
    pub status: String, // "active" or "missing"
    pub first_seen: i64,
    pub last_active: i64,
    pub total_calls: i64,
    pub tokens_saved: i64,
}

/// Normalizes project path string across platforms (uppercasing Windows drive, trimming trailing slashes, consistent separator).
pub fn normalize_project_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut s = trimmed.replace('/', "\\");
    while s.len() > 3 && (s.ends_with('\\') || s.ends_with('/')) {
        s.pop();
    }

    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        let first = s.chars().next().unwrap();
        if first.is_ascii_lowercase() {
            let upper = first.to_ascii_uppercase().to_string();
            s.replace_range(..1, &upper);
        }
    }

    let p = Path::new(&s);
    if let Ok(canonical) = p.canonicalize() {
        let c_str = canonical.to_string_lossy().to_string();
        let stripped = c_str.strip_prefix(r"\\?\").unwrap_or(&c_str);
        let mut res = stripped.replace('/', "\\");
        while res.len() > 3 && (res.ends_with('\\') || res.ends_with('/')) {
            res.pop();
        }
        if res.len() >= 2 && res.as_bytes()[1] == b':' {
            let first = res.chars().next().unwrap();
            if first.is_ascii_lowercase() {
                let upper = first.to_ascii_uppercase().to_string();
                res.replace_range(..1, &upper);
            }
        }
        return res;
    }

    s
}

/// Checks whether a path points to a unit test temporary directory that should not be tracked.
pub fn is_temp_project_path(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    if let Some(file_name) = Path::new(raw).file_name().and_then(|f| f.to_str()) {
        let fn_lower = file_name.to_lowercase();
        if fn_lower.starts_with("ag_tools_test_")
            || fn_lower.starts_with("deep_search_test_")
            || fn_lower.starts_with("ag_graph_mcp_test_")
            || fn_lower.starts_with("ag_neighborhood_test_")
            || fn_lower.starts_with("last_project_path_test_")
        {
            return true;
        }
    }
    if !cfg!(test) {
        if lower.contains("appdata\\local\\temp") || lower.contains("appdata/local/temp") {
            return true;
        }
        if lower.starts_with("/tmp") || lower.contains("/tmp/") || lower.contains("\\tmp\\") {
            return true;
        }
    }
    false
}

/// Lists all tracked projects from usage.db with dynamic status detection and deduplication.
pub fn list_tracked_projects(db_path: &Path) -> Result<Vec<TrackedProject>> {
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;

    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS tracked_projects (
            project_path TEXT PRIMARY KEY,
            project_name TEXT NOT NULL,
            first_seen INTEGER NOT NULL,
            last_active INTEGER NOT NULL,
            total_calls INTEGER DEFAULT 0,
            total_tokens_saved INTEGER DEFAULT 0
        )",
        [],
    );

    let mut stmt = conn.prepare(
        "SELECT project_path, project_name, first_seen, last_active, total_calls, total_tokens_saved
         FROM tracked_projects ORDER BY last_active DESC",
    )?;

    let mut map: HashMap<String, TrackedProject> = HashMap::new();

    let raw_rows = stmt.query_map([], |row| {
        let path_str: String = row.get(0)?;
        let name: String = row.get(1)?;
        let first_seen: i64 = row.get(2)?;
        let last_active: i64 = row.get(3)?;
        let total_calls: i64 = row.get(4)?;
        let tokens_saved: i64 = row.get(5)?;
        Ok((path_str, name, first_seen, last_active, total_calls, tokens_saved))
    })?;

    for row in raw_rows.flatten() {
        let (raw_path, name, first_seen, last_active, total_calls, tokens_saved) = row;
        if is_temp_project_path(&raw_path) {
            continue;
        }

        let norm_path = normalize_project_path(&raw_path);
        if norm_path.is_empty() || is_temp_project_path(&norm_path) {
            continue;
        }

        let entry = map.entry(norm_path.clone()).or_insert_with(|| {
            let status = if Path::new(&norm_path).exists() {
                "active".to_string()
            } else {
                "missing".to_string()
            };
            TrackedProject {
                name: name.clone(),
                path: norm_path.clone(),
                status,
                first_seen,
                last_active,
                total_calls: 0,
                tokens_saved: 0,
            }
        });

        entry.first_seen = entry.first_seen.min(first_seen);
        entry.last_active = entry.last_active.max(last_active);
        entry.total_calls += total_calls;
        entry.tokens_saved += tokens_saved;
    }

    let mut projects: Vec<TrackedProject> = map.into_values().collect();
    projects.sort_by(|a, b| b.last_active.cmp(&a.last_active));

    // Ensure current directory is represented if valid and not a temp dir
    if let Ok(curr) = std::env::current_dir() {
        let norm_curr = normalize_project_path(&curr.to_string_lossy());
        if !norm_curr.is_empty() && !is_temp_project_path(&norm_curr) && !projects.iter().any(|p| p.path == norm_curr) {
            let name = curr
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("current_project")
                .to_string();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            projects.insert(0, TrackedProject {
                name,
                path: norm_curr,
                status: "active".to_string(),
                first_seen: now,
                last_active: now,
                total_calls: 0,
                tokens_saved: 0,
            });
        }
    }

    Ok(projects)
}

/// Prunes projects whose directories no longer exist on the filesystem or are temp test folders.
pub fn prune_missing_projects(db_path: &Path) -> Result<usize> {
    if !db_path.exists() {
        return Ok(0);
    }

    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;

    let _ = conn.execute(
        "DELETE FROM tool_calls WHERE project_path LIKE '%Temp%' OR project_path LIKE '%ag_tools_test_%' OR project_path LIKE '%deep_search_test_%'",
        [],
    );

    let mut stmt = conn.prepare("SELECT project_path FROM tracked_projects")?;
    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut pruned_count = 0;
    for p in paths {
        if is_temp_project_path(&p) || !Path::new(&p).exists() {
            if let Ok(affected) = conn.execute("DELETE FROM tracked_projects WHERE project_path = ?", params![p]) {
                pruned_count += affected;
            }
            continue;
        }

        let norm = normalize_project_path(&p);
        if norm != p {
            let norm_exists: bool = conn
                .query_row(
                    "SELECT 1 FROM tracked_projects WHERE project_path = ?",
                    params![norm],
                    |_| Ok(true),
                )
                .unwrap_or(false);

            if norm_exists {
                let _ = conn.execute(
                    "UPDATE tracked_projects SET \
                        total_calls = total_calls + (SELECT total_calls FROM tracked_projects WHERE project_path = ?1), \
                        total_tokens_saved = total_tokens_saved + (SELECT total_tokens_saved FROM tracked_projects WHERE project_path = ?1), \
                        last_active = MAX(last_active, (SELECT last_active FROM tracked_projects WHERE project_path = ?1)) \
                     WHERE project_path = ?2",
                    params![p, norm],
                );
                let _ = conn.execute("DELETE FROM tracked_projects WHERE project_path = ?", params![p]);
            } else {
                let _ = conn.execute(
                    "UPDATE tracked_projects SET project_path = ? WHERE project_path = ?",
                    params![norm, p],
                );
            }
            let _ = conn.execute(
                "UPDATE tool_calls SET project_path = ? WHERE project_path = ?",
                params![norm, p],
            );
            pruned_count += 1;
        }
    }

    Ok(pruned_count)
}

/// HTTP API handler for GET /api/projects
pub(crate) fn handle_api_projects(request: tiny_http::Request, db_path: &PathBuf) {
    match list_tracked_projects(db_path) {
        Ok(projects) => json_response(request, 200, &json!({ "success": true, "projects": projects })),
        Err(e) => json_response(request, 500, &json!({ "success": false, "error": e.to_string() })),
    }
}

/// HTTP API handler for POST /api/projects/prune
pub(crate) fn handle_api_prune(request: tiny_http::Request, db_path: &PathBuf) {
    match prune_missing_projects(db_path) {
        Ok(pruned) => json_response(
            request,
            200,
            &json!({ "success": true, "pruned_count": pruned, "message": format!("Pruned {} missing projects", pruned) }),
        ),
        Err(e) => json_response(request, 500, &json!({ "success": false, "error": e.to_string() })),
    }
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;
