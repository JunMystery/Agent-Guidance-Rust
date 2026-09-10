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
    pub language: Option<String>,
    pub namespace: Option<String>,
    pub receiver: Option<String>,
}

pub struct ExtractedEdge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: String,
    pub weight: f64,
    pub confidence: f64,
    pub category: String,
    pub call_line: Option<usize>,
}

pub struct CodeChunk {
    pub start_line: usize,
    pub end_line: usize,
    pub hash: String,
    pub text: String,
}

/// Extract symbols across languages (Rust, Python, TS/JS, Go, Kotlin, Java, C#, Shell, etc.)
pub fn extract_symbols_from_content(rel_path: &str, content: &str) -> Vec<ExtractedSymbol> {
    let total_lines = content.lines().count().max(1);
    if let Some(cat) = super::doc_data::classify_doc_or_data(rel_path) {
        return vec![super::doc_data::extract_doc_or_data_symbol(rel_path, total_lines, cat)];
    }
    let file_name = std::path::Path::new(rel_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| rel_path.to_string());
    let lang = crate::context::ast::AstLanguage::from_path(rel_path);
    let mod_sym = ExtractedSymbol {
        id: format!("{}::module::{}::L1", rel_path, file_name),
        name: file_name,
        kind: "module".to_string(),
        parent: None,
        start_line: 1,
        end_line: total_lines,
        signature: Some(format!("mod {}", rel_path)),
        language: Some(lang.as_str().to_string()),
        namespace: None,
        receiver: None,
    };

    let stem = std::path::Path::new(rel_path)
        .file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    if lang.is_supported() {
        let ast_symbols = crate::context::ast::AstEngine::extract_symbols(content, lang);
        if !ast_symbols.is_empty() {
            let pkg_name = ast_symbols.iter().find_map(|s| s.package.clone());
            let mut symbols: Vec<ExtractedSymbol> = ast_symbols
                .into_iter()
                .map(|s| {
                    let id = format!("{}::{}::{}::L{}", rel_path, s.kind, s.name, s.start_line);
                    ExtractedSymbol {
                        id,
                        name: s.name,
                        kind: s.kind,
                        parent: s.parent_symbol,
                        start_line: s.start_line,
                        end_line: s.end_line,
                        signature: Some(s.signature),
                        language: Some(s.language),
                        namespace: s.package,
                        receiver: None,
                    }
                })
                .collect();

            if let Some(pkg) = pkg_name {
                symbols.push(ExtractedSymbol {
                    id: format!("{}::package::{}::L1", rel_path, pkg),
                    name: pkg.clone(),
                    kind: "package".to_string(),
                    parent: Some(pkg.clone()),
                    start_line: 1,
                    end_line: total_lines,
                    signature: Some(format!("package {}", rel_path)),
                    language: Some(lang.as_str().to_string()),
                    namespace: Some(pkg),
                    receiver: None,
                });
            }

            if lang.is_frontend() && !stem.is_empty() {
                symbols.push(ExtractedSymbol {
                    id: format!("{}::component::{}::L1", rel_path, stem),
                    name: stem,
                    kind: "component".to_string(),
                    parent: None,
                    start_line: 1,
                    end_line: total_lines,
                    signature: Some(format!("component {}", rel_path)),
                    language: Some(lang.as_str().to_string()),
                    namespace: None,
                    receiver: None,
                });
            }

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
                end_line: line_num + 10,
                signature: Some(trimmed.to_string()),
                language: Some(lang.as_str().to_string()),
                namespace: None,
                receiver: None,
            });
        }
    }

    if lang.is_frontend() && !stem.is_empty() {
        symbols.push(ExtractedSymbol {
            id: format!("{}::component::{}::L1", rel_path, stem),
            name: stem,
            kind: "component".to_string(),
            parent: None,
            start_line: 1,
            end_line: total_lines,
            signature: Some(format!("component {}", rel_path)),
            language: Some(lang.as_str().to_string()),
            namespace: None,
            receiver: None,
        });
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

// Re-export edge extraction decomposed into edge_parser module
pub use super::edge_parser::extract_edges_from_content;

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
