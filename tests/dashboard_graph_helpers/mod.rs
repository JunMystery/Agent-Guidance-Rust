use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

static NEXT_PORT: AtomicU16 = AtomicU16::new(18200);

pub fn allocate_test_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::SeqCst)
}

pub struct TestServerHandle {
    pub port: u16,
    pub child: Child,
}

impl Drop for TestServerHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn wait_for_server(port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
            let req = format!(
                "GET /health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
                port
            );
            if stream.write_all(req.as_bytes()).is_ok() {
                let mut buf = [0u8; 64];
                if stream.read(&mut buf).is_ok() {
                    return true;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

pub fn spawn_test_server(project_path: &Path) -> TestServerHandle {
    let port = allocate_test_port();
    let bin = env!("CARGO_BIN_EXE_agent-guidance");
    let child = Command::new(bin)
        .args([
            "--dashboard",
            "--port",
            &port.to_string(),
            "--project",
            &project_path.to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn agent-guidance test dashboard server");

    if !wait_for_server(port, Duration::from_secs(6)) {
        panic!("Server on port {} failed to become ready in time", port);
    }

    TestServerHandle { port, child }
}

fn decode_chunked_body(mut input: &str) -> String {
    let mut decoded = String::new();
    while !input.is_empty() {
        let (len_str, rest) = match input.split_once("\r\n") {
            Some(pair) => pair,
            None => break,
        };
        let chunk_len = match usize::from_str_radix(len_str.trim(), 16) {
            Ok(len) => len,
            Err(_) => return input.to_string(),
        };
        if chunk_len == 0 {
            break;
        }
        if rest.len() < chunk_len {
            decoded.push_str(rest);
            break;
        }
        decoded.push_str(&rest[..chunk_len]);
        let after = &rest[chunk_len..];
        input = after.strip_prefix("\r\n").unwrap_or(after);
    }
    decoded
}

pub fn http_get(port: u16, path_and_query: &str) -> (u16, serde_json::Value) {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Failed to connect to test server");
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        path_and_query, port
    );
    stream.write_all(req.as_bytes()).expect("Write error");
    let mut resp = String::new();
    stream.read_to_string(&mut resp).expect("Read error");

    let (headers, body) = resp.split_once("\r\n\r\n").unwrap_or(("", ""));
    let status_code = headers
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);

    let clean_body = if headers.to_lowercase().contains("chunked") {
        decode_chunked_body(body)
    } else {
        body.to_string()
    };

    let json_val: serde_json::Value =
        serde_json::from_str(&clean_body).unwrap_or(serde_json::Value::Null);
    (status_code, json_val)
}

pub struct TestGraphContext {
    pub temp_dir: PathBuf,
    pub db_path: PathBuf,
}

impl TestGraphContext {
    pub fn new(prefix: &str) -> Self {
        let port = allocate_test_port();
        let temp_dir = std::env::temp_dir().join(format!(
            "graph_e2e_{}_{}_{}",
            prefix,
            std::process::id(),
            port
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let ctx = temp_dir.join(".agent-context");
        let _ = std::fs::create_dir_all(&ctx);
        let db_path = ctx.join("code_graph.db");

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS symbols (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                file_path TEXT NOT NULL,
                parent TEXT,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                signature TEXT,
                language TEXT DEFAULT 'rust',
                namespace TEXT,
                receiver TEXT
            );
            CREATE TABLE IF NOT EXISTS symbol_edges (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                weight REAL DEFAULT 1.0,
                confidence REAL DEFAULT 1.0,
                category TEXT DEFAULT 'symbol',
                call_line INTEGER,
                PRIMARY KEY (source_id, target_id, edge_type)
            );",
        )
        .unwrap();

        Self { temp_dir, db_path }
    }

    pub fn insert_symbol(&self, id: &str, name: &str, kind: &str, file: &str, s: usize, e: usize) {
        let conn = rusqlite::Connection::open(&self.db_path).unwrap();
        conn.execute(
            "INSERT INTO symbols (id, name, kind, file_path, start_line, end_line)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, name, kind, file, s as i64, e as i64],
        )
        .unwrap();
    }

    pub fn insert_edge(
        &self,
        src: &str,
        tgt: &str,
        etype: &str,
        weight: f64,
        call_line: Option<usize>,
    ) {
        let conn = rusqlite::Connection::open(&self.db_path).unwrap();
        conn.execute(
            "INSERT INTO symbol_edges (source_id, target_id, edge_type, weight, call_line)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![src, tgt, etype, weight, call_line.map(|l| l as i64)],
        )
        .unwrap();
    }

    #[allow(dead_code)]
    pub fn populate_standard_topology(&self) {
        self.insert_symbol("c::handle_req", "handle_req", "function", "src/controller.rs", 10, 30);
        self.insert_symbol("c::validate", "validate", "function", "src/controller.rs", 35, 50);
        self.insert_symbol("s::process_data", "process_data", "function", "src/service.rs", 10, 40);
        self.insert_symbol("s::transform", "transform", "function", "src/service.rs", 45, 60);
        self.insert_symbol("r::query_db", "query_db", "function", "src/repo.rs", 10, 35);
        self.insert_symbol("u::help_fn", "help_fn", "function", "src/util.rs", 1, 15);

        self.insert_edge("c::handle_req", "c::validate", "calls", 1.0, Some(15));
        self.insert_edge("c::handle_req", "s::process_data", "calls", 1.0, Some(20));
        self.insert_edge("c::validate", "s::process_data", "calls", 1.0, Some(42));
        self.insert_edge("s::process_data", "r::query_db", "calls", 1.0, Some(25));
        self.insert_edge("r::query_db", "c::handle_req", "calls", 1.0, Some(30));
    }
}

impl Drop for TestGraphContext {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}
