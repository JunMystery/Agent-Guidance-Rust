//! Stdio LSP client with timeout protection and JSON-RPC dispatch.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use super::detect::LspServerConfig;
use super::protocol::{decode_message, encode_message, path_to_uri, LspLocation};

/// Active client session with an LSP server process.
pub struct LspClient {
    child: Child,
    stdin: std::process::ChildStdin,
    rx: Receiver<Value>,
    next_id: u64,
    pub server_name: String,
}

impl LspClient {
    /// Launches the language server and performs the standard initialize handshake.
    pub fn start(config: &LspServerConfig, proj_path: &Path) -> Result<Self, String> {
        let mut child = Command::new(&config.command)
            .args(&config.args)
            .current_dir(proj_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", config.command, e))?;

        let stdin = child.stdin.take().ok_or("Failed to open child stdin")?;
        let mut stdout = child.stdout.take().ok_or("Failed to open child stdout")?;

        let (tx, rx) = channel::<Value>();
        std::thread::spawn(move || {
            let mut buf = Vec::with_capacity(4096);
            let mut temp = [0u8; 2048];
            loop {
                match stdout.read(&mut temp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&temp[..n]);
                        while let Some((val, consumed)) = decode_message(&buf) {
                            let _ = tx.send(val);
                            buf.drain(..consumed);
                        }
                    }
                }
            }
        });

        let mut client = Self {
            child,
            stdin,
            rx,
            next_id: 1,
            server_name: config.name.clone(),
        };

        // Handshake: initialize
        let root_uri = path_to_uri(proj_path);
        let init_params = json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {}
        });

        let _ = client.call("initialize", init_params, Duration::from_millis(2000))?;

        // Handshake: initialized notification
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        });
        client.send_raw(&notif)?;

        Ok(client)
    }

    /// Sends a raw JSON-RPC payload without awaiting a response.
    fn send_raw(&mut self, payload: &Value) -> Result<(), String> {
        let encoded = encode_message(payload);
        self.stdin
            .write_all(&encoded)
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("Failed to write to LSP stdin: {}", e))
    }

    /// Dispatches a JSON-RPC request and awaits the corresponding response within `timeout`.
    pub fn call(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let req_id = self.next_id;
        self.next_id += 1;

        let req = json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params
        });

        self.send_raw(&req)?;

        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if let Ok(msg) = self.rx.recv_timeout(remaining.min(Duration::from_millis(100))) {
                if msg.get("id").and_then(|id| id.as_u64()) == Some(req_id) {
                    if let Some(err) = msg.get("error") {
                        return Err(format!("LSP error: {}", err));
                    }
                    return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
                }
            }
        }

        Err(format!("LSP request '{}' timed out after {:?}", method, timeout))
    }

    /// Queries all references of a symbol at the given document coordinates.
    pub fn find_references(&mut self, uri: &str, line: u32, character: u32) -> Result<Vec<LspLocation>, String> {
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character },
            "context": { "includeDeclaration": true }
        });

        let res = self.call("textDocument/references", params, Duration::from_millis(2000))?;
        parse_locations(&res)
    }

    /// Queries the definition of a symbol at the given document coordinates.
    pub fn goto_definition(&mut self, uri: &str, line: u32, character: u32) -> Result<Vec<LspLocation>, String> {
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });

        let res = self.call("textDocument/definition", params, Duration::from_millis(2000))?;
        parse_locations(&res)
    }

    /// Queries the type definition of a symbol at the given document coordinates.
    pub fn goto_type_definition(&mut self, uri: &str, line: u32, character: u32) -> Result<Vec<LspLocation>, String> {
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        });

        let res = self.call("textDocument/typeDefinition", params, Duration::from_millis(2000))?;
        parse_locations(&res)
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn parse_locations(val: &Value) -> Result<Vec<LspLocation>, String> {
    if val.is_null() {
        return Ok(Vec::new());
    }
    if let Some(arr) = val.as_array() {
        let mut locs = Vec::new();
        for item in arr {
            if let Ok(loc) = serde_json::from_value::<LspLocation>(item.clone()) {
                locs.push(loc);
            }
        }
        Ok(locs)
    } else if let Ok(single) = serde_json::from_value::<LspLocation>(val.clone()) {
        Ok(vec![single])
    } else {
        Ok(Vec::new())
    }
}
