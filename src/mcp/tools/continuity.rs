use serde_json::{Value, json};
use std::path::Path;

use crate::mcp::learnings;
use crate::mcp::state::ServerState;
use super::{detect_project_path, ensure_not_cancelled};

pub(crate) fn handle(
    arguments: Value,
    state: &mut ServerState,
) -> Result<String, (i32, String)> {
    ensure_not_cancelled(state)?;
    let op = arguments
        .get("operation")
        .and_then(|o| o.as_str())
        .unwrap_or("load");
    let proj_path_arg = arguments
        .get("project_path")
        .and_then(|p| p.as_str())
        .unwrap_or(".");
    let proj_path = detect_project_path(proj_path_arg, state);

    let resp = match op {
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
      };
    Ok(resp)
}