use serde_json::Value;
use std::path::Path;

use crate::mcp::state::ServerState;
use super::helpers::validate_path;

pub(crate) fn handle_read(
    arguments: &Value,
    proj_path: &Path,
    rel_path: &str,
    state: &mut ServerState,
) -> String {
    if rel_path.is_empty() {
        return "Error: relative_path is required for read operation. Example: project_context(operation=\"read\", project_path=\"...\", relative_path=\"src/main.rs\", target_symbol=\"my_fn\")".to_string();
    }

    let target_symbol = arguments.get("target_symbol").and_then(|s| s.as_str());
    let view_mode = arguments.get("view_mode").and_then(|v| v.as_str()).unwrap_or("auto");
    let start_line_arg = arguments
        .get("start_line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let end_line_arg = arguments
        .get("end_line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let (full_path, resolved_subpath) = if rel_path.starts_with("linked:") {
        let linked = crate::context::multi_project::discover_linked_projects(proj_path);
        if let Some((proj, subpath)) = crate::context::multi_project::resolve_cross_project_path(rel_path, &linked) {
            match validate_path(&proj.root_path, subpath) {
                Ok(p) => (p, subpath.to_string()),
                Err(e) => return format!("Security Error: {}", e),
            }
        } else {
            return format!(
                "Error: Linked project not found for path '{}'. Ensure the project is registered in .agent-context/linked_projects.json or AGENT_GUIDANCE_LINKED_PROJECTS.",
                rel_path
            );
        }
    } else {
        match validate_path(proj_path, rel_path) {
            Ok(p) => (p, rel_path.to_string()),
            Err(err_msg) => return format!("Security Error: {}", err_msg),
        }
    };

    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            let full_file_tokens = crate::optimizer::compressor::estimate_tokens(&content, false) as u64;
            state.last_raw_baseline_tokens = Some(full_file_tokens);
            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();
            let is_exempt = crate::mcp::tools::gate_edit::is_exempt_from_loc_limit(&resolved_subpath);

            // 1. If explicit skeleton requested, or code file is large (>300 LOC) and no target symbol/range requested
            let has_range = start_line_arg.is_some() || end_line_arg.is_some();
            if (view_mode == "skeleton" || (total_lines > 300 && target_symbol.is_none() && !has_range && view_mode != "full")) && !is_exempt {
                let skeleton = crate::optimizer::skeleton::generate_code_skeleton(&content, &resolved_subpath);
                return format!(
                    "# AST Structural Skeleton: `{}` (Total Lines: {})\n\n> **Token Saver Mode**: Function bodies collapsed to line ranges.\n\n```\n{}\n```\n\n---\n**Next Step**: Pass `target_symbol=\"<fn_or_struct_name>\"` or `start_line` / `end_line` to `project_context(operation=\"read\", relative_path=\"{}\")` to view complete body implementation.",
                    rel_path,
                    total_lines,
                    skeleton,
                    rel_path
                );
            }

            // 2. AST Semantic Context Slicing (Zoom Read)
            if let Some(symbol) = target_symbol.filter(|_| {
                view_mode == "zoom" || view_mode == "slice"
            }) {
                if let Some(slice) = crate::optimizer::skeleton::generate_zoom_slice(&content, &resolved_subpath, symbol) {
                    return format!(
                        "# AST Semantic Context Slice (Zoom Read): `{}` (Focus: `{}`)\n\n> **Zoom Read Optimization**: Preserved 100% type, struct & import context while folding {} sibling function bodies.\n> **Token Savings**: ~{}% reduction (from {} lines down to {} lines).\n\n```\n{}\n```",
                        rel_path,
                        symbol,
                        slice.folded_functions_count,
                        slice.savings_percent,
                        slice.original_lines,
                        slice.sliced_lines,
                        slice.sliced_content
                    );
                }
            }

            // 3. Fallback to isolated symbol snippet or bounded file content
            let mut display_lines = lines;
            let mut line_offset = 1;

            if let Some(symbol) = target_symbol {
                let lang = crate::context::ast::AstLanguage::from_path(&resolved_subpath);
                let mut ast_snippet = None;
                if lang.is_supported() {
                    if let Some(ast_sym) = crate::context::ast::AstEngine::find_symbol(&content, lang, symbol) {
                        if ast_sym.start_line > 0
                            && ast_sym.start_line <= total_lines
                            && ast_sym.end_line <= total_lines
                            && ast_sym.end_line >= ast_sym.start_line
                        {
                            line_offset = ast_sym.start_line;
                            ast_snippet = Some(display_lines[ast_sym.start_line - 1..ast_sym.end_line].to_vec());
                        }
                    }
                }

                if let Some(snippet) = ast_snippet {
                    display_lines = snippet;
                } else {
                    // Heuristic fallback scanner
                    let mut matched_snippet = Vec::new();
                    let mut capturing = false;
                    let mut brace_count = 0;
                    let mut has_braces = false;
                    let mut snippet_start = 1;
                    for (idx, line) in display_lines.iter().enumerate() {
                        if line.contains(symbol) && !capturing {
                            capturing = true;
                            snippet_start = idx + 1;
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
                                && (line.contains('}') || line.trim().is_empty())
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
                        line_offset = snippet_start;
                        display_lines = matched_snippet;
                    }
                }
            } else if has_range {
                let start = start_line_arg.unwrap_or(1).max(1);
                let end = end_line_arg.unwrap_or(total_lines).min(total_lines);
                if start <= total_lines && start <= end {
                    line_offset = start;
                    let slice_len = (end - start + 1).min(300);
                    display_lines = display_lines[start - 1..(start - 1 + slice_len).min(total_lines)].to_vec();
                } else {
                    display_lines = Vec::new();
                }
            }

            let count = display_lines.len();
            let was_capped = !has_range && total_lines > 300;

            let show_line_numbers = arguments
                .get("line_numbers")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let bounded = if show_line_numbers {
                display_lines
                    .into_iter()
                    .take(300)
                    .enumerate()
                    .map(|(i, line)| {
                        let line_no = line_offset + i;
                        format!("L{}: {}", line_no, line)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                display_lines
                    .into_iter()
                    .take(300)
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            let slice_end = if count == 0 { 0 } else { (line_offset + count.min(300)).saturating_sub(1) };
            let slice_start = if count == 0 { 0 } else { line_offset };

            let loc_warning = if was_capped && target_symbol.is_none() && !is_exempt {
                format!(
                    "\n\n---\n**ARCHITECTURE MANDATE (300 LOC Cap Exceeded)**: File `{}` has **{} total lines** (capped at 300 lines).\n**MANDATORY ACTION**: Do NOT add new logic directly into this file. Decompose into sub-modules upfront (split entry dispatchers from sub-module handlers).",
                    rel_path, total_lines
                )
            } else {
                String::new()
            };

            let is_indent_sensitive = resolved_subpath.ends_with(".py")
                || resolved_subpath.ends_with(".yaml")
                || resolved_subpath.ends_with(".yml")
                || resolved_subpath.ends_with("Makefile")
                || resolved_subpath.ends_with(".mk")
                || resolved_subpath.ends_with(".nim");

            let indent_note = if is_indent_sensitive {
                "> **Language Note**: Indent-sensitive syntax (Python/YAML/Makefile). Whitespace and indentation preserved 100%.\n\n"
            } else {
                ""
            };

            if let Some(symbol) = target_symbol {
                format!(
                    "# Target Symbol Extracted: '{}' from {} (Lines {}-{} of {})\n\n{}{}{}",
                    symbol, rel_path, slice_start, slice_end, total_lines, indent_note, bounded, loc_warning
                )
            } else {
                format!(
                    "# Bounded File Content: {} (Lines {}-{} of {})\n\n{}{}{}",
                    rel_path, slice_start, slice_end, total_lines, indent_note, bounded, loc_warning
                )
            }
        }
        Err(e) => format!("Failed to read file '{}': {}", rel_path, e),
    }
}
