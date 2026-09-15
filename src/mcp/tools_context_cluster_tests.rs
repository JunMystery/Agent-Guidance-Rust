use super::*;

#[test]
fn test_context_cluster_read_multiple_files() {
    let mut state = ServerState::new();

    let res = handle_tool_call(
        "project_context",
        json!({
            "operation": "read",
            "project_path": ".",
            "relative_paths": ["Cargo.toml", "src/main.rs"]
        }),
        &mut state,
    );

    assert!(res.is_ok(), "Clustered read should succeed: {:?}", res);
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();

    assert!(text.contains("# Multi-File Clustered Context (2 file(s)"));
    assert!(text.contains("File: `Cargo.toml`"));
    assert!(text.contains("File: `src/main.rs`"));
    assert!(text.contains("[package]"));
    assert!(text.contains("One-Turn Edit Authorization"));
    assert!(text.contains("workflow_gate(action=\"authorize_edit\""));
    assert!(text.contains("relative_paths=[\"Cargo.toml\",\"src/main.rs\"]") || text.contains("Cargo.toml"));
}

#[test]
fn test_context_cluster_read_alias() {
    let mut state = ServerState::new();

    let res = handle_tool_call(
        "project_context",
        json!({
            "operation": "cluster_read",
            "project_path": ".",
            "relative_paths": ["Cargo.toml"]
        }),
        &mut state,
    );

    assert!(res.is_ok(), "Operation cluster_read alias should work: {:?}", res);
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("# Multi-File Clustered Context (1 file(s)"));
}

#[test]
fn test_context_cluster_read_traversal_protection() {
    let mut state = ServerState::new();

    let res = handle_tool_call(
        "project_context",
        json!({
            "operation": "read",
            "project_path": ".",
            "relative_paths": ["Cargo.toml", "../../../etc/shadow"]
        }),
        &mut state,
    );

    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("PATH_TRAVERSAL_PROHIBITED"));
    assert!(text.contains("File: `Cargo.toml`"));
}

#[test]
fn test_context_cluster_read_large_file_skeleton() {
    let tmp_dir = std::env::temp_dir().join(format!("cluster_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let large_file = tmp_dir.join("large.rs");
    let content = (1..=320)
        .map(|i| format!("fn test_fn_{}() {{ println!(\"{}\"); }}", i, i))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::write(&large_file, &content);

    let mut state = ServerState::new();

    let res = handle_tool_call(
        "project_context",
        json!({
            "operation": "read",
            "project_path": tmp_dir.to_str().unwrap(),
            "relative_paths": ["large.rs"]
        }),
        &mut state,
    );

    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains("AST Structural Skeleton") || text.contains("300 LOC Exceeded"));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
