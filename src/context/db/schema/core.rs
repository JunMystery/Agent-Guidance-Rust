use anyhow::Result;
use rusqlite::Transaction;

pub fn create_core_tables(tx: &Transaction) -> Result<()> {
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
            language TEXT DEFAULT 'unknown',
            namespace TEXT,
            receiver TEXT,
            FOREIGN KEY (file_path) REFERENCES files(path) ON DELETE CASCADE
        );",
        [],
    )?;

    // Safe backward compatibility migrations
    let _ = tx.execute("ALTER TABLE symbols ADD COLUMN language TEXT DEFAULT 'unknown';", []);
    let _ = tx.execute("ALTER TABLE symbols ADD COLUMN namespace TEXT;", []);
    let _ = tx.execute("ALTER TABLE symbols ADD COLUMN receiver TEXT;", []);

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

    Ok(())
}
