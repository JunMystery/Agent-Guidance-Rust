//! Unit tests for LSP protocol framing, URI conversion, and server detection.

use super::*;
use serde_json::json;
use std::path::PathBuf;

#[test]
fn test_lsp_encode_decode_roundtrip() {
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "textDocument/references",
        "params": { "context": { "includeDeclaration": true } }
    });

    let encoded = encode_message(&payload);
    assert!(encoded.starts_with(b"Content-Length: "));

    let (decoded, consumed) = decode_message(&encoded).expect("Should decode successfully");
    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded["id"], 42);
    assert_eq!(decoded["method"], "textDocument/references");
}

#[test]
fn test_lsp_decode_incomplete_buffer() {
    let payload = json!({ "jsonrpc": "2.0", "id": 1 });
    let encoded = encode_message(&payload);

    // Truncated header
    assert_eq!(decode_message(&encoded[..10]), None);

    // Truncated body
    assert_eq!(decode_message(&encoded[..encoded.len() - 2]), None);
}

#[test]
fn test_lsp_position_and_line_conversion() {
    assert_eq!(line_to_lsp(1), 0);
    assert_eq!(line_to_lsp(50), 49);
    assert_eq!(line_to_lsp(0), 0);

    assert_eq!(line_from_lsp(0), 1);
    assert_eq!(line_from_lsp(49), 50);
}

#[test]
fn test_lsp_path_and_uri_conversion() {
    let temp_file = std::env::temp_dir().join("test_lsp_file.rs");
    let uri = path_to_uri(&temp_file);
    assert!(uri.starts_with("file:///"));

    let base = std::env::temp_dir();
    let rel = uri_to_rel_path(&uri, &base);
    assert_eq!(rel, "test_lsp_file.rs");
}

#[test]
fn test_detect_lsp_server_from_config() {
    let temp_dir = std::env::temp_dir().join("test_lsp_detect_cfg");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(temp_dir.join(".agent-context"));

    let config_json = json!({
        "name": "custom-lsp",
        "command": if cfg!(windows) { "cmd" } else { "sh" },
        "args": ["--custom"]
    });

    std::fs::write(
        temp_dir.join(".agent-context").join("lsp.json"),
        config_json.to_string(),
    ).unwrap();

    let detected = detect_lsp_server(&temp_dir);
    assert!(detected.is_some());
    let cfg = detected.unwrap();
    assert_eq!(cfg.name, "custom-lsp");
    assert_eq!(cfg.args, vec!["--custom".to_string()]);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_graceful_fallback_when_server_missing() {
    let dummy_path = PathBuf::from("/nonexistent_directory_for_lsp_test");
    let result = query_lsp_references(&dummy_path, "src/main.rs", 10, 5);
    assert_eq!(result, None, "Must gracefully return None when LSP is unavailable");

    let def_result = query_lsp_definition(&dummy_path, "src/main.rs", 10, 5);
    assert_eq!(def_result, None, "Must gracefully return None when LSP is unavailable");
}

#[test]
fn test_handle_definition_fallback() {
    let dummy_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let args = serde_json::json!({});
    let resp = crate::mcp::tools::context_lsp::handle_definition(&args, &dummy_path, "LspClient", "");
    assert!(resp.contains("Symbol Definition for 'LspClient'"));
}
