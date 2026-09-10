    use super::*;

    #[test]
    fn test_extract_symbols_and_chunks() {
        let code = r#"
pub struct PaymentService {
    api_key: String,
}

impl PaymentService {
    pub fn process_payment(&self, amount: u64) -> bool {
        let timeout = 30;
        true
    }
}
"#;
        let symbols = extract_symbols_from_content("src/payment.rs", code);
        assert!(symbols.iter().any(|s| s.name == "PaymentService" && s.kind == "struct"));
        assert!(symbols.iter().any(|s| s.name == "process_payment" && s.kind == "function"));

        let chunks = chunk_code_content("src/payment.rs", code, 50, 10);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().any(|c| c.text.contains("process_payment")));
    }

    #[test]
    fn test_extract_edges_uses_symbol_id() {
        let code = r#"
pub fn helper() -> bool {
    true
}

pub fn main_runner() {
    helper();
}
"#;
        let symbols = extract_symbols_from_content("src/app.rs", code);
        let edges = extract_edges_from_content("src/app.rs", code, &symbols);
        assert!(!edges.is_empty(), "Should extract at least 1 call edge");
        let edge = &edges[0];
        assert_eq!(edge.edge_type, "calls");
        assert!(edge.source_id.contains("src/app.rs::function::main_runner"), "source_id must be caller symbol ID: {}", edge.source_id);
        assert!(edge.target_id.contains("src/app.rs::function::helper"), "target_id must be callee symbol ID: {}", edge.target_id);
    }

    #[test]
    fn test_cross_edges_generic_verb_suppression() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::context::db::schema::init_schema(&mut conn).unwrap();

        conn.execute("INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES ('pkg/a.rs', 'h', 1, 1, 1)", []).unwrap();
        conn.execute("INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES ('pkg/b.rs', 'h', 1, 1, 1)", []).unwrap();

        conn.execute(
            "INSERT INTO symbols (id, name, kind, file_path, parent, start_line, end_line, signature)
             VALUES ('pkg/a.rs::function::new::L1', 'new', 'function', 'pkg/a.rs', NULL, 1, 5, 'pub fn new()')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO symbols (id, name, kind, file_path, parent, start_line, end_line, signature)
             VALUES ('pkg/b.rs::function::caller::L1', 'caller', 'function', 'pkg/b.rs', NULL, 1, 5, 'pub fn caller()')",
            [],
        ).unwrap();

        let dir = std::env::temp_dir().join(format!("test_verb_suppression_{}", std::process::id()));
        let pkg_dir = dir.join("pkg");
        let _ = std::fs::create_dir_all(&pkg_dir);
        std::fs::write(pkg_dir.join("a.rs"), "pub fn new() {}").unwrap();
        std::fs::write(pkg_dir.join("b.rs"), "pub fn caller() { new(); }").unwrap();

        let _ = cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();
        let mut stmt = conn.prepare("SELECT edge_type FROM symbol_edges WHERE edge_type = 'calls'").unwrap();
        let edges: Vec<String> = stmt.query_map([], |r| r.get(0)).unwrap().flatten().collect();
        assert!(edges.is_empty(), "Generic verb 'new' must not be arbitrarily linked across files!");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cross_edges_imports_file() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::context::db::schema::init_schema(&mut conn).unwrap();

        conn.execute("INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES ('src/utils.ts', 'h', 1, 1, 1)", []).unwrap();
        conn.execute("INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES ('src/app.ts', 'h', 1, 1, 1)", []).unwrap();

        conn.execute(
            "INSERT INTO symbols (id, name, kind, file_path, parent, start_line, end_line, signature)
             VALUES ('src/utils.ts::module::utils.ts::L1', 'utils.ts', 'module', 'src/utils.ts', NULL, 1, 10, 'mod src/utils.ts')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO symbols (id, name, kind, file_path, parent, start_line, end_line, signature)
             VALUES ('src/app.ts::module::app.ts::L1', 'app.ts', 'module', 'src/app.ts', NULL, 1, 10, 'mod src/app.ts')",
            [],
        ).unwrap();

        let dir = std::env::temp_dir().join(format!("test_imports_file_{}", std::process::id()));
        let src_dir = dir.join("src");
        let _ = std::fs::create_dir_all(&src_dir);
        std::fs::write(src_dir.join("utils.ts"), "export const PI = 3.14;").unwrap();
        std::fs::write(src_dir.join("app.ts"), "import { PI } from './utils';").unwrap();

        let _ = cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();
        let mut stmt = conn.prepare("SELECT edge_type, source_id, target_id FROM symbol_edges WHERE edge_type = 'imports_file'").unwrap();
        let rows: Vec<(String, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().flatten().collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "imports_file");
        assert_eq!(rows[0].1, "src/app.ts::module::app.ts::L1");
        assert_eq!(rows[0].2, "src/utils.ts::module::utils.ts::L1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cross_edges_shares_package() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::context::db::schema::init_schema(&mut conn).unwrap();

        conn.execute("INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES ('src/A.kt', 'h', 1, 1, 1)", []).unwrap();
        conn.execute("INSERT INTO files (path, content_hash, size, modified_at, indexed_at) VALUES ('src/B.kt', 'h', 1, 1, 1)", []).unwrap();

        let dir = std::env::temp_dir().join(format!("test_shares_pkg_{}", std::process::id()));
        let pkg_dir = dir.join("src");
        let _ = std::fs::create_dir_all(&pkg_dir);
        let f1 = "package com.example\nfun foo() {}\n";
        let f2 = "package com.example\nfun bar() {}\n";
        std::fs::write(pkg_dir.join("A.kt"), f1).unwrap();
        std::fs::write(pkg_dir.join("B.kt"), f2).unwrap();

        let syms_a = extract_symbols_from_content("src/A.kt", f1);
        let syms_b = extract_symbols_from_content("src/B.kt", f2);

        for s in syms_a.iter().chain(syms_b.iter()) {
            conn.execute(
                "INSERT INTO symbols (id, name, kind, file_path, parent, start_line, end_line, signature)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![s.id, s.name, s.kind, if s.id.contains("A.kt") { "src/A.kt" } else { "src/B.kt" }, s.parent, s.start_line as i64, s.end_line as i64, s.signature],
            ).unwrap();
        }

        let _ = cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();
        let mut stmt = conn.prepare("SELECT edge_type, source_id, target_id FROM symbol_edges WHERE edge_type = 'shares_package'").unwrap();
        let rows: Vec<(String, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().flatten().collect();
        assert!(!rows.is_empty(), "Files sharing package com.example must produce shares_package edge");
        let _ = std::fs::remove_dir_all(&dir);
    }
