//! Handler for project_context(operation="data_flow") querying dataflow and dynamic dispatch links.

use rusqlite::Connection;
use std::path::Path;

pub fn handle_data_flow(project_path: &Path, query: &str) -> String {
    let query = query.trim();
    if query.is_empty() {
        return "Error: 'query' parameter is required for data_flow operation (pass a function, parameter, or variable name)".to_string();
    }

    let db_path = project_path.join(".agent-context").join("code_graph.db");
    if !db_path.exists() {
        return "No code graph index found. Run task_pipeline or project_context first to generate index.".to_string();
    }

    let conn = match Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(e) => return format!("Failed to open code graph database: {}", e),
    };

    let mut output = Vec::new();
    output.push(format!("# Data Flow & Dispatch Analysis for '{}'", query));

    let pattern = format!("%{}%", query);

    // 1. Forward Data Flow (Sinks: where does this variable/function output flow into?)
    let forward_flows = query_forward_flows(&conn, &pattern, query);
    output.push(format!("\n## Forward Data Flow (Sinks) [{}]", forward_flows.len()));
    if forward_flows.is_empty() {
        output.push("- No outgoing data flow links recorded for this symbol or variable.".to_string());
    } else {
        for f in &forward_flows {
            output.push(format!(
                "- `{}` → `{}` (param: `{}` [arg #{}], L{}, type: `{}`) [conf: {:.2}]",
                f.source_variable, f.callee_id, f.target_parameter, f.arg_index, f.call_line, f.flow_type, f.confidence
            ));
        }
    }

    // 2. Backward Data Flow (Sources: what values flow into this function/parameter?)
    let backward_flows = query_backward_flows(&conn, &pattern, query);
    output.push(format!("\n## Backward Data Flow (Sources) [{}]", backward_flows.len()));
    if backward_flows.is_empty() {
        output.push("- No incoming data flow links recorded for this target parameter.".to_string());
    } else {
        for b in &backward_flows {
            output.push(format!(
                "- `{}` passed `{}` from `{}` (param: `{}`, L{}) [conf: {:.2}]",
                b.caller_id, b.source_variable, b.callee_id, b.target_parameter, b.call_line, b.confidence
            ));
        }
    }

    // 3. Dynamic Dispatch & Trait Implementation Links
    let dispatch_links = query_dispatch_links(&conn, &pattern);
    output.push(format!("\n## Dynamic Dispatch & Trait Contracts [{}]", dispatch_links.len()));
    if dispatch_links.is_empty() {
        output.push("- No dynamic dispatch or trait implementation edges registered.".to_string());
    } else {
        for d in &dispatch_links {
            output.push(format!(
                "- `{}` —[{}]→ `{}` [conf: {:.2}]",
                d.source, d.edge_type, d.target, d.confidence
            ));
        }
    }

    output.join("\n")
}

struct FlowItem {
    caller_id: String,
    callee_id: String,
    source_variable: String,
    target_parameter: String,
    arg_index: usize,
    call_line: usize,
    flow_type: String,
    confidence: f64,
}

fn query_forward_flows(conn: &Connection, pattern: &str, exact_var: &str) -> Vec<FlowItem> {
    let mut items = Vec::new();
    let sql = "SELECT caller_symbol_id, callee_symbol_id, source_variable, target_parameter, arg_index, call_line, flow_type, confidence
               FROM data_flow_edges
               WHERE caller_symbol_id LIKE ?1 OR source_variable = ?2 OR source_variable LIKE ?1
               ORDER BY call_line ASC LIMIT 40";
    if let Ok(mut stmt) = conn.prepare(sql) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![pattern, exact_var], |row| {
            Ok(FlowItem {
                caller_id: row.get(0)?,
                callee_id: row.get(1)?,
                source_variable: row.get(2)?,
                target_parameter: row.get(3)?,
                arg_index: row.get(4)?,
                call_line: row.get(5)?,
                flow_type: row.get(6)?,
                confidence: row.get(7)?,
            })
        }) {
            for r in rows.flatten() {
                items.push(r);
            }
        }
    }
    items
}

fn query_backward_flows(conn: &Connection, pattern: &str, exact_param: &str) -> Vec<FlowItem> {
    let mut items = Vec::new();
    let sql = "SELECT caller_symbol_id, callee_symbol_id, source_variable, target_parameter, arg_index, call_line, flow_type, confidence
               FROM data_flow_edges
               WHERE callee_symbol_id LIKE ?1 OR target_parameter = ?2 OR target_parameter LIKE ?1
               ORDER BY call_line ASC LIMIT 40";
    if let Ok(mut stmt) = conn.prepare(sql) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![pattern, exact_param], |row| {
            Ok(FlowItem {
                caller_id: row.get(0)?,
                callee_id: row.get(1)?,
                source_variable: row.get(2)?,
                target_parameter: row.get(3)?,
                arg_index: row.get(4)?,
                call_line: row.get(5)?,
                flow_type: row.get(6)?,
                confidence: row.get(7)?,
            })
        }) {
            for r in rows.flatten() {
                items.push(r);
            }
        }
    }
    items
}

struct DispatchItem {
    source: String,
    target: String,
    edge_type: String,
    confidence: f64,
}

fn query_dispatch_links(conn: &Connection, pattern: &str) -> Vec<DispatchItem> {
    let mut items = Vec::new();
    let sql = "SELECT source_id, target_id, edge_type, confidence
               FROM symbol_edges
               WHERE (source_id LIKE ?1 OR target_id LIKE ?1)
                 AND edge_type IN ('implements_trait', 'implements_method', 'dynamic_dispatch')
               ORDER BY weight DESC LIMIT 30";
    if let Ok(mut stmt) = conn.prepare(sql) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![pattern], |row| {
            Ok(DispatchItem {
                source: row.get(0)?,
                target: row.get(1)?,
                edge_type: row.get(2)?,
                confidence: row.get(3)?,
            })
        }) {
            for r in rows.flatten() {
                items.push(r);
            }
        }
    }
    items
}
