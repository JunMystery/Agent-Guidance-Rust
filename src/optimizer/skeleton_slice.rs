use crate::context::ast::ZoomSliceResult;

/// Generates an AST-aware semantic context slice (Zoom Read).
/// Attempts Tree-Sitter parsing first, then falls back to heuristic line-by-line folding.
pub fn generate_zoom_slice(content: &str, file_path: &str, target_symbol: &str) -> Option<ZoomSliceResult> {
    let lang = crate::context::ast::AstLanguage::from_path(file_path);
    if lang.is_supported() {
        if let Some(res) = crate::context::ast::AstEngine::generate_zoom_slice(content, lang, file_path, target_symbol) {
            return Some(res);
        }
    }

    generate_zoom_slice_heuristic(content, file_path, target_symbol)
}

fn generate_zoom_slice_heuristic(content: &str, file_path: &str, target_symbol: &str) -> Option<ZoomSliceResult> {
    let clean_target = target_symbol.trim().trim_end_matches("()");
    if !content.contains(clean_target) {
        return None;
    }

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    let mut sliced = Vec::new();
    sliced.push(format!("// === Heuristic Context Slice: '{}' (Focus: '{}') ===", file_path, clean_target));

    let mut i = 0;
    let mut folded_count = 0;
    let mut found_target = false;

    while i < total_lines {
        let line = lines[i];
        let trimmed = line.trim();

        let is_fn_start = trimmed.starts_with("pub fn ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("func ")
            || trimmed.starts_with("fun ")
            || trimmed.starts_with("function ")
            || trimmed.starts_with("export function ");

        if is_fn_start {
            let start_line = i + 1;
            let mut sig_lines = vec![line];
            while !lines[i].contains('{') && !lines[i].trim().ends_with(':') && i + 1 < total_lines {
                i += 1;
                sig_lines.push(lines[i]);
            }
            let fn_sig = sig_lines.join(" ");
            let is_focus = fn_sig.contains(clean_target);

            // Find block end
            let mut depth = 0;
            let body_start = i + 1;
            while i < total_lines {
                let cur = lines[i];
                depth += cur.matches('{').count() as i32;
                depth -= cur.matches('}').count() as i32;
                if depth <= 0 && i >= body_start {
                    break;
                }
                i += 1;
            }
            let end_line = i + 1;

            if is_focus {
                found_target = true;
                sliced.push(format!("// >>> [ZOOM FOCUS: {} (L{}-L{})] >>>", clean_target, start_line, end_line));
                for l in start_line..=end_line.min(total_lines) {
                    sliced.push(lines[l - 1].to_string());
                }
                sliced.push(format!("// <<< [END ZOOM FOCUS: {}] <<<", clean_target));
            } else if end_line > start_line + 1 {
                folded_count += 1;
                let span = end_line.saturating_sub(start_line);
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                if let Some(prefix) = fn_sig.split('{').next() {
                    sliced.push(format!("{}{} {{ /* L{}-{}: {} lines folded */ }}", indent, prefix.trim(), start_line, end_line, span));
                } else {
                    sliced.push(format!("{} {{ /* L{}-{}: {} lines folded */ }}", fn_sig.trim(), start_line, end_line, span));
                }
            } else {
                for l in start_line..=end_line.min(total_lines) {
                    sliced.push(lines[l - 1].to_string());
                }
            }
            i += 1;
            continue;
        }

        sliced.push(line.to_string());
        i += 1;
    }

    if !found_target {
        return None;
    }

    let sliced_content = sliced.join("\n");
    let sliced_lines = sliced.len();
    let savings_percent = if total_lines > sliced_lines {
        ((total_lines - sliced_lines) * 100) / total_lines
    } else {
        0
    };

    Some(ZoomSliceResult {
        sliced_content,
        original_lines: total_lines,
        sliced_lines,
        folded_functions_count: folded_count,
        savings_percent,
        target_found: true,
    })
}
