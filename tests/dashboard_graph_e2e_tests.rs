mod dashboard_graph_helpers;

use dashboard_graph_helpers::*;

// --- TIER 1: FEATURE COVERAGE ---

#[test]
fn test_e2e_t1_view_symbols_default_fallback() {
    let ctx = TestGraphContext::new("t1_fallback");
    ctx.populate_standard_topology();
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph");
    assert_eq!(status, 200);
    assert_eq!(body["graph_available"], true);
    assert!(body["nodes"].as_array().is_some());
    assert!(body["edges"].as_array().is_some());
}

#[test]
fn test_e2e_t1_view_symbols_explicit() {
    let ctx = TestGraphContext::new("t1_symbols");
    ctx.populate_standard_topology();
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph?view=symbols");
    assert_eq!(status, 200);
    assert_eq!(body["graph_available"], true);
    assert!(body.get("mermaid_dag").is_some());
}

#[test]
fn test_e2e_t1_view_files_happy_path() {
    let ctx = TestGraphContext::new("t1_files");
    ctx.populate_standard_topology();
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph?view=files");
    assert_eq!(status, 200);
    let nodes = body["nodes"].as_array().expect("nodes array required");
    let edges = body["edges"].as_array().expect("edges array required");
    assert!(!nodes.is_empty(), "File nodes array must not be empty in view=files: {:?}", body);

    // All file nodes must have path, total_symbols, in_degree, out_degree
    for node in nodes {
        assert!(node.get("path").or_else(|| node.get("id")).is_some());
        assert!(node.get("total_symbols").is_some(), "total_symbols field required on file node: {:?}", node);
    }
    // Zero self-edges: source != target
    for edge in edges {
        assert_ne!(edge["source"], edge["target"], "Zero self-calls in file graph");
    }
}

#[test]
fn test_e2e_t1_view_file_functions_happy_path() {
    let ctx = TestGraphContext::new("t1_funcs");
    ctx.populate_standard_topology();
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph?view=file_functions&file=src/service.rs");
    assert_eq!(status, 200);
    let nodes = body["nodes"].as_array().expect("nodes array required");
    let edges = body["edges"].as_array().expect("edges array required");

    assert!(!nodes.is_empty(), "Functions for target file must be returned");
    for edge in edges {
        let dir = edge["direction"].as_str().unwrap_or("");
        assert!(["incoming", "outgoing", "internal"].contains(&dir));
    }
}

#[test]
fn test_e2e_t1_parameter_parsing_and_encoding() {
    let ctx = TestGraphContext::new("t1_encoded");
    ctx.populate_standard_topology();
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph?view=file_functions&file=src%2Fservice.rs");
    assert_eq!(status, 200);
    assert!(body["nodes"].as_array().is_some());
}

// --- TIER 2: BOUNDARY & CORNER CASES ---

#[test]
fn test_e2e_t2_missing_project_path() {
    let ctx = TestGraphContext::new("t2_missing_proj");
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph?project=/nonexistent/path_xyz_987");
    assert_eq!(status, 200);
    assert_eq!(body["graph_available"], false);
    assert_eq!(body["reason"], "PROJECT_NOT_FOUND");
}

#[test]
fn test_e2e_t2_missing_db() {
    let empty_dir = std::env::temp_dir().join(format!("empty_ctx_{}", allocate_test_port()));
    let _ = std::fs::create_dir_all(&empty_dir);
    let server = spawn_test_server(&empty_dir);

    let (status, body) = http_get(server.port, "/api/graph");
    assert_eq!(status, 200);
    assert_eq!(body["graph_available"], false);
    assert_eq!(body["reason"], "GRAPH_NOT_INDEXED");
    let _ = std::fs::remove_dir_all(&empty_dir);
}

#[test]
fn test_e2e_t2_empty_database() {
    let ctx = TestGraphContext::new("t2_empty_db");
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph");
    assert_eq!(status, 200);
    assert_eq!(body["graph_available"], true);
    assert_eq!(body["total_nodes"], 0);
    assert_eq!(body["total_edges"], 0);
}

#[test]
fn test_e2e_t2_nonexistent_file_drilldown() {
    let ctx = TestGraphContext::new("t2_nonexistent_file");
    ctx.populate_standard_topology();
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph?view=file_functions&file=src/ghost.rs");
    assert_eq!(status, 200);
    assert_eq!(body["nodes"].as_array().map(|a| a.len()).unwrap_or(0), 0);
}

#[test]
fn test_e2e_t2_file_with_internal_self_calls_only() {
    let ctx = TestGraphContext::new("t2_internal_only");
    ctx.insert_symbol("m::f1", "f1", "function", "src/mod.rs", 1, 10);
    ctx.insert_symbol("m::f2", "f2", "function", "src/mod.rs", 12, 25);
    ctx.insert_edge("m::f1", "m::f2", "calls", 1.0, Some(5));
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph?view=files");
    assert_eq!(status, 200);
    let edges = body["edges"].as_array().expect("edges array");
    assert!(edges.is_empty(), "File with only internal calls must yield 0 file edges");
}

