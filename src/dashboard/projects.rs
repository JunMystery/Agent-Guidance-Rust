//! Tracked projects discovery, status inspection, and prune management.

use anyhow::Result;
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
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

/// Lists all tracked projects from usage.db with dynamic status detection.
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

    let mut projects: Vec<TrackedProject> = stmt
        .query_map([], |row| {
            let path_str: String = row.get(0)?;
            let name: String = row.get(1)?;
            let first_seen: i64 = row.get(2)?;
            let last_active: i64 = row.get(3)?;
            let total_calls: i64 = row.get(4)?;
            let tokens_saved: i64 = row.get(5)?;

            let status = if Path::new(&path_str).exists() {
                "active".to_string()
            } else {
                "missing".to_string()
            };

            Ok(TrackedProject {
                name,
                path: path_str,
                status,
                first_seen,
                last_active,
                total_calls,
                tokens_saved,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Ensure current directory is represented if valid
    if let Ok(curr) = std::env::current_dir() {
        let curr_str = curr.to_string_lossy().to_string();
        if !projects.iter().any(|p| p.path == curr_str) {
            let name = curr.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("current_project")
                .to_string();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            projects.insert(0, TrackedProject {
                name,
                path: curr_str,
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

/// Prunes projects whose directories no longer exist on the filesystem.
pub fn prune_missing_projects(db_path: &Path) -> Result<usize> {
    if !db_path.exists() {
        return Ok(0);
    }

    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;

    let mut stmt = conn.prepare("SELECT project_path FROM tracked_projects")?;
    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut pruned_count = 0;
    for p in paths {
        if !Path::new(&p).exists() {
            if let Ok(affected) = conn.execute("DELETE FROM tracked_projects WHERE project_path = ?", params![p]) {
                pruned_count += affected;
            }
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
