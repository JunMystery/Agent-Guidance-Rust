use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;
use crate::context::indexer::{IncrementalIndexer, IndexReport};

static PROJECT_LAST_SYNC: OnceLock<RwLock<HashMap<PathBuf, u64>>> = OnceLock::new();

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Ensures the project code graph and index are fresh before serving GraphRAG / Context requests.
/// Debounced by `debounce_secs` individually per project path so multiple IDEs/projects do not block each other.
pub fn ensure_fresh_graph(project_path: &Path, debounce_secs: u64) -> Result<Option<IndexReport>> {
    let canonical = project_path.canonicalize().unwrap_or_else(|_| project_path.to_path_buf());
    let now = now_secs();

    let map_lock = PROJECT_LAST_SYNC.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let read_guard = map_lock.read().unwrap_or_else(|p| p.into_inner());
        if let Some(&last) = read_guard.get(&canonical) {
            if now.saturating_sub(last) < debounce_secs {
                return Ok(None);
            }
        }
    }
    {
        let mut write_guard = map_lock.write().unwrap_or_else(|p| p.into_inner());
        write_guard.insert(canonical.clone(), now);
    }

    let mut indexer = match IncrementalIndexer::new(project_path) {
        Ok(idx) => idx,
        Err(_) => return Ok(None),
    };

    let report = indexer.incremental_index()?;
    if report.files_indexed > 0 {
        // Trigger background Rayon embedding pool and update community hierarchy
        let path_clone = project_path.to_path_buf();
        std::thread::spawn(move || {
            if let Ok(idx) = IncrementalIndexer::new(&path_clone) {
                let _ = idx.embed_symbols();
                let _ = idx.embed_chunks();
            }
            let engine = super::GraphRagEngine::new(&path_clone);
            let _ = engine.build_or_update("Auto");
        });
        return Ok(Some(report));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debounce_guard() {
        let p = Path::new(".");
        let res1 = ensure_fresh_graph(p, 100);
        assert!(res1.is_ok());
        // Immediate second call must be skipped by debounce
        let res2 = ensure_fresh_graph(p, 100);
        assert!(res2.is_ok());
        assert!(res2.unwrap().is_none());
    }

    #[test]
    fn test_distinct_projects_do_not_block_each_other() {
        let p1 = Path::new("tests");
        let p2 = Path::new("src");
        let res1 = ensure_fresh_graph(p1, 100);
        assert!(res1.is_ok());
        let res2 = ensure_fresh_graph(p2, 100);
        assert!(res2.is_ok());
    }
}
