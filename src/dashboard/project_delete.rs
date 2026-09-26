use anyhow::Result;
use rusqlite::{params, Connection, OpenFlags};
use serde_json::json;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::json_response;
use super::projects_path::normalize_project_path;
use super::StatsCache;

/// Removes a project from usage.db tracked projects and optionally deletes its code graph index.
pub fn delete_tracked_project(db_path: &Path, project_path: &str, delete_index: bool) -> Result<bool> {
    if !db_path.exists() {
        return Ok(false);
    }
    let norm = normalize_project_path(project_path);
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;

    let p1 = project_path.replace('\\', "/");
    let p2 = project_path.replace('/', "\\");
    let n1 = norm.replace('\\', "/");
    let n2 = norm.replace('/', "\\");

    let deleted: usize = conn.execute(
        "DELETE FROM tracked_projects WHERE project_path = ?1 OR project_path = ?2 OR project_path = ?3 OR project_path = ?4",
        params![p1, p2, n1, n2],
    )?;

    let _ = conn.execute(
        "DELETE FROM tool_calls WHERE project_path = ?1 OR project_path = ?2 OR project_path = ?3 OR project_path = ?4 \
         OR rtrim(project_path, '/\\') = ?1 OR rtrim(project_path, '/\\') = ?2",
        params![p1, p2, n1, n2],
    );

    let _ = conn.execute(
        "DELETE FROM project_skill_analytics WHERE project_path = ?1 OR project_path = ?2 OR project_path = ?3 OR project_path = ?4",
        params![p1, p2, n1, n2],
    );

    if delete_index && !norm.is_empty() {
        let p = Path::new(&norm);
        if p.exists() {
            let agent_ctx = p.join(".agent-context");
            if agent_ctx.exists() {
                let _ = std::fs::remove_file(agent_ctx.join("code_graph.db"));
                let _ = std::fs::remove_file(agent_ctx.join("code_graph.db-wal"));
                let _ = std::fs::remove_file(agent_ctx.join("code_graph.db-shm"));
                let _ = std::fs::remove_file(agent_ctx.join("communities.json"));
            }
        }
    }

    Ok(deleted > 0)
}

pub fn handle_api_delete_project(
    mut request: tiny_http::Request,
    db_path: &Path,
    cache: &Arc<Mutex<StatsCache>>,
) {
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    let payload: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();

    let project_path = payload
        .get("project")
        .or_else(|| payload.get("project_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if project_path.is_empty() || project_path == "all" {
        json_response(request, 400, &json!({
            "success": false,
            "error": "Valid project path required to delete"
        }));
        return;
    }

    let delete_index = payload
        .get("delete_index")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    match delete_tracked_project(db_path, project_path, delete_index) {
        Ok(found) => {
            if let Ok(mut guard) = cache.lock() {
                guard.data = None;
            }
            json_response(request, 200, &json!({
                "success": true,
                "deleted": found,
                "project": project_path,
                "index_deleted": delete_index,
                "message": format!("Project '{}' removed from dashboard registry", project_path)
            }));
        }
        Err(e) => json_response(request, 500, &json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_delete_tracked_project_removes_records() {
        let temp_dir = std::env::temp_dir().join("ag_delete_proj_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test_usage.db");
        let _ = std::fs::remove_file(&db_path);

        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE tracked_projects (project_path TEXT PRIMARY KEY, project_name TEXT, first_seen INT, last_active INT, total_calls INT, total_tokens_saved INT)",
            [],
        ).unwrap();
        conn.execute(
            "CREATE TABLE tool_calls (id INTEGER PRIMARY KEY, tool_name TEXT, project_path TEXT)",
            [],
        ).unwrap();
        conn.execute(
            "CREATE TABLE project_skill_analytics (id INTEGER PRIMARY KEY, project_path TEXT, skill_id TEXT, used_at INT)",
            [],
        ).unwrap();

        let proj = "e:/Github/test_project";
        conn.execute(
            "INSERT INTO tracked_projects VALUES (?1, 'test_project', 100, 100, 5, 200)",
            params![proj],
        ).unwrap();
        conn.execute(
            "INSERT INTO tool_calls VALUES (1, 'read', ?1)",
            params![proj],
        ).unwrap();

        let res = delete_tracked_project(&db_path, proj, false).unwrap();
        assert!(res);

        let remaining: i64 = conn.query_row("SELECT COUNT(*) FROM tracked_projects", [], |r| r.get(0)).unwrap();
        assert_eq!(remaining, 0);

        let remaining_tc: i64 = conn.query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0)).unwrap();
        assert_eq!(remaining_tc, 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
