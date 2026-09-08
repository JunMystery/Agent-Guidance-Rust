use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;
use crate::context::indexer::{IncrementalIndexer, IndexReport};

static LAST_SYNC_EPOCH_SECS: AtomicU64 = AtomicU64::new(0);

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Ensures the project code graph and index are fresh before serving GraphRAG / Context requests.
/// Debounced by `debounce_secs` to ensure sub-millisecond overhead on rapid consecutive tool calls.
pub fn ensure_fresh_graph(project_path: &Path, debounce_secs: u64) -> Result<Option<IndexReport>> {
    let now = now_secs();
    let last = LAST_SYNC_EPOCH_SECS.load(Ordering::Relaxed);
    if now.saturating_sub(last) < debounce_secs {
        return Ok(None);
    }
    LAST_SYNC_EPOCH_SECS.store(now, Ordering::Relaxed);

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
}
