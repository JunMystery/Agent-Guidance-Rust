//! Tier 5 Adversarial Hardening and Stress Tests for GraphRAG Dashboard API

#[path = "dashboard_graph_helpers/mod.rs"]
mod helpers;
use helpers::{http_get, spawn_test_server, TestGraphContext};

#[test]
fn test_tier5_adversarial_massive_internal_calls_zero_file_edges() {
    let ctx = TestGraphContext::new("tier5_massive");
    // Insert 50 functions into a single file and 100 internal calls
    for i in 0..50 {
        ctx.insert_symbol(
            &format!("m::fn_{}", i),
            &format!("fn_{}", i),
            "function",
            "src/massive.rs",
            i * 10 + 1,
            i * 10 + 9,
        );
    }
    for i in 0..49 {
        ctx.insert_edge(
            &format!("m::fn_{}", i),
            &format!("m::fn_{}", i + 1),
            "calls",
            1.0,
            Some(i * 10 + 5),
        );
        ctx.insert_edge(
            &format!("m::fn_{}", i + 1),
            &format!("m::fn_{}", i),
            "calls",
            1.0,
            Some(i * 10 + 7),
        );
    }

    let server = spawn_test_server(&ctx.temp_dir);
    let (code, body) = http_get(server.port, "/api/graph?view=files");
    assert_eq!(code, 200);
    assert_eq!(body["total_nodes"], 1);
    assert_eq!(body["total_edges"], 0, "All 98 internal calls must be strictly excluded from file graph");

    let node = &body["nodes"][0];
    assert_eq!(node["path"], "src/massive.rs");
    assert_eq!(node["total_symbols"], 50);
    assert_eq!(node["in_degree"], 0);
    assert_eq!(node["out_degree"], 0);
}

#[test]
fn test_tier5_adversarial_special_chars_and_spaces_in_paths() {
    let ctx = TestGraphContext::new("tier5_special");
    let file1 = "src/nested dir/special module.rs";
    let file2 = "src/unicode_tên.rs";

    ctx.insert_symbol("s1::run", "run", "function", file1, 1, 30);
    ctx.insert_symbol("s2::helper", "helper", "function", file2, 1, 20);
    ctx.insert_edge("s1::run", "s2::helper", "calls", 2.5, Some(15));

    let server = spawn_test_server(&ctx.temp_dir);
    let (code, body) = http_get(server.port, "/api/graph?view=files");
    assert_eq!(code, 200);
    assert_eq!(body["total_nodes"], 2);
    assert_eq!(body["total_edges"], 1);

    // Drilldown on file with URL-encoded spaces and slashes
    let query = "/api/graph?view=file_functions&file=src%2Fnested%20dir%2Fspecial%20module.rs";
    let (code_drill, body_drill) = http_get(server.port, query);
    assert_eq!(code_drill, 200);
    assert_eq!(body_drill["file"], file1);
    assert_eq!(body_drill["total_nodes"], 2);
    assert_eq!(body_drill["total_edges"], 1);

    let edge = &body_drill["edges"][0];
    assert_eq!(edge["direction"], "outgoing");
    assert_eq!(edge["weight"], 2.5);
}

#[test]
fn test_tier5_adversarial_dense_cross_file_calls_aggregation() {
    let ctx = TestGraphContext::new("tier5_dense");
    let file_a = "src/alpha.rs";
    let file_b = "src/beta.rs";

    // 5 functions in A, 5 in B
    for i in 0..5 {
        ctx.insert_symbol(&format!("a::f_{}", i), &format!("af_{}", i), "function", file_a, i * 10, i * 10 + 5);
        ctx.insert_symbol(&format!("b::f_{}", i), &format!("bf_{}", i), "function", file_b, i * 10, i * 10 + 5);
    }

    // 10 calls A -> B
    for i in 0..5 {
        ctx.insert_edge(&format!("a::f_{}", i), &format!("b::f_{}", i), "calls", 1.0, Some(i * 10 + 2));
        ctx.insert_edge(&format!("a::f_{}", i), &format!("b::f_{}", (i + 1) % 5), "calls", 1.0, Some(i * 10 + 3));
    }
    // 5 calls B -> A
    for i in 0..5 {
        ctx.insert_edge(&format!("b::f_{}", i), &format!("a::f_{}", i), "calls", 1.5, Some(i * 10 + 4));
    }

    let server = spawn_test_server(&ctx.temp_dir);
    let (code, body) = http_get(server.port, "/api/graph?view=files");
    assert_eq!(code, 200);
    assert_eq!(body["total_nodes"], 2);
    assert_eq!(body["total_edges"], 2, "Must aggregate into exactly 2 bidirectional edges");

    let edges = body["edges"].as_array().unwrap();
    let edge_a_to_b = edges.iter().find(|e| e["source"] == file_a && e["target"] == file_b).unwrap();
    assert_eq!(edge_a_to_b["weight"], 10.0);
    assert_eq!(edge_a_to_b["calls"].as_array().unwrap().len(), 10);

    let edge_b_to_a = edges.iter().find(|e| e["source"] == file_b && e["target"] == file_a).unwrap();
    assert_eq!(edge_b_to_a["weight"], 7.5); // 5 * 1.5
    assert_eq!(edge_b_to_a["calls"].as_array().unwrap().len(), 5);

    let nodes = body["nodes"].as_array().unwrap();
    for n in nodes {
        assert_eq!(n["in_degree"], 1);
        assert_eq!(n["out_degree"], 1);
    }
}

