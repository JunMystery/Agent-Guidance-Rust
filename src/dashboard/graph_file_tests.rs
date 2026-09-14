//! Unit tests for query_file_graph_data.

use super::graph_test_fixtures::{create_test_project, populate_test_topology};
use crate::dashboard::graph_file_query::query_file_graph_data;

#[test]
fn test_file_nodes_calculation() {
    let (guard, conn) = create_test_project("file_nodes");
    populate_test_topology(&conn);

    let res = query_file_graph_data(&conn, &guard.dir).unwrap();
    assert_eq!(res.total_nodes, 4);

    let ctrl = res.nodes.iter().find(|n| n.path == "src/controller.rs").unwrap();
    assert_eq!(ctrl.total_symbols, 3);
    assert_eq!(ctrl.loc, 70);
    assert_eq!(ctrl.out_degree, 1);
    assert_eq!(ctrl.in_degree, 1);

    let svc = res.nodes.iter().find(|n| n.path == "src/service.rs").unwrap();
    assert_eq!(svc.total_symbols, 1);
    assert_eq!(svc.loc, 40);
    assert_eq!(svc.out_degree, 1);
    assert_eq!(svc.in_degree, 1);

    let util = res.nodes.iter().find(|n| n.path == "src/util.rs").unwrap();
    assert_eq!(util.total_symbols, 1);
    assert_eq!(util.loc, 10);
    assert_eq!(util.out_degree, 0);
    assert_eq!(util.in_degree, 0);
}

#[test]
fn test_self_calls_exclusion() {
    let (guard, conn) = create_test_project("self_calls");
    populate_test_topology(&conn);

    let res = query_file_graph_data(&conn, &guard.dir).unwrap();
    let self_edges: Vec<_> = res.edges.iter().filter(|e| e.source == e.target).collect();
    assert_eq!(self_edges.len(), 0, "Self edges must be strictly excluded");

    let ctrl_to_ctrl = res.edges.iter().find(|e| e.source == "src/controller.rs" && e.target == "src/controller.rs");
    assert!(ctrl_to_ctrl.is_none());
}

#[test]
fn test_edge_weight_and_aggregated_calls() {
    let (guard, conn) = create_test_project("edge_weight");
    populate_test_topology(&conn);

    let res = query_file_graph_data(&conn, &guard.dir).unwrap();
    let edge = res.edges.iter().find(|e| e.source == "src/controller.rs" && e.target == "src/service.rs").unwrap();

    assert_eq!(edge.weight, 2.0);
    assert_eq!(edge.calls.len(), 2);

    let call1 = edge.calls.iter().find(|c| c.source_func == "handle_request").unwrap();
    assert_eq!(call1.target_func, "process_data");
    assert_eq!(call1.caller, "handle_request");
    assert_eq!(call1.callee, "process_data");
    assert_eq!(call1.call_line, Some(20));

    let call2 = edge.calls.iter().find(|c| c.source_func == "validate_input").unwrap();
    assert_eq!(call2.target_func, "process_data");
    assert_eq!(call2.call_line, Some(42));
}

#[test]
fn test_cyclic_dependencies() {
    let (guard, conn) = create_test_project("cycles");
    populate_test_topology(&conn);

    let res = query_file_graph_data(&conn, &guard.dir).unwrap();
    assert_eq!(res.total_edges, 3);

    assert!(res.edges.iter().any(|e| e.source == "src/controller.rs" && e.target == "src/service.rs"));
    assert!(res.edges.iter().any(|e| e.source == "src/service.rs" && e.target == "src/repository.rs"));
    assert!(res.edges.iter().any(|e| e.source == "src/repository.rs" && e.target == "src/controller.rs"));
}

#[test]
fn test_empty_db_file_graph() {
    let (guard, conn) = create_test_project("empty_db");
    let res = query_file_graph_data(&conn, &guard.dir).unwrap();
    assert_eq!(res.total_nodes, 0);
    assert_eq!(res.total_edges, 0);
}

#[test]
fn test_empirical_stress_10_internal_calls_zero_self_edges() {
    let (guard, conn) = create_test_project("stress_10_self_calls");
    conn.execute(
        "INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES ('src/monolith.rs', 'h1', 500, 100, 100)",
        [],
    ).unwrap();

    for i in 0..=10 {
        conn.execute(
            "INSERT INTO symbols (id, name, kind, file_path, start_line, end_line) VALUES (?1, ?2, 'function', 'src/monolith.rs', ?3, ?4)",
            rusqlite::params![format!("m::fn{}", i), format!("fn{}", i), i * 10, i * 10 + 5],
        ).unwrap();
    }

    for i in 0..10 {
        conn.execute(
            "INSERT INTO symbol_edges (source_id, target_id, edge_type, weight, call_line) VALUES (?1, ?2, 'calls', 1.0, ?3)",
            rusqlite::params![format!("m::fn{}", i), format!("m::fn{}", i + 1), i * 10 + 2],
        ).unwrap();
    }

    let res = query_file_graph_data(&conn, &guard.dir).unwrap();
    assert_eq!(res.total_edges, 0, "All 10 internal calls must be strictly excluded; total_edges must be 0");
    assert!(res.edges.is_empty(), "Edges list must be empty for intra-file calls");

    let mono_node = res.nodes.iter().find(|n| n.path == "src/monolith.rs").unwrap();
    assert_eq!(mono_node.total_symbols, 11);
    assert_eq!(mono_node.in_degree, 0, "Internal calls must not contribute to in_degree");
    assert_eq!(mono_node.out_degree, 0, "Internal calls must not contribute to out_degree");
}

