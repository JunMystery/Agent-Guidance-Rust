use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerState {
    pub workflow_stage: String,
    pub plan_approved: bool,
    pub tool_calls: u32,
    pub tokens_original: u64,
    pub tokens_optimized: u64,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            workflow_stage: "Context".to_string(),
            plan_approved: true,
            tool_calls: 0,
            tokens_original: 0,
            tokens_optimized: 0,
        }
    }
}

impl ServerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_call(&mut self, orig_tokens: u64, opt_tokens: u64) {
        self.tool_calls += 1;
        self.tokens_original += orig_tokens;
        self.tokens_optimized += opt_tokens;
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
