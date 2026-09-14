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
    let res = build_subgraph_bundle(&guard.dir, "non_existent_symbol", 250).unwrap();
    assert!(res.is_none());

    let mcp_res = handle_subgraph_bundle(&json!({}), &guard.dir, "non_existent_symbol");
    assert!(mcp_res.contains("No symbol named `non_existent_symbol` found"));
}

#[test]
fn test_subgraph_bundle_full_flow() {
    let (guard, _db) = create_mock_project();
    let bundle = build_subgraph_bundle(&guard.dir, "process_data", 250).unwrap().expect("bundle should exist");

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
    let bundle = build_subgraph_bundle(&guard.dir, "process_data", 50).unwrap().expect("bundle should exist");
    assert!(bundle.total_loc <= 50);
}
