//! Coupled file prediction and forgotten file alerts based on co-change graph.

use anyhow::Result;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use super::recorder::clean_path;

#[derive(Debug, Clone, PartialEq)]
pub struct CoupledFileWarning {
    pub file: String,
    pub coupled_with: String,
    pub co_change_count: usize,
    pub confidence: f64,
}

/// Predicts coupled files that have historically changed together with `current_files`,
/// but are currently omitted from the edit set.
pub fn predict_coupled_files(
    conn: &Connection,
    current_files: &[String],
    min_count: usize,
    min_confidence: f64,
) -> Result<Vec<CoupledFileWarning>> {
    let current_set: HashSet<String> = current_files
        .iter()
        .map(|f| clean_path(f))
        .filter(|f| !f.is_empty())
        .collect();

    if current_set.is_empty() {
        return Ok(Vec::new());
    }

    let mut warnings: HashMap<String, CoupledFileWarning> = HashMap::new();

    let mut stmt = conn.prepare(
        "SELECT file_b, co_change_count, confidence FROM co_change_edges
         WHERE file_a = ?1 AND co_change_count >= ?2 AND confidence >= ?3
         UNION
         SELECT file_a, co_change_count, confidence FROM co_change_edges
         WHERE file_b = ?1 AND co_change_count >= ?2 AND confidence >= ?3",
    )?;

    for touched_file in &current_set {
        let rows = stmt.query_map(
            params![touched_file, min_count as i64, min_confidence],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as usize,
                    row.get::<_, f64>(2)?,
                ))
            },
        )?;

        for item in rows.flatten() {
            let (coupled_file, count, conf) = item;
            if !current_set.contains(&coupled_file) {
                // If already warned by another file, keep higher confidence
                let existing = warnings.get(&coupled_file);
                if existing.map(|e| e.confidence < conf).unwrap_or(true) {
                    warnings.insert(
                        coupled_file.clone(),
                        CoupledFileWarning {
                            file: coupled_file,
                            coupled_with: touched_file.clone(),
                            co_change_count: count,
                            confidence: conf,
                        },
                    );
                }
            }
        }
    }

    let mut result: Vec<CoupledFileWarning> = warnings.into_values().collect();
    result.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    Ok(result)
}
