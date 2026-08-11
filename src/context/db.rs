use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;

#[allow(dead_code)]
pub struct CodeGraphDb {
    conn: Connection,
}

#[allow(dead_code)]
impl CodeGraphDb {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        let mut db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_read_only(db_path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;",
        )?;
        Ok(Self { conn })
    }

    fn init_schema(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                content_hash TEXT NOT NULL,
                size INTEGER NOT NULL,
                modified_at INTEGER NOT NULL,
                indexed_at INTEGER NOT NULL
            );",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS symbols (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                file_path TEXT NOT NULL,
                parent TEXT,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                signature TEXT,
                FOREIGN KEY (file_path) REFERENCES files(path) ON DELETE CASCADE
            );",
            [],
        )?;

        tx.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS symbols_fts USING fts5(
                name,
                kind,
                signature,
                content='symbols',
                content_rowid='rowid'
            );",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_symbols_file_path ON symbols(file_path);",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_modified_at ON files(modified_at);",
            [],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn search_symbols(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, String, usize)>> {
        let safe_query = sanitize_fts5_query(query);
        if safe_query.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = self.conn.prepare(
            "SELECT s.file_path, s.name, s.start_line 
             FROM symbols s
             JOIN symbols_fts f ON s.rowid = f.rowid
             WHERE symbols_fts MATCH ?
             LIMIT ?",
        )?;

        let rows = stmt.query_map(params![safe_query, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? as usize,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
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
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_fts5_query() {
        let input = "fn_test OR NOT * NEAR 'quote'";
        let sanitized = sanitize_fts5_query(input);
        assert_eq!(sanitized, "\"fn_test\" \"quote\"");
    }
}
