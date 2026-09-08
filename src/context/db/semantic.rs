use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use super::CodeGraphDb;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticEdge {
    pub id: i64,
    pub source_symbol: String,
    pub target_symbol: String,
    pub relation_type: String,
    pub description: Option<String>,
    pub confidence: f64,
    pub created_by: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSummary {
    pub module_path: String,
    pub title: String,
    pub summary: String,
    pub tags: Option<String>,
    pub updated_at: i64,
    pub updated_by: String,
}

impl CodeGraphDb {
    pub fn upsert_semantic_edge(
        &self,
        source: &str,
        target: &str,
        relation: &str,
        description: Option<&str>,
        confidence: f64,
        created_by: &str,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO semantic_edges (source_symbol, target_symbol, relation_type, description, confidence, created_by, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(source_symbol, target_symbol, relation_type) DO UPDATE SET
                 description = excluded.description,
                 confidence = excluded.confidence,
                 created_by = excluded.created_by,
                 created_at = excluded.created_at",
            params![source, target, relation, description, confidence, created_by, now],
        )?;
        Ok(())
    }

    pub fn query_semantic_edges_for_symbol(&self, symbol: &str) -> Result<Vec<SemanticEdge>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_symbol, target_symbol, relation_type, description, confidence, created_by, created_at
             FROM semantic_edges
             WHERE source_symbol = ?1 OR target_symbol = ?1 OR source_symbol LIKE ?2 OR target_symbol LIKE ?2
             ORDER BY confidence DESC, created_at DESC LIMIT 50",
        )?;
        let pattern = format!("%{}%", symbol);
        let rows = stmt.query_map(params![symbol, pattern], |row| {
            Ok(SemanticEdge {
                id: row.get(0)?,
                source_symbol: row.get(1)?,
                target_symbol: row.get(2)?,
                relation_type: row.get(3)?,
                description: row.get(4)?,
                confidence: row.get(5)?,
                created_by: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        let mut edges = Vec::new();
        for r in rows {
            edges.push(r?);
        }
        Ok(edges)
    }

    pub fn list_all_semantic_edges(&self, limit: usize) -> Result<Vec<SemanticEdge>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_symbol, target_symbol, relation_type, description, confidence, created_by, created_at
             FROM semantic_edges
             ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(SemanticEdge {
                id: row.get(0)?,
                source_symbol: row.get(1)?,
                target_symbol: row.get(2)?,
                relation_type: row.get(3)?,
                description: row.get(4)?,
                confidence: row.get(5)?,
                created_by: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        let mut edges = Vec::new();
        for r in rows {
            edges.push(r?);
        }
        Ok(edges)
    }

    pub fn upsert_domain_summary(
        &self,
        module_path: &str,
        title: &str,
        summary: &str,
        tags: Option<&str>,
        updated_by: &str,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO domain_summaries (module_path, title, summary, tags, updated_at, updated_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(module_path) DO UPDATE SET
                 title = excluded.title,
                 summary = excluded.summary,
                 tags = excluded.tags,
                 updated_at = excluded.updated_at,
                 updated_by = excluded.updated_by",
            params![module_path, title, summary, tags, now, updated_by],
        )?;
        Ok(())
    }

    pub fn search_domain_summaries_fts(&self, query: &str, limit: usize) -> Result<Vec<DomainSummary>> {
        let sanitized = super::sanitize_fts5_query(query);
        if sanitized.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = self.conn.prepare(
            "SELECT d.module_path, d.title, d.summary, d.tags, d.updated_at, d.updated_by
             FROM domain_summaries_fts f
             JOIN domain_summaries d ON f.rowid = d.rowid
             WHERE domain_summaries_fts MATCH ?1
             ORDER BY rank LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![sanitized, limit as i64], |row| {
            Ok(DomainSummary {
                module_path: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                tags: row.get(3)?,
                updated_at: row.get(4)?,
                updated_by: row.get(5)?,
            })
        })?;
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }
}
