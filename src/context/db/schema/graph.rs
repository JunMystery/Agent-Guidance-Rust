use anyhow::Result;
use rusqlite::Transaction;

pub fn create_graph_tables(tx: &Transaction) -> Result<()> {
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

    // Semantic edges added by AI Agents / Subagents
    tx.execute(
        "CREATE TABLE IF NOT EXISTS semantic_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_symbol TEXT NOT NULL,
            target_symbol TEXT NOT NULL,
            relation_type TEXT NOT NULL,
            description TEXT,
            confidence REAL DEFAULT 0.9,
            created_by TEXT DEFAULT 'agent',
            created_at INTEGER NOT NULL,
            UNIQUE(source_symbol, target_symbol, relation_type)
        );",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_semantic_edges_src ON semantic_edges(source_symbol);",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_semantic_edges_tgt ON semantic_edges(target_symbol);",
        [],
    )?;

    Ok(())
}
