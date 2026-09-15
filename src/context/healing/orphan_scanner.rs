//! Dead / Orphan symbol scanner for detecting unused code artifacts.

use anyhow::Result;
use rusqlite::{params, Connection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanSymbol {
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub line: usize,
}

/// Detects internal/private symbols that have zero callers and zero callees.
pub fn detect_orphan_symbols(conn: &Connection, limit: usize) -> Result<Vec<OrphanSymbol>> {
    // Exclude test files, main entrypoints, and documentation
    let mut stmt = conn.prepare(
        "SELECT s.name, s.kind, s.file_path, s.start_line
         FROM symbols s
         LEFT JOIN symbol_edges in_e ON s.id = in_e.target_id
         LEFT JOIN symbol_edges out_e ON s.id = out_e.source_id
         WHERE in_e.source_id IS NULL
           AND out_e.target_id IS NULL
           AND s.kind IN ('function', 'method', 'struct', 'class')
           AND s.name NOT IN ('main', 'new', 'default', 'init')
           AND s.file_path NOT LIKE '%test%'
           AND s.file_path NOT LIKE '%tests%'
           AND s.file_path NOT LIKE '%.md'
         LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok(OrphanSymbol {
            name: row.get::<_, String>(0)?,
            kind: row.get::<_, String>(1)?,
            file_path: row.get::<_, String>(2)?,
            line: row.get::<_, i64>(3)? as usize,
        })
    })?;

    let mut orphans = Vec::new();
    for r in rows.flatten() {
        orphans.push(r);
    }

    Ok(orphans)
}
