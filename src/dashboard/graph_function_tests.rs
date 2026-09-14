//! Unit tests for query_file_functions_data.

use super::graph_test_fixtures::{create_test_project, populate_test_topology};
use crate::dashboard::graph_function_query::query_file_functions_data;

#[test]
fn test_internal_functions_retrieval() {
    let (guard, conn) = create_test_project("fn_internal");
    populate_test_topology(&conn);

    let res = query_file_functions_data(&conn, &guard.dir, "src/controller.rs").unwrap();
    let internal: Vec<_> = res.nodes.iter().filter(|n| !n.is_external).collect();
    assert_eq!(internal.len(), 3);

    assert!(internal.iter().any(|n| n.name == "handle_request" && n.scope == "internal"));
    assert!(internal.iter().any(|n| n.name == "validate_input" && n.scope == "internal"));
    assert!(internal.iter().any(|n| n.name == "ReqContext" && n.kind == "struct"));
}

#[test]
fn test_incoming_outgoing_edges_classification() {
    let (guard, conn) = create_test_project("fn_edges");
    populate_test_topology(&conn);

    let res = query_file_functions_data(&conn, &guard.dir, "src/controller.rs").unwrap();

    let internal_edges: Vec<_> = res.edges.iter().filter(|e| e.direction == "internal").collect();
    assert_eq!(internal_edges.len(), 1);
    assert_eq!(internal_edges[0].source, "src/controller.rs::handle_request");
    assert_eq!(internal_edges[0].target, "src/controller.rs::validate_input");

    let outgoing_edges: Vec<_> = res.edges.iter().filter(|e| e.direction == "outgoing").collect();
    assert_eq!(outgoing_edges.len(), 2);
    assert!(outgoing_edges.iter().all(|e| e.target == "src/service.rs::process_data"));

    let incoming_edges: Vec<_> = res.edges.iter().filter(|e| e.direction == "incoming").collect();
    assert_eq!(incoming_edges.len(), 1);
    assert_eq!(incoming_edges[0].source, "src/repository.rs::query_db");
    assert_eq!(incoming_edges[0].target, "src/controller.rs::handle_request");
}

#[test]
fn test_external_nodes_retrieval() {
    let (guard, conn) = create_test_project("fn_ext_nodes");
    populate_test_topology(&conn);

    let res = query_file_functions_data(&conn, &guard.dir, "src/controller.rs").unwrap();
    let external: Vec<_> = res.nodes.iter().filter(|n| n.is_external).collect();

    assert_eq!(external.len(), 2);
    let svc_node = external.iter().find(|n| n.file_path == "src/service.rs").unwrap();
    assert_eq!(svc_node.name, "process_data");
    assert!(svc_node.is_external);

    let repo_node = external.iter().find(|n| n.file_path == "src/repository.rs").unwrap();
    assert_eq!(repo_node.name, "query_db");
    assert!(repo_node.is_external);
}

#[test]
fn test_target_file_without_symbols() {
    let (guard, conn) = create_test_project("fn_empty");
    let res = query_file_functions_data(&conn, &guard.dir, "src/non_existent.rs").unwrap();
    assert_eq!(res.total_nodes, 0);
    assert_eq!(res.total_edges, 0);
}

#[test]
fn test_file_functions_all_retrieval() {
    let (guard, conn) = create_test_project("fn_all");
    populate_test_topology(&conn);

    let res = query_file_functions_data(&conn, &guard.dir, "all").unwrap();
    assert_eq!(res.file, "all");
    assert!(res.total_nodes > 0);
    assert!(res.total_edges > 0);
    // All edges in 'all' view should connect cross-file calls
    assert!(res.edges.iter().all(|e| e.direction == "outgoing"));
}
