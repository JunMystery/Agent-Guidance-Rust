use anyhow::Result;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::context::db::CodeGraphDb;
use crate::context::scanner::scan_project;

#[derive(Debug, Default, Clone)]
pub struct IndexReport {
    pub files_scanned: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub symbols_extracted: usize,
    pub edges_created: usize,
    pub chunks_created: usize,
    pub duration_ms: u64,
}

pub struct IncrementalIndexer {
    db: CodeGraphDb,
    project_path: PathBuf,
}

fn compute_hash(content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

impl IncrementalIndexer {
    pub fn new(project_path: &Path) -> Result<Self> {
        let db = CodeGraphDb::open_for_project(project_path)?;
        Ok(Self {
            db,
            project_path: project_path.to_path_buf(),
        })
    }

    /// Full index: index all discovered files regardless of existing cache
    pub fn full_index(&mut self) -> Result<IndexReport> {
        let start = Instant::now();
        let files = scan_project(&self.project_path, 12);
        let mut report = IndexReport {
            files_scanned: files.len(),
            ..Default::default()
        };

        for file in files.iter().filter(|f| f.file_type == "file") {
            if self.index_file(&file.path, &mut report)? {
                report.files_indexed += 1;
            } else {
                report.files_skipped += 1;
            }
        }

        report.duration_ms = start.elapsed().as_millis() as u64;
        Ok(report)
    }

    /// Incremental index: only index files whose content hash has changed
    pub fn incremental_index(&mut self) -> Result<IndexReport> {
        let start = Instant::now();
        let files = scan_project(&self.project_path, 12);
        let mut report = IndexReport {
            files_scanned: files.len(),
            ..Default::default()
        };

        for file in files.iter().filter(|f| f.file_type == "file") {
            let full_path = self.project_path.join(&file.path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let current_hash = compute_hash(&content);
                if let Ok(Some(cached_hash)) = self.db.get_file_content_hash(&file.path) {
                    if cached_hash == current_hash {
                        report.files_skipped += 1;
                        continue;
                    }
                }
                if self.index_file_content(&file.path, &content, current_hash, &mut report)? {
                    report.files_indexed += 1;
                }
            }
        }

        report.duration_ms = start.elapsed().as_millis() as u64;
        Ok(report)
    }

    /// Index a specific list of relative file paths (used by Watcher debounce)
    pub fn index_specific_files(&mut self, paths: &[PathBuf]) -> Result<IndexReport> {
        let start = Instant::now();
        let mut report = IndexReport {
            files_scanned: paths.len(),
            ..Default::default()
        };

        for path in paths {
            let rel_str = path.to_string_lossy().to_string();
            let full_path = self.project_path.join(path);
            if !full_path.exists() {
                // File was deleted
                let _ = self.db.delete_file(&rel_str);
                continue;
            }

            if self.index_file(&rel_str, &mut report)? {
                report.files_indexed += 1;
            } else {
                report.files_skipped += 1;
            }
        }

        report.duration_ms = start.elapsed().as_millis() as u64;
        Ok(report)
    }

    fn index_file(&mut self, rel_path: &str, report: &mut IndexReport) -> Result<bool> {
        let full_path = self.project_path.join(rel_path);
        match std::fs::read_to_string(&full_path) {
            Ok(content) => {
                let current_hash = compute_hash(&content);
                self.index_file_content(rel_path, &content, current_hash, report)
            }
            Err(_) => Ok(false),
        }
    }

    fn index_file_content(
        &mut self,
        rel_path: &str,
        content: &str,
        current_hash: String,
        report: &mut IndexReport,
    ) -> Result<bool> {
        // Skip files > 100KB or minified files
        if content.len() > 100 * 1024 || rel_path.ends_with(".min.js") || rel_path.ends_with(".lock") {
            return Ok(false);
        }

        // Clear existing data for this file
        let _ = self.db.clear_file_data(rel_path);

        let metadata = std::fs::metadata(self.project_path.join(rel_path));
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(content.len() as u64);
        let modified = metadata
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // 1. Upsert files table
        self.db.upsert_file(rel_path, &current_hash, size, modified)?;

        // 2. Extract symbols
        let symbols = extract_symbols_from_content(rel_path, content);
        for s in &symbols {
            self.db.insert_symbol(
                &s.id,
                &s.name,
                &s.kind,
                rel_path,
                s.parent.as_deref(),
                s.start_line,
                s.end_line,
                s.signature.as_deref(),
            )?;
            report.symbols_extracted += 1;
        }

        // 3. Extract edges (imports / calls)
        let edges = extract_edges_from_content(rel_path, content, &symbols);
        for e in edges {
            self.db.insert_edge(&e.source_id, &e.target_id, &e.edge_type, e.weight)?;
            report.edges_created += 1;
        }

        // 4. Content Chunking (50-line sliding window, 10-line overlap)
        let chunks = chunk_code_content(rel_path, content, 50, 10);
        for c in chunks {
            self.db.insert_chunk(rel_path, c.start_line, c.end_line, &c.hash, &c.text)?;
            report.chunks_created += 1;
        }

        Ok(true)
    }

    /// Background embedding of all symbols that do not have vectors yet
    pub fn embed_symbols(&self) -> Result<usize> {
        let unindexed = self.db.get_symbols_without_vectors(100)?;
        if unindexed.is_empty() {
            return Ok(0);
        }

        let model = match crate::ml::embeddings::EmbeddingModel::load_or_download() {
            Ok(m) => m,
            Err(_) => return Ok(0), // Graceful fallback if model files not yet present
        };

        let mut count = 0;
        for (id, name, kind, file_path, sig) in unindexed {
            let passage = format!(
                "{} in {} — {}",
                kind,
                file_path,
                sig.as_deref().unwrap_or(&name)
            );
            if let Ok(vec) = model.embed_text(&passage, Some("passage")) {
                let _ = self.db.store_symbol_vector(&id, &vec, "multilingual-e5-small");
                count += 1;
            }
        }

        Ok(count)
    }

    /// Background embedding of all code chunks (RAG) that do not have vectors yet
    pub fn embed_chunks(&self) -> Result<usize> {
        let unindexed = self.db.get_chunks_without_vectors(50)?;
        if unindexed.is_empty() {
            return Ok(0);
        }

        let model = match crate::ml::embeddings::EmbeddingModel::load_or_download() {
            Ok(m) => m,
            Err(_) => return Ok(0),
        };

        let mut count = 0;
        for (chunk_id, file_path, start, end, text) in unindexed {
            let truncated: String = text.chars().take(512).collect();
            let passage = format!("code in {} lines {}-{}: {}", file_path, start, end, truncated);
            if let Ok(vec) = model.embed_text(&passage, Some("passage")) {
                let _ = self.db.store_chunk_vector(chunk_id, &vec, "multilingual-e5-small");
                count += 1;
            }
        }

        Ok(count)
    }
}

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

    symbols
}

