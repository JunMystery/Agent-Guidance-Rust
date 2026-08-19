use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::context::db::CodeGraphDb;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone)]
pub struct RiskAssessment {
    pub risk_level: RiskLevel,
    pub dependent_count: usize,
    pub dependent_files: Vec<String>,
    pub warning: Option<String>,
}

/// Assess the architectural modification risk of a target file using CodeGraphDb.
/// - Low Risk: < 3 dependencies
/// - Medium Risk: 3 to 8 dependencies
/// - High Risk (Critical Hub): > 8 dependencies
pub fn assess_file_risk(proj_path: &Path, rel_path: &str) -> RiskAssessment {
    if rel_path.is_empty() {
        return RiskAssessment {
            risk_level: RiskLevel::Low,
            dependent_count: 0,
            dependent_files: Vec::new(),
            warning: None,
        };
    }

    if let Ok(db) = CodeGraphDb::open_for_project(proj_path) {
        let count = db.count_incoming_dependencies(rel_path).unwrap_or(0);
        let files = db.get_incoming_dependent_files(rel_path, 10).unwrap_or_default();

        if count > 8 {
            RiskAssessment {
                risk_level: RiskLevel::High,
                dependent_count: count,
                dependent_files: files.clone(),
                warning: Some(format!(
                    "CRITICAL HUB WARNING: File '{}' has {} incoming dependent modules (e.g. {}). Modifying this core file requires explicit justification and verification.",
                    rel_path,
                    count,
                    if files.is_empty() { "—".to_string() } else { files.join(", ") }
                )),
            }
        } else if count >= 3 {
            RiskAssessment {
                risk_level: RiskLevel::Medium,
                dependent_count: count,
                dependent_files: files.clone(),
                warning: Some(format!(
                    "NOTICE: File '{}' is referenced by {} dependent modules ({}). Verify dependent files after editing.",
                    rel_path,
                    count,
                    files.join(", ")
                )),
            }
        } else {
            RiskAssessment {
                risk_level: RiskLevel::Low,
                dependent_count: count,
                dependent_files: files,
                warning: None,
            }
        }
    } else {
        RiskAssessment {
            risk_level: RiskLevel::Low,
            dependent_count: 0,
            dependent_files: Vec::new(),
            warning: None,
        }
    }
}

/// Automatically ensures `.agent-context/` is added to `.gitignore` to prevent Git tracking of local metadata.
pub fn ensure_agent_context_gitignored(proj_path: &Path) {
    if !proj_path.exists() || !proj_path.is_dir() {
        return;
    }
    let gitignore_path = proj_path.join(".gitignore");
    if gitignore_path.exists() {
        if let Ok(content) = fs::read_to_string(&gitignore_path) {
            let lines: Vec<&str> = content.lines().map(|l| l.trim()).collect();
            let already_ignored = lines.iter().any(|&l| {
                l == ".agent-context"
                    || l == ".agent-context/"
                    || l == "/.agent-context"
                    || l == "/.agent-context/"
                    || l.starts_with(".agent-context")
            });
            if !already_ignored {
                let mut updated = content;
                if !updated.ends_with('\n') && !updated.is_empty() {
                    updated.push('\n');
                }
                updated.push_str("\n# Agent Guidance local metadata & rollback snapshots\n.agent-context/\n");
                let _ = fs::write(&gitignore_path, updated);
                info!("Automatically added .agent-context/ to {:?}", gitignore_path);
            }
        }
    } else if proj_path.join(".git").exists() {
        let content = "# Agent Guidance local metadata & rollback snapshots\n.agent-context/\n";
        let _ = fs::write(&gitignore_path, content);
        info!("Created .gitignore with .agent-context/ in {:?}", proj_path);
    }
}

fn get_session_snapshot_dir(proj_path: &Path, session_id: &str) -> PathBuf {
    ensure_agent_context_gitignored(proj_path);
    let clean_session = session_id.replace(['/', '\\', ':', '.'], "_");
    proj_path.join(".agent-context").join("snapshots").join(clean_session)
}

/// Creates a local snapshot of a file prior to first edit in the active session.
pub fn create_file_snapshot(proj_path: &Path, rel_path: &str, session_id: &str) -> Result<bool> {
    if rel_path.is_empty() {
        return Ok(false);
    }

    let source_path = proj_path.join(rel_path);
    if !source_path.exists() || !source_path.is_file() {
        return Ok(false);
    }

    let snapshot_dir = get_session_snapshot_dir(proj_path, session_id);
    let target_snapshot = snapshot_dir.join(rel_path);

    // If snapshot already exists for this file in this session, keep the pristine first-edit copy
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_agent_context_gitignored_appends_to_existing() {
        let temp_dir = std::env::temp_dir().join(format!("impact_gitignore_test1_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);

        let gitignore = temp_dir.join(".gitignore");
        fs::write(&gitignore, "target/\n*.log\n").unwrap();

        ensure_agent_context_gitignored(&temp_dir);

        let content = fs::read_to_string(&gitignore).unwrap();
        assert!(content.contains(".agent-context/"));
        assert!(content.contains("target/"));

        // Idempotency check: calling again shouldn't duplicate
        ensure_agent_context_gitignored(&temp_dir);
        let content_after = fs::read_to_string(&gitignore).unwrap();
        let count = content_after.matches(".agent-context/").count();
        assert_eq!(count, 1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_ensure_agent_context_gitignored_creates_when_in_git_repo() {
        let temp_dir = std::env::temp_dir().join(format!("impact_gitignore_test2_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);

        let git_dir = temp_dir.join(".git");
        fs::create_dir(&git_dir).unwrap();

        ensure_agent_context_gitignored(&temp_dir);

        let gitignore = temp_dir.join(".gitignore");
        assert!(gitignore.exists());
        let content = fs::read_to_string(&gitignore).unwrap();
        assert!(content.contains(".agent-context/"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
