//! Dynamic Context-Aware Rule Generator
//!
//! Integrates Karpathy's 4 core engineering principles (Think Before Coding, Simplicity First,
//! Surgical Changes, Goal-Driven Execution) alongside MCP architectural mandates.
//! Injects strictly relevant, targeted rules per lifecycle phase to prevent token bloat and agent confusion.

/// Returns targeted execution rules tailored to the active workflow phase.
pub fn get_phase_rules(phase: &str) -> &'static str {
    match phase.trim().to_lowercase().as_str() {
        "context" | "search" | "read" | "explore" => {
            "## 🧭 Exploration & Context Mandates (Karpathy: Think Before Coding)\n\
            1. **No Silent Assumptions**: Verify codebase facts via `project_context` before touching code. If uncertain or ambiguous, surface tradeoffs and ask rather than guess.\n\
            2. **Token-Bounded Reads**: Use `target_symbol` or AST skeleton mode (max 300 lines per read).\n\
            3. **No Direct Raw FS**: Route file access through MCP tools rather than raw filesystem dumps."
        }
        "plan" | "architecture" | "design" | "init" => {
            "## 📐 Planning & Architecture Mandates (Karpathy: Simplicity First & Goal-Driven)\n\
            1. **Simplicity First**: Design the minimum code that solves the problem. No speculative abstractions, unrequested flexibility, or unused configurability.\n\
            2. **Upfront Architecture & 300 LOC Hard Cap**: All new and modified files MUST remain < 300 LOC (aim for < 150 LOC per sub-module). Plan decomposition of complex views/services from line 1.\n\
            3. **Goal-Driven Milestones**: Define concrete verifiable criteria for each step (`1. [Step] → verify: [check]`)."
        }
        "build" | "skills" | "implement" | "code" => {
            "## 🛡️ Code Construction Mandates (Karpathy: Surgical Changes & Simplicity First)\n\
            1. **Surgical Changes**: Touch only what you must. Every changed line must trace directly to the request. Do NOT refactor unbroken code or modify unrelated formatting/comments. Clean up your own orphans only.\n\
            2. **Per-File Edit Authorization & 300 LOC Hard-Block**: Must call `workflow_gate(action=\"authorize_edit\", relative_path=\"<exact_path>\")` individually for EACH file before modifying or creating it. Files >= 300 LOC are strictly blocked from adding new code; new files must remain < 300 LOC.\n\
            3. **Error Boundaries & Safety**: Never use unwrap() or empty catch blocks in production paths. Preserve existing error boundaries."
        }
        "test" | "verify" | "review" | "test_recheck" => {
            "## 🧪 Verification & Empirical Testing Mandates (Karpathy: Goal-Driven Execution)\n\
            1. **Empirical Verification**: Run real automated tests via `guidance(operation=\"verify\")` to verify success criteria before claiming completion.\n\
            2. **Zero Assumptions**: Validate edge cases and error branches directly from test output logs.\n\
            3. **Documentation Integrity**: Keep comments and docs synchronized with code modifications."
        }
        _ => {
            "## 🛡️ Core Execution Mandates (Karpathy-Aligned)\n\
            1. **Think Before Coding**: Inspect symbols via `project_context` before code modifications.\n\
            2. **Surgical Changes & Per-File Gating**: Call `workflow_gate(action=\"authorize_edit\", relative_path=\"<file>\")` before modifying/creating each file.\n\
            3. **300 LOC Hard Cap & Simplicity**: Enforce modular decomposition from line 1 with minimal necessary code (< 300 LOC per file)."
        }
    }
}

/// Formats the targeted rules when skills are selected/loaded.
pub fn format_skill_load_rules() -> &'static str {
    get_phase_rules("build")
}
