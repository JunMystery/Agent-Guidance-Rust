use super::idf::{analyze_query_terms, compute_query_downweight_factor};
use super::intent::{classify_intent, is_guidance_path, is_test_path, is_utility_path, SearchIntent};
use super::scorer::calculate_hit_score;
use rusqlite::Connection;

#[test]
fn test_intent_classification_logic_vs_guidance() {
    // 1. Logic intent signals
    assert_eq!(classify_intent("handle_search"), SearchIntent::Logic);
    assert_eq!(classify_intent("CodeGraphDb::open"), SearchIntent::Logic);
    assert_eq!(classify_intent("fn parse_ast() -> Result"), SearchIntent::Logic);
    assert_eq!(classify_intent("src/context/mod.rs"), SearchIntent::Logic);
    assert_eq!(classify_intent("SELECT * FROM symbols WHERE"), SearchIntent::Logic);

    // 2. Guidance intent signals
    assert_eq!(classify_intent("how to use task_pipeline"), SearchIntent::Guidance);
    assert_eq!(classify_intent("architecture rules and guidelines"), SearchIntent::Guidance);
    assert_eq!(classify_intent("workflow gate documentation"), SearchIntent::Guidance);
    assert_eq!(classify_intent("roadmap for v1.8.0"), SearchIntent::Guidance);
    assert_eq!(classify_intent("coding standards protocol"), SearchIntent::Guidance);

    // 3. Universal fallback
    assert_eq!(classify_intent("query"), SearchIntent::Universal);
}

#[test]
fn test_path_role_tagging() {
    assert!(!is_guidance_path("src/main.rs"));
    assert!(!is_test_path("src/main.rs"));
    assert!(!is_utility_path("src/main.rs"));

    assert!(is_guidance_path("README.md"));
    assert!(is_guidance_path("docs/architecture.md"));
    assert!(is_guidance_path("skills/agent-guidance/SKILL.md"));

    assert!(is_test_path("tests/integration_tests.rs"));
    assert!(is_test_path("src/context/db_tests.rs"));
    assert!(is_test_path("src/scanner_test.rs"));
    assert!(is_test_path("tests/fixtures/sample.rs"));

    assert!(is_utility_path("src/utils/helpers.rs"));
    assert!(is_utility_path("src/common/format.rs"));
    assert!(is_utility_path("src/mcp/helpers.rs"));
}

#[test]
fn test_dynamic_idf_ubiquitous_term_penalty() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE files (path TEXT PRIMARY KEY, content_hash TEXT, size INTEGER, modified_at INTEGER, indexed_at INTEGER);
         CREATE TABLE symbols (id TEXT PRIMARY KEY, name TEXT, kind TEXT, file_path TEXT, parent TEXT, start_line INTEGER, end_line INTEGER, signature TEXT, language TEXT, namespace TEXT, receiver TEXT);"
    ).unwrap();

    // Insert 10 distinct files
    for i in 1..=10 {
        let fpath = format!("src/file_{}.rs", i);
        conn.execute("INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES (?1, 'h', 100, 0, 0)", rusqlite::params![fpath]).unwrap();
    }

    // Insert 'config' in 6 files (60% > 30% threshold -> ubiquitous)
    for i in 1..=6 {
        let fpath = format!("src/file_{}.rs", i);
        conn.execute("INSERT INTO symbols (id, name, kind, file_path, start_line, end_line) VALUES (?1, 'config', 'struct', ?2, 1, 10)",
            rusqlite::params![format!("s{}", i), fpath]).unwrap();
    }

    // Insert 'unique_parser' in only 1 file (10% < 30% -> specific)
    conn.execute("INSERT INTO symbols (id, name, kind, file_path, start_line, end_line) VALUES ('s_uniq', 'unique_parser', 'function', 'src/file_1.rs', 20, 30)", []).unwrap();

    let term_weights = analyze_query_terms(&conn, "config unique_parser");
    assert_eq!(term_weights.len(), 2);

    let config_weight = term_weights.iter().find(|t| t.term == "config").unwrap();
    assert!(config_weight.is_ubiquitous);
    assert_eq!(config_weight.multiplier, 0.25);

    let unique_weight = term_weights.iter().find(|t| t.term == "unique_parser").unwrap();
    assert!(!unique_weight.is_ubiquitous);
    assert_eq!(unique_weight.multiplier, 1.0);

    // Multi-term query with one ubiquitous term should not penalize whole query down to 0.25
    let factor = compute_query_downweight_factor(&term_weights);
    assert!(factor > 0.50);

    // Query with only ubiquitous term is penalized
    let ubi_only = analyze_query_terms(&conn, "config");
    let ubi_factor = compute_query_downweight_factor(&ubi_only);
    assert_eq!(ubi_factor, 0.25);
}

