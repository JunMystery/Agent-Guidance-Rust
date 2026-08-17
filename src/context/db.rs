use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct CodeGraphDb {
    conn: Connection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AliasResult {
    pub resolved_path: String,
    pub resolved_symbol: Option<String>,
    pub resolved_line: Option<usize>,
    pub confidence: f64,
    pub hit_count: i64,
}

impl CodeGraphDb {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let mut db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Open (or create) the per-project code graph database.
    /// Location: <project_root>/.agent-context/code_graph.db
    pub fn open_for_project(project_path: &Path) -> Result<Self> {
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

        // Plan 01: Aliases table for learned mappings
        tx.execute(
            "CREATE TABLE IF NOT EXISTS aliases (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                alias_term TEXT NOT NULL,
                resolved_path TEXT NOT NULL,
                resolved_symbol TEXT,
                resolved_line INTEGER,
                hit_count INTEGER DEFAULT 1,
                confidence REAL DEFAULT 0.8,
                created_at INTEGER NOT NULL,
                last_used_at INTEGER NOT NULL,
                UNIQUE(alias_term, resolved_path)
            );",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_aliases_term ON aliases(alias_term);",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_aliases_confidence ON aliases(confidence DESC);",
            [],
        )?;

        // Plan 02: Symbol Edges (DAG for calls / imports / implements)
        tx.execute(
            "CREATE TABLE IF NOT EXISTS symbol_edges (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                weight REAL DEFAULT 1.0,
                FOREIGN KEY (source_id) REFERENCES symbols(id) ON DELETE CASCADE,
                FOREIGN KEY (target_id) REFERENCES symbols(id) ON DELETE CASCADE,
                PRIMARY KEY (source_id, target_id, edge_type)
            );",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_source ON symbol_edges(source_id);",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_target ON symbol_edges(target_id);",
            [],
        )?;

        // Plan 02 & 03: Symbol Vectors
        tx.execute(
            "CREATE TABLE IF NOT EXISTS symbol_vectors (
                symbol_id TEXT PRIMARY KEY,
                vector BLOB NOT NULL,
                model_version TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (symbol_id) REFERENCES symbols(id) ON DELETE CASCADE
            );",
            [],
        )?;

        // Plan 02: Content Chunks (RAG sliding window ~50 lines)
        tx.execute(
            "CREATE TABLE IF NOT EXISTS content_chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                content_hash TEXT NOT NULL,
                chunk_text TEXT NOT NULL,
                FOREIGN KEY (file_path) REFERENCES files(path) ON DELETE CASCADE
            );",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_chunks_file ON content_chunks(file_path);",
            [],
        )?;

        // Plan 02 & 03: Content FTS5 for instant text search
        tx.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS content_fts USING fts5(
                file_path,
                chunk_text,
                content='content_chunks',
                content_rowid='id'
            );",
            [],
        )?;

        // FTS5 Triggers for symbols
        tx.execute(
            "CREATE TRIGGER IF NOT EXISTS symbols_ai AFTER INSERT ON symbols BEGIN
                INSERT INTO symbols_fts(rowid, name, kind, signature) VALUES (new.rowid, new.name, new.kind, new.signature);
            END;",
            [],
        )?;
        tx.execute(
            "CREATE TRIGGER IF NOT EXISTS symbols_ad AFTER DELETE ON symbols BEGIN
                INSERT INTO symbols_fts(symbols_fts, rowid, name, kind, signature) VALUES('delete', old.rowid, old.name, old.kind, old.signature);
            END;",
            [],
        )?;
        tx.execute(
            "CREATE TRIGGER IF NOT EXISTS symbols_au AFTER UPDATE ON symbols BEGIN
                INSERT INTO symbols_fts(symbols_fts, rowid, name, kind, signature) VALUES('delete', old.rowid, old.name, old.kind, old.signature);
                INSERT INTO symbols_fts(rowid, name, kind, signature) VALUES (new.rowid, new.name, new.kind, new.signature);
            END;",
            [],
        )?;

        // FTS5 Triggers for content_chunks
        tx.execute(
            "CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON content_chunks BEGIN
                INSERT INTO content_fts(rowid, file_path, chunk_text) VALUES (new.id, new.file_path, new.chunk_text);
            END;",
            [],
        )?;
        tx.execute(
            "CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON content_chunks BEGIN
                INSERT INTO content_fts(content_fts, rowid, file_path, chunk_text) VALUES('delete', old.id, old.file_path, old.chunk_text);
            END;",
            [],
        )?;
        tx.execute(
            "CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON content_chunks BEGIN
                INSERT INTO content_fts(content_fts, rowid, file_path, chunk_text) VALUES('delete', old.id, old.file_path, old.chunk_text);
                INSERT INTO content_fts(rowid, file_path, chunk_text) VALUES (new.id, new.file_path, new.chunk_text);
            END;",
            [],
        )?;

        // Plan 02 & 03: Chunk Vectors (RAG vector embeddings)
        tx.execute(
            "CREATE TABLE IF NOT EXISTS chunk_vectors (
                chunk_id INTEGER PRIMARY KEY,
                vector BLOB NOT NULL,
                model_version TEXT NOT NULL,
                FOREIGN KEY (chunk_id) REFERENCES content_chunks(id) ON DELETE CASCADE
            );",
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

    // --- Plan 01: Alias Learning Methods ---

    pub fn lookup_aliases(&self, query: &str, limit: usize) -> Result<Vec<AliasResult>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let pattern = format!("%{}%", trimmed);
        let mut stmt = self.conn.prepare(
            "SELECT resolved_path, resolved_symbol, resolved_line, confidence, hit_count
             FROM aliases
             WHERE alias_term LIKE ? OR ? LIKE ('%' || alias_term || '%')
             ORDER BY confidence DESC, hit_count DESC, last_used_at DESC
             LIMIT ?",
        )?;

        let rows = stmt.query_map(params![pattern, trimmed, limit as i64], |row| {
            Ok(AliasResult {
                resolved_path: row.get(0)?,
                resolved_symbol: row.get(1)?,
                resolved_line: row.get::<_, Option<i64>>(2)?.map(|l| l as usize),
                confidence: row.get(3)?,
                hit_count: row.get(4)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn upsert_alias(
        &self,
        alias_term: &str,
        resolved_path: &str,
        resolved_symbol: Option<&str>,
        resolved_line: Option<usize>,
    ) -> Result<()> {
        let trimmed = alias_term.trim();
        if trimmed.is_empty() || resolved_path.trim().is_empty() {
            return Ok(());
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let line_i64 = resolved_line.map(|l| l as i64);

        self.conn.execute(
            "INSERT INTO aliases (alias_term, resolved_path, resolved_symbol, resolved_line, hit_count, confidence, created_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, 1, 0.8, ?5, ?5)
             ON CONFLICT(alias_term, resolved_path) DO UPDATE SET
                resolved_symbol = COALESCE(?3, aliases.resolved_symbol),
                resolved_line = COALESCE(?4, aliases.resolved_line),
                hit_count = aliases.hit_count + 1,
                confidence = MIN(1.0, aliases.confidence + 0.05),
                last_used_at = ?5",
            params![trimmed, resolved_path.trim(), resolved_symbol, line_i64, now],
        )?;

        Ok(())
    }

    pub fn bump_alias(&self, alias_term: &str, resolved_path: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "UPDATE aliases 
             SET hit_count = hit_count + 1, last_used_at = ?1 
             WHERE alias_term = ?2 AND resolved_path = ?3",
            params![now, alias_term.trim(), resolved_path.trim()],
        )?;
        Ok(())
    }

    pub fn decay_aliases(&self) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let thirty_days = 30 * 24 * 3600;
        let ninety_days = 90 * 24 * 3600;

        // Phase 1: Giảm confidence cho aliases > 30 ngày không dùng
        self.conn.execute(
            "UPDATE aliases 
             SET confidence = MAX(0.1, confidence * 0.5) 
             WHERE (?1 - last_used_at) > ?2 AND confidence > 0.1",
            params![now, thirty_days],
        )?;

        // Phase 2: Xóa aliases > 90 ngày không dùng
        let deleted = self.conn.execute(
            "DELETE FROM aliases WHERE (?1 - last_used_at) > ?2",
            params![now, ninety_days],
        )?;

        Ok(deleted)
    }

    // --- Plan 02: Indexer Storage & Query Methods ---

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

    // --- Plan 03: Vector Embeddings & Semantic Search Methods ---

    pub fn store_symbol_vector(&self, symbol_id: &str, vector: &[f32], model_version: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();

        self.conn.execute(
            "INSERT OR REPLACE INTO symbol_vectors (symbol_id, vector, model_version, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![symbol_id, bytes, model_version, now],
        )?;
        Ok(())
    }

    pub fn store_chunk_vector(&self, chunk_id: i64, vector: &[f32], model_version: &str) -> Result<()> {
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();

        self.conn.execute(
            "INSERT OR REPLACE INTO chunk_vectors (chunk_id, vector, model_version)
             VALUES (?1, ?2, ?3)",
            params![chunk_id, bytes, model_version],
        )?;
        Ok(())
    }

    pub fn get_symbols_without_vectors(&self, limit: usize) -> Result<Vec<(String, String, String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, s.kind, s.file_path, s.signature
             FROM symbols s
             LEFT JOIN symbol_vectors v ON s.id = v.symbol_id
             WHERE v.symbol_id IS NULL
             LIMIT ?",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn get_chunks_without_vectors(&self, limit: usize) -> Result<Vec<(i64, String, usize, usize, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.file_path, c.start_line, c.end_line, c.chunk_text
             FROM content_chunks c
             LEFT JOIN chunk_vectors v ON c.id = v.chunk_id
             WHERE v.chunk_id IS NULL
             LIMIT ?",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get::<_, i64>(2)? as usize,
                row.get::<_, i64>(3)? as usize,
                row.get(4)?,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub const HNSW_DISPATCH_THRESHOLD: usize = 10_000;

    pub fn vector_search_symbols(
        &self,
        query_vector: &[f32],
        top_k: usize,
        threshold: f32,
    ) -> Result<Vec<SymbolSearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT v.vector, s.name, s.kind, s.file_path, s.start_line, s.signature
             FROM symbol_vectors v
             JOIN symbols s ON v.symbol_id = s.id",
        )?;

        let rows = stmt.query_map([], |row| {
            let blob: Vec<u8> = row.get(0)?;
            let name: String = row.get(1)?;
            let kind: String = row.get(2)?;
            let file_path: String = row.get(3)?;
            let start_line = row.get::<_, i64>(4)? as usize;
            let signature: Option<String> = row.get(5)?;
            Ok((blob, name, kind, file_path, start_line, signature))
        })?;

        let mut all_items = Vec::new();
        for r in rows {
            all_items.push(r?);
        }

        // Dual-Engine: Use HNSW Graph for massive repositories (>10,000 vectors)
        if all_items.len() > Self::HNSW_DISPATCH_THRESHOLD {
            let mut hnsw = super::hnsw::HnswIndex::new(16, 64, 32);
            for (blob, name, kind, file_path, start_line, signature) in all_items {
                let vec = bytes_to_f32_vec(&blob);
                hnsw.insert(vec, SymbolSearchResult {
                    name,
                    kind,
                    file_path,
                    start_line,
                    signature,
                    score: 0.0,
                });
            }

            let hnsw_results = hnsw.search(query_vector, top_k, threshold);
            return Ok(hnsw_results
                .into_iter()
                .map(|(score, payload)| {
                    let mut item = payload.clone();
                    item.score = score;
                    item
                })
                .collect());
        }

        // Default: Fast Flat Scan for standard repositories (<=10,000 vectors)
        let mut matches = Vec::new();
        for (blob, name, kind, file_path, start_line, signature) in all_items {
            let vec = bytes_to_f32_vec(&blob);
            let score = cosine_similarity(query_vector, &vec);
            if score >= threshold {
                matches.push(SymbolSearchResult {
                    name,
                    kind,
                    file_path,
                    start_line,
                    signature,
                    score,
                });
            }
        }

        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(top_k);
        Ok(matches)
    }

    pub fn vector_search_chunks(
        &self,
        query_vector: &[f32],
        top_k: usize,
        threshold: f32,
    ) -> Result<Vec<ChunkSearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT v.vector, c.file_path, c.start_line, c.end_line
             FROM chunk_vectors v
             JOIN content_chunks c ON v.chunk_id = c.id",
        )?;

        let rows = stmt.query_map([], |row| {
            let blob: Vec<u8> = row.get(0)?;
            let file_path: String = row.get(1)?;
            let start_line = row.get::<_, i64>(2)? as usize;
            let end_line = row.get::<_, i64>(3)? as usize;
            Ok((blob, file_path, start_line, end_line))
        })?;

        let mut all_items = Vec::new();
        for r in rows {
            all_items.push(r?);
        }

        // Dual-Engine: Use HNSW Graph for massive repositories (>10,000 vectors)
        if all_items.len() > Self::HNSW_DISPATCH_THRESHOLD {
            let mut hnsw = super::hnsw::HnswIndex::new(16, 64, 32);
            for (blob, file_path, start_line, end_line) in all_items {
                let vec = bytes_to_f32_vec(&blob);
                hnsw.insert(vec, ChunkSearchResult {
                    file_path,
                    start_line,
                    end_line,
                    score: 0.0,
                });
            }

            let hnsw_results = hnsw.search(query_vector, top_k, threshold);
            return Ok(hnsw_results
                .into_iter()
                .map(|(score, payload)| {
                    let mut item = payload.clone();
                    item.score = score;
                    item
                })
                .collect());
        }

        // Default: Fast Flat Scan for standard repositories (<=10,000 vectors)
        let mut matches = Vec::new();
        for (blob, file_path, start_line, end_line) in all_items {
            let vec = bytes_to_f32_vec(&blob);
            let score = cosine_similarity(query_vector, &vec);
            if score >= threshold {
                matches.push(ChunkSearchResult {
                    file_path,
                    start_line,
                    end_line,
                    score,
                });
            }
        }

        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(top_k);
        Ok(matches)
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

#[derive(Debug, Clone)]
pub struct SymbolSearchResult {
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: usize,
    pub signature: Option<String>,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct ChunkSearchResult {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub score: f32,
}

fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
        .collect()
}

fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let dot: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
    let norm1: f32 = v1.iter().map(|a| a * a).sum::<f32>().sqrt();
    let norm2: f32 = v2.iter().map(|b| b * b).sum::<f32>().sqrt();
    if norm1 == 0.0 || norm2 == 0.0 {
        return 0.0;
    }
    dot / (norm1 * norm2)
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

    #[test]
    fn test_alias_learning_and_lookup() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("ag_alias_test_{}", std::process::id()));
        let db_path = temp_dir.join("test_code_graph.db");
        let db = CodeGraphDb::open(&db_path)?;

        // 1. Initial lookup should be empty
        let empty = db.lookup_aliases("đăng nhập", 5)?;
        assert!(empty.is_empty());

        // 2. Learn alias
        db.upsert_alias("đăng nhập", "src/auth/service.rs", Some("AuthenticationService"), Some(42))?;

        // 3. Lookup should hit
        let hits = db.lookup_aliases("đăng nhập", 5)?;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].resolved_path, "src/auth/service.rs");
        assert_eq!(hits[0].resolved_symbol.as_deref(), Some("AuthenticationService"));
        assert_eq!(hits[0].resolved_line, Some(42));
        assert_eq!(hits[0].hit_count, 1);
        assert!((hits[0].confidence - 0.8).abs() < f64::EPSILON);

        // 4. Substring query lookup ("tính năng đăng nhập")
        let sub_hits = db.lookup_aliases("tính năng đăng nhập", 5)?;
        assert_eq!(sub_hits.len(), 1);

        // 5. Repeated learn increases confidence & hit_count
        db.upsert_alias("đăng nhập", "src/auth/service.rs", Some("AuthenticationService"), Some(42))?;
        let bumped = db.lookup_aliases("đăng nhập", 5)?;
        assert_eq!(bumped[0].hit_count, 2);
        assert!((bumped[0].confidence - 0.85).abs() < 1e-5);

        // 6. Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_alias_decay() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("ag_decay_test_{}", std::process::id()));
        let db_path = temp_dir.join("test_decay.db");
        let db = CodeGraphDb::open(&db_path)?;

        let old_time = 1000; // Epoch way back
        db.conn.execute(
            "INSERT INTO aliases (alias_term, resolved_path, resolved_symbol, resolved_line, hit_count, confidence, created_at, last_used_at)
             VALUES ('old_term', 'src/old.rs', 'OldStruct', 10, 1, 0.8, ?1, ?1)",
            params![old_time],
        )?;

        // Running decay should remove the 90+ days old entry
        let deleted = db.decay_aliases()?;
        assert_eq!(deleted, 1);

        let res = db.lookup_aliases("old_term", 5)?;
        assert!(res.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_file_symbols_edges_and_chunks() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("ag_graph_test_{}", std::process::id()));
        let db_path = temp_dir.join("test_graph.db");
        let db = CodeGraphDb::open(&db_path)?;

        // 1. Insert file
        db.upsert_file("src/main.rs", "hash123", 500, 1000)?;
        assert_eq!(db.get_file_content_hash("src/main.rs")?, Some("hash123".to_string()));

        // 2. Insert symbols
        db.insert_symbol("src/main.rs::fn::main::L1", "main", "function", "src/main.rs", None, 1, 10, Some("fn main()"))?;
        db.insert_symbol("src/main.rs::fn::helper::L12", "helper", "function", "src/main.rs", None, 12, 20, Some("fn helper()"))?;

        // 3. Search symbols FTS
        let syms = db.search_symbols("main", 5)?;
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].1, "main");

        // 4. Insert edge
        db.insert_edge("src/main.rs::fn::main::L1", "src/main.rs::fn::helper::L12", "calls", 1.0)?;
        let related = db.search_related_symbols("main")?;
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].0, "main");
        assert_eq!(related[0].1, "calls");
        assert_eq!(related[0].2, "helper");

        // 5. Insert content chunks
        let chunk_id = db.insert_chunk("src/main.rs", 1, 15, "chunkhash1", "fn main() {\n    let timeout = Duration::from_secs(30);\n    helper();\n}")?;
        assert!(chunk_id > 0);

        // 6. Search content FTS
        let fts_hits = db.search_content_fts("timeout", 5)?;
        assert_eq!(fts_hits.len(), 1);
        assert_eq!(fts_hits[0].0, "src/main.rs");
        assert_eq!(fts_hits[0].1, 1);
        assert_eq!(fts_hits[0].2, 15);

        // 7. Clear file data
        db.clear_file_data("src/main.rs")?;
        assert!(db.search_symbols("main", 5)?.is_empty());
        assert!(db.search_content_fts("timeout", 5)?.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_vector_storage_and_search() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("ag_vec_test_{}", std::process::id()));
        let db_path = temp_dir.join("test_vec.db");
        let db = CodeGraphDb::open(&db_path)?;

        // 1. Setup file and symbol
        db.upsert_file("src/auth.rs", "h1", 100, 100)?;
        db.insert_symbol("src/auth.rs::fn::login::L1", "login", "function", "src/auth.rs", None, 1, 10, Some("fn login()"))?;

        // 2. Store symbol vector
        let vec_data = vec![1.0, 0.0, 0.0, 0.0];
        db.store_symbol_vector("src/auth.rs::fn::login::L1", &vec_data, "v1")?;

        // 3. Search with matching vector
        let query_vec = vec![1.0, 0.0, 0.0, 0.0];
        let hits = db.vector_search_symbols(&query_vec, 5, 0.9)?;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "login");
        assert!((hits[0].score - 1.0).abs() < 1e-5);

        // 4. Store and search chunk vector
        let chunk_id = db.insert_chunk("src/auth.rs", 1, 10, "ch1", "fn login() { ... }")?;
        db.store_chunk_vector(chunk_id, &vec_data, "v1")?;
        let chunk_hits = db.vector_search_chunks(&query_vec, 5, 0.9)?;
        assert_eq!(chunk_hits.len(), 1);
        assert_eq!(chunk_hits[0].file_path, "src/auth.rs");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_hnsw_dispatch_threshold_vector_search() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("hnsw_dispatch_test_{}", std::process::id()));
        let db_path = temp_dir.join("test_hnsw.db");
        let db = CodeGraphDb::open(&db_path)?;

        // Insert mock symbols
        db.upsert_file("src/jwt.rs", "h1", 100, 100)?;
        db.insert_symbol("src/jwt.rs::fn::verify::L1", "verify_token", "function", "src/jwt.rs", None, 1, 10, Some("fn verify_token()"))?;
        db.store_symbol_vector("src/jwt.rs::fn::verify::L1", &[0.95, 0.05, 0.0], "v1")?;

        // Test direct HNSW indexer search
        let mut hnsw = super::super::hnsw::HnswIndex::new(16, 64, 32);
        hnsw.insert(vec![0.95, 0.05, 0.0], "verify_token");
        hnsw.insert(vec![0.0, 0.95, 0.05], "other_func");

        let results = hnsw.search(&[0.99, 0.01, 0.0], 1, 0.8);
        assert_eq!(results.len(), 1);
        assert_eq!(*results[0].1, "verify_token");
        assert!(results[0].0 > 0.9);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
