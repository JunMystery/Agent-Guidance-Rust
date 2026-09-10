use tracing::info;

pub fn register_cli_clients(bin_str: &str, server_id: &str) {
    register_vscode_cli(bin_str, server_id);
    register_claude_cli(bin_str, server_id);
    register_codex_cli(bin_str, server_id);
}

fn register_vscode_cli(bin_str: &str, server_id: &str) {
    let vscode_payload = serde_json::json!({
        "name": server_id,
        "type": "stdio",
        "command": bin_str,
        "args": []
    })
    .to_string();

    let cli_candidates: &[&[&str]] = if cfg!(target_os = "windows") {
        &[&["code.cmd", "code"], &["code-insiders.cmd", "code-insiders"]]
    } else {
        &[&["code"], &["code-insiders"]]
    };

    for variants in cli_candidates {
        for cmd in *variants {
            if let Ok(output) = std::process::Command::new(cmd)
                .args(["--add-mcp", &vscode_payload])
                .output()
            {
                if output.status.success() {
                    info!("Successfully registered with {} via CLI (--add-mcp)", cmd);
                    break;
                }
            }
        }
    }
}

fn register_claude_cli(bin_str: &str, server_id: &str) {
    let claude_cmds = if cfg!(target_os = "windows") {
        &["claude.cmd", "claude.exe", "claude"][..]
    } else {
        &["claude"][..]
    };
    for cmd in claude_cmds {
        if let Ok(output) = std::process::Command::new(cmd)
            .args(["mcp", "add", "--scope", "user", server_id, "--", bin_str])
            .output()
        {
            if output.status.success() {
                info!("Successfully registered with Claude Code via CLI");
                break;
            }
        }
    }
}

fn register_codex_cli(bin_str: &str, server_id: &str) {
    let codex_cmds = if cfg!(target_os = "windows") {
        &["codex.cmd", "codex.exe", "codex"][..]
    } else {
        &["codex"][..]
    };
    for cmd in codex_cmds {
        if let Ok(output) = std::process::Command::new(cmd)
            .args(["mcp", "add", server_id, "--", bin_str])
            .output()
        {
            if output.status.success() {
                info!("Successfully registered with Codex via CLI");
                break;
            }
        }
    }
}
