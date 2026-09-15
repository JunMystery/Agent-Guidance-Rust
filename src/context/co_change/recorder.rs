//! Pairwise co-change recording into SQLite for evolutionary coupling graph.

use anyhow::Result;
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

/// Normalizes a path string to consistent POSIX forward slashes and trims leading slashes.
pub fn clean_path(path: &str) -> String {
    path.trim()
        .trim_start_matches(|c| c == '/' || c == '\\')
        .replace('\\', "/")
}

/// Normalizes a pair of file paths to ensure deterministic alphabetical order (file_a < file_b).
pub fn normalize_file_pair(a: &str, b: &str) -> Option<(String, String)> {
    let clean_a = clean_path(a);
    let clean_b = clean_path(b);

    if clean_a.is_empty() || clean_b.is_empty() || clean_a == clean_b {
        return None;
    }

    if clean_a < clean_b {
        Some((clean_a, clean_b))
    } else {
        Some((clean_b, clean_a))
    }
}

/// Records a pairwise co-change into co_change_edges table.
pub fn record_co_change(conn: &Connection, file_a: &str, file_b: &str) -> Result<bool> {
    let Some((fa, fb)) = normalize_file_pair(file_a, file_b) else {
        return Ok(false);
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    conn.execute(
        "INSERT INTO co_change_edges (file_a, file_b, co_change_count, last_changed_at, confidence)
         VALUES (?1, ?2, 1, ?3, 0.5)
         ON CONFLICT(file_a, file_b) DO UPDATE SET
            co_change_count = co_change_count + 1,
            last_changed_at = ?3,
            confidence = MIN(1.0, 0.5 + (co_change_count + 1) * 0.1)",
        params![fa, fb, now],
    )?;

    Ok(true)
}

/// Records pairwise co-changes for all unique files touched in an edit session.
pub fn record_session_co_changes(conn: &Connection, files: &[String]) -> Result<usize> {
    let unique_files: Vec<String> = files
        .iter()
        .map(|f| clean_path(f))
        .filter(|f| !f.is_empty())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    if unique_files.len() < 2 {
        return Ok(0);
    }

    let mut count = 0;
    for i in 0..unique_files.len() {
        for j in (i + 1)..unique_files.len() {
            if record_co_change(conn, &unique_files[i], &unique_files[j])? {
                count += 1;
            }
        }
    }

    Ok(count)
}
