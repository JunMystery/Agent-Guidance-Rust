use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use serde_json::json;

use crate::context::db::CodeGraphDb;
use crate::context::graph_rag::build_subgraph_bundle;
use crate::mcp::tools::context_bundle::handle_subgraph_bundle;

static BUNDLE_TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestProjectGuard {
    dir: std::path::PathBuf,
}

impl Drop for TestProjectGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn create_mock_project() -> (TestProjectGuard, CodeGraphDb) {
    let id = BUNDLE_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_dir = std::env::temp_dir().join(format!("bundle_test_{}_{}", std::process::id(), id));
    let _ = fs::remove_dir_all(&temp_dir);
    let ctx_dir = temp_dir.join(".agent-context");
    fs::create_dir_all(&ctx_dir).unwrap();

    let src_dir = temp_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let controller_code = "use crate::service;\n\npub fn handle_req() {\n    let res = service::process_data();\n    println!(\"{:?}\", res);\n}\n";
    fs::write(src_dir.join("controller.rs"), controller_code).unwrap();

    let service_code = "use crate::repo;\n\npub fn process_data() -> bool {\n    let valid = true;\n    if valid {\n        repo::save_record();\n    }\n    valid\n}\n";
    fs::write(src_dir.join("service.rs"), service_code).unwrap();

    let repo_code = "pub fn save_record() {\n    println!(\"saving\");\n}\n";
    fs::write(src_dir.join("repo.rs"), repo_code).unwrap();

    let db = CodeGraphDb::open_for_project(&temp_dir).unwrap();
    db.conn.execute_batch(
        "INSERT INTO files (path, content_hash, size, modified_at, indexed_at)
        VALUES ('src/controller.rs', 'hash1', 100, 1, 1),
               ('src/service.rs', 'hash2', 100, 1, 1),
               ('src/repo.rs', 'hash3', 100, 1, 1);
        INSERT INTO symbols (id, name, kind, file_path, start_line, end_line, signature)
        VALUES ('fn_ctrl', 'handle_req', 'function', 'src/controller.rs', 3, 6, 'pub fn handle_req()'),
               ('fn_srv', 'process_data', 'function', 'src/service.rs', 3, 9, 'pub fn process_data() -> bool'),
               ('fn_repo', 'save_record', 'function', 'src/repo.rs', 1, 3, 'pub fn save_record()');
        INSERT INTO symbol_edges (source_id, target_id, edge_type, weight, call_line)
        VALUES ('fn_ctrl', 'fn_srv', 'calls', 1.0, 4),
               ('fn_srv', 'fn_repo', 'calls', 1.0, 6);"
    ).unwrap();

    (TestProjectGuard { dir: temp_dir }, db)
}

#[test]
fn test_subgraph_bundle_target_not_found() {
    let (guard, _db) = create_mock_project();
    let res = build_subgraph_bundle(&guard.dir, "non_existent_symbol", None, 250).unwrap();
    assert!(res.is_none());

    let mcp_res = handle_subgraph_bundle(&json!({}), &guard.dir, "non_existent_symbol");
    assert!(mcp_res.contains("No symbol named `non_existent_symbol` found"));
}

#[test]
fn test_subgraph_bundle_full_flow() {
    let (guard, _db) = create_mock_project();
    let bundle = build_subgraph_bundle(&guard.dir, "process_data", None, 250).unwrap().expect("bundle should exist");

    assert_eq!(bundle.target.name, "process_data");
    assert_eq!(bundle.target.file_path, "src/service.rs");
    assert!(bundle.target_snippet.is_some());

    assert_eq!(bundle.callers.len(), 1);
    assert_eq!(bundle.callers[0].symbol_name, "handle_req");
    assert_eq!(bundle.callers[0].call_line, 4);
    assert!(bundle.callers[0].snippet.is_some());

    assert_eq!(bundle.callees.len(), 1);
    assert_eq!(bundle.callees[0].symbol_name, "save_record");
    assert!(bundle.callees[0].snippet.is_some());

    assert_eq!(bundle.total_callers_count, 1);
    assert_eq!(bundle.total_callees_count, 1);
    assert!(bundle.total_loc <= 250);

    let mcp_res = handle_subgraph_bundle(&json!({"loc_budget": 200}), &guard.dir, "process_data");
    assert!(mcp_res.contains("Multi-File Subgraph Bundle: `process_data`"));
    assert!(mcp_res.contains("Target Implementation"));
    assert!(mcp_res.contains("Immediate Callers"));
    assert!(mcp_res.contains("handle_req"));
    assert!(mcp_res.contains("Immediate Dependencies"));
    assert!(mcp_res.contains("save_record"));
}

