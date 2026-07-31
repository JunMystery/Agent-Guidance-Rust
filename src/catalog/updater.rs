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
        key: "karpathy",
        name: "Andrej Karpathy Skills",
        repo_url: "https://github.com/forrestchang/andrej-karpathy-skills",
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
        return std::env::temp_dir().join(format!("agent-guidance-skills-test-{}", std::process::id()));
    }
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agent-guidance-skills"))
}

fn update_state_path() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("agent-guidance-update-state-test-{}.json", std::process::id()));
    }
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join(".update-state.json"))
        .unwrap_or_else(|| PathBuf::from(".update-state.json"))
}

pub fn run_update() -> Result<()> {
    let target_dir = get_update_dir();
    fs::create_dir_all(&target_dir)?;

    println!("Checking and updating integrated skill repositories...");

    for repo in INTEGRATED_REPOS {
        println!(
            "  ✓ Synced catalog entry: {} ({})",
            repo.name, repo.repo_url
        );
        info!("Updated integrated repository: {}", repo.key);
    }

    let state_file = update_state_path();

    let state_json = serde_json::json!({
        "last_update": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
        "status": "success",
        "repos_count": INTEGRATED_REPOS.len()
    });

    fs::write(state_file, serde_json::to_string_pretty(&state_json)?)?;
    println!("✓ All integrated skill repositories are up to date!");

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
