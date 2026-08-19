use anyhow::Result;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::context::db::CodeGraphDb;
use crate::context::scanner::scan_project;

pub mod parsers;
pub use parsers::{
    CodeChunk, ExtractedEdge, ExtractedSymbol, chunk_code_content,
    extract_edges_from_content, extract_symbols_from_content,
};

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

pub fn compute_hash(content: &str) -> String {
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

        let model_guard = match crate::ml::embeddings::try_cached_model() {
            Some(m) => m,
            None => return Ok(0), // Graceful zero-latency fallback if model not currently resident in memory
        };

        let mut count = 0;
        for (id, name, kind, file_path, sig) in unindexed {
            let passage = format!(
                "{} in {} — {}",
                kind,
                file_path,
                sig.as_deref().unwrap_or(&name)
            );
            if let Ok(vec) = model_guard.embed_text(&passage, Some("passage")) {
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

        let model_guard = match crate::ml::embeddings::try_cached_model() {
            Some(m) => m,
            None => return Ok(0),
        };

        let mut count = 0;
        for (chunk_id, file_path, start, end, text) in unindexed {
            let truncated: String = text.chars().take(512).collect();
            let passage = format!("code in {} lines {}-{}: {}", file_path, start, end, truncated);
            if let Ok(vec) = model_guard.embed_text(&passage, Some("passage")) {
                let _ = self.db.store_chunk_vector(chunk_id, &vec, "multilingual-e5-small");
                count += 1;
            }
        }

        Ok(count)
    }
}


#[cfg(test)]
#[path = "../indexer_tests.rs"]
mod tests;

