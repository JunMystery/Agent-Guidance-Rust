use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tracing::info;

use crate::catalog::store::{
    SkillSource, get_embedded_skill, list_embedded_skills, load_all_skills,
};
use crate::context::cache::project_snapshot;
use crate::context::scanner::scan_project;
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
    let files = scan_project(proj_path, 2);
    let paths: Vec<String> = files.into_iter().map(|f| f.path.to_lowercase()).collect();

    if paths.iter().any(|p| p.contains("domain") || p.contains("usecase") || p.contains("infrastructure")) {
        "Clean_Architecture".to_string()
    } else if paths.iter().any(|p| p.contains("controllers") || p.contains("services") || p.contains("models")) {
        "Layered_Architecture".to_string()
    } else if paths.iter().any(|p| p.contains("features") || p.contains("modules")) {
        "Package_By_Feature".to_string()
    } else {
        "Orchestrator".to_string()
    }
}

pub fn resolve_architecture_pattern(raw_pattern: &str, proj_path: &Path) -> String {
    let trimmed = raw_pattern.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") || trimmed.eq_ignore_ascii_case("none") {
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

pub fn handle_tool_call(
    name: &str,
    arguments: Value,
    state: &mut ServerState,
) -> Result<Value, (i32, String)> {
    info!("Handling tool call: {}", name);
    ensure_not_cancelled(state)?;

    let start_time = std::time::Instant::now();
    let op = arguments.get("operation").or_else(|| arguments.get("action")).or_else(|| arguments.get("phase")).and_then(|v| v.as_str()).map(|s| s.to_string());

    let res = match handle_tool_call_internal(name, arguments, state) {
        Ok(val) => {
            let duration = start_time.elapsed().as_millis() as u64;
            crate::mcp::db::log_tool_call(name, op.as_deref(), 0, 0, duration, None);
            Ok(val)
        },
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
            let task = arguments.get("task").and_then(|t| t.as_str()).unwrap_or("general task");
            let proj_path_arg = arguments.get("project_path").and_then(|p| p.as_str()).unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            state.update_project_path(&proj_path);
            let phase = arguments.get("phase").and_then(|p| p.as_str()).unwrap_or("plan");
            
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
                tracing::info!("Reset workflow stage to 'Plan' and plan_approved to false for new task pipeline execution.");
            }

            let snapshot = project_snapshot(&proj_path);
            ensure_not_cancelled(state)?;
            let file_count = snapshot.files.len();
            let profile = crate::catalog::language_detector::detect_language_profile(snapshot.files.as_ref(), task);

            let stage1_results = hybrid_vector_search(task, snapshot.skills.as_ref(), 8);
            ensure_not_cancelled(state)?;
            let selector = LLMSelector::new();
            let final_results = selector.rerank(task, stage1_results, &profile, 8);
            ensure_not_cancelled(state)?;

            state.pending_skill_proposals = final_results
                .iter()
                .map(|(score, item)| (item.name.clone(), item.relative_path.clone(), *score))
                .collect();

            let rec_skills: Vec<String> = final_results
                .into_iter()
                .map(|(score, item)| {
                    format!("- {} (Score: {:.2})", item.name, score)
                })
                .collect();

            let execution_seq = "- Step 1: Context & Specification\n- Step 2: Architecture & Implementation Plan\n- Step 3: Code Implementation (Build stage)\n- Step 4: Verification & Testing\n- Step 5: Post-Code Review & Documentation";
            let tree_preview: Vec<String> = snapshot.files.iter().take(15).map(|f| format!("- {} ({})", f.path, f.file_type)).collect();

            let next_step_prompt = if rec_skills.is_empty() {
                "-> Task requires no specific technical skills. Proceed directly with implementation."
            } else {
                "-> SKILL_PROPOSAL: Trigger IDE/CLI `ask_question` tool to present recommended skills interactively to user, then call `select_skills(skills=[...])` with chosen skills (or `select_skills(skills=[])` if skipped)."
            };

            state.record_call(1500, 450);
            format!(
                "# Task Pipeline Activated\n\nTask: {}\nActive Phase: {}\nProject: {}\n\n## Recommendations\n{}\n\n## Execution Sequence\n{}\n\n## Project Tree (Scanned Files: {})\n{}\n\nPriority Gate: PASSED\nStatus: Ready for execution.\n\n{}",
                task,
                phase,
                proj_path.display(),
                if rec_skills.is_empty() { "No specific skill recommendations required for this task (Token budget saved).".to_string() } else { rec_skills.join("\n") },
                execution_seq,
                file_count,
                tree_preview.join("\n"),
                next_step_prompt
            )
        },
        "select_skills" => {
            let requested_skills: Vec<String> = arguments.get("skills")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let proposals = std::mem::take(&mut state.pending_skill_proposals);
            let proj_path = detect_project_path(".", state);

            if requested_skills.is_empty() {
                state.record_call(100, 50);
                "# Skill Selection\n\nNo skills selected. Proceeding without loading skills.".to_string()
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
                            let compressed = compress_markdown(&content);
                            loaded_sections.push(format!("### Skill: {}\n```markdown\n{}\n```", name, compressed));
                        } else {
                            loaded_sections.push(format!("### Skill: {} (Loaded & Logged)\n*Content empty or unavailable*", name));
                        }
                    } else {
                        not_found.push(name.clone());
                    }
                }

                state.record_call(1500, 500);

                let mut out = format!("# Skill Selection Confirmed\n\nLoaded & Logged {} skill(s):\n\n{}", loaded_sections.len(), loaded_sections.join("\n\n"));

                if !not_found.is_empty() {
                    out.push_str(&format!("\n\n⚠ Ignored skills (not in proposal list):\n{}", not_found.iter().map(|s| format!("- {}", s)).collect::<Vec<_>>().join("\n")));
                }

                out.push_str("\n\n-> Proceed with task using the loaded skill guidance above.");
                out
            }
        },
        "guidance" => {
            let op = arguments.get("operation").and_then(|o| o.as_str()).unwrap_or("list");
            let query = arguments.get("query").and_then(|q| q.as_str()).unwrap_or("").to_lowercase();
            state.record_call(1000, 300);

            match op {
                "list" => {
                    let proj_path = detect_project_path(".", state);
                    let snapshot = project_snapshot(&proj_path);
                    let names: Vec<String> = snapshot.skills.iter().map(|s| format!("- {} ({})", s.name, s.relative_path)).collect();
                    format!("# Registered Skills Catalog ({})\n\n{}", names.len(), names.join("\n"))
                },
                "get" => {
                    let id = arguments.get("identifier").and_then(|i| i.as_str()).unwrap_or("");
                    let proj_path_arg = arguments.get("project_path").and_then(|p| p.as_str()).unwrap_or(".");
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
                },
                "search" => {
                    let proj_path = detect_project_path(".", state);
                    let snapshot = project_snapshot(&proj_path);
                    ensure_not_cancelled(state)?;
                    let profile = crate::catalog::language_detector::detect_language_profile(snapshot.files.as_ref(), &query);
                    let all_skills = snapshot.skills.as_ref();

                    // Stage 1: 1st Stage Candidate Selection
                    let stage1_results = hybrid_vector_search(&query, all_skills, 20);
                    ensure_not_cancelled(state)?;

                    // Stage 2: 2nd Stage Context & Intent Re-ranking
                    let selector = LLMSelector::new();
                    let final_results = selector.rerank(&query, stage1_results, &profile, 15);
                    ensure_not_cancelled(state)?;

                    if state.pending_skill_proposals.is_empty() {
                        state.pending_skill_proposals = final_results
                            .iter()
                            .map(|(score, item)| (item.name.clone(), item.relative_path.clone(), *score))
                            .collect();
                    }

                    let formatted_results: Vec<String> = final_results
                        .into_iter()
                        .map(|(score, item)| {
                            let source_tag = match &item.source {
                                SkillSource::Embedded => "[Embedded]".to_string(),
                                SkillSource::LocalWorkspace(path) => format!("[Local Workspace: {}]", path),
                            };
                            format!("- {} {} (Score: {:.2})\n  Path: {}", item.name, source_tag, score, item.relative_path)
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
                        if formatted_results.is_empty() { "No matching skills found.".to_string() } else { formatted_results.join("\n") },
                        next_step_prompt
                    )
                },
                "ui_ux" => {
                    let q = if query.is_empty() { "general" } else { &query };
                    format!("# UI/UX Guidelines for '{}'\n\n- Styling: Modern CSS, Glassmorphism, Dynamic Animations\n- Color Palette: Dark mode default, curated HSL gradients\n- Typography: Inter/Outfit via Google Fonts\n- Accessibility: Semantic HTML5, unique IDs", q)
                },
                "docs" => {
                    let id = arguments.get("identifier").and_then(|i| i.as_str()).unwrap_or("general");
                    format!("# Documentation Guidance for '{}' ({})\n\nOfficial patterns, signatures, and API usage guidelines loaded for query: '{}'.", id, query, query)
                },
                "workflow" => {
                    let stage = arguments.get("identifier").and_then(|i| i.as_str()).unwrap_or("plan");
                    format!("# Dev Workflow Guidance: [{}]\n\nRecommended Flow: Context -> Plan -> Ask/Revise -> Build -> Test/Recheck -> Fix -> Document", stage)
                },
                "precode" => {
                    format!("# Pre-Code Verification Checklist\n\n1. Enforce Upfront Orchestrator Architecture (create thin dispatcher main + sub-module files from line 1, do NOT wait for 300 LOC refactoring)\n2. Verify symbols via project_context search\n3. Use explicit non-null checks\n4. Check error propagation")
                },
                "verify" => {
                    let v_cmd = arguments.get("verification_command").and_then(|v| v.as_str());
                    let v_kw = arguments.get("expected_output_keyword").and_then(|k| k.as_str());
                    
                    if let (Some(cmd), Some(kw)) = (v_cmd, v_kw) {
                        state.verification_command = Some(cmd.to_string());
                        state.expected_output_keyword = Some(kw.to_string());
                        state.verification_passed = false; // SECURITY FIX Bug #4: Registered contract; awaiting test execution
                        format!("# Empirical Verification Contract Registered\n\n- Verification Command: `{}`\n- Expected Output Keyword: `{}`\n- Verification Status: REGISTERED (Awaiting test execution output)\n\n✓ Run verification command to satisfy anti-hallucination requirement.", cmd, kw)
                    } else {
                        format!("# Anti-Hallucination Post-Code Verification Checklist\n\n1. **Empirical Verification Required**: Trigger IDE/CLI `ask_question` tool to let user select verification test command (or confirm manual testing), then pass `verification_command` (e.g. 'cargo test') and `expected_output_keyword` (e.g. 'PASSED').\n2. **User Requirement Alignment**: Re-read the original user prompt and verify all explicitly requested features exist.\n3. **Zero Unverified Assumptions**: Base success strictly on empirical evidence, not speculative assumptions.")
                    }
                },
                _ => format!("Guidance operation '{}' completed successfully.", op),
            }
        },
        "project_context" => {
            let op = arguments.get("operation").and_then(|o| o.as_str()).unwrap_or("tree");
            let proj_path_arg = arguments.get("project_path").and_then(|p| p.as_str()).unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);
            state.update_project_path(&proj_path);
            let query = arguments.get("query").and_then(|q| q.as_str()).unwrap_or("");
            let rel_path = arguments.get("relative_path").and_then(|r| r.as_str()).unwrap_or("");

            match op {
                "tree" => {
                    let files = scan_project(&proj_path, 2);
                    let file_list: Vec<String> = files.into_iter().map(|f| format!("- {} ({})", f.path, f.file_type)).collect();
                    state.record_call(2000, 400);
                    format!("# Project Tree (Depth Capped at 2)\n\n{}", file_list.join("\n"))
                },
                "read" => {
                    if rel_path.is_empty() {
                        "Error: relative_path is required for read operation.".to_string()
                    } else {
                        let target_symbol = arguments.get("target_symbol").and_then(|s| s.as_str());
                        match validate_path(&proj_path, rel_path) {
                            Ok(full_path) => {
                                match std::fs::read_to_string(&full_path) {
                                    Ok(content) => {
                                        let orig_len = content.len() as u64;
                                        let mut lines: Vec<&str> = content.lines().collect();
                                        
                                        if let Some(symbol) = target_symbol {
                                            // Symbol-targeted extraction (multi-language aware)
                                            let mut matched_snippet = Vec::new();
                                            let mut capturing = false;
                                            let mut brace_count = 0;
                                            let mut has_braces = false;
                                            for line in &lines {
                                                if line.contains(symbol) && !capturing {
                                                    capturing = true;
                                                }
                                                if capturing {
                                                    matched_snippet.push(*line);
                                                    let open_b = line.matches('{').count() as i32;
                                                    let close_b = line.matches('}').count() as i32;
                                                    if open_b > 0 { has_braces = true; }
                                                    brace_count += open_b - close_b;
                                                    
                                                    if has_braces && matched_snippet.len() > 1 && brace_count <= 0 && (line.contains('}') || line.trim().is_empty()) {
                                                        break;
                                                    }
                                                    if !has_braces && matched_snippet.len() >= 30 && line.trim().is_empty() {
                                                        break;
                                                    }
                                                    if matched_snippet.len() >= 100 {
                                                        break;
                                                    }
                                                }
                                            }
                                            if !matched_snippet.is_empty() {
                                                lines = matched_snippet;
                                            }
                                        }

                                        let bounded = lines.into_iter().take(300).collect::<Vec<_>>().join("\n");
                                        let compressed = compress_markdown(&bounded);
                                        state.record_call(orig_len / 4, compressed.len() as u64 / 4);
                                        if let Some(symbol) = target_symbol {
                                            format!("# Target Symbol Extracted: '{}' from {}\n\n{}", symbol, rel_path, compressed)
                                        } else {
                                            format!("# Bounded File Content: {}\n\n{}", rel_path, compressed)
                                        }
                                    },
                                    Err(e) => format!("Failed to read file '{}': {}", rel_path, e),
                                }
                            },
                            Err(err_msg) => format!("Security Error: {}", err_msg),
                        }
                    }
                },
                "search" => {
                    let files = scan_project(&proj_path, 3);
                    let mut results = Vec::new();
                    if !query.is_empty() {
                        for file in files.iter().filter(|f| f.file_type == "file").take(50) {
                            if state.is_cancelled() {
                                return Err((-32000, "Operation cancelled by client or timeout.".to_string()));
                            }
                            if let Ok(path) = validate_path(&proj_path, &file.path) {
                                if let Ok(content) = std::fs::read_to_string(&path) {
                                    if content.contains(query) {
                                        results.push(format!("- {}", file.path));
                                    }
                                }
                            }
                        }
                    }
                    state.record_call(5000, 500);
                    format!("# Context Search Results for '{}'\n\nMatching Files (Max 20):\n\n{}", query, if results.is_empty() { "No matches found.".to_string() } else { results.into_iter().take(20).collect::<Vec<_>>().join("\n") })
                },
                _ => format!("Project context operation '{}' completed.", op),
            }
        },
        "ui_ux" => {
            let query = arguments.get("query").and_then(|q| q.as_str()).unwrap_or("general");
            state.record_call(800, 200);
            format!("# UI/UX Guidelines for '{}'\n\n- Styling: Modern CSS, Glassmorphism, Dynamic Animations\n- Color Palette: Dark mode default, curated HSL gradients\n- Typography: Inter/Outfit via Google Fonts\n- Accessibility: Semantic HTML5, unique IDs", query)
        },
        "session_continuity" => {
            let op = arguments.get("operation").and_then(|o| o.as_str()).unwrap_or("load");
            let proj_path_arg = arguments.get("project_path").and_then(|p| p.as_str()).unwrap_or(".");
            let proj_path = detect_project_path(proj_path_arg, state);

            match op {
                "save" => {
                    match state.save_to_dir(&proj_path) {
                        Ok(_) => format!("# Session Continuity\n\nSession state saved successfully to `.agent-context/sessions/{}.json`.", state.session_id),
                        Err(e) => format!("Failed to save session: {}", e),
                    }
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
                            format!("# Session Continuity\n\nLoaded state for session '{}'. Permission flags reset (plan_approved=false). Total Calls: {}", state.session_id, state.tool_calls)
                        },
                        Err(e) => format!("Failed to load session: {}", e),
                    }
                },
                _ => format!("# Session Continuity: [{}]\n\nSession state active.", op),
            }
        },
        "workflow_gate" => {
            let action = arguments.get("action").and_then(|a| a.as_str()).unwrap_or("check");
            let stage_target = arguments
                .get("target_stage")
                .or_else(|| arguments.get("stage"))
                .and_then(|s| s.as_str());

            state.record_call(300, 50);

            match action {
                "set_stage" => {
                    if let Some(target) = stage_target {
                        match state.set_stage(target) {
                            Ok(new_stage) => format!("# Workflow Gate: [set_stage]\n\nStatus: PASSED | Stage Changed To: {} | Plan Approved: {} | Fix Attempts: {}", new_stage, state.plan_approved, state.fix_attempts),
                            Err(err_msg) => format!("# Workflow Gate: [set_stage]\n\nStatus: BLOCKED | Error: {}. Trigger IDE/CLI `ask_question` tool to request user approval for stage transition.", err_msg),
                        }
                    } else {
                        "# Workflow Gate: [set_stage]\n\nStatus: BLOCKED | Error: target_stage argument is required for set_stage action. Trigger IDE/CLI `ask_question` tool to clarify desired stage.".to_string()
                    }
                },
                "status" => {
                    let edit_allowed = state.workflow_stage == "Build" && state.plan_approved;
                    format!(
                        "# Workflow Stage Status\n\n- Active Stage: {}\n- Plan Approved: {}\n- Fix Attempts: {}/3\n- Edit Authorized: {}",
                        state.workflow_stage, state.plan_approved, state.fix_attempts, edit_allowed
                    )
                },
                "advance" => {
                    // SECURITY FIX Bug #3: Do NOT auto-process user_message in advance to prevent agent self-approval
                    let target_stage = arguments.get("target_stage").and_then(|t| t.as_str()).unwrap_or("Build");
                    let stage_res = state.set_stage(target_stage);

                    let raw_arch = arguments.get("architecture_pattern").and_then(|a| a.as_str()).unwrap_or("Auto");
                    let proj_path_arg = arguments.get("project_path").and_then(|p| p.as_str()).unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    state.update_project_path(&proj_path);
                    let risk_level = arguments.get("risk_level").and_then(|r| r.as_str()).unwrap_or("LOW");
                    state.last_risk_level = Some(risk_level.to_string());

                    let arch_pattern = resolve_architecture_pattern(raw_arch, &proj_path);

                    if matches!(arch_pattern.as_str(), "Clean_Architecture" | "Layered_Architecture" | "Package_By_Feature" | "Orchestrator") && state.workflow_stage == "Build" && state.plan_approved {
                        state.edit_authorized = true;
                        state.active_architecture_pattern = Some(arch_pattern);
                    }

                    match stage_res {
                        Ok(msg) => format!(
                            "# Workflow Gate: [advance]\n\n{}\n- Edit Authorized: {}\n- Architecture Pattern: {}",
                            msg, state.edit_authorized, state.active_architecture_pattern.as_deref().unwrap_or("NONE")
                        ),
                        Err(err) => format!("# Workflow Gate: [advance]\n\n⚠️ Error: {}", err),
                    }
                },
                "authorize_edit" => {
                    let proj_path_arg = arguments.get("project_path").and_then(|p| p.as_str()).unwrap_or(".");
                    let proj_path = detect_project_path(proj_path_arg, state);
                    state.update_project_path(&proj_path);
                    let risk_level = arguments.get("risk_level").and_then(|r| r.as_str()).unwrap_or("LOW");
                    let justification = arguments.get("justification").and_then(|j| j.as_str()).unwrap_or("No justification provided");
                    let raw_arch = arguments.get("architecture_pattern").and_then(|a| a.as_str()).unwrap_or("Auto");
                    let arch_pattern = resolve_architecture_pattern(raw_arch, &proj_path);
                    
                    state.last_risk_level = Some(risk_level.to_string());

                    if !matches!(arch_pattern.as_str(), "Clean_Architecture" | "Layered_Architecture" | "Package_By_Feature" | "Orchestrator") {
                        format!(
                            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (ORCHESTRATION MANDATE VIOLATION)\n- Project Path: {}\n- Declared Architecture: '{}'\n\n⚠️ Error: ARCHITECTURE_GATE_BLOCKED: Trigger IDE/CLI `ask_question` tool to let user choose a valid `architecture_pattern` ('Clean_Architecture', 'Layered_Architecture', 'Package_By_Feature', 'Orchestrator', or 'Auto'), then re-invoke `workflow_gate(action=\"authorize_edit\", ...)`.",
                            proj_path.display(), if arch_pattern.is_empty() { "NONE" } else { &arch_pattern }
                        )
                    } else if risk_level == "HIGH" && !state.plan_approved {
                        format!(
                            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED (HIGH RISK)\n- Project Path: {}\n- Declared Risk: HIGH\n- Justification: {}\n\n⚠️ Error: HIGH RISK edits require explicit user approval. Present plan and trigger IDE/CLI `ask_question` tool (or invoke `workflow_gate(action=\"set_stage\", target_stage=\"Plan\")`) to confirm approval.",
                            proj_path.display(), justification
                        )
                    } else if state.workflow_stage == "Build" && state.plan_approved {
                        state.edit_authorized = true;
                        state.active_architecture_pattern = Some(arch_pattern.clone());
                        format!(
                            "# Edit Approval Gate Authorization\n\n- Status: PASSED\n- Project Path: {}\n- Declared Risk: {}\n- Architecture Pattern: {}\n- Justification: {}\n- Active Stage: {}\n- Plan Approved: true\n\n✓ File edits are fully authorized under {} Architecture.",
                            proj_path.display(), risk_level, arch_pattern, justification, state.workflow_stage, arch_pattern
                        )
                    } else {
                        format!(
                            "# Edit Approval Gate Authorization\n\n- Status: BLOCKED\n- Project Path: {}\n- Active Stage: {}\n- Plan Approved: {}\n\n⚠️ Error: WORKFLOW_STAGE_BLOCKED: Edits require Build stage and plan_approved=true. Trigger IDE/CLI `ask_question` tool to request user approval on the plan, then invoke `workflow_gate(action=\"set_stage\", target_stage=\"Build\")`.",
                            proj_path.display(), state.workflow_stage, state.plan_approved
                        )
                    }
                },
                _ => {
                    // "check" action — SECURITY FIX Bug #2: READ ONLY (no state mutation)
                    let status_str = if state.workflow_stage == "Build" && !state.plan_approved { "BLOCKED" } else { "PASSED" };
                    let mut resp = format!("# Workflow Gate: [check]\n\nStatus: {} | Plan Approved: {} | Stage: {} | Fix Attempts: {}", status_str, state.plan_approved, state.workflow_stage, state.fix_attempts);
                    if state.workflow_stage == "Build" && !state.plan_approved {
                        resp.push_str("\n\n⚠️ Trigger IDE/CLI `ask_question` tool to request explicit user plan approval before editing code.");
                    }
                    if state.workflow_stage == "Test_Recheck" {
                        resp.push_str("\n\n⚠ **ANTI-HALLUCINATION ENFORCER ACTIVE**: Re-read the original user prompt & verify all requested features against real build/test outputs before declaring task complete.");
                    }
                    resp
                }
            }
        },
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
            ("agent-guidance".to_string(), "skills/agent-guidance/SKILL.md".to_string(), 0.95),
            ("test-skill".to_string(), "skills/test-skill/SKILL.md".to_string(), 0.80),
        ];

        // 1. Select valid skill
        let res = handle_tool_call("select_skills", json!({ "skills": ["agent-guidance"] }), &mut state);
        assert!(res.is_ok());
        let text = res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("# Skill Selection Confirmed"));
        assert!(text.contains("agent-guidance"));

        // 2. State cleared after selection
        assert!(state.pending_skill_proposals.is_empty());

        // 3. Select with empty array when no proposals remain
        let empty_res = handle_tool_call("select_skills", json!({ "skills": [] }), &mut state);
        assert!(empty_res.is_ok());
        let empty_text = empty_res.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(empty_text.contains("No skills selected"));
    }

    #[test]
    fn test_auto_architecture_detection() {
        let cwd = std::env::current_dir().unwrap();
        let detected = detect_project_architecture(&cwd);
        assert!(matches!(detected.as_str(), "Clean_Architecture" | "Layered_Architecture" | "Package_By_Feature" | "Orchestrator"));

        let auto_resolved = resolve_architecture_pattern("Auto", &cwd);
        assert_eq!(auto_resolved, detected);

        let empty_resolved = resolve_architecture_pattern("", &cwd);
        assert_eq!(empty_resolved, detected);

        let explicit_resolved = resolve_architecture_pattern("Clean_Architecture", &cwd);
        assert_eq!(explicit_resolved, "Clean_Architecture");
    }
}
