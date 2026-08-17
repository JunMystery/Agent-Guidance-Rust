use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::context::indexer::IncrementalIndexer;

/// Registry of active watchers — 1 per project
static WATCHERS: OnceLock<Mutex<HashMap<PathBuf, WatcherHandle>>> = OnceLock::new();

struct WatcherHandle {
    started_at: Instant,
    active: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Start watching a project directory for changes in background.
/// Idempotent: if already watching, returns immediately.
pub fn start_watching(project_path: &Path) {
    let canonical = project_path
        .canonicalize()
        .unwrap_or_else(|_| project_path.to_path_buf());
    let registry = WATCHERS.get_or_init(|| Mutex::new(HashMap::new()));

    if let Ok(guard) = registry.lock() {
        if guard.contains_key(&canonical) {
            return;
        }
    }

    let active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let active_clone = active.clone();
    let proj_for_thread = canonical.clone();

    // Spawn background debounce polling/inotify thread
    std::thread::Builder::new()
        .name(format!(
            "ag-watcher-{}",
            canonical
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into())
        ))
        .spawn(move || {
            watcher_loop(proj_for_thread, active_clone);
        })
        .ok();

    if let Ok(mut guard) = registry.lock() {
        guard.insert(
            canonical,
            WatcherHandle {
                started_at: Instant::now(),
                active,
            },
        );
    }
}

/// Check if a project is actively being watched
pub fn is_watching(project_path: &Path) -> bool {
    let canonical = project_path
        .canonicalize()
        .unwrap_or_else(|_| project_path.to_path_buf());
    let registry = WATCHERS.get_or_init(|| Mutex::new(HashMap::new()));
    registry
        .lock()
        .map(|g| g.contains_key(&canonical))
        .unwrap_or(false)
}

const DEBOUNCE_DURATION: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

fn watcher_loop(project_path: PathBuf, active: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut last_event: Option<Instant> = None;
    let mut last_snapshot_time = Instant::now();

    while active.load(std::sync::atomic::Ordering::Relaxed) {
        std::thread::sleep(POLL_INTERVAL);

        // Check if debounce time elapsed and we have pending files to index
        if let Some(last) = last_event {
            if !pending.is_empty() && last.elapsed() >= DEBOUNCE_DURATION {
                let changed: Vec<PathBuf> = pending.drain().collect();
                if let Ok(mut indexer) = IncrementalIndexer::new(&project_path) {
                    let _ = indexer.index_specific_files(&changed);
                    let _ = indexer.embed_symbols();
                    let _ = indexer.embed_chunks();
                }
                last_event = None;
                last_snapshot_time = Instant::now();
            }
        } else if last_snapshot_time.elapsed() >= Duration::from_secs(30) {
            // Periodic background incremental check every 30s
            if let Ok(mut indexer) = IncrementalIndexer::new(&project_path) {
                let _ = indexer.incremental_index();
                let _ = indexer.embed_symbols();
                let _ = indexer.embed_chunks();
            }
            last_snapshot_time = Instant::now();
        }
    }
}

pub fn is_relevant_path(project_root: &Path, path: &Path) -> bool {
    let rel = match path.strip_prefix(project_root) {
        Ok(r) => r.to_string_lossy().to_string(),
        Err(_) => return false,
    };

    let normalized = rel.replace('\\', "/");

    let excluded_dirs = [
        ".git",
        ".agent-context",
        "target",
        "node_modules",
        ".gradle",
        "build",
        "__pycache__",
        ".mypy_cache",
        "dist",
        ".next",
        ".nuxt",
    ];
    for dir in &excluded_dirs {
        if normalized == *dir || normalized.starts_with(&format!("{}/", dir)) {
            return false;
        }
    }

    let excluded_extensions = [
        ".lock", ".min.js", ".min.css", ".map", ".png", ".jpg", ".gif", ".ico", ".woff",
        ".woff2", ".exe", ".dll", ".so", ".dylib",
    ];
    for ext in &excluded_extensions {
        if normalized.ends_with(ext) {
            return false;
        }
    }

    if path.exists() && path.is_dir() {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relevant_path_filtering() {
        let root = Path::new("/workspace/myproject");
        assert!(is_relevant_path(root, &root.join("src/main.rs")));
        assert!(!is_relevant_path(root, &root.join(".git/HEAD")));
        assert!(!is_relevant_path(root, &root.join(".agent-context/code_graph.db")));
        assert!(!is_relevant_path(root, &root.join("target/debug/build")));
        assert!(!is_relevant_path(root, &root.join("package-lock.json.lock")));
    }

    #[test]
    fn test_watcher_idempotency() {
        let temp_dir = std::env::temp_dir().join(format!("ag_watch_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        assert!(!is_watching(&temp_dir));
        start_watching(&temp_dir);
        assert!(is_watching(&temp_dir));
        start_watching(&temp_dir); // Idempotent call
        assert!(is_watching(&temp_dir));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
