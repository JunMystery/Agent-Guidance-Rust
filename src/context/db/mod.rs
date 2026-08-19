use anyhow::Result;
use rusqlite::Connection;
pub use rusqlite::params;
use std::path::Path;

pub mod schema;
pub mod aliases;
pub mod storage;
pub mod vectors;

pub use aliases::AliasResult;
pub use vectors::{ChunkSearchResult, SymbolSearchResult, bytes_to_f32_vec, cosine_similarity};

pub struct CodeGraphDb {
    pub(crate) conn: Connection,
}

impl CodeGraphDb {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let _ = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get::<_, String>(0));
        let _ = conn.busy_timeout(std::time::Duration::from_millis(2000));
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        let _ = conn.pragma_update(None, "foreign_keys", "ON");

        let mut db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_for_project(project_path: &Path) -> Result<Self> {
        crate::mcp::impact::ensure_agent_context_gitignored(project_path);
        let db_path = project_path.join(".agent-context").join("code_graph.db");
        Self::open(&db_path)
    }

    pub fn open_read_only(db_path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        Ok(Self { conn })
    }

    pub fn file_count(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
        Ok(count)
    }

    fn init_schema(&mut self) -> Result<()> {
        schema::init_schema(&mut self.conn)
    }
}

pub fn sanitize_fts5_query(query: &str) -> String {
    // Strip FTS5 operators and special chars: double quotes, asterisks, AND, OR, NOT, NEAR, etc.
    let cleaned: String = query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect();

    let tokens: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|t| {
            !t.eq_ignore_ascii_case("AND")
                && !t.eq_ignore_ascii_case("OR")
                && !t.eq_ignore_ascii_case("NOT")
                && !t.eq_ignore_ascii_case("NEAR")
        })
        .collect();

    if tokens.is_empty() {
        String::new()
    } else {
        tokens
            .iter()
            .map(|t| format!("\"{}\"", t))
            .collect::<Vec<_>>()
            .join(" ")
    }
}


#[cfg(test)]
#[path = "../db_tests.rs"]
mod tests;

