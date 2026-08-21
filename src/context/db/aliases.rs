use anyhow::Result;
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};

use super::CodeGraphDb;
use super::sanitize_fts5_query;

#[derive(Debug, Clone, PartialEq)]
pub struct AliasResult {
    pub resolved_path: String,
    pub resolved_symbol: Option<String>,
    pub resolved_line: Option<usize>,
    pub confidence: f64,
    pub hit_count: i64,
}


impl CodeGraphDb {
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

        // Phase 1: Decay confidence for aliases unused > 30 days
        self.conn.execute(
            "UPDATE aliases 
             SET confidence = MAX(0.1, confidence * 0.5) 
             WHERE (?1 - last_used_at) > ?2 AND confidence > 0.1",
            params![now, thirty_days],
        )?;

        // Phase 2: Delete stale aliases unused > 90 days
        let deleted = self.conn.execute(
            "DELETE FROM aliases WHERE (?1 - last_used_at) > ?2",
            params![now, ninety_days],
        )?;

        Ok(deleted)
    }

    // --- Plan 02: Indexer Storage & Query Methods ---

}
