use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub file_type: String,
    pub size_bytes: u64,
}

pub fn scan_project(root: &Path, max_depth: usize) -> Vec<FileEntry> {
    let mut entries = Vec::new();

    // Canonicalize root to resolve relative paths like '..' and symlinks safely
    let target_root = match root.canonicalize() {
        Ok(path) => path,
        Err(_) => return entries,
    };

    let walker = WalkBuilder::new(&target_root)
        .max_depth(Some(max_depth))
        .hidden(true)
        .git_ignore(true)
        .build();

    for result in walker {
        if let Ok(entry) = result {
            if entry.depth() == 0 {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(&target_root).unwrap_or(path);
            let rel_str = relative.to_string_lossy().to_string();

            let normalized = rel_str.replace('\\', "/");
            if normalized == ".git"
                || normalized.starts_with(".git/")
                || normalized == ".agent-context"
                || normalized.starts_with(".agent-context/")
                || normalized == "target"
                || normalized.starts_with("target/")
                || normalized == "node_modules"
                || normalized.starts_with("node_modules/")
                || normalized == ".gradle"
                || normalized.starts_with(".gradle/")
                || normalized == "build"
                || normalized.starts_with("build/")
            {
                continue;
            }

            let file_type = if path.is_dir() { "directory" } else { "file" };
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);

            entries.push(FileEntry {
                path: rel_str,
                file_type: file_type.to_string(),
                size_bytes,
            });
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_project() {
        let current_dir = Path::new(".");
        let files = scan_project(current_dir, 2);
        assert!(!files.is_empty());
        assert!(files.iter().any(|f| f.path == "Cargo.toml"));
    }

    #[test]
    fn test_scan_project_excludes_agent_context_and_build() {
        let temp_dir = std::env::temp_dir().join(format!("scan_excl_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(temp_dir.join(".agent-context").join("sessions"));
        let _ = std::fs::create_dir_all(temp_dir.join("build").join("outputs"));
        let _ = std::fs::create_dir_all(temp_dir.join("src"));
        let _ = std::fs::write(temp_dir.join("src").join("main.rs"), "fn main() {}");
        let _ = std::fs::write(temp_dir.join(".agent-context").join("sessions").join("test.json"), "{}");

        let files = scan_project(&temp_dir, 5);
        assert!(files.iter().any(|f| f.path.contains("main.rs")));
        assert!(!files.iter().any(|f| f.path.contains(".agent-context")));
        assert!(!files.iter().any(|f| f.path.contains("build")));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
