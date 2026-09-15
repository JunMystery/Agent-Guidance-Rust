use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;

fn temp_dir_for_test(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ag_test_df_{}_{}_{}", name, std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros()));
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
fn test_dataflow_param_def_use_and_alias() {
    let mut conn = Connection::open_in_memory().unwrap();
    crate::context::db::schema::init_schema(&mut conn).unwrap();

    let dir = temp_dir_for_test("param_flow");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let callee_code = r#"
pub fn verify_token(target_token: &str, flag: bool) -> bool {
    !target_token.is_empty()
}
"#;
    let caller_code = r#"
pub fn handle_request(auth_token: &str) -> bool {
    let secret_alias = auth_token;
    verify_token(secret_alias, true)
}
"#;
    fs::write(src_dir.join("callee.rs"), callee_code).unwrap();
    fs::write(src_dir.join("caller.rs"), caller_code).unwrap();

    insert_test_file(&conn, "src/callee.rs");
    insert_test_file(&conn, "src/caller.rs");

    insert_test_symbol(&conn, "src/callee.rs::function::verify_token::L2", "verify_token", "function", "src/callee.rs", None, 2, 4);
    insert_test_symbol(&conn, "src/caller.rs::function::handle_request::L2", "handle_request", "function", "src/caller.rs", None, 2, 5);

    let _ = crate::context::indexer::cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();

    let mut stmt = conn.prepare(
        "SELECT caller_symbol_id, callee_symbol_id, source_variable, target_parameter, arg_index, flow_type
         FROM data_flow_edges"
    ).unwrap();

    let rows: Vec<(String, String, String, String, usize, String)> = stmt.query_map([], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
    }).unwrap().flatten().collect();

    assert!(!rows.is_empty(), "Data flow edge must be extracted from alias to verify_token call");
    let match_flow = rows.iter().find(|(_, _, src, tgt, idx, _)| {
        src == "auth_token" && tgt == "target_token" && *idx == 0
    });
    assert!(match_flow.is_some(), "Flow from auth_token -> target_token must be recorded with arg_index 0");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_dataflow_mcp_query_sinks_and_sources() {
    let dir = temp_dir_for_test("mcp_query");
    let _ = fs::create_dir_all(&dir);

    {
        let db = crate::context::db::CodeGraphDb::open_for_project(&dir).unwrap();
        insert_test_file(&db.conn, "src/auth.rs");
        insert_test_file(&db.conn, "src/api.rs");

        insert_test_symbol(&db.conn, "src/auth.rs::function::login::L1", "login", "function", "src/auth.rs", None, 1, 5);
        insert_test_symbol(&db.conn, "src/api.rs::function::dispatch::L1", "dispatch", "function", "src/api.rs", None, 1, 5);

        db.conn.execute(
            "INSERT INTO data_flow_edges (caller_symbol_id, callee_symbol_id, source_variable, target_parameter, arg_index, call_line, flow_type, confidence)
             VALUES ('src/auth.rs::function::login::L1', 'src/api.rs::function::dispatch::L1', 'raw_password', 'cred', 0, 3, 'alias_flow', 0.95)",
            [],
        ).unwrap();
    }

    let out_forward = crate::mcp::tools::context_dataflow::handle_data_flow(&dir, "raw_password");
    assert!(out_forward.contains("# Data Flow & Dispatch Analysis for 'raw_password'"), "Report title missing");
    assert!(out_forward.contains("Forward Data Flow (Sinks)"), "Forward sinks section missing");
    assert!(out_forward.contains("cred"), "Target parameter cred missing");
    assert!(out_forward.contains("dispatch"), "Callee sink dispatch missing");

    let out_backward = crate::mcp::tools::context_dataflow::handle_data_flow(&dir, "cred");
    assert!(out_backward.contains("Backward Data Flow (Sources)"), "Backward sources section missing");
    assert!(out_backward.contains("login"), "Caller source login missing");
    assert!(out_backward.contains("raw_password"), "Source variable raw_password missing");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_dataflow_ts_arrow_function_and_nested_scope() {
    let mut conn = Connection::open_in_memory().unwrap();
    crate::context::db::schema::init_schema(&mut conn).unwrap();

    let dir = temp_dir_for_test("ts_arrow");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let ts_code = r#"
export const submitData = (payload: string) => {
    dispatchAction(payload);
};

export function outerFunc(x: string) {
    function innerFunc(y: string) {
        sink(y);
    }
    innerFunc(x);
}
"#;
    let callee_code = r#"
export function dispatchAction(actionPayload: string) {}
export function sink(sinkVal: string) {}
"#;
    fs::write(src_dir.join("app.ts"), ts_code).unwrap();
    fs::write(src_dir.join("api.ts"), callee_code).unwrap();

    insert_test_file(&conn, "src/app.ts");
    insert_test_file(&conn, "src/api.ts");

    insert_test_symbol(&conn, "src/app.ts::function::submitData::L2", "submitData", "function", "src/app.ts", None, 2, 4);
    insert_test_symbol(&conn, "src/app.ts::function::outerFunc::L6", "outerFunc", "function", "src/app.ts", None, 6, 11);
    insert_test_symbol(&conn, "src/app.ts::function::innerFunc::L7", "innerFunc", "function", "src/app.ts", Some("outerFunc"), 7, 9);
    insert_test_symbol(&conn, "src/api.ts::function::dispatchAction::L2", "dispatchAction", "function", "src/api.ts", None, 2, 2);
    insert_test_symbol(&conn, "src/api.ts::function::sink::L3", "sink", "function", "src/api.ts", None, 3, 3);

    let _ = crate::context::indexer::cross_edges::resolve_all_cross_edges(&mut conn, &dir).unwrap();

    let mut stmt = conn.prepare("SELECT caller_symbol_id, callee_symbol_id, source_variable, target_parameter FROM data_flow_edges").unwrap();
    let rows: Vec<(String, String, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().flatten().collect();

    assert!(rows.iter().any(|(c, _, s, _)| c.contains("submitData") && s == "payload"), "Arrow function parameters must be tracked");
    assert!(!rows.iter().any(|(c, _, s, _)| c.contains("outerFunc") && s == "y"), "Nested function param 'y' must not leak into outerFunc");

    let _ = fs::remove_dir_all(&dir);
}
