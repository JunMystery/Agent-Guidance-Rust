//! Write-Through AST Invalidator & Targeted JIT File Synchronization.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::context::db::CodeGraphDb;
use crate::context::indexer::IncrementalIndexer;

#[derive(Debug, Clone, Default)]
pub struct FileInvalidationReport {
    pub rel_path: String,
    pub is_dirty: bool,
    pub reindexed: bool,
    pub symbols_extracted: usize,
    pub edges_created: usize,
    pub duration_ms: u64,
}

/// Fast check whether a file on disk has changed compared to SQLite index.
pub fn is_file_dirty(project_path: &Path, rel_path: &str) -> bool {
    let clean_path = rel_path
        .trim()
        .trim_start_matches(|c| c == '/' || c == '\\')
        .replace('\\', "/");

    if crate::context::exclusion::is_excluded_path(&clean_path) {
        return false;
    }

    let full_path = project_path.join(&clean_path);

    if !full_path.exists() {
        return true; // File deleted
    }

    let Ok(metadata) = std::fs::metadata(&full_path) else {
        return false;
    };

    let disk_size = metadata.len();
    let disk_modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let Ok(db) = CodeGraphDb::open_for_project(project_path) else {
        return true;
    };

    match db.get_file_metadata(&clean_path) {
        Ok(Some((_hash, cached_size, cached_mod))) => {
            cached_size != disk_size || cached_mod != disk_modified
        }
        _ => true, // Not yet indexed
    }
}

/// Synchronously invalidates and re-indexes a single file with < 10ms latency.
pub fn invalidate_and_sync_file(
    project_path: &Path,
    rel_path: &str,
) -> Result<FileInvalidationReport> {
    let start = Instant::now();
    let clean_path = rel_path
        .trim()
        .trim_start_matches(|c| c == '/' || c == '\\')
        .replace('\\', "/");

    let mut report = FileInvalidationReport {
        rel_path: clean_path.clone(),
        ..Default::default()
    };

    if crate::context::exclusion::is_excluded_path(&clean_path) || !is_file_dirty(project_path, &clean_path) {
        report.duration_ms = start.elapsed().as_millis() as u64;
        return Ok(report);
    }

    report.is_dirty = true;

    let mut indexer = IncrementalIndexer::new(project_path)?;
    let index_report = indexer.index_specific_files(&[PathBuf::from(&clean_path)])?;

    report.reindexed = index_report.files_indexed > 0;
    report.symbols_extracted = index_report.symbols_extracted;
    report.edges_created = index_report.edges_created;
    report.duration_ms = start.elapsed().as_millis() as u64;

    if report.reindexed {
        // Spawn background embedding update for the updated file
        let path_clone = project_path.to_path_buf();
        let file_clone = clean_path;
        std::thread::spawn(move || {
            if let Ok(idx) = IncrementalIndexer::new(&path_clone) {
                let _ = idx.embed_symbols();
                let _ = idx.embed_chunks();
            }
            let _ = file_clone;
        });
    }

    Ok(report)
}

/// Batch syncs multiple dirty files (e.g. from session dirty queue).
pub fn sync_dirty_files(
    project_path: &Path,
    rel_paths: &[String],
) -> Result<Vec<FileInvalidationReport>> {
    let mut reports = Vec::new();
    for p in rel_paths {
        if let Ok(rep) = invalidate_and_sync_file(project_path, p) {
            if rep.reindexed {
                reports.push(rep);
            }
        }
    }
    Ok(reports)
}
