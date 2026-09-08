use anyhow::Result;
use rusqlite::{Connection, params};
use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_RETENTION_DAYS: i64 = 7;
pub const DAILY_SUMMARY_RETENTION_DAYS: i64 = 90;
pub const MAX_TOOL_CALLS_CEILING: i64 = 50_000;

#[derive(Debug, Clone, Default, Serialize)]
pub struct CleanupSummary {
    pub tool_calls_pruned: usize,
    pub skill_loads_pruned: usize,
    pub embed_queries_pruned: usize,
    pub llm_queries_pruned: usize,
    pub daily_summaries_pruned: usize,
    pub dead_projects_pruned: usize,
    pub lru_tool_calls_pruned: usize,
    pub vacuum_executed: bool,
}

pub fn run_auto_cleanup(conn: &Connection, retention_days: i64) -> Result<CleanupSummary> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let cutoff_raw = now - retention_days * 86400;
    let cutoff_daily_secs = now - DAILY_SUMMARY_RETENTION_DAYS * 86400;
    let cutoff_daily_str = format_day_string(cutoff_daily_secs);
    let cutoff_30d = now - 30 * 86400;

    // 1. Raw detail logs TTL
    let tool_calls_pruned = conn.execute(
        "DELETE FROM tool_calls WHERE started_at < ? OR tool_name = 'mcp_tool'",
        params![cutoff_raw],
    )?;

    let skill_loads_pruned = conn.execute(
        "DELETE FROM skill_loads WHERE loaded_at < ?",
        params![cutoff_raw],
    )?;

    // Fix column names: schema uses 'queried_at'
    let embed_queries_pruned = conn.execute(
        "DELETE FROM embed_queries WHERE queried_at < ?",
        params![cutoff_raw],
    )?;

    let llm_queries_pruned = conn.execute(
        "DELETE FROM llm_queries WHERE queried_at < ?",
        params![cutoff_raw],
    )?;

    // 2. Daily summary retention (90 days)
    let daily_summaries_pruned = conn.execute(
        "DELETE FROM daily_summaries WHERE day < ?",
        params![cutoff_daily_str],
    )?;

    // 3. Auto-prune dead projects missing on disk and inactive for > 30 days
    let mut dead_projects_pruned = 0;
    let mut stmt = conn.prepare(
        "SELECT project_path FROM tracked_projects WHERE last_active < ?",
    )?;
    let missing_paths: Vec<String> = stmt
        .query_map(params![cutoff_30d], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .filter(|p: &String| !Path::new(p).exists())
        .collect();

    for p in &missing_paths {
        dead_projects_pruned += conn.execute(
            "DELETE FROM tracked_projects WHERE project_path = ?",
            params![p],
        )?;
    }

    // 4. Hard-cap LRU ceiling for tool_calls
    let total_calls: i64 = conn
        .query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
        .unwrap_or(0);

    let mut lru_tool_calls_pruned = 0;
    if total_calls > MAX_TOOL_CALLS_CEILING {
        let excess = total_calls - MAX_TOOL_CALLS_CEILING;
        lru_tool_calls_pruned = conn.execute(
            "DELETE FROM tool_calls WHERE id IN (SELECT id FROM tool_calls ORDER BY started_at ASC LIMIT ?)",
            params![excess],
        )?;
    }

    // 5. Disk space reclamation
    let vacuum_executed = conn
        .execute_batch(
            "PRAGMA incremental_vacuum;
             PRAGMA wal_checkpoint(TRUNCATE);",
        )
        .is_ok();

    Ok(CleanupSummary {
        tool_calls_pruned,
        skill_loads_pruned,
        embed_queries_pruned,
        llm_queries_pruned,
        daily_summaries_pruned,
        dead_projects_pruned,
        lru_tool_calls_pruned,
        vacuum_executed,
    })
}

fn format_day_string(now_secs: i64) -> String {
    let days = now_secs / 86400;
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", year, m, d)
}

#[cfg(test)]
#[path = "cleanup_tests.rs"]
mod tests;

