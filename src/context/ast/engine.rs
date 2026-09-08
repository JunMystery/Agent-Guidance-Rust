//! AST Engine providing Tree-Sitter parsing and analysis capabilities.

use tree_sitter::{Language, Parser, Tree};
use super::types::{AstCall, AstLanguage, AstSymbol};
use super::walker;

pub struct AstEngine;

impl AstEngine {
    /// Returns the tree-sitter Language for the given AstLanguage.
    pub fn get_language(lang: AstLanguage) -> Option<Language> {
        match lang {
            AstLanguage::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            AstLanguage::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            AstLanguage::Tsx | AstLanguage::JavaScript => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            AstLanguage::Python => Some(tree_sitter_python::LANGUAGE.into()),
            AstLanguage::Go => Some(tree_sitter_go::LANGUAGE.into()),
            AstLanguage::Unsupported => None,
        }
    }

    /// Parses source code using the appropriate Tree-Sitter grammar.
    pub fn parse(content: &str, lang: AstLanguage) -> Option<Tree> {
        let language = Self::get_language(lang)?;
        let mut parser = Parser::new();
        parser.set_language(&language).ok()?;
        parser.parse(content, None)
    }

    /// Extracts all symbols (functions, structs, classes, methods) via AST analysis.
    pub fn extract_symbols(content: &str, lang: AstLanguage) -> Vec<AstSymbol> {
        if let Some(tree) = Self::parse(content, lang) {
            walker::extract_symbols(tree.root_node(), content.as_bytes(), lang)
        } else {
            Vec::new()
        }
    }

    /// Extracts all function calls via AST analysis.
    pub fn extract_calls(content: &str, lang: AstLanguage) -> Vec<AstCall> {
        if let Some(tree) = Self::parse(content, lang) {
            walker::extract_calls(tree.root_node(), content.as_bytes(), lang)
        } else {
            Vec::new()
        }
    }

    /// Finds a specific symbol by exact name or suffix match.
    pub fn find_symbol(content: &str, lang: AstLanguage, symbol_name: &str) -> Option<AstSymbol> {
        let symbols = Self::extract_symbols(content, lang);
        symbols
            .into_iter()
            .find(|s| s.name == symbol_name || s.name.ends_with(&format!("::{}", symbol_name)))
    }

    /// Generates an AST semantic context slice (Zoom Read) expanding only target_symbol and folding siblings.
    pub fn generate_zoom_slice(
        content: &str,
        lang: AstLanguage,
        file_path: &str,
        target_symbol: &str,
    ) -> Option<super::zoom::ZoomSliceResult> {
        super::zoom::generate_zoom_slice(content, lang, file_path, target_symbol)
    }

    /// Generates a concise AST-aware code skeleton with function bodies collapsed.
    pub fn generate_skeleton(content: &str, lang: AstLanguage, file_path: &str) -> Option<String> {
        let symbols = Self::extract_symbols(content, lang);
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Only collapse function and method bodies
        let fn_symbols: Vec<&AstSymbol> = symbols
            .iter()
            .filter(|s| (s.kind == "function" || s.kind == "method") && s.end_line > s.start_line + 1)
            .collect();

        if fn_symbols.is_empty() {
            return None;
        }

        let is_py = matches!(lang, AstLanguage::Python);
        let mut skeleton = Vec::new();
        skeleton.push(format!("// === AST Structural Skeleton of '{}' (Total Lines: {}) ===", file_path, total_lines));

        let mut cur_line = 1;
        for sym in fn_symbols {
            if sym.start_line < cur_line {
                continue;
            }
            // Emit lines up to start_line
            while cur_line < sym.start_line && cur_line <= total_lines {
                skeleton.push(lines[cur_line - 1].to_string());
                cur_line += 1;
            }

            if cur_line <= total_lines {
                let start_line_idx = sym.start_line - 1;
                let first_line = lines[start_line_idx];
                let span = sym.end_line - sym.start_line;

                if is_py {
                    skeleton.push(first_line.to_string());
                    let indent: String = first_line.chars().take_while(|c| c.is_whitespace()).collect();
                    let body_indent = if indent.is_empty() { "    ".to_string() } else { format!("{}    ", indent) };
                    skeleton.push(format!("{}... /* L{}-{}: {} lines */", body_indent, sym.start_line + 1, sym.end_line, span));
                } else {
                    let indent: String = first_line.chars().take_while(|c| c.is_whitespace()).collect();
                    let trimmed = first_line.trim();
                    if trimmed.ends_with('{') {
                        let header = trimmed.trim_end_matches('{').trim_end();
                        skeleton.push(format!("{}{} {{ /* L{}-{}: {} lines */ }}", indent, header, sym.start_line + 1, sym.end_line, span));
                    } else {
                        skeleton.push(first_line.to_string());
                        skeleton.push(format!("{}{{ /* L{}-{}: {} lines */ }}", indent, sym.start_line + 1, sym.end_line, span));
                    }
                }
                cur_line = sym.end_line + 1;
            }
        }

        // Emit remaining lines after last function
        while cur_line <= total_lines {
            skeleton.push(lines[cur_line - 1].to_string());
            cur_line += 1;
        }

        Some(skeleton.join("\n"))
    }
}
