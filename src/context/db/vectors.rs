use anyhow::Result;
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};

use super::CodeGraphDb;

impl CodeGraphDb {
    pub fn store_symbol_vector(&self, symbol_id: &str, vector: &[f32], model_version: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();

        self.conn.execute(
            "INSERT OR REPLACE INTO symbol_vectors (symbol_id, vector, model_version, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![symbol_id, bytes, model_version, now],
        )?;
        Ok(())
    }

    pub fn store_chunk_vector(&self, chunk_id: i64, vector: &[f32], model_version: &str) -> Result<()> {
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();

        self.conn.execute(
            "INSERT OR REPLACE INTO chunk_vectors (chunk_id, vector, model_version)
             VALUES (?1, ?2, ?3)",
            params![chunk_id, bytes, model_version],
        )?;
        Ok(())
    }

    pub fn get_symbols_without_vectors(&self, limit: usize) -> Result<Vec<(String, String, String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, s.kind, s.file_path, s.signature
             FROM symbols s
             LEFT JOIN symbol_vectors v ON s.id = v.symbol_id
             WHERE v.symbol_id IS NULL
             LIMIT ?",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn get_chunks_without_vectors(&self, limit: usize) -> Result<Vec<(i64, String, usize, usize, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.file_path, c.start_line, c.end_line, c.chunk_text
             FROM content_chunks c
             LEFT JOIN chunk_vectors v ON c.id = v.chunk_id
             WHERE v.chunk_id IS NULL
             LIMIT ?",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get::<_, i64>(2)? as usize,
                row.get::<_, i64>(3)? as usize,
                row.get(4)?,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub const HNSW_DISPATCH_THRESHOLD: usize = 10_000;

    pub fn vector_search_symbols(
        &self,
        query_vector: &[f32],
        top_k: usize,
        threshold: f32,
    ) -> Result<Vec<SymbolSearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT v.vector, s.name, s.kind, s.file_path, s.start_line, s.signature
             FROM symbol_vectors v
             JOIN symbols s ON v.symbol_id = s.id",
        )?;

        let rows = stmt.query_map([], |row| {
            let blob: Vec<u8> = row.get(0)?;
            let name: String = row.get(1)?;
            let kind: String = row.get(2)?;
            let file_path: String = row.get(3)?;
            let start_line = row.get::<_, i64>(4)? as usize;
            let signature: Option<String> = row.get(5)?;
            Ok((blob, name, kind, file_path, start_line, signature))
        })?;

        let mut all_items = Vec::new();
        for r in rows {
            all_items.push(r?);
        }

        // Dual-Engine: Use HNSW Graph for massive repositories (>10,000 vectors)
        if all_items.len() > Self::HNSW_DISPATCH_THRESHOLD {
            let mut hnsw = super::super::hnsw::HnswIndex::new(16, 64, 32);
            for (blob, name, kind, file_path, start_line, signature) in all_items {
                let vec = bytes_to_f32_vec(&blob);
                hnsw.insert(vec, SymbolSearchResult {
                    name,
                    kind,
                    file_path,
                    start_line,
                    signature,
                    score: 0.0,
                });
            }

            let hnsw_results = hnsw.search(query_vector, top_k, threshold);
            return Ok(hnsw_results
                .into_iter()
                .map(|(score, payload)| {
                    let mut item = payload.clone();
                    item.score = score;
                    item
                })
                .collect());
        }

        // Default: Fast Flat Scan for standard repositories (<=10,000 vectors)
        let mut matches = Vec::new();
        for (blob, name, kind, file_path, start_line, signature) in all_items {
            let vec = bytes_to_f32_vec(&blob);
            let score = cosine_similarity(query_vector, &vec);
            if score >= threshold {
                matches.push(SymbolSearchResult {
                    name,
                    kind,
                    file_path,
                    start_line,
                    signature,
                    score,
                });
            }
        }

        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(top_k);
        Ok(matches)
    }

    pub fn vector_search_chunks(
        &self,
        query_vector: &[f32],
        top_k: usize,
        threshold: f32,
    ) -> Result<Vec<ChunkSearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT v.vector, c.file_path, c.start_line, c.end_line
             FROM chunk_vectors v
             JOIN content_chunks c ON v.chunk_id = c.id",
        )?;

        let rows = stmt.query_map([], |row| {
            let blob: Vec<u8> = row.get(0)?;
            let file_path: String = row.get(1)?;
            let start_line = row.get::<_, i64>(2)? as usize;
            let end_line = row.get::<_, i64>(3)? as usize;
            Ok((blob, file_path, start_line, end_line))
        })?;

        let mut all_items = Vec::new();
        for r in rows {
            all_items.push(r?);
        }

        // Dual-Engine: Use HNSW Graph for massive repositories (>10,000 vectors)
        if all_items.len() > Self::HNSW_DISPATCH_THRESHOLD {
            let mut hnsw = super::super::hnsw::HnswIndex::new(16, 64, 32);
            for (blob, file_path, start_line, end_line) in all_items {
                let vec = bytes_to_f32_vec(&blob);
                hnsw.insert(vec, ChunkSearchResult {
                    file_path,
                    start_line,
                    end_line,
                    score: 0.0,
                });
            }

            let hnsw_results = hnsw.search(query_vector, top_k, threshold);
            return Ok(hnsw_results
                .into_iter()
                .map(|(score, payload)| {
                    let mut item = payload.clone();
                    item.score = score;
                    item
                })
                .collect());
        }

        // Default: Fast Flat Scan for standard repositories (<=10,000 vectors)
        let mut matches = Vec::new();
        for (blob, file_path, start_line, end_line) in all_items {
            let vec = bytes_to_f32_vec(&blob);
            let score = cosine_similarity(query_vector, &vec);
            if score >= threshold {
                matches.push(ChunkSearchResult {
                    file_path,
                    start_line,
                    end_line,
                    score,
                });
            }
        }

        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(top_k);
        Ok(matches)
    }
}

#[derive(Debug, Clone)]
pub struct SymbolSearchResult {
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: usize,
    pub signature: Option<String>,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct ChunkSearchResult {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub score: f32,
}

pub fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
        .collect()
}

pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let dot: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
    let norm1: f32 = v1.iter().map(|a| a * a).sum::<f32>().sqrt();
    let norm2: f32 = v2.iter().map(|b| b * b).sum::<f32>().sqrt();
    if norm1 == 0.0 || norm2 == 0.0 {
        return 0.0;
    }
    dot / (norm1 * norm2)
}

