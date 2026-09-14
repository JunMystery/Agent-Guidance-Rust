use anyhow::Result;
use super::CodeGraphDb;
use super::vectors::{bytes_to_f32_vec, dot_similarity_bytes};

impl CodeGraphDb {
    /// Fetches the top-K context vectors (from symbols and chunks) closest to the query vector.
    pub fn fetch_top_context_vectors(&self, query_vector: &[f32], top_k: usize) -> Result<Vec<Vec<f32>>> {
        if query_vector.is_empty() || top_k == 0 {
            return Ok(Vec::new());
        }

        let mut scored_vectors: Vec<(f32, Vec<f32>)> = Vec::new();

        // 1. Scan symbol vectors
        if let Ok(mut stmt) = self.conn.prepare("SELECT vector FROM symbol_vectors") {
            let rows = stmt.query_map([], |row| {
                let blob: Vec<u8> = row.get(0)?;
                Ok(blob)
            })?;
            for r in rows.flatten() {
                let score = dot_similarity_bytes(query_vector, &r);
                if score > 0.05 {
                    let vec = bytes_to_f32_vec(&r);
                    scored_vectors.push((score, vec));
                }
            }
        }

        // 2. Scan chunk vectors
        if let Ok(mut stmt) = self.conn.prepare("SELECT vector FROM chunk_vectors") {
            let rows = stmt.query_map([], |row| {
                let blob: Vec<u8> = row.get(0)?;
                Ok(blob)
            })?;
            for r in rows.flatten() {
                let score = dot_similarity_bytes(query_vector, &r);
                if score > 0.05 {
                    let vec = bytes_to_f32_vec(&r);
                    scored_vectors.push((score, vec));
                }
            }
        }

        scored_vectors.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored_vectors.truncate(top_k);

        Ok(scored_vectors.into_iter().map(|(_, v)| v).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_top_context_vectors_empty_db() {
        let dir = std::env::temp_dir().join(format!("test_vec_empty_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let db_file = dir.join("test.db");
        let db = CodeGraphDb::open(&db_file).unwrap();

        let qv = vec![1.0, 0.0, 0.0];
        let res = db.fetch_top_context_vectors(&qv, 5).unwrap();
        assert!(res.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_fetch_top_context_vectors_populated() {
        let dir = std::env::temp_dir().join(format!("test_vec_pop_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let db_file = dir.join("test.db");
        let db = CodeGraphDb::open(&db_file).unwrap();
        db.conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();

        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        db.store_symbol_vector("sym_1", &v1, "model-1").unwrap();
        db.store_chunk_vector(101, &v2, "model-1").unwrap();

        let qv = vec![0.9, 0.1, 0.0];
        let res = db.fetch_top_context_vectors(&qv, 2).unwrap();
        assert_eq!(res.len(), 2);
        assert_eq!(res[0], v1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
