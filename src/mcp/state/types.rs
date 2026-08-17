use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const SESSION_STALE_TIMEOUT_SECS: u64 = 300;

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
    #[serde(default)]
    pub modified_files: Vec<String>,
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
            modified_files: Vec::new(),
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
            s.session_id = format!(
                "session_{}_{}",
                std::process::id(),
                name.to_lowercase().replace(' ', "_")
            );
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

    /// Records a relative file path modified during this active session.
    pub fn record_modified_file(&mut self, rel_path: &str) {
        let clean = rel_path.trim().replace('\\', "/");
        if !clean.is_empty() && !self.modified_files.contains(&clean) {
            self.modified_files.push(clean);
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

}
