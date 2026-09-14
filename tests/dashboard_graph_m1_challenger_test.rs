mod dashboard_graph_helpers;
use dashboard_graph_helpers::*;

#[test]
fn test_challenger_internal_external_directionality() {
    let ctx = TestGraphContext::new("ch_dir");
    ctx.insert_symbol("core::alpha", "alpha", "function", "src/core.rs", 10, 20);
    ctx.insert_symbol("core::beta", "beta", "function", "src/core.rs", 25, 40);
    ctx.insert_symbol("core::isolated", "isolated", "function", "src/core.rs", 50, 60);

    ctx.insert_symbol("caller::run", "run", "function", "src/caller.rs", 1, 15);
    ctx.insert_symbol("callee::save", "save", "function", "src/callee.rs", 1, 30);

    ctx.insert_edge("core::alpha", "core::beta", "calls", 1.0, Some(15));
    ctx.insert_edge("core::beta", "callee::save", "calls", 1.0, Some(35));
    ctx.insert_edge("caller::run", "core::alpha", "calls", 1.0, Some(5));

    let server = spawn_test_server(&ctx.temp_dir);
    let (status, body) = http_get(server.port, "/api/graph?view=file_functions&file=src/core.rs");
    assert_eq!(status, 200);

    let nodes = body["nodes"].as_array().expect("nodes array");
    let edges = body["edges"].as_array().expect("edges array");

    let internal_nodes: Vec<_> = nodes.iter().filter(|n| n["is_external"] == false).collect();
    assert_eq!(internal_nodes.len(), 3);
    for n in &internal_nodes {
        assert_eq!(n["file_path"], "src/core.rs");
        assert_eq!(n["scope"], "internal");
    }

    let external_nodes: Vec<_> = nodes.iter().filter(|n| n["is_external"] == true).collect();
    assert_eq!(external_nodes.len(), 2);
    for n in &external_nodes {
        assert_ne!(n["file_path"], "src/core.rs");
        assert_eq!(n["scope"], "external");
    }

    let outgoing = edges.iter().find(|e| e["direction"] == "outgoing").expect("outgoing edge");
    assert_eq!(outgoing["source"], "core::beta");
    assert_eq!(outgoing["target"], "callee::save");

    let incoming = edges.iter().find(|e| e["direction"] == "incoming").expect("incoming edge");
    assert_eq!(incoming["source"], "caller::run");
    assert_eq!(incoming["target"], "core::alpha");

    let internal_e = edges.iter().find(|e| e["direction"] == "internal").expect("internal edge");
    assert_eq!(internal_e["source"], "core::alpha");
    assert_eq!(internal_e["target"], "core::beta");
}

#[test]
fn test_challenger_bidirectional_cycle_no_duplicate_nodes() {
    let ctx = TestGraphContext::new("ch_cycle");
    ctx.insert_symbol("a::ping", "ping", "function", "src/a.rs", 10, 20);
    ctx.insert_symbol("b::pong", "pong", "function", "src/b.rs", 10, 20);

    ctx.insert_edge("a::ping", "b::pong", "calls", 1.0, Some(15));
    ctx.insert_edge("b::pong", "a::ping", "calls", 1.0, Some(15));

    let server = spawn_test_server(&ctx.temp_dir);
    let (status, body) = http_get(server.port, "/api/graph?view=file_functions&file=src/a.rs");
    assert_eq!(status, 200);

    let nodes = body["nodes"].as_array().expect("nodes array");
    let edges = body["edges"].as_array().expect("edges array");

    assert_eq!(nodes.len(), 2);
    let pong_nodes: Vec<_> = nodes.iter().filter(|n| n["id"] == "b::pong").collect();
    assert_eq!(pong_nodes.len(), 1, "External node b::pong must NOT be duplicated");
    assert_eq!(pong_nodes[0]["is_external"], true);

    assert_eq!(edges.len(), 2);
    assert!(edges.iter().any(|e| e["direction"] == "outgoing" && e["source"] == "a::ping" && e["target"] == "b::pong"));
    assert!(edges.iter().any(|e| e["direction"] == "incoming" && e["source"] == "b::pong" && e["target"] == "a::ping"));
}

