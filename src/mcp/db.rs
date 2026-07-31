use anyhow::Result;
use rusqlite::{params, Connection, OpenFlags};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

static DB_MUTEX: Mutex<()> = Mutex::new(());

pub fn get_db_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("usage.db"))
        .unwrap_or_else(|| PathBuf::from("usage.db"))
}

pub fn init_db() -> Result<()> {
    let _guard = DB_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let db_path = get_db_path();
    init_db_internal(&db_path)
}

fn get_today_string(now_secs: i64) -> String {
    // Format timestamp to YYYY-MM-DD string
    let days = now_secs / 86400;
    // Simple Unix epoch day to ISO date formula
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

fn prune_old_records(conn: &Connection, cutoff_secs: i64) {
    let _ = conn.execute("DELETE FROM tool_calls WHERE started_at < ?", params![cutoff_secs]);
    let _ = conn.execute("DELETE FROM skill_loads WHERE loaded_at < ?", params![cutoff_secs]);
    let _ = conn.execute("DELETE FROM embed_queries WHERE created_at < ?", params![cutoff_secs]);
    let _ = conn.execute("DELETE FROM llm_queries WHERE created_at < ?", params![cutoff_secs]);
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
    orig_tokens: u64,
    opt_tokens: u64,
    duration_ms: u64,
    error_message: Option<&str>,
) {
    let _guard = DB_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let db_path = get_db_path();

    if let Err(e) = init_db_internal(&db_path) {
        warn!("Failed to initialize usage.db: {}", e);
        return;
    }

    let conn = match Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    ) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to open usage.db for logging: {}", e);
            return;
        }
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let cutoff_24h = now - 86400;
    prune_old_records(&conn, cutoff_24h);

    if let Err(e) = conn.execute(
        "INSERT INTO tool_calls (tool_name, operation, started_at, duration_ms, tokens_original, tokens_optimized, error_message)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![
            tool_name,
            operation,
            now,
            duration_ms as i64,
            orig_tokens as i64,
            opt_tokens as i64,
            error_message
        ],
    ) {
        warn!("Failed to insert tool call into usage.db: {}", e);
    }

    let day_str = get_today_string(now);
    update_daily_summary(&conn, &day_str, 1, 0, 0, 0, orig_tokens as i64, opt_tokens as i64);
}

pub fn log_skill_load(skill_id: &str) {
    let _guard = DB_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let db_path = get_db_path();

    if let Err(e) = init_db_internal(&db_path) {
        warn!("Failed to initialize usage.db: {}", e);
        return;
    }

    let conn = match Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    ) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to open usage.db for logging skill load: {}", e);
            return;
        }
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let cutoff_24h = now - 86400;
    prune_old_records(&conn, cutoff_24h);

    let _ = conn.execute(
        "INSERT INTO skill_loads (skill_id, loaded_at) VALUES (?, ?)",
        params![skill_id, now],
    );

    let day_str = get_today_string(now);
    update_daily_summary(&conn, &day_str, 0, 1, 0, 0, 0, 0);
}

pub fn log_embed_query(query: &str) {
    let _guard = DB_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let db_path = get_db_path();

    if let Err(e) = init_db_internal(&db_path) {
        warn!("Failed to initialize usage.db: {}", e);
        return;
    }

    let conn = match Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    ) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to open usage.db for logging embed query: {}", e);
            return;
        }
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let cutoff_24h = now - 86400;
    prune_old_records(&conn, cutoff_24h);

    let _ = conn.execute(
        "INSERT INTO embed_queries (query, created_at) VALUES (?, ?)",
        params![query, now],
    );

    let day_str = get_today_string(now);
    update_daily_summary(&conn, &day_str, 0, 0, 1, 0, 0, 0);
}

fn init_db_internal(db_path: &PathBuf) -> Result<()> {
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tool_calls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_name TEXT NOT NULL,
            operation TEXT,
            started_at INTEGER NOT NULL,
            duration_ms INTEGER,
            tokens_original INTEGER DEFAULT 0,
            tokens_optimized INTEGER DEFAULT 0,
            error_message TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS skill_loads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            skill_id TEXT NOT NULL,
            loaded_at INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS embed_queries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS llm_queries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS daily_summaries (
            day TEXT PRIMARY KEY,
            tool_calls INTEGER DEFAULT 0,
            skills_loaded INTEGER DEFAULT 0,
            embed_queries INTEGER DEFAULT 0,
            llm_queries INTEGER DEFAULT 0,
            tokens_original INTEGER DEFAULT 0,
            tokens_optimized INTEGER DEFAULT 0
        )",
        [],
    )?;

    Ok(())
}
