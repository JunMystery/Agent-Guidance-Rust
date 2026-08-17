    use super::*;

    #[test]
    fn test_sanitize_fts5_query() {
        let input = "fn_test OR NOT * NEAR 'quote'";
        let sanitized = sanitize_fts5_query(input);
        assert_eq!(sanitized, "\"fn_test\" \"quote\"");
    }

    #[test]
    fn test_alias_learning_and_lookup() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("ag_alias_test_{}", std::process::id()));
        let db_path = temp_dir.join("test_code_graph.db");
        let db = CodeGraphDb::open(&db_path)?;

        // 1. Initial lookup should be empty
        let empty = db.lookup_aliases("đăng nhập", 5)?;
        assert!(empty.is_empty());

        // 2. Learn alias
        db.upsert_alias("đăng nhập", "src/auth/service.rs", Some("AuthenticationService"), Some(42))?;

        // 3. Lookup should hit
        let hits = db.lookup_aliases("đăng nhập", 5)?;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].resolved_path, "src/auth/service.rs");
        assert_eq!(hits[0].resolved_symbol.as_deref(), Some("AuthenticationService"));
        assert_eq!(hits[0].resolved_line, Some(42));
        assert_eq!(hits[0].hit_count, 1);
        assert!((hits[0].confidence - 0.8).abs() < f64::EPSILON);

        // 4. Substring query lookup ("tính năng đăng nhập")
        let sub_hits = db.lookup_aliases("tính năng đăng nhập", 5)?;
        assert_eq!(sub_hits.len(), 1);

        // 5. Repeated learn increases confidence & hit_count
        db.upsert_alias("đăng nhập", "src/auth/service.rs", Some("AuthenticationService"), Some(42))?;
        let bumped = db.lookup_aliases("đăng nhập", 5)?;
        assert_eq!(bumped[0].hit_count, 2);
        assert!((bumped[0].confidence - 0.85).abs() < 1e-5);

        // 6. Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_alias_decay() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("ag_decay_test_{}", std::process::id()));
        let db_path = temp_dir.join("test_decay.db");
        let db = CodeGraphDb::open(&db_path)?;

        let old_time = 1000; // Epoch way back
        db.conn.execute(
            "INSERT INTO aliases (alias_term, resolved_path, resolved_symbol, resolved_line, hit_count, confidence, created_at, last_used_at)
             VALUES ('old_term', 'src/old.rs', 'OldStruct', 10, 1, 0.8, ?1, ?1)",
            params![old_time],
        )?;

        // Running decay should remove the 90+ days old entry
        let deleted = db.decay_aliases()?;
        assert_eq!(deleted, 1);

        let res = db.lookup_aliases("old_term", 5)?;
        assert!(res.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_file_symbols_edges_and_chunks() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("ag_graph_test_{}", std::process::id()));
        let db_path = temp_dir.join("test_graph.db");
        let db = CodeGraphDb::open(&db_path)?;

        // 1. Insert file
        db.upsert_file("src/main.rs", "hash123", 500, 1000)?;
        assert_eq!(db.get_file_content_hash("src/main.rs")?, Some("hash123".to_string()));

        // 2. Insert symbols
        db.insert_symbol("src/main.rs::fn::main::L1", "main", "function", "src/main.rs", None, 1, 10, Some("fn main()"))?;
        db.insert_symbol("src/main.rs::fn::helper::L12", "helper", "function", "src/main.rs", None, 12, 20, Some("fn helper()"))?;

        // 3. Search symbols FTS
        let syms = db.search_symbols("main", 5)?;
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].1, "main");

        // 4. Insert edge
        db.insert_edge("src/main.rs::fn::main::L1", "src/main.rs::fn::helper::L12", "calls", 1.0)?;
        let related = db.search_related_symbols("main")?;
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].0, "main");
        assert_eq!(related[0].1, "calls");
        assert_eq!(related[0].2, "helper");

        // 5. Insert content chunks
        let chunk_id = db.insert_chunk("src/main.rs", 1, 15, "chunkhash1", "fn main() {\n    let timeout = Duration::from_secs(30);\n    helper();\n}")?;
        assert!(chunk_id > 0);

        // 6. Search content FTS
        let fts_hits = db.search_content_fts("timeout", 5)?;
        assert_eq!(fts_hits.len(), 1);
        assert_eq!(fts_hits[0].0, "src/main.rs");
        assert_eq!(fts_hits[0].1, 1);
        assert_eq!(fts_hits[0].2, 15);

        // 7. Clear file data
        db.clear_file_data("src/main.rs")?;
        assert!(db.search_symbols("main", 5)?.is_empty());
        assert!(db.search_content_fts("timeout", 5)?.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_vector_storage_and_search() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("ag_vec_test_{}", std::process::id()));
        let db_path = temp_dir.join("test_vec.db");
        let db = CodeGraphDb::open(&db_path)?;

        // 1. Setup file and symbol
        db.upsert_file("src/auth.rs", "h1", 100, 100)?;
        db.insert_symbol("src/auth.rs::fn::login::L1", "login", "function", "src/auth.rs", None, 1, 10, Some("fn login()"))?;

        // 2. Store symbol vector
        let vec_data = vec![1.0, 0.0, 0.0, 0.0];
        db.store_symbol_vector("src/auth.rs::fn::login::L1", &vec_data, "v1")?;

        // 3. Search with matching vector
        let query_vec = vec![1.0, 0.0, 0.0, 0.0];
        let hits = db.vector_search_symbols(&query_vec, 5, 0.9)?;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "login");
        assert!((hits[0].score - 1.0).abs() < 1e-5);

        // 4. Store and search chunk vector
        let chunk_id = db.insert_chunk("src/auth.rs", 1, 10, "ch1", "fn login() { ... }")?;
        db.store_chunk_vector(chunk_id, &vec_data, "v1")?;
        let chunk_hits = db.vector_search_chunks(&query_vec, 5, 0.9)?;
        assert_eq!(chunk_hits.len(), 1);
        assert_eq!(chunk_hits[0].file_path, "src/auth.rs");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_hnsw_dispatch_threshold_vector_search() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("hnsw_dispatch_test_{}", std::process::id()));
        let db_path = temp_dir.join("test_hnsw.db");
        let db = CodeGraphDb::open(&db_path)?;

        // Insert mock symbols
        db.upsert_file("src/jwt.rs", "h1", 100, 100)?;
        db.insert_symbol("src/jwt.rs::fn::verify::L1", "verify_token", "function", "src/jwt.rs", None, 1, 10, Some("fn verify_token()"))?;
        db.store_symbol_vector("src/jwt.rs::fn::verify::L1", &[0.95, 0.05, 0.0], "v1")?;

        // Test direct HNSW indexer search
        let mut hnsw = super::super::hnsw::HnswIndex::new(16, 64, 32);
        hnsw.insert(vec![0.95, 0.05, 0.0], "verify_token");
        hnsw.insert(vec![0.0, 0.95, 0.05], "other_func");

        let results = hnsw.search(&[0.99, 0.01, 0.0], 1, 0.8);
        assert_eq!(results.len(), 1);
        assert_eq!(*results[0].1, "verify_token");
        assert!(results[0].0 > 0.9);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
