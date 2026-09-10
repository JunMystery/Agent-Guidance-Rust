use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::json_response;
pub use super::projects_path::{is_temp_project_path, normalize_project_path};
pub use super::projects_prune::prune_missing_projects;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrackedProject {
    pub name: String,
    pub path: String,
    pub status: String, // "active" or "missing"
    pub first_seen: i64,
    pub last_active: i64,
    pub total_calls: i64,
    pub tokens_saved: i64,
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
                name,
                path: norm_path,
                status,
                first_seen,
                last_active,
                total_calls: 0,
                tokens_saved: 0,
            }
        });

        entry.total_calls += total_calls;
        entry.tokens_saved += tokens_saved;
        if first_seen < entry.first_seen {
            entry.first_seen = first_seen;
        }
        if last_active > entry.last_active {
            entry.last_active = last_active;
        }
    }

    let mut projects: Vec<TrackedProject> = map.into_values().collect();
    projects.sort_by(|a, b| b.last_active.cmp(&a.last_active));

    // Ensure current directory is included if active
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
