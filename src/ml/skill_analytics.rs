//! Cross-Session Skill Analytics & Historical Contextual Boost.

use rusqlite::{params, Connection, OpenFlags};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::catalog::store::SkillItem;

/// Record a skill usage for a specific project into usage.db.
pub fn record_skill_usage(project_path: &Path, skill_name: &str) {
    let clean_skill = skill_name.trim().to_lowercase();
    if clean_skill.is_empty() {
        return;
    }

    let norm_path = crate::dashboard::projects_path::normalize_project_path(&project_path.to_string_lossy());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let db_path = crate::mcp::db::get_db_path();
    if let Ok(conn) = Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    ) {
        let _ = conn.busy_timeout(std::time::Duration::from_millis(3000));
        let _ = conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_skill_analytics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                used_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_psa_proj_skill ON project_skill_analytics(project_path, skill_id);"
        );
        let _ = conn.execute(
            "INSERT INTO project_skill_analytics (project_path, skill_id, used_at) VALUES (?1, ?2, ?3)",
            params![norm_path, clean_skill, now],
        );
    }
}

/// Query skill invocation frequencies for the specified project.
pub fn get_project_skill_frequencies(project_path: &Path) -> HashMap<String, usize> {
    let mut freqs = HashMap::new();
    let norm_path = crate::dashboard::projects_path::normalize_project_path(&project_path.to_string_lossy());
    let db_path = crate::mcp::db::get_db_path();

    if let Ok(conn) = Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        let _ = conn.busy_timeout(std::time::Duration::from_millis(3000));
        let mut stmt = match conn.prepare(
            "SELECT skill_id, COUNT(*) FROM project_skill_analytics
             WHERE project_path = ?1 COLLATE NOCASE
             GROUP BY skill_id"
        ) {
            Ok(s) => s,
            Err(_) => return freqs,
        };

        let rows = stmt.query_map(params![norm_path], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        });

        if let Ok(mapped) = rows {
            for item in mapped.flatten() {
                freqs.insert(item.0.to_lowercase(), item.1);
            }
        }
    }
    freqs
}

/// Applies a frequency-based analytical boost to skill candidates based on historical usage in this project.
pub fn apply_analytics_boost(
    mut candidates: Vec<(f32, SkillItem)>,
    project_path: &Path,
) -> Vec<(f32, SkillItem)> {
    let freqs = get_project_skill_frequencies(project_path);
    if freqs.is_empty() {
        return candidates;
    }

    for (score, item) in &mut candidates {
        let key = item.name.to_lowercase();
        let key_hyphen = key.replace('_', "-");
        let key_underscore = key.replace('-', "_");

        let count_opt = freqs
            .get(&key)
            .or_else(|| freqs.get(&key_hyphen))
            .or_else(|| freqs.get(&key_underscore));

        if let Some(&count) = count_opt {
            // Analytical bonus: +0.02 per past invocation, clamped to +0.12
            let bonus = (count as f32 * 0.02).clamp(0.0, 0.12);
            *score += bonus;
        }
    }

    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    candidates
}

/// Formats a markdown report summarizing cross-session skill usage for guidance tool.
pub fn format_skill_analytics_report(project_path: &Path) -> String {
    let freqs = get_project_skill_frequencies(project_path);
    let norm_path = crate::dashboard::projects_path::normalize_project_path(&project_path.to_string_lossy());

    let mut report = format!("# Cross-Session Skill Analytics\n\n- Project: `{}`\n", norm_path);
    if freqs.is_empty() {
        report.push_str("\nNo historical skill invocations recorded for this project yet. Skills will accumulate analytics upon selection.\n");
        return report;
    }

    let mut list: Vec<(String, usize)> = freqs.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1));

    report.push_str("\n| Skill Name | Invocations | Contextual Boost |\n| :--- | :---: | :---: |\n");
    for (skill, count) in list {
        let bonus = (count as f32 * 0.02).clamp(0.0, 0.12);
        report.push_str(&format!("| `{}` | {} | +{:.2} |\n", skill, count, bonus));
    }
    report.push_str("\n> Top skills receive an analytical bonus during candidate ranking to prioritize project-specific domain workflows.\n");
    report
}
