/// Generates an AST-aware structural skeleton of source code.
/// Preserves declarations (structs, traits, enums, interfaces, classes, type aliases)
/// and function/method signatures while replacing function implementations with line-range placeholders.
pub fn generate_code_skeleton(content: &str, file_path: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    if total_lines <= 15 {
        return content.to_string();
    }

    // 1. Try native Tree-Sitter AST parsing first for supported languages
    let lang = crate::context::ast::AstLanguage::from_path(file_path);
    if lang.is_supported() {
        if let Some(ast_skeleton) = crate::context::ast::AstEngine::generate_skeleton(content, lang, file_path) {
            return ast_skeleton;
        }
    }

    let mut skeleton = Vec::new();
    let mut i = 0;

    skeleton.push(format!("// === Structural Skeleton of '{}' (Total Lines: {}) ===", file_path, total_lines));

    while i < total_lines {
        let line = lines[i];
        let trimmed = line.trim();

        // Pass comments and docstrings
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') || trimmed.starts_with("#[") || trimmed.starts_with('@') {
            skeleton.push(line.to_string());
            i += 1;
            continue;
        }

        // Pass imports, use statements, module declarations, type aliases, and constants
        if trimmed.starts_with("use ")
            || trimmed.starts_with("import ")
            || trimmed.starts_with("export ") && !trimmed.contains("function") && !trimmed.contains('{')
            || trimmed.starts_with("from ")
            || trimmed.starts_with("mod ")
            || trimmed.starts_with("package ")
            || trimmed.starts_with("pub mod ")
            || trimmed.starts_with("type ")
            || trimmed.starts_with("pub type ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("pub const ")
            || trimmed.starts_with("static ")
            || trimmed.starts_with("pub static ")
        {
            skeleton.push(line.to_string());
            i += 1;
            continue;
        }

        // Check for struct / enum / trait / interface / class header
        if trimmed.starts_with("pub struct ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("pub enum ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("pub trait ")
            || trimmed.starts_with("trait ")
            || trimmed.starts_with("interface ")
            || trimmed.starts_with("export interface ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("export class ")
            || trimmed.starts_with("impl")
        {
            skeleton.push(line.to_string());
            i += 1;
            continue;
        }

        // Check for function / method signatures
        let is_fn_start = trimmed.starts_with("pub fn ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("pub async fn ")
            || trimmed.starts_with("async fn ")
            || trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("func ")
            || trimmed.starts_with("fun ")
            || trimmed.starts_with("function ")
            || trimmed.starts_with("export function ")
            || trimmed.starts_with("public ") && trimmed.contains('(')
            || trimmed.starts_with("private ") && trimmed.contains('(')
            || trimmed.starts_with("protected ") && trimmed.contains('(');

        if is_fn_start {
            let start_line = i + 1;
            let mut sig_lines = Vec::new();
            sig_lines.push(line);

            // If signature spans multiple lines up to `{` or `:`
            while !lines[i].contains('{') && !lines[i].trim().ends_with(':') && i + 1 < total_lines {
                i += 1;
                sig_lines.push(lines[i]);
            }

            let open_bracket = lines[i].contains('{');
            let fn_sig = sig_lines.join(" ");

            if open_bracket {
                // Find matching closing bracket for block
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
                let span = (end_line - start_line + 1).max(1);

                let indent = line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                if let Some(prefix) = fn_sig.split('{').next() {
                    skeleton.push(format!("{}{} {{ /* L{}-{}: {} lines */ }}", indent, prefix.trim(), start_line, end_line, span));
                } else {
                    skeleton.push(format!("{} {{ /* L{}-{}: {} lines */ }}", fn_sig.trim(), start_line, end_line, span));
                }
            } else {
                // Python def / single line
                skeleton.push(format!("{} /* L{}: implementation */", fn_sig.trim(), start_line));
            }
            i += 1;
            continue;
        }

        // Retain struct fields / closing braces
        if trimmed == "}" || trimmed == "};" || trimmed.starts_with("pub ") || trimmed.contains(':') && !trimmed.contains('(') {
            skeleton.push(line.to_string());
        }

        i += 1;
    }

    skeleton.join("\n")
}

/// Generates an AST-aware semantic context slice (Zoom Read).
/// Attempts Tree-Sitter parsing first, then falls back to heuristic line-by-line folding.
pub fn generate_zoom_slice(content: &str, file_path: &str, target_symbol: &str) -> Option<crate::context::ast::ZoomSliceResult> {
    let lang = crate::context::ast::AstLanguage::from_path(file_path);
    if lang.is_supported() {
        if let Some(res) = crate::context::ast::AstEngine::generate_zoom_slice(content, lang, file_path, target_symbol) {
            return Some(res);
        }
    }

    generate_zoom_slice_heuristic(content, file_path, target_symbol)
}

fn generate_zoom_slice_heuristic(content: &str, file_path: &str, target_symbol: &str) -> Option<crate::context::ast::ZoomSliceResult> {
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

    Some(crate::context::ast::ZoomSliceResult {
        sliced_content,
        original_lines: total_lines,
        sliced_lines,
        folded_functions_count: folded_count,
        savings_percent,
        target_found: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_code_skeleton_ast_integration() {
        let rs_code = r#"
// Header comment
// Line 2
// Line 3
// Line 4
// Line 5
// Line 6
// Line 7
// Line 8
// Line 9
// Line 10
pub fn add(a: i32, b: i32) -> i32 {
    let sum = a + b;
    println!("sum");
    sum
}

pub struct Item;
"#;
        let out = generate_code_skeleton(rs_code, "math.rs");
        assert!(out.contains("pub fn add(a: i32, b: i32) -> i32 { /* L13-16: 4 lines */ }"));
        assert!(out.contains("pub struct Item;"));
    }
}
