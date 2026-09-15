use serde_json::Value;
use std::path::Path;

use crate::mcp::state::ServerState;
use super::helpers::validate_path;

pub(crate) fn handle_cluster_read(
    arguments: &Value,
    proj_path: &Path,
    paths: &[String],
    state: &mut ServerState,
) -> String {
    if paths.is_empty() {
        return "Error: No target files specified for clustered read.".to_string();
    }

    let mut sections = Vec::new();
    let mut total_lines_read = 0usize;
    let mut total_raw_tokens = 0u64;
    let mut valid_paths = Vec::new();

    let show_line_numbers = arguments
        .get("line_numbers")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let max_files = paths.len().min(8);
    for (idx, rel_path) in paths.iter().take(max_files).enumerate() {
        let clean_path = rel_path.trim().replace('\\', "/");
        if clean_path.is_empty() {
            continue;
        }

        let full_path = match validate_path(proj_path, &clean_path) {
            Ok(p) => p,
            Err(err_msg) => {
                sections.push(format!(
                    "### [{}/{}] File: `{}`\n\n**Security Error: PATH_TRAVERSAL_PROHIBITED**: {}",
                    idx + 1,
                    max_files,
                    clean_path,
                    err_msg
                ));
                continue;
            }
        };

        match std::fs::read_to_string(&full_path) {
            Ok(content) => {
                let file_tokens = crate::optimizer::compressor::estimate_tokens(&content, false) as u64;
                total_raw_tokens += file_tokens;

                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();
                total_lines_read += total_lines;
                valid_paths.push(clean_path.clone());

                let is_exempt = crate::mcp::tools::gate_edit::is_exempt_from_loc_limit(&clean_path);
                let ext = Path::new(&clean_path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                let lang = match ext {
                    "rs" => "rust",
                    "ts" | "tsx" => "typescript",
                    "js" | "jsx" => "javascript",
                    "py" => "python",
                    "json" => "json",
                    "toml" => "toml",
                    "md" => "markdown",
                    _ => "",
                };

                if total_lines > 300 && !is_exempt {
                    let skeleton = crate::optimizer::skeleton::generate_code_skeleton(&content, &clean_path);
                    sections.push(format!(
                        "### [{}/{}] File: `{}` (Lines 1-{} of {})\n\n> **Notice (300 LOC Exceeded)**: Large file collapsed to AST Structural Skeleton to conserve context. Use `target_symbol` for full function body.\n\n```{}\n{}\n```",
                        idx + 1,
                        max_files,
                        clean_path,
                        skeleton.lines().count(),
                        total_lines,
                        lang,
                        skeleton
                    ));
                } else {
                    let rendered = if show_line_numbers {
                        lines
                            .iter()
                            .enumerate()
                            .map(|(i, l)| format!("L{}: {}", i + 1, l))
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else {
                        lines.join("\n")
                    };

                    sections.push(format!(
                        "### [{}/{}] File: `{}` (Lines 1-{} of {})\n\n```{}\n{}\n```",
                        idx + 1,
                        max_files,
                        clean_path,
                        total_lines,
                        total_lines,
                        lang,
                        rendered
                    ));
                }
            }
            Err(e) => {
                sections.push(format!(
                    "### [{}/{}] File: `{}`\n\nFailed to read file: {}",
                    idx + 1,
                    max_files,
                    clean_path,
                    e
                ));
            }
        }
    }

    state.last_raw_baseline_tokens = Some(total_raw_tokens);

    let gate_paths_json = serde_json::to_string(&valid_paths).unwrap_or_else(|_| "[]".to_string());
    let tip = if !valid_paths.is_empty() {
        format!(
            "\n---\n> [!TIP] **One-Turn Edit Authorization**:\n> Authorize edits across this entire cluster in 1 turn before editing:\n> `workflow_gate(action=\"authorize_edit\", project_path=\"{}\", relative_paths={}, risk_level=\"LOW\", justification=\"Implement planned cluster modifications\", architecture_pattern=\"Auto\")`",
            proj_path.display(),
            gate_paths_json
        )
    } else {
        String::new()
    };

    format!(
        "# Multi-File Clustered Context ({} file(s), {} total lines)\n\n{}\n{}",
        sections.len(),
        total_lines_read,
        sections.join("\n\n"),
        tip
    )
}
