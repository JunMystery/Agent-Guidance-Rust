use anyhow::Result;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::context::db::CodeGraphDb;
use crate::context::scanner::scan_project;

pub mod chunking;
pub mod cross_edges;
pub mod doc_data;
pub mod edge_parser;
pub mod embedder;
pub mod import_resolver;
pub mod parsers;
pub mod resolver;
pub use edge_parser::extract_edges_from_content;
pub use parsers::{
    CodeChunk, ExtractedEdge, ExtractedSymbol, chunk_code_content,
    extract_symbols_from_content,
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

        if let Ok(cross_count) = cross_edges::resolve_all_cross_edges(&mut self.db.conn, &self.project_path) {
            report.edges_created += cross_count;
        }

        report.duration_ms = start.elapsed().as_millis() as u64;
        Ok(report)
    }

    /// Incremental index: only index files whose size, mtime, or content hash has changed
    pub fn incremental_index(&mut self) -> Result<IndexReport> {
        let start = Instant::now();
        let files = scan_project(&self.project_path, 12);
        let mut report = IndexReport {
            files_scanned: files.len(),
            ..Default::default()
        };

        for file in files.iter().filter(|f| f.file_type == "file") {
            let full_path = self.project_path.join(&file.path);
            let metadata = match std::fs::metadata(&full_path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let disk_size = metadata.len();
            let disk_modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            // Fast path: if size and modified_at match cached SQLite record, skip reading disk!
            if let Ok(Some((_cached_hash, cached_size, cached_mod))) = self.db.get_file_metadata(&file.path) {
                if cached_size == disk_size && cached_mod == disk_modified {
                    report.files_skipped += 1;
                    continue;
                }
            }

            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let current_hash = compute_hash(&content);
                if let Ok(Some((cached_hash, _, _))) = self.db.get_file_metadata(&file.path) {
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

        if report.files_indexed > 0 {
            if let Ok(cross_count) = cross_edges::resolve_all_cross_edges(&mut self.db.conn, &self.project_path) {
                report.edges_created += cross_count;
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

        if report.files_indexed > 0 {
            if let Ok(cross_count) = cross_edges::resolve_all_cross_edges(&mut self.db.conn, &self.project_path) {
                report.edges_created += cross_count;
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
        let prefix = format!("{}::%", rel_path);
        let _ = self.db.conn.execute(
            "DELETE FROM symbol_edges WHERE source_id LIKE ?1 OR target_id LIKE ?1",
            rusqlite::params![prefix],
        );

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
            self.db.insert_symbol_full(
                &s.id,
                &s.name,
                &s.kind,
                rel_path,
                s.parent.as_deref(),
                s.start_line,
                s.end_line,
                s.signature.as_deref(),
                s.language.as_deref(),
                s.namespace.as_deref(),
                s.receiver.as_deref(),
            )?;
            report.symbols_extracted += 1;
        }

        // 3. Extract edges (imports / calls)
        let edges = extract_edges_from_content(rel_path, content, &symbols);
        for e in edges {
            self.db.insert_edge_full(
                &e.source_id,
                &e.target_id,
                &e.edge_type,
                e.weight,
                e.confidence,
                &e.category,
                e.call_line,
            )?;
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
        embedder::embed_symbols_batched(&self.db, 100, 32)
    }

    /// Background embedding of all code chunks (RAG) that do not have vectors yet
    pub fn embed_chunks(&self) -> Result<usize> {
        embedder::embed_chunks_batched(&self.db, 50, 32)
    }

    /// Background refresh of GraphRAG hierarchical community clustering
    pub fn update_graph_rag(&self, arch_pattern: &str) -> Result<()> {
        let engine = crate::context::graph_rag::GraphRagEngine::new(&self.project_path);
        let _ = engine.build_or_update(arch_pattern);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../indexer_tests.rs"]
mod tests;

#[cfg(test)]
mod chunking_tests;

#[cfg(test)]
mod embedder_tests;
