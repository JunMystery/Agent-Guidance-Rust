use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tracing::info;

use crate::catalog::store::{
    SkillSource, get_embedded_skill, list_embedded_skills, load_all_skills,
};
use crate::context::cache::project_snapshot;
use crate::context::db::CodeGraphDb;
use crate::context::indexer::IncrementalIndexer;
use crate::context::scanner::scan_project;
use crate::context::watcher::{is_watching, start_watching};
use crate::mcp::state::ServerState;
use crate::ml::embeddings::hybrid_vector_search;
use crate::ml::llm_selector::LLMSelector;
use crate::optimizer::compressor::{compress_markdown, estimate_tokens};

fn ensure_not_cancelled(state: &ServerState) -> Result<(), (i32, String)> {
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

fn detect_parent_process_cwd() -> Option<PathBuf> {
    use sysinfo::{Pid, System};
    let mut sys = System::new_all();
    sys.refresh_all();

    let my_pid = Pid::from_u32(std::process::id());
    if let Some(proc_) = sys.process(my_pid) {
        if let Some(parent_pid) = proc_.parent() {
            if let Some(parent_proc) = sys.process(parent_pid) {
                if let Some(cwd) = parent_proc.cwd() {
                    if cwd.is_dir() && !cwd.to_string_lossy().to_lowercase().contains("antigravity")
                    {
                        return Some(cwd.to_path_buf());
                    }
                }
            }
        }
    }
    None
}

fn is_generic_home_dir(p: &Path) -> bool {
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
        // Check active state memory first, then disk / heuristics
        if let Some(ref memorized) = state.active_architecture_pattern {
            if !memorized.is_empty() {
                return memorized.clone();
            }
        }
        detect_project_architecture(proj_path)
    } else {
        trimmed.to_string()
    }
}

fn detect_project_path(arg_path: &str, state: &ServerState) -> PathBuf {
    // 1. Explicit path parameter provided in tool call (Agent declared working dir)
    if !arg_path.is_empty() && arg_path != "." {
        let p = PathBuf::from(arg_path);
        if p.is_dir() {
            return p;
        }
    }

    // 2. Previously recorded active project path in ServerState memory
    if let Some(ref recorded) = state.project_path {
        let p = PathBuf::from(recorded);
        if p.is_dir() && !is_generic_home_dir(&p) {
            return p;
        }
    }

    // 3. Active workspace roots set during MCP initialize
    if !state.workspace_roots.is_empty() {
        let p = PathBuf::from(&state.workspace_roots[0]);
        if p.is_dir() && !is_generic_home_dir(&p) {
            return p;
        }
    }

    // 4. Environment Variables injected by IDEs (INIT_CWD, WORKSPACE_FOLDER, PROJECT_DIR)
    for env_var in ["INIT_CWD", "WORKSPACE_FOLDER", "PROJECT_DIR"] {
        if let Ok(val) = std::env::var(env_var) {
            let p = PathBuf::from(val);
            if p.is_dir()
                && !p.to_string_lossy().to_lowercase().contains("antigravity")
                && !is_generic_home_dir(&p)
            {
                return p;
            }
        }
    }

    // 5. Parent Process PID CWD Inspection (IDE/Terminal parent directory)
    if let Some(parent_cwd) = detect_parent_process_cwd() {
        if !is_generic_home_dir(&parent_cwd) {
            return parent_cwd;
        }
    }

    // 6. Process current working directory (if not Antigravity installation root or generic home)
    if let Ok(cwd) = std::env::current_dir() {
        if !cwd.to_string_lossy().to_lowercase().contains("antigravity")
            && !is_generic_home_dir(&cwd)
        {
            return cwd;
        }
    }

    // 7. Global persistent project path from prior agent session (fallback if process CWD unavailable)
    if let Some(global_path) = ServerState::read_global_project_path() {
        let p = PathBuf::from(global_path);
        if p.is_dir() && !is_generic_home_dir(&p) {
            return p;
        }
    }

    // 8. If recorded path exists even if home dir, use it
    if let Some(ref recorded) = state.project_path {
        return PathBuf::from(recorded);
    }

    PathBuf::from(".")
}

fn ensure_indexed(proj_path: &Path) -> Option<CodeGraphDb> {
    let db = CodeGraphDb::open_for_project(proj_path).ok()?;
    let file_count = db.file_count().unwrap_or(0);

    if file_count == 0 {
        // First time: run full index immediately for current project
        if let Ok(mut indexer) = IncrementalIndexer::new(proj_path) {
            let _ = indexer.full_index();
        }

        // Spawn background embedding & start watcher
        let path = proj_path.to_path_buf();
        std::thread::spawn(move || {
            if let Ok(idx) = IncrementalIndexer::new(&path) {
                let _ = idx.embed_symbols();
                let _ = idx.embed_chunks();
            }
        });
        start_watching(proj_path);
    } else if !is_watching(proj_path) {
        start_watching(proj_path);
        let path = proj_path.to_path_buf();
        std::thread::spawn(move || {
            if let Ok(mut idx) = IncrementalIndexer::new(&path) {
                let _ = idx.incremental_index();
                let _ = idx.embed_symbols();
                let _ = idx.embed_chunks();
            }
        });
    }

    let _ = db.decay_aliases();
    Some(db)
}

fn embed_query(query: &str) -> Option<Vec<f32>> {
    if let Ok(model) = crate::ml::embeddings::EmbeddingModel::load_or_download() {
        model.embed_text(query, Some("query")).ok()
    } else {
        None
    }
}


