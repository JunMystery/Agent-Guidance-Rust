use anyhow::Result;
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};

use super::CodeGraphDb;
use super::sanitize_fts5_query;

impl CodeGraphDb {
    pub fn get_file_content_hash(&self, path: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT content_hash FROM files WHERE path = ?")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn upsert_file(&self, path: &str, content_hash: &str, size: u64, modified_at: i64) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO files (path, content_hash, size, modified_at, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
                content_hash = ?2,
                size = ?3,
                modified_at = ?4,
                indexed_at = ?5",
            params![path, content_hash, size as i64, modified_at, now],
        )?;
        Ok(())
    }

    pub fn clear_file_data(&self, file_path: &str) -> Result<()> {
        // Cascade will automatically clear symbols, symbol_edges, symbol_vectors, content_chunks, chunk_vectors
        self.conn.execute("DELETE FROM symbols WHERE file_path = ?", params![file_path])?;
        self.conn.execute("DELETE FROM content_chunks WHERE file_path = ?", params![file_path])?;
        Ok(())
    }

    pub fn delete_file(&self, file_path: &str) -> Result<()> {
        self.conn.execute("DELETE FROM files WHERE path = ?", params![file_path])?;
        Ok(())
    }

    pub fn insert_symbol(
        &self,
        id: &str,
        name: &str,
        kind: &str,
        file_path: &str,
        parent: Option<&str>,
        start_line: usize,
        end_line: usize,
        signature: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO symbols (id, name, kind, file_path, parent, start_line, end_line, signature)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                name,
                kind,
                file_path,
                parent,
                start_line as i64,
                end_line as i64,
                signature
            ],
        )?;
        Ok(())
    }

    pub fn insert_edge(&self, source_id: &str, target_id: &str, edge_type: &str, weight: f64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO symbol_edges (source_id, target_id, edge_type, weight)
             VALUES (?1, ?2, ?3, ?4)",
            params![source_id, target_id, edge_type, weight],
        )?;
        Ok(())
    }

    pub fn insert_chunk(
        &self,
        file_path: &str,
        start_line: usize,
        end_line: usize,
        content_hash: &str,
        chunk_text: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO content_chunks (file_path, start_line, end_line, content_hash, chunk_text)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                file_path,
                start_line as i64,
                end_line as i64,
                content_hash,
                chunk_text
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn search_content_fts(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, usize, usize, String)>> {
        let safe_query = sanitize_fts5_query(query);
        if safe_query.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = self.conn.prepare(
            "SELECT c.file_path, c.start_line, c.end_line, snippet(content_fts, 1, '»', '«', '...', 15)
             FROM content_chunks c
             JOIN content_fts f ON c.id = f.rowid
             WHERE content_fts MATCH ?
             LIMIT ?",
        )?;

        let rows = stmt.query_map(params![safe_query, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as usize,
                row.get::<_, i64>(2)? as usize,
                row.get::<_, String>(3)?,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn search_related_symbols(&self, symbol_name: &str) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT s1.name, e.edge_type, s2.name
             FROM symbol_edges e
             JOIN symbols s1 ON e.source_id = s1.id
             JOIN symbols s2 ON e.target_id = s2.id
             WHERE s1.name LIKE ?1 OR s2.name LIKE ?1
             LIMIT 20",
        )?;

        let pattern = format!("%{}%", symbol_name.trim());
        let rows = stmt.query_map(params![pattern], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    /// Count incoming dependencies for a specific file (how many other files import or call symbols in this file)
    pub fn count_incoming_dependencies(&self, file_path: &str) -> Result<usize> {
        let mut stmt = self.conn.prepare(
            "SELECT COUNT(DISTINCT s1.file_path)
             FROM symbol_edges e
             JOIN symbols s1 ON e.source_id = s1.id
             JOIN symbols s2 ON e.target_id = s2.id
             WHERE s2.file_path = ?1 AND s1.file_path != ?1",
        )?;
        let count: i64 = stmt.query_row(params![file_path], |r| r.get(0))?;
        Ok(count as usize)
    }

    /// Get list of other files that depend on symbols defined in this file
    pub fn get_incoming_dependent_files(&self, file_path: &str, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT s1.file_path
             FROM symbol_edges e
             JOIN symbols s1 ON e.source_id = s1.id
             JOIN symbols s2 ON e.target_id = s2.id
             WHERE s2.file_path = ?1 AND s1.file_path != ?1
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![file_path, limit as i64], |r| r.get(0))?;
        let mut files = Vec::new();
        for r in rows {
            files.push(r?);
        }
        Ok(files)
    }
}
