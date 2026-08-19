use std::path::Path;
use crate::catalog::store::SkillItem;
use crate::context::db::CodeGraphDb;

/// Generates an upfront architectural split blueprint based on the active repository code graph.
/// Analyzes existing file sizes to detect candidates for splitting (>= 200 LOC) and proposes
/// target sub-module structures aligned with the detected architectural pattern.
pub fn generate_dynamic_blueprint(proj_path: &Path, task: &str, arch_pattern: &str) -> String {
    let mut large_files = Vec::new();

    let db_path = proj_path.join(".agent-context").join("code_graph.db");
    if db_path.exists() {
        if let Ok(db) = CodeGraphDb::open_read_only(&db_path) {
            // Extract keywords from task for relevance lookup
            let words: Vec<&str> = task
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();

            for word in words {
                if let Ok(syms) = db.search_symbols(word, 5) {
                    for (file_path, sym_name, _line) in syms {
                        let full = proj_path.join(&file_path);
                        if let Ok(content) = std::fs::read_to_string(&full) {
                            let loc = content.lines().count();
                            if loc >= 200 && !large_files.iter().any(|(p, _, _)| p == &file_path) {
                                large_files.push((file_path, sym_name, loc));
                            }
                        }
                    }
                }
            }
        }
    }

    let mut blueprint = String::new();

    if !large_files.is_empty() {
        blueprint.push_str("### ⚠️ Monolithic File Split Alerts (LOC >= 200):\n");
        for (file, sym, loc) in &large_files {
            blueprint.push_str(&format!(
                "- `{}` ({} LOC — near/over limit) containing `{}`\n",
                file, loc, sym
            ));
            blueprint.push_str(&format!(
                "  ↳ **Decomposition Plan**: Extract handler/service logic into dedicated sub-module files (< 150 LOC each) before adding new code.\n"
            ));
        }
        blueprint.push_str("\n");
    }

    blueprint.push_str(&format!("### 📐 Upfront Modular Blueprint for [{}]:\n", arch_pattern));
    match arch_pattern {
        "Clean_Architecture" => {
            blueprint.push_str("- `domain/`: Pure data models and trait definitions (< 150 LOC)\n");
            blueprint.push_str("- `usecase/`: Business application logic and workflow engines (< 200 LOC)\n");
            blueprint.push_str("- `infrastructure/`: Database, external API, and driver implementations (< 200 LOC)\n");
            blueprint.push_str("- `entrypoint/`: Thin dispatch controller (< 80 LOC)\n");
        }
        "Layered_Architecture" => {
            blueprint.push_str("- `controllers/`: Input parsing, parameter validation, and routing (< 100 LOC)\n");
            blueprint.push_str("- `services/`: Core business workflows and orchestration (< 200 LOC)\n");
            blueprint.push_str("- `models/`: Data structures, schemas, and queries (< 150 LOC)\n");
        }
        "CLI_Pipeline" => {
            blueprint.push_str("- `main.rs`: Argument parsing & sub-command dispatcher (< 80 LOC)\n");
            blueprint.push_str("- `commands/`: Dedicated handler per sub-command (< 150 LOC each)\n");
            blueprint.push_str("- `core/`: Pure business engine and processing pipelines (< 200 LOC)\n");
        }
        "Package_By_Feature" => {
            blueprint.push_str("- `<feature>/handlers.rs`: Request handling and flow control (< 120 LOC)\n");
            blueprint.push_str("- `<feature>/service.rs`: Feature business logic (< 180 LOC)\n");
            blueprint.push_str("- `<feature>/types.rs`: Feature-scoped models and errors (< 100 LOC)\n");
        }
        "Flat_Library" => {
            blueprint.push_str("- `lib.rs`: Public API facade and re-exports (< 100 LOC)\n");
            blueprint.push_str("- `internal/`: Focused single-responsibility sub-modules (< 200 LOC each)\n");
        }
        _ => {
            blueprint.push_str("- Entry Dispatcher: Thin main coordinator (< 100 LOC)\n");
            blueprint.push_str("- Feature Modules: Cohesive sub-modules (< 200 LOC each)\n");
        }
    }

    blueprint
}

/// Synthesizes top recommended skills into an actionable, unified step-by-step checklist.
pub fn generate_skill_recipe(skills: &[(f32, SkillItem)], task: &str) -> String {
    if skills.is_empty() {
        return "1. Review project standards and verify architectural boundaries.\n2. Implement targeted modifications respecting the 300 LOC limit.\n3. Run automated tests to verify behavior.".to_string();
    }

    let top_skills: Vec<&SkillItem> = skills.iter().take(3).map(|(_, item)| item).collect();
    let mut checklist = Vec::new();

    checklist.push(format!(
        "1. **Pre-Implementation**: Align task '{}' with patterns from `{}`.",
        task, top_skills[0].name
    ));

    if top_skills.len() > 1 {
        checklist.push(format!(
            "2. **Core Logic**: Apply safety and domain rules from `{}`.",
            top_skills[1].name
        ));
    }

    if top_skills.len() > 2 {
        checklist.push(format!(
            "3. **Quality & Boundary**: Enforce testing and separation guidelines from `{}`.",
            top_skills[2].name
        ));
    }

    checklist.push("4. **Verification**: Confirm changes pass empirical tests before advancing to Proposal stage.".to_string());

    checklist.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::store::SkillSource;

    #[test]
    fn test_generate_dynamic_blueprint_patterns() {
        let path = Path::new(".");
        let bp_clean = generate_dynamic_blueprint(path, "manage inventory", "Clean_Architecture");
        assert!(bp_clean.contains("Clean_Architecture"));
        assert!(bp_clean.contains("domain/"));
        assert!(bp_clean.contains("usecase/"));

        let bp_cli = generate_dynamic_blueprint(path, "cli parser", "CLI_Pipeline");
        assert!(bp_cli.contains("CLI_Pipeline"));
        assert!(bp_cli.contains("main.rs"));
        assert!(bp_cli.contains("commands/"));
    }

    #[test]
    fn test_generate_skill_recipe_steps() {
        let skills = vec![
            (0.9, SkillItem {
                name: "rust-patterns".to_string(),
                relative_path: "skills/rust.md".to_string(),
                source: SkillSource::Embedded,
                content: String::new(),
            }),
            (0.8, SkillItem {
                name: "security-audit".to_string(),
                relative_path: "skills/security.md".to_string(),
                source: SkillSource::Embedded,
                content: String::new(),
            }),
        ];

        let recipe = generate_skill_recipe(&skills, "refactor auth");
        assert!(recipe.contains("Pre-Implementation"));
        assert!(recipe.contains("rust-patterns"));
        assert!(recipe.contains("Core Logic"));
        assert!(recipe.contains("security-audit"));
        assert!(recipe.contains("Verification"));
    }
}
