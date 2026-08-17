use serde_json::Value;
use std::path::Path;

use crate::mcp::state::ServerState;
use crate::optimizer::compressor::compress_markdown;
use super::helpers::validate_path;

pub(crate) fn handle_read(
    arguments: &Value,
    proj_path: &Path,
    rel_path: &str,
    state: &mut ServerState,
) -> String {
                    if rel_path.is_empty() {
                        "Error: relative_path is required for read operation. Example: project_context(operation=\"read\", project_path=\"...\", relative_path=\"src/main.rs\", target_symbol=\"my_fn\")".to_string()
                    } else {
                        let target_symbol = arguments.get("target_symbol").and_then(|s| s.as_str());
                        let view_mode = arguments.get("view_mode").and_then(|v| v.as_str()).unwrap_or("auto");
                        match validate_path(&proj_path, rel_path) {
                            Ok(full_path) => {
                                match std::fs::read_to_string(&full_path) {
                                    Ok(content) => {
                                        let orig_len = content.len() as u64;
                                        let lines: Vec<&str> = content.lines().collect();
                                        let total_lines = lines.len();

                                        // If explicit skeleton requested, or file is large (>300 LOC) and no target symbol requested
                                        if view_mode == "skeleton" || (total_lines > 300 && target_symbol.is_none() && view_mode != "full") {
                                            let skeleton = crate::optimizer::skeleton::generate_code_skeleton(&content, rel_path);
                                            let compressed = compress_markdown(&skeleton);
                                            state.record_call(orig_len / 4, compressed.len() as u64 / 4);

                                            format!(
                                                "# AST Structural Skeleton: `{}` (Total Lines: {})\n\n> ⚡ **Token Saver Mode**: Function bodies collapsed to line ranges.\n\n```\n{}\n```\n\n---\n💡 **Next Step**: Pass `target_symbol=\"<fn_or_struct_name>\"` to `project_context(operation=\"read\", relative_path=\"{}\")` to view complete body implementation.",
                                                rel_path,
                                                total_lines,
                                                compressed,
                                                rel_path
                                            )
                                        } else {
                                            let mut display_lines = lines;
                                            if let Some(symbol) = target_symbol {
                                                // Symbol-targeted extraction (multi-language aware)
                                                let mut matched_snippet = Vec::new();
                                                let mut capturing = false;
                                                let mut brace_count = 0;
                                                let mut has_braces = false;
                                                for line in &display_lines {
                                                    if line.contains(symbol) && !capturing {
                                                        capturing = true;
                                                    }
                                                    if capturing {
                                                        matched_snippet.push(*line);
                                                        let open_b = line.matches('{').count() as i32;
                                                        let close_b = line.matches('}').count() as i32;
                                                        if open_b > 0 {
                                                            has_braces = true;
                                                        }
                                                        brace_count += open_b - close_b;

                                                        if has_braces
                                                            && matched_snippet.len() > 1
                                                            && brace_count <= 0
                                                            && (line.contains('}')
                                                                || line.trim().is_empty())
                                                        {
                                                            break;
                                                        }
                                                        if !has_braces
                                                            && matched_snippet.len() >= 30
                                                            && line.trim().is_empty()
                                                        {
                                                            break;
                                                        }
                                                        if matched_snippet.len() >= 100 {
                                                            break;
                                                        }
                                                    }
                                                }
                                                if !matched_snippet.is_empty() {
                                                    display_lines = matched_snippet;
                                                }
                                            }

                                            let count = display_lines.len();
                                            let was_capped = count > 300;

                                            let bounded = display_lines
                                                .into_iter()
                                                .take(300)
                                                .collect::<Vec<_>>()
                                                .join("\n");
                                            let compressed = compress_markdown(&bounded);
                                            state
                                                .record_call(orig_len / 4, compressed.len() as u64 / 4);

                                            let loc_warning = if was_capped && target_symbol.is_none() {
                                                format!(
                                                    "\n\n---\n⚠️ **ARCHITECTURE MANDATE (300 LOC Cap Exceeded)**: File `{}` has **{} total lines** (capped at 300 lines).\n**MANDATORY ACTION**: Do NOT add new logic directly into this file. Decompose into sub-modules upfront (split entry dispatchers from sub-module handlers).",
                                                    rel_path, count
                                                )
                                            } else if count > 100 && target_symbol.is_none() {
                                                format!(
                                                    "\n\n---\n💡 **Token Optimization Tip**: Pass `target_symbol=\"<fn_or_struct_name>\"` in `project_context(operation=\"read\")` to extract only the target definition and save token budget."
                                                )
                                            } else {
                                                String::new()
                                            };

                                            if let Some(symbol) = target_symbol {
                                                format!(
                                                    "# Target Symbol Extracted: '{}' from {}\n\n{}{}",
                                                    symbol, rel_path, compressed, loc_warning
                                                )
                                            } else {
                                                format!(
                                                    "# Bounded File Content: {}\n\n{}{}",
                                                    rel_path, compressed, loc_warning
                                                )
                                            }
                                        }
                                    }
                                    Err(e) => format!("Failed to read file '{}': {}", rel_path, e),
                                }
                            }
                            Err(err_msg) => format!("Security Error: {}", err_msg),
                        }
                    }
}
