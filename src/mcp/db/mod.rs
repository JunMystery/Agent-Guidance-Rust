pub mod cleanup;
pub mod logger;
pub use cleanup::{CleanupSummary, run_auto_cleanup};
pub use logger::{log_embed_query, log_skill_load, log_tool_call};

use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

static DB_MUTEX: Mutex<()> = Mutex::new(());
static DB_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn get_db_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("usage.db"))
        .unwrap_or_else(|| PathBuf::from("usage.db"))
}

pub fn get_db_size_bytes() -> u64 {
    let path = get_db_path();
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

pub fn init_db() -> Result<()> {
    let _guard = DB_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let db_path = get_db_path();
    ensure_initialized(&db_path)
}

pub(crate) fn ensure_initialized(db_path: &PathBuf) -> Result<()> {
    if !DB_INITIALIZED.load(Ordering::Relaxed) {
        init_db_internal(db_path)?;
        DB_INITIALIZED.store(true, Ordering::Relaxed);
    }
    Ok(())
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

    let _ = conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tool_calls (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_name TEXT NOT NULL,
            operation TEXT,
            started_at INTEGER NOT NULL,
            duration_ms INTEGER,
            tokens_original INTEGER DEFAULT 0,
            tokens_optimized INTEGER DEFAULT 0,
            error_message TEXT
        );
        CREATE TABLE IF NOT EXISTS skill_loads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            skill_id TEXT NOT NULL,
            loaded_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS embed_queries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query_text TEXT NOT NULL,
            queried_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS llm_queries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query_text TEXT NOT NULL,
            queried_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS daily_summaries (
            day TEXT PRIMARY KEY,
            tool_calls INTEGER DEFAULT 0,
            skills_loaded INTEGER DEFAULT 0,
            embed_queries INTEGER DEFAULT 0,
            llm_queries INTEGER DEFAULT 0,
            tokens_original INTEGER DEFAULT 0,
            tokens_optimized INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS tracked_projects (
            project_path TEXT PRIMARY KEY,
            project_name TEXT NOT NULL,
            first_seen INTEGER NOT NULL,
            last_active INTEGER NOT NULL,
            total_calls INTEGER DEFAULT 0,
            total_tokens_saved INTEGER DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_tool_calls_started ON tool_calls(started_at);
        CREATE INDEX IF NOT EXISTS idx_skill_loads_loaded ON skill_loads(loaded_at);
        CREATE INDEX IF NOT EXISTS idx_embed_queries_created ON embed_queries(queried_at);
        CREATE INDEX IF NOT EXISTS idx_tracked_projects_active ON tracked_projects(last_active);
        DELETE FROM tool_calls WHERE tool_name = 'mcp_tool';",
    )?;

    let _ = conn.execute("ALTER TABLE tool_calls ADD COLUMN project_path TEXT", []);

    Ok(())
}
