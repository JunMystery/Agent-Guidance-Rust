//! Test fixtures for graph queries and multi-view graph unit tests.

use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;

pub struct TestDbGuard {
    pub dir: PathBuf,
}

impl Drop for TestDbGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub fn create_test_project(test_name: &str) -> (TestDbGuard, Connection) {
    let temp_dir = std::env::temp_dir().join(format!("{}_{}", test_name, std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    let context_dir = temp_dir.join(".agent-context");
    fs::create_dir_all(&context_dir).unwrap();

    let db_path = context_dir.join("code_graph.db");
    let conn = Connection::open(&db_path).unwrap();

    conn.execute_batch(
        "CREATE TABLE files (
            path TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            size INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            indexed_at INTEGER NOT NULL
        );
        CREATE TABLE symbols (
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
        CREATE TABLE symbol_edges (
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            edge_type TEXT NOT NULL,
            weight REAL DEFAULT 1.0,
            confidence REAL DEFAULT 1.0,
            category TEXT DEFAULT 'symbol',
            call_line INTEGER,
            PRIMARY KEY (source_id, target_id, edge_type)
        );
        CREATE TABLE content_chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            content_hash TEXT NOT NULL,
            chunk_text TEXT NOT NULL
        );",
    ).unwrap();

    (TestDbGuard { dir: temp_dir }, conn)
}

pub fn populate_test_topology(conn: &Connection) {
    conn.execute_batch(
        "INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES
            ('src/controller.rs', 'h1', 1200, 100, 100),
            ('src/service.rs', 'h2', 800, 100, 100),
            ('src/repository.rs', 'h3', 500, 100, 100),
            ('src/util.rs', 'h4', 200, 100, 100);

        INSERT INTO content_chunks (file_path, start_line, end_line, content_hash, chunk_text) VALUES
            ('src/controller.rs', 1, 70, 'ch1', 'controller code'),
            ('src/service.rs', 1, 40, 'ch2', 'service code'),
            ('src/repository.rs', 1, 25, 'ch3', 'repo code'),
            ('src/util.rs', 1, 10, 'ch4', 'util code');

        INSERT INTO symbols (id, name, kind, file_path, start_line, end_line) VALUES
            ('src/controller.rs::handle_request', 'handle_request', 'function', 'src/controller.rs', 10, 30),
            ('src/controller.rs::validate_input', 'validate_input', 'function', 'src/controller.rs', 35, 50),
            ('src/controller.rs::ReqContext', 'ReqContext', 'struct', 'src/controller.rs', 55, 70),
            ('src/service.rs::process_data', 'process_data', 'function', 'src/service.rs', 1, 40),
            ('src/repository.rs::query_db', 'query_db', 'function', 'src/repository.rs', 1, 25),
            ('src/util.rs::helper', 'helper', 'function', 'src/util.rs', 1, 10);

        INSERT INTO symbol_edges (source_id, target_id, edge_type, weight, confidence, call_line) VALUES
            ('src/controller.rs::handle_request', 'src/controller.rs::validate_input', 'calls', 1.0, 1.0, 15),
            ('src/controller.rs::handle_request', 'src/service.rs::process_data', 'calls', 1.0, 1.0, 20),
            ('src/controller.rs::validate_input', 'src/service.rs::process_data', 'calls', 1.0, 1.0, 42),
            ('src/service.rs::process_data', 'src/repository.rs::query_db', 'calls', 1.0, 1.0, 25),
            ('src/repository.rs::query_db', 'src/controller.rs::handle_request', 'calls', 1.0, 1.0, 20);",
    ).unwrap();
}