#[test]
fn test_test_and_utility_fixture_penalty() {
    let query = "database connection pool";

    // Regular source file: neutral
    let s_regular = calculate_hit_score("src/db/connection.rs", 1.0, query, SearchIntent::Logic, 1.0);
    assert!((s_regular - 1.50).abs() < 1e-4);

    // Test file without 'test' in query: penalized by 0.40 (1.50 * 0.40 = 0.60)
    let s_test = calculate_hit_score("tests/db_test.rs", 1.0, query, SearchIntent::Logic, 1.0);
    assert!((s_test - 0.60).abs() < 1e-4);

    // Test file WITH 'test' in query: boosted by 1.20 (1.50 * 1.20 = 1.80)
    let s_test_explicit = calculate_hit_score("tests/db_test.rs", 1.0, "test database connection pool", SearchIntent::Logic, 1.0);
    assert!((s_test_explicit - 1.80).abs() < 1e-4);

    // Utility file without 'util' in query: penalized by 0.70 (1.50 * 0.70 = 1.05)
    let s_util = calculate_hit_score("src/utils/db_helpers.rs", 1.0, query, SearchIntent::Logic, 1.0);
    assert!((s_util - 1.05).abs() < 1e-4);
}

#[test]
fn test_intent_gating_boosts_and_penalties() {
    let query = "code guidelines";

    // 1. Guidance Intent: markdown boosted 2.5x, code penalized to 0.35x
    let s_md_guidance = calculate_hit_score("PROJECT-STANDARDS.md", 1.0, query, SearchIntent::Guidance, 1.0);
    assert_eq!(s_md_guidance, 2.50);

    let s_rs_guidance = calculate_hit_score("src/context/db.rs", 1.0, query, SearchIntent::Guidance, 1.0);
    assert_eq!(s_rs_guidance, 0.35);

    // 2. Logic Intent: code boosted 1.5x, markdown penalized to 0.20x
    let s_rs_logic = calculate_hit_score("src/context/db.rs", 1.0, "parse_ast", SearchIntent::Logic, 1.0);
    assert_eq!(s_rs_logic, 1.50);

    let s_md_logic = calculate_hit_score("PROJECT-STANDARDS.md", 1.0, "parse_ast", SearchIntent::Logic, 1.0);
    assert_eq!(s_md_logic, 0.20);
}

#[test]
fn test_execute_ranked_search_end_to_end() {
    use super::execute_ranked_search;
    use crate::context::db::CodeGraphDb;

    let temp_dir = std::env::temp_dir().join(format!("test_ranked_search_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let db_path = temp_dir.join("test.db");
    let db = CodeGraphDb::open(&db_path).unwrap();

    // Insert 3 files: logic, test, and guidance
    db.upsert_file("src/router.rs", "h1", 100, 0).unwrap();
    db.upsert_file("tests/router_test.rs", "h2", 100, 0).unwrap();
    db.upsert_file("docs/routing_guide.md", "h3", 100, 0).unwrap();

    db.insert_symbol_full("s1", "handle_request", "function", "src/router.rs", None, 10, 20, Some("fn handle_request()"), Some("rust"), None, None).unwrap();
    db.insert_symbol_full("s2", "handle_request", "function", "tests/router_test.rs", None, 15, 25, Some("fn test_handle_request()"), Some("rust"), None, None).unwrap();
    db.insert_symbol_full("s3", "handle_request", "guide", "docs/routing_guide.md", None, 1, 30, Some("# handle_request guide"), Some("markdown"), None, None).unwrap();

    // 1. Query with Logic intent: src/router.rs must be #1, test and docs penalized
    let logic_res = execute_ranked_search(&db, "handle_request", Some("logic"), 5, |_| None);
    assert_eq!(logic_res.intent, SearchIntent::Logic);
    assert!(!logic_res.results.is_empty());
    assert_eq!(logic_res.results[0].path, "src/router.rs");

    // 2. Query with Guidance intent: docs/routing_guide.md must be #1
    let guide_res = execute_ranked_search(&db, "how to use handle_request", Some("guidance"), 5, |_| None);
    assert_eq!(guide_res.intent, SearchIntent::Guidance);
    assert!(!guide_res.results.is_empty());
    assert_eq!(guide_res.results[0].path, "docs/routing_guide.md");

    // 3. Query with explicit test in query: tests/router_test.rs penalty lifted and ranked #1
    let test_res = execute_ranked_search(&db, "test handle_request", Some("logic"), 5, |_| None);
    assert!(!test_res.results.is_empty());
    assert_eq!(test_res.results[0].path, "tests/router_test.rs");

    let _ = std::fs::remove_dir_all(&temp_dir);
}
