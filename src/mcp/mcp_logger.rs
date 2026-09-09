// MCP Crash, Error, and Warning Logging Subsystem
use anyhow::Result;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_COUNTER: AtomicUsize = AtomicUsize::new(0);
static LOGGER_INITIALIZED: OnceLock<bool> = OnceLock::new();

pub const MAX_SQLITE_LOGS: usize = 10_000;
pub const MAX_CRASH_FILE_BYTES: u64 = 2 * 1024 * 1024; // 2 MB
pub const MAX_ROTATED_FILES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel { Crash, Error, Warn, Info }

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Crash => "CRASH",
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLogEntry {
    pub id: i64,
    pub timestamp: i64,
    pub time_str: String,
    pub level: String,
    pub source: String,
    pub message: String,
    pub details: Option<String>,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpLogStats {
    pub total: i64,
    pub crashes: i64,
    pub errors: i64,
    pub warnings: i64,
}

pub fn get_crash_log_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("logs"))
        .unwrap_or_else(|| PathBuf::from(".agent-guidance").join("logs"))
}

fn rotate_crash_logs(dir: &PathBuf) {
    let main_log = dir.join("crash.log");
    if let Ok(meta) = fs::metadata(&main_log) {
        if meta.len() >= MAX_CRASH_FILE_BYTES {
            for i in (1..MAX_ROTATED_FILES).rev() {
                let src = dir.join(format!("crash.{}.log", i));
                let dst = dir.join(format!("crash.{}.log", i + 1));
                if src.exists() { let _ = fs::rename(src, dst); }
            }
            let _ = fs::rename(&main_log, dir.join("crash.1.log"));
        }
    }
}

fn write_emergency_crash_file(ts: i64, lvl: &str, src: &str, msg: &str, det: Option<&str>) {
    let dir = get_crash_log_dir();
    let _ = fs::create_dir_all(&dir);
    rotate_crash_logs(&dir);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(dir.join("crash.log")) {
        let _ = writeln!(f, "[{}] [{}] {}: {}\ndetails:\n{}\n---", ts, lvl, src, msg, det.unwrap_or("--"));
        let _ = f.flush();
        let _ = f.sync_all(); // Hard synchronous flush to disk immediately
    }
}

pub fn init_mcp_logger() {
    LOGGER_INITIALIZED.get_or_init(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
            let loc = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
            let payload = info.payload().downcast_ref::<&str>().copied()
                .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
                .unwrap_or("panic occurred");
            let msg = format!("PANIC at [{}]: {}", loc, payload);
            let bt = format!("{:?}", std::backtrace::Backtrace::force_capture());

            write_emergency_crash_file(now, "CRASH", "panic_hook", &msg, Some(&bt));
            log_mcp_direct(LogLevel::Crash, "panic_hook", &msg, Some(&bt), None);
            prev(info);
        }));
        true
    });
}

pub fn log_mcp(level: LogLevel, source: &str, message: &str, details: Option<&str>, project_path: Option<&str>) {
    init_mcp_logger();
    log_mcp_direct(level, source, message, details, project_path);
}

fn log_mcp_direct(level: LogLevel, source: &str, message: &str, details: Option<&str>, project_path: Option<&str>) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let db_path = crate::mcp::db::get_db_path();
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX;

    if let Ok(conn) = Connection::open_with_flags(&db_path, flags) {
        let _ = conn.busy_timeout(std::time::Duration::from_millis(400));
        let res = conn.execute(
            "INSERT INTO mcp_logs (timestamp, level, source, message, details, project_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![now, level.as_str(), source, message, details, project_path],
        );
        if res.is_ok() {
            if LOG_COUNTER.fetch_add(1, Ordering::Relaxed) % 64 == 0 {
                let count: i64 = conn.query_row("SELECT COUNT(*) FROM mcp_logs", [], |r| r.get(0)).unwrap_or(0);
                if count > MAX_SQLITE_LOGS as i64 {
                    let _ = conn.execute(
                        "DELETE FROM mcp_logs WHERE id IN (SELECT id FROM mcp_logs ORDER BY timestamp ASC LIMIT ?1)",
                        params![count - MAX_SQLITE_LOGS as i64 + 200],
                    );
                }
            }
            return;
        }
    }
    write_emergency_crash_file(now, level.as_str(), source, message, details);
}

