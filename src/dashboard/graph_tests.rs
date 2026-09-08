use super::*;
use std::fs;

#[test]
fn test_query_graph_data_missing_project() {
    let fake_path = Path::new("C:\\non_existent_graph_test_path_123");
    let res = query_graph_data(fake_path).unwrap();
    assert_eq!(res["graph_available"], false);
    assert_eq!(res["reason"], "PROJECT_NOT_FOUND");
}

#[test]
fn test_query_graph_data_missing_db() {
    let temp_dir = std::env::temp_dir().join(format!("graph_test_nodb_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    let res = query_graph_data(&temp_dir).unwrap();
    assert_eq!(res["graph_available"], false);
    assert_eq!(res["reason"], "GRAPH_NOT_INDEXED");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_query_graph_data_success() {
    let temp_dir = std::env::temp_dir().join(format!("graph_test_ok_{}", std::process::id()));
    let context_dir = temp_dir.join(".agent-context");
    let _ = fs::create_dir_all(&context_dir);

    let db_path = context_dir.join("code_graph.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE symbols (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_path TEXT NOT NULL,
            parent TEXT,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            signature TEXT
        );
        CREATE TABLE symbol_edges (
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            edge_type TEXT NOT NULL,
            weight REAL DEFAULT 1.0,
            PRIMARY KEY (source_id, target_id, edge_type)
        );
        INSERT INTO symbols (id, name, kind, file_path, start_line, end_line)
        VALUES ('fn1', 'handle_request', 'function', 'src/main.rs', 10, 25),
               ('fn2', 'parse_data', 'function', 'src/parser.rs', 1, 15);
        INSERT INTO symbol_edges (source_id, target_id, edge_type, weight)
        VALUES ('fn1', 'fn2', 'calls', 1.0);",
    ).unwrap();
    drop(conn);

    let res = query_graph_data(&temp_dir).unwrap();
    assert_eq!(res["graph_available"], true);
    assert_eq!(res["total_nodes"], 2);
    assert_eq!(res["total_edges"], 1);

    let nodes = res["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0]["label"], "handle_request");
    assert_eq!(nodes[0]["loc"], 16);

    let edges = res["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["source"], "fn1");
    assert_eq!(edges[0]["target"], "fn2");
    assert_eq!(edges[0]["type"], "calls");

    assert!(res["mermaid_dag"].as_str().is_some());
    assert!(res["mermaid_dag"].as_str().unwrap().contains("graph TD"));

    let _ = fs::remove_dir_all(&temp_dir);
}

