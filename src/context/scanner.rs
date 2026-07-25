use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
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

            if rel_str == ".git" || rel_str.starts_with(".git/") || rel_str == ".agent-context" || rel_str.starts_with(".agent-context/") {
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
}
