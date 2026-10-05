use super::*;

#[test]
fn test_project_context_tree_max_depth() {
    let temp_dir = std::env::temp_dir().join(format!("ag_tree_test_{}", std::process::id()));
    let level1 = temp_dir.join("dir1");
    let level2 = level1.join("dir2");
    let level3 = level2.join("dir3");
    let _ = std::fs::create_dir_all(&level3);

    std::fs::write(temp_dir.join("root.rs"), "fn root() {}").unwrap();
    std::fs::write(level1.join("l1.rs"), "fn l1() {}").unwrap();
    std::fs::write(level2.join("l2.rs"), "fn l2() {}").unwrap();
    std::fs::write(level3.join("l3.rs"), "fn l3() {}").unwrap();

    let mut state = ServerState::new();

    // 1. Default max_depth (3)
    let res_default = handle_tool_call(
        "project_context",
        json!({
            "operation": "tree",
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );
    assert!(res_default.is_ok());
    let text_default = res_default.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text_default.contains("# Project Tree (Depth Capped at 3)"));
    assert!(text_default.contains("root.rs"));
    assert!(text_default.contains("l1.rs"));
    assert!(text_default.contains("l2.rs"));

    // 2. Explicit max_depth = 1
    let res_d1 = handle_tool_call(
        "project_context",
        json!({
            "operation": "tree",
            "project_path": temp_dir.to_str().unwrap(),
            "max_depth": 1
        }),
        &mut state,
    );
    assert!(res_d1.is_ok());
    let text_d1 = res_d1.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text_d1.contains("# Project Tree (Depth Capped at 1)"));
    assert!(text_d1.contains("root.rs"));
    assert!(!text_d1.contains("l2.rs"));

    // 3. Clamping: max_depth = 10 clamps to 5, max_depth = 0 clamps to 1
    let res_clamp_high = handle_tool_call(
        "project_context",
        json!({
            "operation": "tree",
            "project_path": temp_dir.to_str().unwrap(),
            "max_depth": 10
        }),
        &mut state,
    );
    assert!(res_clamp_high.is_ok());
    let text_clamp_high = res_clamp_high.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text_clamp_high.contains("# Project Tree (Depth Capped at 5)"));

    let res_clamp_low = handle_tool_call(
        "project_context",
        json!({
            "operation": "tree",
            "project_path": temp_dir.to_str().unwrap(),
            "max_depth": 0
        }),
        &mut state,
    );
    assert!(res_clamp_low.is_ok());
    let text_clamp_low = res_clamp_low.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text_clamp_low.contains("# Project Tree (Depth Capped at 1)"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}
