//! Dynamic Inverse Document Frequency (IDF) and ubiquitous term down-weighting.

use rusqlite::{params, Connection};
use std::collections::HashSet;

pub const UBIQUITOUS_TECH_KEYWORDS: &[&str] = &[
    "graph", "project", "config", "state", "context", "data", "file",
    "item", "node", "edge", "handler", "util", "helper", "common",
    "test", "result", "error", "type", "name", "id", "value", "key",
];

pub const UBIQUITOUS_THRESHOLD_RATIO: f64 = 0.30;
pub const UBIQUITOUS_PENALTY_MULTIPLIER: f64 = 0.25;

#[derive(Debug, Clone)]
pub struct TermDocFreq {
    pub term: String,
    pub doc_count: usize,
    pub total_files: usize,
    pub is_ubiquitous: bool,
    pub multiplier: f64,
}

pub fn get_total_indexed_files(conn: &Connection) -> usize {
    conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_, i64>(0))
        .map(|c| c.max(0) as usize)
        .unwrap_or(0)
}

pub fn get_term_document_frequency(conn: &Connection, term: &str) -> usize {
    let clean = term.trim();
    if clean.is_empty() {
        return 0;
    }

    let pattern = format!("%{}%", clean);
    conn.query_row(
        "SELECT COUNT(DISTINCT file_path) FROM symbols WHERE name LIKE ?1 OR name = ?2",
        params![pattern, clean],
        |r| r.get::<_, i64>(0),
    )
    .map(|c| c.max(0) as usize)
    .unwrap_or(0)
}

pub fn analyze_query_terms(conn: &Connection, query: &str) -> Vec<TermDocFreq> {
    let total_files = get_total_indexed_files(conn);
    let raw_tokens: Vec<&str> = query
        .split_whitespace()
        .map(|t| t.trim().trim_matches(|c: char| !c.is_alphanumeric() && c != '_'))
        .filter(|t| !t.is_empty())
        .collect();

    let mut seen = HashSet::new();
    let mut results = Vec::new();

    for token in raw_tokens {
        let lower = token.to_lowercase();
        if seen.contains(&lower) {
            continue;
        }
        seen.insert(lower.clone());

        let doc_count = get_term_document_frequency(conn, &lower);
        let ratio = if total_files > 0 {
            doc_count as f64 / total_files as f64
        } else {
            0.0
        };

        let is_hard_keyword = UBIQUITOUS_TECH_KEYWORDS.contains(&lower.as_str());
        let is_ubiquitous = if total_files >= 10 {
            ratio >= UBIQUITOUS_THRESHOLD_RATIO || (is_hard_keyword && ratio >= 0.20)
        } else {
            is_hard_keyword
        };

        let multiplier = if is_ubiquitous {
            UBIQUITOUS_PENALTY_MULTIPLIER
        } else {
            1.0
        };

        results.push(TermDocFreq {
            term: lower,
            doc_count,
            total_files,
            is_ubiquitous,
            multiplier,
        });
    }

    results
}

pub fn compute_query_downweight_factor(term_weights: &[TermDocFreq]) -> f64 {
    if term_weights.is_empty() {
        return 1.0;
    }
    // If all terms are ubiquitous, penalize whole query
    let all_ubiquitous = term_weights.iter().all(|t| t.is_ubiquitous);
    if all_ubiquitous {
        UBIQUITOUS_PENALTY_MULTIPLIER
    } else {
        // If query has specific terms, those dominate
        let sum: f64 = term_weights.iter().map(|t| t.multiplier).sum();
        (sum / term_weights.len() as f64).clamp(0.25, 1.0)
    }
}
