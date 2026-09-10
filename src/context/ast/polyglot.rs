//! Pure Rust Declarative Polyglot Lexer Registry.
//! Extracts symbols and call expressions for Tier 2 languages without C++ Tree-Sitter bloat.

use regex::Regex;
use std::sync::LazyLock;
use super::types::{AstCall, AstLanguage, AstSymbol};

static PKG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(?:package|namespace)\s+([A-Za-z0-9_.]+)"#).unwrap()
});

static TYPE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(?:(?:pub|public|protected|private|internal|abstract|sealed|open|data|final|static|const)\s+)*(?:enum\s+class|class|interface|struct|enum|record|protocol|trait|union|defmodule)\s+([A-Za-z0-9_]+)|^\s*(?:pub\s+)?const\s+([A-Za-z0-9_]+)\s*=\s*(?:packed\s+)?(?:struct|enum|union)"#).unwrap()
});

static FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(?:(?:pub|public|protected|private|internal|abstract|final|static|open|override|suspend|async|inline|mutating)\s+)*(?:fun|def|defp|fn|func|function|sub)\s+(?:([A-Za-z0-9_]+)\.)?([A-Za-z0-9_]+)(?:\s*\(|[ \t\r\n]|$)"#).unwrap()
});

static C_FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(?:(?:static|inline|virtual|extern|const|constexpr|pub|public|protected|private)\s+)*(?:[A-Za-z0-9_<>*&:]+\s+)+([A-Za-z0-9_]+)\s*\([^;{]*\)\s*(?:const|throws\s+[A-Za-z0-9_,\s]+|[A-Za-z0-9_!]+)?\s*[\{;]"#).unwrap()
});

static SHELL_FN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(?:function\s+([A-Za-z0-9_]+)|([A-Za-z0-9_]+)\s*\(\s*\)\s*\{)"#).unwrap()
});

static CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:([A-Za-z0-9_]+)(?:\.|->|::))?([A-Za-z0-9_]+)\s*\("#).unwrap()
});

static SHELL_SOURCE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(?:\.|source)\s+([^\s;]+)"#).unwrap()
});

/// Extracts symbols from source code for Tier 2 polyglot languages.
pub fn extract_symbols(content: &str, lang: AstLanguage) -> Vec<AstSymbol> {
    let mut symbols = Vec::new();
    let package = PKG_RE.captures(content).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
    let lines: Vec<&str> = content.lines().collect();
    let is_end_delimited = matches!(lang, AstLanguage::Ruby | AstLanguage::Elixir | AstLanguage::Lua);

    // 1. Extract types (classes, interfaces, structs, enums, Zig types, Elixir modules)
    for cap in TYPE_RE.captures_iter(content) {
        let (m, full_cap) = if let Some(m) = cap.get(1) {
            (m, cap.get(0).map_or("", |c| c.as_str()))
        } else if let Some(m) = cap.get(2) {
            (m, cap.get(0).map_or("", |c| c.as_str()))
        } else {
            continue;
        };
        let name = m.as_str().to_string();
        let byte_idx = m.start();
        let start_line = content[..byte_idx].lines().count().max(1);
        let end_line = estimate_block_end(&lines, start_line, is_end_delimited);
        let sig = lines.get(start_line - 1).unwrap_or(&"").trim().to_string();
        let kind = if full_cap.contains("interface") || full_cap.contains("protocol") {
            "interface"
        } else if full_cap.contains("enum") {
            "enum"
        } else if full_cap.contains("struct") {
            "struct"
        } else if full_cap.contains("defmodule") {
            "module"
        } else {
            "class"
        };

        symbols.push(AstSymbol {
            name: name.clone(),
            kind: kind.to_string(),
            start_line,
            end_line,
            start_byte: byte_idx,
            end_byte: byte_idx + name.len(),
            signature: sig,
            body_start_line: Some(start_line),
            body_end_line: Some(end_line),
            parent_symbol: None,
            package: package.clone(),
            language: lang.as_str().to_string(),
        });
    }

    // 2. Extract functions / methods
    let is_shell = matches!(lang, AstLanguage::Shell);
    let is_c_family = matches!(lang, AstLanguage::C | AstLanguage::Cpp | AstLanguage::CSharp | AstLanguage::Java | AstLanguage::Zig);

    if is_shell {
        for cap in SHELL_FN_RE.captures_iter(content) {
            let m = cap.get(1).or_else(|| cap.get(2));
            if let Some(m) = m {
                let name = m.as_str().to_string();
                let byte_idx = m.start();
                let start_line = content[..byte_idx].lines().count().max(1);
                let end_line = estimate_block_end(&lines, start_line, false);
                let sig = lines.get(start_line - 1).unwrap_or(&"").trim().to_string();

                symbols.push(AstSymbol {
                    name,
                    kind: "function".to_string(),
                    start_line,
                    end_line,
                    start_byte: byte_idx,
                    end_byte: byte_idx + m.len(),
                    signature: sig,
                    body_start_line: Some(start_line),
                    body_end_line: Some(end_line),
                    parent_symbol: None,
                    package: None,
                    language: lang.as_str().to_string(),
                });
            }
        }
    } else {
        // Keyword-based functions (fun, def, defp, fn, func, function)
        for cap in FN_RE.captures_iter(content) {
            let receiver = cap.get(1).map(|m| m.as_str().to_string());
            if let Some(m) = cap.get(2) {
                let name = m.as_str().to_string();
                let byte_idx = m.start();
                let start_line = content[..byte_idx].lines().count().max(1);
                let end_line = estimate_block_end(&lines, start_line, is_end_delimited);
                let sig = lines.get(start_line - 1).unwrap_or(&"").trim().to_string();

                symbols.push(AstSymbol {
                    name,
                    kind: "function".to_string(),
                    start_line,
                    end_line,
                    start_byte: byte_idx,
                    end_byte: byte_idx + m.len(),
                    signature: sig,
                    body_start_line: Some(start_line),
                    body_end_line: Some(end_line),
                    parent_symbol: receiver,
                    package: package.clone(),
                    language: lang.as_str().to_string(),
                });
            }
        }

        // C / Java / C# / Zig return-type functions if FN_RE didn't match
        if is_c_family {
            for cap in C_FN_RE.captures_iter(content) {
                if let Some(m) = cap.get(1) {
                    let name = m.as_str().to_string();
                    if name == "if" || name == "for" || name == "while" || name == "switch" || name == "catch" {
                        continue;
                    }
                    let byte_idx = m.start();
                    let start_line = content[..byte_idx].lines().count().max(1);
                    if symbols.iter().any(|s| s.start_line == start_line) {
                        continue;
                    }
                    let end_line = estimate_block_end(&lines, start_line, false);
                    let sig = lines.get(start_line - 1).unwrap_or(&"").trim().to_string();

                    symbols.push(AstSymbol {
                        name,
                        kind: "function".to_string(),
                        start_line,
                        end_line,
                        start_byte: byte_idx,
                        end_byte: byte_idx + m.len(),
                        signature: sig,
                        body_start_line: Some(start_line),
                        body_end_line: Some(end_line),
                        parent_symbol: None,
                        package: package.clone(),
                        language: lang.as_str().to_string(),
                    });
                }
            }
        }
    }

    // Attach enclosing class/struct as parent_symbol if not already set
    let type_spans: Vec<(String, usize, usize)> = symbols
        .iter()
        .filter(|s| s.kind == "class" || s.kind == "interface" || s.kind == "struct" || s.kind == "module")
        .map(|s| (s.name.clone(), s.start_line, s.end_line))
        .collect();

    for sym in &mut symbols {
        if sym.kind == "function" && sym.parent_symbol.is_none() {
            if let Some((parent_name, _, _)) = type_spans
                .iter()
                .find(|(_, start, end)| sym.start_line > *start && sym.end_line <= *end)
            {
                sym.parent_symbol = Some(parent_name.clone());
            }
        }
    }

    symbols
}

