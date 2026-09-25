use std::io::{self, Write};
use anyhow::Result;

pub fn run_setup_server_wizard() -> Result<bool> {
    println!("============================================================");
    println!("  Agent Guidance — Server Worker Interactive Setup Wizard   ");
    println!("============================================================");

    let bind = prompt_default("Listen Address", "0.0.0.0");
    let worker_port_str = prompt_default("ML Worker API Port", "11998");
    let worker_port: u16 = worker_port_str.parse().unwrap_or(11998);

    let dashboard_port_str = prompt_default("Admin Dashboard Port", "11997");
    let dashboard_port: u16 = dashboard_port_str.parse().unwrap_or(11997);

    let api_key = prompt_default("Optional Bearer API Key (leave empty for none)", "");

    println!();
    println!("Generating service configuration scripts for your OS...");

    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "agent-guidance".to_string());

    if cfg!(target_os = "linux") {
        let systemd_content = generate_linux_systemd(&exe, &bind, worker_port, dashboard_port, &api_key);
        if let Some(home) = dirs::home_dir() {
            let user_unit = home.join(".config").join("systemd").join("user").join("agent-guidance.service");
            if let Some(parent) = user_unit.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::write(&user_unit, &systemd_content).is_ok() {
                println!("[OK] Generated systemd user unit at: {:?}", user_unit);
                println!("  To activate: systemctl --user daemon-reload && systemctl --user enable --now agent-guidance");
            }
        }
    } else if cfg!(target_os = "macos") {
        let launchd_content = generate_macos_launchd(&exe, &bind, worker_port, dashboard_port, &api_key);
        if let Some(home) = dirs::home_dir() {
            let plist = home.join("Library").join("LaunchAgents").join("com.junmystery.agent-guidance.plist");
            if let Some(parent) = plist.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::write(&plist, &launchd_content).is_ok() {
                println!("[OK] Generated launchd plist at: {:?}", plist);
                println!("  To activate: launchctl load {:?}", plist);
            }
        }
    } else if cfg!(target_os = "windows") {
        let sch_cmd = generate_windows_task_cmd(&exe, &bind, worker_port, dashboard_port, &api_key);
        println!("[OK] Windows Task Scheduler command:");
        println!("  {}", sch_cmd);
    }

    println!();
    println!("[OK] Server configuration ready.");
    println!("  Run directly: {} --server --bind {} --worker-port {} --port {}", exe, bind, worker_port, dashboard_port);
    println!("============================================================");
    Ok(true)
}

fn prompt_default(prompt: &str, default: &str) -> String {
    print!("? {} [{}]: ", prompt, default);
    let _ = io::stdout().flush();
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn generate_linux_systemd(bin: &str, bind: &str, w_port: u16, d_port: u16, key: &str) -> String {
    let key_flag = if key.is_empty() { String::new() } else { format!(" --api-key {}", key) };
    format!(
        "[Unit]\nDescription=Agent Guidance Remote ML Worker\nAfter=network.target\n\n[Service]\nType=simple\nExecStart={} --server --bind {} --worker-port {} --port {}{}\nRestart=always\nRestartSec=5\n\n[Install]\nWantedBy=default.target\n",
        bin, bind, w_port, d_port, key_flag
    )
}

pub fn generate_macos_launchd(bin: &str, bind: &str, w_port: u16, d_port: u16, key: &str) -> String {
    let key_arg = if key.is_empty() { String::new() } else { format!("<string>--api-key</string><string>{}</string>", key) };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n<key>Label</key><string>com.junmystery.agent-guidance</string>\n<key>ProgramArguments</key>\n<array>\n<string>{}</string>\n<string>--server</string>\n<string>--bind</string>\n<string>{}</string>\n<string>--worker-port</string>\n<string>{}</string>\n<string>--port</string>\n<string>{}</string>\n{}\n</array>\n<key>RunAtLoad</key><true/>\n<key>KeepAlive</key><true/>\n</dict>\n</plist>\n",
        bin, bind, w_port, d_port, key_arg
    )
}

pub fn generate_windows_task_cmd(bin: &str, bind: &str, w_port: u16, d_port: u16, key: &str) -> String {
    let key_flag = if key.is_empty() { String::new() } else { format!(" --api-key {}", key) };
    format!(
        "schtasks /create /tn \"AgentGuidanceServer\" /tr \"\\\"{}\\\" --server --bind {} --worker-port {} --port {}{}\" /sc onlogon /rl highest /f",
        bin, bind, w_port, d_port, key_flag
    )
}
