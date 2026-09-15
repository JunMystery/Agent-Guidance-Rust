use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;

fn temp_dir_for_test(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ag_test_{}_{}_{}", name, std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros()));
    let _ = fs::remove_dir_all(&p);
    p
}

fn insert_test_file(conn: &Connection, path: &str) {
    conn.execute(
        "INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES (?1, 'h', 1, 1, 1)",
        rusqlite::params![path],
    ).unwrap();
}

fn insert_test_symbol(conn: &Connection, id: &str, name: &str, kind: &str, path: &str, parent: Option<&str>, start: usize, end: usize) {
    conn.execute(
        "INSERT INTO symbols (id, name, kind, file_path, parent, start_line, end_line, signature)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, name, kind, path, parent, start as i64, end as i64, format!("sig {}", name)],
    ).unwrap();
}

#[test]
fn test_dispatch_rust_trait_binding() {
    let mut conn = Connection::open_in_memory().unwrap();
    crate::context::db::schema::init_schema(&mut conn).unwrap();

    let dir = temp_dir_for_test("rust_trait");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let trait_code = r#"
pub trait Processor {
    fn process(&self, data: &str);
}
"#;
    let impl_code = r#"
use super::Processor;

pub struct BatchWorker;

impl Processor for BatchWorker {
    fn process(&self, data: &str) {
        println!("{}", data);
    }
}
"#;
    fs::write(src_dir.join("lib.rs"), trait_code).unwrap();
    fs::write(src_dir.join("worker.rs"), impl_code).unwrap();

    insert_test_file(&conn, "src/lib.rs");
    insert_test_file(&conn, "src/worker.rs");

    insert_test_symbol(&conn, "src/lib.rs::trait::Processor::L2", "Processor", "trait", "src/lib.rs", None, 2, 4);
    insert_test_symbol(&conn, "src/lib.rs::function::process::L3", "process", "function", "src/lib.rs", Some("Processor"), 3, 3);
    insert_test_symbol(&conn, "src/worker.rs::struct::BatchWorker::L4", "BatchWorker", "struct", "src/worker.rs", None, 4, 4);
    insert_test_symbol(&conn, "src/worker.rs::function::process::L7", "process", "function", "src/worker.rs", Some("BatchWorker"), 7, 9);

    let _ = crate::context::indexer::cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();

    let mut stmt = conn.prepare("SELECT edge_type, source_id, target_id FROM symbol_edges WHERE edge_type IN ('implements_trait', 'implements_method')").unwrap();
    let rows: Vec<(String, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().flatten().collect();

    assert!(rows.iter().any(|(t, s, tg)| t == "implements_trait" && s.contains("BatchWorker") && tg.contains("Processor")));
    assert!(rows.iter().any(|(t, s, tg)| t == "implements_method" && s.contains("src/worker.rs::function::process") && tg.contains("src/lib.rs::function::process")));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_dispatch_ts_interface_binding() {
    let mut conn = Connection::open_in_memory().unwrap();
    crate::context::db::schema::init_schema(&mut conn).unwrap();

    let dir = temp_dir_for_test("ts_interface");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let iface_code = r#"
export interface Logger {
    log(msg: string): void;
}
"#;
    let class_code = r#"
export class ConsoleLogger implements Logger {
    log(msg: string): void {
        console.log(msg);
    }
}
"#;
    fs::write(src_dir.join("logger.ts"), iface_code).unwrap();
    fs::write(src_dir.join("console.ts"), class_code).unwrap();

    insert_test_file(&conn, "src/logger.ts");
    insert_test_file(&conn, "src/console.ts");

    insert_test_symbol(&conn, "src/logger.ts::interface::Logger::L2", "Logger", "interface", "src/logger.ts", None, 2, 4);
    insert_test_symbol(&conn, "src/logger.ts::function::log::L3", "log", "function", "src/logger.ts", Some("Logger"), 3, 3);
    insert_test_symbol(&conn, "src/console.ts::class::ConsoleLogger::L2", "ConsoleLogger", "class", "src/console.ts", None, 2, 6);
    insert_test_symbol(&conn, "src/console.ts::function::log::L3", "log", "function", "src/console.ts", Some("ConsoleLogger"), 3, 5);

    let _ = crate::context::indexer::cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();

    let mut stmt = conn.prepare("SELECT edge_type, source_id, target_id FROM symbol_edges WHERE edge_type IN ('implements_trait', 'implements_method')").unwrap();
    let rows: Vec<(String, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().flatten().collect();

    assert!(rows.iter().any(|(t, s, tg)| t == "implements_trait" && s.contains("ConsoleLogger") && tg.contains("Logger")));
    assert!(rows.iter().any(|(t, s, tg)| t == "implements_method" && s.contains("src/console.ts::function::log") && tg.contains("src/logger.ts::function::log")));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_dispatch_generic_verb_suppressed() {
    let mut conn = Connection::open_in_memory().unwrap();
    crate::context::db::schema::init_schema(&mut conn).unwrap();

    let dir = temp_dir_for_test("generic_verb");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let code_a = "pub trait Stringer { fn string(&self) -> String; }";
    let code_b = "use super::Stringer; pub struct Item; impl Stringer for Item { fn string(&self) -> String { String::new() } }";
    fs::write(src_dir.join("a.rs"), code_a).unwrap();
    fs::write(src_dir.join("b.rs"), code_b).unwrap();

    insert_test_file(&conn, "src/a.rs");
    insert_test_file(&conn, "src/b.rs");

    insert_test_symbol(&conn, "src/a.rs::trait::Stringer::L1", "Stringer", "trait", "src/a.rs", None, 1, 1);
    insert_test_symbol(&conn, "src/a.rs::function::string::L1", "string", "function", "src/a.rs", Some("Stringer"), 1, 1);
    insert_test_symbol(&conn, "src/b.rs::struct::Item::L1", "Item", "struct", "src/b.rs", None, 1, 1);
    insert_test_symbol(&conn, "src/b.rs::function::string::L1", "string", "function", "src/b.rs", Some("Item"), 1, 1);

    let _ = crate::context::indexer::cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();

    let mut stmt = conn.prepare("SELECT edge_type FROM symbol_edges WHERE edge_type = 'dynamic_dispatch'").unwrap();
    let rows: Vec<String> = stmt.query_map([], |r| r.get(0)).unwrap().flatten().collect();

    assert!(rows.is_empty(), "Generic verb 'string' must not synthesize dynamic_dispatch!");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_dispatch_python_protocol_subclass() {
    let mut conn = Connection::open_in_memory().unwrap();
    crate::context::db::schema::init_schema(&mut conn).unwrap();

    let dir = temp_dir_for_test("py_protocol");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let code_repo = "class Repository:\n    def find_all(self):\n        pass\n";
    let code_impl = "from repo import Repository\nclass SqlRepository(Repository):\n    def find_all(self):\n        return []\n";
    fs::write(src_dir.join("repo.py"), code_repo).unwrap();
    fs::write(src_dir.join("sql_repo.py"), code_impl).unwrap();

    insert_test_file(&conn, "src/repo.py");
    insert_test_file(&conn, "src/sql_repo.py");

    insert_test_symbol(&conn, "src/repo.py::class::Repository::L1", "Repository", "class", "src/repo.py", None, 1, 3);
    insert_test_symbol(&conn, "src/repo.py::function::find_all::L2", "find_all", "function", "src/repo.py", Some("Repository"), 2, 3);
    insert_test_symbol(&conn, "src/sql_repo.py::class::SqlRepository::L2", "SqlRepository", "class", "src/sql_repo.py", None, 2, 4);
    insert_test_symbol(&conn, "src/sql_repo.py::function::find_all::L3", "find_all", "function", "src/sql_repo.py", Some("SqlRepository"), 3, 4);

    let _ = crate::context::indexer::cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();

    let mut stmt = conn.prepare("SELECT edge_type, source_id, target_id FROM symbol_edges WHERE edge_type = 'implements_trait'").unwrap();
    let rows: Vec<(String, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().flatten().collect();

    assert!(rows.iter().any(|(t, s, tg)| t == "implements_trait" && s.contains("SqlRepository") && tg.contains("Repository")), "Python superclass inheritance must synthesize implements_trait");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_dispatch_go_interface_assertion() {
    let mut conn = Connection::open_in_memory().unwrap();
    crate::context::db::schema::init_schema(&mut conn).unwrap();

    let dir = temp_dir_for_test("go_assertion");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let code_iface = "package main\ntype Notifier interface {\n    Notify(msg string)\n}\n";
    let code_impl = "package main\ntype EmailNotifier struct{}\nfunc (e *EmailNotifier) Notify(msg string){}\nvar _ Notifier = (*EmailNotifier)(nil)\n";
    fs::write(src_dir.join("iface.go"), code_iface).unwrap();
    fs::write(src_dir.join("impl.go"), code_impl).unwrap();

    insert_test_file(&conn, "src/iface.go");
    insert_test_file(&conn, "src/impl.go");

    insert_test_symbol(&conn, "src/iface.go::interface::Notifier::L2", "Notifier", "interface", "src/iface.go", None, 2, 4);
    insert_test_symbol(&conn, "src/iface.go::function::Notify::L3", "Notify", "function", "src/iface.go", Some("Notifier"), 3, 3);
    insert_test_symbol(&conn, "src/impl.go::struct::EmailNotifier::L2", "EmailNotifier", "struct", "src/impl.go", None, 2, 2);
    insert_test_symbol(&conn, "src/impl.go::function::Notify::L3", "Notify", "function", "src/impl.go", Some("EmailNotifier"), 3, 3);

    let _ = crate::context::indexer::cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();

    let mut stmt = conn.prepare("SELECT edge_type, source_id, target_id FROM symbol_edges WHERE edge_type = 'implements_trait'").unwrap();
    let rows: Vec<(String, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().flatten().collect();

    assert!(rows.iter().any(|(t, s, tg)| t == "implements_trait" && s.contains("EmailNotifier") && tg.contains("Notifier")), "Go compile-time assertion must produce implements_trait edge");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_dispatch_qualified_namespace_trait_binding() {
    let mut conn = Connection::open_in_memory().unwrap();
    crate::context::db::schema::init_schema(&mut conn).unwrap();

    let dir = temp_dir_for_test("qualified_trait");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let code_trait = "pub trait TaskHandler { fn handle(&self); }";
    let code_impl = "use crate::tasks::TaskHandler; pub struct MyHandler; impl tasks::TaskHandler for MyHandler { fn handle(&self) {} }";
    fs::write(src_dir.join("traits.rs"), code_trait).unwrap();
    fs::write(src_dir.join("handler.rs"), code_impl).unwrap();

    insert_test_file(&conn, "src/traits.rs");
    insert_test_file(&conn, "src/handler.rs");

    insert_test_symbol(&conn, "src/traits.rs::trait::TaskHandler::L1", "TaskHandler", "trait", "src/traits.rs", None, 1, 1);
    insert_test_symbol(&conn, "src/traits.rs::function::handle::L1", "handle", "function", "src/traits.rs", Some("TaskHandler"), 1, 1);
    insert_test_symbol(&conn, "src/handler.rs::struct::MyHandler::L1", "MyHandler", "struct", "src/handler.rs", None, 1, 1);
    insert_test_symbol(&conn, "src/handler.rs::function::handle::L1", "handle", "function", "src/handler.rs", Some("MyHandler"), 1, 1);

    let _ = crate::context::indexer::cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();

    let mut stmt = conn.prepare("SELECT edge_type, source_id, target_id FROM symbol_edges WHERE edge_type = 'implements_trait'").unwrap();
    let rows: Vec<(String, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().flatten().collect();

    assert!(rows.iter().any(|(t, s, tg)| t == "implements_trait" && s.contains("MyHandler") && tg.contains("TaskHandler")), "Qualified trait name must match via leaf fallback");
    let _ = fs::remove_dir_all(&dir);
}