#[test]
fn test_tier5_adversarial_drilldown_isolation_of_unrelated_files() {
    let ctx = TestGraphContext::new("tier5_isolation");
    ctx.insert_symbol("target::core", "core", "function", "src/target.rs", 10, 50);
    ctx.insert_symbol("dep::util", "util", "function", "src/dep.rs", 1, 20);
    ctx.insert_symbol("caller::main", "main", "function", "src/caller.rs", 1, 30);
    ctx.insert_symbol("isolated::orphan", "orphan", "function", "src/isolated.rs", 1, 10);
    ctx.insert_symbol("other::x", "x", "function", "src/other.rs", 1, 15);
    ctx.insert_symbol("other::y", "y", "function", "src/other.rs", 20, 35);

    // Target calls dep (outgoing)
    ctx.insert_edge("target::core", "dep::util", "calls", 1.0, Some(25));
    // Caller calls target (incoming)
    ctx.insert_edge("caller::main", "target::core", "calls", 1.0, Some(12));
    // Unrelated call between other::x and other::y
    ctx.insert_edge("other::x", "other::y", "calls", 1.0, Some(10));

    let server = spawn_test_server(&ctx.temp_dir);
    let (code, body) = http_get(server.port, "/api/graph?view=file_functions&file=src/target.rs");
    assert_eq!(code, 200);
    assert_eq!(body["total_nodes"], 3); // target::core, dep::util, caller::main
    assert_eq!(body["total_edges"], 2);

    let nodes = body["nodes"].as_array().unwrap();
    let ids: Vec<&str> = nodes.iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"target::core"));
    assert!(ids.contains(&"dep::util"));
    assert!(ids.contains(&"caller::main"));
    assert!(!ids.contains(&"isolated::orphan"), "Orphans must not be in drilldown");
    assert!(!ids.contains(&"other::x"), "Unrelated functions must be excluded");
    assert!(!ids.contains(&"other::y"), "Unrelated functions must be excluded");

    let edges = body["edges"].as_array().unwrap();
    let out_edge = edges.iter().find(|e| e["direction"] == "outgoing").unwrap();
    assert_eq!(out_edge["source"], "target::core");
    assert_eq!(out_edge["target"], "dep::util");

    let in_edge = edges.iter().find(|e| e["direction"] == "incoming").unwrap();
    assert_eq!(in_edge["source"], "caller::main");
    assert_eq!(in_edge["target"], "target::core");
}

#[test]
fn test_tier5_adversarial_missing_and_empty_file_parameter() {
    let ctx = TestGraphContext::new("tier5_empty_file");
    ctx.insert_symbol("a::f", "f", "function", "src/a.rs", 1, 10);
    let server = spawn_test_server(&ctx.temp_dir);

    // Missing file parameter
    let (code1, body1) = http_get(server.port, "/api/graph?view=file_functions");
    assert_eq!(code1, 400);
    assert!(body1["error"].as_str().unwrap().contains("Missing required 'file'"));

    // Empty file parameter: file=
    let (code2, body2) = http_get(server.port, "/api/graph?view=file_functions&file=");
    assert_eq!(code2, 400);
    assert!(body2["error"].as_str().unwrap().contains("Missing required 'file'"));
}
