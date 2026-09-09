use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::info;

use super::impact::ensure_agent_context_gitignored;

pub fn get_session_snapshot_dir(proj_path: &Path, session_id: &str) -> PathBuf {
    ensure_agent_context_gitignored(proj_path);
    let clean_session = session_id.replace(['/', '\\', ':', '.'], "_");
    proj_path.join(".agent-context").join("snapshots").join(clean_session)
}

/// Creates a local snapshot of a file prior to first edit in the active session.
pub fn create_file_snapshot(proj_path: &Path, rel_path: &str, session_id: &str) -> Result<bool> {
    if rel_path.is_empty() {
        return Ok(false);
    }
    // Lazy cleanup on edit creation
    let _ = cleanup_stale_snapshots(proj_path, 7, 20);

    let source_path = proj_path.join(rel_path);
    if !source_path.exists() || !source_path.is_file() {
        return Ok(false);
    }

    let snapshot_dir = get_session_snapshot_dir(proj_path, session_id);
    let target_snapshot = snapshot_dir.join(rel_path);

    if target_snapshot.exists() {
        return Ok(false);
    }

    if let Some(parent) = target_snapshot.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(&source_path, &target_snapshot)?;
    info!("Created local rollback snapshot for '{}' in session '{}'", rel_path, session_id);
    Ok(true)
}

/// Restores all original files from the session snapshot directory back to the workspace.
pub fn restore_session_snapshots(proj_path: &Path, session_id: &str) -> Result<Vec<String>> {
    let snapshot_dir = get_session_snapshot_dir(proj_path, session_id);
    if !snapshot_dir.exists() {
        return Ok(Vec::new());
    }

    let mut restored = Vec::new();
    fn visit_dirs(dir: &Path, base_dir: &Path, proj_path: &Path, restored: &mut Vec<String>) -> Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, base_dir, proj_path, restored)?;
                } else if path.is_file() {
                    if let Ok(rel) = path.strip_prefix(base_dir) {
                        let target_dest = proj_path.join(rel);
                        if let Some(p) = target_dest.parent() {
                            let _ = fs::create_dir_all(p);
                        }
                        fs::copy(&path, &target_dest)?;
                        restored.push(rel.to_string_lossy().to_string());
                    }
                }
            }
        }
        Ok(())
    }

    visit_dirs(&snapshot_dir, &snapshot_dir, proj_path, &mut restored)?;
    info!("Restored {} files from session '{}' snapshot", restored.len(), session_id);
    Ok(restored)
}

/// Automatically purges snapshots older than `max_age_days` and enforces LRU ceiling `max_lru_sessions`.
pub fn cleanup_stale_snapshots(proj_path: &Path, max_age_days: i64, max_lru_sessions: usize) -> usize {
    let snapshots_dir = proj_path.join(".agent-context").join("snapshots");
    if !snapshots_dir.exists() || !snapshots_dir.is_dir() {
        return 0;
    }

    let now = SystemTime::now();
    let max_age = Duration::from_secs((max_age_days.max(1) as u64) * 86400);
    let mut pruned = 0;
    let mut session_folders: Vec<(SystemTime, PathBuf)> = Vec::new();

    if let Ok(entries) = fs::read_dir(&snapshots_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let modified = entry.metadata().and_then(|m| m.modified()).unwrap_or(now);
                if now.duration_since(modified).unwrap_or_default() > max_age {
                    if fs::remove_dir_all(&path).is_ok() {
                        pruned += 1;
                    }
                } else {
                    session_folders.push((modified, path));
                }
            }
        }
    }

    // LRU Cap: If more than max_lru_sessions remain, delete oldest
    if session_folders.len() > max_lru_sessions {
        session_folders.sort_by_key(|(mtime, _)| *mtime);
        let excess = session_folders.len() - max_lru_sessions;
        for (_, path) in session_folders.iter().take(excess) {
            if fs::remove_dir_all(path).is_ok() {
                pruned += 1;
            }
        }
    }

    if pruned > 0 {
        info!("Cleaned up {} stale session snapshot folders in {:?}", pruned, snapshots_dir);
    }
    pruned
}

/// Completely clears the `.agent-context/snapshots/` directory.
pub fn clear_all_snapshots(proj_path: &Path) -> Result<()> {
    let snapshots_dir = proj_path.join(".agent-context").join("snapshots");
    if snapshots_dir.exists() {
        fs::remove_dir_all(&snapshots_dir)?;
        info!("Cleared all session snapshots in {:?}", snapshots_dir);
    }
    Ok(())
}
