use anyhow::Result;
use rusqlite::Connection;

pub(crate) fn init_schema(conn: &mut Connection) -> Result<()> {
        let tx = conn.transaction()?;
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

