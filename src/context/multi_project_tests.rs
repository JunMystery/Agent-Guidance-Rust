//! Unit tests for multi-project cross-referencing and virtual monorepo cascade.

use super::*;
use std::path::PathBuf;
use crate::context::db::CodeGraphDb;

#[test]
fn test_resolve_cross_project_path() {
    let proj_a = LinkedProject {
        name: "core-lib".to_string(),
        root_path: PathBuf::from("/fake/core-lib"),
    };
    let linked = vec![proj_a];

    let res = resolve_cross_project_path("linked:core-lib/src/utils.rs", &linked);
    assert!(res.is_some());
    let (p, sub) = res.unwrap();
    assert_eq!(p.name, "core-lib");
    assert_eq!(sub, "src/utils.rs");

    let res_no_prefix = resolve_cross_project_path("core-lib/src/utils.rs", &linked);
    assert!(res_no_prefix.is_some());

    let res_unknown = resolve_cross_project_path("linked:unknown/src/lib.rs", &linked);
    assert!(res_unknown.is_none());
}

#[test]
fn test_discover_linked_projects_from_config() {
    let temp_dir = std::env::temp_dir().join("test_multi_proj_disc");
    let primary = temp_dir.join("primary");
    let secondary = temp_dir.join("secondary");

    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(primary.join(".agent-context"));
    let _ = std::fs::create_dir_all(&secondary);

    let config_json = serde_json::json!([
        { "name": "second", "path": "../secondary" }
    ]);
    std::fs::write(
        primary.join(".agent-context").join("linked_projects.json"),
        config_json.to_string(),
    ).unwrap();

    let discovered = discover_linked_projects(&primary);
    let second = discovered.iter().find(|p| p.name == "second").expect("second project discovered");
    assert_eq!(
        second.root_path,
        secondary.canonicalize().unwrap()
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cascade_symbol_search_across_projects() {
    let temp_dir = std::env::temp_dir().join("test_multi_proj_search");
    let secondary = temp_dir.join("secondary");

    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(secondary.join(".agent-context"));

    let db = CodeGraphDb::open(&secondary.join(".agent-context").join("code_graph.db")).unwrap();
    db.upsert_file("src/api.rs", "dummy_hash", 100, 1000).unwrap();
    db.insert_symbol(
        "src/api.rs::function::fetch_remote_data::L25",
        "fetch_remote_data",
        "function",
        "src/api.rs",
        None,
        25,
        40,
        Some("pub fn fetch_remote_data()"),
    ).unwrap();

    let linked = vec![LinkedProject {
        name: "secondary-service".to_string(),
        root_path: secondary.canonicalize().unwrap(),
    }];

    let syms = search_linked_symbols(&linked, "fetch_remote", 5);
    assert_eq!(syms.len(), 1);
    assert_eq!(syms[0].0, "secondary-service");
    assert_eq!(syms[0].1, "src/api.rs");
    assert_eq!(syms[0].2, "fetch_remote_data");
    assert_eq!(syms[0].3, 25);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cascade_content_search_across_projects() {
    let temp_dir = std::env::temp_dir().join("test_multi_proj_content");
    let secondary = temp_dir.join("secondary");

    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(secondary.join(".agent-context"));

    let db = CodeGraphDb::open(&secondary.join(".agent-context").join("code_graph.db")).unwrap();
    db.upsert_file("src/handler.rs", "dummy_hash2", 100, 1000).unwrap();
    db.insert_chunk(
        "src/handler.rs",
        1,
        15,
        "dummy_hash2",
        "pub async fn handle_webhook_request(payload: String) -> Result<()>",
    ).unwrap();

    let linked = vec![LinkedProject {
        name: "webhook-service".to_string(),
        root_path: secondary.canonicalize().unwrap(),
    }];

    let hits = search_linked_content(&linked, "handle_webhook_request", 5);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].0, "webhook-service");
    assert_eq!(hits[0].1, "src/handler.rs");
    assert_eq!(hits[0].2, 1);
    assert!(hits[0].4.contains("handle_webhook_request"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_discover_linked_projects_from_env() {
    let temp_dir = std::env::temp_dir().join("test_multi_proj_env");
    let primary = temp_dir.join("primary");
    let external = temp_dir.join("external_lib");

    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&primary);
    let _ = std::fs::create_dir_all(&external);

    unsafe {
        std::env::set_var("AGENT_GUIDANCE_LINKED_PROJECTS", external.to_str().unwrap());
    }

    let discovered = discover_linked_projects(&primary);
    assert!(discovered.iter().any(|p| p.root_path == external.canonicalize().unwrap()));

    unsafe {
        std::env::remove_var("AGENT_GUIDANCE_LINKED_PROJECTS");
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}