#[test]
fn test_challenger_target_file_empty_and_nonexistent() {
    let ctx = TestGraphContext::new("ch_empty");
    ctx.insert_symbol("other::foo", "foo", "function", "src/other.rs", 1, 10);
    let server = spawn_test_server(&ctx.temp_dir);

    let (status, body) = http_get(server.port, "/api/graph?view=file_functions&file=src/not_found.rs");
    assert_eq!(status, 200);
    assert_eq!(body["graph_available"], true);
    assert_eq!(body["total_nodes"], 0);
    assert_eq!(body["total_edges"], 0);
    assert_eq!(body["nodes"].as_array().unwrap().len(), 0);
    assert_eq!(body["edges"].as_array().unwrap().len(), 0);

    let (status_empty, body_empty) = http_get(server.port, "/api/graph?view=file_functions&file=");
    assert_eq!(status_empty, 400);
    assert!(body_empty["error"].as_str().unwrap().contains("Missing required 'file'"));

    let (status_missing, body_missing) = http_get(server.port, "/api/graph?view=file_functions");
    assert_eq!(status_missing, 400);
    assert!(body_missing["error"].as_str().unwrap().contains("Missing required 'file'"));
}

#[test]
fn test_challenger_intra_file_self_recursion() {
    let ctx = TestGraphContext::new("ch_recurse");
    ctx.insert_symbol("recurse::fact", "fact", "function", "src/recurse.rs", 1, 20);
    ctx.insert_edge("recurse::fact", "recurse::fact", "calls", 1.0, Some(10));

    let server = spawn_test_server(&ctx.temp_dir);
    let (status, body) = http_get(server.port, "/api/graph?view=file_functions&file=src/recurse.rs");
    assert_eq!(status, 200);

    let nodes = body["nodes"].as_array().expect("nodes");
    let edges = body["edges"].as_array().expect("edges");

    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["is_external"], false);
    assert_eq!(nodes[0]["deg"], 2);

    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["direction"], "internal");
    assert_eq!(edges[0]["source"], "recurse::fact");
    assert_eq!(edges[0]["target"], "recurse::fact");
}

#[test]
fn test_challenger_2hop_isolation_and_deduplication() {
    let ctx = TestGraphContext::new("ch_isolation");
    ctx.insert_symbol("a::f1", "f1", "function", "src/a.rs", 1, 10);
    ctx.insert_symbol("a::f2", "f2", "function", "src/a.rs", 12, 25);
    ctx.insert_symbol("b::shared", "shared", "function", "src/b.rs", 1, 20);
    ctx.insert_symbol("c::distant", "distant", "function", "src/c.rs", 1, 20);

    // Both a::f1 and a::f2 call b::shared
    ctx.insert_edge("a::f1", "b::shared", "calls", 1.0, None);
    ctx.insert_edge("a::f2", "b::shared", "calls", 1.0, Some(15));
    // b::shared calls c::distant (2 hops from a.rs)
    ctx.insert_edge("b::shared", "c::distant", "calls", 1.0, Some(5));

    let server = spawn_test_server(&ctx.temp_dir);
    let (status, body) = http_get(server.port, "/api/graph?view=file_functions&file=src/a.rs");
    assert_eq!(status, 200);

    let nodes = body["nodes"].as_array().expect("nodes");
    let edges = body["edges"].as_array().expect("edges");

    // Must contain a::f1, a::f2, and b::shared. c::distant must NOT appear!
    assert_eq!(nodes.len(), 3);
    assert!(nodes.iter().any(|n| n["id"] == "a::f1" && n["is_external"] == false));
    assert!(nodes.iter().any(|n| n["id"] == "a::f2" && n["is_external"] == false));
    assert!(nodes.iter().any(|n| n["id"] == "b::shared" && n["is_external"] == true));
    assert!(!nodes.iter().any(|n| n["id"] == "c::distant"));

    // 2 edges from a.rs to b::shared, 0 edges to c::distant
    assert_eq!(edges.len(), 2);
    for e in edges {
        assert_eq!(e["direction"], "outgoing");
        assert_eq!(e["target"], "b::shared");
    }
}

