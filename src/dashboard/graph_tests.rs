use super::*;
use std::fs;

#[path = "graph_test_fixtures.rs"]
pub(crate) mod graph_test_fixtures;

#[path = "graph_file_tests.rs"]
mod graph_file_tests;

#[path = "graph_function_tests.rs"]
mod graph_function_tests;

#[test]
fn test_parse_graph_query_global_and_missing() {
    let empty = parse_graph_query("");
    assert_eq!(empty.project, None);
    assert_eq!(empty.view, "symbols");
    assert_eq!(empty.file, None);

    let all = parse_graph_query("project=all&view=files");
    assert_eq!(all.project, None);
    assert_eq!(all.view, "files");

    let empty_proj = parse_graph_query("project=&view=symbols");
    assert_eq!(empty_proj.project, None);
}

#[test]
fn test_parse_graph_query_explicit_project() {
    let q = parse_graph_query("project=e%3A%2Fmy%20repo&view=file_functions&file=src%2Fmain.rs");
    assert_eq!(q.project.as_deref(), Some("e:/my repo"));
    assert_eq!(q.view, "file_functions");
    assert_eq!(q.file.as_deref(), Some("src/main.rs"));
}

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
    let _ = fs::remove_dir_all(&temp_dir);
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
            signature TEXT,
            language TEXT DEFAULT 'unknown',
            namespace TEXT,
            receiver TEXT
        );
        CREATE TABLE symbol_edges (
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            edge_type TEXT NOT NULL,
            weight REAL DEFAULT 1.0,
            confidence REAL DEFAULT 1.0,
            category TEXT DEFAULT 'symbol',
            call_line INTEGER,
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

#[test]
fn test_query_graph_data_with_view_dispatch() {
    let (guard, conn) = graph_test_fixtures::create_test_project("view_dispatch");
    graph_test_fixtures::populate_test_topology(&conn);
    drop(conn);

    let res_files = query_graph_data_with_view(&guard.dir, "files", None).unwrap();
    assert_eq!(res_files["graph_available"], true);
    assert_eq!(res_files["view"], "files");
    assert_eq!(res_files["total_nodes"], 4);

    let res_fns = query_graph_data_with_view(&guard.dir, "file_functions", Some("src/controller.rs")).unwrap();
    assert_eq!(res_fns["graph_available"], true);
    assert_eq!(res_fns["view"], "file_functions");
    assert_eq!(res_fns["file"], "src/controller.rs");
}
