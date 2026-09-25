use anyhow::Result;
use rusqlite::{params, Connection, OpenFlags};
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::SkillStatsResponse;
use crate::catalog::checksum::compute_sha256;
use crate::mcp::db::get_db_path;

pub fn ensure_cache_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS remote_skill_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            skill_name TEXT NOT NULL,
            task_hash TEXT NOT NULL,
            catalog_hash TEXT NOT NULL,
            content TEXT NOT NULL,
            token_count INTEGER NOT NULL,
            cached_at INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_remote_skill_cache_lookup
            ON remote_skill_cache(skill_name, task_hash, catalog_hash);
        CREATE INDEX IF NOT EXISTS idx_remote_skill_cache_time
            ON remote_skill_cache(cached_at);

        CREATE TABLE IF NOT EXISTS remote_stats_cache (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            catalog_hash TEXT NOT NULL,
            total_skills INTEGER NOT NULL,
            total_sections INTEGER NOT NULL,
            vectors_dim INTEGER NOT NULL,
            last_reindex_at INTEGER NOT NULL,
            tombstones_count INTEGER NOT NULL,
            token_savings_ratio REAL NOT NULL,
            cached_at INTEGER NOT NULL
        );",
    )?;
    Ok(())
}

fn open_conn() -> Result<Connection> {
    let db_path = get_db_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;
    ensure_cache_tables(&conn)?;
    Ok(conn)
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn save_cached_remote_stats(stats: &SkillStatsResponse) -> Result<()> {
    let conn = open_conn()?;
    let now = now_ts() as i64;
    conn.execute(
        "INSERT OR REPLACE INTO remote_stats_cache (
            id, catalog_hash, total_skills, total_sections, vectors_dim,
            last_reindex_at, tombstones_count, token_savings_ratio, cached_at
        ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            stats.catalog_hash,
            stats.total_skills as i64,
            stats.total_sections as i64,
            stats.vectors_dim as i64,
            stats.last_reindex_at as i64,
            stats.tombstones_count as i64,
            stats.token_savings_ratio,
            now,
        ],
    )?;
    Ok(())
}

pub fn get_cached_remote_stats() -> Option<SkillStatsResponse> {
    let conn = open_conn().ok()?;
    conn.query_row(
        "SELECT catalog_hash, total_skills, total_sections, vectors_dim,
                last_reindex_at, tombstones_count, token_savings_ratio
         FROM remote_stats_cache WHERE id = 1",
        [],
        |row| {
            Ok(SkillStatsResponse {
                catalog_hash: row.get(0)?,
                total_skills: row.get::<_, i64>(1)? as usize,
                total_sections: row.get::<_, i64>(2)? as usize,
                vectors_dim: row.get::<_, i64>(3)? as usize,
                last_reindex_at: row.get::<_, i64>(4)? as u64,
                tombstones_count: row.get::<_, i64>(5)? as usize,
                token_savings_ratio: row.get::<_, f64>(6)?,
            })
        },
    )
    .ok()
}

pub fn get_cached_slice(skill_name: &str, task: &str, catalog_hash: &str) -> Option<String> {
    let conn = open_conn().ok()?;
    let task_hash = compute_sha256(task.trim().as_bytes());
    let mut stmt = conn
        .prepare(
            "SELECT content FROM remote_skill_cache
             WHERE skill_name = ?1 AND task_hash = ?2 AND catalog_hash = ?3",
        )
        .ok()?;

    let res = stmt
        .query_row(
            params![skill_name.to_lowercase(), task_hash, catalog_hash],
            |r| r.get(0),
        )
        .ok();
    if res.is_some() {
        let now = now_ts() as i64;
        let _ = conn.execute(
            "UPDATE remote_skill_cache SET cached_at = ?1
             WHERE skill_name = ?2 AND task_hash = ?3 AND catalog_hash = ?4",
            params![now, skill_name.to_lowercase(), task_hash, catalog_hash],
        );
    }
    res
}

pub fn save_cached_slice(
    skill_name: &str,
    task: &str,
    catalog_hash: &str,
    content: &str,
    token_count: usize,
) -> Result<()> {
    let conn = open_conn()?;
    let task_hash = compute_sha256(task.trim().as_bytes());
    let now = now_ts() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO remote_skill_cache (
            skill_name, task_hash, catalog_hash, content, token_count, cached_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            skill_name.to_lowercase(),
            task_hash,
            catalog_hash,
            content,
            token_count as i64,
            now,
        ],
    )?;

    // Prune LRU entries if count > 500
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM remote_skill_cache", [], |r| r.get(0))
        .unwrap_or(0);
    if count > 500 {
        let _ = conn.execute(
            "DELETE FROM remote_skill_cache WHERE id IN (
                SELECT id FROM remote_skill_cache ORDER BY cached_at ASC LIMIT ?1
            )",
            params![count - 500],
        );
    }

    Ok(())
}
