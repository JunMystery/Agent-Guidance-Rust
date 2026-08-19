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

    const MAX_SCAN_ENTRIES: usize = 5000;

    let walker = WalkBuilder::new(&target_root)
        .max_depth(Some(max_depth))
        .hidden(true)
        .git_ignore(true)
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                ".git"
                    | ".agent-context"
                    | "target"
                    | "node_modules"
                    | ".gradle"
                    | "build"
                    | "dist"
                    | ".next"
                    | "vendor"
                    | ".venv"
                    | ".cache"
                    | "__pycache__"
                    | ".turbo"
                    | "out"
            )
        })
        .build();

    for result in walker {
        if entries.len() >= MAX_SCAN_ENTRIES {
            break;
        }
        if let Ok(entry) = result {
            if entry.depth() == 0 {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(&target_root).unwrap_or(path);
            let rel_str = relative.to_string_lossy().to_string();

            let file_type = if path.is_dir() { "directory" } else { "file" };
            let size_bytes = if path.is_file() {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            } else {
                0
            };

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