pub fn query_mcp_logs(
    db_path: &PathBuf,
    level_filter: Option<&str>,
    search: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<(Vec<McpLogEntry>, McpLogStats)> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(db_path, flags)?;
    conn.busy_timeout(std::time::Duration::from_millis(500))?;
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS mcp_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,
            level TEXT NOT NULL,
            source TEXT NOT NULL,
            message TEXT NOT NULL,
            details TEXT,
            project_path TEXT
        );",
        [],
    );

    let mut stats = McpLogStats::default();
    let mut stmt = conn.prepare("SELECT level, COUNT(*) FROM mcp_logs GROUP BY level")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows.flatten() {
        match row.0.as_str() {
            "CRASH" => stats.crashes = row.1,
            "ERROR" => stats.errors = row.1,
            "WARN" => stats.warnings = row.1,
            _ => {}
        }
        stats.total += row.1;
    }

    let mut clauses = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(lvl) = level_filter {
        let upper = lvl.to_uppercase();
        if upper != "ALL" && !upper.is_empty() {
            clauses.push("level = ?");
            params_vec.push(Box::new(upper));
        }
    }

    if let Some(q) = search {
        let trimmed = q.trim();
        if !trimmed.is_empty() {
            clauses.push("(message LIKE ? OR source LIKE ? OR details LIKE ?)");
            let p = format!("%{}%", trimmed);
            params_vec.push(Box::new(p.clone()));
            params_vec.push(Box::new(p.clone()));
            params_vec.push(Box::new(p));
        }
    }

    let where_str = if clauses.is_empty() { String::new() } else { format!("WHERE {}", clauses.join(" AND ")) };
    let sql = format!(
        "SELECT id, timestamp, level, source, message, details, project_path FROM mcp_logs {} ORDER BY timestamp DESC, id DESC LIMIT ? OFFSET ?",
        where_str
    );

    params_vec.push(Box::new(limit as i64));
    params_vec.push(Box::new(offset as i64));

    let mut select_stmt = conn.prepare(&sql)?;
    let p_iter = rusqlite::params_from_iter(params_vec.iter().map(|b| &**b));
    let mut results = Vec::new();
    let log_iter = select_stmt.query_map(p_iter, |r| {
        let ts: i64 = r.get(1)?;
        Ok(McpLogEntry {
            id: r.get(0)?,
            timestamp: ts,
            time_str: format_timestamp(ts),
            level: r.get(2)?,
            source: r.get(3)?,
            message: r.get(4)?,
            details: r.get(5)?,
            project_path: r.get(6)?,
        })
    })?;

    for entry in log_iter.flatten() {
        results.push(entry);
    }
    Ok((results, stats))
}

pub fn clear_mcp_logs(db_path: &PathBuf, level_filter: Option<&str>) -> Result<usize> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    let deleted = match level_filter.map(|s| s.to_uppercase()) {
        Some(lvl) if lvl != "ALL" && !lvl.is_empty() => conn.execute("DELETE FROM mcp_logs WHERE level = ?1", params![lvl])?,
        _ => conn.execute("DELETE FROM mcp_logs", [])?,
    };
    Ok(deleted)
}

pub fn prune_expired_mcp_logs(db_path: &PathBuf, retention_days: u32) -> Result<usize> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let s = 86_400i64;
    let deleted = conn.execute(
        "DELETE FROM mcp_logs WHERE (level = 'CRASH' AND timestamp < ?1) OR (level = 'ERROR' AND timestamp < ?2) OR (level = 'WARN' AND timestamp < ?3) OR (level = 'INFO' AND timestamp < ?4)",
        params![now - (retention_days.max(30) as i64 * s), now - (retention_days.max(14) as i64 * s), now - (retention_days.min(7) as i64 * s), now - (retention_days.min(3) as i64 * s)],
    )?;
    Ok(deleted)
}

fn format_timestamp(ts: i64) -> String {
    let diff = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) - ts;
    if diff < 60 { format!("{}s ago", diff.max(0)) }
    else if diff < 3600 { format!("{}m ago", diff / 60) }
    else if diff < 86400 { format!("{}h ago", diff / 3600) }
    else { format!("{}d ago", diff / 86400) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emergency_crash_file_write() {
        let dir = get_crash_log_dir();
        write_emergency_crash_file(123456789, "CRASH", "test_src", "test crash message", Some("test stacktrace"));
        let crash_file = dir.join("crash.log");
        assert!(crash_file.exists());
        let content = fs::read_to_string(&crash_file).unwrap_or_default();
        assert!(content.contains("test crash message"));
    }
}
