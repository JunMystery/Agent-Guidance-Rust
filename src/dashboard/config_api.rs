use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::time::Duration;
use tiny_http::Request;

use crate::client::RemoteMlClient;
use crate::config::{AppConfig, ConfigWatcher, ServerConfig, current_config};
use crate::mcp::config::run_setup;

#[derive(Deserialize)]
struct UpdateServerConfigRequest {
    #[serde(default)]
    url: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    dashboard_port: Option<u16>,
    #[serde(default)]
    dashboard_bind: Option<String>,
    #[serde(default)]
    auto_configure_ide: Option<bool>,
}

#[derive(Deserialize)]
struct TestConnectionRequest {
    url: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

pub fn handle_api_server_config(mut request: Request) {
    match request.method() {
        tiny_http::Method::Get => {
            let cfg = current_config();
            let payload = json!({
                "status": "ok",
                "config": *cfg
            });
            super::router::json_response(request, 200, &payload);
        }
        tiny_http::Method::Post => {
            let mut body = String::new();
            if let Err(e) = request.as_reader().read_to_string(&mut body) {
                let err = json!({ "status": "error", "message": format!("Failed to read request body: {}", e) });
                super::router::json_response(request, 400, &err);
                return;
            }

            let req_data: UpdateServerConfigRequest = match serde_json::from_str(&body) {
                Ok(data) => data,
                Err(e) => {
                    let err = json!({ "status": "error", "message": format!("Invalid JSON payload: {}", e) });
                    super::router::json_response(request, 400, &err);
                    return;
                }
            };

            let mut cfg = (*current_config()).clone();
            if !req_data.url.is_empty() {
                cfg.server.url = req_data.url;
            }
            if let Some(m) = req_data.mode {
                cfg.server.mode = m;
            }
            if let Some(key) = req_data.api_key {
                cfg.server.api_key = key;
            }
            if let Some(t) = req_data.timeout_ms {
                cfg.server.timeout_ms = t;
            }
            if let Some(dp) = req_data.dashboard_port {
                cfg.dashboard.port = dp;
            }
            if let Some(db) = req_data.dashboard_bind {
                cfg.dashboard.bind = db;
            }

            if let Err(e) = ConfigWatcher::global().update(cfg.clone()) {
                let err = json!({ "status": "error", "message": format!("Failed to save config: {}", e) });
                super::router::json_response(request, 500, &err);
                return;
            }

            let mut ide_status = "Skipped".to_string();
            if req_data.auto_configure_ide.unwrap_or(true) {
                if let Ok(exe) = env::current_exe() {
                    match run_setup(&exe) {
                        Ok(_) => ide_status = "Updated across IDEs".to_string(),
                        Err(e) => ide_status = format!("Partial error: {}", e),
                    }
                }
            }

            let payload = json!({
                "status": "ok",
                "message": "Configuration saved successfully",
                "ide_configuration": ide_status,
                "config": cfg
            });
            super::router::json_response(request, 200, &payload);
        }
        _ => {
            super::router::json_response(request, 405, &json!({ "error": "Method not allowed" }));
        }
    }
}

pub fn handle_api_test_connection(mut request: Request) {
    if request.method() != &tiny_http::Method::Post {
        super::router::json_response(request, 405, &json!({ "error": "POST required" }));
        return;
    }

    let mut body = String::new();
    if let Err(e) = request.as_reader().read_to_string(&mut body) {
        let err = json!({ "status": "error", "message": format!("Failed to read request body: {}", e) });
        super::router::json_response(request, 400, &err);
        return;
    }

    let req_data: TestConnectionRequest = match serde_json::from_str(&body) {
        Ok(data) => data,
        Err(e) => {
            let err = json!({ "status": "error", "message": format!("Invalid JSON: {}", e) });
            super::router::json_response(request, 400, &err);
            return;
        }
    };

    let mut test_server = ServerConfig::default();
    test_server.url = req_data.url;
    if let Some(key) = req_data.api_key {
        test_server.api_key = key;
    }
    if let Some(t) = req_data.timeout_ms {
        test_server.timeout_ms = t;
    }

    let client = RemoteMlClient::new(&test_server);
    match client.check_health() {
        Ok((health, latency)) => {
            let payload = json!({
                "status": "ok",
                "latency_ms": latency.as_millis(),
                "server_health": health
            });
            super::router::json_response(request, 200, &payload);
        }
        Err(e) => {
            let payload = json!({
                "status": "error",
                "message": format!("Failed to connect to server: {}", e)
            });
            super::router::json_response(request, 200, &payload);
        }
    }
}
