use crate::catalog::store::{SkillItem, load_all_skills};
use crate::context::scanner::{FileEntry, scan_project};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

const CACHE_TTL: Duration = Duration::from_secs(2);

pub struct ProjectSnapshot {
    pub files: Arc<Vec<FileEntry>>,
    pub skills: Arc<Vec<SkillItem>>,
}

struct CacheEntry {
    snapshot: Arc<ProjectSnapshot>,
    observed_at: SystemTime,
    markers: Vec<Option<SystemTime>>,
}

static PROJECT_CACHE: OnceLock<Mutex<HashMap<PathBuf, CacheEntry>>> = OnceLock::new();

fn marker_paths(root: &Path) -> Vec<PathBuf> {
    vec![
        root.to_path_buf(),
        root.join(".agents").join("skills"),
        root.join(".opencode").join("skills"),
        root.join(".claude").join("skills"),
    ]
}

fn markers(paths: &[PathBuf]) -> Vec<Option<SystemTime>> {
    paths
        .iter()
        .map(|path| {
            std::fs::metadata(path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
        })
        .collect()
}

pub fn project_snapshot(root: &Path) -> Arc<ProjectSnapshot> {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let paths = marker_paths(&canonical_root);
    let current_markers = markers(&paths);
    let now = SystemTime::now();
    let cache = PROJECT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Ok(guard) = cache.lock() {
        if let Some(entry) = guard.get(&canonical_root) {
            let fresh = now.duration_since(entry.observed_at).unwrap_or_default() < CACHE_TTL;
            if fresh && entry.markers == current_markers {
                return entry.snapshot.clone();
            }
        }
    }

    let snapshot = Arc::new(ProjectSnapshot {
        files: Arc::new(scan_project(&canonical_root, 2)),
        skills: Arc::new(load_all_skills(&canonical_root)),
    });

    if let Ok(mut guard) = cache.lock() {
        guard.insert(
            canonical_root,
            CacheEntry {
                snapshot: snapshot.clone(),
                observed_at: now,
                markers: current_markers,
            },
        );
    }

    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_snapshot_within_ttl() {
        let temp_dir = std::env::temp_dir().join(format!("cache_ttl_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let first = project_snapshot(&temp_dir);
        let second = project_snapshot(&temp_dir);
        assert!(Arc::ptr_eq(&first, &second));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