#[test]
fn test_challenger_e2e_10_internal_calls_zero_self_edges_and_weight_aggregation() {
    let ctx = TestGraphContext::new("ch_e2e_file_graph");
    // Monolith file with 11 functions and 10 internal calls
    for i in 0..=10 {
        ctx.insert_symbol(
            &format!("m::fn{}", i),
            &format!("fn{}", i),
            "function",
            "src/monolith.rs",
            i * 10,
            i * 10 + 5,
        );
    }
    for i in 0..10 {
        ctx.insert_edge(
            &format!("m::fn{}", i),
            &format!("m::fn{}", i + 1),
            "calls",
            1.0,
            Some(i * 10 + 2),
        );
    }

    // Client -> Server: 3 distinct function calls
    for i in 1..=3 {
        ctx.insert_symbol(
            &format!("cli::c{}", i),
            &format!("c{}", i),
            "function",
            "src/client.rs",
            i * 10,
            i * 10 + 5,
        );
        ctx.insert_symbol(
            &format!("srv::s{}", i),
            &format!("s{}", i),
            "function",
            "src/server.rs",
            i * 10,
            i * 10 + 5,
        );
        ctx.insert_edge(
            &format!("cli::c{}", i),
            &format!("srv::s{}", i),
            "calls",
            1.0,
            Some(i * 10 + 2),
        );
    }

    // Other client -> Server: 1 call
    ctx.insert_symbol("ocli::req", "req", "function", "src/other_client.rs", 1, 10);
    ctx.insert_edge("ocli::req", "srv::s1", "calls", 1.0, Some(5));

    // Server -> DB: 1 call
    ctx.insert_symbol("db::query", "query", "function", "src/db.rs", 1, 10);
    ctx.insert_edge("srv::s1", "db::query", "calls", 1.0, Some(5));

    let server = spawn_test_server(&ctx.temp_dir);
    let (status, body) = http_get(server.port, "/api/graph?view=files");
    assert_eq!(status, 200);

    let nodes = body["nodes"].as_array().expect("nodes");
    let edges = body["edges"].as_array().expect("edges");

    // 1. Strict self-call exclusion across entire graph
    for edge in edges {
        assert_ne!(edge["source"], edge["target"], "Zero self-edges allowed in file graph");
    }
    let mono_self = edges.iter().find(|e| e["source"] == "src/monolith.rs" || e["target"] == "src/monolith.rs");
    assert!(mono_self.is_none(), "Monolith file with only internal calls must produce 0 edges");

    // 2. Edge weight aggregation: client -> server has exactly weight 3.0 and 3 calls
    let cli_to_srv = edges.iter().find(|e| e["source"] == "src/client.rs" && e["target"] == "src/server.rs")
        .expect("client to server edge must exist");
    assert_eq!(cli_to_srv["weight"].as_f64().unwrap(), 3.0, "Weight must be 3.0 for 3 calls");
    let calls = cli_to_srv["calls"].as_array().expect("calls array");
    assert_eq!(calls.len(), 3, "Calls list must contain 3 items");

    // 3. Node degree metrics: server has in_degree 2 (client, other_client) and out_degree 1 (db)
    let srv_node = nodes.iter().find(|n| n["path"] == "src/server.rs").expect("server node");
    assert_eq!(srv_node["in_degree"].as_u64().unwrap(), 2, "Server in_degree must be 2 distinct caller files");
    assert_eq!(srv_node["out_degree"].as_u64().unwrap(), 1, "Server out_degree must be 1 distinct callee file");

    let mono_node = nodes.iter().find(|n| n["path"] == "src/monolith.rs").expect("monolith node");
    assert_eq!(mono_node["in_degree"].as_u64().unwrap(), 0);
    assert_eq!(mono_node["out_degree"].as_u64().unwrap(), 0);
}
