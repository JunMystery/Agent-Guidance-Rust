use super::*;
use rusqlite::Connection;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE symbols (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_path TEXT NOT NULL,
            start_line INTEGER DEFAULT 1
        );
        CREATE TABLE symbol_edges (
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            edge_type TEXT NOT NULL,
            PRIMARY KEY (source_id, target_id, edge_type)
        );"
    ).unwrap();
    conn
}

#[test]
fn test_detect_file_cycles_happy_path() {
    let conn = setup_test_db();

    // Insert 3 symbols across 3 files forming a cycle: A -> B -> C -> A
    conn.execute_batch(
        "INSERT INTO symbols (id, name, kind, file_path) VALUES
            ('s_a', 'fn_a', 'function', 'src/a.rs'),
            ('s_b', 'fn_b', 'function', 'src/b.rs'),
            ('s_c', 'fn_c', 'function', 'src/c.rs');
         INSERT INTO symbol_edges (source_id, target_id, edge_type) VALUES
            ('s_a', 's_b', 'calls'),
            ('s_b', 's_c', 'calls'),
            ('s_c', 's_a', 'calls');"
    ).unwrap();

    let cycles = detect_file_cycles(&conn).unwrap();
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0], vec!["src/a.rs", "src/b.rs", "src/c.rs"]);
}

#[test]
fn test_detect_file_cycles_no_cycle() {
    let conn = setup_test_db();

    // Directed DAG without cycle: A -> B -> C
    conn.execute_batch(
        "INSERT INTO symbols (id, name, kind, file_path) VALUES
            ('s_a', 'fn_a', 'function', 'src/a.rs'),
            ('s_b', 'fn_b', 'function', 'src/b.rs'),
            ('s_c', 'fn_c', 'function', 'src/c.rs');
         INSERT INTO symbol_edges (source_id, target_id, edge_type) VALUES
            ('s_a', 's_b', 'calls'),
            ('s_b', 's_c', 'calls');"
    ).unwrap();

    let cycles = detect_file_cycles(&conn).unwrap();
    assert!(cycles.is_empty());
}

#[test]
fn test_detect_orphan_symbols() {
    let conn = setup_test_db();

    // s_orphan has no incoming or outgoing edges
    conn.execute_batch(
        "INSERT INTO symbols (id, name, kind, file_path, start_line) VALUES
            ('s_used_1', 'caller_fn', 'function', 'src/active.rs', 10),
            ('s_used_2', 'callee_fn', 'function', 'src/active.rs', 20),
            ('s_orphan', 'dead_utility_function', 'function', 'src/legacy.rs', 35);
         INSERT INTO symbol_edges (source_id, target_id, edge_type) VALUES
            ('s_used_1', 's_used_2', 'calls');"
    ).unwrap();

    let orphans = detect_orphan_symbols(&conn, 10).unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].name, "dead_utility_function");
    assert_eq!(orphans[0].file_path, "src/legacy.rs");
    assert_eq!(orphans[0].line, 35);
}