pub fn handle_tool_call(
    name: &str,
    arguments: Value,
    state: &mut ServerState,
) -> Result<Value, (i32, String)> {
    info!("Handling tool call: {}", name);
    ensure_not_cancelled(state)?;

    let start_time = std::time::Instant::now();
    let op = arguments
        .get("operation")
        .or_else(|| arguments.get("action"))
        .or_else(|| arguments.get("phase"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let res = match handle_tool_call_internal(name, arguments, state) {
        Ok(mut val) => {
            let duration = start_time.elapsed().as_millis() as u64;

            // Universal token optimization & compression across all MCP tool responses
            let opt_enabled = std::env::var("AGENT_GUIDANCE_TOKEN_OPT")
                .map(|v| v != "0")
                .unwrap_or(true);

            let mut orig_tokens = 0;
            let mut opt_tokens = 0;

            if let Some(content_arr) = val.get_mut("content").and_then(|c| c.as_array_mut()) {
                for item in content_arr.iter_mut() {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        let orig = estimate_tokens(text, false) as u64;
                        orig_tokens += orig;
                        if opt_enabled {
                            let compressed = compress_markdown(text);
                            let opt = estimate_tokens(&compressed, false) as u64;
                            opt_tokens += opt;
                            if let Some(obj) = item.as_object_mut() {
                                obj.insert("text".to_string(), Value::String(compressed));
                            }
                        } else {
                            opt_tokens += orig;
                        }
                    }
                }
            }

            state.record_call(orig_tokens, opt_tokens);
            crate::mcp::db::log_tool_call(name, op.as_deref(), orig_tokens, opt_tokens, duration, None);
            Ok(val)
        }
        Err(err) => {
            let duration = start_time.elapsed().as_millis() as u64;
            crate::mcp::db::log_tool_call(name, op.as_deref(), 0, 0, duration, Some(&err.1));
            Err(err)
        }
    };
    res
}

fn handle_tool_call_internal(
    name: &str,
    arguments: Value,
    state: &mut ServerState,
) -> Result<Value, (i32, String)> {
    let response_text = match name {
        "task_pipeline" => {
            let raw_task = arguments
                .get("task")
                .and_then(|t| t.as_str())
                .unwrap_or("general task");
            let task = if raw_task.trim().is_empty() {
                "general task"
            } else {
                raw_task.trim()
            };
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            state.update_project_path(&proj_path);
            let phase = arguments
                .get("phase")
                .and_then(|p| p.as_str())
                .unwrap_or("plan");

            // Record active phase and auto-reset approval state when starting a new planning phase
            state.active_phase = Some(phase.to_string());
            if phase == "plan" {
                state.workflow_stage = "Plan".to_string();
                state.plan_approved = false;
                state.edit_authorized = false;
                state.verification_passed = false;
                state.verification_command = None;
                state.expected_output_keyword = None;
                let _ = state.save_to_dir(&proj_path);
                tracing::info!(
                    "Reset workflow stage to 'Plan' and plan_approved to false for new task pipeline execution."
                );
            }

            let focus = arguments
                .get("focus")
                .and_then(|f| f.as_str())
                .map(|f| f.trim())
                .filter(|f| !f.is_empty() && *f != "general");
            let search_query = if let Some(f) = focus {
                format!("{} {}", task, f)
            } else {
                task.to_string()
            };

            let snapshot = project_snapshot(&proj_path);
            ensure_not_cancelled(state)?;
            let file_count = snapshot.files.len();
            let profile = crate::catalog::language_detector::detect_language_profile(
                snapshot.files.as_ref(),
                &search_query,
            );

            let stage1_results = hybrid_vector_search(&search_query, snapshot.skills.as_ref(), 16);
            ensure_not_cancelled(state)?;
            let selector = LLMSelector::new();
            let final_results = selector.rerank(&search_query, stage1_results, &profile, 16);
            ensure_not_cancelled(state)?;

            let mut seen_names = std::collections::HashSet::new();
            let mut deduped_results = Vec::new();
            for (score, item) in final_results {
                if seen_names.insert(item.name.clone()) {
                    deduped_results.push((score, item));
                    if deduped_results.len() >= 8 {
                        break;
                    }
                }
            }

            state.pending_skill_proposals = deduped_results
                .iter()
                .map(|(score, item)| (item.name.clone(), item.relative_path.clone(), *score))
                .collect();

            let rec_skills: Vec<String> = deduped_results
                .iter()
                .map(|(score, item)| format!("- {} (Score: {:.2})", item.name, score))
                .collect();

            let execution_seq = "- Step 1: Context & Specification\n- Step 2: Architecture & Implementation Plan\n- Step 3: Code Implementation (Build stage)\n- Step 4: Verification & Testing\n- Step 5: Post-Code Review & Documentation";
            let tree_preview: Vec<String> = snapshot
                .files
                .iter()
                .take(15)
                .map(|f| format!("- {} ({})", f.path, f.file_type))
                .collect();

            let next_step_prompt = if rec_skills.is_empty() {
                "-> CODEBASE EXPLORATION: Use `project_context(operation=\"search\", query=\"...\")` to locate code and `project_context(operation=\"read\", relative_path=\"...\")` to inspect functions. Do NOT use raw grep_search, list_dir, or view_file."
            } else {
                "-> SKILL_PROPOSAL: Trigger IDE/CLI `ask_question` tool to present recommended skills interactively to user, then call `select_skills(skills=[...])` with chosen skills (or `select_skills(skills=[])` if skipped).\n-> MANDATORY NEXT STEP: Use `project_context(operation=\"search\", query=\"...\")` to find files and `project_context(operation=\"read\", relative_path=\"...\")` to inspect code. Avoid raw filesystem tools."
            };

            let detected_arch = detect_project_architecture(&proj_path);
            state.active_architecture_pattern = Some(detected_arch.clone());

            let core_rules_checklist = "## Mandatory Agent Execution Mandates (9 Core Rules)\n1. **Context First**: Always run `task_pipeline` or `project_context` before reading files or modifying code.\n2. **Fast Edit Authorization**: Must call `workflow_gate(action=\"authorize_edit\")` before editing.\n3. **Token Budget**: Max 300 lines per read, use symbol extraction over full-file dumps.\n4. **No Direct FS**: Prioritize MCP tools over raw filesystem access.\n5. **Ground & Plan**: Verify codebase facts via search before proposing changes.\n6. **Upfront Architecture & 300 LOC Cap**: Enforce 300 LOC limit from line 1. Split entry dispatchers from sub-module handlers upfront.\n7. **Intent Gate**: Classify request type before acting.\n8. **Delegation First**: Decompose and delegate multi-step tasks when applicable.\n9. **Phase Progression**: Complete Context -> Plan -> Build -> Test -> Review sequence.";

            let dynamic_blueprint = crate::catalog::blueprint::generate_dynamic_blueprint(&proj_path, task, &detected_arch);
            let skill_recipe = crate::catalog::blueprint::generate_skill_recipe(&deduped_results, task);
            let relevant_learnings = crate::mcp::learnings::get_semantic_relevant_learnings(&proj_path, task, 3, 0.82);

            let learnings_section = if relevant_learnings.is_empty() {
                String::new()
            } else {
                format!("\n\n## 💡 Project Memorized Learnings\n{}", relevant_learnings.join("\n"))
            };

            state.record_call(1500, 450);
            format!(
                "# Task Pipeline Activated\n\nTask: {}\nActive Phase: {}\nProject: {}\n\n## Recommendations\n{}{}\n\n## 🍳 Task-Specific Skill Recipe\n{}\n\n## 📐 Dynamic Split Blueprint\n{}\n\n## Architecture Guidance\n- Active Pattern: {}\n- Enforce: Create thin dispatcher main + sub-module files from line 1 (Upfront Architecture, 300 LOC Cap)\n\n{}\n\n## Execution Sequence\n{}\n\n## Project Tree (Scanned Files: {})\n{}\n\nPriority Gate: PASSED\nStatus: Ready for execution.\n\n{}",
                task,
                phase,
                proj_path.display(),
                if rec_skills.is_empty() {
                    "No specific skill recommendations required for this task (Token budget saved)."
                        .to_string()
                } else {
                    rec_skills.join("\n")
                },
                learnings_section,
                skill_recipe,
                dynamic_blueprint,
                detected_arch,
                core_rules_checklist,
                execution_seq,
                file_count,
                tree_preview.join("\n"),
                next_step_prompt
            )
        }
        "select_skills" => {
            let requested_skills: Vec<String> = arguments
                .get("skills")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let proposals = std::mem::take(&mut state.pending_skill_proposals);
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);

            let task_arg = arguments
                .get("task")
                .and_then(|t| t.as_str())
                .unwrap_or("");

            if requested_skills.is_empty() {
                state.record_call(100, 50);
                "# Skill Selection\n\nNo skills selected. Proceeding without loading skills.\n\n-> MANDATORY NEXT STEP: Use `project_context(operation=\"search\", query=\"...\")` to search keywords/symbols or `project_context(operation=\"read\", relative_path=\"...\", target_symbol=\"...\")` to inspect code."
                    .to_string()
            } else {
                let mut loaded_sections = Vec::new();
                let mut not_found = Vec::new();

                for name in &requested_skills {
                    if let Some((_, rel_path, _)) = proposals.iter().find(|(n, _, _)| n == name) {
                        crate::mcp::db::log_skill_load(name);

                        let raw_content = if let Some(c) = get_embedded_skill(name) {
                            Some(c)
                        } else if let Some(c) = get_embedded_skill(rel_path) {
                            Some(c)
                        } else if let Ok(c) = std::fs::read_to_string(name) {
                            Some(c)
                        } else if let Ok(c) = std::fs::read_to_string(rel_path) {
                            Some(c)
                        } else if let Ok(full_path) = validate_path(&proj_path, rel_path) {
                            std::fs::read_to_string(&full_path).ok()
                        } else {
                            None
                        };

                        if let Some(content) = raw_content {
                            let processed = if !task_arg.is_empty() {
                                crate::catalog::slicing::slice_skill_markdown(&content, task_arg, 3)
                            } else {
                                compress_markdown(&content)
                            };
                            loaded_sections.push(format!(
                                "### Skill: {}\n```markdown\n{}\n```",
                                name, processed
                            ));
                        } else {
                            loaded_sections.push(format!(
                                "### Skill: {} (Loaded & Logged)\n*Content empty or unavailable*",
                                name
                            ));
                        }
                    } else if let Some(c) = get_embedded_skill(name) {
                        crate::mcp::db::log_skill_load(name);
                        let processed = if !task_arg.is_empty() {
                            crate::catalog::slicing::slice_skill_markdown(&c, task_arg, 3)
                        } else {
                            compress_markdown(&c)
                        };
                        loaded_sections.push(format!(
                            "### Skill: {} [Embedded Catalog]\n```markdown\n{}\n```",
                            name, processed
                        ));
                    } else if let Ok(full_path) = validate_path(&proj_path, name) {
                        if let Ok(c) = std::fs::read_to_string(&full_path) {
                            crate::mcp::db::log_skill_load(name);
                            let processed = if !task_arg.is_empty() {
                                crate::catalog::slicing::slice_skill_markdown(&c, task_arg, 3)
                            } else {
                                compress_markdown(&c)
                            };
                            loaded_sections.push(format!(
                                "### Skill: {} [Local Workspace]\n```markdown\n{}\n```",
                                name, processed
                            ));
                        } else {
                            not_found.push(name.clone());
                        }
                    } else {
                        not_found.push(name.clone());
                    }
                }

                let snapshot = project_snapshot(&proj_path);
                let profile = crate::catalog::language_detector::detect_language_profile(
                    snapshot.files.as_ref(),
                    task_arg,
                );
                let safety_rules = crate::catalog::slicing::get_language_safety_rules(&profile);

                state.record_call(1500, 500);
                let mut resp = format!(
                    "# Skill Selection Confirmed ({})\n\nLoaded Skills Content:\n\n{}\n\n## 🛡️ Language Safety Rules\n{}",
                    loaded_sections.len(),
                    loaded_sections.join("\n\n---\n\n"),
                    safety_rules
                );

                if !not_found.is_empty() {
                    resp.push_str(&format!(
                        "\n\n⚠️ Skills not found:\n{}",
                        not_found.iter().map(|n| format!("- {}", n)).collect::<Vec<_>>().join("\n")
                    ));
                }

                resp.push_str("\n\n-> MANDATORY NEXT STEP: Use `project_context(operation=\"search\", query=\"...\")` to locate functions/symbols or `project_context(operation=\"read\", relative_path=\"...\")` to view source code.");
                resp
            }
        }
        "guidance" => {
            let op = arguments
                .get("operation")
                .and_then(|o| o.as_str())
                .unwrap_or("list");
            let query = arguments
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("")
                .to_lowercase();
            state.record_call(1000, 300);

            match op {
                "list" => {
                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    let snapshot = project_snapshot(&proj_path);
                    let names: Vec<String> = snapshot
                        .skills
                        .iter()
                        .map(|s| format!("- {} ({})", s.name, s.relative_path))
                        .collect();
                    format!(
                        "# Registered Skills Catalog ({})\n\n{}",
                        names.len(),
                        names.join("\n")
                    )
                }
                "get" => {
                    let id = arguments
                        .get("identifier")
                        .and_then(|i| i.as_str())
                        .unwrap_or("");
                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    if !id.is_empty() {
                        crate::mcp::db::log_skill_load(id);
                    }
                    if let Some(content) = get_embedded_skill(id) {
                        compress_markdown(&content)
                    } else if let Ok(content) = std::fs::read_to_string(id) {
                        compress_markdown(&content)
                    } else if let Ok(full_path) = validate_path(&proj_path, id) {
                        if let Ok(content) = std::fs::read_to_string(&full_path) {
                            compress_markdown(&content)
                        } else {
                            format!("Skill asset not found: {}", id)
                        }
                    } else {
                        format!("Skill asset not found: {}", id)
                    }
                }
                "search" => {
                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    let snapshot = project_snapshot(&proj_path);
                    ensure_not_cancelled(state)?;
                    let profile = crate::catalog::language_detector::detect_language_profile(
                        snapshot.files.as_ref(),
                        &query,
                    );
                    let all_skills = snapshot.skills.as_ref();

                    // Stage 1: 1st Stage Candidate Selection
                    let stage1_results = hybrid_vector_search(&query, all_skills, 20);
                    ensure_not_cancelled(state)?;

                    // Stage 2: 2nd Stage Context & Intent Re-ranking
                    let selector = LLMSelector::new();
                    let final_results = selector.rerank(&query, stage1_results, &profile, 20);
                    ensure_not_cancelled(state)?;

                    let mut seen_names = std::collections::HashSet::new();
                    let mut deduped_results = Vec::new();
                    for (score, item) in final_results {
                        if seen_names.insert(item.name.clone()) {
                            deduped_results.push((score, item));
                            if deduped_results.len() >= 15 {
                                break;
                            }
                        }
                    }

                    if state.pending_skill_proposals.is_empty() {
                        state.pending_skill_proposals = deduped_results
                            .iter()
                            .map(|(score, item)| {
                                (item.name.clone(), item.relative_path.clone(), *score)
                            })
                            .collect();
                    }

                    let formatted_results: Vec<String> = deduped_results
                        .into_iter()
                        .map(|(score, item)| {
                            let source_tag = match &item.source {
                                SkillSource::Embedded => "[Embedded]".to_string(),
                                SkillSource::LocalWorkspace(path) => {
                                    format!("[Local Workspace: {}]", path)
                                }
                            };
                            format!(
                                "- {} {} (Score: {:.2})\n  Path: {}",
                                item.name, source_tag, score, item.relative_path
                            )
                        })
                        .collect();

                    let next_step_prompt = if formatted_results.is_empty() {
                        "-> No matching skills found."
                    } else {
                        "-> SKILL_PROPOSAL: Trigger IDE/CLI `ask_question` tool to present recommended skills interactively to user, then call `select_skills(skills=[...])` with chosen skills (or `select_skills(skills=[])` if skipped)."
                    };

                    format!(
                        "# 2-Stage Skill Search Results for '{}'\n\nStage 1 (Candle BERT Vector Cosine Similarity) -> Stage 2 (Cross-Encoder Re-ranking)\nMatches Found: {}\n\nRecommended Skills:\n{}\n\n{}",
                        query,
                        formatted_results.len(),
                        if formatted_results.is_empty() {
                            "No matching skills found.".to_string()
                        } else {
                            formatted_results.join("\n")
                        },
                        next_step_prompt
                    )
                }
                "ui_ux" => {
                    let q = if query.is_empty() { "general" } else { &query };
                    format!(
                        "# UI/UX Guidelines for '{}'\n\n- Styling: Modern CSS, Glassmorphism, Dynamic Animations\n- Color Palette: Dark mode default, curated HSL gradients\n- Typography: Inter/Outfit via Google Fonts\n- Accessibility: Semantic HTML5, unique IDs",
                        q
                    )
                }
                "docs" => {
                    let id = arguments
                        .get("identifier")
                        .and_then(|i| i.as_str())
                        .unwrap_or("general");

                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    let snapshot = project_snapshot(&proj_path);
                    let search_term = if !query.is_empty() { &query } else { id };

                    let stage1 = hybrid_vector_search(search_term, snapshot.skills.as_ref(), 5);
                    let selector = LLMSelector::new();
                    let profile = crate::catalog::language_detector::detect_language_profile(
                        snapshot.files.as_ref(),
                        search_term,
                    );
                    let reranked = selector.rerank(search_term, stage1, &profile, 3);

                    if reranked.is_empty() {
                        format!(
                            "# Documentation Guidance for '{}' ({})\n\nNo matching documentation skills found in catalog for search term: '{}'.",
                            id, query, search_term
                        )
                    } else {
                        let mut docs_sections = Vec::new();
                        for (score, item) in reranked {
                            if let Some(content) = get_embedded_skill(&item.relative_path) {
                                docs_sections.push(format!(
                                    "### Doc Skill: {} (Score: {:.2})\nPath: {}\n\n{}",
                                    item.name,
                                    score,
                                    item.relative_path,
                                    compress_markdown(&content)
                                ));
                            }
                        }
                        format!(
                            "# Documentation Guidance for '{}'\n\nQuery: '{}'\n\n{}",
                            id,
                            search_term,
                            docs_sections.join("\n\n---\n\n")
                        )
                    }
                }
                "workflow" => {
                    let stage = arguments
                        .get("identifier")
                        .and_then(|i| i.as_str())
                        .unwrap_or("plan")
                        .to_lowercase();

                    let ref_asset = format!("workflow-modes/references/workflow-{}.md", stage);
                    let alt_asset = format!("workflow-modes/references/{}.md", stage);
                    let skill_asset = format!("skills/{}/SKILL.md", stage);

                    if let Some(content) = get_embedded_skill(&ref_asset) {
                        compress_markdown(&content)
                    } else if let Some(content) = get_embedded_skill(&alt_asset) {
                        compress_markdown(&content)
                    } else if let Some(content) = get_embedded_skill(&skill_asset) {
                        compress_markdown(&content)
                    } else {
                        format!(
                            "# Dev Workflow Guidance: [{}]\n\nRecommended Flow: Context -> Plan -> Ask/Revise -> Build -> Test/Recheck -> Fix -> Document",
                            stage
                        )
                    }
                }
                "precode" => {
                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    let snapshot = project_snapshot(&proj_path);
                    let profile = crate::catalog::language_detector::detect_language_profile(
                        snapshot.files.as_ref(),
                        &query,
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
                "verify" => {
                    let v_cmd = arguments
                        .get("verification_command")
                        .and_then(|v| v.as_str());
                    let v_kw = arguments
                        .get("expected_output_keyword")
                        .and_then(|k| k.as_str());

                    if let (Some(cmd), Some(kw)) = (v_cmd, v_kw) {
                        state.verification_command = Some(cmd.to_string());
                        state.expected_output_keyword = Some(kw.to_string());
                        state.verification_passed = false; // SECURITY FIX Bug #4: Registered contract; awaiting test execution
                        format!(
                            "# Empirical Verification Contract Registered\n\n- Verification Command: `{}`\n- Expected Output Keyword: `{}`\n- Verification Status: REGISTERED (Awaiting test execution output)\n\n✓ Run verification command to satisfy anti-hallucination requirement.",
                            cmd, kw
                        )
                    } else {
                        format!(
                            "# Anti-Hallucination Post-Code Verification Checklist\n\n1. **Empirical Verification Required**: Trigger IDE/CLI `ask_question` tool to let user select verification test command (or confirm manual testing), then pass `verification_command` (e.g. 'cargo test') and `expected_output_keyword` (e.g. 'PASSED').\n2. **User Requirement Alignment**: Re-read the original user prompt and verify all explicitly requested features exist.\n3. **Zero Unverified Assumptions**: Base success strictly on empirical evidence, not speculative assumptions."
                        )
                    }
                }
                _ => format!("Guidance operation '{}' completed successfully.", op),
            }
        }
        "project_context" => {
            let op = arguments
                .get("operation")
                .and_then(|o| o.as_str())
                .unwrap_or("tree");
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            state.update_project_path(&proj_path);
            let query = arguments
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("");
            let rel_path = arguments
                .get("relative_path")
                .and_then(|r| r.as_str())
                .unwrap_or("");

            match op {
                "tree" => {
                    let files = scan_project(&proj_path, 2);
                    let file_list: Vec<String> = files
                        .into_iter()
                        .map(|f| format!("- {} ({})", f.path, f.file_type))
                        .collect();
                    state.record_call(2000, 400);
                    format!(
                        "# Project Tree (Depth Capped at 2)\n\n{}",
                        file_list.join("\n")
                    )
                }
                "read" => {
                    if rel_path.is_empty() {
                        "Error: relative_path is required for read operation. Example: project_context(operation=\"read\", project_path=\"...\", relative_path=\"src/main.rs\", target_symbol=\"my_fn\")".to_string()
                    } else {
                        let target_symbol = arguments.get("target_symbol").and_then(|s| s.as_str());
                        let view_mode = arguments.get("view_mode").and_then(|v| v.as_str()).unwrap_or("auto");
                        match validate_path(&proj_path, rel_path) {
                            Ok(full_path) => {
                                match std::fs::read_to_string(&full_path) {
                                    Ok(content) => {
                                        let orig_len = content.len() as u64;
                                        let lines: Vec<&str> = content.lines().collect();
                                        let total_lines = lines.len();

                                        // If explicit skeleton requested, or file is large (>300 LOC) and no target symbol requested
                                        if view_mode == "skeleton" || (total_lines > 300 && target_symbol.is_none() && view_mode != "full") {
                                            let skeleton = crate::optimizer::skeleton::generate_code_skeleton(&content, rel_path);
                                            let compressed = compress_markdown(&skeleton);
                                            state.record_call(orig_len / 4, compressed.len() as u64 / 4);

                                            format!(
                                                "# AST Structural Skeleton: `{}` (Total Lines: {})\n\n> ⚡ **Token Saver Mode**: Function bodies collapsed to line ranges.\n\n```\n{}\n```\n\n---\n💡 **Next Step**: Pass `target_symbol=\"<fn_or_struct_name>\"` to `project_context(operation=\"read\", relative_path=\"{}\")` to view complete body implementation.",
                                                rel_path,
                                                total_lines,
                                                compressed,
                                                rel_path
                                            )
                                        } else {
                                            let mut display_lines = lines;
                                            if let Some(symbol) = target_symbol {
                                                // Symbol-targeted extraction (multi-language aware)
                                                let mut matched_snippet = Vec::new();
                                                let mut capturing = false;
                                                let mut brace_count = 0;
                                                let mut has_braces = false;
                                                for line in &display_lines {
                                                    if line.contains(symbol) && !capturing {
                                                        capturing = true;
                                                    }
                                                    if capturing {
                                                        matched_snippet.push(*line);
                                                        let open_b = line.matches('{').count() as i32;
                                                        let close_b = line.matches('}').count() as i32;
                                                        if open_b > 0 {
                                                            has_braces = true;
                                                        }
                                                        brace_count += open_b - close_b;

                                                        if has_braces
                                                            && matched_snippet.len() > 1
                                                            && brace_count <= 0
                                                            && (line.contains('}')
                                                                || line.trim().is_empty())
                                                        {
                                                            break;
                                                        }
                                                        if !has_braces
                                                            && matched_snippet.len() >= 30
                                                            && line.trim().is_empty()
                                                        {
                                                            break;
                                                        }
                                                        if matched_snippet.len() >= 100 {
                                                            break;
                                                        }
                                                    }
                                                }
                                                if !matched_snippet.is_empty() {
                                                    display_lines = matched_snippet;
                                                }
                                            }

                                            let count = display_lines.len();
                                            let was_capped = count > 300;

                                            let bounded = display_lines
                                                .into_iter()
                                                .take(300)
                                                .collect::<Vec<_>>()
                                                .join("\n");
                                            let compressed = compress_markdown(&bounded);
                                            state
                                                .record_call(orig_len / 4, compressed.len() as u64 / 4);

                                            let loc_warning = if was_capped && target_symbol.is_none() {
                                                format!(
                                                    "\n\n---\n⚠️ **ARCHITECTURE MANDATE (300 LOC Cap Exceeded)**: File `{}` has **{} total lines** (capped at 300 lines).\n**MANDATORY ACTION**: Do NOT add new logic directly into this file. Decompose into sub-modules upfront (split entry dispatchers from sub-module handlers).",
                                                    rel_path, count
                                                )
                                            } else if count > 100 && target_symbol.is_none() {
                                                format!(
                                                    "\n\n---\n💡 **Token Optimization Tip**: Pass `target_symbol=\"<fn_or_struct_name>\"` in `project_context(operation=\"read\")` to extract only the target definition and save token budget."
                                                )
                                            } else {
                                                String::new()
                                            };

                                            if let Some(symbol) = target_symbol {
                                                format!(
                                                    "# Target Symbol Extracted: '{}' from {}\n\n{}{}",
                                                    symbol, rel_path, compressed, loc_warning
                                                )
                                            } else {
                                                format!(
                                                    "# Bounded File Content: {}\n\n{}{}",
                                                    rel_path, compressed, loc_warning
                                                )
                                            }
                                        }
                                    }
                                    Err(e) => format!("Failed to read file '{}': {}", rel_path, e),
                                }
                            }
                            Err(err_msg) => format!("Security Error: {}", err_msg),
                        }
                    }
                }
                "search" => {
                    if query.is_empty() {
                        "Error: query is required for search operation. Example: project_context(operation=\"search\", project_path=\"...\", query=\"search_term\")".to_string()
                    } else {
                        let db_opt = ensure_indexed(&proj_path);
                        let mut results = Vec::new();
                        let mut source = "fallback_scan";

                        if let Some(ref db) = db_opt {
                            // Phase 1: Alias Lookup (<1ms)
                            if let Ok(aliases) = db.lookup_aliases(query, 10) {
                                if !aliases.is_empty() {
                                    source = "alias_cache";
                                    for a in aliases {
                                        results.push(format!(
                                            "- {}:L{} → `{}` (confidence: {:.2}) [alias cache]",
                                            a.resolved_path,
                                            a.resolved_line.unwrap_or(1),
                                            a.resolved_symbol.as_deref().unwrap_or("—"),
                                            a.confidence
                                        ));
                                    }
                                }
                            }

                            // Phase 2: FTS5 Symbol Names (<5ms)
                            if results.is_empty() {
                                if let Ok(syms) = db.search_symbols(query, 15) {
                                    if !syms.is_empty() {
                                        source = "symbol_fts";
                                        for (path, name, line) in syms {
                                            results.push(format!("- {}:L{} → `{}` [symbol index]", path, line, name));
                                        }
                                    }
                                }
                            }

                            // Phase 3: Vector Symbol Signatures (<50ms)
                            if results.is_empty() {
                                if let Some(qv) = embed_query(query) {
                                    if let Ok(vec_syms) = db.vector_search_symbols(&qv, 10, 0.65) {
                                        if !vec_syms.is_empty() {
                                            source = "symbol_vector";
                                            for v in vec_syms {
                                                results.push(format!(
                                                    "- {}:L{} → `{}` (cosine: {:.2}) [semantic symbol]",
                                                    v.file_path, v.start_line, v.name, v.score
                                                ));
                                            }
                                        }
                                    }
                                }
                            }

                            // Phase 4: FTS5 Content Chunks (<5ms)
                            if results.is_empty() {
                                if let Ok(content_hits) = db.search_content_fts(query, 10) {
                                    if !content_hits.is_empty() {
                                        source = "content_fts";
                                        for (path, start, end, snip) in content_hits {
                                            results.push(format!(
                                                "- {}:L{}-{} → `{}` [content index]",
                                                path, start, end, snip.replace('\n', " ")
                                            ));
                                        }
                                    }
                                }
                            }

                            // Phase 5: RAG Vector Content Chunks (<100ms)
                            if results.is_empty() {
                                if let Some(qv) = embed_query(query) {
                                    if let Ok(chunks) = db.vector_search_chunks(&qv, 10, 0.60) {
                                        if !chunks.is_empty() {
                                            source = "content_vector";
                                            for c in chunks {
                                                results.push(format!(
                                                    "- {}:L{}-{} (cosine: {:.2}) [RAG content vector]",
                                                    c.file_path, c.start_line, c.end_line, c.score
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Auto-Learn: learn top result if resolved from non-alias source
                        if !results.is_empty() && source != "alias_cache" {
                            if let Some(ref db) = db_opt {
                                let first_line = &results[0];
                                if let Some(path_part) = first_line.strip_prefix("- ") {
                                    let rel = path_part.split(':').next().unwrap_or("").trim();
                                    if !rel.is_empty() {
                                        let _ = db.upsert_alias(query, rel, None, None);
                                    }
                                }
                            }
                        }

                        state.record_call(2000, 400);
                        format!(
                            "# Context Search Results for '{}'\n\nSource: {} | Cascade: [alias → sym_fts → sym_vec → content_fts → rag_vec]\n\n{}\n\n---\n💡 **Next Step**: Pass `relative_path=\"...\"` to `project_context(operation=\"read\")` to read file content.",
                            query,
                            source,
                            if results.is_empty() {
                                "No matches found across symbol, full-text, and RAG semantic index.".to_string()
                            } else {
                                results.join("\n")
                            }
                        )
                    }
                }
                "navigate" => {
                    if query.is_empty() {
                        "Error: query is required for navigate operation.".to_string()
                    } else {
                        let db_opt = ensure_indexed(&proj_path);
                        let scope = arguments.get("scope").and_then(|s| s.as_str()).unwrap_or("all");
                        let mut sections = Vec::new();

                        if let Some(ref db) = db_opt {
                            // 1. Alias section
                            if let Ok(aliases) = db.lookup_aliases(query, 5) {
                                if !aliases.is_empty() {
                                    let lines: Vec<String> = aliases.iter().map(|a| {
                                        format!("- {}:L{} → `{}` (confidence: {:.2})", a.resolved_path, a.resolved_line.unwrap_or(1), a.resolved_symbol.as_deref().unwrap_or("—"), a.confidence)
                                    }).collect();
                                    sections.push(format!("## From Alias Cache (Instant)\n\n{}", lines.join("\n")));
                                }
                            }

                            // 2. Symbol FTS section
                            if scope == "all" || scope == "symbols" {
                                if let Ok(syms) = db.search_symbols(query, 5) {
                                    if !syms.is_empty() {
                                        let lines: Vec<String> = syms.iter().map(|(p, n, l)| format!("- {}:L{} → `{}`", p, l, n)).collect();
                                        sections.push(format!("## From Symbol Index (FTS5)\n\n{}", lines.join("\n")));
                                    }
                                }
                            }

                            // 3. Content FTS section
                            if scope == "all" || scope == "content" {
                                if let Ok(content_hits) = db.search_content_fts(query, 5) {
                                    if !content_hits.is_empty() {
                                        let lines: Vec<String> = content_hits.iter().map(|(p, s, e, snip)| format!("- {}:L{}-{} → `{}`", p, s, e, snip.replace('\n', " "))).collect();
                                        sections.push(format!("## From Content Full-Text (FTS5)\n\n{}", lines.join("\n")));
                                    }
                                }
                            }

                            // 4. Vector Semantic section
                            if let Some(qv) = embed_query(query) {
                                if scope == "all" || scope == "symbols" {
                                    if let Ok(sym_vecs) = db.vector_search_symbols(&qv, 5, 0.65) {
                                        if !sym_vecs.is_empty() {
                                            let lines: Vec<String> = sym_vecs.iter().map(|v| format!("- {}:L{} → `{}` (cosine: {:.2})", v.file_path, v.start_line, v.name, v.score)).collect();
                                            sections.push(format!("## From Symbol Semantic Vector\n\n{}", lines.join("\n")));
                                        }
                                    }
                                }

                                if scope == "all" || scope == "content" {
                                    if let Ok(chunk_vecs) = db.vector_search_chunks(&qv, 5, 0.60) {
                                        if !chunk_vecs.is_empty() {
                                            let lines: Vec<String> = chunk_vecs.iter().map(|c| format!("- {}:L{}-{} (cosine: {:.2})", c.file_path, c.start_line, c.end_line, c.score)).collect();
                                            sections.push(format!("## From RAG Code Chunk Semantic Vector\n\n{}", lines.join("\n")));
                                        }
                                    }
                                }
                            }

                            // 5. Related Graph Edges
                            if scope == "all" || scope == "edges" {
                                if let Ok(related) = db.search_related_symbols(query) {
                                    if !related.is_empty() {
                                        let lines: Vec<String> = related.iter().map(|(s1, edge, s2)| format!("- `{}` --[{}]--> `{}`", s1, edge, s2)).collect();
                                        sections.push(format!("## Related Graph Symbols (DAG)\n\n{}", lines.join("\n")));
                                    }
                                }
                            }
                        }

                        state.record_call(3000, 500);
                        format!(
                            "# Code Graph Navigation for '{}'\n\n{}\n\n---\n💡 **Next Step**: Pass `relative_path=\"...\"` to `project_context(operation=\"read\")` to view source code.",
                            query,
                            if sections.is_empty() {
                                "No navigation nodes found.".to_string()
                            } else {
                                sections.join("\n\n")
                            }
                        )
                    }
                }
                "learn_alias" => {
                    let alias_term = arguments.get("alias_term").and_then(|a| a.as_str()).unwrap_or("");
                    let resolved_symbol = arguments.get("resolved_symbol").and_then(|s| s.as_str());
                    let resolved_line = arguments.get("resolved_line").and_then(|l| l.as_u64()).map(|l| l as usize);

                    if alias_term.is_empty() || rel_path.is_empty() {
                        "Error: alias_term and relative_path are required for learn_alias. Example: project_context(operation=\"learn_alias\", project_path=\"...\", alias_term=\"đăng nhập\", relative_path=\"src/auth/service.rs\")".to_string()
                    } else {
                        match CodeGraphDb::open_for_project(&proj_path) {
                            Ok(db) => match db.upsert_alias(alias_term, rel_path, resolved_symbol, resolved_line) {
                                Ok(()) => {
                                    state.record_call(500, 100);
                                    format!(
                                        "# Alias Learned Successfully ✓\n\n- Alias Term: `{}`\n- Resolved Path: `{}`\n- Symbol: `{}`\n- Line: {}\n- Persistence: Saved in `.agent-context/code_graph.db`",
                                        alias_term,
                                        rel_path,
                                        resolved_symbol.unwrap_or("—"),
                                        resolved_line.map(|l| l.to_string()).unwrap_or_else(|| "—".to_string())
                                    )
                                }
                                Err(e) => format!("Failed to record alias: {}", e),
                            },
                            Err(e) => format!("Failed to open code graph database: {}", e),
                        }
                    }
                }
                "reindex" => {
                    match IncrementalIndexer::new(&proj_path) {
                        Ok(mut indexer) => {
                            let report = indexer.full_index().unwrap_or_default();
                            let path = proj_path.clone();
                            std::thread::spawn(move || {
                                if let Ok(idx) = IncrementalIndexer::new(&path) {
                                    let _ = idx.embed_symbols();
                                    let _ = idx.embed_chunks();
                                }
                            });
                            state.record_call(5000, 400);
                            format!(
                                "# Project Re-Indexed Successfully ✓\n\n- Files Scanned: {}\n- Files Indexed: {}\n- Files Skipped: {}\n- Symbols Extracted: {}\n- Edges Created: {}\n- Content Chunks: {}\n- Duration: {}ms\n- Background Embedding: Rayon ML pool active\n- Persistence: `.agent-context/code_graph.db`",
                                report.files_scanned,
                                report.files_indexed,
                                report.files_skipped,
                                report.symbols_extracted,
                                report.edges_created,
                                report.chunks_created,
                                report.duration_ms
                            )
                        }
                        Err(e) => format!("Failed to reindex project: {}", e),
                    }
                }
                "architecture" => {
                    let arch_pattern = detect_project_architecture(&proj_path);
                    state.active_architecture_pattern = Some(arch_pattern.clone());
                    let _ = ServerState::save_persisted_architecture(&proj_path, &arch_pattern);
                    state.record_call(1000, 200);
                    format!(
                        "# Project Architecture Analysis\n\n- Detected / Memorized Pattern: {}\n- Workspace Root: {}\n- Persistence: Memorized in `.agent-context/architecture.json`\n\nArchitectural Guidelines:\n- Clean_Architecture: Enforce strict separation between domain, usecase, infrastructure.\n- Layered_Architecture: Enforce controllers -> services -> models flow.\n- Package_By_Feature: Organize code by features/modules.\n- Orchestrator: Keep dispatcher thin and split logic into sub-modules upfront.\n- CLI_Pipeline: Separate argument parsing, command handlers, and core execution engine.\n- Flat_Library: Keep modules focused and avoid over-nested directories.",
                        arch_pattern,
                        proj_path.display()
                    )
                }
                "symbols" | "structure" => {
                    if rel_path.is_empty() {
                        "Error: relative_path is required for symbols/structure operation. Example: project_context(operation=\"symbols\", project_path=\"...\", relative_path=\"src/main.rs\")".to_string()
                    } else {
                        match validate_path(&proj_path, rel_path) {
                            Ok(full_path) => {
                                if let Ok(content) = std::fs::read_to_string(&full_path) {
                                    let mut symbols = Vec::new();
                                    for (idx, line) in content.lines().enumerate() {
                                        let trimmed = line.trim();
                                        if trimmed.starts_with("pub fn ")
                                            || trimmed.starts_with("fn ")
                                            || trimmed.starts_with("pub struct ")
                                            || trimmed.starts_with("struct ")
                                            || trimmed.starts_with("pub enum ")
                                            || trimmed.starts_with("enum ")
                                            || trimmed.starts_with("pub trait ")
                                            || trimmed.starts_with("trait ")
                                            || trimmed.starts_with("impl ")
                                            || trimmed.starts_with("def ")
                                            || trimmed.starts_with("async def ")
                                            || trimmed.starts_with("class ")
                                            || trimmed.starts_with("fun ")
                                            || trimmed.starts_with("suspend fun ")
                                            || trimmed.starts_with("override fun ")
                                            || trimmed.starts_with("object ")
                                            || trimmed.starts_with("companion object")
                                            || trimmed.starts_with("interface ")
                                            || trimmed.starts_with("export function")
                                            || trimmed.starts_with("export class")
                                            || trimmed.starts_with("export const")
                                            || trimmed.starts_with("export interface")
                                            || trimmed.starts_with("func ")
                                        {
                                            symbols.push(format!("L{:04}: {}", idx + 1, trimmed));
                                        }
                                    }
                                    format!(
                                        "# Code Symbol Signatures: {}\n\n{}",
                                        rel_path,
                                        if symbols.is_empty() {
                                            "No top-level symbol signatures found.".to_string()
                                        } else {
                                            symbols.join("\n")
                                        }
                                    )
                                } else {
                                    format!("Failed to read file '{}'", rel_path)
                                }
                            }
                            Err(err_msg) => format!("Security Error: {}", err_msg),
                        }
                    }
                }
                "references" => {
                    if query.is_empty() {
                        "Error: query symbol is required for references operation. Example: project_context(operation=\"references\", project_path=\"...\", query=\"MyStruct\")".to_string()
                    } else {
                        let files = scan_project(&proj_path, 12);
                        let mut refs = Vec::new();
                        let query_lower = query.to_lowercase();
                        for file in files.iter().filter(|f| f.file_type == "file") {
                            if let Ok(path) = validate_path(&proj_path, &file.path) {
                                if let Ok(content) = std::fs::read_to_string(&path) {
                                    for (line_num, line) in content.lines().enumerate() {
                                        if line.to_lowercase().contains(&query_lower) {
                                            let trimmed = line.trim();
                                            let bounded_line = if trimmed.len() > 100 { &trimmed[..100] } else { trimmed };
                                            refs.push(format!("{}:L{} -> {}", file.path, line_num + 1, bounded_line));
                                            if refs.len() >= 30 {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            if refs.len() >= 30 {
                                break;
                            }
                        }
                        format!(
                            "# Symbol References for '{}' (Max 30):\n\n{}",
                            query,
                            if refs.is_empty() {
                                "No references found.".to_string()
                            } else {
                                refs.join("\n")
                            }
                        )
                    }
                }
                _ => format!("Project context operation '{}' completed.", op),
            }
        }
        "ui_ux" => {
            let query = arguments
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("general");
            state.record_call(800, 200);
            format!(
                "# UI/UX Guidelines for '{}'\n\n- Styling: Modern CSS, Glassmorphism, Dynamic Animations\n- Color Palette: Dark mode default, curated HSL gradients\n- Typography: Inter/Outfit via Google Fonts\n- Accessibility: Semantic HTML5, unique IDs",
                query
            )
        }
        "session_continuity" => {
            let op = arguments
                .get("operation")
                .and_then(|o| o.as_str())
                .unwrap_or("load");
            let proj_path_arg = arguments
                .get("project_path")
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);

            match op {
                "save" => match state.save_to_dir(&proj_path) {
                    Ok(_) => format!(
                        "# Session Continuity\n\nSession state saved successfully to `.agent-context/sessions/{}.json`.",
                        state.session_id
                    ),
                    Err(e) => format!("Failed to save session: {}", e),
                },
                "load" => {
                    match ServerState::load_from_dir(&proj_path) {
                        Ok(loaded) => {
                            *state = loaded;
                            // SECURITY FIX Bug #1: Reset permission flags when loading session
                            state.plan_approved = false;
                            state.edit_authorized = false;
                            state.active_architecture_pattern = None;
                            state.verification_passed = false;
                            state.verification_command = None;
                            state.expected_output_keyword = None;
                            state.fix_attempts = 0;
                            format!(
                                "# Session Continuity\n\nLoaded state for session '{}'. Permission flags reset (plan_approved=false). Total Calls: {}",
                                state.session_id, state.tool_calls
                            )
                        }
                        Err(e) => format!("Failed to load session: {}", e),
                    }
                }
                "clear" => {
                    let dir = proj_path.join(".agent-context").join("sessions");
                    if dir.exists() {
                        let _ = std::fs::remove_dir_all(&dir);
                    }
                    *state = ServerState::new();
                    "# Session Continuity: [clear]\n\nSession snapshots cleared successfully. Active session state reset.".to_string()
                }
                "learn" => {
                    let learning = arguments
                        .get("learning")
                        .and_then(|l| l.as_str())
                        .unwrap_or("");
                    let category = arguments
                        .get("category")
                        .and_then(|c| c.as_str())
                        .unwrap_or("general");
                    let is_pinned = arguments
                        .get("pinned")
                        .or_else(|| arguments.get("is_pinned"))
                        .and_then(|p| p.as_bool())
                        .unwrap_or(false);
                    match crate::mcp::learnings::record_project_learning(&proj_path, learning, category, is_pinned) {
                        Ok(msg) => msg,
                        Err(e) => format!("Failed to record learning: {}", e),
                    }
                }
                "handoff" => {
                    let next_action = arguments
                        .get("next_action")
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    match crate::mcp::learnings::write_handoff_summary(&proj_path, state, next_action) {
                        Ok(msg) => msg,
                        Err(e) => format!("Failed to generate handoff protocol: {}", e),
                    }
                }
                "diff" | "changes" => {
                    crate::mcp::learnings::generate_session_diff_summary(&proj_path, state)
                }
                "list" | "sessions" => {
                    let sessions = ServerState::list_sessions(&proj_path);
                    if sessions.is_empty() {
                        "# 🗂️ Active & Archived Sessions\n\nNo session snapshots found in `.agent-context/sessions/`.".to_string()
                    } else {
                        let mut table = String::from("# 🗂️ Active & Archived Sessions\n\n| Session ID | Client / IDE | Workflow Stage | Architecture | Modified Files |\n| :--- | :--- | :--- | :--- | :---: |\n");
                        for s in &sessions {
                            let is_current = s.session_id == state.session_id;
                            let id_str = if is_current {
                                format!("`{}` **(Current)**", s.session_id)
                            } else {
                                format!("`{}`", s.session_id)
                            };
                            let client_str = s.agent_client_name.as_deref().unwrap_or("Default");
                            let arch_str = s.active_architecture_pattern.as_deref().unwrap_or("Auto");
                            let mod_count = format!("{} files", s.modified_files.len());
                            table.push_str(&format!(
                                "| {} | {} | `{}` | {} | {} |\n",
                                id_str, client_str, s.workflow_stage, arch_str, mod_count
                            ));
                        }
                        table.push_str("\n💡 **To switch**: Run `session_continuity(operation=\"switch\", session_id=\"<session_id>\")`");
                        table
                    }
                }
                "switch" => {
                    let target_id = arguments
                        .get("session_id")
                        .or_else(|| arguments.get("id"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("");

                    if target_id.trim().is_empty() {
                        "# Session Continuity: [switch] ⚠️\n\nError: `session_id` parameter is required for switch operation. Call `session_continuity(operation=\"list\")` to view available sessions.".to_string()
                    } else {
                        match ServerState::load_session_by_id(&proj_path, target_id.trim()) {
                            Ok(mut loaded) => {
                                // Zero-Trust Security policy: Inherit context/stage/architecture/files, but reset edit permissions
                                loaded.plan_approved = false;
                                loaded.edit_authorized = false;
                                loaded.fix_attempts = 0;
                                let prev_id = state.session_id.clone();
                                *state = loaded;

                                format!(
                                    "# Session Continuity: [switch] ✓\n\nSuccessfully switched active session from `{}` to `{}`.\n\n- Active Workflow Stage: `{}`\n- Client / IDE: `{}`\n- Architecture Pattern: `{}`\n- Modified Files: {}\n- Zero-Trust Security: `plan_approved=false`, `edit_authorized=false` (Call `task_pipeline` or `workflow_gate` to proceed).",
                                    prev_id,
                                    state.session_id,
                                    state.workflow_stage,
                                    state.agent_client_name.as_deref().unwrap_or("Default"),
                                    state.active_architecture_pattern.as_deref().unwrap_or("Auto"),
                                    state.modified_files.len()
                                )
                            }
                            Err(err) => format!("# Session Continuity: [switch] ⚠️\n\nFailed to switch session: {}", err),
                        }
                    }
                }
                _ => format!("# Session Continuity: [{}]\n\nSession state active.", op),
            }
        }
        "workflow_gate" => {
            let action = arguments
                .get("action")
                .and_then(|a| a.as_str())
                .unwrap_or("check");
            let stage_target = arguments
                .get("target_stage")
                .or_else(|| arguments.get("stage"))
                .and_then(|s| s.as_str());
            let user_msg = arguments.get("user_message").and_then(|u| u.as_str());

            if let Some(msg) = user_msg {
                state.process_user_message(msg);
            }

            state.record_call(300, 50);

            match action {
                "approve" | "approve_plan" => {
                    state.approve_plan();
                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    let _ = state.auto_checkpoint(&proj_path);
                    format!(
                        "# Workflow Gate: [approve_plan]\n\nStatus: PASSED | Plan Approved: true | Stage: {}",
                        state.workflow_stage
                    )
                }
                "pass_verification" => {
                    state.fix_attempts = 0;
                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    let _ = state.auto_checkpoint(&proj_path);
                    format!(
                        "# Workflow Gate: [pass_verification]\n\nStatus: PASSED | Plan Approved: {} | Stage: {} | Fix Attempts: 0\n\n✓ Verification passed and recorded in session checkpoint.",
                        state.plan_approved, state.workflow_stage
                    )
                }
                "set_stage" => {
                    if let Some(target) = stage_target {
                        let proj_path_arg = arguments
                            .get("project_path")
                            .and_then(|p| p.as_str())
                            .unwrap_or(".");
                        let proj_path = detect_project_path(proj_path_arg, state);
                        match state.set_stage(target) {
                            Ok(new_stage) => {
                                let _ = state.auto_checkpoint(&proj_path);
                                format!(
                                    "# Workflow Gate: [set_stage]\n\nStatus: PASSED | Stage Changed To: {} | Plan Approved: {} | Fix Attempts: {}",
                                    new_stage, state.plan_approved, state.fix_attempts
                                )
                            }
                            Err(err_msg) => {
                                let _ = state.auto_checkpoint(&proj_path);
                                format!(
                                    "# Workflow Gate: [set_stage]\n\nStatus: BLOCKED | Error: {}. Trigger IDE/CLI `ask_question` tool to request user approval for stage transition.",
                                    err_msg
                                )
                            }
                        }
                    } else {
                        "# Workflow Gate: [set_stage]\n\nStatus: BLOCKED | Error: target_stage argument is required for set_stage action. Trigger IDE/CLI `ask_question` tool to clarify desired stage.".to_string()
                    }
                }
                "status" => {
                    let edit_allowed = state.workflow_stage == "Build" && state.plan_approved;
                    format!(
                        "# Workflow Stage Status\n\n- Active Stage: {}\n- Plan Approved: {}\n- Fix Attempts: {}/3\n- Edit Authorized: {}",
                        state.workflow_stage, state.plan_approved, state.fix_attempts, edit_allowed
                    )
                }
                "set_architecture" => {
                    let raw_arch = arguments
                        .get("architecture_pattern")
                        .or_else(|| arguments.get("pattern"))
                        .and_then(|a| a.as_str())
                        .unwrap_or("Auto");
                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    state.update_project_path(&proj_path);
                    let arch_pattern = resolve_architecture_pattern(raw_arch, &proj_path, state);
                    state.active_architecture_pattern = Some(arch_pattern.clone());
                    let _ = ServerState::save_persisted_architecture(&proj_path, &arch_pattern);
                    let _ = state.auto_checkpoint(&proj_path);
                    format!(
                        "# Architecture Pattern Locked\n\n- Project: {}\n- Confirmed Architecture: {}\n- Persistence: Saved to `.agent-context/architecture.json`\n\n✓ Pattern memorized for all workflow stages and future sessions.",
                        proj_path.display(),
                        arch_pattern
                    )
                }
                "advance" => {
                    // SECURITY FIX Bug #3: Do NOT auto-process user_message in advance to prevent agent self-approval
                    let target_stage = arguments
                        .get("target_stage")
                        .and_then(|t| t.as_str())
                        .unwrap_or("Build");
                    let stage_res = state.set_stage(target_stage);

                    let raw_arch = arguments
                        .get("architecture_pattern")
                        .and_then(|a| a.as_str())
                        .unwrap_or("Auto");
                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    state.update_project_path(&proj_path);
                    let risk_level = arguments
                        .get("risk_level")
                        .and_then(|r| r.as_str())
                        .unwrap_or("LOW");
                    state.last_risk_level = Some(risk_level.to_string());

                    let arch_pattern = resolve_architecture_pattern(raw_arch, &proj_path, state);

                    if matches!(
                        arch_pattern.as_str(),
                        "Clean_Architecture"
                            | "Layered_Architecture"
                            | "Package_By_Feature"
                            | "Orchestrator"
                            | "CLI_Pipeline"
                            | "Flat_Library"
                    ) && state.workflow_stage == "Build"
                        && state.plan_approved
                    {
                        state.edit_authorized = true;
                        state.active_architecture_pattern = Some(arch_pattern);
                    }

                    if stage_res.is_ok() {
                        let _ = state.auto_checkpoint(&proj_path);
                    }

                    match stage_res {
                        Ok(msg) => format!(
                            "# Workflow Gate: [advance]\n\n{}\n- Edit Authorized: {}\n- Architecture Pattern: {}",
                            msg,
                            state.edit_authorized,
                            state
                                .active_architecture_pattern
                                .as_deref()
                                .unwrap_or("NONE")
                        ),
                        Err(err) => format!("# Workflow Gate: [advance]\n\n⚠️ Error: {}", err),
                    }
                }
                "authorize_edit" => {
                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    state.update_project_path(&proj_path);
                    let rel_path = arguments
                        .get("relative_path")
                        .and_then(|r| r.as_str())
                        .unwrap_or("");
                    let justification = arguments
                        .get("justification")
                        .and_then(|j| j.as_str())
                        .unwrap_or("");
                    let raw_arch = arguments
                        .get("architecture_pattern")
                        .and_then(|a| a.as_str())
                        .unwrap_or("Auto");
                    let arch_pattern = resolve_architecture_pattern(raw_arch, &proj_path, state);

                    // Zero-Turn Predictive Transition: if plan approved and in Plan stage, auto-advance to Build
                    if state.plan_approved && state.workflow_stage == "Plan" {
                        let _ = state.set_stage("Build");
                    }

                    // Perform Code Graph Diff Impact Guard analysis
                    let impact = crate::mcp::impact::assess_file_risk(&proj_path, rel_path);
                    let declared_risk = arguments
                        .get("risk_level")
                        .and_then(|r| r.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| match impact.risk_level {
                            crate::mcp::impact::RiskLevel::High => "HIGH".to_string(),
                            crate::mcp::impact::RiskLevel::Medium => "MEDIUM".to_string(),
                            crate::mcp::impact::RiskLevel::Low => "LOW".to_string(),
                        });
                    state.last_risk_level = Some(declared_risk.clone());

                    if !matches!(
                        arch_pattern.as_str(),
                        "Clean_Architecture"
                            | "Layered_Architecture"
                            | "Package_By_Feature"
                            | "Orchestrator"
                            | "CLI_Pipeline"
                            | "Flat_Library"
                    ) {
                        format!(
                            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (ORCHESTRATION MANDATE VIOLATION)\n- Project Path: {}\n- Declared Architecture: '{}'\n\n⚠️ Error: ARCHITECTURE_GATE_BLOCKED: Trigger IDE/CLI `ask_question` tool to let user choose a valid `architecture_pattern` ('Clean_Architecture', 'Layered_Architecture', 'Package_By_Feature', 'Orchestrator', 'CLI_Pipeline', 'Flat_Library', or 'Auto'), then re-invoke `workflow_gate(action=\"authorize_edit\", ...)`.",
                            proj_path.display(),
                            if arch_pattern.is_empty() {
                                "NONE"
                            } else {
                                &arch_pattern
                            }
                        )
                    } else if impact.risk_level == crate::mcp::impact::RiskLevel::High && (justification.trim().len() < 10 || justification == "No justification provided") {
                        // Strict Gate for Critical Hub: mandatory explanation
                        format!(
                            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (CRITICAL HUB IMPACT GUARD)\n- Target File: `{}`\n- Incoming Dependencies: {}\n- Impacted Modules: {}\n\n⚠️ **Error: HIGH_RISK_JUSTIFICATION_REQUIRED**: This file is a Critical Hub (referenced by >8 modules). You MUST provide a specific `justification` parameter explaining how your changes avoid breaking downstream modules, along with your planned test verification.",
                            rel_path,
                            impact.dependent_count,
                            if impact.dependent_files.is_empty() { "—".to_string() } else { impact.dependent_files.join(", ") }
                        )
                    } else if declared_risk == "HIGH" && !state.plan_approved {
                        format!(
                            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (HIGH RISK)\n- Project Path: {}\n- Declared Risk: HIGH\n- Justification: {}\n\n⚠️ Error: HIGH RISK edits require explicit user approval. Present plan and trigger IDE/CLI `ask_question` tool (or invoke `workflow_gate(action=\"set_stage\", target_stage=\"Plan\")`) to confirm approval.",
                            proj_path.display(),
                            if justification.is_empty() { "No justification provided" } else { justification }
                        )
                    } else if state.workflow_stage == "Build" && state.plan_approved {
                        state.edit_authorized = true;
                        state.active_architecture_pattern = Some(arch_pattern.clone());

                        // Automatically record modified file and take pre-edit snapshot for rollback guard
                        if !rel_path.is_empty() {
                            state.record_modified_file(rel_path);
                            let _ = crate::mcp::impact::create_file_snapshot(&proj_path, rel_path, &state.session_id);
                        }
                        let _ = state.auto_checkpoint(&proj_path);

                        let mut resp = format!(
                            "# Edit Approval Gate Authorization\n\n- Status: PASSED\n- Project Path: {}\n- Target File: {}\n- Assessed Risk Level: {:?} (Dependencies: {})\n- Architecture Pattern: {}\n- Justification: {}\n- Active Stage: {}\n- Plan Approved: true\n\n✓ File edits are fully authorized under {} Architecture.",
                            proj_path.display(),
                            if rel_path.is_empty() { "—" } else { rel_path },
                            impact.risk_level,
                            impact.dependent_count,
                            arch_pattern,
                            if justification.is_empty() { "Standard development task" } else { justification },
                            state.workflow_stage,
                            arch_pattern
                        );

                        if let Some(warn) = impact.warning {
                            resp.push_str(&format!("\n\n⚠️ **Impact Guard**: {}", warn));
                        }

                        resp
                    } else {
                        format!(
                            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED\n- Project Path: {}\n- Active Stage: {}\n- Plan Approved: {}\n\n⚠️ Error: WORKFLOW_STAGE_BLOCKED: Edits require Build stage and plan_approved=true. Trigger IDE/CLI `ask_question` tool to request user approval on the plan, then invoke `workflow_gate(action=\"set_stage\", target_stage=\"Build\")`.",
                            proj_path.display(),
                            state.workflow_stage,
                            state.plan_approved
                        )
                    }
                }
                "rollback" => {
                    let proj_path_arg = arguments
                        .get("project_path")
                        .and_then(|p| p.as_str())
                        .unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    match crate::mcp::impact::restore_session_snapshots(&proj_path, &state.session_id) {
                        Ok(restored) => {
                            if restored.is_empty() {
                                format!(
                                    "# Rollback Guard: [rollback]\n\nNo snapshot files found for session '{}'. No changes were reverted.",
                                    state.session_id
                                )
                            } else {
                                format!(
                                    "# Rollback Guard: [rollback] ✓\n\nSuccessfully restored {} file(s) to their pre-edit state for session '{}':\n\n{}",
                                    restored.len(),
                                    state.session_id,
                                    restored.iter().map(|f| format!("- `{}`", f)).collect::<Vec<_>>().join("\n")
                                )
                            }
                        }
                        Err(e) => format!("# Rollback Guard: [rollback] ⚠️\n\nFailed to restore session snapshots: {}", e),
                    }
                }
                _ => {
                    // "check" action — SECURITY FIX Bug #2: READ ONLY (no state mutation)
                    let status_str = if state.workflow_stage == "Build" && !state.plan_approved {
                        "BLOCKED"
                    } else {
                        "PASSED"
                    };
                    let mut resp = format!(
                        "# Workflow Gate: [check]\n\nStatus: {} | Plan Approved: {} | Stage: {} | Fix Attempts: {}",
                        status_str, state.plan_approved, state.workflow_stage, state.fix_attempts
                    );
                    if state.workflow_stage == "Build" && !state.plan_approved {
                        resp.push_str("\n\n⚠️ Trigger IDE/CLI `ask_question` tool to request explicit user plan approval before editing code.");
                    }
                    if state.workflow_stage == "Test_Recheck" {
                        resp.push_str("\n\n⚠ **ANTI-HALLUCINATION ENFORCER ACTIVE**: Re-read the original user prompt & verify all requested features against real build/test outputs before declaring task complete.");
                    }
                    resp
                }
            }
        }
        _ => format!("Tool '{}' executed successfully.", name),
    };

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": response_text
            }
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_traversal() {
        let base = Path::new(".");
        assert!(validate_path(base, "../../../etc/passwd").is_err());
        assert!(validate_path(base, "Cargo.toml").is_ok());
    }

    #[test]
    fn test_guidance_get_local_skill() {
        let mut state = ServerState::new();
        state.plan_approved = true;
        state.set_stage("Build").unwrap();

        let tmp_dir = std::env::temp_dir().join("test_guidance_get");
        let skill_dir = tmp_dir.join(".agents").join("skills").join("custom");
        let _ = std::fs::create_dir_all(&skill_dir);
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(
            &skill_file,
            "---\nname: custom\n---\n# Custom Skill Content",
        )
        .unwrap();

        let res = handle_tool_call(
            "guidance",
            json!({
                "operation": "get",
                "identifier": skill_file.to_string_lossy().to_string()
            }),
            &mut state,
        );

        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("Custom Skill Content"));

        let _ = std::fs::remove_dir_all(tmp_dir);
    }

    #[test]
    fn test_detect_project_path() {
        let state = ServerState::new();
        // 1. Check explicit path
        let explicit = std::env::current_dir().unwrap();
        let res = detect_project_path(&explicit.to_string_lossy(), &state);
        assert_eq!(res, explicit);

        // 2. Check recorded state.project_path memory
        let mut state_recorded = ServerState::new();
        state_recorded.project_path = Some(explicit.to_string_lossy().to_string());
        let res_recorded = detect_project_path(".", &state_recorded);
        assert_eq!(res_recorded, explicit);

        // 3. Check workspace roots
        let mut state2 = ServerState::new();
        state2.workspace_roots = vec![explicit.to_string_lossy().to_string()];
        let res2 = detect_project_path(".", &state2);
        assert_eq!(res2, explicit);

        // 4. Check that process CWD takes priority over stale global_path_file
        let global_path_file = ServerState::global_project_path_file();
        let parent_dir = explicit.parent().unwrap_or(&explicit);
        let _ = std::fs::write(&global_path_file, parent_dir.to_string_lossy().as_bytes());
        let state_global = ServerState::new();
        let res_global = detect_project_path(".", &state_global);
        assert_eq!(res_global, explicit); // Must return process current_dir (explicit), not global_path_file (parent_dir)
        let _ = std::fs::remove_file(&global_path_file);
    }

    #[test]
    fn test_anti_hallucination_verification() {
        let mut state = ServerState::new();
        state.plan_approved = true;
        state.set_stage("Test_Recheck").unwrap();

        let res = handle_tool_call("guidance", json!({ "operation": "verify" }), &mut state);

        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("Anti-Hallucination Post-Code Verification Checklist"));
        assert!(text.contains("User Requirement Alignment"));

        let check_res = handle_tool_call("workflow_gate", json!({ "action": "check" }), &mut state);
        assert!(check_res.is_ok());
        let check_text = check_res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(check_text.contains("ANTI-HALLUCINATION ENFORCER ACTIVE"));
    }

    #[test]
    fn test_select_skills_flow() {
        let mut state = ServerState::new();
        state.plan_approved = true;
        state.set_stage("Build").unwrap();

        state.pending_skill_proposals = vec![
            (
                "agent-guidance".to_string(),
                "skills/agent-guidance/SKILL.md".to_string(),
                0.95,
            ),
            (
                "test-skill".to_string(),
                "skills/test-skill/SKILL.md".to_string(),
                0.80,
            ),
        ];

        // 1. Select valid skill
        let res = handle_tool_call(
            "select_skills",
            json!({ "skills": ["agent-guidance"] }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("# Skill Selection Confirmed"));
        assert!(text.contains("agent-guidance"));

        // 2. State cleared after selection
        assert!(state.pending_skill_proposals.is_empty());

        // 3. Select with empty array when no proposals remain
        let empty_res = handle_tool_call("select_skills", json!({ "skills": [] }), &mut state);
        assert!(empty_res.is_ok());
        let empty_text = empty_res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(empty_text.contains("No skills selected"));
    }

    #[test]
    fn test_auto_architecture_detection() {
        let cwd = std::env::current_dir().unwrap();
        let detected = detect_project_architecture(&cwd);
        assert!(matches!(
            detected.as_str(),
            "Clean_Architecture" | "Layered_Architecture" | "Package_By_Feature" | "Orchestrator"
        ));

        let state = ServerState::new();
        let auto_resolved = resolve_architecture_pattern("Auto", &cwd, &state);
        assert_eq!(auto_resolved, detected);

        let empty_resolved = resolve_architecture_pattern("", &cwd, &state);
        assert_eq!(empty_resolved, detected);

        let explicit_resolved = resolve_architecture_pattern("Clean_Architecture", &cwd, &state);
        assert_eq!(explicit_resolved, "Clean_Architecture");
    }

    #[test]
    fn test_auto_architecture_gate_authorization() {
        let mut state = ServerState::new();
        state.plan_approved = true;
        state.set_stage("Build").unwrap();

        // 1. Authorize edit with 'Auto' pattern should succeed and resolve pattern
        let res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "authorize_edit",
                "architecture_pattern": "Auto",
                "risk_level": "LOW",
                "justification": "Refactoring test"
            }),
            &mut state,
        );
        assert!(
            res.is_ok(),
            "workflow_gate authorize_edit with 'Auto' must succeed: {:?}",
            res
        );
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("Status: PASSED"));
        assert!(state.edit_authorized);
        assert!(state.active_architecture_pattern.is_some());

        // 2. Query precode guidance should contain active architecture
        let precode_res =
            handle_tool_call("guidance", json!({ "operation": "precode" }), &mut state);
        assert!(precode_res.is_ok());
        let precode_text = precode_res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(precode_text.contains("Architecture Pattern:"));
    }

    #[test]
    fn test_project_context_architecture_operation() {
        let mut state = ServerState::new();
        // project_context(operation="architecture") should succeed even in Plan stage
        state.set_stage("Plan").unwrap();

        let res = handle_tool_call(
            "project_context",
            json!({ "operation": "architecture" }),
            &mut state,
        );
        assert!(
            res.is_ok(),
            "project_context operation 'architecture' must succeed in Plan stage: {:?}",
            res
        );
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("# Project Architecture Analysis"));
        assert!(text.contains("Pattern:"));
    }

    #[test]
    #[ignore = "Requires pre-cached HuggingFace model files; avoid network I/O in unit tests"]
    fn test_task_pipeline_architecture_guidance_output() {
        let mut state = ServerState::new();

        let res = handle_tool_call(
            "task_pipeline",
            json!({ "task": "build new feature", "phase": "plan" }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("## Architecture Guidance"));
        assert!(text.contains("Detected Pattern:"));
    }

    #[test]
    fn test_guidance_workflow_loads_embedded_reference() {
        let mut state = ServerState::new();
        let res = handle_tool_call(
            "guidance",
            json!({ "operation": "workflow", "identifier": "code" }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        // Embedded workflow-code.md should be loaded instead of generic 1-liner
        assert!(text.contains("code") || text.contains("Code") || text.contains("Build"));
        assert!(!text.contains("# Dev Workflow Guidance: [code]\n\nRecommended Flow: Context -> Plan -> Ask/Revise -> Build -> Test/Recheck -> Fix -> Document"));
    }

    #[test]
    #[ignore = "Requires pre-cached HuggingFace model files; avoid network I/O in unit tests"]
    fn test_guidance_docs_vector_search() {
        let mut state = ServerState::new();
        let res = handle_tool_call(
            "guidance",
            json!({ "operation": "docs", "query": "rust testing" }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("# Documentation Guidance for"));
    }

    #[test]
    fn test_workflow_gate_plan_approval_via_user_message() {
        let mut state = ServerState::new();
        assert!(!state.plan_approved);

        // 1. Calling workflow_gate approve_plan action sets plan_approved=true
        let res = handle_tool_call(
            "workflow_gate",
            json!({ "action": "approve_plan" }),
            &mut state,
        );
        assert!(res.is_ok());
        assert!(state.plan_approved);

        // 2. set_stage to Build now succeeds
        let stage_res = handle_tool_call(
            "workflow_gate",
            json!({ "action": "set_stage", "target_stage": "Build" }),
            &mut state,
        );
        assert!(stage_res.is_ok());
        let text = stage_res.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("Status: PASSED"));
        assert_eq!(state.workflow_stage, "Build");
    }

    #[test]
    fn test_architecture_pattern_persistence_and_locking() {
        let temp_dir = std::env::temp_dir().join(format!("arch_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut state = ServerState::new();

        // 1. Explicitly set architecture pattern via workflow_gate
        let set_res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "set_architecture",
                "architecture_pattern": "CLI_Pipeline",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(set_res.is_ok());
        assert_eq!(
            state.active_architecture_pattern.as_deref(),
            Some("CLI_Pipeline")
        );

        // 2. Verify disk persistence in .agent-context/architecture.json
        let loaded = ServerState::load_persisted_architecture(&temp_dir);
        assert_eq!(loaded.as_deref(), Some("CLI_Pipeline"));

        // 3. Stage transition to Plan does not wipe active_architecture_pattern
        let plan_stage = state.set_stage("Plan");
        assert!(plan_stage.is_ok());
        assert_eq!(
            state.active_architecture_pattern.as_deref(),
            Some("CLI_Pipeline")
        );

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_expanded_architecture_patterns() {
        let mut state = ServerState::new();
        state.workflow_stage = "Build".to_string();
        state.plan_approved = true;

        for pattern in &["CLI_Pipeline", "Flat_Library", "Clean_Architecture", "Layered_Architecture", "Package_By_Feature", "Orchestrator"] {
            let res = handle_tool_call(
                "workflow_gate",
                json!({
                    "action": "authorize_edit",
                    "architecture_pattern": pattern,
                    "risk_level": "LOW",
                    "justification": "Testing expanded pattern"
                }),
                &mut state,
            );
            assert!(res.is_ok(), "Pattern {} should be authorized: {:?}", pattern, res);
            let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
            assert!(text.contains("Status: PASSED"), "Pattern {} failed authorization: {}", pattern, text);
        }
    }

    #[test]
    fn test_project_context_read_300_loc_warning() {
        let temp_dir = std::env::temp_dir().join(format!("read_loc_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let large_file = temp_dir.join("large_file.rs");
        let content = (1..=350).map(|i| format!("fn function_{}() {{}}", i)).collect::<Vec<_>>().join("\n");
        let _ = std::fs::write(&large_file, &content);

        let mut state = ServerState::new();
        let res = handle_tool_call(
            "project_context",
            json!({
                "operation": "read",
                "relative_path": "large_file.rs",
                "view_mode": "full",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("ARCHITECTURE MANDATE (300 LOC Cap Exceeded)"));
        assert!(text.contains("350 total lines"));
        assert!(text.contains("Decompose into sub-modules upfront"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_target_symbol_extraction_in_large_file() {
        let temp_dir = std::env::temp_dir().join(format!("symbol_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let large_file = temp_dir.join("giant_service.rs");

        // Create a 600-line file with a specific target function at line 450
        let mut lines = Vec::new();
        for i in 1..=440 {
            lines.push(format!("fn dummy_func_{}() {{ let _x = {}; }}", i, i));
        }
        lines.push("pub fn target_critical_function(user_id: u64) -> bool {".to_string());
        lines.push("    let is_valid = user_id > 1000;".to_string());
        lines.push("    println!(\"Processing user: {}\", user_id);".to_string());
        lines.push("    is_valid".to_string());
        lines.push("}".to_string());
        for i in 446..=600 {
            lines.push(format!("fn trailing_func_{}() {{ let _y = {}; }}", i, i));
        }
        let _ = std::fs::write(&large_file, lines.join("\n"));

        let mut state = ServerState::new();
        let res = handle_tool_call(
            "project_context",
            json!({
                "operation": "read",
                "relative_path": "giant_service.rs",
                "target_symbol": "target_critical_function",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );

        assert!(res.is_ok(), "Target symbol extraction failed: {:?}", res);
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Target Symbol Extracted: 'target_critical_function'"), "Output missing target symbol header: {}", text);
        assert!(text.contains("pub fn target_critical_function(user_id: u64) -> bool"), "Output missing function signature: {}", text);
        assert!(text.contains("Processing user:"), "Output missing function body: {}", text);
        assert!(!text.contains("dummy_func_1"), "Output should NOT contain unrelated top functions: {}", text);
        assert!(!text.contains("trailing_func_500"), "Output should NOT contain unrelated bottom functions: {}", text);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_project_context_deep_search_and_snippets() {
        let temp_dir = std::env::temp_dir().join(format!("deep_search_test_{}", std::process::id()));
        let deep_path = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example").join("ui").join("tour");
        let _ = std::fs::create_dir_all(&deep_path);
        let deep_file = deep_path.join("ProductTour.kt");
        let content = "package com.example.ui.tour\n\nclass ProductTour {\n    fun startTour() {\n        val tourAnchor = 42\n    }\n}\n";
        let _ = std::fs::write(&deep_file, content);

        let mut state = ServerState::new();
        let res = handle_tool_call(
            "project_context",
            json!({
                "operation": "search",
                "query": "touranchor",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok(), "Deep search failed: {:?}", res);
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("ProductTour.kt"), "Deep search did not find ProductTour.kt: {}", text);
        assert!(text.contains("ProductTour") || text.contains("tourAnchor"), "Deep search did not extract symbol/snippet: {}", text);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_project_context_kotlin_symbols() {
        let temp_dir = std::env::temp_dir().join(format!("kotlin_symbols_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let kt_file = temp_dir.join("ProductTour.kt");
        let content = "package com.nodescape.app.ui.tour\n\nclass ProductTourOverlay {\n    suspend fun showTour() {}\n    companion object {\n        fun create() {}\n    }\n}\n";
        let _ = std::fs::write(&kt_file, content);

        let mut state = ServerState::new();
        let res = handle_tool_call(
            "project_context",
            json!({
                "operation": "symbols",
                "relative_path": "ProductTour.kt",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("class ProductTourOverlay"));
        assert!(text.contains("suspend fun showTour"));
        assert!(text.contains("companion object"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_detect_deep_layered_architecture_singular() {
        let temp_dir = std::env::temp_dir().join(format!("deep_arch_layered_{}", std::process::id()));
        let service_dir = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example").join("service");
        let viewmodel_dir = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example").join("viewmodel");
        let _ = std::fs::create_dir_all(&service_dir);
        let _ = std::fs::create_dir_all(&viewmodel_dir);
        let _ = std::fs::write(service_dir.join("PingService.kt"), "class PingService");
        let _ = std::fs::write(viewmodel_dir.join("MainViewModel.kt"), "class MainViewModel");

        let detected = detect_project_architecture(&temp_dir);
        assert_eq!(detected, "Layered_Architecture");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_detect_deep_clean_architecture_infra() {
        let temp_dir = std::env::temp_dir().join(format!("deep_arch_clean_{}", std::process::id()));
        let infra_dir = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example").join("infra");
        let domain_dir = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example").join("domain");
        let _ = std::fs::create_dir_all(&infra_dir);
        let _ = std::fs::create_dir_all(&domain_dir);
        let _ = std::fs::write(infra_dir.join("NetworkClient.kt"), "class NetworkClient");
        let _ = std::fs::write(domain_dir.join("UserEntity.kt"), "class UserEntity");

        let detected = detect_project_architecture(&temp_dir);
        assert_eq!(detected, "Clean_Architecture");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_guidance_precode_kotlin_and_go_rules() {
        let temp_dir = std::env::temp_dir().join(format!("precode_kotlin_test_{}", std::process::id()));
        let kt_dir = temp_dir.join("app").join("src").join("main").join("java").join("com").join("example");
        let _ = std::fs::create_dir_all(&kt_dir);
        let _ = std::fs::write(kt_dir.join("MainActivity.kt"), "class MainActivity");

        let mut state = ServerState::new();
        state.update_project_path(&temp_dir);
        let res = handle_tool_call(
            "guidance",
            json!({
                "operation": "precode",
                "query": "android kotlin UI",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Primary Language: Kotlin/Java"));
        assert!(text.contains("Dispatchers.IO/Default"));
        assert!(text.contains("StateFlow/LiveData lifecycles"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_select_skills_direct_fallback_when_proposals_empty() {
        let mut state = ServerState::new();
        // proposals is empty
        assert!(state.pending_skill_proposals.is_empty());

        let res = handle_tool_call(
            "select_skills",
            json!({
                "skills": ["android-clean-architecture"]
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Skill Selection Confirmed"));
        assert!(text.contains("android-clean-architecture [Embedded Catalog]"));
    }

    #[test]
    #[ignore] // Requires pre-cached HuggingFace model files; avoid network I/O in unit tests
    fn test_task_pipeline_skill_deduplication_and_empty_task_fallback() {
        let mut state = ServerState::new();
        let res = handle_tool_call(
            "task_pipeline",
            json!({
                "task": "",
                "project_path": ".",
                "phase": "plan",
                "focus": "security"
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("# Task Pipeline Activated"));

        // Check that pending_skill_proposals contains no duplicates
        let mut names = std::collections::HashSet::new();
        for (name, _, _) in &state.pending_skill_proposals {
            assert!(names.insert(name.clone()), "Duplicate skill proposal found: {}", name);
        }
    }

    #[test]
    fn test_precode_upfront_split_blueprint() {
        let mut state = ServerState::new();
        state.active_architecture_pattern = Some("CLI_Pipeline".to_string());

        let res = handle_tool_call(
            "guidance",
            json!({ "operation": "precode", "query": "rust" }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Upfront Architecture & 300 LOC Cap (Mandatory)"));
        assert!(text.contains("CLI_Pipeline"));
        assert!(text.contains("CLI entrypoint main (< 80 LOC)"));
    }

    #[test]
    fn test_project_context_cascade_and_learning() {
        let temp_dir = std::env::temp_dir().join(format!("ag_tools_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(temp_dir.join("src"));
        std::fs::write(
            temp_dir.join("src/payment.rs"),
            "pub struct PaymentGateway;\nimpl PaymentGateway {\n    pub fn charge_card(&self) {\n        let timeout = 30;\n    }\n}\n",
        ).unwrap();

        let mut state = ServerState::new();

        // 1. Reindex
        let reindex_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "reindex",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(reindex_res.is_ok());
        let text = reindex_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Project Re-Indexed Successfully"));

        // 2. Search hits symbol FTS
        let search_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "search",
                "project_path": temp_dir.to_str().unwrap(),
                "query": "PaymentGateway"
            }),
            &mut state,
        );
        assert!(search_res.is_ok());
        let text = search_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("PaymentGateway"));

        // 3. Search hits content FTS
        let content_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "search",
                "project_path": temp_dir.to_str().unwrap(),
                "query": "timeout"
            }),
            &mut state,
        );
        assert!(content_res.is_ok());
        let text = content_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("payment.rs"));

        // 4. Learn Alias
        let learn_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "learn_alias",
                "project_path": temp_dir.to_str().unwrap(),
                "alias_term": "thanh toán thẻ",
                "relative_path": "src/payment.rs",
                "resolved_symbol": "PaymentGateway"
            }),
            &mut state,
        );
        assert!(learn_res.is_ok());
        let text = learn_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Alias Learned Successfully"));

        // 5. Search hits Alias Cache
        let alias_search_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "search",
                "project_path": temp_dir.to_str().unwrap(),
                "query": "thanh toán thẻ"
            }),
            &mut state,
        );
        assert!(alias_search_res.is_ok());
        let text = alias_search_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("alias cache"));
        assert!(text.contains("src/payment.rs"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_workflow_gate_zero_turn_advance_and_impact_guard() {
        let temp_dir = std::env::temp_dir().join(format!("ag_impact_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(temp_dir.join("src"));
        let core_file = temp_dir.join("src/core.rs");
        std::fs::write(&core_file, "pub struct CoreConfig;\n").unwrap();

        let mut state = ServerState::new();
        state.workflow_stage = "Plan".to_string();
        state.plan_approved = true;

        // 1. Zero-Turn Predictive Transition (Plan -> Build on authorize_edit)
        let auth_res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "authorize_edit",
                "project_path": temp_dir.to_str().unwrap(),
                "relative_path": "src/core.rs",
                "architecture_pattern": "CLI_Pipeline",
                "justification": "Modifying core config with unit tests"
            }),
            &mut state,
        );
        assert!(auth_res.is_ok());
        let text = auth_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Status: PASSED"));
        assert_eq!(state.workflow_stage, "Build");
        assert!(state.edit_authorized);

        // 2. Modify file content
        std::fs::write(&core_file, "pub struct CoreConfig;\npub fn new_modified_code() {}\n").unwrap();

        // 3. Rollback Guard restores file
        let rollback_res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "rollback",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(rollback_res.is_ok());
        let rollback_text = rollback_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(rollback_text.contains("Successfully restored 1 file(s)"));

        // Verify original content was restored
        let restored_content = std::fs::read_to_string(&core_file).unwrap();
        assert_eq!(restored_content, "pub struct CoreConfig;\n");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore = "Requires pre-cached HuggingFace model files; avoid network I/O in unit tests"]
    fn test_task_pipeline_blueprint_and_recipe() {
        let mut state = ServerState::new();
        let res = handle_tool_call(
            "task_pipeline",
            json!({
                "task": "build payment webhook listener and process transactions",
                "project_path": ".",
                "phase": "plan"
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("## 🍳 Task-Specific Skill Recipe"));
        assert!(text.contains("## 📐 Dynamic Split Blueprint"));
        assert!(text.contains("Upfront Modular Blueprint"));
    }

    #[test]
    #[ignore = "Requires pre-cached HuggingFace model files; avoid network I/O in unit tests"]
    fn test_select_skills_semantic_slicing() {
        let mut state = ServerState::new();
        state.pending_skill_proposals = vec![
            ("android-clean-architecture".to_string(), "skills/android-clean-architecture/SKILL.md".to_string(), 0.95),
        ];

        let res = handle_tool_call(
            "select_skills",
            json!({
                "skills": ["android-clean-architecture"],
                "task": "configure domain usecases and repository traits",
                "project_path": "."
            }),
            &mut state,
        );
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Skill Selection Confirmed"));
        assert!(text.contains("## 🛡️ Language Safety Rules"));
    }

    #[test]
    fn test_session_continuity_learn_and_handoff() {
        let temp_dir = std::env::temp_dir().join(format!("ag_learn_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let mut state = ServerState::new();

        // 1. Record Learning with Category
        let learn_res = handle_tool_call(
            "session_continuity",
            json!({
                "operation": "learn",
                "project_path": temp_dir.to_str().unwrap(),
                "category": "build_test",
                "learning": "Always run cargo test with mock database pool"
            }),
            &mut state,
        );
        assert!(learn_res.is_ok());
        let text = learn_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("Project Learning Saved"));

        // 2. Generate Handoff Protocol
        let handoff_res = handle_tool_call(
            "session_continuity",
            json!({
                "operation": "handoff",
                "project_path": temp_dir.to_str().unwrap(),
                "next_action": "Run cargo test and inspect failure in auth module"
            }),
            &mut state,
        );
        assert!(handoff_res.is_ok());
        let handoff_text = handoff_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(handoff_text.contains("Cross-Agent Handoff Protocol"));

        // 3. Verify handoff file on disk
        let handoff_file = temp_dir.join(".agent-context/handoff.md");
        assert!(handoff_file.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_project_context_skeleton_mode() {
        let temp_dir = std::env::temp_dir().join(format!("ag_skel_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(temp_dir.join("src"));
        let test_file = temp_dir.join("src/large.rs");

        let mut lines = Vec::new();
        lines.push("pub struct BigService;".to_string());
        lines.push("impl BigService {".to_string());
        lines.push("    pub fn heavy_computation(&self) {".to_string());
        for i in 0..320 {
            lines.push(format!("        let var_{} = {};", i, i));
        }
        lines.push("    }".to_string());
        lines.push("}".to_string());

        std::fs::write(&test_file, lines.join("\n")).unwrap();

        let mut state = ServerState::new();

        // Reading file > 300 LOC automatically triggers skeleton mode
        let read_res = handle_tool_call(
            "project_context",
            json!({
                "operation": "read",
                "project_path": temp_dir.to_str().unwrap(),
                "relative_path": "src/large.rs"
            }),
            &mut state,
        );
        assert!(read_res.is_ok());
        let text = read_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("AST Structural Skeleton"));
        assert!(text.contains("pub fn heavy_computation(&self)"));
        assert!(text.contains("Token Saver Mode"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_auto_checkpoint_on_stage_advance_and_edit() {
        let temp_dir = std::env::temp_dir().join(format!("tools_auto_cp_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut state = ServerState::new();

        // 1. Advance to Plan
        let plan_res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "set_stage",
                "target_stage": "Plan",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(plan_res.is_ok());

        let cp_file = temp_dir
            .join(".agent-context")
            .join("sessions")
            .join(format!("{}.json", state.session_id));
        assert!(cp_file.exists(), "Checkpoint should exist after set_stage to Plan");

        // 2. Approve plan
        let app_res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "approve",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(app_res.is_ok());

        // 3. Advance to Build
        let adv_res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "advance",
                "target_stage": "Build",
                "architecture_pattern": "Layered_Architecture",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(adv_res.is_ok());

        let loaded = ServerState::load_from_dir(&temp_dir).unwrap();
        assert_eq!(loaded.workflow_stage, "Build");
        assert!(loaded.plan_approved);
        assert_eq!(
            loaded.active_architecture_pattern.as_deref(),
            Some("Layered_Architecture")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_diff_and_handoff_integration() {
        let temp_dir = std::env::temp_dir().join(format!("tools_diff_handoff_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut state = ServerState::new();
        state.plan_approved = true;
        state.set_stage("Build").unwrap();

        // 1. Authorize edit on src/lib.rs
        let edit_res = handle_tool_call(
            "workflow_gate",
            json!({
                "action": "authorize_edit",
                "relative_path": "src/lib.rs",
                "architecture_pattern": "Layered_Architecture",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(edit_res.is_ok());
        assert_eq!(state.modified_files, vec!["src/lib.rs"]);

        // 2. Call session_continuity(operation="diff")
        let diff_res = handle_tool_call(
            "session_continuity",
            json!({
                "operation": "diff",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(diff_res.is_ok());
        let diff_text = diff_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(diff_text.contains("Session Modification Summary"));
        assert!(diff_text.contains("src/lib.rs"));

        // 3. Call session_continuity(operation="handoff")
        let handoff_res = handle_tool_call(
            "session_continuity",
            json!({
                "operation": "handoff",
                "next_action": "Run cargo test and benchmark",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut state,
        );
        assert!(handoff_res.is_ok());
        let handoff_text = handoff_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(handoff_text.contains("Cross-Agent Handoff Protocol"));
        assert!(handoff_text.contains("src/lib.rs"));
        assert!(handoff_text.contains("Run cargo test and benchmark"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_continuity_list_and_switch() {
        let temp_dir = std::env::temp_dir().join(format!("tools_list_switch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut target_state = ServerState::with_client_name(Some("cursor".to_string()));
        target_state.workflow_stage = "Build".to_string();
        target_state.plan_approved = true;
        target_state.edit_authorized = true;
        target_state.record_modified_file("src/parser.rs");
        assert!(target_state.save_to_dir(&temp_dir).is_ok());

        let mut active_state = ServerState::with_client_name(Some("antigravity".to_string()));

        // 1. List sessions
        let list_res = handle_tool_call(
            "session_continuity",
            json!({
                "operation": "list",
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut active_state,
        );
        assert!(list_res.is_ok());
        let list_text = list_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(list_text.contains("Active & Archived Sessions"));
        assert!(list_text.contains(&target_state.session_id));
        assert!(list_text.contains("cursor"));

        // 2. Switch to target_state.session_id
        let switch_res = handle_tool_call(
            "session_continuity",
            json!({
                "operation": "switch",
                "session_id": target_state.session_id,
                "project_path": temp_dir.to_str().unwrap()
            }),
            &mut active_state,
        );
        assert!(switch_res.is_ok());
        let switch_text = switch_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(switch_text.contains("Successfully switched active session"));

        // 3. Verify active_state inherited target session context with Zero-Trust reset
        assert_eq!(active_state.session_id, target_state.session_id);
        assert_eq!(active_state.workflow_stage, "Build");
        assert_eq!(active_state.modified_files, vec!["src/parser.rs"]);
        assert!(!active_state.edit_authorized, "Zero-Trust policy should reset edit_authorized");
        assert!(!active_state.plan_approved, "Zero-Trust policy should reset plan_approved");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
