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

pub use super::snapshots::{
    cleanup_stale_snapshots, clear_all_snapshots, create_file_snapshot, get_session_snapshot_dir,
    restore_session_snapshots,
};

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
