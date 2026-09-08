//! JSON-RPC 2.0 framing and protocol structures for Language Server Protocol (LSP).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// 0-indexed position in a text document per LSP specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

/// A range in a text document expressed as start and end positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

/// Represents a location inside a resource, such as a line range inside a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspLocation {
    pub uri: String,
    pub range: LspRange,
}

/// Encodes a JSON-RPC payload with LSP `Content-Length` header.
pub fn encode_message(payload: &Value) -> Vec<u8> {
    let body = payload.to_string();
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
}

/// Attempts to decode one framed JSON-RPC message from a byte buffer.
/// Returns `Some((Value, bytes_consumed))` if a complete message is present.
pub fn decode_message(buf: &[u8]) -> Option<(Value, usize)> {
    let header_sep = b"\r\n\r\n";
    let sep_pos = buf.windows(4).position(|w| w == header_sep)?;
    let header_bytes = &buf[..sep_pos];
    let header_str = std::str::from_utf8(header_bytes).ok()?;

    let mut content_length = None;
    for line in header_str.lines() {
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            if let Ok(len) = rest.trim().parse::<usize>() {
                content_length = Some(len);
                break;
            }
        }
    }

    let len = content_length?;
    let body_start = sep_pos + 4;
    let body_end = body_start + len;

    if buf.len() < body_end {
        return None; // Incomplete body
    }

    let body_bytes = &buf[body_start..body_end];
    let val = serde_json::from_slice::<Value>(body_bytes).ok()?;
    Some((val, body_end))
}

/// Converts a standard file path to an LSP file URI (`file:///...`).
pub fn path_to_uri(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy().replace('\\', "/");
    let clean_path = path_str.trim_start_matches('/');
    if cfg!(windows) {
        format!("file:///{}", clean_path)
    } else {
        format!("file:///{}", path_str.trim_start_matches('/'))
    }
}

/// Converts an LSP file URI back to a workspace relative path.
pub fn uri_to_rel_path(uri: &str, proj_path: &Path) -> String {
    let raw = uri.strip_prefix("file:///").unwrap_or(uri);
    let decoded = raw.replace("%20", " ");
    let target_path = Path::new(&decoded);

    if let Ok(rel) = target_path.strip_prefix(proj_path) {
        return rel.to_string_lossy().replace('\\', "/");
    }

    if let Ok(canon_proj) = proj_path.canonicalize() {
        if let Ok(rel) = target_path.strip_prefix(&canon_proj) {
            return rel.to_string_lossy().replace('\\', "/");
        }
    }

    decoded
}

/// Converts a 1-based editor line number to a 0-based LSP line number.
pub fn line_to_lsp(line: usize) -> u32 {
    line.saturating_sub(1) as u32
}

/// Converts a 0-based LSP line number to a 1-based editor line number.
pub fn line_from_lsp(lsp_line: u32) -> usize {
    lsp_line as usize + 1
}