#[test]
fn test_subgraph_bundle_respects_budget() {
    let (guard, _db) = create_mock_project();
    let bundle = build_subgraph_bundle(&guard.dir, "process_data", None, 50).unwrap().expect("bundle should exist");
    assert!(bundle.total_loc <= 50);
}

#[test]
fn test_subgraph_bundle_path_scope_and_zero_lines() {
    let (guard, db) = create_mock_project();

    // Zero lines guard test (no panic)
    let full_path = guard.dir.join("src/service.rs");
    let snip = crate::context::graph_rag::bundle_snippet_extractor::extract_symbol_body(&full_path, "src/service.rs", 3, 9, 0);
    assert!(snip.is_none());

    // Stale line test (start_line past EOF returns None)
    let stale_snip = crate::context::graph_rag::bundle_snippet_extractor::extract_symbol_body(&full_path, "src/service.rs", 999, 1005, 10);
    assert!(stale_snip.is_none());

    let stale_call = crate::context::graph_rag::bundle_snippet_extractor::extract_call_site(&full_path, "src/service.rs", 999, 3);
    assert!(stale_call.is_none());

    // Insert non-call edge (contains)
    db.conn.execute(
        "INSERT INTO symbol_edges (source_id, target_id, edge_type, weight) VALUES ('fn_ctrl', 'fn_srv', 'contains', 1.0)",
        [],
    ).unwrap();

    // Verify contains edge is ignored in callers
    let bundle = build_subgraph_bundle(&guard.dir, "src/service.rs#process_data", None, 250).unwrap().expect("bundle exists");
    assert_eq!(bundle.callers.len(), 1);
    assert_eq!(bundle.callers[0].symbol_name, "handle_req");
    assert_eq!(bundle.total_callers_count, 1);
}

#[test]
fn test_cross_platform_path_slashes() {
    let (guard, db) = create_mock_project();

    // Simulate Windows backslash stored in DB
    db.conn.execute(
        "INSERT INTO files (path, content_hash, size, modified_at, indexed_at)
         VALUES ('src\\win_service.rs', 'hash_win', 100, 1, 1)",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO symbols (id, name, kind, file_path, start_line, end_line, signature)
         VALUES ('fn_win', 'run_win_task', 'function', 'src\\win_service.rs', 1, 3, 'pub fn run_win_task()')",
        [],
    ).unwrap();

    let win_file = guard.dir.join("src").join("win_service.rs");
    fs::write(&win_file, "pub fn run_win_task() {\n    println!(\"win\");\n}\n").unwrap();

    // Query with POSIX slash should match Windows path in DB
    let bundle_posix = build_subgraph_bundle(&guard.dir, "run_win_task", Some("src/win_service.rs"), 250)
        .unwrap()
        .expect("should match via POSIX slash");
    assert_eq!(bundle_posix.target.name, "run_win_task");

    // Query with Windows slash should also match
    let bundle_win = build_subgraph_bundle(&guard.dir, "run_win_task", Some("src\\win_service.rs"), 250)
        .unwrap()
        .expect("should match via Windows slash");
    assert_eq!(bundle_win.target.name, "run_win_task");
}

