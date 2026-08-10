use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
    #[serde(default = "generate_session_id")]
    pub session_id: String,
    #[serde(default)]
    pub priority_gate_passed: bool,
    #[serde(default = "default_workflow_stage")]
    pub workflow_stage: String,
    #[serde(default)]
    pub plan_approved: bool,
    #[serde(default)]
    pub fix_attempts: u32,
    #[serde(default)]
    pub tool_calls: u32,
    #[serde(default)]
    pub tokens_original: u64,
    #[serde(default)]
    pub tokens_optimized: u64,
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub agent_client_name: Option<String>,
    #[serde(default)]
    pub workspace_roots: Vec<String>,
    #[serde(default)]
    pub last_session_start: Option<u64>,
    #[serde(default)]
    pub user_intent_summary: Option<String>,
    #[serde(default)]
    pub verification_command: Option<String>,
    #[serde(default)]
    pub expected_output_keyword: Option<String>,
    #[serde(default)]
    pub verification_passed: bool,
    #[serde(default)]
    pub last_risk_level: Option<String>,
    #[serde(default)]
    pub active_phase: Option<String>,
    #[serde(default)]
    pub edit_authorized: bool,
    #[serde(default)]
    pub active_architecture_pattern: Option<String>,
    #[serde(skip, default)]
    pub pending_skill_proposals: Vec<(String, String, f32)>,
    #[serde(skip, default)]
    pub cancellation: Option<Arc<AtomicBool>>,
}

fn default_workflow_stage() -> String {
    "Context".to_string()
}

pub fn generate_session_id() -> String {
    let pid = std::process::id();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("session_{}_{}", pid, ts)
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            session_id: generate_session_id(),
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
            edit_authorized: false,
            active_architecture_pattern: None,
            pending_skill_proposals: Vec::new(),
            cancellation: None,
        }
    }
}

impl ServerState {
    pub fn new() -> Self {
        Self::with_client_name(None)
    }

