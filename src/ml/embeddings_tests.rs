    use super::*;
    use crate::catalog::store::SkillSource;

    #[test]
    fn test_prefix_formatting() {
        let prompt_query = format!("query: {}", "test code");
        let prompt_passage = format!("passage: {}", "test code");
        assert_eq!(prompt_query, "query: test code");
        assert_eq!(prompt_passage, "passage: test code");
    }

    #[test]
    fn test_cosine_similarity() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let v3 = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&v1, &v2) - 1.0).abs() < 1e-5);
        assert!((cosine_similarity(&v1, &v3) - 0.0).abs() < 1e-5);
    }

    #[test]
    #[ignore = "Requires pre-cached HuggingFace model files; avoid network I/O in CI"]
    fn test_hybrid_vector_search_fallback() {
        let candidates = vec![
            SkillItem {
                name: "context-budget".to_string(),
                relative_path: "context-budget/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "Reducing context size and managing token limits.".to_string(),
            },
            SkillItem {
                name: "rust-testing".to_string(),
                relative_path: "rust-testing/SKILL.md".to_string(),
                source: SkillSource::Embedded,
                content: "Rust unit and integration testing.".to_string(),
            },
        ];

        let results = hybrid_vector_search("reducing context size", &candidates, 2);
        assert!(!results.is_empty());
        assert_eq!(results[0].1.name, "context-budget");
    }

    #[test]
    fn test_device_resolution_and_env_override() {
        // 1. Test forced CPU via env var
        unsafe {
            std::env::set_var("AGENT_GUIDANCE_DEVICE", "cpu");
        }
        let (dev, name) = resolve_optimal_device();
        assert!(matches!(dev, Device::Cpu));
        assert_eq!(name, "CPU");

        // 2. Test auto mode resolution
        unsafe {
            std::env::set_var("AGENT_GUIDANCE_DEVICE", "auto");
        }
        let (_dev_auto, _name_auto) = resolve_optimal_device();
        // Should resolve cleanly without panicking
    }

    #[test]
    fn test_batch_empty_input() {
        if let Ok(model) = cached_model() {
            let res = model.embed_batch(&[], Some("passage"), 16);
            assert!(res.is_ok());
            assert!(res.unwrap().is_empty());
        }
    }

    #[test]
    fn test_batch_embedding_numerical_equivalence() {
        if let Ok(model) = cached_model() {
            let texts = [
                "Implement JWT authentication middleware",
                "Configure PostgreSQL index on user_id",
                "Setup Prometheus metrics exporter",
                "Handle websocket reconnect backoff",
            ];

            // 1. Single embeddings
            let mut single_vecs = Vec::new();
            for t in &texts {
                let v = model.embed_text(t, Some("passage")).unwrap();
                single_vecs.push(v);
            }

            // 2. Batch embeddings with batch_size = 2 (triggers multi-chunk batching)
            let batch_vecs = model.embed_batch(&texts, Some("passage"), 2).unwrap();
            assert_eq!(batch_vecs.len(), texts.len());

            // 3. Verify numerical equivalence (Cosine similarity >= 0.9999)
            for i in 0..texts.len() {
                let sim = cosine_similarity(&single_vecs[i], &batch_vecs[i]);
                assert!(
                    sim >= 0.9999,
                    "Batch vector {} deviates from single vector (sim: {})",
                    i,
                    sim
                );
            }
        }
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_gpu_skill_matrix_scoring_equivalence() {
        let dev = Device::Cpu;
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let v3 = vec![0.7071, 0.7071, 0.0];
        let vectors = vec![v1, v2, v3];

        let matrix = GpuSkillMatrix::from_vectors(&vectors, 12345, &dev).unwrap();
        assert_eq!(matrix.count, 3);
        assert_eq!(matrix.dim, 3);

        let query = vec![1.0, 0.0, 0.0];
        let scores = matrix.score_query(&query, &dev).unwrap();
        assert_eq!(scores.len(), 3);
        assert!((scores[0] - 1.0).abs() < 1e-5);
        assert!((scores[1] - 0.0).abs() < 1e-5);
        assert!((scores[2] - 0.7071).abs() < 1e-4);
    }

    #[test]
    fn test_gpu_batch_cosine_similarity() {
        let dev = Device::Cpu;
        let query = vec![0.0, 1.0, 0.0];
        let targets = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, -1.0, 0.0],
        ];
        let scores = gpu_batch_cosine_similarity(&query, &targets, &dev).unwrap();
        assert_eq!(scores.len(), 3);
        assert!((scores[0] - 0.0).abs() < 1e-5);
        assert!((scores[1] - 1.0).abs() < 1e-5);
        assert!((scores[2] - (-1.0)).abs() < 1e-5);
    }
