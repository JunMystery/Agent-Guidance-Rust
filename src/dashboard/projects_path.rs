use std::path::{Path, PathBuf};

fn is_submodule_dir_name(p: &Path) -> bool {
    if p.join(".git").exists() {
        return false;
    }
    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
        let lower = name.to_lowercase();
        matches!(
            lower.as_str(),
            "src" | "lib" | "tests" | "test" | "benches" | "bin" | "pkg" | "vendor" | "node_modules" | "target" | "build" | "dist"
        )
    } else {
        false
    }
}

/// Finds the root project directory by looking upwards for `.agent-context` folder,
/// or fallback VCS/manifest roots (.git, Cargo.toml, package.json, go.mod, pyproject.toml).
pub fn find_project_root(path: &Path) -> PathBuf {
    let abs_buf;
    let base_path: &Path = if path.is_absolute() {
        path
    } else if let Ok(cwd) = std::env::current_dir() {
        if path == Path::new(".") || path == Path::new("") {
            abs_buf = cwd;
        } else {
            abs_buf = cwd.join(path);
        }
        &abs_buf
    } else {
        path
    };

    let curr = if base_path.is_file() {
        base_path.parent().unwrap_or(base_path).to_path_buf()
    } else {
        base_path.to_path_buf()
    };

    let home = dirs::home_dir();
    let temp = std::env::temp_dir();
    let is_inside_temp = curr.starts_with(&temp);

    // Priority 1: Search upwards for .agent-context directory (prefer outermost root)
    let mut best_agent_root: Option<PathBuf> = None;
    let mut check_agent = curr.clone();
    for _ in 0..15 {
        if let Some(ref h) = home {
            if &check_agent == h {
                break;
            }
        }
        if is_inside_temp && check_agent == temp {
            break;
        }
        if check_agent.join(".agent-context").is_dir() && !is_submodule_dir_name(&check_agent) {
            best_agent_root = Some(check_agent.clone());
        }
        if !check_agent.pop() {
            break;
        }
    }
    if let Some(root) = best_agent_root {
        return root;
    }

    // Priority 2: Search upwards for project manifests or .git
    let mut best_manifest_root: Option<PathBuf> = None;
    let mut check_manifest = curr.clone();
    for _ in 0..15 {
        if let Some(ref h) = home {
            if &check_manifest == h {
                break;
            }
        }
        if is_inside_temp && check_manifest == temp {
            break;
        }
        if (check_manifest.join(".git").exists()
            || check_manifest.join("Cargo.toml").is_file()
            || check_manifest.join("package.json").is_file()
            || check_manifest.join("go.mod").is_file()
            || check_manifest.join("pyproject.toml").is_file())
            && !is_submodule_dir_name(&check_manifest)
        {
            best_manifest_root = Some(check_manifest.clone());
            if check_manifest.join(".git").exists() {
                return check_manifest;
            }
        }
        if !check_manifest.pop() {
            break;
        }
    }
    if let Some(root) = best_manifest_root {
        return root;
    }

    curr
}

/// Normalizes project path string across platforms (uppercasing Windows drive, trimming trailing slashes, consistent separator).
pub fn normalize_project_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut s = trimmed.replace('/', "\\");
    while s.len() > 3 && (s.ends_with('\\') || s.ends_with('/')) {
        s.pop();
    }

    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        let first = s.chars().next().unwrap();
        if first.is_ascii_lowercase() {
            let upper = first.to_ascii_uppercase().to_string();
            s.replace_range(..1, &upper);
        }
    }

    let p = Path::new(&s);
    let resolved = find_project_root(p);

    if let Ok(canonical) = resolved.canonicalize() {
        let c_str = canonical.to_string_lossy().to_string();
        let stripped = c_str.strip_prefix(r"\\?\").unwrap_or(&c_str);
        let mut res = stripped.replace('/', "\\");
        while res.len() > 3 && (res.ends_with('\\') || res.ends_with('/')) {
            res.pop();
        }
        if res.len() >= 2 && res.as_bytes()[1] == b':' {
            let first = res.chars().next().unwrap();
            if first.is_ascii_lowercase() {
                let upper = first.to_ascii_uppercase().to_string();
                res.replace_range(..1, &upper);
            }
        }
        return res;
    }

    let mut res = resolved.to_string_lossy().replace('/', "\\");
    while res.len() > 3 && (res.ends_with('\\') || res.ends_with('/')) {
        res.pop();
    }
    if res.len() >= 2 && res.as_bytes()[1] == b':' {
        let first = res.chars().next().unwrap();
        if first.is_ascii_lowercase() {
            let upper = first.to_ascii_uppercase().to_string();
            res.replace_range(..1, &upper);
        }
    }
    res
}

/// Checks whether a path points to a unit test temporary directory or home dir that should not be tracked.
pub fn is_temp_project_path(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return true;
    }
    let clean = trimmed.trim_end_matches(['/', '\\']);
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_string();
        let clean_home = home_str.trim_end_matches(['/', '\\']);
        if clean.eq_ignore_ascii_case(clean_home)
            || clean.eq_ignore_ascii_case(&clean_home.replace('/', "\\"))
            || clean.eq_ignore_ascii_case(&clean_home.replace('\\', "/"))
        {
            return true;
        }
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let clean_up = userprofile.trim_end_matches(['/', '\\']);
        if clean.eq_ignore_ascii_case(clean_up)
            || clean.eq_ignore_ascii_case(&clean_up.replace('/', "\\"))
            || clean.eq_ignore_ascii_case(&clean_up.replace('\\', "/"))
        {
            return true;
        }
    }
    let lower = trimmed.to_lowercase();
    if let Some(file_name) = Path::new(trimmed).file_name().and_then(|f| f.to_str()) {
        let fn_lower = file_name.to_lowercase();
        if fn_lower.starts_with("ag_tools_test_")
            || fn_lower.starts_with("deep_search_test_")
            || fn_lower.starts_with("ag_graph_mcp_test_")
            || fn_lower.starts_with("ag_neighborhood_test_")
            || fn_lower.starts_with("last_project_path_test_")
        {
            return true;
        }
    }
    if !cfg!(test) {
        if lower.contains("appdata\\local\\temp") || lower.contains("appdata/local/temp") {
            return true;
        }
        if lower.starts_with("/tmp") || lower.contains("/tmp/") || lower.contains("\\tmp\\") {
            return true;
        }
    }
    false
}
