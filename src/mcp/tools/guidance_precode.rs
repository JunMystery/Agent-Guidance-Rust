use serde_json::Value;

use crate::context::cache::project_snapshot;
use crate::mcp::state::ServerState;
use super::helpers::{detect_project_architecture, detect_project_path};

pub(crate) fn handle_precode(
    arguments: &Value,
    query: &str,
    state: &mut ServerState,
) -> String {
    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);
    let snapshot = project_snapshot(&proj_path);
    let profile = crate::catalog::language_detector::detect_language_profile(
        snapshot.files.as_ref(),
        query,
    );
    let active_arch = state
        .active_architecture_pattern
        .clone()
        .unwrap_or_else(|| detect_project_architecture(&proj_path));

    let primary_lang = if profile.primary_languages.contains("rust") {
        "Rust"
    } else if profile.primary_languages.contains("kotlin")
        || profile.primary_languages.contains("java")
    {
        "Kotlin/Java"
    } else if profile.primary_languages.contains("go") {
        "Go"
    } else if profile.primary_languages.contains("python") {
        "Python"
    } else if profile.primary_languages.contains("typescript")
        || profile.primary_languages.contains("javascript")
    {
        "TypeScript/JavaScript"
    } else {
        "General"
    };

    let lang_rules = match primary_lang {
        "Rust" => {
            "- Rust Safety: Explicit lifetime/borrowing checks, handle Result/Option cleanly, avoid unwrap() in production paths."
        }
        "Kotlin/Java" => {
            "- Kotlin/Java Safety: Scope coroutines to Dispatchers.IO/Default, avoid forced unwraps (!!), respect StateFlow/LiveData lifecycles, keep Compose functions idempotent."
        }
        "Go" => {
            "- Go Safety: Enforce explicit error checking (if err != nil), bind goroutine lifecycles to context.Context cancellation, avoid data races on shared structs."
        }
        "Python" => {
            "- Python Safety: Type hints, handle None dereferences explicitly, avoid mutable default arguments."
        }
        "TypeScript/JavaScript" => {
            "- TS/JS Safety: Strict type definitions, optional chaining (`?.`), nullish coalescing (`??`)."
        }
        _ => {
            "- Language Safety: Verify non-null objects before dereferencing, enforce explicit error handling."
        }
    };

    let arch_blueprint = match active_arch.as_str() {
        "Clean_Architecture" => {
            "- Upfront Blueprint: Entry Dispatcher (< 100 LOC) -> `domain/` models/traits (< 200 LOC) -> `usecase/` business logic (< 250 LOC) -> `infrastructure/` (< 250 LOC)."
        }
        "Layered_Architecture" => {
            "- Upfront Blueprint: Dispatcher (< 100 LOC) -> `controllers/` (< 200 LOC) -> `services/` (< 250 LOC) -> `models/` (< 150 LOC)."
        }
        "Package_By_Feature" => {
            "- Upfront Blueprint: Feature Entry (< 100 LOC) -> feature-specific handler (< 200 LOC) -> feature types (< 150 LOC)."
        }
        "CLI_Pipeline" => {
            "- Upfront Blueprint: CLI entrypoint main (< 80 LOC) -> `commands/` sub-handlers (< 200 LOC) -> core execution engine (< 250 LOC)."
        }
        "Flat_Library" => {
            "- Upfront Blueprint: Public API facade (< 120 LOC) -> focused internal modules (< 250 LOC each)."
        }
        _ => {
            "- Upfront Blueprint: Thin main dispatcher (< 100 LOC) -> dedicated feature sub-modules (< 250 LOC each)."
        }
    };

    format!(
        "# Pre-Code Verification Checklist\n\n- Primary Language: {}\n- Architecture Pattern: {}\n\n1. **Upfront Architecture & 300 LOC Cap (Mandatory)**:\n   {}\n   - *Hard Rule*: Do NOT wait for files to reach 300 LOC to refactor. Create sub-modules from line 1.\n2. **Language Rules**:\n   {}\n3. **Symbol & API Grounding**:\n   - Verify symbol signatures using `project_context(operation=\"symbols\")` or search before modifying callers.\n4. **Error Handling Integrity**:\n   - Preserve existing error boundaries. Never use unwrap() or empty catch blocks in production paths.",
        primary_lang, active_arch, arch_blueprint, lang_rules
    )
}
