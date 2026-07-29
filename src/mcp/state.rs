use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

const SESSION_STALE_TIMEOUT_SECS: u64 = 300;

/// Parse cross-platform file:// URIs safely into native OS path strings.
pub fn parse_file_uri(uri: &str) -> String {
    let mut decoded = uri.replace("%20", " ");
    if decoded.starts_with("file://") {
        decoded = decoded.trim_start_matches("file://").to_string();
    }

    if cfg!(windows) {
        // Handle Windows leading slash e.g. /C:/path or /e:/path -> C:\path or E:\path
        if decoded.starts_with('/') && decoded.chars().nth(2) == Some(':') {
            decoded = decoded.trim_start_matches('/').to_string();
        }
        decoded = decoded.replace('/', "\\");
    }

    decoded
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerState {
    pub priority_gate_passed: bool,
    pub workflow_stage: String,
    pub plan_approved: bool,
    pub fix_attempts: u32,
    pub tool_calls: u32,
    pub tokens_original: u64,
    pub tokens_optimized: u64,
    pub project_path: Option<String>,
    pub agent_client_name: Option<String>,
    pub workspace_roots: Vec<String>,
    pub last_session_start: Option<u64>,
    pub user_intent_summary: Option<String>,
    pub verification_command: Option<String>,
    pub expected_output_keyword: Option<String>,
    pub verification_passed: bool,
    pub last_risk_level: Option<String>,
    pub active_phase: Option<String>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            priority_gate_passed: false,
            workflow_stage: "Context".to_string(),
            plan_approved: false,
            fix_attempts: 0,
            tool_calls: 0,
            tokens_original: 0,
            tokens_optimized: 0,
            project_path: None,
            agent_client_name: None,
            workspace_roots: Vec::new(),
            last_session_start: None,
            user_intent_summary: None,
            verification_command: None,
            expected_output_keyword: None,
            verification_passed: false,
            last_risk_level: None,
            active_phase: None,
        }
    }
}

impl ServerState {
    pub fn new() -> Self {
        Self::with_client_name(None)
    }

    pub fn with_client_name(client_name: Option<String>) -> Self {
        Self {
            priority_gate_passed: false,
            workflow_stage: "Context".to_string(),
            plan_approved: false,
            fix_attempts: 0,
            tool_calls: 0,
            tokens_original: 0,
            tokens_optimized: 0,
            project_path: None,
            workspace_roots: Vec::new(),
            last_session_start: None,
            user_intent_summary: None,
            verification_command: None,
            expected_output_keyword: None,
            verification_passed: false,
            last_risk_level: None,
            active_phase: None,
            agent_client_name: client_name,
        }
    }


