    use super::*;

    #[test]
    fn test_record_and_parse_learnings() {
        let temp_dir = std::env::temp_dir().join(format!("learnings_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        let res = record_project_learning(&temp_dir, "Always use mock database in tests", "build_test", true);
        assert!(res.is_ok());

        let res2 = record_project_learning(&temp_dir, "Prefer thin controller layer", "arch", false);
        assert!(res2.is_ok());

        let items = parse_learnings_file(&temp_dir);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].category, "build_test");
        assert!(items[0].is_pinned);
        assert_eq!(items[1].category, "arch");
        assert!(!items[1].is_pinned);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_category_keyword_matching_fallback() {
        let items = vec![
            LearningItem {
                category: "build_test".to_string(),
                content: "Use mock database pool".to_string(),
                is_pinned: true,
            },
            LearningItem {
                category: "arch".to_string(),
                content: "Use layered architecture".to_string(),
                is_pinned: false,
            },
            LearningItem {
                category: "gotcha".to_string(),
                content: "Check exFAT mount error 22".to_string(),
                is_pinned: false,
            },
        ];

        // When task mentions "test", Tier 2 matches build_test category
        let results = match_category_keywords(&items, "Run cargo test with mocks", 3);
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("[PINNED:build_test]"));

        // When task mentions "arch", Tier 2 matches arch category
        let results_arch = match_category_keywords(&items, "Refactor module structure and architecture", 3);
        assert_eq!(results_arch.len(), 1);
        assert!(results_arch[0].contains("[arch]"));

        // When task is unrelated
        let results_none = match_category_keywords(&items, "Cook carbonara spaghetti pasta recipe", 3);
        assert!(results_none.is_empty());
    }

    #[test]
    fn test_strict_context_empty_when_no_match() {
        let temp_dir = std::env::temp_dir().join(format!("strict_empty_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        let _ = record_project_learning(&temp_dir, "Use mock database pool", "build_test", false);

        // Test relevant task
        let relevant = get_semantic_relevant_learnings(&temp_dir, "Run cargo test with mock database", 3, 0.82);
        assert_eq!(relevant.len(), 1, "Relevant task should match");

        // Completely unrelated task with no keyword or vector match above 0.82 threshold
        let results = get_semantic_relevant_learnings(&temp_dir, "Cook carbonara spaghetti pasta recipe", 3, 0.82);
        assert!(results.is_empty(), "Strict Context mode should return empty list for unrelated task");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_compute_line_delta() {
        let original = "line 1\nline 2\nline 3\n";
        let current = "line 1\nline 2 modified\nline 3\nline 4\n";

        let (adds, dels) = compute_line_delta(original, current);
        assert_eq!(adds, 2); // "line 2 modified", "line 4"
        assert_eq!(dels, 1); // "line 2"
    }

    #[test]
    fn test_generate_session_diff_summary() {
        let temp_dir = std::env::temp_dir().join(format!("diff_summary_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);

        let mut state = ServerState::new();
        state.record_modified_file("src/main.rs");

        let summary = generate_session_diff_summary(&temp_dir, &state);
        assert!(summary.contains("Session Modification Summary"));
        assert!(summary.contains("src/main.rs"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pinned_learnings_survive_fifo_overflow() {
        let temp_dir = std::env::temp_dir().join(format!("pinned_fifo_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        // 1. Pre-populate 2 pinned items and 30 transient items
        let mut initial_items = vec![
            LearningItem {
                category: "security".to_string(),
                content: "Immutable rule 1".to_string(),
                is_pinned: true,
            },
            LearningItem {
                category: "policy".to_string(),
                content: "Immutable rule 2".to_string(),
                is_pinned: true,
            },
        ];

        for i in 0..30 {
            initial_items.push(LearningItem {
                category: "dev".to_string(),
                content: format!("Existing item {}", i),
                is_pinned: false,
            });
        }

        assert!(write_learnings_file(&temp_dir, &initial_items).is_ok());

        // 2. Add 2 new transient items (exceeding MAX_LEARNINGS_FIFO = 30)
        let _ = record_project_learning(&temp_dir, "Brand new transient learning A", "dev", false);
        let _ = record_project_learning(&temp_dir, "Brand new transient learning B", "dev", false);

        let items = parse_learnings_file(&temp_dir);
        // Total items should remain 2 pinned + 30 transient = 32
        assert_eq!(items.len(), 32);

        // Verify pinned items still exist 100%
        let pinned: Vec<&LearningItem> = items.iter().filter(|i| i.is_pinned).collect();
        assert_eq!(pinned.len(), 2);
        assert_eq!(pinned[0].content, "Immutable rule 1");
        assert_eq!(pinned[1].content, "Immutable rule 2");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_semantic_learning_deduplication() {
        let temp_dir = std::env::temp_dir().join(format!("dedup_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        // 1. Add initial learning
        let _ = record_project_learning(&temp_dir, "Run tests with mock database pool", "build_test", false);

        // 2. Add almost identical wording (will update existing entry)
        let _ = record_project_learning(&temp_dir, "Run tests with mock database pool", "build_test", true);

        let items = parse_learnings_file(&temp_dir);
        assert_eq!(items.len(), 1, "Duplicate should be merged");
        assert!(items[0].is_pinned, "Existing item should be elevated to pinned");

        let _ = fs::remove_dir_all(&temp_dir);
    }
