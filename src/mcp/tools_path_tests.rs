use std::path::Path;
use serde_json::json;
use crate::dashboard::projects_path::normalize_project_path;
use crate::mcp::state::ServerState;
use super::tools::{detect_project_path, handle_tool_call, validate_path};

#[test]
fn test_validate_path_leading_slashes() {
    let base = Path::new(".");
    let res1 = validate_path(base, "/Cargo.toml");
    assert!(res1.is_ok(), "Leading slash should resolve inside workspace");
    let res2 = validate_path(base, "\\Cargo.toml");
    assert!(res2.is_ok(), "Leading backslash should resolve inside workspace");
    let res3 = validate_path(base, "///Cargo.toml");
    assert!(res3.is_ok(), "Multiple leading slashes should resolve inside workspace");
}

#[test]
fn test_validate_path_cross_platform_slashes() {
    let base = Path::new(".");
    let res1 = validate_path(base, "src\\main.rs");
    assert!(res1.is_ok(), "Windows slash should resolve on all platforms");

    let res2 = validate_path(base, "/src/main.rs");
    assert!(res2.is_ok(), "Leading slash with forward slashes should resolve");

    let res3 = validate_path(base, "\\src\\main.rs");
    assert!(res3.is_ok(), "Leading backslash with Windows slashes should resolve");
}

#[test]
fn test_validate_path_absolute_within_workspace() {
    let base = Path::new(".");
    let abs_cargo = base.canonicalize().unwrap().join("Cargo.toml");
    let res = validate_path(base, abs_cargo.to_str().unwrap());
    assert!(res.is_ok(), "Absolute path inside workspace should be accepted");

    // Non-canonical absolute path (lacking \\?\ on Windows)
    let cwd_cargo = std::env::current_dir().unwrap().join("Cargo.toml");
    let res2 = validate_path(base, cwd_cargo.to_str().unwrap());
    assert!(res2.is_ok(), "Standard absolute path should match when base is relative dot");
}

#[test]
fn test_validate_path_blocks_outside_and_traversal() {
    let base = Path::new(".");
    assert!(validate_path(base, "../../etc/passwd").is_err());
    assert!(validate_path(base, "..\\..\\etc\\passwd").is_err());

    #[cfg(windows)]
    {
        assert!(validate_path(base, "C:\\Windows\\System32\\cmd.exe").is_err());
    }
    #[cfg(not(windows))]
    {
        assert!(validate_path(base, "/etc/passwd").is_err());
    }
}

#[test]
fn test_normalize_project_path_unix_and_windows() {
    // Windows drive path
    let win = normalize_project_path("c:/projects/app///");
    assert!(win.starts_with("C:"), "Windows drive letter should be uppercased");
    assert!(!win.ends_with('/'), "Trailing slashes should be stripped");
    assert!(!win.ends_with('\\'), "Trailing backslashes should be stripped");

    // Unix style path
    #[cfg(not(windows))]
    {
        let unix = normalize_project_path("/home/user/projects/repo///");
        assert!(!unix.contains('\\'), "Unix path must never contain backslashes");
        assert!(unix.starts_with('/'), "Unix path must preserve root slash");
        assert!(!unix.ends_with('/'), "Trailing slash should be stripped");
    }
}

#[test]
fn test_record_modified_file_leading_slashes() {
    let mut state = ServerState::new();
    state.record_modified_file("/src/service.rs");
    state.record_modified_file("\\src\\repo.rs");
    assert_eq!(state.modified_files, vec!["src/service.rs", "src/repo.rs"]);
    assert_eq!(state.dirty_files, vec!["src/service.rs", "src/repo.rs"]);
}

#[test]
fn test_validate_path_traversal() {
    let base = Path::new(".");
    assert!(validate_path(base, "../../../etc/passwd").is_err());
    assert!(validate_path(base, "Cargo.toml").is_ok());

    let mut state = ServerState::new();
    state.plan_approved = true;
    state.set_stage("Build").unwrap();

    let gate_res = handle_tool_call(
        "workflow_gate",
        json!({
            "action": "authorize_edit",
            "project_path": ".",
            "relative_path": "../../../evil.rs",
            "justification": "test traversal"
        }),
        &mut state,
    );
    assert!(gate_res.is_ok());
    let gate_text = gate_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(gate_text.contains("PATH_TRAVERSAL_PROHIBITED"));
}

#[test]
fn test_detect_project_path() {
    let state = ServerState::new();
    // 1. Check explicit path
    let explicit = std::env::current_dir().unwrap();
    let res = detect_project_path(&explicit.to_string_lossy(), &state);
    assert_eq!(res, explicit);

    // 2. Check recorded state.project_path memory
    let mut state_recorded = ServerState::new();
    state_recorded.project_path = Some(explicit.to_string_lossy().to_string());
    let res_recorded = detect_project_path(".", &state_recorded);
    assert_eq!(res_recorded, explicit);

    // 3. Check workspace roots
    let mut state2 = ServerState::new();
    state2.workspace_roots = vec![explicit.to_string_lossy().to_string()];
    let res2 = detect_project_path(".", &state2);
    assert_eq!(res2, explicit);

    // 4. Check that process CWD takes priority over stale global_path_file
    let global_path_file = ServerState::global_project_path_file();
    let parent_dir = explicit.parent().unwrap_or(&explicit);
    let _ = std::fs::write(&global_path_file, parent_dir.to_string_lossy().as_bytes());
    let state_global = ServerState::new();
    let res_global = detect_project_path(".", &state_global);
    assert_eq!(res_global, explicit); // Must return process current_dir (explicit), not global_path_file (parent_dir)
    let _ = std::fs::remove_file(&global_path_file);
}
