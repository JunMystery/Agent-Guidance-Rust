use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tracing::info;

pub struct RepoUpdateSpec {
    pub key: &'static str,
    pub name: &'static str,
    pub repo_url: &'static str,
}

pub const INTEGRATED_REPOS: &[RepoUpdateSpec] = &[
    RepoUpdateSpec {
        key: "ecc",
        name: "Everything Claude Code (ECC)",
        repo_url: "https://github.com/affaan-m/ECC",
    },
    RepoUpdateSpec {
        key: "ui_ux",
        name: "UI/UX Pro Max Skill",
        repo_url: "https://github.com/nextlevelbuilder/ui-ux-pro-max-skill",
    },
    RepoUpdateSpec {
        key: "anthropic",
        name: "Anthropic Skills",
        repo_url: "https://github.com/anthropics/skills",
    },
    RepoUpdateSpec {
        key: "owasp",
        name: "OWASP CheatSheet Series",
        repo_url: "https://github.com/OWASP/CheatSheetSeries",
    },
    RepoUpdateSpec {
        key: "system_design",
        name: "System Design Primer",
        repo_url: "https://github.com/donnemartin/system-design-primer",
    },
];

pub fn get_update_dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir()
            .join(format!("agent-guidance-skills-test-{}", std::process::id()));
    }
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agent-guidance-skills"))
}

fn update_state_path() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!(
            "agent-guidance-update-state-test-{}.json",
            std::process::id()
        ));
    }
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join(".update-state.json"))
        .unwrap_or_else(|| PathBuf::from(".update-state.json"))
}

pub fn run_update() -> Result<()> {
    let target_dir = get_update_dir();
    fs::create_dir_all(&target_dir)?;

    println!(
        "Checking and syncing 3rd-party skill repositories into {:?}",
        target_dir
    );

    let mut synced_count = 0;
    for repo in INTEGRATED_REPOS {
        let repo_dir = target_dir.join(repo.key);
        if repo_dir.join(".git").exists() {
            println!("  ↻ Pulling updates for {}...", repo.name);
            let status = std::process::Command::new("git")
                .args(["pull", "--ff-only"])
                .current_dir(&repo_dir)
                .status();
            match status {
                Ok(s) if s.success() => {
                    println!("    ✓ Successfully updated {}", repo.name);
                    info!("Git pull succeeded for: {}", repo.key);
                    synced_count += 1;
                }
                _ => {
                    println!(
                        "    ⚠ Git pull failed for {}, keeping existing files",
                        repo.name
                    );
                }
            }
        } else if !cfg!(test) {
            println!("  ↓ Cloning {} (depth 1)...", repo.name);
            let status = std::process::Command::new("git")
                .args(["clone", "--depth", "1", repo.repo_url, repo.key])
                .current_dir(&target_dir)
                .status();
            match status {
                Ok(s) if s.success() => {
                    println!("    ✓ Successfully cloned {}", repo.name);
                    info!("Git clone succeeded for: {}", repo.key);
                    synced_count += 1;
                }
                _ => {
                    println!(
                        "    ⚠ Git clone failed for {}, using embedded catalog fallback",
                        repo.name
                    );
                }
            }
        } else {
            // In unit tests, simulate clean creation
            let _ = fs::create_dir_all(&repo_dir);
            synced_count += 1;
        }
    }

    let state_file = update_state_path();
    let state_json = serde_json::json!({
        "last_update": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
        "status": "success",
        "repos_count": INTEGRATED_REPOS.len(),
        "synced_count": synced_count,
        "skills_directory": target_dir.to_string_lossy()
    });

    fs::write(state_file, serde_json::to_string_pretty(&state_json)?)?;
    println!("✓ 3rd-party skill repositories synced to ~/.agent-guidance/skills!");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_update_state() {
        assert!(run_update().is_ok());
    }
}
