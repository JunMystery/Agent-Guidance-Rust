use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSnippet {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub lines: Vec<String>,
}

impl CodeSnippet {
    pub fn format_with_line_numbers(&self) -> String {
        self.lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                if line.starts_with("// ... [") {
                    line.clone()
                } else {
                    format!("L{}: {}", self.start_line + i, line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

pub fn build_safe_path(base_path: &Path, rel_path: &str) -> PathBuf {
    let mut path = base_path.to_path_buf();
    for part in rel_path.split(['/', '\\']) {
        if !part.is_empty() && part != "." && part != ".." {
            path.push(part);
        }
    }
    path
}

const MAX_LINE_CHARS: usize = 300;

fn truncate_line_chars(line: &str) -> String {
    match line.char_indices().nth(MAX_LINE_CHARS - 3) {
        Some((byte_idx, _)) => {
            let mut s = String::with_capacity(byte_idx + 3);
            s.push_str(&line[..byte_idx]);
            s.push_str("...");
            s
        }
        None => line.to_string(),
    }
}

pub fn extract_symbol_body(
    full_path: &Path,
    rel_path: &str,
    start_line: usize,
    end_line: usize,
    max_lines: usize,
) -> Option<CodeSnippet> {
    let content = std::fs::read_to_string(full_path).ok()?;
    let all_lines: Vec<&str> = content.lines().collect();
    let total = all_lines.len();
    if total == 0 || max_lines == 0 || start_line > total {
        return None;
    }

    let start = start_line.max(1).min(total);
    let end = end_line.max(start).min(total);
    let slice_len = (end - start + 1).min(max_lines);
    let effective_end = start + slice_len - 1;

    let mut lines: Vec<String> = all_lines[start - 1..effective_end]
        .iter()
        .map(|s| truncate_line_chars(s))
        .collect();

    if end > effective_end {
        let omitted = end - effective_end;
        lines.push(format!("// ... [{} lines omitted to stay within budget] ...", omitted));
    }

    Some(CodeSnippet {
        file_path: rel_path.to_string(),
        start_line: start,
        end_line: effective_end,
        lines,
    })
}

pub fn extract_call_site(
    full_path: &Path,
    rel_path: &str,
    call_line: usize,
    padding: usize,
) -> Option<CodeSnippet> {
    let content = std::fs::read_to_string(full_path).ok()?;
    let all_lines: Vec<&str> = content.lines().collect();
    let total = all_lines.len();
    if total == 0 || call_line > total {
        return None;
    }

    let cl = if call_line == 0 { 1 } else { call_line };
    let start = cl.saturating_sub(padding).max(1);
    let end = (cl + padding).min(total);

    let lines: Vec<String> = all_lines[start - 1..end]
        .iter()
        .map(|s| truncate_line_chars(s))
        .collect();

    Some(CodeSnippet {
        file_path: rel_path.to_string(),
        start_line: start,
        end_line: end,
        lines,
    })
}
