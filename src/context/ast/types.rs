//! AST domain types and language recognition for Tree-Sitter parsing.

use std::path::Path;

/// Supported languages for native AST parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AstLanguage {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
    Unsupported,
}

impl AstLanguage {
    /// Detects AST language from file path or extension.
    pub fn from_path(path: &str) -> Self {
        let p = Path::new(path);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext.to_ascii_lowercase().as_str() {
            "rs" => AstLanguage::Rust,
            "ts" => AstLanguage::TypeScript,
            "tsx" => AstLanguage::Tsx,
            "js" | "mjs" | "cjs" | "jsx" => AstLanguage::JavaScript,
            "py" | "pyw" => AstLanguage::Python,
            "go" => AstLanguage::Go,
            _ => AstLanguage::Unsupported,
        }
    }

    /// Returns true if this language is supported by native Tree-Sitter.
    pub fn is_supported(&self) -> bool {
        !matches!(self, AstLanguage::Unsupported)
    }
}

/// A parsed symbol extracted via AST analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstSymbol {
    pub name: String,
    pub kind: String,
    pub start_line: usize, // 1-indexed
    pub end_line: usize,   // 1-indexed
    pub start_byte: usize,
    pub end_byte: usize,
    pub signature: String,
    pub body_start_line: Option<usize>,
    pub body_end_line: Option<usize>,
}

/// A function/method call detected in AST for call graph building.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstCall {
    pub caller_name: Option<String>,
    pub callee_name: String,
    pub line: usize,
}
