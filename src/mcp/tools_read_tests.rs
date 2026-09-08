use super::*;
use serde_json::json;
use crate::mcp::state::ServerState;

#[test]
fn test_context_read_start_and_end_line_range() {
    let temp_dir = std::env::temp_dir().join(format!("read_range_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let sample_file = temp_dir.join("sample.rs");

    let mut lines = Vec::new();
    for i in 1..=50 {
        lines.push(format!("fn line_test_{}() {{ let _val = {}; }}", i, i));
    }
    let _ = std::fs::write(&sample_file, lines.join("\n"));

    let mut state = ServerState::new();
    let res = handle_tool_call(
        "project_context",
        json!({
            "operation": "read",
            "relative_path": "sample.rs",
            "start_line": 10,
            "end_line": 15,
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );

    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();

    // Verify header metadata contains line range
    assert!(text.contains("(Lines 10-15 of 50)"), "Missing range header: {}", text);

    // Verify default output is raw lines (no L10: prefix to save tokens)
    assert!(text.contains("fn line_test_10()"), "Missing line_test_10: {}", text);
    assert!(!text.contains("L10: "), "Should not contain L10: by default: {}", text);
    assert!(!text.contains("L15: "), "Should not contain L15: by default: {}", text);

    // Verify line_numbers: true option adds line prefixes
    let res_with_ln = handle_tool_call(
        "project_context",
        json!({
            "operation": "read",
            "relative_path": "sample.rs",
            "start_line": 10,
            "end_line": 15,
            "line_numbers": true,
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );
    assert!(res_with_ln.is_ok());
    let text_ln = res_with_ln.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text_ln.contains("L10: fn line_test_10()"), "Missing L10 prefix with line_numbers=true: {}", text_ln);
    assert!(text_ln.contains("L15: fn line_test_15()"), "Missing L15 prefix with line_numbers=true: {}", text_ln);

    // Verify it did not include lines outside range
    assert!(!text_ln.contains("L9: "), "Should not contain L9: {}", text_ln);
    assert!(!text_ln.contains("L16: "), "Should not contain L16: {}", text_ln);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_context_read_python_indent_preservation_and_tag() {
    let temp_dir = std::env::temp_dir().join(format!("read_py_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let py_file = temp_dir.join("service.py");

    let py_code = r#"class UserService:
    def __init__(self):
        self.users = []

    def get_user(self, user_id):
        if not user_id:
            return None
        return next((u for u in self.users if u['id'] == user_id), None)
"#;
    let _ = std::fs::write(&py_file, py_code);

    let mut state = ServerState::new();
    let res = handle_tool_call(
        "project_context",
        json!({
            "operation": "read",
            "relative_path": "service.py",
            "project_path": temp_dir.to_str().unwrap()
        }),
        &mut state,
    );

    assert!(res.is_ok());
    let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();

    // Verify language note present
    assert!(text.contains("Language Note"), "Missing indent-sensitive note: {}", text);
    assert!(text.contains("Indent-sensitive syntax (Python/YAML/Makefile)"), "Missing py note: {}", text);

    // Verify indentation preserved cleanly without line numbers
    assert!(text.contains("class UserService:"), "Missing UserService: {}", text);
    assert!(text.contains("    def __init__(self):"), "Indentation lost at def __init__: {}", text);
    assert!(text.contains("            return None"), "Indentation lost at return None: {}", text);
    assert!(!text.contains("L1: "), "Should not contain L1 prefix: {}", text);

    let _ = std::fs::remove_dir_all(&temp_dir);
}
