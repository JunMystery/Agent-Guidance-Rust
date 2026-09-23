//! Centralized filtering and exclusion rules for project scanning, indexing, and GraphRAG.

const EXCLUDED_DIRS: &[&str] = &[
    ".git", ".agent-context", ".svn", ".hg",
    "__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache",
    ".tox", ".nox", ".hypothesis", ".venv", "venv", "env",
    "node_modules", ".next", ".nuxt", ".turbo", ".svelte-kit",
    ".parcel-cache", ".docusaurus", ".vuepress", ".output",
    "dist", "build", "out", "coverage", ".nyc_output", ".yarn",
    "target", ".gradle", ".dart_tool", "cmake-build-debug",
    "cmake-build-release", "CMakeFiles", ".idea", ".vscode", ".vs",
    "tmp", "temp", ".temp", ".cache", "cache",
];

const EXCLUDED_EXTENSIONS: &[&str] = &[
    // Bytecode & binaries
    "pyc", "pyo", "pyd", "class", "so", "dll", "dylib", "exe", "bin",
    "obj", "o", "a", "lib", "wasm", "tmp", "temp", "swp", "swo", "bak", "orig",
    // Archives & compression
    "zip", "tar", "gz", "tgz", "bz2", "xz", "7z", "rar", "jar", "war",
    // Media & fonts
    "png", "jpg", "jpeg", "gif", "ico", "webp", "avif", "svg",
    "mp3", "mp4", "wav", "ogg", "woff", "woff2", "ttf", "eot",
    // Databases, logs, maps, locks
    "db", "sqlite", "sqlite3", "db-shm", "db-wal", "log", "map", "lock",
    // Documentation & text documents (no AST/export code value, direct read by agent on demand)
    "md", "markdown", "mdown", "mkdn", "txt", "text", "rst", "adoc", "asciidoc", "pdf", "rtf",
    // Data dumps, tables & heavy notebooks
    "csv", "tsv", "jsonl", "ndjson", "parquet", "ipynb", "drawio", "excalidraw",
];

const EXCLUDED_FILES: &[&str] = &[
    "package-lock.json", "pnpm-lock.yaml", "yarn.lock", "bun.lockb",
    "cargo.lock", "composer.lock", "gemfile.lock", "poetry.lock",
    ".ds_store", "thumbs.db", "desktop.ini",
    "license", "copying", "authors",
];

#[inline]
pub fn is_excluded_dir_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    EXCLUDED_DIRS.iter().any(|&d| d == lower)
}

#[inline]
pub fn is_excluded_file_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    EXCLUDED_FILES.iter().any(|&f| f == lower)
}

#[inline]
pub fn is_excluded_extension(ext: &str) -> bool {
    let clean = ext.trim_start_matches('.').to_ascii_lowercase();
    EXCLUDED_EXTENSIONS.iter().any(|&e| e == clean)
}

pub fn is_excluded_path(rel_path: &str) -> bool {
    let normalized = rel_path.replace('\\', "/");
    let parts: Vec<&str> = normalized
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    if parts.is_empty() {
        return false;
    }

    for part in &parts[..parts.len() - 1] {
        if is_excluded_dir_name(part) {
            return true;
        }
    }

    let last = parts.last().unwrap();
    if is_excluded_dir_name(last) || is_excluded_file_name(last) {
        return true;
    }

    let lower = last.to_ascii_lowercase();
    if lower.ends_with(".min.js") || lower.ends_with(".min.css") {
        return true;
    }

    if let Some(dot_pos) = last.rfind('.') {
        if is_excluded_extension(&last[dot_pos + 1..]) {
            return true;
        }
    }

    false
}

pub fn is_excluded_entry(entry: &ignore::DirEntry) -> bool {
    let name_str = entry.file_name().to_string_lossy();
    if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
        is_excluded_dir_name(&name_str)
    } else {
        is_excluded_path(&name_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_excluded_dirs() {
        assert!(is_excluded_dir_name("__pycache__"));
        assert!(is_excluded_dir_name(".pytest_cache"));
        assert!(is_excluded_dir_name("temp"));
        assert!(is_excluded_dir_name("TEMP"));
        assert!(is_excluded_dir_name(".cache"));
        assert!(!is_excluded_dir_name("src"));
    }

    #[test]
    fn test_excluded_paths() {
        assert!(is_excluded_path("src/__pycache__/module.cpython-312.pyc"));
        assert!(is_excluded_path("./__pycache__/foo.pyc"));
        assert!(is_excluded_path(".\\temp\\foo.tmp"));
        assert!(is_excluded_path("nested/dir/.venv/bin/activate"));
        assert!(is_excluded_path("temp/test.tmp"));
        assert!(is_excluded_path(".cache/data.bin"));
        assert!(is_excluded_path("package-lock.json"));
        assert!(is_excluded_path("Cargo.lock"));
        assert!(is_excluded_path("bundle.min.js"));
        // Docs & Markdown
        assert!(is_excluded_path("README.md"));
        assert!(is_excluded_path("docs/ARCHITECTURE.md"));
        assert!(is_excluded_path("notes.txt"));
        assert!(is_excluded_path("LICENSE"));
        // Data dumps & notebooks
        assert!(is_excluded_path("dataset.csv"));
        assert!(is_excluded_path("metrics.tsv"));
        assert!(is_excluded_path("experiments/notebook.ipynb"));
        assert!(is_excluded_path("dumps/data.parquet"));
        // Source & Structured config files MUST NOT be excluded
        assert!(!is_excluded_path("src/context/scanner.rs"));
        assert!(!is_excluded_path("./src/main.rs"));
        assert!(!is_excluded_path("tests/common.rs"));
        assert!(!is_excluded_path("Cargo.toml"));
        assert!(!is_excluded_path("package.json"));
        assert!(!is_excluded_path("config.yaml"));
        assert!(!is_excluded_path("settings.json"));
    }
}
