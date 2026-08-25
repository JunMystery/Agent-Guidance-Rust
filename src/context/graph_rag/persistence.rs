use anyhow::Result;
use std::fs;
use std::path::Path;
use super::community::CommunityHierarchy;

pub fn communities_file_path(project_path: &Path) -> std::path::PathBuf {
    project_path.join(".agent-context").join("communities.json")
}

pub fn save_hierarchy(project_path: &Path, hierarchy: &CommunityHierarchy) -> Result<()> {
    let dir = project_path.join(".agent-context");
    fs::create_dir_all(&dir)?;

    let file_path = communities_file_path(project_path);
    let json = serde_json::to_string_pretty(hierarchy)?;

    let tmp_path = dir.join(format!("communities.tmp.{}", std::process::id()));
    fs::write(&tmp_path, &json)?;
    if let Err(_) = fs::rename(&tmp_path, &file_path) {
        let _ = fs::write(&file_path, &json);
        let _ = fs::remove_file(&tmp_path);
    }

    // Also update .agent-context/architecture.json with detected architecture
    let arch_file = dir.join("architecture.json");
    let payload = serde_json::json!({
        "architecture_pattern": hierarchy.detected_architecture,
        "updated_at": hierarchy.updated_at,
        "community_count": hierarchy.communities.len()
    });
    let arch_json = serde_json::to_string_pretty(&payload)?;
    let _ = fs::write(arch_file, arch_json);

    Ok(())
}

pub fn load_hierarchy(project_path: &Path) -> Option<CommunityHierarchy> {
    let file_path = communities_file_path(project_path);
    if file_path.exists() {
        if let Ok(content) = fs::read_to_string(&file_path) {
            if let Ok(hierarchy) = serde_json::from_str::<CommunityHierarchy>(&content) {
                return Some(hierarchy);
            }
        }
    }
    None
}
