use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

use super::ServerState;

impl ServerState {
    pub fn cleanup_stale_sessions(proj_path: &Path) {
        let dir = proj_path.join(".agent-context").join("sessions");
        if !dir.exists() {
            return;
        }

        let now = SystemTime::now();
        let max_age = std::time::Duration::from_secs(30 * 86400); // 30 Days Retention
        let mut entries = Vec::new();

        if let Ok(read_dir) = fs::read_dir(&dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(metadata) = entry.metadata() {
                        let modified = metadata.modified().unwrap_or(now);
                        if now.duration_since(modified).unwrap_or_default() > max_age {
                            let _ = fs::remove_file(&path);
                            continue;
                        }
                        entries.push((modified, path));
                    }
                }
            }
        }

        // LRU Cap: If > 100 session files, delete oldest
        if entries.len() > 100 {
            entries.sort_by_key(|(mtime, _)| *mtime);
            for (_, path) in entries.iter().take(entries.len() - 100) {
                let _ = fs::remove_file(path);
            }
        }
    }

    pub fn save_to_dir(&self, proj_path: &Path) -> Result<(), String> {
        let dir = proj_path.join(".agent-context").join("sessions");
        if let Err(e) = fs::create_dir_all(&dir) {
            return Err(format!("Failed to create directory: {}", e));
        }
        let file_path = dir.join(format!("{}.json", self.session_id));
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;

        // Atomic write via temp file
        let tmp_file = dir.join(format!("{}.json.tmp.{}", self.session_id, std::process::id()));
        fs::write(&tmp_file, &content).map_err(|e| e.to_string())?;
        if let Err(_e) = fs::rename(&tmp_file, &file_path) {
            let _ = fs::write(&file_path, &content);
            let _ = fs::remove_file(&tmp_file);
        }

        // Also write atomic link pointer for legacy single-session tools
        let legacy_file = proj_path.join(".agent-context").join("session.json");
        let _ = fs::write(legacy_file, content);
        Ok(())
    }

    /// Automatically saves snapshot checkpoint to `.agent-context/sessions/{session_id}.json`.
    pub fn auto_checkpoint(&self, proj_path: &Path) -> Result<(), String> {
        self.save_to_dir(proj_path)
    }

    pub fn load_from_dir(proj_path: &Path) -> Result<Self, String> {
        Self::cleanup_stale_sessions(proj_path);
        let dir = proj_path.join(".agent-context").join("sessions");

        // 1. Try finding most recent session file in sessions/
        if dir.exists() {
            if let Ok(read_dir) = fs::read_dir(&dir) {
                let mut session_files: Vec<(SystemTime, std::path::PathBuf)> = read_dir
                    .flatten()
                    .filter_map(|e| {
                        let p = e.path();
                        if p.extension().and_then(|s| s.to_str()) == Some("json") {
                            let mtime = e.metadata().ok()?.modified().ok()?;
                            Some((mtime, p))
                        } else {
                            None
                        }
                    })
                    .collect();

                session_files.sort_by_key(|(mtime, _)| *mtime);
                if let Some((_, newest_path)) = session_files.last() {
                    if let Ok(content) = fs::read_to_string(newest_path) {
                        if let Ok(loaded) = serde_json::from_str::<Self>(&content) {
                            return Ok(loaded);
                        }
                    }
                }
            }
        }

        // 2. Legacy fallback: .agent-context/session.json
        let legacy_file = proj_path.join(".agent-context").join("session.json");
        if legacy_file.exists() {
            let content = fs::read_to_string(legacy_file).map_err(|e| e.to_string())?;
            return serde_json::from_str(&content).map_err(|e| e.to_string());
        }

        Ok(Self::new())
    }

    /// Lists all active/saved sessions from `.agent-context/sessions/`.
    pub fn list_sessions(proj_path: &Path) -> Vec<Self> {
        let dir = proj_path.join(".agent-context").join("sessions");
        if !dir.exists() {
            return Vec::new();
        }

        let Ok(read_dir) = fs::read_dir(&dir) else {
            return Vec::new();
        };

        let mut sessions: Vec<(SystemTime, Self)> = read_dir
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("json") {
                    let mtime = e.metadata().ok()?.modified().ok()?;
                    let content = fs::read_to_string(&p).ok()?;
                    let loaded = serde_json::from_str::<Self>(&content).ok()?;
                    Some((mtime, loaded))
                } else {
                    None
                }
            })
            .collect();

        sessions.sort_by_key(|(mtime, _)| *mtime);
        sessions.into_iter().rev().map(|(_, s)| s).collect()
    }

    /// Loads a specific session by its session_id.
    pub fn load_session_by_id(proj_path: &Path, target_id: &str) -> Result<Self, String> {
        let dir = proj_path.join(".agent-context").join("sessions");
        let file_path = dir.join(format!("{}.json", target_id));
        if !file_path.exists() {
            return Err(format!(
                "Session '{}' not found in '{}'. Use operation=\"list\" to view available sessions.",
                target_id,
                dir.display()
            ));
        }

        let content = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
        serde_json::from_str::<Self>(&content).map_err(|e| format!("Corrupted session file '{}': {}", target_id, e))
    }
}
