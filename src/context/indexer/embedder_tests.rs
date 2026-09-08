//! Unit tests for batched embedder and content-addressable cache.

#[cfg(test)]
mod tests {
    use crate::context::db::CodeGraphDb;
    use crate::context::indexer::compute_hash;
    use crate::context::indexer::embedder::embed_symbols_batched;

    #[test]
    fn test_cached_embedding_storage_and_retrieval() {
        let temp_dir = std::env::temp_dir().join("test_embedder_cache");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        let db = CodeGraphDb::open(&temp_dir.join("code_graph.db")).unwrap();
        let dummy_vec = vec![0.1f32, 0.2, 0.3, 0.4];
        let p_hash = "dummy_hash_12345";

        db.store_cached_embedding(p_hash, &dummy_vec, "test-model").unwrap();

        let loaded = db.get_cached_embedding(p_hash).unwrap();
        assert!(loaded.is_some());
        let v = loaded.unwrap();
        assert_eq!(v.len(), 4);
        assert!((v[0] - 0.1).abs() < 1e-5);
        assert!((v[3] - 0.4).abs() < 1e-5);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_embed_symbols_cache_hit_bypasses_model() {
        let temp_dir = std::env::temp_dir().join("test_embedder_hit");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        let db = CodeGraphDb::open(&temp_dir.join("code_graph.db")).unwrap();
        let dummy_vec = vec![0.5f32, 0.6, 0.7, 0.8];

        let passage = "function in src/lib.rs — pub fn execute()";
        let p_hash = compute_hash(passage);
        db.store_cached_embedding(&p_hash, &dummy_vec, "multilingual-e5-small").unwrap();

        db.upsert_file("src/lib.rs", "dummy_file_hash", 100, 1000).unwrap();
        db.insert_symbol(
            "src/lib.rs::function::execute::L10",
            "execute",
            "function",
            "src/lib.rs",
            None,
            10,
            20,
            Some("pub fn execute()"),
        ).unwrap();

        // Even with no cached neural model in memory, cache hit resolves the vector!
        let count = embed_symbols_batched(&db, 10, 32).unwrap();
        assert_eq!(count, 1);

        // Verify that symbol_vectors now contains the precomputed vector
        let unindexed = db.get_symbols_without_vectors(10).unwrap();
        assert!(unindexed.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
