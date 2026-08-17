/// Generates an AST-aware structural skeleton of source code.
/// Preserves declarations (structs, traits, enums, interfaces, classes, type aliases)
/// and function/method signatures while replacing function implementations with line-range placeholders.
pub fn generate_code_skeleton(content: &str, file_path: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    if total_lines <= 15 {
        return content.to_string();
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
