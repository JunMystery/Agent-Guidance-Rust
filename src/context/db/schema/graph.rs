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
            confidence REAL DEFAULT 1.0,
            category TEXT DEFAULT 'symbol',
            call_line INTEGER,
            FOREIGN KEY (source_id) REFERENCES symbols(id) ON DELETE CASCADE,
            FOREIGN KEY (target_id) REFERENCES symbols(id) ON DELETE CASCADE,
            PRIMARY KEY (source_id, target_id, edge_type)
        );",
        [],
    )?;

    // Safe backward compatibility migrations
    let _ = tx.execute("ALTER TABLE symbol_edges ADD COLUMN confidence REAL DEFAULT 1.0;", []);
    let _ = tx.execute("ALTER TABLE symbol_edges ADD COLUMN category TEXT DEFAULT 'symbol';", []);
    let _ = tx.execute("ALTER TABLE symbol_edges ADD COLUMN call_line INTEGER;", []);

    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_edges_source ON symbol_edges(source_id);",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_edges_target ON symbol_edges(target_id);",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_edges_src_tgt ON symbol_edges(source_id, target_id);",
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

    // Plan 04: Intra-procedural Data Flow Edges (Milestone v1.8.0)
    tx.execute(
        "CREATE TABLE IF NOT EXISTS data_flow_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            caller_symbol_id TEXT NOT NULL,
            callee_symbol_id TEXT NOT NULL,
            source_variable TEXT NOT NULL,
            target_parameter TEXT NOT NULL,
            arg_index INTEGER NOT NULL,
            call_line INTEGER NOT NULL,
            flow_type TEXT DEFAULT 'direct_param',
            confidence REAL DEFAULT 0.85,
            FOREIGN KEY (caller_symbol_id) REFERENCES symbols(id) ON DELETE CASCADE,
            FOREIGN KEY (callee_symbol_id) REFERENCES symbols(id) ON DELETE CASCADE,
            UNIQUE(caller_symbol_id, callee_symbol_id, source_variable, target_parameter, call_line)
        );",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_df_caller ON data_flow_edges(caller_symbol_id);",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_df_callee ON data_flow_edges(callee_symbol_id);",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_df_src_var ON data_flow_edges(source_variable);",
        [],
    )?;

    // Evolutionary Coupling & Continuous Learning Graph (Milestone v1.9.0)
    tx.execute(
        "CREATE TABLE IF NOT EXISTS co_change_edges (
            file_a TEXT NOT NULL,
            file_b TEXT NOT NULL,
            co_change_count INTEGER DEFAULT 1,
            last_changed_at INTEGER NOT NULL,
            confidence REAL DEFAULT 0.5,
            PRIMARY KEY (file_a, file_b)
        );",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_co_change_fa ON co_change_edges(file_a);",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_co_change_fb ON co_change_edges(file_b);",
        [],
    )?;

    Ok(())
}
