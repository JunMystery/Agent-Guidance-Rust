use std::path::{Path, PathBuf};
use crate::context::db::CodeGraphDb;
use crate::context::scanner::scan_project;
use crate::context::watcher::{is_watching, start_watching};
use crate::mcp::state::ServerState;

pub fn ensure_not_cancelled(state: &ServerState) -> Result<(), (i32, String)> {
    if state.is_cancelled() {
        Err((-32000, "Request cancelled after timeout".to_string()))
    } else {
        Ok(())
    }
}

/// Validate that a relative path stays within the base directory root
pub fn validate_path(base_path: &Path, rel_path: &str) -> Result<PathBuf, String> {
    if rel_path.contains("..") {
        return Err("Path traversal (..) is strictly prohibited.".to_string());
    }

    let canonical_base = base_path
        .canonicalize()
        .map_err(|e| format!("Invalid project path: {}", e))?;

    let clean_base = canonical_base.to_string_lossy();
    let clean_base_str = clean_base.strip_prefix(r"\\?\").unwrap_or(&clean_base);
    let p_clean_base = Path::new(clean_base_str);

    let p_rel = Path::new(rel_path);
    let stripped = if p_rel.is_absolute() {
        if let Ok(sub) = p_rel.strip_prefix(base_path) {
            sub.to_string_lossy().to_string()
        } else if let Ok(sub) = p_rel.strip_prefix(&canonical_base) {
            sub.to_string_lossy().to_string()
        } else if let Ok(sub) = p_rel.strip_prefix(p_clean_base) {
            sub.to_string_lossy().to_string()
        } else if let Ok(can_rel) = p_rel.canonicalize() {
            if let Ok(sub) = can_rel.strip_prefix(&canonical_base) {
                sub.to_string_lossy().to_string()
            } else {
                let candidate = rel_path.trim_start_matches(|c| c == '/' || c == '\\');
                let joined = canonical_base.join(candidate);
                if joined.exists() && joined.canonicalize().map(|p| p.starts_with(&canonical_base)).unwrap_or(false) {
                    candidate.to_string()
                } else {
                    return Err("Target path resolves outside workspace root.".to_string());
                }
            }
        } else {
            let candidate = rel_path.trim_start_matches(|c| c == '/' || c == '\\');
            let joined = canonical_base.join(candidate);
            if joined.starts_with(&canonical_base) {
                candidate.to_string()
            } else {
                return Err("Target path resolves outside workspace root.".to_string());
            }
        }
    } else {
        rel_path.to_string()
    };

    // 2. Strip leading slashes and normalize separators
    let clean_rel = stripped
        .trim()
        .trim_start_matches(|c| c == '/' || c == '\\')
        .replace('\\', "/");

    if clean_rel.is_empty() {
        return Ok(canonical_base);
    }

    let full_path = canonical_base.join(&clean_rel);

    if full_path.exists() {
        let canonical_full = full_path
            .canonicalize()
            .map_err(|e| format!("Invalid target path: {}", e))?;

        if !canonical_full.starts_with(&canonical_base) {
            return Err("Target path resolves outside workspace root.".to_string());
        }
        Ok(canonical_full)
    } else {
        if full_path.starts_with(&canonical_base) {
            Ok(full_path)
        } else {
            Err("Target path resolves outside workspace root.".to_string())
        }
    }
}

pub fn detect_parent_process_cwd() -> Option<PathBuf> {
    use sysinfo::{Pid, System};
    let mut sys = System::new_all();
    sys.refresh_all();

    let my_pid = Pid::from_u32(std::process::id());
    if let Some(proc_) = sys.process(my_pid) {
        if let Some(parent_pid) = proc_.parent() {
            if let Some(parent_proc) = sys.process(parent_pid) {
                if let Some(cwd) = parent_proc.cwd() {
                    if cwd.is_dir() && !cwd.to_string_lossy().to_lowercase().contains("antigravity") {
                        return Some(cwd.to_path_buf());
                    }
                }
            }
        }
    }
    None
}

pub fn is_generic_home_dir(p: &Path) -> bool {
    if let Some(home) = dirs::home_dir() {
        if p == home {
            return true;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if p == Path::new(&home) {
            return true;
        }
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        if p == Path::new(&userprofile) {
            return true;
        }
    }
    false
}

pub fn detect_project_architecture(proj_path: &Path) -> String {
    // 1. Check persistent project architecture configuration if present
    if let Some(persisted) = ServerState::load_persisted_architecture(proj_path) {
        return persisted;
    }

    // 2. Check GraphRAG communities hierarchy if available
    if let Some(hierarchy) = crate::context::graph_rag::persistence::load_hierarchy(proj_path) {
        if !hierarchy.detected_architecture.is_empty()
            && !hierarchy.detected_architecture.eq_ignore_ascii_case("auto")
            && !hierarchy.detected_architecture.eq_ignore_ascii_case("none")
        {
            let _ = ServerState::save_persisted_architecture(proj_path, &hierarchy.detected_architecture);
            return hierarchy.detected_architecture;
        }
    }

    let files = scan_project(proj_path, 8);
    let paths: Vec<String> = files.into_iter().map(|f| f.path.to_lowercase()).collect();

    let detected = if paths.iter().any(|p| {
        p.contains("domain")
            || p.contains("usecase")
            || p.contains("use_case")
            || p.contains("infrastructure")
            || p.contains("infra")
            || p.contains("entities")
            || p.contains("entity")
    }) {
        "Clean_Architecture".to_string()
    } else if paths.iter().any(|p| {
        p.contains("controller")
            || p.contains("service")
            || p.contains("model")
            || p.contains("viewmodel")
            || p.contains("repository")
            || p.contains("dao")
            || p.contains("database")
    }) {
        "Layered_Architecture".to_string()
    } else if paths.iter().any(|p| {
        p.contains("feature")
            || p.contains("module")
            || p.contains("screens")
            || p.contains("pages")
    }) {
        "Package_By_Feature".to_string()
    } else if paths.iter().any(|p| {
        p.contains("commands")
            || p.contains("command")
            || p.contains("cli")
            || p.contains("cmd")
            || p.contains("args")
            || p.contains("opt")
    }) {
        "CLI_Pipeline".to_string()
    } else if paths.len() <= 12 {
        "Flat_Library".to_string()
    } else {
        "Orchestrator".to_string()
    };

    // Automatically persist the detected pattern to .agent-context/architecture.json
    let _ = ServerState::save_persisted_architecture(proj_path, &detected);
    detected
}

pub fn resolve_architecture_pattern(raw_pattern: &str, proj_path: &Path, state: &ServerState) -> String {
    let trimmed = raw_pattern.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("auto")
        || trimmed.eq_ignore_ascii_case("none")
    {
        if let Some(active) = &state.active_architecture_pattern {
            if !active.eq_ignore_ascii_case("auto")
                && !active.eq_ignore_ascii_case("none")
                && !active.is_empty()
            {
                return active.clone();
            }
        }
        detect_project_architecture(proj_path)
    } else {
        trimmed.to_string()
    }
}

pub fn detect_project_path(explicit_path: &str, state: &ServerState) -> PathBuf {
    if explicit_path != "." && !explicit_path.trim().is_empty() {
        let p = PathBuf::from(explicit_path);
        if !is_generic_home_dir(&p) {
            return crate::dashboard::projects_path::find_project_root(&p);
        }
    }

    if let Some(ref sp) = state.project_path {
        let p = PathBuf::from(sp);
        if !is_generic_home_dir(&p) {
            return crate::dashboard::projects_path::find_project_root(&p);
        }
    }

    if let Some(first_root) = state.workspace_roots.first() {
        let p = PathBuf::from(first_root);
        if !is_generic_home_dir(&p) {
            return crate::dashboard::projects_path::find_project_root(&p);
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if !is_generic_home_dir(&cwd) {
            return crate::dashboard::projects_path::find_project_root(&cwd);
        }
    }

    if let Some(parent_cwd) = detect_parent_process_cwd() {
        if !is_generic_home_dir(&parent_cwd) {
            return crate::dashboard::projects_path::find_project_root(&parent_cwd);
        }
    }

    if let Some(gp) = ServerState::read_global_project_path() {
        let p = PathBuf::from(&gp);
        if !is_generic_home_dir(&p) {
            return crate::dashboard::projects_path::find_project_root(&p);
        }
    }

    PathBuf::from(".")
}

pub fn ensure_indexed(proj_path: &Path) -> Option<CodeGraphDb> {
    let _ = crate::context::graph_rag::jit_sync::ensure_fresh_graph(proj_path, 15);
    if !is_watching(proj_path) {
        start_watching(proj_path);
    }
    let start = std::time::Instant::now();
    while crate::context::graph_rag::jit_sync::is_indexing(proj_path) && start.elapsed() < std::time::Duration::from_millis(2000) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    CodeGraphDb::open_for_project(proj_path).ok()
}

pub fn embed_query(query: &str) -> Option<Vec<f32>> {
    crate::ml::embeddings::try_cached_model().and_then(|model| {
        model.embed_text(query, Some("query")).ok()
    })
}

/// Truncate string to at most `max_chars` UTF-8 characters safely without splitting multibyte characters.
pub fn truncate_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Truncate string to at most `max_bytes` without splitting UTF-8 char boundaries.
pub fn truncate_bytes_safe(s: &str, max_bytes: usize) -> &str {
    if max_bytes >= s.len() {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
