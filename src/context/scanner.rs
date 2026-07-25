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
    let walker = WalkBuilder::new(root)
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
            let relative = path.strip_prefix(root).unwrap_or(path);
            let rel_str = relative.to_string_lossy().to_string();

            if rel_str.starts_with(".git") || rel_str.starts_with(".agent-context") {
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
