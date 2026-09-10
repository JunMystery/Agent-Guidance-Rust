use anyhow::Result;
use rusqlite::{params, Connection, OpenFlags};
use std::path::Path;

use super::projects_path::{is_temp_project_path, normalize_project_path};

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
                let root_name = Path::new(&norm).file_name().and_then(|n| n.to_str()).unwrap_or("project");
                let _ = conn.execute(
                    "UPDATE tracked_projects SET project_path = ?1, project_name = ?2 WHERE project_path = ?3",
                    params![norm, root_name, p],
                );
            }
            let _ = conn.execute(
                "UPDATE tool_calls SET project_path = ? WHERE project_path = ?",
                params![norm, p],
            );
            pruned_count += 1;
        }
    }

    // Normalize any legacy un-migrated paths remaining directly in tool_calls
    if let Ok(mut stmt_tc) = conn.prepare("SELECT DISTINCT project_path FROM tool_calls WHERE project_path IS NOT NULL AND project_path != ''") {
        if let Ok(tc_paths) = stmt_tc.query_map([], |row| row.get::<_, String>(0)) {
            let paths: Vec<String> = tc_paths.filter_map(|r| r.ok()).collect();
            for p in paths {
                if is_temp_project_path(&p) {
                    let _ = conn.execute("UPDATE tool_calls SET project_path = NULL WHERE project_path = ?", params![p]);
                    continue;
                }
                let norm = normalize_project_path(&p);
                if norm != p {
                    let _ = conn.execute("UPDATE tool_calls SET project_path = ? WHERE project_path = ?", params![norm, p]);
                }
            }
        }
    }

    Ok(pruned_count)
}
