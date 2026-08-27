use std::path::{Path, PathBuf};
use crate::context::db::CodeGraphDb;
use crate::context::indexer::IncrementalIndexer;
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

    let full_path = canonical_base.join(rel_path);

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
    if let Ok(home) = std::env::var("HOME") {
        if p == Path::new(&home) {
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
            return p;
        }
    }

    if let Some(ref sp) = state.project_path {
        let p = PathBuf::from(sp);
        if !is_generic_home_dir(&p) {
            return p;
        }
    }

    if let Some(first_root) = state.workspace_roots.first() {
        let p = PathBuf::from(first_root);
        if !is_generic_home_dir(&p) {
            return p;
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        let mut curr = cwd.clone();
        for _ in 0..10 {
            if curr.join(".git").exists()
                || curr.join("Cargo.toml").exists()
                || curr.join("package.json").exists()
                || curr.join("go.mod").exists()
                || curr.join("pyproject.toml").exists()
            {
                return curr;
            }
            if !curr.pop() {
                break;
            }
        }
        if !is_generic_home_dir(&cwd) {
            return cwd;
        }
    }

    if let Some(parent_cwd) = detect_parent_process_cwd() {
        if !is_generic_home_dir(&parent_cwd) {
            return parent_cwd;
        }
    }

    if let Some(gp) = ServerState::read_global_project_path() {
        let p = PathBuf::from(&gp);
        if !is_generic_home_dir(&p) {
            return p;
        }
    }

    PathBuf::from(".")
}

pub fn ensure_indexed(proj_path: &Path) -> Option<CodeGraphDb> {
    if let Ok(mut indexer) = IncrementalIndexer::new(proj_path) {
        let _ = indexer.incremental_index();
        let path = proj_path.to_path_buf();
        std::thread::spawn(move || {
            if let Ok(idx) = IncrementalIndexer::new(&path) {
                let _ = idx.embed_symbols();
                let _ = idx.embed_chunks();
            }
        });
    }
    if !is_watching(proj_path) {
        start_watching(proj_path);
    }
    CodeGraphDb::open_for_project(proj_path).ok()
}

pub fn embed_query(query: &str) -> Option<Vec<f32>> {
    crate::ml::embeddings::try_cached_model().and_then(|model| {
        model.embed_text(query, Some("query")).ok()
    })
}
