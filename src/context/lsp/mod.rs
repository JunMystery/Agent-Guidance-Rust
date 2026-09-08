//! Language Server Protocol (LSP) Bridge for compiler-accurate symbol intelligence.

pub mod client;
pub mod detect;
pub mod protocol;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub use client::LspClient;
pub use detect::{detect_lsp_server, is_command_available, LspServerConfig};
pub use protocol::{
    decode_message, encode_message, line_from_lsp, line_to_lsp, path_to_uri, uri_to_rel_path,
    LspLocation, LspPosition, LspRange,
};

use std::path::Path;

/// Dispatches a `textDocument/references` query via the auto-detected LSP server.
/// Returns `Some((server_name, locations))` on success, or `None` if LSP is unavailable or errored.
pub fn query_lsp_references(
    proj_path: &Path,
    rel_path: &str,
    line: usize,
    col: usize,
) -> Option<(String, Vec<LspLocation>)> {
    let config = detect_lsp_server(proj_path)?;
    let mut client = LspClient::start(&config, proj_path).ok()?;
    let server_name = client.server_name.clone();

    let full_path = proj_path.join(rel_path);
    let uri = path_to_uri(&full_path);
    let lsp_line = line_to_lsp(line);
    let lsp_col = col.saturating_sub(1) as u32;

    let locs = client.find_references(&uri, lsp_line, lsp_col).ok()?;
    Some((server_name, locs))
}

/// Dispatches a `textDocument/definition` query via the auto-detected LSP server.
/// Returns `Some((server_name, locations))` on success, or `None` if LSP is unavailable or errored.
pub fn query_lsp_definition(
    proj_path: &Path,
    rel_path: &str,
    line: usize,
    col: usize,
) -> Option<(String, Vec<LspLocation>)> {
    let config = detect_lsp_server(proj_path)?;
    let mut client = LspClient::start(&config, proj_path).ok()?;
    let server_name = client.server_name.clone();

    let full_path = proj_path.join(rel_path);
    let uri = path_to_uri(&full_path);
    let lsp_line = line_to_lsp(line);
    let lsp_col = col.saturating_sub(1) as u32;

    let locs = client.goto_definition(&uri, lsp_line, lsp_col).ok()?;
    Some((server_name, locs))
}

/// Dispatches a `textDocument/typeDefinition` query via the auto-detected LSP server.
/// Returns `Some((server_name, locations))` on success, or `None` if LSP is unavailable or errored.
pub fn query_lsp_type_definition(
    proj_path: &Path,
    rel_path: &str,
    line: usize,
    col: usize,
) -> Option<(String, Vec<LspLocation>)> {
    let config = detect_lsp_server(proj_path)?;
    let mut client = LspClient::start(&config, proj_path).ok()?;
    let server_name = client.server_name.clone();

    let full_path = proj_path.join(rel_path);
    let uri = path_to_uri(&full_path);
    let lsp_line = line_to_lsp(line);
    let lsp_col = col.saturating_sub(1) as u32;

    let locs = client.goto_type_definition(&uri, lsp_line, lsp_col).ok()?;
    Some((server_name, locs))
}
