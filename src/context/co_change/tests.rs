use super::*;
use rusqlite::Connection;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE co_change_edges (
            file_a TEXT NOT NULL,
            file_b TEXT NOT NULL,
            co_change_count INTEGER DEFAULT 1,
            last_changed_at INTEGER NOT NULL,
            confidence REAL DEFAULT 0.5,
            PRIMARY KEY (file_a, file_b)
        );
        CREATE INDEX idx_co_change_fa ON co_change_edges(file_a);
        CREATE INDEX idx_co_change_fb ON co_change_edges(file_b);"
    ).unwrap();
    conn
}

#[test]
fn test_normalize_file_pair() {
    assert_eq!(normalize_file_pair("src/b.rs", "src/a.rs"), Some(("src/a.rs".into(), "src/b.rs".into())));
    assert_eq!(normalize_file_pair("src/a.rs", "src/b.rs"), Some(("src/a.rs".into(), "src/b.rs".into())));
    assert_eq!(normalize_file_pair("\\src\\a.rs", "/src/b.rs"), Some(("src/a.rs".into(), "src/b.rs".into())));
    assert_eq!(normalize_file_pair("src/a.rs", "src/a.rs"), None);
    assert_eq!(normalize_file_pair("", "src/a.rs"), None);
}

#[test]
fn test_record_co_change_and_confidence() {
    let conn = setup_test_db();

    // First record: count = 1, confidence = 0.5
    assert!(record_co_change(&conn, "src/auth.rs", "tests/auth_tests.rs").unwrap());

    // Second record: count = 2, confidence = 0.7
    assert!(record_co_change(&conn, "tests/auth_tests.rs", "src/auth.rs").unwrap());

    let (count, conf): (usize, f64) = conn.query_row(
        "SELECT co_change_count, confidence FROM co_change_edges WHERE file_a = 'src/auth.rs' AND file_b = 'tests/auth_tests.rs'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();

    assert_eq!(count, 2);
    assert!((conf - 0.7).abs() < 1e-4);
}

#[test]
fn test_record_session_co_changes() {
    let conn = setup_test_db();
    let files = vec![
        "src/model.rs".to_string(),
        "src/service.rs".to_string(),
        "tests/service_tests.rs".to_string(),
    ];

    // 3 pairs: (model, service), (model, service_tests), (service, service_tests)
    let recorded = record_session_co_changes(&conn, &files).unwrap();
    assert_eq!(recorded, 3);
}

#[test]
fn test_predict_coupled_files_alert() {
    let conn = setup_test_db();
    let _ = record_co_change(&conn, "src/handler.rs", "tests/handler_tests.rs").unwrap();
    let _ = record_co_change(&conn, "src/handler.rs", "tests/handler_tests.rs").unwrap();

    // Agent only edits src/handler.rs, omitting tests/handler_tests.rs
    let current = vec!["src/handler.rs".to_string()];
    let warnings = predict_coupled_files(&conn, &current, 1, 0.5).unwrap();

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].file, "tests/handler_tests.rs");
    assert_eq!(warnings[0].coupled_with, "src/handler.rs");
    assert_eq!(warnings[0].co_change_count, 2);

    // If agent ALSO edits tests/handler_tests.rs, no warning is emitted
    let both = vec!["src/handler.rs".to_string(), "tests/handler_tests.rs".to_string()];
    let no_warnings = predict_coupled_files(&conn, &both, 1, 0.5).unwrap();
    assert!(no_warnings.is_empty());
}