#[test]
fn test_e2e_t2_circular_cross_file_dependencies() {
    let ctx = TestGraphContext::new("t2_circular");
    ctx.insert_symbol("a::fa", "fa", "function", "src/a.rs", 1, 10);
    ctx.insert_symbol("b::fb", "fb", "function", "src/b.rs", 1, 10);
    ctx.insert_edge("a::fa", "b::fb", "calls", 1.0, Some(5));
    ctx.insert_edge("b::fb", "a::fa", "calls", 1.0, Some(5));
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph?view=files");
    assert_eq!(status, 200);
    let edges = body["edges"].as_array().expect("edges array");
    assert_eq!(edges.len(), 2, "Circular dependency produces 2 directed edges");
}

// --- TIER 3: CROSS-FEATURE COMBINATIONS ---

#[test]
fn test_e2e_t3_cross_view_file_node_to_drilldown_consistency() {
    let ctx = TestGraphContext::new("t3_consistency");
    ctx.populate_standard_topology();
    let server = spawn_test_server(&ctx.temp_dir);

    let (_, files_body) = http_get(server.port, "/api/graph?view=files");
    let (_, func_body) = http_get(server.port, "/api/graph?view=file_functions&file=src/service.rs");

    let svc_node = files_body["nodes"]
        .as_array()
        .and_then(|ns| ns.iter().find(|n| n["path"] == "src/service.rs" || n["id"] == "src/service.rs"));
    if let Some(n) = svc_node {
        let internal_funcs = func_body["nodes"]
            .as_array()
            .map(|funcs| funcs.iter().filter(|f| f["is_external"] == false).count())
            .unwrap_or(0);
        assert_eq!(n["total_symbols"], internal_funcs);
    }
}

#[test]
fn test_e2e_t3_isolated_subgraph_verification() {
    let ctx = TestGraphContext::new("t3_isolation");
    ctx.populate_standard_topology();
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph?view=file_functions&file=src/service.rs");
    assert_eq!(status, 200);
    let nodes = body["nodes"].as_array().expect("nodes array");
    for node in nodes {
        let file = node["file_path"].as_str().unwrap_or("");
        assert_ne!(file, "src/util.rs", "Isolated util function must not appear in service subgraph");
    }
}

#[test]
fn test_e2e_t3_mode_transition_query_sequence() {
    let ctx = TestGraphContext::new("t3_transitions");
    ctx.populate_standard_topology();
    let server = spawn_test_server(&ctx.temp_dir);

    // Sequence: symbols -> files -> file_functions -> files
    let (s1, _) = http_get(server.port, "/api/graph?view=symbols");
    assert_eq!(s1, 200);
    let (s2, _) = http_get(server.port, "/api/graph?view=files");
    assert_eq!(s2, 200);
    let (s3, _) = http_get(server.port, "/api/graph?view=file_functions&file=src/service.rs");
    assert_eq!(s3, 200);
    let (s4, _) = http_get(server.port, "/api/graph?view=files");
    assert_eq!(s4, 200);
}

// --- TIER 4: REAL-WORLD APPLICATION SCENARIOS ---

#[test]
fn test_e2e_t4_full_realistic_architecture_scenario() {
    let ctx = TestGraphContext::new("t4_arch");
    ctx.populate_standard_topology();
    let server = spawn_test_server(&ctx.temp_dir);

    let (s_files, body_files) = http_get(server.port, "/api/graph?view=files");
    assert_eq!(s_files, 200);
    let edges = body_files["edges"].as_array().expect("file edges");

    // Check controller -> service edge has aggregated calls and weight = 2.0
    let c_to_s = edges.iter().find(|e| {
        e["source"].as_str().unwrap_or("").contains("controller")
            && e["target"].as_str().unwrap_or("").contains("service")
    });
    if let Some(e) = c_to_s {
        assert_eq!(e["weight"].as_f64().unwrap_or(0.0), 2.0);
        assert_eq!(e["calls"].as_array().map(|c| c.len()).unwrap_or(0), 2);
    }
}

#[test]
fn test_e2e_t4_real_current_codebase_graph_query() {
    let current = std::env::current_dir().unwrap();
    if !current.join(".agent-context").join("code_graph.db").exists() {
        return;
    }
    let server = spawn_test_server(&current);
    let (status, body) = http_get(server.port, "/api/graph?view=symbols");
    assert_eq!(status, 200, "Response status: {}, body: {:?}", status, body);
    assert_eq!(body["graph_available"], true, "Unexpected body: {:?}", body);
}