#[test]
fn test_empirical_edge_weights_3_calls_aggregation() {
    let (guard, conn) = create_test_project("edge_weights_3_calls");
    conn.execute_batch(
        "INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES
            ('src/caller.rs', 'h1', 300, 100, 100),
            ('src/callee.rs', 'h2', 300, 100, 100);
         INSERT INTO symbols (id, name, kind, file_path, start_line, end_line) VALUES
            ('c::f1', 'f1', 'function', 'src/caller.rs', 10, 20),
            ('c::f2', 'f2', 'function', 'src/caller.rs', 30, 40),
            ('c::f3', 'f3', 'function', 'src/caller.rs', 50, 60),
            ('t::g1', 'g1', 'function', 'src/callee.rs', 10, 20),
            ('t::g2', 'g2', 'function', 'src/callee.rs', 30, 40),
            ('t::g3', 'g3', 'function', 'src/callee.rs', 50, 60);
         INSERT INTO symbol_edges (source_id, target_id, edge_type, weight, call_line) VALUES
            ('c::f1', 't::g1', 'calls', 1.0, 15),
            ('c::f2', 't::g2', 'calls', 1.0, 35),
            ('c::f3', 't::g3', 'calls', 1.0, 55);",
    ).unwrap();

    let res = query_file_graph_data(&conn, &guard.dir).unwrap();
    assert_eq!(res.total_edges, 1, "Should have exactly 1 file-to-file edge");
    let edge = &res.edges[0];
    assert_eq!(edge.source, "src/caller.rs");
    assert_eq!(edge.target, "src/callee.rs");
    assert_eq!(edge.weight, 3.0, "Weight must be exactly 3.0 for 3 distinct calls of weight 1.0");
    assert_eq!(edge.calls.len(), 3, "Calls list must contain exactly 3 items");

    let funcs: Vec<(&str, &str)> = edge.calls.iter().map(|c| (c.source_func.as_str(), c.target_func.as_str())).collect();
    assert!(funcs.contains(&("f1", "g1")));
    assert!(funcs.contains(&("f2", "g2")));
    assert!(funcs.contains(&("f3", "g3")));
}

#[test]
fn test_empirical_node_metrics_distinct_connected_files() {
    let (guard, conn) = create_test_project("node_metrics_distinct");
    conn.execute_batch(
        "INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES
            ('src/a.rs', 'ha', 500, 100, 100),
            ('src/b.rs', 'hb', 500, 100, 100),
            ('src/c.rs', 'hc', 500, 100, 100),
            ('src/d.rs', 'hd', 500, 100, 100),
            ('src/e.rs', 'he', 500, 100, 100);
         INSERT INTO symbols (id, name, kind, file_path, start_line, end_line) VALUES
            ('a::f1', 'f1', 'function', 'src/a.rs', 10, 20),
            ('a::f2', 'f2', 'function', 'src/a.rs', 30, 40),
            ('b::f1', 'f1', 'function', 'src/b.rs', 10, 20),
            ('b::f2', 'f2', 'function', 'src/b.rs', 30, 40),
            ('c::f1', 'f1', 'function', 'src/c.rs', 10, 20),
            ('d::f1', 'f1', 'function', 'src/d.rs', 10, 20),
            ('d::f2', 'f2', 'function', 'src/d.rs', 30, 40),
            ('e::f1', 'f1', 'function', 'src/e.rs', 10, 20);
         INSERT INTO symbol_edges (source_id, target_id, edge_type, weight, call_line) VALUES
            ('a::f1', 'b::f1', 'calls', 1.0, 15),
            ('a::f2', 'b::f2', 'calls', 1.0, 35),
            ('a::f1', 'c::f1', 'calls', 1.0, 18),
            ('d::f1', 'a::f1', 'calls', 1.0, 12),
            ('d::f2', 'a::f2', 'calls', 1.0, 32),
            ('e::f1', 'a::f1', 'calls', 1.0, 14);",
    ).unwrap();

    let res = query_file_graph_data(&conn, &guard.dir).unwrap();
    let node_a = res.nodes.iter().find(|n| n.path == "src/a.rs").unwrap();
    assert_eq!(node_a.out_degree, 2, "A calls B and C; out_degree must be 2 distinct files");
    assert_eq!(node_a.in_degree, 2, "D and E call A; in_degree must be 2 distinct files");

    let node_b = res.nodes.iter().find(|n| n.path == "src/b.rs").unwrap();
    assert_eq!(node_b.in_degree, 1, "Only file A calls B; in_degree must be 1 distinct file");
    assert_eq!(node_b.out_degree, 0);

    let edge_ab = res.edges.iter().find(|e| e.source == "src/a.rs" && e.target == "src/b.rs").unwrap();
    assert_eq!(edge_ab.weight, 2.0);
    assert_eq!(edge_ab.calls.len(), 2);
}
