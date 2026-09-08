pub mod scanner;
pub use scanner::scan_workspace_skills;

use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

#[derive(Embed)]
#[folder = "skills/"]
pub struct SkillAssets;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillSource {
    Embedded,
    LocalWorkspace(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillItem {
    pub name: String,
    pub relative_path: String,
    pub source: SkillSource,
    pub content: String,
}

static SEMANTIC_DOC_CACHE: OnceLock<RwLock<HashMap<String, Arc<super::indexer::SkillSemanticDocument>>>> =
    OnceLock::new();

impl SkillItem {
    /// Returns a shared reference to the cached semantic document, extracting and memoizing it on first access.
    pub fn get_semantic_doc(&self) -> Arc<super::indexer::SkillSemanticDocument> {
        let cache = SEMANTIC_DOC_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
        if let Ok(guard) = cache.read() {
            if let Some(doc) = guard.get(&self.name) {
                return doc.clone();
            }
        }
        let doc = Arc::new(super::indexer::SkillSemanticDocument::extract(
            &self.name,
            &self.content,
        ));
        if let Ok(mut guard) = cache.write() {
            guard.insert(self.name.clone(), doc.clone());
        }
        doc
    }

    /// Extracts structured semantic document for this skill, using cached doc if present.
    pub fn to_semantic_doc(&self) -> super::indexer::SkillSemanticDocument {
        (*self.get_semantic_doc()).clone()
    }

    /// Returns a structured, high-density search passage (~1,500 chars) for vector embedding and scoring.
    pub fn to_search_passage(&self) -> String {
        self.get_semantic_doc().to_passage(1500)
    }
}

pub fn list_embedded_skills() -> Vec<String> {
    SkillAssets::iter()
        .map(|path| path.as_ref().to_string())
        .filter(|path| path.ends_with("SKILL.md"))
        .collect()
}

pub fn get_embedded_skill(path: &str) -> Option<String> {
    if let Some(file) = SkillAssets::get(path) {
        return std::str::from_utf8(file.data.as_ref())
            .ok()
            .map(|s| s.to_string());
    }
    let trimmed = path.trim_start_matches("skills/").trim_start_matches('/');
    let variations = [
        format!("{}/SKILL.md", trimmed),
        format!("skills/{}/SKILL.md", trimmed),
        format!("{}.md", trimmed),
        trimmed.to_string(),
    ];
    for v in &variations {
        if let Some(file) = SkillAssets::get(v) {
            return std::str::from_utf8(file.data.as_ref())
                .ok()
                .map(|s| s.to_string());
        }
    }
    None
}

pub fn get_skills_target_dir() -> std::path::PathBuf {
    if cfg!(test) {
        return std::env::temp_dir()
            .join(format!("agent-guidance-skills-test-{}", std::process::id()));
    }
    dirs::home_dir()
        .map(|h| h.join(".agent-guidance").join("skills"))
        .unwrap_or_else(|| std::path::PathBuf::from(".agent-guidance-skills"))
}

/// Skills are embedded directly into the binary; no disk copy required.
pub fn sync_embedded_skills_to_disk() -> anyhow::Result<usize> {
    Ok(0)
}

pub fn load_all_skills(proj_path: &Path) -> Vec<SkillItem> {
    let mut skills = Vec::new();

    // 1. Embedded skills
    for path in list_embedded_skills() {
        if let Some(content) = get_embedded_skill(&path) {
            let name = path.split('/').next().unwrap_or(&path).to_string();

            skills.push(SkillItem {
                name,
                relative_path: path.clone(),
                source: SkillSource::Embedded,
                content,
            });
        }
    }

    // 2. Scanned workspace local skills
    let local_skills = scan_workspace_skills(proj_path);
    for local in local_skills {
        if !skills.iter().any(|s| s.name == local.name) {
            skills.push(local);
        }
    }

    skills
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::scanner::{extract_frontmatter_name, scan_skill_dir_recursive};
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn test_extract_frontmatter_name() {
        let content = "---\nname: my-skill\ndescription: test\n---";
        assert_eq!(extract_frontmatter_name(content), Some("my-skill".to_string()));
    }

    #[test]
    fn test_scan_workspace_skills() {
        let tmp_dir = std::env::temp_dir().join("test_agent_skills");
        let skill_dir = tmp_dir.join(".agents").join("skills").join("test-skill");
        let _ = fs::create_dir_all(&skill_dir);
        let skill_file = skill_dir.join("SKILL.md");
        let mut file = File::create(&skill_file).unwrap();
        writeln!(file, "---\nname: test-skill\n---").unwrap();

        let scanned = scan_workspace_skills(&tmp_dir);
        assert!(!scanned.is_empty());
        assert_eq!(scanned[0].name, "test-skill");

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_scan_ignores_non_skill_md_files() {
        let tmp_dir = std::env::temp_dir().join(format!(
            "test_skill_filter_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let skills_root = tmp_dir.join("skills");

        let repo_dir = skills_root.join("cloned-repo");
        let _ = fs::create_dir_all(&repo_dir);
        File::create(repo_dir.join("README.md")).unwrap().write_all(b"# Repo readme").unwrap();
        File::create(repo_dir.join("CHANGELOG.md")).unwrap().write_all(b"# Changes").unwrap();

        let docs_dir = repo_dir.join("docs");
        let _ = fs::create_dir_all(&docs_dir);
        File::create(docs_dir.join("guide.md")).unwrap().write_all(b"# Guide").unwrap();

        let real_skill = repo_dir.join("sub").join("real-skill");
        let _ = fs::create_dir_all(&real_skill);
        File::create(real_skill.join("SKILL.md")).unwrap()
            .write_all(b"---\nname: real-skill\n---\nActual skill content").unwrap();

        let refs_dir = real_skill.join("references");
        let _ = fs::create_dir_all(&refs_dir);
        File::create(refs_dir.join("api-docs.md")).unwrap().write_all(b"# API reference").unwrap();

        let mut results = Vec::new();
        scan_skill_dir_recursive(&skills_root, &skills_root, &mut results);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "real-skill");

        let _ = fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_scan_owasp_cheatsheets_plain_mds() {
        let tmp_dir = std::env::temp_dir().join(format!(
            "test_owasp_cheatsheets_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let owasp_root = tmp_dir.join("owasp");
        let cheatsheets_dir = owasp_root.join("cheatsheets");
        let _ = fs::create_dir_all(&cheatsheets_dir);

        File::create(cheatsheets_dir.join("SQL_Injection_Prevention_Cheat_Sheet.md"))
            .unwrap()
            .write_all(b"# SQL Injection Prevention Cheat Sheet\n\nDefense in depth against SQLi.")
            .unwrap();

        File::create(cheatsheets_dir.join("README.md"))
            .unwrap()
            .write_all(b"# Cheatsheets Index")
            .unwrap();

        let mut results = Vec::new();
        scan_skill_dir_recursive(&owasp_root, &owasp_root, &mut results);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "sql-injection-prevention-cheat-sheet");
        assert!(results[0].content.contains("Defense in depth against SQLi"));

        let _ = fs::remove_dir_all(tmp_dir);
    }
}