fn extract_name_after(line: &str, prefixes: &[&str], kind: &'static str) -> Option<(String, &'static str)> {
    for prefix in prefixes {
        if let Some(rest) = line.strip_prefix(prefix) {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                return Some((name, kind));
            }
        }
    }
    None
}

/// Extract edge relationships (imports + calls heuristic)
pub fn extract_edges_from_content(
    rel_path: &str,
    content: &str,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedEdge> {
    let mut edges = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        // 1. Imports extraction
        if trimmed.starts_with("use ") || trimmed.starts_with("import ") || trimmed.starts_with("from ") {
            let imported_target = trimmed
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .trim_end_matches(';');
            if !imported_target.is_empty() {
                edges.push(ExtractedEdge {
                    source_id: format!("{}::L{}", rel_path, idx + 1),
                    target_id: imported_target.to_string(),
                    edge_type: "imports".to_string(),
                    weight: 1.0,
                });
            }
        }

        // 2. Call heuristic: if line mentions another symbol
        for sym in symbols {
            if sym.start_line != (idx + 1) && line.contains(&format!("{}(", sym.name)) {
                edges.push(ExtractedEdge {
                    source_id: format!("{}::L{}", rel_path, idx + 1),
                    target_id: sym.id.clone(),
                    edge_type: "calls".to_string(),
                    weight: 1.0,
                });
            }
        }
    }

    edges
}

/// Chunk content into overlapping sliding windows (e.g. 50 lines with 10 overlap)
pub fn chunk_code_content(
    _rel_path: &str,
    content: &str,
    window_size: usize,
    overlap: usize,
) -> Vec<CodeChunk> {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total == 0 {
        return Vec::new();
    }

    let step = if window_size > overlap {
        window_size - overlap
    } else {
        window_size
    };
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < total {
        let end = (start + window_size).min(total);
        let chunk_lines = &lines[start..end];
        let non_empty = chunk_lines.iter().filter(|l| !l.trim().is_empty()).count();

        if non_empty >= 3 {
            let text = chunk_lines.join("\n");
            let hash = compute_hash(&text);
            chunks.push(CodeChunk {
                start_line: start + 1,
                end_line: end,
                hash,
                text,
            });
        }

        start += step;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_symbols_and_chunks() {
        let code = r#"
pub struct PaymentService {
    api_key: String,
}

impl PaymentService {
    pub fn process_payment(&self, amount: u64) -> bool {
        let timeout = 30;
        true
    }
}
"#;
        let symbols = extract_symbols_from_content("src/payment.rs", code);
        assert!(symbols.iter().any(|s| s.name == "PaymentService" && s.kind == "struct"));
        assert!(symbols.iter().any(|s| s.name == "process_payment" && s.kind == "function"));

        let chunks = chunk_code_content("src/payment.rs", code, 50, 10);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("process_payment"));
    }
}
