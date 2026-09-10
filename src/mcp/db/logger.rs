use anyhow::Result;
use rusqlite::{Connection, OpenFlags, params};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

static DB_CONN: Mutex<Option<Connection>> = Mutex::new(None);
static LAST_PRUNE_SECS: AtomicI64 = AtomicI64::new(0);

fn with_db<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Connection) -> Result<R>,
{
    let mut guard = DB_CONN.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let db_path = super::get_db_path();
        if let Err(e) = super::ensure_initialized(&db_path) {
            warn!("Failed to initialize usage.db: {}", e);
            return None;
        }
        match Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        ) {
            Ok(conn) => {
                let _ = conn.busy_timeout(std::time::Duration::from_millis(2000));
                *guard = Some(conn);
            }
            Err(e) => {
                warn!("Failed to open usage.db: {}", e);
                return None;
            }
        }
    }

    if let Some(ref conn) = *guard {
        match f(conn) {
            Ok(res) => Some(res),
            Err(e) => {
                warn!("usage.db operation failed: {}", e);
                None
            }
        }
    } else {
        None
    }
}

fn maybe_prune(conn: &Connection, now: i64) {
    let last = LAST_PRUNE_SECS.load(Ordering::Relaxed);
    if now - last > 3600 {
        LAST_PRUNE_SECS.store(now, Ordering::Relaxed);
        let _ = super::cleanup::run_auto_cleanup(conn, super::cleanup::DEFAULT_RETENTION_DAYS);
    }
}

fn get_today_string(now_secs: i64) -> String {
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

fn update_daily_summary(
    conn: &Connection,
    day_str: &str,
    tool_calls_delta: i64,
    skills_loaded_delta: i64,
    embed_queries_delta: i64,
    llm_queries_delta: i64,
    tokens_orig_delta: i64,
    tokens_opt_delta: i64,
) {
    let _ = conn.execute(
        "INSERT INTO daily_summaries (day, tool_calls, skills_loaded, embed_queries, llm_queries, tokens_original, tokens_optimized)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(day) DO UPDATE SET
             tool_calls = tool_calls + excluded.tool_calls,
             skills_loaded = skills_loaded + excluded.skills_loaded,
             embed_queries = embed_queries + excluded.embed_queries,
             llm_queries = llm_queries + excluded.llm_queries,
             tokens_original = tokens_original + excluded.tokens_original,
             tokens_optimized = tokens_optimized + excluded.tokens_optimized",
        params![
            day_str,
            tool_calls_delta,
            skills_loaded_delta,
            embed_queries_delta,
            llm_queries_delta,
            tokens_orig_delta,
            tokens_opt_delta
        ],
    );
}

pub fn log_tool_call(
    tool_name: &str,
    operation: Option<&str>,
    target: Option<&str>,
    orig_tokens: u64,
    opt_tokens: u64,
    duration_ms: u64,
    error_message: Option<&str>,
    project_path: Option<&str>,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let day_str = get_today_string(now);
    let tokens_saved = orig_tokens.saturating_sub(opt_tokens);

    let normalized_proj = project_path.map(crate::dashboard::projects::normalize_project_path);
    let effective_proj = normalized_proj
        .as_deref()
        .filter(|s| !s.is_empty() && !crate::dashboard::projects::is_temp_project_path(s));

    with_db(|conn| {
        maybe_prune(conn, now);

        conn.execute(
            "INSERT INTO tool_calls (tool_name, operation, started_at, duration_ms, tokens_original, tokens_optimized, error_message, project_path, target)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                tool_name,
                operation,
                now,
                duration_ms as i64,
                orig_tokens as i64,
                opt_tokens as i64,
                error_message,
                effective_proj,
                target
            ],
        )?;

        if let Some(normalized) = effective_proj {
            let p = std::path::Path::new(normalized);
            let proj_name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("project");

            let _ = conn.execute(
                "INSERT INTO tracked_projects (project_path, project_name, first_seen, last_active, total_calls, total_tokens_saved)
                 VALUES (?1, ?2, ?3, ?3, 1, ?4)
                 ON CONFLICT(project_path) DO UPDATE SET
                     last_active = ?3,
                     total_calls = total_calls + 1,
                     total_tokens_saved = total_tokens_saved + ?4",
                params![normalized, proj_name, now, tokens_saved as i64],
            );
        }

        update_daily_summary(
            conn,
            &day_str,
            1,
            0,
            0,
            0,
            orig_tokens as i64,
            opt_tokens as i64,
        );
        Ok(())
    });
}

pub fn log_skill_load(skill_id: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let day_str = get_today_string(now);

    with_db(|conn| {
        maybe_prune(conn, now);
        conn.execute(
            "INSERT INTO skill_loads (skill_id, loaded_at) VALUES (?, ?)",
            params![skill_id, now],
        )?;
        update_daily_summary(conn, &day_str, 0, 1, 0, 0, 0, 0);
        Ok(())
    });
}

pub fn log_embed_query(query: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let day_str = get_today_string(now);

    with_db(|conn| {
        maybe_prune(conn, now);
        conn.execute(
            "INSERT INTO embed_queries (query_text, queried_at) VALUES (?, ?)",
            params![query, now],
        )?;
        update_daily_summary(conn, &day_str, 0, 0, 1, 0, 0, 0);
        Ok(())
    });
}
