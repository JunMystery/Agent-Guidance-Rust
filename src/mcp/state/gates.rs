use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

use super::ServerState;
use super::types::SESSION_STALE_TIMEOUT_SECS;

impl ServerState {
    pub fn priority_gate_path() -> std::path::PathBuf {
        if cfg!(test) {
            std::env::temp_dir().join(format!("gate_passed_test_{}", std::process::id()))
        } else {
            dirs::home_dir()
                .map(|h| h.join(".agent-guidance").join(".gate_passed"))
                .unwrap_or_else(|| std::path::PathBuf::from(".gate_passed"))
        }
    }

    pub fn priority_gate_pass(&mut self) {
        self.priority_gate_passed = true;
        self.last_session_start = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        let path = Self::priority_gate_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let sentinel = serde_json::json!({
            "status": "PASSED",
            "timestamp": self.last_session_start
        });
        let _ = fs::write(&path, serde_json::to_string(&sentinel).unwrap_or_default());
    }

    pub fn priority_gate_check(&mut self) -> Result<(), String> {
        if self.priority_gate_passed {
            if let Some(ts) = self.last_session_start {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if now > ts && now - ts > SESSION_STALE_TIMEOUT_SECS {
                    let mins = (now - ts) / 60;
                    info!(
                        "Session is stale ({}m since last task_pipeline). Agent should re-call task_pipeline.",
                        mins
                    );
                }
            }
            return Ok(());
        }

        let path = Self::priority_gate_path();
        if path.exists() {
            self.priority_gate_passed = true;
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(sentinel) = serde_json::from_str::<serde_json::Value>(&content) {
                    self.last_session_start = sentinel.get("timestamp").and_then(|t| t.as_u64());
                }
            }
            return Ok(());
        }

        Err("PRIORITY_REQUIRED: Priority gate locked. Call agent-guidance-mcp_task_pipeline first to unlock gated tools.".to_string())
    }

    pub fn session_freshness_note(&self) -> Option<String> {
        let ts = self.last_session_start?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now > ts && now - ts > SESSION_STALE_TIMEOUT_SECS {
            let mins = (now - ts) / 60;
            Some(format!(
                "⚠ Session may be stale — {}m since last task_pipeline. Consider re-calling task_pipeline for fresh context.",
                mins
            ))
        } else {
            None
        }
    }

    pub fn approve_plan(&mut self) {
        self.plan_approved = true;
    }

    pub fn load_persisted_architecture(proj_path: &Path) -> Option<String> {
        let arch_file = proj_path.join(".agent-context").join("architecture.json");
        if arch_file.exists() {
            if let Ok(content) = fs::read_to_string(&arch_file) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(pat) = val.get("architecture_pattern").and_then(|p| p.as_str()) {
                        let trimmed = pat.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    pub fn save_persisted_architecture(proj_path: &Path, pattern: &str) -> Result<(), String> {
        let dir = proj_path.join(".agent-context");
        if let Err(e) = fs::create_dir_all(&dir) {
            return Err(format!("Failed to create .agent-context directory: {}", e));
        }
        let arch_file = dir.join("architecture.json");
        let payload = serde_json::json!({
            "architecture_pattern": pattern,
            "updated_at": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
        });
        let content = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
        fs::write(&arch_file, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_stage(&mut self, target: &str) -> Result<String, String> {
        let normalized = match target.trim().to_lowercase().as_str() {
            "context" => "Context",
            "plan" => "Plan",
            "ask_revise" | "ask" | "revise" | "ask/revise" => "Ask_Revise",
            "build" => "Build",
            "test_recheck" | "test" | "recheck" | "test/recheck" => "Test_Recheck",
            "fix" => "Fix",
            "proposal" | "document" | "review" | "proposal/review" => "Proposal",
            _ => {
                return Err(format!(
                    "Invalid workflow stage '{}'. Allowed stages: Context, Plan, Ask_Revise, Build, Test_Recheck, Fix, Proposal (or Review).",
                    target
                ));
            }
        };

        if normalized == "Plan" || normalized == "Context" {
            self.plan_approved = false;
            self.edit_authorized = false;
            // Note: active_architecture_pattern is preserved across stages for session consistency
        }

        if normalized == "Build" && !self.plan_approved {
            return Err("WORKFLOW_STAGE_BLOCKED: Cannot enter 'Build' stage because plan_approved is false. Obtain explicit user approval first.".to_string());
        }

        if normalized == "Fix" {
            self.fix_attempts += 1;
            if self.fix_attempts > 3 {
                self.workflow_stage = "Ask_Revise".to_string();
                self.plan_approved = false;
                self.fix_attempts = 0;
                return Err("WORKFLOW_STAGE_BLOCKED: Circuit breaker triggered after 3 consecutive failed fix attempts. Workflow stage automatically reset to 'Ask_Revise' with plan_approved=false. Please request user guidance.".to_string());
            }
        } else if normalized == "Test_Recheck"
            || normalized == "Proposal"
            || normalized == "Context"
        {
            self.fix_attempts = 0;
        }

        self.workflow_stage = normalized.to_string();
        Ok(self.workflow_stage.clone())
    }

    pub fn process_user_message(&mut self, message: &str) -> bool {
        let msg = message.trim().to_lowercase();
        if msg.is_empty() {
            return false;
        }

        let approval_keywords = [
            "ok",
            "proceed",
            "approved",
            "approve",
            "start",
            "go ahead",
            "do it",
            "làm đi",
            "đồng ý",
            "chấp nhận",
            "yes",
            "yep",
            "lgtm",
            "looks good",
            "agree",
            "let's do it",
            "make the change",
            "exec",
            "triển khai",
            "tiến hành",
            "duyệt",
            "thực thi",
            "chốt",
            "ok bro",
            "được đấy",
        ];

        for kw in approval_keywords {
            if msg.contains(kw) {
                self.plan_approved = true;
                return true;
            }
        }

        false
    }
}
