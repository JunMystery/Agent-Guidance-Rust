// AST & Regex Symbol / Edge / Chunk Parsers
use super::compute_hash;

#[derive(Debug, Clone)]
pub struct ExtractedSymbol {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub parent: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
}

pub struct ExtractedEdge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: String,
    pub weight: f64,
}

pub struct CodeChunk {
    pub start_line: usize,
    pub end_line: usize,
    pub hash: String,
    pub text: String,
}

/// Extract symbols across languages (Rust, Python, TS/JS, Go, Kotlin, Java)
pub fn extract_symbols_from_content(rel_path: &str, content: &str) -> Vec<ExtractedSymbol> {
    let total_lines = content.lines().count().max(1);
    let file_name = std::path::Path::new(rel_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| rel_path.to_string());
    let mod_sym = ExtractedSymbol {
        id: format!("{}::module::{}::L1", rel_path, file_name),
        name: file_name,
        kind: "module".to_string(),
        parent: None,
        start_line: 1,
        end_line: total_lines,
        signature: Some(format!("mod {}", rel_path)),
    };

    let lang = crate::context::ast::AstLanguage::from_path(rel_path);
    if lang.is_supported() {
        let ast_symbols = crate::context::ast::AstEngine::extract_symbols(content, lang);
        if !ast_symbols.is_empty() {
            let mut symbols: Vec<ExtractedSymbol> = ast_symbols
                .into_iter()
                .map(|s| {
                    let id = format!("{}::{}::{}::L{}", rel_path, s.kind, s.name, s.start_line);
                    ExtractedSymbol {
                        id,
                        name: s.name,
                        kind: s.kind,
                        parent: None,
                        start_line: s.start_line,
                        end_line: s.end_line,
                        signature: Some(s.signature),
                    }
                })
                .collect();
            symbols.push(mod_sym);
            return symbols;
        }
    }

    let mut symbols = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let line_num = idx + 1;

        let detected = if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") {
            extract_name_after(trimmed, &["pub fn ", "fn "], "function")
        } else if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
            extract_name_after(trimmed, &["pub struct ", "struct "], "struct")
        } else if trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ") {
            extract_name_after(trimmed, &["pub enum ", "enum "], "enum")
        } else if trimmed.starts_with("pub trait ") || trimmed.starts_with("trait ") {
            extract_name_after(trimmed, &["pub trait ", "trait "], "trait")
        } else if trimmed.starts_with("impl ") {
            extract_name_after(trimmed, &["impl "], "impl")
        } else if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
            extract_name_after(trimmed, &["async def ", "def "], "function")
        } else if trimmed.starts_with("class ") {
            extract_name_after(trimmed, &["class "], "class")
        } else if trimmed.starts_with("export function ") || trimmed.starts_with("function ") {
            extract_name_after(trimmed, &["export function ", "function "], "function")
        } else if trimmed.starts_with("export class ") {
            extract_name_after(trimmed, &["export class "], "class")
        } else if trimmed.starts_with("export interface ") || trimmed.starts_with("interface ") {
            extract_name_after(trimmed, &["export interface ", "interface "], "interface")
        } else if trimmed.starts_with("export const ") {
            extract_name_after(trimmed, &["export const "], "constant")
        } else if trimmed.starts_with("func ") {
            extract_name_after(trimmed, &["func "], "function")
        } else {
            None
        };

        if let Some((name, kind)) = detected {
            let id = format!("{}::{}::{}::L{}", rel_path, kind, name, line_num);
            symbols.push(ExtractedSymbol {
                id,
                name,
                kind: kind.to_string(),
                parent: None,
                start_line: line_num,
                end_line: line_num + 10, // Estimate
                signature: Some(trimmed.to_string()),
            });
        }
    }

    symbols.push(mod_sym);
    symbols
}

fn extract_name_after(line: &str, prefixes: &[&str], kind: &'static str) -> Option<(String, &'static str)> {
    for prefix in prefixes {
        if let Some(rest) = line.strip_prefix(prefix) {
            let name = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                return Some((name.to_string(), kind));
            }
        }
    }
    None
}

/// Extract call edges and import edges for a file
pub fn extract_edges_from_content(
    rel_path: &str,
    content: &str,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedEdge> {
    let mut edges = Vec::new();
    let mod_id = super::resolver::find_module_symbol(symbols)
        .map(|s| s.id.as_str())
        .unwrap_or("");

    // 1. Imports extraction
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("use ") || trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            let token = trimmed.split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|s| !s.is_empty() && *s != "use" && *s != "import" && *s != "from" && *s != "crate" && *s != "super")
                .last();
            if let Some(target_name) = token {
                if let Some(target) = super::resolver::resolve_local_callee(symbols, target_name, mod_id) {
                    if !mod_id.is_empty() && mod_id != target.id {
                        edges.push(ExtractedEdge {
                            source_id: mod_id.to_string(),
                            target_id: target.id.clone(),
                            edge_type: "imports".to_string(),
                            weight: 1.0,
                        });
                    }
                }
            }
        }
    }

    // 2. Call edges: AST-native when supported, heuristic fallback otherwise
    let lang = crate::context::ast::AstLanguage::from_path(rel_path);
    if lang.is_supported() {
        let ast_calls = crate::context::ast::AstEngine::extract_calls(content, lang);
        for call in ast_calls {
            let caller = super::resolver::find_enclosing_symbol(symbols, call.line);
            let caller_id = caller.map(|c| c.id.as_str()).unwrap_or(mod_id);
            if caller_id.is_empty() { continue; }

            if let Some(target) = super::resolver::resolve_local_callee(symbols, &call.callee_name, caller_id) {
                edges.push(ExtractedEdge {
                    source_id: caller_id.to_string(),
                    target_id: target.id.clone(),
                    edge_type: "calls".to_string(),
                    weight: 1.0,
                });
            }
        }
    } else {
        for (idx, line) in content.lines().enumerate() {
            let line_num = idx + 1;
            let caller = super::resolver::find_enclosing_symbol(symbols, line_num);
            let caller_id = caller.map(|c| c.id.as_str()).unwrap_or(mod_id);
            if caller_id.is_empty() { continue; }

            for sym in symbols {
                if sym.kind != "module" && sym.id != caller_id && line.contains(&format!("{}(", sym.name)) {
                    edges.push(ExtractedEdge {
                        source_id: caller_id.to_string(),
                        target_id: sym.id.clone(),
                        edge_type: "calls".to_string(),
                        weight: 1.0,
                    });
                }
            }
        }
    }

    edges
}

/// Chunk content into overlapping sliding windows (e.g. 50 lines with 10 overlap)
pub fn chunk_code_content(
    rel_path: &str,
    content: &str,
    window_size: usize,
    overlap: usize,
) -> Vec<CodeChunk> {
    let symbols = extract_symbols_from_content(rel_path, content);
    super::chunking::build_semantic_chunks(rel_path, content, &symbols, window_size, overlap)
}