#[test]
fn test_subgraph_bundle_excludes_self_recursion() {
    let (guard, db) = create_mock_project();

    // Insert recursive self-call
    db.conn.execute(
        "INSERT INTO symbol_edges (source_id, target_id, edge_type, weight, call_line)
         VALUES ('fn_srv', 'fn_srv', 'calls', 1.0, 7)",
        [],
    ).unwrap();

    let bundle = build_subgraph_bundle(&guard.dir, "process_data", None, 250).unwrap().expect("bundle exists");
    // Neither callees nor callers should contain fn_srv (itself)
    assert!(bundle.callees.iter().all(|c| c.symbol_name != "process_data"));
    assert!(bundle.callers.iter().all(|c| c.symbol_name != "process_data"));
}

#[test]
fn test_build_safe_path_blocks_traversal() {
    let base = std::path::Path::new("/workspace/project");
    let safe = crate::context::graph_rag::bundle_snippet_extractor::build_safe_path(base, "../../etc/passwd");
    assert!(!safe.to_string_lossy().contains(".."));
    assert!(safe.ends_with("etc/passwd") || safe.ends_with("etc\\passwd"));
}

#[test]
fn test_omitted_line_format_no_line_number() {
    let (guard, _db) = create_mock_project();
    let full_path = guard.dir.join("src/service.rs");
    // Max 3 lines out of 8 lines -> 5 lines omitted
    let snip = crate::context::graph_rag::bundle_snippet_extractor::extract_symbol_body(&full_path, "src/service.rs", 3, 10, 3).unwrap();
    let formatted = snip.format_with_line_numbers();
    assert!(formatted.contains("// ... ["));
    // Ensure the omission banner does NOT have "L{no}: // ... ["
    for line in formatted.lines() {
        if line.contains("// ... [") {
            assert!(!line.starts_with("L"));
        }
    }
}

#[test]
fn test_context_bundle_symbol_alias_argument() {
    let (guard, _db) = create_mock_project();
    let res = handle_subgraph_bundle(&json!({"symbol": "process_data"}), &guard.dir, "process_data");
    assert!(res.contains("Multi-File Subgraph Bundle: `process_data`"));
}

#[test]
fn test_line_truncation_on_long_lines() {
    let (guard, _db) = create_mock_project();
    let file = guard.dir.join("src").join("long_line.rs");
    let long_line = format!("pub fn long_fn() {{ let x = \"{}\"; }}", "a".repeat(400));
    fs::write(&file, &long_line).unwrap();

    let snip = crate::context::graph_rag::bundle_snippet_extractor::extract_symbol_body(&file, "src/long_line.rs", 1, 1, 10).unwrap();
    assert_eq!(snip.lines.len(), 1);
    assert!(snip.lines[0].len() <= 300);
    assert!(snip.lines[0].ends_with("..."));
}

#[test]
fn test_leading_slash_and_empty_target_guard() {
    let (guard, _db) = create_mock_project();

    // Leading slash should match
    let bundle = build_subgraph_bundle(&guard.dir, "process_data", Some("/src/service.rs"), 250)
        .unwrap()
        .expect("should match leading slash");
    assert_eq!(bundle.target.name, "process_data");

    // Malformed '#' query should return None cleanly
    let empty_res = build_subgraph_bundle(&guard.dir, "#", None, 250).unwrap();
    assert!(empty_res.is_none());

    // String loc_budget coercion
    let res = handle_subgraph_bundle(&json!({"loc_budget": "120"}), &guard.dir, "process_data");
    assert!(res.contains("Packed LOC"));
}

#[test]
fn test_exact_path_priority_over_longer_like_match() {
    let (guard, db) = create_mock_project();

    // Insert shorter symbol in exact path src/service.rs and longer symbol in domain/service.rs
    db.conn.execute(
        "INSERT INTO files (path, content_hash, size, modified_at, indexed_at)
         VALUES ('src/domain/service.rs', 'hash_dom', 200, 1, 1)",
        [],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO symbols (id, name, kind, file_path, start_line, end_line, signature)
         VALUES ('fn_dom', 'process_data', 'function', 'src/domain/service.rs', 1, 50, 'pub fn process_data()')",
        [],
    ).unwrap();

    let bundle = build_subgraph_bundle(&guard.dir, "process_data", Some("src/service.rs"), 250)
        .unwrap()
        .expect("should find symbol");
    assert_eq!(bundle.target.file_path, "src/service.rs");
}