/// Extracts calls from source code for Tier 2 polyglot languages.
pub fn extract_calls(content: &str, lang: AstLanguage) -> Vec<AstCall> {
    let mut calls = Vec::new();
    let symbols = extract_symbols(content, lang);

    if matches!(lang, AstLanguage::Shell) {
        for (idx, line) in content.lines().enumerate() {
            if let Some(cap) = SHELL_SOURCE_RE.captures(line) {
                if let Some(target) = cap.get(1) {
                    calls.push(AstCall {
                        caller_name: None,
                        callee_name: target.as_str().to_string(),
                        line: idx + 1,
                        receiver: Some("source".to_string()),
                        namespace: None,
                    });
                }
            }
        }
    }

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let caller = symbols
            .iter()
            .find(|s| s.kind == "function" && line_num >= s.start_line && line_num <= s.end_line)
            .map(|s| s.name.clone());

        for cap in CALL_RE.captures_iter(line) {
            let receiver = cap.get(1).map(|m| m.as_str().to_string());
            if let Some(callee) = cap.get(2) {
                let name = callee.as_str().to_string();
                if name == "if" || name == "for" || name == "while" || name == "switch" || name == "catch" {
                    continue;
                }
                calls.push(AstCall {
                    caller_name: caller.clone(),
                    callee_name: name,
                    line: line_num,
                    receiver,
                    namespace: None,
                });
            }
        }
    }

    calls
}

fn estimate_block_end(lines: &[&str], start_line: usize, is_end_delimited: bool) -> usize {
    if is_end_delimited {
        let mut depth = 0;
        let mut started = false;
        for (i, line) in lines.iter().enumerate().skip(start_line.saturating_sub(1)) {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") || trimmed.starts_with("defp ") || trimmed.starts_with("class ") || trimmed.starts_with("module ") || trimmed.ends_with(" do") || trimmed.starts_with("do ") || trimmed.starts_with("if ") || trimmed.starts_with("function ") {
                started = true;
                depth += 1;
            }
            if trimmed == "end" || trimmed.starts_with("end ") || trimmed.ends_with(" end") {
                depth -= 1;
                if started && depth <= 0 {
                    return i + 1;
                }
            }
        }
        return (start_line + 15).min(lines.len());
    }

    let mut depth = 0;
    let mut started = false;
    for (i, line) in lines.iter().enumerate().skip(start_line.saturating_sub(1)) {
        let open = line.chars().filter(|c| *c == '{').count();
        let close = line.chars().filter(|c| *c == '}').count();
        if open > 0 { started = true; }
        depth += open as i32 - close as i32;
        if started && depth <= 0 {
            return i + 1;
        }
    }
    lines.len().min(start_line + 10)
}