    pub fn set_roots_from_initialize(&mut self, params: &Value) {
        if let Some(roots) = params.get("roots").and_then(|r| r.as_array()) {
            let mut parsed_roots = Vec::new();
            for r in roots {
                if let Some(uri) = r.get("uri").and_then(|u| u.as_str()) {
                    let path_str = parse_file_uri(uri);
                    if !path_str.is_empty() {
                        parsed_roots.push(path_str);
                    }
                }
            }
            if !parsed_roots.is_empty() {
                info!("Captured workspace roots from initialize: {:?}", parsed_roots);
                self.workspace_roots = parsed_roots;
            }
        }
    }

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
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
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
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                if now > ts && now - ts > SESSION_STALE_TIMEOUT_SECS {
                    let mins = (now - ts) / 60;
                    info!("Session is stale ({}m since last task_pipeline). Agent should re-call task_pipeline.", mins);
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
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
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

    pub fn record_call(&mut self, orig_tokens: u64, opt_tokens: u64) {
        self.tool_calls += 1;
        self.tokens_original += orig_tokens;
        self.tokens_optimized += opt_tokens;
    }

    pub fn set_stage(&mut self, target: &str) -> Result<String, String> {
        let normalized = match target.trim().to_lowercase().as_str() {
            "context" => "Context",
            "plan" => "Plan",
            "ask_revise" | "ask" | "revise" | "ask/revise" => "Ask_Revise",
            "build" => "Build",
            "test_recheck" | "test" | "recheck" | "test/recheck" => "Test_Recheck",
            "fix" => "Fix",
            "proposal" | "document" => "Proposal",
            _ => return Err(format!("Invalid workflow stage '{}'. Allowed stages: Context, Plan, Ask_Revise, Build, Test_Recheck, Fix, Proposal.", target)),
        };

        if normalized == "Plan" {
            self.plan_approved = false;
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
        } else if normalized == "Test_Recheck" || normalized == "Proposal" || normalized == "Context" {
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
            "ok", "proceed", "approved", "approve", "start", "go ahead",
            "do it", "làm đi", "đồng ý", "chấp nhận", "yes", "yep", "lgtm",
            "looks good", "agree", "let's do it", "make the change", "exec",
            "triển khai", "tiến hành", "duyệt", "thực thi", "chốt", "ok bro", "được đấy"
        ];

        for kw in approval_keywords {
            if msg.contains(kw) {
                self.plan_approved = true;
                return true;
            }
        }

        false
    }

    pub fn can_call_tool(&mut self, tool_name: &str, args: &Value) -> Result<(), String> {
        // 1. Unlocks gate tool
        if tool_name == "task_pipeline" {
            self.priority_gate_pass();
            return Ok(());
        }

        // 2. Whitelisted & Not Gated tools bypass priority gate check
        let is_whitelisted_or_ungated = matches!(
            tool_name,
            "health_check" | "diagnose" | "token_stats" | "require_edit_approval" | "usage_report"
            | "workflow_gate" | "session_continuity"
        );

        if !is_whitelisted_or_ungated {
            // Check Layer 2 / Layer 3 Priority Gate
            self.priority_gate_check()?;
        }

        // 3. Perform Stage Checks
        match self.workflow_stage.as_str() {
            "Context" => {
                if !is_whitelisted_or_ungated && tool_name != "workflow_gate" && tool_name != "session_continuity" {
                    return Err(format!(
                        "WORKFLOW_STAGE_BLOCKED: Tool '{}' is blocked in 'Context' stage. Call task_pipeline and workflow_gate(action=\"set_stage\", target_stage=\"Plan\") first.",
                        tool_name
                    ));
                }
                Ok(())
            },
            "Plan" => {
                if tool_name == "project_context" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if op == "diff" || op == "architecture" {
                        return Err(format!("WORKFLOW_STAGE_BLOCKED: Operation '{}' on project_context is blocked in 'Plan' stage.", op));
                    }
                }
                Ok(())
            },
            "Ask_Revise" => {
                if tool_name == "project_context" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if matches!(op, "read" | "search" | "symbols" | "references" | "structure" | "callers" | "callees" | "diff") {
                        return Err(format!("WORKFLOW_STAGE_BLOCKED: Code reading operation '{}' is blocked in 'Ask_Revise' stage.", op));
                    }
                } else if tool_name == "guidance" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if op == "precode" || op == "verify" {
                        return Err(format!("WORKFLOW_STAGE_BLOCKED: Guidance operation '{}' is blocked in 'Ask_Revise' stage.", op));
                    }
                }
                Ok(())
            },
            "Build" => {
                if !self.plan_approved {
                    Err("WORKFLOW_STAGE_BLOCKED: Tool execution in 'Build' stage is blocked because plan_approved is false. Obtain user approval first.".to_string())
                } else {
                    Ok(())
                }
            },
            "Test_Recheck" => {
                if tool_name == "guidance" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if op == "precode" {
                        return Err("WORKFLOW_STAGE_BLOCKED: Operation 'precode' is blocked in 'Test_Recheck' stage.".to_string());
                    }
                }
                Ok(())
            },
            "Fix" => Ok(()),
            "Proposal" => {
                if tool_name == "project_context" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if matches!(op, "diff" | "structure" | "symbols") {
                        return Err(format!("WORKFLOW_STAGE_BLOCKED: Operation '{}' is blocked in 'Proposal' stage.", op));
                    }
                }
                Ok(())
            },
            _ => Ok(()),
        }
    }

    pub fn save_to_dir(&self, proj_path: &Path) -> Result<(), String> {
        let dir = proj_path.join(".agent-context");
        if let Err(e) = fs::create_dir_all(&dir) {
            return Err(format!("Failed to create directory: {}", e));
        }
        let file_path = dir.join("session.json");
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(file_path, content).map_err(|e| e.to_string())
    }

    pub fn load_from_dir(proj_path: &Path) -> Result<Self, String> {
        let file_path = proj_path.join(".agent-context").join("session.json");
        if !file_path.exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(file_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_transitions_and_circuit_breaker() {
        let mut state = ServerState::new();
        state.priority_gate_pass();
        assert_eq!(state.workflow_stage, "Context");
        assert!(!state.plan_approved);

        assert!(state.set_stage("Plan").is_ok());
        assert_eq!(state.workflow_stage, "Plan");

        // Transitioning to Build should fail if not approved
        assert!(state.set_stage("Build").is_err());

        // Process approval
        assert!(state.process_user_message("Looks good, proceed!"));
        assert!(state.plan_approved);

        // Now transition to Build succeeds
        assert!(state.set_stage("Build").is_ok());

        // Test Circuit breaker in Fix stage
        assert!(state.set_stage("Fix").is_ok());
        assert_eq!(state.fix_attempts, 1);
        assert!(state.set_stage("Fix").is_ok());
        assert_eq!(state.fix_attempts, 2);
        assert!(state.set_stage("Fix").is_ok());
        assert_eq!(state.fix_attempts, 3);

        // 4th Fix attempt triggers circuit breaker reset
        let res = state.set_stage("Fix");
        assert!(res.is_err());
        assert_eq!(state.workflow_stage, "Ask_Revise");
        assert!(!state.plan_approved);
        assert_eq!(state.fix_attempts, 0);
    }

    #[test]
    fn test_priority_gate_and_tool_categories() {
        // Ensure sentinel file from previous runs is cleaned up for test isolation
        let path = ServerState::priority_gate_path();
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        let mut state = ServerState::new();

        // 1. Gated tool fails initially with PRIORITY_REQUIRED
        let err = state.can_call_tool("guidance", &serde_json::json!({})).unwrap_err();
        assert!(err.contains("PRIORITY_REQUIRED"));

        // 2. Whitelisted & Not Gated tools succeed without priority gate unlock
        assert!(state.can_call_tool("health_check", &serde_json::json!({})).is_ok());
        assert!(state.can_call_tool("diagnose", &serde_json::json!({})).is_ok());
        assert!(state.can_call_tool("token_stats", &serde_json::json!({})).is_ok());
        assert!(state.can_call_tool("require_edit_approval", &serde_json::json!({})).is_ok());
        assert!(state.can_call_tool("usage_report", &serde_json::json!({})).is_ok());
        assert!(state.can_call_tool("workflow_gate", &serde_json::json!({})).is_ok());
        assert!(state.can_call_tool("session_continuity", &serde_json::json!({"operation": "load"})).is_ok());

        // 3. Calling task_pipeline unlocks priority gate
        assert!(state.can_call_tool("task_pipeline", &serde_json::json!({})).is_ok());
        assert!(state.priority_gate_passed);

        // 4. Now gated tool passes priority check (and workflow_gate passes in Context stage)
        assert!(state.can_call_tool("workflow_gate", &serde_json::json!({})).is_ok());

        // Clean up sentinel file created by task_pipeline
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }

    #[test]
    fn test_parse_file_uri_cross_platform() {
        assert_eq!(parse_file_uri("file:///C:/Users/test/project"), if cfg!(windows) { "C:\\Users\\test\\project" } else { "C:/Users/test/project" });
        assert_eq!(parse_file_uri("file:///e:/Github/Agent-Guidance-Rust"), if cfg!(windows) { "e:\\Github\\Agent-Guidance-Rust" } else { "e:/Github/Agent-Guidance-Rust" });
        assert_eq!(parse_file_uri("file:///home/user/project%20name"), if cfg!(windows) { "\\home\\user\\project name" } else { "/home/user/project name" });
    }

    #[test]
    fn test_vietnamese_approval_keywords() {
        let mut state = ServerState::new();
        assert!(!state.plan_approved);

        assert!(state.process_user_message("chốt triển khai đi bro"));
        assert!(state.plan_approved);

        state.plan_approved = false;
        assert!(state.process_user_message("đồng ý duyệt"));
        assert!(state.plan_approved);
    }
}
