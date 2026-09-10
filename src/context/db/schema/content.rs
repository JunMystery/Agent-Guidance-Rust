use anyhow::Result;
use rusqlite::Transaction;

pub fn create_content_tables(tx: &Transaction) -> Result<()> {
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

    // Proposal 03: Content-addressable embedding cache
    tx.execute(
        "CREATE TABLE IF NOT EXISTS embedding_cache (
            passage_hash TEXT PRIMARY KEY,
            vector BLOB NOT NULL,
            model_version TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );",
        [],
    )?;

    // Domain summaries and architectural notes added by AI Agents
    tx.execute(
        "CREATE TABLE IF NOT EXISTS domain_summaries (
            module_path TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            tags TEXT,
            updated_at INTEGER NOT NULL,
            updated_by TEXT NOT NULL
        );",
        [],
    )?;
    tx.execute(
        "CREATE VIRTUAL TABLE IF NOT EXISTS domain_summaries_fts USING fts5(
            module_path UNINDEXED,
            title,
            summary,
            tags,
            content='domain_summaries',
            content_rowid='rowid'
        );",
        [],
    )?;
    tx.execute(
        "CREATE TRIGGER IF NOT EXISTS domain_summaries_ai AFTER INSERT ON domain_summaries BEGIN
            INSERT INTO domain_summaries_fts(rowid, module_path, title, summary, tags) VALUES (new.rowid, new.module_path, new.title, new.summary, new.tags);
        END;",
        [],
    )?;
    tx.execute(
        "CREATE TRIGGER IF NOT EXISTS domain_summaries_ad AFTER DELETE ON domain_summaries BEGIN
            INSERT INTO domain_summaries_fts(domain_summaries_fts, rowid, module_path, title, summary, tags) VALUES('delete', old.rowid, old.module_path, old.title, old.summary, old.tags);
        END;",
        [],
    )?;
    tx.execute(
        "CREATE TRIGGER IF NOT EXISTS domain_summaries_au AFTER UPDATE ON domain_summaries BEGIN
            INSERT INTO domain_summaries_fts(domain_summaries_fts, rowid, module_path, title, summary, tags) VALUES('delete', old.rowid, old.module_path, old.title, old.summary, old.tags);
            INSERT INTO domain_summaries_fts(rowid, module_path, title, summary, tags) VALUES (new.rowid, new.module_path, new.title, new.summary, new.tags);
        END;",
        [],
    )?;

    Ok(())
}
