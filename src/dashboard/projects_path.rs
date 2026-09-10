use std::path::Path;

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
    if let Ok(canonical) = p.canonicalize() {
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

    s
}

/// Checks whether a path points to a unit test temporary directory that should not be tracked.
pub fn is_temp_project_path(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    if let Some(file_name) = Path::new(raw).file_name().and_then(|f| f.to_str()) {
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
