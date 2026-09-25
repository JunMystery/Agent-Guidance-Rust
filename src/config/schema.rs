use serde::{Deserialize, Serialize};

pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:11998";
pub const DEFAULT_TIMEOUT_MS: u64 = 3500;
pub const DEFAULT_DASHBOARD_PORT: u16 = 11997;
pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub resilience: ResilienceConfig,
    #[serde(default)]
    pub dashboard: DashboardConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            resilience: ResilienceConfig::default(),
            dashboard: DashboardConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerConfig {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default = "default_url")]
    pub url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<SshTunnelConfig>,
}

fn default_mode() -> String {
    "local".to_string()
}

fn default_protocol() -> String {
    "http".to_string()
}

fn default_url() -> String {
    DEFAULT_SERVER_URL.to_string()
}

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            protocol: default_protocol(),
            url: default_url(),
            api_key: String::new(),
            timeout_ms: default_timeout_ms(),
            ssh: None,
        }
    }
}

impl ServerConfig {
    pub fn is_remote(&self) -> bool {
        self.mode.eq_ignore_ascii_case("remote")
    }

    pub fn endpoint_url(&self, endpoint: &str) -> String {
        let base = self.url.trim_end_matches('/');
        let path = endpoint.trim_start_matches('/');
        format!("{}/{}", base, path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SshTunnelConfig {
    pub host: String,
    pub user: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    #[serde(default)]
    pub identity_file: String,
    #[serde(default = "default_ssh_remote_port")]
    pub remote_port: u16,
}

fn default_ssh_port() -> u16 {
    22
}

fn default_ssh_remote_port() -> u16 {
    11998
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResilienceConfig {
    #[serde(default = "default_true")]
    pub fallback_to_local: bool,
    #[serde(default = "default_true")]
    pub cache_vectors: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            fallback_to_local: true,
            cache_vectors: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardConfig {
    #[serde(default = "default_dashboard_port")]
    pub port: u16,
    #[serde(default = "default_bind_addr")]
    pub bind: String,
    #[serde(default = "default_true")]
    pub auto_open: bool,
}

fn default_dashboard_port() -> u16 {
    DEFAULT_DASHBOARD_PORT
}

fn default_bind_addr() -> String {
    DEFAULT_BIND_ADDR.to_string()
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            port: default_dashboard_port(),
            bind: default_bind_addr(),
            auto_open: true,
        }
    }
}