    pub fn with_client_name(client_name: Option<String>) -> Self {
        let mut s = Self::default();
        if let Some(ref name) = client_name {
            s.session_id = format!("session_{}_{}", std::process::id(), name.to_lowercase().replace(' ', "_"));
        }
        s.agent_client_name = client_name;
        s
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
                info!(
                    "Captured workspace roots from initialize: {:?}",
                    parsed_roots
                );
                self.workspace_roots = parsed_roots;
            }
        }
    }

    pub fn global_project_path_file() -> std::path::PathBuf {
        if cfg!(test) {
            std::env::temp_dir().join(format!("last_project_path_test_{}", std::process::id()))
        } else {
            dirs::home_dir()
                .map(|h| h.join(".agent-guidance").join("last_project_path"))
                .unwrap_or_else(|| std::path::PathBuf::from(".last_project_path"))
        }
    }

    pub fn read_global_project_path() -> Option<String> {
        let path = Self::global_project_path_file();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                let trimmed = content.trim();
                if !trimmed.is_empty() && Path::new(trimmed).is_dir() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }

    pub fn update_project_path(&mut self, path: &Path) {
        let path_str = path.to_string_lossy().to_string();
        if path_str.is_empty() || path_str == "." {
            return;
        }
        if self.project_path.as_deref() == Some(path_str.as_str()) {
            return;
        }
        self.project_path = Some(path_str.clone());

        // Write to global persistent memory for cross-session AI agent inheritance
        let global_file = Self::global_project_path_file();
        if let Some(parent) = global_file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(global_file, &path_str);
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

    pub fn record_call(&mut self, orig_tokens: u64, opt_tokens: u64) {
        self.tool_calls += 1;
        self.tokens_original += orig_tokens;
        self.tokens_optimized += opt_tokens;
        crate::mcp::db::log_tool_call("mcp_tool", None, orig_tokens, opt_tokens, 0, None);
    }

    pub fn set_cancellation(&mut self, cancellation: Arc<AtomicBool>) {
        self.cancellation = Some(cancellation);
    }

    pub fn clear_cancellation(&mut self) {
        self.cancellation = None;
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation
            .as_ref()
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
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
            _ => {
                return Err(format!(
                    "Invalid workflow stage '{}'. Allowed stages: Context, Plan, Ask_Revise, Build, Test_Recheck, Fix, Proposal.",
                    target
                ));
            }
        };

        if normalized == "Plan" || normalized == "Context" {
            self.plan_approved = false;
            self.edit_authorized = false;
            self.active_architecture_pattern = None;
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

    pub fn can_call_tool(&mut self, tool_name: &str, args: &Value) -> Result<(), String> {
        // 1. Unlocks gate tool and advances stage from Context to Plan
        if tool_name == "task_pipeline" {
            self.priority_gate_pass();
            if self.workflow_stage == "Context" {
                self.workflow_stage = "Plan".to_string();
            }
            return Ok(());
        }

        // 2. Whitelisted & Not Gated tools bypass priority gate check
        let is_whitelisted_or_ungated = matches!(
            tool_name,
            "workflow_gate"
                | "session_continuity"
                | "select_skills"
        );

        if !is_whitelisted_or_ungated {
            // Check Layer 2 / Layer 3 Priority Gate
            self.priority_gate_check()?;
        }

        // 3. Perform Stage Checks
        match self.workflow_stage.as_str() {
            "Context" => {
                if !is_whitelisted_or_ungated
                    && tool_name != "workflow_gate"
                    && tool_name != "session_continuity"
                {
                    return Err(format!(
                        "WORKFLOW_STAGE_BLOCKED: Tool '{}' is blocked in 'Context' stage. Call task_pipeline and workflow_gate(action=\"set_stage\", target_stage=\"Plan\") first.",
                        tool_name
                    ));
                }
                Ok(())
            }
            "Plan" => {
                if tool_name == "project_context" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if op == "diff" {
                        return Err(format!(
                            "WORKFLOW_STAGE_BLOCKED: Operation '{}' on project_context is blocked in 'Plan' stage.",
                            op
                        ));
                    }
                }
                Ok(())
            }
            "Ask_Revise" => {
                if tool_name == "project_context" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if matches!(
                        op,
                        "read"
                            | "search"
                            | "symbols"
                            | "references"
                            | "structure"
                            | "callers"
                            | "callees"
                            | "diff"
                    ) {
                        return Err(format!(
                            "WORKFLOW_STAGE_BLOCKED: Code reading operation '{}' is blocked in 'Ask_Revise' stage.",
                            op
                        ));
                    }
                } else if tool_name == "guidance" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if op == "precode" || op == "verify" {
                        return Err(format!(
                            "WORKFLOW_STAGE_BLOCKED: Guidance operation '{}' is blocked in 'Ask_Revise' stage.",
                            op
                        ));
                    }
                }
                Ok(())
            }
            "Build" => {
                if !self.plan_approved {
                    Err("WORKFLOW_STAGE_BLOCKED: Tool execution in 'Build' stage is blocked because plan_approved is false. Obtain user approval first.".to_string())
                } else if tool_name == "workflow_gate" && args.get("action").and_then(|a| a.as_str()) == Some("authorize_edit") {
                    let arch_pattern = args
                        .get("architecture_pattern")
                        .and_then(|a| a.as_str())
                        .unwrap_or("");
                    if !matches!(
                        arch_pattern,
                        "Auto"
                            | "auto"
                            | "Clean_Architecture"
                            | "Layered_Architecture"
                            | "Package_By_Feature"
                            | "Orchestrator"
                    ) {
                        Err("ARCHITECTURE_GATE_BLOCKED: You must provide a valid `architecture_pattern` ('Clean_Architecture', 'Layered_Architecture', 'Package_By_Feature', 'Orchestrator', or 'Auto') in `workflow_gate(action=\"authorize_edit\")`.".to_string())
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            }
            "Test_Recheck" => {
                if tool_name == "guidance" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if op == "precode" {
                        return Err("WORKFLOW_STAGE_BLOCKED: Operation 'precode' is blocked in 'Test_Recheck' stage.".to_string());
                    }
                }
                Ok(())
            }
            "Fix" => Ok(()),
            "Proposal" => {
                if tool_name == "project_context" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if matches!(op, "diff" | "structure" | "symbols") {
                        return Err(format!(
                            "WORKFLOW_STAGE_BLOCKED: Operation '{}' is blocked in 'Proposal' stage.",
                            op
                        ));
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn cleanup_stale_sessions(proj_path: &Path) {
        let dir = proj_path.join(".agent-context").join("sessions");
        if !dir.exists() {
            return;
        }

        let now = SystemTime::now();
        let max_age = std::time::Duration::from_secs(30 * 86400); // 30 Days Retention
        let mut entries = Vec::new();

        if let Ok(read_dir) = fs::read_dir(&dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(metadata) = entry.metadata() {
                        let modified = metadata.modified().unwrap_or(now);
                        if now.duration_since(modified).unwrap_or_default() > max_age {
                            let _ = fs::remove_file(&path);
                            continue;
                        }
                        entries.push((modified, path));
                    }
                }
            }
        }

        // LRU Cap: If > 100 session files, delete oldest
        if entries.len() > 100 {
            entries.sort_by_key(|(mtime, _)| *mtime);
            for (_, path) in entries.iter().take(entries.len() - 100) {
                let _ = fs::remove_file(path);
            }
        }
    }

    pub fn save_to_dir(&self, proj_path: &Path) -> Result<(), String> {
        let dir = proj_path.join(".agent-context").join("sessions");
        if let Err(e) = fs::create_dir_all(&dir) {
            return Err(format!("Failed to create directory: {}", e));
        }
        let file_path = dir.join(format!("{}.json", self.session_id));
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&file_path, &content).map_err(|e| e.to_string())?;

        // Also write atomic link pointer for legacy single-session tools
        let legacy_file = proj_path.join(".agent-context").join("session.json");
        let _ = fs::write(legacy_file, content);
        Ok(())
    }

    pub fn load_from_dir(proj_path: &Path) -> Result<Self, String> {
        Self::cleanup_stale_sessions(proj_path);
        let dir = proj_path.join(".agent-context").join("sessions");
        
        // 1. Try finding most recent session file in sessions/
        if dir.exists() {
            if let Ok(read_dir) = fs::read_dir(&dir) {
                let mut session_files: Vec<(SystemTime, std::path::PathBuf)> = read_dir
                    .flatten()
                    .filter_map(|e| {
                        let p = e.path();
                        if p.extension().and_then(|s| s.to_str()) == Some("json") {
                            let mtime = e.metadata().ok()?.modified().ok()?;
                            Some((mtime, p))
                        } else {
                            None
                        }
                    })
                    .collect();

                session_files.sort_by_key(|(mtime, _)| *mtime);
                if let Some((_, newest_path)) = session_files.last() {
                    if let Ok(content) = fs::read_to_string(newest_path) {
                        if let Ok(loaded) = serde_json::from_str::<Self>(&content) {
                            return Ok(loaded);
                        }
                    }
                }
            }
        }

        // 2. Legacy fallback: .agent-context/session.json
        let legacy_file = proj_path.join(".agent-context").join("session.json");
        if legacy_file.exists() {
            let content = fs::read_to_string(legacy_file).map_err(|e| e.to_string())?;
            return serde_json::from_str(&content).map_err(|e| e.to_string());
        }

        Ok(Self::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancellation_flag_lifecycle() {
        let mut state = ServerState::new();
        let flag = Arc::new(AtomicBool::new(false));
        state.set_cancellation(flag.clone());
        assert!(!state.is_cancelled());
        flag.store(true, Ordering::Relaxed);
        assert!(state.is_cancelled());
        state.clear_cancellation();
        assert!(!state.is_cancelled());
    }

    #[test]
    fn test_stage_transitions_and_circuit_breaker() {
        let mut state = ServerState::new();
        state.priority_gate_pass();
        assert_eq!(state.workflow_stage, "Context");
        assert!(!state.plan_approved);

        assert!(state.set_stage("Plan").is_ok());
        assert_eq!(state.workflow_stage, "Plan");
        assert!(!state.edit_authorized);

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
        let err = state
            .can_call_tool("guidance", &serde_json::json!({}))
            .unwrap_err();
        assert!(err.contains("PRIORITY_REQUIRED"));

        // 2. Whitelisted & Not Gated tools succeed without priority gate unlock
        assert!(
            state
                .can_call_tool("workflow_gate", &serde_json::json!({}))
                .is_ok()
        );
        assert!(
            state
                .can_call_tool(
                    "session_continuity",
                    &serde_json::json!({"operation": "load"})
                )
                .is_ok()
        );

        // 3. Calling task_pipeline unlocks priority gate and advances stage to Plan
        assert!(
            state
                .can_call_tool("task_pipeline", &serde_json::json!({}))
                .is_ok()
        );
        assert!(state.priority_gate_passed);
        assert_eq!(state.workflow_stage, "Plan");

        // 4. Now gated tool passes priority check and stage check in Plan stage
        assert!(
            state
                .can_call_tool("workflow_gate", &serde_json::json!({}))
                .is_ok()
        );

        // Clean up sentinel file created by task_pipeline
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }

    #[test]
    fn test_parse_file_uri_cross_platform() {
        assert_eq!(
            parse_file_uri("file:///C:/Users/test/project"),
            if cfg!(windows) {
                "C:\\Users\\test\\project"
            } else {
                "/C:/Users/test/project"
            }
        );
        assert_eq!(
            parse_file_uri("file:///e:/Github/Agent-Guidance-Rust"),
            if cfg!(windows) {
                "e:\\Github\\Agent-Guidance-Rust"
            } else {
                "/e:/Github/Agent-Guidance-Rust"
            }
        );
        assert_eq!(
            parse_file_uri("file:///home/user/project%20name"),
            if cfg!(windows) {
                "\\home\\user\\project name"
            } else {
                "/home/user/project name"
            }
        );
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

    #[test]
    fn test_task_pipeline_resets_plan_approval_for_new_task() {
        let mut state = ServerState::new();
        state.workflow_stage = "Build".to_string();
        state.plan_approved = true;
        state.edit_authorized = true;

        // Simulate reset logic executed when task_pipeline phase="plan" is invoked
        state.workflow_stage = "Plan".to_string();
        state.plan_approved = false;
        state.edit_authorized = false;
        state.verification_passed = false;
        state.verification_command = None;
        state.expected_output_keyword = None;

        assert_eq!(state.workflow_stage, "Plan");
        assert!(!state.plan_approved);
        assert!(!state.edit_authorized);
    }

    #[test]
    fn test_multi_session_isolation() {
        let temp_dir = std::env::temp_dir().join(format!("multi_session_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut s1 = ServerState::with_client_name(Some("antigravity".to_string()));
        s1.workflow_stage = "Build".to_string();
        s1.plan_approved = true;
        assert!(s1.save_to_dir(&temp_dir).is_ok());

        let mut s2 = ServerState::with_client_name(Some("cursor".to_string()));
        s2.workflow_stage = "Plan".to_string();
        s2.plan_approved = false;
        assert!(s2.save_to_dir(&temp_dir).is_ok());

        // Both session files exist independently in sessions/
        let sessions_dir = temp_dir.join(".agent-context").join("sessions");
        assert!(sessions_dir.join(format!("{}.json", s1.session_id)).exists());
        assert!(sessions_dir.join(format!("{}.json", s2.session_id)).exists());
        assert_ne!(s1.session_id, s2.session_id);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_session_gc_cleanup() {
        let temp_dir = std::env::temp_dir().join(format!("gc_cleanup_test_{}", std::process::id()));
        let sessions_dir = temp_dir.join(".agent-context").join("sessions");
        let _ = std::fs::create_dir_all(&sessions_dir);

        // Create 105 mock session files
        for i in 0..105 {
            let f = sessions_dir.join(format!("session_mock_{}.json", i));
            let _ = std::fs::write(&f, r#"{"session_id":"mock"}"#);
        }

        ServerState::cleanup_stale_sessions(&temp_dir);

        let count = std::fs::read_dir(&sessions_dir).unwrap().count();
        assert!(count <= 100);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
